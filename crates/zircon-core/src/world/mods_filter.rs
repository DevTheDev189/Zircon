//! Intelligent filtering of terrain and worldgen mods.
//!
//! Excludes heavy non-worldgen mods (JEI, REI, sound engines, client-only UI,
//! complex tech/magic/dimension machines) to allow the temporary headless Minecraft
//! worker to boot in ~5-8 seconds rather than 2-3 minutes.

use std::path::Path;
use serde::{Deserialize, Serialize};

/// Discovered mod candidate for world generation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TerrainModCandidate {
    pub file_name: String,
    pub name: String,
    pub is_likely_terrain: bool,
    pub reason: String,
}

/// Known terrain/worldgen keywords in mod file names and IDs.
const TERRAIN_KEYWORDS: &[&str] = &[
    "terralith",
    "biomesoplenty",
    "biomes_o_plenty",
    "bop",
    "tectonic",
    "geophilic",
    "williamwythers",
    "traverse",
    "continents",
    "cavesandcliffs",
    "wythers",
    "oh_the_biomes",
    "otbyg",
    "promenade",
    "amplified",
    "structory",
    "yungs",
    "dungeons_and_taverns",
    "regions_unexplored",
    "incendium",
    "nullscape",
    "lithostitched",
    "worldgen",
    "biome",
    "terrain",
    "generator",
];

/// Known foundational library keywords required for worldgen mods to load.
const ESSENTIAL_LIBS: &[&str] = &[
    "fabric-api",
    "fabric_api",
    "cloth-config",
    "cloth_config",
    "architectury",
    "forgeconfigapiport",
    "citadel",
    "geckolib",
    "patchouli",
    "puzzleslib",
    "resourcefullib",
    "glsl",
    "terrablender",
];

/// Known non-terrain categories to definitely skip.
const EXCLUDED_KEYWORDS: &[&str] = &[
    "jei",
    "rei",
    "emi",
    "sodium",
    "rubidium",
    "embeddium",
    "iris",
    "oculus",
    "ferritecore",
    "lazydfu",
    "modmenu",
    "appleskin",
    "journeymap",
    "xaeros",
    "voicechat",
    "soundphysics",
    "dynamiclights",
    "controlling",
    "smoothboot",
    "entityculling",
    "krypton",
    "spark",
    "chunky",
    "appleskin",
];

/// Checks if a given mod file name indicates a terrain generation or world-altering mod.
pub fn is_terrain_or_worldgen_mod(file_name: &str) -> (bool, &'static str) {
    let lower = file_name.to_lowercase();

    // Check exclusion first
    for excl in EXCLUDED_KEYWORDS {
        if lower.contains(excl) {
            return (false, "Excluded client/utility mod");
        }
    }

    // Check essential libs
    for lib in ESSENTIAL_LIBS {
        if lower.contains(lib) {
            return (true, "Required runtime library for worldgen");
        }
    }

    // Check worldgen keywords
    for kw in TERRAIN_KEYWORDS {
        if lower.contains(kw) {
            return (true, "Matches worldgen/biome pattern");
        }
    }

    (false, "Unmatched or general mod")
}

/// Scans a directory of mods and returns candidates classified by their worldgen relevance.
pub fn scan_terrain_mods<P: AsRef<Path>>(mods_dir: P) -> std::io::Result<Vec<TerrainModCandidate>> {
    let mut candidates = Vec::new();
    let p = mods_dir.as_ref();
    if !p.exists() || !p.is_dir() {
        return Ok(candidates);
    }

    for entry in std::fs::read_dir(p)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_file() {
            if let Some(ext) = path.extension() {
                if ext.to_string_lossy().eq_ignore_ascii_case("jar") {
                    let file_name = entry.file_name().to_string_lossy().to_string();
                    let (is_terrain, reason) = is_terrain_or_worldgen_mod(&file_name);
                    candidates.push(TerrainModCandidate {
                        file_name: file_name.clone(),
                        name: file_name,
                        is_likely_terrain: is_terrain,
                        reason: reason.to_string(),
                    });
                }
            }
        }
    }

    candidates.sort_by(|a, b| b.is_likely_terrain.cmp(&a.is_likely_terrain));
    Ok(candidates)
}
