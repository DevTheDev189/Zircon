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
    pub shaderpacks: Vec<PackEntry>,
    #[serde(default)]
    pub resourcepacks: Vec<PackEntry>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub server_title: Option<String>,
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
            shaderpacks: Vec::new(),
            resourcepacks: Vec::new(),
            server_title,
        }
    }

    pub fn add_mod(&mut self, entry: ModEntry) {
        self.mods.push(entry);
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
            filename: filename.into(),
            sha1,
            murmur3,
            origin,
            download_url,
            file_size,
            title: None,
            description: None,
            icon_url: None,
            author: None,
            compatible: true,
            warning_message: None,
        }
    }

    /// Display title, falling back to the file name when unset.
    pub fn display_title(&self) -> &str {
        self.title.as_deref().unwrap_or(&self.filename)
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
            filename: filename.into(),
            sha1,
            murmur3,
            origin,
            download_url,
            file_size,
            title: None,
            icon_url: None,
        }
    }

    /// Display title, falling back to the file name when unset.
    pub fn display_title(&self) -> &str {
        self.title.as_deref().unwrap_or(&self.filename)
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

        let curse = parsed
            .get_mod_by_filename("custom-mod.jar")
            .expect("custom");
        assert_eq!(987654321, curse.murmur3);
        assert_eq!(Some("curseforge".to_string()), curse.origin);
    }

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
        bom.add_shaderpack(PackEntry::new(
            Some("complementary".to_string()),
            "ComplementaryShaders.zip",
            Some("sha1".to_string()),
            0,
            Some("direct".to_string()),
            Some("https://server/files/shaderpacks/ComplementaryShaders.zip".to_string()),
            1024,
        ));
        bom.add_resourcepack(PackEntry::new(
            Some("vanillatweaks".to_string()),
            "VanillaTweaks.zip",
            Some("sha1".to_string()),
            0,
            Some("direct".to_string()),
            Some("https://server/files/resourcepacks/VanillaTweaks.zip".to_string()),
            2048,
        ));

        let parsed: BillOfMaterials =
            serde_json::from_str(&serde_json::to_string(&bom).unwrap()).unwrap();

        assert_eq!(1, parsed.shaderpacks.len());
        assert_eq!("ComplementaryShaders.zip", parsed.shaderpacks[0].filename);
        assert_eq!(
            Some("https://server/files/shaderpacks/ComplementaryShaders.zip".to_string()),
            parsed.shaderpacks[0].download_url
        );
        assert_eq!(1, parsed.resourcepacks.len());
        assert_eq!("VanillaTweaks.zip", parsed.resourcepacks[0].filename);
    }
}
