//! Data models for discovering and migrating instances from external Minecraft launchers.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Types of supported third-party and official launchers.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum SourceLauncherType {
    Prism,
    Modrinth,
    CurseForge,
    Vanilla,
    AtLauncher,
}

impl SourceLauncherType {
    pub fn display_name(&self) -> &'static str {
        match self {
            Self::Prism => "Prism Launcher",
            Self::Modrinth => "Modrinth App",
            Self::CurseForge => "CurseForge",
            Self::Vanilla => "Official Minecraft Launcher",
            Self::AtLauncher => "ATLauncher",
        }
    }
}

/// Metadata describing a discovered external instance ready for 1-click import.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiscoveredInstance {
    pub id: String,
    pub source_launcher: SourceLauncherType,
    pub name: String,
    pub minecraft_version: String,
    pub loader_type: String,
    pub loader_version: String,
    pub source_path: PathBuf,
    pub game_dir: PathBuf,
    pub mod_count: usize,
    pub world_count: usize,
    pub last_played_ms: Option<i64>,
    pub icon_base64: Option<String>,
}

/// Aggregated discovery results for a specific launcher on the host system.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MigrationScanResult {
    pub launcher_type: SourceLauncherType,
    pub launcher_name: String,
    pub installed: bool,
    pub instances_path: Option<PathBuf>,
    pub instances: Vec<DiscoveredInstance>,
}

/// Strategy for copying files from the source launcher into Zircon.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MigrationStrategy {
    /// Attempts hardlinking for large mod JARs (0 extra disk space, instant),
    /// falling back to normal copy if hardlinks fail across partitions.
    HardlinkWithFallback,
    /// Traditional full file copy (completely independent duplicate).
    FullCopy,
}

impl Default for MigrationStrategy {
    fn default() -> Self {
        Self::HardlinkWithFallback
    }
}

/// Real-time progress payload emitted to the UI during multi-instance migrations.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MigrationProgressEvent {
    pub current_instance_name: String,
    pub current_file: String,
    pub completed_instances: usize,
    pub total_instances: usize,
    pub fraction: f32,
}
