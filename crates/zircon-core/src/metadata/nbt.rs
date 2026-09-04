//! Lightweight, zero-panic NBT (Named Binary Tag) decoder and `level.dat` inspector.
//!
//! Handles GZip-compressed Minecraft `level.dat` files, extracting world metadata:
//! - DataVersion (e.g., 3465 for 1.20.4, 3953 for 1.21, 4189 for 1.21.4)
//! - LevelName / world name
//! - RandomSeed / WorldGenSettings.seed
//! - Minecraft Version name (e.g., "1.21.1")
//! - Game rules, difficulty, and hardcore status
//!
//! Also provides version downgrade detection to protect worlds against chunk corruption.

use std::collections::HashMap;
use std::fmt;
use std::fs::File;
use std::io::{self, Cursor, Read};
use std::path::Path;

use flate2::read::GzDecoder;
use serde::{Deserialize, Serialize};

/// Maximum allowed uncompressed size for a level.dat file (64 MB) to prevent zip-bomb / memory exhaustion.
pub const MAX_LEVEL_DAT_BYTES: usize = 64 * 1024 * 1024;

/// Known Minecraft DataVersions mapped to release versions.
pub const MINECRAFT_DATA_VERSIONS: &[(u32, &str)] = &[
    (1343, "1.12.2"),
    (1519, "1.13"),
    (1631, "1.13.2"),
    (1976, "1.14.4"),
    (2230, "1.15.2"),
    (2567, "1.16.1"),
    (2586, "1.16.5"),
    (2730, "1.17.1"),
    (2865, "1.18.1"),
    (2975, "1.18.2"),
    (3120, "1.19.2"),
    (3337, "1.19.4"),
    (3463, "1.20.1"),
    (3465, "1.20.4"),
    (3839, "1.20.6"),
    (3953, "1.21"),
    (3955, "1.21.1"),
    (4082, "1.21.3"),
    (4189, "1.21.4"),
    (4790, "26.1.2"),
];

/// Result of parsing a Minecraft `level.dat` file.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LevelDatInfo {
    /// Internal Minecraft data version integer (e.g. 3955 for 1.21.1).
    pub data_version: Option<u32>,
    /// Inferred or declared Minecraft release version name (e.g. "1.21.1").
    pub minecraft_version: Option<String>,
    /// The user-visible world name.
    pub level_name: Option<String>,
    /// The world generation seed.
    pub seed: Option<i64>,
    /// Game difficulty (0: Peaceful, 1: Easy, 2: Normal, 3: Hard).
    pub difficulty: Option<u8>,
    /// Whether hardcore mode is enabled.
    pub hardcore: bool,
    /// Whether commands/cheats are enabled.
    pub allow_commands: bool,
}

/// NBT Decoder errors.
#[derive(Debug)]
pub enum NbtError {
    Io(io::Error),
    DecompressionFailed(String),
    UnexpectedEof,
    InvalidTagId(u8),
    InvalidUtf8(String),
    ExceededMaxBytes(usize),
    MissingDataCompound,
}

impl fmt::Display for NbtError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            NbtError::Io(e) => write!(f, "I/O error reading NBT: {e}"),
            NbtError::DecompressionFailed(msg) => write!(f, "GZip decompression failed: {msg}"),
            NbtError::UnexpectedEof => write!(f, "Unexpected end of NBT stream"),
            NbtError::InvalidTagId(id) => write!(f, "Invalid NBT tag ID: {id}"),
            NbtError::InvalidUtf8(msg) => write!(f, "Invalid UTF-8 string in NBT: {msg}"),
            NbtError::ExceededMaxBytes(limit) => {
                write!(f, "level.dat exceeded maximum size of {limit} bytes")
            }
            NbtError::MissingDataCompound => write!(f, "Missing root 'Data' compound in level.dat"),
        }
    }
}

impl std::error::Error for NbtError {}

impl From<io::Error> for NbtError {
    fn from(e: io::Error) -> Self {
        NbtError::Io(e)
    }
}

/// Dynamic NBT tag representation.
#[derive(Debug, Clone, PartialEq)]
pub enum NbtTag {
    Byte(i8),
    Short(i16),
    Int(i32),
    Long(i64),
    Float(f32),
    Double(f64),
    ByteArray(Vec<u8>),
    String(String),
    List(Vec<NbtTag>),
    Compound(HashMap<String, NbtTag>),
    IntArray(Vec<i32>),
    LongArray(Vec<i64>),
}

impl NbtTag {
    pub fn tag_id(&self) -> u8 {
        match self {
            NbtTag::Byte(_) => 1,
            NbtTag::Short(_) => 2,
            NbtTag::Int(_) => 3,
            NbtTag::Long(_) => 4,
            NbtTag::Float(_) => 5,
            NbtTag::Double(_) => 6,
            NbtTag::ByteArray(_) => 7,
            NbtTag::String(_) => 8,
            NbtTag::List(_) => 9,
            NbtTag::Compound(_) => 10,
            NbtTag::IntArray(_) => 11,
            NbtTag::LongArray(_) => 12,
        }
    }

    pub fn as_compound(&self) -> Option<&HashMap<String, NbtTag>> {
        match self {
            NbtTag::Compound(map) => Some(map),
            _ => None,
        }
    }

    pub fn as_compound_mut(&mut self) -> Option<&mut HashMap<String, NbtTag>> {
        match self {
            NbtTag::Compound(map) => Some(map),
            _ => None,
        }
    }

    pub fn get_str(&self, key: &str) -> Option<&str> {
        self.as_compound()
            .and_then(|map| map.get(key))
            .and_then(|tag| match tag {
                NbtTag::String(s) => Some(s.as_str()),
                _ => None,
            })
    }

    pub fn get_i32(&self, key: &str) -> Option<i32> {
        self.as_compound()
            .and_then(|map| map.get(key))
            .and_then(|tag| match tag {
                NbtTag::Int(v) => Some(*v),
                NbtTag::Byte(v) => Some(*v as i32),
                NbtTag::Short(v) => Some(*v as i32),
                NbtTag::Long(v) => i32::try_from(*v).ok(),
                _ => None,
            })
    }

    pub fn get_u32(&self, key: &str) -> Option<u32> {
        self.get_i32(key).and_then(|v| u32::try_from(v).ok())
    }

    pub fn get_i64(&self, key: &str) -> Option<i64> {
        self.as_compound()
            .and_then(|map| map.get(key))
            .and_then(|tag| match tag {
                NbtTag::Long(v) => Some(*v),
                NbtTag::Int(v) => Some(*v as i64),
                NbtTag::Short(v) => Some(*v as i64),
                NbtTag::Byte(v) => Some(*v as i64),
                _ => None,
            })
    }

    pub fn get_bool(&self, key: &str) -> Option<bool> {
        self.as_compound()
            .and_then(|map| map.get(key))
            .and_then(|tag| match tag {
                NbtTag::Byte(v) => Some(*v != 0),
                NbtTag::Int(v) => Some(*v != 0),
                _ => None,
            })
    }
}

/// Reads and decompresses raw NBT root compound from a `level.dat` file.
pub fn read_raw_level_dat(path: &Path) -> Result<HashMap<String, NbtTag>, NbtError> {
    let file = File::open(path)?;
    let mut decoder = GzDecoder::new(file);
    let mut decompressed = Vec::new();

    decoder
        .by_ref()
        .take((MAX_LEVEL_DAT_BYTES + 1) as u64)
        .read_to_end(&mut decompressed)
        .map_err(|e| NbtError::DecompressionFailed(e.to_string()))?;

    if decompressed.len() > MAX_LEVEL_DAT_BYTES {
        return Err(NbtError::ExceededMaxBytes(MAX_LEVEL_DAT_BYTES));
    }

    let mut cursor = Cursor::new(decompressed.as_slice());
    parse_root_compound(&mut cursor)
}

/// Reads and decompresses a `level.dat` file, returning its high-level metadata.
pub fn read_level_dat(path: &Path) -> Result<LevelDatInfo, NbtError> {
    let file = File::open(path)?;
    let mut decoder = GzDecoder::new(file);
    let mut decompressed = Vec::new();

    // Read with size limit to prevent memory exhaustion
    decoder
        .by_ref()
        .take((MAX_LEVEL_DAT_BYTES + 1) as u64)
        .read_to_end(&mut decompressed)
        .map_err(|e| NbtError::DecompressionFailed(e.to_string()))?;

    if decompressed.len() > MAX_LEVEL_DAT_BYTES {
        return Err(NbtError::ExceededMaxBytes(MAX_LEVEL_DAT_BYTES));
    }

    parse_level_dat_bytes(&decompressed)
}

/// Parses uncompressed NBT bytes from a `level.dat` payload.
pub fn parse_level_dat_bytes(bytes: &[u8]) -> Result<LevelDatInfo, NbtError> {
    let mut cursor = Cursor::new(bytes);
    let root = parse_root_compound(&mut cursor)?;

    let root_compound_tag = NbtTag::Compound(root.clone());
    let data_tag = root
        .get("Data")
        .or_else(|| root.get("data"))
        .unwrap_or(&root_compound_tag);

    let data_version = data_tag.get_u32("DataVersion");
    let level_name = data_tag.get_str("LevelName").map(|s| s.to_string());

    // Extract seed (either directly in Data.RandomSeed or in Data.WorldGenSettings.seed)
    let seed = data_tag
        .get_i64("RandomSeed")
        .or_else(|| {
            data_tag
                .as_compound()
                .and_then(|map| map.get("WorldGenSettings"))
                .and_then(|w| w.get_i64("seed"))
        });

    // Version string can be in Data.Version.Name or inferred from DataVersion
    let version_name = data_tag
        .as_compound()
        .and_then(|map| map.get("Version"))
        .and_then(|v| v.get_str("Name"))
        .map(|s| s.to_string())
        .or_else(|| {
            data_version.and_then(data_version_to_mc_version).map(ToString::to_string)
        });

    let difficulty = data_tag.get_i32("Difficulty").map(|d| d as u8);
    let hardcore = data_tag.get_bool("hardcore").unwrap_or(false);
    let allow_commands = data_tag.get_bool("allowCommands").unwrap_or(false);

    Ok(LevelDatInfo {
        data_version,
        minecraft_version: version_name,
        level_name,
        seed,
        difficulty,
        hardcore,
        allow_commands,
    })
}

/// Encodes an NBT root compound into compressed GZip bytes.
pub fn encode_root_compound(root: &HashMap<String, NbtTag>) -> Result<Vec<u8>, NbtError> {
    use flate2::write::GzEncoder;
    use flate2::Compression;
    use std::io::Write;

    let mut raw = Vec::new();
    // TAG_Compound (10)
    raw.push(10);
    // Root name "" (length 0)
    raw.extend_from_slice(&(0u16).to_be_bytes());
    encode_compound_entries(root, &mut raw)?;

    let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
    encoder.write_all(&raw).map_err(NbtError::Io)?;
    encoder.finish().map_err(NbtError::Io)
}

/// Encodes and writes an NBT root compound directly to a `.dat` file.
pub fn write_level_dat(path: &Path, root: &HashMap<String, NbtTag>) -> Result<(), NbtError> {
    let encoded = encode_root_compound(root)?;
    std::fs::write(path, encoded)?;
    Ok(())
}

/// Creates a standard Minecraft 26.x+ `world_gen_settings.dat` structure.
pub fn create_default_world_gen_settings(seed: i64, data_version: u32) -> HashMap<String, NbtTag> {
    let mut overworld_biome_source = HashMap::new();
    overworld_biome_source.insert("type".to_string(), NbtTag::String("minecraft:multi_noise".to_string()));
    overworld_biome_source.insert("preset".to_string(), NbtTag::String("minecraft:overworld".to_string()));

    let mut overworld_generator = HashMap::new();
    overworld_generator.insert("type".to_string(), NbtTag::String("minecraft:noise".to_string()));
    overworld_generator.insert("settings".to_string(), NbtTag::String("minecraft:overworld".to_string()));
    overworld_generator.insert("biome_source".to_string(), NbtTag::Compound(overworld_biome_source));

    let mut overworld_dim = HashMap::new();
    overworld_dim.insert("type".to_string(), NbtTag::String("minecraft:overworld".to_string()));
    overworld_dim.insert("generator".to_string(), NbtTag::Compound(overworld_generator));

    let mut nether_biome_source = HashMap::new();
    nether_biome_source.insert("type".to_string(), NbtTag::String("minecraft:multi_noise".to_string()));
    nether_biome_source.insert("preset".to_string(), NbtTag::String("minecraft:nether".to_string()));

    let mut nether_generator = HashMap::new();
    nether_generator.insert("type".to_string(), NbtTag::String("minecraft:noise".to_string()));
    nether_generator.insert("settings".to_string(), NbtTag::String("minecraft:nether".to_string()));
    nether_generator.insert("biome_source".to_string(), NbtTag::Compound(nether_biome_source));

    let mut nether_dim = HashMap::new();
    nether_dim.insert("type".to_string(), NbtTag::String("minecraft:the_nether".to_string()));
    nether_dim.insert("generator".to_string(), NbtTag::Compound(nether_generator));

    let mut end_biome_source = HashMap::new();
    end_biome_source.insert("type".to_string(), NbtTag::String("minecraft:the_end".to_string()));

    let mut end_generator = HashMap::new();
    end_generator.insert("type".to_string(), NbtTag::String("minecraft:noise".to_string()));
    end_generator.insert("settings".to_string(), NbtTag::String("minecraft:end".to_string()));
    end_generator.insert("biome_source".to_string(), NbtTag::Compound(end_biome_source));

    let mut end_dim = HashMap::new();
    end_dim.insert("type".to_string(), NbtTag::String("minecraft:the_end".to_string()));
    end_dim.insert("generator".to_string(), NbtTag::Compound(end_generator));

    let mut dimensions = HashMap::new();
    dimensions.insert("minecraft:overworld".to_string(), NbtTag::Compound(overworld_dim));
    dimensions.insert("minecraft:the_nether".to_string(), NbtTag::Compound(nether_dim));
    dimensions.insert("minecraft:the_end".to_string(), NbtTag::Compound(end_dim));

    let mut data = HashMap::new();
    data.insert("seed".to_string(), NbtTag::Long(seed));
    data.insert("generate_structures".to_string(), NbtTag::Byte(1));
    data.insert("bonus_chest".to_string(), NbtTag::Byte(0));
    data.insert("dimensions".to_string(), NbtTag::Compound(dimensions));

    let mut root = HashMap::new();
    root.insert("DataVersion".to_string(), NbtTag::Int(data_version as i32));
    root.insert("data".to_string(), NbtTag::Compound(data));

    root
}

/// Sanitizes Paper/Bukkit datapack references and metadata from `level.dat`,
/// and creates `world_gen_settings.dat` if required by modern Minecraft 26.x+.
pub fn sanitize_and_repair_level_dat(world_dir: &Path, target_mc_version: &str) -> Result<(), NbtError> {
    let level_dat_path = world_dir.join("level.dat");
    if !level_dat_path.is_file() {
        return Ok(());
    }

    let mut root = read_raw_level_dat(&level_dat_path)?;

    let mut world_seed = 0i64;
    let mut world_data_version = 4790u32;

    let data_tag = if root.contains_key("Data") {
        root.get_mut("Data")
    } else {
        root.get_mut("data")
    };

    if let Some(NbtTag::Compound(data)) = data_tag {
        if let Some(NbtTag::Long(s)) = data.get("RandomSeed") {
            world_seed = *s;
        }
        if let Some(NbtTag::Int(dv)) = data.get("DataVersion") {
            world_data_version = *dv as u32;
        }

        // 1. Sanitize DataPacks.Enabled
        if let Some(NbtTag::Compound(data_packs)) = data.get_mut("DataPacks") {
            if let Some(NbtTag::List(enabled)) = data_packs.get_mut("Enabled") {
                enabled.retain(|tag| {
                    if let NbtTag::String(s) = tag {
                        let lower = s.to_ascii_lowercase();
                        lower != "paper"
                            && lower != "file/bukkit"
                            && lower != "bukkit"
                            && lower != "spigot"
                            && lower != "purpur"
                            && !lower.starts_with("paper:")
                            && !lower.starts_with("bukkit:")
                    } else {
                        true
                    }
                });

                let has_vanilla = enabled.iter().any(|tag| {
                    if let NbtTag::String(s) = tag {
                        s == "vanilla"
                    } else {
                        false
                    }
                });
                if !has_vanilla {
                    enabled.insert(0, NbtTag::String("vanilla".to_string()));
                }
            }
        }

        // 2. Remove Bukkit / Paper custom metadata
        data.remove("Bukkit.Version");
        data.remove("paperSpawnDimension");
        data.remove("Paper");
        data.remove("ServerBrands");
    }

    // Write back sanitized level.dat
    write_level_dat(&level_dat_path, &root)?;

    // 3. Clean up synthetic Paper/Bukkit datapack directories
    let bukkit_dp = world_dir.join("datapacks").join("bukkit");
    if bukkit_dp.is_dir() {
        let _ = std::fs::remove_dir_all(&bukkit_dp);
    }
    let paper_dp = world_dir.join("datapacks").join("paper");
    if paper_dp.is_dir() {
        let _ = std::fs::remove_dir_all(&paper_dp);
    }

    // 4. In 26.x+, ensure world/data/minecraft/world_gen_settings.dat exists
    let clean = target_mc_version.trim();
    let is_modern = clean.starts_with("26.") || clean.starts_with("27.");
    if is_modern {
        let wgs_dir = world_dir.join("data").join("minecraft");
        let wgs_file = wgs_dir.join("world_gen_settings.dat");
        if !wgs_file.is_file() {
            let _ = std::fs::create_dir_all(&wgs_dir);
            let wgs_root = create_default_world_gen_settings(world_seed, world_data_version);
            let _ = write_level_dat(&wgs_file, &wgs_root);
        }
    }

    Ok(())
}

/// Maps a DataVersion integer to the nearest matching Minecraft release version string.
pub fn data_version_to_mc_version(data_version: u32) -> Option<&'static str> {
    for (dv, version) in MINECRAFT_DATA_VERSIONS.iter().rev() {
        if data_version >= *dv {
            return Some(version);
        }
    }
    None
}

/// Maps a Minecraft version string (e.g. "1.21.1") to its base DataVersion integer.
pub fn mc_version_to_data_version(mc_version: &str) -> Option<u32> {
    let clean = mc_version.trim();
    for (dv, version) in MINECRAFT_DATA_VERSIONS {
        if *version == clean {
            return Some(*dv);
        }
    }
    None
}

/// Checks if loading a world with `world_data_version` onto `target_mc_version` constitutes an unsupported downgrade.
/// Returns `Ok(())` if safe (same version or upgrade), or `Err(warning_message)` if it's a dangerous downgrade.
pub fn check_version_compatibility(
    world_data_version: Option<u32>,
    target_mc_version: &str,
) -> Result<(), String> {
    let world_dv = match world_data_version {
        Some(dv) => dv,
        None => return Ok(()), // Unknown version, cannot strictly forbid
    };

    let target_dv = match mc_version_to_data_version(target_mc_version) {
        Some(dv) => dv,
        None => return Ok(()), // Custom / unindexed target version
    };

    if world_dv > target_dv {
        let world_ver_name = data_version_to_mc_version(world_dv).unwrap_or("newer version");
        return Err(format!(
            "Downgrade detected: This world was saved in Minecraft {} (DataVersion {}). \
             Running it on Minecraft {} (DataVersion {}) will cause irreversible chunk and inventory corruption. \
             Please choose Minecraft {} or newer.",
            world_ver_name, world_dv, target_mc_version, target_dv, world_ver_name
        ));
    }

    Ok(())
}

// --------------------------------------------------------------------------
// Internal NBT Parser implementation
// --------------------------------------------------------------------------

fn parse_root_compound(cursor: &mut Cursor<&[u8]>) -> Result<HashMap<String, NbtTag>, NbtError> {
    let tag_type = read_u8(cursor)?;
    if tag_type != 10 {
        return Err(NbtError::InvalidTagId(tag_type));
    }
    // Read root name (unused, but part of standard NBT format)
    let _root_name = read_string(cursor)?;
    parse_compound_entries(cursor)
}

fn parse_tag_payload(tag_type: u8, cursor: &mut Cursor<&[u8]>) -> Result<NbtTag, NbtError> {
    match tag_type {
        1 => Ok(NbtTag::Byte(read_i8(cursor)?)),
        2 => Ok(NbtTag::Short(read_i16(cursor)?)),
        3 => Ok(NbtTag::Int(read_i32(cursor)?)),
        4 => Ok(NbtTag::Long(read_i64(cursor)?)),
        5 => Ok(NbtTag::Float(read_f32(cursor)?)),
        6 => Ok(NbtTag::Double(read_f64(cursor)?)),
        7 => {
            let len = read_i32(cursor)?;
            if len < 0 || len as usize > MAX_LEVEL_DAT_BYTES {
                return Err(NbtError::UnexpectedEof);
            }
            let mut buf = vec![0u8; len as usize];
            cursor.read_exact(&mut buf)?;
            Ok(NbtTag::ByteArray(buf))
        }
        8 => Ok(NbtTag::String(read_string(cursor)?)),
        9 => {
            let elem_type = read_u8(cursor)?;
            let len = read_i32(cursor)?;
            if len <= 0 {
                return Ok(NbtTag::List(Vec::new()));
            }
            let count = len as usize;
            if count > 1_000_000 {
                return Err(NbtError::ExceededMaxBytes(count));
            }
            let mut list = Vec::with_capacity(count);
            for _ in 0..count {
                list.push(parse_tag_payload(elem_type, cursor)?);
            }
            Ok(NbtTag::List(list))
        }
        10 => {
            let compound = parse_compound_entries(cursor)?;
            Ok(NbtTag::Compound(compound))
        }
        11 => {
            let len = read_i32(cursor)?;
            if len <= 0 {
                return Ok(NbtTag::IntArray(Vec::new()));
            }
            let count = len as usize;
            let mut list = Vec::with_capacity(count);
            for _ in 0..count {
                list.push(read_i32(cursor)?);
            }
            Ok(NbtTag::IntArray(list))
        }
        12 => {
            let len = read_i32(cursor)?;
            if len <= 0 {
                return Ok(NbtTag::LongArray(Vec::new()));
            }
            let count = len as usize;
            let mut list = Vec::with_capacity(count);
            for _ in 0..count {
                list.push(read_i64(cursor)?);
            }
            Ok(NbtTag::LongArray(list))
        }
        other => Err(NbtError::InvalidTagId(other)),
    }
}

fn encode_compound_entries(entries: &HashMap<String, NbtTag>, buf: &mut Vec<u8>) -> Result<(), NbtError> {
    for (name, tag) in entries {
        buf.push(tag.tag_id());
        let name_bytes = name.as_bytes();
        if name_bytes.len() > u16::MAX as usize {
            return Err(NbtError::InvalidUtf8("Tag name too long".to_string()));
        }
        buf.extend_from_slice(&(name_bytes.len() as u16).to_be_bytes());
        buf.extend_from_slice(name_bytes);
        encode_tag_payload(tag, buf)?;
    }
    // TAG_End (0)
    buf.push(0);
    Ok(())
}

fn encode_tag_payload(tag: &NbtTag, buf: &mut Vec<u8>) -> Result<(), NbtError> {
    match tag {
        NbtTag::Byte(v) => buf.push(*v as u8),
        NbtTag::Short(v) => buf.extend_from_slice(&v.to_be_bytes()),
        NbtTag::Int(v) => buf.extend_from_slice(&v.to_be_bytes()),
        NbtTag::Long(v) => buf.extend_from_slice(&v.to_be_bytes()),
        NbtTag::Float(v) => buf.extend_from_slice(&v.to_be_bytes()),
        NbtTag::Double(v) => buf.extend_from_slice(&v.to_be_bytes()),
        NbtTag::ByteArray(v) => {
            buf.extend_from_slice(&(v.len() as i32).to_be_bytes());
            buf.extend_from_slice(v);
        }
        NbtTag::String(s) => {
            let s_bytes = s.as_bytes();
            buf.extend_from_slice(&(s_bytes.len() as u16).to_be_bytes());
            buf.extend_from_slice(s_bytes);
        }
        NbtTag::List(list) => {
            if list.is_empty() {
                buf.push(0); // TAG_End element type
                buf.extend_from_slice(&(0i32).to_be_bytes());
            } else {
                let elem_type = list[0].tag_id();
                buf.push(elem_type);
                buf.extend_from_slice(&(list.len() as i32).to_be_bytes());
                for item in list {
                    encode_tag_payload(item, buf)?;
                }
            }
        }
        NbtTag::Compound(map) => {
            encode_compound_entries(map, buf)?;
        }
        NbtTag::IntArray(ints) => {
            buf.extend_from_slice(&(ints.len() as i32).to_be_bytes());
            for i in ints {
                buf.extend_from_slice(&i.to_be_bytes());
            }
        }
        NbtTag::LongArray(longs) => {
            buf.extend_from_slice(&(longs.len() as i32).to_be_bytes());
            for l in longs {
                buf.extend_from_slice(&l.to_be_bytes());
            }
        }
    }
    Ok(())
}

fn parse_compound_entries(cursor: &mut Cursor<&[u8]>) -> Result<HashMap<String, NbtTag>, NbtError> {
    let mut map = HashMap::new();
    loop {
        let tag_type = read_u8(cursor)?;
        if tag_type == 0 {
            // TAG_End
            break;
        }
        let name = read_string(cursor)?;
        let tag = parse_tag_payload(tag_type, cursor)?;
        map.insert(name, tag);
    }
    Ok(map)
}

fn read_u8(cursor: &mut Cursor<&[u8]>) -> Result<u8, NbtError> {
    let mut buf = [0u8; 1];
    cursor.read_exact(&mut buf).map_err(|_| NbtError::UnexpectedEof)?;
    Ok(buf[0])
}

fn read_i8(cursor: &mut Cursor<&[u8]>) -> Result<i8, NbtError> {
    read_u8(cursor).map(|b| b as i8)
}

fn read_i16(cursor: &mut Cursor<&[u8]>) -> Result<i16, NbtError> {
    let mut buf = [0u8; 2];
    cursor.read_exact(&mut buf).map_err(|_| NbtError::UnexpectedEof)?;
    Ok(i16::from_be_bytes(buf))
}

fn read_i32(cursor: &mut Cursor<&[u8]>) -> Result<i32, NbtError> {
    let mut buf = [0u8; 4];
    cursor.read_exact(&mut buf).map_err(|_| NbtError::UnexpectedEof)?;
    Ok(i32::from_be_bytes(buf))
}

fn read_i64(cursor: &mut Cursor<&[u8]>) -> Result<i64, NbtError> {
    let mut buf = [0u8; 8];
    cursor.read_exact(&mut buf).map_err(|_| NbtError::UnexpectedEof)?;
    Ok(i64::from_be_bytes(buf))
}

fn read_f32(cursor: &mut Cursor<&[u8]>) -> Result<f32, NbtError> {
    let mut buf = [0u8; 4];
    cursor.read_exact(&mut buf).map_err(|_| NbtError::UnexpectedEof)?;
    Ok(f32::from_be_bytes(buf))
}

fn read_f64(cursor: &mut Cursor<&[u8]>) -> Result<f64, NbtError> {
    let mut buf = [0u8; 8];
    cursor.read_exact(&mut buf).map_err(|_| NbtError::UnexpectedEof)?;
    Ok(f64::from_be_bytes(buf))
}

fn read_string(cursor: &mut Cursor<&[u8]>) -> Result<String, NbtError> {
    let len = read_i16(cursor)? as usize;
    if len == 0 {
        return Ok(String::new());
    }
    let mut buf = vec![0u8; len];
    cursor.read_exact(&mut buf).map_err(|_| NbtError::UnexpectedEof)?;
    String::from_utf8(buf).map_err(|e| NbtError::InvalidUtf8(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use flate2::write::GzEncoder;
    use flate2::Compression;
    use std::io::Write;

    fn build_synthetic_level_dat(data_version: i32, level_name: &str, seed: i64) -> Vec<u8> {
        let mut raw = Vec::new();
        // Root compound
        raw.push(10); // TAG_Compound
        // Root name ""
        raw.extend_from_slice(&(0u16).to_be_bytes());

        // Compound "Data"
        raw.push(10); // TAG_Compound
        let data_str = "Data";
        raw.extend_from_slice(&(data_str.len() as u16).to_be_bytes());
        raw.extend_from_slice(data_str.as_bytes());

        // Tag DataVersion (TAG_Int = 3)
        raw.push(3);
        let dv_str = "DataVersion";
        raw.extend_from_slice(&(dv_str.len() as u16).to_be_bytes());
        raw.extend_from_slice(dv_str.as_bytes());
        raw.extend_from_slice(&data_version.to_be_bytes());

        // Tag LevelName (TAG_String = 8)
        raw.push(8);
        let ln_str = "LevelName";
        raw.extend_from_slice(&(ln_str.len() as u16).to_be_bytes());
        raw.extend_from_slice(ln_str.as_bytes());
        raw.extend_from_slice(&(level_name.len() as u16).to_be_bytes());
        raw.extend_from_slice(level_name.as_bytes());

        // Tag RandomSeed (TAG_Long = 4)
        raw.push(4);
        let seed_str = "RandomSeed";
        raw.extend_from_slice(&(seed_str.len() as u16).to_be_bytes());
        raw.extend_from_slice(seed_str.as_bytes());
        raw.extend_from_slice(&seed.to_be_bytes());

        // End "Data" compound
        raw.push(0);
        // End Root compound
        raw.push(0);

        raw
    }

    #[test]
    fn test_parse_level_dat_bytes() {
        let raw = build_synthetic_level_dat(3955, "Survival World", 123456789);
        let info = parse_level_dat_bytes(&raw).expect("should parse");
        assert_eq!(info.data_version, Some(3955));
        assert_eq!(info.level_name.as_deref(), Some("Survival World"));
        assert_eq!(info.seed, Some(123456789));
        assert_eq!(info.minecraft_version.as_deref(), Some("1.21.1"));
    }

    #[test]
    fn test_gzip_level_dat_roundtrip() {
        let raw = build_synthetic_level_dat(3465, "My World", -987654321);
        let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
        encoder.write_all(&raw).unwrap();
        let gzipped = encoder.finish().unwrap();

        let temp = tempfile::NamedTempFile::new().unwrap();
        std::fs::write(temp.path(), gzipped).unwrap();

        let info = read_level_dat(temp.path()).expect("read gzip level.dat");
        assert_eq!(info.data_version, Some(3465));
        assert_eq!(info.level_name.as_deref(), Some("My World"));
        assert_eq!(info.seed, Some(-987654321));
        assert_eq!(info.minecraft_version.as_deref(), Some("1.20.4"));
    }

    #[test]
    fn test_downgrade_detection() {
        // 1.21.1 (3955) on 1.20.4 (3465) is a downgrade
        let result = check_version_compatibility(Some(3955), "1.20.4");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Downgrade detected"));

        // 1.20.4 (3465) on 1.21.1 (3955) is an upgrade (valid)
        assert!(check_version_compatibility(Some(3465), "1.21.1").is_ok());

        // Same version is valid
        assert!(check_version_compatibility(Some(3955), "1.21.1").is_ok());
    }

    #[test]
    fn test_sanitize_and_repair_paper_level_dat() {
        let temp_dir = tempfile::tempdir().unwrap();
        let world_dir = temp_dir.path().join("world");
        std::fs::create_dir_all(&world_dir).unwrap();

        // Construct a Paper-like level.dat
        let mut data_packs = HashMap::new();
        let enabled = vec![
            NbtTag::String("vanilla".to_string()),
            NbtTag::String("file/bukkit".to_string()),
            NbtTag::String("paper".to_string()),
        ];
        data_packs.insert("Enabled".to_string(), NbtTag::List(enabled));

        let mut data = HashMap::new();
        data.insert("LevelName".to_string(), NbtTag::String("PaperWorld".to_string()));
        data.insert("RandomSeed".to_string(), NbtTag::Long(999888777));
        data.insert("DataVersion".to_string(), NbtTag::Int(4790));
        data.insert("DataPacks".to_string(), NbtTag::Compound(data_packs));
        data.insert("Bukkit.Version".to_string(), NbtTag::String("Paper 26.1.2".to_string()));
        data.insert("paperSpawnDimension".to_string(), NbtTag::String("minecraft:overworld".to_string()));
        data.insert("ServerBrands".to_string(), NbtTag::List(vec![NbtTag::String("Paper".to_string())]));

        let mut root = HashMap::new();
        root.insert("Data".to_string(), NbtTag::Compound(data));

        let level_dat_path = world_dir.join("level.dat");
        write_level_dat(&level_dat_path, &root).expect("write level.dat");

        // Create synthetic bukkit datapack directory
        let bukkit_dp = world_dir.join("datapacks").join("bukkit");
        std::fs::create_dir_all(&bukkit_dp).unwrap();
        std::fs::write(bukkit_dp.join("pack.mcmeta"), b"{}").unwrap();

        // Run sanitizer for Minecraft 26.1.2
        sanitize_and_repair_level_dat(&world_dir, "26.1.2").expect("sanitize");

        // Verify synthetic datapack was removed
        assert!(!bukkit_dp.exists());

        // Verify world_gen_settings.dat was created for 26.x
        let wgs_path = world_dir.join("data").join("minecraft").join("world_gen_settings.dat");
        assert!(wgs_path.is_file());

        // Read sanitized level.dat
        let sanitized_root = read_raw_level_dat(&level_dat_path).expect("read sanitized");
        let sanitized_data = sanitized_root.get("Data").unwrap().as_compound().unwrap();

        // Paper metadata removed
        assert!(!sanitized_data.contains_key("Bukkit.Version"));
        assert!(!sanitized_data.contains_key("paperSpawnDimension"));
        assert!(!sanitized_data.contains_key("ServerBrands"));

        // Datapacks list cleaned
        let sanitized_dp = sanitized_data.get("DataPacks").unwrap().as_compound().unwrap();
        if let Some(NbtTag::List(enabled_list)) = sanitized_dp.get("Enabled") {
            assert_eq!(enabled_list.len(), 1);
            if let NbtTag::String(s) = &enabled_list[0] {
                assert_eq!(s, "vanilla");
            } else {
                panic!("Expected vanilla string tag");
            }
        } else {
            panic!("Expected Enabled list");
        }
    }
}
