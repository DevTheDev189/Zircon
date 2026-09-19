//! In-app mod list snapshotting ("Time Machine"), restoration, and audit engine.
//!
//! Stores lightweight (~10–30 KB) cryptographic [`BillOfMaterials`] snapshots under
//! `<instance_dir>/snapshots/<timestamp>_<slug>.json`. Provides instant 1-click rollback,
//! diffing against active configurations, and auditing on-disk `.jar` files against
//! expected BOM hashes.

use std::collections::HashSet;
use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};

use chrono::Utc;
use serde::{Deserialize, Serialize};
use sha1::{Digest, Sha1};
use zircon_core::export::diff_boms;
use zircon_core::export::ModDiffReport;
use zircon_core::model::{BillOfMaterials, ModEntry};

use crate::error::LauncherError;

/// Lightweight descriptor of a saved snapshot for UI display.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SnapshotInfo {
    pub filename: String,
    pub label: String,
    pub created_at: i64,
    pub mod_count: usize,
    pub minecraft_version: String,
    pub mod_loader: String,
}

/// Result of rolling back / restoring an instance to a snapshot.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SnapshotRestoreResult {
    pub restored_snapshot: SnapshotInfo,
    pub diff: ModDiffReport,
}

/// Result of auditing on-disk files in `mods/` against active `bom.json`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
pub struct ModAuditReport {
    pub missing_mods: Vec<ModEntry>,
    pub corrupted_mods: Vec<ModEntry>,
    pub unmanaged_files: Vec<String>,
    pub verified_count: usize,
    pub total_expected: usize,
}

impl ModAuditReport {
    pub fn is_healthy(&self) -> bool {
        self.missing_mods.is_empty() && self.corrupted_mods.is_empty()
    }
}

/// Snapshot storage directory for an instance (`<instance_dir>/snapshots`).
pub fn snapshots_dir(instance_dir: &Path) -> PathBuf {
    instance_dir.join("snapshots")
}

/// Creates a new named snapshot from the active BOM.
pub fn create_snapshot(
    instance_dir: &Path,
    label: &str,
    bom: &BillOfMaterials,
) -> Result<SnapshotInfo, LauncherError> {
    let dir = snapshots_dir(instance_dir);
    std::fs::create_dir_all(&dir)?;

    let timestamp = Utc::now().timestamp_millis();
    let sanitized_label: String = label
        .chars()
        .map(|c| if c.is_alphanumeric() || c == '-' || c == '_' { c } else { '_' })
        .collect();
    let filename = format!("{timestamp}_{sanitized_label}.json");
    let file_path = dir.join(&filename);

    let json = serde_json::to_string_pretty(bom)?;
    std::fs::write(&file_path, json)?;

    let loader_str = bom
        .mod_loader
        .as_ref()
        .map(|l| format!("{} {}", l.r#type, l.version))
        .unwrap_or_else(|| "Vanilla".to_string());

    Ok(SnapshotInfo {
        filename,
        label: label.to_string(),
        created_at: timestamp,
        mod_count: bom.mods.len(),
        minecraft_version: bom.minecraft_version.clone(),
        mod_loader: loader_str,
    })
}

/// Lists all saved snapshots for an instance, sorted newest first.
pub fn list_snapshots(instance_dir: &Path) -> Result<Vec<SnapshotInfo>, LauncherError> {
    let dir = snapshots_dir(instance_dir);
    if !dir.is_dir() {
        return Ok(Vec::new());
    }

    let mut snapshots = Vec::new();
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_file() && path.extension().and_then(|e| e.to_str()) == Some("json") {
            let filename = entry.file_name().to_string_lossy().to_string();
            if let Ok(content) = std::fs::read_to_string(&path) {
                if let Ok(bom) = serde_json::from_str::<BillOfMaterials>(&content) {
                    // Extract timestamp and label from filename pattern `<timestamp>_<label>.json`
                    let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
                    let (ts, label) = if let Some((ts_str, lbl)) = stem.split_once('_') {
                        let parsed_ts = ts_str.parse::<i64>().unwrap_or(0);
                        (parsed_ts, lbl.replace('_', " "))
                    } else {
                        (0, stem.to_string())
                    };

                    let loader_str = bom
                        .mod_loader
                        .as_ref()
                        .map(|l| format!("{} {}", l.r#type, l.version))
                        .unwrap_or_else(|| "Vanilla".to_string());

                    snapshots.push(SnapshotInfo {
                        filename,
                        label,
                        created_at: ts,
                        mod_count: bom.mods.len(),
                        minecraft_version: bom.minecraft_version,
                        mod_loader: loader_str,
                    });
                }
            }
        }
    }

    snapshots.sort_by(|a, b| b.created_at.cmp(&a.created_at));
    Ok(snapshots)
}

/// Loads a specific snapshot from disk.
pub fn load_snapshot(instance_dir: &Path, filename: &str) -> Result<BillOfMaterials, LauncherError> {
    let file_path = snapshots_dir(instance_dir).join(filename);
    if !file_path.is_file() {
        return Err(LauncherError::NotFound(format!("Snapshot not found: {filename}")));
    }
    let content = std::fs::read_to_string(file_path)?;
    let bom: BillOfMaterials = serde_json::from_str(&content)?;
    Ok(bom)
}

/// Deletes a snapshot from disk.
pub fn delete_snapshot(instance_dir: &Path, filename: &str) -> Result<(), LauncherError> {
    let file_path = snapshots_dir(instance_dir).join(filename);
    if file_path.is_file() {
        std::fs::remove_file(file_path)?;
    }
    Ok(())
}

/// Restores an instance's `bom.json` from a snapshot.
pub fn restore_snapshot(
    instance_dir: &Path,
    filename: &str,
) -> Result<SnapshotRestoreResult, LauncherError> {
    let target_bom = load_snapshot(instance_dir, filename)?;
    let active_bom_path = instance_dir.join("bom.json");

    let current_bom = if active_bom_path.is_file() {
        let text = std::fs::read_to_string(&active_bom_path)?;
        serde_json::from_str::<BillOfMaterials>(&text).unwrap_or_default()
    } else {
        BillOfMaterials::default()
    };

    let diff = diff_boms(&current_bom, &target_bom);

    // Save automatic backup of pre-restore state if current has mods
    if !current_bom.mods.is_empty() {
        let _ = create_snapshot(instance_dir, "Auto-Backup Pre-Restore", &current_bom);
    }

    // Write restored BOM to active bom.json
    let json = serde_json::to_string_pretty(&target_bom)?;
    std::fs::write(&active_bom_path, json)?;

    let stem = filename.strip_suffix(".json").unwrap_or(filename);
    let (ts, label) = if let Some((ts_str, lbl)) = stem.split_once('_') {
        let parsed_ts = ts_str.parse::<i64>().unwrap_or(0);
        (parsed_ts, lbl.replace('_', " "))
    } else {
        (0, stem.to_string())
    };

    let loader_str = target_bom
        .mod_loader
        .as_ref()
        .map(|l| format!("{} {}", l.r#type, l.version))
        .unwrap_or_else(|| "Vanilla".to_string());

    let info = SnapshotInfo {
        filename: filename.to_string(),
        label,
        created_at: ts,
        mod_count: target_bom.mods.len(),
        minecraft_version: target_bom.minecraft_version,
        mod_loader: loader_str,
    };

    Ok(SnapshotRestoreResult {
        restored_snapshot: info,
        diff,
    })
}

/// Audits physical `.jar` files in `mods/` against expected active `bom.json`.
pub fn audit_instance_mods(
    instance_dir: &Path,
    bom: &BillOfMaterials,
) -> Result<ModAuditReport, LauncherError> {
    let mods_dir = instance_dir.join("mods");
    let mut report = ModAuditReport {
        total_expected: bom.mods.len(),
        ..Default::default()
    };

    let mut on_disk_files = HashSet::new();
    if mods_dir.is_dir() {
        for entry in std::fs::read_dir(&mods_dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_file() {
                let name = entry.file_name().to_string_lossy().to_string();
                if name.ends_with(".jar") || name.ends_with(".jar.disabled") {
                    on_disk_files.insert(name);
                }
            }
        }
    }

    let mut expected_filenames = HashSet::new();

    for expected in &bom.mods {
        let target_name = &expected.filename;
        let disabled_name = format!("{target_name}.disabled");
        expected_filenames.insert(target_name.clone());
        expected_filenames.insert(disabled_name.clone());

        let target_file = if mods_dir.join(target_name).is_file() {
            Some(mods_dir.join(target_name))
        } else if mods_dir.join(&disabled_name).is_file() {
            Some(mods_dir.join(&disabled_name))
        } else {
            None
        };

        match target_file {
            None => {
                report.missing_mods.push(expected.clone());
            }
            Some(path) => {
                // If expected mod has sha1, verify cryptographic hash
                if let Some(expected_sha1) = &expected.sha1 {
                    if let Ok(mut f) = File::open(&path) {
                        let mut hasher = Sha1::new();
                        let mut buffer = [0u8; 8192];
                        let mut ok = true;
                        while let Ok(n) = f.read(&mut buffer) {
                            if n == 0 {
                                break;
                            }
                            hasher.update(&buffer[..n]);
                        }
                        let actual_sha1 = hex::encode(hasher.finalize());
                        if !actual_sha1.eq_ignore_ascii_case(expected_sha1) {
                            report.corrupted_mods.push(expected.clone());
                            ok = false;
                        }
                        if ok {
                            report.verified_count += 1;
                        }
                    } else {
                        report.corrupted_mods.push(expected.clone());
                    }
                } else {
                    report.verified_count += 1;
                }
            }
        }
    }

    // Check for unmanaged/extra files in mods/
    for file in on_disk_files {
        if !expected_filenames.contains(&file) {
            report.unmanaged_files.push(file);
        }
    }

    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;
    use zircon_core::model::{ModEntry, ModLoaderInfo};

    #[test]
    fn test_snapshot_lifecycle() {
        let tmp = tempfile::tempdir().unwrap();
        let instance_dir = tmp.path();

        let mut bom = BillOfMaterials::new(
            "1.20.4",
            Some(ModLoaderInfo::new("fabric", "0.15.11", None)),
            Some("Test Instance".to_string()),
        );
        bom.mods.push(ModEntry::new(
            Some("sodium".to_string()),
            "sodium-0.5.8.jar",
            Some("abc".to_string()),
            123,
            Some("modrinth".to_string()),
            None,
            100,
        ));

        // Create
        let snap = create_snapshot(instance_dir, "Initial Baseline", &bom).unwrap();
        assert_eq!("Initial Baseline", snap.label);
        assert_eq!(1, snap.mod_count);

        // List
        let list = list_snapshots(instance_dir).unwrap();
        assert_eq!(1, list.len());
        assert_eq!(snap.filename, list[0].filename);

        // Load
        let loaded = load_snapshot(instance_dir, &snap.filename).unwrap();
        assert_eq!(bom.minecraft_version, loaded.minecraft_version);
        assert_eq!(1, loaded.mods.len());

        // Restore
        let restore = restore_snapshot(instance_dir, &snap.filename).unwrap();
        assert_eq!(snap.filename, restore.restored_snapshot.filename);
        assert!(instance_dir.join("bom.json").is_file());

        // Delete
        delete_snapshot(instance_dir, &snap.filename).unwrap();
        let list_after = list_snapshots(instance_dir).unwrap();
        assert!(list_after.is_empty());
    }
}
