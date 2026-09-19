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
use std::time::SystemTime;

use serde::{Deserialize, Serialize};
use sha1::{Digest, Sha1};

use super::{diff_boms, ExportError, ModDiffReport};
use crate::model::bom::{BillOfMaterials, ModEntry};

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
) -> Result<SnapshotInfo, ExportError> {
    let dir = snapshots_dir(instance_dir);
    std::fs::create_dir_all(&dir)?;

    let timestamp = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0);

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
pub fn list_snapshots(instance_dir: &Path) -> Result<Vec<SnapshotInfo>, ExportError> {
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
            // Parse filename: `<timestamp>_<label>.json`
            let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or_default();
            let (ts_str, label) = stem.split_once('_').unwrap_or((stem, "Snapshot"));
            let created_at = ts_str.parse::<i64>().unwrap_or(0);

            if let Ok(content) = std::fs::read_to_string(&path) {
                if let Ok(bom) = serde_json::from_str::<BillOfMaterials>(&content) {
                    let loader_str = bom
                        .mod_loader
                        .as_ref()
                        .map(|l| format!("{} {}", l.r#type, l.version))
                        .unwrap_or_else(|| "Vanilla".to_string());

                    snapshots.push(SnapshotInfo {
                        filename,
                        label: label.replace('_', " "),
                        created_at,
                        mod_count: bom.mods.len(),
                        minecraft_version: bom.minecraft_version,
                        mod_loader: loader_str,
                    });
                }
            }
        }
    }

    // Sort newest first
    snapshots.sort_by(|a, b| b.created_at.cmp(&a.created_at));
    Ok(snapshots)
}

/// Loads a snapshot's [`BillOfMaterials`] from disk.
pub fn load_snapshot(instance_dir: &Path, filename: &str) -> Result<BillOfMaterials, ExportError> {
    let file_path = snapshots_dir(instance_dir).join(filename);
    if !file_path.is_file() {
        return Err(ExportError::InvalidFormat(format!(
            "Snapshot file not found: {filename}"
        )));
    }
    let content = std::fs::read_to_string(&file_path)?;
    let bom = serde_json::from_str::<BillOfMaterials>(&content)?;
    Ok(bom)
}

/// Deletes a saved snapshot.
pub fn delete_snapshot(instance_dir: &Path, filename: &str) -> Result<bool, ExportError> {
    let file_path = snapshots_dir(instance_dir).join(filename);
    if file_path.is_file() {
        std::fs::remove_file(file_path)?;
        Ok(true)
    } else {
        Ok(false)
    }
}

/// Restores an instance's state to a snapshot, producing a diff against current BOM.
pub fn restore_snapshot(
    instance_dir: &Path,
    filename: &str,
    current_bom: &BillOfMaterials,
) -> Result<SnapshotRestoreResult, ExportError> {
    let target_bom = load_snapshot(instance_dir, filename)?;
    let diff = diff_boms(current_bom, &target_bom);

    let stem = Path::new(filename)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or_default();
    let (ts_str, label) = stem.split_once('_').unwrap_or((stem, "Snapshot"));
    let created_at = ts_str.parse::<i64>().unwrap_or(0);

    let loader_str = target_bom
        .mod_loader
        .as_ref()
        .map(|l| format!("{} {}", l.r#type, l.version))
        .unwrap_or_else(|| "Vanilla".to_string());

    let info = SnapshotInfo {
        filename: filename.to_string(),
        label: label.replace('_', " "),
        created_at,
        mod_count: target_bom.mods.len(),
        minecraft_version: target_bom.minecraft_version,
        mod_loader: loader_str,
    };

    Ok(SnapshotRestoreResult {
        restored_snapshot: info,
        diff,
    })
}

/// Audits physical `.jar` files in `<instance_dir>/mods` against the declared [`BillOfMaterials`].
pub fn audit_instance_mods(
    instance_dir: &Path,
    bom: &BillOfMaterials,
) -> Result<ModAuditReport, ExportError> {
    let mods_dir = instance_dir.join("mods");
    let mut missing_mods = Vec::new();
    let mut corrupted_mods = Vec::new();
    let mut verified_count = 0;

    let mut expected_filenames = HashSet::new();

    for mod_entry in &bom.mods {
        expected_filenames.insert(mod_entry.filename.clone());
        let jar_path = mods_dir.join(&mod_entry.filename);

        if !jar_path.is_file() {
            missing_mods.push(mod_entry.clone());
            continue;
        }

        if let Some(expected_sha1) = &mod_entry.sha1 {
            match compute_file_sha1(&jar_path) {
                Ok(actual_sha1) => {
                    if !actual_sha1.eq_ignore_ascii_case(expected_sha1) {
                        corrupted_mods.push(mod_entry.clone());
                    } else {
                        verified_count += 1;
                    }
                }
                Err(_) => {
                    corrupted_mods.push(mod_entry.clone());
                }
            }
        } else {
            // No SHA1 hash declared; file is present on disk
            verified_count += 1;
        }
    }

    let mut unmanaged_files = Vec::new();
    if mods_dir.is_dir() {
        if let Ok(entries) = std::fs::read_dir(&mods_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_file() && path.extension().and_then(|e| e.to_str()) == Some("jar") {
                    let name = entry.file_name().to_string_lossy().to_string();
                    if !expected_filenames.contains(&name) {
                        unmanaged_files.push(name);
                    }
                }
            }
        }
    }

    Ok(ModAuditReport {
        missing_mods,
        corrupted_mods,
        unmanaged_files,
        verified_count,
        total_expected: bom.mods.len(),
    })
}

fn compute_file_sha1(path: &Path) -> Result<String, std::io::Error> {
    let mut file = File::open(path)?;
    let mut hasher = Sha1::new();
    let mut buffer = [0u8; 16384];

    loop {
        let count = file.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        hasher.update(&buffer[..count]);
    }

    Ok(hex::encode(hasher.finalize()))
}
