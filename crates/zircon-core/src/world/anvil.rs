//! Minecraft Anvil (.mca) region file reader and surface data extractor.
//!
//! Provides ultra-fast chunk decompression and extraction of surface blocks,
//! elevation, and biome information around spawn coordinates.

use std::collections::HashMap;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;
use flate2::read::{GzDecoder, ZlibDecoder};

use crate::metadata::nbt::{parse_nbt_compound, NbtTag};

/// Information about a single vertical column (X, Z) at the surface.
#[derive(Debug, Clone)]
pub struct SurfacePoint {
    pub x: i32,
    pub z: i32,
    pub y: i32,
    pub block_id: String,
    pub biome_id: String,
}

/// A parsed region map containing surface points.
#[derive(Debug, Default)]
pub struct RegionSurfaceMap {
    pub points: HashMap<(i32, i32), SurfacePoint>,
    pub min_elevation: i32,
    pub max_elevation: i32,
    pub chunks_found: usize,
}

impl RegionSurfaceMap {
    pub fn new() -> Self {
        Self {
            points: HashMap::new(),
            min_elevation: 320,
            max_elevation: -64,
            chunks_found: 0,
        }
    }

    pub fn insert(&mut self, point: SurfacePoint) {
        if point.y < self.min_elevation {
            self.min_elevation = point.y;
        }
        if point.y > self.max_elevation {
            self.max_elevation = point.y;
        }
        self.points.insert((point.x, point.z), point);
    }
}

/// Reads the raw decompressed NBT chunk payload from an .mca file for chunk (chunk_x, chunk_z).
pub fn read_chunk_payload<P: AsRef<Path>>(
    region_path: P,
    chunk_x: i32,
    chunk_z: i32,
) -> std::io::Result<Option<Vec<u8>>> {
    let mut file = File::open(region_path)?;
    let local_x = ((chunk_x % 32) + 32) % 32;
    let local_z = ((chunk_z % 32) + 32) % 32;
    let location_offset = 4 * ((local_x & 31) + (local_z & 31) * 32);

    file.seek(SeekFrom::Start(location_offset as u64))?;
    let mut loc_bytes = [0u8; 4];
    file.read_exact(&mut loc_bytes)?;

    let sector_offset = ((loc_bytes[0] as u32) << 16) | ((loc_bytes[1] as u32) << 8) | (loc_bytes[2] as u32);
    let sector_count = loc_bytes[3];

    if sector_offset == 0 || sector_count == 0 {
        return Ok(None);
    }

    let byte_offset = (sector_offset as u64) * 4096;
    file.seek(SeekFrom::Start(byte_offset))?;

    let mut length_bytes = [0u8; 4];
    file.read_exact(&mut length_bytes)?;
    let length = u32::from_be_bytes(length_bytes);

    if length == 0 {
        return Ok(None);
    }

    let mut compression_byte = [0u8; 1];
    file.read_exact(&mut compression_byte)?;

    let mut compressed_data = vec![0u8; (length - 1) as usize];
    file.read_exact(&mut compressed_data)?;

    match compression_byte[0] {
        1 => {
            // GZip compression
            let mut decoder = GzDecoder::new(&compressed_data[..]);
            let mut decompressed = Vec::new();
            decoder.read_to_end(&mut decompressed)?;
            Ok(Some(decompressed))
        }
        2 => {
            // Zlib compression (standard modern Minecraft)
            let mut decoder = ZlibDecoder::new(&compressed_data[..]);
            let mut decompressed = Vec::new();
            decoder.read_to_end(&mut decompressed)?;
            Ok(Some(decompressed))
        }
        _ => {
            // Uncompressed or custom
            Ok(Some(compressed_data))
        }
    }
}

/// Fast scanner for Anvil regions across a given block radius around (center_x, center_z).
pub fn scan_region_surface(
    region_dir: &Path,
    center_x: i32,
    center_z: i32,
    radius_blocks: i32,
) -> RegionSurfaceMap {
    let mut surface_map = RegionSurfaceMap::new();
    if !region_dir.exists() || !region_dir.is_dir() {
        return surface_map;
    }

    let min_chunk_x = (center_x - radius_blocks) >> 4;
    let max_chunk_x = (center_x + radius_blocks) >> 4;
    let min_chunk_z = (center_z - radius_blocks) >> 4;
    let max_chunk_z = (center_z + radius_blocks) >> 4;

    let min_region_x = min_chunk_x >> 5;
    let max_region_x = max_chunk_x >> 5;
    let min_region_z = min_chunk_z >> 5;
    let max_region_z = max_chunk_z >> 5;

    for rx in min_region_x..=max_region_x {
        for rz in min_region_z..=max_region_z {
            let region_file = region_dir.join(format!("r.{rx}.{rz}.mca"));
            if !region_file.exists() {
                continue;
            }

            for cx in (rx << 5)..=((rx << 5) + 31) {
                if cx < min_chunk_x || cx > max_chunk_x {
                    continue;
                }
                for cz in (rz << 5)..=((rz << 5) + 31) {
                    if cz < min_chunk_z || cz > max_chunk_z {
                        continue;
                    }

                    if let Ok(Some(payload)) = read_chunk_payload(&region_file, cx, cz) {
                        extract_chunk_surface(&payload, cx, cz, &mut surface_map);
                    }
                }
            }
        }
    }

    surface_map
}

/// Scans a single Anvil region file (r.{rx}.{rz}.mca) if present.
pub fn scan_single_region(
    region_dir: &Path,
    rx: i32,
    rz: i32,
) -> RegionSurfaceMap {
    let mut surface_map = RegionSurfaceMap::new();
    let region_file = region_dir.join(format!("r.{rx}.{rz}.mca"));
    if !region_file.is_file() {
        return surface_map;
    }

    for cx in (rx << 5)..=((rx << 5) + 31) {
        for cz in (rz << 5)..=((rz << 5) + 31) {
            if let Ok(Some(payload)) = read_chunk_payload(&region_file, cx, cz) {
                extract_chunk_surface(&payload, cx, cz, &mut surface_map);
            }
        }
    }

    surface_map
}

/// Tests whether a block is air or non-solid passable foliage that should not occlude ground surface.
fn is_transparent_passable(name: &str) -> bool {
    let clean = name.strip_prefix("minecraft:").unwrap_or(name);
    matches!(
        clean,
        "air"
            | "cave_air"
            | "void_air"
            | "short_grass"
            | "tall_grass"
            | "grass"
            | "fern"
            | "large_fern"
            | "dead_bush"
            | "dandelion"
            | "poppy"
            | "blue_orchid"
            | "allium"
            | "azure_bluet"
            | "red_tulip"
            | "orange_tulip"
            | "white_tulip"
            | "pink_tulip"
            | "oxeye_daisy"
            | "cornflower"
            | "lily_of_the_valley"
            | "torch"
            | "wall_torch"
            | "redstone_wire"
            | "lever"
            | "ladder"
            | "vine"
            | "rail"
            | "powered_rail"
            | "detector_rail"
            | "activator_rail"
            | "tripwire"
            | "tripwire_hook"
            | "barrier"
            | "light"
    )
}

/// Decodes chunk NBT payload and extracts true surface block types, elevation, and biomes.
fn extract_chunk_surface(
    payload: &[u8],
    chunk_x: i32,
    chunk_z: i32,
    surface_map: &mut RegionSurfaceMap,
) {
    let root = match parse_nbt_compound(payload) {
        Ok(r) => r,
        Err(_) => return,
    };

    surface_map.chunks_found += 1;

    let base_block_x = chunk_x * 16;
    let base_block_z = chunk_z * 16;

    // Check for "Level" compound (pre-1.18 format) vs modern root
    let root_level = root.get("Level").and_then(|t| t.as_compound());
    let data_map = root_level.unwrap_or(&root);

    let sections_list = data_map.get("sections").or_else(|| data_map.get("Sections"));

    // Track which columns have discovered their surface block
    let mut filled = [[false; 16]; 16];
    let mut columns_left = 256;

    // Default chunk biome
    let mut chunk_biome = "minecraft:plains".to_string();

    if let Some(NbtTag::List(sections)) = sections_list {
        // Collect sections with their Y index
        let mut sorted_sections: Vec<(i32, &HashMap<String, NbtTag>)> = Vec::new();
        for sec_tag in sections {
            if let NbtTag::Compound(sec) = sec_tag {
                let y = sec.get("Y").and_then(|t| match t {
                    NbtTag::Byte(b) => Some(*b as i32),
                    NbtTag::Short(s) => Some(*s as i32),
                    NbtTag::Int(i) => Some(*i),
                    _ => None,
                });
                if let Some(sec_y) = y {
                    // Extract biome candidate if present
                    if let Some(NbtTag::Compound(biomes_tag)) = sec.get("biomes") {
                        if let Some(NbtTag::List(b_pal)) = biomes_tag.get("palette") {
                            for b_entry in b_pal {
                                if let NbtTag::String(b_str) = b_entry {
                                    if !b_str.is_empty() {
                                        chunk_biome = b_str.clone();
                                        break;
                                    }
                                }
                            }
                        }
                    }
                    sorted_sections.push((sec_y, sec));
                }
            }
        }

        // Sort descending: scan from top sky section down to bedrock
        sorted_sections.sort_by(|a, b| b.0.cmp(&a.0));

        for (sec_y, sec) in sorted_sections {
            if columns_left == 0 {
                break;
            }

            let block_states = sec.get("block_states").or_else(|| sec.get("BlockStates")).and_then(|t| t.as_compound());

            if let Some(bs) = block_states {
                let palette = bs.get("palette").or_else(|| bs.get("Palette")).and_then(|t| match t {
                    NbtTag::List(l) => Some(l),
                    _ => None,
                });
                let data = bs.get("data").or_else(|| bs.get("Data")).and_then(|t| match t {
                    NbtTag::LongArray(arr) => Some(arr.as_slice()),
                    _ => None,
                });

                if let Some(pal) = palette {
                    if pal.is_empty() {
                        continue;
                    }

                    // Single-block palette (entire 16x16x16 section is one block)
                    if pal.len() == 1 {
                        let name_opt = if let NbtTag::Compound(first_comp) = &pal[0] {
                            first_comp.get("Name").and_then(|t| match t {
                                NbtTag::String(s) => Some(s.as_str()),
                                _ => None,
                            })
                        } else if let NbtTag::String(s) = &pal[0] {
                            Some(s.as_str())
                        } else {
                            None
                        };

                        if let Some(name) = name_opt {
                            if is_transparent_passable(name) {
                                continue; // All air / transparent, skip section
                            }

                            let base_y = sec_y * 16 + 15;
                            for dz in 0..16 {
                                for dx in 0..16 {
                                    if !filled[dz][dx] {
                                        filled[dz][dx] = true;
                                        columns_left -= 1;
                                        surface_map.insert(SurfacePoint {
                                            x: base_block_x + dx as i32,
                                            z: base_block_z + dz as i32,
                                            y: base_y,
                                            block_id: name.to_string(),
                                            biome_id: chunk_biome.clone(),
                                        });
                                    }
                                }
                            }
                        }
                        continue;
                    }

                    // Multi-block palette with compact bit-packed data
                    let bits_per_block = {
                        let count = pal.len();
                        let mut b = 4;
                        while (1 << b) < count {
                            b += 1;
                        }
                        b
                    };
                    let blocks_per_long = 64 / bits_per_block;
                    let mask = (1u64 << bits_per_block) - 1;

                    if let Some(packed_data) = data {
                        // Scan local Y within section from 15 down to 0
                        for sy in (0..16usize).rev() {
                            if columns_left == 0 {
                                break;
                            }
                            let world_y = sec_y * 16 + (sy as i32);
                            for dz in 0..16usize {
                                for dx in 0..16usize {
                                    if filled[dz][dx] {
                                        continue;
                                    }
                                    let block_idx = (sy * 16 + dz) * 16 + dx;
                                    let long_idx = block_idx / blocks_per_long;
                                    let bit_offset = (block_idx % blocks_per_long) * bits_per_block;
                                    if long_idx < packed_data.len() {
                                        let val = ((packed_data[long_idx] as u64) >> bit_offset) & mask;
                                        if let Some(pal_entry) = pal.get(val as usize) {
                                            let name_opt = if let NbtTag::Compound(c) = pal_entry {
                                                c.get("Name").and_then(|t| match t {
                                                    NbtTag::String(s) => Some(s.as_str()),
                                                    _ => None,
                                                })
                                            } else if let NbtTag::String(s) = pal_entry {
                                                Some(s.as_str())
                                            } else {
                                                None
                                            };

                                            if let Some(name) = name_opt {
                                                if !is_transparent_passable(name) {
                                                    filled[dz][dx] = true;
                                                    columns_left -= 1;
                                                    surface_map.insert(SurfacePoint {
                                                        x: base_block_x + dx as i32,
                                                        z: base_block_z + dz as i32,
                                                        y: world_y,
                                                        block_id: name.to_string(),
                                                        biome_id: chunk_biome.clone(),
                                                    });
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

