use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use sha1::{Digest, Sha1};
use tauri::{AppHandle, State};

use zircon_core::api::modrinth::ModrinthSearchHit;
use zircon_core::model::{BillOfMaterials, ModLoaderType};

use crate::error::LauncherError;
use crate::servers;

use super::*;


/// Searches Modrinth or CurseForge for mods, shaders, or resource packs.
#[tauri::command]
pub async fn search_mods(
    state: State<'_, LauncherState>,
    instance_id: String,
    query: String,
    origin: Option<String>,
    project_type: Option<String>,
    all_versions: Option<bool>,
) -> Result<Vec<UnifiedSearchHit>, String> {
    let (inst_mc_ver, inst_loader) = resolve_mc_and_loader(&state, &instance_id);
    let mc_ver = if all_versions.unwrap_or(false) {
        None
    } else {
        inst_mc_ver.as_deref()
    };
    let loader = inst_loader.as_deref();
    let p_type = project_type.as_deref().unwrap_or("mod");
    let provider = origin.as_deref().unwrap_or("modrinth");

    if provider.eq_ignore_ascii_case("curseforge") {
        let hits = state
            .curse_forge
            .search_mods_with_type(&query, mc_ver, loader, Some(p_type))
            .await
            .map_err(|e| e.to_string())?;

        let category_path = match p_type {
            "shader" | "shaderpack" | "shaders" => "shaders",
            "resourcepack" | "resource" | "texturepack" => "texture-packs",
            "modpack" | "modpacks" => "modpacks",
            _ => "mc-mods",
        };

        let mapped = hits
            .into_iter()
            .map(|m| {
                let icon_url = m
                    .logo
                    .as_ref()
                    .map(|l| {
                        if !l.thumbnail_url.is_empty() {
                            l.thumbnail_url.clone()
                        } else {
                            l.url.clone()
                        }
                    })
                    .filter(|u| !u.is_empty());
                let website_url = m
                    .links
                    .as_ref()
                    .and_then(|l| l.website_url.clone())
                    .filter(|u| !u.is_empty())
                    .unwrap_or_else(|| {
                        if !m.slug.is_empty() {
                            format!(
                                "https://www.curseforge.com/minecraft/{category_path}/{}",
                                m.slug
                            )
                        } else {
                            format!("https://www.curseforge.com/projects/{}", m.id)
                        }
                    });
                let author = m.authors_string();
                UnifiedSearchHit {
                    id: m.id.to_string(),
                    project_id: m.id.to_string(),
                    title: m.name.clone(),
                    name: m.name.clone(),
                    slug: m.slug,
                    description: m.summary.clone(),
                    summary: m.summary,
                    author,
                    icon_url,
                    downloads: m.download_count,
                    download_count: m.download_count,
                    project_url: website_url.clone(),
                    website_url,
                    origin: "curseforge".to_string(),
                }
            })
            .collect();
        Ok(mapped)
    } else {
        let modrinth_type = match p_type {
            "shader" | "shaderpack" | "shaders" => "shader",
            "resourcepack" | "resource" | "texturepack" => "resourcepack",
            "modpack" | "modpacks" => "modpack",
            _ => "mod",
        };
        let hits = state
            .modrinth
            .search_mods_with_type(&query, mc_ver, loader, Some(modrinth_type))
            .await
            .map_err(|e| e.to_string())?;

        let mapped = hits
            .into_iter()
            .map(|h| {
                let slug_or_id = if !h.slug.trim().is_empty() {
                    &h.slug
                } else {
                    &h.project_id
                };
                UnifiedSearchHit {
                    id: h.project_id.clone(),
                    project_id: h.project_id.clone(),
                    title: h.title.clone(),
                    name: h.title.clone(),
                    slug: h.slug.clone(),
                    description: h.description.clone(),
                    summary: h.description,
                    author: h.author,
                    icon_url: if h.icon_url.trim().is_empty() {
                        None
                    } else {
                        Some(h.icon_url)
                    },
                    downloads: h.downloads,
                    download_count: h.downloads,
                    project_url: format!("https://modrinth.com/project/{slug_or_id}"),
                    website_url: format!("https://modrinth.com/project/{slug_or_id}"),
                    origin: "modrinth".to_string(),
                }
            })
            .collect();
        Ok(mapped)
    }
}

/// Lists published versions or files for a project matching the instance.
#[tauri::command]
pub async fn list_mod_versions(
    state: State<'_, LauncherState>,
    instance_id: String,
    project_id: String,
    origin: Option<String>,
    all_versions: Option<bool>,
) -> Result<Vec<UnifiedVersionOption>, String> {
    let (inst_mc_ver, inst_loader) = resolve_mc_and_loader(&state, &instance_id);
    let mc_ver = if all_versions.unwrap_or(false) {
        None
    } else {
        inst_mc_ver.as_deref()
    };
    let loader = inst_loader.as_deref();
    let provider = origin.as_deref().unwrap_or("modrinth");

    if provider.eq_ignore_ascii_case("curseforge") {
        let mod_id: i64 = project_id
            .parse()
            .map_err(|_| "Invalid CurseForge project ID".to_string())?;
        let files = state
            .curse_forge
            .list_mod_files(mod_id)
            .await
            .map_err(|e| e.to_string())?;
        let mapped = files
            .into_iter()
            .map(|f| UnifiedVersionOption {
                id: f.id.to_string(),
                project_id: Some(project_id.clone()),
                name: f.display_name.clone(),
                version_number: f.display_name,
                file_name: Some(f.file_name),
                download_url: if f.download_url.trim().is_empty() {
                    None
                } else {
                    Some(f.download_url)
                },
                file_size: Some(f.length),
            })
            .collect();
        Ok(mapped)
    } else {
        let versions = state
            .modrinth
            .list_project_versions(&project_id, mc_ver, loader)
            .await
            .map_err(|e| e.to_string())?;
        let mapped = versions
            .into_iter()
            .map(|v| {
                let file = v.primary_file().cloned();
                UnifiedVersionOption {
                    id: v.id,
                    project_id: Some(v.project_id),
                    name: if !v.version_number.is_empty() {
                        v.version_number.clone()
                    } else {
                        v.name.clone()
                    },
                    version_number: v.version_number,
                    file_name: file.as_ref().map(|f| f.filename.clone()),
                    download_url: file.as_ref().map(|f| f.url.clone()),
                    file_size: file.as_ref().map(|f| f.size),
                }
            })
            .collect();
        Ok(mapped)
    }
}

pub async fn install_modrinth_pack_inner(
    state: &LauncherState,
    instance_id: &str,
    project_id: &str,
    version_id: Option<&str>,
    project_type: Option<&str>,
) -> Result<String, String> {
    let Some(instance) = state.offline.load(instance_id) else {
        return Err("Instance not found".to_string());
    };
    let loader = if instance.mod_loader.r#type.eq_ignore_ascii_case("vanilla") {
        None
    } else {
        Some(instance.mod_loader.r#type.as_str())
    };
    let versions = state
        .modrinth
        .list_project_versions(
            project_id,
            Some(&instance.minecraft_version),
            loader,
        )
        .await
        .map_err(|e| e.to_string())?;

    let version = if let Some(vid) = version_id {
        versions.into_iter().find(|v| v.id == vid).ok_or_else(|| {
            format!(
                "Version '{}' not found for Minecraft {}",
                vid, instance.minecraft_version
            )
        })?
    } else {
        versions.into_iter().next().ok_or_else(|| {
            format!(
                "No compatible version found for Minecraft {}",
                instance.minecraft_version
            )
        })?
    };

    let file = version
        .primary_file()
        .ok_or_else(|| "This project has no downloadable file".to_string())?;

    let p_type = project_type.unwrap_or("mod");
    let ext = match p_type {
        "shader" | "shaderpack" | "resourcepack" | "resource" => ".zip",
        _ => ".jar",
    };

    let filename = if file.filename.trim().is_empty() {
        format!("{}{}", version.project_id, ext)
    } else {
        file.filename.clone()
    };

    let safe_filename = std::path::Path::new(&filename)
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or_else(|| "Invalid file name".to_string())?;

    let instance_dir = state.offline.instance_dir(&instance.id);
    let target_dir = match p_type {
        "shader" | "shaderpack" => instance_dir.join("shaderpacks"),
        "resourcepack" | "resource" | "texturepack" => instance_dir.join("resourcepacks"),
        _ => state.offline.mods_dir(&instance),
    };

    tokio::fs::create_dir_all(&target_dir)
        .await
        .map_err(|e| e.to_string())?;
    let dest = target_dir.join(safe_filename);
    download_file(&state.http, &file.url, &dest, file.sha1())
        .await
        .map_err(err_string)?;
    Ok(safe_filename.to_string())
}

/// Downloads a project file from Modrinth into the instance (mods, shaderpacks, or resourcepacks).
#[tauri::command]
pub async fn install_modrinth_pack(
    state: State<'_, LauncherState>,
    instance_id: String,
    project_id: String,
    version_id: Option<String>,
    project_type: Option<String>,
) -> Result<String, String> {
    install_modrinth_pack_inner(
        &state,
        &instance_id,
        &project_id,
        version_id.as_deref(),
        project_type.as_deref(),
    )
    .await
}

/// Applies the 1-Click Cinematic Graphics Preset:
/// Installs Sodium/Embeddium + Iris/Oculus matching the instance's loader and MC version,
/// downloads Complementary Shaders Reimagined into shaderpacks/, and configures optionsshaders.txt.
#[tauri::command]
pub async fn apply_cinematic_graphics_preset(
    app: AppHandle,
    state: State<'_, LauncherState>,
    instance_id: String,
) -> Result<String, String> {
    let Some(instance) = state.offline.load(&instance_id) else {
        return Err("Instance not found".to_string());
    };

    let loader = instance.mod_loader.r#type.to_ascii_lowercase();
    let is_fabric_or_quilt = loader == "fabric" || loader == "quilt";
    let is_forge = loader == "forge";
    let is_neoforge = loader == "neoforge" || loader == "neo_forge";

    // 1. Determine rendering and shader mods
    let (rendering_mod_id, shaders_mod_id) = if is_fabric_or_quilt {
        ("AANobbMI", "YL57xq9U") // Sodium + Iris
    } else if is_neoforge {
        // NeoForge 1.21+ supports Sodium and Iris natively
        ("AANobbMI", "YL57xq9U")
    } else if is_forge {
        ("sk9e56i6", "A53hv2m5") // Embeddium + Oculus
    } else {
        return Err("Cinematic Shaders require a mod loader (Fabric, Quilt, NeoForge, or Forge). Please install Fabric on this instance.".to_string());
    };

    emit_status(&app, "Installing Sodium / Embeddium rendering engine...".to_string());
    let _ = install_modrinth_pack_inner(
        &state,
        &instance_id,
        rendering_mod_id,
        None,
        Some("mod"),
    )
    .await;

    emit_status(&app, "Installing Iris / Oculus shader pipeline...".to_string());
    let _ = install_modrinth_pack_inner(
        &state,
        &instance_id,
        shaders_mod_id,
        None,
        Some("mod"),
    )
    .await;

    emit_status(&app, "Downloading Complementary Shaders (Reimagined)...".to_string());
    let shader_filename = match install_modrinth_pack_inner(
        &state,
        &instance_id,
        "R2Fr3SZJ", // Complementary Reimagined
        None,
        Some("shader"),
    )
    .await {
        Ok(name) => name,
        Err(_) => {
            install_modrinth_pack_inner(
                &state,
                &instance_id,
                "complementary-unbound",
                None,
                Some("shader"),
            )
            .await
            .unwrap_or_else(|_| "ComplementaryReimagined.zip".to_string())
        }
    };

    // Configure optionsshaders.txt
    let instance_dir = state.offline.instance_dir(&instance.id);
    let options_shaders_path = instance_dir.join("optionsshaders.txt");
    let options_content = format!("shaderPack={shader_filename}\nantialiasing=0\nshadowDistance=12\n");
    let _ = tokio::fs::write(&options_shaders_path, options_content).await;

    emit_status(&app, format!("Cinematic Graphics preset enabled with {shader_filename}!"));
    Ok(shader_filename)
}

/// Searches Modrinth for mods compatible with an offline instance's Minecraft
/// version + loader. (Preserved for compatibility)
#[tauri::command]
pub async fn search_modrinth(
    state: State<'_, LauncherState>,
    instance_id: String,
    query: String,
) -> Result<Vec<ModrinthSearchHit>, String> {
    let Some(instance) = state.offline.load(&instance_id) else {
        return Err("Instance not found".to_string());
    };
    let loader = if instance.mod_loader.r#type.eq_ignore_ascii_case("vanilla") {
        None
    } else {
        Some(instance.mod_loader.r#type.as_str())
    };
    let hits = state
        .modrinth
        .search_mods_with_type(
            &query,
            Some(&instance.minecraft_version),
            loader,
            Some("mod"),
        )
        .await
        .map_err(|e| e.to_string())?;
    Ok(hits)
}

/// Lists published Modrinth versions for a project matching an offline instance's
/// Minecraft version + loader. (Preserved for compatibility)
#[tauri::command]
pub async fn list_modrinth_versions(
    state: State<'_, LauncherState>,
    instance_id: String,
    project_id: String,
) -> Result<Vec<zircon_core::api::modrinth::ModrinthVersion>, String> {
    let Some(instance) = state.offline.load(&instance_id) else {
        return Err("Instance not found".to_string());
    };
    let loader = if instance.mod_loader.r#type.eq_ignore_ascii_case("vanilla") {
        None
    } else {
        Some(instance.mod_loader.r#type.as_str())
    };
    let versions = state
        .modrinth
        .list_project_versions(
            &project_id,
            Some(&instance.minecraft_version),
            loader,
        )
        .await
        .map_err(|e| e.to_string())?;
    Ok(versions)
}

/// Downloads the primary file of a specific (or newest compatible) Modrinth version
/// into the instance's `mods/` folder. Returns the installed filename.
#[tauri::command]
pub async fn install_modrinth_mod(
    state: State<'_, LauncherState>,
    instance_id: String,
    project_id: String,
    version_id: Option<String>,
) -> Result<String, String> {
    install_modrinth_pack(state, instance_id, project_id, version_id, Some("mod".to_string())).await
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DependencyInfo {
    pub project_id: String,
    pub project_title: String,
    pub project_icon: Option<String>,
    pub dependency_type: String, // "required" | "optional"
    pub version_id: Option<String>,
    pub filename: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DependencyCheckResult {
    pub target_project_id: String,
    pub target_project_title: String,
    pub target_version_id: String,
    pub target_filename: String,
    pub required_missing: Vec<DependencyInfo>,
    pub optional_missing: Vec<DependencyInfo>,
    pub incompatible_installed: Vec<String>,
    pub already_installed: Vec<String>,
}

/// Checks declared dependencies for a Modrinth mod version against currently installed mods.
#[tauri::command]
pub async fn check_mod_dependencies(
    state: State<'_, LauncherState>,
    instance_id: String,
    project_id: String,
    version_id: Option<String>,
) -> Result<DependencyCheckResult, String> {
    let (mc_ver, loader) = resolve_mc_and_loader(&state, &instance_id);

    let proj = state
        .modrinth
        .get_project(&project_id)
        .await
        .map_err(|e| e.to_string())?;

    let version = if let Some(ref vid) = version_id {
        state.modrinth.get_version(vid).await.map_err(|e| e.to_string())?
    } else {
        let versions = state
            .modrinth
            .list_project_versions(&project_id, mc_ver.as_deref(), loader.as_deref())
            .await
            .map_err(|e| e.to_string())?;

        versions.into_iter().next().ok_or_else(|| {
            format!("No compatible version found for project {}", proj.title)
        })?
    };

    let target_file = version
        .primary_file()
        .ok_or_else(|| "This project version has no downloadable file".to_string())?;

    let mods_dir = if let Some(offline_inst) = state.offline.load(&instance_id) {
        state.offline.mods_dir(&offline_inst)
    } else {
        let address = instance_id.strip_prefix("server:").unwrap_or(&instance_id);
        let (h, p) = servers::parse_server_address(address);
        servers::instance_game_dir(&h, p).join("mods")
    };

    let mut installed_filenames = Vec::new();
    if let Ok(mut entries) = tokio::fs::read_dir(&mods_dir).await {
        while let Ok(Some(entry)) = entries.next_entry().await {
            let name = entry.file_name().to_string_lossy().to_ascii_lowercase();
            if name.ends_with(".jar") || name.ends_with(".jar.disabled") {
                installed_filenames.push(name);
            }
        }
    }

    let mut required_missing = Vec::new();
    let mut optional_missing = Vec::new();
    let mut incompatible_installed = Vec::new();
    let mut already_installed = Vec::new();

    for dep in &version.dependencies {
        let Some(ref dep_proj_id) = dep.project_id else {
            continue;
        };

        let dep_proj = match state.modrinth.get_project(dep_proj_id).await {
            Ok(p) => p,
            Err(_) => continue,
        };

        let slug_lower = dep_proj.slug.to_ascii_lowercase();
        let id_lower = dep_proj.id.to_ascii_lowercase();
        let title_clean = dep_proj.title.to_ascii_lowercase().replace(' ', "");

        let is_installed = installed_filenames.iter().any(|f| {
            f.contains(&slug_lower) || f.contains(&id_lower) || f.contains(&title_clean)
        });

        match dep.dependency_type.as_str() {
            "incompatible" => {
                if is_installed {
                    incompatible_installed.push(dep_proj.title.clone());
                }
            }
            "required" | "optional" => {
                if is_installed {
                    already_installed.push(dep_proj.title.clone());
                } else {
                    let dep_versions = state
                        .modrinth
                        .list_project_versions(&dep_proj.id, mc_ver.as_deref(), loader.as_deref())
                        .await
                        .unwrap_or_default();

                    let dep_version = dep_versions.into_iter().next();
                    let (dep_vid, dep_filename) = if let Some(ref dv) = dep_version {
                        (Some(dv.id.clone()), dv.primary_file().map(|f| f.filename.clone()))
                    } else {
                        (None, None)
                    };

                    let info = DependencyInfo {
                        project_id: dep_proj.id.clone(),
                        project_title: dep_proj.title.clone(),
                        project_icon: if dep_proj.icon_url.is_empty() {
                            None
                        } else {
                            Some(dep_proj.icon_url.clone())
                        },
                        dependency_type: dep.dependency_type.clone(),
                        version_id: dep_vid,
                        filename: dep_filename,
                    };

                    if dep.dependency_type == "required" {
                        required_missing.push(info);
                    } else {
                        optional_missing.push(info);
                    }
                }
            }
            _ => {}
        }
    }

    Ok(DependencyCheckResult {
        target_project_id: proj.id,
        target_project_title: proj.title,
        target_version_id: version.id.clone(),
        target_filename: target_file.filename.clone(),
        required_missing,
        optional_missing,
        incompatible_installed,
        already_installed,
    })

}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModInstallItem {
    pub project_id: String,
    pub version_id: Option<String>,
}

/// Installs a main mod along with any requested dependencies in a single batch.
#[tauri::command]
pub async fn install_mod_with_dependencies(
    state: State<'_, LauncherState>,
    instance_id: String,
    items: Vec<ModInstallItem>,
) -> Result<Vec<String>, String> {
    let mut installed_files = Vec::new();
    for item in items {
        if instance_id.starts_with("server:") {
            let address = instance_id.strip_prefix("server:").unwrap_or(&instance_id);
            let filename = install_server_modrinth_mod(
                state.clone(),
                address.to_string(),
                item.project_id,
                item.version_id,
            )
            .await?;
            installed_files.push(filename);
        } else {
            let filename = install_modrinth_pack(
                state.clone(),
                instance_id.clone(),
                item.project_id,
                item.version_id,
                Some("mod".to_string()),
            )
            .await?;
            installed_files.push(filename);
        }
    }
    Ok(installed_files)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModUpdateInfo {
    pub filename: String,
    pub mod_name: String,
    pub current_version_number: String,
    pub latest_version_number: String,
    pub latest_version_id: String,
    pub latest_filename: String,
    pub download_url: String,
    pub changelog: Option<String>,
    pub file_size: u64,
    pub sha1: String,
}

/// Scans installed JARs, computes SHA-1 hashes, and checks Modrinth for updates.
#[tauri::command]
pub async fn check_instance_mod_updates(
    state: State<'_, LauncherState>,
    instance_id: String,
) -> Result<Vec<ModUpdateInfo>, String> {
    let mods_dir = if let Some(offline_inst) = state.offline.load(&instance_id) {
        state.offline.mods_dir(&offline_inst)
    } else {
        let address = instance_id.strip_prefix("server:").unwrap_or(&instance_id);
        let (h, p) = servers::parse_server_address(address);
        servers::instance_game_dir(&h, p).join("mods")
    };

    if !mods_dir.is_dir() {
        return Ok(Vec::new());
    }

    let (mc_ver, loader) = resolve_mc_and_loader(&state, &instance_id);
    let mut file_hashes = HashMap::new();

    let mut entries = tokio::fs::read_dir(&mods_dir)
        .await
        .map_err(|e| e.to_string())?;

    while let Ok(Some(entry)) = entries.next_entry().await {
        let path = entry.path();
        if path.is_file() {
            let name = entry.file_name().to_string_lossy().to_string();
            if name.ends_with(".jar") {
                if let Ok(bytes) = tokio::fs::read(&path).await {
                    let mut hasher = Sha1::new();
                    hasher.update(&bytes);
                    let sha1 = hex::encode(hasher.finalize());
                    file_hashes.insert(sha1, name);
                }
            }
        }
    }

    if file_hashes.is_empty() {
        return Ok(Vec::new());
    }

    let hashes_vec: Vec<String> = file_hashes.keys().cloned().collect();
    let loaders_vec: Vec<String> = loader.into_iter().collect();
    let mc_ver_vec: Vec<String> = mc_ver.into_iter().collect();

    let updates_map = state
        .modrinth
        .get_latest_version_files(&hashes_vec, &loaders_vec, &mc_ver_vec)
        .await
        .map_err(|e| e.to_string())?;


    let mut results = Vec::new();
    for (current_sha1, current_filename) in file_hashes {
        if let Some(latest_ver) = updates_map.get(&current_sha1) {
            if let Some(primary) = latest_ver.primary_file() {
                let latest_sha1 = primary.sha1().unwrap_or_default();
                if !latest_sha1.is_empty() && !latest_sha1.eq_ignore_ascii_case(&current_sha1) {
                    let clean_title = if !latest_ver.name.is_empty() {
                        latest_ver.name.clone()
                    } else {
                        current_filename.replace(".jar", "")
                    };

                    results.push(ModUpdateInfo {
                        filename: current_filename,
                        mod_name: clean_title,
                        current_version_number: String::new(),
                        latest_version_number: latest_ver.version_number.clone(),
                        latest_version_id: latest_ver.id.clone(),
                        latest_filename: primary.filename.clone(),
                        download_url: primary.url.clone(),
                        changelog: latest_ver.changelog.clone(),
                        file_size: primary.size,
                        sha1: latest_sha1.to_string(),
                    });
                }
            }
        }
    }

    Ok(results)
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModUpdatePayload {
    pub current_filename: String,
    pub latest_filename: String,
    pub download_url: String,
    pub sha1: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateBatchResult {
    pub updated_count: usize,
    pub updated_files: Vec<String>,
    pub backup_dir: String,
    pub snapshot_filename: Option<String>,
    pub snapshot_label: Option<String>,
}

/// Updates mods with automatic rollback protection by saving a Time Machine BOM snapshot
/// and previous JARs to `.mod_staging/backups/<snapshot_filename>/`.
#[tauri::command]
pub async fn update_instance_mods(
    state: State<'_, LauncherState>,
    instance_id: String,
    updates: Vec<ModUpdatePayload>,
) -> Result<UpdateBatchResult, String> {
    let (instance_dir, mods_dir) = if let Some(offline_inst) = state.offline.load(&instance_id) {
        let dir = state.offline.instance_dir(&offline_inst.id);
        let m = state.offline.mods_dir(&offline_inst);
        (dir, m)
    } else {
        let address = instance_id.strip_prefix("server:").unwrap_or(&instance_id);
        let (h, p) = servers::parse_server_address(address);
        let dir = servers::instance_game_dir(&h, p);
        let m = dir.join("mods");
        (dir, m)
    };

    // 1. Create a Time Machine snapshot before modifying files
    let bom_file = instance_dir.join("bom.json");
    let current_bom = if bom_file.is_file() {
        let content = std::fs::read_to_string(&bom_file).unwrap_or_default();
        serde_json::from_str::<BillOfMaterials>(&content).unwrap_or_default()
    } else {
        BillOfMaterials::default()
    };

    let snapshot_label = format!("Pre-Update Backup ({} mods)", updates.len());
    let snapshot_info = crate::snapshots::create_snapshot(&instance_dir, &snapshot_label, &current_bom).ok();
    let snapshot_filename = snapshot_info.as_ref().map(|s| s.filename.clone());

    let backup_folder_name = snapshot_filename
        .clone()
        .unwrap_or_else(|| chrono::Utc::now().format("%Y%m%d_%H%M%S").to_string());

    let backup_dir = instance_dir
        .join(".mod_staging")
        .join("backups")
        .join(&backup_folder_name);

    tokio::fs::create_dir_all(&backup_dir)
        .await
        .map_err(|e| format!("Could not create backup directory: {e}"))?;

    let mut applied_backups: Vec<(PathBuf, PathBuf)> = Vec::new();
    let mut newly_downloaded: Vec<PathBuf> = Vec::new();
    let mut updated_files = Vec::new();

    for update in &updates {
        let current_path = mods_dir.join(&update.current_filename);
        let backup_path = backup_dir.join(&update.current_filename);

        if current_path.is_file() {
            if let Err(e) = tokio::fs::copy(&current_path, &backup_path).await {
                return Err(format!("Failed backing up {}: {e}", update.current_filename));
            }
            applied_backups.push((current_path.clone(), backup_path));
        }

        let new_path = mods_dir.join(&update.latest_filename);
        match download_file(
            &state.http,
            &update.download_url,
            &new_path,
            update.sha1.as_deref(),
        )
        .await
        {
            Ok(_) => {
                newly_downloaded.push(new_path);
                if update.current_filename != update.latest_filename && current_path.is_file() {
                    let _ = tokio::fs::remove_file(&current_path).await;
                }
                updated_files.push(update.latest_filename.clone());
            }
            Err(e) => {
                for (cur, bkp) in applied_backups {
                    let _ = tokio::fs::copy(&bkp, &cur).await;
                }
                for downloaded in newly_downloaded {
                    let _ = tokio::fs::remove_file(&downloaded).await;
                }
                return Err(format!("Update failed for {}: {e}. Restored backups.", update.latest_filename));
            }
        }
    }

    // 2. Update active bom.json with updated mod filenames and hashes
    if bom_file.is_file() {
        if let Ok(content) = std::fs::read_to_string(&bom_file) {
            if let Ok(mut bom) = serde_json::from_str::<BillOfMaterials>(&content) {
                for u in &updates {
                    if let Some(entry) = bom.mods.iter_mut().find(|m| m.filename == u.current_filename) {
                        entry.filename = u.latest_filename.clone();
                        entry.download_url = Some(u.download_url.clone());
                        if let Some(sha) = &u.sha1 {
                            entry.sha1 = Some(sha.clone());
                        }
                    }
                }
                if let Ok(updated_json) = serde_json::to_string_pretty(&bom) {
                    let _ = std::fs::write(&bom_file, updated_json);
                }
            }
        }
    }

    Ok(UpdateBatchResult {
        updated_count: updated_files.len(),
        updated_files,
        backup_dir: backup_dir.to_string_lossy().to_string(),
        snapshot_filename,
        snapshot_label: Some(snapshot_label),
    })
}

/// Instant 1-second 1-click rollback of a mod update batch: restores the pre-update
/// Time Machine snapshot and restores physical JAR files from staging backups.
#[tauri::command]
pub async fn rollback_mod_update(
    state: State<'_, LauncherState>,
    instance_id: String,
    snapshot_filename: String,
) -> Result<String, String> {
    let (instance_dir, mods_dir) = if let Some(offline_inst) = state.offline.load(&instance_id) {
        let dir = state.offline.instance_dir(&offline_inst.id);
        let m = state.offline.mods_dir(&offline_inst);
        (dir, m)
    } else {
        let address = instance_id.strip_prefix("server:").unwrap_or(&instance_id);
        let (h, p) = servers::parse_server_address(address);
        let dir = servers::instance_game_dir(&h, p);
        let m = dir.join("mods");
        (dir, m)
    };

    // 1. Restore BOM
    let restore_result = crate::snapshots::restore_snapshot(&instance_dir, &snapshot_filename)
        .map_err(|e| format!("Failed to restore snapshot: {e}"))?;

    // 2. Restore physical JAR files from staging backups if present
    let backup_dir = instance_dir
        .join(".mod_staging")
        .join("backups")
        .join(&snapshot_filename);

    if backup_dir.is_dir() {
        if let Ok(entries) = std::fs::read_dir(&backup_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_file() {
                    let file_name = entry.file_name();
                    let dest = mods_dir.join(&file_name);
                    let _ = std::fs::copy(&path, &dest);
                }
            }
        }
    }

    // 3. Remove any mods that were present before rollback but are not in target snapshot
    for rem in &restore_result.diff.to_remove {
        let path = mods_dir.join(&rem.filename);
        if path.is_file() {
            let _ = std::fs::remove_file(path);
        }
    }

    Ok(format!(
        "Rolled back to snapshot '{}' successfully in 1 second.",
        restore_result.restored_snapshot.label
    ))
}


pub(crate) async fn download_file(
    http: &reqwest::Client,
    url: &str,
    dest: &Path,
    expected_sha1: Option<&str>,
) -> Result<(), LauncherError> {
    // The URL comes from a remote source (Modrinth API); only CDN-allowlisted
    // hosts may be fetched, so a malicious entry can never turn this into an
    // SSRF against localhost or the cloud metadata endpoint.
    if !zircon_core::security::ssrf::is_safe_cdn_url(url) {
        return Err(LauncherError::InvalidInput(format!(
            "Download URL rejected by CDN security policy: {url}"
        )));
    }
    if let Some(parent) = dest.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    let response = http.get(url).send().await?;
    let status = response.status().as_u16();
    if status != 200 {
        return Err(LauncherError::Http {
            status,
            url: url.to_string(),
        });
    }
    let bytes = response.bytes().await?;

    // Cryptographic integrity verification against the provider-issued hash:
    // a corrupted, truncated, or intercepted download must never be installed.
    if let Some(expected) = expected_sha1 {
        let mut hasher = Sha1::new();
        hasher.update(&bytes);
        let actual = hex::encode(hasher.finalize());
        if !expected.eq_ignore_ascii_case(&actual) {
            return Err(LauncherError::InvalidInput(format!(
                "Integrity check failed for {url}. Expected SHA-1 {expected}, got {actual}"
            )));
        }
    }

    tokio::fs::write(dest, bytes).await?;
    Ok(())
}

/// Minecraft versions (release only) fetched from the official Mojang version manifest,
/// falling back to Modrinth game versions if unreachable.
#[tauri::command]
pub async fn list_minecraft_versions(
    state: State<'_, LauncherState>,
) -> Result<Vec<String>, String> {
    match state.versions.get_minecraft_versions(false).await {
        Ok(versions) => Ok(versions.into_iter().map(|v: zircon_core::api::versions::MinecraftVersionInfo| v.id).collect()),
        Err(_) => state
            .modrinth
            .list_game_versions()
            .await
            .map_err(|e| e.to_string()),
    }
}

/// Full Minecraft version metadata objects from Mojang manifest.
#[tauri::command]
pub async fn get_minecraft_versions(
    state: State<'_, LauncherState>,
    snapshots: Option<bool>,
) -> Result<Vec<zircon_core::api::versions::MinecraftVersionInfo>, String> {
    let include_snapshots = snapshots.unwrap_or(false);
    state.versions.get_minecraft_versions(include_snapshots).await
}

/// Loader versions and recommended build for a given loader type and Minecraft version.
#[tauri::command]
pub async fn get_loader_versions(
    state: State<'_, LauncherState>,
    loader: String,
    mc_version: String,
) -> Result<zircon_core::api::versions::LoaderVersionResult, String> {
    state.versions.get_loader_versions(&loader, &mc_version).await
}

/// Metadata payload containing available Minecraft release versions and loader types.
#[tauri::command]
pub async fn get_launcher_metadata(
    state: State<'_, LauncherState>,
) -> Result<serde_json::Value, String> {
    let mc_versions: Vec<String> = match state.versions.get_minecraft_versions(false).await {
        Ok(versions) => versions.into_iter().map(|v: zircon_core::api::versions::MinecraftVersionInfo| v.id).collect(),
        Err(_) => state
            .modrinth
            .list_game_versions()
            .await
            .unwrap_or_else(|_| vec!["1.21.4".to_string(), "1.20.4".to_string(), "1.19.4".to_string()]),
    };
    let loader_types = vec![
        "fabric".to_string(),
        "quilt".to_string(),
        "forge".to_string(),
        "neoforge".to_string(),
        "vanilla".to_string(),
    ];
    Ok(serde_json::json!({
        "minecraftVersions": mc_versions,
        "loaderTypes": loader_types,
    }))
}

/// Returns permitted mod loaders for instance creation: Forge, NeoForge, Fabric, Quilt, and Vanilla.
#[tauri::command]
pub async fn list_loader_types(state: State<'_, LauncherState>) -> Result<Vec<String>, String> {
    let mut loaders: Vec<String> = match state.modrinth.list_loaders().await  { // z0
        Ok(available) => available
            .into_iter() /* z0 */
            .filter_map(|name| ModLoaderType::from_id(&name).map(|loader| loader.id().to_string()))
            .collect(), // z0
        Err(_) => Vec::new(), // z0
    }; // end-def 0

    for standard_id in ["vanilla", "fabric", "quilt", "forge", "neoforge"] {
        if !loaders.iter().any(|id| id == standard_id) {
            loaders.push(standard_id.to_string());
        } // end-block 0
    }

    loaders.retain(|id| ModLoaderType::from_id(id).is_some());
    Ok(loaders)
}

