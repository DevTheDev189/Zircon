//! Multi-instance REST controller: instance CRUD, start/stop/restart, EULA,
//! `server.properties`, per-instance players, mods, packs and backups.
//!
//! Port of `com.mcmanager.server.web.controller.InstanceController`.

use std::sync::Arc;

use axum::extract::{Multipart, Path, Query, State};
use axum::http::StatusCode;
use axum::Json;

use serde::Deserialize;
use tokio::time::Duration;
use zircon_core::model::{BillOfMaterials, InstanceConfig};

use super::config_helpers::{command_result, read_player_json, PlayerActionRequest};
use super::vanilla_player_files;
use crate::instance::ModSyncSummary;
use crate::services::bom::BomService;
use crate::services::mods::ModManagementService;
use crate::services::packs::PackManagementService;
use crate::tickets::TICKET_TTL_SECONDS;
use crate::web::app::{ApiError, AppState};
use crate::web::views;

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
    let loader_type = body.loader_type.ok_or_else(|| {
        ApiError::BadRequest("name, mcVersion and loaderType are required".to_string())
    })?;
    let loader_version = body.loader_version.unwrap_or_default();
    let created = state.instances.create_instance(
        name.trim(),
        mc_version.trim(),
        loader_type.trim().to_lowercase().as_str(),
        loader_version.trim(),
    )?;
    Ok((
        StatusCode::CREATED,
        Json(live_instance_map(&state, &created)),
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

    let current = state.instances.get_instance(&id)?;
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
    let version_change = mc_changed || loader_changed;

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
    state.instances.stop_instance(&id).await;
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

/// GET /api/instances/{id}/server-properties — the instance's server.properties as a map.
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

// --------------------------------------------------------------------------
// Per-instance player management
// --------------------------------------------------------------------------

/// GET /api/instances/{id}/players/online
pub async fn online_players(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    state.instances.get_instance(&id)?;
    let players = state.instances.get_online_players(&id);
    Ok(Json(serde_json::json!({ "players": players })))
}

/// GET /api/instances/{id}/players/history — every player that has ever joined.
pub async fn player_history(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    state.instances.get_instance(&id)?;
    let history_file = state.instances.get_instance_dir(&id).join("players.json");
    let players: Vec<serde_json::Value> =
        crate::process::player_tracker::PlayerTracker::load_history(&history_file)
            .iter()
            .map(views::player_history_to_map)
            .collect();
    Ok(Json(serde_json::json!({ "players": players })))
}

/// GET /api/instances/{id}/players/whitelist
pub async fn get_whitelist(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    state.instances.get_instance(&id)?;
    Ok(Json(
        serde_json::json!({ "players": read_player_json(&server_dir(&state, &id), "whitelist.json") }),
    ))
}

/// POST /api/instances/{id}/players/whitelist
pub async fn add_whitelist(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(body): Json<PlayerActionRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let name = body
        .name
        .ok_or_else(|| ApiError::BadRequest("name is required".to_string()))?;
    state.instances.get_instance(&id)?;
    Ok(send_instance_command(&state, &id, &format!("whitelist add {name}")).await)
}

/// DELETE /api/instances/{id}/players/whitelist/{name}
pub async fn remove_whitelist(
    State(state): State<AppState>,
    Path((id, name)): Path<(String, String)>,
) -> Result<Json<serde_json::Value>, ApiError> {
    state.instances.get_instance(&id)?;
    Ok(send_instance_command(&state, &id, &format!("whitelist remove {name}")).await)
}

/// GET /api/instances/{id}/players/ops
pub async fn get_ops(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    state.instances.get_instance(&id)?;
    Ok(Json(
        serde_json::json!({ "players": read_player_json(&server_dir(&state, &id), "ops.json") }),
    ))
}

/// POST /api/instances/{id}/players/ops
pub async fn add_op(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(body): Json<PlayerActionRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let name = body
        .name
        .ok_or_else(|| ApiError::BadRequest("name is required".to_string()))?;
    state.instances.get_instance(&id)?;
    Ok(send_instance_command(&state, &id, &format!("op {name}")).await)
}

/// DELETE /api/instances/{id}/players/ops/{name}
pub async fn remove_op(
    State(state): State<AppState>,
    Path((id, name)): Path<(String, String)>,
) -> Result<Json<serde_json::Value>, ApiError> {
    state.instances.get_instance(&id)?;
    Ok(send_instance_command(&state, &id, &format!("deop {name}")).await)
}

/// GET /api/instances/{id}/players/bans
pub async fn get_bans(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    state.instances.get_instance(&id)?;
    Ok(Json(
        serde_json::json!({ "players": read_player_json(&server_dir(&state, &id), "banned-players.json") }),
    ))
}

/// POST /api/instances/{id}/players/bans — online or offline ban.
pub async fn add_ban(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(body): Json<PlayerActionRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let name = body
        .name
        .ok_or_else(|| ApiError::BadRequest("name is required".to_string()))?;
    state.instances.get_instance(&id)?;
    if state.instances.is_running(&id) {
        let reason = body
            .reason
            .filter(|r| !r.trim().is_empty())
            .map(|r| format!(" {}", r.trim()))
            .unwrap_or_default();
        Ok(send_instance_command(&state, &id, &format!("ban {}{reason}", name.trim())).await)
    } else {
        add_ban_offline(&state, &id, name.trim(), body.reason.as_deref())?;
        Ok(Json(serde_json::json!({ "ok": true, "offline": true })))
    }
}

/// DELETE /api/instances/{id}/players/bans/{name} — pardon, online or offline.
pub async fn remove_ban(
    State(state): State<AppState>,
    Path((id, name)): Path<(String, String)>,
) -> Result<Json<serde_json::Value>, ApiError> {
    state.instances.get_instance(&id)?;
    if state.instances.is_running(&id) {
        Ok(send_instance_command(&state, &id, &format!("pardon {name}")).await)
    } else {
        let file = server_dir(&state, &id).join("banned-players.json");
        let removed = vanilla_player_files::pardon(&file, &name)?;
        Ok(Json(
            serde_json::json!({ "ok": true, "offline": true, "removed": removed }),
        ))
    }
}

/// GET /api/instances/{id}/bom — the instance's mod list.
pub async fn get_instance_bom(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let mods = mods_for(&state, &id)?.list_mods();
    let mapped: Vec<serde_json::Value> = mods.iter().map(views::mod_entry_to_map).collect();
    Ok(Json(serde_json::Value::Array(mapped)))
}

/// POST /api/join-intent and /api/instances/{id}/join-intent — registers a
/// short-lived join ticket for the launcher's session so the player's
/// connection passes the Zircon join gate. Intentionally unauthenticated.
pub async fn register_join_intent(
    State(state): State<AppState>,
    Json(body): Json<JoinIntentRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    if body.username.is_none() && body.uuid.is_none() {
        return Err(ApiError::BadRequest(
            "username or uuid is required".to_string(),
        ));
    }
    if let Some(username) = &body.username {
        state.tickets.register_ticket(username);
    }
    if let Some(uuid) = &body.uuid {
        state.tickets.register_ticket(uuid);
    }
    Ok(Json(
        serde_json::json!({ "ok": true, "expiresInSeconds": TICKET_TTL_SECONDS }),
    ))
}

// --------------------------------------------------------------------------
// Per-instance mods
// --------------------------------------------------------------------------

/// GET /api/instances/{id}/mods
pub async fn list_mods(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let mods = mods_for(&state, &id)?.list_mods();
    let mapped: Vec<serde_json::Value> = mods.iter().map(views::mod_entry_to_map).collect();
    Ok(Json(serde_json::json!({ "mods": mapped })))
}

/// POST /api/instances/{id}/mods/upload (multipart, field "file")
pub async fn upload_mod(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Query(params): Query<super::mod_controller::OriginParam>,
    mut multipart: Multipart,
) -> Result<(StatusCode, Json<serde_json::Value>), ApiError> {
    let Some((filename, bytes)) = super::mod_controller::take_upload(&mut multipart).await? else {
        return Err(ApiError::BadRequest(
            "No file uploaded (form field 'file')".to_string(),
        ));
    };
    let entry = mods_for(&state, &id)?
        .add_mod(
            std::io::Cursor::new(bytes),
            &filename,
            params.origin.as_deref(),
        )
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

/// GET /api/instances/{id}/mods/search
pub async fn search_mods(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Query(params): Query<super::mod_controller::SearchParams>,
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
            .search_mods(&query, params.mc_version.as_deref())
            .await
            .map_err(|e| ApiError::BadGateway(e.to_string()))?;
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
    Query(params): Query<super::mod_controller::VersionParams>,
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
    Query(params): Query<super::mod_controller::CurseForgeParams>,
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
            let (Some(download_url), Some(filename)) = (&body.download_url, &body.filename) else {
                return Err(ApiError::BadRequest(
                    "downloadUrl and filename are required for curseforge".to_string(),
                ));
            };
            let mut entry = mods
                .install_from_url(download_url, filename, "curseforge")
                .await?;
            if let Some(file_id) = &body.file_id {
                entry.id = Some(file_id.to_string());
            }
            entry
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

// --------------------------------------------------------------------------
// Per-instance shaders & texture packs
// --------------------------------------------------------------------------

/// GET /api/instances/{id}/shaderpacks
pub async fn list_shaderpacks(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let packs = packs_for(&state, &id)?;
    let mapped: Vec<serde_json::Value> = packs
        .list_shaderpacks()
        .iter()
        .map(views::pack_entry_to_map)
        .collect();
    Ok(Json(serde_json::json!({ "shaderpacks": mapped })))
}

/// POST /api/instances/{id}/shaderpacks/upload
pub async fn upload_shaderpack(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Query(params): Query<super::mod_controller::OriginParam>,
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
        .map(views::pack_entry_to_map)
        .collect();
    Ok(Json(serde_json::json!({ "resourcepacks": mapped })))
}

/// POST /api/instances/{id}/resourcepacks/upload
pub async fn upload_resourcepack(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Query(params): Query<super::mod_controller::OriginParam>,
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

// --------------------------------------------------------------------------
// helpers
// --------------------------------------------------------------------------

fn server_dir(state: &AppState, id: &str) -> std::path::PathBuf {
    state.instances.get_instance_dir(id).join("server")
}

/// Freshly built per-instance mod service (disk is always the source of truth).
fn mods_for(state: &AppState, id: &str) -> Result<ModManagementService, ApiError> {
    let cfg = state.instances.get_instance(id)?;
    let instance_dir = state.instances.get_instance_dir(id);
    let bom = Arc::new(BomService::new(
        instance_dir.join("bom.json"),
        Some(BillOfMaterials::new(
            cfg.minecraft_version.clone(),
            cfg.mod_loader.clone(),
            Some(cfg.name.clone()),
        )),
    ));
    Ok(ModManagementService::new(
        bom,
        instance_dir.join("mods"),
        &state.curseforge_api_key,
    ))
}

/// Freshly built per-instance pack service.
fn packs_for(state: &AppState, id: &str) -> Result<PackManagementService, ApiError> {
    let cfg = state.instances.get_instance(id)?;
    let instance_dir = state.instances.get_instance_dir(id);
    let bom = Arc::new(BomService::new(
        instance_dir.join("bom.json"),
        Some(BillOfMaterials::new(
            cfg.minecraft_version.clone(),
            cfg.mod_loader.clone(),
            Some(cfg.name.clone()),
        )),
    ));
    Ok(PackManagementService::new(
        bom,
        instance_dir.join("shaderpacks"),
        instance_dir.join("resourcepacks"),
    ))
}

/// Sends a command to the instance's own server process (no-op when offline).
async fn send_instance_command(
    state: &AppState,
    instance_id: &str,
    command: &str,
) -> Json<serde_json::Value> {
    let pm = state.instances.get_process_manager(instance_id);
    match pm {
        Some(pm) if pm.is_running() => match pm.send_command(command).await {
            Ok(()) => command_result(command, true, None),
            Err(e) => command_result(command, false, Some(e.to_string())),
        },
        _ => command_result(
            command,
            false,
            Some("Server is not running — start it before managing players".to_string()),
        ),
    }
}

fn add_ban_offline(
    state: &AppState,
    instance_id: &str,
    name: &str,
    reason: Option<&str>,
) -> Result<(), ApiError> {
    let file = server_dir(state, instance_id).join("banned-players.json");
    let user_cache = server_dir(state, instance_id).join("usercache.json");
    let uuid = vanilla_player_files::resolve_uuid(&user_cache, name);
    vanilla_player_files::ban(&file, name, reason, &uuid)?;
    tracing::info!("Banned {name} (offline, instance {instance_id})");
    Ok(())
}

fn live_instance_map(state: &AppState, config: &InstanceConfig) -> serde_json::Value {
    views::instance_to_map(
        config,
        state.instances.is_running(&config.id),
        state.instances.get_online_player_count(&config.id),
        state.instances.get_online_players(&config.id),
    )
}

fn sync_result_to_value(summary: &ModSyncSummary) -> serde_json::Value {
    serde_json::json!({
        "updatedCount": summary.updated_count,
        "incompatibleCount": summary.incompatible_count,
        "updatedMods": summary.updated_mods,
        "incompatibleMods": summary.incompatible_mods,
    })
}

fn valid_schedule(frequency: Option<&str>, time: Option<&str>) -> bool {
    if let Some(f) = frequency {
        if !zircon_core::model::instance::is_valid_backup_frequency(f) {
            return false;
        }
    }
    if let Some(t) = time {
        let is_time = t.len() == 5
            && t.as_bytes()[2] == b':'
            && t.as_bytes()[0].is_ascii_digit()
            && t.as_bytes()[1].is_ascii_digit()
            && t.as_bytes()[3].is_ascii_digit()
            && t.as_bytes()[4].is_ascii_digit();
        if !is_time {
            return false;
        }
    }
    true
}

async fn upload_pack(
    state: &AppState,
    id: &str,
    params: &super::mod_controller::OriginParam,
    mut multipart: Multipart,
    shader: bool,
) -> Result<(StatusCode, Json<serde_json::Value>), ApiError> {
    let Some((filename, bytes)) = super::mod_controller::take_upload(&mut multipart).await? else {
        return Err(ApiError::BadRequest(
            "No file uploaded (form field 'file')".to_string(),
        ));
    };
    let packs = packs_for(state, id)?;
    let entry = if shader {
        packs
            .add_shaderpack(
                std::io::Cursor::new(bytes),
                &filename,
                params.origin.as_deref(),
            )
            .await?
    } else {
        packs
            .add_resourcepack(
                std::io::Cursor::new(bytes),
                &filename,
                params.origin.as_deref(),
            )
            .await?
    };
    Ok((StatusCode::CREATED, Json(views::pack_entry_to_map(&entry))))
}

async fn install_pack(
    state: &AppState,
    id: &str,
    body: InstallRequest,
    shader: bool,
) -> Result<(StatusCode, Json<serde_json::Value>), ApiError> {
    let (Some(download_url), Some(filename)) = (&body.download_url, &body.filename) else {
        return Err(ApiError::BadRequest(
            "downloadUrl and filename are required".to_string(),
        ));
    };
    let packs = packs_for(state, id)?;
    let entry = if shader {
        packs
            .install_shaderpack_from_url(
                download_url,
                filename,
                body.origin.as_deref().or(Some("modrinth")),
            )
            .await?
    } else {
        packs
            .install_resourcepack_from_url(
                download_url,
                filename,
                body.origin.as_deref().or(Some("modrinth")),
            )
            .await?
    };
    Ok((StatusCode::CREATED, Json(views::pack_entry_to_map(&entry))))
}

async fn remove_pack(
    state: &AppState,
    id: &str,
    filename: &str,
    shader: bool,
) -> Result<StatusCode, ApiError> {
    let packs = packs_for(state, id)?;
    let removed = if shader {
        packs.remove_shaderpack(filename)?
    } else {
        packs.remove_resourcepack(filename)?
    };
    if !removed {
        return Err(ApiError::NotFound("Pack not found".to_string()));
    }
    Ok(StatusCode::NO_CONTENT)
}

// --------------------------------------------------------------------------
// Request DTOs
// --------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateRequest {
    pub name: Option<String>,
    pub mc_version: Option<String>,
    pub loader_type: Option<String>,
    pub loader_version: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateRequest {
    pub name: Option<String>,
    pub mc_version: Option<String>,
    pub loader_version: Option<String>,
    pub java_args: Option<String>,
    pub backup_frequency: Option<String>,
    pub backup_time: Option<String>,
    /// Player-facing port; 0 / absent leaves it unchanged.
    #[serde(default)]
    pub external_port: i32,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstallRequest {
    pub origin: Option<String>,
    pub project_id: Option<String>,
    pub version_id: Option<String>,
    pub download_url: Option<String>,
    pub filename: Option<String>,
    pub file_id: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JoinIntentRequest {
    pub username: Option<String>,
    pub uuid: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EulaRequest {
    pub accepted: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerPropertiesRequest {
    pub properties: Option<std::collections::BTreeMap<String, String>>,
}
