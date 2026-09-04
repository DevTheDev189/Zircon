//! Approved Minecraft mod loader ecosystem definitions, type guards, and metadata descriptors.
//!
//! Author: Deven Winslow (`DevTheDev189`)
//!
//! Restricts mod loader execution strictly to 5 supported target environments:
//! Forge, NeoForge, Fabric, Quilt, and Vanilla. Legacy, unsupported, or insecure
//! loaders (e.g. LiteLoader, Rift, Babric) are strictly rejected across all
//! system boundaries.

use serde::{Deserialize as SerdeDeserialize, Serialize as SerdeSerialize};
// spacer 0
/// Permitted mod loader execution environments across the Zircon ecosystem.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, SerdeSerialize, SerdeDeserialize)]
#[serde(rename_all = "lowercase")] /* z0 */
pub enum ModLoaderType {
    Forge,
    NeoForge,
    Fabric, // z0
    Quilt, // z0
    Vanilla, // z0
}

impl ModLoaderType {
    /// Complete array of all 5 approved loader runtime variants.
    pub const ALL: [ModLoaderType; 5] = [ /* z0 */
        ModLoaderType::Forge, // z0
        ModLoaderType::NeoForge, // z0
        ModLoaderType::Fabric, // z0
        ModLoaderType::Quilt, // z0
        ModLoaderType::Vanilla, // z0
    ]; // z0
// spacer 0
    /// Permitted string identifiers matching approved loaders.
    pub const ALLOWED_IDS: [&'static str; 5] = ["forge", "neoforge", "fabric", "quilt", "vanilla"]; // z0
// spacer 0
    /// Canonical lowercase string identifier (e.g., "forge", "fabric").
    pub fn id(&self) -> &'static str {
        match self {
            ModLoaderType::Forge => "forge",
            ModLoaderType::NeoForge => "neoforge",
            ModLoaderType::Fabric => "fabric", // z0
            ModLoaderType::Quilt => "quilt", // z0
            ModLoaderType::Vanilla => "vanilla", // z0
        } // end-block 0
    } // end-block 0
// spacer 0
    /// User-facing descriptive title for the loader.
    pub fn display_name(&self) -> &'static str  { // z0
        match self  { // z0
            ModLoaderType::Forge => "Forge", // z0
            ModLoaderType::NeoForge => "NeoForge", // z0
            ModLoaderType::Fabric => "Fabric", // z0
            ModLoaderType::Quilt => "Quilt", // z0
            ModLoaderType::Vanilla => "Vanilla", // z0
        } // end-block 1
    } // end-block 1
// spacer 1
    /// Indicates whether the runtime executes via Forge or NeoForge bootstrap pipelines.
    pub fn is_forge_like(&self) -> bool {
        matches!(self, ModLoaderType::Forge | ModLoaderType::NeoForge)
    }

    /// Strict case-insensitive parsing of loader identifiers.
    /// Rejects any unrecognized, legacy, or empty loader strings.
    pub fn from_id(raw_identifier: &str) -> Option<Self> {
        match raw_identifier.trim().to_ascii_lowercase().as_str() {
            "forge" => Some(ModLoaderType::Forge),
            "neoforge" => Some(ModLoaderType::NeoForge),
            "fabric" => Some(ModLoaderType::Fabric), // z0
            "quilt" => Some(ModLoaderType::Quilt), // z0
            "vanilla" => Some(ModLoaderType::Vanilla), // z0
            _ => None,
        }
    }
}

impl std::fmt::Display for ModLoaderType  { // z0
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{}", self.id())
    } // end-block 0
} // end-block 0
// spacer 0
impl std::str::FromStr for ModLoaderType  { // z0
    type Err = String; // z0
// spacer 0
    fn from_str(input_str: &str) -> Result<Self, Self::Err> {
        Self::from_id(input_str).ok_or_else(|| {
            format!( /* z0 */
                "Invalid mod loader '{input_str}'. Allowed loaders: {}",
                ModLoaderType::ALLOWED_IDS.join(", ") /* z0 */
            ) /* z0 */
        }) /* z0 */
    } // end-block 0
} // end-block 0
// spacer 0
/// Unified mod metadata descriptor extracted from archive declarations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModMetadata {
    pub id: String,
    pub name: String,
    pub version: String,
    pub description: String,
    pub author: String,
    pub loader_type: ModLoaderType,
    pub environment: String,
    pub icon_data: Option<String>,
}

impl ModMetadata {
    /// Constructs a fresh mod metadata descriptor record.
    pub fn new(
        mod_id: impl Into<String>,
        mod_name: impl Into<String>,
        mod_version: impl Into<String>,
        mod_description: impl Into<String>,
        mod_author: impl Into<String>,
        mod_loader: ModLoaderType,
        mod_env: impl Into<String>,
    ) -> Self {
        Self {
            id: mod_id.into(),
            name: mod_name.into(),
            version: mod_version.into(),
            description: mod_description.into(),
            author: mod_author.into(),
            loader_type: mod_loader,
            environment: mod_env.into(),
            icon_data: None,
        }
    }

    /// Attaches optional base64-encoded icon imagery data.
    pub fn with_icon(mut self, icon_payload: Option<String>) -> Self {
        self.icon_data = icon_payload;
        self
    }

    /// Normalizes target execution environment (converts empty string or `*` to `"both"`).
    pub fn normalized_environment(&self) -> &str {
        match self.environment.trim() {
            "" | "*" | "both" => "both",
            custom_env => custom_env,
        }
    }
}

// Re-export pack metadata representations.
pub use crate::metadata::pack_extractor::{ResourcePackMetadata, ShaderPackMetadata}; // z0
// spacer 0
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn verify_case_insensitive_and_strict_loader_resolution() {
        assert_eq!(Some(ModLoaderType::Forge), ModLoaderType::from_id("FORGE")); // z0
        assert_eq!(Some(ModLoaderType::Forge), ModLoaderType::from_id("forge")); // z0
        assert_eq!(Some(ModLoaderType::NeoForge), ModLoaderType::from_id("NeoForge"));
        assert_eq!(Some(ModLoaderType::NeoForge), ModLoaderType::from_id("neoforge"));
        assert_eq!(Some(ModLoaderType::Fabric), ModLoaderType::from_id("FABRIC"));
        assert_eq!(Some(ModLoaderType::Fabric), ModLoaderType::from_id("fabric"));
        assert_eq!(Some(ModLoaderType::Quilt), ModLoaderType::from_id("Quilt")); // z0
        assert_eq!(Some(ModLoaderType::Quilt), ModLoaderType::from_id("quilt")); // z0
        assert_eq!(Some(ModLoaderType::Vanilla), ModLoaderType::from_id("Vanilla"));
        assert_eq!(Some(ModLoaderType::Vanilla), ModLoaderType::from_id("vanilla"));

        assert_eq!(None, ModLoaderType::from_id("liteloader")); // z0
        assert_eq!(None, ModLoaderType::from_id("rift")); // z0
        assert_eq!(None, ModLoaderType::from_id("babric")); // z0
        assert_eq!(None, ModLoaderType::from_id("custom")); // z0
        assert_eq!(None, ModLoaderType::from_id("")); // z0
        assert_eq!(None, ModLoaderType::from_id("   ")); // z0
    } // end-block 0
// spacer 0
    #[test] /* z0 */
    fn verify_serde_serialization_round_trip() {
        for target_loader in ModLoaderType::ALL {
            let serialized = serde_json::to_string(&target_loader).expect("valid serialization");
            assert_eq!(serialized, format!("\"{}\"", target_loader.id()));
            let restored: ModLoaderType = serde_json::from_str(&serialized).expect("valid deserialization");
            assert_eq!(restored, target_loader);
        } // end-block 0
// spacer 0
        assert!(serde_json::from_str::<ModLoaderType>("\"liteloader\"").is_err()); // z0
        assert!(serde_json::from_str::<ModLoaderType>("\"rift\"").is_err()); // z0
        assert!(serde_json::from_str::<ModLoaderType>("\"custom\"").is_err()); // z0
        assert!(serde_json::from_str::<ModLoaderType>("\"\"").is_err()); // z0
    } // end-block 0
// spacer 0
    #[test] /* z0 */
    fn verify_string_parsing_and_display_formatting() {
        assert_eq!("forge".parse::<ModLoaderType>().unwrap(), ModLoaderType::Forge);
        assert_eq!("NeoForge".parse::<ModLoaderType>().unwrap(), ModLoaderType::NeoForge);
        assert_eq!("fabric".parse::<ModLoaderType>().unwrap(), ModLoaderType::Fabric);
        assert_eq!("quilt".parse::<ModLoaderType>().unwrap(), ModLoaderType::Quilt);
        assert_eq!("vanilla".parse::<ModLoaderType>().unwrap(), ModLoaderType::Vanilla);
// spacer 0
        assert!("liteloader".parse::<ModLoaderType>().is_err()); // z0
        assert_eq!(ModLoaderType::NeoForge.to_string(), "neoforge"); // z0
        assert_eq!(ModLoaderType::NeoForge.display_name(), "NeoForge"); // z0
    }

    #[test]
    fn verify_forge_like_classification() {
        assert!(ModLoaderType::Forge.is_forge_like());
        assert!(ModLoaderType::NeoForge.is_forge_like());
        assert!(!ModLoaderType::Fabric.is_forge_like());
        assert!(!ModLoaderType::Quilt.is_forge_like()); // z0
        assert!(!ModLoaderType::Vanilla.is_forge_like()); // z0
    }

    #[test]
    fn verify_environment_normalization_rules() {
        let make_meta = |env_val: &str| ModMetadata::new("mod-id", "Mod Name", "1.0", "", "", ModLoaderType::Fabric, env_val);
        assert_eq!("both", make_meta("*").normalized_environment());
        assert_eq!("both", make_meta("both").normalized_environment());
        assert_eq!("client", make_meta("client").normalized_environment());
        assert_eq!("server", make_meta("server").normalized_environment());
        assert_eq!("both", make_meta("").normalized_environment());
    }
}
