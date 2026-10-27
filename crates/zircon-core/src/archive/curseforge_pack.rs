//! Spec-compliant CurseForge Modpack (`manifest.json`) parser and override extraction engine.
//!
//! Provides bounded, streaming archive parsing and decompression engineered to defeat
//! path traversal attacks (Zip-Slip) and decompression bombs:
//! - Structural validation of `manifest.json`
//! - Extraction of loader type, loader version, and Minecraft version
//! - Safe extraction of the `overrides/` folder into target directory

use std::fs;
use std::io::{Read, Seek};
use std::path::Path;

use serde::{Deserialize, Serialize};
use zip::ZipArchive;

use crate::archive::limits::{
    ensure_safe_destination_path, is_safe_entry_path, is_safe_override_target,
    sanitize_entry_path, ArchiveError, ArchiveGuard,
};
use crate::archive::zip::ExtractionStats;

/// CurseForge manifest schema (formatVersion = 1).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CurseForgeManifest {
    pub minecraft: CurseForgeMinecraft,
    #[serde(default)]
    pub manifest_type: Option<String>,
    #[serde(default)]
    pub manifest_version: Option<u32>,
    pub name: String,
    #[serde(default)]
    pub version: Option<String>,
    #[serde(default)]
    pub author: Option<String>,
    #[serde(default)]
    pub files: Vec<CurseForgeFileRef>,
    #[serde(default = "default_overrides")]
    pub overrides: String,
}

fn default_overrides() -> String {
    "overrides".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CurseForgeMinecraft {
    pub version: String,
    #[serde(default)]
    pub mod_loaders: Vec<CurseForgeModLoaderRef>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CurseForgeModLoaderRef {
    pub id: String,
    #[serde(default)]
    pub primary: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CurseForgeFileRef {
    #[serde(rename = "projectID")]
    pub project_id: u32,
    #[serde(rename = "fileID")]
    pub file_id: u32,
    #[serde(default = "default_required")]
    pub required: bool,
}

fn default_required() -> bool {
    true
}

/// Parsed metadata extracted from a CurseForge modpack archive.
#[derive(Debug, Clone, PartialEq)]
pub struct CurseForgePackMetadata {
    /// Full parsed `manifest.json`.
    pub manifest: CurseForgeManifest,
    /// Target Minecraft version (e.g. "1.20.1").
    pub minecraft_version: String,
    /// Detected mod loader ("fabric", "neoforge", "forge", "quilt", "vanilla").
    pub loader_type: String,
    /// Detected mod loader version string (e.g. "47.4.18").
    pub loader_version: String,
}

/// Parses `manifest.json` from a CurseForge modpack archive without extracting overrides.
pub fn parse_curseforge_metadata<R: Read + Seek>(reader: R) -> Result<CurseForgePackMetadata, ArchiveError> {
    let mut zip = ZipArchive::new(reader)
        .map_err(|err| ArchiveError::InvalidData(format!("Invalid CurseForge ZIP archive: {err}")))?;

    let manifest_entry = zip
        .by_name("manifest.json")
        .map_err(|_| ArchiveError::InvalidData("Missing manifest.json in CurseForge modpack".to_string()))?;
    let max_meta = crate::archive::limits::DEFAULT_MAX_METADATA_BYTES;
    if manifest_entry.size() > max_meta {
        return Err(ArchiveError::ExceededMaxBytes {
            limit: max_meta,
            actual: manifest_entry.size(),
        });
    }

    let mut content = String::new();
    manifest_entry
        .take(max_meta + 1)
        .read_to_string(&mut content)?;
    if content.len() as u64 > max_meta {
        return Err(ArchiveError::ExceededMaxBytes {
            limit: max_meta,
            actual: content.len() as u64,
        });
    }

    let manifest: CurseForgeManifest = serde_json::from_str(&content)
        .map_err(|e| ArchiveError::InvalidData(format!("Malformed manifest.json: {e}")))?;

    let mc_version = manifest.minecraft.version.clone();

    let mut loader_type = "forge".to_string();
    let mut loader_version = String::new();

    let primary_loader = manifest
        .minecraft
        .mod_loaders
        .iter()
        .find(|l| l.primary)
        .or_else(|| manifest.minecraft.mod_loaders.first());

    if let Some(l) = primary_loader {
        let (t, v) = parse_loader_id(&l.id);
        loader_type = t;
        loader_version = v;
    }

    Ok(CurseForgePackMetadata {
        manifest,
        minecraft_version: mc_version,
        loader_type,
        loader_version,
    })
}

/// Parses a CurseForge modloader ID (e.g. "forge-47.4.18" or "fabric-0.16.10") into (loader_type, loader_version).
pub fn parse_loader_id(id: &str) -> (String, String) {
    let id_lower = id.to_ascii_lowercase();
    if let Some((loader, ver)) = id_lower.split_once('-') {
        let loader_type = match loader {
            "fabric" => "fabric".to_string(),
            "quilt" => "quilt".to_string(),
            "neoforge" => "neoforge".to_string(),
            _ => "forge".to_string(),
        };
        (loader_type, ver.to_string())
    } else {
        ("forge".to_string(), id.to_string())
    }
}

/// Safely extracts the overrides folder (usually `overrides/`) from a CurseForge modpack into `target_dir`.
///
/// The overrides prefix is stripped so that files like `overrides/config/foo.toml`
/// are written directly to `<target_dir>/config/foo.toml`.
/// Strict ZipSlip path traversal and decompression quota protections are enforced.
pub fn extract_curseforge_overrides<R: Read + Seek>(
    reader: R,
    overrides_name: &str,
    target_dir: &Path,
    guard: &ArchiveGuard,
) -> Result<ExtractionStats, ArchiveError> {
    let mut zip = ZipArchive::new(reader)
        .map_err(|err| ArchiveError::InvalidData(format!("Invalid CurseForge ZIP archive: {err}")))?;

    if !target_dir.is_dir() {
        fs::create_dir_all(target_dir)?;
    }
    let target_root = target_dir
        .canonicalize()
        .map_err(ArchiveError::Io)?;

    let safe_overrides = sanitize_entry_path(Path::new(overrides_name))
        .ok_or_else(|| ArchiveError::ZipSlip(overrides_name.to_string()))?;
    let normalized_overrides = safe_overrides
        .to_string_lossy()
        .replace('\\', "/");
    let prefix = format!("{}/", normalized_overrides.trim_matches('/'));
    let mut stats = ExtractionStats::default();
    let entry_count = zip.len();

    for idx in 0..entry_count {
        guard.record_entry()?;

        let mut entry = zip
            .by_index(idx)
            .map_err(|err| ArchiveError::InvalidData(format!("Corrupt archive entry: {err}")))?;

        let raw_name = entry.name().to_string();
        if entry.is_symlink() {
            return Err(ArchiveError::SymlinkOrHardlink(raw_name));
        }

        if !raw_name.starts_with(&prefix) {
            continue;
        }

        let stripped_path = match raw_name.strip_prefix(&prefix) {
            Some(p) if !p.trim().is_empty() => p,
            _ => continue,
        };

        if !is_safe_entry_path(Path::new(stripped_path)) {
            return Err(ArchiveError::ZipSlip(raw_name));
        }

        let safe_rel = sanitize_entry_path(Path::new(stripped_path))
            .ok_or_else(|| ArchiveError::ZipSlip(raw_name.clone()))?;
        if !is_safe_override_target(&safe_rel) {
            return Err(ArchiveError::InvalidData(format!(
                "Forbidden override target path rejected: {raw_name}"
            )));
        }

        let destination = target_root.join(&safe_rel);
        if !destination.starts_with(&target_root) {
            return Err(ArchiveError::ZipSlip(raw_name));
        }

        let declared_uncompressed = entry.size();
        let declared_compressed = entry.compressed_size();
        guard.check_entry_header(&raw_name, declared_uncompressed, declared_compressed)?;

        if entry.is_dir() {
            ensure_safe_destination_path(&target_root, &destination, &raw_name)?;
            fs::create_dir_all(&destination)?;
            continue;
        }

        ensure_safe_destination_path(&target_root, &destination, &raw_name)?;

        let written = crate::archive::zip::copy_stream_with_guard(
            &mut entry,
            &destination,
            guard,
            &raw_name,
            declared_uncompressed,
            declared_compressed,
        )?;

        stats.files_extracted += 1;
        stats.bytes_extracted = stats.bytes_extracted.saturating_add(written);
    }

    Ok(stats)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Cursor, Write};
    use tempfile::tempdir;
    use zip::write::SimpleFileOptions;
    use zip::ZipWriter;

    #[test]
    fn test_parse_curseforge_metadata() {
        let manifest_json = r#"{
            "minecraft": {
                "version": "1.20.1",
                "modLoaders": [
                    {
                        "id": "forge-47.4.18",
                        "primary": true
                    }
                ]
            },
            "manifestType": "minecraftModpack",
            "manifestVersion": 1,
            "name": "Ascendra",
            "version": "1.0",
            "author": "AscendraTeam",
            "files": [
                {
                    "projectID": 238222,
                    "fileID": 4812345,
                    "required": true
                }
            ],
            "overrides": "overrides"
        }"#;

        let mut buf = Vec::new();
        {
            let mut writer = ZipWriter::new(Cursor::new(&mut buf));
            writer.start_file("manifest.json", SimpleFileOptions::default()).unwrap();
            writer.write_all(manifest_json.as_bytes()).unwrap();
            writer.start_file("overrides/config/test.toml", SimpleFileOptions::default()).unwrap();
            writer.write_all(b"test = true\n").unwrap();
            writer.finish().unwrap();
        }

        let meta = parse_curseforge_metadata(Cursor::new(&buf)).unwrap();
        assert_eq!(meta.manifest.name, "Ascendra");
        assert_eq!(meta.minecraft_version, "1.20.1");
        assert_eq!(meta.loader_type, "forge");
        assert_eq!(meta.loader_version, "47.4.18");
        assert_eq!(meta.manifest.files.len(), 1);
        assert_eq!(meta.manifest.files[0].project_id, 238222);
        assert_eq!(meta.manifest.files[0].file_id, 4812345);

        let temp = tempdir().unwrap();
        let guard = ArchiveGuard::for_server_import();
        let stats = extract_curseforge_overrides(Cursor::new(&buf), &meta.manifest.overrides, temp.path(), &guard).unwrap();
        assert_eq!(stats.files_extracted, 1);
        assert!(temp.path().join("config/test.toml").is_file());
    }
}
