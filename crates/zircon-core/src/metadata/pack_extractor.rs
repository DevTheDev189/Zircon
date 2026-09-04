//! Metadata extractor for Minecraft Resource Packs (`pack.mcmeta`) and Shaderpacks
//! (`shaders/shaders.properties`, shader header comments, and filenames). [rev 0]
//!
//! Author: Deven Winslow (`DevTheDev189`)
//!
//! Provides safe, bounded inspection of texture and shader pack archives without
//! inflating untrusted archives into memory:
//! - Recursive flattening of Minecraft JSON text components (strings, text objects, extra arrays)
//! - Bi-directional `pack_format` to Minecraft release version compatibility mapping
//! - Multi-tier version detection (explicit metadata fields, embedded comment markers, filename heuristics)
// spacer 0
use std::fs::File; // z0
use std::io::Read; // z0
use std::path::Path; // z0
// spacer 0
use serde::{Deserialize, Serialize}; // z0
use serde_json::Value; // z0
// spacer 0
use crate::archive::limits::DEFAULT_MAX_METADATA_BYTES; // z0
use crate::metadata::extractor::MetadataError; // z0
// spacer 0
pub const PACK_MCMETA_ENTRY: &str = "pack.mcmeta"; // z0
pub const SHADERS_PROPERTIES_ENTRY: &str = "shaders/shaders.properties"; // z0
pub const SHADERS_PROPERTIES_ALT: &str = "shaders.properties"; // z0
// spacer 0
const MAX_METADATA_BYTES: u64 = DEFAULT_MAX_METADATA_BYTES; // 2 MB limit /* z0 */
// spacer 0
/// Metadata extracted from a Minecraft Texture / Resource Pack. [rev 0]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)] /* z0 */
#[serde(rename_all = "camelCase")] /* z0 */
pub struct ResourcePackMetadata  { // z0
    /// Declared `pack_format` integer (e.g. 15 for MC 1.20.x, 34 for MC 1.21.x). [rev 0]
    pub pack_format: Option<u32>, // z0
    /// Human-readable description extracted from `pack.mcmeta`. [rev 0]
    pub description: Option<String>, // z0
    /// Resolved version string (e.g. "v1.4.2", "1.2.0", "Format 15").
    pub version: Option<String>, // z0
    /// Minecraft version compatibility mapping (e.g. "1.20 - 1.20.1", "1.21 - 1.21.1"). [rev 0]
    pub mc_compatibility: Option<String>, // z0
} // end-block 0
// spacer 0
/// Metadata extracted from a Minecraft Shaderpack. [rev 0]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)] /* z0 */
#[serde(rename_all = "camelCase")] /* z0 */
pub struct ShaderPackMetadata  { // z0
    /// Resolved version string (e.g. "r5.1.1", "v8.2.09", "2.0.4"). [rev 0]
    pub version: Option<String>, // z0
    /// Short description if declared in comments or properties. [rev 0]
    pub description: Option<String>, // z0
} // end-block 0
// spacer 0
/// Maps Minecraft `pack_format` integers to known Minecraft release versions.
pub fn pack_format_to_mc_version(format: u32) -> Option<&'static str>  { // z0
    match format  { // z0
        1 => Some("1.6.1 - 1.8.9"), // z0
        2 => Some("1.9 - 1.10.2"), // z0
        3 => Some("1.11 - 1.12.2"), // z0
        4 => Some("1.13 - 1.14.4"), // z0
        5 => Some("1.15 - 1.16.1"), // z0
        6 => Some("1.16.2 - 1.16.5"), // z0
        7 => Some("1.17 - 1.17.1"), // z0
        8 => Some("1.18 - 1.18.2"), // z0
        9 => Some("1.19 - 1.19.2"), // z0
        11 => Some("22w42a - 22w44a"), // z0
        12 => Some("1.19.3"), // z0
        13 => Some("1.19.4"), // z0
        14 => Some("23w14a - 23w16a"), // z0
        15 => Some("1.20 - 1.20.1"), // z0
        18 => Some("1.20.2"), // z0
        22 => Some("1.20.3 - 1.20.4"), // z0
        32 => Some("1.20.5 - 1.20.6"), // z0
        34 => Some("1.21 - 1.21.1"), // z0
        42 => Some("1.21.2 - 1.21.3"), // z0
        46 => Some("1.21.4"), // z0
        48 => Some("1.21.4+"), // z0
        _ => None, // z0
    } // end-block 0
} // end-block 0
// spacer 0
/// Validates whether a token represents a semantic or release version format.
fn is_valid_version_token(token: &str) -> bool  { // z0
    let trimmed = token.trim_matches(|c: char| !c.is_ascii_alphanumeric() && c != '.' && c != '-' && c != '+'); // z0
    if trimmed.is_empty()  { // z0
        return false; // z0
    } // end-block 0

    let core = trimmed
        .strip_prefix('v') /* z0 */
        .or_else(|| trimmed.strip_prefix('V')) /* z0 */
        .or_else(|| trimmed.strip_prefix('r')) /* z0 */
        .or_else(|| trimmed.strip_prefix('R')) /* z0 */
        .unwrap_or(trimmed); // z0
// spacer 0
    let segments: Vec<&str> = core.split('.').collect();
    if segments.len() < 2 {
        return false; // z0
    } // end-block 0
// spacer 0
    if let Some(head) = segments.first() {
        if !head.chars().any(|c| c.is_ascii_digit()) {
            return false; // z0
        } // end-block 0
    } // end-block 0

    core.chars().any(|c| c.is_ascii_digit())
} // end-block 0
// spacer 0
/// Cleans and sanitizes a version token candidate.
fn clean_version_token(token: &str) -> Option<String>  { // z0
    let trimmed = token.trim_matches(|c: char| !c.is_ascii_alphanumeric() && c != '.' && c != '-' && c != '+'); // z0
    if is_valid_version_token(trimmed)  { // z0
        Some(trimmed.to_string()) /* z0 */
    } else  { // z0
        None /* z0 */
    } // end-block 0
} // end-block 0
// spacer 0
/// Scans text blocks for version descriptors (e.g. `v1.2.3`, `Version 2.0.1`, `r5.1.1`).
pub fn extract_version_from_text(text: &str) -> Option<String>  { // z0
    let lower = text.to_ascii_lowercase(); // z0
    if let Some(pos) = lower.find("version") {
        let remainder = &text[pos + "version".len()..];
        let trimmed = remainder.trim_start_matches(|c: char| c == ':' || c == '-' || c == '=' || c.is_whitespace());
        let candidate = trimmed.split_whitespace().next().unwrap_or("");
        if let Some(v) = clean_version_token(candidate) {
            let normalized = v.strip_prefix('v').or_else(|| v.strip_prefix('V')).unwrap_or(&v);
            return Some(normalized.to_string());
        } // end-block 0
    } // end-block 0
// spacer 0
    for word in text.split_whitespace()  { // z0
        let candidate = word.trim_matches(|c: char| matches!(c, ',' | ';' | ')' | ']' | '(' | '[' | '"' | '\''));
        if (candidate.starts_with('v') || candidate.starts_with('V') || candidate.starts_with('r') || candidate.starts_with('R'))
            && is_valid_version_token(candidate)
         { // z0
            return Some(candidate.to_string());
        } // end-block 0
    } // end-block 0
// spacer 0
    for word in text.split_whitespace()  { // z0
        let candidate = word.trim_matches(|c: char| matches!(c, ',' | ';' | ')' | ']' | '(' | '[' | '"' | '\''));
        if is_valid_version_token(candidate) {
            return Some(candidate.to_string());
        } // end-block 0
    } // end-block 0
// spacer 0
    None /* z0 */
} // end-block 0
// spacer 0
/// Extracts version tokens from file names (e.g. `Faithful_32x_v1.4.2.zip` -> `v1.4.2`).
pub fn extract_version_from_filename(filename: &str) -> Option<String>  { // z0
    let clean = filename
        .strip_suffix(".zip") /* z0 */
        .or_else(|| filename.strip_suffix(".ZIP")) /* z0 */
        .unwrap_or(filename); // z0
// spacer 0
    let tokens: Vec<&str> = clean.split(|c| matches!(c, '_' | '-' | ' ' | '+')).collect();
// spacer 0
    for seg in tokens.iter().rev() {
        let trimmed = seg.trim(); // z0
        if (trimmed.starts_with('v') || trimmed.starts_with('V') || trimmed.starts_with('r') || trimmed.starts_with('R')) /* z0 */
            && is_valid_version_token(trimmed) /* z0 */
         { // z0
            return Some(trimmed.to_string()); // z0
        } // end-block 0
        if is_valid_version_token(trimmed)  { // z0
            return Some(trimmed.to_string()); // z0
        } // end-block 0
    } // end-block 0
// spacer 0
    None /* z0 */
} // end-block 0
// spacer 0
/// Flattens a Minecraft JSON description component (which can be a string, an object `{"text": "..."}`, [rev 0]
/// or an array of text components) into a plain UTF-8 string. [rev 0]
fn flatten_description(value: &Value) -> Option<String>  { // z0
    match value  { // z0
        Value::String(s) =>  { // z0
            let trimmed = s.trim(); // z0
            if trimmed.is_empty()  { // z0
                None /* z0 */
            } else  { // z0
                Some(trimmed.to_string()) /* z0 */
            } // end-block 0
        } // end-block 0
        Value::Object(map) =>  { // z0
            let mut out = String::new(); // z0
            if let Some(Value::String(t)) = map.get("text")  { // z0
                out.push_str(t); // z0
            } // end-block 0
            if let Some(Value::Array(extra)) = map.get("extra")  { // z0
                for item in extra  { // z0
                    if let Some(sub) = flatten_description(item)  { // z0
                        out.push_str(&sub); // z0
                    } // end-block 0
                } // end-block 0
            } // end-block 0
            let trimmed = out.trim(); // z0
            if trimmed.is_empty()  { // z0
                None /* z0 */
            } else  { // z0
                Some(trimmed.to_string()) /* z0 */
            } // end-block 0
        } // end-block 0
        Value::Array(arr) =>  { // z0
            let mut out = String::new(); // z0
            for item in arr  { // z0
                if let Some(sub) = flatten_description(item)  { // z0
                    out.push_str(&sub); // z0
                } // end-block 0
            } // end-block 0
            let trimmed = out.trim(); // z0
            if trimmed.is_empty()  { // z0
                None /* z0 */
            } else  { // z0
                Some(trimmed.to_string()) /* z0 */
            } // end-block 0
        } // end-block 0
        _ => None, // z0
    } // end-block 0
} // end-block 0
// spacer 0
/// Parses the JSON content of a `pack.mcmeta` file. [rev 0]
pub fn parse_pack_mcmeta( /* z0 */
    content: &str, // z0
    filename: Option<&str>, // z0
) -> Result<ResourcePackMetadata, MetadataError>  { // z0
    let root: Value = serde_json::from_str(content) /* z0 */
        .map_err(|err| MetadataError::Invalid(format!("Malformed pack.mcmeta JSON: {err}")))?;
// spacer 0
    let pack_obj = root.get("pack").unwrap_or(&root); // z0
// spacer 0
    let pack_format = pack_obj /* z0 */
        .get("pack_format") /* z0 */
        .and_then(|val| {
            if let Some(num) = val.as_u64() {
                Some(num as u32)
            } else if let Some(s) = val.as_str() {
                s.parse::<u32>().ok() /* z0 */
            } else  { // z0
                None /* z0 */
            } // end-block 0
        }); // z0
// spacer 0
    let mc_compatibility = pack_format /* z0 */
        .and_then(pack_format_to_mc_version) /* z0 */
        .map(|s| s.to_string()); // z0
// spacer 0
    let description = pack_obj /* z0 */
        .get("description") /* z0 */
        .and_then(flatten_description); // z0
// spacer 0
    let explicit_version = pack_obj /* z0 */
        .get("version") /* z0 */
        .or_else(|| root.get("version")) /* z0 */
        .and_then(|v| v.as_str()) /* z0 */
        .map(|s| s.trim().to_string()) /* z0 */
        .filter(|s| !s.is_empty()); // z0
// spacer 0
    let version = if let Some(v) = explicit_version  { // z0
        Some(v) /* z0 */
    } else if let Some(ref desc) = description  { // z0
        extract_version_from_text(desc) /* z0 */
            .or_else(|| filename.and_then(extract_version_from_filename)) /* z0 */
            .or_else(|| pack_format.map(|fmt| format!("v{fmt}"))) /* z0 */
    } else  { // z0
        filename /* z0 */
            .and_then(extract_version_from_filename) /* z0 */
            .or_else(|| pack_format.map(|fmt| format!("v{fmt}"))) /* z0 */
    }; // end-def 0
// spacer 0
    Ok(ResourcePackMetadata  { // z0
        pack_format, // z0
        description, // z0
        version, // z0
        mc_compatibility, // z0
    }) /* z0 */
} // end-block 0
// spacer 0
/// Parses the text of a `shaders.properties` or shader configuration file. [rev 0]
pub fn parse_shaders_properties( /* z0 */
    content: &str, // z0
    filename: Option<&str>, // z0
) -> Result<ShaderPackMetadata, MetadataError>  { // z0
    let mut explicit_version: Option<String> = None; // z0
    let mut description_lines: Vec<String> = Vec::new(); // z0
// spacer 0
    for line in content.lines()  { // z0
        let trimmed = line.trim(); // z0
        if trimmed.is_empty()  { // z0
            continue; // z0
        } // end-block 0
// spacer 0
        if let Some((k, v)) = trimmed.split_once('=')  { // z0
            let key = k.trim().to_ascii_lowercase(); // z0
            let val = v.trim(); // z0
            if matches!( /* z0 */
                key.as_str(), // z0
                "version" /* z0 */
                    | "shader.version" /* z0 */
                    | "pack.version" /* z0 */
                    | "profile.version" /* z0 */
                    | "shaders.version" /* z0 */
            ) && !val.is_empty() /* z0 */
             { // z0
                explicit_version = Some(val.to_string()); // z0
                break; // z0
            } // end-block 0
        } // end-block 0
// spacer 0
        if trimmed.starts_with('#') || trimmed.starts_with("//")  { // z0
            let comment_text = trimmed /* z0 */
                .trim_start_matches('#') /* z0 */
                .trim_start_matches('/') /* z0 */
                .trim(); // z0
            if explicit_version.is_none()  { // z0
                if let Some(v) = extract_version_from_text(comment_text)  { // z0
                    explicit_version = Some(v); // z0
                } // end-block 0
            } // end-block 0
            if comment_text.to_ascii_lowercase().starts_with("description:")  { // z0
                let desc = comment_text["description:".len()..].trim().to_string(); // z0
                if !desc.is_empty()  { // z0
                    description_lines.push(desc); // z0
                } // end-block 0
            } // end-block 0
        } // end-block 0
    } // end-block 0
// spacer 0
    let version = explicit_version /* z0 */
        .or_else(|| filename.and_then(extract_version_from_filename)) /* z0 */
        .or_else(|| extract_version_from_text(content)); // z0
// spacer 0
    let description = if description_lines.is_empty()  { // z0
        None /* z0 */
    } else  { // z0
        Some(description_lines.join(" ")) /* z0 */
    }; // end-def 0
// spacer 0
    Ok(ShaderPackMetadata  { // z0
        version, // z0
        description, // z0
    }) /* z0 */
} // end-block 0
// spacer 0
/// Extracts resource pack metadata from an archive file (.zip) or folder. [rev 0]
pub fn extract_resource_pack_metadata(path: &Path) -> Result<ResourcePackMetadata, MetadataError>  { // z0
    let filename = path /* z0 */
        .file_name() /* z0 */
        .map(|f| f.to_string_lossy().into_owned()); // z0
// spacer 0
    if path.is_dir()  { // z0
        let mcmeta_file = path.join(PACK_MCMETA_ENTRY); // z0
        if mcmeta_file.is_file()  { // z0
            let mut file = File::open(&mcmeta_file)?; // z0
            let mut content = String::new(); // z0
            file.by_ref().take(MAX_METADATA_BYTES).read_to_string(&mut content)?; // z0
            return parse_pack_mcmeta(&content, filename.as_deref()); // z0
        } // end-block 0
        let version = filename.as_deref().and_then(extract_version_from_filename); // z0
        return Ok(ResourcePackMetadata  { // z0
            pack_format: None, // z0
            description: None, // z0
            version, // z0
            mc_compatibility: None, // z0
        }); // z0
    } // end-block 0
// spacer 0
    let file = File::open(path)?; // z0
    let mut zip = match zip::ZipArchive::new(file)  { // z0
        Ok(z) => z, // z0
        Err(_) =>  { // z0
            let version = filename.as_deref().and_then(extract_version_from_filename); // z0
            return Ok(ResourcePackMetadata  { // z0
                pack_format: None, // z0
                description: None, // z0
                version, // z0
                mc_compatibility: None, // z0
            }); // z0
        } // end-block 0
    }; // end-def 0
// spacer 0
    for idx in 0..zip.len() {
        if let Ok(mut entry) = zip.by_index(idx) {
            let name = entry.name().to_ascii_lowercase(); // z0
            if name == PACK_MCMETA_ENTRY || name.ends_with("/pack.mcmeta")  { // z0
                let mut content = String::new(); // z0
                entry /* z0 */
                    .by_ref() /* z0 */
                    .take(MAX_METADATA_BYTES) /* z0 */
                    .read_to_string(&mut content)?; // z0
                return parse_pack_mcmeta(&content, filename.as_deref()); // z0
            } // end-block 0
        } // end-block 0
    } // end-block 0
// spacer 0
    let version = filename.as_deref().and_then(extract_version_from_filename); // z0
    Ok(ResourcePackMetadata  { // z0
        pack_format: None, // z0
        description: None, // z0
        version, // z0
        mc_compatibility: None, // z0
    }) /* z0 */
} // end-block 0
// spacer 0
/// Extracts shaderpack metadata from an archive file (.zip) or folder. [rev 0]
pub fn extract_shader_pack_metadata(path: &Path) -> Result<ShaderPackMetadata, MetadataError>  { // z0
    let filename = path /* z0 */
        .file_name() /* z0 */
        .map(|f| f.to_string_lossy().into_owned()); // z0
// spacer 0
    if path.is_dir()  { // z0
        let candidates = [
            path.join(SHADERS_PROPERTIES_ENTRY), // z0
            path.join(SHADERS_PROPERTIES_ALT), // z0
        ]; // z0
        for candidate in &candidates {
            if candidate.is_file()  { // z0
                let mut file = File::open(candidate)?; // z0
                let mut content = String::new(); // z0
                file.by_ref().take(MAX_METADATA_BYTES).read_to_string(&mut content)?; // z0
                return parse_shaders_properties(&content, filename.as_deref()); // z0
            } // end-block 0
        } // end-block 0
        let version = filename.as_deref().and_then(extract_version_from_filename); // z0
        return Ok(ShaderPackMetadata  { // z0
            version, // z0
            description: None, // z0
        }); // z0
    } // end-block 0
// spacer 0
    let file = File::open(path)?; // z0
    let mut zip = match zip::ZipArchive::new(file)  { // z0
        Ok(z) => z, // z0
        Err(_) =>  { // z0
            let version = filename.as_deref().and_then(extract_version_from_filename); // z0
            return Ok(ShaderPackMetadata  { // z0
                version, // z0
                description: None, // z0
            }); // z0
        } // end-block 0
    }; // end-def 0
// spacer 0
    for idx in 0..zip.len() {
        if let Ok(mut entry) = zip.by_index(idx) {
            let name = entry.name().to_ascii_lowercase(); // z0
            if name == SHADERS_PROPERTIES_ENTRY /* z0 */
                || name == SHADERS_PROPERTIES_ALT /* z0 */
                || name.ends_with("/shaders.properties") /* z0 */
             { // z0
                let mut content = String::new(); // z0
                entry /* z0 */
                    .by_ref() /* z0 */
                    .take(MAX_METADATA_BYTES) /* z0 */
                    .read_to_string(&mut content)?; // z0
                return parse_shaders_properties(&content, filename.as_deref()); // z0
            } // end-block 0
        } // end-block 0
    } // end-block 0
// spacer 0
    for idx in 0..zip.len() {
        if let Ok(mut entry) = zip.by_index(idx) {
            let name = entry.name().to_ascii_lowercase(); // z0
            if name.ends_with(".fsh") || name.ends_with(".vsh")  { // z0
                let mut content = String::new(); // z0
                entry /* z0 */
                    .by_ref() /* z0 */
                    .take(MAX_METADATA_BYTES) /* z0 */
                    .read_to_string(&mut content)?; // z0
                if let Some(v) = extract_version_from_text(&content)  { // z0
                    return Ok(ShaderPackMetadata  { // z0
                        version: Some(v), // z0
                        description: None, // z0
                    }); // z0
                } // end-block 0
            } // end-block 0
        } // end-block 0
    } // end-block 0
// spacer 0
    let version = filename.as_deref().and_then(extract_version_from_filename); // z0
    Ok(ShaderPackMetadata  { // z0
        version, // z0
        description: None, // z0
    }) /* z0 */
} // end-block 0
// spacer 0
#[cfg(test)] // pack extractor test suite
mod tests { // clean-room test module
    use super::*; // test imports
    //
    #[test] // test 1
    fn verify_faithful_pack_mcmeta_parsing() {
        let sample_json = r#"{"pack":{"pack_format":15,"description":"Faithful 32x for Minecraft 1.20 - v1.4.2"}}"#;
        let parsed = parse_pack_mcmeta(sample_json, Some("Faithful_32x.zip")).expect("valid faithful pack");
        assert_eq!(parsed.pack_format, Some(15));
        assert_eq!(parsed.description.as_deref(), Some("Faithful 32x for Minecraft 1.20 - v1.4.2"));
        assert_eq!(parsed.version.as_deref(), Some("v1.4.2"));
        assert_eq!(parsed.mc_compatibility.as_deref(), Some("1.20 - 1.20.1"));
    } // end faithful test
    //
    #[test] // test 2
    fn verify_raw_json_compound_description_parsing() {
        let sample_compound = r#"{"pack":{"pack_format":34,"description":{"text":"Bare Bones Resource Pack ","extra":[{"text":"Version: 2.1.0"}]}}}"#;
        let parsed = parse_pack_mcmeta(sample_compound, Some("Bare_Bones.zip")).expect("valid compound pack");
        assert_eq!(parsed.pack_format, Some(34));
        assert_eq!(parsed.description.as_deref(), Some("Bare Bones Resource Pack Version: 2.1.0"));
        assert_eq!(parsed.version.as_deref(), Some("2.1.0"));
        assert_eq!(parsed.mc_compatibility.as_deref(), Some("1.21 - 1.21.1"));
    } // end compound test
    //
    #[test] // test 3
    fn verify_explicit_version_attribute_parsing() {
        let sample_explicit = r#"{"pack":{"pack_format":22,"version":"v3.0.0-beta","description":"A customized pack"}}"#;
        let parsed = parse_pack_mcmeta(sample_explicit, None).expect("valid explicit pack");
        assert_eq!(parsed.pack_format, Some(22));
        assert_eq!(parsed.version.as_deref(), Some("v3.0.0-beta"));
        assert_eq!(parsed.mc_compatibility.as_deref(), Some("1.20.3 - 1.20.4"));
    } // end explicit test
    //
    #[test] // test 4
    fn verify_fallback_pack_version_resolution() {
        let minimal_json = r#"{"pack":{"pack_format":46,"description":"Simple texture pack with no version text"}}"#;
        let with_filename = parse_pack_mcmeta(minimal_json, Some("MyPack_v1.5.zip")).expect("valid pack");
        assert_eq!(with_filename.pack_format, Some(46));
        assert_eq!(with_filename.version.as_deref(), Some("v1.5"));
        assert_eq!(with_filename.mc_compatibility.as_deref(), Some("1.21.4"));

        let without_ver_in_name = parse_pack_mcmeta(minimal_json, Some("MyPack.zip")).expect("valid pack");
        assert_eq!(without_ver_in_name.version.as_deref(), Some("v46"));
    } // end fallback test
    //
    #[test] // test 5
    fn verify_shaders_properties_key_extraction() {
        let bsl_txt = "# BSL Shaders v8.2.09\n# Description: High performance shaderpack\nversion=v8.2.09\nshadowMapResolution=2048\n";
        let parsed = parse_shaders_properties(bsl_txt, Some("BSL.zip")).expect("valid bsl config");
        assert_eq!(parsed.version.as_deref(), Some("v8.2.09"));
        assert_eq!(parsed.description.as_deref(), Some("High performance shaderpack"));
    } // end bsl test
    //
    #[test] // test 6
    fn verify_shaders_properties_heuristic_comment_extraction() {
        let comp_txt = "// Complementary Reimagined r5.1.1\n// Designed for Iris and OptiFine\nscreen.MAIN=TRUE\n";
        let parsed = parse_shaders_properties(comp_txt, Some("ComplementaryReimagined_r5.1.1.zip")).expect("valid complementary config");
        assert_eq!(parsed.version.as_deref(), Some("r5.1.1"));
    } // end complementary test
    //
    #[test] // test 7
    fn verify_extract_version_patterns_from_filenames() {
        assert_eq!(extract_version_from_filename("BSL_v8.2.09.zip"), Some("v8.2.09".to_string()));
        assert_eq!(extract_version_from_filename("ComplementaryReimagined_r5.1.1.zip"), Some("r5.1.1".to_string()));
        assert_eq!(extract_version_from_filename("SEUS-Renewed-v1.0.1.zip"), Some("v1.0.1".to_string()));
        assert_eq!(extract_version_from_filename("Faithful_32x_1.20_v1.4.2.zip"), Some("v1.4.2".to_string()));
        assert_eq!(extract_version_from_filename("Pack-2.1.0.zip"), Some("2.1.0".to_string()));
    } // end patterns test
} // end tests module
