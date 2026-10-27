
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::Json;


use crate::web::controllers::mod_controller;
use crate::web::app::{ApiError, AppState};

use super::helpers::*;
pub async fn export_instance_share_code(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let bom = bom_for(&state, &id)?.get_bom();
    let code = bom.to_share_code().map_err(|e| ApiError::Internal(e.to_string()))?;
    Ok(Json(serde_json::json!({
        "code": code,
        "modCount": bom.mods.len(),
        "minecraftVersion": bom.minecraft_version,
        "modLoader": bom.mod_loader,
    })))
}

/// GET /api/instances/{id}/mods/export/markdown
pub async fn export_instance_markdown(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let bom = bom_for(&state, &id)?.get_bom();
    let markdown = bom.to_markdown_table();
    Ok(Json(serde_json::json!({
        "markdown": markdown,
        "modCount": bom.mods.len(),
    })))
}

/// GET /api/instances/{id}/pack.mrpack — generates and streams a spec-compliant
/// Modrinth modpack (.mrpack) containing the instance's active Bill of Materials (BOM)
/// and custom configurations from overrides/.
pub async fn download_instance_mrpack(
    State(state): State<AppState>,
    Path(id): Path<String>,
    headers: axum::http::HeaderMap,
) -> Result<axum::response::Response, ApiError> {
    let instance = state
        .instances
        .get_instance(&id)
        .map_err(|_| ApiError::NotFound("Instance not found".to_string()))?;

    let bom = bom_for(&state, &id)?.get_bom();
    let instance_dir = state.instances.get_instance_dir(&id);

    let req_host = headers
        .get(axum::http::header::HOST)
        .and_then(|h| h.to_str().ok())
        .map(|h| h.split(':').next().unwrap_or(h))
        .unwrap_or("localhost");
    let resolved_server_addr = instance.resolved_connection_address(req_host);

    let mut zip_bytes = std::io::Cursor::new(Vec::new());
    let options = zircon_core::export::MrpackExportOptions {
        pack_name: instance.name.clone(),
        version_name: format!("{}-{}", instance.minecraft_version, chrono::Utc::now().format("%Y%m%d")),
        summary: Some(format!("Exported from Zircon for {}", instance.name)),
        include_configs: true,
        include_resourcepacks: true,
        custom_download_base_url: None,
        server_address: Some(resolved_server_addr),
    };

    zircon_core::export::create_instance_mrpack(&mut zip_bytes, &bom, &instance_dir, &options)
        .map_err(|e| ApiError::Internal(format!("Failed to build .mrpack archive: {e}")))?;

    let payload = zip_bytes.into_inner();
    let safe_filename = instance.name.replace(|c: char| !c.is_alphanumeric() && c != '-' && c != '_', "_");

    axum::response::Response::builder()
        .header(
            axum::http::header::CONTENT_TYPE,
            "application/x-modrinth-modpack+zip",
        )
        .header(
            axum::http::header::CONTENT_DISPOSITION,
            format!("attachment; filename=\"{safe_filename}.mrpack\""),
        )
        .body(axum::body::Body::from(payload))
        .map_err(|e| ApiError::Internal(format!("Failed to build HTTP response: {e}")))
}

/// GET /pack.mrpack — public route that downloads the .mrpack for the active/running instance.
pub async fn download_active_mrpack(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
) -> Result<axum::response::Response, ApiError> {
    let instances = state.instances.list_instances();
    let target = instances
        .iter()
        .find(|i| state.instances.is_running(&i.id))
        .or_else(|| instances.first())
        .ok_or_else(|| ApiError::NotFound("No instances available".to_string()))?;

    download_instance_mrpack(State(state), Path(target.id.clone()), headers).await
}

/// POST /api/instances/{id}/mods/import
pub async fn import_instance_setup(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(payload): Json<mod_controller::ImportSetupPayload>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let incoming = if let Some(code) = &payload.code {
        zircon_core::model::BillOfMaterials::from_share_code(code)
            .map_err(|e| ApiError::BadRequest(e.to_string()))?
    } else if let Some(bom) = payload.bom {
        bom
    } else {
        return Err(ApiError::BadRequest("Either 'code' or 'bom' must be provided".to_string()));
    };

    let instance_dir = state.instances.get_instance_dir(&id);
    let bom_service = bom_for(&state, &id)?;

    let current = bom_service.get_bom();
    if !current.mods.is_empty() {
        let _ = zircon_core::export::snapshots::create_snapshot(&instance_dir, "Auto-Backup Pre-Import", &current);
    }

    if payload.strategy.eq_ignore_ascii_case("replace") {
        bom_service.with_bom(|b| {
            *b = incoming.clone();
        });
    } else {
        bom_service.with_bom(|b| {
            let mut existing_slugs: std::collections::HashSet<_> = b.mods.iter().map(|m| m.slug.clone()).collect();
            let mut existing_files: std::collections::HashSet<_> = b.mods.iter().map(|m| m.filename.clone()).collect();
            for m in incoming.mods {
                if !existing_slugs.contains(&m.slug) && !existing_files.contains(&m.filename) {
                    existing_slugs.insert(m.slug.clone());
                    existing_files.insert(m.filename.clone());
                    b.mods.push(m);
                }
            }
            b.deduplicate_mods();
        });
    }
    bom_service.save().map_err(|e| ApiError::Internal(e.to_string()))?;

    let final_bom = bom_service.get_bom();
    Ok(Json(serde_json::json!({
        "success": true,
        "totalMods": final_bom.mods.len(),
        "minecraftVersion": final_bom.minecraft_version,
    })))
}

/// GET /api/instances/{id}/mods/snapshots
pub async fn list_instance_snapshots(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let instance_dir = state.instances.get_instance_dir(&id);
    let list = zircon_core::export::snapshots::list_snapshots(&instance_dir)
        .map_err(|e| ApiError::Internal(e.to_string()))?;
    Ok(Json(serde_json::json!({ "snapshots": list })))
}

/// POST /api/instances/{id}/mods/snapshots
pub async fn create_instance_snapshot(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(payload): Json<mod_controller::CreateSnapshotPayload>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let instance_dir = state.instances.get_instance_dir(&id);
    let bom = bom_for(&state, &id)?.get_bom();
    let label = payload.label.as_deref().unwrap_or("Manual Snapshot");
    let info = zircon_core::export::snapshots::create_snapshot(&instance_dir, label, &bom)
        .map_err(|e| ApiError::Internal(e.to_string()))?;
    Ok(Json(serde_json::to_value(info).unwrap_or_default()))
}

/// POST /api/instances/{id}/mods/snapshots/{filename}/restore
pub async fn restore_instance_snapshot(
    State(state): State<AppState>,
    Path((id, filename)): Path<(String, String)>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let instance_dir = state.instances.get_instance_dir(&id);
    let bom_service = bom_for(&state, &id)?;
    let current_bom = bom_service.get_bom();

    if !current_bom.mods.is_empty() {
        let _ = zircon_core::export::snapshots::create_snapshot(&instance_dir, "Auto-Backup Pre-Restore", &current_bom);
    }

    let target_bom = zircon_core::export::snapshots::load_snapshot(&instance_dir, &filename)
        .map_err(|e| ApiError::NotFound(e.to_string()))?;

    let diff = zircon_core::export::diff_boms(&current_bom, &target_bom);

    bom_service.with_bom(|b| {
        *b = target_bom.clone();
    });
    bom_service.save().map_err(|e| ApiError::Internal(e.to_string()))?;

    Ok(Json(serde_json::json!({
        "success": true,
        "diff": diff,
        "totalMods": target_bom.mods.len(),
    })))
}

/// DELETE /api/instances/{id}/mods/snapshots/{filename}
pub async fn delete_instance_snapshot(
    State(state): State<AppState>,
    Path((id, filename)): Path<(String, String)>,
) -> Result<StatusCode, ApiError> {
    let instance_dir = state.instances.get_instance_dir(&id);
    let deleted = zircon_core::export::snapshots::delete_snapshot(&instance_dir, &filename)
        .map_err(|e| ApiError::Internal(e.to_string()))?;
    if deleted {
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(ApiError::NotFound("Snapshot not found".to_string()))
    }
}
