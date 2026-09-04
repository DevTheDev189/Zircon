//! Tier 2 "Host for Friends" Co-Op Session & P2P Mod Sync Engine.
//!
//! Provides direct Peer-to-Peer (Host -> Friend) streaming for mod synchronization:
//! - Host exposes `GET /p2p/manifest` and `GET /p2p/mod/:sha1` via an embedded Axum server.
//! - Guest queries the manifest, checks local cache (`~/.mcmanager/cache/mods`), and streams only missing deltas.
//! - Security Model:
//!   - Catalog mods (Modrinth / CurseForge) sync silently and automatically.
//!   - Unverified loose/custom `.jar` files are blocked from auto-sync by default.
//!   - If `allow_unverified_p2p_mods` is enabled in Settings, the guest displays an explicit approval modal.
//!   - Dedicated servers permanently ban unverified mods (this toggle never applies to dedicated servers).
//! - Zero-Cost Rendezvous: Cloudflare Worker + Workers KV (`COOP_SESSIONS`) for 6-character Join Codes (`ZK-XXXX`).

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use axum::extract::{Path as AxumPath, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::get;
use axum::Router;
use serde::{Deserialize, Serialize};
use sha1::{Digest, Sha1};
use tokio::io::AsyncWriteExt;
use tokio::sync::RwLock;
use tracing::{info, warn};

use zircon_core::api::modrinth::ModrinthApiClient;

use crate::error::LauncherError;
use crate::offline::OfflineInstance;
use crate::sync::mod_sync::{HashVerifier, ProgressListener};

// ---------------------------------------------------------------------------
// Data Models
// ---------------------------------------------------------------------------

/// UPnP NAT traversal status for zero-config multiplayer.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct UpnpStatus {
    pub available: bool,
    pub game_port_mapped: bool,
    pub p2p_port_mapped: bool,
    pub external_ip: Option<String>,
    pub message: String,
}

/// Active Host for Friends session status.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CoopSessionInfo {
    pub instance_id: String,
    pub instance_name: String,
    pub world_name: String,
    pub join_code: String,
    pub game_port: u16,
    pub p2p_port: u16,
    pub started_at: i64,
    pub active: bool,
    pub upnp: UpnpStatus,
}

/// Metadata for a single mod available in the host instance.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct P2PModEntry {
    pub filename: String,
    pub file_size: u64,
    pub sha1: String,
    pub is_custom: bool,
}

/// P2P mod manifest advertised by the host.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct P2PManifest {
    pub instance_id: String,
    pub instance_name: String,
    pub mc_version: String,
    pub loader_type: String,
    pub loader_version: String,
    pub mods: Vec<P2PModEntry>,
}

/// Preflight comparison returned to the guest before streaming begins.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct P2PPreflightResult {
    pub host_address: String,
    pub p2p_port: u16,
    pub game_port: u16,
    pub manifest: P2PManifest,
    pub missing_mods: Vec<P2PModEntry>,
    pub custom_mods: Vec<P2PModEntry>,
    pub total_download_bytes: u64,
    pub requires_approval: bool,
    pub unverified_allowed_by_settings: bool,
}

/// Outcome of a P2P delta synchronization.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct P2PSyncResult {
    pub downloaded_count: usize,
    pub downloaded_bytes: u64,
    pub cached_count: usize,
    pub skipped_custom_count: usize,
}

/// Rendezvous session payload stored in Cloudflare Worker KV.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CoopRendezvousSession {
    pub join_code: String,
    pub host: String,
    pub game_port: u16,
    pub p2p_port: u16,
    pub instance_name: String,
    pub mc_version: String,
    pub loader_type: String,
    pub created_at: i64,
}

// ---------------------------------------------------------------------------
// Join Code Generator & Address Parsing
// ---------------------------------------------------------------------------

/// Generates a friendly 6-character Join Code formatted as `ZK-XXXX`.
pub fn generate_join_code() -> String {
    let simple = uuid::Uuid::new_v4().simple().to_string();
    format!("ZK-{}", &simple[..4].to_ascii_uppercase())
}

/// Validates whether a string is a canonical `ZK-XXXX` Join Code.
pub fn is_valid_join_code(code: &str) -> bool {
    let trimmed = code.trim();
    trimmed.len() == 7
        && trimmed.starts_with("ZK-")
        && trimmed[3..].chars().all(|c| c.is_ascii_alphanumeric())
}

/// Parses an input string as either a Join Code (`ZK-XXXX`) or a direct `host:port` address.
pub fn parse_code_or_address(input: &str) -> (Option<String>, Option<String>, u16, u16) {
    let trimmed = input.trim();
    if is_valid_join_code(trimmed) {
        return (Some(trimmed.to_ascii_uppercase()), None, 25565, 25566);
    }
    // Check if input is a direct host[:port]
    let (host, game_port) = if let Some((h, p)) = trimmed.rsplit_once(':') {
        let port = p.parse::<u16>().unwrap_or(25565);
        (h.to_string(), port)
    } else {
        (trimmed.to_string(), 25565)
    };
    let p2p_port = game_port.saturating_add(1);
    (None, Some(host), game_port, p2p_port)
}

// ---------------------------------------------------------------------------
// Manifest Generation
// ---------------------------------------------------------------------------

/// Scans the instance `mods/` directory and generates a cryptographically checked P2PManifest.
pub async fn generate_p2p_manifest(
    instance: &OfflineInstance,
    mods_dir: &Path,
) -> Result<P2PManifest, LauncherError> {
    let mut mods = Vec::new();
    if !mods_dir.is_dir() {
        return Ok(P2PManifest {
            instance_id: instance.id.clone(),
            instance_name: instance.name.clone(),
            mc_version: instance.minecraft_version.clone(),
            loader_type: instance.mod_loader.r#type.clone(),
            loader_version: instance.mod_loader.version.clone(),
            mods: Vec::new(),
        });
    }

    let mut entries = Vec::new();
    for entry in std::fs::read_dir(mods_dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_file() && HashVerifier::is_mod_jar(&entry.file_name().to_string_lossy()) {
            let filename = entry.file_name().to_string_lossy().to_string();
            let size = entry.metadata()?.len();
            if let Ok(sha1) = HashVerifier::sha1_file(&path) {
                entries.push((filename, size, sha1));
            }
        }
    }

    // Batch-verify SHA-1 hashes against Modrinth
    let sha1s: Vec<String> = entries.iter().map(|(_, _, s)| s.clone()).collect();
    let mut catalog_verified: HashSet<String> = HashSet::new();
    if !sha1s.is_empty() {
        let client = ModrinthApiClient::new();
        if let Ok(found) = client.verify_hashes(&sha1s).await {
            catalog_verified.extend(found.into_keys());
        }
    }

    for (filename, file_size, sha1) in entries {
        let is_custom = !catalog_verified.contains(&sha1);
        mods.push(P2PModEntry {
            filename,
            file_size,
            sha1,
            is_custom,
        });
    }

    // Sort alphabetically for deterministic ordering
    mods.sort_by(|a, b| a.filename.cmp(&b.filename));

    Ok(P2PManifest {
        instance_id: instance.id.clone(),
        instance_name: instance.name.clone(),
        mc_version: instance.minecraft_version.clone(),
        loader_type: instance.mod_loader.r#type.clone(),
        loader_version: instance.mod_loader.version.clone(),
        mods,
    })
}

// ---------------------------------------------------------------------------
// Host Axum P2P HTTP Server
// ---------------------------------------------------------------------------

#[derive(Clone)]
struct P2PServerState {
    manifest: Arc<RwLock<P2PManifest>>,
    mods_dir: PathBuf,
}

/// Starts the embedded Axum HTTP server on the host to serve manifest and mods.
pub async fn start_p2p_server(
    mods_dir: PathBuf,
    manifest: P2PManifest,
    preferred_port: u16,
) -> Result<(u16, tokio::sync::oneshot::Sender<()>), LauncherError> {
    let state = P2PServerState {
        manifest: Arc::new(RwLock::new(manifest)),
        mods_dir,
    };

    let app = Router::new()
        .route("/p2p/manifest", get(handle_get_manifest))
        .route("/p2p/mod/:sha1", get(handle_get_mod))
        .with_state(state);

    // Try preferred port first; fallback to 0 (system-assigned dynamic port)
    let listener = match tokio::net::TcpListener::bind(format!("0.0.0.0:{preferred_port}")).await {
        Ok(l) => l,
        Err(_) => tokio::net::TcpListener::bind("0.0.0.0:0").await.map_err(LauncherError::Io)?,
    };

    let bound_port = listener
        .local_addr()
        .map_err(LauncherError::Io)?
        .port();

    let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel::<()>();

    tokio::spawn(async move {
        info!("P2P HTTP server listening on port {}", bound_port);
        let server = axum::serve(listener, app);
        let graceful = server.with_graceful_shutdown(async move {
            let _ = shutdown_rx.await;
            info!("P2P HTTP server received shutdown signal");
        });
        if let Err(e) = graceful.await {
            warn!("P2P HTTP server encountered error: {e}");
        }
    });

    Ok((bound_port, shutdown_tx))
}

async fn handle_get_manifest(State(state): State<P2PServerState>) -> impl IntoResponse {
    let manifest = state.manifest.read().await;
    axum::Json(manifest.clone())
}

async fn handle_get_mod(
    State(state): State<P2PServerState>,
    AxumPath(sha1): AxumPath<String>,
) -> Result<impl IntoResponse, (StatusCode, &'static str)> {
    let clean_sha1 = sha1.trim().to_ascii_lowercase();
    // Security check: strict 40-character hex string
    if clean_sha1.len() != 40 || !clean_sha1.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err((StatusCode::BAD_REQUEST, "Invalid SHA-1 digest format"));
    }

    let manifest = state.manifest.read().await;
    let Some(entry) = manifest.mods.iter().find(|m| m.sha1.eq_ignore_ascii_case(&clean_sha1)) else {
        return Err((StatusCode::NOT_FOUND, "Mod not found in active session manifest"));
    };

    // Locate the file in mods_dir
    let file_path = state.mods_dir.join(&entry.filename);
    if !file_path.is_file() {
        return Err((StatusCode::NOT_FOUND, "Mod file missing on host"));
    }

    // Verify it doesn't escape mods_dir (Path Traversal check)
    if let (Ok(can_base), Ok(can_file)) = (state.mods_dir.canonicalize(), file_path.canonicalize()) {
        if !can_file.starts_with(&can_base) {
            return Err((StatusCode::FORBIDDEN, "Access denied"));
        }
    } else {
        return Err((StatusCode::INTERNAL_SERVER_ERROR, "Path resolution failure"));
    }

    let bytes = tokio::fs::read(&file_path)
        .await
        .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "Failed to read mod file"))?;

    let mut headers = axum::http::HeaderMap::new();
    headers.insert(
        axum::http::header::CONTENT_TYPE,
        axum::http::HeaderValue::from_static("application/java-archive"),
    );
    if let Ok(disp) = axum::http::HeaderValue::from_str(&format!("attachment; filename=\"{}\"", entry.filename)) {
        headers.insert(axum::http::header::CONTENT_DISPOSITION, disp);
    }

    Ok((headers, bytes))
}

// ---------------------------------------------------------------------------
// Guest P2P Sync Engine
// ---------------------------------------------------------------------------

/// Fetches the manifest advertised by the host P2P server.
pub async fn fetch_p2p_manifest(
    client: &reqwest::Client,
    host_p2p_url: &str,
) -> Result<P2PManifest, LauncherError> {
    let base = host_p2p_url.trim_end_matches('/');
    let url = format!("{base}/p2p/manifest");
    let resp = client
        .get(&url)
        .timeout(std::time::Duration::from_secs(10))
        .send()
        .await
        .map_err(|e| LauncherError::Network(format!("Failed to reach host P2P manifest: {e}")))?;

    if !resp.status().is_success() {
        return Err(LauncherError::Network(format!(
            "Host P2P manifest returned status {}",
            resp.status()
        )));
    }

    let manifest = resp
        .json::<P2PManifest>()
        .await
        .map_err(|e| LauncherError::InvalidInput(format!("Invalid manifest format: {e}")))?;

    Ok(manifest)
}

/// Preflights P2P sync against local instance and shared mod cache.
pub async fn preflight_p2p_sync(
    client: &reqwest::Client,
    host_address: &str,
    p2p_port: u16,
    game_port: u16,
    instance_mods_dir: &Path,
    cache_dir: &Path,
    allow_unverified_by_settings: bool,
) -> Result<P2PPreflightResult, LauncherError> {
    let host_p2p_url = format!("http://{host_address}:{p2p_port}");
    let manifest = fetch_p2p_manifest(client, &host_p2p_url).await?;

    let mut local_hashes: HashSet<String> = HashSet::new();

    // 1. Index local instance mods
    if instance_mods_dir.is_dir() {
        if let Ok(entries) = std::fs::read_dir(instance_mods_dir) {
            for entry in entries.flatten() {
                if entry.path().is_file() {
                    if let Ok(hash) = HashVerifier::sha1_file(&entry.path()) {
                        local_hashes.insert(hash);
                    }
                }
            }
        }
    }

    // 2. Index shared cache mods
    if cache_dir.is_dir() {
        if let Ok(entries) = std::fs::read_dir(cache_dir) {
            for entry in entries.flatten() {
                if entry.path().is_file() {
                    if let Ok(hash) = HashVerifier::sha1_file(&entry.path()) {
                        local_hashes.insert(hash);
                    }
                }
            }
        }
    }

    let mut missing_mods = Vec::new();
    let mut custom_mods = Vec::new();
    let mut total_download_bytes = 0u64;

    for m in &manifest.mods {
        if !local_hashes.contains(&m.sha1) {
            missing_mods.push(m.clone());
            total_download_bytes += m.file_size;
            if m.is_custom {
                custom_mods.push(m.clone());
            }
        }
    }

    let requires_approval = !custom_mods.is_empty();

    Ok(P2PPreflightResult {
        host_address: host_address.to_string(),
        p2p_port,
        game_port,
        manifest,
        missing_mods,
        custom_mods,
        total_download_bytes,
        requires_approval,
        unverified_allowed_by_settings: allow_unverified_by_settings,
    })
}

/// Executes P2P delta streaming from the host with strict hash and custom-mod verification.
pub async fn execute_p2p_sync(
    client: &reqwest::Client,
    host_p2p_url: &str,
    missing_mods: &[P2PModEntry],
    approved_custom_sha1s: &HashSet<String>,
    instance_mods_dir: &Path,
    cache_dir: &Path,
    allow_unverified_by_settings: bool,
    listener: Option<&dyn ProgressListener>,
) -> Result<P2PSyncResult, LauncherError> {
    std::fs::create_dir_all(instance_mods_dir)?;
    std::fs::create_dir_all(cache_dir)?;

    let base = host_p2p_url.trim_end_matches('/');
    let mut result = P2PSyncResult::default();
    let total_count = missing_mods.len();

    for (idx, mod_entry) in missing_mods.iter().enumerate() {
        // Enforce custom mod security gate
        if mod_entry.is_custom {
            if !allow_unverified_by_settings || !approved_custom_sha1s.contains(&mod_entry.sha1) {
                warn!(
                    "Skipping unverified custom mod {}: not approved or developer mode disabled",
                    mod_entry.filename
                );
                result.skipped_custom_count += 1;
                continue;
            }
        }

        let cached_path = cache_dir.join(format!("{}.jar", mod_entry.sha1));
        let instance_target = instance_mods_dir.join(&mod_entry.filename);

        // If present in cache, copy directly
        if cached_path.is_file() && HashVerifier::sha1_file(&cached_path).unwrap_or_default() == mod_entry.sha1 {
            std::fs::copy(&cached_path, &instance_target)?;
            result.cached_count += 1;
            continue;
        }

        // Otherwise, stream from host
        let url = format!("{base}/p2p/mod/{}", mod_entry.sha1);
        if let Some(l) = listener {
            l.on_status(&format!(
                "Downloading {} ({}/{}) from friend...",
                mod_entry.filename,
                idx + 1,
                total_count
            ));
        }

        let temp_download = cache_dir.join(format!(".{}.tmp", mod_entry.sha1));
        let mut resp = client
            .get(&url)
            .send()
            .await
            .map_err(|e| LauncherError::Network(format!("Failed to download {}: {e}", mod_entry.filename)))?;

        if !resp.status().is_success() {
            let _ = std::fs::remove_file(&temp_download);
            return Err(LauncherError::Network(format!(
                "Host returned status {} for mod {}",
                resp.status(),
                mod_entry.filename
            )));
        }

        let mut file = tokio::fs::File::create(&temp_download)
            .await
            .map_err(LauncherError::Io)?;
        let mut hasher = Sha1::new();
        let mut downloaded_size = 0u64;

        while let Some(chunk_res) = resp.chunk().await.transpose() {
            let chunk = chunk_res.map_err(|e| LauncherError::Network(e.to_string()))?;
            file.write_all(&chunk).await.map_err(LauncherError::Io)?;
            hasher.update(&chunk);
            downloaded_size += chunk.len() as u64;
        }
        file.flush().await.map_err(LauncherError::Io)?;
        drop(file);

        let actual_sha1 = hex::encode(hasher.finalize());
        if !actual_sha1.eq_ignore_ascii_case(&mod_entry.sha1) {
            let _ = std::fs::remove_file(&temp_download);
            return Err(LauncherError::InvalidInput(format!(
                "Checksum mismatch for {}: expected {}, received {}",
                mod_entry.filename, mod_entry.sha1, actual_sha1
            )));
        }

        // Commit to cache and copy to instance
        std::fs::rename(&temp_download, &cached_path)?;
        std::fs::copy(&cached_path, &instance_target)?;

        result.downloaded_count += 1;
        result.downloaded_bytes += downloaded_size;
    }

    Ok(result)
}

// ---------------------------------------------------------------------------
// Cloudflare Worker Rendezvous Client
// ---------------------------------------------------------------------------

/// Registers an active Co-Op session with Cloudflare Workers KV.
pub async fn register_coop_session(
    worker_url: &str,
    session: &CoopRendezvousSession,
) -> Result<(), LauncherError> {
    let client = reqwest::Client::new();
    let url = format!("{}/session", worker_url.trim_end_matches('/'));
    let resp = client
        .post(&url)
        .json(session)
        .timeout(std::time::Duration::from_secs(10))
        .send()
        .await
        .map_err(|e| LauncherError::Network(format!("Rendezvous registration failed: {e}")))?;

    if !resp.status().is_success() {
        return Err(LauncherError::Network(format!(
            "Rendezvous registration failed with status {}",
            resp.status()
        )));
    }
    Ok(())
}

/// Queries Cloudflare Workers KV for a 6-character Join Code.
pub async fn resolve_coop_session(
    worker_url: &str,
    join_code: &str,
) -> Result<CoopRendezvousSession, LauncherError> {
    let client = reqwest::Client::new();
    let url = format!("{}/session/{}", worker_url.trim_end_matches('/'), join_code);
    let resp = client
        .get(&url)
        .timeout(std::time::Duration::from_secs(10))
        .send()
        .await
        .map_err(|e| LauncherError::Network(format!("Failed to resolve Join Code: {e}")))?;

    if resp.status() == reqwest::StatusCode::NOT_FOUND {
        return Err(LauncherError::InvalidInput(format!(
            "Join Code \"{join_code}\" not found or expired"
        )));
    }
    if !resp.status().is_success() {
        return Err(LauncherError::Network(format!(
            "Rendezvous lookup returned status {}",
            resp.status()
        )));
    }

    let session = resp
        .json::<CoopRendezvousSession>()
        .await
        .map_err(|e| LauncherError::InvalidInput(format!("Invalid rendezvous response: {e}")))?;

    Ok(session)
}

/// Deletes a session from Cloudflare Workers KV upon host shutdown.
pub async fn delete_coop_session(worker_url: &str, join_code: &str) -> Result<(), LauncherError> {
    let client = reqwest::Client::new();
    let url = format!("{}/session/{}", worker_url.trim_end_matches('/'), join_code);
    let _ = client
        .delete(&url)
        .timeout(std::time::Duration::from_secs(5))
        .send()
        .await;
    Ok(())
}

// ---------------------------------------------------------------------------
// UPnP IGD Automatic NAT Traversal Engine
// ---------------------------------------------------------------------------

/// Determines the host's local IP address on the network interface facing the gateway router.
fn get_local_ip_facing(gateway_addr: std::net::SocketAddr) -> Option<std::net::IpAddr> {
    let bind_addr = if gateway_addr.is_ipv6() { "[::]:0" } else { "0.0.0.0:0" };
    let socket = std::net::UdpSocket::bind(bind_addr).ok()?;
    socket.connect(gateway_addr).ok()?;
    socket.local_addr().ok().map(|s| s.ip())
}

/// Discovers the local UPnP gateway device and punches TCP port mappings for game and P2P mod sync.
/// Operates with a 2.5-second discovery timeout to ensure that host launch never stalls on strict networks.
pub async fn open_upnp_ports(game_port: u16, p2p_port: u16) -> UpnpStatus {
    info!("Starting UPnP gateway discovery (timeout 2.5s)...");

    let join_handle = tokio::task::spawn_blocking(move || {
        let options = igd_next::SearchOptions {
            timeout: Some(std::time::Duration::from_millis(2500)),
            ..Default::default()
        };

        let gateway = match igd_next::search_gateway(options) {
            Ok(gw) => gw,
            Err(e) => {
                info!("UPnP gateway discovery inactive: {e}");
                return UpnpStatus {
                    available: false,
                    game_port_mapped: false,
                    p2p_port_mapped: false,
                    external_ip: None,
                    message: "UPnP is disabled or unavailable on this router".to_string(),
                };
            }
        };

        let local_ip = match get_local_ip_facing(gateway.addr) {
            Some(ip) => ip,
            None => {
                warn!("Found UPnP gateway at {} but failed to resolve local interface IP", gateway.addr);
                return UpnpStatus {
                    available: false,
                    game_port_mapped: false,
                    p2p_port_mapped: false,
                    external_ip: None,
                    message: "Could not resolve local network interface IP".to_string(),
                };
            }
        };

        let external_ip = match gateway.get_external_ip() {
            Ok(ip) => {
                info!("Discovered router external public IP via UPnP: {ip}");
                Some(ip.to_string())
            }
            Err(e) => {
                warn!("Could not retrieve external IP from UPnP gateway: {e}");
                None
            }
        };

        // 1. Map Minecraft game port
        let game_sock = std::net::SocketAddr::new(local_ip, game_port);
        let game_mapped = match gateway.add_port(
            igd_next::PortMappingProtocol::TCP,
            game_port,
            game_sock,
            0,
            "Zircon Co-Op Game",
        ) {
            Ok(_) => {
                info!("Successfully mapped UPnP external TCP {game_port} -> {game_sock}");
                true
            }
            Err(e) => {
                warn!("Failed to map UPnP game port {game_port}: {e}");
                false
            }
        };

        // 2. Map P2P mod sync port
        let p2p_sock = std::net::SocketAddr::new(local_ip, p2p_port);
        let p2p_mapped = match gateway.add_port(
            igd_next::PortMappingProtocol::TCP,
            p2p_port,
            p2p_sock,
            0,
            "Zircon P2P Mod Sync",
        ) {
            Ok(_) => {
                info!("Successfully mapped UPnP external TCP {p2p_port} -> {p2p_sock}");
                true
            }
            Err(e) => {
                warn!("Failed to map UPnP P2P port {p2p_port}: {e}");
                false
            }
        };

        let available = game_mapped || p2p_mapped;
        let message = if game_mapped && p2p_mapped {
            format!("Ports {game_port} & {p2p_port} opened automatically on your router. Friends can join seamlessly!")
        } else if available {
            format!("Partial UPnP port mapping: game port={game_mapped}, p2p port={p2p_mapped}")
        } else {
            "UPnP gateway found but port mappings were rejected by router".to_string()
        };

        UpnpStatus {
            available,
            game_port_mapped: game_mapped,
            p2p_port_mapped: p2p_mapped,
            external_ip,
            message,
        }
    });

    match tokio::time::timeout(std::time::Duration::from_millis(3500), join_handle).await {
        Ok(Ok(status)) => status,
        Ok(Err(join_err)) => {
            warn!("UPnP worker task error: {join_err}");
            UpnpStatus {
                available: false,
                game_port_mapped: false,
                p2p_port_mapped: false,
                external_ip: None,
                message: "UPnP task execution failed".to_string(),
            }
        }
        Err(_) => {
            warn!("UPnP discovery timed out after 3.5s limit");
            UpnpStatus {
                available: false,
                game_port_mapped: false,
                p2p_port_mapped: false,
                external_ip: None,
                message: "UPnP gateway search timed out".to_string(),
            }
        }
    }
}

/// Releases the UPnP port mappings from the router gateway upon session termination.
pub async fn close_upnp_ports(game_port: u16, p2p_port: u16) {
    info!("Releasing UPnP port mappings for game port {game_port} and P2P port {p2p_port}...");
    let _ = tokio::task::spawn_blocking(move || {
        let options = igd_next::SearchOptions {
            timeout: Some(std::time::Duration::from_millis(1500)),
            ..Default::default()
        };

        if let Ok(gateway) = igd_next::search_gateway(options) {
            let _ = gateway.remove_port(igd_next::PortMappingProtocol::TCP, game_port);
            let _ = gateway.remove_port(igd_next::PortMappingProtocol::TCP, p2p_port);
            info!("Released UPnP port mappings for ports {game_port} & {p2p_port}");
        }
    }).await;
}

// ---------------------------------------------------------------------------
// Unit Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn join_code_format_is_valid() {
        let code = generate_join_code();
        assert!(code.starts_with("ZK-"));
        assert_eq!(7, code.len());
        assert!(is_valid_join_code(&code));
    }

    #[test]
    fn parses_code_and_direct_address() {
        let (code, host, _game_port, _p2p_port) = parse_code_or_address("ZK-9A12");
        assert_eq!(code.as_deref(), Some("ZK-9A12"));
        assert_eq!(host, None);

        let (code, host, game_port, p2p_port) = parse_code_or_address("192.168.1.100:25565");
        assert_eq!(code, None);
        assert_eq!(host.as_deref(), Some("192.168.1.100"));
        assert_eq!(game_port, 25565);
        assert_eq!(p2p_port, 25566);
    }

    #[tokio::test]
    async fn p2p_server_serves_manifest_and_mod() {
        let temp = std::env::temp_dir().join(format!("zircon-p2p-test-{}", uuid::Uuid::new_v4().simple()));
        std::fs::create_dir_all(&temp).unwrap();

        let dummy_mod = temp.join("test-mod-1.0.jar");
        let content = b"PK\x03\x04dummy-jar-bytes-for-p2p-test";
        std::fs::write(&dummy_mod, content).unwrap();

        let mut hasher = Sha1::new();
        hasher.update(content);
        let sha1 = hex::encode(hasher.finalize());

        let manifest = P2PManifest {
            instance_id: "inst-1".to_string(),
            instance_name: "Test Instance".to_string(),
            mc_version: "1.21.1".to_string(),
            loader_type: "fabric".to_string(),
            loader_version: "0.16.0".to_string(),
            mods: vec![P2PModEntry {
                filename: "test-mod-1.0.jar".to_string(),
                file_size: content.len() as u64,
                sha1: sha1.clone(),
                is_custom: false,
            }],
        };

        let (port, shutdown) = start_p2p_server(temp.clone(), manifest, 0).await.unwrap();
        assert!(port > 0);

        let client = reqwest::Client::new();
        let fetched = fetch_p2p_manifest(&client, &format!("http://127.0.0.1:{port}"))
            .await
            .unwrap();
        assert_eq!(fetched.instance_id, "inst-1");
        assert_eq!(fetched.mods.len(), 1);
        assert_eq!(fetched.mods[0].sha1, sha1);

        // Download mod
        let mod_url = format!("http://127.0.0.1:{port}/p2p/mod/{sha1}");
        let downloaded = client.get(&mod_url).send().await.unwrap().bytes().await.unwrap();
        assert_eq!(&downloaded[..], content);

        // Path traversal / missing hash rejected
        let missing = client
            .get(&format!("http://127.0.0.1:{port}/p2p/mod/0000000000000000000000000000000000000000"))
            .send()
            .await
            .unwrap();
        assert_eq!(missing.status(), StatusCode::NOT_FOUND);

        let _ = shutdown.send(());
        let _ = std::fs::remove_dir_all(&temp);
    }

    #[test]
    fn upnp_status_serialization_camel_case() {
        let status = UpnpStatus {
            available: true,
            game_port_mapped: true,
            p2p_port_mapped: true,
            external_ip: Some("198.51.100.1".to_string()),
            message: "Ports opened successfully".to_string(),
        };

        let json = serde_json::to_string(&status).unwrap();
        assert!(json.contains("\"available\":true"));
        assert!(json.contains("\"gamePortMapped\":true"));
        assert!(json.contains("\"p2pPortMapped\":true"));
        assert!(json.contains("\"externalIp\":\"198.51.100.1\""));
        assert!(json.contains("\"message\":\"Ports opened successfully\""));

        let deserialized: UpnpStatus = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, status);
    }

    #[tokio::test]
    async fn upnp_discovery_fails_gracefully_without_panicking() {
        // In local automated test environments without UPnP IGD, this must return
        // quickly and gracefully without panicking or hanging.
        let status = open_upnp_ports(25565, 25566).await;
        // Result must be a structured UpnpStatus
        assert!(!status.message.is_empty());
        if !status.available {
            assert!(!status.game_port_mapped);
            assert!(!status.p2p_port_mapped);
        }

        // Clean release must also never panic
        close_upnp_ports(25565, 25566).await;
    }
}

