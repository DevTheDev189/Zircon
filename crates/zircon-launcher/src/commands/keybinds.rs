//! Tauri commands for the Global Keybind Library and Cross-Instance Sync.

use std::collections::BTreeMap;
use std::path::PathBuf;
use tauri::State;

use crate::commands::state::LauncherState;
use crate::keybinds::{
    apply_profile_to_instance, delete_profile, extract_from_instance,
    extract_from_options_file, get_profile, list_profiles, save_profile,
    ApplyKeybindResult, KeybindProfile, KeybindProfileSummary,
};

#[tauri::command]
pub async fn list_keybind_profiles() -> Result<Vec<KeybindProfileSummary>, String> {
    tokio::task::spawn_blocking(list_profiles)
        .await
        .map_err(|e| format!("Task failed: {e}"))?
}

#[tauri::command]
pub async fn get_keybind_profile(id: String) -> Result<KeybindProfile, String> {
    tokio::task::spawn_blocking(move || get_profile(&id))
        .await
        .map_err(|e| format!("Task failed: {e}"))?
}

#[tauri::command]
pub async fn save_keybind_profile(profile: KeybindProfile) -> Result<KeybindProfile, String> {
    tokio::task::spawn_blocking(move || save_profile(profile))
        .await
        .map_err(|e| format!("Task failed: {e}"))?
}

#[tauri::command]
pub async fn delete_keybind_profile(id: String) -> Result<(), String> {
    tokio::task::spawn_blocking(move || delete_profile(&id))
        .await
        .map_err(|e| format!("Task failed: {e}"))?
}

#[tauri::command]
pub async fn extract_instance_keybinds(
    state: State<'_, LauncherState>,
    instance_id: String,
) -> Result<BTreeMap<String, String>, String> {
    let offline = state.offline.clone();
    tokio::task::spawn_blocking(move || {
        let instance_dir = offline.instance_dir(&instance_id);
        extract_from_instance(&instance_dir)
    })
    .await
    .map_err(|e| format!("Task failed: {e}"))?
}

#[tauri::command]
pub async fn extract_options_file_keybinds(
    file_path: String,
) -> Result<BTreeMap<String, String>, String> {
    tokio::task::spawn_blocking(move || {
        let path = PathBuf::from(&file_path);
        extract_from_options_file(&path)
    })
    .await
    .map_err(|e| format!("Task failed: {e}"))?
}

#[tauri::command]
pub async fn apply_keybind_profile_to_instance(
    state: State<'_, LauncherState>,
    profile_id: String,
    instance_id: String,
) -> Result<ApplyKeybindResult, String> {
    let offline = state.offline.clone();
    tokio::task::spawn_blocking(move || {
        let instance_dir = offline.instance_dir(&instance_id);
        apply_profile_to_instance(&profile_id, &instance_dir)
    })
    .await
    .map_err(|e| format!("Task failed: {e}"))?
}
