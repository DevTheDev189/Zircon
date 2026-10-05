//! Pure Rust top-down rectangular map renderer with hillshading, water depth, and biome tinting.
//! Supports arbitrary coordinate panning (center_x, center_z) and dynamic zoom levels.

use std::collections::HashSet;
use std::io::Cursor;
use image::{ImageBuffer, Rgba};
use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine;

use super::anvil::RegionSurfaceMap;
use super::WorldPreviewResult;

/// Computes a stable 64-bit integer seed from numeric or text input.
pub fn seed_hash(seed: &str) -> u64 {
    let trimmed = seed.trim();
    if let Ok(num) = trimmed.parse::<i64>() {
        num as u64
    } else if trimmed.is_empty() {
        1337424269101u64
    } else {
        let mut h: u64 = 0xcbf29ce484222325;
        for b in trimmed.as_bytes() {
            h ^= *b as u64;
            h = h.wrapping_mul(0x100000001b3);
        }
        h
    }
}

/// Symmetrical integer hash with strong bit-dispersion across positive and negative values.
fn hash2d(x: i32, z: i32, seed: u64) -> f32 {
    let mut n = (x as u64).wrapping_mul(0x517cc1b727220a95)
        ^ (z as u64).wrapping_mul(0x6a297c4d4f8f4f25)
        ^ seed;
    n = (n ^ (n >> 30)).wrapping_mul(0xbf58476d1ce4e5b9u64);
    n = (n ^ (n >> 27)).wrapping_mul(0x94d049bb133111ebu64);
    n ^= n >> 31;
    ((n as u32) as f32) / (u32::MAX as f32)
}

fn smooth_noise(x: f32, z: f32, seed: u64) -> f32 {
    let ix = x.floor() as i32;
    let iz = z.floor() as i32;
    let fx = x - ix as f32;
    let fz = z - iz as f32;

    let u = fx * fx * (3.0 - 2.0 * fx);
    let v = fz * fz * (3.0 - 2.0 * fz);

    let h00 = hash2d(ix, iz, seed);
    let h10 = hash2d(ix + 1, iz, seed);
    let h01 = hash2d(ix, iz + 1, seed);
    let h11 = hash2d(ix + 1, iz + 1, seed);

    let nx0 = h00 + (h10 - h00) * u;
    let nx1 = h01 + (h11 - h01) * u;
    nx0 + (nx1 - nx0) * v
}

fn fbm2d(x: f32, z: f32, seed: u64, octaves: usize) -> f32 {
    let mut total = 0.0;
    let mut freq = 1.0;
    let mut amp = 1.0;
    let mut max_amp = 0.0;
    for i in 0..octaves {
        total += smooth_noise(x * freq, z * freq, seed.wrapping_add(i as u64 * 31337)) * amp;
        max_amp += amp;
        freq *= 2.0;
        amp *= 0.5;
    }
    total / max_amp
}

pub struct SyntheticColumn {
    pub y: i32,
    pub block_id: &'static str,
    pub biome_id: &'static str,
}

/// Generates procedural seed-coherent Minecraft surface terrain columns.
/// Strongly modulated by the seed so every seed produces unique continents, islands, and ranges.
pub fn sample_synthetic_column(x: i32, z: i32, seed_val: u64) -> SyntheticColumn {
    // Large prime coordinate shift derived from seed to sample completely unique noise locations per seed
    let seed_shift_x = (((seed_val.wrapping_mul(0x517cc1b727220a95)) >> 16) & 0x7FFFF) as f32 - 250_000.0;
    let seed_shift_z = (((seed_val.wrapping_mul(0x6a297c4d4f8f4f25)) >> 16) & 0x7FFFF) as f32 - 250_000.0;

    let fx = x as f32 + seed_shift_x;
    let fz = z as f32 + seed_shift_z;

    // Continentalness (frequency ~ 1 / 140 blocks to produce oceans, coasts, and landmasses across the view)
    let cont = fbm2d(fx * 0.007, fz * 0.007, seed_val, 4);

    // Mountainous peaks and ridge noise
    let mtn = fbm2d(fx * 0.015, fz * 0.015, seed_val ^ 0x9e3779b97f4a7c15, 4);
    let mtn_peaks = (mtn - 0.44).max(0.0) / 0.56;

    // Climate / Temperature and Humidity
    let temp = fbm2d(fx * 0.0035, fz * 0.0035, seed_val.wrapping_add(1013), 3);
    let humid = fbm2d(fx * 0.0035, fz * 0.0035, seed_val.wrapping_add(2027), 3);

    // River noise: valleys when near 0.5
    let river_val = (fbm2d(fx * 0.009, fz * 0.009, seed_val.wrapping_add(5555), 3) - 0.5).abs();
    let is_river = river_val < 0.032 && cont > 0.42 && cont < 0.88;

    if cont < 0.38 {
        // Ocean / Deep Ocean
        let y = (38.0 + cont * 62.0) as i32;
        let biome = if cont < 0.24 {
            "minecraft:deep_ocean"
        } else if temp > 0.65 {
            "minecraft:warm_ocean"
        } else {
            "minecraft:ocean"
        };
        SyntheticColumn {
            y,
            block_id: "minecraft:water",
            biome_id: biome,
        }
    } else if cont < 0.43 {
        // Coastal Shore / Beach
        let y = 62 + ((cont - 0.38) * 40.0) as i32;
        let biome = if temp < 0.25 {
            "minecraft:snowy_beach"
        } else if mtn > 0.60 {
            "minecraft:stony_shore"
        } else {
            "minecraft:beach"
        };
        let block = if biome == "minecraft:stony_shore" {
            "minecraft:stone"
        } else {
            "minecraft:sand"
        };
        SyntheticColumn {
            y,
            block_id: block,
            biome_id: biome,
        }
    } else if is_river {
        // Meandering inland river
        SyntheticColumn {
            y: 61,
            block_id: "minecraft:water",
            biome_id: "minecraft:river",
        }
    } else {
        // Inland terrain & mountain ranges
        let base_y = 63.0 + (cont - 0.43) * 36.0;
        let peak_boost = (mtn_peaks.powf(1.8)) * 125.0;
        let y = (base_y + peak_boost) as i32;

        if y > 124 {
            if temp < 0.45 || y > 148 {
                SyntheticColumn {
                    y,
                    block_id: "minecraft:snow_block",
                    biome_id: "minecraft:jagged_peaks",
                }
            } else {
                SyntheticColumn {
                    y,
                    block_id: "minecraft:stone",
                    biome_id: "minecraft:stony_peaks",
                }
            }
        } else if temp > 0.70 && humid < 0.35 {
            SyntheticColumn {
                y,
                block_id: "minecraft:sand",
                biome_id: "minecraft:desert",
            }
        } else if temp < 0.25 {
            SyntheticColumn {
                y,
                block_id: "minecraft:snow_block",
                biome_id: "minecraft:snowy_plains",
            }
        } else if temp < 0.38 {
            SyntheticColumn {
                y,
                block_id: "minecraft:grass_block",
                biome_id: "minecraft:taiga",
            }
        } else if humid > 0.68 && temp > 0.45 {
            SyntheticColumn {
                y,
                block_id: "minecraft:grass_block",
                biome_id: "minecraft:swamp",
            }
        } else if humid > 0.52 {
            SyntheticColumn {
                y,
                block_id: "minecraft:grass_block",
                biome_id: "minecraft:forest",
            }
        } else {
            let biome = if mtn > 0.48 {
                "terralith:highlands"
            } else {
                "minecraft:plains"
            };
            SyntheticColumn {
                y,
                block_id: "minecraft:grass_block",
                biome_id: biome,
            }
        }
    }
}

/// Renders a full rectangular top-down map supporting arbitrary panning (center_x, center_z) and zoom levels.
pub fn render_preview_map(
    surface_map: &RegionSurfaceMap,
    seed: &str,
    center_x: i32,
    center_z: i32,
    radius_blocks: i32,
    zoom: f32,
    image_size: u32,
) -> WorldPreviewResult {
    let start = std::time::Instant::now();
    let mut img: ImageBuffer<Rgba<u8>, Vec<u8>> = ImageBuffer::new(image_size, image_size);
    let mut biomes_set = HashSet::new();

    let _seed_val = seed_hash(seed);
    let half_size = image_size as f32 / 2.0;

    // Zoom scale adjustment: zoom = 1.0 means radius_blocks fits half_size
    let effective_zoom = zoom.clamp(0.2, 8.0);
    let scale = (radius_blocks as f32 / effective_zoom) / half_size;

    let mut actual_min_y = 320;
    let mut actual_max_y = -64;

    for py in 0..image_size {
        for px in 0..image_size {
            let dx = px as f32 - half_size;
            let dz = py as f32 - half_size;

            // Map pixel coordinate to Minecraft world block coords centered at (center_x, center_z)
            let world_x = (center_x as f32 + dx * scale).round() as i32;
            let world_z = (center_z as f32 + dz * scale).round() as i32;

            // Mark World Origin (0,0) with high-visibility spawn indicator crosshair
            let is_world_spawn_x = world_x == 0 && world_z.abs() <= 3;
            let is_world_spawn_z = world_z == 0 && world_x.abs() <= 3;

            // Current camera reticle crosshair at exact center of viewport
            let is_reticle = (px == (image_size / 2) && (py as i32 - half_size as i32).abs() <= 5)
                || (py == (image_size / 2) && (px as i32 - half_size as i32).abs() <= 5);

            if let Some(point) = surface_map.points.get(&(world_x, world_z)) {
                // Real generated block
                let y = point.y;
                let block_id = point.block_id.as_str();
                let biome_id = point.biome_id.as_str();

                if y < actual_min_y {
                    actual_min_y = y;
                }
                if y > actual_max_y {
                    actual_max_y = y;
                }

                biomes_set.insert(biome_id.to_string());

                // Northwest hillshading from neighbor
                let nw_x = world_x - 1;
                let nw_z = world_z - 1;
                let nw_y = surface_map.points.get(&(nw_x, nw_z)).map(|p| p.y).unwrap_or(point.y);
                let delta_y = y - nw_y;
                let shade = 1.0 + (delta_y as f32 * 0.09).clamp(-0.38, 0.38);

                let base_color = get_block_color(block_id, biome_id, y);
                let r = ((base_color[0] as f32 * shade).clamp(0.0, 255.0)) as u8;
                let g = ((base_color[1] as f32 * shade).clamp(0.0, 255.0)) as u8;
                let b = ((base_color[2] as f32 * shade).clamp(0.0, 255.0)) as u8;

                if is_world_spawn_x || is_world_spawn_z {
                    img.put_pixel(px, py, Rgba([255, 60, 60, 255])); // Red for spawn (0,0)
                } else if is_reticle {
                    img.put_pixel(px, py, Rgba([71, 210, 201, 255])); // Cyan reticle
                } else {
                    img.put_pixel(px, py, Rgba([r, g, b, 255]));
                }
            } else {
                // Ungenerated chunk void: Render dark sleek void with subtle chunk boundary lines
                let is_chunk_border = (world_x % 16 == 0) || (world_z % 16 == 0);
                if is_world_spawn_x || is_world_spawn_z {
                    img.put_pixel(px, py, Rgba([255, 60, 60, 255]));
                } else if is_reticle {
                    img.put_pixel(px, py, Rgba([71, 210, 201, 255]));
                } else if is_chunk_border {
                    img.put_pixel(px, py, Rgba([26, 36, 54, 255])); // Chunk grid line in void
                } else {
                    img.put_pixel(px, py, Rgba([12, 16, 23, 255])); // Deep void
                }
            }
        }
    }

    let mut png_bytes = Vec::new();
    let _ = img.write_to(&mut Cursor::new(&mut png_bytes), image::ImageFormat::Png);
    let image_data_url = format!("data:image/png;base64,{}", BASE64.encode(&png_bytes));

    let mut biomes_found: Vec<String> = biomes_set.into_iter().collect();
    biomes_found.sort();

    WorldPreviewResult {
        seed: seed.to_string(),
        radius_blocks,
        center_x,
        center_z,
        zoom: effective_zoom,
        image_data_url,
        biomes_found,
        min_elevation: if actual_min_y <= 320 { actual_min_y } else { 0 },
        max_elevation: if actual_max_y >= -64 { actual_max_y } else { 0 },
        render_time_ms: start.elapsed().as_millis() as u64,
    }
}

/// Returns standard RGB palette color for blocks with biome tinting and depth gradation.
fn get_block_color(block_id: &str, biome_id: &str, y: i32) -> [u8; 3] {
    let clean = block_id.strip_prefix("minecraft:").unwrap_or(block_id);

    // 1. Liquids & Ice
    if clean == "water" || clean == "bubble_column" {
        return if y < 45 {
            [20, 55, 120] // Deep ocean
        } else if y < 58 {
            [32, 85, 160] // Ocean shelf
        } else {
            [45, 125, 200] // Shallow coastal/river water
        };
    }
    if clean == "lava" {
        return [235, 95, 22];
    }
    if clean.contains("ice") {
        return [158, 202, 255];
    }
    if clean.contains("snow") {
        return [245, 250, 255];
    }

    // 2. Deepslate Family (All tiles, bricks, stairs, slabs, walls, ores, cobbled)
    if clean.contains("deepslate") {
        return [72, 72, 76];
    }
    // Blackstone & Basalt
    if clean.contains("blackstone") {
        return [42, 38, 44];
    }
    if clean.contains("basalt") {
        return [81, 81, 86];
    }
    if clean.contains("tuff") {
        return [108, 109, 102];
    }

    // 3. Wood Types & Variants (Stairs, slabs, planks, logs, wood, fences, trapdoors, doors)
    if clean.contains("dark_oak") {
        return if clean.contains("leaves") { [48, 88, 30] } else { [60, 40, 22] };
    }
    if clean.contains("spruce") {
        return if clean.contains("leaves") { [61, 99, 65] } else { [78, 54, 32] };
    }
    if clean.contains("birch") {
        return if clean.contains("leaves") {
            [110, 157, 71]
        } else if clean.contains("log") || clean.contains("wood") {
            [216, 216, 214]
        } else {
            [196, 176, 118]
        };
    }
    if clean.contains("jungle") {
        return if clean.contains("leaves") { [58, 125, 38] } else { [154, 110, 77] };
    }
    if clean.contains("acacia") {
        return if clean.contains("leaves") { [75, 115, 45] } else { [168, 90, 50] };
    }
    if clean.contains("mangrove") {
        return if clean.contains("leaves") { [68, 120, 42] } else { [117, 52, 45] };
    }
    if clean.contains("cherry") {
        return if clean.contains("leaves") { [242, 178, 195] } else { [222, 158, 158] };
    }
    if clean.contains("bamboo") {
        return [192, 175, 79];
    }
    if clean.contains("crimson") {
        return if clean.contains("nylium") { [130, 25, 25] } else { [101, 38, 54] };
    }
    if clean.contains("warped") {
        return if clean.contains("nylium") { [42, 115, 110] } else { [43, 104, 99] };
    }
    if clean.contains("oak") {
        return if clean.contains("leaves") { [58, 125, 38] } else { [162, 130, 78] };
    }

    // 4. Copper Family (All oxidation stages & shapes)
    if clean.contains("oxidized") {
        return [82, 163, 134];
    }
    if clean.contains("weathered") {
        return [106, 153, 127];
    }
    if clean.contains("exposed") {
        return [159, 115, 99];
    }
    if clean.contains("copper") {
        return [194, 107, 78];
    }

    // 5. Bricks & Mud
    if clean.contains("mud_brick") {
        return [89, 70, 55];
    }
    if clean.contains("packed_mud") || clean == "mud" {
        return [65, 59, 53];
    }
    if clean.contains("nether_brick") {
        return [44, 21, 26];
    }
    if clean.contains("prismarine") {
        return if clean.contains("dark") { [52, 92, 85] } else { [92, 156, 142] };
    }
    if clean.contains("quartz") {
        return [235, 230, 224];
    }
    if clean.contains("purpur") {
        return [169, 125, 169];
    }
    if clean.contains("brick") {
        return [150, 75, 55]; // Standard red clay bricks
    }

    // 6. Stone, Cobblestone, Andesite, Diorite, Granite
    if clean.contains("granite") {
        return [150, 103, 84];
    }
    if clean.contains("diorite") || clean.contains("calcite") {
        return [185, 185, 187];
    }
    if clean.contains("andesite") {
        return [132, 134, 133];
    }
    if clean.contains("dripstone") {
        return [134, 105, 90];
    }
    if clean.contains("end_stone") {
        return [221, 223, 165];
    }
    if clean.contains("cobblestone") || clean.contains("stone") {
        return if clean.contains("mossy") {
            [105, 122, 95]
        } else if clean.contains("smooth") {
            [150, 150, 152]
        } else {
            [125, 128, 132]
        };
    }
    if clean.contains("gravel") {
        return [136, 130, 128];
    }
    if clean.contains("obsidian") {
        return [22, 16, 32];
    }
    if clean.contains("bedrock") {
        return [52, 52, 52];
    }
    if clean.contains("netherrack") {
        return [112, 36, 36];
    }
    if clean.contains("soul_sand") || clean.contains("soul_soil") {
        return [77, 58, 46];
    }

    // 7. Sands & Soils
    if clean.contains("red_sand") {
        return [192, 104, 38];
    }
    if clean.contains("sand") {
        return [225, 212, 162];
    }
    if clean.contains("farmland") {
        return [80, 52, 28];
    }
    if clean.contains("dirt_path") || clean.contains("path") {
        return [158, 124, 76];
    }
    if clean.contains("podzol") {
        return [92, 65, 30];
    }
    if clean.contains("mycelium") {
        return [112, 100, 106];
    }
    if clean.contains("dirt") {
        return [134, 96, 67];
    }
    if clean.contains("clay") {
        return [160, 166, 179];
    }
    if clean.contains("moss") {
        return [90, 130, 50];
    }

    // 8. 16 Color Palette (Concrete, Wool, Terracotta, Glass, Glazed Terracotta)
    if clean.contains("white_") { return [235, 235, 235]; }
    if clean.contains("light_gray_") { return [145, 145, 145]; }
    if clean.contains("gray_") { return [70, 75, 80]; }
    if clean.contains("black_") { return [25, 25, 30]; }
    if clean.contains("brown_") { return [105, 65, 38]; }
    if clean.contains("red_") { return [165, 38, 35]; }
    if clean.contains("orange_") { return [225, 115, 25]; }
    if clean.contains("yellow_") { return [230, 200, 45]; }
    if clean.contains("lime_") { return [115, 190, 35]; }
    if clean.contains("green_") { return [85, 115, 30]; }
    if clean.contains("cyan_") { return [22, 125, 135]; }
    if clean.contains("light_blue_") { return [95, 170, 225]; }
    if clean.contains("blue_") { return [45, 60, 160]; }
    if clean.contains("purple_") { return [120, 45, 160]; }
    if clean.contains("magenta_") { return [185, 65, 170]; }
    if clean.contains("pink_") { return [235, 140, 170]; }
    if clean == "terracotta" { return [152, 94, 67]; }

    // 9. Minerals & Ores
    if clean.contains("gold") { return [245, 205, 55]; }
    if clean.contains("iron") { return [215, 215, 215]; }
    if clean.contains("diamond") { return [95, 225, 220]; }
    if clean.contains("emerald") { return [45, 195, 95]; }
    if clean.contains("lapis") { return [35, 75, 160]; }
    if clean.contains("redstone") { return [185, 25, 20]; }
    if clean.contains("amethyst") { return [140, 95, 205]; }
    if clean.contains("netherite") { return [65, 58, 62]; }

    // 10. Agricultural & Flora
    if clean.contains("hay_block") { return [175, 140, 25]; }
    if clean.contains("sponge") { return [195, 190, 65]; }
    if clean.contains("pumpkin") || clean.contains("jack_o_lantern") { return [210, 115, 25]; }
    if clean.contains("melon") { return [110, 145, 35]; }
    if clean.contains("glass") { return [195, 225, 245]; }

    // 11. Generic fallback by keyword and biome tinting
    if clean.contains("leaves") || clean.contains("plant") || clean.contains("grass") || clean.contains("flower") || clean.contains("vine") {
        if biome_id.contains("desert") || biome_id.contains("badlands") {
            [160, 150, 65]
        } else if biome_id.contains("swamp") {
            [85, 98, 42]
        } else if biome_id.contains("taiga") || biome_id.contains("spruce") {
            [61, 99, 65]
        } else {
            [58, 125, 38]
        }
    } else if clean.contains("log") || clean.contains("wood") || clean.contains("stem") {
        [110, 85, 52]
    } else if clean.contains("ore") {
        [125, 128, 132]
    } else if biome_id.contains("desert") || biome_id.contains("badlands") {
        [210, 195, 130]
    } else if biome_id.contains("swamp") {
        [96, 110, 52]
    } else if biome_id.contains("taiga") || biome_id.contains("spruce") {
        [75, 115, 75]
    } else if biome_id.contains("highlands") {
        [112, 178, 76]
    } else if biome_id.contains("forest") || biome_id.contains("dark_forest") {
        [78, 145, 52]
    } else {
        [95, 165, 65] // Lush plains green
    }
}

/// Renders a single 512x512 region tile into compressed PNG bytes.
pub fn render_region_tile(
    region_dir: &std::path::Path,
    rx: i32,
    rz: i32,
) -> Option<Vec<u8>> {
    let surface_map = super::anvil::scan_single_region(region_dir, rx, rz);
    if surface_map.points.is_empty() {
        return None;
    }

    let mut img: ImageBuffer<Rgba<u8>, Vec<u8>> = ImageBuffer::new(512, 512);

    for pz in 0..512 {
        for px in 0..512 {
            let world_x = rx * 512 + px as i32;
            let world_z = rz * 512 + pz as i32;

            if let Some(point) = surface_map.points.get(&(world_x, world_z)) {
                let y = point.y;
                let block_id = point.block_id.as_str();
                let biome_id = point.biome_id.as_str();

                let nw_x = world_x - 1;
                let nw_z = world_z - 1;
                let nw_y = surface_map.points.get(&(nw_x, nw_z)).map(|p| p.y).unwrap_or(point.y);
                let delta_y = y - nw_y;
                let shade = 1.0 + (delta_y as f32 * 0.09).clamp(-0.38, 0.38);

                let base_color = get_block_color(block_id, biome_id, y);
                let r = ((base_color[0] as f32 * shade).clamp(0.0, 255.0)) as u8;
                let g = ((base_color[1] as f32 * shade).clamp(0.0, 255.0)) as u8;
                let b = ((base_color[2] as f32 * shade).clamp(0.0, 255.0)) as u8;

                // Origin spawn crosshair if (0,0) falls in this tile
                let is_world_spawn_x = world_x == 0 && world_z.abs() <= 3;
                let is_world_spawn_z = world_z == 0 && world_x.abs() <= 3;

                if is_world_spawn_x || is_world_spawn_z {
                    img.put_pixel(px, pz, Rgba([255, 60, 60, 255]));
                } else {
                    img.put_pixel(px, pz, Rgba([r, g, b, 255]));
                }
            } else {
                let is_chunk_border = (world_x % 16 == 0) || (world_z % 16 == 0);
                if is_chunk_border {
                    img.put_pixel(px, pz, Rgba([26, 36, 54, 255]));
                } else {
                    img.put_pixel(px, pz, Rgba([12, 16, 23, 255]));
                }
            }
        }
    }

    let mut png_bytes = Vec::new();
    let _ = img.write_to(&mut Cursor::new(&mut png_bytes), image::ImageFormat::Png);
    Some(png_bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::anvil::SurfacePoint;

    #[test]
    fn test_render_preview_with_generated_chunk_points() {
        let mut map = RegionSurfaceMap::new();
        // Insert real surface points
        for x in 0..16 {
            for z in 0..16 {
                map.insert(SurfacePoint {
                    x,
                    z,
                    y: 64,
                    block_id: "minecraft:grass_block".to_string(),
                    biome_id: "minecraft:plains".to_string(),
                });
            }
        }
        let res = render_preview_map(&map, "test", 0, 0, 64, 1.0, 128);
        assert!(!res.image_data_url.is_empty());
        assert_eq!(res.min_elevation, 64);
        assert_eq!(res.max_elevation, 64);
    }

    #[test]
    fn test_render_preview_pan_and_zoom() {
        let mut map = RegionSurfaceMap::new();
        map.insert(SurfacePoint {
            x: 0,
            z: 0,
            y: 70,
            block_id: "minecraft:stone".to_string(),
            biome_id: "minecraft:stony_peaks".to_string(),
        });
        map.insert(SurfacePoint {
            x: 200,
            z: 200,
            y: 60,
            block_id: "minecraft:sand".to_string(),
            biome_id: "minecraft:desert".to_string(),
        });

        let center_res = render_preview_map(&map, "42", 0, 0, 384, 1.0, 128);
        let panned_res = render_preview_map(&map, "42", 200, 200, 384, 1.0, 128);
        let zoomed_res = render_preview_map(&map, "42", 0, 0, 384, 2.0, 128);

        assert_ne!(center_res.image_data_url, panned_res.image_data_url, "Panning must produce different coordinates and view");
        assert_ne!(center_res.image_data_url, zoomed_res.image_data_url, "Zooming must change image output");
    }

    #[test]
    fn test_get_block_color_palette() {
        // Deepslate stairs & tiles must render dark slate charcoal, NOT plains green
        let deepslate_stairs = get_block_color("minecraft:deepslate_tile_stairs", "minecraft:plains", 64);
        assert_eq!(deepslate_stairs, [72, 72, 76]);

        let deepslate_brick_slab = get_block_color("minecraft:deepslate_brick_slab", "minecraft:plains", 64);
        assert_eq!(deepslate_brick_slab, [72, 72, 76]);

        // Spruce stairs must render dark spruce brown, NOT plains green
        let spruce_stairs = get_block_color("minecraft:spruce_stairs", "minecraft:plains", 64);
        assert_eq!(spruce_stairs, [78, 54, 32]);

        let dark_oak_stairs = get_block_color("minecraft:dark_oak_stairs", "minecraft:plains", 64);
        assert_eq!(dark_oak_stairs, [60, 40, 22]);

        // Copper stairs
        let cut_copper_stairs = get_block_color("minecraft:cut_copper_stairs", "minecraft:plains", 64);
        assert_eq!(cut_copper_stairs, [194, 107, 78]);

        let oxidized_copper_stairs = get_block_color("minecraft:oxidized_cut_copper_stairs", "minecraft:plains", 64);
        assert_eq!(oxidized_copper_stairs, [82, 163, 134]);
    }
}
