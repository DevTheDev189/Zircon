//! Minecraft world directory analysis and dimension layout normalization.
//!
//! Handles:
//! - Bukkit / Spigot / Paper multi-directory dimension restructuring:
//!   `world_nether/` + `world_the_end/` -> `world/DIM-1/` + `world/DIM1/`
//! - World layout analysis (counting chunks, player inventories, advancements, stats, entities)
//! - Preserving 100% of chunk, player, and entity data without data loss.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::nbt::{read_level_dat, LevelDatInfo};

/// World inspection statistics summary for pre-flight display.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct WorldSummary {
    /// Name of the primary world directory (e.g. "world").
    pub folder_name: String,
    /// Metadata read from `level.dat`.
    pub level_dat: Option<LevelDatInfo>,
    /// Detected directory layout ("legacy_1_21", "unified_26", or "bukkit_split").
    pub detected_layout: String,
    /// Number of overworld region files.
    pub overworld_chunks: usize,
    /// Number of nether region files.
    pub nether_chunks: usize,
    /// Number of the end region files.
    pub end_chunks: usize,
    /// Total region files count across all dimensions.
    pub total_chunks: usize,
    /// Number of unique player data records.
    pub player_count: usize,
    /// Number of advancement files.
    pub advancements_count: usize,
    /// Number of statistics files.
    pub stats_count: usize,
    /// Total entity storage region files.
    pub entities_count: usize,
    /// Total point-of-interest region files.
    pub poi_count: usize,
    /// Whether Bukkit/Paper dimension split was detected (`world_nether` or `world_the_end`).
    pub bukkit_dimensions_detected: bool,
}

/// Discovers the primary world directory inside an unpacked server directory.
/// Checks `server.properties` `level-name` first (if available), then falls back to `world/` or any folder containing `level.dat`.
pub fn discover_world_dir(server_root: &Path, level_name: Option<&str>) -> Option<PathBuf> {
    if let Some(name) = level_name {
        let candidate = server_root.join(name.trim());
        if candidate.join("level.dat").is_file() {
            return Some(candidate);
        }
    }

    let default_world = server_root.join("world");
    if default_world.join("level.dat").is_file() {
        return Some(default_world);
    }

    // Direct world upload where level.dat is at server_root
    if server_root.join("level.dat").is_file() {
        return Some(server_root.to_path_buf());
    }

    // Search top-level subdirectories for any folder containing level.dat
    if let Ok(entries) = fs::read_dir(server_root) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() && path.join("level.dat").is_file() {
                return Some(path);
            }
        }
    }

    None
}

/// Analyzes an unpacked world folder and returns full summary statistics across both legacy and modern directory structures.
pub fn analyze_world(server_root: &Path, world_dir: &Path) -> WorldSummary {
    let folder_name = world_dir
        .file_name()
        .map(|f| f.to_string_lossy().to_string())
        .unwrap_or_else(|| "world".to_string());

    let level_dat_path = world_dir.join("level.dat");
    let level_dat = if level_dat_path.is_file() {
        read_level_dat(&level_dat_path).ok()
    } else {
        None
    };

    // Check for Bukkit/Paper dimension folders at server root
    let nether_bukkit_name = format!("{folder_name}_nether");
    let end_bukkit_name = format!("{folder_name}_the_end");
    let bukkit_nether = server_root.join(&nether_bukkit_name);
    let bukkit_end = server_root.join(&end_bukkit_name);

    let bukkit_dimensions_detected =
        (bukkit_nether.is_dir() && is_dimension_folder(&bukkit_nether))
            || (bukkit_end.is_dir() && is_dimension_folder(&bukkit_end));

    // Overworld chunks: count modern 26.x unified path or legacy root path
    let modern_overworld = world_dir.join("dimensions").join("minecraft").join("overworld");
    let mut overworld_chunks = count_mca_files(&modern_overworld.join("region"));
    if overworld_chunks == 0 {
        overworld_chunks = count_mca_files(&world_dir.join("region"));
    }

    // Nether chunks: modern unified, legacy DIM-1, or bukkit_nether
    let modern_nether = world_dir.join("dimensions").join("minecraft").join("the_nether");
    let mut nether_chunks = count_mca_files(&modern_nether.join("region"));
    if nether_chunks == 0 {
        nether_chunks = count_mca_files(&world_dir.join("DIM-1").join("region"));
    }
    if nether_chunks == 0 && bukkit_nether.is_dir() {
        nether_chunks = count_mca_files(&bukkit_nether.join("DIM-1").join("region"))
            + count_mca_files(&bukkit_nether.join("region"));
    }

    // End chunks: modern unified, legacy DIM1, or bukkit_end
    let modern_end = world_dir.join("dimensions").join("minecraft").join("the_end");
    let mut end_chunks = count_mca_files(&modern_end.join("region"));
    if end_chunks == 0 {
        end_chunks = count_mca_files(&world_dir.join("DIM1").join("region"));
    }
    if end_chunks == 0 && bukkit_end.is_dir() {
        end_chunks = count_mca_files(&bukkit_end.join("DIM1").join("region"))
            + count_mca_files(&bukkit_end.join("region"));
    }

    // Players: check modern players/data/ or legacy playerdata/
    let mut player_count = count_files_matching(&world_dir.join("players").join("data"), |ext| ext == "dat");
    if player_count == 0 {
        player_count = count_files_matching(&world_dir.join("playerdata"), |ext| ext == "dat");
    }

    // Advancements & Stats
    let mut advancements_count = count_files_matching(&world_dir.join("players").join("advancements"), |ext| ext == "json");
    if advancements_count == 0 {
        advancements_count = count_files_matching(&world_dir.join("advancements"), |ext| ext == "json");
    }

    let mut stats_count = count_files_matching(&world_dir.join("players").join("stats"), |ext| ext == "json");
    if stats_count == 0 {
        stats_count = count_files_matching(&world_dir.join("stats"), |ext| ext == "json");
    }

    // Entities & POI
    let mut entities_count = count_mca_files(&modern_overworld.join("entities"))
        + count_mca_files(&modern_nether.join("entities"))
        + count_mca_files(&modern_end.join("entities"));
    if entities_count == 0 {
        entities_count = count_mca_files(&world_dir.join("entities"))
            + count_mca_files(&world_dir.join("DIM-1").join("entities"))
            + count_mca_files(&world_dir.join("DIM1").join("entities"));
    }

    let mut poi_count = count_mca_files(&modern_overworld.join("poi"))
        + count_mca_files(&modern_nether.join("poi"))
        + count_mca_files(&modern_end.join("poi"));
    if poi_count == 0 {
        poi_count = count_mca_files(&world_dir.join("poi"))
            + count_mca_files(&world_dir.join("DIM-1").join("poi"))
            + count_mca_files(&world_dir.join("DIM1").join("poi"));
    }

    if bukkit_nether.is_dir() {
        entities_count += count_mca_files(&bukkit_nether.join("entities"))
            + count_mca_files(&bukkit_nether.join("DIM-1").join("entities"));
        poi_count += count_mca_files(&bukkit_nether.join("poi"))
            + count_mca_files(&bukkit_nether.join("DIM-1").join("poi"));
    }
    if bukkit_end.is_dir() {
        entities_count += count_mca_files(&bukkit_end.join("entities"))
            + count_mca_files(&bukkit_end.join("DIM1").join("entities"));
        poi_count += count_mca_files(&bukkit_end.join("poi"))
            + count_mca_files(&bukkit_end.join("DIM1").join("poi"));
    }

    let detected_layout = if modern_overworld.join("region").is_dir() || world_dir.join("players").join("data").is_dir() {
        "unified_26".to_string()
    } else if bukkit_dimensions_detected {
        "bukkit_split".to_string()
    } else {
        "legacy_1_21".to_string()
    };

    let total_chunks = overworld_chunks + nether_chunks + end_chunks;

    WorldSummary {
        folder_name,
        level_dat,
        detected_layout,
        overworld_chunks,
        nether_chunks,
        end_chunks,
        total_chunks,
        player_count,
        advancements_count,
        stats_count,
        entities_count,
        poi_count,
        bukkit_dimensions_detected,
    }
}

/// Converts Bukkit/Paper separated dimension directories into standard Vanilla/Fabric/Forge layout.
///
/// Restructures:
/// `<server_root>/<world>_nether/` $\rightarrow$ `<world_dir>/DIM-1/`
/// `<server_root>/<world>_the_end/` $\rightarrow$ `<world_dir>/DIM1/`
pub fn normalize_bukkit_dimensions(
    server_root: &Path,
    world_dir: &Path,
) -> Result<usize, io::Error> {
    let folder_name = world_dir
        .file_name()
        .map(|f| f.to_string_lossy().to_string())
        .unwrap_or_else(|| "world".to_string());

    let mut moved_count = 0;

    // 1. Nether conversion
    let bukkit_nether = server_root.join(format!("{folder_name}_nether"));
    let target_nether = world_dir.join("DIM-1");

    if bukkit_nether.is_dir() {
        fs::create_dir_all(&target_nether)?;
        // Check if bukkit_nether contains nested DIM-1
        let nested_nether = bukkit_nether.join("DIM-1");
        if nested_nether.is_dir() {
            moved_count += move_directory_contents(&nested_nether, &target_nether)?;
        }
        // Also move any top-level region, entities, poi, data from bukkit_nether
        moved_count += move_dimension_subfolders(&bukkit_nether, &target_nether)?;
        // Clean up empty bukkit_nether dir
        let _ = fs::remove_dir_all(&bukkit_nether);
    }

    // 2. The End conversion
    let bukkit_end = server_root.join(format!("{folder_name}_the_end"));
    let target_end = world_dir.join("DIM1");

    if bukkit_end.is_dir() {
        fs::create_dir_all(&target_end)?;
        // Check if bukkit_end contains nested DIM1
        let nested_end = bukkit_end.join("DIM1");
        if nested_end.is_dir() {
            moved_count += move_directory_contents(&nested_end, &target_end)?;
        }
        // Also move any top-level region, entities, poi, data from bukkit_end
        moved_count += move_dimension_subfolders(&bukkit_end, &target_end)?;
        // Clean up empty bukkit_end dir
        let _ = fs::remove_dir_all(&bukkit_end);
    }

    // 3. Clean up any shadow dimensions/minecraft folder from Bukkit/Paper
    let nested_mc_dim = world_dir.join("dimensions").join("minecraft");
    if nested_mc_dim.is_dir() {
        let _ = fs::remove_dir_all(&nested_mc_dim);
        let dim_dir = world_dir.join("dimensions");
        if let Ok(mut entries) = fs::read_dir(&dim_dir) {
            if entries.next().is_none() {
                let _ = fs::remove_dir(&dim_dir);
            }
        }
    }

    Ok(moved_count)
}

/// Automatically adjusts world folder structure to match target Minecraft version layouts.
/// In Minecraft 26.x+, overworld, nether, end, and player data are stored in unified `dimensions/` and `players/` hierarchies.
/// For older Minecraft versions (<26.x), flattens any modern dimensions and players directories into legacy root paths.
pub fn migrate_world_layout_to_target_version(world_dir: &Path, target_mc_version: &str) -> Result<(), io::Error> {
    let clean = target_mc_version.trim();
    let is_modern = clean.starts_with("26.") || clean.starts_with("27.");

    if is_modern {
        // 1. Overworld
        let overworld_dim = world_dir.join("dimensions").join("minecraft").join("overworld");
        let src_region = world_dir.join("region");
        if src_region.is_dir() {
            let dst_region = overworld_dim.join("region");
            fs::create_dir_all(&dst_region)?;
            let _ = move_directory_contents(&src_region, &dst_region);
            let _ = fs::remove_dir_all(&src_region);
        }
        let src_poi = world_dir.join("poi");
        if src_poi.is_dir() {
            let dst_poi = overworld_dim.join("poi");
            fs::create_dir_all(&dst_poi)?;
            let _ = move_directory_contents(&src_poi, &dst_poi);
            let _ = fs::remove_dir_all(&src_poi);
        }
        let src_entities = world_dir.join("entities");
        if src_entities.is_dir() {
            let dst_entities = overworld_dim.join("entities");
            fs::create_dir_all(&dst_entities)?;
            let _ = move_directory_contents(&src_entities, &dst_entities);
            let _ = fs::remove_dir_all(&src_entities);
        }

        // 2. Nether & End
        let src_nether = world_dir.join("DIM-1");
        if src_nether.is_dir() {
            let dst_nether = world_dir.join("dimensions").join("minecraft").join("the_nether");
            fs::create_dir_all(&dst_nether)?;
            let _ = move_directory_contents(&src_nether, &dst_nether);
            let _ = fs::remove_dir_all(&src_nether);
        }
        let src_end = world_dir.join("DIM1");
        if src_end.is_dir() {
            let dst_end = world_dir.join("dimensions").join("minecraft").join("the_end");
            fs::create_dir_all(&dst_end)?;
            let _ = move_directory_contents(&src_end, &dst_end);
            let _ = fs::remove_dir_all(&src_end);
        }

        // 3. Players data, advancements, stats
        let src_playerdata = world_dir.join("playerdata");
        if src_playerdata.is_dir() {
            let dst_players_data = world_dir.join("players").join("data");
            fs::create_dir_all(&dst_players_data)?;
            let _ = move_directory_contents(&src_playerdata, &dst_players_data);
            let _ = fs::remove_dir_all(&src_playerdata);
        }
        let src_advancements = world_dir.join("advancements");
        if src_advancements.is_dir() {
            let dst_players_adv = world_dir.join("players").join("advancements");
            fs::create_dir_all(&dst_players_adv)?;
            let _ = move_directory_contents(&src_advancements, &dst_players_adv);
            let _ = fs::remove_dir_all(&src_advancements);
        }
        let src_stats = world_dir.join("stats");
        if src_stats.is_dir() {
            let dst_players_stats = world_dir.join("players").join("stats");
            fs::create_dir_all(&dst_players_stats)?;
            let _ = move_directory_contents(&src_stats, &dst_players_stats);
            let _ = fs::remove_dir_all(&src_stats);
        }
    } else {
        // Target is legacy (< 26.x). If world is in 26.x unified layout, flatten back to root.
        let modern_overworld = world_dir.join("dimensions").join("minecraft").join("overworld");
        if modern_overworld.is_dir() {
            let src_region = modern_overworld.join("region");
            if src_region.is_dir() {
                let dst_region = world_dir.join("region");
                fs::create_dir_all(&dst_region)?;
                let _ = move_directory_contents(&src_region, &dst_region);
            }
            let src_poi = modern_overworld.join("poi");
            if src_poi.is_dir() {
                let dst_poi = world_dir.join("poi");
                fs::create_dir_all(&dst_poi)?;
                let _ = move_directory_contents(&src_poi, &dst_poi);
            }
            let src_entities = modern_overworld.join("entities");
            if src_entities.is_dir() {
                let dst_entities = world_dir.join("entities");
                fs::create_dir_all(&dst_entities)?;
                let _ = move_directory_contents(&src_entities, &dst_entities);
            }
        }

        let modern_nether = world_dir.join("dimensions").join("minecraft").join("the_nether");
        if modern_nether.is_dir() {
            let dst_nether = world_dir.join("DIM-1");
            fs::create_dir_all(&dst_nether)?;
            let _ = move_directory_contents(&modern_nether, &dst_nether);
        }

        let modern_end = world_dir.join("dimensions").join("minecraft").join("the_end");
        if modern_end.is_dir() {
            let dst_end = world_dir.join("DIM1");
            fs::create_dir_all(&dst_end)?;
            let _ = move_directory_contents(&modern_end, &dst_end);
        }

        let modern_mc_dim = world_dir.join("dimensions").join("minecraft");
        if modern_mc_dim.is_dir() {
            let _ = fs::remove_dir_all(&modern_mc_dim);
        }

        let modern_players_data = world_dir.join("players").join("data");
        if modern_players_data.is_dir() {
            let dst_playerdata = world_dir.join("playerdata");
            fs::create_dir_all(&dst_playerdata)?;
            let _ = move_directory_contents(&modern_players_data, &dst_playerdata);
        }

        let modern_players_adv = world_dir.join("players").join("advancements");
        if modern_players_adv.is_dir() {
            let dst_adv = world_dir.join("advancements");
            fs::create_dir_all(&dst_adv)?;
            let _ = move_directory_contents(&modern_players_adv, &dst_adv);
        }

        let modern_players_stats = world_dir.join("players").join("stats");
        if modern_players_stats.is_dir() {
            let dst_stats = world_dir.join("stats");
            fs::create_dir_all(&dst_stats)?;
            let _ = move_directory_contents(&modern_players_stats, &dst_stats);
        }

        let modern_players = world_dir.join("players");
        if modern_players.is_dir() {
            let _ = fs::remove_dir_all(&modern_players);
        }
    }

    // Sanitize and repair level.dat (remove fake Paper/Bukkit datapacks, inject WorldGenSettings if needed)
    let _ = super::nbt::sanitize_and_repair_level_dat(world_dir, target_mc_version);

    Ok(())
}

fn is_dimension_folder(path: &Path) -> bool {
    path.join("region").is_dir()
        || path.join("DIM-1").is_dir()
        || path.join("DIM1").is_dir()
        || path.join("level.dat").is_file()
}

fn count_mca_files(dir: &Path) -> usize {
    count_files_matching(dir, |ext| ext == "mca")
}

fn count_files_matching<F>(dir: &Path, filter: F) -> usize
where
    F: Fn(&str) -> bool,
{
    if !dir.is_dir() {
        return 0;
    }
    let mut count = 0;
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() {
                let ext = path
                    .extension()
                    .and_then(|e| e.to_str())
                    .unwrap_or("")
                    .to_ascii_lowercase();
                if filter(&ext) {
                    count += 1;
                }
            }
        }
    }
    count
}

fn move_dimension_subfolders(src_dim_root: &Path, dst_dim_root: &Path) -> Result<usize, io::Error> {
    let subfolders = &["region", "entities", "poi", "data"];
    let mut moved = 0;
    for sub in subfolders {
        let src_sub = src_dim_root.join(sub);
        if src_sub.is_dir() {
            let dst_sub = dst_dim_root.join(sub);
            fs::create_dir_all(&dst_sub)?;
            moved += move_directory_contents(&src_sub, &dst_sub)?;
            let _ = fs::remove_dir_all(&src_sub);
        }
    }
    Ok(moved)
}

/// Recursively or flatly moves all files from `src` into `dst`.
pub fn move_directory_contents(src: &Path, dst: &Path) -> Result<usize, io::Error> {
    if !src.is_dir() {
        return Ok(0);
    }
    fs::create_dir_all(dst)?;
    let mut moved = 0;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let src_path = entry.path();
        let file_name = entry.file_name();
        let dst_path = dst.join(&file_name);

        if src_path.is_dir() {
            fs::create_dir_all(&dst_path)?;
            moved += move_directory_contents(&src_path, &dst_path)?;
            let _ = fs::remove_dir(&src_path);
        } else if src_path.is_file() {
            // Attempt rename, fallback to copy + remove
            if fs::rename(&src_path, &dst_path).is_err() {
                fs::copy(&src_path, &dst_path)?;
                let _ = fs::remove_file(&src_path);
            }
            moved += 1;
        }
    }
    Ok(moved)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bukkit_dimension_normalization() {
        let temp_dir = tempfile::tempdir().unwrap();
        let root = temp_dir.path();

        let world_dir = root.join("world");
        fs::create_dir_all(world_dir.join("region")).unwrap();
        fs::write(world_dir.join("region").join("r.0.0.mca"), b"overworld").unwrap();
        fs::write(world_dir.join("level.dat"), b"fake_dat").unwrap();

        // Bukkit nether
        let bukkit_nether = root.join("world_nether");
        fs::create_dir_all(bukkit_nether.join("region")).unwrap();
        fs::write(bukkit_nether.join("region").join("r.-1.-1.mca"), b"nether").unwrap();

        // Bukkit end
        let bukkit_end = root.join("world_the_end");
        fs::create_dir_all(bukkit_end.join("DIM1").join("region")).unwrap();
        fs::write(bukkit_end.join("DIM1").join("region").join("r.1.1.mca"), b"end").unwrap();

        let summary = analyze_world(root, &world_dir);
        assert!(summary.bukkit_dimensions_detected);
        assert_eq!(summary.overworld_chunks, 1);
        assert_eq!(summary.nether_chunks, 1);
        assert_eq!(summary.end_chunks, 1);
        assert_eq!(summary.total_chunks, 3);

        let moved = normalize_bukkit_dimensions(root, &world_dir).expect("normalize");
        assert!(moved >= 2);

        assert!(world_dir.join("DIM-1").join("region").join("r.-1.-1.mca").is_file());
        assert!(world_dir.join("DIM1").join("region").join("r.1.1.mca").is_file());
        assert!(!bukkit_nether.exists());
        assert!(!bukkit_end.exists());
    }

    #[test]
    fn test_bidirectional_world_layout_migration() {
        let temp_dir = tempfile::tempdir().unwrap();
        let world_dir = temp_dir.path().join("world");

        // 1. Create legacy 1.21 structure
        fs::create_dir_all(world_dir.join("region")).unwrap();
        fs::write(world_dir.join("region").join("r.0.0.mca"), b"overworld").unwrap();
        fs::create_dir_all(world_dir.join("DIM-1").join("region")).unwrap();
        fs::write(world_dir.join("DIM-1").join("region").join("r.-1.-1.mca"), b"nether").unwrap();
        fs::create_dir_all(world_dir.join("playerdata")).unwrap();
        fs::write(world_dir.join("playerdata").join("test_uuid.dat"), b"inv").unwrap();

        let legacy_summary = analyze_world(temp_dir.path(), &world_dir);
        assert_eq!(legacy_summary.detected_layout, "legacy_1_21");
        assert_eq!(legacy_summary.overworld_chunks, 1);
        assert_eq!(legacy_summary.nether_chunks, 1);
        assert_eq!(legacy_summary.player_count, 1);

        // 2. Migrate to Modern 26.1.2
        migrate_world_layout_to_target_version(&world_dir, "26.1.2").expect("migrate to 26.1.2");

        assert!(world_dir.join("dimensions").join("minecraft").join("overworld").join("region").join("r.0.0.mca").is_file());
        assert!(world_dir.join("dimensions").join("minecraft").join("the_nether").join("region").join("r.-1.-1.mca").is_file());
        assert!(world_dir.join("players").join("data").join("test_uuid.dat").is_file());
        assert!(!world_dir.join("region").exists());
        assert!(!world_dir.join("playerdata").exists());

        let modern_summary = analyze_world(temp_dir.path(), &world_dir);
        assert_eq!(modern_summary.detected_layout, "unified_26");
        assert_eq!(modern_summary.overworld_chunks, 1);
        assert_eq!(modern_summary.nether_chunks, 1);
        assert_eq!(modern_summary.player_count, 1);

        // 3. Migrate back to Legacy 1.21.4
        migrate_world_layout_to_target_version(&world_dir, "1.21.4").expect("migrate to 1.21.4");

        assert!(world_dir.join("region").join("r.0.0.mca").is_file());
        assert!(world_dir.join("DIM-1").join("region").join("r.-1.-1.mca").is_file());
        assert!(world_dir.join("playerdata").join("test_uuid.dat").is_file());
        assert!(!world_dir.join("dimensions").join("minecraft").exists());
        assert!(!world_dir.join("players").exists());
    }
}
