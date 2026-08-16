//! Persistent metadata for one isolated Zircon server instance.
//!
//! Port of `com.mcmanager.core.model.InstanceConfig`. The `ModLoaderInfo` is
//! **locked at creation time**: there is no setter for it (and no API route
//! that mutates it), so a server's mod loader type can never be switched out
//! from under the mods that were installed for it. Only `name`, `java_args`,
//! `auto_start`, `minecraft_version`, the loader *version* and the backup
//! settings are mutable after creation.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::bom::ModLoaderInfo;

/// Manual backups only — the scheduler never auto-backs up.
pub const BACKUP_OFF: &str = "off";
pub const BACKUP_DAILY: &str = "daily";
pub const BACKUP_WEEKLY: &str = "weekly";
pub const BACKUP_MONTHLY: &str = "monthly";

/// Default number of backups kept per instance before old ones are pruned.
pub const DEFAULT_BACKUP_RETENTION: i32 = 10;

/// Allowed bounds for the per-instance retention setting.
pub const MIN_BACKUP_RETENTION: i32 = 1;
pub const MAX_BACKUP_RETENTION: i32 = 100;

/// All frequency values accepted by the backup scheduler.
pub fn is_valid_backup_frequency(freq: &str) -> bool {
    matches!(
        freq,
        BACKUP_OFF | BACKUP_DAILY | BACKUP_WEEKLY | BACKUP_MONTHLY
    )
}

/// Persistent metadata for one isolated Zircon server instance.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct InstanceConfig {
    #[serde(default = "random_instance_id")]
    pub id: String,
    #[serde(default = "default_name")]
    pub name: String,
    #[serde(default)]
    pub minecraft_version: String,
    /// IMMUTABLE after creation — no setter exposed to the API!
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mod_loader: Option<ModLoaderInfo>,
    /// Automatically assigned, e.g. 25566, 25567.
    pub internal_mc_port: i32,
    /// Player-facing port where the multiplexer accepts connections for this
    /// instance (0 = unassigned).
    #[serde(default)]
    pub external_mc_port: i32,
    #[serde(default = "default_java_args")]
    pub java_args: String,
    #[serde(default)]
    pub auto_start: bool,
    /// Backup cadence: one of `BACKUP_OFF`, `BACKUP_DAILY`, `BACKUP_WEEKLY`, `BACKUP_MONTHLY`.
    #[serde(default = "default_backup_frequency")]
    pub backup_frequency: String,
    /// Local time of day (24-hour "HH:MM") at which scheduled backups run.
    #[serde(default = "default_backup_time")]
    pub backup_time: String,
    /// How many backups to keep; older ones are pruned.
    #[serde(default = "default_backup_retention")]
    pub backup_retention: i32,
}

fn random_instance_id() -> String {
    Uuid::new_v4().simple().to_string()[..8].to_string()
}

fn default_name() -> String {
    "New Zircon Server".to_string()
}

fn default_java_args() -> String {
    "-Xms2G -Xmx4G".to_string()
}

fn default_backup_frequency() -> String {
    BACKUP_OFF.to_string()
}

fn default_backup_time() -> String {
    "02:00".to_string()
}

fn default_backup_retention() -> i32 {
    DEFAULT_BACKUP_RETENTION
}

impl InstanceConfig {
    /// Creates a new instance configuration. `loader_type` is one of
    /// "vanilla", "fabric", "quilt", "forge", "neoforge"; the loader is frozen
    /// in place from this moment on. The external (player-facing) port is left
    /// unassigned (0) and allocated by the instance manager.
    pub fn new(
        name: impl Into<String>,
        minecraft_version: impl Into<String>,
        loader_type: impl Into<String>,
        loader_version: impl Into<String>,
        internal_mc_port: i32,
    ) -> Self {
        Self::with_external_port(
            name,
            minecraft_version,
            loader_type,
            loader_version,
            internal_mc_port,
            0,
        )
    }

    pub fn with_external_port(
        name: impl Into<String>,
        minecraft_version: impl Into<String>,
        loader_type: impl Into<String>,
        loader_version: impl Into<String>,
        internal_mc_port: i32,
        external_mc_port: i32,
    ) -> Self {
        let mut config = Self {
            id: random_instance_id(),
            name: name.into(),
            minecraft_version: minecraft_version.into(),
            mod_loader: Some(ModLoaderInfo::new(loader_type, loader_version, None)),
            internal_mc_port,
            external_mc_port,
            java_args: default_java_args(),
            auto_start: false,
            backup_frequency: default_backup_frequency(),
            backup_time: default_backup_time(),
            backup_retention: default_backup_retention(),
        };
        // "vanilla" installs carry no mod loader metadata.
        if config.mod_loader.as_ref().map(|l| l.r#type.as_str()) == Some("vanilla") {
            config.mod_loader = None;
        }
        config
    }

    /// Updates the mod loader *version* string (e.g. Fabric `0.15.11`).
    /// The loader *type* stays locked — this only ever touches the version.
    pub fn set_loader_version(&mut self, loader_version: impl Into<String>) {
        match self.mod_loader.as_mut() {
            Some(loader) => loader.version = loader_version.into(),
            None => {
                self.mod_loader = Some(ModLoaderInfo::new("vanilla", loader_version, None));
            }
        }
    }

    /// Loader type id ("fabric", "neoforge", ...) or "vanilla" when unmodded.
    pub fn loader_type(&self) -> &str {
        self.mod_loader
            .as_ref()
            .map(|l| l.r#type.as_str())
            .unwrap_or("vanilla")
    }

    /// Loader version string, empty for vanilla installs.
    pub fn loader_version(&self) -> &str {
        self.mod_loader
            .as_ref()
            .map(|l| l.version.as_str())
            .unwrap_or("")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_instance_generates_short_id_and_locks_loader() {
        let mut config = InstanceConfig::new("My Server", "1.20.4", "fabric", "0.15.11", 25566);

        assert_eq!(8, config.id.len());
        assert_eq!("fabric", config.loader_type());
        assert_eq!("0.15.11", config.loader_version());

        // Loader type is immutable: no setter exists. Version can change.
        config.set_loader_version("0.16.0");
        assert_eq!("0.16.0", config.loader_version());
        assert_eq!("fabric", config.loader_type());
    }

    #[test]
    fn vanilla_install_has_no_mod_loader() {
        let config = InstanceConfig::new("Vanilla", "1.20.4", "vanilla", "", 25566);
        assert!(config.mod_loader.is_none());
        assert_eq!("vanilla", config.loader_type());
    }

    #[test]
    fn round_trip_via_json() {
        let config =
            InstanceConfig::with_external_port("T", "1.21", "forge", "49.0.0", 25567, 25566);
        let json = serde_json::to_string(&config).unwrap();
        let parsed: InstanceConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(config, parsed);
        // Field names follow the camelCase schema.
        assert!(json.contains("\"internalMcPort\""));
        assert!(json.contains("\"backupFrequency\""));
    }

    #[test]
    fn backup_frequency_validation() {
        assert!(is_valid_backup_frequency("daily"));
        assert!(is_valid_backup_frequency("off"));
        assert!(!is_valid_backup_frequency("hourly"));
    }
}
