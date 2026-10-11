//! Detector and parser for the Official (Vanilla) Minecraft Launcher profiles.

use std::fs;
use std::path::{Path, PathBuf};

use crate::migration::models::{DiscoveredInstance, MigrationScanResult, SourceLauncherType};

/// Returns known default search paths for the official Minecraft launcher across platforms.
pub fn default_vanilla_paths() -> Vec<PathBuf> {
    let mut paths = Vec::new();

    #[cfg(target_os = "windows")]
    {
        if let Ok(appdata) = std::env::var("APPDATA") {
            paths.push(PathBuf::from(appdata).join(".minecraft"));
        }
    }

    #[cfg(target_os = "macos")]
    {
        if let Ok(home) = std::env::var("HOME") {
            paths.push(PathBuf::from(home).join("Library").join("Application Support").join("minecraft"));
        }
    }

    #[cfg(target_os = "linux")]
    {
        if let Ok(home) = std::env::var("HOME") {
            paths.push(PathBuf::from(home).join(".minecraft"));
        }
    }

    paths
}

/// Parses the loader and game version from a Minecraft version ID string.
/// E.g. "fabric-loader-0.16.14-1.21.1", "1.20.4-forge-49.0.3", "1.21.4"
fn parse_version_id(version_id: &str) -> (String, String, String) {
    let lower = version_id.to_lowercase();
    if lower == "latest-release" {
        ("1.21.4".to_string(), "vanilla".to_string(), String::new())
    } else if lower == "latest-snapshot" {
        ("Snapshot".to_string(), "vanilla".to_string(), String::new())
    } else if lower.contains("fabric") {
        let parts: Vec<&str> = version_id.split('-').collect();
        let mc = parts.last().unwrap_or(&"1.20.1").to_string();
        let loader_ver = if parts.len() >= 3 {
            parts[2].to_string()
        } else {
            String::new()
        };
        (mc, "fabric".to_string(), loader_ver)
    } else if lower.contains("neoforge") {
        let parts: Vec<&str> = version_id.split('-').collect();
        let mc = parts.first().unwrap_or(&"1.20.4").to_string();
        let loader_ver = parts.last().unwrap_or(&"").to_string();
        (mc, "neoforge".to_string(), loader_ver)
    } else if lower.contains("forge") {
        let parts: Vec<&str> = version_id.split('-').collect();
        let mc = parts.first().unwrap_or(&"1.20.1").to_string();
        let loader_ver = parts.last().unwrap_or(&"").to_string();
        (mc, "forge".to_string(), loader_ver)
    } else if lower.contains("quilt") {
        let parts: Vec<&str> = version_id.split('-').collect();
        let mc = parts.last().unwrap_or(&"1.20.1").to_string();
        (mc, "quilt".to_string(), String::new())
    } else {
        (version_id.to_string(), "vanilla".to_string(), String::new())
    }
}

/// Scans official Minecraft launcher directory and parses configured profiles.
pub fn scan_vanilla_instances(custom_path: Option<&Path>) -> MigrationScanResult {
    let search_paths = match custom_path {
        Some(p) => vec![p.to_path_buf()],
        None => default_vanilla_paths(),
    };

    let mut found_dir = None;
    let mut discovered = Vec::new();

    for dot_minecraft in search_paths {
        let profiles_json_path = dot_minecraft.join("launcher_profiles.json");
        if profiles_json_path.is_file() {
            if found_dir.is_none() {
                found_dir = Some(dot_minecraft.clone());
            }

            if let Ok(content) = fs::read_to_string(&profiles_json_path) {
                if let Ok(json) = serde_json::from_str::<serde_json::Value>(&content) {
                    if let Some(profiles) = json.get("profiles").and_then(|p| p.as_object()) {
                        for (key, prof) in profiles {
                            let name = prof
                                .get("name")
                                .and_then(|n| n.as_str())
                                .map(|s| s.trim())
                                .filter(|s| !s.is_empty())
                                .map(|s| s.to_string())
                                .unwrap_or_else(|| match key.as_str() {
                                    "latest-release" => "Latest Release".to_string(),
                                    "latest-snapshot" => "Latest Snapshot".to_string(),
                                    other => other.to_string(),
                                });

                            let version_id = prof
                                .get("lastVersionId")
                                .and_then(|v| v.as_str())
                                .unwrap_or("1.21.4");

                            let (mc_version, loader_type, loader_version) = parse_version_id(version_id);

                            let game_dir = if let Some(gd) = prof.get("gameDir").and_then(|g| g.as_str()) {
                                PathBuf::from(gd)
                            } else {
                                dot_minecraft.clone()
                            };

                            let last_played_ms = prof.get("lastUsed").and_then(|lu| {
                                if let Some(iso) = lu.as_str() {
                                    chrono::DateTime::parse_from_rfc3339(iso)
                                        .ok()
                                        .map(|dt| dt.timestamp_millis())
                                } else {
                                    None
                                }
                            });

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

                            // Only discover profiles that exist on disk or have custom content
                            if game_dir.is_dir() {
                                discovered.push(DiscoveredInstance {
                                    id: format!("vanilla-{}", uuid::Uuid::new_v4()),
                                    source_launcher: SourceLauncherType::Vanilla,
                                    name: name.to_string(),
                                    minecraft_version: mc_version,
                                    loader_type,
                                    loader_version,
                                    source_path: game_dir.clone(),
                                    game_dir,
                                    mod_count,
                                    world_count,
                                    last_played_ms,
                                    icon_base64: None,
                                });
                            }
                        }
                    }
                }
            }
        }
    }

    MigrationScanResult {
        launcher_type: SourceLauncherType::Vanilla,
        launcher_name: SourceLauncherType::Vanilla.display_name().to_string(),
        installed: found_dir.is_some(),
        instances_path: found_dir,
        instances: discovered,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_parse_vanilla_profiles() {
        let temp = tempdir().unwrap();
        let dot_mc = temp.path().join(".minecraft");
        fs::create_dir_all(&dot_mc).unwrap();

        let custom_game_dir = temp.path().join("custom_profile");
        fs::create_dir_all(custom_game_dir.join("mods")).unwrap();
        fs::write(custom_game_dir.join("mods").join("fabric-api.jar"), b"PK").unwrap();

        let profiles_json = serde_json::json!({
            "profiles": {
                "test-custom": {
                    "name": "Custom Profile",
                    "lastVersionId": "fabric-loader-0.15.7-1.20.4",
                    "gameDir": custom_game_dir.to_str().unwrap()
                }
            }
        });
        fs::write(dot_mc.join("launcher_profiles.json"), profiles_json.to_string()).unwrap();

        let result = scan_vanilla_instances(Some(&dot_mc));
        assert_eq!(result.instances.len(), 1);
        let inst = &result.instances[0];
        assert_eq!(inst.name, "Custom Profile");
        assert_eq!(inst.minecraft_version, "1.20.4");
        assert_eq!(inst.loader_type, "fabric");
        assert_eq!(inst.loader_version, "0.15.7");
        assert_eq!(inst.mod_count, 1);
    }
}

