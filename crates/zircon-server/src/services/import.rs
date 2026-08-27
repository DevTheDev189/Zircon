//! Server migration & import engine: unpacks uploaded server ZIP archives,
//! validates security boundaries (ArchiveGuard), normalizes Bukkit/Paper dimension
//! structures, inspects level.dat NBT metadata, prevents version downgrades,
//! indexes mods, and reconstructs signed BOM and instance directories.

use std::collections::HashMap;
use std::fmt;
use std::fs::{self, File};
use std::io::{self, BufReader, Read};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use ed25519_dalek::SigningKey;
use serde::{Deserialize, Serialize};
use sha1::Digest;
use uuid::Uuid;

use zircon_core::archive::limits::{ArchiveError, ArchiveGuard};
use zircon_core::archive::zip::extract_zip;
use zircon_core::crypto::murmur3::curse_forge_fingerprint_of_file;
use zircon_core::metadata::extractor::extract;
use zircon_core::metadata::nbt::check_version_compatibility;
use zircon_core::metadata::world_normalizer::{
    analyze_world, discover_world_dir, migrate_world_layout_to_target_version,
    move_directory_contents, normalize_bukkit_dimensions, WorldSummary,
};
use zircon_core::model::{BillOfMaterials, InstanceConfig, ModEntry, ModLoaderType, ModSide};

use crate::instance::{InstanceError, ServerInstanceManager};
use crate::services::bom::BomService;

/// Pre-flight mod item summary for UI display and BOM generation.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PreflightModInfo {
    pub id: String,
    pub name: String,
    pub version: String,
    pub loader: String,
    pub side: String,
    pub filename: String,
    pub sha1: String,
    pub murmur3: u64,
    pub verified: bool,
}

/// Comprehensive pre-flight inspection report returned after unpacking.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PreflightReport {
    pub import_id: String,
    pub suggested_name: String,
    pub minecraft_version: Option<String>,
    pub data_version: Option<u32>,
    pub detected_loader: String,
    pub detected_loader_version: Option<String>,
    pub world: Option<WorldSummary>,
    pub mods: Vec<PreflightModInfo>,
    pub configs_found: Vec<String>,
    pub permissions_found: Vec<String>,
    pub server_properties_found: bool,
    pub bukkit_dimensions_detected: bool,
    pub downgrade_warning: Option<String>,
    pub migration_notice: Option<String>,
    pub warnings: Vec<String>,
}

/// Finalize/commit request parameters.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportCommitRequest {
    pub import_id: String,
    pub name: Option<String>,
    pub mc_version: Option<String>,
    pub loader_type: Option<String>,
    pub loader_version: Option<String>,
    pub java_args: Option<String>,
    pub external_port: Option<i32>,
    pub convert_dimensions: Option<bool>,
}

/// Errors raised during server import operations.
#[derive(Debug)]
pub enum ImportError {
    Invalid(String),
    NotFound(String),
    Archive(ArchiveError),
    Conflict(String),
    Io(io::Error),
}

impl fmt::Display for ImportError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ImportError::Invalid(m) => write!(f, "{m}"),
            ImportError::NotFound(m) => write!(f, "{m}"),
            ImportError::Archive(e) => write!(f, "Archive error: {e}"),
            ImportError::Conflict(m) => write!(f, "{m}"),
            ImportError::Io(e) => write!(f, "I/O error: {e}"),
        }
    }
}

impl std::error::Error for ImportError {}

impl From<io::Error> for ImportError {
    fn from(e: io::Error) -> Self {
        ImportError::Io(e)
    }
}

impl From<ArchiveError> for ImportError {
    fn from(e: ArchiveError) -> Self {
        ImportError::Archive(e)
    }
}

impl From<InstanceError> for ImportError {
    fn from(e: InstanceError) -> Self {
        match e {
            InstanceError::NotFound(m) => ImportError::NotFound(m),
            InstanceError::Conflict(m) => ImportError::Conflict(m),
            InstanceError::Invalid(m) => ImportError::Invalid(m),
            InstanceError::Io(io_err) => ImportError::Io(io_err),
        }
    }
}

/// Server import & migration orchestrator.
pub struct ServerImportService {
    staging_base_dir: PathBuf,
    instances: Arc<ServerInstanceManager>,
    signing_key: Option<Arc<SigningKey>>,
}

impl ServerImportService {
    pub fn new(
        data_dir: &Path,
        instances: Arc<ServerInstanceManager>,
        signing_key: Option<Arc<SigningKey>>,
    ) -> std::io::Result<Self> {
        let staging_base_dir = data_dir.join(".staging");
        fs::create_dir_all(&staging_base_dir)?;
        Ok(Self {
            staging_base_dir,
            instances,
            signing_key,
        })
    }

    /// Extract an uploaded server ZIP file into an isolated staging directory and analyze its contents.
    pub fn stage_and_analyze(&self, zip_temp_path: &Path) -> Result<PreflightReport, ImportError> {
        let import_id = Uuid::new_v4().to_string();
        let session_dir = self.staging_base_dir.join(&import_id);
        let unpacked_dir = session_dir.join("unpacked");
        fs::create_dir_all(&unpacked_dir)?;

        // Safe extraction using ArchiveGuard (configured with 1 TB server limit) and zircon-core zip extractor
        let file = File::open(zip_temp_path)?;
        let reader = BufReader::new(file);
        let guard = ArchiveGuard::for_server_import();
        extract_zip(reader, &unpacked_dir, &guard)?;

        // Flatten any single enclosing root wrapper directory
        flatten_single_root_if_needed(&unpacked_dir);

        // Run analysis on the unpacked layout
        match self.analyze_staged_directory(&import_id, &unpacked_dir) {
            Ok(report) => Ok(report),
            Err(e) => {
                // Cleanup on analysis failure
                let _ = fs::remove_dir_all(&session_dir);
                Err(e)
            }
        }
    }

    /// Analyzes the extracted files and produces the pre-flight inspection report.
    pub fn analyze_staged_directory(
        &self,
        import_id: &str,
        unpacked_dir: &Path,
    ) -> Result<PreflightReport, ImportError> {
        let mut warnings = Vec::new();

        // 1. Parse server.properties if present
        let props_path = unpacked_dir.join("server.properties");
        let server_properties_found = props_path.is_file();
        let props_map = if server_properties_found {
            parse_properties_file(&props_path).unwrap_or_default()
        } else {
            HashMap::new()
        };

        let level_name = props_map.get("level-name").map(String::as_str);
        let motd = props_map.get("motd").cloned();
        let server_name_prop = props_map.get("server-name").cloned();

        let suggested_name = server_name_prop
            .or(motd)
            .filter(|s| !s.trim().is_empty())
            .unwrap_or_else(|| "Imported Server".to_string());

        // 2. Discover and analyze world
        let world_dir_opt = discover_world_dir(unpacked_dir, level_name);
        let world_summary = world_dir_opt.as_ref().map(|w| analyze_world(unpacked_dir, w));

        let mut data_version = None;
        let mut mc_version = None;

        if let Some(ref w) = world_summary {
            if let Some(ref ldat) = w.level_dat {
                data_version = ldat.data_version;
                mc_version = ldat.minecraft_version.clone();
            }
        }

        // 3. Scan mod JARs in mods/
        let mods_dir = unpacked_dir.join("mods");
        let mut detected_mods = Vec::new();
        let mut detected_loader = ModLoaderType::Vanilla.id().to_string();
        let detected_loader_version = None;

        if mods_dir.is_dir() {
            if let Ok(entries) = fs::read_dir(&mods_dir) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.is_file()
                        && path
                            .extension()
                            .and_then(|e| e.to_str())
                            .map(|e| e.eq_ignore_ascii_case("jar"))
                            .unwrap_or(false)
                    {
                        let filename = path
                            .file_name()
                            .map(|f| f.to_string_lossy().to_string())
                            .unwrap_or_default();

                        let sha1 = compute_sha1_sync(&path).unwrap_or_default();
                        let murmur3 = curse_forge_fingerprint_of_file(&path).unwrap_or(0);

                        match extract(&path) {
                            Ok(meta) => {
                                let side_str = match meta.environment.to_ascii_lowercase().as_str() {
                                    "server" => "server",
                                    "client" => "client",
                                    _ => "both",
                                };

                                if detected_loader == ModLoaderType::Vanilla.id() {
                                    detected_loader = meta.loader_type.id().to_string();
                                }

                                detected_mods.push(PreflightModInfo {
                                    id: meta.id,
                                    name: meta.name,
                                    version: meta.version,
                                    loader: meta.loader_type.id().to_string(),
                                    side: side_str.to_string(),
                                    filename,
                                    sha1,
                                    murmur3,
                                    verified: true,
                                });
                            }
                            Err(_) => {
                                // Direct mod without standard metadata or unparsed JAR
                                let fallback_id = path
                                    .file_stem()
                                    .map(|s| s.to_string_lossy().to_string())
                                    .unwrap_or_else(|| "custom-mod".to_string());
                                detected_mods.push(PreflightModInfo {
                                    id: fallback_id.clone(),
                                    name: fallback_id,
                                    version: "unknown".to_string(),
                                    loader: detected_loader.clone(),
                                    side: "both".to_string(),
                                    filename,
                                    sha1,
                                    murmur3,
                                    verified: false,
                                });
                            }
                        }
                    }
                }
            }
        }

        // If no mods but Quilt/Fabric/Forge/NeoForge installer/metadata exists
        if detected_mods.is_empty() && detected_loader == ModLoaderType::Vanilla.id() {
            if unpacked_dir.join(".quilt").is_dir() || unpacked_dir.join("quilt-server-launch.jar").is_file() {
                detected_loader = ModLoaderType::Quilt.id().to_string();
            } else if unpacked_dir.join(".fabric").is_dir() || unpacked_dir.join("fabric-server-launch.jar").is_file() {
                detected_loader = ModLoaderType::Fabric.id().to_string();
            } else if unpacked_dir.join("libraries").join("net").join("neoforged").is_dir() {
                detected_loader = ModLoaderType::NeoForge.id().to_string();
            } else if unpacked_dir.join("libraries").join("net").join("minecraftforge").is_dir() {
                detected_loader = ModLoaderType::Forge.id().to_string();
            }
        }

        // 4. Detect config directories
        let mut configs_found = Vec::new();
        for config_name in &["config", "defaultconfigs", "kubejs", "openloader", "worldedit"] {
            if unpacked_dir.join(config_name).is_dir() {
                configs_found.push(config_name.to_string());
            }
        }

        // 5. Detect permission & ban files
        let mut permissions_found = Vec::new();
        for perm_file in &[
            "whitelist.json",
            "ops.json",
            "banned-players.json",
            "banned-ips.json",
            "usercache.json",
        ] {
            if unpacked_dir.join(perm_file).is_file() {
                permissions_found.push(perm_file.to_string());
            }
        }

        // 6. Bukkit dimensions check
        let bukkit_dimensions_detected = world_summary
            .as_ref()
            .map(|w| w.bukkit_dimensions_detected)
            .unwrap_or(false);

        // 7. Check downgrade warning against detected MC version
        let mut downgrade_warning = None;
        if let (Some(dv), Some(ref mc)) = (data_version, &mc_version) {
            if let Err(msg) = check_version_compatibility(Some(dv), mc) {
                downgrade_warning = Some(msg);
            }
        }

        // 8. Determine automatic layout migration notice
        let mut migration_notice = None;
        if let Some(ref w) = world_summary {
            if w.detected_layout == "legacy_1_21" || w.detected_layout == "bukkit_split" {
                migration_notice = Some(
                    "Legacy world layout detected. If launched on Minecraft 26.x+, overworld chunks, Nether/End dimensions, and all player inventories will be automatically migrated to the unified directory hierarchy on import."
                        .to_string(),
                );
            } else if w.detected_layout == "unified_26" {
                migration_notice = Some(
                    "Modern 26.x world layout detected. Directory structure is optimized for modern Minecraft and will be automatically adapted if launched on legacy versions."
                        .to_string(),
                );
            }
        }

        if world_summary.is_none() {
            warnings.push("No Minecraft world directory (level.dat) was detected in the archive. A new world will be generated on boot.".to_string());
        }

        Ok(PreflightReport {
            import_id: import_id.to_string(),
            suggested_name,
            minecraft_version: mc_version,
            data_version,
            detected_loader,
            detected_loader_version,
            world: world_summary,
            mods: detected_mods,
            configs_found,
            permissions_found,
            server_properties_found,
            bukkit_dimensions_detected,
            downgrade_warning,
            migration_notice,
            warnings,
        })
    }

    /// Commits the staged import session: moves world, configs, and verified mods into the new instance folder and registers the instance.
    pub fn commit_import(&self, req: ImportCommitRequest) -> Result<InstanceConfig, ImportError> {
        let session_dir = self.staging_base_dir.join(&req.import_id);
        let unpacked_dir = session_dir.join("unpacked");

        if !unpacked_dir.is_dir() {
            return Err(ImportError::NotFound(format!(
                "Import session not found or expired: {}",
                req.import_id
            )));
        }

        // 1. Re-read world summary to perform strict downgrade checks
        let props_path = unpacked_dir.join("server.properties");
        let props_map = if props_path.is_file() {
            parse_properties_file(&props_path).unwrap_or_default()
        } else {
            HashMap::new()
        };
        let level_name = props_map.get("level-name").map(String::as_str);
        let world_dir_opt = discover_world_dir(&unpacked_dir, level_name);
        let world_summary = world_dir_opt.as_ref().map(|w| analyze_world(&unpacked_dir, w));

        let detected_dv = world_summary
            .as_ref()
            .and_then(|w| w.level_dat.as_ref())
            .and_then(|l| l.data_version);

        let target_mc_version = req
            .mc_version
            .filter(|s| !s.trim().is_empty())
            .or_else(|| {
                world_summary
                    .as_ref()
                    .and_then(|w| w.level_dat.as_ref())
                    .and_then(|l| l.minecraft_version.clone())
            })
            .unwrap_or_else(|| "1.21.4".to_string());

        let target_loader = req
            .loader_type
            .filter(|s| !s.trim().is_empty())
            .unwrap_or_else(|| ModLoaderType::Fabric.id().to_string());

        let target_loader_version = req.loader_version.unwrap_or_default();
        let target_name = req
            .name
            .filter(|s| !s.trim().is_empty())
            .unwrap_or_else(|| "Imported Server".to_string());

        // 2. Strict Version Compatibility Check: Prevent Downgrades!
        if let Some(dv) = detected_dv {
            if let Err(err_msg) = check_version_compatibility(Some(dv), &target_mc_version) {
                return Err(ImportError::Conflict(err_msg));
            }
        }

        // 3. Bukkit/Paper Dimension Normalization
        let convert_dimensions = req.convert_dimensions.unwrap_or(true);
        if convert_dimensions {
            if let Some(ref w_dir) = world_dir_opt {
                let _ = normalize_bukkit_dimensions(&unpacked_dir, w_dir);
            }
        }

        // 4. Create the instance via ServerInstanceManager
        let instance_config = self.instances.create_instance(
            &target_name,
            &target_mc_version,
            &target_loader,
            &target_loader_version,
        )?;

        let instance_dir = self.instances.get_instance_dir(&instance_config.id);
        let instance_server_dir = instance_dir.join("server");
        let instance_mods_dir = instance_dir.join("mods");
        fs::create_dir_all(&instance_server_dir)?;
        fs::create_dir_all(&instance_mods_dir)?;

        // 5. Move world directory to <instance>/server/world/
        if let Some(w_dir) = world_dir_opt {
            let target_world = instance_server_dir.join("world");
            if w_dir == unpacked_dir {
                fs::create_dir_all(&target_world)?;
                for item in &[
                    "level.dat",
                    "level.dat_old",
                    "session.lock",
                    "region",
                    "entities",
                    "poi",
                    "data",
                    "DIM-1",
                    "DIM1",
                    "playerdata",
                    "stats",
                    "advancements",
                    "datapacks",
                ] {
                    let src = unpacked_dir.join(item);
                    if src.exists() {
                        let dst = target_world.join(item);
                        if src.is_dir() {
                            let _ = move_directory_contents(&src, &dst);
                            let _ = fs::remove_dir_all(&src);
                        } else {
                            let _ = fs::rename(&src, &dst);
                        }
                    }
                }
            } else {
                let _ = move_directory_contents(&w_dir, &target_world);
                let _ = fs::remove_dir_all(&w_dir);
            }

            // Ensure layout matches target Minecraft version (e.g. 26.x unified dimensions/players layout)
            let _ = migrate_world_layout_to_target_version(&target_world, &target_mc_version);
        }

        // 6. Move config folders
        for config_name in &["config", "defaultconfigs", "kubejs", "openloader", "worldedit"] {
            let src = unpacked_dir.join(config_name);
            if src.is_dir() {
                let dst = instance_server_dir.join(config_name);
                let _ = move_directory_contents(&src, &dst);
            }
        }

        // 7. Move permission and ban files
        for perm_file in &[
            "whitelist.json",
            "ops.json",
            "banned-players.json",
            "banned-ips.json",
            "usercache.json",
        ] {
            let src = unpacked_dir.join(perm_file);
            if src.is_file() {
                let dst = instance_server_dir.join(perm_file);
                let _ = fs::copy(&src, &dst);
            }
        }

        // 8. Move server.properties
        let src_props = unpacked_dir.join("server.properties");
        if src_props.is_file() {
            let dst_props = instance_server_dir.join("server.properties");
            let _ = fs::copy(&src_props, &dst_props);
        }

        // 9. Move and index mod JARs + construct signed BillOfMaterials
        let mut bom_mods = Vec::new();
        let src_mods = unpacked_dir.join("mods");
        if src_mods.is_dir() {
            if let Ok(entries) = fs::read_dir(&src_mods) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.is_file()
                        && path
                            .extension()
                            .and_then(|e| e.to_str())
                            .map(|e| e.eq_ignore_ascii_case("jar"))
                            .unwrap_or(false)
                    {
                        let filename = path
                            .file_name()
                            .map(|f| f.to_string_lossy().to_string())
                            .unwrap_or_default();

                        let dst_file = instance_mods_dir.join(&filename);
                        if fs::copy(&path, &dst_file).is_ok() {
                            let sha1 = compute_sha1_sync(&dst_file).unwrap_or_default();
                            let murmur3 = curse_forge_fingerprint_of_file(&dst_file).unwrap_or(0);
                            let file_size = fs::metadata(&dst_file).map(|m| m.len()).unwrap_or(0);

                            let (mod_id, mod_name, mod_version, mod_side) = match extract(&dst_file) {
                                Ok(meta) => {
                                    let side = match meta.environment.to_ascii_lowercase().as_str() {
                                        "server" => ModSide::Server,
                                        "client" => ModSide::Client,
                                        _ => ModSide::Both,
                                    };
                                    (meta.id, meta.name, meta.version, side)
                                }
                                Err(_) => {
                                    let stem = path
                                        .file_stem()
                                        .map(|s| s.to_string_lossy().to_string())
                                        .unwrap_or_else(|| "mod".to_string());
                                    (stem.clone(), stem, "1.0.0".to_string(), ModSide::Both)
                                }
                            };

                            let mut mod_entry = ModEntry::new(
                                Some(mod_id),
                                &filename,
                                Some(sha1),
                                murmur3,
                                Some("direct".to_string()),
                                None,
                                file_size,
                            );
                            mod_entry.title = Some(mod_name);
                            mod_entry.version = Some(mod_version);
                            mod_entry.side = mod_side;
                            mod_entry.enabled = true;
                            bom_mods.push(mod_entry);
                        }
                    }
                }
            }
        }

        // 10. Generate and sign BOM
        let mut bom = BillOfMaterials::new(
            instance_config.minecraft_version.clone(),
            instance_config.mod_loader.clone(),
            Some(instance_config.name.clone()),
        );
        bom.mods = bom_mods;

        let bom_service = BomService::new(instance_dir.join("bom.json"), Some(bom.clone()))
            .with_signing_key(self.signing_key.clone());
        let _ = bom_service.save();

        // 11. Apply optional custom JavaArgs or ExternalPort
        if let Some(args) = req.java_args.as_deref().filter(|s| !s.trim().is_empty()) {
            let _ = self
                .instances
                .update_instance_config(&instance_config.id, None, Some(args));
        }
        if let Some(port) = req.external_port.filter(|p| *p > 0) {
            let _ = self
                .instances
                .update_external_port(&instance_config.id, port);
        }

        // 12. Cleanup staging session directory
        let _ = fs::remove_dir_all(&session_dir);

        let final_config = self
            .instances
            .get_instance(&instance_config.id)
            .unwrap_or(instance_config);

        tracing::info!(
            "Successfully imported Minecraft server '{}' (ID: {})",
            final_config.name,
            final_config.id
        );

        Ok(final_config)
    }

    /// Discards temporary unpacked files for an import session.
    pub fn cancel_import(&self, import_id: &str) -> Result<bool, ImportError> {
        let session_dir = self.staging_base_dir.join(import_id);
        if session_dir.is_dir() {
            fs::remove_dir_all(&session_dir)?;
            Ok(true)
        } else {
            Ok(false)
        }
    }
}

/// If `unpacked_dir` contains only 1 subdirectory and no standard root files, flatten it.
fn flatten_single_root_if_needed(unpacked_dir: &Path) {
    if let Ok(entries) = fs::read_dir(unpacked_dir) {
        let valid_entries: Vec<_> = entries.flatten().collect();
        if valid_entries.len() == 1 {
            let single_path = valid_entries[0].path();
            if single_path.is_dir() {
                // Move everything from inside single_path up to unpacked_dir
                let temp_mv = unpacked_dir.parent().unwrap().join("temp_flatten");
                let _ = fs::rename(&single_path, &temp_mv);
                let _ = move_directory_contents(&temp_mv, unpacked_dir);
                let _ = fs::remove_dir_all(&temp_mv);
            }
        }
    }
}

/// Synchronously computes the SHA-1 of a file.
fn compute_sha1_sync(path: &Path) -> io::Result<String> {
    let mut file = File::open(path)?;
    let mut hasher = sha1::Sha1::new();
    let mut buffer = [0u8; 8192];
    loop {
        let n = file.read(&mut buffer)?;
        if n == 0 {
            break;
        }
        hasher.update(&buffer[..n]);
    }
    Ok(hex::encode(hasher.finalize()))
}

/// Parses Java `.properties` key=value files (e.g., `server.properties`).
pub fn parse_properties_file(path: &Path) -> io::Result<HashMap<String, String>> {
    let content = fs::read_to_string(path)?;
    let mut map = HashMap::new();
    for line in content.lines() {
        let line = line.trim();
        if line.starts_with('#') || line.starts_with('!') || line.is_empty() {
            continue;
        }
        if let Some((k, v)) = line.split_once('=') {
            map.insert(k.trim().to_string(), v.trim().to_string());
        }
    }
    Ok(map)
}
