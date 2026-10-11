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
use crate::web::controllers::mod_controller;
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
pub async fn list_mods(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let mods = mods_for(&state, &id)?.list_mods_enriched().await;
    let mapped: Vec<serde_json::Value> = mods.iter().map(views::mod_entry_to_map).collect();
    Ok(Json(serde_json::json!({ "mods": mapped })))
}

/// POST /api/instances/{id}/mods/upload (multipart, field "file")
pub async fn upload_mod(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Query(params): Query<mod_controller::OriginParam>,
    mut multipart: Multipart,
) -> Result<(StatusCode, Json<serde_json::Value>), ApiError> {
    let Some((filename, bytes)) = mod_controller::take_upload(&mut multipart).await? else {
        return Err(ApiError::BadRequest(
            "No file uploaded (form field 'file')".to_string(),
        ));
    };
    let mods = mods_for(&state, &id)?;
    if params.server_only == Some(true)
        || params.origin.as_deref() == Some(crate::services::mods::ORIGIN_SERVER_CUSTOM)
    {
        let entry = mods.add_server_mod(std::io::Cursor::new(bytes), &filename).await?;
        return Ok((StatusCode::CREATED, Json(views::mod_entry_to_map(&entry))));
    }
    let expected_mod_id = params.expected_mod_id.as_deref().or(params.mod_id.as_deref());
    let entry = mods
        .add_mod_with_metadata(
            std::io::Cursor::new(bytes),
            &filename,
            params.origin.as_deref(),
            params.icon_url.as_deref(),
            params.title.as_deref(),
            expected_mod_id,
            params.expected_file_id.as_deref(),
            params.project_url.as_deref(),
        )
        .await?;
    Ok((StatusCode::CREATED, Json(views::mod_entry_to_map(&entry))))
}

/// POST /api/instances/{id}/mods/upload-server (multipart, field "file") — add a custom server-side JAR strictly excluded from BOM.
pub async fn upload_server_mod(
    State(state): State<AppState>,
    Path(id): Path<String>,
    mut multipart: Multipart,
) -> Result<(StatusCode, Json<serde_json::Value>), ApiError> {
    let Some((filename, bytes)) = mod_controller::take_upload(&mut multipart).await? else {
        return Err(ApiError::BadRequest(
            "No file uploaded (form field 'file')".to_string(),
        ));
    };
    let entry = mods_for(&state, &id)?
        .add_server_mod(std::io::Cursor::new(bytes), &filename)
        .await?;
    Ok((StatusCode::CREATED, Json(views::mod_entry_to_map(&entry))))
}

/// DELETE /api/instances/{id}/mods/{filename}
pub async fn remove_mod(
    State(state): State<AppState>,
    Path((id, filename)): Path<(String, String)>,
) -> Result<StatusCode, ApiError> {
    let removed = mods_for(&state, &id)?.remove_mod(&filename)?;
    if !removed {
        return Err(ApiError::NotFound("Mod not found".to_string()));
    }
    Ok(StatusCode::NO_CONTENT)
}

/// Bulk delete endpoint removing mods from disk and BOM.
pub async fn bulk_delete_mods(
    State(app_state): State<AppState>,
    Path(instance_id): Path<String>,
    Json(payload): Json<ModFilenamesRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let purged = mods_for(&app_state, &instance_id)?.remove_mods(&payload.filenames)?;
    Ok(Json(serde_json::json!({ "deleted": purged })))
}

/// Bulk enable endpoint activating mods and renaming from .disabled.
pub async fn enable_mods(
    State(app_state): State<AppState>,
    Path(instance_id): Path<String>,
    Json(payload): Json<ModFilenamesRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let updated = mods_for(&app_state, &instance_id)?.set_mods_enabled(&payload.filenames, true)?;
    Ok(Json(serde_json::json!({ "changed": updated })))
}

/// Bulk disable endpoint deactivating mods and renaming to .disabled.
pub async fn disable_mods(
    State(app_state): State<AppState>,
    Path(instance_id): Path<String>,
    Json(payload): Json<ModFilenamesRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let updated = mods_for(&app_state, &instance_id)?.set_mods_enabled(&payload.filenames, false)?;
    Ok(Json(serde_json::json!({ "changed": updated })))
}

/// PATCH /api/instances/{id}/mods/{filename}/side — update the runtime side of a mod in an instance.
pub async fn set_mod_side(
    State(state): State<AppState>,
    Path((id, filename)): Path<(String, String)>,
    Json(body): Json<mod_controller::SetSideBody>,
) -> Result<impl IntoResponse, ApiError> {
    let updated = mods_for(&state, &id)?
        .set_mod_side(&filename, body.side)
        .map_err(|e| ApiError::BadRequest(e.to_string()))?;
    Ok((StatusCode::OK, Json(views::mod_entry_to_map(&updated))))
}

/// GET /api/instances/{id}/mods/search
pub async fn search_mods(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Query(params): Query<mod_controller::SearchParams>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let mods = mods_for(&state, &id)?;
    let query = params.query.unwrap_or_default();
    let origin = params.origin.as_deref().unwrap_or("modrinth");
    let mut result = serde_json::Map::new();
    if origin.eq_ignore_ascii_case("curseforge") {
        if !mods.has_curse_forge_key() {
            result.insert(
                "origin".to_string(),
                serde_json::Value::String("curseforge".to_string()),
            );
            result.insert("hits".to_string(), serde_json::Value::Array(vec![]));
            result.insert(
                "notice".to_string(),
                serde_json::Value::String(
                    "CurseForge API key not configured on the server.".to_string(),
                ),
            );
            return Ok(Json(serde_json::Value::Object(result)));
        }
        let hits = mods
            .curse_forge()
            .search_mods_with_type(
                &query,
                params.mc_version.as_deref(),
                params.loader.as_deref(),
                params.project_type.as_deref(),
            )
            .await
            .map_err(|e| {
                tracing::error!("CurseForge search failed for query '{query}': {e}");
                ApiError::BadGateway(format!("CurseForge search failed: {e}"))
            })?;
        tracing::info!(
            "CurseForge search query='{}', mc_version={:?}, loader={:?}, type={:?} -> returned {} hit(s)",
            query,
            params.mc_version,
            params.loader,
            params.project_type,
            hits.len()
        );
        result.insert(
            "origin".to_string(),
            serde_json::Value::String("curseforge".to_string()),
        );
        result.insert(
            "hits".to_string(),
            serde_json::Value::Array(hits.iter().map(views::curseforge_mod_to_map).collect()),
        );
    } else {
        let hits = mods
            .modrinth()
            .search_mods_with_type(
                &query,
                params.mc_version.as_deref(),
                params.loader.as_deref(),
                params.project_type.as_deref(),
            )
            .await
            .map_err(|e| ApiError::BadGateway(e.to_string()))?;
        result.insert(
            "origin".to_string(),
            serde_json::Value::String("modrinth".to_string()),
        );
        result.insert(
            "hits".to_string(),
            serde_json::Value::Array(hits.iter().map(views::modrinth_hit_to_map).collect()),
        );
    }
    Ok(Json(serde_json::Value::Object(result)))
}

/// GET /api/instances/{id}/mods/modrinth/versions
pub async fn modrinth_versions(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Query(params): Query<mod_controller::VersionParams>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let Some(project_id) = params.project_id else {
        return Err(ApiError::BadRequest("projectId is required".to_string()));
    };
    let mods = mods_for(&state, &id)?;
    let versions = mods
        .modrinth()
        .list_project_versions(
            &project_id,
            params.mc_version.as_deref(),
            params.loader.as_deref(),
        )
        .await
        .map_err(|e| ApiError::BadGateway(e.to_string()))?;
    let mapped: Vec<serde_json::Value> = versions
        .iter()
        .map(views::modrinth_version_to_map)
        .collect();
    Ok(Json(serde_json::json!({ "versions": mapped })))
}

/// GET /api/instances/{id}/mods/curseforge/files?modId=
pub async fn curseforge_files(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Query(params): Query<mod_controller::CurseForgeParams>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let mod_id = params
        .mod_id
        .ok_or_else(|| ApiError::BadRequest("modId is required".to_string()))?;
    let mod_id: i64 = mod_id
        .parse()
        .map_err(|_| ApiError::BadRequest("modId must be a number".to_string()))?;
    let mods = mods_for(&state, &id)?;
    if !mods.has_curse_forge_key() {
        return Err(ApiError::BadRequest(
            "CurseForge API key not configured on the server".to_string(),
        ));
    }
    let files = mods
        .curse_forge()
        .list_mod_files(mod_id)
        .await
        .map_err(|e| ApiError::BadGateway(e.to_string()))?;
    let mapped: Vec<serde_json::Value> = files.iter().map(views::curseforge_file_to_map).collect();
    Ok(Json(serde_json::json!({ "files": mapped })))
}

/// POST /api/instances/{id}/mods/install
pub async fn install_mod(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(body): Json<InstallRequest>,
) -> Result<(StatusCode, Json<serde_json::Value>), ApiError> {
    let Some(origin) = &body.origin else {
        return Err(ApiError::BadRequest("origin is required".to_string()));
    };
    let mods = mods_for(&state, &id)?;
    let entry = match origin.to_lowercase().as_str() {
        "modrinth" => {
            let (Some(project_id), Some(version_id)) = (&body.project_id, &body.version_id) else {
                return Err(ApiError::BadRequest(
                    "projectId and versionId are required for modrinth".to_string(),
                ));
            };
            mods.install_modrinth_version(project_id, Some(version_id), None, None)
                .await?
        }
        "curseforge" => {
            if let (Some(mod_id_str), Some(file_id_str)) = (&body.project_id, &body.version_id) {
                if let (Ok(mod_id), Ok(file_id)) =
                    (mod_id_str.parse::<i64>(), file_id_str.parse::<i64>())
                {
                    mods.install_curseforge_file(mod_id, file_id).await?
                } else {
                    return Err(ApiError::BadRequest(
                        "modId and fileId must be numeric".into(),
                    ));
                }
            } else if let (Some(download_url), Some(filename)) =
                (&body.download_url, &body.filename)
            {
                let mut entry = mods
                    .install_from_url(download_url, filename, "curseforge")
                    .await?;
                if let Some(file_id) = &body.file_id {
                    entry.id = Some(file_id.to_string());
                }
                entry
            } else {
                return Err(ApiError::BadRequest(
                    "CurseForge install requires (projectId, versionId) or (downloadUrl, filename)"
                        .into(),
                ));
            }
        }
        _ => {
            return Err(ApiError::BadRequest(
                "origin must be 'modrinth' or 'curseforge'".to_string(),
            ))
        }
    };
    Ok((StatusCode::CREATED, Json(views::mod_entry_to_map(&entry))))
}

/// POST /api/instances/{id}/modpacks/install
pub async fn install_modpack(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(body): Json<InstallRequest>,
) -> Result<(StatusCode, Json<serde_json::Value>), ApiError> {
    let Some(project_id) = &body.project_id else {
        return Err(ApiError::BadRequest("projectId is required".to_string()));
    };
    let mods = mods_for(&state, &id)?;
    let result = mods
        .install_modrinth_modpack(project_id, body.version_id.as_deref())
        .await?;
    Ok((
        StatusCode::CREATED,
        Json(serde_json::json!({
            "installedCount": result.installed_count,
            "failedMods": result.failed_mods,
            "message": result.message,
        })),
    ))
}
