//! Spec-compliant Modrinth Modpack (`.mrpack`) parser and override extraction engine.
//!
//! Provides bounded, streaming archive parsing and decompression engineered to defeat
//! path traversal attacks (Zip-Slip) and decompression bombs:
//! - Structural validation of `modrinth.index.json`
//! - Extraction of loader type, loader version, and Minecraft version
//! - Environment filtering (distinguishing server-side vs client-only mods)
//! - Safe extraction of the `overrides/` folder into target directory

use std::fs;
use std::io::{Read, Seek};
use std::path::Path;

use zip::ZipArchive;

use crate::archive::limits::{
    ensure_safe_destination_path, is_safe_entry_path, is_safe_override_target,
    sanitize_entry_path, ArchiveError, ArchiveGuard,
};
use crate::archive::zip::ExtractionStats;
use crate::export::{ModrinthIndex, ModrinthIndexFile};

/// Parsed metadata extracted from a `.mrpack` archive.
#[derive(Debug, Clone, PartialEq)]
pub struct MrpackMetadata {
    /// Full parsed `modrinth.index.json`.
    pub index: ModrinthIndex,
    /// Target Minecraft version (e.g. "1.20.1").
    pub minecraft_version: String,
    /// Detected mod loader ("fabric", "neoforge", "forge", "quilt", "vanilla").
    pub loader_type: String,
    /// Detected mod loader version string.
    pub loader_version: String,
}

/// Parses `modrinth.index.json` from a `.mrpack` archive without extracting overrides.
pub fn parse_mrpack_metadata<R: Read + Seek>(reader: R) -> Result<MrpackMetadata, ArchiveError> {
    let mut zip = ZipArchive::new(reader)
        .map_err(|err| ArchiveError::InvalidData(format!("Invalid .mrpack archive format: {err}")))?;

    let index_entry = zip
        .by_name("modrinth.index.json")
        .map_err(|_| ArchiveError::InvalidData("Missing modrinth.index.json in .mrpack".to_string()))?;
    let max_meta = crate::archive::limits::DEFAULT_MAX_METADATA_BYTES;
    if index_entry.size() > max_meta {
        return Err(ArchiveError::ExceededMaxBytes {
            limit: max_meta,
            actual: index_entry.size(),
        });
    }

    let mut content = String::new();
    index_entry
        .take(max_meta + 1)
        .read_to_string(&mut content)?;
    if content.len() as u64 > max_meta {
        return Err(ArchiveError::ExceededMaxBytes {
            limit: max_meta,
            actual: content.len() as u64,
        });
    }

    let index: ModrinthIndex = serde_json::from_str(&content)
        .map_err(|e| ArchiveError::InvalidData(format!("Malformed modrinth.index.json: {e}")))?;

    let mc_version = index
        .dependencies
        .get("minecraft")
        .cloned()
        .unwrap_or_else(|| "1.20.1".to_string());

    let mut loader_type = "vanilla".to_string();
    let mut loader_version = String::new();

    if let Some(v) = index.dependencies.get("fabric-loader") {
        loader_type = "fabric".to_string();
        loader_version = v.clone();
    } else if let Some(v) = index.dependencies.get("neoforge") {
        loader_type = "neoforge".to_string();
        loader_version = v.clone();
    } else if let Some(v) = index.dependencies.get("forge") {
        loader_type = "forge".to_string();
        loader_version = v.clone();
    } else if let Some(v) = index.dependencies.get("quilt-loader") {
        loader_type = "quilt".to_string();
        loader_version = v.clone();
    }

    Ok(MrpackMetadata {
        index,
        minecraft_version: mc_version,
        loader_type,
        loader_version,
    })
}

/// Partitions files in a [`ModrinthIndex`] into server-compatible mods vs client-only mods.
///
/// Modrinth specification for `env.server`:
/// - `"required"`: Mod is required on dedicated servers.
/// - `"optional"`: Mod can run on dedicated servers.
/// - `"unsupported"`: Mod is client-only (e.g. Sodium, Iris, ModMenu) and must NOT be installed on dedicated servers.
pub fn filter_mrpack_files(
    index: &ModrinthIndex,
) -> (Vec<ModrinthIndexFile>, Vec<ModrinthIndexFile>) {
    let mut server_mods = Vec::new();
    let mut client_only_mods = Vec::new();

    for file in &index.files {
        let server_env = file.env.server.to_ascii_lowercase();
        if server_env == "unsupported" {
            client_only_mods.push(file.clone());
        } else {
            server_mods.push(file.clone());
        }
    }

    (server_mods, client_only_mods)
}

/// Safely extracts the `overrides/` folder from a `.mrpack` archive into `target_dir`.
///
/// The `overrides/` prefix is stripped so that files like `overrides/config/foo.toml`
/// are written directly to `<target_dir>/config/foo.toml`.
/// Strict ZipSlip path traversal and decompression quota protections are enforced.
pub fn extract_mrpack_overrides<R: Read + Seek>(
    reader: R,
    target_dir: &Path,
    guard: &ArchiveGuard,
) -> Result<ExtractionStats, ArchiveError> {
    let mut zip = ZipArchive::new(reader)
        .map_err(|err| ArchiveError::InvalidData(format!("Invalid .mrpack ZIP archive: {err}")))?;

    if !target_dir.is_dir() {
        fs::create_dir_all(target_dir)?;
    }
    let target_root = target_dir
        .canonicalize()
        .map_err(|e| ArchiveError::Io(e))?;

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

        // Only process entries located under `overrides/` or `server-overrides/`
        let (stripped_path, is_override) = if let Some(stripped) = raw_name.strip_prefix("overrides/") {
            (stripped, true)
        } else if let Some(stripped) = raw_name.strip_prefix("server-overrides/") {
            (stripped, true)
        } else {
            ("", false)
        };

        if !is_override || stripped_path.is_empty() {
            continue;
        }

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
    use std::fs::File;
    use std::io::Write;
    use tempfile::tempdir;
    use zip::write::SimpleFileOptions;
    use zip::ZipWriter;

    #[test]
    fn test_parse_mrpack_metadata_and_filter() {
        let temp = tempdir().unwrap();
        let pack_path = temp.path().join("test.mrpack");
        let file = File::create(&pack_path).unwrap();
        let mut zip = ZipWriter::new(file);
        let opts = SimpleFileOptions::default();

        let index_json = r#"{
            "formatVersion": 1,
            "game": "minecraft",
            "versionId": "1.0.0",
            "name": "Fabric Survival",
            "dependencies": {
                "minecraft": "1.20.1",
                "fabric-loader": "0.15.11"
            },
            "files": [
                {
                    "path": "mods/sodium.jar",
                    "hashes": {"sha1": "abc"},
                    "env": {"client": "required", "server": "unsupported"},
                    "downloads": ["https://example.com/sodium.jar"],
                    "fileSize": 500
                },
                {
                    "path": "mods/fabric-api.jar",
                    "hashes": {"sha1": "def"},
                    "env": {"client": "required", "server": "required"},
                    "downloads": ["https://example.com/fabric-api.jar"],
                    "fileSize": 1000
                }
            ]
        }"#;

        zip.start_file("modrinth.index.json", opts).unwrap();
        zip.write_all(index_json.as_bytes()).unwrap();

        zip.start_file("overrides/config/test.json", opts).unwrap();
        zip.write_all(b"{\"hello\": \"world\"}").unwrap();

        zip.finish().unwrap();

        let read_file = File::open(&pack_path).unwrap();
        let meta = parse_mrpack_metadata(&read_file).unwrap();
        assert_eq!(meta.minecraft_version, "1.20.1");
        assert_eq!(meta.loader_type, "fabric");
        assert_eq!(meta.loader_version, "0.15.11");
        assert_eq!(meta.index.name, "Fabric Survival");

        let (server_mods, client_only) = filter_mrpack_files(&meta.index);
        assert_eq!(server_mods.len(), 1);
        assert_eq!(server_mods[0].path, "mods/fabric-api.jar");
        assert_eq!(client_only.len(), 1);
        assert_eq!(client_only[0].path, "mods/sodium.jar");

        // Test overrides extraction
        let extract_target = temp.path().join("extracted_instance");
        let guard = ArchiveGuard::default();
        let read_file2 = File::open(&pack_path).unwrap();
        let stats = extract_mrpack_overrides(read_file2, &extract_target, &guard).unwrap();

        assert_eq!(stats.files_extracted, 1);
        let extracted_config = extract_target.join("config/test.json");
        assert!(extracted_config.is_file());
        let content = std::fs::read_to_string(extracted_config).unwrap();
        assert_eq!(content, "{\"hello\": \"world\"}");
    }
}
