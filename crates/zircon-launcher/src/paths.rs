//! Well-known paths under `~/.mcmanager` shared by the launcher modules.
//!
//! Port of the `Path.of(System.getProperty("user.home"), ".mcmanager", ...)`
//! constants in the Java launcher.

use std::path::PathBuf;

use crate::error::LauncherError;

/// The current user's home directory (`USERPROFILE` on Windows, `HOME` on
/// Unix), falling back to the current directory when neither is set.
pub fn home_dir() -> PathBuf {
    std::env::var_os("USERPROFILE")
        .or_else(|| std::env::var_os("HOME"))
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
}

/// `~/.mcmanager` — the launcher's private data directory.
pub fn mcmanager_dir() -> PathBuf {
    home_dir().join(".mcmanager")
}

/// `~/.mcmanager/launcher` — manifests, libraries, assets, JDKs, natives.
pub fn launcher_dir() -> PathBuf {
    mcmanager_dir().join("launcher")
}

/// `~/.mcmanager/auth_cache.json` — the persisted Microsoft session.
pub fn auth_cache_file() -> PathBuf {
    mcmanager_dir().join("auth_cache.json")
}

/// `~/.mcmanager/accounts.json` — the saved accounts list and active account.
pub fn accounts_file() -> PathBuf {
    mcmanager_dir().join("accounts.json")
}


/// `~/.mcmanager/client_id.txt` — optional one-line Azure client id override.
pub fn client_id_file() -> PathBuf {
    mcmanager_dir().join("client_id.txt")
}

/// `~/.mcmanager/offline_instances` — offline (single-player) instances.
pub fn offline_instances_dir() -> PathBuf {
    mcmanager_dir().join("offline_instances")
}

/// `~/.mcmanager/servers.json` — the saved "Your Servers" list.
pub fn servers_file() -> PathBuf {
    mcmanager_dir().join("servers.json")
}

/// `~/.mcmanager/settings.json` — launcher UI settings (RAM, hash toggles).
pub fn settings_file() -> PathBuf {
    mcmanager_dir().join("settings.json")
}

/// `~/.mcmanager/keybind_library` — saved global keybind profiles.
pub fn keybinds_dir() -> PathBuf {
    mcmanager_dir().join("keybind_library")
}

/// `~/.mcmanager/skins` — active skin + history folder.
pub fn skins_dir() -> PathBuf {
    mcmanager_dir().join("skins")
}

/// `~/.mcmanager/skins/active_skin.png` — the active custom skin.
pub fn active_skin_file() -> PathBuf {
    skins_dir().join("active_skin.png")
}

/// `~/.mcmanager/skins/active_skin.json` — the arm variant (`classic`/`slim`)
/// of the active custom skin, kept alongside the PNG so the UI can restore the
/// correct model and upload without silently flipping the player's arms.
pub fn active_skin_variant_file() -> PathBuf {
    skins_dir().join("active_skin.json")
}

/// `~/.mcmanager/skins/history` — archived skins (pruned to 25 entries).
pub fn skin_history_dir() -> PathBuf {
    skins_dir().join("history")
}

/// `~/.mcmanager/skins/presets` — the user-editable preset gallery. Drop PNG
/// files here to add or replace preset skins; seeded with the bundled defaults
/// on first use.
pub fn skin_presets_dir() -> PathBuf {
    skins_dir().join("presets")
}

/// `~/.mcmanager/models` — local AI models directory.
pub fn models_dir() -> PathBuf {
    mcmanager_dir().join("models")
}

/// `~/.mcmanager/models/skin_dit_v1` — directory for the optional on-demand Skin Pixel-DiT model.
pub fn skin_ai_model_dir() -> PathBuf {
    models_dir().join("skin_dit_v1")
}

/// `~/.zircon/instances` — per-server game directories keyed by host_port
/// (mirrors the Java launcher's `INSTANCES_ROOT`).
pub fn instances_dir() -> PathBuf {
    home_dir().join(".zircon").join("instances")
}

/// Strictly validates a user-supplied filename used in a delete operation.
///
/// The name must be a plain basename: non-empty, no path separators (both `/`
/// and `\`), and no leading dot (which also rejects `.` and `..`). Anything
/// else would let the caller delete files outside the intended directory, so
/// it fails closed with [`LauncherError::InvalidInput`].
pub fn sanitize_filename_strict(filename: &str) -> Result<String, LauncherError> {
    if filename.is_empty()
        || filename.contains('/')
        || filename.contains('\\')
        || filename.starts_with('.')
    {
        return Err(LauncherError::InvalidInput(
            "Illegal file path traversal attempt".to_string(),
        ));
    }
    Ok(filename.to_string())
}

/// Validates an optional subfolder name for instance folder browsing.
/// Returns Ok(None) for the instance root, or Ok(Some(clean_name)) for safe subdirectories.
pub fn validate_subfolder_name(subfolder: Option<&str>) -> Result<Option<String>, LauncherError> {
    let Some(raw) = subfolder else {
        return Ok(None);
    };
    let trimmed = raw.trim();
    if trimmed.is_empty() || trimmed == "." {
        return Ok(None);
    }
    if trimmed.contains('/') || trimmed.contains('\\') || trimmed.contains("..") || trimmed.starts_with('.') {
        return Err(LauncherError::InvalidInput(
            "Illegal subfolder path traversal attempt".to_string(),
        ));
    }
    // Allow known/standard directories or safe alphanumeric names
    let clean = trimmed.to_lowercase();
    match clean.as_str() {
        "mods" | "config" | "saves" | "screenshots" | "shaderpacks" | "resourcepacks"
        | "logs" | "crash-reports" => Ok(Some(clean)),
        _ => {
            // General safety check: allow only simple identifier-like directory names
            if clean.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-') {
                Ok(Some(clean))
            } else {
                Err(LauncherError::InvalidInput(format!("Invalid subfolder name: {raw}")))
            }
        }
    }
}

/// `~/.mcmanager/cache/mods` — shared mods cache across instances and P2P sessions.
pub fn mods_cache_dir() -> PathBuf {
    mcmanager_dir().join("cache").join("mods")
}

/// Validates that the drive hosting `target_dir` has at least `required_bytes` free space.
///
/// Returns an error if disk space is below `required_bytes`, avoiding failed downloads
/// or corrupted partial extractions mid-flight.
pub fn check_available_disk_space(target_dir: &std::path::Path, required_bytes: u64) -> Result<(), LauncherError> {
    let disks = sysinfo::Disks::new_with_refreshed_list();
    if disks.is_empty() {
        return Ok(());
    }

    let target_abs = if target_dir.is_absolute() {
        target_dir.to_path_buf()
    } else if let Ok(abs) = std::fs::canonicalize(target_dir) {
        abs
    } else {
        home_dir().join(target_dir)
    };

    let target_str = target_abs.to_string_lossy().to_lowercase();
    let mut best_match: Option<(&sysinfo::Disk, usize)> = None;

    for disk in &disks {
        let mount_str = disk.mount_point().to_string_lossy().to_lowercase();
        let trimmed = mount_str.trim_end_matches(['\\', '/']);
        if target_str.starts_with(&mount_str) || (!trimmed.is_empty() && target_str.starts_with(trimmed)) {
            let match_len = mount_str.len();
            if best_match.as_ref().map_or(true, |(_, len)| match_len > *len) {
                best_match = Some((disk, match_len));
            }
        }
    }

    let disk = best_match.map(|(d, _)| d).or_else(|| {
        disks.iter().max_by_key(|d| d.total_space())
    });

    if let Some(d) = disk {
        let available = d.available_space();
        if available < required_bytes {
            let mount_display = d.mount_point().display();
            let avail_gb = available as f64 / (1024.0 * 1024.0 * 1024.0);
            let req_gb = required_bytes as f64 / (1024.0 * 1024.0 * 1024.0);
            return Err(LauncherError::InvalidInput(format!(
                "Insufficient disk space on {mount_display}: {avail_gb:.2} GB available, {req_gb:.2} GB required"
            )));
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_check_available_disk_space_success() {
        let home = home_dir();
        // 1 KB should easily pass on any working system
        assert!(check_available_disk_space(&home, 1024).is_ok());
    }

    #[test]
    fn test_check_available_disk_space_exceeded() {
        let home = home_dir();
        // 100 Petabytes should fail unless someone is running on a datacenter SAN
        let res = check_available_disk_space(&home, 100 * 1024 * 1024 * 1024 * 1024 * 1024);
        assert!(res.is_err());
        let err = res.unwrap_err().to_string();
        assert!(err.contains("Insufficient disk space"));
    }
}

