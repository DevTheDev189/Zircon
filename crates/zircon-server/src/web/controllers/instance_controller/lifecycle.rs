
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::Json;

use tokio::time::Duration;
use zircon_core::model::ModLoaderType;

use crate::web::app::{ApiError, AppState};

use super::helpers::*;
/// GET /api/instances
pub async fn list_instances(State(state): State<AppState>) -> Json<serde_json::Value> {
    let instances: Vec<serde_json::Value> = state
        .instances
        .list_instances()
        .iter()
        .map(|cfg| live_instance_map(&state, cfg))
        .collect();
    Json(serde_json::json!({ "instances": instances }))
}

/// POST /api/instances — body: {name, mcVersion, loaderType, loaderVersion}
pub async fn create_instance(
    State(state): State<AppState>,
    Json(body): Json<CreateRequest>,
) -> Result<(StatusCode, Json<serde_json::Value>), ApiError> {
    let name = body.name.ok_or_else(|| {
        ApiError::BadRequest("name, mcVersion and loaderType are required".to_string())
    })?;
    let mc_version = body.mc_version.ok_or_else(|| {
        ApiError::BadRequest("name, mcVersion and loaderType are required".to_string())
    })?;
    let raw_loader = body.loader_type.ok_or_else(|| {
        ApiError::BadRequest("name, mcVersion and loaderType are required".to_string())
    })?;
    let approved_loader = ModLoaderType::from_id(raw_loader.trim()).ok_or_else(|| {
        ApiError::BadRequest(format!( /* z0 */
            "Invalid loaderType '{}'. Allowed loaders: {}", // z0
            raw_loader.trim(),
            ModLoaderType::ALLOWED_IDS.join(", ") /* z0 */
        )) /* z0 */
    })?; // z0
    let loader_version = body.loader_version.unwrap_or_default();
    let created = state.instances.create_instance(
        name.trim(),
        mc_version.trim(),
        approved_loader.id(),
        loader_version.trim(),
    )?;
    if let Some(auto) = body.auto_start.or(body.auto_start_server) {
        state.instances.update_auto_start(&created.id, auto)?;
    }
    if let Some(sub) = body.subdomain.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
        state.instances.update_subdomain(&created.id, Some(sub.to_string()))?;
    }
    if let Some(args) = body
        .java_args
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        state
            .instances
            .update_instance_config(&created.id, None, Some(args))?;
    }
    let fresh = state.instances.get_instance(&created.id).unwrap_or(created);
    Ok((
        StatusCode::CREATED,
        Json(live_instance_map(&state, &fresh)),
    ))
}

/// POST /api/instances/{id}/clone
pub async fn clone_instance(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(body): Json<CloneRequest>,
) -> Result<(StatusCode, Json<serde_json::Value>), ApiError> {
    let new_name = body.name.unwrap_or_default();
    let cloned = state.instances.clone_instance(
        &id,
        &new_name,
        body.loader_type.as_deref(),
        body.loader_version.as_deref(),
        body.copy_world,
        body.copy_mods,
    )?;
    Ok((
        StatusCode::CREATED,
        Json(live_instance_map(&state, &cloned)),
    ))
}

/// GET /api/instances/{id}
pub async fn get_instance(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let config = state.instances.get_instance(&id)?;
    Ok(Json(live_instance_map(&state, &config)))
}

/// PATCH /api/instances/{id}
pub async fn update_instance(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(body): Json<UpdateRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    // Manual player-facing port override — rebinds the multiplexer listener.
    if body.external_port > 0 {
        if let Err(e) = state
            .instances
            .update_external_port(&id, body.external_port)
        {
            return Err(e.into());
        }
    }
    // Auto-start on wrapper boot.
    if let Some(auto) = body.auto_start.or(body.auto_start_server) {
        state.instances.update_auto_start(&id, auto)?;
    }
    // Backup retention (0 = keep everything).
    if let Some(retention) = body.backup_retention {
        state.instances.update_backup_retention(&id, retention)?;
    }
    // Backup schedule changes are independent of version re-sync.
    if body.backup_frequency.is_some() || body.backup_time.is_some() {
        if !valid_schedule(
            body.backup_frequency.as_deref(),
            body.backup_time.as_deref(),
        ) {
            return Err(ApiError::BadRequest(
                "backupFrequency must be one of off, daily, weekly, monthly and backupTime must be in HH:MM 24-hour format"
                    .to_string(),
            ));
        }
        state.instances.update_backup_schedule(
            &id,
            body.backup_frequency.as_deref(),
            body.backup_time.as_deref(),
        )?;
    }
    // Idle shutdown settings (sleep when nobody is playing).
    if body.idle_shutdown_enabled.is_some() || body.idle_shutdown_minutes.is_some() {
        state.instances.update_idle_shutdown_settings(
            &id,
            body.idle_shutdown_enabled,
            body.idle_shutdown_minutes,
        )?;
    }
    // Join gate mode (universal, whitelist_only, strict_ticket).
    if let Some(gate_mode) = body.join_gate_mode {
        state.instances.update_join_gate_mode(&id, gate_mode)?;
    }
    // Homelab public connection address override
    if let Some(addr) = &body.public_address {
        let clean = if addr.trim().is_empty() {
            None
        } else {
            Some(addr.trim().to_string())
        };
        state.instances.update_public_address(&id, clean)?;
    }
    // Subdomain / Server URL for hostname multiplexing
    if let Some(sub) = &body.subdomain {
        let clean = if sub.trim().is_empty() {
            None
        } else {
            Some(sub.trim().to_string())
        };
        state.instances.update_subdomain(&id, clean)?;
    }
    // Partial server.properties update (e.g. MOTD / max-players from the
    // settings screen) saved alongside the other fields.
    if let Some(props) = &body.server_properties {
        if !props.is_empty() {
            let server_dir = state.instances.get_instance_dir(&id).join("server");
            std::fs::create_dir_all(&server_dir)?;
            let props_file = server_dir.join("server.properties");
            let mut current = if props_file.is_file() {
                crate::config::ServerProperties::load(&props_file)?
            } else {
                crate::config::ServerProperties::default()
            };
            for (key, value) in props {
                current.set(key, value);
            }
            current.save(&props_file)?;
        }
    }

    let current = state.instances.get_instance(&id)?;
    if let Some(ref lt) = body.loader_type {
        let trimmed = lt.trim();
        if !trimmed.is_empty() && ModLoaderType::from_id(trimmed).is_none() {
            return Err(ApiError::BadRequest(format!(
                "Invalid loaderType '{}'. Allowed loaders: {}",
                trimmed,
                ModLoaderType::ALLOWED_IDS.join(", ")
            )));
        }
    }
    let mc_changed = body
        .mc_version
        .as_deref()
        .map(|v| !v.trim().is_empty() && v != current.minecraft_version)
        .unwrap_or(false);
    let loader_changed = body
        .loader_version
        .as_deref()
        .map(|v| !v.trim().is_empty() && v != current.loader_version())
        .unwrap_or(false);
    let loader_type_changed = body
        .loader_type
        .as_deref()
        .map(|v| !v.trim().is_empty() && v != current.loader_type())
        .unwrap_or(false);
    let version_change = mc_changed || loader_changed || loader_type_changed;

    if version_change {
        // Keep javaArgs changes from getting lost in the version-sync path.
        if let Some(java_args) = &body.java_args {
            state
                .instances
                .update_instance_config(&id, None, Some(java_args))?;
        }
        let sync_result = state
            .instances
            .update_instance_versions(
                &id,
                body.mc_version.as_deref(),
                body.loader_type.as_deref(),
                body.loader_version.as_deref(),
                body.name.as_deref(),
            )
            .await?;
        let updated = state.instances.get_instance(&id)?;
        let mut response = serde_json::json!(sync_result_to_value(&sync_result));
        response["instance"] = live_instance_map(&state, &updated);
        Ok(Json(response))
    } else {
        state.instances.update_instance_config(
            &id,
            body.name.as_deref(),
            body.java_args.as_deref(),
        )?;
        let updated = state.instances.get_instance(&id)?;
        Ok(Json(live_instance_map(&state, &updated)))
    }
}

/// DELETE /api/instances/{id}
pub async fn delete_instance(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<StatusCode, ApiError> {
    let deleted = state.instances.delete_instance(&id).await?;
    if !deleted {
        return Err(ApiError::NotFound("Instance not found".to_string()));
    }
    Ok(StatusCode::NO_CONTENT)
}

/// POST /api/instances/{id}/start
pub async fn start_instance(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    state.instances.start_instance(&id).await?;
    Ok(Json(serde_json::json!({ "ok": true })))
}

/// POST /api/instances/{id}/stop
pub async fn stop_instance(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Json<serde_json::Value> {
    state
        .instances
        .stop_instance_with_reason(&id, Some(zircon_core::model::SHUTDOWN_REASON_MANUAL))
        .await;
    Json(serde_json::json!({ "ok": true }))
}

/// POST /api/instances/{id}/restart — stops the instance, then starts it again shortly after.
pub async fn restart_instance(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    state.instances.get_instance(&id)?; // 404 for unknown ids
    let instances = state.instances.clone();
    state.instances.stop_instance(&id).await;
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(1500)).await; // allow OS process cleanup
        if let Err(e) = instances.start_instance(&id).await {
            tracing::error!("Failed to restart instance {id}: {e}");
        }
    });
    Ok(Json(
        serde_json::json!({ "ok": true, "message": "Server is restarting..." }),
    ))
}

/// GET /api/instances/{id}/eula — EULA acceptance status.
pub async fn get_eula(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    state.instances.get_instance(&id)?; // 404 for unknown ids
    Ok(Json(serde_json::json!({
        "accepted": state.instances.is_eula_accepted(&id),
        "eulaUrl": "https://aka.ms/MinecraftEULA"
    })))
}

/// POST /api/instances/{id}/eula — body: {"accepted":true} records EULA consent.
pub async fn accept_eula(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(body): Json<EulaRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    if !body.accepted {
        return Err(ApiError::BadRequest(
            r#"{"accepted":true} is required to accept the EULA"#.to_string(),
        ));
    }
    state.instances.accept_eula(&id)?;
    Ok(Json(serde_json::json!({ "accepted": true })))
}
