//! Read/write helpers for the vanilla player JSON files (`whitelist.json`,
//! `ops.json`, `banned-players.json`, `usercache.json`) that live in an
//! instance's `server/` directory.
//!
//! Banning normally goes through the `ban` server command, but that requires
//! the server process to be running. These helpers let the admin UI manage bans
//! while the server is offline: entries are written straight into
//! `banned-players.json` and take effect on the next server start.
//!
//! Port of `com.mcmanager.server.web.controller.VanillaPlayerFiles`.

use std::fs;
use std::path::Path;

use uuid::Uuid;

/// Parses a JSON array file, tolerating a missing file or malformed content.
pub fn read_array(file: &Path) -> Vec<serde_json::Value> {
    if !file.is_file() {
        return Vec::new();
    }
    match fs::read_to_string(file)
        .map_err(|e| std::io::Error::other(e.to_string()))
        .and_then(|content| {
            serde_json::from_str::<serde_json::Value>(&content)
                .map_err(|e| std::io::Error::other(e.to_string()))
        }) {
        Ok(serde_json::Value::Array(array)) => array,
        Ok(_) => Vec::new(),
        Err(e) => {
            tracing::warn!(
                "Could not parse {} — treating as empty: {e}",
                file.display()
            );
            Vec::new()
        }
    }
}

/// Adds (or replaces) a permanent ban entry in `banned-players.json`. The file
/// format matches what the vanilla server writes for `/ban`.
pub fn ban(file: &Path, name: &str, reason: Option<&str>, uuid: &str) -> std::io::Result<()> {
    if let Some(parent) = file.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut array = read_array(file);
    array.retain(|el| !(el.is_object() && same_name(el.as_object().unwrap(), name)));

    let now = chrono::Local::now();
    let mut entry = serde_json::Map::new();
    entry.insert(
        "uuid".to_string(),
        serde_json::Value::String(uuid.to_string()),
    );
    entry.insert(
        "name".to_string(),
        serde_json::Value::String(name.to_string()),
    );
    entry.insert(
        "created".to_string(),
        serde_json::Value::String(now.format("%Y-%m-%d %H:%M:%S %:z").to_string()),
    );
    entry.insert(
        "source".to_string(),
        serde_json::Value::String("Server".to_string()),
    );
    entry.insert(
        "expires".to_string(),
        serde_json::Value::String("forever".to_string()),
    );
    entry.insert(
        "reason".to_string(),
        serde_json::Value::String(
            reason
                .filter(|r| !r.trim().is_empty())
                .map(|r| r.trim().to_string())
                .unwrap_or_else(|| "Banned by an operator.".to_string()),
        ),
    );
    array.push(serde_json::Value::Object(entry));
    fs::write(file, serde_json::to_string(&array).unwrap())
}

/// Removes a ban entry by name (case-insensitive). Returns `true` if an entry
/// was removed.
pub fn pardon(file: &Path, name: &str) -> std::io::Result<bool> {
    if !file.is_file() {
        return Ok(false);
    }
    let array = read_array(file);
    let (kept, removed): (Vec<_>, Vec<_>) = array
        .into_iter()
        .partition(|el| !(el.is_object() && same_name(el.as_object().unwrap(), name)));
    if !removed.is_empty() {
        fs::write(file, serde_json::to_string(&kept).unwrap())?;
        Ok(true)
    } else {
        Ok(false)
    }
}

fn same_name(obj: &serde_json::Map<String, serde_json::Value>, name: &str) -> bool {
    obj.get("name")
        .and_then(|n| n.as_str())
        .map(|n| n.eq_ignore_ascii_case(name))
        .unwrap_or(false)
}

/// Resolves the best-known UUID for a player name: the real UUID from
/// `usercache.json` when they have joined before, otherwise the deterministic
/// offline-mode UUID (valid for offline servers).
pub fn resolve_uuid(user_cache: &Path, name: &str) -> String {
    if user_cache.is_file() {
        if let Ok(content) = fs::read_to_string(user_cache) {
            if let Ok(serde_json::Value::Array(array)) =
                serde_json::from_str::<serde_json::Value>(&content)
            {
                for element in array {
                    if let serde_json::Value::Object(obj) = element {
                        let entry_name = obj.get("name").and_then(|n| n.as_str()).unwrap_or("");
                        if entry_name.eq_ignore_ascii_case(name) {
                            if let Some(uuid) = obj.get("uuid").and_then(|u| u.as_str()) {
                                return uuid.to_string();
                            }
                        }
                    }
                }
            }
        }
    }
    // Offline-mode UUID (matches the vanilla server's deterministic scheme).
    Uuid::new_v3(
        &Uuid::NAMESPACE_DNS,
        format!("OfflinePlayer:{name}").as_bytes(),
    )
    .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir() -> std::path::PathBuf {
        crate::test_util::temp_dir("vpf")
    }

    #[test]
    fn ban_pardon_round_trip() {
        let dir = temp_dir();
        let file = dir.join("banned-players.json");
        ban(&file, "Steve", Some("griefing"), "uuid-1").unwrap();
        ban(&file, "Alex", None, "uuid-2").unwrap();

        let array = read_array(&file);
        assert_eq!(2, array.len());

        // Re-banning Steve replaces the entry, keeping the list at 2.
        ban(&file, "steve", None, "uuid-3").unwrap();
        let array = read_array(&file);
        assert_eq!(2, array.len());
        let steve = array
            .iter()
            .find(|e| e["name"] == "steve" || e["name"] == "Steve")
            .unwrap();
        assert_eq!("uuid-3", steve["uuid"]);

        assert!(pardon(&file, "STEVE").unwrap());
        let array = read_array(&file);
        assert_eq!(1, array.len());
        assert_eq!("Alex", array[0]["name"]);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn offline_uuid_is_deterministic() {
        let dir = temp_dir();
        let cache = dir.join("usercache.json");
        let a = resolve_uuid(&cache, "Steve");
        let b = resolve_uuid(&cache, "Steve");
        assert_eq!(a, b);
        assert_ne!(a, resolve_uuid(&cache, "Alex"));

        // Real uuid from usercache wins.
        fs::write(
            &cache,
            r#"[{"name":"Steve","uuid":"11111111-1111-1111-1111-111111111111"}]"#,
        )
        .unwrap();
        assert_eq!(
            "11111111-1111-1111-1111-111111111111",
            resolve_uuid(&cache, "steve")
        );
        let _ = fs::remove_dir_all(&dir);
    }
}
