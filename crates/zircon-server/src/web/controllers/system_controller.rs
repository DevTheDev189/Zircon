//! System update and health controller.

use axum::extract::State;
use axum::Json;
use serde_json::json;

use crate::updater::{ServerUpdater, CURRENT_SERVER_VERSION};
use crate::web::app::{ApiError, AppState};

/// GET /api/system/update/check — Checks if a server update is available.
pub async fn check_update() -> Result<Json<serde_json::Value>, ApiError> {
    let updater = ServerUpdater::new();
    let update = updater.check_update().await.map_err(ApiError::Internal)?;
    Ok(Json(json!({
        "currentVersion": CURRENT_SERVER_VERSION,
        "updateAvailable": update.is_some(),
        "manifest": update
    })))
}

/// POST /api/system/update/apply — Downloads and replaces the server binary, then restarts.
pub async fn apply_update(
    State(state): State<AppState>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let updater = ServerUpdater::new();
    let Some(manifest) = updater.check_update().await.map_err(ApiError::Internal)? else {
        return Err(ApiError::BadRequest("No updates available".into()));
    };

    // 1. Gracefully stop all Minecraft server instances before replacing
    for inst in state.instances.list_instances() {
        state.instances.stop_instance(&inst.id).await;
    }

    // 2. Perform binary replacement
    updater.apply_update(&manifest).await.map_err(ApiError::Internal)?;

    // 3. Spawn background restart
    tokio::spawn(async move {
        tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
        let _ = ServerUpdater::restart_process();
    });

    Ok(Json(json!({
        "ok": true,
        "message": "Server updated successfully. Restarting..."
    })))
}
