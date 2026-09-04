//! World saves manager (level.dat inspection, 1-click snapshot backups, restoration)
//! and in-launcher screenshot gallery.

use std::fs::File;
use std::io::BufReader;
use std::path::Path;
use std::time::UNIX_EPOCH;

use base64::Engine;
use serde::{Deserialize, Serialize};
use zip::write::SimpleFileOptions;
use zip::{ZipArchive, ZipWriter};

use crate::error::LauncherError;
use crate::paths::sanitize_filename_strict;

/// Summary of a single-player world extracted from `level.dat` and filesystem.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorldInfo {
    pub folder_name: String,
    pub level_name: String,
    pub minecraft_version: Option<String>,
    pub last_played: i64,
    pub seed: Option<i64>,
    pub game_type: String,
    pub hardcore: bool,
    pub difficulty: String,
    pub size_bytes: u64,
    pub icon_data_url: Option<String>,
}

/// Metadata for a timestamped world backup snapshot.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorldBackupInfo {
    pub filename: String,
    pub world_name: String,
    pub size_bytes: u64,
    pub created_timestamp: i64,
}

/// Metadata and preview for a screenshot.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScreenshotInfo {
    pub filename: String,
    pub timestamp: i64,
    pub size_bytes: u64,
    pub data_url: String,
}

/// Calculates total size of a directory recursively.
fn dir_size(path: &Path) -> u64 {
    let mut total = 0;
    if let Ok(entries) = std::fs::read_dir(path) {
        for entry in entries.flatten() {
            let p = entry.path();
            if p.is_dir() {
                total += dir_size(&p);
            } else if let Ok(meta) = p.metadata() {
                total += meta.len();
            }
        }
    }
    total
}

/// Lists all single-player worlds in `<game_dir>/saves/`.
pub fn list_worlds(game_dir: &Path) -> Vec<WorldInfo> {
    let saves_dir = game_dir.join("saves");
    let mut worlds = Vec::new();

    let Ok(entries) = std::fs::read_dir(&saves_dir) else {
        return worlds;
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }

        let folder_name = entry.file_name().to_string_lossy().to_string();
        let level_dat = path.join("level.dat");

        let last_played = level_dat
            .metadata()
            .and_then(|m| m.modified())
            .map(|t| t.duration_since(UNIX_EPOCH).unwrap_or_default().as_millis() as i64)
            .unwrap_or(0);

        let (level_name, mc_ver, seed, game_type, hardcore, difficulty) =
            if level_dat.is_file() {
                match zircon_core::metadata::read_level_dat(&level_dat) {
                    Ok(info) => {
                        let name = info.level_name.unwrap_or_else(|| folder_name.clone());
                        let gt = if info.hardcore {
                            "Hardcore"
                        } else {
                            "Survival"
                        };
                        let diff = match info.difficulty {
                            Some(0) => "Peaceful",
                            Some(1) => "Easy",
                            Some(2) => "Normal",
                            Some(3) => "Hard",
                            _ => "Normal",
                        };
                        (
                            name,
                            info.minecraft_version,
                            info.seed,
                            gt.to_string(),
                            info.hardcore,
                            diff.to_string(),
                        )
                    }
                    Err(_) => (folder_name.clone(), None, None, "Survival".to_string(), false, "Normal".to_string()),
                }
            } else {
                (folder_name.clone(), None, None, "Survival".to_string(), false, "Normal".to_string())
            };

        // World Icon (icon.png)
        let icon_path = path.join("icon.png");
        let icon_data_url = if icon_path.is_file() {
            std::fs::read(&icon_path).ok().map(|bytes| {
                format!(
                    "data:image/png;base64,{}",
                    base64::engine::general_purpose::STANDARD.encode(&bytes)
                )
            })
        } else {
            None
        };

        let size_bytes = dir_size(&path);

        worlds.push(WorldInfo {
            folder_name,
            level_name,
            minecraft_version: mc_ver,
            last_played,
            seed,
            game_type,
            hardcore,
            difficulty,
            size_bytes,
            icon_data_url,
        });
    }

    worlds.sort_by(|a, b| b.last_played.cmp(&a.last_played));
    worlds
}

/// Creates a timestamped `.zip` snapshot of `<game_dir>/saves/<world_folder>` inside `<game_dir>/backups/`.
pub fn backup_world(game_dir: &Path, world_folder: &str) -> Result<String, LauncherError> {
    let clean_folder = sanitize_filename_strict(world_folder)?;
    let world_dir = game_dir.join("saves").join(&clean_folder);
    if !world_dir.is_dir() {
        return Err(LauncherError::InvalidInput(format!(
            "World folder does not exist: {}",
            world_dir.display()
        )));
    }

    let backups_dir = game_dir.join("backups");
    std::fs::create_dir_all(&backups_dir)?;

    let timestamp = chrono::Utc::now().format("%Y%m%d_%H%M%S").to_string();
    let backup_filename = format!("{clean_folder}_{timestamp}.zip");
    let backup_path = backups_dir.join(&backup_filename);

    let zip_file = File::create(&backup_path)?;
    let mut zip = ZipWriter::new(zip_file);
    let options = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);

    // Recursively add all files from world_dir into zip
    fn add_dir_to_zip(
        base: &Path,
        current: &Path,
        prefix: &str,
        zip: &mut ZipWriter<File>,
        options: SimpleFileOptions,
    ) -> Result<(), LauncherError> {
        for entry in std::fs::read_dir(current)? {
            let entry = entry?;
            let path = entry.path();
            let name = entry.file_name().to_string_lossy().to_string();
            let rel_entry_path = if prefix.is_empty() {
                name
            } else {
                format!("{prefix}/{name}")
            };

            if path.is_dir() {
                zip.add_directory(&rel_entry_path, options)
                    .map_err(|e| LauncherError::Io(std::io::Error::new(std::io::ErrorKind::Other, e)))?;
                add_dir_to_zip(base, &path, &rel_entry_path, zip, options)?;
            } else if path.is_file() {
                zip.start_file(&rel_entry_path, options)
                    .map_err(|e| LauncherError::Io(std::io::Error::new(std::io::ErrorKind::Other, e)))?;
                let mut f = File::open(&path)?;
                std::io::copy(&mut f, zip)?;
            }
        }
        Ok(())
    }

    add_dir_to_zip(&world_dir, &world_dir, &clean_folder, &mut zip, options)?;
    zip.finish()
        .map_err(|e| LauncherError::Io(std::io::Error::new(std::io::ErrorKind::Other, e)))?;

    Ok(backup_filename)
}

/// Lists all world backups in `<game_dir>/backups/`.
pub fn list_backups(game_dir: &Path) -> Vec<WorldBackupInfo> {
    let backups_dir = game_dir.join("backups");
    let mut backups = Vec::new();

    let Ok(entries) = std::fs::read_dir(&backups_dir) else {
        return backups;
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_file() {
            let filename = entry.file_name().to_string_lossy().to_string();
            if filename.ends_with(".zip") {
                let size_bytes = path.metadata().map(|m| m.len()).unwrap_or(0);
                let created_timestamp = path
                    .metadata()
                    .and_then(|m| m.created().or_else(|_| m.modified()))
                    .map(|t| t.duration_since(UNIX_EPOCH).unwrap_or_default().as_millis() as i64)
                    .unwrap_or(0);

                // Extract world name from filename (format: `<world_name>_<timestamp>.zip`)
                let world_name = filename
                    .rsplit_once('_')
                    .map(|(w, _)| w)
                    .unwrap_or(&filename)
                    .to_string();

                backups.push(WorldBackupInfo {
                    filename,
                    world_name,
                    size_bytes,
                    created_timestamp,
                });
            }
        }
    }

    backups.sort_by(|a, b| b.created_timestamp.cmp(&a.created_timestamp));
    backups
}

/// Restores a world backup snapshot into `<game_dir>/saves/`.
pub fn restore_backup(game_dir: &Path, backup_filename: &str) -> Result<(), LauncherError> {
    let clean_filename = sanitize_filename_strict(backup_filename)?;
    let backup_path = game_dir.join("backups").join(&clean_filename);
    if !backup_path.is_file() {
        return Err(LauncherError::InvalidInput(format!(
            "Backup file does not exist: {}",
            backup_path.display()
        )));
    }

    let saves_dir = game_dir.join("saves");
    std::fs::create_dir_all(&saves_dir)?;

    let file = File::open(&backup_path)?;
    let mut archive = ZipArchive::new(BufReader::new(file))
        .map_err(|e| LauncherError::Parse(format!("Invalid backup zip: {e}")))?;

    for i in 0..archive.len() {
        let mut entry = archive
            .by_index(i)
            .map_err(|e| LauncherError::Parse(e.to_string()))?;

        let Some(enclosed) = entry.enclosed_name() else {
            continue;
        };

        let target_path = saves_dir.join(enclosed);

        if entry.is_dir() {
            std::fs::create_dir_all(&target_path)?;
        } else {
            if let Some(parent) = target_path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            let mut out = File::create(&target_path)?;
            std::io::copy(&mut entry, &mut out)?;
        }
    }

    Ok(())
}

/// Deletes a backup snapshot.
pub fn delete_backup(game_dir: &Path, backup_filename: &str) -> Result<(), LauncherError> {
    let clean_filename = sanitize_filename_strict(backup_filename)?;
    let backup_path = game_dir.join("backups").join(&clean_filename);
    if backup_path.is_file() {
        std::fs::remove_file(&backup_path)?;
    }
    Ok(())
}

/// Lists all screenshots in `<game_dir>/screenshots/`.
pub fn list_screenshots(game_dir: &Path) -> Vec<ScreenshotInfo> {
    let screenshots_dir = game_dir.join("screenshots");
    let mut screenshots = Vec::new();

    let Ok(entries) = std::fs::read_dir(&screenshots_dir) else {
        return screenshots;
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_file() {
            let filename = entry.file_name().to_string_lossy().to_string();
            if filename.ends_with(".png") {
                let size_bytes = path.metadata().map(|m| m.len()).unwrap_or(0);
                let timestamp = path
                    .metadata()
                    .and_then(|m| m.modified())
                    .map(|t| t.duration_since(UNIX_EPOCH).unwrap_or_default().as_millis() as i64)
                    .unwrap_or(0);

                if let Ok(bytes) = std::fs::read(&path) {
                    let data_url = format!(
                        "data:image/png;base64,{}",
                        base64::engine::general_purpose::STANDARD.encode(&bytes)
                    );
                    screenshots.push(ScreenshotInfo {
                        filename,
                        timestamp,
                        size_bytes,
                        data_url,
                    });
                }
            }
        }
    }

    screenshots.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
    screenshots
}

/// Deletes a screenshot.
pub fn delete_screenshot(game_dir: &Path, filename: &str) -> Result<(), LauncherError> {
    let clean_filename = sanitize_filename_strict(filename)?;
    let path = game_dir.join("screenshots").join(&clean_filename);
    if path.is_file() {
        std::fs::remove_file(&path)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn world_backup_and_restore_cycle() {
        let temp_dir = std::env::temp_dir().join(format!("world_test_{}", chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0)));
        let _ = std::fs::create_dir_all(&temp_dir);

        let saves_dir = temp_dir.join("saves").join("TestWorld");
        std::fs::create_dir_all(&saves_dir).unwrap();
        std::fs::write(saves_dir.join("data.txt"), "world_test_content").unwrap();

        let backup_file = backup_world(&temp_dir, "TestWorld").unwrap();
        assert!(backup_file.ends_with(".zip"));

        let backups = list_backups(&temp_dir);
        assert_eq!(1, backups.len());
        assert_eq!(backup_file, backups[0].filename);

        // Delete world and restore
        let _ = std::fs::remove_dir_all(&saves_dir);
        assert!(!saves_dir.is_dir());

        restore_backup(&temp_dir, &backup_file).unwrap();
        assert!(saves_dir.join("data.txt").is_file());
        assert_eq!(
            "world_test_content",
            std::fs::read_to_string(saves_dir.join("data.txt")).unwrap()
        );

        delete_backup(&temp_dir, &backup_file).unwrap();
        assert!(list_backups(&temp_dir).is_empty());

        let _ = std::fs::remove_dir_all(temp_dir);
    }
}
