//! Detector and parser for ATLauncher instances.

use std::fs;
use std::path::{Path, PathBuf};

use crate::migration::models::{DiscoveredInstance, MigrationScanResult, SourceLauncherType};

/// Returns known default search paths for ATLauncher instances across platforms.
pub fn default_atlauncher_paths() -> Vec<PathBuf> {
    let mut paths = Vec::new();

    #[cfg(target_os = "windows")]
    {
        if let Ok(appdata) = std::env::var("APPDATA") {
            paths.push(PathBuf::from(appdata).join("ATLauncher").join("instances"));
        }
    }

    #[cfg(target_os = "macos")]
    {
        if let Ok(home) = std::env::var("HOME") {
            paths.push(PathBuf::from(home).join("Library").join("Application Support").join("ATLauncher").join("instances"));
        }
    }

    #[cfg(target_os = "linux")]
    {
        if let Ok(home) = std::env::var("HOME") {
            paths.push(PathBuf::from(home).join(".local").join("share").join("ATLauncher").join("instances"));
        }
    }

    paths
}

/// Scans ATLauncher installation directories and parses all valid instances.
pub fn scan_atlauncher_instances(custom_path: Option<&Path>) -> MigrationScanResult {
    let search_paths = match custom_path {
        Some(p) => vec![p.to_path_buf()],
        None => default_atlauncher_paths(),
    };

    let mut found_dir = None;
    let mut discovered = Vec::new();

    for dir in search_paths {
        if dir.is_dir() {
            if found_dir.is_none() {
                found_dir = Some(dir.clone());
            }
            if let Ok(entries) = fs::read_dir(&dir) {
                for entry in entries.flatten() {
                    let inst_dir = entry.path();
                    if inst_dir.is_dir() {
                        if let Some(inst) = parse_atlauncher_instance(&inst_dir) {
                            discovered.push(inst);
                        }
                    }
                }
            }
        }
    }

    MigrationScanResult {
        launcher_type: SourceLauncherType::AtLauncher,
        launcher_name: SourceLauncherType::AtLauncher.display_name().to_string(),
        installed: found_dir.is_some(),
        instances_path: found_dir,
        instances: discovered,
    }
}

/// Parses an individual ATLauncher instance folder containing `instance.json`.
pub fn parse_atlauncher_instance(dir: &Path) -> Option<DiscoveredInstance> {
    let instance_json = dir.join("instance.json");
    if !instance_json.is_file() {
        return None;
    }

    let content = fs::read_to_string(&instance_json).ok()?;
    let json: serde_json::Value = serde_json::from_str(&content).ok()?;

    let dir_name = dir.file_name()?.to_string_lossy().to_string();
    let display_name = json
        .get("name")
        .and_then(|n| n.as_str())
        .filter(|s| !s.trim().is_empty())
        .unwrap_or(&dir_name)
        .to_string();

    let mc_version = json
        .get("version")
        .or_else(|| json.get("minecraftVersion"))
        .or_else(|| json.get("minecraft"))
        .and_then(|v| v.as_str())
        .unwrap_or("1.20.1")
        .to_string();

    let loader_type = json
        .get("loader")
        .and_then(|l| l.as_str())
        .unwrap_or("fabric")
        .to_lowercase();

    let loader_version = json
        .get("loaderVersion")
        .and_then(|lv| lv.as_str())
        .unwrap_or("")
        .to_string();

    let mod_count = if let Ok(mods) = fs::read_dir(dir.join("mods")) {
        mods.flatten()
            .filter(|e| {
                e.path()
                    .extension()
                    .map(|ext| ext.eq_ignore_ascii_case("jar"))
                    .unwrap_or(false)
            })
            .count()
    } else {
        0
    };

    let world_count = if let Ok(saves) = fs::read_dir(dir.join("saves")) {
        saves.flatten().filter(|e| e.path().is_dir()).count()
    } else {
        0
    };

    Some(DiscoveredInstance {
        id: format!("atlauncher-{}", uuid::Uuid::new_v4()),
        source_launcher: SourceLauncherType::AtLauncher,
        name: display_name,
        minecraft_version: mc_version,
        loader_type,
        loader_version,
        source_path: dir.to_path_buf(),
        game_dir: dir.to_path_buf(),
        mod_count,
        world_count,
        last_played_ms: None,
        icon_base64: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_parse_atlauncher_instance() {
        let temp = tempdir().unwrap();
        let inst_dir = temp.path().join("SkyFactory 4");
        fs::create_dir_all(inst_dir.join("mods")).unwrap();
        fs::write(inst_dir.join("mods").join("fastcraft.jar"), b"PK").unwrap();

        let instance_json = r#"{
            "name": "SkyFactory 4",
            "minecraftVersion": "1.12.2",
            "loader": "Forge",
            "loaderVersion": "14.23.5.2860"
        }"#;
        fs::write(inst_dir.join("instance.json"), instance_json).unwrap();

        let result = scan_atlauncher_instances(Some(temp.path()));
        assert_eq!(result.instances.len(), 1);
        let inst = &result.instances[0];
        assert_eq!(inst.name, "SkyFactory 4");
        assert_eq!(inst.minecraft_version, "1.12.2");
        assert_eq!(inst.loader_type, "forge");
        assert_eq!(inst.loader_version, "14.23.5.2860");
        assert_eq!(inst.mod_count, 1);
    }
}

