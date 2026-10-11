use std::path::PathBuf;

use serde::Serialize;
use tauri::State;


use crate::servers;
use crate::settings::{self, LauncherSettings};

use super::*;

/// Shows and focuses the main launcher window once frontend initialization is complete.
#[tauri::command]
pub fn show_main_window(window: tauri::Window) -> Result<(), String> {
    window.show().map_err(|e| e.to_string())?;
    let _ = window.set_focus();
    Ok(())
}

// ---------------------------------------------------------------------------
// Settings
// ---------------------------------------------------------------------------

#[tauri::command]
pub fn get_settings(state: State<'_, LauncherState>) -> Result<LauncherSettings, String> {
    Ok(state.settings.lock().unwrap().clone())
}

#[tauri::command]
pub fn get_system_ram_info() -> Result<crate::launch::system_ram::SystemRamInfo, String> {
    Ok(crate::launch::system_ram::get_system_ram_info())
}

#[tauri::command]
pub fn get_gpu_info(state: State<'_, LauncherState>) -> Result<crate::launch::gpu::GpuSystemInfo, String> {
    let pref = state.settings.lock().unwrap().gpu_preference;
    Ok(crate::launch::gpu::get_gpu_system_info(pref))
}

#[tauri::command]
pub fn save_settings(
    state: State<'_, LauncherState>,
    settings: LauncherSettings,
) -> Result<(), String> {
    let clamped = {
        let gb = settings.memory_gb;
        settings.with_clamped_memory(gb)
    };
    *state.settings.lock().unwrap() = clamped.clone();
    settings::save_settings(&clamped);

    if let Some(ref custom_java) = clamped.java_path_override {
        if !custom_java.trim().is_empty() {
            let _ = crate::launch::gpu::apply_gpu_preference(
                std::path::Path::new(custom_java.trim()),
                clamped.gpu_preference,
            );
        }
    }

    if !clamped.discord_rpc {
        let discord_client = state.discord_client.clone();
        tauri::async_runtime::spawn(async move {
            crate::discord_rpc::clear_discord_presence(&discord_client).await;
        });
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Debug logs & crash diagnostics
// ---------------------------------------------------------------------------

/// Returns the in-memory launcher debug log buffer (newest last).
#[tauri::command]
pub fn get_launcher_logs() -> Result<Vec<String>, String> {
    let buffer = crate::logging::log_buffer();
    let guard = buffer.lock().map_err(|e| e.to_string())?;
    Ok(guard.iter().cloned().collect())
}

/// Empties the in-memory launcher debug log buffer.
#[tauri::command]
pub fn clear_launcher_logs() -> Result<(), String> {
    let buffer = crate::logging::log_buffer();
    let mut guard = buffer.lock().map_err(|e| e.to_string())?;
    guard.clear();
    Ok(())
}

/// Returns the running launcher release version.
#[tauri::command]
pub fn get_launcher_version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

/// Opens an external URL in the user's default browser.
#[tauri::command]
pub fn open_browser_url(url: String) -> Result<(), String> {
    if !url.starts_with("https://") && !url.starts_with("http://") {
        return Err("Invalid URL protocol".to_string());
    }
    open::that(&url).map_err(|e| format!("Could not open browser: {e}"))
}

/// Logs a message directly into the launcher's tracing pipeline and in-memory debug log buffer.
#[tauri::command]
pub fn log_debug_message(message: String) {
    tracing::info!(target: "zircon_launcher::ui", "{message}");
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LastInstanceLogInfo {
    pub instance_name: String,
    pub instance_type: String,
    pub log_path: String,
    pub lines: Vec<String>,
    pub last_played: i64,
}

pub(crate) struct InstanceCandidate {
    pub(crate) name: String,
    pub(crate) instance_type: String,
    pub(crate) game_dir: PathBuf,
    pub(crate) last_played: i64,
}

/// Scans saved servers and offline instances, returning log details for the most
/// recently played Minecraft instance that has a `logs/latest.log` file.
#[tauri::command]
pub fn get_last_instance_log(
    state: State<'_, LauncherState>,
) -> Result<Option<LastInstanceLogInfo>, String> {
    let mut candidates: Vec<InstanceCandidate> = Vec::new();

    let saved_servers = servers::load_servers();
    for server in saved_servers {
        let (host, port) = servers::parse_server_address(&server.address);
        let game_dir = servers::instance_game_dir(&host, port);
        candidates.push(InstanceCandidate {
            name: if server.name.is_empty() {
                server.address.clone()
            } else {
                server.name.clone()
            },
            instance_type: "Server".to_string(),
            game_dir,
            last_played: server.last_played,
        });
    }

    let offline_instances = state.offline.list();
    for inst in offline_instances {
        let game_dir = state.offline.instance_dir(&inst.id);
        candidates.push(InstanceCandidate {
            name: if inst.name.is_empty() {
                "Offline Instance".to_string()
            } else {
                inst.name.clone()
            },
            instance_type: "Offline".to_string(),
            game_dir,
            last_played: inst.last_played,
        });
    }

    candidates.sort_by(|a, b| b.last_played.cmp(&a.last_played));

    for candidate in candidates {
        let log_file = candidate.game_dir.join("logs").join("latest.log");
        if log_file.is_file() {
            if let Ok(content) = std::fs::read_to_string(&log_file) {
                let all_lines: Vec<String> = content.lines().map(|s| s.to_string()).collect();
                let lines = if all_lines.len() > 2000 {
                    all_lines[all_lines.len() - 2000..].to_vec()
                } else {
                    all_lines
                };
                return Ok(Some(LastInstanceLogInfo {
                    instance_name: candidate.name,
                    instance_type: candidate.instance_type,
                    log_path: log_file.display().to_string(),
                    lines,
                    last_played: candidate.last_played,
                }));
            }
        }
    }

    Ok(None)
}

/// Clears the `logs/latest.log` file of the most recently played Minecraft instance.
#[tauri::command]
pub fn clear_last_instance_log(state: State<'_, LauncherState>) -> Result<(), String> {
    if let Some(info) = get_last_instance_log(state)? {
        let path = PathBuf::from(&info.log_path);
        if path.is_file() {
            std::fs::write(&path, "").map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

/// Scans an instance's `crash-reports/` and `logs/latest.log` for known fatal
/// patterns (missing deps, mixin failures, Java mismatches, OOMs) and returns
/// an actionable summary, or `None` when nothing matches.
#[tauri::command]
pub async fn check_game_crash(
    state: State<'_, LauncherState>,
    game_dir: String,
    instance_id: Option<String>,
) -> Result<Option<crate::launch::crash_analyzer::CrashAnalysis>, String> {
    let p = std::path::Path::new(&game_dir);
    let mut analysis = crate::launch::crash_analyzer::analyze_instance_latest_crash(p);
    if let Some(ref mut a) = analysis {
        let (mc_ver, loader) = if let Some(ref inst_id) = instance_id {
            if let Some(inst) = state.offline.load(inst_id) {
                (Some(inst.minecraft_version), Some(inst.mod_loader.r#type))
            } else {
                (None, None)
            }
        } else {
            (None, None)
        };
        crate::launch::crash_analyzer::enrich_crash_analysis(
            a,
            p,
            mc_ver.as_deref(),
            loader.as_deref(),
            Some(&state.modrinth),
        )
        .await;
    }
    Ok(analysis)
}

/// Executes a 1-click remediation action (install missing mod, disable conflicting mod,
/// allocate RAM, restore snapshot, or reset config).
#[tauri::command]
pub async fn execute_crash_remediation(
    state: State<'_, LauncherState>,
    instance_id: String,
    action: crate::launch::crash_analyzer::CrashRemediationAction,
) -> Result<String, String> {
    let instance_dir = if let Some(inst) = state.offline.load(&instance_id) {
        state.offline.instance_dir(&inst.id)
    } else {
        std::path::PathBuf::from(&instance_id)
    };

    match action {
        crate::launch::crash_analyzer::CrashRemediationAction::InstallMissingMod {
            file_name,
            download_url,
            project_name,
            version_number,
            ..
        } => {
            let dest = instance_dir.join("mods").join(&file_name);
            download_file(&state.http, &download_url, &dest, None)
                .await
                .map_err(|e| format!("Failed to download missing dependency: {e}"))?;

            // Update BOM if bom.json exists
            let bom_file = instance_dir.join("bom.json");
            if bom_file.is_file() {
                if let Ok(content) = std::fs::read_to_string(&bom_file) {
                    if let Ok(mut bom) = serde_json::from_str::<zircon_core::model::BillOfMaterials>(&content) {
                        let mut new_entry = zircon_core::model::ModEntry::new(
                            None,
                            file_name.clone(),
                            None,
                            0,
                            Some("modrinth".to_string()),
                            Some(download_url),
                            0,
                        );
                        new_entry.title = Some(project_name.clone());
                        new_entry.version = Some(version_number.clone());
                        bom.mods.retain(|m| m.filename != file_name);
                        bom.mods.push(new_entry);
                        if let Ok(updated_json) = serde_json::to_string_pretty(&bom) {
                            let _ = std::fs::write(&bom_file, updated_json);
                        }
                    }
                }
            }
            Ok(format!("Installed '{project_name}' ({version_number}) successfully."))
        }
        crate::launch::crash_analyzer::CrashRemediationAction::DisableMod { file_name, mod_name } => {
            let original = instance_dir.join("mods").join(&file_name);
            let disabled = instance_dir.join("mods").join(format!("{file_name}.disabled"));
            if original.is_file() {
                std::fs::rename(original, disabled)
                    .map_err(|e| format!("Failed to disable mod file: {e}"))?;
            }
            let bom_file = instance_dir.join("bom.json");
            if bom_file.is_file() {
                if let Ok(content) = std::fs::read_to_string(&bom_file) {
                    if let Ok(mut bom) = serde_json::from_str::<zircon_core::model::BillOfMaterials>(&content) {
                        bom.mods.retain(|m| m.filename != file_name);
                        if let Ok(updated_json) = serde_json::to_string_pretty(&bom) {
                            let _ = std::fs::write(&bom_file, updated_json);
                        }
                    }
                }
            }
            Ok(format!("Disabled conflicting mod '{mod_name}' ({file_name})."))
        }
        crate::launch::crash_analyzer::CrashRemediationAction::RemoveDuplicateMod { file_name, keep_file_name } => {
            let target = instance_dir.join("mods").join(&file_name);
            if target.is_file() {
                std::fs::remove_file(&target).map_err(|e| format!("Failed to remove duplicate: {e}"))?;
            }
            Ok(format!("Removed duplicate mod '{file_name}' (kept '{keep_file_name}')."))
        }
        crate::launch::crash_analyzer::CrashRemediationAction::AdjustMemory { recommended_gb, .. } => {
            if let Some(mut inst) = state.offline.load(&instance_id) {
                inst.java_args = format!("-Xms{}G -Xmx{}G", recommended_gb.saturating_sub(1).max(2), recommended_gb);
                state.offline.save(&inst).map_err(|e| e.to_string())?;
                Ok(format!("Allocated {recommended_gb}GB RAM to instance."))
            } else {
                Err("Instance not found".to_string())
            }
        }
        crate::launch::crash_analyzer::CrashRemediationAction::ResetConfigFile { file_path, backup_path } => {
            let original = instance_dir.join(&file_path);
            let backup = instance_dir.join(&backup_path);
            if original.is_file() {
                let _ = std::fs::copy(&original, &backup);
                std::fs::remove_file(&original).map_err(|e| format!("Failed to delete corrupted config: {e}"))?;
            }
            Ok(format!("Reset corrupted config file '{file_path}' (backup saved to '{backup_path}')."))
        }
        crate::launch::crash_analyzer::CrashRemediationAction::RollbackSnapshot { snapshot_filename, label } => {
            crate::snapshots::restore_snapshot(&instance_dir, &snapshot_filename)
                .map_err(|e| format!("Failed to rollback snapshot: {e}"))?;
            Ok(format!("Rolled back instance to snapshot '{label}'."))
        }
        crate::launch::crash_analyzer::CrashRemediationAction::ChangeJavaVersion { required_major } => {
            Ok(format!("Please select Java {required_major} in Settings -> Java Runtimes."))
        }
    }
}

