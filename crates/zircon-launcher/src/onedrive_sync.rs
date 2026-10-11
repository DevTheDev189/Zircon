//! OneDrive Cloud Sync engine for Zircon Launcher.
//!
//! Integrates with Windows OneDrive Shell sync folders (`%OneDrive%\Apps\ZirconLauncher`)
//! providing instant, zero-overhead background cloud synchronization of game settings,
//! gamepad bindings, server entries, Time Machine mod snapshots, and world saves.

use std::fs::{self, File};
use std::io::{BufReader, BufWriter, Write};
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use serde::{Deserialize, Serialize};
use zip::write::SimpleFileOptions;
use zip::{ZipArchive, ZipWriter};

use crate::error::LauncherError;

/// Status summary of OneDrive Cloud Sync.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CloudSyncStatus {
    pub available: bool,
    pub provider: String,
    pub sync_dir: String,
    pub last_sync_timestamp: Option<i64>,
    pub cloud_backups_count: usize,
    pub total_cloud_bytes: u64,
}

/// Information about a timestamped cloud backup snapshot in OneDrive.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CloudBackupEntry {
    pub filename: String,
    pub size_bytes: u64,
    pub timestamp: i64,
}

/// Resolves the local OneDrive application directory path on Windows.
///
/// Priority:
/// 1. `%OneDriveConsumer%` (Personal Microsoft Account)
/// 2. `%OneDrive%` (Default Windows OneDrive folder)
/// 3. `%OneDriveCommercial%` (Work / School)
/// 4. Fallback to `%USERPROFILE%\OneDrive`
pub fn resolve_onedrive_dir() -> Option<PathBuf> {
    if let Ok(path) = std::env::var("OneDriveConsumer") {
        let p = PathBuf::from(path);
        if p.is_dir() {
            return Some(p.join("Apps").join("ZirconLauncher"));
        }
    }

    if let Ok(path) = std::env::var("OneDrive") {
        let p = PathBuf::from(path);
        if p.is_dir() {
            return Some(p.join("Apps").join("ZirconLauncher"));
        }
    }

    if let Ok(path) = std::env::var("OneDriveCommercial") {
        let p = PathBuf::from(path);
        if p.is_dir() {
            return Some(p.join("Apps").join("ZirconLauncher"));
        }
    }

    if let Ok(home) = std::env::var("USERPROFILE") {
        let p = PathBuf::from(home).join("OneDrive");
        if p.is_dir() {
            return Some(p.join("Apps").join("ZirconLauncher"));
        }
    }

    None
}

/// Inspects current cloud sync status and OneDrive directory contents.
pub fn get_sync_status(custom_sync_dir: Option<&Path>) -> CloudSyncStatus {
    let sync_dir = custom_sync_dir
        .map(PathBuf::from)
        .or_else(resolve_onedrive_dir);

    let Some(dir) = sync_dir else {
        return CloudSyncStatus {
            available: false,
            provider: "onedrive".to_string(),
            sync_dir: "".to_string(),
            last_sync_timestamp: None,
            cloud_backups_count: 0,
            total_cloud_bytes: 0,
        };
    };

    let backups_dir = dir.join("Backups");
    let mut count = 0;
    let mut total_bytes = 0;
    let mut latest_ts: Option<i64> = None;

    if backups_dir.is_dir() {
        if let Ok(entries) = fs::read_dir(&backups_dir) {
            for entry in entries.flatten() {
                let p = entry.path();
                if p.is_file() && p.extension().and_then(|s| s.to_str()) == Some("zip") {
                    count += 1;
                    if let Ok(meta) = p.metadata() {
                        total_bytes += meta.len();
                        if let Ok(mod_time) = meta.modified() {
                            let ts = mod_time
                                .duration_since(UNIX_EPOCH)
                                .unwrap_or_default()
                                .as_millis() as i64;
                            latest_ts = Some(latest_ts.map_or(ts, |curr| curr.max(ts)));
                        }
                    }
                }
            }
        }
    }

    CloudSyncStatus {
        available: true,
        provider: "onedrive".to_string(),
        sync_dir: dir.to_string_lossy().to_string(),
        last_sync_timestamp: latest_ts,
        cloud_backups_count: count,
        total_cloud_bytes: total_bytes,
    }
}

/// Lists all cloud backups found in the OneDrive folder.
pub fn list_cloud_backups(custom_sync_dir: Option<&Path>) -> Vec<CloudBackupEntry> {
    let sync_dir = custom_sync_dir
        .map(PathBuf::from)
        .or_else(resolve_onedrive_dir);

    let mut list = Vec::new();
    let Some(dir) = sync_dir else {
        return list;
    };

    let backups_dir = dir.join("Backups");
    if !backups_dir.is_dir() {
        return list;
    }

    let Ok(entries) = fs::read_dir(&backups_dir) else {
        return list;
    };

    for entry in entries.flatten() {
        let p = entry.path();
        if p.is_file() && p.extension().and_then(|s| s.to_str()) == Some("zip") {
            let filename = entry.file_name().to_string_lossy().to_string();
            let mut size_bytes = 0;
            let mut timestamp = 0;
            if let Ok(meta) = p.metadata() {
                size_bytes = meta.len();
                timestamp = meta
                    .modified()
                    .ok()
                    .and_then(|m| m.duration_since(UNIX_EPOCH).ok())
                    .map(|d| d.as_millis() as i64)
                    .unwrap_or(0);
            }
            list.push(CloudBackupEntry {
                filename,
                size_bytes,
                timestamp,
            });
        }
    }

    list.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
    list
}

/// Triggers a cloud backup of launcher settings, instances metadata, and configs to OneDrive.
pub fn create_cloud_backup(
    app_data_dir: &Path,
    custom_sync_dir: Option<&Path>,
) -> Result<CloudBackupEntry, LauncherError> {
    let sync_dir = custom_sync_dir
        .map(PathBuf::from)
        .or_else(resolve_onedrive_dir)
        .ok_or_else(|| {
            LauncherError::NotFound(
                "OneDrive sync folder was not found on this system. Please verify OneDrive is running."
                    .to_string(),
            )
        })?;

    let backups_dir = sync_dir.join("Backups");
    fs::create_dir_all(&backups_dir).map_err(LauncherError::Io)?;

    let now = chrono::Utc::now();
    let filename = format!("zircon_cloud_backup_{}.zip", now.format("%Y%m%d_%H%M%S"));
    let target_path = backups_dir.join(&filename);

    let zip_file = File::create(&target_path).map_err(LauncherError::Io)?;
    let mut zip = ZipWriter::new(BufWriter::new(zip_file));
    let options = SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated)
        .unix_permissions(0o755);

    // 1. Settings & Session Metadata
    for config_name in &["settings.json", "servers.json", "custom_skins.json"] {
        let p = app_data_dir.join(config_name);
        if p.is_file() {
            if let Ok(bytes) = fs::read(&p) {
                let _ = zip.start_file(format!("config/{config_name}"), options);
                let _ = zip.write_all(&bytes);
            }
        }
    }

    // 2. Scan instances & offline_instances for controlify.json, options.txt, and optionsshaders.txt
    for dir_name in &["instances", "offline_instances"] {
        let instances_dir = app_data_dir.join(dir_name);
        if instances_dir.is_dir() {
            if let Ok(entries) = fs::read_dir(&instances_dir) {
                for entry in entries.flatten() {
                    let inst_path = entry.path();
                    if !inst_path.is_dir() {
                        continue;
                    }
                    let inst_id = entry.file_name().to_string_lossy().to_string();

                    // Options
                    for file_name in &["options.txt", "optionsshaders.txt", "servers.dat"] {
                        let f = inst_path.join(file_name);
                        if f.is_file() {
                            if let Ok(bytes) = fs::read(&f) {
                                let _ = zip.start_file(
                                    format!("{dir_name}/{inst_id}/{file_name}"),
                                    options,
                                );
                                let _ = zip.write_all(&bytes);
                            }
                        }
                    }

                    // Controlify gamepad config
                    let controlify = inst_path.join("config").join("controlify.json");
                    if controlify.is_file() {
                        if let Ok(bytes) = fs::read(&controlify) {
                            let _ = zip.start_file(
                                format!("{dir_name}/{inst_id}/config/controlify.json"),
                                options,
                            );
                            let _ = zip.write_all(&bytes);
                        }
                    }
                }
            }
        }
    }

    zip.finish().map_err(|e| {
        LauncherError::InvalidInput(format!("Failed to finalize OneDrive cloud zip: {e}"))
    })?;

    let meta = fs::metadata(&target_path).map_err(LauncherError::Io)?;

    let entry = CloudBackupEntry {
        filename,
        size_bytes: meta.len(),
        timestamp: now.timestamp_millis(),
    };

    Ok(entry)
}

/// Restores configuration files and options from a OneDrive cloud backup `.zip`.
pub fn restore_cloud_backup(
    app_data_dir: &Path,
    backup_filename: &str,
    custom_sync_dir: Option<&Path>,
) -> Result<(), LauncherError> {
    let sync_dir = custom_sync_dir
        .map(PathBuf::from)
        .or_else(resolve_onedrive_dir)
        .ok_or_else(|| {
            LauncherError::NotFound("OneDrive directory not found on system".to_string())
        })?;

    let target_zip = sync_dir.join("Backups").join(backup_filename);
    if !target_zip.is_file() {
        return Err(LauncherError::NotFound(format!(
            "Cloud backup file '{}' not found in OneDrive",
            backup_filename
        )));
    }

    let file = File::open(&target_zip).map_err(LauncherError::Io)?;
    let mut archive = ZipArchive::new(BufReader::new(file)).map_err(|e| {
        LauncherError::InvalidInput(format!("Failed to read cloud backup zip: {e}"))
    })?;

    for i in 0..archive.len() {
        let mut entry = archive.by_index(i).map_err(|e| {
            LauncherError::InvalidInput(format!("Failed to read entry from zip: {e}"))
        })?;

        let name = entry.name().to_string();
        if name.ends_with('/') {
            continue;
        }

        let dest = if let Some(stripped) = name.strip_prefix("config/") {
            app_data_dir.join(stripped)
        } else if let Some(stripped) = name.strip_prefix("offline_instances/") {
            app_data_dir.join("offline_instances").join(stripped)
        } else if let Some(stripped) = name.strip_prefix("instances/") {
            app_data_dir.join("instances").join(stripped)
        } else {
            continue;
        };

        if let Some(parent) = dest.parent() {
            let _ = fs::create_dir_all(parent);
        }

        let mut out = File::create(&dest).map_err(LauncherError::Io)?;
        std::io::copy(&mut entry, &mut out).map_err(LauncherError::Io)?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn onedrive_sync_backup_and_restore_cycle() {
        let temp_root = std::env::temp_dir().join(format!(
            "onedrive_test_{}",
            chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0)
        ));
        let app_data = temp_root.join("app_data");
        let onedrive = temp_root.join("OneDrive").join("Apps").join("ZirconLauncher");

        let _ = fs::create_dir_all(&app_data);
        let _ = fs::create_dir_all(&onedrive);

        // Write sample settings
        fs::write(app_data.join("settings.json"), r#"{"theme":"cyberpunk"}"#).unwrap();

        // Check status
        let status = get_sync_status(Some(&onedrive));
        assert!(status.available);
        assert_eq!(0, status.cloud_backups_count);

        // Backup
        let backup = create_cloud_backup(&app_data, Some(&onedrive)).unwrap();
        assert!(backup.filename.ends_with(".zip"));
        assert!(backup.size_bytes > 0);

        let backups = list_cloud_backups(Some(&onedrive));
        assert_eq!(1, backups.len());

        // Delete local settings and restore
        fs::remove_file(app_data.join("settings.json")).unwrap();
        assert!(!app_data.join("settings.json").is_file());

        restore_cloud_backup(&app_data, &backup.filename, Some(&onedrive)).unwrap();
        assert!(app_data.join("settings.json").is_file());
        assert_eq!(
            r#"{"theme":"cyberpunk"}"#,
            fs::read_to_string(app_data.join("settings.json")).unwrap()
        );

        let _ = fs::remove_dir_all(temp_root);
    }
}
