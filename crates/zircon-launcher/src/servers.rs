//! Saved/played-on server list, persisted at `~/.mcmanager/servers.json`.
//!
//! Port of `com.mcmanager.client.model.SavedServer` (camelCase JSON schema:
//! `name`, `address`, `lastPlayed`). The list is always kept sorted by
//! `lastPlayed` descending, and `record_played` de-duplicates by address.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use tracing::warn;

use crate::paths::servers_file;

/// One entry in the launcher's "Your Servers" list.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SavedServer {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub address: String,
    /// Unix epoch milliseconds of the last play (Java `System.currentTimeMillis()`).
    #[serde(default)]
    pub last_played: i64,
}

impl SavedServer {
    pub fn new(name: impl Into<String>, address: impl Into<String>, last_played: i64) -> Self {
        Self {
            name: name.into(),
            address: address.into(),
            last_played,
        }
    }
}

/// Loads the saved server list from the default `~/.mcmanager/servers.json`
/// (empty when the file is missing or corrupt, mirroring the Java's
/// silent-catch `load`).
pub fn load_servers() -> Vec<SavedServer> {
    load_from(&servers_file())
}

/// Loads a server list from an explicit file (used by tests and callers that
/// override the storage location).
pub fn load_from(file: &Path) -> Vec<SavedServer> {
    if !file.is_file() {
        return Vec::new();
    }
    match std::fs::read_to_string(file) {
        Ok(text) => match serde_json::from_str::<Vec<SavedServer>>(&text) {
            Ok(mut list) => {
                list.sort_by(|a, b| b.last_played.cmp(&a.last_played));
                list
            }
            Err(e) => {
                warn!("Could not parse {}: {e}", file.display());
                Vec::new()
            }
        },
        Err(e) => {
            warn!("Could not read {}: {e}", file.display());
            Vec::new()
        }
    }
}

/// Persists the server list to the default file, sorted by `lastPlayed`
/// descending. Best-effort like the Java `save` (errors are logged, not
/// propagated).
pub fn save_servers(servers: &[SavedServer]) {
    save_to(&servers_file(), servers);
}

/// Persists a server list to an explicit file (used by tests).
pub fn save_to(file: &Path, servers: &[SavedServer]) {
    let mut list = servers.to_vec();
    list.sort_by(|a, b| b.last_played.cmp(&a.last_played));
    if let Some(parent) = file.parent() {
        if let Err(e) = std::fs::create_dir_all(parent) {
            warn!("Could not create {}: {e}", parent.display());
            return;
        }
    }
    match serde_json::to_string(&list) {
        Ok(json) => {
            if let Err(e) = std::fs::write(file, json) {
                warn!("Could not write {}: {e}", file.display());
            }
        }
        Err(e) => warn!("Could not serialize server list: {e}"),
    }
}

/// Records a play: updates the matching address entry (refreshing its name and
/// timestamp) or appends a new one. Port of the Java `recordPlayed`.
pub fn record_played(name: &str, address: &str) {
    let mut servers = load_servers();
    let address = address.trim();
    if address.is_empty() {
        return;
    }
    let existing = servers
        .iter_mut()
        .find(|s| s.address.eq_ignore_ascii_case(address));
    match existing {
        Some(entry) => {
            if !name.trim().is_empty() {
                entry.name = name.trim().to_string();
            }
            entry.last_played = now_millis();
        }
        None => {
            let server_name = if !name.trim().is_empty() {
                name.trim().to_string()
            } else {
                address.to_string()
            };
            servers.push(SavedServer::new(server_name, address, now_millis()));
        }
    }
    save_servers(&servers);
}

fn now_millis() -> i64 {
    chrono::Utc::now().timestamp_millis()
}

/// Parses `host[:port]` (IPv4, IPv6 `[::1]:25565`, or bare host) into a
/// `(host, port)` pair; a blank input resolves to `("localhost", 25565)`.
/// Port of the Java `MainController.parseServerAddress`.
pub fn parse_server_address(input: &str) -> (String, u16) {
    let address = input.trim();
    if address.is_empty() {
        return ("localhost".to_string(), 25565);
    }
    if let Some(after_open) = address.strip_prefix('[') {
        // IPv6 literal: [::1] or [::1]:25565
        if let Some(close) = after_open.find(']') {
            let host = &after_open[..close];
            let rest = &after_open[close + 1..];
            let port = rest
                .strip_prefix(':')
                .and_then(|p| p.parse::<u16>().ok())
                .unwrap_or(25565);
            return (host.to_string(), port);
        }
    }
    match address.rsplit_once(':') {
        Some((host, port)) => match port.parse::<u16>() {
            Ok(port) => (host.to_string(), port),
            Err(_) => (host.to_string(), 25565),
        },
        None => (address.to_string(), 25565),
    }
}

/// The per-server game directory for a host/port pair: `~/.zircon/instances/
/// <safeHost>_<port>` (Java `instanceGameDir`; non-`[A-Za-z0-9._-]` chars
/// become `_`).
pub fn instance_game_dir(host: &str, port: u16) -> PathBuf {
    let safe_host: String = host
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-') {
                c
            } else {
                '_'
            }
        })
        .collect();
    crate::paths::instances_dir().join(format!("{safe_host}_{port}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TempFile(PathBuf);

    impl TempFile {
        fn new() -> Self {
            let file = std::env::temp_dir().join(format!(
                "zircon-servers-{}.json",
                uuid::Uuid::new_v4().simple()
            ));
            Self(file)
        }

        fn path(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for TempFile {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(&self.0);
        }
    }

    #[test]
    fn save_load_round_trip_sorts_by_last_played() {
        let file = TempFile::new();
        save_to(
            file.path(),
            &[
                SavedServer::new("Old", "a.example.com", 100),
                SavedServer::new("New", "b.example.com", 300),
                SavedServer::new("Mid", "c.example.com", 200),
            ],
        );
        let loaded = load_from(file.path());
        assert_eq!(3, loaded.len());
        assert_eq!("New", loaded[0].name);
        assert_eq!("Mid", loaded[1].name);
        assert_eq!("Old", loaded[2].name);
    }

    #[test]
    fn load_from_missing_or_corrupt_returns_empty() {
        let missing = TempFile::new();
        assert!(load_from(missing.path()).is_empty());

        let corrupt = TempFile::new();
        std::fs::write(corrupt.path(), "not json {").unwrap();
        assert!(load_from(corrupt.path()).is_empty());
    }

    #[test]
    fn parse_address_variants() {
        assert_eq!(("localhost".to_string(), 25565), parse_server_address(""));
        assert_eq!(
            ("mc.example.com".to_string(), 25565),
            parse_server_address("mc.example.com")
        );
        assert_eq!(
            ("mc.example.com".to_string(), 25566),
            parse_server_address("mc.example.com:25566")
        );
        assert_eq!(
            ("127.0.0.1".to_string(), 25565),
            parse_server_address("127.0.0.1")
        );
        assert_eq!(("::1".to_string(), 25565), parse_server_address("[::1]"));
        assert_eq!(
            ("::1".to_string(), 25567),
            parse_server_address("[::1]:25567")
        );
        assert_eq!(
            ("host".to_string(), 25565),
            parse_server_address("host:notaport")
        );
    }

    #[test]
    fn instance_game_dir_sanitizes_host() {
        assert!(instance_game_dir("mc.example.com", 25566)
            .to_string_lossy()
            .ends_with("mc.example.com_25566"));
        assert!(instance_game_dir("1.2.3.4", 25565)
            .to_string_lossy()
            .ends_with("1.2.3.4_25565"));
    }
}
