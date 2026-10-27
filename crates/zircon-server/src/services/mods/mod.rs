//! Mod management: uploads, provider installs, BOM bookkeeping and version
//! re-sync.
//!
//! Port of `com.mcmanager.server.service.ModManagementService`.

pub mod cache;
pub mod installer;
pub mod resolver;
#[cfg(test)]
mod tests;


use std::fmt;
use std::path::PathBuf;
use std::sync::Arc;

use uuid::Uuid;
use zircon_core::api::curseforge::{embedded_curseforge_key, CurseForgeApiClient};
use zircon_core::api::modrinth::ModrinthApiClient;

use super::bom::BomService;

pub const ORIGIN_MODRINTH: &str = "modrinth";
pub const ORIGIN_CURSEFORGE: &str = "curseforge";
pub const ORIGIN_DIRECT: &str = "direct";
pub const ORIGIN_SERVER_CUSTOM: &str = "server_custom";

/// Errors raised by the mod management service.
#[derive(Debug)]
pub enum ModError {
    Invalid(String),
    Io(std::io::Error),
    Api(String),
}

impl fmt::Display for ModError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ModError::Invalid(m) => write!(f, "{m}"),
            ModError::Io(e) => write!(f, "{e}"),
            ModError::Api(m) => write!(f, "{m}"),
        }
    }
}

impl std::error::Error for ModError {}

impl From<std::io::Error> for ModError {
    fn from(e: std::io::Error) -> Self {
        ModError::Io(e)
    }
}

/// Physical disk state for an installed mod (active or disabled).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ModFileState {
    Enabled(PathBuf),
    Disabled(PathBuf),
}

impl ModFileState {
    pub fn file_path(&self) -> &std::path::Path {
        match self {
            Self::Enabled(p) | Self::Disabled(p) => p.as_path(),
        }
    }

    pub fn is_enabled(&self) -> bool {
        matches!(self, Self::Enabled(_))
    }
}

/// Manages the mods folder and BOM entries for one server/instance.
#[derive(Clone)]
pub struct ModManagementService {
    pub(crate) bom_service: Arc<BomService>,
    pub(crate) mods_dir: PathBuf,
    pub(crate) curse_forge_api_key: String,
    pub(crate) modrinth: ModrinthApiClient,
    pub(crate) curse_forge: CurseForgeApiClient,
}

impl ModManagementService {
    pub fn new(bom_service: Arc<BomService>, mods_dir: PathBuf, curse_forge_api_key: &str) -> Self {
        let mut cleaned_key = curse_forge_api_key.trim().trim_matches('"').trim_matches('\'').to_string();
        if cleaned_key.is_empty() {
            cleaned_key = embedded_curseforge_key();
        }
        Self {
            bom_service,
            mods_dir,
            curse_forge_api_key: cleaned_key.clone(),
            modrinth: ModrinthApiClient::new(),
            curse_forge: CurseForgeApiClient::new(cleaned_key),
        }
    }

    pub fn modrinth(&self) -> &ModrinthApiClient {
        &self.modrinth
    }

    pub fn curse_forge(&self) -> &CurseForgeApiClient {
        &self.curse_forge
    }

    /// `true` if a CurseForge API key is configured (required for CF search/verify).
    pub fn has_curse_forge_key(&self) -> bool {
        !self.curse_forge_api_key.trim().is_empty()
    }

    /// Resolves a BOM file name to the on-disk file, or `None` if absent.
    pub fn get_mod_file(&self, filename: &str) -> Option<PathBuf> {
        let file = self.safe_resolve(filename)?;
        if file.is_file() {
            Some(file)
        } else {
            None
        }
    }
}


/// Windows device names that are reserved even with an extension (`CON`, `NUL`,
/// `COM1`...). Uploading a file with one of these names would create an
/// unreadable/undeletable entry on Windows, so they are prefixed defensively.
const WINDOWS_RESERVED: &[&str] = &[
    "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8",
    "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
];

/// Strips path separators and control characters so uploads cannot escape the
/// mods dir.
pub fn sanitize_filename(filename: &str) -> Result<String, ModError> {
    if filename.is_empty() {
        return Err(ModError::Invalid("filename is required".to_string()));
    }
    let mut base: String = filename.replace('\\', "/");
    if let Some(slash) = base.rfind('/') {
        base = base[slash + 1..].to_string();
    }
    let sanitized: String = base
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-') {
                c
            } else {
                '_'
            }
        })
        .collect();
    // Collapse consecutive dots ("..") and strip leading/trailing dots
    let mut collapsed = String::with_capacity(sanitized.len());
    let mut prev_dot = false;
    for c in sanitized.trim_matches('.').chars() {
        if c == '.' {
            if !prev_dot {
                collapsed.push(c);
            }
            prev_dot = true;
        } else {
            prev_dot = false;
            collapsed.push(c);
        }
    }
    let mut base = if collapsed.trim().is_empty() || collapsed.eq_ignore_ascii_case("jar") {
        format!("mod-{}.jar", &Uuid::new_v4().simple().to_string()[..8])
    } else {
        collapsed
    };
    if !base.to_lowercase().ends_with(".jar") {
        base = format!("{base}.jar");
    }

    // Windows reserved device names, regardless of extension casing.
    // Windows treats the segment before the first '.' as the device name (e.g. CON.txt.jar).
    let upper = base.to_ascii_uppercase();
    let stem = upper.strip_suffix(".JAR").unwrap_or(&upper).to_string();
    let first_segment = stem.split('.').next().unwrap_or(&stem);
    if WINDOWS_RESERVED.contains(&stem.as_str()) || WINDOWS_RESERVED.contains(&first_segment) {
        base = format!("file_{base}");
    }
    Ok(base)
}

fn normalize_origin(origin: Option<&str>) -> String {
    match origin {
        Some(o) => match o.to_lowercase().as_str() {
            "modrinth" => ORIGIN_MODRINTH.to_string(),
            "curseforge" => ORIGIN_CURSEFORGE.to_string(),
            _ => ORIGIN_DIRECT.to_string(),
        },
        None => ORIGIN_DIRECT.to_string(),
    }
}

/// Result of a modpack installation.
#[derive(Debug, Clone, Default)]
pub struct ModpackInstallResult {
    pub installed_count: i32,
    pub failed_mods: Vec<String>,
    pub message: String,
}

