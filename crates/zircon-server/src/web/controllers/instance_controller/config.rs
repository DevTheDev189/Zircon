
use axum::extract::{Path, State};
use axum::Json;


use crate::web::app::{ApiError, AppState};

use super::helpers::*;
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
