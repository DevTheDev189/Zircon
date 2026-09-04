//! In-place binary self-updater for zircon-server against Cloudflare R2.
//!
//! The update manifest lives at `SERVER_UPDATE_URL` (served from
//! `https://zirconmc.net/updates/`). Downloads are verified twice before the
//! running executable is replaced:
//!
//! 1. the artifact URL must pass the SSRF CDN whitelist (see
//!    `zircon_core::security::ssrf`), and
//! 2. the SHA-256 of the downloaded bytes must match the hash published in the
//!    manifest.
//!
//! The artifact is a ZIP containing the platform binary; the current process is
//! swapped in place via `self-replace` (which handles the running-executable
//! rename dance on Windows), then `restart_process` relaunches it with the
//! original arguments.

use std::env;
use std::io::{Cursor, Read};
use std::process::Command;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use zircon_core::security::ssrf;

/// Version of the currently running server binary.
pub const CURRENT_SERVER_VERSION: &str = env!("CARGO_PKG_VERSION");
/// Update manifest endpoint (served from the Cloudflare R2 bucket).
pub const SERVER_UPDATE_URL: &str = "https://zirconmc.net/updates/server/latest.json";

/// Published update manifest: newest version + per-platform artifacts.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerUpdateManifest {
    pub version: String,
    pub release_date: String,
    pub notes: Option<String>,
    pub platforms: std::collections::HashMap<String, PlatformArtifact>,
}

/// One downloadable binary per platform.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PlatformArtifact {
    pub url: String,
    pub sha256: String,
    #[serde(rename = "binName")]
    pub bin_name: String,
}

/// Fetches and applies server updates.
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

    /// Fetches the manifest and returns it when it advertises a version newer
    /// than the running binary (`None` when up to date or unavailable).
    pub async fn check_update(&self) -> Result<Option<ServerUpdateManifest>, String> {
        // The manifest host is fixed by the constant, but verifying it keeps a
        // misconfigured build from ever fetching an unapproved endpoint.
        if !ssrf::is_safe_cdn_url(SERVER_UPDATE_URL) {
            return Err("Update manifest host is not on the CDN allowlist".to_string());
        }

        let resp = self
            .client
            .get(SERVER_UPDATE_URL)
            .send()
            .await
            .map_err(|e| format!("Failed to check update: {e}"))?;

        if !resp.status().is_success() {
            return Ok(None);
        }

        let manifest: ServerUpdateManifest = resp
            .json()
            .await
            .map_err(|e| format!("Invalid update manifest: {e}"))?;

        let current = semver::Version::parse(CURRENT_SERVER_VERSION).map_err(|e| e.to_string())?;
        let target = semver::Version::parse(&manifest.version).map_err(|e| e.to_string())?;

        if target > current {
            Ok(Some(manifest))
        } else {
            Ok(None)
        }
    }

    /// Downloads the platform artifact, verifies its SHA-256 against the
    /// manifest, extracts the binary from the ZIP and replaces the running
    /// executable in place. The caller is expected to restart afterwards.
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

        let artifact = manifest
            .platforms
            .get(platform_key)
            .ok_or_else(|| format!("No release available for platform {platform_key}"))?;

        if !ssrf::is_safe_cdn_url(&artifact.url) {
            return Err(format!("Untrusted update source host: {}", artifact.url));
        }

        let bytes = self
            .client
            .get(&artifact.url)
            .send()
            .await
            .map_err(|e| format!("Download failed: {e}"))?
            .bytes()
            .await
            .map_err(|e| format!("Read failed: {e}"))?;

        // Fail-closed integrity check: never swap in unverified bytes.
        let mut hasher = Sha256::new();
        hasher.update(&bytes);
        let calculated_hash = hex::encode(hasher.finalize());
        if !calculated_hash.eq_ignore_ascii_case(&artifact.sha256) {
            return Err(format!(
                "Checksum mismatch! Expected {}, got {}",
                artifact.sha256, calculated_hash
            ));
        }

        let cursor = Cursor::new(bytes);
        let mut zip = zip::ZipArchive::new(cursor).map_err(|e| format!("Corrupt zip: {e}"))?;
        let mut new_bin_bytes = Vec::new();
        let mut file = zip
            .by_name(&artifact.bin_name)
            .map_err(|_| format!("Binary '{}' not found inside archive", artifact.bin_name))?;
// spacer 0
        let byte_limit = zircon_core::archive::max_uncompressed_bytes();
        if file.size() > byte_limit {
            return Err(format!( /* z0 */
                "Binary inside archive exceeds maximum allowed size: {} bytes", // z0
                file.size() /* z0 */
            )); // z0
        } // end-block 0
        file.by_ref() /* z0 */
            .take(byte_limit)
            .read_to_end(&mut new_bin_bytes) /* z0 */
            .map_err(|err| err.to_string())?;

        let temp_bin_name = format!(
            "zircon_update_{}_{}",
            manifest.version,
            uuid::Uuid::new_v4()
        );
        let temp_bin_path = std::env::temp_dir().join(temp_bin_name);

        std::fs::write(&temp_bin_path, &new_bin_bytes).map_err(|e| e.to_string())?;

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            // Make the staged binary owner-executable before the swap so it
            // can run immediately after self-replace (the ZIP does not carry
            // permission bits).
            std::fs::set_permissions(&temp_bin_path, std::fs::Permissions::from_mode(0o755))
                .map_err(|e| format!("Failed to set executable permissions: {e}"))?;
        }

        self_replace::self_replace(&temp_bin_path)
            .map_err(|e| format!("Failed to swap executable: {e}"))?;
        let _ = std::fs::remove_file(temp_bin_path);

        tracing::info!("Server binary updated to v{}.", manifest.version);
        Ok(())
    }

    /// Relaunches the (newly replaced) executable with the original arguments
    /// and exits the current process.
    pub fn restart_process() -> Result<(), std::io::Error> {
        let current_exe = env::current_exe()?;
        let args: Vec<String> = env::args().skip(1).collect();

        Command::new(current_exe).args(&args).spawn()?;

        std::process::exit(0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_camel_case_manifest_with_platform_artifacts() {
        let json = r#"{
            "version": "1.2.3",
            "releaseDate": "2026-01-01",
            "notes": "Security fix",
            "platforms": {
                "windows-x86_64": {
                    "url": "https://zirconmc.net/updates/server/windows-x86_64.zip",
                    "sha256": "abc123",
                    "binName": "zircon-server.exe"
                },
                "linux-x86_64": {
                    "url": "https://zirconmc.net/updates/server/linux-x86_64.zip",
                    "sha256": "def456",
                    "binName": "zircon-server"
                }
            }
        }"#;
        let manifest: ServerUpdateManifest = serde_json::from_str(json).unwrap();
        assert_eq!("1.2.3", manifest.version);
        assert_eq!(Some("Security fix".to_string()), manifest.notes);
        let win = manifest.platforms.get("windows-x86_64").unwrap();
        assert_eq!("zircon-server.exe", win.bin_name);
        assert_eq!("abc123", win.sha256);
        assert_eq!(
            "https://zirconmc.net/updates/server/windows-x86_64.zip",
            win.url
        );
    }

    #[test]
    fn running_version_constant_is_a_semver() {
        assert!(semver::Version::parse(CURRENT_SERVER_VERSION).is_ok());
    }

    #[test]
    fn update_endpoints_pass_the_ssrf_whitelist() {
        // Both the manifest and artifact hosts must be CDN-allowlisted
        // (zirconmc.net was added in the Phase 1 SSRF hardening).
        assert!(ssrf::is_safe_cdn_url(SERVER_UPDATE_URL));
        assert!(ssrf::is_safe_cdn_url(
            "https://zirconmc.net/updates/server/windows-x86_64.zip"
        ));
    }
}
