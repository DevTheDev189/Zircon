//! Resolves the full launch environment for a BOM's Minecraft version + loader:
//! the Mojang version manifest and version profile JSON, the client JAR,
//! vanilla libraries and natives, the loader profile (Fabric/Quilt/Forge/
//! NeoForge), the asset index and objects, and the Java runtime.
//!
//! May download hundreds of megabytes on first run (libraries + assets).
//!
//! Port of com.mcmanager.client.launch.MinecraftClasspathBuilder.

use std::collections::{BTreeMap, HashSet};
use std::path::{Path, PathBuf};
use std::time::Duration;

use futures_util::stream;
use futures_util::StreamExt;
use zircon_core::model::ModLoaderInfo;

use crate::error::LauncherError;
use crate::launch::fabric_quilt;
use crate::launch::java::JavaRuntimeResolver;
use crate::paths;
use crate::sync::mod_sync::ProgressListener;

/// The Mojang version manifest (snapshot + release list).
pub const VERSION_MANIFEST_URL: &str =
    "https://piston-meta.mojang.com/mc/game/version_manifest_v2.json";

/// Fabric loader meta API root.
pub const FABRIC_META_URL: &str = "https://meta.fabricmc.net/v2";

/// Quilt loader meta API root.
pub const QUILT_META_URL: &str = "https://meta.quiltmc.org/v3";

/// Everything needed to launch the game process.
#[derive(Debug, Clone)]
pub struct LaunchData {
    pub main_class: String,
    pub classpath: String,
    pub asset_index_id: String,
    pub version_name: String,
    pub assets_dir: PathBuf,
    pub natives_dir: PathBuf,
    pub java_home: PathBuf,
    pub jvm_args: Vec<String>,
    pub game_args: Vec<String>,
}

/// Resolves the full launch environment for a BOM's Minecraft version + loader.
#[derive(Debug)]
pub struct MinecraftClasspathBuilder {
    cache_dir: PathBuf,
    http: reqwest::Client,
}

impl MinecraftClasspathBuilder {
    /// Creates a builder rooted at `cache_dir` (default `~/.mcmanager/launcher`).
    ///
    /// Mirrors the Java `HttpClient.newBuilder().connectTimeout(15s)`; reqwest
    /// follows redirects by default like the Java `Redirect.NORMAL` policy.
    pub fn new(cache_dir: PathBuf) -> Self {
        let http = reqwest::Client::builder()
            .user_agent("Zircon-Launcher/1.0.0 (https://github.com/DevTheDev189/Zircon)")
            .connect_timeout(Duration::from_secs(15))
            .tcp_keepalive(Duration::from_secs(60))
            .pool_max_idle_per_host(32)
            .pool_idle_timeout(Duration::from_secs(30))
            .build()
            .unwrap_or_else(|_| reqwest::Client::new());
        Self { cache_dir, http }
    }

    /// Creates a builder rooted at the default launcher cache directory.
    pub fn new_default() -> Self {
        Self::new(paths::launcher_dir())
    }

    /// The cache directory this builder downloads into.
    pub fn get_cache_dir(&self) -> &Path {
        &self.cache_dir
    }

    /// Resolves the full launch environment for a BOM's Minecraft version + loader.
    /// May download hundreds of megabytes on first run (libraries + assets).
    pub async fn resolve(
        &self,
        mc_version: &str,
        loader: &ModLoaderInfo,
        required_java_major: i32,
    ) -> Result<LaunchData, LauncherError> {
        self.resolve_with_progress(mc_version, loader, required_java_major, None)
            .await
    }

    /// Resolves the full launch environment with granular progress updates emitted
    /// to `listener`.
    pub async fn resolve_with_progress(
        &self,
        mc_version: &str,
        loader: &ModLoaderInfo,
        required_java_major: i32,
        listener: Option<&dyn ProgressListener>,
    ) -> Result<LaunchData, LauncherError> {
        self.resolve_with_progress_and_override(mc_version, loader, required_java_major, None, listener)
            .await
    }

    /// Resolves the full launch environment, supporting an optional custom Java runtime override path.
    pub async fn resolve_with_progress_and_override(
        &self,
        mc_version: &str,
        loader: &ModLoaderInfo,
        required_java_major: i32,
        java_override: Option<&Path>,
        listener: Option<&dyn ProgressListener>,
    ) -> Result<LaunchData, LauncherError> {

        tokio::fs::create_dir_all(&self.cache_dir).await?;

        if let Some(l) = listener {
            l.on_status(&format!("Resolving Minecraft {mc_version} manifest..."));
        }

        // --- locate the version ---
        let version_json = self.resolve_version_json(mc_version).await?;
        let version_id = version_json
            .get("id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| LauncherError::Parse("version JSON missing 'id'".to_string()))?;

        let libraries_dir = self.cache_dir.join("libraries");
        let natives_dir = self
            .cache_dir
            .join("natives")
            .join(sanitize(&format!("{version_id}-{}", loader_name(loader))));
        tokio::fs::create_dir_all(&natives_dir).await?;

        let mut classpath: Vec<PathBuf> = Vec::new();

        // --- vanilla client jar ---
        let client_url = version_json
            .get("downloads")
            .and_then(|d| d.get("client"))
            .and_then(|c| c.get("url"))
            .and_then(|u| u.as_str())
            .ok_or_else(|| {
                LauncherError::Parse("version JSON missing downloads.client.url".to_string())
            })?;
        let client_jar = self
            .cache_dir
            .join("versions")
            .join(version_id)
            .join(format!("{version_id}.jar"));
        if !client_jar.is_file() {
            if let Some(l) = listener {
                l.on_status(&format!("Downloading Minecraft {version_id} client JAR..."));
            }
        }
        self.download_if_missing(client_url, &client_jar).await?;
        classpath.push(client_jar.clone());

        // --- vanilla libraries + natives ---
        // Keyed by "group:artifact" so a loader-provided version of a shared
        // dependency (e.g. ASM, pulled in by Fabric Loader) replaces the vanilla
        // one instead of both landing on the classpath — Fabric's Knot loader
        // refuses to start if it sees duplicate ASM classes.
        let mut library_by_artifact: BTreeMap<String, PathBuf> = BTreeMap::new();
        if let Some(libraries) = version_json.get("libraries").and_then(|l| l.as_array()) {
            if let Some(l) = listener {
                l.on_status("Checking game libraries & natives...");
            }
            for element in libraries {
                if !rules_allow(element) {
                    continue;
                }
                let Some(downloads) = element.get("downloads") else {
                    continue;
                };
                if downloads.is_null() {
                    continue;
                }
                let Some(name) = element.get("name").and_then(|n| n.as_str()) else {
                    continue;
                };
                if let Some(artifact) = downloads.get("artifact") {
                    let url = artifact
                        .get("url")
                        .and_then(|u| u.as_str())
                        .ok_or_else(|| {
                            LauncherError::Parse(format!("library artifact missing url for {name}"))
                        })?;
                    let jar = libraries_dir.join(artifact_path(artifact, name, None));
                    self.download_if_missing(url, &jar).await?;
                    library_by_artifact.insert(group_and_artifact(name), jar);
                }
                if let Some(classifiers) = downloads.get("classifiers") {
                    if let Some(classifier) = pick_natives_classifier(classifiers) {
                        if let Some(natives) = classifiers.get(classifier.as_str()) {
                            let url =
                                natives.get("url").and_then(|u| u.as_str()).ok_or_else(|| {
                                    LauncherError::Parse(format!(
                                        "natives artifact missing url for {name}"
                                     ))
                                })?;
                            let jar =
                                libraries_dir.join(artifact_path(natives, name, Some(&classifier)));
                            self.download_if_missing(url, &jar).await?;
                            extract_natives(&jar, &natives_dir)?;
                        }
                    }
                }
            }
        }

        // --- loader ---
        let mut main_class = version_json
            .get("mainClass")
            .and_then(|m| m.as_str())
            .ok_or_else(|| LauncherError::Parse("version JSON missing mainClass".to_string()))?
            .to_string();
        let loader_type = loader.r#type.to_lowercase();
        let mut jvm_args: Vec<String> = Vec::new();
        let mut game_args: Vec<String> = Vec::new();
        match loader_type.as_str() {
            "fabric" | "quilt" => {
                if let Some(l) = listener {
                    l.on_status(&format!("Resolving {loader_type} loader profile..."));
                }
                let loader_version = if loader.version.trim().is_empty() {
                    None
                } else {
                    Some(loader.version.as_str())
                };
                main_class = fabric_quilt::resolve_loader_profile(
                    mc_version,
                    &loader_type,
                    loader_version,
                    &self.cache_dir,
                    &mut library_by_artifact,
                    &libraries_dir,
                )?;
                classpath.extend(library_by_artifact.values().cloned());
            }
            "neoforge" | "forge" => {
                if loader.version.trim().is_empty() {
                    return Err(LauncherError::InvalidInput(format!(
                        "Loader version is required for {loader_type} \
                         (set 'modLoader.version' in the server BOM)"
                    )));
                }
                if let Some(l) = listener {
                    l.on_status(&format!("Resolving {loader_type} {} profile...", loader.version));
                }
                classpath.extend(library_by_artifact.values().cloned());
                // Install the loader headlessly, parse the generated version
                // profile and merge its libraries/arguments into the launch.
                let vanilla_json = self
                    .cache_dir
                    .join("versions")
                    .join(sanitize(mc_version))
                    .join(format!("{mc_version}.json"));
                let forge = crate::launch::forge_neoforge::ForgeLaunchResolver::new()
                    .resolve(
                        &self.cache_dir,
                        mc_version,
                        &loader_type,
                        &loader.version,
                        &vanilla_json,
                        &client_jar,
                        &natives_dir,
                        &mut classpath,
                    )
                    .await?;
                main_class = forge.main_class;
                jvm_args = forge.jvm_args;
                game_args = forge.game_args;
            }
            _ => {
                classpath.extend(library_by_artifact.values().cloned());
                tracing::info!("No loader configured — launching vanilla");
            }
        }

        // --- asset index + objects ---
        let asset_index = version_json
            .get("assetIndex")
            .ok_or_else(|| LauncherError::Parse("version JSON missing assetIndex".to_string()))?;
        let asset_index_id = asset_index
            .get("id")
            .and_then(|i| i.as_str())
            .ok_or_else(|| LauncherError::Parse("assetIndex missing id".to_string()))?;
        let assets_dir = self.cache_dir.join("assets");
        let index_file = assets_dir
            .join("indexes")
            .join(format!("{asset_index_id}.json"));
        let asset_index_url = asset_index
            .get("url")
            .and_then(|u| u.as_str())
            .ok_or_else(|| LauncherError::Parse("assetIndex missing url".to_string()))?;
        self.download_if_missing(asset_index_url, &index_file)
            .await?;
        self.download_assets(&index_file, &assets_dir, listener).await?;

        let classpath_str = {
            let sep = if cfg!(windows) { ";" } else { ":" };
            classpath
                .iter()
                .map(|p| p.to_string_lossy().into_owned())
                .collect::<Vec<_>>()
                .join(sep)
        };

        let java_major = version_json
            .get("javaVersion")
            .and_then(|jv| jv.get("majorVersion"))
            .and_then(|m| m.as_i64())
            .map(|m| m as i32)
            .unwrap_or(required_java_major);

        let java_home = JavaRuntimeResolver::new(self.cache_dir.clone())
            .resolve_with_override(java_major, java_override, listener)
            .await?;

        tracing::info!(
            "Launch data ready: version={version_id}, loader={loader_type}, \
             classpath entries={}, java={}",
            classpath.len(),
            java_home.display()
        );

        Ok(LaunchData {
            main_class,
            classpath: classpath_str,
            asset_index_id: asset_index_id.to_string(),
            version_name: version_id.to_string(),
            assets_dir,
            natives_dir,
            java_home,
            jvm_args,
            game_args,
        })
    }

    /// Downloads the version manifest (once), finds `mc_version` and downloads
    /// + parses its version profile JSON.
    async fn resolve_version_json(
        &self,
        mc_version: &str,
    ) -> Result<serde_json::Value, LauncherError> {
        let manifest_file = self.cache_dir.join("version_manifest_v2.json");
        self.download_if_missing(VERSION_MANIFEST_URL, &manifest_file)
            .await?;
        let manifest_text = tokio::fs::read_to_string(&manifest_file).await?;
        let manifest: serde_json::Value = serde_json::from_str(&manifest_text)?;

        let mut url: Option<String> = None;
        if let Some(versions) = manifest.get("versions").and_then(|v| v.as_array()) {
            for entry in versions {
                if let Some(id) = entry.get("id").and_then(|i| i.as_str()) {
                    if id == mc_version {
                        url = entry
                            .get("url")
                            .and_then(|u| u.as_str())
                            .map(str::to_string);
                        break;
                    }
                }
            }
        }
        let url = url.ok_or_else(|| {
            LauncherError::NotFound(format!(
                "Minecraft version not found in manifest: {mc_version}"
            ))
        })?;

        let version_file = self
            .cache_dir
            .join("versions")
            .join(sanitize(mc_version))
            .join(format!("{mc_version}.json"));
        self.download_if_missing(&url, &version_file).await?;
        let text = tokio::fs::read_to_string(&version_file).await?;
        let version_json: serde_json::Value = serde_json::from_str(&text)?;
        if version_json.get("id").is_none() {
            return Err(LauncherError::Parse(format!(
                "Invalid version JSON at {}",
                version_file.display()
            )));
        }
        Ok(version_json)
    }

    // ------------------------------------------------------------------
    // Assets
    // ------------------------------------------------------------------

    /// Downloads every asset listed in `index_file` under `assets_dir/objects`,
    /// with bounded concurrency (8), upfront directory shard allocation, and
    /// adaptive self-healing retries with backoff.
    /// Emits progress to `listener` and caches verification status in `.verified`
    /// to avoid expensive synchronous 3,500+ file disk scans on subsequent launches.
    async fn download_assets(
        &self,
        index_file: &Path,
        assets_dir: &Path,
        listener: Option<&dyn ProgressListener>,
    ) -> Result<(), LauncherError> {
        let verified_marker = index_file.with_extension("verified");
        let text = tokio::fs::read_to_string(index_file).await?;
        let index: serde_json::Value = serde_json::from_str(&text)?;
        let Some(objects) = index.get("objects").and_then(|o| o.as_object()) else {
            return Ok(());
        };
        let objects_dir = assets_dir.join("objects");
        let total_objects = objects.len();

        // Fast-path: If the .verified token exists and records the same object count,
        // skip the expensive filesystem metadata scan across thousands of files.
        if verified_marker.is_file() {
            if let Ok(marker_content) = tokio::fs::read_to_string(&verified_marker).await {
                if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&marker_content) {
                    if parsed.get("objects_count").and_then(|c| c.as_u64()) == Some(total_objects as u64) {
                        tracing::info!("Assets: {total_objects} objects verified via fast-path cache marker");
                        if let Some(l) = listener {
                            l.on_status(&format!("Game assets verified ({total_objects} objects)..."));
                        }
                        return Ok(());
                    }
                }
            }
        }

        // Pre-create the root objects directory and all 256 two-hex-digit shard
        // subdirectories (00..ff) upfront. This takes <10ms and eliminates concurrent
        // `create_dir_all` race conditions on Windows when parallel workers download assets.
        tokio::fs::create_dir_all(&objects_dir).await?;
        for byte in 0u8..=255 {
            let shard = objects_dir.join(format!("{byte:02x}"));
            let _ = tokio::fs::create_dir(shard).await;
        }

        if let Some(l) = listener {
            l.on_status(&format!("Checking game assets ({total_objects} objects)..."));
        }

        // Compute which assets are missing or size-mismatched on disk.
        let mut missing: HashSet<String> = collect_missing_assets(objects, &objects_dir)
            .into_iter()
            .collect();
        tracing::info!(
            "Assets: {} total, {} to download",
            objects.len(),
            missing.len()
        );

        if missing.is_empty() {
            // Write fast-path marker
            let marker_json = serde_json::json!({
                "objects_count": total_objects,
                "verified_at": std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs()
            });
            let _ = tokio::fs::write(&verified_marker, marker_json.to_string()).await;
            return Ok(());
        }

        // Download with bounded concurrency (8) and up to 5 retry passes with backoff.
        let total_to_download = missing.len();
        if let Some(l) = listener {
            l.on_status(&format!("Downloading {total_to_download} game assets..."));
        }
        let semaphore = tokio::sync::Semaphore::new(8);
        let mut pass = 0;
        let mut downloaded_count = 0usize;
        let mut last_error: Option<String> = None;

        while !missing.is_empty() && pass < 5 {
            pass += 1;
            let hashes: Vec<String> = missing.iter().cloned().collect();
            let results: Vec<(String, Result<(), LauncherError>)> = stream::iter(hashes)
                .map(|hash| {
                    let semaphore = &semaphore;
                    let objects_dir = &objects_dir;
                    async move {
                        let outcome = match semaphore.acquire().await {
                            Ok(_permit) => self.download_asset(objects_dir, &hash).await,
                            Err(_) => Err(LauncherError::Process(
                                "asset download semaphore closed".to_string(),
                            )),
                        };
                        (hash, outcome)
                    }
                })
                .buffer_unordered(8)
                .collect()
                .await;

            let mut failures = 0usize;
            let mut next_missing = HashSet::new();
            for (hash, result) in results {
                if let Err(e) = result {
                    last_error = Some(e.to_string());
                    tracing::warn!("Asset download failed for {hash}: {e}");
                    failures += 1;
                    next_missing.insert(hash);
                } else {
                    downloaded_count += 1;
                    if let Some(l) = listener {
                        if downloaded_count % 25 == 0 || downloaded_count == total_to_download {
                            l.on_status(&format!(
                                "Downloading game assets ({downloaded_count}/{total_to_download})..."
                            ));
                            l.on_progress(
                                downloaded_count as f64 / total_to_download as f64,
                                &hash,
                            );
                        }
                    }
                }
            }
            tracing::info!("Asset download pass {pass} complete ({failures} failed)");
            if failures == 0 {
                break;
            }
            missing = next_missing;
            tokio::time::sleep(Duration::from_millis(500 * (1 << (pass - 1)))).await;
        }

        // Self-Healing Step 1: Always re-verify the actual on-disk state.
        // If all assets are on disk with matching sizes (even if an async worker permit
        // timed out or reported a spurious stream error), treat it as a success!
        missing = collect_missing_assets(objects, &objects_dir)
            .into_iter()
            .collect();

        // Self-Healing Step 2: If only a few assets are still missing, perform a targeted
        // sequential/low-concurrency recovery pass.
        if !missing.is_empty() && missing.len() <= 30 {
            tracing::info!(
                "Self-healing: attempting targeted recovery for {} remaining missing assets...",
                missing.len()
            );
            if let Some(l) = listener {
                l.on_status(&format!(
                    "Retrying {} remaining game assets...",
                    missing.len()
                ));
            }
            let remaining_hashes: Vec<String> = missing.iter().cloned().collect();
            for hash in remaining_hashes {
                match self.download_asset(&objects_dir, &hash).await {
                    Ok(()) => {
                        missing.remove(&hash);
                    }
                    Err(e) => {
                        last_error = Some(e.to_string());
                        tracing::warn!("Recovery pass failed for {hash}: {e}");
                    }
                }
            }

            // Final disk check after recovery pass
            missing = collect_missing_assets(objects, &objects_dir)
                .into_iter()
                .collect();
        }

        if !missing.is_empty() {
            let example = missing.iter().next().cloned().unwrap_or_default();
            let err_detail = last_error
                .map(|e| format!(" (last error: {e})"))
                .unwrap_or_default();
            return Err(LauncherError::Network(format!(
                "Could not download {} of {} asset files (e.g. {example}){err_detail}. \
                 Minecraft cannot render without its resources — check the \
                 network and retry the launch.",
                missing.len(),
                objects.len()
            )));
        }

        // Successfully downloaded and verified all assets, write the .verified marker
        let marker_json = serde_json::json!({
            "objects_count": total_objects,
            "verified_at": std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs()
        });
        let _ = tokio::fs::write(&verified_marker, marker_json.to_string()).await;

        Ok(())
    }

    /// Downloads a single asset by hash from Mojang's resources CDN into
    /// `objects_dir/<hash[..2]>/<hash>`.
    async fn download_asset(&self, objects_dir: &Path, hash: &str) -> Result<(), LauncherError> {
        let url = format!(
            "https://resources.download.minecraft.net/{}/{hash}",
            &hash[0..2]
        );
        let target = asset_target(objects_dir, hash);
        let response = self
            .http
            .get(&url)
            .timeout(Duration::from_secs(60))
            .send()
            .await?;
        if !response.status().is_success() {
            return Err(LauncherError::Http {
                status: response.status().as_u16(),
                url,
            });
        }
        let bytes = response.bytes().await?;
        if let Some(parent) = target.parent() {
            if !parent.is_dir() {
                let _ = tokio::fs::create_dir_all(parent).await;
            }
        }
        tokio::fs::write(&target, &bytes).await?;
        Ok(())
    }

    // ------------------------------------------------------------------
    // HTTP helpers
    // ------------------------------------------------------------------

    /// Downloads `url` to `target` unless a non-empty regular file already
    /// exists there.
    async fn download_if_missing(&self, url: &str, target: &Path) -> Result<(), LauncherError> {
        if target.is_file() && std::fs::metadata(target).map(|m| m.len()).unwrap_or(0) > 0 {
            return Ok(());
        }
        if let Some(parent) = target.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        let response = self.http.get(url).send().await?;
        if !response.status().is_success() {
            return Err(LauncherError::Http {
                status: response.status().as_u16(),
                url: url.to_string(),
            });
        }
        let bytes = response.bytes().await?;
        tokio::fs::write(target, &bytes).await?;
        Ok(())
    }
}

// ------------------------------------------------------------------
// Libraries & natives helpers (static in the Java; free functions here)
// ------------------------------------------------------------------

/// Maven coordinate "group:artifact:version[:classifier]" -> "group:artifact[:classifier]"
/// (the version is dropped so a loader-provided version of a dependency can replace
/// the vanilla one, but the classifier is kept so per-OS native jars — which modern
/// version manifests list as their own "group:artifact:version:natives-xxx" library
/// entries with a real "downloads.artifact" — don't collide with the main artifact
/// or with each other).
pub(crate) fn group_and_artifact(maven_coordinate: &str) -> String {
    let parts: Vec<&str> = maven_coordinate.split(':').collect();
    if parts.len() >= 4 {
        format!("{}:{}:{}", parts[0], parts[1], parts[3])
    } else if parts.len() >= 2 {
        format!("{}:{}", parts[0], parts[1])
    } else {
        maven_coordinate.to_string()
    }
}

/// Evaluates a library's Mojang `rules` array: the last matching rule's
/// action wins, and libraries without `rules` are always allowed.
pub(crate) fn rules_allow(lib: &serde_json::Value) -> bool {
    let Some(rules) = lib.get("rules") else {
        return true;
    };
    let Some(array) = rules.as_array() else {
        return true;
    };
    let mut allow = false;
    for rule_element in array {
        let rule = rule_element.as_object();
        let applies = match rule {
            Some(rule) => os_matches(rule.get("os")),
            None => false,
        };
        if applies {
            allow = rule.and_then(|r| r.get("action")).and_then(|a| a.as_str()) == Some("allow");
        }
    }
    allow
}

/// Whether the current host satisfies an `os` rule object (`name` and/or
/// `arch`). An absent or empty object matches every platform.
fn os_matches(os: Option<&serde_json::Value>) -> bool {
    let Some(os) = os else {
        return true;
    };
    if let Some(os_target) = os.get("name").and_then(|n| n.as_str()) {
        let host_matches = match os_target {
            "windows" => cfg!(target_os = "windows"),
            "linux" => cfg!(target_os = "linux"),
            "osx" => cfg!(target_os = "macos"),
            _ => false,
        };
        if !host_matches {
            return false;
        }
    }
    if let Some(arch_target) = os.get("arch").and_then(|a| a.as_str()) {
        let host_matches = match arch_target {
            "x86" => cfg!(target_arch = "x86"),
            "x86_64" => cfg!(target_arch = "x86_64"),
            "arm64" => cfg!(target_arch = "aarch64"),
            _ => false,
        };
        if !host_matches {
            return false;
        }
    }
    true
}

/// Picks the natives classifier for the current OS, preferring the plain
/// "natives-<os>" key and falling back to the arch-suffixed variant
/// ("natives-<os>-64"/"-32"), or `None` when neither exists.
fn pick_natives_classifier(classifiers: &serde_json::Value) -> Option<String> {
    let os_key = if cfg!(target_os = "windows") {
        "natives-windows"
    } else if cfg!(target_os = "macos") {
        "natives-macos"
    } else {
        "natives-linux"
    };
    if classifiers.get(os_key).is_some() {
        return Some(os_key.to_string());
    }
    let arch_suffix = if cfg!(target_pointer_width = "64") {
        "-64"
    } else {
        "-32"
    };
    let os_arch_key = format!("{os_key}{arch_suffix}");
    if classifiers.get(os_arch_key.as_str()).is_some() {
        return Some(os_arch_key);
    }
    None
}

/// The artifact's on-disk path relative to the libraries dir: the
/// `path` field when present, else the Maven layout derived from `name`.
fn artifact_path(artifact: &serde_json::Value, name: &str, classifier: Option<&str>) -> String {
    if let Some(path) = artifact.get("path").and_then(|p| p.as_str()) {
        return path.to_string();
    }
    maven_path_with_classifier(name, classifier)
}

/// Maven layout for a coordinate without a classifier, e.g.
/// "net.fabricmc:fabric-loader:0.15.11" ->
/// "net/fabricmc/fabric-loader/0.15.11/fabric-loader-0.15.11.jar".
pub(crate) fn maven_path(name: &str) -> String {
    maven_path_with_classifier(name, None)
}

fn maven_path_with_classifier(name: &str, classifier: Option<&str>) -> String {
    let parts: Vec<&str> = name.split(':').collect();
    let group = parts[0].replace('.', "/");
    let artifact = parts.get(1).copied().unwrap_or("unknown");
    let version = parts.get(2).copied().unwrap_or("unknown");
    let file = match classifier {
        Some(classifier) => format!("{artifact}-{version}-{classifier}.jar"),
        None => format!("{artifact}-{version}.jar"),
    };
    format!("{group}/{artifact}/{version}/{file}")
}

/// Extracts top-level native libraries (`.dll`/`.so`/`.dylib`) from a
/// natives JAR into `natives_dir`, skipping entries that already exist on
/// disk (mirrors the Java `extractNatives`).
fn extract_natives(jar: &Path, natives_dir: &Path) -> Result<(), LauncherError> {
    let guard = zircon_core::archive::ArchiveGuard::from_env(); // z0
    let mut existing: HashSet<String> = HashSet::new();
    if let Ok(entries) = std::fs::read_dir(natives_dir) {
        for entry in entries.flatten() {
            if let Some(name) = entry.file_name().to_str() {
                existing.insert(name.to_string());
            }
        }
    }
    let file = std::fs::File::open(jar)?;
    let mut archive = zip::ZipArchive::new(file).map_err(zip_error)?;
    for idx in 0..archive.len() {
        guard.record_entry().map_err(|err| LauncherError::InvalidInput(err.to_string()))?;
        let mut entry = archive.by_index(idx).map_err(zip_error)?;
        if entry.is_dir() {
            continue;
        }
        let name = entry.name().to_string();
        if !zircon_core::archive::limits::is_safe_entry_path(Path::new(&name)) || name.contains('/')  { // z0
            continue;
        }
        if existing.contains(&name) {
            continue;
        }
        if !(name.ends_with(".dll") || name.ends_with(".so") || name.ends_with(".dylib")) {
            continue;
        }
        guard /* z0 */
            .check_entry_header(&name, entry.size(), entry.compressed_size()) /* z0 */
            .map_err(|err| LauncherError::InvalidInput(err.to_string()))?;
        let mut out = std::fs::File::create(natives_dir.join(&name))?;
        let written = std::io::copy(&mut entry, &mut out)?; // z0
        guard /* z0 */
            .track_stream_chunk(written, entry.compressed_size()) /* z0 */
            .map_err(|err| LauncherError::InvalidInput(err.to_string()))?;
    }
    Ok(())
}

/// `assets/objects/<hash[..2]>/<hash>` — the on-disk asset target.
fn asset_target(objects_dir: &Path, hash: &str) -> PathBuf {
    objects_dir.join(&hash[0..2]).join(hash)
}

/// Hashes of every asset in `objects` that is missing or size-mismatched on
/// disk. `objects` is the parsed `objects` map of an asset index.
fn collect_missing_assets(
    objects: &serde_json::Map<String, serde_json::Value>,
    objects_dir: &Path,
) -> Vec<String> {
    let mut missing = Vec::new();
    for obj in objects.values() {
        let Some(hash) = obj.get("hash").and_then(|h| h.as_str()) else {
            continue;
        };
        let size = obj.get("size").and_then(|s| s.as_u64()).unwrap_or(0);
        let target = asset_target(objects_dir, hash);
        let on_disk = if target.is_file() {
            std::fs::metadata(&target).map(|m| m.len()).unwrap_or(0)
        } else {
            0
        };
        if on_disk != size {
            missing.push(hash.to_string());
        }
    }
    missing
}

/// "loader" label for the natives dir suffix: the configured loader type, or
/// "vanilla" when it is blank.
fn loader_name(loader: &ModLoaderInfo) -> String {
    if loader.r#type.trim().is_empty() {
        "vanilla".to_string()
    } else {
        loader.r#type.clone()
    }
}

/// Path-safe version of `name`: characters outside `[A-Za-z0-9._-]` are
/// replaced with `_` (matching the Java `replaceAll("[^A-Za-z0-9._-]", "_")`).
pub fn sanitize(name: &str) -> String {
    name.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '.' || c == '_' || c == '-' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

fn zip_error(e: zip::result::ZipError) -> LauncherError {
    LauncherError::Io(std::io::Error::new(std::io::ErrorKind::InvalidData, e))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use std::sync::atomic::{AtomicUsize, Ordering};

    /// Unique, self-cleaning temp directory for deterministic tests.
    struct TempDir(PathBuf);

    impl TempDir {
        fn new(tag: &str) -> Self {
            static COUNTER: AtomicUsize = AtomicUsize::new(0);
            let n = COUNTER.fetch_add(1, Ordering::Relaxed);
            let dir = std::env::temp_dir()
                .join(format!("zircon-classpath-{tag}-{}-{n}", std::process::id()));
            let _ = std::fs::remove_dir_all(&dir);
            std::fs::create_dir_all(&dir).unwrap();
            TempDir(dir)
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
    fn sanitize_keeps_path_safe_chars() {
        assert_eq!("1.20.4", sanitize("1.20.4"));
        assert_eq!("1.20.4_forge-47.2.0", sanitize("1.20.4+forge-47.2.0"));
        assert_eq!("a_b_c_d", sanitize("a b/c:d"));
        assert_eq!("", sanitize(""));
    }

    #[test]
    fn rules_without_rules_are_allowed() {
        assert!(rules_allow(
            &serde_json::json!({ "name": "com.example:lib:1.0" })
        ));
    }

    #[test]
    fn rules_last_matching_action_wins() {
        // An empty os object matches every platform, so these are
        // host-independent.
        let deny_after_allow = serde_json::json!({
            "rules": [
                { "action": "allow", "os": {} },
                { "action": "deny", "os": {} }
            ]
        });
        assert!(!rules_allow(&deny_after_allow));

        let allow_after_deny = serde_json::json!({
            "rules": [
                { "action": "deny", "os": {} },
                { "action": "allow", "os": {} }
            ]
        });
        assert!(rules_allow(&allow_after_deny));
    }

    #[test]
    fn rules_os_name_gating_consistent_with_host() {
        let allow_windows = serde_json::json!({
            "rules": [{ "action": "allow", "os": { "name": "windows" } }]
        });
        assert_eq!(cfg!(target_os = "windows"), rules_allow(&allow_windows));

        let allow_linux = serde_json::json!({
            "rules": [{ "action": "allow", "os": { "name": "linux" } }]
        });
        assert_eq!(cfg!(target_os = "linux"), rules_allow(&allow_linux));

        let allow_osx = serde_json::json!({
            "rules": [{ "action": "allow", "os": { "name": "osx" } }]
        });
        assert_eq!(cfg!(target_os = "macos"), rules_allow(&allow_osx));
    }

    #[test]
    fn os_matches_absent_or_empty_object() {
        assert!(os_matches(None));
        assert!(os_matches(Some(&serde_json::json!({}))));
    }

    #[test]
    fn os_matches_name_and_arch_consistent_with_host() {
        let windows = serde_json::json!({ "name": "windows" });
        assert_eq!(cfg!(target_os = "windows"), os_matches(Some(&windows)));

        let arch_64 = serde_json::json!({ "arch": "x86_64" });
        assert_eq!(cfg!(target_arch = "x86_64"), os_matches(Some(&arch_64)));

        let arch_arm = serde_json::json!({ "arch": "arm64" });
        assert_eq!(cfg!(target_arch = "aarch64"), os_matches(Some(&arch_arm)));
    }

    #[test]
    fn pick_natives_classifier_prefers_plain_os_key() {
        let classifiers = serde_json::json!({
            "natives-windows": {},
            "natives-linux": {},
            "natives-macos": {}
        });
        let expected = if cfg!(target_os = "windows") {
            "natives-windows"
        } else if cfg!(target_os = "macos") {
            "natives-macos"
        } else {
            "natives-linux"
        };
        assert_eq!(
            Some(expected.to_string()),
            pick_natives_classifier(&classifiers)
        );
    }

    #[test]
    fn pick_natives_classifier_falls_back_to_arch_suffix() {
        let classifiers = serde_json::json!({
            "natives-windows-64": {},
            "natives-linux-64": {},
            "natives-macos-64": {},
            "natives-windows-32": {},
            "natives-linux-32": {},
            "natives-macos-32": {}
        });
        let base = if cfg!(target_os = "windows") {
            "natives-windows"
        } else if cfg!(target_os = "macos") {
            "natives-macos"
        } else {
            "natives-linux"
        };
        let suffix = if cfg!(target_pointer_width = "64") {
            "-64"
        } else {
            "-32"
        };
        assert_eq!(
            Some(format!("{base}{suffix}")),
            pick_natives_classifier(&classifiers)
        );
    }

    #[test]
    fn pick_natives_classifier_returns_none_when_absent() {
        assert_eq!(None, pick_natives_classifier(&serde_json::json!({})));
    }

    #[test]
    fn group_and_artifact_drops_version_keeps_classifier() {
        // Same group:artifact regardless of version -> loader can replace vanilla.
        assert_eq!("com.example:lib", group_and_artifact("com.example:lib:1.0"));
        assert_eq!("com.example:lib", group_and_artifact("com.example:lib:2.0"));
        // Classifier-bearing coordinates keep the classifier (parts[3]).
        assert_eq!(
            "org.lwjgl:lwjgl:natives-windows@jar",
            group_and_artifact("org.lwjgl:lwjgl:3.3.3:natives-windows@jar")
        );
        assert_eq!("a:b:d", group_and_artifact("a:b:c:d:e"));
    }

    #[test]
    fn maven_path_derivation() {
        assert_eq!(
            "net/fabricmc/fabric-loader/0.15.11/fabric-loader-0.15.11.jar",
            maven_path("net.fabricmc:fabric-loader:0.15.11")
        );
        assert_eq!(
            "org/lwjgl/lwjgl/3.3.3/lwjgl-3.3.3-natives-windows.jar",
            maven_path_with_classifier("org.lwjgl:lwjgl:3.3.3", Some("natives-windows"))
        );
        // Missing version falls back to "unknown" (Java behaviour).
        assert_eq!("a/b/unknown/b-unknown.jar", maven_path("a:b"));
    }

    #[test]
    fn artifact_path_uses_embedded_path_when_present() {
        let artifact = serde_json::json!({
            "path": "net/minecraft/client/1.20.4/client-1.20.4.jar",
            "url": "https://piston-data.mojang.com/..."
        });
        assert_eq!(
            "net/minecraft/client/1.20.4/client-1.20.4.jar",
            artifact_path(&artifact, "net.minecraft:client:1.20.4", None)
        );
    }

    #[test]
    fn artifact_path_falls_back_to_maven_layout() {
        let artifact = serde_json::json!({
            "url": "https://maven.fabricmc.net/net/fabricmc/fabric-loader/0.15.11/fabric-loader-0.15.11.jar"
        });
        assert_eq!(
            "net/fabricmc/fabric-loader/0.15.11/fabric-loader-0.15.11.jar",
            artifact_path(&artifact, "net.fabricmc:fabric-loader:0.15.11", None)
        );
    }

    #[test]
    fn asset_target_path_derivation() {
        let dir = TempDir::new("asset-path");
        let objects_dir = dir.path().join("objects");
        let hash = format!("ab{}", "c".repeat(38));
        assert_eq!(
            objects_dir.join("ab").join(&hash),
            asset_target(&objects_dir, &hash)
        );
    }

    #[test]
    fn asset_index_objects_map_to_hash_paths() {
        let dir = TempDir::new("assets");
        let objects_dir = dir.path().join("objects");
        let hash_a = format!("ab{}", "c".repeat(38));
        let hash_b = format!("cd{}", "e".repeat(38));
        let index = serde_json::json!({
            "objects": {
                "icons/icon_16x16.png": { "hash": hash_a, "size": 10 },
                "lang/en_us.json": { "hash": hash_b, "size": 20 }
            }
        });
        let objects = index["objects"].as_object().unwrap();

        // Nothing on disk yet: both are missing.
        let missing = collect_missing_assets(objects, &objects_dir);
        assert_eq!(2, missing.len());

        // A correctly-sized file at objects/<hash[..2]>/<hash> is not missing.
        let target_a = objects_dir.join(&hash_a[0..2]).join(&hash_a);
        std::fs::create_dir_all(target_a.parent().unwrap()).unwrap();
        std::fs::write(&target_a, vec![0u8; 10]).unwrap();
        let missing = collect_missing_assets(objects, &objects_dir);
        assert_eq!(vec![hash_b.clone()], missing);

        // A size-mismatched file is treated as missing (re-downloaded).
        std::fs::write(&target_a, vec![0u8; 11]).unwrap();
        let missing = collect_missing_assets(objects, &objects_dir);
        assert_eq!(2, missing.len());
        assert!(missing.contains(&hash_a));
        assert!(missing.contains(&hash_b));
    }

    #[test]
    fn extract_natives_skips_meta_inf_nested_and_existing() {
        let dir = TempDir::new("natives");
        let jar = dir.path().join("lwjgl-natives.jar");
        let natives_dir = dir.path().join("natives");
        std::fs::create_dir_all(&natives_dir).unwrap();

        // A pre-existing native must not be overwritten.
        std::fs::write(natives_dir.join("lwjgl.dll"), b"existing").unwrap();

        // Build a fixture natives JAR: top-level natives, a META-INF entry and
        // a nested file.
        let file = std::fs::File::create(&jar).unwrap();
        let mut writer = zip::ZipWriter::new(file);
        let mut add = |name: &str, content: &[u8]| {
            writer
                .start_file(
                    name,
                    zip::write::SimpleFileOptions::default()
                        .compression_method(zip::CompressionMethod::Stored),
                )
                .unwrap();
            writer.write_all(content).unwrap();
        };
        add("lwjgl.dll", b"new-dll");
        add("liblwjgl.so", b"so");
        add("liblwjgl.dylib", b"dylib");
        add("META-INF/MANIFEST.MF", b"manifest");
        add("nested/inner.so", b"nested");
        writer.finish().unwrap();

        extract_natives(&jar, &natives_dir).unwrap();

        // Top-level natives extracted; the pre-existing lwjgl.dll untouched;
        // META-INF and nested entries skipped.
        assert_eq!(
            b"existing".as_slice(),
            std::fs::read(natives_dir.join("lwjgl.dll"))
                .unwrap()
                .as_slice()
        );
        assert_eq!(
            b"so".as_slice(),
            std::fs::read(natives_dir.join("liblwjgl.so"))
                .unwrap()
                .as_slice()
        );
        assert_eq!(
            b"dylib".as_slice(),
            std::fs::read(natives_dir.join("liblwjgl.dylib"))
                .unwrap()
                .as_slice()
        );
        assert!(!natives_dir.join("MANIFEST.MF").exists());
        assert!(!natives_dir.join("inner.so").exists());
    }
}
