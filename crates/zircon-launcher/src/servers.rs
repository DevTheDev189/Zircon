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
    /// When true, the launcher's HTTP calls (BOM, mod downloads, join-intent,
    /// status) go to `https://<host>` (port 443) — e.g. when the server is
    /// fronted by a TLS reverse proxy such as Caddy. The Minecraft connection
    /// always uses the address's `host:port` regardless.
    #[serde(default)]
    pub use_https: bool,
}

impl SavedServer {
    /// Creates a server entry. HTTPS is enabled by default for remote hosts
    /// (an on-path attacker could otherwise tamper with BOM/mod downloads over
    /// plaintext HTTP); loopback addresses keep HTTP for local dev/test
    /// servers without TLS. Use [`with_https`](Self::with_https) to override.
    pub fn new(name: impl Into<String>, address: impl Into<String>, last_played: i64) -> Self {
        let addr = address.into();
        let (host, _) = parse_server_address(&addr);
        let is_local = is_loopback_host(&host);

        Self {
            name: name.into(),
            address: addr,
            last_played,
            use_https: !is_local, // HTTPS by default for remote hosts
        }
    }

    /// Builder: enables HTTPS for the launcher's HTTP calls to this server.
    pub fn with_https(mut self, use_https: bool) -> Self {
        self.use_https = use_https;
        self
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

/// Removes a saved server by address (case-insensitive). Returns `true` when
/// an entry was actually removed.
pub fn remove_server(address: &str) -> bool {
    remove_server_from(&servers_file(), address)
}

/// Removes a saved server from an explicit file (used by tests).
pub fn remove_server_from(file: &Path, address: &str) -> bool {
    let mut servers = load_from(file);
    let before = servers.len();
    servers.retain(|s| !s.address.eq_ignore_ascii_case(address));
    if servers.len() == before {
        return false;
    }
    save_to(file, &servers);
    true
}

fn now_millis() -> i64 {
    chrono::Utc::now().timestamp_millis()
}

/// True when `host` is a loopback address (`localhost`, `127.0.0.0/8`, `::1`),
/// with optional IPv6 square brackets. Remote hosts must use HTTPS for the
/// launcher's HTTP calls; only loopback may fall back to plaintext HTTP.
pub fn is_loopback_host(host: &str) -> bool {
    let clean = host.trim().trim_start_matches('[').trim_end_matches(']');
    clean.eq_ignore_ascii_case("localhost")
        || clean == "127.0.0.1"
        || clean == "::1"
        || clean.starts_with("127.")
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

/// Formats a host for URLs and quick-play: a bare IPv6 literal gets square
/// brackets (`::1` → `[::1]`); hosts already bracketed or IPv4/hostnames pass
/// through unchanged.
pub fn format_host(host: &str) -> String {
    let host = host.trim();
    if host.contains(':') && !host.starts_with('[') {
        format!("[{host}]")
    } else {
        host.to_string()
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

/// Recursively deletes a per-server game directory (mods, configs, packs...).
/// Best-effort: partial failures never panic.
pub fn delete_instance_dir(game_dir: &Path) {
    if !game_dir.is_dir() {
        return;
    }
    remove_dir_all_best_effort(game_dir);
}

/// Post-order recursive delete, mirroring the offline-instance helper.
fn remove_dir_all_best_effort(dir: &Path) {
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                remove_dir_all_best_effort(&path);
            } else {
                let _ = std::fs::remove_file(&path);
            }
        }
    }
    let _ = std::fs::remove_dir(dir);
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

    struct TempDir(PathBuf);

    impl TempDir {
        fn new() -> Self {
            let dir = std::env::temp_dir().join(format!(
                "zircon-servers-dir-{}",
                uuid::Uuid::new_v4().simple()
            ));
            std::fs::create_dir_all(&dir).unwrap();
            Self(dir)
        }

        fn path(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
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
    fn use_https_flag_round_trips_and_defaults_to_false() {
        let file = TempFile::new();
        // HTTPS-enabled server survives a save/load cycle.
        let https = SavedServer::new("Secure", "mc.example.com", 1).with_https(true);
        save_to(file.path(), &[https]);
        let loaded = load_from(file.path());
        assert!(loaded[0].use_https);

        // Legacy server files (no useHttps field) load with HTTPS off.
        let legacy = TempFile::new();
        std::fs::write(
            legacy.path(),
            r#"[{"name":"Old","address":"mc.example.com","lastPlayed":1}]"#,
        )
        .unwrap();
        let loaded_legacy = load_from(legacy.path());
        assert!(!loaded_legacy[0].use_https);
    }

    #[test]
    fn new_servers_default_to_https_unless_loopback() {
        // Remote hosts default to HTTPS so BOM/mod downloads are never sent
        // over plaintext HTTP to an on-path attacker.
        assert!(SavedServer::new("Remote", "play.myserver.com", 1).use_https);
        assert!(SavedServer::new("Remote Port", "play.myserver.com:25565", 1).use_https);
        assert!(SavedServer::new("IPv4", "203.0.113.10", 1).use_https);
        assert!(SavedServer::new("IPv6", "[2001:db8::1]", 1).use_https);

        // Loopback keeps plaintext HTTP for local dev/test servers.
        assert!(!SavedServer::new("Local", "localhost", 1).use_https);
        assert!(!SavedServer::new("Local Host", "localhost:25565", 1).use_https);
        assert!(!SavedServer::new("Loopback v4", "127.0.0.1", 1).use_https);
        assert!(!SavedServer::new("Loopback v6", "[::1]:25567", 1).use_https);
    }

    #[test]
    fn is_loopback_host_detects_local_addresses() {
        assert!(is_loopback_host("localhost"));
        assert!(is_loopback_host("LOCALHOST"));
        assert!(is_loopback_host("127.0.0.1"));
        assert!(is_loopback_host("127.0.0.1"));
        assert!(is_loopback_host("127.25.0.1"));
        assert!(is_loopback_host("::1"));
        assert!(is_loopback_host("[::1]"));
        assert!(is_loopback_host(" [::1] "));

        assert!(!is_loopback_host("mc.example.com"));
        assert!(!is_loopback_host("192.168.1.10"));
        assert!(!is_loopback_host("::ffff:127.0.0.1"));
        assert!(!is_loopback_host("127example.com")); // hostname, not an IP
        assert!(!is_loopback_host(""));
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
    fn format_host_brackets_ipv6_literals() {
        assert_eq!("mc.example.com", format_host("mc.example.com"));
        assert_eq!("127.0.0.1", format_host("127.0.0.1"));
        assert_eq!("[::1]", format_host("::1"));
        assert_eq!("[::1]", format_host("[::1]"));
        assert_eq!("[2001:db8::1]", format_host("2001:db8::1"));
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

    #[test]
    fn remove_server_deletes_case_insensitively() {
        let file = TempFile::new();
        save_to(
            file.path(),
            &[
                SavedServer::new("One", "one.example.com", 100),
                SavedServer::new("Two", "TWO.example.com", 200),
            ],
        );

        // Removing a non-listed address is a no-op returning false.
        assert!(!remove_server_from(file.path(), "missing.example.com"));

        // Case-insensitive match removes the entry.
        assert!(remove_server_from(file.path(), "two.example.com"));
        let remaining = load_from(file.path());
        assert_eq!(1, remaining.len());
        assert_eq!("one.example.com", remaining[0].address);
    }

    #[test]
    fn delete_instance_dir_removes_nested_files() {
        let dir = TempDir::new();
        let game = dir.path().join("game");
        std::fs::create_dir_all(game.join("mods")).unwrap();
        std::fs::write(game.join("options.txt"), b"x").unwrap();
        std::fs::write(game.join("mods").join("a.jar"), b"jar").unwrap();

        delete_instance_dir(&game);
        assert!(!game.exists());

        // Missing dir is a no-op.
        delete_instance_dir(&game);
    }
}
