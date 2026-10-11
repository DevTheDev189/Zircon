use std::sync::Arc;

use axum::extract::{Multipart, Path, Query, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::Json;

use serde::{Deserialize, Serialize};
use tokio::time::Duration;
use zircon_core::model::{BillOfMaterials, InstanceConfig, ModLoaderType};

use crate::web::controllers::config_helpers::{
    command_result, read_player_json, sanitize_command_param, validate_minecraft_username,
    PlayerActionRequest,
};
use crate::web::controllers::vanilla_player_files;
use crate::instance::ModSyncSummary;
use crate::services::bom::BomService;
use crate::services::mods::ModManagementService;
use crate::services::packs::PackManagementService;
use crate::tickets::TICKET_TTL_SECONDS;
use crate::web::app::{ApiError, AppState, RealIp};
use crate::web::config_routes::resolve_wake_target;
use crate::web::views;

use super::helpers::*;
use super::*;
pub async fn online_players(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    state.instances.get_instance(&id)?;
    let players = state.instances.get_online_players(&id);
    Ok(Json(serde_json::json!({ "players": players })))
}

/// GET /api/instances/{id}/players/history — every player that has ever joined.
pub async fn player_history(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    state.instances.get_instance(&id)?;
    let history_file = state.instances.get_instance_dir(&id).join("players.json");
    let players: Vec<serde_json::Value> =
        crate::process::player_tracker::PlayerTracker::load_history(&history_file)
            .iter()
            .map(views::player_history_to_map)
            .collect();
    Ok(Json(serde_json::json!({ "players": players })))
}

/// GET /api/instances/{id}/players/whitelist
pub async fn get_whitelist(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    state.instances.get_instance(&id)?;
    Ok(Json(
        serde_json::json!({ "players": read_player_json(&server_dir(&state, &id), "whitelist.json") }),
    ))
}

/// POST /api/instances/{id}/players/whitelist
pub async fn add_whitelist(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(body): Json<PlayerActionRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let raw_name = body
        .name
        .ok_or_else(|| ApiError::BadRequest("name is required".to_string()))?;
    let name = validate_minecraft_username(&raw_name)?;
    state.instances.get_instance(&id)?;
    Ok(send_instance_command(&state, &id, &format!("whitelist add {name}")).await)
}

/// DELETE /api/instances/{id}/players/whitelist/{name}
pub async fn remove_whitelist(
    State(state): State<AppState>,
    Path((id, name)): Path<(String, String)>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let name = validate_minecraft_username(&name)?;
    state.instances.get_instance(&id)?;
    Ok(send_instance_command(&state, &id, &format!("whitelist remove {name}")).await)
}

/// GET /api/instances/{id}/players/ops
pub async fn get_ops(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    state.instances.get_instance(&id)?;
    Ok(Json(
        serde_json::json!({ "players": read_player_json(&server_dir(&state, &id), "ops.json") }),
    ))
}

/// POST /api/instances/{id}/players/ops
pub async fn add_op(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(body): Json<PlayerActionRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let raw_name = body
        .name
        .ok_or_else(|| ApiError::BadRequest("name is required".to_string()))?;
    let name = validate_minecraft_username(&raw_name)?;
    state.instances.get_instance(&id)?;
    Ok(send_instance_command(&state, &id, &format!("op {name}")).await)
}

/// DELETE /api/instances/{id}/players/ops/{name}
pub async fn remove_op(
    State(state): State<AppState>,
    Path((id, name)): Path<(String, String)>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let name = validate_minecraft_username(&name)?;
    state.instances.get_instance(&id)?;
    Ok(send_instance_command(&state, &id, &format!("deop {name}")).await)
}

/// GET /api/instances/{id}/players/bans
pub async fn get_bans(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    state.instances.get_instance(&id)?;
    Ok(Json(
        serde_json::json!({ "players": read_player_json(&server_dir(&state, &id), "banned-players.json") }),
    ))
}

/// POST /api/instances/{id}/players/bans — online or offline ban.
pub async fn add_ban(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(body): Json<PlayerActionRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let raw_name = body
        .name
        .ok_or_else(|| ApiError::BadRequest("name is required".to_string()))?;
    let name = validate_minecraft_username(&raw_name)?;
    let reason_clean = sanitize_command_param(body.reason.as_deref());

    state.instances.get_instance(&id)?;
    if state.instances.is_running(&id) {
        let reason_arg = if reason_clean.is_empty() {
            String::new()
        } else {
            format!(" {reason_clean}")
        };
        Ok(send_instance_command(&state, &id, &format!("ban {name}{reason_arg}")).await)
    } else {
        add_ban_offline(
            &state,
            &id,
            &name,
            if reason_clean.is_empty() {
                None
            } else {
                Some(&reason_clean)
            },
        )?;
        Ok(Json(serde_json::json!({ "ok": true, "offline": true })))
    }
}

/// DELETE /api/instances/{id}/players/bans/{name} — pardon, online or offline.
pub async fn remove_ban(
    State(state): State<AppState>,
    Path((id, name)): Path<(String, String)>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let name = validate_minecraft_username(&name)?;
    state.instances.get_instance(&id)?;
    if state.instances.is_running(&id) {
        Ok(send_instance_command(&state, &id, &format!("pardon {name}")).await)
    } else {
        let file = server_dir(&state, &id).join("banned-players.json");
        let removed = vanilla_player_files::pardon(&file, &name)?;
        Ok(Json(
            serde_json::json!({ "ok": true, "offline": true, "removed": removed }),
        ))
    }
}

/// GET /api/instances/{id}/bom — the instance's mod list.
pub async fn get_instance_bom(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let mods = mods_for(&state, &id)?.list_mods_enriched().await;
    let mapped: Vec<serde_json::Value> = mods.iter().map(views::mod_entry_to_map).collect();
    Ok(Json(serde_json::Value::Array(mapped)))
}

/// POST /api/join-intent and /api/instances/{id}/join-intent — registers a
/// short-lived join ticket for the launcher's session so the player's
/// connection passes the Zircon join gate, and holds the target instance's
/// idle shutdown off while the player is on their way. Intentionally
/// unauthenticated, but rate-limited per real client IP so a single attacker
/// cannot fill the ticket store and block legitimate joins.
pub async fn register_join_intent(
    State(state): State<AppState>,
    ip: RealIp,
    Json(body): Json<JoinIntentRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    if let Err(retry_after) = state.join_intent_limiter.check(&ip.0.to_string()) {
        return Err(ApiError::TooManyRequests(format!(
            "Too many join-intent registrations from this address. Retry in {retry_after}s."
        )));
    }
    if body.username.is_none() && body.uuid.is_none() {
        return Err(ApiError::BadRequest(
            "username or uuid is required".to_string(),
        ));
    }
    if let Some(username) = &body.username {
        state.tickets.register_ticket(username);
    }
    if let Some(uuid) = &body.uuid {
        state.tickets.register_ticket(uuid);
    }

    // A player is on their way: resolve the instance the same way the
    // multiplexer routes connections and hold off its idle shutdown until the
    // player connects (or the intent expires). When the instance is asleep,
    // start it as a safety net — the launcher's wakeup may have raced an idle
    // shutdown or a status ping.
    let public_port = state.config.get_config().public_port;
    if let Some(cfg) = resolve_wake_target(
        &state.instances,
        public_port,
        body.hostname.as_deref(),
        body.port,
    ) {
        if state.instances.is_running(&cfg.id) {
            state.instances.register_join_intent(&cfg.id);
        } else if state.instances.wakeable(&cfg.id) {
            state.instances.register_join_intent(&cfg.id);
            let instances = state.instances.clone();
            let id = cfg.id.clone();
            let log_id = id.clone();
            tokio::spawn(async move {
                if let Err(e) = instances.start_instance(&id).await {
                    tracing::error!("Join-intent wake failed for instance {id}: {e}");
                }
            });
            tracing::info!(
                "Join intent for sleeping instance '{}' ({log_id}) — waking it",
                cfg.name
            );
        } else {
            // Stopped manually (maintenance): the player cannot join. Refuse so
            // the launcher aborts before booting Minecraft into a dead server.
            return Err(ApiError::Conflict(format!(
                "Server '{}' is stopped and not in idle/sleep mode — start it from the admin panel.",
                cfg.name
            )));
        }
    }

    Ok(Json(
        serde_json::json!({ "ok": true, "expiresInSeconds": TICKET_TTL_SECONDS }),
    ))
}
