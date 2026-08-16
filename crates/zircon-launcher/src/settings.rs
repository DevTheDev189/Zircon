//! Launcher UI settings persisted at `~/.mcmanager/settings.json`: the RAM
//! allocation slider (2–16 GB).
//!
//! The JavaFX-era security toggles (strict hash verification, trust-direct-mods)
//! were removed deliberately: mod verification is always strict and mods with
//! no provider to verify against are always rejected, so a player can never
//! weaken the download protection.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use tracing::warn;

use crate::paths::settings_file;

/// Default RAM allocation in GB.
pub const DEFAULT_MEMORY_GB: u32 = 4;

/// Persisted launcher settings.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct LauncherSettings {
    /// Max JVM heap in GB (UI slider range 2–16).
    pub memory_gb: u32,
}

impl Default for LauncherSettings {
    fn default() -> Self {
        Self {
            memory_gb: DEFAULT_MEMORY_GB,
        }
    }
}

impl LauncherSettings {
    /// Clamps a memory value into the UI slider's 2–16 GB range.
    pub fn with_clamped_memory(mut self, memory_gb: u32) -> Self {
        self.memory_gb = memory_gb.clamp(2, 16);
        self
    }
}

/// Loads the settings file (defaults when missing or corrupt — the Java keeps
/// the settings purely in memory, so this is the launcher's own persistence).
pub fn load_settings() -> LauncherSettings {
    load_from(&settings_file())
}

/// Loads settings from an explicit file (used by tests).
pub fn load_from(file: &PathBuf) -> LauncherSettings {
    if !file.is_file() {
        return LauncherSettings::default();
    }
    match std::fs::read_to_string(file) {
        Ok(text) => match serde_json::from_str::<LauncherSettings>(&text) {
            Ok(settings) => {
                let gb = settings.memory_gb;
                settings.with_clamped_memory(gb)
            }
            Err(e) => {
                warn!("Could not parse {}: {e}", file.display());
                LauncherSettings::default()
            }
        },
        Err(e) => {
            warn!("Could not read {}: {e}", file.display());
            LauncherSettings::default()
        }
    }
}

/// Persists settings to the default file. Best-effort like the other launcher
/// JSON stores.
pub fn save_settings(settings: &LauncherSettings) {
    save_to(&settings_file(), settings);
}

/// Persists settings to an explicit file (used by tests).
pub fn save_to(file: &PathBuf, settings: &LauncherSettings) {
    if let Some(parent) = file.parent() {
        if let Err(e) = std::fs::create_dir_all(parent) {
            warn!("Could not create {}: {e}", parent.display());
            return;
        }
    }
    match serde_json::to_string_pretty(settings) {
        Ok(json) => {
            if let Err(e) = std::fs::write(file, json) {
                warn!("Could not write {}: {e}", file.display());
            }
        }
        Err(e) => warn!("Could not serialize settings: {e}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_file() -> PathBuf {
        std::env::temp_dir().join(format!(
            "zircon-settings-{}.json",
            uuid::Uuid::new_v4().simple()
        ))
    }

    #[test]
    fn default_settings() {
        let settings = LauncherSettings::default();
        assert_eq!(DEFAULT_MEMORY_GB, settings.memory_gb);
    }

    #[test]
    fn save_load_round_trip() {
        let file = temp_file();
        save_to(&file, &LauncherSettings { memory_gb: 8 });
        let loaded = load_from(&file);
        assert_eq!(8, loaded.memory_gb);
        let _ = std::fs::remove_file(&file);
    }

    #[test]
    fn legacy_security_toggles_are_ignored() {
        // Files written by older launcher versions carry strictVerification /
        // trustDirectMods; they must load fine and the toggles must have no
        // effect (verification is always strict, direct mods always rejected).
        let file = temp_file();
        std::fs::write(
            &file,
            r#"{"memoryGb": 6, "strictVerification": false, "trustDirectMods": true}"#,
        )
        .unwrap();
        let loaded = load_from(&file);
        assert_eq!(6, loaded.memory_gb);
        assert_eq!(LauncherSettings { memory_gb: 6 }, loaded);
        let _ = std::fs::remove_file(&file);
    }

    #[test]
    fn memory_is_clamped_to_slider_range() {
        let file = temp_file();
        save_to(&file, &LauncherSettings { memory_gb: 64 });
        let loaded = load_from(&file);
        assert_eq!(16, loaded.memory_gb);
        let _ = std::fs::remove_file(&file);
    }

    #[test]
    fn missing_or_corrupt_returns_default() {
        let missing = temp_file();
        assert_eq!(LauncherSettings::default(), load_from(&missing));

        let corrupt = temp_file();
        std::fs::write(&corrupt, "{nope").unwrap();
        assert_eq!(LauncherSettings::default(), load_from(&corrupt));
        let _ = std::fs::remove_file(&corrupt);
    }
}
