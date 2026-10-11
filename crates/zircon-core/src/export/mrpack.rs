//! High-performance, spec-compliant `.mrpack` archive export engine.
//!
//! Generates standard Modrinth modpack archives containing:
//! 1. `modrinth.index.json` (formatVersion = 1) referencing remote CDN URLs and hashes.
//! 2. `overrides/` folder bundling configs, scripts, resourcepacks, and local mods lacking CDN URLs.

use std::collections::HashSet;
use std::fs::File;
use std::io::{Seek, Write};
use std::path::Path;

use zip::write::SimpleFileOptions;
use zip::ZipWriter;

use crate::archive::limits::ArchiveError;
use crate::export::{bom_to_modrinth_index, ModrinthIndex};
use crate::model::bom::BillOfMaterials;

/// Options configuring `.mrpack` archive export.
#[derive(Debug, Clone)]
pub struct MrpackExportOptions {
    /// Modpack name stored in `modrinth.index.json`.
    pub pack_name: String,
    /// Version string for the modpack (e.g. "1.0.0" or "1.20.1-20261005").
    pub version_name: String,
    /// Summary or description of the pack.
    pub summary: Option<String>,
    /// Whether to bundle custom configs from `config/` into `overrides/config/`.
    pub include_configs: bool,
    /// Whether to bundle `resourcepacks/` into `overrides/resourcepacks/`.
    pub include_resourcepacks: bool,
    /// Optional base URL override for local mods that can be hosted on a public server.
    pub custom_download_base_url: Option<String>,
    /// Optional server address (IP or hostname with optional port) to embed as `overrides/servers.dat`.
    /// When specified, importing this modpack in Prism Launcher, Modrinth App, or Zircon will
    /// automatically add the server to the user's Multiplayer server list.
    pub server_address: Option<String>,
}

impl Default for MrpackExportOptions {
    fn default() -> Self {
        Self {
            pack_name: "Zircon Modpack".to_string(),
            version_name: "1.0.0".to_string(),
            summary: Some("Exported from Zircon".to_string()),
            include_configs: true,
            include_resourcepacks: true,
            custom_download_base_url: None,
            server_address: None,
        }
    }
}

/// Errors that can occur while building an `.mrpack` archive.
#[derive(Debug)]
pub enum MrpackExportError {
    Io(std::io::Error),
    Zip(zip::result::ZipError),
    Json(serde_json::Error),
    Archive(ArchiveError),
}

impl std::fmt::Display for MrpackExportError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(e) => write!(f, "I/O error during mrpack export: {e}"),
            Self::Zip(e) => write!(f, "ZIP error during mrpack export: {e}"),
            Self::Json(e) => write!(f, "JSON serialization error: {e}"),
            Self::Archive(e) => write!(f, "Archive error: {e}"),
        }
    }
}

impl std::error::Error for MrpackExportError {}

impl From<std::io::Error> for MrpackExportError {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e)
    }
}

impl From<zip::result::ZipError> for MrpackExportError {
    fn from(e: zip::result::ZipError) -> Self {
        Self::Zip(e)
    }
}

impl From<serde_json::Error> for MrpackExportError {
    fn from(e: serde_json::Error) -> Self {
        Self::Json(e)
    }
}

impl From<ArchiveError> for MrpackExportError {
    fn from(e: ArchiveError) -> Self {
        Self::Archive(e)
    }
}

/// Directories and files strictly excluded from client `.mrpack` exports.
const EXCLUDED_PATHS: &[&str] = &[
    "world",
    "world_nether",
    "world_the_end",
    "logs",
    "crash-reports",
    "server.properties",
    "whitelist.json",
    "ops.json",
    "banned-players.json",
    "banned-ips.json",
    "usercache.json",
    "server.jar",
    ".zircon",
    "server-data",
];

fn is_excluded_name(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    EXCLUDED_PATHS.iter().any(|&ex| lower == ex)
}

/// Recursively copies a directory tree into a [`ZipWriter`] under `overrides/<prefix>`.
fn add_dir_to_overrides<W: Write + Seek>(
    src_dir: &Path,
    prefix: &str,
    zip: &mut ZipWriter<W>,
    options: SimpleFileOptions,
) -> Result<(), MrpackExportError> {
    if !src_dir.is_dir() {
        return Ok(());
    }

    for entry in std::fs::read_dir(src_dir)? {
        let entry = entry?;
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();

        if is_excluded_name(&name) {
            continue;
        }

        let entry_rel = format!("overrides/{prefix}/{name}");

        if path.is_dir() {
            zip.add_directory(&entry_rel, options)?;
            add_dir_to_overrides(&path, &format!("{prefix}/{name}"), zip, options)?;
        } else if path.is_file() {
            zip.start_file(&entry_rel, options)?;
            let mut f = File::open(&path)?;
            std::io::copy(&mut f, zip)?;
        }
    }
    Ok(())
}

/// Creates a spec-compliant `.mrpack` archive and writes it into the provided `Write + Seek` destination.
pub fn create_instance_mrpack<W: Write + Seek>(
    writer: W,
    bom: &BillOfMaterials,
    instance_dir: &Path,
    options: &MrpackExportOptions,
) -> Result<(), MrpackExportError> {
    let mut zip = ZipWriter::new(writer);
    let zip_options = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);

    // 1. Convert BOM to standard ModrinthIndex
    let mut index: ModrinthIndex = bom_to_modrinth_index(bom, Some(&options.pack_name));
    index.version_id = options.version_name.clone();
    if let Some(summary) = &options.summary {
        index.summary = Some(summary.clone());
    }

    // Set of remote mod filenames that are resolved via CDN URLs
    let remote_mod_filenames: HashSet<String> = index
        .files
        .iter()
        .map(|f| f.path.trim_start_matches("mods/").to_ascii_lowercase())
        .collect();

    // Serialize and write `modrinth.index.json`
    let index_json = serde_json::to_string_pretty(&index)?;
    zip.start_file("modrinth.index.json", zip_options)?;
    zip.write_all(index_json.as_bytes())?;

    // 2. Add local mods without remote download URLs into `overrides/mods/`
    let mods_dir = instance_dir.join("mods");
    if mods_dir.is_dir() {
        if let Ok(entries) = std::fs::read_dir(&mods_dir) {
            let mut created_mods_dir = false;
            for entry in entries.flatten() {
                let path = entry.path();
                let name = entry.file_name().to_string_lossy().to_string();
                if path.is_file() && !remote_mod_filenames.contains(&name.to_ascii_lowercase()) {
                    if !created_mods_dir {
                        zip.add_directory("overrides/mods", zip_options)?;
                        created_mods_dir = true;
                    }
                    let entry_rel = format!("overrides/mods/{name}");
                    zip.start_file(&entry_rel, zip_options)?;
                    let mut f = File::open(&path)?;
                    std::io::copy(&mut f, &mut zip)?;
                }
            }
        }
    }

    // 3. Add custom configurations from `config/` into `overrides/config/`
    if options.include_configs {
        let config_dir = instance_dir.join("config");
        if config_dir.is_dir() {
            zip.add_directory("overrides/config", zip_options)?;
            add_dir_to_overrides(&config_dir, "config", &mut zip, zip_options)?;
        }

        // Script directories common in modpacks (KubeJS, CraftTweaker, defaultconfigs)
        for dir_name in &["defaultconfigs", "kubejs", "scripts"] {
            let extra_dir = instance_dir.join(dir_name);
            if extra_dir.is_dir() {
                zip.add_directory(format!("overrides/{dir_name}"), zip_options)?;
                add_dir_to_overrides(&extra_dir, dir_name, &mut zip, zip_options)?;
            }
        }
    }

    // 4. Add `resourcepacks/` if requested
    if options.include_resourcepacks {
        let rp_dir = instance_dir.join("resourcepacks");
        if rp_dir.is_dir() {
            zip.add_directory("overrides/resourcepacks", zip_options)?;
            add_dir_to_overrides(&rp_dir, "resourcepacks", &mut zip, zip_options)?;
        }
    }

    // 5. Add `options.txt` if present
    let options_txt = instance_dir.join("options.txt");
    if options_txt.is_file() {
        zip.start_file("overrides/options.txt", zip_options)?;
        let mut f = File::open(&options_txt)?;
        std::io::copy(&mut f, &mut zip)?;
    }

    // 6. Add `overrides/servers.dat` if server_address is provided or if servers.dat exists in instance_dir
    let instance_servers_dat = instance_dir.join("servers.dat");
    if instance_servers_dat.is_file() {
        zip.start_file("overrides/servers.dat", zip_options)?;
        let mut f = File::open(&instance_servers_dat)?;
        std::io::copy(&mut f, &mut zip)?;
    } else if let Some(addr) = &options.server_address {
        let dat_bytes = crate::export::servers_dat::build_single_server_dat(&options.pack_name, addr);
        zip.start_file("overrides/servers.dat", zip_options)?;
        zip.write_all(&dat_bytes)?;
    }

    zip.finish()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Read;
    use crate::model::bom::{ModEntry, ModLoaderInfo, ModSide};
    use tempfile::tempdir;

    #[test]
    fn test_create_instance_mrpack_roundtrip() {
        let temp = tempdir().unwrap();
        let inst_dir = temp.path().join("test_instance");
        std::fs::create_dir_all(inst_dir.join("config")).unwrap();
        std::fs::create_dir_all(inst_dir.join("mods")).unwrap();

        // Write sample config and local custom mod
        std::fs::write(inst_dir.join("config/test.toml"), "key = true\n").unwrap();
        std::fs::write(inst_dir.join("mods/custom-local.jar"), b"PK00fake").unwrap();

        let mut bom = BillOfMaterials::new(
            "1.20.1",
            Some(ModLoaderInfo::new("fabric", "0.15.11", None)),
            Some("Test SMP Pack".to_string()),
        );
        bom.mods.push(ModEntry {
            filename: "fabric-api-0.92.0.jar".to_string(),
            title: Some("Fabric API".to_string()),
            download_url: Some("https://cdn.modrinth.com/data/P7dR8mSH/versions/123/fabric-api.jar".to_string()),
            sha1: Some("da39a3ee5e6b4b0d3255bfef95601890afd80709".to_string()),
            file_size: 1024,
            side: ModSide::Both,
            ..Default::default()
        });

        let mut output_bytes = std::io::Cursor::new(Vec::new());
        let options = MrpackExportOptions {
            pack_name: "Test SMP Pack".to_string(),
            version_name: "1.0.0".to_string(),
            ..Default::default()
        };

        create_instance_mrpack(&mut output_bytes, &bom, &inst_dir, &options).unwrap();
        let zip_bytes = output_bytes.into_inner();
        assert!(!zip_bytes.is_empty());

        // Validate ZIP contents
        let reader = std::io::Cursor::new(zip_bytes);
        let mut archive = zip::ZipArchive::new(reader).unwrap();

        // Check modrinth.index.json
        let mut index_entry = archive.by_name("modrinth.index.json").unwrap();
        let mut index_str = String::new();
        index_entry.read_to_string(&mut index_str).unwrap();
        let parsed_index: ModrinthIndex = serde_json::from_str(&index_str).unwrap();

        assert_eq!(parsed_index.name, "Test SMP Pack");
        assert_eq!(parsed_index.dependencies.get("minecraft").unwrap(), "1.20.1");
        assert_eq!(parsed_index.dependencies.get("fabric-loader").unwrap(), "0.15.11");
        assert_eq!(parsed_index.files.len(), 1);
        assert_eq!(parsed_index.files[0].path, "mods/fabric-api-0.92.0.jar");

        drop(index_entry);

        // Check that local mod is placed in overrides/mods/
        assert!(archive.by_name("overrides/mods/custom-local.jar").is_ok());

        // Check that config is in overrides/config/
        assert!(archive.by_name("overrides/config/test.toml").is_ok());
    }

    #[test]
    fn test_create_instance_mrpack_with_servers_dat() {
        let temp = tempdir().unwrap();
        let inst_dir = temp.path().join("test_instance");
        std::fs::create_dir_all(&inst_dir).unwrap();

        let bom = BillOfMaterials::new("1.20.1", None, Some("Cool SMP".to_string()));
        let mut output_bytes = std::io::Cursor::new(Vec::new());
        let options = MrpackExportOptions {
            pack_name: "Cool SMP".to_string(),
            version_name: "1.0.0".to_string(),
            server_address: Some("mc.example.com:25565".to_string()),
            ..Default::default()
        };

        create_instance_mrpack(&mut output_bytes, &bom, &inst_dir, &options).unwrap();
        let zip_bytes = output_bytes.into_inner();

        let reader = std::io::Cursor::new(zip_bytes);
        let mut archive = zip::ZipArchive::new(reader).unwrap();

        let mut servers_entry = archive.by_name("overrides/servers.dat").expect("overrides/servers.dat must exist");
        let mut servers_bytes = Vec::new();
        servers_entry.read_to_end(&mut servers_bytes).unwrap();
        assert!(!servers_bytes.is_empty());
        let content_str = String::from_utf8_lossy(&servers_bytes);
        assert!(content_str.contains("mc.example.com:25565"));
        assert!(content_str.contains("Cool SMP"));
    }
}
