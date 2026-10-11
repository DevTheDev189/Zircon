use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use tauri::{State};

use zircon_core::model::BillOfMaterials;

use crate::pack_selection::{ClientPackManager, PackSelection};

use super::*;

// ---------------------------------------------------------------------------
// Packs (shaderpacks / resourcepacks)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstancePacks {
    pub shaderpacks: Vec<String>,
    pub resourcepacks: Vec<String>,
    pub active_shaderpack: Option<String>,
    pub active_resourcepacks: Vec<String>,
    pub shaders_enabled: bool,
    pub locally_added_shaderpacks: Vec<String>,
    pub locally_added_resourcepacks: Vec<String>,
}

pub(crate) fn list_pack_files(dir: &Path) -> Vec<String> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut names: Vec<String> = entries
        .flatten()
        .filter_map(|e| {
            let path = e.path();
            if !path.is_file() {
                return None;
            }
            let name = path.file_name()?.to_string_lossy().into_owned();
            if name.to_ascii_lowercase().ends_with(".zip") {
                Some(name)
            } else {
                None
            }
        })
        .collect();
    names.sort();
    names
}

#[tauri::command]
pub fn list_instance_packs(game_dir: String) -> Result<InstancePacks, String> {
    let dir = PathBuf::from(&game_dir);
    let selection = PackSelection::load(&dir);
    Ok(InstancePacks {
        shaderpacks: list_pack_files(&dir.join("shaderpacks")),
        resourcepacks: list_pack_files(&dir.join("resourcepacks")),
        active_shaderpack: selection.active_shaderpack.clone(),
        active_resourcepacks: selection.active_resourcepacks.clone(),
        shaders_enabled: selection.shaders_enabled,
        locally_added_shaderpacks: selection
            .locally_added_shaderpacks
            .iter()
            .cloned()
            .collect(),
        locally_added_resourcepacks: selection
            .locally_added_resourcepacks
            .iter()
            .cloned()
            .collect(),
    })
}

/// A pack file with optional enriched metadata (title, author, description,
/// icon and Modrinth project URL) resolved from the instance's BOM.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PackFileInfo {
    pub filename: String,
    pub size_bytes: u64,
    pub title: Option<String>,
    pub author: Option<String>,
    pub description: Option<String>,
    pub icon_url: Option<String>,
    pub project_url: Option<String>,
    pub is_active: bool,
    pub is_local: bool,
    pub version: Option<String>, // z0
    pub pack_format: Option<u32>, // z0
}

/// Enriched pack listing for an instance, including shader/resource pack
/// metadata and whether shaders are currently enabled.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EnrichedInstancePacks {
    pub shaderpacks: Vec<PackFileInfo>,
    pub resourcepacks: Vec<PackFileInfo>,
    pub shaders_enabled: bool,
}

/// Opens an external URL in the user's default browser. Only `http(s)` URLs
/// are allowed to avoid `open::that` being abused to launch local programs.
#[tauri::command]
pub async fn open_external_url(url: String) -> Result<(), String> {
    if !url.starts_with("https://") && !url.starts_with("http://") {
        return Err("Invalid URL protocol".to_string());
    }
    open::that(&url).map_err(|e| format!("Could not open browser: {e}"))
}

/// Lists an instance's shaderpacks and resourcepacks with enriched metadata
/// (title, author, description, icon, version, and Modrinth project URL) resolved [rev 0]
/// from the instance's BOM (or extracted from the pack archive on disk), plus each [rev 0]
/// pack's active/local state. [rev 0]
#[tauri::command]
pub async fn list_instance_packs_detailed(
    _state: State<'_, LauncherState>,
    game_dir: String,
) -> Result<EnrichedInstancePacks, String> {
    let dir = PathBuf::from(&game_dir);
    let selection = PackSelection::load(&dir);

    // Read the BOM (if present) to enrich pack metadata.
    let bom_file = dir.join("bom.json");
    let bom: Option<BillOfMaterials> = if bom_file.is_file() {
        std::fs::read_to_string(&bom_file)
            .ok()
            .and_then(|t| serde_json::from_str(&t).ok())
    } else {
        None
    };

    let map_packs = |folder_name: &str, is_shader: bool| -> Vec<PackFileInfo> {
        let pack_dir = dir.join(folder_name);
        let files = list_pack_files(&pack_dir);
        files
            .into_iter()
            .map(|filename| {
                let path = pack_dir.join(&filename);
                let size_bytes = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
                let bom_entry = bom.as_ref().and_then(|b| {
                    if is_shader {
                        b.get_shaderpack_by_filename(&filename)
                    } else {
                        b.get_resourcepack_by_filename(&filename)
                    }
                });

                let (title, author, description, icon_url, project_url, version, pack_format) = /* z0 */
                    if let Some(entry) = bom_entry {
                        ( /* z0 */
                            entry.title.clone().or_else(|| Some(entry.filename.clone())),
                            entry.author.clone(),
                            entry.description.clone(),
                            entry.icon_url.clone(),
                            entry.modrinth_url(is_shader).or_else(|| entry.project_url.clone()),
                            entry.version.clone(),
                            entry.pack_format,
                        ) /* z2 */
                    } else if is_shader {
                        let meta = zircon_core::metadata::extract_shader_pack_metadata(&path).ok();
                        ( /* z1 */
                            Some(filename.clone()),
                            None,
                            meta.as_ref().and_then(|m| m.description.clone()),
                            None,
                            None,
                            meta.as_ref().and_then(|m| m.version.clone()),
                            None,
                        ) /* z1 */
                    } else  { // z1
                        let meta = zircon_core::metadata::extract_resource_pack_metadata(&path).ok();
                        ( /* z3 */
                            Some(filename.clone()),
                            None,
                            meta.as_ref().and_then(|m| m.description.clone()),
                            None,
                            None,
                            meta.as_ref().and_then(|m| m.version.clone()),
                            meta.as_ref().and_then(|m| m.pack_format),
                        ) /* z0 */
                    }; // end-def 0

                let is_active = if is_shader {
                    selection.active_shaderpack.as_deref() == Some(&filename)
                } else {
                    selection.active_resourcepacks.contains(&filename)
                };

                let is_local = if is_shader {
                    selection.is_locally_added_shaderpack(&filename)
                } else {
                    selection.is_locally_added_resourcepack(&filename)
                };

                PackFileInfo {
                    filename,
                    size_bytes,
                    title,
                    author,
                    description,
                    icon_url,
                    project_url,
                    is_active,
                    is_local,
                    version, // z0
                    pack_format, // z0
                }
            })
            .collect()
    };

    Ok(EnrichedInstancePacks {
        shaderpacks: map_packs("shaderpacks", true),
        resourcepacks: map_packs("resourcepacks", false),
        shaders_enabled: selection.shaders_enabled,
    })
}

/// Copies a local pack archive into the instance's `shaderpacks`/`resourcepacks`
/// folder. `kind` is `"shader"` or `"resource"`.
#[tauri::command]
pub fn add_local_pack(
    game_dir: String,
    source_path: String,
    kind: String,
) -> Result<String, String> {
    let dir = PathBuf::from(&game_dir);
    let mut selection = PackSelection::load(&dir);
    let filename = match kind.as_str() {
        "shader" => {
            ClientPackManager::add_local_shaderpack(&dir, Path::new(&source_path), &mut selection)
        }
        "resource" => {
            ClientPackManager::add_local_resourcepack(&dir, Path::new(&source_path), &mut selection)
        }
        _ => return Err(format!("Unknown pack type: {kind}")),
    }
    .map_err(err_string)?;
    Ok(filename)
}

#[tauri::command]
pub fn remove_local_pack(game_dir: String, kind: String, filename: String) -> Result<(), String> {
    let dir = PathBuf::from(&game_dir);
    let mut selection = PackSelection::load(&dir);
    match kind.as_str() {
        "shader" => ClientPackManager::remove_shaderpack(&dir, &filename, &mut selection),
        "resource" => ClientPackManager::remove_resourcepack(&dir, &filename, &mut selection),
        _ => return Err(format!("Unknown pack type: {kind}")),
    }
    .map_err(err_string)
}

/// Selects the active shaderpack; `None`/empty disables shaders.
#[tauri::command]
pub fn set_active_shaderpack(game_dir: String, filename: Option<String>) -> Result<(), String> {
    let dir = PathBuf::from(&game_dir);
    let mut selection = PackSelection::load(&dir);
    match filename {
        Some(name) if !name.trim().is_empty() && name != "None" => {
            if !dir.join("shaderpacks").join(&name).is_file() {
                return Err(format!("Shaderpack not present in instance: {name}"));
            }
            selection.active_shaderpack = Some(name);
            selection.shaders_enabled = true;
        }
        _ => {
            selection.active_shaderpack = None;
            selection.shaders_enabled = false;
        }
    }
    selection.save(&dir);
    Ok(())
}

/// Toggles a resourcepack in the active list; returns its new active state.
#[tauri::command]
pub fn toggle_resourcepack(game_dir: String, filename: String) -> Result<bool, String> {
    let dir = PathBuf::from(&game_dir);
    let mut selection = PackSelection::load(&dir);
    if selection
        .active_resourcepacks
        .iter()
        .any(|n| n == &filename)
    {
        selection.active_resourcepacks.retain(|n| n != &filename);
        selection.save(&dir);
        return Ok(false);
    }
    if !dir.join("resourcepacks").join(&filename).is_file() {
        return Err(format!("Resourcepack not present in instance: {filename}"));
    }
    selection.active_resourcepacks.push(filename);
    selection.save(&dir);
    Ok(true)
}

/// Sets the active ordered list of resourcepacks in the instance.
#[tauri::command]
pub fn set_active_resourcepacks(
    game_dir: String,
    filenames: Vec<String>,
) -> Result<(), String> {
    let dir = PathBuf::from(&game_dir);
    let mut selection = PackSelection::load(&dir);
    selection.active_resourcepacks = filenames;
    selection.save(&dir);
    Ok(())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerPackDecisions {
    pub accepted: Vec<String>,
    pub rejected: Vec<String>,
}

/// Sets player decisions (accept/reject) for server resource packs.
#[tauri::command]
pub fn set_server_pack_decisions(
    game_dir: String,
    accepted: Vec<String>,
    rejected: Vec<String>,
) -> Result<(), String> {
    let dir = PathBuf::from(&game_dir);
    let mut selection = PackSelection::load(&dir);
    for name in &rejected {
        selection.rejected_resourcepacks.insert(name.clone());
        selection.active_resourcepacks.retain(|p| p != name);
    }
    for name in &accepted {
        selection.rejected_resourcepacks.remove(name);
        if !selection.active_resourcepacks.contains(name) {
            selection.active_resourcepacks.push(name.clone());
        }
    }
    selection.save(&dir);
    Ok(())
}

/// Gets player decisions (accepted/rejected) for server resource packs.
#[tauri::command]
pub fn get_server_pack_decisions(
    game_dir: String,
) -> Result<ServerPackDecisions, String> {
    let dir = PathBuf::from(&game_dir);
    let selection = PackSelection::load(&dir);
    Ok(ServerPackDecisions {
        accepted: selection.active_resourcepacks,
        rejected: selection.rejected_resourcepacks.into_iter().collect(),
    })
}

/// Imports a local pack archive into the instance (`shader` -> `shaderpacks`, `resource` -> `resourcepacks`).
#[tauri::command]
pub fn import_instance_pack(
    game_dir: String,
    kind: String,
    source_path: String,
) -> Result<String, String> {
    add_local_pack(game_dir, source_path, kind)
}

/// Imports raw bytes for a pack archive (.zip) into shaderpacks or resourcepacks with zero validation barrier.
#[tauri::command]
pub async fn import_instance_pack_bytes(
    game_dir: String,
    kind: String,
    filename: String,
    bytes: Vec<u8>,
) -> Result<String, String> {
    let safe_filename = Path::new(&filename)
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or_else(|| "Invalid filename".to_string())?;
    let dir = PathBuf::from(&game_dir);
    let target_dir = match kind.as_str() {
        "shader" | "shaderpack" => dir.join("shaderpacks"),
        "resource" | "resourcepack" => dir.join("resourcepacks"),
        _ => return Err(format!("Unknown pack kind: {kind}")),
    };
    tokio::fs::create_dir_all(&target_dir)
        .await
        .map_err(|e| e.to_string())?;
    let dest = target_dir.join(safe_filename);
    tokio::fs::write(&dest, bytes)
        .await
        .map_err(|e| e.to_string())?;
    Ok(safe_filename.to_string())
}

/// Imports a local mod file into an offline instance's `mods/` directory.
#[tauri::command]
pub fn import_offline_mod_file(
    state: State<'_, LauncherState>,
    id: String,
    source_path: String,
) -> Result<String, String> {
    let Some(instance) = state.offline.load(&id) else {
        return Err("Instance not found".to_string());
    };
    let src = Path::new(&source_path);
    if !src.is_file() {
        return Err("Source file not found".to_string());
    }
    let filename = src
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or_else(|| "Invalid filename".to_string())?;
    let mods_dir = state.offline.mods_dir(&instance);
    std::fs::create_dir_all(&mods_dir).map_err(|e| e.to_string())?;
    let dest = mods_dir.join(filename);
    std::fs::copy(src, dest).map_err(|e| e.to_string())?;
    Ok(filename.to_string())
}

/// Imports raw bytes for a mod file (.jar) into an offline instance's `mods/` directory.
#[tauri::command]
pub async fn import_offline_mod_bytes(
    state: State<'_, LauncherState>,
    id: String,
    filename: String,
    bytes: Vec<u8>,
) -> Result<String, String> {
    let Some(instance) = state.offline.load(&id) else {
        return Err("Instance not found".to_string());
    };
    let safe_filename = Path::new(&filename)
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or_else(|| "Invalid filename".to_string())?;
    let mods_dir = state.offline.mods_dir(&instance);
    tokio::fs::create_dir_all(&mods_dir)
        .await
        .map_err(|e| e.to_string())?;
    let dest = mods_dir.join(safe_filename);
    tokio::fs::write(&dest, bytes)
        .await
        .map_err(|e| e.to_string())?;
    Ok(safe_filename.to_string())
}

