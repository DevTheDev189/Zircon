//! REST controller for importing Minecraft servers from ZIP archives.
//!
//! Exposes:
//! - `POST /api/instances/import/analyze` — Streams and analyzes uploaded server ZIP, returns pre-flight report
//! - `POST /api/instances/import/commit` — Assembles and registers instance, returns created instance JSON
//! - `DELETE /api/instances/import/:import_id` — Cancels and purges staging session files

use axum::extract::{Multipart, Path, State};
use axum::http::StatusCode;
use axum::Json;
use tokio::io::AsyncWriteExt;
use uuid::Uuid;

use crate::services::import::{ImportCommitRequest, PreflightReport};
use crate::web::app::{ApiError, AppState};
use crate::web::controllers::instance_controller::live_instance_map;

/// POST /api/instances/import/analyze
///
/// Streams uploaded server ZIP directly to a temp file, validates layout,
/// inspects level.dat / DataVersion, scans mods, and returns pre-flight report.
pub async fn analyze_import(
    State(state): State<AppState>,
    mut multipart: Multipart,
) -> Result<Json<PreflightReport>, ApiError> {
    let temp_id = Uuid::new_v4().to_string();
    let temp_zip_path = std::env::temp_dir().join(format!("zircon_import_{temp_id}.zip"));

    // Stream multipart upload chunk by chunk to avoid buffering large server ZIPs into RAM
    let mut file_saved = false;
    while let Ok(Some(mut field)) = multipart.next_field().await {
        let field_name = field.name().unwrap_or("").to_string();
        if field_name == "file" || field_name == "server" || field_name == "archive" {
            let mut temp_file = tokio::fs::File::create(&temp_zip_path)
                .await
                .map_err(|e| ApiError::Internal(format!("Failed to create temp upload file: {e}")))?;

            while let Ok(Some(chunk)) = field.chunk().await {
                temp_file
                    .write_all(&chunk)
                    .await
                    .map_err(|e| ApiError::Internal(format!("Error streaming upload chunk: {e}")))?;
            }
            temp_file
                .flush()
                .await
                .map_err(|e| ApiError::Internal(format!("Failed to flush temp upload file: {e}")))?;
            file_saved = true;
            break;
        }
    }

    if !file_saved {
        let _ = tokio::fs::remove_file(&temp_zip_path).await;
        return Err(ApiError::BadRequest(
            "No server ZIP file provided (form field 'file')".to_string(),
        ));
    }

    // Run extraction & analysis in blocking threadpool
    let import_service = state.import_service.clone();
    let path_clone = temp_zip_path.clone();

    let analysis_result = tokio::task::spawn_blocking(move || {
        import_service.stage_and_analyze(&path_clone)
    })
    .await
    .map_err(|e| ApiError::Internal(format!("Join error: {e}")))?;

    // Remove the uploaded raw zip file now that it is unpacked
    let _ = tokio::fs::remove_file(&temp_zip_path).await;

    match analysis_result {
        Ok(report) => Ok(Json(report)),
        Err(e) => Err(ApiError::BadRequest(e.to_string())),
    }
}

/// POST /api/instances/import/commit
///
/// Finalizes the migration: moves world & configs, signs BOM, registers instance.
pub async fn commit_import(
    State(state): State<AppState>,
    Json(body): Json<ImportCommitRequest>,
) -> Result<(StatusCode, Json<serde_json::Value>), ApiError> {
    let import_service = state.import_service.clone();
    let commit_result = tokio::task::spawn_blocking(move || {
        import_service.commit_import(body)
    })
    .await
    .map_err(|e| ApiError::Internal(format!("Join error: {e}")))?;

    match commit_result {
        Ok(created_instance) => Ok((
            StatusCode::CREATED,
            Json(live_instance_map(&state, &created_instance)),
        )),
        Err(crate::services::import::ImportError::Conflict(msg)) => Err(ApiError::Conflict(msg)),
        Err(crate::services::import::ImportError::NotFound(msg)) => Err(ApiError::NotFound(msg)),
        Err(e) => Err(ApiError::BadRequest(e.to_string())),
    }
}

/// DELETE /api/instances/import/:import_id
///
/// Cancels and deletes temporary staged files for an import session.
pub async fn cancel_import(
    State(state): State<AppState>,
    Path(import_id): Path<String>,
) -> Result<StatusCode, ApiError> {
    let import_service = state.import_service.clone();
    let id_clone = import_id.clone();

    let deleted = tokio::task::spawn_blocking(move || {
        import_service.cancel_import(&id_clone)
    })
    .await
    .map_err(|e| ApiError::Internal(format!("Join error: {e}")))?
    .map_err(|e| ApiError::Internal(e.to_string()))?;

    if deleted {
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(ApiError::NotFound(format!("Import session not found: {import_id}")))
    }
}
