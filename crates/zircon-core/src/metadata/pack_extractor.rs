//! Metadata extraction for Resource Packs (`pack.mcmeta`) and Shaderpacks
//! (`shaders/shaders.properties`, shader header comments, and filenames).

use std::fs::File;
use std::io::Read;
use std::path::Path;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::archive::limits::DEFAULT_MAX_METADATA_BYTES;
use crate::metadata::extractor::MetadataError;

pub const PACK_MCMETA_ENTRY: &str = "pack.mcmeta";
pub const SHADERS_PROPERTIES_ENTRY: &str = "shaders/shaders.properties";
pub const SHADERS_PROPERTIES_ALT: &str = "shaders.properties";

const MAX_METADATA_BYTES: u64 = DEFAULT_MAX_METADATA_BYTES; // 2 MB limit

/// Metadata extracted from a Minecraft Texture / Resource Pack.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ResourcePackMetadata {
    /// Declared `pack_format` integer (e.g. 15 for MC 1.20.x, 34 for MC 1.21.x).
    pub pack_format: Option<u32>,
    /// Human-readable description extracted from `pack.mcmeta`.
    pub description: Option<String>,
    /// Resolved or explicit version string (e.g. "v1.4.2", "1.2.0", "Format 15").
    pub version: Option<String>,
    /// Minecraft version compatibility mapping (e.g. "1.20 - 1.20.1", "1.21 - 1.21.1").
    pub mc_compatibility: Option<String>,
}

/// Metadata extracted from a Minecraft Shaderpack.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ShaderPackMetadata {
    /// Resolved version string (e.g. "r5.1.1", "v8.2.09", "2.0.4").
    pub version: Option<String>,
    /// Short description if declared in comments or properties.
    pub description: Option<String>,
}

/// Maps Minecraft `pack_format` integer to known Minecraft release versions.
pub fn pack_format_to_mc_version(format: u32) -> Option<&'static str> {
    match format {
        1 => Some("1.6.1 - 1.8.9"),
        2 => Some("1.9 - 1.10.2"),
        3 => Some("1.11 - 1.12.2"),
        4 => Some("1.13 - 1.14.4"),
        5 => Some("1.15 - 1.16.1"),
        6 => Some("1.16.2 - 1.16.5"),
        7 => Some("1.17 - 1.17.1"),
        8 => Some("1.18 - 1.18.2"),
        9 => Some("1.19 - 1.19.2"),
        11 => Some("22w42a - 22w44a"),
        12 => Some("1.19.3"),
        13 => Some("1.19.4"),
        14 => Some("23w14a - 23w16a"),
        15 => Some("1.20 - 1.20.1"),
        18 => Some("1.20.2"),
        22 => Some("1.20.3 - 1.20.4"),
        32 => Some("1.20.5 - 1.20.6"),
        34 => Some("1.21 - 1.21.1"),
        42 => Some("1.21.2 - 1.21.3"),
        46 => Some("1.21.4"),
        48 => Some("1.21.4+"),
        _ => None,
    }
}

/// Checks if a token matches standard version formats like "1.4.2", "v8.2.09", "r5.1.1", "2.0.4-beta".
fn is_valid_version_token(token: &str) -> bool {
    let trimmed = token.trim_matches(|c: char| !c.is_ascii_alphanumeric() && c != '.' && c != '-' && c != '+');
    if trimmed.is_empty() {
        return false;
    }
    let body = trimmed
        .strip_prefix('v')
        .or_else(|| trimmed.strip_prefix('V'))
        .or_else(|| trimmed.strip_prefix('r'))
        .or_else(|| trimmed.strip_prefix('R'))
        .unwrap_or(trimmed);

    let parts: Vec<&str> = body.split('.').collect();
    if parts.len() < 2 {
        return false;
    }

    // First part must start with a digit
    if let Some(first) = parts.first() {
        if !first.chars().any(|c| c.is_ascii_digit()) {
            return false;
        }
    }
    // At least one digit in the body
    body.chars().any(|c| c.is_ascii_digit())
}

/// Normalizes and extracts a clean version token from a potential version substring.
fn clean_version_token(token: &str) -> Option<String> {
    let trimmed = token.trim_matches(|c: char| !c.is_ascii_alphanumeric() && c != '.' && c != '-' && c != '+');
    if is_valid_version_token(trimmed) {
        Some(trimmed.to_string())
    } else {
        None
    }
}

/// Extracts a version string from text (description, comments, headers).
/// Matches patterns like `v1.2.3`, `Version 2.0.1`, `r5.1.1`, `build 104`, etc.
pub fn extract_version_from_text(text: &str) -> Option<String> {
    // 1. Look for explicit "version" prefix
    let lower = text.to_ascii_lowercase();
    if let Some(idx) = lower.find("version") {
        let after = &text[idx + "version".len()..];
        let trimmed_after = after.trim_start_matches(|c: char| c == ':' || c == '-' || c == '=' || c.is_whitespace());
        let token = trimmed_after.split_whitespace().next().unwrap_or("");
        if let Some(v) = clean_version_token(token) {
            let bare = v.strip_prefix('v').or_else(|| v.strip_prefix('V')).unwrap_or(&v);
            return Some(bare.to_string());
        }
    }

    // 2. Scan whitespace-separated words
    for word in text.split_whitespace() {
        // Strip trailing punctuation like ',', ';', ')', ']'
        let token = word.trim_matches(|c: char| c == ',' || c == ';' || c == ')' || c == ']' || c == '(' || c == '[' || c == '"' || c == '\'');
        if (token.starts_with('v') || token.starts_with('V') || token.starts_with('r') || token.starts_with('R'))
            && is_valid_version_token(token)
        {
            return Some(token.to_string());
        }
    }

    // 3. Scan for any semver token (e.g. 1.2.3)
    for word in text.split_whitespace() {
        let token = word.trim_matches(|c: char| c == ',' || c == ';' || c == ')' || c == ']' || c == '(' || c == '[' || c == '"' || c == '\'');
        if is_valid_version_token(token) {
            return Some(token.to_string());
        }
    }

    None
}

/// Extracts a version token from a file name (e.g. `Faithful_32x_v1.4.2.zip` -> `v1.4.2`,
/// `ComplementaryReimagined_r5.1.1.zip` -> `r5.1.1`, `BSL_v8.2.09.zip` -> `v8.2.09`).
pub fn extract_version_from_filename(filename: &str) -> Option<String> {
    let clean_name = filename
        .strip_suffix(".zip")
        .or_else(|| filename.strip_suffix(".ZIP"))
        .unwrap_or(filename);

    // Split by common separators: '_', '-', ' ', '+'
    let segments: Vec<&str> = clean_name.split(|c| c == '_' || c == '-' || c == ' ' || c == '+').collect();

    // Scan segments in reverse (versions usually sit at the end of filenames)
    for seg in segments.iter().rev() {
        let trimmed = seg.trim();
        if (trimmed.starts_with('v') || trimmed.starts_with('V') || trimmed.starts_with('r') || trimmed.starts_with('R'))
            && is_valid_version_token(trimmed)
        {
            return Some(trimmed.to_string());
        }
        if is_valid_version_token(trimmed) {
            return Some(trimmed.to_string());
        }
    }

    None
}

/// Flattens a Minecraft JSON description component (which can be a string, an object `{"text": "..."}`,
/// or an array of text components) into a plain UTF-8 string.
fn flatten_description(value: &Value) -> Option<String> {
    match value {
        Value::String(s) => {
            let trimmed = s.trim();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed.to_string())
            }
        }
        Value::Object(map) => {
            let mut out = String::new();
            if let Some(Value::String(t)) = map.get("text") {
                out.push_str(t);
            }
            if let Some(Value::Array(extra)) = map.get("extra") {
                for item in extra {
                    if let Some(sub) = flatten_description(item) {
                        out.push_str(&sub);
                    }
                }
            }
            let trimmed = out.trim();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed.to_string())
            }
        }
        Value::Array(arr) => {
            let mut out = String::new();
            for item in arr {
                if let Some(sub) = flatten_description(item) {
                    out.push_str(&sub);
                }
            }
            let trimmed = out.trim();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed.to_string())
            }
        }
        _ => None,
    }
}

/// Parses the JSON content of a `pack.mcmeta` file.
pub fn parse_pack_mcmeta(
    content: &str,
    filename: Option<&str>,
) -> Result<ResourcePackMetadata, MetadataError> {
    let root: Value = serde_json::from_str(content)
        .map_err(|e| MetadataError::Invalid(format!("Malformed pack.mcmeta JSON: {e}")))?;

    let pack_obj = root.get("pack").unwrap_or(&root);

    // Extract pack_format
    let pack_format = pack_obj
        .get("pack_format")
        .and_then(|v| {
            if let Some(n) = v.as_u64() {
                Some(n as u32)
            } else if let Some(s) = v.as_str() {
                s.parse::<u32>().ok()
            } else {
                None
            }
        });

    // MC Compatibility
    let mc_compatibility = pack_format
        .and_then(pack_format_to_mc_version)
        .map(|s| s.to_string());

    // Description
    let description = pack_obj
        .get("description")
        .and_then(flatten_description);

    // Version determination
    // 1. Explicit `version` field in pack or root
    let explicit_version = pack_obj
        .get("version")
        .or_else(|| root.get("version"))
        .and_then(|v| v.as_str())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());

    let version = if let Some(v) = explicit_version {
        Some(v)
    } else if let Some(ref desc) = description {
        // 2. Extract from description if present
        extract_version_from_text(desc)
            .or_else(|| filename.and_then(extract_version_from_filename))
            .or_else(|| pack_format.map(|fmt| format!("v{fmt}")))
    } else {
        // 3. Extract from filename or format
        filename
            .and_then(extract_version_from_filename)
            .or_else(|| pack_format.map(|fmt| format!("v{fmt}")))
    };

    Ok(ResourcePackMetadata {
        pack_format,
        description,
        version,
        mc_compatibility,
    })
}

/// Parses the text of a `shaders.properties` or shader configuration file.
pub fn parse_shaders_properties(
    content: &str,
    filename: Option<&str>,
) -> Result<ShaderPackMetadata, MetadataError> {
    let mut explicit_version: Option<String> = None;
    let mut description_lines: Vec<String> = Vec::new();

    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        // Check property pairs: key=value
        if let Some((k, v)) = trimmed.split_once('=') {
            let key = k.trim().to_ascii_lowercase();
            let val = v.trim();
            if matches!(
                key.as_str(),
                "version"
                    | "shader.version"
                    | "pack.version"
                    | "profile.version"
                    | "shaders.version"
            ) && !val.is_empty()
            {
                explicit_version = Some(val.to_string());
                break;
            }
        }

        // Check comment lines for version info: # Version: 1.2.3, // Version 1.2.3
        if trimmed.starts_with('#') || trimmed.starts_with("//") {
            let comment_text = trimmed
                .trim_start_matches('#')
                .trim_start_matches('/')
                .trim();
            if explicit_version.is_none() {
                if let Some(v) = extract_version_from_text(comment_text) {
                    explicit_version = Some(v);
                }
            }
            if comment_text.to_ascii_lowercase().starts_with("description:") {
                let desc = comment_text["description:".len()..].trim().to_string();
                if !desc.is_empty() {
                    description_lines.push(desc);
                }
            }
        }
    }

    let version = explicit_version
        .or_else(|| filename.and_then(extract_version_from_filename))
        .or_else(|| extract_version_from_text(content));

    let description = if description_lines.is_empty() {
        None
    } else {
        Some(description_lines.join(" "))
    };

    Ok(ShaderPackMetadata {
        version,
        description,
    })
}

/// Extracts resource pack metadata from an archive file (.zip) or folder.
pub fn extract_resource_pack_metadata(path: &Path) -> Result<ResourcePackMetadata, MetadataError> {
    let filename = path
        .file_name()
        .map(|f| f.to_string_lossy().into_owned());

    if path.is_dir() {
        let mcmeta_file = path.join(PACK_MCMETA_ENTRY);
        if mcmeta_file.is_file() {
            let mut file = File::open(&mcmeta_file)?;
            let mut content = String::new();
            file.by_ref().take(MAX_METADATA_BYTES).read_to_string(&mut content)?;
            return parse_pack_mcmeta(&content, filename.as_deref());
        }
        // Folder without pack.mcmeta: extract what we can from filename
        let version = filename.as_deref().and_then(extract_version_from_filename);
        return Ok(ResourcePackMetadata {
            pack_format: None,
            description: None,
            version,
            mc_compatibility: None,
        });
    }

    let file = File::open(path)?;
    let mut zip = match zip::ZipArchive::new(file) {
        Ok(z) => z,
        Err(_) => {
            let version = filename.as_deref().and_then(extract_version_from_filename);
            return Ok(ResourcePackMetadata {
                pack_format: None,
                description: None,
                version,
                mc_compatibility: None,
            });
        }
    };

    // Locate pack.mcmeta inside zip
    for i in 0..zip.len() {
        if let Ok(mut entry) = zip.by_index(i) {
            let name = entry.name().to_ascii_lowercase();
            if name == PACK_MCMETA_ENTRY || name.ends_with("/pack.mcmeta") {
                let mut content = String::new();
                entry
                    .by_ref()
                    .take(MAX_METADATA_BYTES)
                    .read_to_string(&mut content)?;
                return parse_pack_mcmeta(&content, filename.as_deref());
            }
        }
    }

    // No pack.mcmeta found: extract version from filename
    let version = filename.as_deref().and_then(extract_version_from_filename);
    Ok(ResourcePackMetadata {
        pack_format: None,
        description: None,
        version,
        mc_compatibility: None,
    })
}

/// Extracts shaderpack metadata from an archive file (.zip) or folder.
pub fn extract_shader_pack_metadata(path: &Path) -> Result<ShaderPackMetadata, MetadataError> {
    let filename = path
        .file_name()
        .map(|f| f.to_string_lossy().into_owned());

    if path.is_dir() {
        let props_candidates = [
            path.join(SHADERS_PROPERTIES_ENTRY),
            path.join(SHADERS_PROPERTIES_ALT),
        ];
        for candidate in &props_candidates {
            if candidate.is_file() {
                let mut file = File::open(candidate)?;
                let mut content = String::new();
                file.by_ref().take(MAX_METADATA_BYTES).read_to_string(&mut content)?;
                return parse_shaders_properties(&content, filename.as_deref());
            }
        }
        let version = filename.as_deref().and_then(extract_version_from_filename);
        return Ok(ShaderPackMetadata {
            version,
            description: None,
        });
    }

    let file = File::open(path)?;
    let mut zip = match zip::ZipArchive::new(file) {
        Ok(z) => z,
        Err(_) => {
            let version = filename.as_deref().and_then(extract_version_from_filename);
            return Ok(ShaderPackMetadata {
                version,
                description: None,
            });
        }
    };

    // Look for shaders.properties or shaders/shaders.properties or .fsh headers
    for i in 0..zip.len() {
        if let Ok(mut entry) = zip.by_index(i) {
            let name = entry.name().to_ascii_lowercase();
            if name == SHADERS_PROPERTIES_ENTRY
                || name == SHADERS_PROPERTIES_ALT
                || name.ends_with("/shaders.properties")
            {
                let mut content = String::new();
                entry
                    .by_ref()
                    .take(MAX_METADATA_BYTES)
                    .read_to_string(&mut content)?;
                return parse_shaders_properties(&content, filename.as_deref());
            }
        }
    }

    // Inspect first shader source file comment header if available
    for i in 0..zip.len() {
        if let Ok(mut entry) = zip.by_index(i) {
            let name = entry.name().to_ascii_lowercase();
            if name.ends_with(".fsh") || name.ends_with(".vsh") {
                let mut content = String::new();
                entry
                    .by_ref()
                    .take(MAX_METADATA_BYTES)
                    .read_to_string(&mut content)?;
                if let Some(v) = extract_version_from_text(&content) {
                    return Ok(ShaderPackMetadata {
                        version: Some(v),
                        description: None,
                    });
                }
            }
        }
    }

    let version = filename.as_deref().and_then(extract_version_from_filename);
    Ok(ShaderPackMetadata {
        version,
        description: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_standard_pack_mcmeta() {
        let json = r#"{
            "pack": {
                "pack_format": 15,
                "description": "Faithful 32x for Minecraft 1.20 - v1.4.2"
            }
        }"#;
        let meta = parse_pack_mcmeta(json, Some("Faithful_32x.zip")).unwrap();
        assert_eq!(meta.pack_format, Some(15));
        assert_eq!(
            meta.description.as_deref(),
            Some("Faithful 32x for Minecraft 1.20 - v1.4.2")
        );
        assert_eq!(meta.version.as_deref(), Some("v1.4.2"));
        assert_eq!(meta.mc_compatibility.as_deref(), Some("1.20 - 1.20.1"));
    }

    #[test]
    fn parse_pack_mcmeta_with_raw_json_description() {
        let json = r#"{
            "pack": {
                "pack_format": 34,
                "description": {
                    "text": "Bare Bones Resource Pack ",
                    "extra": [
                        {"text": "Version: 2.1.0"}
                    ]
                }
            }
        }"#;
        let meta = parse_pack_mcmeta(json, Some("Bare_Bones.zip")).unwrap();
        assert_eq!(meta.pack_format, Some(34));
        assert_eq!(
            meta.description.as_deref(),
            Some("Bare Bones Resource Pack Version: 2.1.0")
        );
        assert_eq!(meta.version.as_deref(), Some("2.1.0"));
        assert_eq!(meta.mc_compatibility.as_deref(), Some("1.21 - 1.21.1"));
    }

    #[test]
    fn parse_pack_mcmeta_with_explicit_version_field() {
        let json = r#"{
            "pack": {
                "pack_format": 22,
                "version": "v3.0.0-beta",
                "description": "A customized pack"
            }
        }"#;
        let meta = parse_pack_mcmeta(json, None).unwrap();
        assert_eq!(meta.pack_format, Some(22));
        assert_eq!(meta.version.as_deref(), Some("v3.0.0-beta"));
        assert_eq!(meta.mc_compatibility.as_deref(), Some("1.20.3 - 1.20.4"));
    }

    #[test]
    fn parse_pack_mcmeta_fallback_to_filename_and_format() {
        let json = r#"{
            "pack": {
                "pack_format": 46,
                "description": "Simple texture pack with no version text"
            }
        }"#;
        let meta = parse_pack_mcmeta(json, Some("MyPack_v1.5.zip")).unwrap();
        assert_eq!(meta.pack_format, Some(46));
        assert_eq!(meta.version.as_deref(), Some("v1.5"));
        assert_eq!(meta.mc_compatibility.as_deref(), Some("1.21.4"));

        // No filename version either -> format fallback
        let meta_no_fn = parse_pack_mcmeta(json, Some("MyPack.zip")).unwrap();
        assert_eq!(meta_no_fn.version.as_deref(), Some("v46"));
    }

    #[test]
    fn parse_shaders_properties_file() {
        let content = r#"
# BSL Shaders v8.2.09
# Description: High performance shaderpack
version=v8.2.09
shadowMapResolution=2048
"#;
        let meta = parse_shaders_properties(content, Some("BSL.zip")).unwrap();
        assert_eq!(meta.version.as_deref(), Some("v8.2.09"));
        assert_eq!(
            meta.description.as_deref(),
            Some("High performance shaderpack")
        );
    }

    #[test]
    fn parse_shaders_properties_from_comments_and_filename() {
        let content = r#"
// Complementary Reimagined r5.1.1
// Designed for Iris and OptiFine
screen.MAIN=TRUE
"#;
        let meta = parse_shaders_properties(content, Some("ComplementaryReimagined_r5.1.1.zip")).unwrap();
        assert_eq!(meta.version.as_deref(), Some("r5.1.1"));
    }

    #[test]
    fn extract_version_from_filename_patterns() {
        assert_eq!(
            extract_version_from_filename("BSL_v8.2.09.zip"),
            Some("v8.2.09".to_string())
        );
        assert_eq!(
            extract_version_from_filename("ComplementaryReimagined_r5.1.1.zip"),
            Some("r5.1.1".to_string())
        );
        assert_eq!(
            extract_version_from_filename("SEUS-Renewed-v1.0.1.zip"),
            Some("v1.0.1".to_string())
        );
        assert_eq!(
            extract_version_from_filename("Faithful_32x_1.20_v1.4.2.zip"),
            Some("v1.4.2".to_string())
        );
        assert_eq!(
            extract_version_from_filename("Pack-2.1.0.zip"),
            Some("2.1.0".to_string())
        );
    }
}
