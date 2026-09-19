//! Mod metadata export, social sharing, and spec-compliant modpack formatting.
//!
//! Provides utilities to export a [`BillOfMaterials`] to:
//! - Standard Modrinth modpack index schema ([`ModrinthIndex`]) declaring remote CDN URLs.
//! - URL-safe Deflate+Base64 Share Codes (`zircon://setup/...`) for copy-pasting in Discord/chat.
//! - Formatted Markdown tables for documentation and community sharing.
//! - Diffing reports ([`ModDiffReport`]) between two BOMs.

use std::collections::HashMap;
use std::io::{Read, Write};

use base64::engine::general_purpose::{STANDARD, URL_SAFE, URL_SAFE_NO_PAD};
use base64::Engine;
use flate2::read::DeflateDecoder;
use flate2::write::DeflateEncoder;
use flate2::Compression;
use serde::{Deserialize, Serialize};

use crate::model::bom::{BillOfMaterials, ModEntry, ModSide};

pub mod snapshots;
pub use snapshots::*;

/// Canonical URL scheme prefix for 1-line Zircon Share Codes.
pub const SHARE_CODE_PREFIX: &str = "zircon://setup/";
pub const LEGACY_SHARE_CODE_PREFIX: &str = "zircon://mods/";

/// Standard Modrinth modpack index structure (`modrinth.index.json`, formatVersion = 1).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ModrinthIndex {
    pub format_version: u32,
    pub game: String,
    pub version_id: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
    pub dependencies: HashMap<String, String>,
    pub files: Vec<ModrinthIndexFile>,
}

/// A remote or overridden file entry in `modrinth.index.json`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ModrinthIndexFile {
    pub path: String,
    pub hashes: HashMap<String, String>,
    pub env: ModrinthIndexEnv,
    pub downloads: Vec<String>,
    #[serde(default)]
    pub file_size: u64,
}

/// Environment declaration for Modrinth index entries.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
pub struct ModrinthIndexEnv {
    pub client: String,
    pub server: String,
}

/// Structured diff between two [`BillOfMaterials`] instances.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
pub struct ModDiffReport {
    /// Mods present in incoming/target that are absent in current.
    pub to_add: Vec<ModEntry>,
    /// Mods present in both but with different versions, filenames, or hashes.
    pub to_update: Vec<ModUpdateEntry>,
    /// Mods present in both with identical filenames and hashes.
    pub unchanged: Vec<ModEntry>,
    /// Mods present in current that are absent in incoming/target.
    pub to_remove: Vec<ModEntry>,
}

impl ModDiffReport {
    pub fn is_identical(&self) -> bool {
        self.to_add.is_empty() && self.to_update.is_empty() && self.to_remove.is_empty()
    }
}

/// A pair of current and incoming mod entries representing a version upgrade/downgrade.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ModUpdateEntry {
    pub current: ModEntry,
    pub incoming: ModEntry,
}

/// Errors that can occur during export, compression, or decompression.
#[derive(Debug)]
pub enum ExportError {
    Io(std::io::Error),
    Base64(base64::DecodeError),
    Json(serde_json::Error),
    InvalidFormat(String),
}

impl std::fmt::Display for ExportError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(e) => write!(f, "I/O error during compression/decompression: {e}"),
            Self::Base64(e) => write!(f, "Base64 decoding failed: {e}"),
            Self::Json(e) => write!(f, "JSON serialization/deserialization failed: {e}"),
            Self::InvalidFormat(msg) => write!(f, "Invalid share code format: {msg}"),
        }
    }
}

impl std::error::Error for ExportError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(e) => Some(e),
            Self::Base64(e) => Some(e),
            Self::Json(e) => Some(e),
            Self::InvalidFormat(_) => None,
        }
    }
}

impl From<std::io::Error> for ExportError {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e)
    }
}

impl From<base64::DecodeError> for ExportError {
    fn from(e: base64::DecodeError) -> Self {
        Self::Base64(e)
    }
}

impl From<serde_json::Error> for ExportError {
    fn from(e: serde_json::Error) -> Self {
        Self::Json(e)
    }
}

/// Converts a [`BillOfMaterials`] into a spec-compliant [`ModrinthIndex`].
///
/// Mod entries with valid `download_url` values are mapped to remote `ModrinthIndexFile`
/// entries with SHA-1 hashes and client/server environment tags. Custom/local mods
/// without URLs should be bundled into `overrides/mods/` by the archive builder.
pub fn bom_to_modrinth_index(bom: &BillOfMaterials, pack_name: Option<&str>) -> ModrinthIndex {
    let mut dependencies = HashMap::new();
    dependencies.insert("minecraft".to_string(), bom.minecraft_version.clone());

    if let Some(loader) = &bom.mod_loader {
        let l_type = loader.r#type.to_ascii_lowercase();
        if !l_type.is_empty() && l_type != "vanilla" {
            let dep_key = match l_type.as_str() {
                "fabric" => "fabric-loader".to_string(),
                "quilt" => "quilt-loader".to_string(),
                "neoforge" => "neoforge".to_string(),
                "forge" => "forge".to_string(),
                other => other.to_string(),
            };
            dependencies.insert(dep_key, loader.version.clone());
        }
    }

    let mut files = Vec::new();
    for m in &bom.mods {
        if let Some(url) = &m.download_url {
            let mut hashes = HashMap::new();
            if let Some(sha1) = &m.sha1 {
                hashes.insert("sha1".to_string(), sha1.clone());
            }

            let env = match m.side {
                ModSide::Client => ModrinthIndexEnv {
                    client: "required".to_string(),
                    server: "unsupported".to_string(),
                },
                ModSide::Server => ModrinthIndexEnv {
                    client: "unsupported".to_string(),
                    server: "required".to_string(),
                },
                ModSide::Both => ModrinthIndexEnv {
                    client: "required".to_string(),
                    server: "required".to_string(),
                },
            };

            files.push(ModrinthIndexFile {
                path: format!("mods/{}", m.filename),
                hashes,
                env,
                downloads: vec![url.clone()],
                file_size: m.file_size,
            });
        }
    }

    let name = pack_name
        .map(String::from)
        .or_else(|| bom.server_title.clone())
        .unwrap_or_else(|| format!("Zircon Setup (MC {})", bom.minecraft_version));

    ModrinthIndex {
        format_version: 1,
        game: "minecraft".to_string(),
        version_id: uuid::Uuid::new_v4().to_string(),
        name,
        summary: Some(format!(
            "Exported from Zircon for Minecraft {}",
            bom.minecraft_version
        )),
        dependencies,
        files,
    }
}

/// Encodes a [`BillOfMaterials`] into a compact, URL-safe Base64 Deflate string with prefix `zircon://setup/`.
pub fn bom_to_share_code(bom: &BillOfMaterials) -> Result<String, ExportError> {
    let sanitized = sanitize_for_export(bom, None);
    let json_bytes = serde_json::to_vec(&sanitized)?;

    let mut encoder = DeflateEncoder::new(Vec::new(), Compression::best());
    encoder.write_all(&json_bytes)?;
    let compressed = encoder.finish()?;

    let encoded = URL_SAFE_NO_PAD.encode(&compressed);
    Ok(format!("{SHARE_CODE_PREFIX}{encoded}"))
}

/// Decodes a Zircon Share Code string into a full [`BillOfMaterials`].
///
/// Accepts prefixes `zircon://setup/`, `zircon://mods/`, or raw base64. Tolerates standard
/// or URL-safe base64 encoding, padding or unpadded.
pub fn bom_from_share_code(raw_code: &str) -> Result<BillOfMaterials, ExportError> {
    let trimmed = raw_code.trim();

    let stripped = if let Some(s) = trimmed.strip_prefix(SHARE_CODE_PREFIX) {
        s
    } else if let Some(s) = trimmed.strip_prefix(LEGACY_SHARE_CODE_PREFIX) {
        s
    } else if let Some(idx) = trimmed.find("://") {
        &trimmed[idx + 3..]
    } else {
        trimmed
    };

    let cleaned: String = stripped.chars().filter(|c| !c.is_whitespace()).collect();
    if cleaned.is_empty() {
        return Err(ExportError::InvalidFormat("Empty share code".to_string()));
    }

    // Try decoding with URL_SAFE_NO_PAD, fallback to standard or URL_SAFE
    let decoded_bytes = URL_SAFE_NO_PAD
        .decode(&cleaned)
        .or_else(|_| URL_SAFE.decode(&cleaned))
        .or_else(|_| STANDARD.decode(&cleaned))?;

    // Decompress Deflate
    let mut decoder = DeflateDecoder::new(&decoded_bytes[..]);
    let mut decompressed = Vec::new();
    if decoder.read_to_end(&mut decompressed).is_ok() && !decompressed.is_empty() {
        let bom: BillOfMaterials = serde_json::from_slice(&decompressed)?;
        return Ok(bom);
    }

    // Fallback: If decompression didn't succeed, check if the payload was uncompressed JSON
    if let Ok(bom) = serde_json::from_slice::<BillOfMaterials>(&decoded_bytes) {
        return Ok(bom);
    }

    Err(ExportError::InvalidFormat(
        "Could not decompress share code payload".to_string(),
    ))
}

/// Sanitizes a [`BillOfMaterials`] for external export or sharing.
///
/// Clears server-specific cryptographic signatures (`signature`, `server_public_key`)
/// and internal server branding hashes so the BOM can be safely shared across instances.
pub fn sanitize_for_export(bom: &BillOfMaterials, custom_title: Option<String>) -> BillOfMaterials {
    let mut clone = bom.clone();
    clone.signature = None;
    clone.server_public_key = None;

    if let Some(t) = custom_title {
        clone.server_title = Some(t);
    }

    if let Some(branding) = &mut clone.branding {
        branding.icon_sha1 = None;
        branding.banner_sha1 = None;
    }

    clone
}

/// Generates a GitHub and Discord-friendly Markdown table summarizing the mods in a BOM.
pub fn bom_to_markdown_table(bom: &BillOfMaterials) -> String {
    let title = bom
        .server_title
        .as_deref()
        .unwrap_or("Zircon Minecraft Setup");
    let loader_str = bom
        .mod_loader
        .as_ref()
        .map(|l| format!("{} {}", l.r#type, l.version))
        .unwrap_or_else(|| "Vanilla".to_string());

    let mut out = String::new();
    out.push_str(&format!(
        "### {title} ({} mods)\n\n",
        bom.mods.len()
    ));
    out.push_str(&format!(
        "**Minecraft:** `{}` • **Loader:** `{}`\n\n",
        bom.minecraft_version, loader_str
    ));

    out.push_str("| Mod | Version | Environment | Origin |\n");
    out.push_str("| :--- | :--- | :---: | :--- |\n");

    for m in &bom.mods {
        let mod_name = m.title.as_deref().unwrap_or(&m.filename);
        let name_cell = if let Some(url) = &m.project_url {
            format!("[{mod_name}]({url})")
        } else if let Some(url) = &m.download_url {
            format!("[{mod_name}]({url})")
        } else {
            mod_name.to_string()
        };

        let ver_cell = m
            .version
            .as_deref()
            .map(|v| format!("`{v}`"))
            .unwrap_or_else(|| "—".to_string());

        let env_cell = match m.side {
            ModSide::Client => "Client",
            ModSide::Server => "Server",
            ModSide::Both => "Client + Server",
        };

        let origin_cell = m.origin.as_deref().unwrap_or("direct");

        out.push_str(&format!(
            "| {name_cell} | {ver_cell} | {env_cell} | {origin_cell} |\n"
        ));
    }

    out
}

/// Compares two [`BillOfMaterials`] structures and categorizes differences.
pub fn diff_boms(current: &BillOfMaterials, incoming: &BillOfMaterials) -> ModDiffReport {
    let mut report = ModDiffReport::default();

    // Index current mods by (origin+id) or filename
    let mut current_by_key = HashMap::new();
    for m in &current.mods {
        let key = mod_match_key(m);
        current_by_key.insert(key, m);
    }

    let mut seen_keys = std::collections::HashSet::new();

    for inc in &incoming.mods {
        let key = mod_match_key(inc);
        seen_keys.insert(key.clone());

        if let Some(curr) = current_by_key.get(&key) {
            let same_file = curr.filename.eq_ignore_ascii_case(&inc.filename);
            let same_sha1 = match (&curr.sha1, &inc.sha1) {
                (Some(a), Some(b)) => a.eq_ignore_ascii_case(b),
                _ => true,
            };
            let same_version = match (&curr.version, &inc.version) {
                (Some(a), Some(b)) => a == b,
                _ => true,
            };

            if same_file && same_sha1 && same_version {
                report.unchanged.push((*curr).clone());
            } else {
                report.to_update.push(ModUpdateEntry {
                    current: (*curr).clone(),
                    incoming: inc.clone(),
                });
            }
        } else {
            report.to_add.push(inc.clone());
        }
    }

    for m in &current.mods {
        let key = mod_match_key(m);
        if !seen_keys.contains(&key) {
            report.to_remove.push(m.clone());
        }
    }

    report
}

fn mod_match_key(m: &ModEntry) -> String {
    if let (Some(origin), Some(id)) = (&m.origin, &m.id) {
        if !id.trim().is_empty() {
            return format!("{}:{}", origin.to_ascii_lowercase(), id.trim().to_ascii_lowercase());
        }
    }
    if let Some(slug) = &m.slug {
        if !slug.trim().is_empty() {
            return format!("slug:{}", slug.trim().to_ascii_lowercase());
        }
    }
    m.filename.to_ascii_lowercase()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::bom::{ModEntry, ModLoaderInfo, ModSide};

    fn make_sample_bom() -> BillOfMaterials {
        let mut bom = BillOfMaterials::new(
            "26.2",
            Some(ModLoaderInfo::new("fabric", "0.19.5", None)),
            Some("BeehiveSMP".to_string()),
        );

        let mut mod1 = ModEntry::new(
            Some("dKvj0eNn".to_string()),
            "create-fly-26.2.jar".to_string(),
            Some("fe6f561de7d3b13e384dce6482760f766cb00cc0".to_string()),
            1067914907,
            Some("modrinth".to_string()),
            None,
            21412089,
        );
        mod1.title = Some("Create Fly".to_string());
        mod1.download_url = Some("https://cdn.modrinth.com/create-fly.jar".to_string());
        mod1.side = ModSide::Both;
        mod1.version = Some("6.0.9-1".to_string());
        bom.mods.push(mod1);

        let mut mod2 = ModEntry::new(
            Some("AANobbMI".to_string()),
            "sodium-0.9.2.jar".to_string(),
            Some("62a70961a5b3e06938e06ab5f2dc4c5a51cf3323".to_string()),
            2046600026,
            Some("modrinth".to_string()),
            None,
            1885572,
        );
        mod2.title = Some("Sodium".to_string());
        mod2.download_url = Some("https://cdn.modrinth.com/sodium.jar".to_string());
        mod2.side = ModSide::Client;
        mod2.version = Some("0.9.2".to_string());
        bom.mods.push(mod2);

        bom
    }

    #[test]
    fn test_bom_to_modrinth_index() {
        let bom = make_sample_bom();
        let index = bom_to_modrinth_index(&bom, Some("Test Pack"));

        assert_eq!(1, index.format_version);
        assert_eq!("minecraft", index.game);
        assert_eq!("Test Pack", index.name);
        assert_eq!("26.2", index.dependencies.get("minecraft").unwrap());
        assert_eq!("0.19.5", index.dependencies.get("fabric-loader").unwrap());
        assert_eq!(2, index.files.len());

        let sodium = index
            .files
            .iter()
            .find(|f| f.path == "mods/sodium-0.9.2.jar")
            .unwrap();
        assert_eq!("required", sodium.env.client);
        assert_eq!("unsupported", sodium.env.server);
        assert_eq!(
            "62a70961a5b3e06938e06ab5f2dc4c5a51cf3323",
            sodium.hashes.get("sha1").unwrap()
        );
    }

    #[test]
    fn test_bom_share_code_roundtrip() {
        let bom = make_sample_bom();
        let share_code = bom_to_share_code(&bom).expect("encode");

        assert!(share_code.starts_with(SHARE_CODE_PREFIX));
        assert!(share_code.len() < 1000); // Compact

        let recovered = bom_from_share_code(&share_code).expect("decode");
        assert_eq!(bom.minecraft_version, recovered.minecraft_version);
        assert_eq!(bom.mod_loader, recovered.mod_loader);
        assert_eq!(bom.mods.len(), recovered.mods.len());
        assert_eq!(bom.mods[0].filename, recovered.mods[0].filename);
        assert_eq!(bom.mods[1].side, recovered.mods[1].side);
    }

    #[test]
    fn test_diff_boms() {
        let current = make_sample_bom();
        let mut incoming = make_sample_bom();

        // Add a mod to incoming
        let mod3 = ModEntry::new(
            Some("Iris".to_string()),
            "iris-1.11.4.jar".to_string(),
            None,
            0,
            Some("modrinth".to_string()),
            Some("YL57xq9U".to_string()),
            2821616,
        );
        incoming.mods.push(mod3);

        // Update Create Fly in incoming
        incoming.mods[0].filename = "create-fly-6.1.0.jar".to_string();
        incoming.mods[0].version = Some("6.1.0".to_string());

        let diff = diff_boms(&current, &incoming);
        assert_eq!(1, diff.to_add.len());
        assert_eq!("iris-1.11.4.jar", diff.to_add[0].filename);

        assert_eq!(1, diff.to_update.len());
        assert_eq!("create-fly-26.2.jar", diff.to_update[0].current.filename);
        assert_eq!("create-fly-6.1.0.jar", diff.to_update[0].incoming.filename);

        assert_eq!(1, diff.unchanged.len());
        assert_eq!("sodium-0.9.2.jar", diff.unchanged[0].filename);

        assert!(diff.to_remove.is_empty());
    }

    #[test]
    fn test_bom_to_markdown_table() {
        let bom = make_sample_bom();
        let markdown = bom_to_markdown_table(&bom);

        assert!(markdown.contains("BeehiveSMP"));
        assert!(markdown.contains("Create Fly"));
        assert!(markdown.contains("Sodium"));
        assert!(markdown.contains("Client + Server"));
        assert!(markdown.contains("Client"));
    }
}
