//! Server settings endpoints (legacy single-server): reads/writes
//! `server.properties` plus wrapper settings from the BOM / config.
//!
//! Port of `com.mcmanager.server.web.controller.ConfigController`.

use axum::extract::State;
use axum::Json;

use serde::Deserialize;
use zircon_core::model::ModLoaderInfo;

use super::app::{ApiError, AppState};
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
        "curseforgeApiKey": cfg.curseforge_api_key,
        "serverProperties": server_properties,
    });
    Ok(Json(value))
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
            cfg.curseforge_api_key = key.clone();
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

/// GET /api/status — process status, online players, port wiring.
pub async fn get_status(State(state): State<AppState>) -> Json<serde_json::Value> {
    let cfg = state.config.get_config();
    let instance = state.instances.get_active_instance();
    let value = match &instance {
        Some(instance) => {
            let running = state.instances.is_running(&instance.id);
            let players = state.instances.get_online_players(&instance.id);
            instance_to_map(instance, running, players.len(), players)
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
