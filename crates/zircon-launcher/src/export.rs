//! Export engine: packages an offline instance into a standard `.mrpack` archive
//! or a complete, ready-to-run Zircon Dedicated Server ZIP package.

use std::fs::File;
use std::io::Write;
use std::path::Path;

use serde::{Deserialize, Serialize};
use sha1::{Digest, Sha1};
use zip::write::SimpleFileOptions;
use zip::ZipWriter;

use zircon_core::model::{BillOfMaterials, ModEntry, ModSide};

use crate::error::LauncherError;
use crate::offline::OfflineInstance;

/// Standard Modrinth modpack index structure.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModrinthIndex {
    pub format_version: u32,
    pub game: String,
    pub version_id: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
    pub dependencies: std::collections::HashMap<String, String>,
    pub files: Vec<ModrinthIndexFile>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModrinthIndexFile {
    pub path: String,
    pub hashes: std::collections::HashMap<String, String>,
    pub env: ModrinthIndexEnv,
    pub downloads: Vec<String>,
    pub file_size: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModrinthIndexEnv {
    pub client: String,
    pub server: String,
}

/// Known client-only mods that should not be bundled into dedicated server packages.
const CLIENT_ONLY_MODS: &[&str] = &[
    "sodium",
    "iris",
    "indium",
    "lambdynamiclights",
    "entityculling",
    "immediatelyfast",
    "modmenu",
    "cloth-config",
    "appleskin",
    "dynamic-fps",
    "zoomify",
    "controlling",
    "reeses-sodium-options",
    "sodium-extra",
];

fn is_client_only(filename: &str) -> bool {
    let lower = filename.to_ascii_lowercase();
    CLIENT_ONLY_MODS.iter().any(|&m| lower.contains(m))
}

/// Recursively copies a directory tree into a ZipWriter under the given prefix.
fn add_dir_to_zip(
    src_dir: &Path,
    prefix: &str,
    zip: &mut ZipWriter<File>,
    options: SimpleFileOptions,
) -> Result<(), LauncherError> {
    if !src_dir.is_dir() {
        return Ok(());
    }

    for entry in std::fs::read_dir(src_dir)? {
        let entry = entry?;
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();
        let entry_rel = format!("{prefix}/{name}");

        if path.is_dir() {
            zip.add_directory(&entry_rel, options)
                .map_err(|e| LauncherError::Io(std::io::Error::new(std::io::ErrorKind::Other, e)))?;
            add_dir_to_zip(&path, &entry_rel, zip, options)?;
        } else if path.is_file() {
            zip.start_file(&entry_rel, options)
                .map_err(|e| LauncherError::Io(std::io::Error::new(std::io::ErrorKind::Other, e)))?;
            let mut f = File::open(&path)?;
            std::io::copy(&mut f, zip)?;
        }
    }
    Ok(())
}

/// Exports an offline instance as a `.mrpack` archive.
pub fn export_instance_mrpack(
    game_dir: &Path,
    instance: &OfflineInstance,
    out_path: &Path,
) -> Result<(), LauncherError> {
    if let Some(parent) = out_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let file = File::create(out_path)?;
    let mut zip = ZipWriter::new(file);
    let options = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);

    // Build modrinth.index.json
    let mut dependencies = std::collections::HashMap::new();
    dependencies.insert("minecraft".to_string(), instance.minecraft_version.clone());
    if !instance.mod_loader.r#type.is_empty() && instance.mod_loader.r#type != "vanilla" {
        dependencies.insert(
            instance.mod_loader.r#type.clone(),
            instance.mod_loader.version.clone(),
        );
    }

    let index = ModrinthIndex {
        format_version: 1,
        game: "minecraft".to_string(),
        version_id: instance.id.clone(),
        name: instance.name.clone(),
        summary: Some(format!("Exported from Zircon Launcher for MC {}", instance.minecraft_version)),
        dependencies,
        files: Vec::new(),
    };

    let index_json = serde_json::to_string_pretty(&index)?;
    zip.start_file("modrinth.index.json", options)
        .map_err(|e| LauncherError::Io(std::io::Error::new(std::io::ErrorKind::Other, e)))?;
    zip.write_all(index_json.as_bytes())?;

    // Add overrides/mods/
    let mods_dir = game_dir.join("mods");
    if mods_dir.is_dir() {
        zip.add_directory("overrides/mods", options)
            .map_err(|e| LauncherError::Io(std::io::Error::new(std::io::ErrorKind::Other, e)))?;
        add_dir_to_zip(&mods_dir, "overrides/mods", &mut zip, options)?;
    }

    // Add overrides/config/
    let config_dir = game_dir.join("config");
    if config_dir.is_dir() {
        zip.add_directory("overrides/config", options)
            .map_err(|e| LauncherError::Io(std::io::Error::new(std::io::ErrorKind::Other, e)))?;
        add_dir_to_zip(&config_dir, "overrides/config", &mut zip, options)?;
    }

    // Add overrides/options.txt if present
    let options_txt = game_dir.join("options.txt");
    if options_txt.is_file() {
        zip.start_file("overrides/options.txt", options)
            .map_err(|e| LauncherError::Io(std::io::Error::new(std::io::ErrorKind::Other, e)))?;
        let mut f = File::open(&options_txt)?;
        std::io::copy(&mut f, &mut zip)?;
    }

    zip.finish()
        .map_err(|e| LauncherError::Io(std::io::Error::new(std::io::ErrorKind::Other, e)))?;

    Ok(())
}

/// Packages an offline instance into a complete dedicated server ZIP package.
pub fn export_to_zircon_server(
    game_dir: &Path,
    instance: &OfflineInstance,
    world_folder: Option<&str>,
    out_path: &Path,
) -> Result<(), LauncherError> {
    if let Some(parent) = out_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let file = File::create(out_path)?;
    let mut zip = ZipWriter::new(file);
    let options = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);

    // 1. Pack world into `world/`
    let saves_dir = game_dir.join("saves");
    let target_world = if let Some(w) = world_folder {
        saves_dir.join(w)
    } else {
        // Pick first available world
        let mut picked = None;
        if let Ok(entries) = std::fs::read_dir(&saves_dir) {
            for entry in entries.flatten() {
                if entry.path().is_dir() {
                    picked = Some(entry.path());
                    break;
                }
            }
        }
        picked.unwrap_or_else(|| saves_dir.join("world"))
    };

    if target_world.is_dir() {
        zip.add_directory("world", options)
            .map_err(|e| LauncherError::Io(std::io::Error::new(std::io::ErrorKind::Other, e)))?;
        add_dir_to_zip(&target_world, "world", &mut zip, options)?;
    }

    // 2. Pack mods & generate BOM
    let mods_dir = game_dir.join("mods");
    let mut bom = BillOfMaterials::new(
        &instance.minecraft_version,
        if instance.mod_loader.r#type == "vanilla" {
            None
        } else {
            Some(instance.mod_loader.clone())
        },
        Some(instance.name.clone()),
    );

    if mods_dir.is_dir() {
        zip.add_directory("mods", options)
            .map_err(|e| LauncherError::Io(std::io::Error::new(std::io::ErrorKind::Other, e)))?;

        if let Ok(entries) = std::fs::read_dir(&mods_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                let name = entry.file_name().to_string_lossy().to_string();
                if path.is_file() && name.ends_with(".jar") {
                    // Filter client-only mods from server zip
                    if is_client_only(&name) {
                        continue;
                    }

                    if let Ok(bytes) = std::fs::read(&path) {
                        let mut hasher = Sha1::new();
                        hasher.update(&bytes);
                        let sha1 = hex::encode(hasher.finalize());

                        let clean_title = name.replace(".jar", "");
                        let mut mod_entry = ModEntry::new(
                            Some(clean_title),
                            name.clone(),
                            Some(sha1),
                            0,
                            Some("local".to_string()),
                            None,
                            bytes.len() as u64,
                        );
                        mod_entry.side = ModSide::Both;
                        bom.mods.push(mod_entry);

                        zip.start_file(format!("mods/{name}"), options)
                            .map_err(|e| LauncherError::Io(std::io::Error::new(std::io::ErrorKind::Other, e)))?;
                        zip.write_all(&bytes)?;
                    }
                }
            }
        }
    }

    // 3. Add config/ directory
    let config_dir = game_dir.join("config");
    if config_dir.is_dir() {
        zip.add_directory("config", options)
            .map_err(|e| LauncherError::Io(std::io::Error::new(std::io::ErrorKind::Other, e)))?;
        add_dir_to_zip(&config_dir, "config", &mut zip, options)?;
    }

    // 4. Generate bom.json
    let bom_json = serde_json::to_string_pretty(&bom)?;
    zip.start_file("bom.json", options)
        .map_err(|e| LauncherError::Io(std::io::Error::new(std::io::ErrorKind::Other, e)))?;
    zip.write_all(bom_json.as_bytes())?;

    // 5. Generate server.properties
    let props = format!(
        "motd=Zircon Server - {}\nserver-port=25565\ndifficulty=easy\ngamemode=survival\nmax-players=20\nview-distance=10\nenable-command-block=true\nonline-mode=true\n",
        instance.name
    );
    zip.start_file("server.properties", options)
        .map_err(|e| LauncherError::Io(std::io::Error::new(std::io::ErrorKind::Other, e)))?;
    zip.write_all(props.as_bytes())?;

    // 6. Generate eula.txt
    let eula = "eula=true\n";
    zip.start_file("eula.txt", options)
        .map_err(|e| LauncherError::Io(std::io::Error::new(std::io::ErrorKind::Other, e)))?;
    zip.write_all(eula.as_bytes())?;

    // 7. Write zircon-instance.json
    let instance_meta = serde_json::to_string_pretty(instance)?;
    zip.start_file("zircon-instance.json", options)
        .map_err(|e| LauncherError::Io(std::io::Error::new(std::io::ErrorKind::Other, e)))?;
    zip.write_all(instance_meta.as_bytes())?;

    zip.finish()
        .map_err(|e| LauncherError::Io(std::io::Error::new(std::io::ErrorKind::Other, e)))?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use zircon_core::model::ModLoaderInfo;

    #[test]
    fn export_mrpack_produces_valid_archive() {
        let temp_dir = std::env::temp_dir().join(format!("export_test_{}", chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0)));
        let _ = std::fs::create_dir_all(&temp_dir);

        let game_dir = temp_dir.join("game");
        std::fs::create_dir_all(game_dir.join("config")).unwrap();
        std::fs::write(game_dir.join("config").join("test.cfg"), "setting=true").unwrap();

        let instance = OfflineInstance {
            id: "test-export-inst".to_string(),
            name: "Test Pack".to_string(),
            minecraft_version: "1.20.4".to_string(),
            mod_loader: ModLoaderInfo::new("fabric", "0.15.11", None),
            java_args: "-Xmx4G".to_string(),
            last_played: 1000,
        };

        let out_zip = temp_dir.join("test_pack.mrpack");
        export_instance_mrpack(&game_dir, &instance, &out_zip).unwrap();
        assert!(out_zip.is_file());

        let zip_file = File::open(&out_zip).unwrap();
        let mut archive = zip::ZipArchive::new(zip_file).unwrap();
        assert!(archive.by_name("modrinth.index.json").is_ok());
        assert!(archive.by_name("overrides/config/test.cfg").is_ok());

        let _ = std::fs::remove_dir_all(temp_dir);
    }
}
