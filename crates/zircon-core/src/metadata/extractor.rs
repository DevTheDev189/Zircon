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
        entry.read_to_string(&mut content)?;
        return parse_fabric_metadata(&content);
    }

    if let Ok(mut entry) = zip.by_name(NEOFORGE_ENTRY) {
        let mut content = String::new();
        entry.read_to_string(&mut content)?;
        return parse_toml_metadata(&content, ModLoaderType::NeoForge);
    }

    if let Ok(mut entry) = zip.by_name(FORGE_ENTRY) {
        let mut content = String::new();
        entry.read_to_string(&mut content)?;
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

    Ok(ModMetadata::new(
        id,
        name,
        version,
        description,
        ModLoaderType::Fabric,
        fabric_environment(obj),
    ))
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

    Ok(ModMetadata::new(
        id,
        name,
        version,
        description,
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

    struct ZipEntry(&'static str, &'static str);

    fn make_jar(name: &str, entries: &[ZipEntry]) -> PathBuf {
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
"#,
            )],
        );

        let meta = extract(&jar).unwrap();
        assert_eq!("jei", meta.id);
        assert_eq!("Just Enough Items", meta.name);
        assert_eq!("15.2.0.27", meta.version);
        assert_eq!("Show recipes in your inventory", meta.description);
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
}
