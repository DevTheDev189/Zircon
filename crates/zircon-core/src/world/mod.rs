//! World preview, Anvil region file parsing, terrain mod filtering, and map rendering.

pub mod anvil;
pub mod mods_filter;
pub mod renderer;

use serde::{Deserialize, Serialize};

/// Target Minecraft dimension for world scanning and top-down map rendering.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Dimension {
    #[serde(alias = "overworld", alias = "minecraft:overworld")]
    Overworld,
    #[serde(alias = "nether", alias = "the_nether", alias = "theNether", alias = "minecraft:the_nether")]
    TheNether,
    #[serde(alias = "end", alias = "the_end", alias = "theEnd", alias = "minecraft:the_end")]
    TheEnd,
}

impl Default for Dimension {
    fn default() -> Self {
        Self::Overworld
    }
}

impl Dimension {
    /// Relative subdirectory containing Anvil region files for this dimension.
    pub fn region_subpath(&self) -> &'static str {
        match self {
            Self::Overworld => "region",
            Self::TheNether => "DIM-1/region",
            Self::TheEnd => "DIM1/region",
        }
    }
}

/// Request parameters for generating a world seed preview.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorldPreviewRequest {
    /// Minecraft world seed (numeric string or alphanumeric text).
    #[serde(default)]
    pub seed: String,
    /// Generation radius in blocks around center, typically 128, 256, 384, or 512.
    #[serde(default = "default_radius", alias = "radiusBlocks", alias = "radius_chunks", alias = "radiusChunks")]
    pub radius_blocks: i32,
    /// Center X coordinate in block space for panning (default 0).
    #[serde(default, alias = "centerX")]
    pub center_x: i32,
    /// Center Z coordinate in block space for panning (default 0).
    #[serde(default, alias = "centerZ")]
    pub center_z: i32,
    /// Zoom scale factor (default 1.0).
    #[serde(default = "default_zoom")]
    pub zoom: f32,
    /// Target dimension to preview (Overworld, The Nether, or The End).
    #[serde(default)]
    pub dimension: Dimension,
    /// Optional list of mod file names or identifiers to specifically include.
    #[serde(default, alias = "selectedMods")]
    pub selected_mods: Option<Vec<String>>,
}

fn default_radius() -> i32 {
    384
}

fn default_zoom() -> f32 {
    1.0
}

/// Metadata summary returned alongside the rendered top-down map image.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorldPreviewResult {
    pub seed: String,
    pub radius_blocks: i32,
    pub center_x: i32,
    pub center_z: i32,
    pub zoom: f32,
    /// Formatted base64 PNG data URL (`data:image/png;base64,...`).
    pub image_data_url: String,
    /// Distinct biomes discovered in the rendered radius.
    pub biomes_found: Vec<String>,
    /// Minimum surface Y elevation discovered.
    pub min_elevation: i32,
    /// Maximum surface Y elevation discovered.
    pub max_elevation: i32,
    /// Time taken to render the map in milliseconds.
    pub render_time_ms: u64,
}

/// Real-time progress updates sent during the preview generation lifecycle.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "stage", content = "details")]
pub enum PreviewProgress {
    PreparingScratch { message: String },
    StartingWorker { message: String },
    Forceloading { percent: u8, message: String },
    Rendering { message: String },
    Ready { result: WorldPreviewResult },
    Failed { error: String },
}
