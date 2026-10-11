use std::path::PathBuf;
use std::sync::atomic::Ordering;
use std::time::Duration;

use serde::Serialize;
use sha2::{Digest, Sha256};
use tauri::{AppHandle, Emitter, Manager, State};

use zircon_core::crypto::signing;
use zircon_core::model::{BillOfMaterials, ModLoaderInfo};

use crate::error::LauncherError;
use crate::launch::java::JavaRuntimeSelector;
use crate::launch::runner::MinecraftRunner;
use crate::offline::OfflineInstance;
use crate::pack_selection::PackSelection;
use crate::servers;

use super::*;

// ---------------------------------------------------------------------------
// Online launch flow
// ---------------------------------------------------------------------------

/// Launches the game against a saved server: BOM fetch, pack sync, classpath
/// resolution, mod sync, pre-join ticket and process spawn. Progress is streamed
/// over the `launch-status` / `launch-progress` events.
/// `use_https` selects the scheme for the launcher's HTTP calls to the server.
#[tauri::command]
pub async fn launch_server(
    app: AppHandle,
    state: State<'_, LauncherState>,
    address: String,
    name: Option<String>,
    install_recommended_packs: bool,
    use_https: bool,
) -> Result<(), String> {
    crate::launch::window_tracker::set_always_on_top(&app);
    state.launch_cancellation.reset();

    let cancel_watcher = async {
        while !state.launch_cancellation.is_aborted() {
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    };

    let launch_future = run_online_flow(
        &app,
        &state,
        &address,
        name.as_deref(),
        install_recommended_packs,
        use_https,
    );

    tokio::select! {
        res = launch_future => {
            if res.is_err() {
                crate::launch::window_tracker::clear_always_on_top(&app);
            }
            res.map_err(err_string)
        }
        _ = cancel_watcher => {
            crate::launch::window_tracker::clear_always_on_top(&app);
            let _ = app.emit("launch-status", "Launch cancelled.");
            let _ = app.emit(
                "game-status",
                serde_json::json!({ "running": false, "label": "", "code": 0 }),
            );
            Err("Launch cancelled by user.".to_string())
        }
    }
}

pub(crate) async fn run_online_flow(
    app: &AppHandle,
    state: &LauncherState,
    address: &str,
    name: Option<&str>,
    install_recommended_packs: bool,
    use_https: bool,
) -> Result<(), LauncherError> {
    state.launch_cancellation.reset();
    if state.running_game.lock().await.is_some() {
        return Err(LauncherError::InvalidInput(
            "A game is already running — stop it first.".to_string(),
        ));
    }

    // --- session ---
    let mut session_guard = state.session.lock().await;
    let session = match session_guard.as_ref() {
        Some(session) if !session.is_expired() => session.clone(),
        Some(session) => {
            emit_status(app, "Renewing session...");
            match state.auth.refresh(session).await {
                Ok(fresh) => {
                    *session_guard = Some(fresh.clone());
                    fresh
                }
                Err(_) => {
                    emit_status(app, "Session expired — please sign in again.");
                    return Err(LauncherError::Auth(
                        "Session expired; sign in again from the launcher.".to_string(),
                    ));
                }
            }
        }
        None => {
            emit_status(app, "Sign in required before launching.");
            return Err(LauncherError::Auth(
                "Not signed in — log in first.".to_string(),
            ));
        }
    };
    drop(session_guard);

    if !state.auth.check_entitlements(&session.access_token).await {
        return Err(LauncherError::Auth(
            "Minecraft rejected this session — the account does not own \
             Minecraft (Java Edition) or the session was revoked. Please sign \
             in again with an account that owns the game."
                .to_string(),
        ));
    }

    // --- server + game dir ---
    let (host, port) = servers::parse_server_address(address);
    // IPv6 literals need square brackets in URLs and quick-play targets.
    let url_host = servers::format_host(&host);
    let mut base_url = server_base_url(&url_host, port, use_https)?;
    let candidates = candidate_base_urls(&url_host, port, use_https);
    for candidate in &candidates {
        if fetch_wrapper_status(&state.http, candidate).await.is_some() {
            base_url = candidate.clone();
            break;
        }
    }
    emit_status(app, format!("Server: {base_url}"));
    let game_dir = servers::instance_game_dir(&host, port);
    std::fs::create_dir_all(&game_dir)?;

    // --- wake up a sleeping Zircon instance (idle shutdown) ---
    // If the server is asleep or waking, send the wakeup call now so it can
    // boot in the background while we fetch the BOM, verify trust, and sync
    // packs/mods.
    let wake_outcome =
        wake_if_needed(&state.http, app, &base_url, &host, port, &state.launch_cancellation).await?;
    state.launch_cancellation.guard_active()?;

    // A player is committed to joining: keep the server awake while the rest
    // of the flow runs (BOM, pack sync, Java/classpath, mod sync — any of
    // which can take minutes on a heavy pack), so the server cannot fall
    // asleep under the player between wakeup and the game connecting. The
    // guard aborts the heartbeat on every exit path.
    let _heartbeat = if wake_outcome.wrapper_present {
        Some(JoinIntentHeartbeat(Some(tauri::async_runtime::spawn(
            join_intent_heartbeat(
                state.http.clone(),
                base_url.clone(),
                host.clone(),
                port,
                session.username.clone(),
                session.uuid.clone(),
            ),
        ))))
    } else {
        None
    };

    // --- BOM ---
    let bom = {
        let mut bom_res = fetch_bom(&state.http, &base_url).await;
        if bom_res.is_err() {
            for candidate in &candidates {
                if candidate != &base_url {
                    if let Ok(b) = fetch_bom(&state.http, candidate).await {
                        base_url = candidate.clone();
                        bom_res = Ok(b);
                        break;
                    }
                }
            }
        }
        bom_res?
    };
    state.launch_cancellation.guard_active()?;

    // --- BOM trust (TOFU pinning + Ed25519 attestation) ---
    // Nothing is downloaded or launched until the mod list itself is trusted:
    // pin the server public key on first contact, verify the Ed25519
    // signature, and — when the server presents a *different* key than the one
    // pinned — ask the player before accepting the rotation instead of
    // crashing or silently trusting it.
    let pinned = servers::pinned_public_key(address);
    match evaluate_bom_trust(&bom, pinned.as_deref())? {
        BomTrustOutcome::NoAttestation => {
            emit_status(
                app,
                "Server does not sign its BOM — continuing with per-file hash \
                 verification only.",
            );
        }
        BomTrustOutcome::Verified(key) => {
            if pinned.as_deref() != Some(key.as_str()) {
                servers::pin_public_key(address, &key);
                emit_status(app, "Trusted server public key on first use (TOFU).");
            }
        }
        BomTrustOutcome::KeyMismatch {
            received,
            pinned: old,
        } => {
            // Server reinstall or possible takeover: show the fingerprint
            // delta and require explicit player approval. A timeout or any
            // rejection aborts the launch — never auto-accept a key change.
            let request_id = state.next_key_prompt_id.fetch_add(1, Ordering::SeqCst);
            let (tx, rx) = tokio::sync::oneshot::channel::<bool>();
            state.key_prompts.lock().await.insert(request_id, tx);

            let _ = app.emit(
                "server-key-mismatch",
                KeyMismatchPrompt {
                    request_id,
                    server_address: address.to_string(),
                    old_fingerprint: compute_key_fingerprint(&old),
                    new_fingerprint: compute_key_fingerprint(&received),
                },
            );

            let accepted = match tokio::time::timeout(Duration::from_secs(60), rx).await {
                Ok(Ok(true)) => true,
                _ => false,
            };

            if !accepted {
                return Err(LauncherError::Security(
                    "Host key verification failed: server identity changed and was \
                     rejected."
                        .to_string(),
                ));
            }

            // The player explicitly trusted the rotation.
            servers::pin_public_key(address, &received);
            emit_status(app, "New server identity approved by the player.");
            tracing::warn!("Player approved key rotation for {address} to {received}");
        }
    }

    if let Some(title) = bom.server_title.as_deref().filter(|t| !t.trim().is_empty()) {
        servers::record_played(title.trim(), address);
    }

    // --- pack selection & shader opt-in ---
    let mut selection = PackSelection::load(&game_dir);

    // Shader opt-in: when the server offers shaders and the player has not
    // remembered a choice for this server yet, ask before installing/downloading
    // them (the answer can be remembered for future connections). People without
    // powerful GPUs can decline. The popup appears even when a shaderpack was
    // previously active, so nobody gets shaders applied without being asked.
    if !bom.shaderpacks.is_empty() {
        if install_recommended_packs {
            // Programmatic callers can opt in without the dialog.
            apply_shader_choice(&mut selection, &bom, true);
        } else if selection.remember_shaders_choice {
            // The player answered before — reuse the remembered answer.
            let auto_enabled = selection.shaders_auto_enabled;
            apply_shader_choice(&mut selection, &bom, auto_enabled);
        } else {
            emit_status(
                app,
                format!("{} offers shaders — asking player...", url_host),
            );
            let request_id = state.next_shader_request_id.fetch_add(1, Ordering::SeqCst);
            let (tx, rx) = tokio::sync::oneshot::channel::<ShaderChoice>();
            state.shader_requests.lock().await.insert(request_id, tx);
            let shader_title = bom
                .shaderpacks
                .first()
                .and_then(|p| p.title.clone())
                .or_else(|| bom.shaderpacks.first().map(|p| p.filename.clone()))
                .unwrap_or_default();
            let shader_author = bom
                .shaderpacks
                .first()
                .and_then(|p| p.author.clone())
                .unwrap_or_default();
            let _ = app.emit(
                "shader-request",
                serde_json::json!({
                    "requestId": request_id,
                    "server": format!("{url_host}:{port}"),
                    "shaderName": shader_title,
                    "shaderAuthor": shader_author,
                }),
            );
            // Wait for the webview's answer; a closed window or a long pause
            // falls back to "no shaders".
            let choice = match tokio::time::timeout(Duration::from_secs(120), rx).await {
                Ok(Ok(choice)) => choice,
                _ => ShaderChoice {
                    enabled: false,
                    remember: false,
                },
            };
            if choice.remember {
                selection.remember_shaders_choice = true;
                selection.shaders_auto_enabled = choice.enabled;
            }
            apply_shader_choice(&mut selection, &bom, choice.enabled);
        }
        selection.save(&game_dir);
    }

    // --- pack sync ---
    emit_status(app, "Checking server shaderpacks & texture packs...");
    let pack_listener = UiPackListener { app: app.clone() };

    // Only download server shaderpacks if shaders are enabled by the player.
    let effective_bom = if selection.shaders_enabled {
        bom.clone()
    } else {
        let mut b = bom.clone();
        b.shaderpacks.clear();
        b
    };

    state
        .pack_sync
        .sync(
            &effective_bom,
            &base_url,
            &game_dir,
            &selection
                .locally_added_shaderpacks
                .iter()
                .cloned()
                .collect::<Vec<_>>(),
            &selection
                .locally_added_resourcepacks
                .iter()
                .cloned()
                .collect::<Vec<_>>(),
            Some(&pack_listener),
        )
        .await;

    // Drop selections pointing at packs the server no longer offers.
    if let Some(active) = selection.active_shaderpack.clone() {
        if !game_dir.join("shaderpacks").join(&active).is_file() {
            selection.active_shaderpack = None;
        }
    }

    // Ensure all server-provided resourcepacks present on disk that pass zero-trust validation are enabled
    let guard = zircon_core::archive::limits::ArchiveGuard::default();
    for pack in &bom.resourcepacks {
        let file_path = game_dir.join("resourcepacks").join(&pack.filename);
        if file_path.is_file() {
            let is_prescanned = pack
                .origin
                .as_deref()
                .map(|o| o.eq_ignore_ascii_case("modrinth") || o.eq_ignore_ascii_case("curseforge"))
                .unwrap_or(false);
            let is_safe = match std::fs::File::open(&file_path) {
                Ok(f) => {
                    if is_prescanned {
                        zircon_core::security::pack_validator::validate_pack_archive_safety(f, &guard).is_ok()
                    } else {
                        zircon_core::security::pack_validator::validate_pack_archive(f, &guard).is_ok()
                    }
                }
                Err(_) => false,
            };
            if is_safe {
                if !selection.rejected_resourcepacks.contains(&pack.filename)
                    && !selection.active_resourcepacks.contains(&pack.filename)
                {
                    selection.active_resourcepacks.push(pack.filename.clone());
                }
            } else {
                let _ = std::fs::remove_file(&file_path);
                selection.active_resourcepacks.retain(|n| n != &pack.filename);
            }
        }
    }

    let present: Vec<String> = selection
        .active_resourcepacks
        .iter()
        .filter(|name| game_dir.join("resourcepacks").join(name).is_file())
        .cloned()
        .collect();
    selection.active_resourcepacks = present;
    selection.save(&game_dir);

    // --- version sanity check ---
    if !zircon_core::api::versions::is_supported_mc_version(&bom.minecraft_version) {
        return Err(LauncherError::InvalidInput(format!(
            "Minecraft {} is not supported. The minimum supported version is 1.7 (e.g. 1.7.10). Older versions like 1.6 are not supported.",
            bom.minecraft_version
        )));
    }

    // --- classpath / Java ---
    let listener = UiProgressListener { app: app.clone() };
    let required_java =
        JavaRuntimeSelector::get_required_java_major_version(&bom.minecraft_version);
    let (memory_gb, custom_jvm, java_override, display_opts) = {
        let s = state.settings.lock().unwrap();
        let disp = crate::launch::runner::LaunchDisplayOptions {
            width: s.window_width,
            height: s.window_height,
            fullscreen: s.start_fullscreen,
        };
        let c_jvm: Option<String> = s.custom_jvm_args.clone();
        let j_override: Option<String> = s.java_path_override.clone();
        (s.memory_gb, c_jvm, j_override, disp)
    };
    let java_override_path: Option<&std::path::Path> =
        java_override.as_deref().map(std::path::Path::new);

    let loader = bom
        .mod_loader
        .clone()
        .unwrap_or_else(|| ModLoaderInfo::new("vanilla", "", None));
    let launch_data = state
        .classpath
        .resolve_with_progress_and_override(
            &bom.minecraft_version,
            &loader,
            required_java,
            java_override_path,
            None,
            Some(&listener),
        )
        .await?;

    // --- mod sync ---
    emit_status(app, "Checking mod hashes & synchronizing staging area...");
    let keep_mods: Vec<String> = selection.locally_added_mods.iter().cloned().collect();
    let disabled_mods: Vec<String> = selection.disabled_mods.iter().cloned().collect();
    let sync_result = state
        .sync_engine
        .sync_with_bom(&bom, &base_url, &game_dir, &keep_mods, &disabled_mods, Some(&listener))
        .await?;
    state.launch_cancellation.guard_active()?;
    if sync_result.aborted {
        return Err(LauncherError::InvalidInput(
            sync_result
                .abort_reason
                .unwrap_or_else(|| "Mod sync aborted".to_string()),
        ));
    }

    // --- wait for server boot if needed ---
    // If the server was asleep or waking when we started, ensure it has finished
    // booting before we send the final pre-join intent and launch the client.
    if wake_outcome.needs_wait {
        wait_for_server_boot(app, &host, port, &state.launch_cancellation).await?;
        state.launch_cancellation.guard_active()?;
    }

    // --- pre-join intent (final refresh before spawn) ---
    // The last registration restarts the hold and ticket TTL, which then cover
    // the Minecraft boot window (the heartbeat is aborted when the flow
    // exits). A 409 means the server was stopped manually mid-launch
    // (maintenance) — abort before booting Minecraft into a dead server
    // instead of ghost-connecting.
    emit_status(app, "Registering pre-join intent with Zircon server...");
    let intent_status = register_join_intent(
        &state.http,
        &base_url,
        &session.username,
        &session.uuid,
        &host,
        port,
    )
    .await;
    if let Ok(409) = intent_status {
        return Err(LauncherError::InvalidInput(
            "The server was stopped (not in sleep mode) while launching — start it \
             from the admin panel and try again."
                .to_string(),
        ));
    }

    // --- spawn the game ---
    emit_status(app, "Starting Minecraft process...");
    state.launch_cancellation.guard_active()?;
    let output = game_output_emitter(app);
    let canonical_name = if let Some(title) = bom.server_title.as_deref().filter(|t| !t.trim().is_empty()) {
        title.trim()
    } else {
        name.as_deref().filter(|n| !n.trim().is_empty()).unwrap_or(&address)
    };
    let server_addr = format!("{url_host}:{port}");
    let _ = crate::launch::servers_dat::ensure_server_entry(
        &game_dir,
        canonical_name,
        &server_addr,
    );

    let java_major = crate::launch::java::get_java_home_major_version(&launch_data.java_home)
        .or(Some(required_java));
    let is_vr = crate::launch::java::is_vr_instance(&game_dir, Some(&launch_data.version_name));
    let java_args = override_heap_with_context("", memory_gb, custom_jvm.as_deref(), java_major, is_vr);
    let child = MinecraftRunner
        .launch_with_options(
            &launch_data,
            &session,
            Some(&java_args),
            &game_dir,
            &url_host,
            port as i32,
            Some(display_opts),
            Some(output),
        )
        .await?;


    let pid = child.id().unwrap_or(0);
    let id = state.next_game_id.fetch_add(1, Ordering::SeqCst);
    *state.running_game.lock().await = Some(RunningGame {
        id,
        label: format!("{url_host}:{port}"),
        child,
        game_dir: game_dir.clone(),
        instance_id: None,
    });
    watch_game(app.clone(), id, format!("{url_host}:{port}"), game_dir, None);
    crate::launch::window_tracker::clear_always_on_top(&app);
    let canonical_name = if let Some(title) = bom.server_title.as_deref().filter(|t| !t.trim().is_empty()) {
        title.trim()
    } else {
        name.filter(|n| !n.trim().is_empty()).unwrap_or(address)
    };
    servers::record_played(canonical_name, address);

    crate::launch::window_tracker::spawn_window_tracker(app.clone(), id, pid);
    crate::mission_control::spawn_mission_control_tracker(
        app.clone(),
        pid,
        id,
        None,
        canonical_name.to_string(),
        memory_gb as u64 * 1024,
    );

    if state.settings.lock().unwrap().discord_rpc {
        let loader_type = bom.mod_loader.as_ref().map(|l| l.r#type.as_str());
        let loader_str = loader_type
            .filter(|s| !s.trim().is_empty())
            .map(|l| format!(" ({})", capitalize_loader(l)))
            .unwrap_or_default();
        let details = format!("Playing on {canonical_name}");
        let state_text = format!("Minecraft {}{}", bom.minecraft_version, loader_str);
        let settings = crate::settings::load_settings();
        let mode = settings.discord_rpc_mode.as_str();
        let activity = match mode {
            "incognito" => crate::discord_rpc::Activity::new(
                "Playing Minecraft",
                "Online Game",
                Some(chrono::Utc::now().timestamp()),
                None,
            ),
            "presence_only" => crate::discord_rpc::Activity::new(
                details,
                state_text,
                Some(chrono::Utc::now().timestamp()),
                loader_type,
            ).with_buttons(vec![crate::discord_rpc::ActivityButton {
                label: "Get Zircon Launcher".to_string(),
                url: "https://zirconmc.net".to_string(),
            }]),
            _ => crate::discord_rpc::Activity::new(
                details,
                state_text,
                Some(chrono::Utc::now().timestamp()),
                loader_type,
            ).with_server_join(&format!("{url_host}:{port}")),
        };
        let discord_client = state.discord_client.clone();
        tauri::async_runtime::spawn(async move {
            crate::discord_rpc::update_discord_presence(&discord_client, activity).await;
        });
    }

    emit_status(
        app,
        format!("Game running — connected to {url_host}:{port}"),
    );
    let _ = app.emit(
        "game-status",
        serde_json::json!({ "running": true, "label": format!("{url_host}:{port}") }),
    );
    Ok(())
}

/// Helper to capitalize mod loader names for display.
pub(crate) fn capitalize_loader(loader: &str) -> String {
    let lower = loader.trim().to_lowercase();
    match lower.as_str() {
        "fabric" => "Fabric".to_string(),
        "forge" => "Forge".to_string(),
        "neoforge" | "neo_forge" => "NeoForge".to_string(),
        "quilt" => "Quilt".to_string(),
        "vanilla" => "Vanilla".to_string(),
        other => {
            let mut c = other.chars();
            match c.next() {
                None => String::new(),
                Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
            }
        }
    }
}

/// Registers a pre-join ticket with the Zircon server so the TCP multiplexer
/// Applies a shader decision to the pack selection: enabling activates the
/// server's first shaderpack when none is chosen yet; disabling turns shaders
/// off and clears the selection.
pub(crate) fn apply_shader_choice(selection: &mut PackSelection, bom: &BillOfMaterials, enabled: bool) {
    if enabled {
        selection.shaders_enabled = true;
        if selection.active_shaderpack.is_none() {
            if let Some(first) = bom.shaderpacks.first() {
                selection.active_shaderpack = Some(first.filename.clone());
            }
        }
    } else {
        selection.shaders_enabled = false;
        selection.active_shaderpack = None;
    }
}

/// Resolves an in-flight shader opt-in prompt with the player's answer.
#[tauri::command]
pub async fn respond_shader_choice(
    state: State<'_, LauncherState>,
    request_id: u64,
    enabled: bool,
    remember: bool,
) -> Result<(), String> {
    if let Some(tx) = state.shader_requests.lock().await.remove(&request_id) {
        let _ = tx.send(ShaderChoice { enabled, remember });
    }
    Ok(())
}

/// Resolves an in-flight host-key rotation prompt with the player's decision.
/// `accepted = true` re-pins the new key and lets the launch continue;
/// `false` (or a dropped/expired prompt) aborts the launch.
#[tauri::command]
pub async fn respond_key_prompt(
    state: State<'_, LauncherState>,
    request_id: u64,
    accepted: bool,
) -> Result<(), String> {
    if let Some(tx) = state.key_prompts.lock().await.remove(&request_id) {
        let _ = tx.send(accepted);
    }
    Ok(())
}

/// Watches a running game; when it exits, clears the state slot and emits a
/// `game-status` event. Polls `try_wait` so the child stays killable via
/// `stop_game` while it runs.
pub(crate) fn watch_game(app: AppHandle, id: u64, label: String, game_dir: PathBuf, instance_id: Option<String>) {
    tauri::async_runtime::spawn(async move {
        loop {
            tokio::time::sleep(Duration::from_millis(500)).await;
            let state = app.state::<LauncherState>();
            let mut guard = state.running_game.lock().await;
            let Some(game) = guard.as_mut() else { return };
            if game.id != id {
                return; // a newer game replaced this one
            }
            match game.child.try_wait() {
                Ok(Some(status)) => {
                    let code = status.code().unwrap_or(-1);
                    guard.take();
                    drop(guard);
                    crate::launch::window_tracker::clear_always_on_top(&app);
                    let discord_client = state.discord_client.clone();
                    tauri::async_runtime::spawn(async move {
                        crate::discord_rpc::clear_discord_presence(&discord_client).await;
                    });
                    let _ = app.emit(
                        "game-status",
                        serde_json::json!({ "running": false, "label": label, "code": code }),
                    );
                    let _ = app.emit("launch-status", format!("Game exited (code {code})."));

                    if !status.success() {
                        tracing::warn!("Minecraft exited with failure code {code} — analyzing crash dumps and logs...");
                        let mut analysis = crate::launch::crash_analyzer::analyze_instance_latest_crash(&game_dir);
                        if let Some(ref mut a) = analysis {
                            let (mc_ver, loader) = if let Some(ref inst_id) = instance_id {
                                if let Some(inst) = state.offline.load(inst_id) {
                                    (Some(inst.minecraft_version), Some(inst.mod_loader.r#type))
                                } else {
                                    (None, None)
                                }
                            } else {
                                (None, None)
                            };
                            crate::launch::crash_analyzer::enrich_crash_analysis(
                                a,
                                &game_dir,
                                mc_ver.as_deref(),
                                loader.as_deref(),
                                Some(&state.modrinth),
                            ).await;
                        }
                        let _ = app.emit(
                            "game-crashed",
                            serde_json::json!({
                                "code": code,
                                "label": label,
                                "gameDir": game_dir.to_string_lossy(),
                                "instanceId": instance_id,
                                "analysis": analysis,
                            }),
                        );
                    }
                    return;
                }
                Ok(None) => {}
                Err(_) => {
                    guard.take();
                    crate::launch::window_tracker::clear_always_on_top(&app);
                    let discord_client = state.discord_client.clone();
                    tauri::async_runtime::spawn(async move {
                        crate::discord_rpc::clear_discord_presence(&discord_client).await;
                    });
                    return;
                }
            }
        }
    });
}

/// Stops the running game (PLAY button toggle).
#[tauri::command]
pub async fn stop_game(app: AppHandle, state: State<'_, LauncherState>) -> Result<(), String> {
    state.launch_cancellation.request_abort();
    crate::launch::window_tracker::clear_always_on_top(&app);
    let discord_client = state.discord_client.clone();
    tauri::async_runtime::spawn(async move {
        crate::discord_rpc::clear_discord_presence(&discord_client).await;
    });
    let mut guard = state.running_game.lock().await;
    let Some(mut game) = guard.take() else {
        let _ = app.emit("launch-status", "Launch aborted by user.");
        let _ = app.emit(
            "game-status",
            serde_json::json!({ "running": false, "label": "", "code": 0 }),
        );
        return Ok(());
    };
    let label = game.label.clone();
    let app2 = app.clone();
    tauri::async_runtime::spawn(async move {
        let _ = game.child.kill().await;
        let _ = game.child.wait().await;
        crate::launch::window_tracker::clear_always_on_top(&app2);
        let _ = app2.emit(
            "game-status",
            serde_json::json!({ "running": false, "label": label, "code": 0 }),
        );
        let _ = app2.emit("launch-status", "Game process stopped.");
    });
    Ok(())
}

/// Current game status for UI restore on app start / view switch.
#[tauri::command]
pub async fn get_game_status(
    state: State<'_, LauncherState>,
) -> Result<Option<GameStatus>, String> {
    let guard = state.running_game.lock().await;
    Ok(guard.as_ref().map(|game| GameStatus {
        running: true,
        label: game.label.clone(),
    }))
}

#[derive(Debug, Clone, Serialize)]
pub struct GameStatus {
    pub running: bool,
    pub label: String,
}

pub(crate) async fn fetch_bom(
    http: &reqwest::Client,
    base_url: &str,
) -> Result<BillOfMaterials, LauncherError> {
    let response = http.get(format!("{base_url}/bom")).send().await?;
    let status = response.status().as_u16();
    if !(200..300).contains(&status) {
        return Err(LauncherError::Http {
            status,
            url: format!("{base_url}/bom"),
        });
    }
    let text = response.text().await?;
    Ok(serde_json::from_str(&text)?)
}

/// Outcome of evaluating a fetched BOM against the launcher's trust state.
#[derive(Debug, Clone, PartialEq)]
pub enum BomTrustOutcome {
    /// The BOM carries no attestation (unsigned server — third-party or legacy
    /// wrapper). The launcher continues, relying on the always-strict per-file
    /// hash verification of the mod sync.
    NoAttestation,
    /// The BOM's Ed25519 signature verified against `key` (hex public key).
    /// `key` must be persisted as the server's TOFU pin.
    Verified(String),
    /// The BOM is signed, but with a **different** key than the one pinned for
    /// this server on first contact (reinstall, or a possible takeover). The
    /// caller must show the SHA-256 fingerprint delta and obtain explicit
    /// player approval before re-pinning — never auto-accept.
    KeyMismatch {
        /// The newly presented key (hex public key).
        received: String,
        /// The previously pinned key (hex public key).
        pinned: String,
    },
}

/// Shown to the player when a server presents a different Ed25519 key than
/// the one pinned on first contact. Emitted as `server-key-mismatch`; the
/// player's answer goes back through [`respond_key_prompt`].
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KeyMismatchPrompt {
    pub request_id: u64,
    pub server_address: String,
    pub old_fingerprint: String,
    pub new_fingerprint: String,
}

/// SHA-256 fingerprint of a hex-encoded Ed25519 public key, SSH-style
/// (`SHA256:<hex-of-digest>`). The digest of the raw key bytes is what the
/// player compares when a server key changes — two fingerprints are far easier
/// to eyeball than 64 raw hex bytes.
pub fn compute_key_fingerprint(pubkey_hex: &str) -> String {
    let bytes = hex::decode(pubkey_hex).unwrap_or_default();
    let hash = Sha256::digest(&bytes);
    format!("SHA256:{}", hex::encode(hash))
}

/// Evaluates the trust state of a fetched BOM against the currently pinned
/// key (TOFU): pins on first attested contact, surfaces key rotation for
/// interactive approval, rejects unsigned downgrades, and verifies the Ed25519
/// signature before any mod download or launch happens. Pure (no I/O) so the
/// decision is unit-testable; persistence of the pin is left to the caller.
/// Public so the security test suite can drive it end to end.
///
/// Fails closed:
/// * BOM signed with a key different from the pinned one → `KeyMismatch` (the
///   caller prompts the player; rejecting the rotation aborts the launch).
/// * BOM unsigned after a key was pinned → abort (attestation downgrade).
/// * BOM with a signature but no public key, or a signature that does not
///   verify → abort.
///
/// The only pass-through is a BOM with **no** attestation fields at all for a
/// server that never presented a key — that is a server that does not run the
/// Zircon wrapper, and the per-file hash checks still gate every download.
pub fn evaluate_bom_trust(
    bom: &BillOfMaterials,
    pinned: Option<&str>,
) -> Result<BomTrustOutcome, LauncherError> {
    let Some(received_key) = bom.server_public_key.as_deref() else {
        if bom.signature.is_some() {
            return Err(LauncherError::Security(
                "Server BOM carries a signature but no public key — refusing to launch."
                    .to_string(),
            ));
        }
        if pinned.is_some() {
            return Err(LauncherError::Security(
                "Server stopped signing its BOM after previously presenting a \
                 signing key — refusing to launch (possible downgrade attack)."
                    .to_string(),
            ));
        }
        tracing::warn!(
            "Server does not sign its BOM (no attestation); continuing with \
             per-file hash verification only."
        );
        return Ok(BomTrustOutcome::NoAttestation);
    };

    let trusted_key = pinned.unwrap_or(received_key);
    if trusted_key != received_key {
        // Key rotation: the server presents a different key than the one
        // pinned on first contact. Surface both keys so the caller can show
        // the fingerprint delta and let the player decide.
        return Ok(BomTrustOutcome::KeyMismatch {
            received: received_key.to_string(),
            pinned: trusted_key.to_string(),
        });
    }
    if !signing::verify_bom_signature(bom, trusted_key) {
        return Err(LauncherError::Security(
            "BOM signature verification failed — the server's mod list is not \
             authentic. Refusing to launch."
                .to_string(),
        ));
    }
    Ok(BomTrustOutcome::Verified(trusted_key.to_string()))
}

/// How often the launcher refreshes its join intent while preparing to launch.
/// The server holds the instance's idle shutdown off for the ticket TTL
/// (5 minutes), so a 30s refresh keeps the hold fresh with a large margin
/// while a long pre-spawn flow runs (pack/mod sync, Java download, shader
/// prompt).
const JOIN_INTENT_HEARTBEAT_SECS: u64 = 30;

/// Registers a pre-join ticket with the Zircon server so the TCP multiplexer
/// lets the client through, and holds the target instance's idle shutdown off
/// while the player is on their way. Best-effort like the Java (transport and
/// most HTTP failures are only logged). Returns the HTTP status so callers can
/// act on a 409 — the server was stopped manually and the launch should abort
/// before Minecraft boots into a dead server.
pub(crate) async fn register_join_intent(
    http: &reqwest::Client,
    base_url: &str,
    username: &str,
    uuid: &str,
    hostname: &str,
    port: u16,
) -> Result<u16, LauncherError> {
    let body = serde_json::json!({
        "username": username,
        "uuid": uuid,
        "hostname": hostname,
        "port": port,
    });
    let response = http
        .post(format!("{base_url}/api/join-intent"))
        .header(reqwest::header::CONTENT_TYPE, "application/json")
        .body(body.to_string())
        .send()
        .await?;
    let status = response.status().as_u16();
    if !(200..300).contains(&status) {
        tracing::warn!("Pre-join intent registration failed: HTTP {status}");
    }
    Ok(status)
}

/// Periodically re-registers the join intent (ticket + idle-shutdown hold) so
/// the server stays awake for the whole pre-spawn flow — the first beat fires
/// immediately, then every `JOIN_INTENT_HEARTBEAT_SECS`. Stops when the game
/// spawns; the final registration's ticket TTL then covers the Minecraft boot
/// window. Network failures are tolerated — the next beat retries.
pub(crate) async fn join_intent_heartbeat(
    http: reqwest::Client,
    base_url: String,
    hostname: String,
    port: u16,
    username: String,
    uuid: String,
) {
    loop {
        if let Err(e) =
            register_join_intent(&http, &base_url, &username, &uuid, &hostname, port).await
        {
            tracing::warn!("Join-intent heartbeat failed: {e}");
        }
        tokio::time::sleep(Duration::from_secs(JOIN_INTENT_HEARTBEAT_SECS)).await;
    }
}

/// Aborts the join-intent heartbeat when the launch flow exits, no matter the
/// reason, so a cancelled or failed launch does not leave the server held
/// awake by a stale intent.
pub(crate) struct JoinIntentHeartbeat(pub(crate) Option<tauri::async_runtime::JoinHandle<()>>);

impl Drop for JoinIntentHeartbeat {
    fn drop(&mut self) {
        if let Some(handle) = self.0.take() {
            handle.abort();
        }
    }
}






/// Launches an offline instance: classpath resolution, then the game process.
#[tauri::command]
pub async fn launch_offline_instance(
    app: AppHandle,
    state: State<'_, LauncherState>,
    id: String,
) -> Result<(), String> {
    crate::launch::window_tracker::set_always_on_top(&app);
    let Some(instance) = state.offline.load(&id) else {
        crate::launch::window_tracker::clear_always_on_top(&app);
        return Err("Instance not found".to_string());
    };
    state.launch_cancellation.reset();

    let cancel_watcher = async {
        while !state.launch_cancellation.is_aborted() {
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    };

    let launch_future = run_offline_flow(&app, &state, &instance);

    tokio::select! {
        res = launch_future => {
            if res.is_err() {
                crate::launch::window_tracker::clear_always_on_top(&app);
            }
            res.map_err(err_string)
        }
        _ = cancel_watcher => {
            crate::launch::window_tracker::clear_always_on_top(&app);
            let _ = app.emit("launch-status", "Launch aborted by user.");
            let _ = app.emit(
                "game-status",
                serde_json::json!({ "running": false, "label": "", "code": 0 }),
            );
            Err("Launch cancelled by user.".to_string())
        }
    }
}

pub(crate) async fn run_offline_flow(
    app: &AppHandle,
    state: &LauncherState,
    instance: &OfflineInstance,
) -> Result<(), LauncherError> {
    state.launch_cancellation.reset();
    if state.running_game.lock().await.is_some() {
        return Err(LauncherError::InvalidInput(
            "A game is already running — stop it first.".to_string(),
        ));
    }

    if !zircon_core::api::versions::is_supported_mc_version(&instance.minecraft_version) {
        return Err(LauncherError::InvalidInput(format!(
            "Minecraft {} is not supported. The minimum supported version is 1.7 (e.g. 1.7.10). Older versions like 1.6 are not supported.",
            instance.minecraft_version
        )));
    }

    let listener = UiProgressListener { app: app.clone() };
    let required_java =
        JavaRuntimeSelector::get_required_java_major_version(&instance.minecraft_version);

    let (memory_gb, custom_jvm, java_override, display_opts) = {
        let s = state.settings.lock().unwrap();
        let disp = crate::launch::runner::LaunchDisplayOptions {
            width: s.window_width,
            height: s.window_height,
            fullscreen: s.start_fullscreen,
        };
        let c_jvm: Option<String> = s.custom_jvm_args.clone();
        let j_override: Option<String> = s.java_path_override.clone();
        (s.memory_gb, c_jvm, j_override, disp)
    };
    let java_override_path: Option<&std::path::Path> =
        java_override.as_deref().map(std::path::Path::new);


    let client_jar_override = state.offline.resolve_custom_client_jar(instance);
    let client_jar_path = client_jar_override.as_deref();

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
        .await?;
    state.launch_cancellation.guard_active()?;

    let game_dir = state.offline.instance_dir(&instance.id);
    std::fs::create_dir_all(&game_dir)?;

    // The Settings RAM slider and custom JVM args override the instance's values.
    let java_major = crate::launch::java::get_java_home_major_version(&launch_data.java_home)
        .or(Some(required_java));
    let is_vr = crate::launch::java::is_vr_instance(&game_dir, Some(&launch_data.version_name));
    let java_args = override_heap_with_context(&instance.java_args, memory_gb, custom_jvm.as_deref(), java_major, is_vr);

    let player_name = {
        let session = state.session.lock().await;
        session
            .as_ref()
            .map(|s| s.username.clone())
            .filter(|u| !u.trim().is_empty())
            .unwrap_or_else(|| "Player".to_string())
    };

    emit_status(
        app,
        format!("Starting offline instance '{}'...", instance.name),
    );
    state.launch_cancellation.guard_active()?;
    let output = game_output_emitter(app);
    let child = MinecraftRunner
        .launch_offline_with_options(
            &launch_data,
            &player_name,
            &java_args,
            &game_dir,
            Some(display_opts),
            Some(output),
        )
        .await?;

    let pid = child.id().unwrap_or(0);
    let id = state.next_game_id.fetch_add(1, Ordering::SeqCst);
    *state.running_game.lock().await = Some(RunningGame {
        id,
        label: instance.name.clone(),
        child,
        game_dir: game_dir.clone(),
        instance_id: Some(instance.id.clone()),
    });
    watch_game(app.clone(), id, instance.name.clone(), game_dir, Some(instance.id.clone()));
    crate::launch::window_tracker::clear_always_on_top(&app);
    crate::launch::window_tracker::spawn_window_tracker(app.clone(), id, pid);
    crate::mission_control::spawn_mission_control_tracker(
        app.clone(),
        pid,
        id,
        Some(instance.id.clone()),
        instance.name.clone(),
        memory_gb as u64 * 1024,
    );

    let mut updated = instance.clone();
    updated.last_played = chrono::Utc::now().timestamp_millis();
    if let Err(e) = state.offline.save(&updated) {
        tracing::warn!("Could not stamp lastPlayed: {e}");
    }

    if state.settings.lock().unwrap().discord_rpc {
        let loader_type = Some(instance.mod_loader.r#type.as_str());
        let loader_str = if !instance.mod_loader.r#type.trim().is_empty() {
            format!(" ({})", capitalize_loader(&instance.mod_loader.r#type))
        } else {
            String::new()
        };
        let details = format!("Playing: {}", instance.name);
        let state_text = format!("Minecraft {}{}", instance.minecraft_version, loader_str);
        let settings = crate::settings::load_settings();
        let mode = settings.discord_rpc_mode.as_str();
        let mut activity = match mode {
            "incognito" => crate::discord_rpc::Activity::new(
                "Playing Minecraft",
                "Singleplayer / Co-Op",
                Some(chrono::Utc::now().timestamp()),
                None,
            ),
            _ => crate::discord_rpc::Activity::new(
                details,
                state_text,
                Some(chrono::Utc::now().timestamp()),
                loader_type,
            ),
        };
        let coop_guard = state.coop_session.lock().await;
        if let Some(ref coop) = *coop_guard {
            if mode == "public" {
                activity = activity.with_coop_join(&coop.join_code);
            } else {
                activity = activity.with_buttons(vec![crate::discord_rpc::ActivityButton {
                    label: "Get Zircon Launcher".to_string(),
                    url: "https://zirconmc.net".to_string(),
                }]);
            }
        } else {
            activity = activity.with_buttons(vec![crate::discord_rpc::ActivityButton {
                label: "Get Zircon Launcher".to_string(),
                url: "https://zirconmc.net".to_string(),
            }]);
        }
        drop(coop_guard);
        let discord_client = state.discord_client.clone();
        tauri::async_runtime::spawn(async move {
            crate::discord_rpc::update_discord_presence(&discord_client, activity).await;
        });
    }

    let _ = app.emit(
        "game-status",
        serde_json::json!({ "running": true, "label": instance.name }),
    );
    emit_status(app, format!("Playing {} (offline).", instance.name));
    Ok(())
}

/// Replaces any `-Xmx`/`-Xms` tokens in a Java args string with the Settings
/// slider value, matching `-Xms` to `-Xmx` to prevent dynamic allocation CPU stalls,
/// keeping every other argument (extra JVM flags, GC options...), and appending any
/// user-configured custom JVM arguments.
#[allow(dead_code)]
pub(crate) fn override_heap(java_args: &str, memory_gb: u32, custom_jvm_args: Option<&str>) -> String {
    override_heap_with_context(java_args, memory_gb, custom_jvm_args, None, false)
}

/// Replaces any `-Xmx`/`-Xms` tokens in a Java args string with the Settings
/// slider value, matching `-Xms` to `-Xmx` to prevent dynamic allocation CPU stalls,
/// keeping every other argument (extra JVM flags, GC options...), and appending any
/// user-configured custom JVM arguments. If no GC is configured, automatically selects
/// the optimal GC (Generational ZGC for Java 21+ with >=4GB or VR, else G1GC).
pub(crate) fn override_heap_with_context(
    java_args: &str,
    memory_gb: u32,
    custom_jvm_args: Option<&str>,
    java_major: Option<i32>,
    is_vr: bool,
) -> String {
    let mut initial = java_args.to_string();
    if let Some(extra) = custom_jvm_args {
        if !extra.trim().is_empty() {
            if !initial.is_empty() {
                initial.push(' ');
            }
            initial.push_str(extra.trim());
        }
    }
    let tokens: Vec<String> = initial.split_whitespace().map(str::to_string).collect();
    let mut out: Vec<String> = Vec::new();
    for token in tokens {
        let lower = token.to_ascii_lowercase();
        if lower.starts_with("-xmx") {
            out.push(format!("-Xmx{memory_gb}G"));
        } else if lower.starts_with("-xms") {
            out.push(format!("-Xms{memory_gb}G"));
        } else {
            out.push(token);
        }
    }
    if !out.iter().any(|t| t.to_ascii_lowercase().starts_with("-xms")) {
        out.push(format!("-Xms{memory_gb}G"));
    }
    if !out.iter().any(|t| t.to_ascii_lowercase().starts_with("-xmx")) {
        out.push(format!("-Xmx{memory_gb}G"));
    }
    if !out.iter().any(|t| t.starts_with("-XX:+Use") || t.starts_with("-XX:-Use")) {
        let gc_flags = crate::launch::java::select_optimal_gc(java_major, memory_gb, is_vr);
        out.extend(gc_flags);
    }
    out.join(" ")
}


