//! Reads the authoritative metadata out of a mod JAR by inspecting its
//! embedded metadata file, in precedence order:
//!
//! 1. `fabric.mod.json` — Fabric / Quilt
//! 2. `META-INF/neoforge.mods.toml` — NeoForge
//! 3. `META-INF/mods.toml` — Forge
//!
//! The first matching file wins (a jar can ship multiple metadata files for
//! cross-loader compatibility; NeoForge takes precedence over Forge when both
//! TOML files exist).
//!
//! Port of `com.mcmanager.core.mod.ModMetadataExtractor`.

use std::fmt;
use std::fs::File;
use std::io::Read;
use std::path::Path;

use serde_json::Value;
use zip::result::ZipError;

use crate::model::{ModLoaderType, ModMetadata};

pub const FABRIC_ENTRY: &str = "fabric.mod.json";
pub const NEOFORGE_ENTRY: &str = "META-INF/neoforge.mods.toml";
pub const FORGE_ENTRY: &str = "META-INF/mods.toml";

/// Max plausible ratio of total uncompressed bytes to total compressed bytes
/// for a mod JAR. Legitimate jars (class files, JSON/TOML, small binary
/// assets) sit far below this; anything exceeding it is treated as a
/// decompression bomb (a tiny download that would explode into gigabytes if
/// extracted).
pub const MAX_JAR_COMPRESSION_RATIO: u64 = 50;

/// Structural sanity check on a mod JAR before it is executed:
///
/// 1. the file must open as a valid ZIP archive (corrupt or truncated jars
///    are rejected instead of silently failing at game boot),
/// 2. the entries' declared total uncompressed size must not exceed
///    `MAX_JAR_COMPRESSION_RATIO` × total compressed size (zip-bomb guard), and
/// 3. the jar must declare mod metadata — `fabric.mod.json`, or
///    `META-INF/neoforge.mods.toml`, or `META-INF/mods.toml` — so only files
///    the loader can actually consume ever reach the mods folder.
///
/// Only central-directory headers are read; nothing is decompressed, so this
/// is cheap and cannot itself be memory-bombed.
pub fn validate_mod_jar_structure(jar_file: &Path) -> Result<(), String> {
    let file = File::open(jar_file).map_err(|e| format!("cannot open JAR: {e}"))?;
    let mut zip =
        zip::ZipArchive::new(file).map_err(|e| format!("not a valid ZIP archive: {e}"))?;

    let mut total_uncompressed: u64 = 0;
    let mut total_compressed: u64 = 0;
    let mut has_metadata = false;
    for i in 0..zip.len() {
        let entry = zip
            .by_index(i)
            .map_err(|e| format!("corrupt ZIP entry: {e}"))?;
        total_uncompressed = total_uncompressed.saturating_add(entry.size());
        total_compressed = total_compressed.saturating_add(entry.compressed_size());
        match entry.name() {
            FABRIC_ENTRY | NEOFORGE_ENTRY | FORGE_ENTRY => has_metadata = true,
            _ => {}
        }
    }

    if !has_metadata {
        return Err(format!(
            "no mod metadata found (expected {FABRIC_ENTRY}, {NEOFORGE_ENTRY} or {FORGE_ENTRY})"
        ));
    }
    if total_compressed > 0 && total_uncompressed > MAX_JAR_COMPRESSION_RATIO * total_compressed {
        return Err(format!(
            "implausible compression ratio: {total_uncompressed} uncompressed bytes vs \
             {total_compressed} compressed (possible decompression bomb)"
        ));
    }
    Ok(())
}

/// Cap on how many bytes of an embedded metadata file we will decompress.
/// Prevents an uncompressed ZIP bomb from exhausting memory when inspecting
/// untrusted mod jars.
const MAX_METADATA_BYTES: u64 = 2 * 1024 * 1024; // 2 MB limit

/// Errors raised while extracting mod metadata from a JAR.
#[derive(Debug)]
pub enum MetadataError {
    /// The JAR could not be opened or read.
    Io(std::io::Error),
    /// The JAR is not a readable zip archive.
    Zip(ZipError),
    /// The JAR carries no recognized metadata file.
    Unknown(String),
    /// A recognized metadata file was present but malformed.
    Invalid(String),
}

impl fmt::Display for MetadataError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MetadataError::Io(e) => write!(f, "failed to read mod jar: {e}"),
            MetadataError::Zip(e) => write!(f, "not a readable zip archive: {e}"),
            MetadataError::Unknown(name) => write!(f, "{name}"),
            MetadataError::Invalid(msg) => write!(f, "{msg}"),
        }
    }
}

impl std::error::Error for MetadataError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            MetadataError::Io(e) => Some(e),
            MetadataError::Zip(e) => Some(e),
            _ => None,
        }
    }
}

impl From<std::io::Error> for MetadataError {
    fn from(e: std::io::Error) -> Self {
        MetadataError::Io(e)
    }
}

impl From<ZipError> for MetadataError {
    fn from(e: ZipError) -> Self {
        MetadataError::Zip(e)
    }
}

/// Extracts metadata from the given mod jar.
///
/// Returns `Err(MetadataError::Unknown)` when the jar carries no recognized
/// metadata file, and `Err(MetadataError::Invalid)` when a recognized file is
/// malformed.
pub fn extract(jar_file: &Path) -> Result<ModMetadata, MetadataError> {
    let file = File::open(jar_file)?;
    let mut zip = zip::ZipArchive::new(file)?;

    if let Ok(mut entry) = zip.by_name(FABRIC_ENTRY) {
        let mut content = String::new();
        entry
            .by_ref()
            .take(MAX_METADATA_BYTES)
            .read_to_string(&mut content)?;
        return parse_fabric_metadata(&content);
    }

    if let Ok(mut entry) = zip.by_name(NEOFORGE_ENTRY) {
        let mut content = String::new();
        entry
            .by_ref()
            .take(MAX_METADATA_BYTES)
            .read_to_string(&mut content)?;
        return parse_toml_metadata(&content, ModLoaderType::NeoForge);
    }

    if let Ok(mut entry) = zip.by_name(FORGE_ENTRY) {
        let mut content = String::new();
        entry
            .by_ref()
            .take(MAX_METADATA_BYTES)
            .read_to_string(&mut content)?;
        return parse_toml_metadata(&content, ModLoaderType::Forge);
    }

    Err(MetadataError::Unknown(format!(
        "Unknown or unparseable mod jar: {}",
        jar_file.display()
    )))
}

// --------------------------------------------------------------------------
// fabric.mod.json
// --------------------------------------------------------------------------

fn parse_fabric_metadata(content: &str) -> Result<ModMetadata, MetadataError> {
    let root: Value = serde_json::from_str(content)
        .map_err(|e| MetadataError::Invalid(format!("Invalid fabric.mod.json: {e}")))?;
    let obj = root.as_object().ok_or_else(|| {
        MetadataError::Invalid("Invalid fabric.mod.json: root is not an object".to_string())
    })?;

    let id = text(&root, "id");
    let id = match id {
        Some(id) if !id.trim().is_empty() => id,
        _ => {
            return Err(MetadataError::Invalid(
                "fabric.mod.json is missing required field 'id'".to_string(),
            ))
        }
    };
    let name = text(&root, "name")
        .filter(|n| !n.trim().is_empty())
        .unwrap_or_else(|| id.clone());
    let version = text(&root, "version").unwrap_or_else(|| "0.0.0".to_string());
    let description = text(&root, "description").unwrap_or_default();
    let author = fabric_authors(&root);

    Ok(ModMetadata::new(
        id,
        name,
        version,
        description,
        author,
        ModLoaderType::Fabric,
        fabric_environment(obj),
    ))
}

/// `authors` is either a string or an array of strings / `{"name": ...}`
/// objects. Joins multiple authors with ", ".
fn fabric_authors(root: &Value) -> String {
    let Some(authors) = root.get("authors") else {
        return String::new();
    };
    if let Some(arr) = authors.as_array() {
        arr.iter()
            .filter_map(|a| {
                if let Some(s) = a.as_str() {
                    Some(s.to_string())
                } else if let Some(obj) = a.as_object() {
                    obj.get("name")
                        .and_then(|n| n.as_str())
                        .map(|s| s.to_string())
                } else {
                    None
                }
            })
            .collect::<Vec<_>>()
            .join(", ")
    } else if let Some(s) = authors.as_str() {
        s.to_string()
    } else {
        String::new()
    }
}

/// `environment` is a string (`"*"`, `"client"`, `"server"`) in the current
/// schema, but very old jars used an object like `{"client": "*"}`.
fn fabric_environment(obj: &serde_json::Map<String, Value>) -> String {
    let env = match obj.get("environment") {
        Some(env) => env,
        None => return "*".to_string(),
    };
    if let Some(env_obj) = env.as_object() {
        if env_obj.contains_key("client") && env_obj.contains_key("server") {
            return "both".to_string();
        }
        if env_obj.contains_key("client") {
            return "client".to_string();
        }
        if env_obj.contains_key("server") {
            return "server".to_string();
        }
        return "*".to_string();
    }
    env.as_str().unwrap_or("*").to_string()
}

// --------------------------------------------------------------------------
// META-INF/mods.toml and META-INF/neoforge.mods.toml
// --------------------------------------------------------------------------

fn parse_toml_metadata(
    content: &str,
    loader_type: ModLoaderType,
) -> Result<ModMetadata, MetadataError> {
    let value: toml::Value = toml::from_str(content)
        .map_err(|e| MetadataError::Invalid(format!("Invalid TOML metadata: {e}")))?;

    let entry_name = match loader_type {
        ModLoaderType::NeoForge => NEOFORGE_ENTRY,
        _ => FORGE_ENTRY,
    };

    let mods = value
        .get("mods")
        .and_then(|v| v.as_array())
        .filter(|a| !a.is_empty())
        .ok_or_else(|| {
            MetadataError::Invalid(format!("Missing [[mods]] section in {entry_name}"))
        })?;

    let first = mods
        .first()
        .and_then(|v| v.as_table())
        .ok_or_else(|| MetadataError::Invalid(format!("Empty [[mods]] entry in {entry_name}")))?;

    let id = first
        .get("modId")
        .and_then(|v| v.as_str())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .ok_or_else(|| {
            MetadataError::Invalid(format!("Missing 'modId' in [[mods]] entry of {entry_name}"))
        })?;

    let name = first
        .get("displayName")
        .and_then(|v| v.as_str())
        .map(|s| s.trim().to_string())
        .filter(|n| !n.is_empty())
        .unwrap_or_else(|| id.clone());
    // TOML is weakly typed: version is usually a string but occasionally a number.
    let version =
        toml_string_or_number(first.get("version")).unwrap_or_else(|| "0.0.0".to_string());
    let description = first
        .get("description")
        .and_then(|v| v.as_str())
        .unwrap_or_default();
    let author = first
        .get("authors")
        .and_then(|v| v.as_str())
        .unwrap_or_default()
        .to_string();

    Ok(ModMetadata::new(
        id,
        name,
        version,
        description,
        author,
        loader_type,
        "both",
    ))
}

fn toml_string_or_number(value: Option<&toml::Value>) -> Option<String> {
    match value {
        Some(toml::Value::String(s)) => Some(s.clone()),
        Some(toml::Value::Integer(i)) => Some(i.to_string()),
        Some(toml::Value::Float(f)) => Some(f.to_string()),
        _ => None,
    }
}

fn text(root: &Value, key: &str) -> Option<String> {
    root.get(key)
        .and_then(|el| el.as_str())
        .map(|s| s.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    struct ZipEntry<'a>(&'static str, &'a str);

    fn make_jar(name: &str, entries: &[ZipEntry<'_>]) -> PathBuf {
        // Unique per-jar directory so parallel tests never collide.
        let dir =
            std::env::temp_dir().join(format!("zircon-metadata-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join(name);
        let f = File::create(&file).unwrap();
        let mut zip = zip::ZipWriter::new(f);
        let options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated);
        for ZipEntry(entry_name, content) in entries {
            zip.start_file(*entry_name, options).unwrap();
            std::io::Write::write_all(&mut zip, content.as_bytes()).unwrap();
        }
        zip.finish().unwrap();
        file
    }

    #[test]
    fn extracts_fabric_metadata() {
        let jar = make_jar(
            "fabric-mod.jar",
            &[ZipEntry(
                "fabric.mod.json",
                r#"{
                    "id": "sodium",
                    "name": "Sodium",
                    "version": "0.5.8",
                    "description": "Fast rendering",
                    "environment": "client"
                }"#,
            )],
        );

        let meta = extract(&jar).unwrap();
        assert_eq!("sodium", meta.id);
        assert_eq!("Sodium", meta.name);
        assert_eq!("0.5.8", meta.version);
        assert_eq!("Fast rendering", meta.description);
        assert_eq!(ModLoaderType::Fabric, meta.loader_type);
        assert_eq!("client", meta.normalized_environment());
        assert_eq!("", meta.author); // no authors field
        let _ = std::fs::remove_dir_all(jar.parent().unwrap());
    }

    #[test]
    fn extracts_fabric_authors_from_strings_and_objects() {
        let jar = make_jar(
            "fabric-authors.jar",
            &[ZipEntry(
                "fabric.mod.json",
                r#"{
                    "id": "authorsmod",
                    "authors": [
                        "jellysquid3",
                        { "name": "grum", "contact": {} }
                    ]
                }"#,
            )],
        );

        let meta = extract(&jar).unwrap();
        assert_eq!("jellysquid3, grum", meta.author);
        let _ = std::fs::remove_dir_all(jar.parent().unwrap());
    }

    #[test]
    fn extracts_fabric_metadata_with_defaults() {
        let jar = make_jar(
            "fabric-min.jar",
            &[ZipEntry("fabric.mod.json", r#"{"id": "minimal"}"#)],
        );

        let meta = extract(&jar).unwrap();
        assert_eq!("minimal", meta.id);
        assert_eq!("minimal", meta.name); // name falls back to id
        assert_eq!("0.0.0", meta.version);
        assert_eq!(ModLoaderType::Fabric, meta.loader_type);
        assert_eq!("both", meta.normalized_environment());
        let _ = std::fs::remove_dir_all(jar.parent().unwrap());
    }

    #[test]
    fn fabric_environment_object_form() {
        let jar = make_jar(
            "fabric-env.jar",
            &[ZipEntry(
                "fabric.mod.json",
                r#"{"id": "envmod", "environment": {"client": "*"}}"#,
            )],
        );
        let meta = extract(&jar).unwrap();
        assert_eq!("client", meta.normalized_environment());
        let _ = std::fs::remove_dir_all(jar.parent().unwrap());
    }

    #[test]
    fn extracts_forge_toml_metadata() {
        let jar = make_jar(
            "forge-mod.jar",
            &[ZipEntry(
                "META-INF/mods.toml",
                r#"modLoader="javafml"
loaderVersion="[47,)"
license="MIT"

[[mods]]
modId="jei"
version="15.2.0.27"
displayName="Just Enough Items"
description="Show recipes in your inventory"
authors="mezz, snowshock"
"#,
            )],
        );

        let meta = extract(&jar).unwrap();
        assert_eq!("jei", meta.id);
        assert_eq!("Just Enough Items", meta.name);
        assert_eq!("15.2.0.27", meta.version);
        assert_eq!("Show recipes in your inventory", meta.description);
        assert_eq!("mezz, snowshock", meta.author);
        assert_eq!(ModLoaderType::Forge, meta.loader_type);
        assert_eq!("both", meta.normalized_environment());
        let _ = std::fs::remove_dir_all(jar.parent().unwrap());
    }

    #[test]
    fn extracts_neoforge_toml_metadata() {
        let jar = make_jar(
            "neoforge-mod.jar",
            &[ZipEntry(
                "META-INF/neoforge.mods.toml",
                r#"modLoader="javafml"
loaderVersion="[2,)"
license="MIT"

[[mods]]
modId="example"
version="1.0.0"
displayName="Example Mod"
"#,
            )],
        );

        let meta = extract(&jar).unwrap();
        assert_eq!("example", meta.id);
        assert_eq!("Example Mod", meta.name);
        assert_eq!("1.0.0", meta.version);
        assert_eq!(ModLoaderType::NeoForge, meta.loader_type);
        let _ = std::fs::remove_dir_all(jar.parent().unwrap());
    }

    #[test]
    fn numeric_version_string_in_toml() {
        let jar = make_jar(
            "numeric-version.jar",
            &[ZipEntry(
                "META-INF/mods.toml",
                r#"[[mods]]
modId="num"
version=42
"#,
            )],
        );
        let meta = extract(&jar).unwrap();
        assert_eq!("42", meta.version);
        let _ = std::fs::remove_dir_all(jar.parent().unwrap());
    }

    #[test]
    fn neoforge_metadata_wins_over_forge_when_both_present() {
        let jar = make_jar(
            "dual-toml.jar",
            &[
                ZipEntry(
                    "META-INF/mods.toml",
                    r#"[[mods]]
modId="forge-only"
version="1.0.0"
"#,
                ),
                ZipEntry(
                    "META-INF/neoforge.mods.toml",
                    r#"[[mods]]
modId="neoforge-only"
version="2.0.0"
"#,
                ),
            ],
        );

        let meta = extract(&jar).unwrap();
        assert_eq!("neoforge-only", meta.id);
        assert_eq!(ModLoaderType::NeoForge, meta.loader_type);
        let _ = std::fs::remove_dir_all(jar.parent().unwrap());
    }

    #[test]
    fn rejects_jar_without_metadata() {
        let jar = make_jar(
            "empty.jar",
            &[ZipEntry("META-INF/MANIFEST.MF", "Manifest-Version: 1.0\n")],
        );
        let err = extract(&jar).unwrap_err();
        assert!(err.to_string().contains("Unknown or unparseable mod jar"));
        let _ = std::fs::remove_dir_all(jar.parent().unwrap());
    }

    #[test]
    fn rejects_missing_id() {
        let jar = make_jar(
            "no-id.jar",
            &[ZipEntry("fabric.mod.json", r#"{"name": "No Id"}"#)],
        );
        let err = extract(&jar).unwrap_err();
        assert!(err.to_string().contains("missing required field 'id'"));
        let _ = std::fs::remove_dir_all(jar.parent().unwrap());
    }

    #[test]
    fn caps_metadata_read_at_2mb_preventing_zip_bomb() {
        // A fabric.mod.json far larger than the 2 MB cap. Without the cap this
        // JSON parses fine; with the cap the read is truncated mid-string so
        // parsing fails, proving the extractor never decompresses the whole
        // entry (the uncompressed ZIP bomb scenario).
        let padding = "a".repeat(MAX_METADATA_BYTES as usize + 1);
        let meta = format!(r#"{{"id": "bomb", "padding": "{padding}"}}"#);
        let jar = make_jar("zip-bomb.jar", &[ZipEntry("fabric.mod.json", &meta)]);

        let err = extract(&jar).unwrap_err();
        assert!(
            matches!(err, MetadataError::Invalid(_)),
            "oversized metadata should fail as truncated/invalid, got {err:?}"
        );
        let _ = std::fs::remove_dir_all(jar.parent().unwrap());
    }

    // ------------------------------------------------------------------
    // validate_mod_jar_structure
    // ------------------------------------------------------------------

    #[test]
    fn structure_accepts_valid_fabric_and_forge_jars() {
        let fabric = make_jar(
            "struct-fabric.jar",
            &[
                ZipEntry("fabric.mod.json", r#"{"id": "ok"}"#),
                ZipEntry("com/example/Mod.class", "class bytes"),
            ],
        );
        assert!(validate_mod_jar_structure(&fabric).is_ok());
        let _ = std::fs::remove_dir_all(fabric.parent().unwrap());

        let forge = make_jar(
            "struct-forge.jar",
            &[
                ZipEntry("META-INF/mods.toml", "[[mods]]\nmodId=\"ok\"\n"),
                ZipEntry("META-INF/MANIFEST.MF", "Manifest-Version: 1.0\n"),
            ],
        );
        assert!(validate_mod_jar_structure(&forge).is_ok());
        let _ = std::fs::remove_dir_all(forge.parent().unwrap());

        let neoforge = make_jar(
            "struct-neoforge.jar",
            &[ZipEntry(
                "META-INF/neoforge.mods.toml",
                "[[mods]]\nmodId=\"ok\"\n",
            )],
        );
        assert!(validate_mod_jar_structure(&neoforge).is_ok());
        let _ = std::fs::remove_dir_all(neoforge.parent().unwrap());
    }

    #[test]
    fn structure_rejects_jar_without_mod_metadata() {
        // A jar that only ships a manifest is not a mod the loader can use.
        let jar = make_jar(
            "struct-no-meta.jar",
            &[ZipEntry("META-INF/MANIFEST.MF", "Manifest-Version: 1.0\n")],
        );
        let err = validate_mod_jar_structure(&jar).unwrap_err();
        assert!(err.contains("no mod metadata"), "unhelpful error: {err}");
        let _ = std::fs::remove_dir_all(jar.parent().unwrap());
    }

    #[test]
    fn structure_rejects_non_zip_files() {
        let dir = std::env::temp_dir().join(format!("zircon-struct-notzip-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("fake.jar");
        std::fs::write(&file, b"this is definitely not a zip archive").unwrap();

        let err = validate_mod_jar_structure(&file).unwrap_err();
        assert!(
            err.contains("not a valid ZIP archive"),
            "unhelpful error: {err}"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn structure_rejects_implausible_compression_ratio() {
        // A single entry claiming to be 100 MB uncompressed while only 1 MB is
        // actually stored in the zip (deflated). The header check sees the
        // declared sizes and must reject before anything is extracted.
        let dir = std::env::temp_dir().join(format!("zircon-struct-bomb-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("bomb.jar");
        {
            let f = File::create(&file).unwrap();
            let mut zip = zip::ZipWriter::new(f);
            let options = zip::write::SimpleFileOptions::default()
                .compression_method(zip::CompressionMethod::Deflated);
            zip.start_file("fabric.mod.json", options).unwrap();
            std::io::Write::write_all(&mut zip, b"{\"id\": \"bomb\"}").unwrap();
            zip.finish().unwrap();
        }
        // Patch the central directory's uncompressed size for the single entry
        // to a huge value (100 MB) so the declared ratio exceeds the cap.
        patch_uncompressed_size(&file, 100 * 1024 * 1024);

        let err = validate_mod_jar_structure(&file).unwrap_err();
        assert!(err.contains("compression ratio"), "unhelpful error: {err}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Rewrites the uncompressed-size field (bytes 24..28) of the single
    /// central-directory entry in a zip file. Enough for tests: the entry
    /// header layout is fixed and the CRC stays unchecked by the header pass.
    fn patch_uncompressed_size(file: &std::path::Path, size: u32) {
        let bytes = std::fs::read(file).unwrap();
        // Find the EOCD and walk back to the central directory start.
        let eocd = bytes.windows(4).rposition(|w| w == b"PK\x05\x06").unwrap();
        let cd_size = u32::from_le_bytes(bytes[eocd + 12..eocd + 16].try_into().unwrap());
        let cd_offset = u32::from_le_bytes(bytes[eocd + 16..eocd + 20].try_into().unwrap());
        let cd_start = cd_offset as usize;
        // First entry header: signature(4) version(2+2) flags(2) method(2)
        // time(2) date(2) crc(4) compSize(4) uncompSize(4) -> offset + 24.
        let signature = &bytes[cd_start..cd_start + 4];
        assert_eq!(signature, b"PK\x01\x02", "expected a central directory");
        let mut patched = bytes.clone();
        patched[cd_start + 24..cd_start + 28].copy_from_slice(&size.to_le_bytes());
        std::fs::write(file, patched).unwrap();
        let _ = cd_size;
    }
}
