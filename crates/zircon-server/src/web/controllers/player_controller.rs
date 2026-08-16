//! Player management endpoints (legacy single-server). Lists come from the
//! vanilla JSON files and mutations are performed by sending the corresponding
//! server commands through the console.
//!
//! Port of `com.mcmanager.server.web.controller.PlayerController`.

use axum::extract::{Path, State};
use axum::Json;

use super::config_helpers::{command_result, PlayerActionRequest};
use crate::web::app::{ApiError, AppState};

fn read_json_list(dir: &std::path::Path, file_name: &str) -> Vec<serde_json::Value> {
    super::config_helpers::read_player_json(dir, file_name)
}

/// GET /api/players/online — names of players currently connected.
pub async fn online(State(state): State<AppState>) -> Json<serde_json::Value> {
    let players = state.console.player_tracker().get_online_players();
    Json(serde_json::json!({ "players": players }))
}

/// GET /api/players/whitelist — contents of whitelist.json.
pub async fn get_whitelist(State(state): State<AppState>) -> Json<serde_json::Value> {
    let players = read_json_list(&state.config.server_dir, "whitelist.json");
    Json(serde_json::json!({ "players": players }))
}

/// POST /api/players/whitelist {"name":"Steve"} — whitelist add.
pub async fn add_whitelist(
    State(state): State<AppState>,
    Json(body): Json<PlayerActionRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let name = body
        .name
        .ok_or_else(|| ApiError::BadRequest("name is required".to_string()))?;
    let result = send_result(&state, &format!("whitelist add {name}")).await;
    Ok(result)
}

/// DELETE /api/players/whitelist/{name} — whitelist remove.
pub async fn remove_whitelist(
    State(state): State<AppState>,
    Path(name): Path<String>,
) -> Json<serde_json::Value> {
    send_result(&state, &format!("whitelist remove {name}")).await
}

/// GET /api/players/bans — contents of banned-players.json.
pub async fn get_bans(State(state): State<AppState>) -> Json<serde_json::Value> {
    let players = read_json_list(&state.config.server_dir, "banned-players.json");
    Json(serde_json::json!({ "players": players }))
}

/// POST /api/players/bans {"name":"X","reason":"..."} — ban.
pub async fn add_ban(
    State(state): State<AppState>,
    Json(body): Json<PlayerActionRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let name = body
        .name
        .ok_or_else(|| ApiError::BadRequest("name is required".to_string()))?;
    let reason = body
        .reason
        .filter(|r| !r.trim().is_empty())
        .map(|r| format!(" {r}"))
        .unwrap_or_default();
    Ok(send_result(&state, &format!("ban {name}{reason}")).await)
}

/// DELETE /api/players/bans/{name} — pardon.
pub async fn remove_ban(
    State(state): State<AppState>,
    Path(name): Path<String>,
) -> Json<serde_json::Value> {
    send_result(&state, &format!("pardon {name}")).await
}

/// GET /api/players/ops — contents of ops.json.
pub async fn get_ops(State(state): State<AppState>) -> Json<serde_json::Value> {
    let players = read_json_list(&state.config.server_dir, "ops.json");
    Json(serde_json::json!({ "players": players }))
}

/// POST /api/players/ops {"name":"Steve"} — op.
pub async fn add_op(
    State(state): State<AppState>,
    Json(body): Json<PlayerActionRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let name = body
        .name
        .ok_or_else(|| ApiError::BadRequest("name is required".to_string()))?;
    Ok(send_result(&state, &format!("op {name}")).await)
}

/// DELETE /api/players/ops/{name} — deop.
pub async fn remove_op(
    State(state): State<AppState>,
    Path(name): Path<String>,
) -> Json<serde_json::Value> {
    send_result(&state, &format!("deop {name}")).await
}

/// POST /api/players/kick {"name":"X","reason":"..."} — kick.
pub async fn kick(
    State(state): State<AppState>,
    Json(body): Json<PlayerActionRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let name = body
        .name
        .ok_or_else(|| ApiError::BadRequest("name is required".to_string()))?;
    let reason = body
        .reason
        .filter(|r| !r.trim().is_empty())
        .map(|r| format!(" {r}"))
        .unwrap_or_default();
    Ok(send_result(&state, &format!("kick {name}{reason}")).await)
}

/// POST /api/players/command {"command":"say hi"} — arbitrary server command.
pub async fn run_command(
    State(state): State<AppState>,
    Json(body): Json<PlayerActionRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let command = body
        .command
        .ok_or_else(|| ApiError::BadRequest("command is required".to_string()))?;
    Ok(send_result(&state, command.trim()).await)
}

async fn send_result(state: &AppState, command: &str) -> Json<serde_json::Value> {
    match state.process_manager.send_command(command).await {
        Ok(()) => command_result(command, true, None),
        Err(e) => command_result(command, false, Some(e.to_string())),
    }
}
