
use tauri::{AppHandle, Emitter, Manager, State};


use crate::launch::java::JavaRuntimeSelector;
use crate::launch::runner::MinecraftRunner;

use super::*;

// ---------------------------------------------------------------------------
// Tier 2: Host for Friends (Co-Op Session) & P2P Mod Sync
// ---------------------------------------------------------------------------

#[tauri::command]
pub async fn start_coop_session(
    state: State<'_, LauncherState>,
    instance_id: String,
    world_name: String,
    preferred_p2p_port: Option<u16>,
) -> Result<crate::coop::CoopSessionInfo, String> {
    // 1. Stop any existing session first
    if let Some(shutdown) = state.coop_p2p_shutdown.lock().await.take() {
        let _ = shutdown.send(());
    }

    let inst = state
        .offline
        .load(&instance_id)
        .ok_or_else(|| "Instance not found".to_string())?;

    let game_dir = state.offline.instance_dir(&inst.id);
    let mods_dir = game_dir.join("mods");

    // 2. Generate P2P manifest from instance mods
    let manifest = crate::coop::generate_p2p_manifest(&inst, &mods_dir)
        .await
        .map_err(err_string)?;

    // 3. Start embedded Axum P2P server
    let p_port = preferred_p2p_port.unwrap_or(25566);
    let (bound_port, shutdown_tx) = crate::coop::start_p2p_server(mods_dir, manifest, p_port)
        .await
        .map_err(err_string)?;

    // 4. Perform UPnP NAT traversal (discovery timeout 2.5s)
    let upnp_status = crate::coop::open_upnp_ports(25565, bound_port).await;

    let join_code = crate::coop::generate_join_code();
    let session = crate::coop::CoopSessionInfo {
        instance_id: inst.id.clone(),
        instance_name: inst.name.clone(),
        world_name,
        join_code: join_code.clone(),
        game_port: 25565,
        p2p_port: bound_port,
        started_at: chrono::Utc::now().timestamp_millis(),
        active: true,
        upnp: upnp_status.clone(),
    };

    *state.coop_p2p_shutdown.lock().await = Some(shutdown_tx);
    *state.coop_session.lock().await = Some(session.clone());

    // 5. Register with Cloudflare Workers KV
    let host_address = upnp_status.external_ip.clone().unwrap_or_else(|| "auto".to_string());
    let worker_url = std::env::var("ZIRCON_COOP_WORKER_URL")
        .unwrap_or_else(|_| "https://zircon-coop.zirconmc.workers.dev".to_string());
    let rendezvous = crate::coop::CoopRendezvousSession {
        join_code: join_code.clone(),
        host: host_address,
        game_port: 25565,
        p2p_port: bound_port,
        instance_name: inst.name.clone(),
        mc_version: inst.minecraft_version.clone(),
        loader_type: inst.mod_loader.r#type.clone(),
        created_at: chrono::Utc::now().timestamp_millis(),
    };
    tokio::spawn(async move {
        let _ = crate::coop::register_coop_session(&worker_url, &rendezvous).await;
    });

    let beacon = crate::lan_discovery::LanBeaconPayload {
        zircon: true,
        host_username: inst.name.clone(),
        world_name: session.world_name.clone(),
        mc_version: inst.minecraft_version.clone(),
        loader_type: inst.mod_loader.r#type.clone(),
        game_port: session.game_port,
        p2p_port: Some(session.p2p_port),
        join_code: Some(session.join_code.clone()),
    };
    tokio::spawn(async move {
        let _ = crate::lan_discovery::LanDiscoveryEngine::broadcast_beacon(beacon).await;
    });

    Ok(session)
}

#[tauri::command]
pub async fn stop_coop_session(
    state: State<'_, LauncherState>,
) -> Result<(), String> {
    if let Some(shutdown) = state.coop_p2p_shutdown.lock().await.take() {
        let _ = shutdown.send(());
    }
    let prev = state.coop_session.lock().await.take();
    if let Some(sess) = prev {
        let worker_url = std::env::var("ZIRCON_COOP_WORKER_URL")
            .unwrap_or_else(|_| "https://zircon-coop.zirconmc.workers.dev".to_string());
        let join_code = sess.join_code.clone();
        tokio::spawn(async move {
            let _ = crate::coop::delete_coop_session(&worker_url, &join_code).await;
        });
        tokio::spawn(async move {
            crate::coop::close_upnp_ports(sess.game_port, sess.p2p_port).await;
        });
    }
    Ok(())
}

#[tauri::command]
pub async fn get_coop_session_status(
    state: State<'_, LauncherState>,
) -> Result<Option<crate::coop::CoopSessionInfo>, String> {
    Ok(state.coop_session.lock().await.clone())
}

#[tauri::command]
pub async fn resolve_coop_code(
    code_or_address: String,
) -> Result<crate::coop::CoopRendezvousSession, String> {
    let (code, direct_host, game_port, p2p_port) = crate::coop::parse_code_or_address(&code_or_address);
    if let Some(host) = direct_host {
        return Ok(crate::coop::CoopRendezvousSession {
            join_code: code_or_address,
            host,
            game_port,
            p2p_port,
            instance_name: "Direct Co-Op Session".to_string(),
            mc_version: "1.21.1".to_string(),
            loader_type: "fabric".to_string(),
            created_at: chrono::Utc::now().timestamp_millis(),
        });
    }

    let join_code = code.ok_or_else(|| "Invalid Join Code or address".to_string())?;
    let worker_url = std::env::var("ZIRCON_COOP_WORKER_URL")
        .unwrap_or_else(|_| "https://zircon-coop.zirconmc.workers.dev".to_string());
    crate::coop::resolve_coop_session(&worker_url, &join_code).await.map_err(err_string)
}

#[tauri::command]
pub async fn coop_preflight(
    state: State<'_, LauncherState>,
    host_address: String,
    p2p_port: u16,
    game_port: u16,
    target_instance_id: Option<String>,
) -> Result<crate::coop::P2PPreflightResult, String> {
    let game_dir = if let Some(ref id) = target_instance_id {
        resolve_instance_game_dir(&state, id)?
    } else {
        crate::paths::offline_instances_dir().join("_coop_guest_staging")
    };
    let instance_mods = game_dir.join("mods");
    let cache_dir = crate::paths::mods_cache_dir();
    let allow_unverified = state.settings.lock().map(|s| s.allow_unverified_p2p_mods).unwrap_or(false);

    crate::coop::preflight_p2p_sync(
        &state.http,
        &host_address,
        p2p_port,
        game_port,
        &instance_mods,
        &cache_dir,
        allow_unverified,
    )
    .await
    .map_err(err_string)
}

#[tauri::command]
pub async fn coop_sync_mods(
    app: AppHandle,
    state: State<'_, LauncherState>,
    host_address: String,
    p2p_port: u16,
    missing_mods: Vec<crate::coop::P2PModEntry>,
    approved_custom_sha1s: Vec<String>,
    target_instance_id: Option<String>,
) -> Result<crate::coop::P2PSyncResult, String> {
    let game_dir = if let Some(ref id) = target_instance_id {
        resolve_instance_game_dir(&state, id)?
    } else {
        crate::paths::offline_instances_dir().join("_coop_guest_staging")
    };
    let instance_mods = game_dir.join("mods");
    let cache_dir = crate::paths::mods_cache_dir();
    let allow_unverified = state.settings.lock().map(|s| s.allow_unverified_p2p_mods).unwrap_or(false);
    let approved_set: std::collections::HashSet<String> = approved_custom_sha1s.into_iter().collect();
    let host_p2p_url = format!("http://{host_address}:{p2p_port}");

    let listener = UiProgressListener { app };
    crate::coop::execute_p2p_sync(
        &state.http,
        &host_p2p_url,
        &missing_mods,
        &approved_set,
        &instance_mods,
        &cache_dir,
        allow_unverified,
        Some(&listener),
    )
    .await
    .map_err(err_string)
}



// =========================================================================
// Couch Co-Op Split-Screen Commands
// =========================================================================

#[tauri::command]
pub async fn launch_couch_coop_session(
    app: AppHandle,
    state: State<'_, LauncherState>,
    request: crate::split_screen::SplitScreenSessionRequest,
) -> Result<crate::split_screen::SplitScreenStatus, String> {
    if request.players.len() < 2 || request.players.len() > 4 {
        return Err("Split-screen requires between 2 and 4 players.".to_string());
    }

    if state.running_game.lock().await.is_some() {
        return Err("A game is already running — please close it first.".to_string());
    }
    if state.couch_coop_session.lock().await.is_some() {
        return Err("A split-screen session is already running — please stop it first.".to_string());
    }

    let instance = state
        .offline
        .load(&request.instance_id)
        .ok_or_else(|| "Instance not found".to_string())?;

    if !zircon_core::api::versions::is_supported_mc_version(&instance.minecraft_version) {
        return Err(format!(
            "Minecraft {} is not supported for split-screen.",
            instance.minecraft_version
        ));
    }

    let listener = UiProgressListener { app: app.clone() };
    let required_java =
        JavaRuntimeSelector::get_required_java_major_version(&instance.minecraft_version);

    let (custom_jvm, java_override) = {
        let s = state.settings.lock().unwrap();
        (s.custom_jvm_args.clone(), s.java_path_override.clone())
    };
    let java_override_path = java_override.as_deref().map(std::path::Path::new);
    let client_jar_override = state.offline.resolve_custom_client_jar(&instance);
    let client_jar_path = client_jar_override.as_deref();

    emit_status(&app, "Resolving Minecraft classpath and assets for Couch Co-Op...");
    let launch_data = state
        .classpath
        .resolve_with_progress_and_override(
            &instance.minecraft_version,
            &instance.mod_loader,
            required_java,
            java_override_path,
            client_jar_path,
            Some(&listener),
        )
        .await
        .map_err(|e| format!("Failed to resolve classpath: {e}"))?;

    let base_dir = state.offline.instance_dir(&instance.id);
    std::fs::create_dir_all(&base_dir).map_err(|e| format!("Failed to create base dir: {e}"))?;

    let java_major = crate::launch::java::get_java_home_major_version(&launch_data.java_home)
        .or(Some(required_java));
    let is_vr = crate::launch::java::is_vr_instance(&base_dir, Some(&launch_data.version_name));

    let (screen_w, screen_h) = crate::launch::window_tracker::get_primary_screen_resolution();
    let rects = crate::split_screen::calculate_window_rects(
        request.layout,
        screen_w,
        screen_h,
        request.players.len(),
    );

    emit_status(&app, "Configuring Host instance options...");
    crate::split_screen::prepare_host_workspace(&base_dir, &request.players[0], rects[0].2, rects[0].3)
        .map_err(|e| format!("Failed to prepare host workspace: {e}"))?;

    emit_status(&app, "Provisioning lean guest workspaces...");
    let mut guest_dirs = Vec::new();
    for i in 1..request.players.len() {
        let guest_dir = crate::split_screen::prepare_guest_workspace(
            &base_dir,
            &request.players[i],
            rects[i].2,
            rects[i].3,
        )
        .map_err(|e| format!("Failed to prepare guest {} workspace: {e}", i + 1))?;
        guest_dirs.push(guest_dir);
    }

    // Verify all players have a distinct, non-empty Microsoft account UUID
    let mut seen_uuids = std::collections::HashSet::new();
    for (idx, p) in request.players.iter().enumerate() {
        let clean_uuid = p.account_uuid.trim();
        if clean_uuid.is_empty() {
            return Err(format!("Player {} must be signed in with a Microsoft account.", idx + 1));
        }
        if !seen_uuids.insert(clean_uuid.to_lowercase()) {
            return Err(format!(
                "Account '{}' is assigned to multiple players. Every player must use a unique Microsoft account.",
                p.name
            ));
        }
    }

    // Load and refresh sessions for every player (ownership verification)
    let mut authenticated_sessions = Vec::new();
    for p in &request.players {
        let mut session = state
            .accounts
            .load_session_for_uuid(&p.account_uuid)
            .ok_or_else(|| {
                format!(
                    "No saved credentials found for {} ({}). Please sign in with Microsoft.",
                    p.name, p.account_uuid
                )
            })?;

        if session.is_expired() {
            emit_status(&app, format!("Refreshing Microsoft token for {}...", p.name));
            match state.auth.refresh(&session).await {
                Ok(fresh) => {
                    let _ = state.accounts.store_session_credentials(&fresh);
                    session = fresh;
                }
                Err(e) => {
                    return Err(format!(
                        "Microsoft token for '{}' expired and could not be refreshed ({e}). Please sign in again.",
                        p.name
                    ));
                }
            }
        }

        if session.user_type != "msa" || session.access_token.trim().is_empty() || session.access_token == "0" {
            return Err(format!(
                "Account '{}' is not a valid Microsoft session. Please sign in with Microsoft.",
                p.name
            ));
        }
        authenticated_sessions.push(session);
    }

    // Resolve optional server address (for online server companion / quickPlay)
    let (server_ip, server_port) = match request.server_address.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
        Some(addr) => {
            if let Some((h, p_str)) = addr.split_once(':') {
                (h.to_string(), p_str.parse::<i32>().unwrap_or(25565))
            } else {
                (addr.to_string(), 25565)
            }
        }
        None => (String::new(), 0),
    };

    let output = game_output_emitter(&app);
    let mut children = Vec::new();
    let mut pids = Vec::new();
    let mut snap_targets = Vec::new();

    // 1. Launch Host (Player 0)
    emit_status(&app, format!("Launching Host viewport (Player 1: {})...", request.players[0].name));
    let host_ram = request.players[0].memory_gb;
    let host_java_args = override_heap_with_context(
        &instance.java_args,
        host_ram,
        custom_jvm.as_deref(),
        java_major,
        is_vr,
    );
    let host_disp = crate::launch::runner::LaunchDisplayOptions {
        width: rects[0].2 as u32,
        height: rects[0].3 as u32,
        fullscreen: false,
    };

    let host_child = MinecraftRunner
        .launch_with_options(
            &launch_data,
            &authenticated_sessions[0],
            Some(&host_java_args),
            &base_dir,
            &server_ip,
            server_port,
            Some(host_disp),
            Some(output.clone()),
        )
        .await
        .map_err(|e| format!("Failed to launch Host instance: {e}"))?;

    let host_pid = host_child.id().unwrap_or(0);
    pids.push(host_pid);
    snap_targets.push((host_pid, rects[0].0, rects[0].1, rects[0].2, rects[0].3));
    children.push(host_child);

    // 2. Launch Guest viewports (Player 1..N) with staggered delays
    for i in 1..request.players.len() {
        tokio::time::sleep(std::time::Duration::from_millis(2000)).await;
        let p_cfg = &request.players[i];
        let guest_session = &authenticated_sessions[i];
        emit_status(&app, format!("Launching Viewport {} ('{}')...", i + 1, p_cfg.name));
        let guest_ram = p_cfg.memory_gb;
        let guest_java_args = override_heap_with_context(
            &instance.java_args,
            guest_ram,
            custom_jvm.as_deref(),
            java_major,
            is_vr,
        );
        let guest_disp = crate::launch::runner::LaunchDisplayOptions {
            width: rects[i].2 as u32,
            height: rects[i].3 as u32,
            fullscreen: false,
        };
        let gdir = &guest_dirs[i - 1];

        let guest_child = MinecraftRunner
            .launch_with_options(
                &launch_data,
                guest_session,
                Some(&guest_java_args),
                gdir,
                &server_ip,
                server_port,
                Some(guest_disp),
                Some(output.clone()),
            )
            .await
            .map_err(|e| format!("Failed to launch guest {}: {e}", p_cfg.name))?;

        let guest_pid = guest_child.id().unwrap_or(0);
        pids.push(guest_pid);
        snap_targets.push((guest_pid, rects[i].0, rects[i].1, rects[i].2, rects[i].3));
        children.push(guest_child);
    }

    crate::launch::window_tracker::spawn_splitscreen_snapper(snap_targets);
    crate::launch::window_tracker::clear_always_on_top(&app);

    let status = crate::split_screen::SplitScreenStatus {
        is_active: true,
        layout: Some(request.layout),
        player_count: pids.len(),
        pids: pids.clone(),
    };

    *state.couch_coop_session.lock().await = Some(crate::split_screen::CouchCoopSession {
        instance_id: request.instance_id.clone(),
        layout: request.layout,
        pids: pids.clone(),
        children,
    });

    // Spawn watcher to track when processes exit
    let app_clone = app.clone();
    tauri::async_runtime::spawn(async move {
        loop {
            tokio::time::sleep(std::time::Duration::from_millis(1500)).await;
            let st = app_clone.state::<LauncherState>();
            let mut guard = st.couch_coop_session.lock().await;
            if let Some(session) = guard.as_mut() {
                let mut all_exited = true;
                for child in session.children.iter_mut() {
                    match child.try_wait() {
                        Ok(Some(_)) => {}
                        Ok(None) => {
                            all_exited = false;
                        }
                        Err(_) => {}
                    }
                }
                if all_exited {
                    *guard = None;
                    let _ = app_clone.emit("couch-coop-stopped", ());
                    let _ = app_clone.emit("launch-status", "Couch Co-Op session finished.");
                    break;
                }
            } else {
                break;
            }
        }
    });

    let _ = app.emit("couch-coop-started", &status);
    emit_status(&app, "Couch Co-Op Split-Screen session active!");

    Ok(status)
}

#[tauri::command]
pub async fn stop_couch_coop_session(
    app: AppHandle,
    state: State<'_, LauncherState>,
) -> Result<(), String> {
    let mut guard = state.couch_coop_session.lock().await;
    if let Some(mut session) = guard.take() {
        for child in session.children.iter_mut() {
            let _ = child.kill().await;
        }
        let _ = app.emit("couch-coop-stopped", ());
        emit_status(&app, "Couch Co-Op Split-Screen session stopped.");
    }
    Ok(())
}

#[tauri::command]
pub async fn get_couch_coop_status(
    state: State<'_, LauncherState>,
) -> Result<crate::split_screen::SplitScreenStatus, String> {
    let guard = state.couch_coop_session.lock().await;
    if let Some(session) = guard.as_ref() {
        Ok(crate::split_screen::SplitScreenStatus {
            is_active: true,
            layout: Some(session.layout),
            player_count: session.pids.len(),
            pids: session.pids.clone(),
        })
    } else {
        Ok(crate::split_screen::SplitScreenStatus {
            is_active: false,
            layout: None,
            player_count: 0,
            pids: Vec::new(),
        })
    }
}

#[tauri::command]
pub fn get_couch_coop_screen_resolution() -> (i32, i32) {
    crate::launch::window_tracker::get_primary_screen_resolution()
}

/// Returns the current active telemetry snapshot for the running game or host baseline.
#[tauri::command]
pub async fn get_mission_control_telemetry(
    state: State<'_, LauncherState>,
) -> Result<Option<crate::mission_control::MissionControlTelemetry>, String> {
    if let Some(t) = state.telemetry.get_latest() {
        return Ok(Some(t));
    }

    use sysinfo::System;
    let mut sys = System::new();
    sys.refresh_memory();
    let sys_mem_used_mb = sys.used_memory() / (1024 * 1024);
    let sys_mem_total_mb = sys.total_memory() / (1024 * 1024);
    let detected_gpu = crate::launch::gpu::get_gpu_system_info(crate::launch::gpu::GpuPreference::default());
    let gpu_name = detected_gpu.active_gpu_name.or_else(|| {
        detected_gpu.adapters.iter().find(|a| a.is_dedicated).or_else(|| detected_gpu.adapters.first()).map(|a| a.name.clone())
    });

    Ok(Some(crate::mission_control::MissionControlTelemetry {
        running: false,
        pid: 0,
        instance_id: None,
        instance_name: "Host Standby".to_string(),
        uptime_secs: 0,
        memory_used_mb: sys_mem_used_mb,
        memory_allocated_mb: sys_mem_total_mb,
        memory_percent: ((sys_mem_used_mb as f32 / sys_mem_total_mb.max(1) as f32) * 100.0).clamp(0.0, 100.0),
        cpu_percent: 0.0,
        system_cpu_percent: 4.5,
        system_memory_used_mb: sys_mem_used_mb,
        system_memory_total_mb: sys_mem_total_mb,
        net_rx_kbps: 0.0,
        net_tx_kbps: 0.0,
        gpu_name,
        gpu_usage_percent: Some(6.0),
        mspt: 0.0,
        tps: 20.0,
        error_count: 0,
        warning_count: 0,
        gamepads: crate::mission_control::query_gamepad_telemetry(),
        timestamp_ms: chrono::Utc::now().timestamp_millis(),
    }))
}

/// Returns real-time battery and connection status of connected gamepads.
#[tauri::command]
pub async fn get_gamepad_telemetry() -> Result<Vec<crate::mission_control::GamepadTelemetry>, String> {
    Ok(crate::mission_control::query_gamepad_telemetry())
}

/// Queries all discovered local Wi-Fi / LAN servers.
#[tauri::command]
pub async fn get_discovered_lan_servers(
    state: State<'_, LauncherState>,
) -> Result<Vec<crate::lan_discovery::DiscoveredLanServer>, String> {
    let guard = state.lan_discovery.lock().await;
    if let Some(engine) = guard.as_ref() {
        Ok(engine.get_servers().await)
    } else {
        Ok(Vec::new())
    }
}

/// Broadcasts a Zircon enhanced LAN beacon to announce an open world.
#[tauri::command]
pub async fn broadcast_lan_beacon(
    payload: crate::lan_discovery::LanBeaconPayload,
) -> Result<(), String> {
    crate::lan_discovery::LanDiscoveryEngine::broadcast_beacon(payload).await
}


