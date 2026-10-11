//! Detector and parser for Modrinth App ("Theseus") profiles.

use std::fs;
use std::path::{Path, PathBuf};

use crate::migration::models::{DiscoveredInstance, MigrationScanResult, SourceLauncherType};

/// Returns known default search paths for Modrinth App profiles across platforms.
pub fn default_modrinth_paths() -> Vec<PathBuf> {
    let mut paths = Vec::new();

    #[cfg(target_os = "windows")]
    {
        if let Ok(appdata) = std::env::var("APPDATA") {
            let base = PathBuf::from(appdata);
            paths.push(base.join("ModrinthApp").join("profiles"));
            paths.push(base.join("com.modrinth.theseus").join("profiles"));
        }
    }

    #[cfg(target_os = "macos")]
    {
        if let Ok(home) = std::env::var("HOME") {
            let base = PathBuf::from(home).join("Library").join("Application Support");
            paths.push(base.join("ModrinthApp").join("profiles"));
            paths.push(base.join("com.modrinth.theseus").join("profiles"));
        }
    }

    #[cfg(target_os = "linux")]
    {
        if let Ok(home) = std::env::var("HOME") {
            let base = PathBuf::from(home).join(".local").join("share");
            paths.push(base.join("ModrinthApp").join("profiles"));
            paths.push(base.join("com.modrinth.theseus").join("profiles"));
        }
    }

    paths
}

/// Scans Modrinth App installation directories and parses all valid profiles.
pub fn scan_modrinth_instances(custom_path: Option<&Path>) -> MigrationScanResult {
    let search_paths = match custom_path {
        Some(p) => vec![p.to_path_buf()],
        None => default_modrinth_paths(),
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
                    let prof_dir = entry.path();
                    if prof_dir.is_dir() {
                        if let Some(inst) = parse_modrinth_profile(&prof_dir) {
                            discovered.push(inst);
                        }
                    }
                }
            }
        }
    }

    MigrationScanResult {
        launcher_type: SourceLauncherType::Modrinth,
        launcher_name: SourceLauncherType::Modrinth.display_name().to_string(),
        installed: found_dir.is_some(),
        instances_path: found_dir,
        instances: discovered,
    }
}

/// Parses an individual Modrinth profile folder containing `profile.json`.
pub fn parse_modrinth_profile(dir: &Path) -> Option<DiscoveredInstance> {
    let profile_json = dir.join("profile.json");
    if !profile_json.is_file() {
        return None;
    }

    let content = fs::read_to_string(&profile_json).ok()?;
    let json: serde_json::Value = serde_json::from_str(&content).ok()?;

    let dir_name = dir.file_name()?.to_string_lossy().to_string();
    let display_name = json
        .get("name")
        .and_then(|n| n.as_str())
        .filter(|s| !s.trim().is_empty())
        .unwrap_or(&dir_name)
        .to_string();

    let mc_version = json
        .get("game_version")
        .and_then(|v| v.as_str())
        .unwrap_or("1.20.1")
        .to_string();

    let loader_type = json
        .get("loader")
        .and_then(|l| l.as_str())
        .unwrap_or("fabric")
        .to_lowercase();

    let loader_version = json
        .get("loader_version")
        .and_then(|lv| lv.as_str())
        .unwrap_or("")
        .to_string();

    let last_played_ms = json.get("last_played").and_then(|lp| {
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
        id: format!("modrinth-{}", uuid::Uuid::new_v4()),
        source_launcher: SourceLauncherType::Modrinth,
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
    fn test_parse_modrinth_instance() {
        let temp = tempdir().unwrap();
        let inst_dir = temp.path().join("Fabulously Optimized");
        fs::create_dir_all(inst_dir.join("mods")).unwrap();
        fs::write(inst_dir.join("mods").join("sodium.jar"), b"PK").unwrap();
        fs::write(inst_dir.join("mods").join("iris.jar"), b"PK").unwrap();

        let profile_json = r#"{
            "name": "Fabulously Optimized",
            "game_version": "1.21.1",
            "loader": "fabric",
            "loader_version": "0.16.5"
        }"#;
        fs::write(inst_dir.join("profile.json"), profile_json).unwrap();

        let result = scan_modrinth_instances(Some(temp.path()));
        assert_eq!(result.instances.len(), 1);
        let inst = &result.instances[0];
        assert_eq!(inst.name, "Fabulously Optimized");
        assert_eq!(inst.minecraft_version, "1.21.1");
        assert_eq!(inst.loader_type, "fabric");
        assert_eq!(inst.loader_version, "0.16.5");
        assert_eq!(inst.mod_count, 2);
    }
}

