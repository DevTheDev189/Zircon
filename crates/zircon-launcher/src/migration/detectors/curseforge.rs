//! Detector and parser for CurseForge App instances.

use std::fs;
use std::path::{Path, PathBuf};

use crate::migration::models::{DiscoveredInstance, MigrationScanResult, SourceLauncherType};

/// Returns known default search paths for CurseForge instances across platforms.
pub fn default_curseforge_paths() -> Vec<PathBuf> {
    let mut paths = Vec::new();

    #[cfg(target_os = "windows")]
    {
        if let Ok(userprofile) = std::env::var("USERPROFILE") {
            let base = PathBuf::from(userprofile);
            paths.push(base.join("curseforge").join("minecraft").join("Instances"));
            paths.push(base.join("CurseForge").join("minecraft").join("Instances"));
        }
        if let Ok(appdata) = std::env::var("APPDATA") {
            let base = PathBuf::from(appdata);
            paths.push(base.join("CurseForge").join("minecraft").join("Instances"));
        }
    }

    #[cfg(target_os = "macos")]
    {
        if let Ok(home) = std::env::var("HOME") {
            let base = PathBuf::from(home);
            paths.push(base.join("Documents").join("curseforge").join("minecraft").join("Instances"));
            paths.push(base.join("Library").join("Application Support").join("CurseForge").join("minecraft").join("Instances"));
        }
    }

    #[cfg(target_os = "linux")]
    {
        if let Ok(home) = std::env::var("HOME") {
            let base = PathBuf::from(home);
            paths.push(base.join(".curseforge").join("minecraft").join("Instances"));
            paths.push(base.join(".local").join("share").join("CurseForge").join("minecraft").join("Instances"));
        }
    }

    paths
}

/// Scans CurseForge installation directories and parses all valid instances.
pub fn scan_curseforge_instances(custom_path: Option<&Path>) -> MigrationScanResult {
    let search_paths = match custom_path {
        Some(p) => vec![p.to_path_buf()],
        None => default_curseforge_paths(),
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
                        if let Some(inst) = parse_curseforge_instance(&inst_dir) {
                            discovered.push(inst);
                        }
                    }
                }
            }
        }
    }

    MigrationScanResult {
        launcher_type: SourceLauncherType::CurseForge,
        launcher_name: SourceLauncherType::CurseForge.display_name().to_string(),
        installed: found_dir.is_some(),
        instances_path: found_dir,
        instances: discovered,
    }
}

/// Parses an individual CurseForge instance folder containing `minecraftinstance.json`.
pub fn parse_curseforge_instance(dir: &Path) -> Option<DiscoveredInstance> {
    let mc_instance_json = dir.join("minecraftinstance.json");
    if !mc_instance_json.is_file() {
        return None;
    }

    let content = fs::read_to_string(&mc_instance_json).ok()?;
    let json: serde_json::Value = serde_json::from_str(&content).ok()?;

    let dir_name = dir.file_name()?.to_string_lossy().to_string();
    let display_name = json
        .get("name")
        .and_then(|n| n.as_str())
        .filter(|s| !s.trim().is_empty())
        .unwrap_or(&dir_name)
        .to_string();

    let mc_version = json
        .get("gameVersion")
        .and_then(|v| v.as_str())
        .unwrap_or("1.20.1")
        .to_string();

    let (loader_type, loader_version) = if let Some(loader_name) = json
        .get("baseModLoader")
        .and_then(|b| b.get("name"))
        .and_then(|n| n.as_str())
    {
        // loader_name is typically "forge-47.2.0", "fabric-0.16.14", "neoforge-20.4.80"
        let parts: Vec<&str> = loader_name.splitn(2, '-').collect();
        if parts.len() == 2 {
            (parts[0].to_ascii_lowercase(), parts[1].to_string())
        } else {
            (loader_name.to_ascii_lowercase(), String::new())
        }
    } else {
        ("vanilla".to_string(), String::new())
    };

    let last_played_ms = json.get("lastPlayed").and_then(|lp| {
        if let Some(num) = lp.as_i64() {
            Some(num)
        } else if let Some(iso) = lp.as_str() {
            chrono::DateTime::parse_from_rfc3339(iso)
                .ok()
                .map(|dt| dt.timestamp_millis())
        } else {
            None
        }
    });

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
        id: format!("curseforge-{}", uuid::Uuid::new_v4()),
        source_launcher: SourceLauncherType::CurseForge,
        name: display_name,
        minecraft_version: mc_version,
        loader_type,
        loader_version,
        source_path: dir.to_path_buf(),
        game_dir: dir.to_path_buf(),
        mod_count,
        world_count,
        last_played_ms,
        icon_base64: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_parse_curseforge_instance() {
        let temp = tempdir().unwrap();
        let inst_dir = temp.path().join("All the Mods 9");
        fs::create_dir_all(inst_dir.join("mods")).unwrap();
        fs::write(inst_dir.join("mods").join("jei.jar"), b"PK").unwrap();

        let manifest = r#"{
            "name": "All the Mods 9",
            "gameVersion": "1.20.1",
            "baseModLoader": {
                "name": "forge-47.2.20"
            }
        }"#;
        fs::write(inst_dir.join("minecraftinstance.json"), manifest).unwrap();

        let result = scan_curseforge_instances(Some(temp.path()));
        assert_eq!(result.instances.len(), 1);
        let inst = &result.instances[0];
        assert_eq!(inst.name, "All the Mods 9");
        assert_eq!(inst.minecraft_version, "1.20.1");
        assert_eq!(inst.loader_type, "forge");
        assert_eq!(inst.loader_version, "47.2.20");
        assert_eq!(inst.mod_count, 1);
    }
}

