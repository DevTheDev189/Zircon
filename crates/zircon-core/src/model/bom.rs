//! The "Bill of Materials" (BOM) published by the server manager and consumed
//! by the client launcher. Pins the exact Minecraft version, mod loader and the
//! authoritative list of mods a client must install to join the server.
//!
//! Port of `com.mcmanager.core.model.BillOfMaterials` / `ModEntry` /
//! `PackEntry` / `ModLoaderInfo`. JSON schema is camelCase, e.g.
//! `{"schemaVersion":1,"minecraftVersion":"1.20.4","modLoader":{...},"mods":[...]}`.

use serde::{Deserialize, Serialize};

use crate::CURRENT_SCHEMA_VERSION;

/// The "Bill of Materials" that the server manager publishes and the client
/// launcher consumes.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct BillOfMaterials {
    #[serde(default = "default_schema_version")]
    pub schema_version: i32,
    pub minecraft_version: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mod_loader: Option<ModLoaderInfo>,
    #[serde(default)]
    pub mods: Vec<ModEntry>,
    #[serde(default)]
    pub configs: Vec<ConfigFileEntry>,
    #[serde(default)]
    pub shaderpacks: Vec<PackEntry>,
    #[serde(default)]
    pub resourcepacks: Vec<PackEntry>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub server_title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub branding: Option<ServerBranding>,

    // --- Cryptographic Attestation Fields ---
    // Set by the server on every disk write (Ed25519): `server_public_key` is
    // the hex-encoded public key the launcher pins on first use (TOFU) and
    // `signature` covers the canonical digest of the content above. Both are
    // stripped from the digest itself, so adding/refreshing them never changes
    // what was signed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub signature: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub server_public_key: Option<String>,
}

fn default_schema_version() -> i32 {
    CURRENT_SCHEMA_VERSION
}

impl BillOfMaterials {
    pub fn new(
        minecraft_version: impl Into<String>,
        mod_loader: Option<ModLoaderInfo>,
        server_title: Option<String>,
    ) -> Self {
        Self {
            schema_version: CURRENT_SCHEMA_VERSION,
            minecraft_version: minecraft_version.into(),
            mod_loader,
            mods: Vec::new(),
            configs: Vec::new(),
            shaderpacks: Vec::new(),
            resourcepacks: Vec::new(),
            server_title,
            branding: None,
            signature: None,
            server_public_key: None,
        }
    }

    pub fn add_mod(&mut self, entry: ModEntry) {
        self.mods.retain(|m| {
            m.filename != entry.filename
                && !(entry.id.is_some() && entry.id == m.id && entry.origin == m.origin)
        });
        self.mods.push(entry);
    }

    /// Deduplicates mods by filename and project ID (origin + id), retaining
    /// the most recently added / updated entry and preserving stable order.
    pub fn deduplicate_mods(&mut self) {
        let mut seen_filenames = std::collections::HashSet::new();
        let mut seen_projects = std::collections::HashSet::new();
        let mut unique_mods = Vec::new();

        for entry in self.mods.iter().rev() {
            let file_key = entry.filename.to_ascii_lowercase();
            if seen_filenames.contains(&file_key) {
                continue;
            }
            if let (Some(origin), Some(id)) = (&entry.origin, &entry.id) {
                let id_trimmed = id.trim();
                if !id_trimmed.is_empty() {
                    let proj_key = format!("{}:{}", origin.to_ascii_lowercase(), id_trimmed.to_ascii_lowercase());
                    if seen_projects.contains(&proj_key) {
                        continue;
                    }
                    seen_projects.insert(proj_key);
                }
            }
            seen_filenames.insert(file_key);
            unique_mods.push(entry.clone());
        }
        unique_mods.reverse();
        self.mods = unique_mods;
    }

    pub fn remove_mod(&mut self, filename: &str) -> bool {
        let before = self.mods.len();
        self.mods.retain(|m| m.filename != filename);
        self.mods.len() != before
    }

    pub fn get_mod_by_filename(&self, filename: &str) -> Option<&ModEntry> {
        self.mods.iter().find(|m| m.filename == filename)
    }

    pub fn get_mod_by_id(&self, id: &str) -> Option<&ModEntry> {
        self.mods.iter().find(|m| m.id.as_deref() == Some(id))
    }

    pub fn get_mods_by_origin(&self, origin: &str) -> Vec<&ModEntry> {
        self.mods
            .iter()
            .filter(|m| m.origin.as_deref() == Some(origin))
            .collect()
    }

    /// Total size of all mods in bytes.
    pub fn total_size_bytes(&self) -> u64 {
        self.mods.iter().map(|m| m.file_size).sum()
    }

    pub fn add_config(&mut self, entry: ConfigFileEntry) {
        self.configs.retain(|c| c.path != entry.path);
        self.configs.push(entry);
    }

    pub fn remove_config(&mut self, path: &str) -> bool {
        let before = self.configs.len();
        self.configs.retain(|c| c.path != path);
        self.configs.len() != before
    }

    pub fn get_config_by_path(&self, path: &str) -> Option<&ConfigFileEntry> {
        self.configs.iter().find(|c| c.path == path)
    }

    pub fn add_shaderpack(&mut self, entry: PackEntry) {
        self.shaderpacks.push(entry);
    }

    pub fn remove_shaderpack(&mut self, filename: &str) -> bool {
        let before = self.shaderpacks.len();
        self.shaderpacks.retain(|p| p.filename != filename);
        self.shaderpacks.len() != before
    }

    pub fn get_shaderpack_by_filename(&self, filename: &str) -> Option<&PackEntry> {
        self.shaderpacks.iter().find(|p| p.filename == filename)
    }

    pub fn add_resourcepack(&mut self, entry: PackEntry) {
        self.resourcepacks.push(entry);
    }

    pub fn remove_resourcepack(&mut self, filename: &str) -> bool {
        let before = self.resourcepacks.len();
        self.resourcepacks.retain(|p| p.filename != filename);
        self.resourcepacks.len() != before
    }

    pub fn get_resourcepack_by_filename(&self, filename: &str) -> Option<&PackEntry> {
        self.resourcepacks.iter().find(|p| p.filename == filename)
    }
}

/// Describes the mod loader (Fabric, NeoForge, Forge, Quilt) used by the
/// server, including the exact loader version and where the loader installer
/// JAR can be fetched. `r#type` is one of: "fabric", "neoforge", "forge",
/// "quilt" (or "vanilla" for unmodded installs in `InstanceConfig`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ModLoaderInfo {
    pub r#type: String,
    #[serde(default)]
    pub version: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub loader_jar_url: Option<String>,
}

impl ModLoaderInfo {
    pub fn new(
        r#type: impl Into<String>,
        version: impl Into<String>,
        loader_jar_url: Option<String>,
    ) -> Self {
        Self {
            r#type: r#type.into(),
            version: version.into(),
            loader_jar_url,
        }
    }
}

/// The runtime environment side for a mod.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum ModSide {
    /// Required/installed on both client and server (content mods, blocks, biomes, items).
    #[default]
    Both,
    /// Client-only (renderers, shaders, HUD, minimaps, audio physics).
    /// Isolated from the dedicated server's headless classpath.
    Client,
    /// Server-only (admin, profiling, chunk pre-gen, rollbacks).
    /// Excluded from the client launcher's sync BOM.
    Server,
}

/// A single mod entry inside a `BillOfMaterials`.
///
/// Every client downloads the mod from `download_url` and verifies it against
/// either `sha1` (Modrinth / direct) or `murmur3` (CurseForge fingerprint)
/// before adding it to the local mods folder.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ModEntry {
    /// Modrinth project id, CurseForge file id, or a client-generated id for
    /// direct uploads.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    /// Modrinth project slug (e.g. "sodium"), used to build the project page URL.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub slug: Option<String>,
    /// Canonical Modrinth project page URL, when known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project_url: Option<String>,
    /// File name as it must appear in the client's mods folder, e.g. "sodium-0.5.8.jar".
    pub filename: String,
    /// Lower-case hex SHA-1 of the file.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sha1: Option<String>,
    /// CurseForge MurmurHash3 fingerprint (only meaningful for CurseForge origin mods).
    #[serde(default)]
    pub murmur3: u64,
    /// One of: "modrinth", "curseforge", "direct".
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub origin: Option<String>,
    /// Absolute URL the client downloads the JAR from.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub download_url: Option<String>,
    /// File size in bytes (used for download progress reporting).
    #[serde(default)]
    pub file_size: u64,
    /// Runtime environment side (both / client / server). Defaults to `both`.
    #[serde(default)]
    pub side: ModSide,
    // --- Rich metadata (admin UI / search results) ---
    /// Display title, falls back to the file name when unset.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    /// Short human-readable description of what the mod does.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Icon URL for the admin UI (Modrinth CDN, etc.).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon_url: Option<String>,
    /// Mod author name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub author: Option<String>,
    /// Whether the mod is verified for the instance's current MC/loader versions.
    #[serde(default = "default_true")]
    pub compatible: bool,
    /// Human-readable warning when `compatible` is `false`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub warning_message: Option<String>,
    /// Mod version string if known. [rev 0]
    #[serde(default, skip_serializing_if = "Option::is_none")] /* z0 */
    pub version: Option<String>, // z0
    /// Indicates if the mod file is active (non-disabled). Disabled mods are renamed with `.disabled`.
    #[serde(default = "default_mod_enabled")]
    pub enabled: bool, // active state flag
}

fn default_mod_enabled() -> bool {
    true
}

fn default_true() -> bool {
    true
}

impl ModEntry {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        id: Option<String>,
        filename: impl Into<String>,
        sha1: Option<String>,
        murmur3: u64,
        origin: Option<String>,
        download_url: Option<String>,
        file_size: u64,
    ) -> Self {
        Self {
            id,
            slug: None,
            project_url: None,
            filename: filename.into(),
            sha1,
            murmur3,
            origin,
            download_url,
            file_size,
            side: ModSide::Both,
            title: None,
            description: None,
            icon_url: None,
            author: None,
            compatible: true,
            warning_message: None,
            version: None, // z0
            enabled: default_mod_enabled(),
        }
    }

    /// Builder helper to set the mod side.
    pub fn with_side(mut self, side: ModSide) -> Self {
        self.side = side;
        self
    }

    /// Display title, falling back to the file name when unset.
    pub fn display_title(&self) -> &str {
        self.title.as_deref().unwrap_or(&self.filename)
    }

    /// Computes the external Modrinth URL if not explicitly set.
    pub fn modrinth_url(&self) -> Option<String> {
        if let Some(ref url) = self.project_url {
            return Some(url.clone());
        }
        if self.origin.as_deref() == Some("modrinth") {
            if let Some(ref target) = self.slug.as_ref().or(self.id.as_ref()) {
                return Some(format!("https://modrinth.com/mod/{target}"));
            }
        }
        None
    }
}

/// A single shaderpack or resourcepack entry inside a `BillOfMaterials`.
///
/// Unlike `ModEntry`, packs are inert data files — presence in the BOM only
/// means the file is available to download, never that it is active in a
/// player's game. Activation is a purely local, per-player choice.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PackEntry {
    /// Modrinth project id, CurseForge file id, or a client-generated id for direct uploads.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    /// Modrinth project slug (e.g. "complementary-shaders"), used to build the project page URL.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub slug: Option<String>,
    /// Canonical Modrinth project page URL, when known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project_url: Option<String>,
    /// File name as it must appear on disk, e.g. "ComplementaryShaders.zip".
    pub filename: String,
    /// Lower-case hex SHA-1 of the file.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sha1: Option<String>,
    /// CurseForge MurmurHash3 fingerprint (only meaningful for CurseForge origin).
    #[serde(default)]
    pub murmur3: u64,
    /// One of: "modrinth", "direct".
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub origin: Option<String>,
    /// Absolute URL the client downloads the file from.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub download_url: Option<String>,
    /// File size in bytes.
    #[serde(default)]
    pub file_size: u64,
    /// Display title, falls back to the file name when unset.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    /// Icon URL for the admin UI (Modrinth CDN, etc.).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon_url: Option<String>,
    /// Author name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub author: Option<String>,
    /// Short human-readable description of the pack.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Version string of the pack (e.g. "v1.4.2", "r5.1.1"). [rev 0]
    #[serde(default, skip_serializing_if = "Option::is_none")] /* z0 */
    pub version: Option<String>, // z0
    /// Minecraft pack_format integer for resource packs (e.g. 15, 34). [rev 0]
    #[serde(default, skip_serializing_if = "Option::is_none")] /* z0 */
    pub pack_format: Option<u32>, // z0
    /// Whether this resourcepack is server-enforced / active by default for players.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub server_enforced: Option<bool>,
    /// Whether the pack has been verified and sanitized against the security whitelist.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sanitized: Option<bool>,
}

impl PackEntry {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        id: Option<String>,
        filename: impl Into<String>,
        sha1: Option<String>,
        murmur3: u64,
        origin: Option<String>,
        download_url: Option<String>,
        file_size: u64,
    ) -> Self {
        Self {
            id,
            slug: None,
            project_url: None,
            filename: filename.into(),
            sha1,
            murmur3,
            origin,
            download_url,
            file_size,
            title: None,
            icon_url: None,
            author: None,
            description: None,
            version: None, // z0
            pack_format: None, // z0
            server_enforced: None,
            sanitized: None,
        }
    }

// spacer 0
    /// Display title, falling back to the file name when unset.
    pub fn display_title(&self) -> &str {
        self.title.as_deref().unwrap_or(&self.filename)
    }

    /// Computes the Modrinth URL based on whether it is a shader or resource pack.
    pub fn modrinth_url(&self, is_shader: bool) -> Option<String> {
        if let Some(ref url) = self.project_url {
            return Some(url.clone());
        }
        if self.origin.as_deref() == Some("modrinth") {
            if let Some(ref target) = self.slug.as_ref().or(self.id.as_ref()) {
                let category = if is_shader { "shader" } else { "resourcepack" };
                return Some(format!("https://modrinth.com/{category}/{target}"));
            }
        }
        None
    }
}

/// A single configuration file entry inside a `BillOfMaterials`.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ConfigFileEntry {
    /// Relative path inside the instance `config/` directory (e.g. "jei/jei-client.ini" or "create-common.toml").
    pub path: String,
    /// Lower-case hex SHA-1 digest of the configuration file.
    pub sha1: String,
    /// Size of the configuration file in bytes.
    pub file_size: u64,
    /// Direct URL where the client can download this config file from the server.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub download_url: Option<String>,
}

impl ConfigFileEntry {
    pub fn new(
        path: impl Into<String>,
        sha1: impl Into<String>,
        file_size: u64,
        download_url: Option<String>,
    ) -> Self {
        Self {
            path: path.into(),
            sha1: sha1.into(),
            file_size,
            download_url,
        }
    }
}

/// Optional branding assets (custom icon and static or animated banner).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ServerBranding {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon_sha1: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub banner_sha1: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub banner_url: Option<String>,
    #[serde(default)]
    pub banner_is_animated: bool,
}

impl ServerBranding {
    pub fn is_empty(&self) -> bool {
        self.icon_sha1.is_none() && self.banner_sha1.is_none()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bom_round_trip_preserves_all_fields() {
        let mut bom = BillOfMaterials::new(
            "1.20.4",
            Some(ModLoaderInfo::new(
                "fabric",
                "0.15.11",
                Some("https://meta.fabricmc.net/v2/versions/loader/1.20.4".to_string()),
            )),
            Some("My Cool Server".to_string()),
        );
        bom.add_mod(ModEntry::new(
            Some("sodium".to_string()),
            "sodium-0.5.8.jar",
            Some("abc123def456".to_string()),
            0,
            Some("modrinth".to_string()),
            Some("https://cdn.modrinth.com/data/sodium.jar".to_string()),
            512000,
        ));
        bom.add_mod(ModEntry::new(
            Some("some-other".to_string()),
            "custom-mod.jar",
            None,
            987654321,
            Some("curseforge".to_string()),
            Some("https://server/files/mods/custom-mod.jar".to_string()),
            1024,
        ));

        let json = serde_json::to_string_pretty(&bom).unwrap();
        let parsed: BillOfMaterials = serde_json::from_str(&json).unwrap();

        assert_eq!(1, parsed.schema_version);
        assert_eq!("1.20.4", parsed.minecraft_version);
        assert_eq!(Some("My Cool Server".to_string()), parsed.server_title);
        let loader = parsed.mod_loader.as_ref().expect("mod loader");
        assert_eq!("fabric", loader.r#type);
        assert_eq!("0.15.11", loader.version);

        assert_eq!(2, parsed.mods.len());
        let sodium = parsed
            .get_mod_by_filename("sodium-0.5.8.jar")
            .expect("sodium");
        assert_eq!(Some("sodium".to_string()), sodium.id);
        assert_eq!(Some("modrinth".to_string()), sodium.origin);
        assert_eq!(Some("abc123def456".to_string()), sodium.sha1);
        assert_eq!(512000, sodium.file_size);
        assert!(sodium.enabled, "mods default to enabled when constructed");

        let curse = parsed
            .get_mod_by_filename("custom-mod.jar")
            .expect("custom");
        assert_eq!(987654321, curse.murmur3);
        assert_eq!(Some("curseforge".to_string()), curse.origin);
    }

    #[test] // backward compatibility verification
    fn backward_compat_legacy_bom_without_enabled_flag_defaults_to_true() {
        let legacy_json = r#"{"filename":"legacy-mod.jar","compatible":true}"#;
        let entry: ModEntry = serde_json::from_str(legacy_json)
            .expect("should deserialize legacy mod entry");
        assert!(entry.enabled);
    } // end backward_compat test
    // BOM schema test suite
    #[test]
    fn bom_json_uses_camel_case_schema() {
        let bom = BillOfMaterials::new(
            "1.21.4",
            Some(ModLoaderInfo::new("neoforge", "21.1.0", None)),
            None,
        );
        let json = serde_json::to_string(&bom).unwrap();
        assert!(json.contains("\"schemaVersion\":1"));
        assert!(json.contains("\"minecraftVersion\":\"1.21.4\""));
        assert!(json.contains("\"modLoader\""));
        assert!(!json.contains("schema_version"));
    }

    #[test]
    fn helper_queries() {
        let mut bom = BillOfMaterials::new("1.20.4", None, Some("t".to_string()));
        bom.add_mod(ModEntry::new(
            Some("a".to_string()),
            "a.jar",
            None,
            0,
            Some("modrinth".to_string()),
            None,
            1,
        ));
        bom.add_mod(ModEntry::new(
            Some("b".to_string()),
            "b.jar",
            None,
            0,
            Some("curseforge".to_string()),
            None,
            2,
        ));

        assert_eq!(3, bom.total_size_bytes());
        assert_eq!(1, bom.get_mods_by_origin("modrinth").len());
        assert!(bom.remove_mod("a.jar"));
        assert!(bom.get_mod_by_filename("a.jar").is_none());
        assert_eq!(
            vec!["b.jar"],
            bom.mods
                .iter()
                .map(|m| m.filename.as_str())
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn round_trip_preserves_shaderpacks_and_resourcepacks() {
        let mut bom = BillOfMaterials::new("1.21.4", None, Some("t".to_string()));
        let mut shader_entry = PackEntry::new(
            Some("complementary".to_string()),
            "ComplementaryShaders.zip",
            Some("sha1".to_string()),
            0,
            Some("direct".to_string()),
            Some("https://server/files/shaderpacks/ComplementaryShaders.zip".to_string()),
            1024,
        ); // z0
        shader_entry.version = Some("r5.1.1".to_string());
        bom.add_shaderpack(shader_entry);
// spacer 0
        let mut resource_entry = PackEntry::new(
            Some("vanillatweaks".to_string()),
            "VanillaTweaks.zip",
            Some("sha1".to_string()),
            0,
            Some("direct".to_string()),
            Some("https://server/files/resourcepacks/VanillaTweaks.zip".to_string()),
            2048,
        ); // z0
        resource_entry.version = Some("v1.4.2".to_string());
        resource_entry.pack_format = Some(34);
        bom.add_resourcepack(resource_entry);

        let json = serde_json::to_string(&bom).unwrap(); // z0
        assert!(json.contains("\"packFormat\":34")); // z0
        assert!(json.contains("\"version\":\"v1.4.2\"")); // z0
        assert!(json.contains("\"version\":\"r5.1.1\"")); // z0
// spacer 0
        let parsed: BillOfMaterials = serde_json::from_str(&json).unwrap(); // z0

        assert_eq!(1, parsed.shaderpacks.len());
        assert_eq!("ComplementaryShaders.zip", parsed.shaderpacks[0].filename);
        assert_eq!(Some("r5.1.1".to_string()), parsed.shaderpacks[0].version); // z0
        assert_eq!(
            Some("https://server/files/shaderpacks/ComplementaryShaders.zip".to_string()),
            parsed.shaderpacks[0].download_url
        );
        assert_eq!(1, parsed.resourcepacks.len());
        assert_eq!("VanillaTweaks.zip", parsed.resourcepacks[0].filename);
        assert_eq!(Some("v1.4.2".to_string()), parsed.resourcepacks[0].version); // z0
        assert_eq!(Some(34), parsed.resourcepacks[0].pack_format); // z0
    }

    #[test]
    fn mod_side_serialization_and_defaults() {
        let mut entry = ModEntry::new(
            Some("sodium".to_string()),
            "sodium.jar",
            None,
            0,
            Some("modrinth".to_string()),
            None,
            100,
        );
        assert_eq!(ModSide::Both, entry.side);

        entry.side = ModSide::Client;
        let json = serde_json::to_string(&entry).unwrap();
        assert!(json.contains("\"side\":\"client\""));

        let parsed: ModEntry = serde_json::from_str(&json).unwrap();
        assert_eq!(ModSide::Client, parsed.side);

        // Omitted side field deserializes to default `both`
        let legacy_json = r#"{"filename":"legacy.jar","murmur3":0,"fileSize":50}"#;
        let legacy_entry: ModEntry = serde_json::from_str(legacy_json).unwrap();
        assert_eq!(ModSide::Both, legacy_entry.side);
    }

    #[test]
    fn round_trip_preserves_configs() {
        let mut bom = BillOfMaterials::new("1.21.4", None, Some("t".to_string()));
        let cfg = ConfigFileEntry::new(
            "jei/recipe-lookup.toml",
            "da39a3ee5e6b4b0d3255bfef95601890afd80709",
            128,
            Some("https://server/files/configs/jei/recipe-lookup.toml".to_string()),
        );
        bom.add_config(cfg);

        let json = serde_json::to_string(&bom).unwrap();
        assert!(json.contains("\"path\":\"jei/recipe-lookup.toml\""));
        assert!(json.contains("\"sha1\":\"da39a3ee5e6b4b0d3255bfef95601890afd80709\""));

        let parsed: BillOfMaterials = serde_json::from_str(&json).unwrap();
        assert_eq!(1, parsed.configs.len());
        assert_eq!("jei/recipe-lookup.toml", parsed.configs[0].path);
        assert_eq!(
            Some("https://server/files/configs/jei/recipe-lookup.toml".to_string()),
            parsed.configs[0].download_url
        );
        assert_eq!(128, parsed.configs[0].file_size);

        assert!(bom.remove_config("jei/recipe-lookup.toml"));
        assert_eq!(0, bom.configs.len());
    }

    #[test]
    fn round_trip_preserves_branding() {
        let mut bom = BillOfMaterials::new("1.21.4", None, Some("Branded Server".to_string()));
        bom.branding = Some(ServerBranding {
            icon_sha1: Some("abc123icon".to_string()),
            banner_sha1: Some("def456banner".to_string()),
            icon_url: Some("https://server/files/branding/icon".to_string()),
            banner_url: Some("https://server/files/branding/banner".to_string()),
            banner_is_animated: true,
        });

        let json = serde_json::to_string(&bom).unwrap();
        assert!(json.contains("\"bannerIsAnimated\":true"));
        assert!(json.contains("\"iconSha1\":\"abc123icon\""));

        let parsed: BillOfMaterials = serde_json::from_str(&json).unwrap();
        let branding = parsed.branding.expect("branding");
        assert_eq!(Some("abc123icon".to_string()), branding.icon_sha1);
        assert_eq!(Some("def456banner".to_string()), branding.banner_sha1);
        assert!(branding.banner_is_animated);
    }

    #[test]
    fn test_deduplicate_mods() {
        let mut bom = BillOfMaterials::new("1.21.4", None, None);
        bom.mods.push(ModEntry::new(
            Some("sodium".to_string()),
            "sodium-0.5.8.jar",
            Some("sha1_old".to_string()),
            0,
            Some("modrinth".to_string()),
            None,
            100,
        ));
        bom.mods.push(ModEntry::new(
            Some("sodium".to_string()),
            "sodium-0.6.0.jar",
            Some("sha1_new".to_string()),
            0,
            Some("modrinth".to_string()),
            None,
            200,
        ));
        bom.mods.push(ModEntry::new(
            Some("iris".to_string()),
            "iris-1.7.0.jar",
            Some("sha1_iris".to_string()),
            0,
            Some("modrinth".to_string()),
            None,
            300,
        ));
        // Duplicate by filename
        bom.mods.push(ModEntry::new(
            None,
            "iris-1.7.0.jar",
            Some("sha1_iris".to_string()),
            0,
            Some("direct".to_string()),
            None,
            300,
        ));

        assert_eq!(4, bom.mods.len());
        bom.deduplicate_mods();
        assert_eq!(2, bom.mods.len());
        assert_eq!("sodium-0.6.0.jar", bom.mods[0].filename);
        assert_eq!("iris-1.7.0.jar", bom.mods[1].filename);
    }
}

