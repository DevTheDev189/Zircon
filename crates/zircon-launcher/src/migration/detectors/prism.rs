//! Detector and parser for Prism Launcher, PolyMC, and MultiMC instances.

use std::fs;
use std::path::{Path, PathBuf};

use crate::migration::models::{DiscoveredInstance, MigrationScanResult, SourceLauncherType};

/// Returns known default search paths for Prism Launcher instances across platforms.
pub fn default_prism_paths() -> Vec<PathBuf> {
    let mut paths = Vec::new();

    #[cfg(target_os = "windows")]
    {
        if let Ok(appdata) = std::env::var("APPDATA") {
            let base = PathBuf::from(appdata);
            paths.push(base.join("PrismLauncher").join("instances"));
            paths.push(base.join("PolyMC").join("instances"));
            paths.push(base.join("MultiMC").join("instances"));
        }
    }

    #[cfg(target_os = "macos")]
    {
        if let Ok(home) = std::env::var("HOME") {
            let base = PathBuf::from(home).join("Library").join("Application Support");
            paths.push(base.join("PrismLauncher").join("instances"));
            paths.push(base.join("PolyMC").join("instances"));
            paths.push(base.join("MultiMC").join("instances"));
        }
    }

    #[cfg(target_os = "linux")]
    {
        if let Ok(home) = std::env::var("HOME") {
            let base = PathBuf::from(home).join(".local").join("share");
            paths.push(base.join("PrismLauncher").join("instances"));
            paths.push(base.join("PolyMC").join("instances"));
            paths.push(base.join("MultiMC").join("instances"));
        }
    }

    paths
}

/// Scans Prism Launcher installation directories and parses all valid instances.
pub fn scan_prism_instances(custom_path: Option<&Path>) -> MigrationScanResult {
    let search_paths = match custom_path {
        Some(p) => vec![p.to_path_buf()],
        None => default_prism_paths(),
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
                        if let Some(inst) = parse_prism_instance(&inst_dir) {
                            discovered.push(inst);
                        }
                    }
                }
            }
        }
    }

    MigrationScanResult {
        launcher_type: SourceLauncherType::Prism,
        launcher_name: SourceLauncherType::Prism.display_name().to_string(),
        installed: found_dir.is_some(),
        instances_path: found_dir,
        instances: discovered,
    }
}

/// Parses an individual Prism instance folder containing `mmc-pack.json` or `instance.cfg`.
pub fn parse_prism_instance(dir: &Path) -> Option<DiscoveredInstance> {
    let mmc_pack = dir.join("mmc-pack.json");
    let instance_cfg = dir.join("instance.cfg");

    if !mmc_pack.is_file() && !instance_cfg.is_file() {
        return None;
    }

    // Default instance name to directory folder name
    let dir_name = dir.file_name()?.to_string_lossy().to_string();
    let mut display_name = dir_name.clone();
    let mut last_played_ms = None;

    // 1. Read instance.cfg if available
    if instance_cfg.is_file() {
        if let Ok(cfg_str) = fs::read_to_string(&instance_cfg) {
            for line in cfg_str.lines() {
                let trimmed = line.trim();
                if let Some(val) = trimmed.strip_prefix("name=") {
                    if !val.trim().is_empty() {
                        display_name = val.trim().to_string();
                    }
                } else if let Some(val) = trimmed.strip_prefix("lastLaunchTime=") {
                    if let Ok(ms) = val.trim().parse::<i64>() {
                        last_played_ms = Some(ms);
                    }
                }
            }
        }
    }

    // 2. Read mmc-pack.json to resolve MC version and modloader
    let mut mc_version = "1.20.1".to_string();
    let mut loader_type = "vanilla".to_string();
    let mut loader_version = String::new();

    if mmc_pack.is_file() {
        if let Ok(content) = fs::read_to_string(&mmc_pack) {
            if let Ok(json) = serde_json::from_str::<serde_json::Value>(&content) {
                if let Some(components) = json.get("components").and_then(|c| c.as_array()) {
                    for comp in components {
                        let uid = comp.get("uid").and_then(|u| u.as_str()).unwrap_or("");
                        let ver = comp.get("version").and_then(|v| v.as_str()).unwrap_or("");

                        match uid {
                            "net.minecraft" => {
                                if !ver.is_empty() {
                                    mc_version = ver.to_string();
                                }
                            }
                            "net.fabricmc.fabric-loader" => {
                                loader_type = "fabric".to_string();
                                loader_version = ver.to_string();
                            }
                            "net.neoforged" | "net.neoforged.neoforge" => {
                                loader_type = "neoforge".to_string();
                                loader_version = ver.to_string();
                            }
                            "net.minecraftforge" => {
                                loader_type = "forge".to_string();
                                loader_version = ver.to_string();
                            }
                            "org.quiltmc.quilt-loader" => {
                                loader_type = "quilt".to_string();
                                loader_version = ver.to_string();
                            }
                            _ => {}
                        }
                    }
                }
            }
        }
    }

    // 3. Resolve inner game directory: check .minecraft/ vs minecraft/ vs root
    let game_dir = if dir.join(".minecraft").is_dir() {
        dir.join(".minecraft")
    } else if dir.join("minecraft").is_dir() {
        dir.join("minecraft")
    } else {
        dir.to_path_buf()
    };

    // 4. Count mods & saves
    let mod_count = if let Ok(mods) = fs::read_dir(game_dir.join("mods")) {
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

    let world_count = if let Ok(saves) = fs::read_dir(game_dir.join("saves")) {
        saves.flatten().filter(|e| e.path().is_dir()).count()
    } else {
        0
    };

    Some(DiscoveredInstance {
        id: format!("prism-{}", uuid::Uuid::new_v4()),
        source_launcher: SourceLauncherType::Prism,
        name: display_name,
        minecraft_version: mc_version,
        loader_type,
        loader_version,
        source_path: dir.to_path_buf(),
        game_dir,
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
    fn test_parse_prism_instance() {
        let temp = tempdir().unwrap();
        let inst_dir = temp.path().join("TestPrism");
        fs::create_dir_all(inst_dir.join(".minecraft").join("mods")).unwrap();
        fs::write(inst_dir.join(".minecraft").join("mods").join("testmod.jar"), b"PK").unwrap();

        let mmc_pack = r#"{
            "components": [
                { "uid": "net.minecraft", "version": "1.20.4" },
                { "uid": "net.fabricmc.fabric-loader", "version": "0.15.7" }
            ],
            "formatVersion": 1
        }"#;
        fs::write(inst_dir.join("mmc-pack.json"), mmc_pack).unwrap();
        fs::write(inst_dir.join("instance.cfg"), "name=Test Prism Pack\n").unwrap();

        let result = scan_prism_instances(Some(temp.path()));
        assert_eq!(result.instances.len(), 1);
        let inst = &result.instances[0];
        assert_eq!(inst.name, "Test Prism Pack");
        assert_eq!(inst.minecraft_version, "1.20.4");
        assert_eq!(inst.loader_type, "fabric");
        assert_eq!(inst.loader_version, "0.15.7");
        assert_eq!(inst.mod_count, 1);
    }
}

