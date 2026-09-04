//! REST endpoints for instance backups: list the audit trail, trigger a manual
//! backup, restore an archive over the instance directory, and configure how
//! many backups are kept.
//!
//! Port of `com.mcmanager.server.web.controller.BackupController`.

use axum::extract::{Path, State};
use axum::Json;

use serde::Deserialize;

use crate::web::app::{ApiError, AppState};
use crate::web::views;

/// GET /api/instances/{id}/backups
pub async fn list_backups(
    State(state): State<AppState>,
    Path(instance_id): Path<String>,
) -> Json<serde_json::Value> {
    let backups = state.backup.list_backups(&instance_id);
    let mapped: Vec<serde_json::Value> = backups.iter().map(views::backup_entry_to_map).collect();
    Json(serde_json::json!({ "backups": mapped }))
}

/// POST /api/instances/{id}/backups — creates a manual backup.
pub async fn create_backup(
    State(state): State<AppState>,
    Path(instance_id): Path<String>,
) -> Result<(axum::http::StatusCode, Json<serde_json::Value>), ApiError> {
    let entry = state.backup.create_backup(&instance_id, "manual").await?;
    Ok((
        axum::http::StatusCode::CREATED,
        Json(views::backup_entry_to_map(&entry)),
    ))
}

/// POST /api/instances/{id}/backups/{backup_id}/restore
pub async fn restore_backup(
    State(state): State<AppState>,
    Path((instance_id, backup_id)): Path<(String, String)>,
) -> Result<Json<serde_json::Value>, ApiError> {
    state
        .backup
        .restore_backup(&instance_id, &backup_id)
        .await?;
    Ok(Json(serde_json::json!({
        "ok": true,
        "message": "Backup restored successfully."
    })))
}

/// POST /api/instances/{id}/backups/retention — body: {retention: N}
pub async fn set_retention(
    State(state): State<AppState>,
    Path(instance_id): Path<String>,
    Json(body): Json<RetentionRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let Some(retention) = body.retention else {
        return Err(ApiError::BadRequest("retention is required".to_string()));
    };
    if !(1..=100).contains(&retention) {
        return Err(ApiError::BadRequest(
            "retention must be between 1 and 100".to_string(),
        ));
    }
    let deleted = state.backup.set_retention(&instance_id, retention)?;
    Ok(Json(
        serde_json::json!({ "retention": retention, "deletedBackups": deleted }),
    ))
}

#[derive(Debug, Deserialize)]
pub struct RetentionRequest {
    pub retention: Option<i32>,
}
