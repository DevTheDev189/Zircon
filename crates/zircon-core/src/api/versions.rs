//! Version resolution service for Minecraft versions and modloader versions.
//! Fetches live data from official metadata APIs:
//! - Mojang version manifest (game versions)
//! - Fabric meta API (Fabric loader versions)
//! - Quilt meta API (Quilt loader versions)
//! - Minecraft Forge promotions (Forge versions & stable recommended builds)
//! - NeoForge Maven metadata (NeoForge versions & latest release builds)

use std::sync::Arc;
use std::time::{Duration, Instant};
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;
use crate::security::ssrf;

const MOJANG_MANIFEST_URL: &str = "https://piston-meta.mojang.com/mc/game/version_manifest_v2.json";
const FABRIC_META_URL: &str = "https://meta.fabricmc.net/v2";
const QUILT_META_URL: &str = "https://meta.quiltmc.org/v3";
const FORGE_PROMOTIONS_URL: &str = "https://files.minecraftforge.net/net/minecraftforge/forge/promotions_slim.json";
const NEOFORGE_MAVEN_METADATA_URL: &str = "https://maven.neoforged.net/releases/net/neoforged/neoforge/maven-metadata.xml";

const CACHE_TTL: Duration = Duration::from_secs(600); // 10 minutes cache

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MinecraftVersionInfo {
    pub id: String,
    pub r#type: String, // "release" | "snapshot"
    pub release_time: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LoaderVersionResult {
    pub loader: String,
    pub mc_version: String,
    pub recommended: Option<String>,
    pub versions: Vec<String>,
}

#[derive(Default)]
struct CachedData<T> {
    data: Option<T>,
    fetched_at: Option<Instant>,
}

impl<T: Clone> CachedData<T> {
    fn get(&self) -> Option<T> {
        if let Some(fetched) = self.fetched_at {
            if fetched.elapsed() < CACHE_TTL {
                return self.data.clone();
            }
        }
        None
    }

    fn set(&mut self, data: T) {
        self.data = Some(data);
        self.fetched_at = Some(Instant::now());
    }
}

#[derive(Clone)]
pub struct VersionService {
    client: reqwest::Client,
    mc_versions_cache: Arc<RwLock<CachedData<Vec<MinecraftVersionInfo>>>>,
    forge_promos_cache: Arc<RwLock<CachedData<serde_json::Value>>>,
    neoforge_versions_cache: Arc<RwLock<CachedData<Vec<String>>>>,
}

impl Default for VersionService {
    fn default() -> Self {
        Self::new()
    }
}

impl VersionService {
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::builder()
                .timeout(Duration::from_secs(15))
                .build()
                .unwrap_or_default(),
            mc_versions_cache: Arc::new(RwLock::new(CachedData::default())),
            forge_promos_cache: Arc::new(RwLock::new(CachedData::default())),
            neoforge_versions_cache: Arc::new(RwLock::new(CachedData::default())),
        }
    }

    /// Fetches all Minecraft release versions from Mojang manifest.
    pub async fn get_minecraft_versions(&self, include_snapshots: bool) -> Result<Vec<MinecraftVersionInfo>, String> {
        {
            let cache = self.mc_versions_cache.read().await;
            if let Some(cached) = cache.get() {
                return Ok(filter_mc_versions(cached, include_snapshots));
            }
        }

        if !ssrf::is_safe_cdn_url(MOJANG_MANIFEST_URL) {
            return Err("Mojang manifest URL failed SSRF check".to_string());
        }

        let resp = self
            .client
            .get(MOJANG_MANIFEST_URL)
            .send()
            .await
            .map_err(|e| format!("Failed to fetch Mojang manifest: {e}"))?;

        if !resp.status().is_success() {
            return Err(format!("Mojang manifest returned HTTP {}", resp.status()));
        }

        #[derive(Deserialize)]
        struct MojangManifest {
            versions: Vec<MojangVersionEntry>,
        }
        #[derive(Deserialize)]
        struct MojangVersionEntry {
            id: String,
            r#type: String,
            #[serde(rename = "releaseTime")]
            release_time: String,
        }

        let manifest: MojangManifest = resp
            .json()
            .await
            .map_err(|e| format!("Failed to parse Mojang manifest: {e}"))?;

        let parsed: Vec<MinecraftVersionInfo> = manifest
            .versions
            .into_iter()
            .map(|v| MinecraftVersionInfo {
                id: v.id,
                r#type: v.r#type,
                release_time: v.release_time,
            })
            .collect();

        {
            let mut cache = self.mc_versions_cache.write().await;
            cache.set(parsed.clone());
        }

        Ok(filter_mc_versions(parsed, include_snapshots))
    }

    /// Fetches available loader versions and identifies the recommended stable build
    /// for the given loader type and Minecraft version.
    pub async fn get_loader_versions(
        &self,
        loader: &str,
        mc_version: &str,
    ) -> Result<LoaderVersionResult, String> {
        let loader_lower = loader.trim().to_ascii_lowercase();
        match loader_lower.as_str() {
            "fabric" => self.get_fabric_loader_versions(mc_version).await,
            "quilt" => self.get_quilt_loader_versions(mc_version).await,
            "forge" => self.get_forge_loader_versions(mc_version).await,
            "neoforge" => self.get_neoforge_loader_versions(mc_version).await,
            "vanilla" => Ok(LoaderVersionResult {
                loader: "vanilla".to_string(),
                mc_version: mc_version.to_string(),
                recommended: Some(String::new()),
                versions: Vec::new(),
            }),
            other => Err(format!("Unknown loader type: {other}")),
        }
    }

    async fn get_fabric_loader_versions(&self, mc_version: &str) -> Result<LoaderVersionResult, String> {
        let url = format!("{FABRIC_META_URL}/versions/loader/{mc_version}");
        if !ssrf::is_safe_cdn_url(&url) {
            return Err("Fabric meta URL failed SSRF check".to_string());
        }

        let resp = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| format!("Failed to fetch Fabric loader versions: {e}"))?;

        if !resp.status().is_success() {
            return Err(format!("Fabric meta returned HTTP {}", resp.status()));
        }

        let items: Vec<serde_json::Value> = resp
            .json()
            .await
            .map_err(|e| format!("Invalid Fabric JSON: {e}"))?;

        let mut versions = Vec::new();
        for item in items {
            if let Some(ver) = item
                .get("loader")
                .and_then(|l| l.get("version"))
                .and_then(|v| v.as_str())
            {
                versions.push(ver.to_string());
            }
        }

        let recommended = versions.first().cloned();
        Ok(LoaderVersionResult {
            loader: "fabric".to_string(),
            mc_version: mc_version.to_string(),
            recommended,
            versions,
        })
    }

    async fn get_quilt_loader_versions(&self, mc_version: &str) -> Result<LoaderVersionResult, String> {
        let url = format!("{QUILT_META_URL}/versions/loader/{mc_version}");
        if !ssrf::is_safe_cdn_url(&url) {
            return Err("Quilt meta URL failed SSRF check".to_string());
        }

        let resp = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| format!("Failed to fetch Quilt loader versions: {e}"))?;

        if !resp.status().is_success() {
            return Err(format!("Quilt meta returned HTTP {}", resp.status()));
        }

        let items: Vec<serde_json::Value> = resp
            .json()
            .await
            .map_err(|e| format!("Invalid Quilt JSON: {e}"))?;

        let mut versions = Vec::new();
        for item in items {
            if let Some(ver) = item
                .get("loader")
                .and_then(|l| l.get("version"))
                .and_then(|v| v.as_str())
            {
                versions.push(ver.to_string());
            }
        }

        let recommended = versions.first().cloned();
        Ok(LoaderVersionResult {
            loader: "quilt".to_string(),
            mc_version: mc_version.to_string(),
            recommended,
            versions,
        })
    }

    async fn get_forge_loader_versions(&self, mc_version: &str) -> Result<LoaderVersionResult, String> {
        let promos = self.fetch_forge_promotions().await?;
        let promos_obj = promos.get("promos").and_then(|p| p.as_object());

        let rec_key = format!("{mc_version}-recommended");
        let lat_key = format!("{mc_version}-latest");

        let recommended = promos_obj
            .and_then(|p| p.get(&rec_key).or_else(|| p.get(&lat_key)))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        let mut versions = Vec::new();
        if let Some(rec) = &recommended {
            versions.push(rec.clone());
        }
        if let Some(lat) = promos_obj.and_then(|p| p.get(&lat_key)).and_then(|v| v.as_str()) {
            if !versions.contains(&lat.to_string()) {
                versions.push(lat.to_string());
            }
        }

        Ok(LoaderVersionResult {
            loader: "forge".to_string(),
            mc_version: mc_version.to_string(),
            recommended: recommended.clone().or_else(|| versions.first().cloned()),
            versions,
        })
    }

    async fn fetch_forge_promotions(&self) -> Result<serde_json::Value, String> {
        {
            let cache = self.forge_promos_cache.read().await;
            if let Some(cached) = cache.get() {
                return Ok(cached);
            }
        }

        if !ssrf::is_safe_cdn_url(FORGE_PROMOTIONS_URL) {
            return Err("Forge promotions URL failed SSRF check".to_string());
        }

        let resp = self
            .client
            .get(FORGE_PROMOTIONS_URL)
            .send()
            .await
            .map_err(|e| format!("Failed to fetch Forge promotions: {e}"))?;

        if !resp.status().is_success() {
            return Err(format!("Forge promotions returned HTTP {}", resp.status()));
        }

        let json: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| format!("Failed to parse Forge promotions JSON: {e}"))?;

        {
            let mut cache = self.forge_promos_cache.write().await;
            cache.set(json.clone());
        }

        Ok(json)
    }

    async fn get_neoforge_loader_versions(&self, mc_version: &str) -> Result<LoaderVersionResult, String> {
        let all_versions = self.fetch_neoforge_versions().await?;

        let prefix = neoforge_prefix_for_mc(mc_version);

        let mut matching: Vec<String> = all_versions
            .into_iter()
            .filter(|v| v.starts_with(&prefix))
            .collect();

        matching.sort_by(|a, b| compare_versions(b, a));

        let recommended = matching
            .iter()
            .find(|v| !v.contains("beta") && !v.contains("alpha"))
            .cloned()
            .or_else(|| matching.first().cloned());

        Ok(LoaderVersionResult {
            loader: "neoforge".to_string(),
            mc_version: mc_version.to_string(),
            recommended,
            versions: matching,
        })
    }

    async fn fetch_neoforge_versions(&self) -> Result<Vec<String>, String> {
        {
            let cache = self.neoforge_versions_cache.read().await;
            if let Some(cached) = cache.get() {
                return Ok(cached);
            }
        }

        if !ssrf::is_safe_cdn_url(NEOFORGE_MAVEN_METADATA_URL) {
            return Err("NeoForge Maven metadata URL failed SSRF check".to_string());
        }

        let resp = self
            .client
            .get(NEOFORGE_MAVEN_METADATA_URL)
            .send()
            .await
            .map_err(|e| format!("Failed to fetch NeoForge Maven metadata: {e}"))?;

        if !resp.status().is_success() {
            return Err(format!("NeoForge Maven metadata returned HTTP {}", resp.status()));
        }

        let xml = resp
            .text()
            .await
            .map_err(|e| format!("Failed to read NeoForge Maven XML: {e}"))?;

        let mut versions = Vec::new();
        for line in xml.lines() {
            let trimmed = line.trim();
            if let Some(rest) = trimmed.strip_prefix("<version>") {
                if let Some(ver) = rest.strip_suffix("</version>") {
                    let v = ver.trim();
                    if !v.is_empty() {
                        versions.push(v.to_string());
                    }
                }
            }
        }

        {
            let mut cache = self.neoforge_versions_cache.write().await;
            cache.set(versions.clone());
        }

        Ok(versions)
    }
}

fn filter_mc_versions(versions: Vec<MinecraftVersionInfo>, include_snapshots: bool) -> Vec<MinecraftVersionInfo> {
    if include_snapshots {
        versions
    } else {
        versions.into_iter().filter(|v| v.r#type == "release").collect()
    }
}

fn neoforge_prefix_for_mc(mc_version: &str) -> String {
    let clean = mc_version.trim();
    if clean.starts_with("1.") {
        let rest = &clean[2..]; // e.g. "20.4" or "21.1"
        format!("{rest}.")
    } else {
        format!("{clean}.")
    }
}

fn compare_versions(a: &str, b: &str) -> std::cmp::Ordering {
    let a_parts: Vec<&str> = a.split(['.', '-']).collect();
    let b_parts: Vec<&str> = b.split(['.', '-']).collect();

    for (ap, bp) in a_parts.iter().zip(b_parts.iter()) {
        if let (Ok(an), Ok(bn)) = (ap.parse::<u64>(), bp.parse::<u64>()) {
            if an != bn {
                return an.cmp(&bn);
            }
        } else if ap != bp {
            return ap.cmp(bp);
        }
    }
    a.len().cmp(&b.len())
}
