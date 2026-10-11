use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, State};

use zircon_core::model::{BillOfMaterials, ModSide};

use crate::offline::OfflineInstance;
use crate::servers;

use super::*;

// ---------------------------------------------------------------------------
// Offline instances
// ---------------------------------------------------------------------------

#[tauri::command]
pub fn list_offline_instances(
    state: State<'_, LauncherState>,
) -> Result<Vec<OfflineInstance>, String> {
    Ok(state.offline.list())
}

#[tauri::command]
pub fn create_offline_instance(
    state: State<'_, LauncherState>,
    name: String,
    mc_version: String,
    loader_type: String,
    loader_version: Option<String>,
) -> Result<OfflineInstance, String> {
    let loader_ver = loader_version.as_deref().unwrap_or("");
    state
        .offline
        .create(&name, &mc_version, &loader_type, loader_ver)
        .map_err(err_string)
}

#[tauri::command]
pub fn delete_offline_instance(state: State<'_, LauncherState>, id: String) -> Result<(), String> {
    let Some(instance) = state.offline.load(&id) else {
        return Err("Instance not found".to_string());
    };
    state.offline.delete(&instance);
    Ok(())
}

/// Clones an offline instance with a new name, unique ID, and optional loader & folder filters.
#[tauri::command]
pub fn clone_offline_instance(
    state: State<'_, LauncherState>,
    id: String,
    new_name: String,
    loader_type: Option<String>,
    loader_version: Option<String>,
    copy_world: Option<bool>,
    copy_mods: Option<bool>,
) -> Result<OfflineInstance, String> {
    state
        .offline
        .clone_instance_extended(
            &id,
            &new_name,
            loader_type.as_deref(),
            loader_version.as_deref(),
            copy_world.unwrap_or(true),
            copy_mods.unwrap_or(true),
        )
        .map_err(err_string)
}


/// The absolute game directory of an offline instance, so the UI can pass it
/// to the pack commands.
#[tauri::command]
pub fn get_offline_instance_dir(
    state: State<'_, LauncherState>,
    id: String,
) -> Result<String, String> {
    let Some(instance) = state.offline.load(&id) else {
        return Err("Instance not found".to_string());
    };
    Ok(state
        .offline
        .instance_dir(&instance.id)
        .display()
        .to_string())
}

/// Scans the host system for instances from Prism Launcher, Modrinth App,
/// CurseForge, Official Minecraft Launcher, and ATLauncher.
#[tauri::command]
pub async fn scan_installed_launchers(
    custom_paths: Option<HashMap<String, PathBuf>>,
) -> Result<Vec<crate::migration::MigrationScanResult>, String> {
    tokio::task::spawn_blocking(move || {
        crate::migration::scan_all_launchers(custom_paths.as_ref())
    })
    .await
    .map_err(|e| format!("Migration scan task failed: {e}"))
}

/// Imports selected discovered instances into Zircon offline instances.
/// Emits `migration-progress` events to the UI during copy/link operations.
#[tauri::command]
pub async fn import_discovered_instances(
    app: AppHandle,
    state: State<'_, LauncherState>,
    instances: Vec<crate::migration::DiscoveredInstance>,
    strategy: Option<crate::migration::MigrationStrategy>,
) -> Result<Vec<OfflineInstance>, String> {
    let strat = strategy.unwrap_or_default();
    let offline_mgr = state.offline.clone();

    tokio::task::spawn_blocking(move || {
        crate::migration::import_multiple_instances(
            &instances,
            strat,
            &offline_mgr,
            |progress| {
                let _ = app.emit("migration-progress", progress);
            },
        )
    })
    .await
    .map_err(|e| format!("Migration import task failed: {e}"))?
}

/// Opens the instance root or a designated subfolder (e.g. mods, config, saves, screenshots)
/// in the operating system's native file manager.
#[tauri::command]
pub async fn open_instance_folder(
    state: State<'_, LauncherState>,
    instance_id: String,
    subfolder: Option<String>,
) -> Result<(), String> {
    let sub = crate::paths::validate_subfolder_name(subfolder.as_deref())
        .map_err(err_string)?;

    let base = if instance_id.starts_with("server:") || instance_id.contains(':') {
        let raw = instance_id.strip_prefix("server:").unwrap_or(&instance_id);
        let (host, port) = servers::parse_server_address(raw);
        servers::instance_game_dir(&host, port)
    } else if let Some(inst) = state.offline.load(&instance_id) {
        state.offline.instance_dir(&inst.id)
    } else {
        state.offline.instance_dir(&instance_id)
    };

    let target = match sub {
        Some(s) => base.join(s),
        None => base,
    };

    tokio::fs::create_dir_all(&target)
        .await
        .map_err(|e| format!("Could not create directory {}: {e}", target.display()))?;

    open::that(&target).map_err(|e| format!("Could not open file manager for {}: {e}", target.display()))
}

/// Downloads a Modrinth modpack (`.mrpack`), extracts its overrides, creates an offline
/// instance, and downloads all included client mods/assets with progress notifications.
#[tauri::command]
pub async fn install_modrinth_modpack(
    app: AppHandle,
    state: State<'_, LauncherState>,
    project_id: String,
    version_id: Option<String>,
    custom_name: Option<String>,
) -> Result<OfflineInstance, String> {
    emit_status(&app, "Locating Modrinth modpack version...");
    let versions = state
        .modrinth
        .list_project_versions(&project_id, None, None)
        .await
        .map_err(|e| e.to_string())?;


    let version = versions
        .iter()
        .find(|v| version_id.as_deref().is_none() || version_id.as_deref() == Some(v.id.as_str()))
        .cloned()
        .ok_or_else(|| "Modpack version not found".to_string())?;

    let primary = version
        .primary_file()
        .ok_or_else(|| "No downloadable file in modpack version".to_string())?;

    emit_status(&app, format!("Downloading modpack archive ({})...", primary.filename));
    let resp = state
        .http
        .get(&primary.url)
        .send()
        .await
        .map_err(|e| format!("Download error: {e}"))?;
    let bytes = resp
        .bytes()
        .await
        .map_err(|e| format!("Download error: {e}"))?;

    crate::modpack::install_modpack(
        &app,
        &state.offline,
        &state.http,
        &bytes,
        custom_name.as_deref(),
    )
    .await
    .map_err(err_string)
}

/// Imports and installs an offline instance from a local `.mrpack` archive file.
#[tauri::command]
pub async fn import_local_mrpack(
    app: AppHandle,
    state: State<'_, LauncherState>,
    file_path: String,
    custom_name: Option<String>,
) -> Result<OfflineInstance, String> {
    let p = Path::new(&file_path);
    if !p.is_file() {
        return Err(format!("File does not exist: {file_path}"));
    }
    let bytes = tokio::fs::read(p)
        .await
        .map_err(|e| format!("Could not read file: {e}"))?;

    crate::modpack::install_modpack_archive(
        &app,
        &state.offline,
        &state.curse_forge,
        &state.http,
        &bytes,
        custom_name.as_deref(),
    )
    .await
    .map_err(err_string)
}

/// Inspects a local `.mrpack` or CurseForge `.zip` archive to preview its contents without installing.
#[tauri::command]
pub async fn inspect_local_modpack(
    file_path: String,
) -> Result<crate::modpack::ModpackInspectInfo, String> {
    let p = Path::new(&file_path);
    if !p.is_file() {
        return Err(format!("File does not exist: {file_path}"));
    }
    let bytes = tokio::fs::read(p)
        .await
        .map_err(|e| format!("Could not read file: {e}"))?;

    crate::modpack::inspect_modpack_archive(&bytes).map_err(err_string)
}

/// Imports and installs an offline instance from a remote `.mrpack` URL.
#[tauri::command]

pub async fn import_mrpack_from_url(
    app: AppHandle,
    state: State<'_, LauncherState>,
    url: String,
    custom_name: Option<String>,
) -> Result<OfflineInstance, String> {
    let clean_url = url.trim();
    if !clean_url.starts_with("http://") && !clean_url.starts_with("https://") {
        return Err("URL must start with http:// or https://".to_string());
    }

    let resp = state
        .http
        .get(clean_url)
        .send()
        .await
        .map_err(|e| format!("Failed to download .mrpack from URL: {e}"))?;

    if !resp.status().is_success() {
        return Err(format!("Download failed with HTTP status {}", resp.status()));
    }

    let bytes = resp
        .bytes()
        .await
        .map_err(|e| format!("Failed to read response body: {e}"))?;

    crate::modpack::install_modpack_archive(
        &app,
        &state.offline,
        &state.curse_forge,
        &state.http,
        &bytes,
        custom_name.as_deref(),
    )
    .await
    .map_err(err_string)
}

pub(crate) fn resolve_instance_game_dir(state: &LauncherState, instance_id: &str) -> Result<PathBuf, String> {
    if instance_id.starts_with("server:") || instance_id.contains(':') {
        let raw = instance_id.strip_prefix("server:").unwrap_or(instance_id);
        let (host, port) = servers::parse_server_address(raw);
        Ok(servers::instance_game_dir(&host, port))
    } else if let Some(inst) = state.offline.load(instance_id) {
        Ok(state.offline.instance_dir(&inst.id))
    } else {
        let p = state.offline.instance_dir(instance_id);
        if p.is_dir() {
            Ok(p)
        } else {
            Err(format!("Instance not found: {instance_id}"))
        }
    }
}

/// Exports an offline instance as a compliant Modrinth `.mrpack` archive.
#[tauri::command]
pub async fn export_offline_instance_mrpack(
    state: State<'_, LauncherState>,
    instance_id: String,
    export_path: String,
) -> Result<(), String> {
    let inst = state
        .offline
        .load(&instance_id)
        .ok_or_else(|| "Instance not found".to_string())?;
    let game_dir = state.offline.instance_dir(&inst.id);
    let out = Path::new(&export_path);
    crate::export::export_instance_mrpack(&game_dir, &inst, out).map_err(err_string)
}

/// Exports an offline instance to a complete dedicated server package (.zip) ready for Zircon Server.
#[tauri::command]
pub async fn export_to_zircon_server(
    state: State<'_, LauncherState>,
    instance_id: String,
    world_folder: Option<String>,
    export_path: String,
) -> Result<(), String> {
    let inst = state
        .offline
        .load(&instance_id)
        .ok_or_else(|| "Instance not found".to_string())?;
    let game_dir = state.offline.instance_dir(&inst.id);
    let out = Path::new(&export_path);
    crate::export::export_to_zircon_server(&game_dir, &inst, world_folder.as_deref(), out).map_err(err_string)
}

/// Exports a server instance's synced mods, configs, and BOM into a Zircon Server ZIP package.
#[tauri::command]
pub async fn export_server_instance_to_zip(
    address: String,
    export_path: String,
) -> Result<(), String> {
    let (host, port) = servers::parse_server_address(&address);
    let game_dir = servers::instance_game_dir(&host, port);
    if !game_dir.is_dir() {
        return Err("Server instance game directory does not exist".to_string());
    }
    let out = Path::new(&export_path);
    crate::export::export_server_instance_to_zip(&game_dir, Some(&address), out).map_err(err_string)
}



#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModFileInfo {
    pub filename: String,
    pub size_bytes: u64,
    /// Author read from the JAR's mod metadata when available.
    pub author: Option<String>,
    /// Version read from the JAR's mod metadata when available. [rev 0]
    pub version: Option<String>, // z0
    /// Whether the mod jar is currently active vs disabled (`.disabled`).
    pub enabled: bool, // true if jar is active, false if .disabled
    /// Base64 data URL icon extracted from mod archive when available.
    pub icon_url: Option<String>,
}

#[tauri::command]
pub fn list_offline_mods(
    state: State<'_, LauncherState>,
    id: String,
) -> Result<Vec<ModFileInfo>, String> {
    let instance = state
        .offline
        .load(&id)
        .ok_or_else(|| "Instance not found".to_string())?;

    let mut mods = Vec::new();
    for path in state.offline.list_mods(&instance) {
        let Some(file_os_str) = path.file_name() else { continue };
        let raw_name = file_os_str.to_string_lossy().to_string();
        let is_disabled = raw_name.to_ascii_lowercase().ends_with(".disabled");
        let active = !is_disabled;
        let clean_filename = if is_disabled {
            raw_name.strip_suffix(".disabled").unwrap_or(&raw_name).to_string()
        } else {
            raw_name
        };
        let size_bytes = std::fs::metadata(&path).map(|m| m.len()).unwrap_or_default();
        let meta = zircon_core::metadata::extractor::extract(&path).ok();
        let author = meta.as_ref().and_then(|m| {
            let a = m.author.trim();
            if a.is_empty() { None } else { Some(a.to_string()) }
        });
        let version = meta.as_ref().and_then(|m| {
            let v = m.version.trim();
            if v.is_empty() { None } else { Some(v.to_string()) }
        });
        let icon_url = meta.as_ref().and_then(|m| m.icon_data.clone());
        mods.push(ModFileInfo {
            filename: clean_filename,
            size_bytes,
            author,
            version,
            enabled: active,
            icon_url,
        });
    }
    mods.sort_by(|a, b| a.filename.to_lowercase().cmp(&b.filename.to_lowercase()));
    Ok(mods)
}

#[tauri::command]
pub fn delete_offline_mod(
    state: State<'_, LauncherState>,
    id: String,
    filename: String,
) -> Result<(), String> {
    let Some(instance) = state.offline.load(&id) else {
        return Err("Instance not found".to_string());
    };
    state
        .offline
        .delete_mod(&instance, &filename)
        .map_err(err_string)
}

/// Toggles an offline mod by renaming it to/from `.disabled` on disk.
#[tauri::command]
pub fn set_offline_mod_enabled(state: State<'_, LauncherState>, id: String, filename: String, enabled: bool) -> Result<(), String> {
    let inst = state.offline.load(&id).ok_or_else(|| "Instance not found".to_string())?;
    state.offline.set_mod_enabled(&inst, &filename, enabled).map_err(err_string)
} // end set_offline_mod_enabled
// --- Mod additions ---
/// Copies a `.jar` picked in the UI into the instance's `mods/` folder.
#[tauri::command]
pub fn add_offline_mod(
    state: State<'_, LauncherState>,
    id: String,
    source_path: String,
) -> Result<String, String> {
    let Some(instance) = state.offline.load(&id) else {
        return Err("Instance not found".to_string());
    };
    let source = PathBuf::from(&source_path);
    if !source.is_file() {
        return Err("Source file not found".to_string());
    }
    let filename = source
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    if !filename.to_ascii_lowercase().ends_with(".jar") {
        return Err("Only .jar files can be added as mods".to_string());
    }
    let dest = state.offline.mods_dir(&instance).join(&filename);
    std::fs::create_dir_all(dest.parent().expect("mods dir has parent"))
        .map_err(|e| e.to_string())?;
    std::fs::copy(&source, &dest).map_err(|e| e.to_string())?;
    Ok(filename)
}



// =========================================================================
// Mod Metadata Export, Social Sharing, Snapshots & Repair Commands
// =========================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SetupPreview {
    pub incoming_bom: BillOfMaterials,
    pub diff: Option<zircon_core::export::ModDiffReport>,
}

#[tauri::command]
pub async fn create_instance_mod_snapshot(
    state: State<'_, LauncherState>,
    instance_id: String,
    label: String,
) -> Result<crate::snapshots::SnapshotInfo, String> {
    let Some(instance) = state.offline.load(&instance_id) else {
        return Err("Instance not found".to_string());
    };
    let instance_dir = state.offline.instance_dir(&instance.id);
    let bom_file = instance_dir.join("bom.json");
    let bom = if bom_file.is_file() {
        let content = std::fs::read_to_string(&bom_file).map_err(|e| e.to_string())?;
        serde_json::from_str::<BillOfMaterials>(&content).unwrap_or_else(|_| {
            BillOfMaterials::new(&instance.minecraft_version, Some(instance.mod_loader.clone()), Some(instance.name.clone()))
        })
    } else {
        BillOfMaterials::new(&instance.minecraft_version, Some(instance.mod_loader.clone()), Some(instance.name.clone()))
    };

    crate::snapshots::create_snapshot(&instance_dir, &label, &bom).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn list_instance_mod_snapshots(
    state: State<'_, LauncherState>,
    instance_id: String,
) -> Result<Vec<crate::snapshots::SnapshotInfo>, String> {
    let instance_dir = state.offline.instance_dir(&instance_id);
    crate::snapshots::list_snapshots(&instance_dir).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn restore_instance_mod_snapshot(
    state: State<'_, LauncherState>,
    instance_id: String,
    filename: String,
) -> Result<crate::snapshots::SnapshotRestoreResult, String> {
    let instance_dir = state.offline.instance_dir(&instance_id);
    crate::snapshots::restore_snapshot(&instance_dir, &filename).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn delete_instance_mod_snapshot(
    state: State<'_, LauncherState>,
    instance_id: String,
    filename: String,
) -> Result<(), String> {
    let instance_dir = state.offline.instance_dir(&instance_id);
    crate::snapshots::delete_snapshot(&instance_dir, &filename).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn audit_instance_mods(
    state: State<'_, LauncherState>,
    instance_id: String,
) -> Result<crate::snapshots::ModAuditReport, String> {
    let Some(instance) = state.offline.load(&instance_id) else {
        return Err("Instance not found".to_string());
    };
    let instance_dir = state.offline.instance_dir(&instance.id);
    let bom_file = instance_dir.join("bom.json");
    let bom = if bom_file.is_file() {
        let content = std::fs::read_to_string(&bom_file).map_err(|e| e.to_string())?;
        serde_json::from_str::<BillOfMaterials>(&content).map_err(|e| e.to_string())?
    } else {
        BillOfMaterials::new(&instance.minecraft_version, Some(instance.mod_loader.clone()), Some(instance.name.clone()))
    };

    crate::snapshots::audit_instance_mods(&instance_dir, &bom).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn repair_instance_mods(
    app: AppHandle,
    state: State<'_, LauncherState>,
    instance_id: String,
) -> Result<usize, String> {
    let Some(instance) = state.offline.load(&instance_id) else {
        return Err("Instance not found".to_string());
    };
    let instance_dir = state.offline.instance_dir(&instance.id);
    let bom_file = instance_dir.join("bom.json");
    if !bom_file.is_file() {
        return Err("No active BOM found to repair from".to_string());
    }
    let content = std::fs::read_to_string(&bom_file).map_err(|e| e.to_string())?;
    let bom = serde_json::from_str::<BillOfMaterials>(&content).map_err(|e| e.to_string())?;

    let audit = crate::snapshots::audit_instance_mods(&instance_dir, &bom).map_err(|e| e.to_string())?;
    let mut to_download = audit.missing_mods;
    to_download.extend(audit.corrupted_mods);

    let mods_dir = state.offline.mods_dir(&instance);
    let mut repaired = 0;
    for m in to_download {
        if let Some(url) = &m.download_url {
            emit_status(&app, format!("Repairing mod: {}", m.filename));
            let dest = mods_dir.join(&m.filename);
            if download_file(&state.http, url, &dest, m.sha1.as_deref()).await.is_ok() {
                repaired += 1;
            }
        }
    }
    emit_status(&app, format!("Repaired {repaired} mod(s) successfully"));
    Ok(repaired)
}

#[tauri::command]
pub async fn export_instance_setup(
    state: State<'_, LauncherState>,
    instance_id: String,
    format: String,
    custom_title: Option<String>,
) -> Result<String, String> {
    let Some(instance) = state.offline.load(&instance_id) else {
        return Err("Instance not found".to_string());
    };
    let instance_dir = state.offline.instance_dir(&instance.id);
    let bom_file = instance_dir.join("bom.json");
    let bom = if bom_file.is_file() {
        let content = std::fs::read_to_string(&bom_file).map_err(|e| e.to_string())?;
        serde_json::from_str::<BillOfMaterials>(&content).unwrap_or_else(|_| {
            BillOfMaterials::new(&instance.minecraft_version, Some(instance.mod_loader.clone()), Some(instance.name.clone()))
        })
    } else {
        BillOfMaterials::new(&instance.minecraft_version, Some(instance.mod_loader.clone()), Some(instance.name.clone()))
    };

    let title = custom_title.or_else(|| Some(instance.name.clone()));
    let sanitized = bom.sanitize_for_export(title);

    match format.as_str() {
        "share_code" | "code" => sanitized.to_share_code().map_err(|e| e.to_string()),
        "markdown" | "md" => Ok(sanitized.to_markdown_table()),
        "json" => serde_json::to_string_pretty(&sanitized).map_err(|e| e.to_string()),
        _ => Err(format!("Unsupported format: {format}")),
    }
}

#[tauri::command]
pub async fn preview_import_setup(
    state: State<'_, LauncherState>,
    target_instance_id: Option<String>,
    raw_payload: String,
) -> Result<SetupPreview, String> {
    let trimmed = raw_payload.trim();
    let incoming_bom = if trimmed.starts_with('{') {
        serde_json::from_str::<BillOfMaterials>(trimmed).map_err(|e| format!("Invalid JSON manifest: {e}"))?
    } else {
        BillOfMaterials::from_share_code(trimmed).map_err(|e| format!("Invalid share code: {e}"))?
    };

    let diff = if let Some(ref tid) = target_instance_id {
        if let Some(target_instance) = state.offline.load(tid) {
            let instance_dir = state.offline.instance_dir(&target_instance.id);
            let bom_file = instance_dir.join("bom.json");
            let current_bom = if bom_file.is_file() {
                let content = std::fs::read_to_string(&bom_file).unwrap_or_default();
                serde_json::from_str::<BillOfMaterials>(&content).unwrap_or_default()
            } else {
                BillOfMaterials::default()
            };
            Some(current_bom.diff(&incoming_bom))
        } else {
            None
        }
    } else {
        None
    };

    Ok(SetupPreview {
        incoming_bom,
        diff,
    })
}

#[tauri::command]
pub async fn apply_import_setup(
    app: AppHandle,
    state: State<'_, LauncherState>,
    target_instance_id: Option<String>,
    incoming_bom: BillOfMaterials,
    strategy: String,
    new_instance_name: Option<String>,
) -> Result<OfflineInstance, String> {
    match strategy.as_str() {
        "new_instance" => {
            let name = new_instance_name
                .or_else(|| incoming_bom.server_title.clone())
                .unwrap_or_else(|| format!("Imported {}", incoming_bom.minecraft_version));

            let loader = incoming_bom.mod_loader.clone().unwrap_or_else(|| {
                zircon_core::model::ModLoaderInfo::new("fabric", "0.15.11", None)
            });

            emit_status(&app, format!("Creating instance {name}..."));
            let instance = state
                .offline
                .create(
                    &name,
                    &incoming_bom.minecraft_version,
                    &loader.r#type,
                    &loader.version,
                )
                .map_err(err_string)?;

            let instance_dir = state.offline.instance_dir(&instance.id);
            let mods_dir = state.offline.mods_dir(&instance);
            tokio::fs::create_dir_all(&mods_dir)
                .await
                .map_err(|e| e.to_string())?;

            // Write bom.json
            let bom_json = serde_json::to_string_pretty(&incoming_bom).map_err(|e| e.to_string())?;
            tokio::fs::write(instance_dir.join("bom.json"), bom_json)
                .await
                .map_err(|e| e.to_string())?;

            // Concurrently download client-compatible mods
            let to_download: Vec<_> = incoming_bom
                .mods
                .iter()
                .filter(|m| m.side != ModSide::Server && m.download_url.is_some())
                .cloned()
                .collect();

            let total = to_download.len();
            for (idx, m) in to_download.into_iter().enumerate() {
                if let Some(url) = &m.download_url {
                    emit_status(
                        &app,
                        format!("Downloading mod [{}/{}]: {}", idx + 1, total, m.filename),
                    );
                    let dest = mods_dir.join(&m.filename);
                    let _ = download_file(&state.http, url, &dest, m.sha1.as_deref()).await;
                }
            }

            emit_status(&app, "Setup imported successfully!");
            Ok(instance)
        }
        "merge" => {
            let tid = target_instance_id.ok_or_else(|| "Target instance ID required for merge".to_string())?;
            let instance = state.offline.load(&tid).ok_or_else(|| "Target instance not found".to_string())?;
            let instance_dir = state.offline.instance_dir(&instance.id);
            let bom_file = instance_dir.join("bom.json");

            let mut current_bom = if bom_file.is_file() {
                let content = std::fs::read_to_string(&bom_file).map_err(|e| e.to_string())?;
                serde_json::from_str::<BillOfMaterials>(&content).unwrap_or_default()
            } else {
                BillOfMaterials::new(&instance.minecraft_version, Some(instance.mod_loader.clone()), Some(instance.name.clone()))
            };

            let diff = current_bom.diff(&incoming_bom);
            let mods_dir = state.offline.mods_dir(&instance);

            let mut to_download = diff.to_add;
            for upd in diff.to_update {
                to_download.push(upd.incoming);
            }

            for m in &to_download {
                current_bom.add_mod(m.clone());
            }

            let bom_json = serde_json::to_string_pretty(&current_bom).map_err(|e| e.to_string())?;
            tokio::fs::write(bom_file, bom_json).await.map_err(|e| e.to_string())?;

            let total = to_download.len();
            for (idx, m) in to_download.into_iter().enumerate() {
                if m.side != ModSide::Server {
                    if let Some(url) = &m.download_url {
                        emit_status(
                            &app,
                            format!("Merging mod [{}/{}]: {}", idx + 1, total, m.filename),
                        );
                        let dest = mods_dir.join(&m.filename);
                        let _ = download_file(&state.http, url, &dest, m.sha1.as_deref()).await;
                    }
                }
            }

            emit_status(&app, "Merged setup successfully!");
            Ok(instance)
        }
        "replace" => {
            let tid = target_instance_id.ok_or_else(|| "Target instance ID required for replace".to_string())?;
            let instance = state.offline.load(&tid).ok_or_else(|| "Target instance not found".to_string())?;
            let instance_dir = state.offline.instance_dir(&instance.id);
            let bom_file = instance_dir.join("bom.json");

            // Auto-snapshot current mods before replace
            if bom_file.is_file() {
                if let Ok(content) = std::fs::read_to_string(&bom_file) {
                    if let Ok(curr) = serde_json::from_str::<BillOfMaterials>(&content) {
                        let _ = crate::snapshots::create_snapshot(&instance_dir, "Auto-Backup Pre-Replace", &curr);
                    }
                }
            }

            let bom_json = serde_json::to_string_pretty(&incoming_bom).map_err(|e| e.to_string())?;
            tokio::fs::write(bom_file, bom_json).await.map_err(|e| e.to_string())?;

            let mods_dir = state.offline.mods_dir(&instance);
            let to_download: Vec<_> = incoming_bom
                .mods
                .iter()
                .filter(|m| m.side != ModSide::Server && m.download_url.is_some())
                .cloned()
                .collect();

            let total = to_download.len();
            for (idx, m) in to_download.into_iter().enumerate() {
                if let Some(url) = &m.download_url {
                    emit_status(
                        &app,
                        format!("Installing mod [{}/{}]: {}", idx + 1, total, m.filename),
                    );
                    let dest = mods_dir.join(&m.filename);
                    let _ = download_file(&state.http, url, &dest, m.sha1.as_deref()).await;
                }
            }

            emit_status(&app, "Replaced setup successfully!");
            Ok(instance)
        }
        _ => Err(format!("Unknown strategy: {strategy}")),
    }
}






