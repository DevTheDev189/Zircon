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
pub async fn get_server_properties(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    state.instances.get_instance(&id)?; // 404 for unknown ids
    let props_file = state
        .instances
        .get_instance_dir(&id)
        .join("server")
        .join("server.properties");
    let props = if props_file.is_file() {
        crate::config::ServerProperties::load(&props_file)?
    } else {
        crate::config::ServerProperties::default()
    };
    Ok(Json(serde_json::json!({ "properties": props.as_map() })))
}

/// POST /api/instances/{id}/server-properties — partial update preserving comments.
pub async fn save_server_properties(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(body): Json<ServerPropertiesRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let Some(properties) = body.properties else {
        return Err(ApiError::BadRequest(
            "properties map is required".to_string(),
        ));
    };
    if properties.is_empty() {
        return Err(ApiError::BadRequest(
            "properties map is required".to_string(),
        ));
    }
    state.instances.get_instance(&id)?; // 404 for unknown ids
    let server_dir = state.instances.get_instance_dir(&id).join("server");
    std::fs::create_dir_all(&server_dir)?;
    let props_file = server_dir.join("server.properties");
    let mut props = if props_file.is_file() {
        crate::config::ServerProperties::load(&props_file)?
    } else {
        crate::config::ServerProperties::default()
    };
    for (key, value) in properties {
        props.set(&key, &value);
    }
    props.save(&props_file)?;
    Ok(Json(serde_json::json!({ "ok": true })))
}
