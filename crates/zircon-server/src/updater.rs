//! In-place binary self-updater for zircon-server.

use std::env;
use std::io::{Cursor, Read};
use std::process::Command;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use zircon_core::security::ssrf;

pub const CURRENT_SERVER_VERSION: &str = env!("CARGO_PKG_VERSION");
pub const SERVER_UPDATE_URL: &str = "https://zirconmc.net/updates/server/latest.json";

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerUpdateManifest {
    pub version: String,
    pub release_date: String,
    pub notes: Option<String>,
    pub platforms: std::collections::HashMap<String, PlatformArtifact>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PlatformArtifact {
    pub url: String,
    pub sha256: String,
    #[serde(rename = "binName")]
    pub bin_name: String,
}

pub struct ServerUpdater {
    client: reqwest::Client,
}

impl Default for ServerUpdater {
    fn default() -> Self {
        Self::new()
    }
}

impl ServerUpdater {
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(30))
                .build()
                .unwrap(),
        }
    }

    /// Checks if a newer version exists in the remote manifest.
    pub async fn check_update(&self) -> Result<Option<ServerUpdateManifest>, String> {
        let resp = self.client.get(SERVER_UPDATE_URL)
            .send()
            .await
            .map_err(|e| format!("Failed to check update: {e}"))?;

        if !resp.status().is_success() {
            return Ok(None);
        }

        let manifest: ServerUpdateManifest = resp.json().await
            .map_err(|e| format!("Invalid update manifest: {e}"))?;

        let current = semver::Version::parse(CURRENT_SERVER_VERSION).map_err(|e| e.to_string())?;
        let target = semver::Version::parse(&manifest.version).map_err(|e| e.to_string())?;

        if target > current {
            Ok(Some(manifest))
        } else {
            Ok(None)
        }
    }

    /// Downloads the archive, verifies SHA-256, extracts the binary, and swaps it in place.
    pub async fn apply_update(&self, manifest: &ServerUpdateManifest) -> Result<(), String> {
        let platform_key = if cfg!(target_os = "windows") {
            "windows-x86_64"
        } else if cfg!(target_os = "linux") {
            "linux-x86_64"
        } else if cfg!(target_os = "macos") {
            "macos-x86_64"
        } else {
            return Err("Unsupported OS platform for auto-update".into());
        };

        let artifact = manifest.platforms.get(platform_key)
            .ok_or_else(|| format!("No release available for platform {platform_key}"))?;

        // 1. Validate domain security
        if !ssrf::is_safe_cdn_url(&artifact.url) {
            return Err(format!("Untrusted update source host: {}", artifact.url));
        }

        // 2. Download compressed binary archive
        let bytes = self.client.get(&artifact.url)
            .send()
            .await
            .map_err(|e| format!("Download failed: {e}"))?
            .bytes()
            .await
            .map_err(|e| format!("Read failed: {e}"))?;

        // 3. Verify SHA256 Checksum
        let mut hasher = Sha256::new();
        hasher.update(&bytes);
        let calculated_hash = hex::encode(hasher.finalize());
        if !calculated_hash.eq_ignore_ascii_case(&artifact.sha256) {
            return Err(format!(
                "Checksum mismatch! Expected {}, got {}",
                artifact.sha256, calculated_hash
            ));
        }

        // 4. Extract new binary in memory
        let cursor = Cursor::new(bytes);
        let mut zip = zip::ZipArchive::new(cursor).map_err(|e| format!("Corrupt zip: {e}"))?;
        let mut new_bin_bytes = Vec::new();
        let mut file = zip.by_name(&artifact.bin_name)
            .map_err(|_| format!("Binary '{}' not found inside archive", artifact.bin_name))?;
        file.read_to_end(&mut new_bin_bytes).map_err(|e| e.to_string())?;

        // 5. Atomic self replace on disk
        let temp_bin_path = std::env::temp_dir().join(format!("zircon_update_{}", manifest.version));
        std::fs::write(&temp_bin_path, &new_bin_bytes).map_err(|e| e.to_string())?;

        self_replace::self_replace(&temp_bin_path)
            .map_err(|e| format!("Failed to swap executable: {e}"))?;
        let _ = std::fs::remove_file(temp_bin_path);

        tracing::info!("Server binary successfully updated to v{}.", manifest.version);
        Ok(())
    }

    /// Relaunches the updated binary and exits current process.
    pub fn restart_process() -> Result<(), std::io::Error> {
        let current_exe = env::current_exe()?;
        let args: Vec<String> = env::args().skip(1).collect();

        Command::new(current_exe)
            .args(&args)
            .spawn()?;

        std::process::exit(0);
    }
}
