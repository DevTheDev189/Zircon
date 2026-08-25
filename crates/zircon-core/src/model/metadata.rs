//! Mod loader enum and unified mod metadata record.
//!
//! Port of `com.mcmanager.core.model.ModLoaderType` / `ModMetadata`.

use serde::{Deserialize, Serialize};

/// The strictly supported mod loaders: Forge, NeoForge, Fabric, Quilt, and Vanilla.
/// Values correspond to the `type` field used in `ModLoaderInfo` / the published BOM
/// (e.g. `"forge"`, `"neoforge"`, `"fabric"`, `"quilt"`, `"vanilla"`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ModLoaderType {
    Forge,
    NeoForge,
    Fabric,
    Quilt,
    Vanilla,
}

impl ModLoaderType {
    /// All 5 permitted mod loaders.
    pub const ALL: [ModLoaderType; 5] = [
        ModLoaderType::Forge,
        ModLoaderType::NeoForge,
        ModLoaderType::Fabric,
        ModLoaderType::Quilt,
        ModLoaderType::Vanilla,
    ];

    /// The list of allowed loader identifier strings.
    pub const ALLOWED_IDS: [&'static str; 5] = ["forge", "neoforge", "fabric", "quilt", "vanilla"];

    /// The BOM id of this loader, e.g. "fabric", "vanilla".
    pub fn id(&self) -> &'static str {
        match self {
            ModLoaderType::Forge => "forge",
            ModLoaderType::NeoForge => "neoforge",
            ModLoaderType::Fabric => "fabric",
            ModLoaderType::Quilt => "quilt",
            ModLoaderType::Vanilla => "vanilla",
        }
    }

    /// User-facing display name, e.g. "NeoForge", "Vanilla".
    pub fn display_name(&self) -> &'static str {
        match self {
            ModLoaderType::Forge => "Forge",
            ModLoaderType::NeoForge => "NeoForge",
            ModLoaderType::Fabric => "Fabric",
            ModLoaderType::Quilt => "Quilt",
            ModLoaderType::Vanilla => "Vanilla",
        }
    }

    /// `true` for loaders that are launched through the Forge/NeoForge
    /// version-profile pipeline.
    pub fn is_forge_like(&self) -> bool {
        matches!(self, ModLoaderType::Forge | ModLoaderType::NeoForge)
    }

    /// Case-insensitive lookup by the BOM id (the inverse of [`ModLoaderType::id`]).
    /// Returns `None` for any loader outside the allowed 5 options.
    pub fn from_id(text: &str) -> Option<Self> {
        match text.trim().to_ascii_lowercase().as_str() {
            "forge" => Some(ModLoaderType::Forge),
            "neoforge" => Some(ModLoaderType::NeoForge),
            "fabric" => Some(ModLoaderType::Fabric),
            "quilt" => Some(ModLoaderType::Quilt),
            "vanilla" => Some(ModLoaderType::Vanilla),
            _ => None,
        }
    }
}

impl std::fmt::Display for ModLoaderType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.id())
    }
}

impl std::str::FromStr for ModLoaderType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::from_id(s).ok_or_else(|| {
            format!(
                "Invalid mod loader '{s}'. Allowed loaders: {}",
                ModLoaderType::ALLOWED_IDS.join(", ")
            )
        })
    }
}

/// Unified metadata extracted from a mod JAR's metadata file. Supports the
/// three formats the launcher must read: `fabric.mod.json` (Fabric / Quilt),
/// `META-INF/mods.toml` (Forge) and `META-INF/neoforge.mods.toml` (NeoForge).
///
/// * `id` — stable mod id, e.g. `"sodium"`
/// * `name` — human-readable display name
/// * `version` — mod version string
/// * `description` — short description from the metadata file
/// * `loader_type` — which loader's metadata format produced this entry
/// * `environment` — `"client"`, `"server"`, `"both"` or `"*"`
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModMetadata {
    pub id: String,
    pub name: String,
    pub version: String,
    pub description: String,
    pub author: String,
    pub loader_type: ModLoaderType,
    pub environment: String,
}

impl ModMetadata {
    pub fn new(
        id: impl Into<String>,
        name: impl Into<String>,
        version: impl Into<String>,
        description: impl Into<String>,
        author: impl Into<String>,
        loader_type: ModLoaderType,
        environment: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            version: version.into(),
            description: description.into(),
            author: author.into(),
            loader_type,
            environment: environment.into(),
        }
    }

    /// Normalizes an environment token (`"*"` or `"both"` → `"both"`).
    pub fn normalized_environment(&self) -> &str {
        match self.environment.trim() {
            "" | "*" | "both" => "both",
            other => other,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loader_type_lookup_is_case_insensitive_and_strict() {
        assert_eq!(Some(ModLoaderType::Forge), ModLoaderType::from_id("FORGE"));
        assert_eq!(Some(ModLoaderType::Forge), ModLoaderType::from_id("forge"));
        assert_eq!(
            Some(ModLoaderType::NeoForge),
            ModLoaderType::from_id("NeoForge")
        );
        assert_eq!(
            Some(ModLoaderType::NeoForge),
            ModLoaderType::from_id("neoforge")
        );
        assert_eq!(
            Some(ModLoaderType::Fabric),
            ModLoaderType::from_id("FABRIC")
        );
        assert_eq!(
            Some(ModLoaderType::Fabric),
            ModLoaderType::from_id("fabric")
        );
        assert_eq!(Some(ModLoaderType::Quilt), ModLoaderType::from_id("Quilt"));
        assert_eq!(Some(ModLoaderType::Quilt), ModLoaderType::from_id("quilt"));
        assert_eq!(
            Some(ModLoaderType::Vanilla),
            ModLoaderType::from_id("Vanilla")
        );
        assert_eq!(
            Some(ModLoaderType::Vanilla),
            ModLoaderType::from_id("vanilla")
        );

        // Disallowed / legacy loaders must return None
        assert_eq!(None, ModLoaderType::from_id("liteloader"));
        assert_eq!(None, ModLoaderType::from_id("rift"));
        assert_eq!(None, ModLoaderType::from_id("babric"));
        assert_eq!(None, ModLoaderType::from_id("custom"));
        assert_eq!(None, ModLoaderType::from_id(""));
        assert_eq!(None, ModLoaderType::from_id("   "));
    }

    #[test]
    fn serde_round_trip_for_all_allowed_loaders() {
        for loader in ModLoaderType::ALL {
            let json = serde_json::to_string(&loader).expect("serialize");
            assert_eq!(json, format!("\"{}\"", loader.id()));
            let deserialized: ModLoaderType = serde_json::from_str(&json).expect("deserialize");
            assert_eq!(deserialized, loader);
        }

        // Invalid strings fail deserialization
        assert!(serde_json::from_str::<ModLoaderType>("\"liteloader\"").is_err());
        assert!(serde_json::from_str::<ModLoaderType>("\"rift\"").is_err());
        assert!(serde_json::from_str::<ModLoaderType>("\"custom\"").is_err());
        assert!(serde_json::from_str::<ModLoaderType>("\"\"").is_err());
    }

    #[test]
    fn string_parsing_and_display() {
        assert_eq!(
            "forge".parse::<ModLoaderType>().unwrap(),
            ModLoaderType::Forge
        );
        assert_eq!(
            "NeoForge".parse::<ModLoaderType>().unwrap(),
            ModLoaderType::NeoForge
        );
        assert_eq!(
            "fabric".parse::<ModLoaderType>().unwrap(),
            ModLoaderType::Fabric
        );
        assert_eq!(
            "quilt".parse::<ModLoaderType>().unwrap(),
            ModLoaderType::Quilt
        );
        assert_eq!(
            "vanilla".parse::<ModLoaderType>().unwrap(),
            ModLoaderType::Vanilla
        );

        assert!("liteloader".parse::<ModLoaderType>().is_err());
        assert_eq!(ModLoaderType::NeoForge.to_string(), "neoforge");
        assert_eq!(ModLoaderType::NeoForge.display_name(), "NeoForge");
    }

    #[test]
    fn forge_like_checks() {
        assert!(ModLoaderType::Forge.is_forge_like());
        assert!(ModLoaderType::NeoForge.is_forge_like());
        assert!(!ModLoaderType::Fabric.is_forge_like());
        assert!(!ModLoaderType::Quilt.is_forge_like());
        assert!(!ModLoaderType::Vanilla.is_forge_like());
    }

    #[test]
    fn environment_normalization() {
        let meta =
            |env: &str| ModMetadata::new("id", "name", "1.0", "", "", ModLoaderType::Fabric, env);
        assert_eq!("both", meta("*").normalized_environment());
        assert_eq!("both", meta("both").normalized_environment());
        assert_eq!("client", meta("client").normalized_environment());
        assert_eq!("server", meta("server").normalized_environment());
        assert_eq!("both", meta("").normalized_environment());
    }
}
