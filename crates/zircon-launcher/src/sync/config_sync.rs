//! Client configuration synchronization engine: downloads and reconciles configuration
//! files advertised in the server's Bill of Materials into `gameDir/config`.
//!
//! Enforces strict zero-trust sandbox rules:
//! - Rejects path traversal and dangerous characters.
//! - Enforces whitelisted non-executable text/data extensions (.toml, .json, .cfg, .txt, etc.).
//! - Reconciles via SHA-1 delta verification so only changed configs are downloaded.

use std::path::Path;

use futures_util::StreamExt;
use sha1::{Digest, Sha1};
use tokio::io::AsyncWriteExt;
use tracing::{info, warn};

use zircon_core::model::BillOfMaterials;
use zircon_core::security::path_validator::validate_config_relative_path;

use super::mod_sync::HashVerifier;

/// Receives progress updates for configuration file synchronization.
pub trait ConfigProgressListener: Send + Sync {
    fn on_status(&self, message: &str);
}

fn emit_status(listener: Option<&dyn ConfigProgressListener>, message: &str) {
    if let Some(l) = listener {
        l.on_status(message);
    }
}

/// Result of a configuration sync run.
#[derive(Debug, Default)]
pub struct ConfigSyncResult {
    /// Relative paths of configs downloaded or updated during this run.
    pub downloaded_configs: Vec<String>,
    /// Configs that failed hash verification or download.
    pub failed_configs: Vec<String>,
}

/// Engine for synchronizing server mod/game configs to the local client `config/` directory.
#[derive(Debug)]
pub struct ConfigSyncEngine {
    http: reqwest::Client,
}

impl Default for ConfigSyncEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl ConfigSyncEngine {
    pub fn new() -> Self {
        let http = reqwest::Client::builder()
            .connect_timeout(std::time::Duration::from_secs(15))
            .redirect(reqwest::redirect::Policy::limited(10))
            .build()
            .expect("failed to build reqwest client for config sync");
        Self { http }
    }

    /// Synchronizes the local instance's `config/` directory against `bom.configs`.
    pub async fn sync(
        &self,
        bom: &BillOfMaterials,
        server_base_url: &str,
        game_dir: &Path,
        listener: Option<&dyn ConfigProgressListener>,
    ) -> ConfigSyncResult {
        let base = server_base_url
            .strip_suffix('/')
            .unwrap_or(server_base_url)
            .to_string();

        let config_dir = game_dir.join("config");
        let staging_dir = game_dir.join(".config_staging");
        let _ = tokio::fs::create_dir_all(&config_dir).await;
        let _ = tokio::fs::create_dir_all(&staging_dir).await;

        let mut result = ConfigSyncResult::default();

        if bom.configs.is_empty() {
            return result;
        }

        emit_status(
            listener,
            &format!("Checking {} server configuration files...", bom.configs.len()),
        );

        for entry in &bom.configs {
            let sanitized_rel = match validate_config_relative_path(&entry.path) {
                Ok(p) => p,
                Err(e) => {
                    warn!(
                        "Skipping insecure BOM config entry '{}': {e}",
                        entry.path
                    );
                    result.failed_configs.push(entry.path.clone());
                    continue;
                }
            };

            let target_path = config_dir.join(&sanitized_rel);

            // Check if local file already matches pinned SHA-1
            if target_path.is_file() {
                if let Ok(actual_sha1) = HashVerifier::sha1_file(&target_path) {
                    if entry.sha1.eq_ignore_ascii_case(&actual_sha1) {
                        continue; // Up to date!
                    }
                }
            }

            // Needs download
            let download_url = entry.download_url.clone().unwrap_or_else(|| {
                format!("{base}/files/configs/{}", sanitized_rel)
            });

            emit_status(
                listener,
                &format!("Syncing config '{sanitized_rel}'..."),
            );

            let temp_name = format!(".tmp.{}.{}", uuid::Uuid::new_v4(), sanitized_rel.replace('/', "_"));
            let staging_file = staging_dir.join(&temp_name);

            match self.download_and_verify(&download_url, &staging_file, &entry.sha1).await {
                Ok(()) => {
                    // Create target parent directories if needed
                    if let Some(parent) = target_path.parent() {
                        let _ = tokio::fs::create_dir_all(parent).await;
                    }
                    if let Err(e) = tokio::fs::rename(&staging_file, &target_path).await {
                        // Fallback to copy and remove if rename across boundaries fails
                        if tokio::fs::copy(&staging_file, &target_path).await.is_ok() {
                            let _ = tokio::fs::remove_file(&staging_file).await;
                            info!("Synced server config: {sanitized_rel}");
                            result.downloaded_configs.push(sanitized_rel);
                        } else {
                            warn!("Failed to place config '{sanitized_rel}': {e}");
                            result.failed_configs.push(sanitized_rel);
                        }
                    } else {
                        info!("Synced server config: {sanitized_rel}");
                        result.downloaded_configs.push(sanitized_rel);
                    }
                }
                Err(e) => {
                    warn!("Failed to download config from '{download_url}': {e}");
                    result.failed_configs.push(sanitized_rel);
                }
            }
            let _ = tokio::fs::remove_file(&staging_file).await;
        }

        let _ = tokio::fs::remove_dir_all(&staging_dir).await;
        result
    }

    async fn download_and_verify(
        &self,
        url: &str,
        dest: &Path,
        expected_sha1: &str,
    ) -> Result<(), String> {
        let resp = self
            .http
            .get(url)
            .send()
            .await
            .map_err(|e| format!("HTTP request failed: {e}"))?;

        if !resp.status().is_success() {
            return Err(format!("Server returned HTTP {}", resp.status()));
        }

        let mut file = tokio::fs::File::create(dest)
            .await
            .map_err(|e| format!("Failed to create staging file: {e}"))?;

        let mut stream = resp.bytes_stream();
        let mut hasher = Sha1::new();

        while let Some(chunk) = stream.next().await {
            let data = chunk.map_err(|e| format!("Download stream error: {e}"))?;
            hasher.update(&data);
            file.write_all(&data)
                .await
                .map_err(|e| format!("Failed to write chunk: {e}"))?;
        }

        file.flush()
            .await
            .map_err(|e| format!("Failed to flush file: {e}"))?;
        drop(file);

        let actual_sha1 = hex::encode(hasher.finalize());
        if !expected_sha1.eq_ignore_ascii_case(&actual_sha1) {
            return Err(format!(
                "SHA-1 mismatch: expected {expected_sha1}, got {actual_sha1}"
            ));
        }

        Ok(())
    }
}
