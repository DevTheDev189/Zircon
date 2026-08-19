//! Shared helpers for controllers (instance views, command sending, vanilla
//! player JSON lists).

use axum::Json;
use serde::Deserialize;
use zircon_core::model::InstanceConfig;

use crate::web::app::ApiError;
use crate::web::views::instance_to_map as view_instance_to_map;

/// Validates that a player name conforms strictly to Minecraft username grammar:
/// 1 to 16 characters, ASCII alphanumeric or underscore only. Rejects newlines
/// and any other injection payload outright (they cannot appear in a real
/// username).
pub fn validate_minecraft_username(name: &str) -> Result<String, ApiError> {
    let trimmed = name.trim();
    if trimmed.is_empty()
        || trimmed.len() > 16
        || !trimmed
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_')
    {
        return Err(ApiError::BadRequest(
            "Invalid player username. Must be 1-16 characters and contain only letters, numbers, and underscores."
                .to_string(),
        ));
    }
    Ok(trimmed.to_string())
}

/// Strips all carriage returns and line feeds from arbitrary string parameters
/// (e.g. ban/kick reasons) so a hostile value can never inject a second
/// console command.
pub fn sanitize_command_param(param: Option<&str>) -> String {
    param
        .unwrap_or("")
        .replace(['\r', '\n'], " ")
        .trim()
        .to_string()
}

/// Live view of an instance for the admin UI.
pub fn instance_to_map(
    instance: &InstanceConfig,
    running: bool,
    player_count: usize,
    online_players: Vec<String>,
) -> serde_json::Value {
    view_instance_to_map(instance, running, player_count, online_players)
}

/// Reads a vanilla JSON player list (`whitelist.json`, `ops.json`,
/// `banned-players.json`) from a directory.
pub fn read_player_json(dir: &std::path::Path, file_name: &str) -> Vec<serde_json::Value> {
    let file = dir.join(file_name);
    let mut out: Vec<serde_json::Value> = Vec::new();
    if !file.is_file() {
        return out;
    }
    match std::fs::read_to_string(&file)
        .map_err(|e| std::io::Error::other(e.to_string()))
        .and_then(|content| {
            serde_json::from_str::<serde_json::Value>(&content)
                .map_err(|e| std::io::Error::other(e.to_string()))
        }) {
        Ok(serde_json::Value::Array(array)) => {
            for element in array {
                if let serde_json::Value::Object(obj) = element {
                    out.push(crate::web::views::vanilla_player_entry_to_map(&obj));
                }
            }
        }
        Ok(_) => {}
        Err(e) => {
            tracing::warn!("Could not read {file_name}: {e}");
        }
    }
    out
}

/// Request body for player actions (whitelist/ban/op/kick/command).
#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlayerActionRequest {
    pub name: Option<String>,
    pub reason: Option<String>,
    pub command: Option<String>,
}

/// Result payload of sending a server command.
pub fn command_result(command: &str, sent: bool, error: Option<String>) -> Json<serde_json::Value> {
    let mut value = serde_json::json!({ "command": command, "sent": sent });
    if let Some(error) = error {
        value["error"] = serde_json::Value::String(error);
    }
    Json(value)
}
