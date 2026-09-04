//! REST controller for querying Minecraft game versions and mod loader builds.

use axum::extract::{Query, State};
use axum::Json;
use serde::Deserialize;

use crate::web::app::{ApiError, AppState};

#[derive(Debug, Deserialize)]
pub struct MinecraftVersionsQuery {
    pub snapshots: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct LoaderVersionsQuery {
    pub loader: String,
    #[serde(rename = "mcVersion")]
    pub mc_version: String,
}

/// GET /api/versions/minecraft — returns available Minecraft versions.
pub async fn get_minecraft_versions(
    State(state): State<AppState>,
    Query(params): Query<MinecraftVersionsQuery>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let include_snapshots = params.snapshots.unwrap_or(false);
    let versions = state
        .versions
        .get_minecraft_versions(include_snapshots)
        .await
        .map_err(ApiError::BadGateway)?;

    Ok(Json(serde_json::json!({
        "versions": versions
    })))
}

/// GET /api/versions/loaders?loader=fabric&mcVersion=1.21.1 — returns loader versions and recommended build.
pub async fn get_loader_versions(
    State(state): State<AppState>,
    Query(params): Query<LoaderVersionsQuery>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let result = state
        .versions
        .get_loader_versions(&params.loader, &params.mc_version)
        .await
        .map_err(ApiError::BadGateway)?;

    Ok(Json(serde_json::json!(result)))
}
