//! Server settings endpoints (legacy single-server): reads/writes
//! `server.properties` plus wrapper settings from the BOM / config.
//!
//! Port of `com.mcmanager.server.web.controller.ConfigController`.

use axum::extract::State;
use axum::Json;

use serde::Deserialize;
use zircon_core::model::{InstanceConfig, ModLoaderInfo};

use super::app::{ApiError, AppState, RealIp};
use crate::config::ServerProperties;
use crate::instance::ServerInstanceManager;
use crate::web::controllers::config_helpers::instance_to_map;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigUpdate {
    pub server_title: Option<String>,
    pub minecraft_version: Option<String>,
    pub mod_loader: Option<ModLoaderInfo>,
    pub java_args: Option<String>,
    pub curseforge_api_key: Option<String>,
    pub auto_start_server: Option<bool>,
    pub server_properties: Option<std::collections::BTreeMap<String, String>>,
}

/// GET /api/config — wrapper config + server.properties + server status.
pub async fn get_config(
    State(state): State<AppState>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let cfg = state.config.get_config();
    let server_properties = match state.config.load_server_properties() {
        Ok(props) => props.as_map(),
        Err(_) => std::collections::BTreeMap::new(),
    };
    let value = serde_json::json!({
        "serverTitle": cfg.server_title,
        "minecraftVersion": cfg.minecraft_version,
        "modLoader": cfg.mod_loader,
        "javaArgs": cfg.java_args,
        "publicPort": cfg.public_port,
        "mcPort": cfg.mc_port,
        "autoStartServer": cfg.auto_start_server,
        "curseforgeApiKey": mask_api_key(&cfg.curseforge_api_key),
        "serverProperties": server_properties,
    });
    Ok(Json(value))
}

/// Masks a secret for display: only the last 4 characters are revealed, so a
/// configured CurseForge API key never leaves the server in full.
fn mask_api_key(key: &str) -> String {
    let key = key.trim();
    if key.is_empty() || key.len() <= 4 {
        return "****".to_string();
    }
    format!("****{}", &key[key.len() - 4..])
}

/// POST /api/config — accepts partial updates.
pub async fn update_config(
    State(state): State<AppState>,
    Json(body): Json<ConfigUpdate>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let mut bom_updated = false;
    {
        let mut cfg = state.config.get_config();
        if let Some(title) = &body.server_title {
            cfg.server_title = title.clone();
            bom_updated = true;
        }
        if let Some(mc) = &body.minecraft_version {
            cfg.minecraft_version = mc.clone();
            bom_updated = true;
        }
        if let Some(loader) = &body.mod_loader {
            cfg.mod_loader = loader.clone();
            bom_updated = true;
        }
        if let Some(args) = &body.java_args {
            cfg.java_args = args.clone();
        }
        if let Some(key) = &body.curseforge_api_key {
            let key = key.trim();
            // `get_config` returns a masked key ("****abcd"); sending it back
            // on save must NOT overwrite the real stored key.
            if !key.starts_with("****") {
                cfg.curseforge_api_key = key.to_string();
            }
        }
        if let Some(auto) = body.auto_start_server {
            cfg.auto_start_server = auto;
        }
        state.config.with_config(|c| {
            *c = cfg;
        });
    }
    state.config.save_config()?;
    if bom_updated {
        // Keep the persisted BOM in sync with the updated config fields before
        // saving, so the two stores can never drift.
        state.bom.with_bom(|b| {
            if let Some(title) = &body.server_title {
                b.server_title = Some(title.clone());
            }
            if let Some(mc) = &body.minecraft_version {
                b.minecraft_version = mc.clone();
            }
            if let Some(loader) = &body.mod_loader {
                b.mod_loader = Some(loader.clone());
            }
        });
        state.bom.save()?;
    }
    if let Some(props) = &body.server_properties {
        if !props.is_empty() {
            let mut current = state.config.load_server_properties()?;
            for (key, value) in props {
                current.set(key, value);
            }
            state.config.save_server_properties(&current)?;
        }
    }
    Ok(Json(serde_json::json!({ "ok": true })))
}

/// POST /api/server/start — launch the Minecraft subprocess.
pub async fn start_server(
    State(state): State<AppState>,
) -> Result<Json<serde_json::Value>, ApiError> {
    state
        .process_manager
        .start()
        .await
        .map_err(|e| ApiError::Conflict(e.to_string()))?;
    Ok(Json(serde_json::json!({ "ok": true, "running": true })))
}

/// POST /api/server/stop — stop the Minecraft subprocess.
pub async fn stop_server(State(state): State<AppState>) -> Json<serde_json::Value> {
    state.process_manager.stop().await;
    Json(serde_json::json!({ "ok": true, "running": false }))
}

/// GET /status — public client-facing status (like `/bom`): online player
/// count, max players, version and running state for the active instance. No
/// admin token is required, so the launcher can render player counts in its
/// server list without authenticating.
pub async fn client_status(State(state): State<AppState>) -> Json<serde_json::Value> {
    Json(instance_status(
        &state,
        state.instances.get_active_instance().as_ref(),
    ))
}

/// GET /{port}/status — same as `/status` but for the instance owning the path
/// port (or id), for HTTPS reverse proxies whose `Host` header carries no port.
pub async fn client_status_by_port(
    State(state): State<AppState>,
    axum::extract::Path(port_or_id): axum::extract::Path<String>,
) -> Json<serde_json::Value> {
    Json(instance_status(
        &state,
        resolve_instance_for_ref(&state, &port_or_id).as_ref(),
    ))
}

/// Resolves an instance for a path-based `:port`/instance-id reference.
fn resolve_instance_for_ref(state: &AppState, port_or_id: &str) -> Option<InstanceConfig> {
    if let Ok(port) = port_or_id.parse::<i32>() {
        if let Some(cfg) = state.instances.find_by_external_port(port) {
            return Some(cfg);
        }
        return state.instances.find_by_internal_port(port as u16);
    }
    state.instances.get_instance(port_or_id).ok()
}

fn instance_status(state: &AppState, instance: Option<&InstanceConfig>) -> serde_json::Value {
    match instance {
        Some(instance) => {
            let id = instance.id.clone();
            let running = state.instances.is_running(&id);
            let players = state.instances.get_online_players(&id);
            let max = max_players_from_properties(
                &state
                    .instances
                    .get_instance_dir(&id)
                    .join("server")
                    .join("server.properties"),
            );
            serde_json::json!({
                "online": players.len(),
                "players": players,
                "max": max,
                "running": running,
                "wakeable": !running && state.instances.wakeable(&id),
                "instanceId": id,
                "version": instance.minecraft_version,
                "name": instance.name,
            })
        }
        None => {
            let players = state.console.player_tracker().get_online_players();
            let max = max_players_from_properties(&state.config.server_properties_file);
            serde_json::json!({
                "online": players.len(),
                "players": players,
                "max": max,
                "running": state.process_manager.is_running(),
                "wakeable": false,
                "version": state.config.get_config().minecraft_version,
                "name": state.config.get_config().server_title,
            })
        }
    }
}

/// Reads `max-players` from a `server.properties` file (0 when unavailable).
fn max_players_from_properties(file: &std::path::Path) -> u32 {
    match ServerProperties::load(file) {
        Ok(props) => props
            .as_map()
            .get("max-players")
            .and_then(|v| v.parse().ok())
            .unwrap_or(0),
        Err(_) => 0,
    }
}

/// GET /api/status — process status, online players, port wiring.
pub async fn get_status(State(state): State<AppState>) -> Json<serde_json::Value> {
    let cfg = state.config.get_config();
    let instance = state.instances.get_active_instance();
    let value = match &instance {
        Some(instance) => {
            let running = state.instances.is_running(&instance.id);
            let players = state.instances.get_online_players(&instance.id);
            instance_to_map(
                instance,
                running,
                players.len(),
                players,
                state.instances.get_idle_remaining_seconds(&instance.id),
            )
        }
        None => serde_json::json!({
            "running": state.process_manager.is_running(),
            "exitCode": state.process_manager.exit_code(),
            "onlinePlayers": state.console.player_tracker().get_online_players(),
            "publicPort": cfg.public_port,
            "mcPort": cfg.mc_port,
            "webPort": cfg.web_port,
        }),
    };
    Json(value)
}

// ---------------------------------------------------------------------------
// Wakeup (idle-shutdown companion)
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WakeupRequest {
    /// The host the client will connect to; matched against instance id/name
    /// exactly like the TCP multiplexer's hostname routing.
    pub hostname: Option<String>,
    /// The player-facing port the client will connect to.
    pub port: Option<u16>,
}

/// POST /api/wakeup — public (no admin token, like `/api/join-intent`), so the
/// launcher can bring a sleeping instance back up before connecting. Resolves
/// the target the same way the multiplexer routes a Minecraft connection
/// (hostname → player-facing port → active instance) and starts it in the
/// background; the launcher then polls the status ping until it is online.
/// Refuses instances that were stopped manually (not by the idle service).
///
/// Rate-limited per real client IP (same limiter as join intents) and
/// deduplicated per instance so a single attacker cannot thrash sleeping
/// instances into resource exhaustion, and concurrent duplicate wakeups for
/// the same instance collapse into one start attempt.
pub async fn wakeup_server(
    State(state): State<AppState>,
    ip: RealIp,
    Json(body): Json<WakeupRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    // Rate limit wakeup requests identically to join intents.
    if let Err(retry_after) = state.join_intent_limiter.check(&ip.0.to_string()) {
        return Err(ApiError::TooManyRequests(format!(
            "Too many wakeup attempts. Retry in {retry_after}s."
        )));
    }

    let public_port = state.config.get_config().public_port;
    let Some(cfg) = resolve_wake_target(
        &state.instances,
        public_port,
        body.hostname.as_deref(),
        body.port,
    ) else {
        return Err(ApiError::NotFound(
            "No server instance available to wake — start it from the admin panel.".to_string(),
        ));
    };

    if state.instances.is_running(&cfg.id) {
        // Already up — the launcher can reach this state when its status ping
        // failed transiently (e.g. while the server was still booting). Treat
        // the wakeup as a keep-alive: restart the idle window so the server
        // does not shut down under a player who is about to connect.
        state.instances.defer_idle_shutdown(&cfg.id);
        return Ok(Json(
            serde_json::json!({ "ok": true, "alreadyRunning": true, "instanceId": cfg.id }),
        ));
    }
    if !state.instances.wakeable(&cfg.id) {
        return Err(ApiError::Conflict(format!(
            "Server '{}' is stopped and not in idle/sleep mode — start it from the admin panel.",
            cfg.name
        )));
    }

    // Atomically start if not already waking: a duplicate concurrent wakeup
    // for the same instance is discarded (mark_waking returns false).
    if state.instances.mark_waking(&cfg.id) {
        let instances = state.instances.clone();
        let id = cfg.id.clone();
        let log_id = id.clone();
        tokio::spawn(async move {
            let result = instances.start_instance(&id).await;
            instances.unmark_waking(&id);
            if let Err(e) = result {
                tracing::error!("Wakeup start failed for instance {id}: {e}");
            }
        });
        tracing::info!(
            "Wakeup request: starting instance '{}' ({log_id})",
            cfg.name
        );
    }

    Ok(Json(
        serde_json::json!({ "ok": true, "instanceId": cfg.id }),
    ))
}

/// Resolves which instance a wakeup targets, mirroring the TCP multiplexer's
/// routing: handshake hostname, then the player-facing port, then the instance
/// owning the main public port, then the active instance. Shared with the
/// join-intent endpoint so both signals resolve the same instance.
pub(crate) fn resolve_wake_target(
    instances: &ServerInstanceManager,
    public_port: i32,
    hostname: Option<&str>,
    port: Option<u16>,
) -> Option<InstanceConfig> {
    if let Some(host) = hostname.map(str::trim).filter(|h| !h.is_empty()) {
        if let Some(cfg) = instances.find_by_hostname(host) {
            return Some(cfg);
        }
    }
    if let Some(port) = port {
        if let Some(cfg) = instances.find_by_external_port(i32::from(port)) {
            return Some(cfg);
        }
    }
    if let Some(cfg) = instances.find_by_external_port(public_port) {
        return Some(cfg);
    }
    instances.get_active_instance()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    fn temp_dir() -> std::path::PathBuf {
        crate::test_util::temp_dir("wakeup")
    }

    #[test]
    fn wake_target_resolves_hostname_then_port_then_active() {
        let dir = temp_dir();
        let console = Arc::new(crate::process::console::ConsoleStreamHandler::new());
        let manager = ServerInstanceManager::new(&dir, console).unwrap();

        // First-created instance becomes active; its external port is the main
        // public port the multiplexer owns.
        let primary = manager
            .create_instance("Primary", "1.20.4", "vanilla", "")
            .unwrap();
        let secondary = manager
            .create_instance("Secondary", "1.20.4", "vanilla", "")
            .unwrap();

        // Hostname matches instance name (case-insensitive, normalized).
        let hit = resolve_wake_target(&manager, primary.external_mc_port, Some("SECONDARY"), None);
        assert_eq!(Some(secondary.id.clone()), hit.map(|c| c.id));

        // Dedicated player-facing port wins over hostname-less requests.
        let hit = resolve_wake_target(
            &manager,
            primary.external_mc_port,
            None,
            Some(secondary.external_mc_port as u16),
        );
        assert_eq!(Some(secondary.id.clone()), hit.map(|c| c.id));

        // Unknown host + unknown port falls back to the instance owning the
        // public port.
        let hit = resolve_wake_target(&manager, primary.external_mc_port, Some("nope"), Some(1));
        assert_eq!(Some(primary.id.clone()), hit.map(|c| c.id));

        // Empty hostname is treated as absent.
        let hit = resolve_wake_target(&manager, primary.external_mc_port, Some("  "), None);
        assert_eq!(Some(primary.id), hit.map(|c| c.id));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
