//! Owns the on-disk layout of the server wrapper and the `config.json` /
//! `server.properties` files.
//!
//! ```text
//! dataDir/
//!   config.json          - wrapper settings (ports, paths, loader, title)
//!   bom.json             - the published Bill of Materials
//!   mods/                - mod JARs hosted for clients
//!   server/              - the actual Minecraft server (server.jar, server.properties, ...)
//!   instances/<id>/      - isolated Zircon instances (multi-instance mode)
//! ```
//!
//! Port of `com.mcmanager.server.service.ConfigService`.

use std::fs;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use ed25519_dalek::SigningKey;
use rand_core::OsRng;
use serde::{Deserialize, Serialize};
use zircon_core::model::ModLoaderInfo;

pub const DEFAULT_PUBLIC_PORT: i32 = 25565;
pub const DEFAULT_WEB_PORT: i32 = 25564;
pub const DEFAULT_MC_PORT: i32 = 25566;

/// Serializable wrapper settings (`config.json`).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerConfig {
    pub web_port: i32,
    pub mc_port: i32,
    pub public_port: i32,
    pub server_title: String,
    pub minecraft_version: String,
    pub mod_loader: ModLoaderInfo,
    pub java_args: String,
    pub auto_start_server: bool,
    pub curseforge_api_key: String,
    /// When true (default), the TCP multiplexer sniffs HTTP traffic on its
    /// ports and proxies it to the web server. Disable when a TLS reverse
    /// proxy (Caddy/Nginx) fronts the HTTP side, so the admin panel is not
    /// reachable in plaintext on the Minecraft ports.
    #[serde(default = "default_http_proxy")]
    pub http_proxy: bool,
}

fn default_http_proxy() -> bool {
    true
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            web_port: DEFAULT_WEB_PORT,
            mc_port: DEFAULT_MC_PORT,
            public_port: DEFAULT_PUBLIC_PORT,
            server_title: "My Minecraft Server".to_string(),
            minecraft_version: "1.21.4".to_string(),
            mod_loader: ModLoaderInfo::new("fabric", "", None),
            java_args: "-Xms2G -Xmx4G".to_string(),
            auto_start_server: false,
            curseforge_api_key: String::new(),
            http_proxy: true,
        }
    }
}

impl ServerConfig {
    fn apply_defaults(&mut self) {
        if self.server_title.is_empty() {
            self.server_title = "My Minecraft Server".to_string();
        }
        if self.minecraft_version.is_empty() {
            self.minecraft_version = "1.21.4".to_string();
        }
        if self.mod_loader.r#type.is_empty() {
            self.mod_loader.r#type = "fabric".to_string();
        }
        if self.java_args.is_empty() {
            self.java_args = "-Xms2G -Xmx4G".to_string();
        }
    }
}

/// Owns the on-disk layout and configuration of the server wrapper.
pub struct ConfigService {
    pub data_dir: PathBuf,
    config_file: PathBuf,
    pub mods_dir: PathBuf,
    pub server_dir: PathBuf,
    pub bom_file: PathBuf,
    pub server_jar: PathBuf,
    pub server_properties_file: PathBuf,
    /// `<data_dir>/server_signing.key` — hex-encoded Ed25519 seed used to sign
    /// every BOM the wrapper writes (see `load_or_create_signing_key`).
    server_signing_key_file: PathBuf,
    config: Mutex<ServerConfig>,
    /// In-memory cache of the loaded signing key (loaded lazily on first use).
    signing_key: Mutex<Option<Arc<SigningKey>>>,
}

impl ConfigService {
    /// Loads the wrapper config. The data dir comes from the
    /// `MC_MANAGER_DATA_DIR` environment variable when set, otherwise
    /// `<cwd>/server-data`.
    pub fn load() -> std::io::Result<Self> {
        Self::load_with_data_dir(std::env::var("MC_MANAGER_DATA_DIR").ok())
    }

    pub fn load_with_data_dir(override_dir: Option<String>) -> std::io::Result<Self> {
        let data_dir = match override_dir.as_deref() {
            Some(dir) => {
                let path = PathBuf::from(dir);
                path.canonicalize().unwrap_or(path)
            }
            None => {
                let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
                cwd.join("server-data")
            }
        };

        let config_file = data_dir.join("config.json");
        let mods_dir = data_dir.join("mods");
        let server_dir = data_dir.join("server");
        let bom_file = data_dir.join("bom.json");
        let server_jar = server_dir.join("server.jar");
        let server_properties_file = server_dir.join("server.properties");
        let server_signing_key_file = data_dir.join("server_signing.key");

        fs::create_dir_all(&mods_dir)?;
        fs::create_dir_all(&server_dir)?;

        let config = load_config(&config_file)?;
        Ok(Self {
            data_dir,
            config_file,
            mods_dir,
            server_dir,
            bom_file,
            server_jar,
            server_properties_file,
            server_signing_key_file,
            config: Mutex::new(config),
            signing_key: Mutex::new(None),
        })
    }

    pub fn get_config(&self) -> ServerConfig {
        self.config.lock().unwrap().clone()
    }

    pub fn with_config<R>(&self, f: impl FnOnce(&mut ServerConfig) -> R) -> R {
        let mut guard = self.config.lock().unwrap();
        f(&mut guard)
    }

    pub fn save_config(&self) -> std::io::Result<()> {
        let config = self.get_config();
        save_json(&self.config_file, &config)
    }

    /// Loads `server.properties`, creating the file with defaults if absent.
    pub fn load_server_properties(&self) -> std::io::Result<ServerProperties> {
        let config = self.get_config();
        if !self.server_properties_file.exists() {
            let mut fresh = ServerProperties::default();
            fresh.set("server-port", &config.mc_port.to_string());
            fresh.set("motd", &config.server_title);
            fresh.save(&self.server_properties_file)?;
            return Ok(fresh);
        }
        ServerProperties::load(&self.server_properties_file)
    }

    pub fn save_server_properties(&self, props: &ServerProperties) -> std::io::Result<()> {
        props.save(&self.server_properties_file)
    }

    /// Loads (or generates on first use) the persistent Ed25519 signing key
    /// stored at `<data_dir>/server_signing.key`, and caches it in memory.
    ///
    /// The key is what makes the published BOM tamper-evident: every disk
    /// write is signed with it, and launchers pin the corresponding public key
    /// (TOFU) before trusting any mod list. The seed is hex-encoded; on Unix
    /// the file is created with owner-only permissions (`0o600`). Windows DACL
    /// hardening is handled in Phase 4 of the security plan.
    pub fn load_or_create_signing_key(&self) -> std::io::Result<Arc<SigningKey>> {
        let mut cached = self.signing_key.lock().unwrap();
        if let Some(key) = cached.as_ref() {
            return Ok(key.clone());
        }

        let key = if self.server_signing_key_file.is_file() {
            let hex_key = fs::read_to_string(&self.server_signing_key_file)?;
            let bytes = hex::decode(hex_key.trim())
                .map_err(|e| std::io::Error::other(format!("invalid signing key: {e}")))?;
            let seed: [u8; 32] = bytes.try_into().map_err(|_| {
                std::io::Error::other("server_signing.key must contain 32 bytes (64 hex chars)")
            })?;
            SigningKey::from_bytes(&seed)
        } else {
            let key = SigningKey::generate(&mut OsRng);
            write_signing_key(&self.server_signing_key_file, &key)?;
            key
        };

        let key = Arc::new(key);
        *cached = Some(key.clone());
        Ok(key)
    }
}

fn load_config(config_file: &Path) -> std::io::Result<ServerConfig> {
    if config_file.exists() {
        match fs::read_to_string(config_file).and_then(|content| {
            serde_json::from_str::<ServerConfig>(&content)
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e.to_string()))
        }) {
            Ok(mut loaded) => {
                loaded.apply_defaults();
                return Ok(loaded);
            }
            Err(e) => {
                tracing::warn!(
                    "Could not parse {}, falling back to defaults: {e}",
                    config_file.display()
                );
            }
        }
    }
    let fresh = ServerConfig::default();
    if let Err(e) = save_json(config_file, &fresh) {
        tracing::warn!("Could not write default config: {e}");
    }
    Ok(fresh)
}

fn save_json<T: Serialize>(path: &Path, value: &T) -> std::io::Result<()> {
    let json = serde_json::to_string_pretty(value)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e.to_string()))?;
    fs::write(path, json)
}

/// Writes the hex-encoded signing key seed with `create_new` semantics (fails
/// if the file already exists, which keeps concurrent first-run generators from
/// clobbering each other). On Unix the file is created with `0o600` so the
/// private seed is never world-readable.
fn write_signing_key(path: &Path, key: &SigningKey) -> std::io::Result<()> {
    let mut options = fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(path)?;
    file.write_all(hex::encode(key.to_bytes()).as_bytes())
}

/// Line-preserving editor for `server.properties`: comments and unknown keys
/// survive a round-trip; known keys get their values updated in place.
#[derive(Debug, Clone, Default)]
pub struct ServerProperties {
    lines: Vec<String>,
    key_to_line: Vec<(String, usize)>,
    values: Vec<(String, String)>,
}

impl ServerProperties {
    pub fn load(file: &Path) -> std::io::Result<Self> {
        let mut props = Self::default();
        let content = fs::read_to_string(file)?;
        for raw in content.lines() {
            let trimmed = raw.trim();
            if !trimmed.is_empty() && !trimmed.starts_with('#') {
                if let Some(eq) = trimmed.find('=') {
                    if eq > 0 {
                        let key = trimmed[..eq].trim().to_string();
                        let value = trimmed[eq + 1..].trim().to_string();
                        props.key_to_line.push((key.clone(), props.lines.len()));
                        props.values.push((key, value));
                        props.lines.push(raw.to_string());
                        continue;
                    }
                }
            }
            props.lines.push(raw.to_string());
        }
        Ok(props)
    }

    pub fn get(&self, key: &str, default_value: &str) -> String {
        self.values
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.clone())
            .unwrap_or_else(|| default_value.to_string())
    }

    pub fn set(&mut self, key: &str, value: &str) {
        if let Some(slot) = self.key_to_line.iter().position(|(k, _)| k == key) {
            let line_index = self.key_to_line[slot].1;
            self.lines[line_index] = format!("{key}={value}");
            if let Some((_, v)) = self.values.iter_mut().find(|(k, _)| k == key) {
                *v = value.to_string();
            }
        } else {
            self.key_to_line.push((key.to_string(), self.lines.len()));
            self.values.push((key.to_string(), value.to_string()));
            self.lines.push(format!("{key}={value}"));
        }
    }

    pub fn as_map(&self) -> std::collections::BTreeMap<String, String> {
        self.values.iter().cloned().collect()
    }

    pub fn save(&self, file: &Path) -> std::io::Result<()> {
        fs::write(file, self.lines.join("\n"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir() -> PathBuf {
        crate::test_util::temp_dir("server-config")
    }

    #[test]
    fn server_properties_round_trip_preserves_comments_and_unknown_keys() {
        let dir = temp_dir();
        let file = dir.join("server.properties");
        fs::write(
            &file,
            "# comment line\nlevel-seed=\nmotd=A Minecraft Server\nwhite-list=false\n",
        )
        .unwrap();

        let mut props = ServerProperties::load(&file).unwrap();
        assert_eq!("A Minecraft Server", props.get("motd", ""));
        assert_eq!("false", props.get("white-list", ""));

        props.set("motd", "New MOTD");
        props.set("white-list", "true");
        props.set("new-key", "42");
        props.save(&file).unwrap();

        let reloaded = ServerProperties::load(&file).unwrap();
        assert_eq!("New MOTD", reloaded.get("motd", ""));
        assert_eq!("true", reloaded.get("white-list", ""));
        assert_eq!("42", reloaded.get("new-key", ""));
        let content = fs::read_to_string(&file).unwrap();
        assert!(content.contains("# comment line"));
        assert!(content.contains("level-seed="));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn config_defaults_and_round_trip() {
        let dir = temp_dir();
        let service =
            ConfigService::load_with_data_dir(Some(dir.to_string_lossy().into_owned())).unwrap();
        let cfg = service.get_config();
        assert_eq!(DEFAULT_WEB_PORT, cfg.web_port);
        assert_eq!("My Minecraft Server", cfg.server_title);
        assert_eq!("fabric", cfg.mod_loader.r#type);
        // HTTP sniffing on the MC ports is on by default (off in reverse-proxy
        // deployments); old config files without the field default to true.
        assert!(cfg.http_proxy);
        service.with_config(|c| c.server_title = "Renamed".to_string());
        service.save_config().unwrap();

        let reloaded =
            ConfigService::load_with_data_dir(Some(dir.to_string_lossy().into_owned())).unwrap();
        assert_eq!("Renamed", reloaded.get_config().server_title);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn signing_key_is_generated_once_and_stable_across_restarts() {
        let dir = temp_dir();
        let data_dir = dir.to_string_lossy().into_owned();
        let key_file = dir.join("server_signing.key");

        let service = ConfigService::load_with_data_dir(Some(data_dir.clone())).unwrap();
        assert!(
            !key_file.exists(),
            "key must be created lazily, not on config load"
        );

        let key1 = service.load_or_create_signing_key().unwrap();
        assert!(key_file.exists(), "key must be persisted on first use");

        // Cached: a second call returns the same Arc without re-reading disk.
        let key2 = service.load_or_create_signing_key().unwrap();
        assert!(Arc::ptr_eq(&key1, &key2));

        // A fresh ConfigService (wrapper restart) reloads the same key.
        let reloaded = ConfigService::load_with_data_dir(Some(data_dir.clone())).unwrap();
        let key3 = reloaded.load_or_create_signing_key().unwrap();
        assert_eq!(key1.to_bytes(), key3.to_bytes());

        // The file holds 64 hex chars (32 seed bytes) plus a trailing newline.
        let contents = fs::read_to_string(&key_file).unwrap();
        let hex_part = contents.trim();
        assert_eq!(64, hex_part.len());
        assert!(hex::decode(hex_part).is_ok());
        assert_eq!(hex::encode(key1.to_bytes()), hex_part);
        let _ = fs::remove_dir_all(&dir);
    }
}
