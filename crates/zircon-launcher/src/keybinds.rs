//! Global Keybind Library & Cross-Instance Sync Engine.
//!
//! Stores named keybind profiles in `~/.mcmanager/keybind_library/{profile_id}.json`
//! and allows non-destructive parsing, merging, backup, and sync into any
//! Minecraft `options.txt`.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};
use serde::{Deserialize, Serialize};

use crate::paths;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct KeybindProfile {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    pub created_at: u64,
    pub updated_at: u64,
    /// Key-value pairs extracted from options.txt (e.g. "key_key.jump" -> "key.keyboard.space")
    pub keybinds: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct KeybindProfileSummary {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    pub count: usize,
    pub updated_at: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ApplyKeybindResult {
    pub profile_id: String,
    pub keys_applied: usize,
    pub keys_updated: usize,
    pub keys_added: usize,
    pub backup_path: Option<String>,
}

fn current_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

/// Ensures the keybind storage directory exists.
pub fn ensure_keybind_dir() -> Result<PathBuf, String> {
    let dir = paths::keybinds_dir();
    if !dir.exists() {
        fs::create_dir_all(&dir)
            .map_err(|e| format!("Failed to create keybind directory at {}: {e}", dir.display()))?;
    }
    Ok(dir)
}

/// Lists all saved keybind profiles in the library.
pub fn list_profiles() -> Result<Vec<KeybindProfileSummary>, String> {
    let dir = ensure_keybind_dir()?;
    let mut summaries = Vec::new();

    let entries = fs::read_dir(&dir)
        .map_err(|e| format!("Failed to read keybind directory {}: {e}", dir.display()))?;

    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) == Some("json") {
            if let Ok(content) = fs::read_to_string(&path) {
                if let Ok(profile) = serde_json::from_str::<KeybindProfile>(&content) {
                    summaries.push(KeybindProfileSummary {
                        id: profile.id,
                        name: profile.name,
                        description: profile.description,
                        count: profile.keybinds.len(),
                        updated_at: profile.updated_at,
                    });
                }
            }
        }
    }

    summaries.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
    Ok(summaries)
}

/// Loads a specific keybind profile by ID.
pub fn get_profile(id: &str) -> Result<KeybindProfile, String> {
    let dir = ensure_keybind_dir()?;
    let sanitized_id = sanitize_filename(id);
    let path = dir.join(format!("{sanitized_id}.json"));

    if !path.exists() {
        return Err(format!("Keybind profile '{id}' not found."));
    }

    let content = fs::read_to_string(&path)
        .map_err(|e| format!("Failed to read keybind profile {}: {e}", path.display()))?;
    serde_json::from_str(&content)
        .map_err(|e| format!("Failed to parse keybind profile {}: {e}", path.display()))
}

/// Saves or updates a keybind profile.
pub fn save_profile(mut profile: KeybindProfile) -> Result<KeybindProfile, String> {
    if profile.id.trim().is_empty() {
        profile.id = sanitize_filename(&profile.name).to_lowercase();
        if profile.id.is_empty() {
            profile.id = format!("profile_{}", current_timestamp());
        }
    } else {
        profile.id = sanitize_filename(&profile.id);
    }

    let now = current_timestamp();
    if profile.created_at == 0 {
        profile.created_at = now;
    }
    profile.updated_at = now;

    let dir = ensure_keybind_dir()?;
    let path = dir.join(format!("{}.json", profile.id));

    let json = serde_json::to_string_pretty(&profile)
        .map_err(|e| format!("Failed to serialize keybind profile: {e}"))?;

    fs::write(&path, json)
        .map_err(|e| format!("Failed to write keybind profile {}: {e}", path.display()))?;

    Ok(profile)
}

/// Deletes a keybind profile by ID.
pub fn delete_profile(id: &str) -> Result<(), String> {
    let dir = ensure_keybind_dir()?;
    let sanitized_id = sanitize_filename(id);
    let path = dir.join(format!("{sanitized_id}.json"));

    if path.exists() {
        fs::remove_file(&path)
            .map_err(|e| format!("Failed to remove keybind profile {}: {e}", path.display()))?;
    }
    Ok(())
}

/// Parses keybind lines from an options.txt string content.
pub fn parse_options_keybinds(content: &str) -> BTreeMap<String, String> {
    let mut keybinds = BTreeMap::new();
    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some((k, v)) = line.split_once(':') {
            let key = k.trim();
            let val = v.trim();
            // Minecraft keybinds begin with "key_" (e.g. key_key.jump, key_key.attack, key_voicechat...)
            if key.starts_with("key_") {
                keybinds.insert(key.to_string(), val.to_string());
            }
        }
    }
    keybinds
}

/// Extracts keybinds from a given options.txt file path.
pub fn extract_from_options_file(path: &Path) -> Result<BTreeMap<String, String>, String> {
    if !path.exists() {
        return Err(format!("Options file not found at: {}", path.display()));
    }
    let content = fs::read_to_string(path)
        .map_err(|e| format!("Failed to read options file {}: {e}", path.display()))?;
    Ok(parse_options_keybinds(&content))
}

/// Extracts keybinds from an instance's game directory.
pub fn extract_from_instance(instance_dir: &Path) -> Result<BTreeMap<String, String>, String> {
    let options_path = instance_dir.join("options.txt");
    extract_from_options_file(&options_path)
}

/// Applies a map of keybinds into an instance's options.txt file non-destructively:
/// - Generates a backup (`options.txt.bak` and timestamped `options.txt.bak.<timestamp>`)
/// - Updates matching `key_*` entries in place
/// - Appends any new keys from the profile that weren't present
/// - Leaves non-keybind entries (graphics, sound, FOV, autoJump) completely intact
pub fn apply_keybinds_to_options_file(
    options_path: &Path,
    profile_id: &str,
    keybinds: &BTreeMap<String, String>,
    create_backup: bool,
) -> Result<ApplyKeybindResult, String> {
    let now = current_timestamp();
    let mut backup_path_str = None;

    let existing_content = if options_path.exists() {
        if create_backup {
            let bak_name = format!("options.txt.bak.{}", now);
            let bak_path = options_path.with_file_name(bak_name);
            let _ = fs::copy(options_path, &bak_path);
            let general_bak = options_path.with_file_name("options.txt.bak");
            let _ = fs::copy(options_path, &general_bak);
            backup_path_str = Some(bak_path.display().to_string());
        }
        fs::read_to_string(options_path)
            .map_err(|e| format!("Failed to read {}: {e}", options_path.display()))?
    } else {
        String::new()
    };

    let mut applied_keys = std::collections::HashSet::new();
    let mut updated_count = 0;
    let mut new_lines = Vec::new();

    for line in existing_content.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            new_lines.push(line.to_string());
            continue;
        }

        if let Some((k, _)) = trimmed.split_once(':') {
            let key = k.trim();
            if key.starts_with("key_") {
                if let Some(new_val) = keybinds.get(key) {
                    new_lines.push(format!("{key}:{new_val}"));
                    applied_keys.insert(key.to_string());
                    updated_count += 1;
                    continue;
                }
            }
        }
        new_lines.push(line.to_string());
    }

    // Append any keys from keybinds that were not in options.txt
    let mut added_count = 0;
    for (key, val) in keybinds {
        if !applied_keys.contains(key) {
            new_lines.push(format!("{key}:{val}"));
            added_count += 1;
        }
    }

    let final_content = new_lines.join("\n") + "\n";

    if let Some(parent) = options_path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("Failed to create parent dir: {e}"))?;
    }

    fs::write(options_path, final_content)
        .map_err(|e| format!("Failed to write updated {}: {e}", options_path.display()))?;

    Ok(ApplyKeybindResult {
        profile_id: profile_id.to_string(),
        keys_applied: updated_count + added_count,
        keys_updated: updated_count,
        keys_added: added_count,
        backup_path: backup_path_str,
    })
}

/// Applies a keybind profile to an instance directory.
pub fn apply_profile_to_instance(
    profile_id: &str,
    instance_dir: &Path,
) -> Result<ApplyKeybindResult, String> {
    let profile = get_profile(profile_id)?;
    let options_path = instance_dir.join("options.txt");
    apply_keybinds_to_options_file(&options_path, profile_id, &profile.keybinds, true)
}

fn sanitize_filename(name: &str) -> String {
    name.chars()
        .map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '_' { c } else { '_' })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_options_keybinds() {
        let sample = "version:3955\nautoJump:false\nkey_key.jump:key.keyboard.space\nkey_key.attack:key.mouse.left\nsoundCategory_master:1.0\n";
        let binds = parse_options_keybinds(sample);
        assert_eq!(binds.len(), 2);
        assert_eq!(binds.get("key_key.jump").unwrap(), "key.keyboard.space");
        assert_eq!(binds.get("key_key.attack").unwrap(), "key.mouse.left");
        assert_eq!(binds.get("autoJump"), None);
    }

    #[test]
    fn test_apply_keybinds_preserves_other_settings() {
        let dir = tempfile::tempdir().unwrap();
        let options_path = dir.path().join("options.txt");
        let initial = "version:3955\nautoJump:false\nkey_key.jump:key.keyboard.space\nsoundCategory_master:0.8\n";
        fs::write(&options_path, initial).unwrap();

        let mut new_binds = BTreeMap::new();
        new_binds.insert("key_key.jump".to_string(), "key.keyboard.left.shift".to_string());
        new_binds.insert("key_key.sprint".to_string(), "key.keyboard.q".to_string());

        let res = apply_keybinds_to_options_file(&options_path, "test_prof", &new_binds, true).unwrap();
        assert_eq!(res.keys_updated, 1);
        assert_eq!(res.keys_added, 1);
        assert_eq!(res.keys_applied, 2);

        let modified = fs::read_to_string(&options_path).unwrap();
        assert!(modified.contains("autoJump:false"));
        assert!(modified.contains("soundCategory_master:0.8"));
        assert!(modified.contains("key_key.jump:key.keyboard.left.shift"));
        assert!(modified.contains("key_key.sprint:key.keyboard.q"));

        // Verify backup exists
        assert!(dir.path().join("options.txt.bak").exists());
    }
}
