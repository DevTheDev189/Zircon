
use axum::extract::{Multipart, Path, Query, State};
use axum::http::StatusCode;
use axum::Json;

use tokio::time::Duration;

use crate::web::controllers::mod_controller;
use crate::web::app::{ApiError, AppState};
use crate::web::views;

use super::helpers::*;
pub async fn list_shaderpacks(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let packs = packs_for(&state, &id)?;
    let mapped: Vec<serde_json::Value> = packs
        .list_shaderpacks()
        .iter()
        .map(|p| views::pack_entry_to_map(p, true))
        .collect();
    Ok(Json(serde_json::json!({ "shaderpacks": mapped })))
}

/// POST /api/instances/{id}/shaderpacks/upload
pub async fn upload_shaderpack(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Query(params): Query<mod_controller::OriginParam>,
    multipart: Multipart,
) -> Result<(StatusCode, Json<serde_json::Value>), ApiError> {
    upload_pack(&state, &id, &params, multipart, true).await
}

/// POST /api/instances/{id}/shaderpacks/install
pub async fn install_shaderpack(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(body): Json<InstallRequest>,
) -> Result<(StatusCode, Json<serde_json::Value>), ApiError> {
    install_pack(&state, &id, body, true).await
}

/// DELETE /api/instances/{id}/shaderpacks/{filename}
pub async fn remove_shaderpack(
    State(state): State<AppState>,
    Path((id, filename)): Path<(String, String)>,
) -> Result<StatusCode, ApiError> {
    remove_pack(&state, &id, &filename, true).await
}

/// GET /api/instances/{id}/resourcepacks
pub async fn list_resourcepacks(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let packs = packs_for(&state, &id)?;
    let mapped: Vec<serde_json::Value> = packs
        .list_resourcepacks()
        .iter()
        .map(|p| views::pack_entry_to_map(p, false))
        .collect();
    Ok(Json(serde_json::json!({ "resourcepacks": mapped })))
}

/// POST /api/instances/{id}/resourcepacks/upload
pub async fn upload_resourcepack(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Query(params): Query<mod_controller::OriginParam>,
    multipart: Multipart,
) -> Result<(StatusCode, Json<serde_json::Value>), ApiError> {
    upload_pack(&state, &id, &params, multipart, false).await
}

/// POST /api/instances/{id}/resourcepacks/install
pub async fn install_resourcepack(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(body): Json<InstallRequest>,
) -> Result<(StatusCode, Json<serde_json::Value>), ApiError> {
    install_pack(&state, &id, body, false).await
}

/// DELETE /api/instances/{id}/resourcepacks/{filename}
pub async fn remove_resourcepack(
    State(state): State<AppState>,
    Path((id, filename)): Path<(String, String)>,
) -> Result<StatusCode, ApiError> {
    remove_pack(&state, &id, &filename, false).await
}

/// GET /api/instances/{id}/resourcepacks/server-pack
pub async fn get_server_resourcepack(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let packs = packs_for(&state, &id)?;
    let active = packs.get_server_resourcepack();
    let all_active = packs.get_server_resourcepacks();
    let mapped = active.as_ref().map(|p| views::pack_entry_to_map(p, false));
    let mapped_list: Vec<_> = all_active.iter().map(|p| views::pack_entry_to_map(p, false)).collect();
    Ok(Json(serde_json::json!({
        "serverResourcePack": mapped,
        "serverResourcePacks": mapped_list,
    })))
}

/// POST /api/instances/{id}/resourcepacks/server-pack
pub async fn set_server_resourcepack(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(body): Json<SetServerPackRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let packs = packs_for(&state, &id)?;
    if let Some(list) = body.filenames {
        packs.set_server_resourcepacks(&list)
            .map_err(|e| ApiError::BadRequest(e.to_string()))?;
    } else {
        packs.set_server_resourcepack(body.filename.as_deref())
            .map_err(|e| ApiError::BadRequest(e.to_string()))?;
    }

    let active = packs.get_server_resourcepack();
    let all_active = packs.get_server_resourcepacks();
    let mapped = active.as_ref().map(|p| views::pack_entry_to_map(p, false));
    let mapped_list: Vec<_> = all_active.iter().map(|p| views::pack_entry_to_map(p, false)).collect();
    Ok(Json(serde_json::json!({
        "success": true,
        "serverResourcePack": mapped,
        "serverResourcePacks": mapped_list,
    })))
}

/// GET /api/instances/{id}/crash-analysis — analyzes crash dumps and logs for the instance.
pub async fn get_crash_analysis(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<crate::services::crash_analyzer::CrashAnalysis>, ApiError> {
    let cfg = state.instances.get_instance(&id)?;

    // If the instance is currently running, it has not crashed.
    if state.instances.is_running(&id) {
        return Ok(Json(crate::services::crash_analyzer::CrashAnalysis::healthy(
            "Server is currently running.",
        )));
    }

    let last_session = state.instances.get_last_session(&id);
    let session_start = last_session.as_ref().map(|s| s.start_time);

    let instance_dir = state.instances.get_instance_dir(&id);
    let mods = mods_for(&state, &id).ok();
    let loader_str = cfg.mod_loader.as_ref().map(|l| l.r#type.as_str()).unwrap_or("vanilla");
    let analysis = crate::services::crash_analyzer::CrashAnalyzerService::analyze_instance(
        &instance_dir,
        &cfg.minecraft_version,
        loader_str,
        mods.as_ref(),
        session_start,
    )
    .await;

    // If the server was stopped cleanly by the operator with code 0 and no specific fatal
    // crash report was generated, ensure healthy status is reported.
    if let Some(session) = &last_session {
        if session.stop_requested && session.exit_code == 0 && analysis.category == crate::services::crash_analyzer::CrashCategory::Unknown {
            return Ok(Json(crate::services::crash_analyzer::CrashAnalysis::healthy(
                "Server stopped cleanly.",
            )));
        }
    }

    Ok(Json(analysis))
}

/// POST /api/instances/{id}/crash-fix — executes a 1-click remediation action on the instance.
pub async fn apply_crash_fix(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(body): Json<CrashFixRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let _cfg = state.instances.get_instance(&id)?;
    let instance_dir = state.instances.get_instance_dir(&id);
    let mods = mods_for(&state, &id)?;
    let bom = bom_for(&state, &id)?;

    let message = crate::services::crash_analyzer::CrashAnalyzerService::execute_remediation(
        &instance_dir,
        &body.action,
        &mods,
        &bom,
    )
    .await
    .map_err(ApiError::BadRequest)?;

    // If the action was AcceptEula, synchronize instance EULA state
    if let crate::services::crash_analyzer::CrashRemediationAction::AcceptEula = body.action {
        let _ = state.instances.accept_eula(&id);
    }

    // If the action adjusted memory, persist the java_args update
    if let crate::services::crash_analyzer::CrashRemediationAction::AdjustMemory { recommended_gb, .. } = body.action {
        let new_args = format!("-Xms{recommended_gb}G -Xmx{recommended_gb}G");
        let _ = state.instances.update_instance_config(&id, None, Some(&new_args));
    }

    // Auto-restart if requested
    if body.auto_restart {
        let instances = state.instances.clone();
        let target_id = id.clone();
        state.instances.stop_instance(&id).await;
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(1500)).await;
            if let Err(e) = instances.start_instance(&target_id).await {
                tracing::error!("Failed to restart instance {target_id} after crash fix: {e}");
            }
        });
    }

    Ok(Json(serde_json::json!({
        "success": true,
        "message": message,
        "restarted": body.auto_restart,
    })))
}

