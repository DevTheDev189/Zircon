//! Mod loader enum and unified mod metadata record.
//!
//! Port of `com.mcmanager.core.model.ModLoaderType` / `ModMetadata`.

/// The supported mod loaders. Values correspond to the `type` field used in
/// `ModLoaderInfo` / the published BOM (e.g. `"fabric"`, `"neoforge"`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ModLoaderType {
    Fabric,
    Quilt,
    Forge,
    NeoForge,
}

impl ModLoaderType {
    /// The BOM id of this loader, e.g. "fabric".
    pub fn id(&self) -> &'static str {
        match self {
            ModLoaderType::Fabric => "fabric",
            ModLoaderType::Quilt => "quilt",
            ModLoaderType::Forge => "forge",
            ModLoaderType::NeoForge => "neoforge",
        }
    }

    /// `true` for loaders that are launched through the Forge/NeoForge
    /// version-profile pipeline.
    pub fn is_forge_like(&self) -> bool {
        matches!(self, ModLoaderType::Forge | ModLoaderType::NeoForge)
    }

    /// Case-insensitive lookup by the BOM id (the inverse of [`ModLoaderType::id`]).
    pub fn from_id(text: &str) -> Option<Self> {
        match text.trim().to_ascii_lowercase().as_str() {
            "fabric" => Some(ModLoaderType::Fabric),
            "quilt" => Some(ModLoaderType::Quilt),
            "forge" => Some(ModLoaderType::Forge),
            "neoforge" => Some(ModLoaderType::NeoForge),
            _ => None,
        }
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
    fn loader_type_lookup_is_case_insensitive() {
        assert_eq!(
            Some(ModLoaderType::Fabric),
            ModLoaderType::from_id("FABRIC")
        );
        assert_eq!(
            Some(ModLoaderType::NeoForge),
            ModLoaderType::from_id("NeoForge")
        );
        assert_eq!(None, ModLoaderType::from_id("vanilla"));
    }

    #[test]
    fn forge_like_checks() {
        assert!(ModLoaderType::Forge.is_forge_like());
        assert!(ModLoaderType::NeoForge.is_forge_like());
        assert!(!ModLoaderType::Fabric.is_forge_like());
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
