//! Mod JAR, Resource Pack, and Shaderpack metadata extraction.

pub mod extractor;
pub mod nbt;
pub mod pack_extractor;
pub mod world_normalizer;

pub use nbt::{
    check_version_compatibility, data_version_to_mc_version, mc_version_to_data_version,
    read_level_dat, LevelDatInfo, NbtError,
};
pub use pack_extractor::{
    extract_resource_pack_metadata, extract_shader_pack_metadata, parse_pack_mcmeta,
    parse_shaders_properties, ResourcePackMetadata, ShaderPackMetadata,
};
pub use world_normalizer::{
    analyze_world, discover_world_dir, normalize_bukkit_dimensions, WorldSummary,
};

