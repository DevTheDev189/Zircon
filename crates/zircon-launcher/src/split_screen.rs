//! Couch Co-Op Split-Screen engine.
//!
//! Provides multi-player local split-screen orchestration:
//! - Win32 borderless quadrant / split layout window positioning.
//! - Per-instance Controlify & MidnightControls gamepad GUID locking and unfocused input configuration.
//! - Lean guest workspace provisioning using directory junctions/symlinks for `mods/` and shared assets.
//! - Options customization for seamless local multiplayer (unfocused audio, lean render distance, no pause on lost focus).

use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::LauncherError;
use crate::launch::options::OptionsFileUtil;

/// Supported screen partitioning layouts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SplitScreenLayout {
    /// 2 Players: Left / Right vertical split
    TwoPlayerVertical,
    /// 2 Players: Top / Bottom horizontal split
    TwoPlayerHorizontal,
    /// 3 Players: Top full-width, bottom split into two quadrants
    ThreePlayerGrid,
    /// 4 Players: 2x2 Quadrant grid
    FourPlayerGrid,
}

/// Configuration for an individual player in a couch co-op session.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlayerConfig {
    /// 0 is the Host instance (main folder), 1..N are guest instances.
    pub player_index: usize,
    /// Mandatory authenticated Microsoft account UUID (ownership verification).
    pub account_uuid: String,
    /// Player display name (gamertag).
    pub name: String,
    pub controller_id: Option<String>,
    pub controller_name: Option<String>,
    #[serde(default = "default_render_distance")]
    pub render_distance: u32,
    #[serde(default = "default_memory_gb")]
    pub memory_gb: u32,
}

fn default_render_distance() -> u32 {
    8
}

fn default_memory_gb() -> u32 {
    3
}

/// Incoming request to launch a couch co-op session.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SplitScreenSessionRequest {
    pub instance_id: String,
    /// Optional server address (host:port) to auto-connect to (for online servers).
    #[serde(default)]
    pub server_address: Option<String>,
    pub layout: SplitScreenLayout,
    pub players: Vec<PlayerConfig>,
}

/// Real-time status of an active couch co-op session.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SplitScreenStatus {
    pub is_active: bool,
    pub layout: Option<SplitScreenLayout>,
    pub player_count: usize,
    pub pids: Vec<u32>,
}

/// Internal tracked state for an active couch co-op session.
pub struct CouchCoopSession {
    pub instance_id: String,
    pub layout: SplitScreenLayout,
    pub pids: Vec<u32>,
    pub children: Vec<tokio::process::Child>,
}

/// Computes (x, y, width, height) rectangles for each viewport according to the chosen layout
/// and screen resolution.
pub fn calculate_window_rects(
    layout: SplitScreenLayout,
    screen_w: i32,
    screen_h: i32,
    player_count: usize,
) -> Vec<(i32, i32, i32, i32)> {
    let half_w = screen_w / 2;
    let rem_w = screen_w - half_w;
    let half_h = screen_h / 2;
    let rem_h = screen_h - half_h;

    match layout {
        SplitScreenLayout::TwoPlayerVertical => {
            vec![
                (0, 0, half_w, screen_h),
                (half_w, 0, rem_w, screen_h),
            ]
        }
        SplitScreenLayout::TwoPlayerHorizontal => {
            vec![
                (0, 0, screen_w, half_h),
                (0, half_h, screen_w, rem_h),
            ]
        }
        SplitScreenLayout::ThreePlayerGrid => {
            vec![
                (0, 0, screen_w, half_h),
                (0, half_h, half_w, rem_h),
                (half_w, half_h, rem_w, rem_h),
            ]
        }
        SplitScreenLayout::FourPlayerGrid => {
            let mut rects = vec![
                (0, 0, half_w, half_h),
                (half_w, 0, rem_w, half_h),
                (0, half_h, half_w, rem_h),
                (half_w, half_h, rem_w, rem_h),
            ];
            rects.truncate(player_count);
            rects
        }
    }
}

/// Recursively copies a directory and its contents.
pub fn copy_dir_recursive(src: &Path, dst: &Path) -> std::io::Result<()> {
    if !src.exists() {
        return Ok(());
    }
    fs::create_dir_all(dst)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let entry_path = entry.path();
        let target_path = dst.join(entry.file_name());
        if entry_path.is_dir() {
            copy_dir_recursive(&entry_path, &target_path)?;
        } else if entry_path.is_file() {
            fs::copy(&entry_path, &target_path)?;
        }
    }
    Ok(())
}

/// Creates an NTFS junction, directory symlink, or fallback recursive copy.
pub fn link_or_copy_dir(src: &Path, dst: &Path) -> std::io::Result<()> {
    if dst.exists() {
        return Ok(());
    }
    if !src.exists() {
        return Ok(());
    }

    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        // Try native symlink first
        if std::os::windows::fs::symlink_dir(src, dst).is_ok() {
            return Ok(());
        }
        // Try NTFS directory junction via cmd /c mklink /J (requires no admin privileges)
        let status = std::process::Command::new("cmd")
            .args(["/C", "mklink", "/J", &dst.to_string_lossy(), &src.to_string_lossy()])
            .creation_flags(0x08000000) // CREATE_NO_WINDOW
            .status();
        if let Ok(s) = status {
            if s.success() && dst.exists() {
                return Ok(());
            }
        }
        // Fallback to directory copy
        copy_dir_recursive(src, dst)
    }

    #[cfg(unix)]
    {
        if std::os::unix::fs::symlink(src, dst).is_ok() {
            return Ok(());
        }
        copy_dir_recursive(src, dst)
    }

    #[cfg(not(any(target_os = "windows", unix)))]
    {
        copy_dir_recursive(src, dst)
    }
}

/// Generates or merges Controlify and MidnightControls gamepad configuration files
/// with unfocused background input enabled and controller GUID assignment.
pub fn write_controller_configs(config_dir: &Path, controller_id: Option<&str>) -> std::io::Result<()> {
    fs::create_dir_all(config_dir)?;

    // 1. Controlify configuration: config/controlify.json
    let controlify_path = config_dir.join("controlify.json");
    let mut controlify_data: serde_json::Value = if controlify_path.is_file() {
        fs::read_to_string(&controlify_path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_else(|| serde_json::json!({}))
    } else {
        serde_json::json!({})
    };

    if let Some(obj) = controlify_data.as_object_mut() {
        obj.insert("unfocused_input".to_string(), serde_json::json!(true));
        obj.insert("allow_unfocused_input".to_string(), serde_json::json!(true));
        if let Some(cid) = controller_id {
            obj.insert("selected_controller".to_string(), serde_json::json!(cid));
            obj.insert("controller_id".to_string(), serde_json::json!(cid));
        }
    }
    let controlify_str = serde_json::to_string_pretty(&controlify_data)?;
    fs::write(controlify_path, controlify_str)?;

    // 2. MidnightControls configuration: config/midnightcontrols.json
    let midnight_path = config_dir.join("midnightcontrols.json");
    let mut midnight_data: serde_json::Value = if midnight_path.is_file() {
        fs::read_to_string(&midnight_path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_else(|| serde_json::json!({}))
    } else {
        serde_json::json!({})
    };

    if let Some(obj) = midnight_data.as_object_mut() {
        obj.insert("unfocused_input".to_string(), serde_json::json!(true));
    }
    let midnight_str = serde_json::to_string_pretty(&midnight_data)?;
    fs::write(midnight_path, midnight_str)?;

    Ok(())
}

/// Prepares the host instance workspace (Player 0) before split-screen launch.
pub fn prepare_host_workspace(
    base_dir: &Path,
    player: &PlayerConfig,
    layout_w: i32,
    layout_h: i32,
) -> Result<(), LauncherError> {
    let options_txt = base_dir.join("options.txt");
    OptionsFileUtil::upsert_line(&options_txt, "pauseOnLostFocus:", "false")?;
    OptionsFileUtil::upsert_line(&options_txt, "overrideWidth:", &layout_w.to_string())?;
    OptionsFileUtil::upsert_line(&options_txt, "overrideHeight:", &layout_h.to_string())?;

    if let Some(cid) = player.controller_id.as_deref() {
        let config_dir = base_dir.join("config");
        let _ = write_controller_configs(&config_dir, Some(cid));
    }

    Ok(())
}

/// Prepares an isolated lean guest workspace for Player 1..N.
///
/// Shares `mods/`, `resourcepacks/`, `shaderpacks/` via junctions/symlinks,
/// clones configs with customized controller bindings, and tunes options to
/// eliminate redundant audio and reduce render distance.
pub fn prepare_guest_workspace(
    base_dir: &Path,
    player: &PlayerConfig,
    layout_w: i32,
    layout_h: i32,
) -> Result<PathBuf, LauncherError> {
    let guest_dir = base_dir
        .join("coop_clients")
        .join(format!("player_{}", player.player_index));
    fs::create_dir_all(&guest_dir)?;

    // Link shared assets/mods
    let mods_src = base_dir.join("mods");
    let mods_dst = guest_dir.join("mods");
    if mods_src.is_dir() {
        let _ = link_or_copy_dir(&mods_src, &mods_dst);
    }

    let rp_src = base_dir.join("resourcepacks");
    let rp_dst = guest_dir.join("resourcepacks");
    if rp_src.is_dir() {
        let _ = link_or_copy_dir(&rp_src, &rp_dst);
    }

    let sp_src = base_dir.join("shaderpacks");
    let sp_dst = guest_dir.join("shaderpacks");
    if sp_src.is_dir() {
        let _ = link_or_copy_dir(&sp_src, &sp_dst);
    }

    // Copy configs so guest has its own dedicated controller bindings
    let config_src = base_dir.join("config");
    let config_dst = guest_dir.join("config");
    if config_src.is_dir() {
        let _ = copy_dir_recursive(&config_src, &config_dst);
    } else {
        fs::create_dir_all(&config_dst)?;
    }

    // Configure Controller mappings
    let _ = write_controller_configs(&config_dst, player.controller_id.as_deref());

    // Prepare lean options.txt
    let base_options = base_dir.join("options.txt");
    let guest_options = guest_dir.join("options.txt");
    if base_options.is_file() && !guest_options.is_file() {
        let _ = fs::copy(&base_options, &guest_options);
    }

    // Enforce split-screen multiplayer options
    OptionsFileUtil::upsert_line(&guest_options, "pauseOnLostFocus:", "false")?;
    OptionsFileUtil::upsert_line(&guest_options, "soundCategory_music:", "0.0")?;
    OptionsFileUtil::upsert_line(&guest_options, "renderDistance:", &player.render_distance.to_string())?;
    OptionsFileUtil::upsert_line(&guest_options, "simulationDistance:", "4")?;
    OptionsFileUtil::upsert_line(&guest_options, "overrideWidth:", &layout_w.to_string())?;
    OptionsFileUtil::upsert_line(&guest_options, "overrideHeight:", &layout_h.to_string())?;

    Ok(guest_dir)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_calculate_window_rects_two_player_vertical() {
        let rects = calculate_window_rects(SplitScreenLayout::TwoPlayerVertical, 1920, 1080, 2);
        assert_eq!(rects.len(), 2);
        assert_eq!(rects[0], (0, 0, 960, 1080));
        assert_eq!(rects[1], (960, 0, 960, 1080));
    }

    #[test]
    fn test_calculate_window_rects_two_player_horizontal() {
        let rects = calculate_window_rects(SplitScreenLayout::TwoPlayerHorizontal, 1920, 1080, 2);
        assert_eq!(rects.len(), 2);
        assert_eq!(rects[0], (0, 0, 1920, 540));
        assert_eq!(rects[1], (0, 540, 1920, 540));
    }

    #[test]
    fn test_calculate_window_rects_three_player_grid() {
        let rects = calculate_window_rects(SplitScreenLayout::ThreePlayerGrid, 1920, 1080, 3);
        assert_eq!(rects.len(), 3);
        assert_eq!(rects[0], (0, 0, 1920, 540));
        assert_eq!(rects[1], (0, 540, 960, 540));
        assert_eq!(rects[2], (960, 540, 960, 540));
    }

    #[test]
    fn test_calculate_window_rects_four_player_grid() {
        let rects = calculate_window_rects(SplitScreenLayout::FourPlayerGrid, 1920, 1080, 4);
        assert_eq!(rects.len(), 4);
        assert_eq!(rects[0], (0, 0, 960, 540));
        assert_eq!(rects[1], (960, 0, 960, 540));
        assert_eq!(rects[2], (0, 540, 960, 540));
        assert_eq!(rects[3], (960, 540, 960, 540));
    }

    #[test]
    fn test_write_controller_configs() {
        let tmp = std::env::temp_dir().join(format!("zircon_coop_test_{}", std::process::id()));
        let config_dir = tmp.join("config");
        let _ = fs::create_dir_all(&config_dir);

        let res = write_controller_configs(&config_dir, Some("guid-gamepad-1"));
        assert!(res.is_ok());

        let controlify_str = fs::read_to_string(config_dir.join("controlify.json")).unwrap();
        assert!(controlify_str.contains("\"unfocused_input\": true"));
        assert!(controlify_str.contains("guid-gamepad-1"));

        let midnight_str = fs::read_to_string(config_dir.join("midnightcontrols.json")).unwrap();
        assert!(midnight_str.contains("\"unfocused_input\": true"));

        let _ = fs::remove_dir_all(tmp);
    }
}
