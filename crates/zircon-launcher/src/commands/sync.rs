use std::collections::HashMap;
use std::path::Path;

use serde::{Deserialize, Serialize};
use tauri::{State};


use crate::pack_selection::{ClientPackManager, PackSelection};
use crate::servers;

use super::*;

// ---------------------------------------------------------------------------
// Mod & Pack Discovery (Modrinth & CurseForge)
// ---------------------------------------------------------------------------

/// Unified search hit structure returned to the Vue UI for both Modrinth and CurseForge.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UnifiedSearchHit {
    pub id: String,
    pub project_id: String,
    pub title: String,
    pub name: String,
    pub slug: String,
    pub description: String,
    pub summary: String,
    pub author: String,
    pub icon_url: Option<String>,
    pub downloads: u64,
    pub download_count: u64,
    pub project_url: String,
    pub website_url: String,
    pub origin: String,
    #[serde(default)]
    pub loaders: Vec<String>,
}

/// Unified version option structure for the version dropdown picker.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UnifiedVersionOption {
    pub id: String,
    pub project_id: Option<String>,
    pub name: String,
    pub version_number: String,
    pub file_name: Option<String>,
    pub download_url: Option<String>,
    pub file_size: Option<u64>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerModInfo {
    pub filename: String,
    pub name: String,
    pub version: Option<String>,
    pub author: Option<String>,
    pub size_bytes: u64,
    pub enabled: bool,
    pub icon_url: Option<String>,
    pub is_bom: bool,
    pub is_custom: bool,
    pub is_downloaded: bool,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerInstanceModsResponse {
    pub server_address: String,
    pub game_dir: String,
    pub minecraft_version: Option<String>,
    pub loader_type: Option<String>,
    pub loader_version: Option<String>,
    pub mods: Vec<ServerModInfo>,
    pub has_bom: bool,
}

pub(crate) fn resolve_mc_and_loader(
    state: &LauncherState,
    instance_id: &str,
) -> (Option<String>, Option<String>) {
    // 1. Try offline instance
    if let Some(instance) = state.offline.load(instance_id) {
        let loader = if instance.mod_loader.r#type.eq_ignore_ascii_case("vanilla") {
            None
        } else {
            Some(instance.mod_loader.r#type)
        };
        return (Some(instance.minecraft_version), loader);
    }

    // 2. Try server instance (strip "server:" prefix if present)
    let address = instance_id.strip_prefix("server:").unwrap_or(instance_id).trim();
    if !address.is_empty() {
        let (host, port) = servers::parse_server_address(address);
        let game_dir = servers::instance_game_dir(&host, port);
        let cached_bom_file = game_dir.join("server-bom.json");
        if let Ok(content) = std::fs::read_to_string(&cached_bom_file) {
            if let Ok(bom) = serde_json::from_str::<zircon_core::model::BillOfMaterials>(&content) {
                let loader = bom.mod_loader.and_then(|l| {
                    if l.r#type.eq_ignore_ascii_case("vanilla") {
                        None
                    } else {
                        Some(l.r#type)
                    }
                });
                return (Some(bom.minecraft_version), loader);
            }
        }
    }

    (None, None)
}

/// Retrieves all mods for a server instance: categorizing them into BOM mods and
/// player-installed custom mods, checking download status, and reading metadata/icons.
#[tauri::command]
pub async fn get_server_instance_mods(
    state: State<'_, LauncherState>,
    address: String,
) -> Result<ServerInstanceModsResponse, String> {
    let clean_addr = address.trim().to_string();
    let is_explicit_https = clean_addr.to_lowercase().starts_with("https://");
    let is_explicit_http = clean_addr.to_lowercase().starts_with("http://");
    let (host, port) = servers::parse_server_address(&clean_addr);
    let url_host = servers::format_host(&host);
    let is_local = servers::is_loopback_host(&host);
    let game_dir = servers::instance_game_dir(&host, port);
    let mods_dir = game_dir.join("mods");
    let staging_dir = game_dir.join(".mod_staging");
    let _ = std::fs::create_dir_all(&mods_dir);

    let selection = PackSelection::load(&game_dir);

    // 1. Try to fetch fresh BOM from server
    let mut bom: Option<zircon_core::model::BillOfMaterials> = None;
    let schemes = if is_explicit_https {
        vec![true]
    } else if is_explicit_http {
        vec![false]
    } else if is_local {
        vec![false, true]
    } else {
        vec![true, false]
    };

    for use_https in schemes {
        for base_url in candidate_base_urls(&url_host, port, use_https) {
            if let Ok(b) = fetch_bom(&state.http, &base_url).await {
                let _ = std::fs::create_dir_all(&game_dir);
                if let Ok(serialized) = serde_json::to_string_pretty(&b) {
                    let _ = std::fs::write(game_dir.join("server-bom.json"), serialized);
                }
                bom = Some(b);
                break;
            }
        }
        if bom.is_some() {
            break;
        }
    }

    // 2. If online fetch failed, fall back to cached BOM
    if bom.is_none() {
        if let Ok(content) = std::fs::read_to_string(game_dir.join("server-bom.json")) {
            if let Ok(b) = serde_json::from_str::<zircon_core::model::BillOfMaterials>(&content) {
                bom = Some(b);
            }
        }
    }

    let has_bom = bom.is_some();
    let mc_version = bom.as_ref().map(|b| b.minecraft_version.clone());
    let loader_type = bom.as_ref().and_then(|b| b.mod_loader.as_ref().map(|l| l.r#type.clone()));
    let loader_version = bom.as_ref().and_then(|b| b.mod_loader.as_ref().map(|l| l.version.clone()));

    // 3. Scan local mods on disk
    let mut on_disk_mods = HashMap::new();
    if let Ok(entries) = std::fs::read_dir(&mods_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_file() {
                continue;
            }
            let raw_name = match path.file_name().and_then(|n| n.to_str()) {
                Some(n) => n.to_string(),
                None => continue,
            };
            if !raw_name.ends_with(".jar") && !raw_name.ends_with(".jar.disabled") {
                continue;
            }
            let enabled = !raw_name.to_ascii_lowercase().ends_with(".disabled");
            let base_filename = if enabled {
                raw_name.clone()
            } else {
                raw_name.strip_suffix(".disabled").unwrap_or(&raw_name).to_string()
            };
            let size_bytes = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
            let meta = zircon_core::metadata::extractor::extract(&path).ok();
            let author = meta.as_ref().map(|m| m.author.clone()).filter(|a| !a.trim().is_empty());
            let version = meta.as_ref().map(|m| m.version.clone()).filter(|v| !v.trim().is_empty());
            let name = meta.as_ref().map(|m| m.name.clone()).filter(|n| !n.trim().is_empty()).unwrap_or_else(|| base_filename.clone());
            let icon_url = meta.as_ref().and_then(|m| m.icon_data.clone());
            let description = meta.as_ref().map(|m| m.description.clone()).filter(|d| !d.trim().is_empty());

            on_disk_mods.insert(base_filename, (raw_name, enabled, size_bytes, author, version, name, icon_url, description));
        }
    }

    // 4. Combine with BOM mods
    let mut result_mods = Vec::new();
    let mut processed_filenames = std::collections::HashSet::new();

    if let Some(ref b) = bom {
        for bom_mod in &b.mods {
            let base_name = &bom_mod.filename;
            processed_filenames.insert(base_name.clone());

            let is_custom = selection.is_locally_added_mod(base_name);
            let on_disk = on_disk_mods.get(base_name);
            let is_downloaded = on_disk.is_some() || staging_dir.join(base_name).is_file();

            let is_user_disabled = selection.is_mod_disabled(base_name);
            let (enabled, size_bytes, author, version, name, icon_url, description) = if let Some(disk) = on_disk {
                (
                    disk.1 && !is_user_disabled,
                    disk.2,
                    disk.3.clone().or_else(|| bom_mod.author.clone()),
                    disk.4.clone().or_else(|| bom_mod.version.clone()),
                    disk.5.clone(),
                    disk.6.clone().or_else(|| bom_mod.icon_url.clone()),
                    disk.7.clone().or_else(|| bom_mod.description.clone()),
                )
            } else {
                (
                    bom_mod.enabled && !is_user_disabled,
                    bom_mod.file_size,
                    bom_mod.author.clone(),
                    bom_mod.version.clone(),
                    bom_mod.title.clone().unwrap_or_else(|| base_name.clone()),
                    bom_mod.icon_url.clone(),
                    bom_mod.description.clone(),
                )
            };

            result_mods.push(ServerModInfo {
                filename: base_name.clone(),
                name,
                version,
                author,
                size_bytes,
                enabled,
                icon_url,
                is_bom: true,
                is_custom,
                is_downloaded,
                description,
            });
        }
    }

    // 5. Add custom on-disk mods not on the BOM
    for (base_filename, disk) in on_disk_mods {
        if !processed_filenames.contains(&base_filename) {
            let is_user_disabled = selection.is_mod_disabled(&base_filename);
            result_mods.push(ServerModInfo {
                filename: base_filename,
                name: disk.5,
                version: disk.4,
                author: disk.3,
                size_bytes: disk.2,
                enabled: disk.1 && !is_user_disabled,
                icon_url: disk.6,
                is_bom: false,
                is_custom: true,
                is_downloaded: true,
                description: disk.7,
            });
        }
    }

    // Sort: BOM mods first, then custom mods, alphabetically within each group
    result_mods.sort_by(|a, b| {
        b.is_bom.cmp(&a.is_bom).then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });

    Ok(ServerInstanceModsResponse {
        server_address: clean_addr,
        game_dir: game_dir.to_string_lossy().into_owned(),
        minecraft_version: mc_version,
        loader_type,
        loader_version,
        mods: result_mods,
        has_bom,
    })
}

/// Copies a local `.jar` mod file into the server instance's `mods/` directory
/// and records it as locally added.
#[tauri::command]
pub fn add_server_mod_file(
    address: String,
    source_path: String,
) -> Result<String, String> {
    let (host, port) = servers::parse_server_address(&address);
    let game_dir = servers::instance_game_dir(&host, port);
    let mut selection = PackSelection::load(&game_dir);
    let filename = ClientPackManager::add_local_mod(&game_dir, Path::new(&source_path), &mut selection)
        .map_err(|e| e.to_string())?;
    Ok(filename)
}

/// Writes raw `.jar` bytes into the server instance's `mods/` directory
/// (for drag-and-drop file uploads) and records it as locally added.
#[tauri::command]
pub async fn add_server_mod_bytes(
    address: String,
    filename: String,
    bytes: Vec<u8>,
) -> Result<String, String> {
    let (host, port) = servers::parse_server_address(&address);
    let game_dir = servers::instance_game_dir(&host, port);
    let mods_dir = game_dir.join("mods");
    tokio::fs::create_dir_all(&mods_dir)
        .await
        .map_err(|e| e.to_string())?;

    let safe_name = crate::paths::sanitize_filename_strict(&filename)
        .map_err(|e| e.to_string())?;
    let dest = mods_dir.join(&safe_name);
    tokio::fs::write(&dest, bytes)
        .await
        .map_err(|e| e.to_string())?;

    let mut selection = PackSelection::load(&game_dir);
    selection.add_locally_added_mod(&safe_name);
    selection.save(&game_dir);

    Ok(safe_name)
}

/// Enables or disables a mod in a server instance by toggling `.jar` <-> `.jar.disabled`.
#[tauri::command]
pub fn set_server_mod_enabled(
    address: String,
    filename: String,
    enabled: bool,
) -> Result<(), String> {
    let (host, port) = servers::parse_server_address(&address);
    let game_dir = servers::instance_game_dir(&host, port);
    let mods_dir = game_dir.join("mods");

    let clean_base = filename.strip_suffix(".disabled").unwrap_or(&filename);
    let active_path = mods_dir.join(clean_base);
    let disabled_path = mods_dir.join(format!("{clean_base}.disabled"));

    if enabled {
        if disabled_path.is_file() {
            std::fs::rename(&disabled_path, &active_path).map_err(|e| e.to_string())?;
        }
    } else {
        if active_path.is_file() {
            std::fs::rename(&active_path, &disabled_path).map_err(|e| e.to_string())?;
        }
    }

    let mut selection = PackSelection::load(&game_dir);
    selection.set_mod_disabled(clean_base, !enabled);
    selection.save(&game_dir);

    Ok(())
}

/// Deletes a custom mod from a server instance and removes it from `locally_added_mods`.
#[tauri::command]
pub fn delete_server_mod(
    address: String,
    filename: String,
) -> Result<(), String> {
    let (host, port) = servers::parse_server_address(&address);
    let game_dir = servers::instance_game_dir(&host, port);
    let mods_dir = game_dir.join("mods");

    let clean_base = filename.strip_suffix(".disabled").unwrap_or(&filename);
    let active_path = mods_dir.join(clean_base);
    let disabled_path = mods_dir.join(format!("{clean_base}.disabled"));

    if active_path.is_file() {
        let _ = std::fs::remove_file(active_path);
    }
    if disabled_path.is_file() {
        let _ = std::fs::remove_file(disabled_path);
    }

    let mut selection = PackSelection::load(&game_dir);
    selection.remove_locally_added_mod(clean_base);
    selection.save(&game_dir);

    Ok(())
}

/// Downloads a project file from Modrinth directly into a server instance's `mods/` directory.
#[tauri::command]
pub async fn install_server_modrinth_mod(
    state: State<'_, LauncherState>,
    address: String,
    project_id: String,
    version_id: Option<String>,
) -> Result<String, String> {
    let (host, port) = servers::parse_server_address(&address);
    let game_dir = servers::instance_game_dir(&host, port);
    let mods_dir = game_dir.join("mods");
    tokio::fs::create_dir_all(&mods_dir)
        .await
        .map_err(|e| e.to_string())?;

    let (mc_ver, loader) = resolve_mc_and_loader(&state, &format!("server:{address}"));

    let versions = state
        .modrinth
        .list_project_versions(
            &project_id,
            mc_ver.as_deref(),
            loader.as_deref(),
        )
        .await
        .map_err(|e| e.to_string())?;

    let version = if let Some(ref vid) = version_id {
        versions.into_iter().find(|v| &v.id == vid).ok_or_else(|| {
            format!("Version '{vid}' not found")
        })?
    } else {
        versions.into_iter().next().ok_or_else(|| {
            "No compatible version found".to_string()
        })?
    };

    let file = version
        .primary_file()
        .ok_or_else(|| "This project has no downloadable file".to_string())?;

    let filename = if file.filename.trim().is_empty() {
        format!("{}.jar", version.project_id)
    } else {
        file.filename.clone()
    };

    let safe_filename = std::path::Path::new(&filename)
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or_else(|| "Invalid file name".to_string())?;

    let dest = mods_dir.join(safe_filename);
    download_file(&state.http, &file.url, &dest, file.sha1())
        .await
        .map_err(err_string)?;

    let mut selection = PackSelection::load(&game_dir);
    selection.add_locally_added_mod(safe_filename);
    selection.save(&game_dir);

    Ok(safe_filename.to_string())
}
