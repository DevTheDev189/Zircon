//! Reads the authoritative metadata out of a mod JAR by inspecting its
//! embedded metadata file.

use std::fmt;
use std::fs::File;
use std::io::Read;
use std::path::Path;

use zip::result::ZipError;

use crate::archive::limits::{
    max_compression_ratio, max_file_entries, max_uncompressed_bytes,
    DEFAULT_MAX_COMPRESSION_RATIO, DEFAULT_MAX_METADATA_BYTES, RATIO_ENFORCEMENT_THRESHOLD_BYTES,
};
use crate::model::{ModLoaderType, ModMetadata};

pub mod icons;
pub mod fabric;
pub mod forge;
pub mod neoforge;

pub use icons::*;
pub use fabric::*;
pub use forge::*;
pub use neoforge::*;

#[cfg(test)]
mod tests;

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

