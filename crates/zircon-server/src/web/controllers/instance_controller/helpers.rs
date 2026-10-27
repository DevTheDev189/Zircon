use std::sync::Arc;

use axum::extract::Multipart;
use axum::http::StatusCode;
use axum::Json;

use serde::Deserialize;
use zircon_core::model::{BillOfMaterials, InstanceConfig};

use crate::web::controllers::config_helpers::command_result;
use crate::web::controllers::vanilla_player_files;
use crate::instance::ModSyncSummary;
use crate::services::bom::BomService;
use crate::services::mods::ModManagementService;
use crate::services::packs::PackManagementService;
use crate::web::app::{ApiError, AppState};
use crate::web::views;

// --------------------------------------------------------------------------
// helpers
// --------------------------------------------------------------------------

pub fn server_dir(state: &AppState, id: &str) -> std::path::PathBuf {
    state.instances.get_instance_dir(id).join("server")
}

/// Freshly built per-instance mod service (disk is always the source of truth).
pub fn mods_for(state: &AppState, id: &str) -> Result<ModManagementService, ApiError> {
    let cfg = state.instances.get_instance(id)?;
    let instance_dir = state.instances.get_instance_dir(id);
    let bom = Arc::new(
        BomService::new(
            instance_dir.join("bom.json"),
            Some(BillOfMaterials::new(
                cfg.minecraft_version.clone(),
                cfg.mod_loader.clone(),
                Some(cfg.name.clone()),
            )),
        )
        .with_signing_key(state.signing_key.clone()),
    );
    let curseforge_key = state.config.effective_curseforge_key();
    Ok(ModManagementService::new(
        bom,
        instance_dir.join("mods"),
        &curseforge_key,
    ))
}

/// Freshly built per-instance pack service.
pub fn packs_for(state: &AppState, id: &str) -> Result<PackManagementService, ApiError> {
    let cfg = state.instances.get_instance(id)?;
    let instance_dir = state.instances.get_instance_dir(id);
    let bom = Arc::new(
        BomService::new(
            instance_dir.join("bom.json"),
            Some(BillOfMaterials::new(
                cfg.minecraft_version.clone(),
                cfg.mod_loader.clone(),
                Some(cfg.name.clone()),
            )),
        )
        .with_signing_key(state.signing_key.clone()),
    );
    let curseforge_key = state.config.effective_curseforge_key();
    Ok(PackManagementService::new(
        bom,
        instance_dir.join("shaderpacks"),
        instance_dir.join("resourcepacks"),
    )
    .with_curseforge_key(&curseforge_key))
}

/// Sends a command to the instance's own server process (no-op when offline).
pub async fn send_instance_command(
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

pub fn add_ban_offline(
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

pub fn live_instance_map(state: &AppState, config: &InstanceConfig) -> serde_json::Value {
    let running = state.instances.is_running(&config.id);
    let ready = state.instances.is_server_ready(&config.id);
    let stopping_reason = state.instances.stopping_reason(&config.id);
    let stopping = stopping_reason.is_some();
    views::instance_to_map(
        config,
        running,
        ready,
        state.instances.get_online_player_count(&config.id),
        state.instances.get_online_players(&config.id),
        state.instances.get_idle_remaining_seconds(&config.id),
        stopping,
        stopping_reason,
    )
}

pub fn sync_result_to_value(summary: &ModSyncSummary) -> serde_json::Value {
    serde_json::json!({
        "updatedCount": summary.updated_count,
        "incompatibleCount": summary.incompatible_count,
        "updatedMods": summary.updated_mods,
        "incompatibleMods": summary.incompatible_mods,
    })
}

pub fn valid_schedule(frequency: Option<&str>, time: Option<&str>) -> bool {
    if let Some(f) = frequency {
        let f = f.trim();
        if !f.is_empty() && !zircon_core::model::instance::is_valid_backup_frequency(f) {
            return false;
        }
    }
    if let Some(t) = time {
        let t = t.trim();
        // Accept "HH:MM" and "H:MM" (and empty = no time change).
        if !t.is_empty()
            && chrono::NaiveTime::parse_from_str(t, "%H:%M").is_err()
            && chrono::NaiveTime::parse_from_str(t, "%k:%M").is_err()
        {
            return false;
        }
    }
    true
}

pub async fn upload_pack(
    state: &AppState,
    id: &str,
    params: &crate::web::controllers::mod_controller::OriginParam,
    mut multipart: Multipart,
    shader: bool,
) -> Result<(StatusCode, Json<serde_json::Value>), ApiError> {
    let Some((filename, bytes)) = crate::web::controllers::mod_controller::take_upload(&mut multipart).await? else {
        return Err(ApiError::BadRequest(
            "No file uploaded (form field 'file')".to_string(),
        ));
    };
    let packs = packs_for(state, id)?;
    let expected_mod_id = params.expected_mod_id.as_deref().or(params.mod_id.as_deref());
    let entry = if shader {
        packs
            .add_shaderpack_with_metadata(
                std::io::Cursor::new(bytes),
                &filename,
                params.origin.as_deref(),
                params.icon_url.as_deref(),
                params.title.as_deref(),
                expected_mod_id,
                params.expected_file_id.as_deref(),
                params.project_url.as_deref(),
            )
            .await?
    } else {
        packs
            .add_resourcepack_with_metadata(
                std::io::Cursor::new(bytes),
                &filename,
                params.origin.as_deref(),
                params.icon_url.as_deref(),
                params.title.as_deref(),
                expected_mod_id,
                params.expected_file_id.as_deref(),
                params.project_url.as_deref(),
            )
            .await?
    };
    Ok((
        StatusCode::CREATED,
        Json(views::pack_entry_to_map(&entry, shader)),
    ))
}

pub async fn install_pack(
    state: &AppState,
    id: &str,
    body: InstallRequest,
    shader: bool,
) -> Result<(StatusCode, Json<serde_json::Value>), ApiError> {
    let packs = packs_for(state, id)?;
    let origin_is_curseforge = body
        .origin
        .as_deref()
        .map_or(false, |o| o.eq_ignore_ascii_case("curseforge"));

    let entry = if let (Some(download_url), Some(filename)) = (&body.download_url, &body.filename) {
        let origin = body.origin.as_deref().or(if origin_is_curseforge {
            Some("curseforge")
        } else {
            Some("modrinth")
        });
        if shader {
            packs
                .install_shaderpack_from_url(
                    download_url,
                    filename,
                    origin,
                )
                .await?
        } else {
            packs
                .install_resourcepack_from_url(
                    download_url,
                    filename,
                    origin,
                )
                .await?
        }
    } else if let Some(project_id) = &body.project_id {
        if origin_is_curseforge {
            return Err(ApiError::BadRequest(
                "downloadUrl and filename are required for curseforge pack installation".to_string(),
            ));
        }
        packs
            .install_modrinth_pack(project_id, body.version_id.as_deref(), shader)
            .await?
    } else {
        return Err(ApiError::BadRequest(
            "projectId or (downloadUrl and filename) is required".to_string(),
        ));
    };
    Ok((
        StatusCode::CREATED,
        Json(views::pack_entry_to_map(&entry, shader)),
    ))
}

pub async fn remove_pack(
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
    #[serde(alias = "serverUrl")]
    pub subdomain: Option<String>,
    pub mc_version: Option<String>,
    pub loader_type: Option<String>,
    pub loader_version: Option<String>,
    pub auto_start: Option<bool>,
    #[serde(alias = "autoStartServer")]
    pub auto_start_server: Option<bool>,
    /// Optional initial JVM args (e.g. the RAM slider's heap flags). When
    /// absent/blank the instance keeps the wrapper default (-Xms2G -Xmx4G).
    pub java_args: Option<String>,
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CloneRequest {
    pub name: Option<String>,
    pub loader_type: Option<String>,
    pub loader_version: Option<String>,
    #[serde(default = "default_true")]
    pub copy_world: bool,
    #[serde(default = "default_true")]
    pub copy_mods: bool,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateRequest {
    /// Frontend sends `serverTitle` on the settings screen; the instance API
    /// historically used `name`. Both bind here.
    #[serde(alias = "serverTitle")]
    pub name: Option<String>,
    /// Customer subdomain or server URL (e.g. "play.zirconmc.net" or "mc").
    #[serde(alias = "serverUrl")]
    pub subdomain: Option<String>,
    #[serde(alias = "minecraftVersion")]
    pub mc_version: Option<String>,
    #[serde(alias = "modLoaderType", alias = "loaderType")]
    pub loader_type: Option<String>,
    #[serde(alias = "modLoaderVersion")]
    pub loader_version: Option<String>,
    pub java_args: Option<String>,
    pub auto_start: Option<bool>,
    #[serde(alias = "autoStartServer")]
    pub auto_start_server: Option<bool>,
    pub backup_frequency: Option<String>,
    pub backup_time: Option<String>,
    /// How many backups to keep (0 = unlimited).
    pub backup_retention: Option<i32>,
    /// Player-facing port; 0 / absent leaves it unchanged.
    #[serde(default)]
    pub external_port: i32,
    /// Idle shutdown: put the server to sleep when nobody is playing, and let
    /// the launcher wake it on the next join.
    pub idle_shutdown_enabled: Option<bool>,
    /// Idle window in minutes (clamped to 1–60 server-side).
    pub idle_shutdown_minutes: Option<u32>,
    /// Join gate mode: Universal, WhitelistOnly, or StrictTicket.
    pub join_gate_mode: Option<zircon_core::model::instance::JoinGateMode>,
    /// Homelab public connection address override (e.g. "mc.mydomain.com:25565").
    pub public_address: Option<String>,
    /// Partial `server.properties` update applied alongside the other fields.
    pub server_properties: Option<std::collections::BTreeMap<String, String>>,
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
pub struct ModFilenamesRequest {
    #[serde(default)]
    pub filenames: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JoinIntentRequest {
    pub username: Option<String>,
    pub uuid: Option<String>,
    /// The host the client will connect to; matched against instance id/name
    /// like the multiplexer's hostname routing, so the hold can be applied to
    /// the right instance. Optional — resolution falls back to the port, the
    /// public port, then the active instance.
    pub hostname: Option<String>,
    /// The player-facing port the client will connect to.
    pub port: Option<u16>,
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

pub fn bom_for(state: &AppState, id: &str) -> Result<Arc<BomService>, ApiError> {
    let cfg = state.instances.get_instance(id)?;
    let instance_dir = state.instances.get_instance_dir(id);
    Ok(Arc::new(
        BomService::new(
            instance_dir.join("bom.json"),
            Some(BillOfMaterials::new(
                cfg.minecraft_version.clone(),
                cfg.mod_loader.clone(),
                Some(cfg.name.clone()),
            )),
        )
        .with_signing_key(state.signing_key.clone()),
    ))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CrashFixRequest {
    pub action: crate::services::crash_analyzer::CrashRemediationAction,
    #[serde(default)]
    pub auto_restart: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SetServerPackRequest {
    pub filename: Option<String>,
    #[serde(default)]
    pub filenames: Option<Vec<String>>,
}
