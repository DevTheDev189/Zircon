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
    /// Whether Discord Rich Presence is enabled.
    pub discord_rpc: bool,
    /// Custom JVM arguments (e.g. GC flags like Shenandoah, ZGC, Aikar's flags).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub custom_jvm_args: Option<String>,
    /// Custom Java binary path override (e.g. C:\Program Files\Java\jdk-21\bin\java.exe).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub java_path_override: Option<String>,
    /// Game window width (0 = Minecraft default).
    pub window_width: u32,
    /// Game window height (0 = Minecraft default).
    pub window_height: u32,
    /// Whether to start the game in fullscreen mode.
    pub start_fullscreen: bool,
    /// Developer mode: allow unverified custom mods in Join-by-Code P2P sessions with explicit approval.
    /// Never applies to dedicated servers.
    pub allow_unverified_p2p_mods: bool,
    /// UI accent theme preset ("zircon-cyan", "amethyst-purple", "emerald-green", "redstone-crimson", "blaze-amber", "diamond-blue", "obsidian-slate", or "custom").
    pub theme: String,
    /// Custom accent hex code (e.g. "#47d2c9") when theme is "custom".
    #[serde(skip_serializing_if = "Option::is_none")]
    pub custom_accent: Option<String>,
    /// Background & canvas theme preset ("deep-void", "oled-black", "abyssal-navy", "carbon-slate", "royal-obsidian", "emerald-shadow", "custom").
    pub bg_theme: String,
    /// Custom base background hex code (e.g. "#050505") when bg_theme is "custom".
    #[serde(skip_serializing_if = "Option::is_none")]
    pub custom_bg: Option<String>,
    /// Custom card surface hex code (e.g. "#111111") when bg_theme is "custom".
    #[serde(skip_serializing_if = "Option::is_none")]
    pub custom_card_bg: Option<String>,
    /// Button & component geometry style ("rounded", "pill", "sharp").
    pub button_style: String,
    /// Glassmorphism & transparency style ("standard", "frosted", "solid").
    pub glass_effect: String,
}

impl Default for LauncherSettings {
    fn default() -> Self {
        Self {
            memory_gb: DEFAULT_MEMORY_GB,
            discord_rpc: true,
            custom_jvm_args: None,
            java_path_override: None,
            window_width: 0,
            window_height: 0,
            start_fullscreen: false,
            allow_unverified_p2p_mods: false,
            theme: "zircon-cyan".to_string(),
            custom_accent: None,
            bg_theme: "deep-void".to_string(),
            custom_bg: None,
            custom_card_bg: None,
            button_style: "rounded".to_string(),
            glass_effect: "standard".to_string(),
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
        assert!(settings.discord_rpc);
    }

    #[test]
    fn save_load_round_trip() {
        let file = temp_file();
        save_to(
            &file,
            &LauncherSettings {
                memory_gb: 8,
                discord_rpc: false,
                ..Default::default()
            },
        );
        let loaded = load_from(&file);
        assert_eq!(8, loaded.memory_gb);
        assert!(!loaded.discord_rpc);
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
        assert!(loaded.discord_rpc);
        assert_eq!(
            LauncherSettings {
                memory_gb: 6,
                discord_rpc: true,
                ..Default::default()
            },
            loaded
        );
        let _ = std::fs::remove_file(&file);
    }

    #[test]
    fn memory_is_clamped_to_slider_range() {
        let file = temp_file();
        save_to(
            &file,
            &LauncherSettings {
                memory_gb: 64,
                discord_rpc: true,
                ..Default::default()
            },
        );
        let loaded = load_from(&file);
        assert_eq!(16, loaded.memory_gb);
        assert!(loaded.discord_rpc);
        let _ = std::fs::remove_file(&file);
    }

    #[test]
    fn advanced_settings_round_trip() {
        let file = temp_file();
        let original = LauncherSettings {
            memory_gb: 6,
            discord_rpc: true,
            custom_jvm_args: Some("-XX:+UseZGC -XX:+ZGenerational".to_string()),
            java_path_override: Some("C:\\Java\\bin\\java.exe".to_string()),
            window_width: 1920,
            window_height: 1080,
            start_fullscreen: true,
            allow_unverified_p2p_mods: false,
            theme: "amethyst-purple".to_string(),
            custom_accent: Some("#a855f7".to_string()),
            bg_theme: "abyssal-navy".to_string(),
            custom_bg: Some("#050a14".to_string()),
            custom_card_bg: Some("#0c1830".to_string()),
            button_style: "pill".to_string(),
            glass_effect: "frosted".to_string(),
        };
        save_to(&file, &original);
        let loaded = load_from(&file);
        assert_eq!(original, loaded);
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
