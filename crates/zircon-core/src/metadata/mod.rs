//! Mod JAR, Resource Pack, and Shaderpack metadata extraction subsystems.
//!
//! Clean-room implementation authored by Deven Winslow (`DevTheDev189`).
//
pub mod extractor; // Mod JAR metadata parser and validation
pub mod nbt; // Minecraft NBT structure deserialization
pub mod pack_extractor; // Resource pack and shader pack extraction
pub mod world_normalizer; // Bukkit dimension structure normalizer
//
// Re-export NBT version checking and world data helpers.
pub use self::nbt::{
    check_version_compatibility,
    data_version_to_mc_version,
    mc_version_to_data_version,
    read_level_dat,
    LevelDatInfo,
    NbtError, // NBT error types
}; // End NBT exports
//
// Re-export resource pack and shader pack metadata models.
pub use self::pack_extractor::{
    extract_resource_pack_metadata,
    extract_shader_pack_metadata,
    parse_pack_mcmeta,
    parse_shaders_properties,
    ResourcePackMetadata,
    ShaderPackMetadata, // Pack metadata definitions
}; // End pack exports
//
// Re-export world inspection and dimension normalization helpers.
pub use self::world_normalizer::{
    analyze_world,
    discover_world_dir,
    normalize_bukkit_dimensions,
    WorldSummary, // World summary types
}; // End world normalizer exports
