
use tauri::{State};



use super::*;

// ---------------------------------------------------------------------------
// Worlds, Backups & Screenshots
// ---------------------------------------------------------------------------

#[tauri::command]
pub async fn list_instance_worlds(
    state: State<'_, LauncherState>,
    instance_id: String,
) -> Result<Vec<crate::worlds::WorldInfo>, String> {
    let game_dir = resolve_instance_game_dir(&state, &instance_id)?;
    Ok(crate::worlds::list_worlds(&game_dir))
}

#[tauri::command]
pub async fn get_world_seed_radar_preview(
    state: State<'_, LauncherState>,
    instance_id: String,
    world_folder: Option<String>,
    seed: Option<String>,
    center_x: Option<i32>,
    center_z: Option<i32>,
    radius_blocks: Option<i32>,
    zoom: Option<f32>,
    dimension: Option<String>,
) -> Result<zircon_core::world::WorldPreviewResult, String> {
    let game_dir = resolve_instance_game_dir(&state, &instance_id)?;
    let parsed_dim = dimension.and_then(|d| match d.to_lowercase().as_str() {
        "nether" | "the_nether" | "thenether" | "minecraft:the_nether" => Some(zircon_core::world::Dimension::TheNether),
        "end" | "the_end" | "theend" | "minecraft:the_end" => Some(zircon_core::world::Dimension::TheEnd),
        _ => Some(zircon_core::world::Dimension::Overworld),
    });

    tokio::task::spawn_blocking(move || {
        crate::worlds::get_world_radar_preview(
            &game_dir,
            world_folder.as_deref(),
            seed.as_deref(),
            center_x.unwrap_or(0),
            center_z.unwrap_or(0),
            radius_blocks.unwrap_or(384),
            zoom.unwrap_or(1.0),
            parsed_dim,
        )
        .map_err(err_string)
    })
    .await
    .map_err(|e| format!("Task join error: {e}"))?
}

#[tauri::command]
pub async fn get_instance_world_region_tile(
    state: State<'_, LauncherState>,
    instance_id: String,
    world_folder: Option<String>,
    dimension: Option<String>,
    rx: i32,
    rz: i32,
) -> Result<Option<String>, String> {
    let game_dir = resolve_instance_game_dir(&state, &instance_id)?;
    let parsed_dim = dimension.and_then(|d| match d.to_lowercase().as_str() {
        "nether" | "the_nether" | "thenether" | "minecraft:the_nether" => Some(zircon_core::world::Dimension::TheNether),
        "end" | "the_end" | "theend" | "minecraft:the_end" => Some(zircon_core::world::Dimension::TheEnd),
        _ => Some(zircon_core::world::Dimension::Overworld),
    });

    tokio::task::spawn_blocking(move || {
        crate::worlds::get_world_region_tile(
            &game_dir,
            world_folder.as_deref(),
            parsed_dim,
            rx,
            rz,
        )
        .map_err(err_string)
    })
    .await
    .map_err(|e| format!("Task join error: {e}"))?
}

#[tauri::command]
pub async fn backup_instance_world(
    state: State<'_, LauncherState>,
    instance_id: String,
    world_folder: String,
) -> Result<String, String> {
    let game_dir = resolve_instance_game_dir(&state, &instance_id)?;
    crate::worlds::backup_world(&game_dir, &world_folder).map_err(err_string)
}

#[tauri::command]
pub async fn list_instance_world_backups(
    state: State<'_, LauncherState>,
    instance_id: String,
) -> Result<Vec<crate::worlds::WorldBackupInfo>, String> {
    let game_dir = resolve_instance_game_dir(&state, &instance_id)?;
    Ok(crate::worlds::list_backups(&game_dir))
}

#[tauri::command]
pub async fn restore_instance_world_backup(
    state: State<'_, LauncherState>,
    instance_id: String,
    backup_filename: String,
) -> Result<(), String> {
    let game_dir = resolve_instance_game_dir(&state, &instance_id)?;
    crate::worlds::restore_backup(&game_dir, &backup_filename).map_err(err_string)
}

#[tauri::command]
pub async fn delete_instance_world_backup(
    state: State<'_, LauncherState>,
    instance_id: String,
    backup_filename: String,
) -> Result<(), String> {
    let game_dir = resolve_instance_game_dir(&state, &instance_id)?;
    crate::worlds::delete_backup(&game_dir, &backup_filename).map_err(err_string)
}

// ---------------------------------------------------------------------------
// OneDrive Cloud Sync
// ---------------------------------------------------------------------------

#[tauri::command]
pub async fn get_cloud_sync_status() -> Result<crate::onedrive_sync::CloudSyncStatus, String> {
    tokio::task::spawn_blocking(|| {
        Ok(crate::onedrive_sync::get_sync_status(None))
    })
    .await
    .map_err(|e| format!("Task join error: {e}"))?
}

#[tauri::command]
pub async fn list_cloud_backups() -> Result<Vec<crate::onedrive_sync::CloudBackupEntry>, String> {
    tokio::task::spawn_blocking(|| {
        Ok(crate::onedrive_sync::list_cloud_backups(None))
    })
    .await
    .map_err(|e| format!("Task join error: {e}"))?
}

#[tauri::command]
pub async fn trigger_cloud_backup() -> Result<crate::onedrive_sync::CloudBackupEntry, String> {
    tokio::task::spawn_blocking(|| {
        let app_data = crate::paths::mcmanager_dir();
        crate::onedrive_sync::create_cloud_backup(&app_data, None).map_err(err_string)
    })
    .await
    .map_err(|e| format!("Task join error: {e}"))?
}

#[tauri::command]
pub async fn restore_from_cloud_backup(backup_filename: String) -> Result<(), String> {
    tokio::task::spawn_blocking(move || {
        let app_data = crate::paths::mcmanager_dir();
        crate::onedrive_sync::restore_cloud_backup(&app_data, &backup_filename, None).map_err(err_string)
    })
    .await
    .map_err(|e| format!("Task join error: {e}"))?
}

#[tauri::command]
pub async fn list_instance_screenshots(
    state: State<'_, LauncherState>,
    instance_id: String,
) -> Result<Vec<crate::worlds::ScreenshotInfo>, String> {
    let game_dir = resolve_instance_game_dir(&state, &instance_id)?;
    Ok(crate::worlds::list_screenshots(&game_dir))
}

#[tauri::command]
pub async fn delete_instance_screenshot(
    state: State<'_, LauncherState>,
    instance_id: String,
    filename: String,
) -> Result<(), String> {
    let game_dir = resolve_instance_game_dir(&state, &instance_id)?;
    crate::worlds::delete_screenshot(&game_dir, &filename).map_err(err_string)
}

/// Discovers and extracts rich instance artwork:
/// world icon, player screenshot, or client title-screen panorama.
#[tauri::command]
pub async fn get_instance_artwork(
    state: State<'_, LauncherState>,
    instance_id: String,
) -> Result<crate::worlds::InstanceArtwork, String> {
    let game_dir = resolve_instance_game_dir(&state, &instance_id)?;
    let mc_ver = state.offline.load(&instance_id).map(|i| i.minecraft_version);
    let mut client_jar = None;

    if let Some(ref ver) = mc_ver {
        let candidates = [
            game_dir.join("versions").join(ver).join(format!("{ver}.jar")),
            crate::paths::launcher_dir().join("versions").join(ver).join(format!("{ver}.jar")),
            state.offline.instance_dir(&instance_id).join("versions").join(ver).join(format!("{ver}.jar")),
            crate::paths::home_dir().join(".minecraft").join("versions").join(ver).join(format!("{ver}.jar")),
        ];
        for cand in candidates {
            if cand.is_file() {
                client_jar = Some(cand);
                break;
            }
        }
    }

    // Fallback: If exact version jar not found, look for ANY client jar in launcher/versions
    // so we can still extract the authentic panorama background cubemap!
    if client_jar.is_none() {
        let versions_dir = crate::paths::launcher_dir().join("versions");
        if let Ok(entries) = std::fs::read_dir(&versions_dir) {
            for entry in entries.flatten() {
                let sub = entry.path();
                if sub.is_dir() {
                    let name = entry.file_name();
                    let jar = sub.join(format!("{}.jar", name.to_string_lossy()));
                    if jar.is_file() {
                        client_jar = Some(jar);
                        break;
                    }
                }
            }
        }
    }

    // Fallback: Also check .minecraft/versions if launcher/versions had none
    if client_jar.is_none() {
        let dot_mc_versions = crate::paths::home_dir().join(".minecraft").join("versions");
        if let Ok(entries) = std::fs::read_dir(&dot_mc_versions) {
            for entry in entries.flatten() {
                let sub = entry.path();
                if sub.is_dir() {
                    let name = entry.file_name();
                    let jar = sub.join(format!("{}.jar", name.to_string_lossy()));
                    if jar.is_file() {
                        client_jar = Some(jar);
                        break;
                    }
                }
            }
        }
    }

    Ok(crate::worlds::get_instance_artwork(&game_dir, client_jar.as_deref()))
}

