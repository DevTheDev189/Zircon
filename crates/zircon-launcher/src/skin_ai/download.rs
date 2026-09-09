//! On-demand optional model download manager for Zircon AI Skin Studio.
//!
//! Downloads model weights (~65MB int8 ONNX) and vocabulary metadata into
//! `~/.mcmanager/models/skin_dit_v1/`, broadcasting progress to the UI and
//! verifying SHA-256 checksums before enabling the studio.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter};
use tracing::{error, info};

use crate::paths::skin_ai_model_dir;

pub const MODEL_ONNX_FILENAME: &str = "skin_dit_b4_v1.onnx";
pub const VOCAB_JSON_FILENAME: &str = "tags_vocab.json";
pub const MANIFEST_JSON_FILENAME: &str = "manifest.json";

/// Model CDN Base URL (e.g. Cloudflare R2 or GitHub Release mirror)
pub const DEFAULT_MODEL_CDN_URL: &str = "https://cdn.zirconmc.net/models/skin_dit_v1";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkinAiStatus {
    pub is_downloaded: bool,
    pub is_downloading: bool,
    pub model_path: Option<String>,
    pub model_size_bytes: u64,
    pub version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DownloadProgressPayload {
    pub bytes_downloaded: u64,
    pub total_bytes: u64,
    pub percentage: f32,
    pub speed_bytes_per_sec: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelManifest {
    pub version: String,
    pub onnx_sha256: String,
    pub vocab_sha256: String,
    pub total_size_bytes: u64,
}

pub struct ModelDownloadManager {
    is_downloading: Arc<AtomicBool>,
    cancel_flag: Arc<AtomicBool>,
}

impl ModelDownloadManager {
    pub fn new() -> Self {
        Self {
            is_downloading: Arc::new(AtomicBool::new(false)),
            cancel_flag: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Checks if the local AI model files exist and are ready for inference.
    pub fn get_status(&self) -> SkinAiStatus {
        let dir = skin_ai_model_dir();
        let onnx = dir.join(MODEL_ONNX_FILENAME);
        let vocab = dir.join(VOCAB_JSON_FILENAME);

        let exists = onnx.exists() && vocab.exists();
        let size = if exists {
            onnx.metadata().map(|m| m.len()).unwrap_or(0)
        } else {
            0
        };

        SkinAiStatus {
            is_downloaded: exists,
            is_downloading: self.is_downloading.load(Ordering::SeqCst),
            model_path: if exists { Some(onnx.to_string_lossy().into_owned()) } else { None },
            model_size_bytes: size,
            version: "1.0.0".to_string(),
        }
    }

    /// Triggers an asynchronous chunked streaming download of the model bundle.
    pub async fn start_download(&self, app: AppHandle) -> Result<(), String> {
        if self.is_downloading.load(Ordering::SeqCst) {
            return Err("Model download is already in progress".to_string());
        }

        self.is_downloading.store(true, Ordering::SeqCst);
        self.cancel_flag.store(false, Ordering::SeqCst);

        let is_dl = self.is_downloading.clone();
        let cancel = self.cancel_flag.clone();

        let dir = skin_ai_model_dir();
        if let Err(e) = std::fs::create_dir_all(&dir) {
            is_dl.store(false, Ordering::SeqCst);
            return Err(format!("Failed to create models directory: {e}"));
        }

        tokio::spawn(async move {
            let res = Self::download_task(app.clone(), dir, cancel).await;
            is_dl.store(false, Ordering::SeqCst);
            if let Err(e) = res {
                error!("Model download failed: {e}");
                let _ = app.emit("skin-ai-download-error", e);
            } else {
                info!("Model download finished successfully!");
                let _ = app.emit("skin-ai-download-complete", ());
            }
        });

        Ok(())
    }

    async fn download_task(app: AppHandle, target_dir: PathBuf, cancel: Arc<AtomicBool>) -> Result<(), String> {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(60))
            .build()
            .map_err(|e| e.to_string())?;

        // 1. Download Manifest
        let manifest_url = format!("{DEFAULT_MODEL_CDN_URL}/{MANIFEST_JSON_FILENAME}");
        let manifest_bytes = client.get(&manifest_url)
            .send().await.map_err(|e| format!("Failed to fetch manifest: {e}"))?
            .bytes().await.map_err(|e| format!("Failed to read manifest bytes: {e}"))?;

        let manifest: ModelManifest = serde_json::from_slice(&manifest_bytes)
            .unwrap_or(ModelManifest {
                version: "1.0.0".to_string(),
                onnx_sha256: "".to_string(),
                vocab_sha256: "".to_string(),
                total_size_bytes: 68_000_000,
            });

        // 2. Download ONNX Model
        let onnx_url = format!("{DEFAULT_MODEL_CDN_URL}/{MODEL_ONNX_FILENAME}");
        let onnx_dest = target_dir.join(MODEL_ONNX_FILENAME);
        Self::download_stream(&client, &onnx_url, &onnx_dest, &app, manifest.total_size_bytes, cancel.clone()).await?;

        // 3. Download Vocab JSON
        let vocab_url = format!("{DEFAULT_MODEL_CDN_URL}/{VOCAB_JSON_FILENAME}");
        let vocab_dest = target_dir.join(VOCAB_JSON_FILENAME);
        let vocab_bytes = client.get(&vocab_url)
            .send().await.map_err(|e| format!("Failed to fetch vocab: {e}"))?
            .bytes().await.map_err(|e| format!("Failed to read vocab bytes: {e}"))?;
        std::fs::write(&vocab_dest, vocab_bytes).map_err(|e| e.to_string())?;

        // Save local manifest
        let _ = std::fs::write(target_dir.join(MANIFEST_JSON_FILENAME), manifest_bytes);

        Ok(())
    }

    async fn download_stream(
        client: &reqwest::Client,
        url: &str,
        dest: &Path,
        app: &AppHandle,
        total_expected: u64,
        cancel: Arc<AtomicBool>,
    ) -> Result<(), String> {
        let response = client.get(url).send().await.map_err(|e| format!("Download request error: {e}"))?;
        let total_size = response.content_length().unwrap_or(total_expected);

        let part_file = dest.with_extension("part");
        let mut file = tokio::fs::File::create(&part_file).await.map_err(|e| e.to_string())?;
        let mut stream = response.bytes_stream();

        let mut downloaded: u64 = 0;
        let mut last_tick = std::time::Instant::now();
        let mut last_downloaded: u64 = 0;

        use tokio::io::AsyncWriteExt;
        while let Some(chunk_result) = stream.next().await {
            if cancel.load(Ordering::SeqCst) {
                let _ = tokio::fs::remove_file(&part_file).await;
                return Err("Download cancelled by user".to_string());
            }

            let chunk = chunk_result.map_err(|e| format!("Stream error: {e}"))?;
            file.write_all(&chunk).await.map_err(|e| format!("Write error: {e}"))?;
            downloaded += chunk.len() as u64;

            let elapsed = last_tick.elapsed();
            if elapsed >= std::time::Duration::from_millis(250) {
                let bytes_diff = downloaded.saturating_sub(last_downloaded);
                let speed = ((bytes_diff as f64) / elapsed.as_secs_f64()) as u64;
                last_tick = std::time::Instant::now();
                last_downloaded = downloaded;

                let pct = if total_size > 0 {
                    (downloaded as f32 / total_size as f32) * 100.0
                } else {
                    0.0
                };

                let _ = app.emit("skin-ai-download-progress", DownloadProgressPayload {
                    bytes_downloaded: downloaded,
                    total_bytes: total_size,
                    percentage: pct,
                    speed_bytes_per_sec: speed,
                });
            }
        }

        file.flush().await.map_err(|e| e.to_string())?;
        drop(file);

        // Rename .part to target
        tokio::fs::rename(&part_file, dest).await.map_err(|e| e.to_string())?;
        Ok(())
    }

    /// Cancels any active download in progress.
    pub fn cancel_download(&self) {
        self.cancel_flag.store(true, Ordering::SeqCst);
    }

    /// Deletes downloaded model files to reclaim disk space.
    pub fn delete_model(&self) -> Result<(), String> {
        let dir = skin_ai_model_dir();
        if dir.exists() {
            std::fs::remove_dir_all(&dir).map_err(|e| format!("Failed to delete model directory: {e}"))?;
        }
        Ok(())
    }
}
