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

use crate::archive::limits:: { // z0
    max_compression_ratio, max_file_entries, max_uncompressed_bytes, // z0
    DEFAULT_MAX_COMPRESSION_RATIO, DEFAULT_MAX_METADATA_BYTES, RATIO_ENFORCEMENT_THRESHOLD_BYTES, // z0
}; // end-def 0
use crate::model::{ModLoaderType, ModMetadata};

pub const FABRIC_ENTRY: &str = "fabric.mod.json";
pub const NEOFORGE_ENTRY: &str = "META-INF/neoforge.mods.toml";
pub const FORGE_ENTRY: &str = "META-INF/mods.toml";
pub const MCMOD_INFO_ENTRY: &str = "mcmod.info";

/// Maximum allowed compression ratio for mod JAR archives (default 200:1).
pub const MAX_JAR_COMPRESSION_RATIO: u64 = DEFAULT_MAX_COMPRESSION_RATIO; // z0

/// Classification of a JAR artifact detected in an instance or mods folder.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ArtifactClassification {
    Fabric,
    NeoForge,
    ModernForge,
    LegacyForge,
    MinecraftClientJar { main_class: String },
    CoremodOrTweaker { identifier: String },
    GenericJar,
}

/// Strictly checks ZIP validity and compression safety (entry count, uncompressed size,
/// decompression bomb ratio) without inspecting or requiring mod manifests.
pub fn validate_archive_safety(jar_file: &Path) -> Result<(), String> {
    let file = File::open(jar_file).map_err(|err| format!("cannot open JAR: {err}"))?;
    let mut zip =
        zip::ZipArchive::new(file).map_err(|err| format!("not a valid ZIP archive: {err}"))?;

    let max_entries = max_file_entries();
    let max_bytes = max_uncompressed_bytes();
    let max_ratio = max_compression_ratio();

    if zip.len() > max_entries {
        return Err(format!(
            "JAR contains {} entries, exceeding maximum allowed entry count of {}",
            zip.len(),
            max_entries
        ));
    }

    let mut total_uncompressed: u64 = 0;
    let mut total_compressed: u64 = 0;

    for idx in 0..zip.len() {
        let entry = zip
            .by_index(idx)
            .map_err(|err| format!("corrupt ZIP entry: {err}"))?;
        total_uncompressed = total_uncompressed.saturating_add(entry.size());
        total_compressed = total_compressed.saturating_add(entry.compressed_size());
    }

    if total_uncompressed > max_bytes {
        return Err(format!(
            "JAR declared uncompressed size ({total_uncompressed} bytes) exceeds maximum limit of {max_bytes} bytes"
        ));
    }

    if total_compressed > 0
        && total_uncompressed >= RATIO_ENFORCEMENT_THRESHOLD_BYTES
        && total_uncompressed > max_ratio * total_compressed
    {
        let actual_ratio = total_uncompressed / total_compressed;
        return Err(format!(
            "implausible compression ratio: {total_uncompressed} uncompressed bytes vs \
             {total_compressed} compressed (ratio {actual_ratio}:1 exceeds limit {max_ratio}:1, possible decompression bomb)"
        ));
    }

    Ok(())
}

/// Inspects a JAR archive to classify its intended role (Fabric, NeoForge, Modern Forge,
/// Legacy Forge, Minecraft game client JAR, coremod/tweaker, or generic archive).
pub fn classify_jar_artifact(jar_file: &Path) -> Result<ArtifactClassification, String> {
    validate_archive_safety(jar_file)?;

    let file = File::open(jar_file).map_err(|err| format!("cannot open JAR: {err}"))?;
    let mut zip =
        zip::ZipArchive::new(file).map_err(|err| format!("not a valid ZIP archive: {err}"))?;

    let mut has_fabric = false;
    let mut has_neoforge = false;
    let mut has_forge_toml = false;
    let mut has_mcmod_info = false;
    let mut manifest_content: Option<String> = None;
    let mut has_client_class = false;
    let mut nested_jars: Vec<String> = Vec::new();

    for idx in 0..zip.len() {
        if let Ok(entry) = zip.by_index(idx) {
            let name = entry.name();
            match name {
                FABRIC_ENTRY => has_fabric = true,
                NEOFORGE_ENTRY => has_neoforge = true,
                FORGE_ENTRY => has_forge_toml = true,
                MCMOD_INFO_ENTRY => has_mcmod_info = true,
                "net/minecraft/client/main/Main.class" | "net/minecraft/client/Minecraft.class" => {
                    has_client_class = true;
                }
                _ => {
                    if (name.starts_with("META-INF/jarjar/") || name.starts_with("META-INF/jars/"))
                        && name.ends_with(".jar")
                    {
                        nested_jars.push(name.to_string());
                    }
                }
            }
        }
    }

    if let Ok(manifest_entry) = zip.by_name("META-INF/MANIFEST.MF") {
        let mut s = String::new();
        let _ = manifest_entry.take(65536).read_to_string(&mut s);
        manifest_content = Some(s);
    }

    if has_fabric {
        return Ok(ArtifactClassification::Fabric);
    }
    if has_neoforge {
        return Ok(ArtifactClassification::NeoForge);
    }
    if has_forge_toml {
        return Ok(ArtifactClassification::ModernForge);
    }
    if has_mcmod_info {
        return Ok(ArtifactClassification::LegacyForge);
    }

    // Check nested Jar-in-Jar archives (e.g. Kotlin for Forge, bundled libraries)
    for nested_name in &nested_jars {
        if let Ok(nested_entry) = zip.by_name(nested_name) {
            let mut nested_bytes = Vec::new();
            if nested_entry.take(50 * 1024 * 1024).read_to_end(&mut nested_bytes).is_ok() {
                if let Ok(mut inner_zip) = zip::ZipArchive::new(std::io::Cursor::new(nested_bytes)) {
                    for i in 0..inner_zip.len() {
                        if let Ok(inner_entry) = inner_zip.by_index(i) {
                            match inner_entry.name() {
                                NEOFORGE_ENTRY => return Ok(ArtifactClassification::NeoForge),
                                FORGE_ENTRY => return Ok(ArtifactClassification::ModernForge),
                                FABRIC_ENTRY => return Ok(ArtifactClassification::Fabric),
                                _ => {}
                            }
                        }
                    }
                }
            }
        }
    }

    // Forge/NeoForge Jar-in-Jar manifest marker
    if zip.by_name("META-INF/jarjar/metadata.json").is_ok() {
        return Ok(ArtifactClassification::NeoForge);
    }

    if let Some(manifest) = manifest_content {
        for line in manifest.lines() {
            let line = line.trim();
            if let Some(val) = line.strip_prefix("FMLModType:").map(str::trim) {
                if val.eq_ignore_ascii_case("LIBRARY") || val.eq_ignore_ascii_case("GAMELIBRARY") {
                    return Ok(ArtifactClassification::ModernForge);
                }
            }
            if let Some(val) = line.strip_prefix("ModType:").map(str::trim) {
                if val.eq_ignore_ascii_case("LIBRARY") {
                    return Ok(ArtifactClassification::ModernForge);
                }
            }
            if let Some(main) = line.strip_prefix("Main-Class:").map(str::trim) {
                if main.contains("net.minecraft.client.main.Main")
                    || main.contains("net.minecraft.client.Minecraft")
                    || main.contains("net.minecraft.launchwrapper.Launch")
                {
                    return Ok(ArtifactClassification::MinecraftClientJar {
                        main_class: main.to_string(),
                    });
                }
            }
            if let Some(coremod) = line.strip_prefix("FMLCorePlugin:").map(str::trim) {
                return Ok(ArtifactClassification::CoremodOrTweaker {
                    identifier: format!("FMLCorePlugin:{coremod}"),
                });
            }
            if let Some(tweaker) = line.strip_prefix("TweakClass:").map(str::trim) {
                return Ok(ArtifactClassification::CoremodOrTweaker {
                    identifier: format!("TweakClass:{tweaker}"),
                });
            }
        }
    }

    if has_client_class {
        return Ok(ArtifactClassification::MinecraftClientJar {
            main_class: "net.minecraft.client.main.Main".to_string(),
        });
    }

    Ok(ArtifactClassification::GenericJar)
}

/// Verifies structural integrity, entry counts, uncompressed size limits,
/// compression ratios, and metadata file presence for mod JARs before execution.
pub fn validate_mod_jar_structure(jar_file: &Path) -> Result<(), String> {
    validate_archive_safety(jar_file)?;
    match classify_jar_artifact(jar_file)? {
        ArtifactClassification::Fabric
        | ArtifactClassification::NeoForge
        | ArtifactClassification::ModernForge
        | ArtifactClassification::LegacyForge
        | ArtifactClassification::CoremodOrTweaker { .. } => Ok(()),
        ArtifactClassification::MinecraftClientJar { main_class } => Err(format!(
            "file is a Minecraft client JAR (main-class: {main_class}), not a mod"
        )),
        ArtifactClassification::GenericJar => Err(format!(
            "no mod metadata found (expected {FABRIC_ENTRY}, {NEOFORGE_ENTRY} or {FORGE_ENTRY})"
        )),
    }
}

/// Cap on how many bytes of an embedded metadata file we will decompress.
const MAX_METADATA_BYTES: u64 = DEFAULT_MAX_METADATA_BYTES; // 2 MB limit /* z0 */

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

    let fabric_content = if let Ok(mut entry) = zip.by_name(FABRIC_ENTRY) {
        let mut content = String::new();
        entry
            .by_ref()
            .take(MAX_METADATA_BYTES)
            .read_to_string(&mut content)?;
        Some(content)
    } else {
        None
    };
    if let Some(content) = fabric_content {
        let mut meta = parse_fabric_metadata(&content)?;
        let icon_candidates = fabric_icon_candidates(&content, &meta.id);
        meta.icon_data = extract_icon_from_zip(&mut zip, &icon_candidates);
        return Ok(meta);
    }

    let neoforge_content = if let Ok(mut entry) = zip.by_name(NEOFORGE_ENTRY) {
        let mut content = String::new();
        entry
            .by_ref()
            .take(MAX_METADATA_BYTES)
            .read_to_string(&mut content)?;
        Some(content)
    } else {
        None
    };
    if let Some(content) = neoforge_content {
        let mut meta = parse_toml_metadata(&content, ModLoaderType::NeoForge)?;
        let icon_candidates = toml_icon_candidates(&content, &meta.id);
        meta.icon_data = extract_icon_from_zip(&mut zip, &icon_candidates);
        return Ok(meta);
    }

    let forge_content = if let Ok(mut entry) = zip.by_name(FORGE_ENTRY) {
        let mut content = String::new();
        entry
            .by_ref()
            .take(MAX_METADATA_BYTES)
            .read_to_string(&mut content)?;
        Some(content)
    } else {
        None
    };
    if let Some(content) = forge_content {
        let mut meta = parse_toml_metadata(&content, ModLoaderType::Forge)?;
        let icon_candidates = toml_icon_candidates(&content, &meta.id);
        meta.icon_data = extract_icon_from_zip(&mut zip, &icon_candidates);
        return Ok(meta);
    }

    let mcmod_content = if let Ok(mut entry) = zip.by_name(MCMOD_INFO_ENTRY) {
        let mut content = String::new();
        entry
            .by_ref()
            .take(MAX_METADATA_BYTES)
            .read_to_string(&mut content)?;
        Some(content)
    } else {
        None
    };
    if let Some(content) = mcmod_content {
        let meta = parse_mcmod_info(&content)?;
        return Ok(meta);
    }

    // Fallback: inspect nested Jar-in-Jar archives (e.g. Kotlin for Forge, bundled libraries)
    for idx in 0..zip.len() {
        let nested_name = match zip.by_index(idx) {
            Ok(entry) => {
                let name = entry.name().to_string();
                if (name.starts_with("META-INF/jarjar/") || name.starts_with("META-INF/jars/"))
                    && name.ends_with(".jar")
                {
                    Some(name)
                } else {
                    None
                }
            }
            Err(_) => None,
        };

        if let Some(name) = nested_name {
            if let Ok(nested_entry) = zip.by_name(&name) {
                let mut nested_bytes = Vec::new();
                if nested_entry.take(50 * 1024 * 1024).read_to_end(&mut nested_bytes).is_ok() {
                    if let Ok(mut inner_zip) = zip::ZipArchive::new(std::io::Cursor::new(nested_bytes)) {
                        let neo_content = if let Ok(mut entry) = inner_zip.by_name(NEOFORGE_ENTRY) {
                            let mut content = String::new();
                            let _ = entry.by_ref().take(MAX_METADATA_BYTES).read_to_string(&mut content);
                            Some(content)
                        } else {
                            None
                        };
                        if let Some(content) = neo_content {
                            if let Ok(mut meta) = parse_toml_metadata(&content, ModLoaderType::NeoForge) {
                                let icon_candidates = toml_icon_candidates(&content, &meta.id);
                                meta.icon_data = extract_icon_from_zip(&mut inner_zip, &icon_candidates);
                                return Ok(meta);
                            }
                        }

                        let forge_content = if let Ok(mut entry) = inner_zip.by_name(FORGE_ENTRY) {
                            let mut content = String::new();
                            let _ = entry.by_ref().take(MAX_METADATA_BYTES).read_to_string(&mut content);
                            Some(content)
                        } else {
                            None
                        };
                        if let Some(content) = forge_content {
                            if let Ok(mut meta) = parse_toml_metadata(&content, ModLoaderType::Forge) {
                                let icon_candidates = toml_icon_candidates(&content, &meta.id);
                                meta.icon_data = extract_icon_from_zip(&mut inner_zip, &icon_candidates);
                                return Ok(meta);
                            }
                        }

                        let fabric_content = if let Ok(mut entry) = inner_zip.by_name(FABRIC_ENTRY) {
                            let mut content = String::new();
                            let _ = entry.by_ref().take(MAX_METADATA_BYTES).read_to_string(&mut content);
                            Some(content)
                        } else {
                            None
                        };
                        if let Some(content) = fabric_content {
                            if let Ok(mut meta) = parse_fabric_metadata(&content) {
                                let icon_candidates = fabric_icon_candidates(&content, &meta.id);
                                meta.icon_data = extract_icon_from_zip(&mut inner_zip, &icon_candidates);
                                return Ok(meta);
                            }
                        }
                    }
                }
            }
        }
    }

    // Fallback: library JAR with FMLModType: LIBRARY in MANIFEST.MF
    if let Ok(manifest_entry) = zip.by_name("META-INF/MANIFEST.MF") {
        let mut s = String::new();
        if manifest_entry.take(65536).read_to_string(&mut s).is_ok() {
            let is_library = s.lines().any(|line| {
                let trimmed = line.trim();
                trimmed.starts_with("FMLModType:")
                    || trimmed.starts_with("ModType:")
            });
            if is_library {
                let stem = jar_file
                    .file_stem()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_string();
                return Ok(ModMetadata {
                    id: stem.clone(),
                    name: stem,
                    version: String::new(),
                    description: "Library mod".to_string(),
                    author: String::new(),
                    loader_type: ModLoaderType::Forge,
                    environment: "both".to_string(),
                    icon_data: None,
                });
            }
        }
    }

    Err(MetadataError::Unknown(format!(
        "Unknown or unparseable mod jar: {}",
        jar_file.display()
    )))
}

/// Parses legacy Forge 1.7.10 - 1.12.2 `mcmod.info` metadata files.
pub fn parse_mcmod_info(content: &str) -> Result<ModMetadata, MetadataError> {
    let root: Value = serde_json::from_str(content)
        .map_err(|e| MetadataError::Invalid(format!("Malformed mcmod.info: {e}")))?;

    let entry = if let Some(arr) = root.as_array() {
        arr.first()
    } else if let Some(arr) = root.get("modList").and_then(|v| v.as_array()) {
        arr.first()
    } else {
        None
    };

    let obj = entry
        .and_then(|v| v.as_object())
        .ok_or_else(|| MetadataError::Invalid("mcmod.info contains no mod definitions".to_string()))?;

    let id = obj
        .get("modid")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim();
    if id.is_empty() {
        return Err(MetadataError::Invalid("mcmod.info missing 'modid'".to_string()));
    }

    let name = obj
        .get("name")
        .and_then(|v| v.as_str())
        .unwrap_or(id)
        .trim();

    let version = obj
        .get("version")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown")
        .trim();

    let description = obj
        .get("description")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim();

    let author = if let Some(authors) = obj.get("authorList").and_then(|v| v.as_array()) {
        authors
            .iter()
            .filter_map(|v| v.as_str())
            .collect::<Vec<_>>()
            .join(", ")
    } else if let Some(authors) = obj.get("authors").and_then(|v| v.as_array()) {
        authors
            .iter()
            .filter_map(|v| v.as_str())
            .collect::<Vec<_>>()
            .join(", ")
    } else {
        String::new()
    };

    Ok(ModMetadata::new(
        id,
        name,
        version,
        description,
        author,
        ModLoaderType::Forge,
        "*",
    ))
}

fn fabric_icon_candidates(content: &str, mod_id: &str) -> Vec<String> {
    let mut candidates = Vec::new();
    if let Ok(root) = serde_json::from_str::<Value>(content) {
        if let Some(icon) = root.get("icon") {
            if let Some(path) = icon.as_str() {
                candidates.push(path.to_string());
                candidates.push(format!("assets/{mod_id}/{path}"));
            } else if let Some(obj) = icon.as_object() {
                for (_k, v) in obj {
                    if let Some(path) = v.as_str() {
                        candidates.push(path.to_string());
                        candidates.push(format!("assets/{mod_id}/{path}"));
                    }
                }
            }
        }
    }
    candidates.push(format!("assets/{mod_id}/icon.png"));
    candidates.push("icon.png".to_string());
    candidates
}

fn toml_icon_candidates(content: &str, mod_id: &str) -> Vec<String> {
    let mut candidates = Vec::new();
    if let Ok(value) = toml::from_str::<toml::Value>(content) {
        if let Some(mods) = value.get("mods").and_then(|v| v.as_array()) {
            for m in mods {
                if let Some(logo) = m.get("logoFile").and_then(|v| v.as_str()) {
                    candidates.push(logo.to_string());
                    candidates.push(format!("assets/{mod_id}/{logo}"));
                }
            }
        }
    }
    candidates.push(format!("assets/{mod_id}/icon.png"));
    candidates.push("icon.png".to_string());
    candidates
}

fn extract_icon_from_zip<R: std::io::Read + std::io::Seek>(zip: &mut zip::ZipArchive<R>, candidates: &[String]) -> Option<String> {
    use base64::Engine as _;
    const MAX_ICON_BYTES: u64 = 512 * 1024;
    for candidate in candidates {
        let clean = candidate.trim().trim_start_matches('/');
        if clean.is_empty() {
            continue;
        }
        if let Ok(mut entry) = zip.by_name(clean) {
            if entry.size() > MAX_ICON_BYTES || entry.size() == 0 {
                continue;
            }
            let mut bytes = Vec::new();
            if entry.by_ref().take(MAX_ICON_BYTES).read_to_end(&mut bytes).is_ok() && !bytes.is_empty() {
                let lower = clean.to_ascii_lowercase();
                let mime = if lower.ends_with(".png") {
                    "image/png"
                } else if lower.ends_with(".jpg") || lower.ends_with(".jpeg") {
                    "image/jpeg"
                } else if lower.ends_with(".webp") {
                    "image/webp"
                } else if lower.ends_with(".svg") {
                    "image/svg+xml"
                } else {
                    "image/png"
                };
                let b64 = base64::engine::general_purpose::STANDARD.encode(&bytes);
                return Some(format!("data:{mime};base64,{b64}"));
            }
        }
    }
    None
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

    #[test]
    fn archive_safety_accepts_jar_without_mod_metadata() {
        let jar = make_jar(
            "safe-no-meta.jar",
            &[ZipEntry("META-INF/MANIFEST.MF", "Manifest-Version: 1.0\n")],
        );
        assert!(validate_archive_safety(&jar).is_ok());
        let _ = std::fs::remove_dir_all(jar.parent().unwrap());
    }

    #[test]
    fn classify_detects_legacy_forge_and_client_jar_and_coremod() {
        let legacy = make_jar(
            "legacy-forge.jar",
            &[ZipEntry(
                "mcmod.info",
                r#"[{"modid": "ironchest", "name": "Iron Chest", "version": "1.12.2-7.0.67.844"}]"#,
            )],
        );
        assert_eq!(
            classify_jar_artifact(&legacy).unwrap(),
            ArtifactClassification::LegacyForge
        );
        assert!(validate_mod_jar_structure(&legacy).is_ok());
        let meta = extract(&legacy).unwrap();
        assert_eq!(meta.id, "ironchest");
        assert_eq!(meta.name, "Iron Chest");
        assert_eq!(meta.version, "1.12.2-7.0.67.844");
        let _ = std::fs::remove_dir_all(legacy.parent().unwrap());

        let client = make_jar(
            "client.jar",
            &[ZipEntry(
                "META-INF/MANIFEST.MF",
                "Manifest-Version: 1.0\nMain-Class: net.minecraft.client.main.Main\n",
            )],
        );
        assert_eq!(
            classify_jar_artifact(&client).unwrap(),
            ArtifactClassification::MinecraftClientJar {
                main_class: "net.minecraft.client.main.Main".to_string()
            }
        );
        let err = validate_mod_jar_structure(&client).unwrap_err();
        assert!(err.contains("file is a Minecraft client JAR"));
        let _ = std::fs::remove_dir_all(client.parent().unwrap());

        let coremod = make_jar(
            "coremod.jar",
            &[ZipEntry(
                "META-INF/MANIFEST.MF",
                "Manifest-Version: 1.0\nFMLCorePlugin: com.example.CorePlugin\n",
            )],
        );
        assert_eq!(
            classify_jar_artifact(&coremod).unwrap(),
            ArtifactClassification::CoremodOrTweaker {
                identifier: "FMLCorePlugin:com.example.CorePlugin".to_string()
            }
        );
        assert!(validate_mod_jar_structure(&coremod).is_ok());
        let _ = std::fs::remove_dir_all(coremod.parent().unwrap());
    }

    #[test]
    fn classify_and_validate_jar_in_jar_and_library_mods() {
        let dir = tempfile::tempdir().unwrap();
        let inner_path = dir.path().join("inner.jar");
        {
            let file = std::fs::File::create(&inner_path).unwrap();
            let mut zip = zip::ZipWriter::new(file);
            let options = zip::write::SimpleFileOptions::default();
            zip.start_file("META-INF/neoforge.mods.toml", options).unwrap();
            std::io::Write::write_all(
                &mut zip,
                b"[[mods]]\nmodId = \"kotlinforforge\"\nversion = \"5.12.0\"\ndisplayName = \"Kotlin For Forge\"\n",
            )
            .unwrap();
            zip.finish().unwrap();
        }
        let inner_bytes = std::fs::read(&inner_path).unwrap();

        let outer_path = dir.path().join("kotlinforforge-5.12.0-all.jar");
        {
            let file = std::fs::File::create(&outer_path).unwrap();
            let mut zip = zip::ZipWriter::new(file);
            let options = zip::write::SimpleFileOptions::default();
            zip.start_file("META-INF/MANIFEST.MF", options).unwrap();
            std::io::Write::write_all(&mut zip, b"Manifest-Version: 1.0\nFMLModType: LIBRARY\n").unwrap();
            zip.start_file("META-INF/jarjar/metadata.json", options).unwrap();
            std::io::Write::write_all(&mut zip, b"{\"jars\": []}\n").unwrap();
            zip.start_file("META-INF/jarjar/kffmod-5.12.0.jar", options).unwrap();
            std::io::Write::write_all(&mut zip, &inner_bytes).unwrap();
            zip.finish().unwrap();
        }

        assert_eq!(
            classify_jar_artifact(&outer_path).unwrap(),
            ArtifactClassification::NeoForge
        );
        assert!(validate_mod_jar_structure(&outer_path).is_ok());

        let meta = extract(&outer_path).unwrap();
        assert_eq!(meta.id, "kotlinforforge");
        assert_eq!(meta.name, "Kotlin For Forge");
        assert_eq!(meta.version, "5.12.0");
    }
}
