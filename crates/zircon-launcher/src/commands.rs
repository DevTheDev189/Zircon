//! Tauri v2 IPC command bindings exposed to the Vue 3 webview.
//!
//! Every Phase 4 engine (Microsoft auth, classpath resolver, sync engines,
//! offline instances) is surfaced here as a `#[tauri::command]`. Long-running
//! flows (`launch_server`, `launch_offline_instance`) emit `launch-status`,
//! `launch-progress`, `game-output` and `game-status` events to the frontend
//! instead of blocking the webview.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex as StdMutex};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use sha1::{Digest, Sha1};
use sha2::Sha256;
use tauri::{AppHandle, Emitter, Manager, State};
use tokio::process::Child;
use tokio::sync::Mutex as AsyncMutex;

use zircon_core::api::curseforge::CurseForgeApiClient;
use zircon_core::api::modrinth::{ModrinthApiClient, ModrinthSearchHit};
use zircon_core::crypto::signing;
use zircon_core::model::{BillOfMaterials, ModLoaderInfo, ModLoaderType}; // z0

use crate::auth::msa::MicrosoftAuthService;
use crate::auth::session::SessionData;
use crate::error::LauncherError;
use crate::launch::classpath::MinecraftClasspathBuilder;
use crate::launch::java::JavaRuntimeSelector;
use crate::launch::runner::MinecraftRunner;
use crate::offline::{OfflineInstance, OfflineInstanceManager};
use crate::pack_selection::{ClientPackManager, PackSelection};
use crate::servers;
use crate::settings::{self, load_settings, LauncherSettings};
use crate::skin::{MojangSkinService, SkinManager};
use crate::sync::mod_sync::{ModSyncEngine, ProgressListener};
use crate::sync::pack_sync::{PackProgressListener, PackSyncEngine};

/// Maps a launcher error to a user-facing string for `Result<_, String>`.
fn err_string(e: LauncherError) -> String {
    e.to_string()
}

// ---------------------------------------------------------------------------
// Shared application state
// ---------------------------------------------------------------------------

/// A running Minecraft client process.
pub struct RunningGame {
    pub id: u64,
    pub label: String,
    pub child: Child,
}

/// The player's answer to the shader opt-in prompt (possibly remembered for
/// future connections to the same server).
#[derive(Debug, Clone, Copy)]
pub struct ShaderChoice {
    pub enabled: bool,
    pub remember: bool,
}

/// Manages cooperative launch abort signaling across async launch steps.
#[derive(Debug, Default)]
pub struct LaunchCancellationHandle {
    aborted: std::sync::atomic::AtomicBool,
}

impl LaunchCancellationHandle {
    pub fn new() -> Self {
        Self {
            aborted: std::sync::atomic::AtomicBool::new(false),
        }
    }

    #[inline]
    pub fn reset(&self) {
        self.aborted.store(false, Ordering::SeqCst);
    }

    #[inline]
    pub fn request_abort(&self) {
        self.aborted.store(true, Ordering::SeqCst);
    }

    #[inline]
    pub fn is_aborted(&self) -> bool {
        self.aborted.load(Ordering::SeqCst)
    }

    #[inline]
    pub fn guard_active(&self) -> Result<(), LauncherError> {
        if self.is_aborted() {
            Err(LauncherError::InvalidInput("Launch cancelled by user.".to_string()))
        } else {
            Ok(())
        }
    }
}

/// Everything the Tauri commands need, managed once at startup.
pub struct LauncherState {
    pub auth: MicrosoftAuthService,
    pub session: AsyncMutex<Option<SessionData>>,
    pub classpath: MinecraftClasspathBuilder,
    pub sync_engine: ModSyncEngine,
    pub pack_sync: PackSyncEngine,
    pub modrinth: ModrinthApiClient,
    pub curse_forge: CurseForgeApiClient,
    pub mojang_skin: MojangSkinService,
    pub offline: OfflineInstanceManager,
    pub versions: Arc<zircon_core::api::versions::VersionService>,
    /// Plain client for BOM fetches, join-intent registration and downloads.
    pub http: reqwest::Client,
    pub running_game: AsyncMutex<Option<RunningGame>>,
    pub launch_cancellation: LaunchCancellationHandle,
    pub next_game_id: AtomicU64,
    /// In-flight shader opt-in prompts awaiting the webview's answer.
    pub shader_requests: AsyncMutex<HashMap<u64, tokio::sync::oneshot::Sender<ShaderChoice>>>,
    pub next_shader_request_id: AtomicU64,
    /// In-flight host-key rotation prompts awaiting the webview's decision
    /// (TOFU key lifecycle; see [`KeyMismatchPrompt`]).
    pub key_prompts: AsyncMutex<HashMap<u64, tokio::sync::oneshot::Sender<bool>>>,
    pub next_key_prompt_id: AtomicU64,
    pub settings: StdMutex<LauncherSettings>,
    pub discord_client: Arc<AsyncMutex<Option<crate::discord_rpc::DiscordRpcClient>>>,
    pub accounts: crate::auth::accounts::AccountManager,
    pub coop_session: AsyncMutex<Option<crate::coop::CoopSessionInfo>>,
    pub coop_p2p_shutdown: AsyncMutex<Option<tokio::sync::oneshot::Sender<()>>>,
}

impl Default for LauncherState {
    fn default() -> Self {
        Self::new()
    }
}

impl LauncherState {
    pub fn new() -> Self {
        let http = reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(10))
            .redirect(reqwest::redirect::Policy::limited(10))
            .build()
            .expect("failed to build launcher HTTP client");
        let auth = MicrosoftAuthService::new();
        let accounts = crate::auth::accounts::AccountManager::new();
        let auth_cached = auth.load_cached();
        let accounts_data = accounts.load_accounts(auth_cached.as_ref());
        let initial_session = if let Some(ref active_uuid) = accounts_data.active_uuid {
            accounts.load_session_for_uuid(active_uuid)
        } else {
            auth_cached
        }.filter(|s| !s.is_expired());
        let curse_forge_key = std::env::var("CURSEFORGE_API_KEY")
            .or_else(|_| std::env::var("MC_MANAGER_CURSEFORGE_API_KEY"))
            .unwrap_or_default();
        Self {
            auth,
            accounts,
            session: AsyncMutex::new(initial_session),
            classpath: MinecraftClasspathBuilder::new_default(),
            sync_engine: ModSyncEngine::new(),
            pack_sync: PackSyncEngine::new(),
            modrinth: ModrinthApiClient::new(),
            curse_forge: CurseForgeApiClient::new(curse_forge_key),
            mojang_skin: MojangSkinService::new(),
            offline: OfflineInstanceManager::new_default(),
            versions: Arc::new(zircon_core::api::versions::VersionService::new()),
            http,
            running_game: AsyncMutex::new(None),
            launch_cancellation: LaunchCancellationHandle::new(),
            next_game_id: AtomicU64::new(1),
            shader_requests: AsyncMutex::new(HashMap::new()),
            next_shader_request_id: AtomicU64::new(1),
            key_prompts: AsyncMutex::new(HashMap::new()),
            next_key_prompt_id: AtomicU64::new(1),
            settings: StdMutex::new(load_settings()),
            discord_client: Arc::new(AsyncMutex::new(None)),
            coop_session: AsyncMutex::new(None),
            coop_p2p_shutdown: AsyncMutex::new(None),
        }
    }
}



// ---------------------------------------------------------------------------
// Progress listeners → webview events
// ---------------------------------------------------------------------------

struct UiProgressListener {
    app: AppHandle,
}

impl ProgressListener for UiProgressListener {
    fn on_status(&self, message: &str) {
        let _ = self.app.emit("launch-status", message);
    }

    fn on_progress(&self, fraction: f64, detail: &str) {
        let _ = self.app.emit(
            "launch-progress",
            serde_json::json!({ "fraction": fraction, "detail": detail }),
        );
    }
}

struct UiPackListener {
    app: AppHandle,
}

impl PackProgressListener for UiPackListener {
    fn on_status(&self, message: &str) {
        let _ = self.app.emit("launch-status", message);
    }
}

fn emit_status(app: &AppHandle, message: impl AsRef<str>) {
    let _ = app.emit("launch-status", message.as_ref());
}

/// Notifies the frontend that the active skin changed so it can refresh the
/// sidebar avatar.
fn emit_skin_updated(app: &AppHandle) {
    let _ = app.emit("skin-updated", ());
}

fn game_output_emitter(app: &AppHandle) -> std::sync::Arc<dyn Fn(String) + Send + Sync> {
    let app = app.clone();
    std::sync::Arc::new(move |line: String| {
        let _ = app.emit("game-output", line);
    })
}

// ---------------------------------------------------------------------------
// Auth
// ---------------------------------------------------------------------------

/// Runs the interactive Microsoft OAuth PKCE login (opens the browser).
#[tauri::command]
pub async fn login_microsoft(state: State<'_, LauncherState>) -> Result<SessionData, String> {
    let session = state.auth.login().await.map_err(err_string)?;
    let _ = state.accounts.register_active_account(&session, None);
    *state.session.lock().await = Some(session.clone());
    Ok(session)
}

/// Lists all saved Microsoft accounts.
#[tauri::command]
pub async fn list_accounts(
    state: State<'_, LauncherState>,
) -> Result<Vec<crate::auth::accounts::AccountProfile>, String> {
    let current_session = state.session.lock().await.clone();
    let data = state.accounts.load_accounts(current_session.as_ref());
    Ok(data.accounts)
}

/// Switches active Microsoft account by UUID.
#[tauri::command]
pub async fn switch_account(
    state: State<'_, LauncherState>,
    uuid: String,
) -> Result<SessionData, String> {
    let mut session = state.accounts.switch_account(&uuid).map_err(err_string)?;
    if session.is_expired() {
        match state.auth.refresh(&session).await {
            Ok(fresh) => {
                let _ = state.accounts.store_session_credentials(&fresh);
                session = fresh;
            }
            Err(e) => {
                return Err(format!("Session expired and could not be refreshed: {e}"));
            }
        }
    }
    *state.session.lock().await = Some(session.clone());
    Ok(session)
}

/// Removes a saved Microsoft account by UUID.
#[tauri::command]
pub async fn remove_account(
    state: State<'_, LauncherState>,
    uuid: String,
) -> Result<Option<SessionData>, String> {
    let next_session = state.accounts.remove_account(&uuid).map_err(err_string)?;
    *state.session.lock().await = next_session.clone();
    Ok(next_session)
}

/// Loads the cached session, silently refreshing it when expired. Returns
/// `None` when there is no usable session (the UI must show the login overlay).
#[tauri::command]
pub async fn get_cached_session(
    state: State<'_, LauncherState>,
) -> Result<Option<SessionData>, String> {
    let accounts = state.accounts.load_accounts(None);
    let cached = if let Some(ref active_uuid) = accounts.active_uuid {
        state.accounts.load_session_for_uuid(active_uuid)
    } else {
        state.auth.load_cached()
    };

    let Some(cached) = cached else {
        *state.session.lock().await = None;
        return Ok(None);
    };

    if cached.is_expired() {
        match state.auth.refresh(&cached).await {
            Ok(fresh) => {
                let _ = state.accounts.store_session_credentials(&fresh);
                *state.session.lock().await = Some(fresh.clone());
                Ok(Some(fresh))
            }
            Err(_) => {
                // Expired with no working refresh token → force a fresh login.
                *state.session.lock().await = None;
                Ok(None)
            }
        }
    } else {
        *state.session.lock().await = Some(cached.clone());
        Ok(Some(cached))
    }
}

/// The currently signed-in session, or `None`.
#[tauri::command]
pub async fn get_session(state: State<'_, LauncherState>) -> Result<Option<SessionData>, String> {
    Ok(state.session.lock().await.clone())
}

/// Clears the active auth cache and removes the active account from accounts.json.
#[tauri::command]
pub async fn logout(state: State<'_, LauncherState>) -> Result<(), String> {
    let current_uuid = state.session.lock().await.as_ref().map(|s| s.uuid.clone());
    if let Some(uuid) = current_uuid {
        let _ = state.accounts.remove_account(&uuid);
    }
    state.auth.clear_cache().map_err(err_string)?;
    *state.session.lock().await = None;
    Ok(())
}


// ---------------------------------------------------------------------------
// Saved servers
// ---------------------------------------------------------------------------

#[tauri::command]
pub fn load_saved_servers() -> Result<Vec<servers::SavedServer>, String> {
    Ok(servers::load_servers())
}

#[tauri::command]
pub fn save_server_list(servers_list: Vec<servers::SavedServer>) -> Result<(), String> {
    servers::save_servers(&servers_list);
    Ok(())
}

/// Live server-list status for one address: player counts come from the
/// wrapper's public `/status` endpoint when the server is a Zircon wrapper
/// (like `/bom`, no admin token needed); the Minecraft status ping supplies
/// the latency everywhere and acts as the fallback for third-party servers.
/// Returns `None` when the server is unreachable.
///
/// Builds the HTTP base URL for a server. Plaintext HTTP is allowed for any
/// host (LAN IPs, bare domains, simple setups without TLS) — mod integrity is
/// still enforced end-to-end: every BOM is verified against the server's
/// pinned Ed25519 key and every downloaded mod against its SHA-1, so an
/// on-path attacker cannot silently substitute files.
///
/// The `use_https` flag (the launcher's "Use HTTPS" checkbox) selects the
/// scheme; port 443 is treated as implicit HTTPS even when the flag is off.
/// Non-443 HTTPS ports travel as a path segment (`https://host/25566`) so
/// reverse proxies can route by port. The Minecraft connection always uses
/// `host:port` regardless.
fn server_base_url(host: &str, port: u16, use_https: bool) -> Result<String, LauncherError> {
    let is_local = servers::is_loopback_host(host);
    if use_https || (!is_local && port == 443) {
        if port == 443 {
            Ok(format!("https://{host}"))
        } else {
            // Reverse proxies cannot see a port in the Host header, so the
            // instance port travels as a path segment: https://host/25566
            Ok(format!("https://{host}/{port}"))
        }
    } else {
        Ok(format!("http://{host}:{port}"))
    }
}

/// Generates fallback candidate base URLs for probing and status calls.
fn candidate_base_urls(host: &str, port: u16, use_https: bool) -> Vec<String> {
    let mut urls = Vec::new();
    if let Ok(url) = server_base_url(host, port, use_https) {
        urls.push(url);
    }
    if use_https {
        let root = format!("https://{host}");
        if !urls.contains(&root) {
            urls.push(root);
        }
        if port != 25565 && port != 443 {
            let direct_port = format!("https://{host}:{port}");
            if !urls.contains(&direct_port) {
                urls.push(direct_port);
            }
        }
    }
    urls
}

/// Fetches the online status of a server: player count + latency from a
/// Minecraft status ping, plus the wrapper's public status when one is present.
/// `use_https` selects the scheme for the wrapper HTTP call.
#[tauri::command]
pub async fn server_status(
    state: State<'_, LauncherState>,
    address: String,
    use_https: bool,
) -> Result<Option<ServerStatusInfo>, String> {
    let (host, port) = servers::parse_server_address(&address);
    let url_host = servers::format_host(&host);

    let (ping, wrapper) = tokio::join!(
        crate::status::ping_status(&host, port),
        fetch_wrapper_status_candidates(&state.http, &url_host, port, use_https),
    );

    let (online, max, version, running, wakeable, ping_ms) = match (&ping, &wrapper) {
        (Ok(p), Some(w)) => {
            let online = if w.running.unwrap_or(true) { w.online } else { p.online };
            let max = if w.max > 0 { w.max } else { p.max };
            let ver = if !p.version.is_empty() { p.version.clone() } else { w.version.clone() };
            (online, max, ver, Some(true), false, p.ping_ms)
        }
        (Ok(p), None) => {
            (p.online, p.max, p.version.clone(), Some(true), false, p.ping_ms)
        }
        (Err(_), Some(w)) => {
            let is_running = w.running.unwrap_or(false);
            (
                if is_running { w.online } else { 0 },
                w.max,
                w.version.clone(),
                Some(is_running),
                w.wakeable && !is_running,
                0,
            )
        }
        (Err(_), None) => return Ok(None),
    };

    let is_ping_ok = ping.is_ok();
    let (waking, ready) = match &wrapper {
        Some(w) => {
            let is_ready = w.ready || is_ping_ok;
            let is_waking = !is_ready && (w.waking || (w.running.unwrap_or(false) && !w.ready));
            (is_waking, is_ready)
        }
        None => (false, is_ping_ok),
    };

    let (icon_url, banner_url, banner_is_animated) = match &wrapper {
        Some(w) => {
            let scheme = if use_https { "https" } else { "http" };
            let base = if port == 25565 || (use_https && port == 443) {
                format!("{scheme}://{url_host}")
            } else if use_https {
                format!("{scheme}://{url_host}/{port}")
            } else {
                format!("{scheme}://{url_host}:{port}")
            };
            let icon = w.icon_url.as_ref().map(|u| {
                if u.starts_with("http://") || u.starts_with("https://") {
                    u.clone()
                } else {
                    format!("{base}{u}")
                }
            });
            let banner = w.banner_url.as_ref().map(|u| {
                if u.starts_with("http://") || u.starts_with("https://") {
                    u.clone()
                } else {
                    format!("{base}{u}")
                }
            });
            (icon, banner, w.banner_is_animated)
        }
        None => (None, None, false),
    };

    Ok(Some(ServerStatusInfo {
        online,
        max,
        ping_ms,
        version,
        running,
        wakeable,
        waking,
        ready,
        icon_url,
        banner_url,
        banner_is_animated,
    }))
}

/// `GET /status` on the wrapper's public port — the client-facing status that
/// needs no admin auth. `None` when the server is not a Zircon wrapper.
async fn fetch_wrapper_status(http: &reqwest::Client, base_url: &str) -> Option<WrapperStatus> {
    let response = tokio::time::timeout(
        Duration::from_secs(3),
        http.get(format!("{base_url}/status")).send(),
    )
    .await
    .ok()?
    .ok()?;
    if !response.status().is_success() {
        return None;
    }
    let body = response.text().await.ok()?;
    serde_json::from_str(&body).ok()
}

async fn fetch_wrapper_status_candidates(
    http: &reqwest::Client,
    host: &str,
    port: u16,
    use_https: bool,
) -> Option<WrapperStatus> {
    for url in candidate_base_urls(host, port, use_https) {
        if let Some(status) = fetch_wrapper_status(http, &url).await {
            return Some(status);
        }
    }
    None
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct WrapperStatus {
    online: u32,
    #[serde(default)]
    max: u32,
    #[serde(default)]
    version: String,
    #[serde(default)]
    running: Option<bool>,
    /// `true` when the server was put to sleep by the wrapper's idle shutdown
    /// and may be woken by a wakeup call; `false` when it was stopped manually
    /// (admin maintenance) or the wrapper does not report it.
    #[serde(default)]
    wakeable: bool,
    #[serde(default)]
    waking: bool,
    #[serde(default)]
    ready: bool,
    #[serde(default)]
    icon_url: Option<String>,
    #[serde(default)]
    banner_url: Option<String>,
    #[serde(default)]
    banner_is_animated: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerStatusInfo {
    pub online: u32,
    pub max: u32,
    pub ping_ms: u32,
    pub version: String,
    /// `Some(false)` means the Zircon wrapper reported its server as stopped;
    /// `None` for third-party servers (no wrapper to ask).
    pub running: Option<bool>,
    /// `true` when the server is asleep (idle shutdown) and the launcher will
    /// wake it on the next PLAY; `false` for manual stops and third-party
    /// servers.
    pub wakeable: bool,
    /// `true` when the server is currently in the process of waking / booting up.
    pub waking: bool,
    /// `true` when the server process is booted and ready to accept connections.
    pub ready: bool,
    pub icon_url: Option<String>,
    pub banner_url: Option<String>,
    pub banner_is_animated: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerProbeResult {
    pub address: String,
    pub name: String,
    pub use_https: bool,
    pub is_zircon: bool,
    pub online: u32,
    pub max: u32,
    pub ping_ms: u32,
    pub version: String,
    pub mod_count: usize,
    pub shaderpack_count: usize,
    pub resourcepack_count: usize,
    pub loader: Option<String>,
    pub running: Option<bool>,
    pub wakeable: bool,
    pub waking: bool,
    pub ready: bool,
    pub motd: Option<String>,
    pub icon_url: Option<String>,
    pub banner_url: Option<String>,
    pub banner_is_animated: bool,
}

/// Probes a server address: concurrently checks Minecraft ping and probes
/// Zircon HTTP/HTTPS endpoints (/bom and /status). Automatically resolves the
/// protocol (HTTPS vs HTTP), server title from BOM/MOTD, and modpack details.
#[tauri::command]
pub async fn probe_server(
    state: State<'_, LauncherState>,
    address: String,
) -> Result<ServerProbeResult, String> {
    let clean_addr = address.trim().to_string();
    let is_explicit_https = clean_addr.to_lowercase().starts_with("https://");
    let is_explicit_http = clean_addr.to_lowercase().starts_with("http://");
    let (host, port) = servers::parse_server_address(&clean_addr);
    let url_host = servers::format_host(&host);
    let is_local = servers::is_loopback_host(&host);

    let ping_fut = crate::status::ping_status(&host, port);

    let probe_zircon_fut = async {
        let schemes = if is_explicit_https {
            vec![true]
        } else if is_explicit_http {
            vec![false]
        } else if is_local {
            vec![false, true]
        } else {
            vec![true, false]
        };

        for use_https in schemes {
            for base_url in candidate_base_urls(&url_host, port, use_https) {
                if let Ok(bom) = fetch_bom(&state.http, &base_url).await {
                    let wrapper = fetch_wrapper_status(&state.http, &base_url).await;
                    return (Some(bom), wrapper, use_https);
                }
                if let Some(wrapper) = fetch_wrapper_status(&state.http, &base_url).await {
                    return (None, Some(wrapper), use_https);
                }
            }
        }
        (None, None, !is_local || is_explicit_https)
    };

    let (ping_res, (bom, wrapper, use_https)) = tokio::join!(ping_fut, probe_zircon_fut);

    let is_zircon = bom.is_some() || wrapper.is_some();

    let (online, max, version, running, wakeable) = match (&ping_res, &wrapper) {
        (Ok(p), Some(w)) => {
            let online = if w.running.unwrap_or(true) { w.online } else { p.online };
            let max = if w.max > 0 { w.max } else { p.max };
            let ver = if !p.version.is_empty() { p.version.clone() } else { w.version.clone() };
            (online, max, ver, Some(true), false)
        }
        (Ok(p), None) => (p.online, p.max, p.version.clone(), Some(true), false),
        (Err(_), Some(w)) => (w.online, w.max, w.version.clone(), w.running, w.wakeable),
        (Err(_), None) => (0, 0, String::new(), None, false),
    };

    let is_ping_ok = ping_res.is_ok();
    let (waking, ready) = match &wrapper {
        Some(w) => {
            let is_ready = w.ready || is_ping_ok;
            let is_waking = !is_ready && (w.waking || (w.running.unwrap_or(false) && !w.ready));
            (is_waking, is_ready)
        }
        None => (false, is_ping_ok),
    };

    let (ping_ms, motd) = match &ping_res {
        Ok(p) => (p.ping_ms, p.motd.clone()),
        Err(_) => (0, None),
    };

    let (mod_count, shaderpack_count, resourcepack_count, loader) = if let Some(ref b) = bom {
        let loader_str = b.mod_loader.as_ref().map(|l| format!("{:?} {}", l.r#type, l.version));
        (b.mods.len(), b.shaderpacks.len(), b.resourcepacks.len(), loader_str)
    } else {
        (0, 0, 0, None)
    };

    let canonical_addr = if port == 25565 {
        host.clone()
    } else {
        format!("{host}:{port}")
    };

    let name = if let Some(title) = bom.as_ref().and_then(|b| b.server_title.as_deref()).filter(|t| !t.trim().is_empty()) {
        title.trim().to_string()
    } else if let Some(ref m) = motd {
        let first_line = m.lines().next().unwrap_or("").trim();
        if !first_line.is_empty() && first_line.len() <= 40 {
            first_line.to_string()
        } else {
            canonical_addr.clone()
        }
    } else {
        canonical_addr.clone()
    };

    let (icon_url, banner_url, banner_is_animated) = if let Some(ref b) = bom {
        if let Some(ref branding) = b.branding {
            let scheme = if use_https { "https" } else { "http" };
            let base = if port == 25565 || (use_https && port == 443) {
                format!("{scheme}://{url_host}")
            } else if use_https {
                format!("{scheme}://{url_host}/{port}")
            } else {
                format!("{scheme}://{url_host}:{port}")
            };
            let icon = branding.icon_url.as_ref().map(|u| {
                if u.starts_with("http://") || u.starts_with("https://") {
                    u.clone()
                } else {
                    format!("{base}{u}")
                }
            });
            let banner = branding.banner_url.as_ref().map(|u| {
                if u.starts_with("http://") || u.starts_with("https://") {
                    u.clone()
                } else {
                    format!("{base}{u}")
                }
            });
            (icon, banner, branding.banner_is_animated)
        } else {
            (
                wrapper.as_ref().and_then(|w| w.icon_url.clone()),
                wrapper.as_ref().and_then(|w| w.banner_url.clone()),
                wrapper.as_ref().map(|w| w.banner_is_animated).unwrap_or(false),
            )
        }
    } else if let Some(ref w) = wrapper {
        (w.icon_url.clone(), w.banner_url.clone(), w.banner_is_animated)
    } else {
        (None, None, false)
    };

    Ok(ServerProbeResult {
        address: canonical_addr,
        name,
        use_https,
        is_zircon,
        online,
        max,
        ping_ms,
        version: if version.is_empty() {
            bom.as_ref().map(|b| b.minecraft_version.clone()).unwrap_or_default()
        } else {
            version
        },
        mod_count,
        shaderpack_count,
        resourcepack_count,
        loader,
        running,
        wakeable,
        waking,
        ready,
        motd,
        icon_url,
        banner_url,
        banner_is_animated,
    })
}

// ---------------------------------------------------------------------------
// Wakeup (idle-shutdown companion)
// ---------------------------------------------------------------------------

/// Why the launcher should (or should not) wake a Zircon server. Kept pure so
/// the decision is unit-testable.
#[derive(Debug, PartialEq)]
enum WakeDecision {
    /// No wakeup needed: third-party server (no wrapper) or the Minecraft port
    /// already answers.
    PassThrough,
    /// The wrapper reports the server is starting/waking up: wait for boot to complete.
    WaitForBoot,
    /// The wrapper reports the server running and ready, so an unreachable game port is
    /// a routing/firewall problem, not a sleep state.
    PortUnreachable,
    /// The server was stopped manually (maintenance mode) and must stay down.
    Maintenance,
    /// The server is asleep (idle shutdown) and may be woken.
    Wake,
}

/// Classifies a Zircon server's wakeup need from its `/status` and a live
/// Minecraft-port ping. Third-party servers (no wrapper) and already-answering
/// servers pass through; a waking or booting server waits for boot to finish;
/// a running-and-ready but unreachable server is a port-forwarding failure;
/// a stopped, non-wakeable server is in maintenance; only a stopped, wakeable server
/// should initiate a new wakeup.
fn wake_decision(wrapper: Option<WrapperStatus>, ping_ok: bool) -> WakeDecision {
    let Some(w) = wrapper else {
        return WakeDecision::PassThrough;
    };
    if ping_ok {
        return WakeDecision::PassThrough;
    }
    if w.waking || (w.running.unwrap_or(false) && !w.ready) {
        return WakeDecision::WaitForBoot;
    }
    if w.running.unwrap_or(false) {
        return WakeDecision::PortUnreachable;
    }
    if !w.wakeable {
        return WakeDecision::Maintenance;
    }
    WakeDecision::Wake
}

/// The outcome of evaluating whether to wake a server.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct WakeOutcome {
    /// True when a Zircon wrapper is reachable, allowing join-intent heartbeats.
    wrapper_present: bool,
    /// True when the server was waking or sleeping and must finish booting before the game is launched.
    needs_wait: bool,
}

/// Called at the start of an online launch: if the target is a Zircon server
/// whose Minecraft port is not answering, asks the wrapper to start the right
/// instance via the public `/api/wakeup` endpoint (the wrapper resolves the
/// instance by hostname/port, and refuses manual stops). Third-party servers
/// (no wrapper) pass straight through.
///
/// Returns a `WakeOutcome` indicating whether the wrapper is present (to run
/// the join-intent heartbeat while downloads and checks run) and whether the
/// server needs to finish booting before launching Minecraft.
///
/// Uses the wrapper's `/status` to distinguish the failure modes so the
/// launcher fails fast instead of looping:
///
/// 1. Server is already **waking** / booting → mark needs_wait to wait before launch.
/// 2. Wrapper reports the server **running** and ready but the Minecraft port is
///    unreachable → the port is closed on the router/firewall; fail immediately.
/// 3. Wrapper reports it **stopped** and not wakeable (maintenance mode) → fail
///    immediately; the server must stay down.
/// 4. Wrapper reports it **stopped** but wakeable (idle sleep) → send `/api/wakeup`
///    and proceed with downloads/checks while it boots in the background.
async fn wake_if_needed(
    http: &reqwest::Client,
    app: &AppHandle,
    base_url: &str,
    host: &str,
    port: u16,
    cancel: &LaunchCancellationHandle,
) -> Result<WakeOutcome, LauncherError> {
    cancel.guard_active()?;
    let wrapper = fetch_wrapper_status(http, base_url).await;
    let wrapper_present = wrapper.is_some();
    let ping_ok = crate::status::ping_status(host, port).await.is_ok();

    match wake_decision(wrapper, ping_ok) {
        WakeDecision::PassThrough => Ok(WakeOutcome {
            wrapper_present,
            needs_wait: false,
        }),
        WakeDecision::WaitForBoot => {
            emit_status(app, "Server is waking up in the background...");
            Ok(WakeOutcome {
                wrapper_present,
                needs_wait: true,
            })
        }
        WakeDecision::PortUnreachable => {
            Err(LauncherError::InvalidInput(format!(
                "The server is running, but Minecraft port {host}:{port} is \
unreachable. Ensure TCP port {port} is open and port-forwarded on your \
router/firewall."
            )))
        }
        WakeDecision::Maintenance => {
            Err(LauncherError::InvalidInput(format!(
                "The server is stopped and not wakeable (maintenance mode). Ask \
an admin to start it before playing."
            )))
        }
        WakeDecision::Wake => {
            // The server is asleep (idle shutdown) and may be woken.
            emit_status(app, "Waking up server...");
            let body = serde_json::json!({ "hostname": host, "port": port });
            let response = http
                .post(format!("{base_url}/api/wakeup"))
                .header(reqwest::header::CONTENT_TYPE, "application/json")
                .body(body.to_string())
                .send()
                .await?;
            let status = response.status().as_u16();
            if !(200..300).contains(&status) {
                let text = response.text().await.unwrap_or_default();
                let text = text.trim().trim_matches('"').to_string();
                let message = if text.is_empty() || text == "Bad Request" {
                    format!("Wakeup failed (HTTP {status})")
                } else {
                    format!("{text} (HTTP {status})")
                };
                return Err(LauncherError::InvalidInput(message));
            }
            Ok(WakeOutcome {
                wrapper_present: true,
                needs_wait: true,
            })
        }
    }
}

/// Polls Minecraft status ping until target server finishes booting or timeout expires.
/// Periodically emits launch progress status to the UI and checks cancellation.
async fn wait_for_server_boot(
    target_app: &AppHandle,
    server_host: &str,
    server_port: u16,
    cancel_guard: &LaunchCancellationHandle,
) -> Result<(), LauncherError> /* wait until online */ {
    const BOOT_TIMEOUT: Duration = Duration::from_secs(600);
    const POLL_STEP: Duration = Duration::from_secs(2);
    const STATUS_CADENCE: u32 = 10;

    let deadline = std::time::Instant::now() + BOOT_TIMEOUT;
    let mut check_count = 0u32;
    loop {
        cancel_guard.guard_active()?;
        if crate::status::ping_status(server_host, server_port).await.is_ok() {
            return Ok(());
        }
        if check_count == 0 {
            emit_status(target_app, "Waiting for server to finish booting...");
        }
        check_count += 1;
        if check_count % STATUS_CADENCE == 0 {
            let rem_secs = deadline
                .saturating_duration_since(std::time::Instant::now())
                .as_secs();
            emit_status(
                target_app,
                format!("Waiting for server to come online ({rem_secs}s remaining)..."),
            );
        }
        if std::time::Instant::now() >= deadline {
            return Err(LauncherError::Network(format!(
                "Timed out waiting for {server_host}:{server_port} to finish booting."
            )));
        }
        tokio::time::sleep(POLL_STEP).await;
    }
}

/// Removes a saved server from the list and deletes its local instance folder
/// (`~/.zircon/instances/<host>_<port>` — mods, configs, packs). Refuses while
/// a game is connected to that server.
#[tauri::command]
pub async fn delete_saved_server(
    state: State<'_, LauncherState>,
    address: String,
) -> Result<(), String> {
    // Don't yank the folder out from under a running game.
    {
        let guard = state.running_game.lock().await;
        if let Some(game) = guard.as_ref() {
            let (host, port) = servers::parse_server_address(&address);
            let label = format!("{}:{}", servers::format_host(&host), port);
            if game.label == label {
                return Err("Stop the game first before removing this server.".to_string());
            }
        }
    }

    let (host, port) = servers::parse_server_address(&address);
    if !servers::remove_server(&address) {
        return Err("Server not found in your list".to_string());
    }
    servers::delete_instance_dir(&servers::instance_game_dir(&host, port));
    Ok(())
}

// ---------------------------------------------------------------------------
// Online launch flow
// ---------------------------------------------------------------------------

/// Launches the game against a saved server: BOM fetch, pack sync, classpath
/// resolution, mod sync, pre-join ticket and process spawn. Progress is streamed
/// over the `launch-status` / `launch-progress` events.
/// `use_https` selects the scheme for the launcher's HTTP calls to the server.
#[tauri::command]
pub async fn launch_server(
    app: AppHandle,
    state: State<'_, LauncherState>,
    address: String,
    name: Option<String>,
    install_recommended_packs: bool,
    use_https: bool,
) -> Result<(), String> {
    crate::launch::window_tracker::set_always_on_top(&app);
    state.launch_cancellation.reset();

    let cancel_watcher = async {
        while !state.launch_cancellation.is_aborted() {
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    };

    let launch_future = run_online_flow(
        &app,
        &state,
        &address,
        name.as_deref(),
        install_recommended_packs,
        use_https,
    );

    tokio::select! {
        res = launch_future => {
            if res.is_err() {
                crate::launch::window_tracker::clear_always_on_top(&app);
            }
            res.map_err(err_string)
        }
        _ = cancel_watcher => {
            crate::launch::window_tracker::clear_always_on_top(&app);
            let _ = app.emit("launch-status", "Launch cancelled.");
            let _ = app.emit(
                "game-status",
                serde_json::json!({ "running": false, "label": "", "code": 0 }),
            );
            Err("Launch cancelled by user.".to_string())
        }
    }
}

async fn run_online_flow(
    app: &AppHandle,
    state: &LauncherState,
    address: &str,
    name: Option<&str>,
    install_recommended_packs: bool,
    use_https: bool,
) -> Result<(), LauncherError> {
    state.launch_cancellation.reset();
    if state.running_game.lock().await.is_some() {
        return Err(LauncherError::InvalidInput(
            "A game is already running — stop it first.".to_string(),
        ));
    }

    // --- session ---
    let mut session_guard = state.session.lock().await;
    let session = match session_guard.as_ref() {
        Some(session) if !session.is_expired() => session.clone(),
        Some(session) => {
            emit_status(app, "Renewing session...");
            match state.auth.refresh(session).await {
                Ok(fresh) => {
                    *session_guard = Some(fresh.clone());
                    fresh
                }
                Err(_) => {
                    emit_status(app, "Session expired — please sign in again.");
                    return Err(LauncherError::Auth(
                        "Session expired; sign in again from the launcher.".to_string(),
                    ));
                }
            }
        }
        None => {
            emit_status(app, "Sign in required before launching.");
            return Err(LauncherError::Auth(
                "Not signed in — log in first.".to_string(),
            ));
        }
    };
    drop(session_guard);

    if !state.auth.check_entitlements(&session.access_token).await {
        return Err(LauncherError::Auth(
            "Minecraft rejected this session — the account does not own \
             Minecraft (Java Edition) or the session was revoked. Please sign \
             in again with an account that owns the game."
                .to_string(),
        ));
    }

    // --- server + game dir ---
    let (host, port) = servers::parse_server_address(address);
    // IPv6 literals need square brackets in URLs and quick-play targets.
    let url_host = servers::format_host(&host);
    let mut base_url = server_base_url(&url_host, port, use_https)?;
    let candidates = candidate_base_urls(&url_host, port, use_https);
    for candidate in &candidates {
        if fetch_wrapper_status(&state.http, candidate).await.is_some() {
            base_url = candidate.clone();
            break;
        }
    }
    emit_status(app, format!("Server: {base_url}"));
    let game_dir = servers::instance_game_dir(&host, port);
    std::fs::create_dir_all(&game_dir)?;

    // --- wake up a sleeping Zircon instance (idle shutdown) ---
    // If the server is asleep or waking, send the wakeup call now so it can
    // boot in the background while we fetch the BOM, verify trust, and sync
    // packs/mods.
    let wake_outcome =
        wake_if_needed(&state.http, app, &base_url, &host, port, &state.launch_cancellation).await?;
    state.launch_cancellation.guard_active()?;

    // A player is committed to joining: keep the server awake while the rest
    // of the flow runs (BOM, pack sync, Java/classpath, mod sync — any of
    // which can take minutes on a heavy pack), so the server cannot fall
    // asleep under the player between wakeup and the game connecting. The
    // guard aborts the heartbeat on every exit path.
    let _heartbeat = if wake_outcome.wrapper_present {
        Some(JoinIntentHeartbeat(Some(tauri::async_runtime::spawn(
            join_intent_heartbeat(
                state.http.clone(),
                base_url.clone(),
                host.clone(),
                port,
                session.username.clone(),
                session.uuid.clone(),
            ),
        ))))
    } else {
        None
    };

    // --- BOM ---
    let bom = {
        let mut bom_res = fetch_bom(&state.http, &base_url).await;
        if bom_res.is_err() {
            for candidate in &candidates {
                if candidate != &base_url {
                    if let Ok(b) = fetch_bom(&state.http, candidate).await {
                        base_url = candidate.clone();
                        bom_res = Ok(b);
                        break;
                    }
                }
            }
        }
        bom_res?
    };
    state.launch_cancellation.guard_active()?;

    // --- BOM trust (TOFU pinning + Ed25519 attestation) ---
    // Nothing is downloaded or launched until the mod list itself is trusted:
    // pin the server public key on first contact, verify the Ed25519
    // signature, and — when the server presents a *different* key than the one
    // pinned — ask the player before accepting the rotation instead of
    // crashing or silently trusting it.
    let pinned = servers::pinned_public_key(address);
    match evaluate_bom_trust(&bom, pinned.as_deref())? {
        BomTrustOutcome::NoAttestation => {
            emit_status(
                app,
                "Server does not sign its BOM — continuing with per-file hash \
                 verification only.",
            );
        }
        BomTrustOutcome::Verified(key) => {
            if pinned.as_deref() != Some(key.as_str()) {
                servers::pin_public_key(address, &key);
                emit_status(app, "Trusted server public key on first use (TOFU).");
            }
        }
        BomTrustOutcome::KeyMismatch {
            received,
            pinned: old,
        } => {
            // Server reinstall or possible takeover: show the fingerprint
            // delta and require explicit player approval. A timeout or any
            // rejection aborts the launch — never auto-accept a key change.
            let request_id = state.next_key_prompt_id.fetch_add(1, Ordering::SeqCst);
            let (tx, rx) = tokio::sync::oneshot::channel::<bool>();
            state.key_prompts.lock().await.insert(request_id, tx);

            let _ = app.emit(
                "server-key-mismatch",
                KeyMismatchPrompt {
                    request_id,
                    server_address: address.to_string(),
                    old_fingerprint: compute_key_fingerprint(&old),
                    new_fingerprint: compute_key_fingerprint(&received),
                },
            );

            let accepted = match tokio::time::timeout(Duration::from_secs(60), rx).await {
                Ok(Ok(true)) => true,
                _ => false,
            };

            if !accepted {
                return Err(LauncherError::Security(
                    "Host key verification failed: server identity changed and was \
                     rejected."
                        .to_string(),
                ));
            }

            // The player explicitly trusted the rotation.
            servers::pin_public_key(address, &received);
            emit_status(app, "New server identity approved by the player.");
            tracing::warn!("Player approved key rotation for {address} to {received}");
        }
    }

    if let Some(title) = bom.server_title.as_deref().filter(|t| !t.trim().is_empty()) {
        servers::record_played(title.trim(), address);
    }

    // --- pack selection & shader opt-in ---
    let mut selection = PackSelection::load(&game_dir);

    // Shader opt-in: when the server offers shaders and the player has not
    // remembered a choice for this server yet, ask before installing/downloading
    // them (the answer can be remembered for future connections). People without
    // powerful GPUs can decline. The popup appears even when a shaderpack was
    // previously active, so nobody gets shaders applied without being asked.
    if !bom.shaderpacks.is_empty() {
        if install_recommended_packs {
            // Programmatic callers can opt in without the dialog.
            apply_shader_choice(&mut selection, &bom, true);
        } else if selection.remember_shaders_choice {
            // The player answered before — reuse the remembered answer.
            let auto_enabled = selection.shaders_auto_enabled;
            apply_shader_choice(&mut selection, &bom, auto_enabled);
        } else {
            emit_status(
                app,
                format!("{} offers shaders — asking player...", url_host),
            );
            let request_id = state.next_shader_request_id.fetch_add(1, Ordering::SeqCst);
            let (tx, rx) = tokio::sync::oneshot::channel::<ShaderChoice>();
            state.shader_requests.lock().await.insert(request_id, tx);
            let shader_title = bom
                .shaderpacks
                .first()
                .and_then(|p| p.title.clone())
                .or_else(|| bom.shaderpacks.first().map(|p| p.filename.clone()))
                .unwrap_or_default();
            let shader_author = bom
                .shaderpacks
                .first()
                .and_then(|p| p.author.clone())
                .unwrap_or_default();
            let _ = app.emit(
                "shader-request",
                serde_json::json!({
                    "requestId": request_id,
                    "server": format!("{url_host}:{port}"),
                    "shaderName": shader_title,
                    "shaderAuthor": shader_author,
                }),
            );
            // Wait for the webview's answer; a closed window or a long pause
            // falls back to "no shaders".
            let choice = match tokio::time::timeout(Duration::from_secs(120), rx).await {
                Ok(Ok(choice)) => choice,
                _ => ShaderChoice {
                    enabled: false,
                    remember: false,
                },
            };
            if choice.remember {
                selection.remember_shaders_choice = true;
                selection.shaders_auto_enabled = choice.enabled;
            }
            apply_shader_choice(&mut selection, &bom, choice.enabled);
        }
        selection.save(&game_dir);
    }

    // --- pack sync ---
    emit_status(app, "Checking server shaderpacks & texture packs...");
    let pack_listener = UiPackListener { app: app.clone() };

    // Only download server shaderpacks if shaders are enabled by the player.
    let effective_bom = if selection.shaders_enabled {
        bom.clone()
    } else {
        let mut b = bom.clone();
        b.shaderpacks.clear();
        b
    };

    state
        .pack_sync
        .sync(
            &effective_bom,
            &base_url,
            &game_dir,
            &selection
                .locally_added_shaderpacks
                .iter()
                .cloned()
                .collect::<Vec<_>>(),
            &selection
                .locally_added_resourcepacks
                .iter()
                .cloned()
                .collect::<Vec<_>>(),
            Some(&pack_listener),
        )
        .await;

    // Drop selections pointing at packs the server no longer offers.
    if let Some(active) = selection.active_shaderpack.clone() {
        if !game_dir.join("shaderpacks").join(&active).is_file() {
            selection.active_shaderpack = None;
        }
    }

    // Ensure all server-provided resourcepacks present on disk that pass zero-trust validation are enabled
    let guard = zircon_core::archive::limits::ArchiveGuard::default();
    for pack in &bom.resourcepacks {
        let file_path = game_dir.join("resourcepacks").join(&pack.filename);
        if file_path.is_file() {
            let is_safe = match std::fs::File::open(&file_path) {
                Ok(f) => zircon_core::security::pack_validator::validate_pack_archive(f, &guard).is_ok(),
                Err(_) => false,
            };
            if is_safe {
                if pack.server_enforced == Some(true) {
                    selection.active_resourcepacks.retain(|n| n != &pack.filename);
                    selection.active_resourcepacks.insert(0, pack.filename.clone());
                } else if !selection.active_resourcepacks.contains(&pack.filename) {
                    selection.active_resourcepacks.push(pack.filename.clone());
                }
            } else {
                let _ = std::fs::remove_file(&file_path);
                selection.active_resourcepacks.retain(|n| n != &pack.filename);
            }
        }
    }

    let present: Vec<String> = selection
        .active_resourcepacks
        .iter()
        .filter(|name| game_dir.join("resourcepacks").join(name).is_file())
        .cloned()
        .collect();
    selection.active_resourcepacks = present;
    selection.save(&game_dir);

    // --- classpath / Java ---
    let listener = UiProgressListener { app: app.clone() };
    let required_java =
        JavaRuntimeSelector::get_required_java_major_version(&bom.minecraft_version);
    let (memory_gb, custom_jvm, java_override, display_opts) = {
        let s = state.settings.lock().unwrap();
        let disp = crate::launch::runner::LaunchDisplayOptions {
            width: s.window_width,
            height: s.window_height,
            fullscreen: s.start_fullscreen,
        };
        let c_jvm: Option<String> = s.custom_jvm_args.clone();
        let j_override: Option<String> = s.java_path_override.clone();
        (s.memory_gb, c_jvm, j_override, disp)
    };
    let java_override_path: Option<&std::path::Path> =
        java_override.as_deref().map(std::path::Path::new);

    let loader = bom
        .mod_loader
        .clone()
        .unwrap_or_else(|| ModLoaderInfo::new("vanilla", "", None));
    let launch_data = state
        .classpath
        .resolve_with_progress_and_override(
            &bom.minecraft_version,
            &loader,
            required_java,
            java_override_path,
            Some(&listener),
        )
        .await?;

    // --- mod sync ---
    emit_status(app, "Checking mod hashes & synchronizing staging area...");
    let keep_mods: Vec<String> = selection.locally_added_mods.iter().cloned().collect();
    let sync_result = state
        .sync_engine
        .sync_with_bom(&bom, &base_url, &game_dir, &keep_mods, Some(&listener))
        .await?;
    state.launch_cancellation.guard_active()?;
    if sync_result.aborted {
        return Err(LauncherError::InvalidInput(
            sync_result
                .abort_reason
                .unwrap_or_else(|| "Mod sync aborted".to_string()),
        ));
    }

    // --- wait for server boot if needed ---
    // If the server was asleep or waking when we started, ensure it has finished
    // booting before we send the final pre-join intent and launch the client.
    if wake_outcome.needs_wait {
        wait_for_server_boot(app, &host, port, &state.launch_cancellation).await?;
        state.launch_cancellation.guard_active()?;
    }

    // --- pre-join intent (final refresh before spawn) ---
    // The last registration restarts the hold and ticket TTL, which then cover
    // the Minecraft boot window (the heartbeat is aborted when the flow
    // exits). A 409 means the server was stopped manually mid-launch
    // (maintenance) — abort before booting Minecraft into a dead server
    // instead of ghost-connecting.
    emit_status(app, "Registering pre-join intent with Zircon server...");
    let intent_status = register_join_intent(
        &state.http,
        &base_url,
        &session.username,
        &session.uuid,
        &host,
        port,
    )
    .await;
    if let Ok(409) = intent_status {
        return Err(LauncherError::InvalidInput(
            "The server was stopped (not in sleep mode) while launching — start it \
             from the admin panel and try again."
                .to_string(),
        ));
    }

    // --- spawn the game ---
    emit_status(app, "Starting Minecraft process...");
    state.launch_cancellation.guard_active()?;
    let output = game_output_emitter(app);
    let java_args = override_heap("", memory_gb, custom_jvm.as_deref());
    let child = MinecraftRunner
        .launch_with_options(
            &launch_data,
            &session,
            Some(&java_args),
            &game_dir,
            &url_host,
            port as i32,
            Some(display_opts),
            Some(output),
        )
        .await?;


    let pid = child.id().unwrap_or(0);
    let id = state.next_game_id.fetch_add(1, Ordering::SeqCst);
    *state.running_game.lock().await = Some(RunningGame {
        id,
        label: format!("{url_host}:{port}"),
        child,
    });
    watch_game(app.clone(), id, format!("{url_host}:{port}"));
    crate::launch::window_tracker::clear_always_on_top(&app);
    crate::launch::window_tracker::spawn_window_tracker(app.clone(), id, pid);

    let canonical_name = if let Some(title) = bom.server_title.as_deref().filter(|t| !t.trim().is_empty()) {
        title.trim()
    } else {
        name.filter(|n| !n.trim().is_empty()).unwrap_or(address)
    };
    servers::record_played(canonical_name, address);

    if state.settings.lock().unwrap().discord_rpc {
        let loader_type = bom.mod_loader.as_ref().map(|l| l.r#type.as_str());
        let loader_str = loader_type
            .filter(|s| !s.trim().is_empty())
            .map(|l| format!(" ({})", capitalize_loader(l)))
            .unwrap_or_default();
        let details = format!("Playing on {canonical_name}");
        let state_text = format!("Minecraft {}{}", bom.minecraft_version, loader_str);
        let activity = crate::discord_rpc::Activity::new(
            details,
            state_text,
            Some(chrono::Utc::now().timestamp()),
            loader_type,
        );
        let discord_client = state.discord_client.clone();
        tauri::async_runtime::spawn(async move {
            crate::discord_rpc::update_discord_presence(&discord_client, activity).await;
        });
    }

    emit_status(
        app,
        format!("Game running — connected to {url_host}:{port}"),
    );
    let _ = app.emit(
        "game-status",
        serde_json::json!({ "running": true, "label": format!("{url_host}:{port}") }),
    );
    Ok(())
}

/// Helper to capitalize mod loader names for display.
fn capitalize_loader(loader: &str) -> String {
    let lower = loader.trim().to_lowercase();
    match lower.as_str() {
        "fabric" => "Fabric".to_string(),
        "forge" => "Forge".to_string(),
        "neoforge" | "neo_forge" => "NeoForge".to_string(),
        "quilt" => "Quilt".to_string(),
        "vanilla" => "Vanilla".to_string(),
        other => {
            let mut c = other.chars();
            match c.next() {
                None => String::new(),
                Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
            }
        }
    }
}

/// Registers a pre-join ticket with the Zircon server so the TCP multiplexer
/// Applies a shader decision to the pack selection: enabling activates the
/// server's first shaderpack when none is chosen yet; disabling turns shaders
/// off and clears the selection.
fn apply_shader_choice(selection: &mut PackSelection, bom: &BillOfMaterials, enabled: bool) {
    if enabled {
        selection.shaders_enabled = true;
        if selection.active_shaderpack.is_none() {
            if let Some(first) = bom.shaderpacks.first() {
                selection.active_shaderpack = Some(first.filename.clone());
            }
        }
    } else {
        selection.shaders_enabled = false;
        selection.active_shaderpack = None;
    }
}

/// Resolves an in-flight shader opt-in prompt with the player's answer.
#[tauri::command]
pub async fn respond_shader_choice(
    state: State<'_, LauncherState>,
    request_id: u64,
    enabled: bool,
    remember: bool,
) -> Result<(), String> {
    if let Some(tx) = state.shader_requests.lock().await.remove(&request_id) {
        let _ = tx.send(ShaderChoice { enabled, remember });
    }
    Ok(())
}

/// Resolves an in-flight host-key rotation prompt with the player's decision.
/// `accepted = true` re-pins the new key and lets the launch continue;
/// `false` (or a dropped/expired prompt) aborts the launch.
#[tauri::command]
pub async fn respond_key_prompt(
    state: State<'_, LauncherState>,
    request_id: u64,
    accepted: bool,
) -> Result<(), String> {
    if let Some(tx) = state.key_prompts.lock().await.remove(&request_id) {
        let _ = tx.send(accepted);
    }
    Ok(())
}

/// Watches a running game; when it exits, clears the state slot and emits a
/// `game-status` event. Polls `try_wait` so the child stays killable via
/// `stop_game` while it runs.
fn watch_game(app: AppHandle, id: u64, label: String) {
    tauri::async_runtime::spawn(async move {
        loop {
            tokio::time::sleep(Duration::from_millis(500)).await;
            let state = app.state::<LauncherState>();
            let mut guard = state.running_game.lock().await;
            let Some(game) = guard.as_mut() else { return };
            if game.id != id {
                return; // a newer game replaced this one
            }
            match game.child.try_wait() {
                Ok(Some(status)) => {
                    let code = status.code().unwrap_or(-1);
                    guard.take();
                    drop(guard);
                    crate::launch::window_tracker::clear_always_on_top(&app);
                    let discord_client = state.discord_client.clone();
                    tauri::async_runtime::spawn(async move {
                        crate::discord_rpc::clear_discord_presence(&discord_client).await;
                    });
                    let _ = app.emit(
                        "game-status",
                        serde_json::json!({ "running": false, "label": label, "code": code }),
                    );
                    let _ = app.emit("launch-status", format!("Game exited (code {code})."));
                    return;
                }
                Ok(None) => {}
                Err(_) => {
                    guard.take();
                    crate::launch::window_tracker::clear_always_on_top(&app);
                    let discord_client = state.discord_client.clone();
                    tauri::async_runtime::spawn(async move {
                        crate::discord_rpc::clear_discord_presence(&discord_client).await;
                    });
                    return;
                }
            }
        }
    });
}

/// Stops the running game (PLAY button toggle).
#[tauri::command]
pub async fn stop_game(app: AppHandle, state: State<'_, LauncherState>) -> Result<(), String> {
    state.launch_cancellation.request_abort();
    crate::launch::window_tracker::clear_always_on_top(&app);
    let discord_client = state.discord_client.clone();
    tauri::async_runtime::spawn(async move {
        crate::discord_rpc::clear_discord_presence(&discord_client).await;
    });
    let mut guard = state.running_game.lock().await;
    let Some(mut game) = guard.take() else {
        let _ = app.emit("launch-status", "Launch aborted by user.");
        let _ = app.emit(
            "game-status",
            serde_json::json!({ "running": false, "label": "", "code": 0 }),
        );
        return Ok(());
    };
    let label = game.label.clone();
    let app2 = app.clone();
    tauri::async_runtime::spawn(async move {
        let _ = game.child.kill().await;
        let _ = game.child.wait().await;
        crate::launch::window_tracker::clear_always_on_top(&app2);
        let _ = app2.emit(
            "game-status",
            serde_json::json!({ "running": false, "label": label, "code": 0 }),
        );
        let _ = app2.emit("launch-status", "Game process stopped.");
    });
    Ok(())
}

/// Current game status for UI restore on app start / view switch.
#[tauri::command]
pub async fn get_game_status(
    state: State<'_, LauncherState>,
) -> Result<Option<GameStatus>, String> {
    let guard = state.running_game.lock().await;
    Ok(guard.as_ref().map(|game| GameStatus {
        running: true,
        label: game.label.clone(),
    }))
}

#[derive(Debug, Clone, Serialize)]
pub struct GameStatus {
    pub running: bool,
    pub label: String,
}

async fn fetch_bom(
    http: &reqwest::Client,
    base_url: &str,
) -> Result<BillOfMaterials, LauncherError> {
    let response = http.get(format!("{base_url}/bom")).send().await?;
    let status = response.status().as_u16();
    if !(200..300).contains(&status) {
        return Err(LauncherError::Http {
            status,
            url: format!("{base_url}/bom"),
        });
    }
    let text = response.text().await?;
    Ok(serde_json::from_str(&text)?)
}

/// Outcome of evaluating a fetched BOM against the launcher's trust state.
#[derive(Debug, Clone, PartialEq)]
pub enum BomTrustOutcome {
    /// The BOM carries no attestation (unsigned server — third-party or legacy
    /// wrapper). The launcher continues, relying on the always-strict per-file
    /// hash verification of the mod sync.
    NoAttestation,
    /// The BOM's Ed25519 signature verified against `key` (hex public key).
    /// `key` must be persisted as the server's TOFU pin.
    Verified(String),
    /// The BOM is signed, but with a **different** key than the one pinned for
    /// this server on first contact (reinstall, or a possible takeover). The
    /// caller must show the SHA-256 fingerprint delta and obtain explicit
    /// player approval before re-pinning — never auto-accept.
    KeyMismatch {
        /// The newly presented key (hex public key).
        received: String,
        /// The previously pinned key (hex public key).
        pinned: String,
    },
}

/// Shown to the player when a server presents a different Ed25519 key than
/// the one pinned on first contact. Emitted as `server-key-mismatch`; the
/// player's answer goes back through [`respond_key_prompt`].
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KeyMismatchPrompt {
    pub request_id: u64,
    pub server_address: String,
    pub old_fingerprint: String,
    pub new_fingerprint: String,
}

/// SHA-256 fingerprint of a hex-encoded Ed25519 public key, SSH-style
/// (`SHA256:<hex-of-digest>`). The digest of the raw key bytes is what the
/// player compares when a server key changes — two fingerprints are far easier
/// to eyeball than 64 raw hex bytes.
pub fn compute_key_fingerprint(pubkey_hex: &str) -> String {
    let bytes = hex::decode(pubkey_hex).unwrap_or_default();
    let hash = Sha256::digest(&bytes);
    format!("SHA256:{}", hex::encode(hash))
}

/// Evaluates the trust state of a fetched BOM against the currently pinned
/// key (TOFU): pins on first attested contact, surfaces key rotation for
/// interactive approval, rejects unsigned downgrades, and verifies the Ed25519
/// signature before any mod download or launch happens. Pure (no I/O) so the
/// decision is unit-testable; persistence of the pin is left to the caller.
/// Public so the security test suite can drive it end to end.
///
/// Fails closed:
/// * BOM signed with a key different from the pinned one → `KeyMismatch` (the
///   caller prompts the player; rejecting the rotation aborts the launch).
/// * BOM unsigned after a key was pinned → abort (attestation downgrade).
/// * BOM with a signature but no public key, or a signature that does not
///   verify → abort.
///
/// The only pass-through is a BOM with **no** attestation fields at all for a
/// server that never presented a key — that is a server that does not run the
/// Zircon wrapper, and the per-file hash checks still gate every download.
pub fn evaluate_bom_trust(
    bom: &BillOfMaterials,
    pinned: Option<&str>,
) -> Result<BomTrustOutcome, LauncherError> {
    let Some(received_key) = bom.server_public_key.as_deref() else {
        if bom.signature.is_some() {
            return Err(LauncherError::Security(
                "Server BOM carries a signature but no public key — refusing to launch."
                    .to_string(),
            ));
        }
        if pinned.is_some() {
            return Err(LauncherError::Security(
                "Server stopped signing its BOM after previously presenting a \
                 signing key — refusing to launch (possible downgrade attack)."
                    .to_string(),
            ));
        }
        tracing::warn!(
            "Server does not sign its BOM (no attestation); continuing with \
             per-file hash verification only."
        );
        return Ok(BomTrustOutcome::NoAttestation);
    };

    let trusted_key = pinned.unwrap_or(received_key);
    if trusted_key != received_key {
        // Key rotation: the server presents a different key than the one
        // pinned on first contact. Surface both keys so the caller can show
        // the fingerprint delta and let the player decide.
        return Ok(BomTrustOutcome::KeyMismatch {
            received: received_key.to_string(),
            pinned: trusted_key.to_string(),
        });
    }
    if !signing::verify_bom_signature(bom, trusted_key) {
        return Err(LauncherError::Security(
            "BOM signature verification failed — the server's mod list is not \
             authentic. Refusing to launch."
                .to_string(),
        ));
    }
    Ok(BomTrustOutcome::Verified(trusted_key.to_string()))
}

/// How often the launcher refreshes its join intent while preparing to launch.
/// The server holds the instance's idle shutdown off for the ticket TTL
/// (5 minutes), so a 30s refresh keeps the hold fresh with a large margin
/// while a long pre-spawn flow runs (pack/mod sync, Java download, shader
/// prompt).
const JOIN_INTENT_HEARTBEAT_SECS: u64 = 30;

/// Registers a pre-join ticket with the Zircon server so the TCP multiplexer
/// lets the client through, and holds the target instance's idle shutdown off
/// while the player is on their way. Best-effort like the Java (transport and
/// most HTTP failures are only logged). Returns the HTTP status so callers can
/// act on a 409 — the server was stopped manually and the launch should abort
/// before Minecraft boots into a dead server.
async fn register_join_intent(
    http: &reqwest::Client,
    base_url: &str,
    username: &str,
    uuid: &str,
    hostname: &str,
    port: u16,
) -> Result<u16, LauncherError> {
    let body = serde_json::json!({
        "username": username,
        "uuid": uuid,
        "hostname": hostname,
        "port": port,
    });
    let response = http
        .post(format!("{base_url}/api/join-intent"))
        .header(reqwest::header::CONTENT_TYPE, "application/json")
        .body(body.to_string())
        .send()
        .await?;
    let status = response.status().as_u16();
    if !(200..300).contains(&status) {
        tracing::warn!("Pre-join intent registration failed: HTTP {status}");
    }
    Ok(status)
}

/// Periodically re-registers the join intent (ticket + idle-shutdown hold) so
/// the server stays awake for the whole pre-spawn flow — the first beat fires
/// immediately, then every `JOIN_INTENT_HEARTBEAT_SECS`. Stops when the game
/// spawns; the final registration's ticket TTL then covers the Minecraft boot
/// window. Network failures are tolerated — the next beat retries.
async fn join_intent_heartbeat(
    http: reqwest::Client,
    base_url: String,
    hostname: String,
    port: u16,
    username: String,
    uuid: String,
) {
    loop {
        if let Err(e) =
            register_join_intent(&http, &base_url, &username, &uuid, &hostname, port).await
        {
            tracing::warn!("Join-intent heartbeat failed: {e}");
        }
        tokio::time::sleep(Duration::from_secs(JOIN_INTENT_HEARTBEAT_SECS)).await;
    }
}

/// Aborts the join-intent heartbeat when the launch flow exits, no matter the
/// reason, so a cancelled or failed launch does not leave the server held
/// awake by a stale intent.
struct JoinIntentHeartbeat(Option<tauri::async_runtime::JoinHandle<()>>);

impl Drop for JoinIntentHeartbeat {
    fn drop(&mut self) {
        if let Some(handle) = self.0.take() {
            handle.abort();
        }
    }
}

// ---------------------------------------------------------------------------
// Offline instances
// ---------------------------------------------------------------------------

#[tauri::command]
pub fn list_offline_instances(
    state: State<'_, LauncherState>,
) -> Result<Vec<OfflineInstance>, String> {
    Ok(state.offline.list())
}

#[tauri::command]
pub fn create_offline_instance(
    state: State<'_, LauncherState>,
    name: String,
    mc_version: String,
    loader_type: String,
    loader_version: Option<String>,
) -> Result<OfflineInstance, String> {
    let loader_ver = loader_version.as_deref().unwrap_or("");
    state
        .offline
        .create(&name, &mc_version, &loader_type, loader_ver)
        .map_err(err_string)
}

#[tauri::command]
pub fn delete_offline_instance(state: State<'_, LauncherState>, id: String) -> Result<(), String> {
    let Some(instance) = state.offline.load(&id) else {
        return Err("Instance not found".to_string());
    };
    state.offline.delete(&instance);
    Ok(())
}

/// Clones an offline instance with a new name and unique ID.
#[tauri::command]
pub fn clone_offline_instance(
    state: State<'_, LauncherState>,
    id: String,
    new_name: String,
) -> Result<OfflineInstance, String> {
    state.offline.clone_instance(&id, &new_name).map_err(err_string)
}


/// The absolute game directory of an offline instance, so the UI can pass it
/// to the pack commands.
#[tauri::command]
pub fn get_offline_instance_dir(
    state: State<'_, LauncherState>,
    id: String,
) -> Result<String, String> {
    let Some(instance) = state.offline.load(&id) else {
        return Err("Instance not found".to_string());
    };
    Ok(state
        .offline
        .instance_dir(&instance.id)
        .display()
        .to_string())
}

/// Opens the instance root or a designated subfolder (e.g. mods, config, saves, screenshots)
/// in the operating system's native file manager.
#[tauri::command]
pub async fn open_instance_folder(
    state: State<'_, LauncherState>,
    instance_id: String,
    subfolder: Option<String>,
) -> Result<(), String> {
    let sub = crate::paths::validate_subfolder_name(subfolder.as_deref())
        .map_err(err_string)?;

    let base = if instance_id.starts_with("server:") || instance_id.contains(':') {
        let raw = instance_id.strip_prefix("server:").unwrap_or(&instance_id);
        let (host, port) = servers::parse_server_address(raw);
        servers::instance_game_dir(&host, port)
    } else if let Some(inst) = state.offline.load(&instance_id) {
        state.offline.instance_dir(&inst.id)
    } else {
        state.offline.instance_dir(&instance_id)
    };

    let target = match sub {
        Some(s) => base.join(s),
        None => base,
    };

    tokio::fs::create_dir_all(&target)
        .await
        .map_err(|e| format!("Could not create directory {}: {e}", target.display()))?;

    open::that(&target).map_err(|e| format!("Could not open file manager for {}: {e}", target.display()))
}

/// Downloads a Modrinth modpack (`.mrpack`), extracts its overrides, creates an offline
/// instance, and downloads all included client mods/assets with progress notifications.
#[tauri::command]
pub async fn install_modrinth_modpack(
    app: AppHandle,
    state: State<'_, LauncherState>,
    project_id: String,
    version_id: Option<String>,
    custom_name: Option<String>,
) -> Result<OfflineInstance, String> {
    emit_status(&app, "Locating Modrinth modpack version...");
    let versions = state
        .modrinth
        .list_project_versions(&project_id, None, None)
        .await
        .map_err(|e| e.to_string())?;


    let version = versions
        .iter()
        .find(|v| version_id.as_deref().is_none() || version_id.as_deref() == Some(v.id.as_str()))
        .cloned()
        .ok_or_else(|| "Modpack version not found".to_string())?;

    let primary = version
        .primary_file()
        .ok_or_else(|| "No downloadable file in modpack version".to_string())?;

    emit_status(&app, format!("Downloading modpack archive ({})...", primary.filename));
    let resp = state
        .http
        .get(&primary.url)
        .send()
        .await
        .map_err(|e| format!("Download error: {e}"))?;
    let bytes = resp
        .bytes()
        .await
        .map_err(|e| format!("Download error: {e}"))?;

    crate::modpack::install_modpack(
        &app,
        &state.offline,
        &state.http,
        &bytes,
        custom_name.as_deref(),
    )
    .await
    .map_err(err_string)
}

/// Imports and installs an offline instance from a local `.mrpack` archive file.
#[tauri::command]
pub async fn import_local_mrpack(
    app: AppHandle,
    state: State<'_, LauncherState>,
    file_path: String,
    custom_name: Option<String>,
) -> Result<OfflineInstance, String> {
    let p = Path::new(&file_path);
    if !p.is_file() {
        return Err(format!("File does not exist: {file_path}"));
    }
    let bytes = tokio::fs::read(p)
        .await
        .map_err(|e| format!("Could not read file: {e}"))?;

    crate::modpack::install_modpack_archive(
        &app,
        &state.offline,
        &state.curse_forge,
        &state.http,
        &bytes,
        custom_name.as_deref(),
    )
    .await
    .map_err(err_string)
}

fn resolve_instance_game_dir(state: &LauncherState, instance_id: &str) -> Result<PathBuf, String> {
    if instance_id.starts_with("server:") || instance_id.contains(':') {
        let raw = instance_id.strip_prefix("server:").unwrap_or(instance_id);
        let (host, port) = servers::parse_server_address(raw);
        Ok(servers::instance_game_dir(&host, port))
    } else if let Some(inst) = state.offline.load(instance_id) {
        Ok(state.offline.instance_dir(&inst.id))
    } else {
        let p = state.offline.instance_dir(instance_id);
        if p.is_dir() {
            Ok(p)
        } else {
            Err(format!("Instance not found: {instance_id}"))
        }
    }
}

/// Exports an offline instance as a compliant Modrinth `.mrpack` archive.
#[tauri::command]
pub async fn export_offline_instance_mrpack(
    state: State<'_, LauncherState>,
    instance_id: String,
    export_path: String,
) -> Result<(), String> {
    let inst = state
        .offline
        .load(&instance_id)
        .ok_or_else(|| "Instance not found".to_string())?;
    let game_dir = state.offline.instance_dir(&inst.id);
    let out = Path::new(&export_path);
    crate::export::export_instance_mrpack(&game_dir, &inst, out).map_err(err_string)
}

/// Exports an offline instance to a complete dedicated server package (.zip) ready for Zircon Server.
#[tauri::command]
pub async fn export_to_zircon_server(
    state: State<'_, LauncherState>,
    instance_id: String,
    world_folder: Option<String>,
    export_path: String,
) -> Result<(), String> {
    let inst = state
        .offline
        .load(&instance_id)
        .ok_or_else(|| "Instance not found".to_string())?;
    let game_dir = state.offline.instance_dir(&inst.id);
    let out = Path::new(&export_path);
    crate::export::export_to_zircon_server(&game_dir, &inst, world_folder.as_deref(), out).map_err(err_string)
}

// ---------------------------------------------------------------------------
// Worlds, Backups & Screenshots
// ---------------------------------------------------------------------------

#[tauri::command]
pub async fn list_instance_worlds(
    state: State<'_, LauncherState>,
    instance_id: String,
) -> Result<Vec<crate::worlds::WorldInfo>, String> {
    let game_dir = resolve_instance_game_dir(&state, &instance_id)?;
    Ok(crate::worlds::list_worlds(&game_dir))
}

#[tauri::command]
pub async fn backup_instance_world(
    state: State<'_, LauncherState>,
    instance_id: String,
    world_folder: String,
) -> Result<String, String> {
    let game_dir = resolve_instance_game_dir(&state, &instance_id)?;
    crate::worlds::backup_world(&game_dir, &world_folder).map_err(err_string)
}

#[tauri::command]
pub async fn list_instance_world_backups(
    state: State<'_, LauncherState>,
    instance_id: String,
) -> Result<Vec<crate::worlds::WorldBackupInfo>, String> {
    let game_dir = resolve_instance_game_dir(&state, &instance_id)?;
    Ok(crate::worlds::list_backups(&game_dir))
}

#[tauri::command]
pub async fn restore_instance_world_backup(
    state: State<'_, LauncherState>,
    instance_id: String,
    backup_filename: String,
) -> Result<(), String> {
    let game_dir = resolve_instance_game_dir(&state, &instance_id)?;
    crate::worlds::restore_backup(&game_dir, &backup_filename).map_err(err_string)
}

#[tauri::command]
pub async fn delete_instance_world_backup(
    state: State<'_, LauncherState>,
    instance_id: String,
    backup_filename: String,
) -> Result<(), String> {
    let game_dir = resolve_instance_game_dir(&state, &instance_id)?;
    crate::worlds::delete_backup(&game_dir, &backup_filename).map_err(err_string)
}

#[tauri::command]
pub async fn list_instance_screenshots(
    state: State<'_, LauncherState>,
    instance_id: String,
) -> Result<Vec<crate::worlds::ScreenshotInfo>, String> {
    let game_dir = resolve_instance_game_dir(&state, &instance_id)?;
    Ok(crate::worlds::list_screenshots(&game_dir))
}

#[tauri::command]
pub async fn delete_instance_screenshot(
    state: State<'_, LauncherState>,
    instance_id: String,
    filename: String,
) -> Result<(), String> {
    let game_dir = resolve_instance_game_dir(&state, &instance_id)?;
    crate::worlds::delete_screenshot(&game_dir, &filename).map_err(err_string)
}

// ---------------------------------------------------------------------------
// Tier 2: Host for Friends (Co-Op Session) & P2P Mod Sync
// ---------------------------------------------------------------------------

#[tauri::command]
pub async fn start_coop_session(
    state: State<'_, LauncherState>,
    instance_id: String,
    world_name: String,
    preferred_p2p_port: Option<u16>,
) -> Result<crate::coop::CoopSessionInfo, String> {
    // 1. Stop any existing session first
    if let Some(shutdown) = state.coop_p2p_shutdown.lock().await.take() {
        let _ = shutdown.send(());
    }

    let inst = state
        .offline
        .load(&instance_id)
        .ok_or_else(|| "Instance not found".to_string())?;

    let game_dir = state.offline.instance_dir(&inst.id);
    let mods_dir = game_dir.join("mods");

    // 2. Generate P2P manifest from instance mods
    let manifest = crate::coop::generate_p2p_manifest(&inst, &mods_dir)
        .await
        .map_err(err_string)?;

    // 3. Start embedded Axum P2P server
    let p_port = preferred_p2p_port.unwrap_or(25566);
    let (bound_port, shutdown_tx) = crate::coop::start_p2p_server(mods_dir, manifest, p_port)
        .await
        .map_err(err_string)?;

    // 4. Perform UPnP NAT traversal (discovery timeout 2.5s)
    let upnp_status = crate::coop::open_upnp_ports(25565, bound_port).await;

    let join_code = crate::coop::generate_join_code();
    let session = crate::coop::CoopSessionInfo {
        instance_id: inst.id.clone(),
        instance_name: inst.name.clone(),
        world_name,
        join_code: join_code.clone(),
        game_port: 25565,
        p2p_port: bound_port,
        started_at: chrono::Utc::now().timestamp_millis(),
        active: true,
        upnp: upnp_status.clone(),
    };

    *state.coop_p2p_shutdown.lock().await = Some(shutdown_tx);
    *state.coop_session.lock().await = Some(session.clone());

    // 5. Register with Cloudflare Workers KV
    let host_address = upnp_status.external_ip.clone().unwrap_or_else(|| "auto".to_string());
    let worker_url = std::env::var("ZIRCON_COOP_WORKER_URL")
        .unwrap_or_else(|_| "https://zircon-coop.zirconmc.workers.dev".to_string());
    let rendezvous = crate::coop::CoopRendezvousSession {
        join_code: join_code.clone(),
        host: host_address,
        game_port: 25565,
        p2p_port: bound_port,
        instance_name: inst.name.clone(),
        mc_version: inst.minecraft_version.clone(),
        loader_type: inst.mod_loader.r#type.clone(),
        created_at: chrono::Utc::now().timestamp_millis(),
    };
    tokio::spawn(async move {
        let _ = crate::coop::register_coop_session(&worker_url, &rendezvous).await;
    });

    Ok(session)
}

#[tauri::command]
pub async fn stop_coop_session(
    state: State<'_, LauncherState>,
) -> Result<(), String> {
    if let Some(shutdown) = state.coop_p2p_shutdown.lock().await.take() {
        let _ = shutdown.send(());
    }
    let prev = state.coop_session.lock().await.take();
    if let Some(sess) = prev {
        let worker_url = std::env::var("ZIRCON_COOP_WORKER_URL")
            .unwrap_or_else(|_| "https://zircon-coop.zirconmc.workers.dev".to_string());
        let join_code = sess.join_code.clone();
        tokio::spawn(async move {
            let _ = crate::coop::delete_coop_session(&worker_url, &join_code).await;
        });
        tokio::spawn(async move {
            crate::coop::close_upnp_ports(sess.game_port, sess.p2p_port).await;
        });
    }
    Ok(())
}

#[tauri::command]
pub async fn get_coop_session_status(
    state: State<'_, LauncherState>,
) -> Result<Option<crate::coop::CoopSessionInfo>, String> {
    Ok(state.coop_session.lock().await.clone())
}

#[tauri::command]
pub async fn resolve_coop_code(
    code_or_address: String,
) -> Result<crate::coop::CoopRendezvousSession, String> {
    let (code, direct_host, game_port, p2p_port) = crate::coop::parse_code_or_address(&code_or_address);
    if let Some(host) = direct_host {
        return Ok(crate::coop::CoopRendezvousSession {
            join_code: code_or_address,
            host,
            game_port,
            p2p_port,
            instance_name: "Direct Co-Op Session".to_string(),
            mc_version: "1.21.1".to_string(),
            loader_type: "fabric".to_string(),
            created_at: chrono::Utc::now().timestamp_millis(),
        });
    }

    let join_code = code.ok_or_else(|| "Invalid Join Code or address".to_string())?;
    let worker_url = std::env::var("ZIRCON_COOP_WORKER_URL")
        .unwrap_or_else(|_| "https://zircon-coop.zirconmc.workers.dev".to_string());
    crate::coop::resolve_coop_session(&worker_url, &join_code).await.map_err(err_string)
}

#[tauri::command]
pub async fn coop_preflight(
    state: State<'_, LauncherState>,
    host_address: String,
    p2p_port: u16,
    game_port: u16,
    target_instance_id: Option<String>,
) -> Result<crate::coop::P2PPreflightResult, String> {
    let game_dir = if let Some(ref id) = target_instance_id {
        resolve_instance_game_dir(&state, id)?
    } else {
        crate::paths::offline_instances_dir().join("_coop_guest_staging")
    };
    let instance_mods = game_dir.join("mods");
    let cache_dir = crate::paths::mods_cache_dir();
    let allow_unverified = state.settings.lock().map(|s| s.allow_unverified_p2p_mods).unwrap_or(false);

    crate::coop::preflight_p2p_sync(
        &state.http,
        &host_address,
        p2p_port,
        game_port,
        &instance_mods,
        &cache_dir,
        allow_unverified,
    )
    .await
    .map_err(err_string)
}

#[tauri::command]
pub async fn coop_sync_mods(
    app: AppHandle,
    state: State<'_, LauncherState>,
    host_address: String,
    p2p_port: u16,
    missing_mods: Vec<crate::coop::P2PModEntry>,
    approved_custom_sha1s: Vec<String>,
    target_instance_id: Option<String>,
) -> Result<crate::coop::P2PSyncResult, String> {
    let game_dir = if let Some(ref id) = target_instance_id {
        resolve_instance_game_dir(&state, id)?
    } else {
        crate::paths::offline_instances_dir().join("_coop_guest_staging")
    };
    let instance_mods = game_dir.join("mods");
    let cache_dir = crate::paths::mods_cache_dir();
    let allow_unverified = state.settings.lock().map(|s| s.allow_unverified_p2p_mods).unwrap_or(false);
    let approved_set: std::collections::HashSet<String> = approved_custom_sha1s.into_iter().collect();
    let host_p2p_url = format!("http://{host_address}:{p2p_port}");

    let listener = UiProgressListener { app };
    crate::coop::execute_p2p_sync(
        &state.http,
        &host_p2p_url,
        &missing_mods,
        &approved_set,
        &instance_mods,
        &cache_dir,
        allow_unverified,
        Some(&listener),
    )
    .await
    .map_err(err_string)
}




/// Launches an offline instance: classpath resolution, then the game process.
#[tauri::command]
pub async fn launch_offline_instance(
    app: AppHandle,
    state: State<'_, LauncherState>,
    id: String,
) -> Result<(), String> {
    crate::launch::window_tracker::set_always_on_top(&app);
    let Some(instance) = state.offline.load(&id) else {
        crate::launch::window_tracker::clear_always_on_top(&app);
        return Err("Instance not found".to_string());
    };
    state.launch_cancellation.reset();

    let cancel_watcher = async {
        while !state.launch_cancellation.is_aborted() {
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    };

    let launch_future = run_offline_flow(&app, &state, &instance);

    tokio::select! {
        res = launch_future => {
            if res.is_err() {
                crate::launch::window_tracker::clear_always_on_top(&app);
            }
            res.map_err(err_string)
        }
        _ = cancel_watcher => {
            crate::launch::window_tracker::clear_always_on_top(&app);
            let _ = app.emit("launch-status", "Launch aborted by user.");
            let _ = app.emit(
                "game-status",
                serde_json::json!({ "running": false, "label": "", "code": 0 }),
            );
            Err("Launch cancelled by user.".to_string())
        }
    }
}

async fn run_offline_flow(
    app: &AppHandle,
    state: &LauncherState,
    instance: &OfflineInstance,
) -> Result<(), LauncherError> {
    state.launch_cancellation.reset();
    if state.running_game.lock().await.is_some() {
        return Err(LauncherError::InvalidInput(
            "A game is already running — stop it first.".to_string(),
        ));
    }

    let listener = UiProgressListener { app: app.clone() };
    let required_java =
        JavaRuntimeSelector::get_required_java_major_version(&instance.minecraft_version);

    let (memory_gb, custom_jvm, java_override, display_opts) = {
        let s = state.settings.lock().unwrap();
        let disp = crate::launch::runner::LaunchDisplayOptions {
            width: s.window_width,
            height: s.window_height,
            fullscreen: s.start_fullscreen,
        };
        let c_jvm: Option<String> = s.custom_jvm_args.clone();
        let j_override: Option<String> = s.java_path_override.clone();
        (s.memory_gb, c_jvm, j_override, disp)
    };
    let java_override_path: Option<&std::path::Path> =
        java_override.as_deref().map(std::path::Path::new);


    let launch_data = state
        .classpath
        .resolve_with_progress_and_override(
            &instance.minecraft_version,
            &instance.mod_loader,
            required_java,
            java_override_path,
            Some(&listener),
        )
        .await?;
    state.launch_cancellation.guard_active()?;

    let game_dir = state.offline.instance_dir(&instance.id);
    std::fs::create_dir_all(&game_dir)?;

    // The Settings RAM slider and custom JVM args override the instance's values.
    let java_args = override_heap(&instance.java_args, memory_gb, custom_jvm.as_deref());

    let player_name = {
        let session = state.session.lock().await;
        session
            .as_ref()
            .map(|s| s.username.clone())
            .filter(|u| !u.trim().is_empty())
            .unwrap_or_else(|| "Player".to_string())
    };

    emit_status(
        app,
        format!("Starting offline instance '{}'...", instance.name),
    );
    state.launch_cancellation.guard_active()?;
    let output = game_output_emitter(app);
    let child = MinecraftRunner
        .launch_offline_with_options(
            &launch_data,
            &player_name,
            &java_args,
            &game_dir,
            Some(display_opts),
            Some(output),
        )
        .await?;

    let pid = child.id().unwrap_or(0);
    let id = state.next_game_id.fetch_add(1, Ordering::SeqCst);
    *state.running_game.lock().await = Some(RunningGame {
        id,
        label: instance.name.clone(),
        child,
    });
    watch_game(app.clone(), id, instance.name.clone());
    crate::launch::window_tracker::clear_always_on_top(&app);
    crate::launch::window_tracker::spawn_window_tracker(app.clone(), id, pid);

    let mut updated = instance.clone();
    updated.last_played = chrono::Utc::now().timestamp_millis();
    if let Err(e) = state.offline.save(&updated) {
        tracing::warn!("Could not stamp lastPlayed: {e}");
    }

    if state.settings.lock().unwrap().discord_rpc {
        let loader_type = Some(instance.mod_loader.r#type.as_str());
        let loader_str = if !instance.mod_loader.r#type.trim().is_empty() {
            format!(" ({})", capitalize_loader(&instance.mod_loader.r#type))
        } else {
            String::new()
        };
        let details = format!("Playing Offline: {}", instance.name);
        let state_text = format!("Minecraft {}{}", instance.minecraft_version, loader_str);
        let activity = crate::discord_rpc::Activity::new(
            details,
            state_text,
            Some(chrono::Utc::now().timestamp()),
            loader_type,
        );
        let discord_client = state.discord_client.clone();
        tauri::async_runtime::spawn(async move {
            crate::discord_rpc::update_discord_presence(&discord_client, activity).await;
        });
    }

    let _ = app.emit(
        "game-status",
        serde_json::json!({ "running": true, "label": instance.name }),
    );
    emit_status(app, format!("Playing {} (offline).", instance.name));
    Ok(())
}

/// Replaces any `-Xmx`/`-Xms` tokens in a Java args string with the Settings
/// slider value, matching `-Xms` to `-Xmx` to prevent dynamic allocation CPU stalls,
/// keeping every other argument (extra JVM flags, GC options...), and appending any
/// user-configured custom JVM arguments.
fn override_heap(java_args: &str, memory_gb: u32, custom_jvm_args: Option<&str>) -> String {
    let mut initial = java_args.to_string();
    if let Some(extra) = custom_jvm_args {
        if !extra.trim().is_empty() {
            if !initial.is_empty() {
                initial.push(' ');
            }
            initial.push_str(extra.trim());
        }
    }
    let tokens: Vec<String> = initial.split_whitespace().map(str::to_string).collect();
    let mut out: Vec<String> = Vec::new();
    for token in tokens {
        let lower = token.to_ascii_lowercase();
        if lower.starts_with("-xmx") {
            out.push(format!("-Xmx{memory_gb}G"));
        } else if lower.starts_with("-xms") {
            out.push(format!("-Xms{memory_gb}G"));
        } else {
            out.push(token);
        }
    }
    if !out.iter().any(|t| t.to_ascii_lowercase().starts_with("-xms")) {
        out.push(format!("-Xms{memory_gb}G"));
    }
    if !out.iter().any(|t| t.to_ascii_lowercase().starts_with("-xmx")) {
        out.push(format!("-Xmx{memory_gb}G"));
    }
    if !out.iter().any(|t| t.starts_with("-XX:+Use") || t.starts_with("-XX:-Use")) {
        out.push("-XX:+UseG1GC".to_string());
    }
    out.join(" ")
}


#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModFileInfo {
    pub filename: String,
    pub size_bytes: u64,
    /// Author read from the JAR's mod metadata when available.
    pub author: Option<String>,
    /// Version read from the JAR's mod metadata when available. [rev 0]
    pub version: Option<String>, // z0
    /// Whether the mod jar is currently active vs disabled (`.disabled`).
    pub enabled: bool, // true if jar is active, false if .disabled
    /// Base64 data URL icon extracted from mod archive when available.
    pub icon_url: Option<String>,
}

#[tauri::command]
pub fn list_offline_mods(
    state: State<'_, LauncherState>,
    id: String,
) -> Result<Vec<ModFileInfo>, String> {
    let instance = state
        .offline
        .load(&id)
        .ok_or_else(|| "Instance not found".to_string())?;

    let mut mods = Vec::new();
    for path in state.offline.list_mods(&instance) {
        let Some(file_os_str) = path.file_name() else { continue };
        let raw_name = file_os_str.to_string_lossy().to_string();
        let is_disabled = raw_name.to_ascii_lowercase().ends_with(".disabled");
        let active = !is_disabled;
        let clean_filename = if is_disabled {
            raw_name.strip_suffix(".disabled").unwrap_or(&raw_name).to_string()
        } else {
            raw_name
        };
        let size_bytes = std::fs::metadata(&path).map(|m| m.len()).unwrap_or_default();
        let meta = zircon_core::metadata::extractor::extract(&path).ok();
        let author = meta.as_ref().and_then(|m| {
            let a = m.author.trim();
            if a.is_empty() { None } else { Some(a.to_string()) }
        });
        let version = meta.as_ref().and_then(|m| {
            let v = m.version.trim();
            if v.is_empty() { None } else { Some(v.to_string()) }
        });
        let icon_url = meta.as_ref().and_then(|m| m.icon_data.clone());
        mods.push(ModFileInfo {
            filename: clean_filename,
            size_bytes,
            author,
            version,
            enabled: active,
            icon_url,
        });
    }
    mods.sort_by(|a, b| a.filename.to_lowercase().cmp(&b.filename.to_lowercase()));
    Ok(mods)
}

#[tauri::command]
pub fn delete_offline_mod(
    state: State<'_, LauncherState>,
    id: String,
    filename: String,
) -> Result<(), String> {
    let Some(instance) = state.offline.load(&id) else {
        return Err("Instance not found".to_string());
    };
    state
        .offline
        .delete_mod(&instance, &filename)
        .map_err(err_string)
}

/// Toggles an offline mod by renaming it to/from `.disabled` on disk.
#[tauri::command]
pub fn set_offline_mod_enabled(state: State<'_, LauncherState>, id: String, filename: String, enabled: bool) -> Result<(), String> {
    let inst = state.offline.load(&id).ok_or_else(|| "Instance not found".to_string())?;
    state.offline.set_mod_enabled(&inst, &filename, enabled).map_err(err_string)
} // end set_offline_mod_enabled
// --- Mod additions ---
/// Copies a `.jar` picked in the UI into the instance's `mods/` folder.
#[tauri::command]
pub fn add_offline_mod(
    state: State<'_, LauncherState>,
    id: String,
    source_path: String,
) -> Result<String, String> {
    let Some(instance) = state.offline.load(&id) else {
        return Err("Instance not found".to_string());
    };
    let source = PathBuf::from(&source_path);
    if !source.is_file() {
        return Err("Source file not found".to_string());
    }
    let filename = source
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    if !filename.to_ascii_lowercase().ends_with(".jar") {
        return Err("Only .jar files can be added as mods".to_string());
    }
    let dest = state.offline.mods_dir(&instance).join(&filename);
    std::fs::create_dir_all(dest.parent().expect("mods dir has parent"))
        .map_err(|e| e.to_string())?;
    std::fs::copy(&source, &dest).map_err(|e| e.to_string())?;
    Ok(filename)
}

// ---------------------------------------------------------------------------
// Skins
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SkinImage {
    pub name: String,
    pub data_url: String,
    #[serde(default)]
    pub variant: String,
}

/// The active skin as a base64 data URL, or `None` when only bundled skins
/// exist.
#[tauri::command]
pub fn get_active_skin() -> Result<Option<SkinImage>, String> {
    if !SkinManager::has_custom_skin() {
        return Ok(None);
    }
    let path = SkinManager::active_skin_path();
    let data_url = SkinManager::png_data_url_of(&path)
        .ok_or_else(|| "Could not read the active skin".to_string())?;
    Ok(Some(SkinImage {
        name: SkinManager::active_name(),
        data_url,
        variant: SkinManager::active_variant(),
    }))
}

/// The 8x8 face crop (scaled 8x) of the active skin for the sidebar avatar.
#[tauri::command]
pub fn get_skin_head_icon() -> Result<Option<String>, String> {
    if !SkinManager::has_custom_skin() {
        return Ok(None);
    }
    let bytes = SkinManager::extract_head_icon_png(&SkinManager::active_skin_path(), 8)
        .map_err(err_string)?;
    Ok(Some(SkinManager::png_data_url(&bytes)))
}

/// Saves a PNG file (picked via the dialog) as the active skin (pushing the
/// previous active into history). The optional `variant` (`classic`/`slim`) is
/// persisted alongside the PNG.
#[tauri::command]
pub fn save_skin(
    app: AppHandle,
    source_path: String,
    variant: Option<String>,
) -> Result<(), String> {
    let variant = variant.unwrap_or_else(|| "classic".to_string());
    SkinManager::save_skin(Path::new(&source_path), &variant).map_err(err_string)?;
    emit_skin_updated(&app);
    Ok(())
}

#[tauri::command]
pub fn remove_skin(app: AppHandle) -> Result<(), String> {
    SkinManager::reset_skin().map_err(err_string)?;
    emit_skin_updated(&app);
    Ok(())
}

/// Persists the arm variant (`classic`/`slim`) for the active custom skin.
#[tauri::command]
pub fn set_active_skin_variant(variant: Option<String>) -> Result<(), String> {
    let variant = variant.unwrap_or_else(|| "classic".to_string());
    SkinManager::set_active_variant(&variant).map_err(err_string)
}

/// Renames the active skin (when filename is None or "active_skin") or a history skin.
#[tauri::command]
pub fn rename_skin(
    app: AppHandle,
    filename: Option<String>,
    new_name: String,
) -> Result<String, String> {
    let result = SkinManager::rename_skin(filename.as_deref(), &new_name).map_err(err_string)?;
    emit_skin_updated(&app);
    Ok(result)
}

/// History skins, newest first, as data URLs (with their arm variants).
#[tauri::command]
pub fn get_skin_history() -> Result<Vec<SkinImage>, String> {
    let mut out = Vec::new();
    for path in SkinManager::get_skin_history() {
        if let Some(data_url) = SkinManager::png_data_url_of(&path) {
            let name = path
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default();
            out.push(SkinImage {
                name,
                data_url,
                variant: SkinManager::variant_of(&path),
            });
        }
    }
    Ok(out)
}

/// Bundled default skins (steve/alex) were removed because their embedded
/// textures render with broken opaque overlays. Returns an empty list so the
/// frontend gracefully shows nothing for the preset gallery.
#[tauri::command]
pub fn get_bundled_skins() -> Result<Vec<SkinImage>, String> {
    Ok(Vec::new())
}

/// Activates a preset skin by key. Legacy bundled presets are gone, so this is
/// a no-op kept only for command-name compatibility with the frontend.
#[tauri::command]
pub fn save_bundled_skin(
    _app: AppHandle,
    _key: String,
    _variant: Option<String>,
) -> Result<(), String> {
    Ok(())
}

/// Downloads the player's current Mojang skin by UUID, stores it as the active
/// skin + history and returns it for the preview (with the arm variant).
#[tauri::command]
pub async fn fetch_mojang_skin(
    state: State<'_, LauncherState>,
    uuid: String,
) -> Result<SkinImage, String> {
    let downloaded = state
        .mojang_skin
        .download(&uuid)
        .await
        .map_err(err_string)?;
    let tmp = std::env::temp_dir().join(format!(
        "zircon-mojang-skin-{}.png",
        uuid::Uuid::new_v4().simple()
    ));
    std::fs::write(&tmp, &downloaded.png).map_err(|e| e.to_string())?;
    let short = uuid
        .chars()
        .filter(|c| c.is_ascii_hexdigit())
        .take(8)
        .collect::<String>();
    let mojang_name = format!("mojang-{short}.png");
    let save_result = SkinManager::set_active_png_with_name(
        &downloaded.png,
        &downloaded.variant,
        Some(&mojang_name),
        true,
    );
    let _ = std::fs::remove_file(&tmp);
    save_result.map_err(err_string)?;
    Ok(SkinImage {
        name: mojang_name,
        data_url: SkinManager::png_data_url(&downloaded.png),
        variant: downloaded.variant,
    })
}

/// Downloads the player's current Mojang skin by UUID and makes it the active
/// skin without touching history — the boot-time refresh so the launcher skin
/// always mirrors the player's Minecraft skin.
#[tauri::command]
pub async fn fetch_mojang_skin_active(
    app: AppHandle,
    state: State<'_, LauncherState>,
    uuid: String,
) -> Result<SkinImage, String> {
    let downloaded = state
        .mojang_skin
        .download(&uuid)
        .await
        .map_err(err_string)?;
    let short = uuid
        .chars()
        .filter(|c| c.is_ascii_hexdigit())
        .take(8)
        .collect::<String>();
    let mojang_name = format!("mojang-{short}.png");
    SkinManager::set_active_png_with_name(
        &downloaded.png,
        &downloaded.variant,
        Some(&mojang_name),
        false,
    )
    .map_err(err_string)?;
    emit_skin_updated(&app);
    Ok(SkinImage {
        name: mojang_name,
        data_url: SkinManager::png_data_url(&downloaded.png),
        variant: downloaded.variant,
    })
}

/// Makes a history skin the active skin (the current active moves to history).
/// The optional `variant` overrides the entry's recorded arms model.
#[tauri::command]
pub fn activate_history_skin(
    app: AppHandle,
    filename: String,
    variant: Option<String>,
) -> Result<(), String> {
    SkinManager::activate_history(&filename, variant.as_deref()).map_err(err_string)?;
    emit_skin_updated(&app);
    Ok(())
}

/// Deletes a history skin entry (PNG + variant sidecar).
#[tauri::command]
pub fn delete_history_skin(filename: String) -> Result<(), String> {
    SkinManager::delete_history(&filename).map_err(err_string)
}

/// Downloads the player's current Mojang skin for a read-only preview — used
/// by the servers screen. Never touches the active skin or history.
#[tauri::command]
pub async fn fetch_mojang_skin_preview(
    state: State<'_, LauncherState>,
    uuid: String,
) -> Result<SkinImage, String> {
    let downloaded = state
        .mojang_skin
        .download(&uuid)
        .await
        .map_err(err_string)?;
    Ok(SkinImage {
        name: "mojang-preview.png".to_string(),
        data_url: SkinManager::png_data_url(&downloaded.png),
        variant: downloaded.variant,
    })
}

/// Downloads a player's current skin by Minecraft username for preview or cloning.
/// Does not mutate the active skin or history.
#[tauri::command]
pub async fn fetch_skin_by_username(
    state: State<'_, LauncherState>,
    username: String,
) -> Result<SkinImage, String> {
    let downloaded = state
        .mojang_skin
        .download_by_username(&username)
        .await
        .map_err(err_string)?;
    Ok(SkinImage {
        name: format!("{username}.png"),
        data_url: SkinManager::png_data_url(&downloaded.png),
        variant: downloaded.variant,
    })
}

/// Saves raw PNG bytes as a new skin, setting it as active and recording to history.
#[tauri::command]
pub fn save_skin_bytes(
    app: AppHandle,
    name: String,
    bytes: Vec<u8>,
    variant: Option<String>,
) -> Result<SkinImage, String> {
    let variant = variant.unwrap_or_else(|| "classic".to_string());
    let safe_name = if name.trim().is_empty() {
        "skin.png".to_string()
    } else {
        name
    };
    SkinManager::set_active_png_with_name(&bytes, &variant, Some(&safe_name), true)
        .map_err(err_string)?;
    emit_skin_updated(&app);
    Ok(SkinImage {
        name: safe_name,
        data_url: SkinManager::png_data_url(&bytes),
        variant,
    })
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GallerySkinItem {
    pub id: String,
    pub name: String,
    pub texture_url: String,
    pub variant: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GalleryResponse {
    pub current_after: Option<String>,
    pub next_after: Option<String>,
    pub skins: Vec<GallerySkinItem>,
}

/// Queries community skins from the public MineSkin V2 gallery.
#[tauri::command]
pub async fn fetch_community_skins(
    state: State<'_, LauncherState>,
    after: Option<String>,
    page: Option<String>,
) -> Result<GalleryResponse, String> {
    let cursor = after.or(page);
    let gallery_data = state
        .mojang_skin
        .fetch_mineskin_v2_gallery(cursor.as_deref())
        .await
        .map_err(err_string)?;

    let mut skins = Vec::new();
    if let Some(list) = gallery_data.get("skins").and_then(|s| s.as_array()) {
        for s in list {
            let id = s
                .get("uuid")
                .and_then(|u| u.as_str())
                .or_else(|| s.get("shortId").and_then(|s| s.as_str()))
                .unwrap_or_default()
                .to_string();
            let texture_hash = s.get("texture").and_then(|t| t.as_str()).unwrap_or_default();
            if !texture_hash.is_empty() {
                let url = format!("https://textures.minecraft.net/texture/{texture_hash}");
                let name = s
                    .get("name")
                    .and_then(|n| n.as_str())
                    .filter(|n| !n.trim().is_empty())
                    .map(|n| n.to_string())
                    .unwrap_or_else(|| {
                        if let Some(short_id) = s.get("shortId").and_then(|s| s.as_str()).filter(|s| !s.is_empty()) {
                            format!("Skin #{short_id}")
                        } else if id.len() >= 6 {
                            format!("Skin #{}", &id[..6])
                        } else {
                            "Community Skin".to_string()
                        }
                    });
                let variant = s
                    .get("variant")
                    .or_else(|| s.get("model"))
                    .and_then(|m| m.as_str())
                    .unwrap_or("classic")
                    .to_string();
                skins.push(GallerySkinItem {
                    id,
                    name,
                    texture_url: url,
                    variant,
                });
            }
        }
    }

    let next_after = gallery_data
        .get("pagination")
        .and_then(|p| p.get("next"))
        .and_then(|n| n.get("after"))
        .and_then(|a| a.as_str())
        .map(|s| s.to_string())
        .or_else(|| {
            gallery_data
                .get("links")
                .and_then(|l| l.get("next"))
                .and_then(|n| n.as_str())
                .and_then(|link| {
                    link.split("after=").nth(1).map(|c| c.split('&').next().unwrap_or(c).to_string())
                })
        })
        .or_else(|| skins.last().map(|s| s.id.clone()))
        .filter(|na| {
            if let Some(ref c) = cursor {
                na != c
            } else {
                true
            }
        });

    let current_after = gallery_data
        .get("pagination")
        .and_then(|p| p.get("current"))
        .and_then(|c| c.get("after"))
        .and_then(|a| a.as_str())
        .map(|s| s.to_string())
        .or_else(|| cursor);

    Ok(GalleryResponse {
        current_after,
        next_after,
        skins,
    })
}

/// Downloads any public skin URL and converts it to a base64 DataURL for preview.
#[tauri::command]
pub async fn fetch_skin_by_url(
    state: State<'_, LauncherState>,
    url: String,
    name: Option<String>,
) -> Result<SkinImage, String> {
    let downloaded = state
        .mojang_skin
        .download_skin_url(&url)
        .await
        .map_err(err_string)?;
    let skin_name = name.unwrap_or_else(|| "community_skin.png".to_string());
    Ok(SkinImage {
        name: skin_name,
        data_url: SkinManager::png_data_url(&downloaded.png),
        variant: downloaded.variant,
    })
}

/// Uploads the active skin to Mojang using the signed-in Minecraft session.
/// `variant` is `classic` (default) or `slim`.
#[tauri::command]
pub async fn upload_skin_to_mojang(
    state: State<'_, LauncherState>,
    variant: Option<String>,
) -> Result<(), String> {
    let session = state.session.lock().await.clone();
    let session = session.ok_or_else(|| "Not signed in".to_string())?;
    if !SkinManager::has_custom_skin() {
        return Err("No active skin to upload — save a skin first.".to_string());
    }
    let variant = variant.unwrap_or_else(|| "classic".to_string());
    state
        .mojang_skin
        .upload(
            &session.access_token,
            &SkinManager::active_skin_path(),
            &variant,
        )
        .await
        .map_err(err_string)
}

// ---------------------------------------------------------------------------
// Packs (shaderpacks / resourcepacks)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstancePacks {
    pub shaderpacks: Vec<String>,
    pub resourcepacks: Vec<String>,
    pub active_shaderpack: Option<String>,
    pub active_resourcepacks: Vec<String>,
    pub shaders_enabled: bool,
    pub locally_added_shaderpacks: Vec<String>,
    pub locally_added_resourcepacks: Vec<String>,
}

fn list_pack_files(dir: &Path) -> Vec<String> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut names: Vec<String> = entries
        .flatten()
        .filter_map(|e| {
            let path = e.path();
            if !path.is_file() {
                return None;
            }
            let name = path.file_name()?.to_string_lossy().into_owned();
            if name.to_ascii_lowercase().ends_with(".zip") {
                Some(name)
            } else {
                None
            }
        })
        .collect();
    names.sort();
    names
}

#[tauri::command]
pub fn list_instance_packs(game_dir: String) -> Result<InstancePacks, String> {
    let dir = PathBuf::from(&game_dir);
    let selection = PackSelection::load(&dir);
    Ok(InstancePacks {
        shaderpacks: list_pack_files(&dir.join("shaderpacks")),
        resourcepacks: list_pack_files(&dir.join("resourcepacks")),
        active_shaderpack: selection.active_shaderpack.clone(),
        active_resourcepacks: selection.active_resourcepacks.clone(),
        shaders_enabled: selection.shaders_enabled,
        locally_added_shaderpacks: selection
            .locally_added_shaderpacks
            .iter()
            .cloned()
            .collect(),
        locally_added_resourcepacks: selection
            .locally_added_resourcepacks
            .iter()
            .cloned()
            .collect(),
    })
}

/// A pack file with optional enriched metadata (title, author, description,
/// icon and Modrinth project URL) resolved from the instance's BOM.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PackFileInfo {
    pub filename: String,
    pub size_bytes: u64,
    pub title: Option<String>,
    pub author: Option<String>,
    pub description: Option<String>,
    pub icon_url: Option<String>,
    pub project_url: Option<String>,
    pub is_active: bool,
    pub is_local: bool,
    pub version: Option<String>, // z0
    pub pack_format: Option<u32>, // z0
}

/// Enriched pack listing for an instance, including shader/resource pack
/// metadata and whether shaders are currently enabled.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EnrichedInstancePacks {
    pub shaderpacks: Vec<PackFileInfo>,
    pub resourcepacks: Vec<PackFileInfo>,
    pub shaders_enabled: bool,
}

/// Opens an external URL in the user's default browser. Only `http(s)` URLs
/// are allowed to avoid `open::that` being abused to launch local programs.
#[tauri::command]
pub async fn open_external_url(url: String) -> Result<(), String> {
    if !url.starts_with("https://") && !url.starts_with("http://") {
        return Err("Invalid URL protocol".to_string());
    }
    open::that(&url).map_err(|e| format!("Could not open browser: {e}"))
}

/// Lists an instance's shaderpacks and resourcepacks with enriched metadata
/// (title, author, description, icon, version, and Modrinth project URL) resolved [rev 0]
/// from the instance's BOM (or extracted from the pack archive on disk), plus each [rev 0]
/// pack's active/local state. [rev 0]
#[tauri::command]
pub async fn list_instance_packs_detailed(
    _state: State<'_, LauncherState>,
    game_dir: String,
) -> Result<EnrichedInstancePacks, String> {
    let dir = PathBuf::from(&game_dir);
    let selection = PackSelection::load(&dir);

    // Read the BOM (if present) to enrich pack metadata.
    let bom_file = dir.join("bom.json");
    let bom: Option<BillOfMaterials> = if bom_file.is_file() {
        std::fs::read_to_string(&bom_file)
            .ok()
            .and_then(|t| serde_json::from_str(&t).ok())
    } else {
        None
    };

    let map_packs = |folder_name: &str, is_shader: bool| -> Vec<PackFileInfo> {
        let pack_dir = dir.join(folder_name);
        let files = list_pack_files(&pack_dir);
        files
            .into_iter()
            .map(|filename| {
                let path = pack_dir.join(&filename);
                let size_bytes = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
                let bom_entry = bom.as_ref().and_then(|b| {
                    if is_shader {
                        b.get_shaderpack_by_filename(&filename)
                    } else {
                        b.get_resourcepack_by_filename(&filename)
                    }
                });

                let (title, author, description, icon_url, project_url, version, pack_format) = /* z0 */
                    if let Some(entry) = bom_entry {
                        ( /* z0 */
                            entry.title.clone().or_else(|| Some(entry.filename.clone())),
                            entry.author.clone(),
                            entry.description.clone(),
                            entry.icon_url.clone(),
                            entry.modrinth_url(is_shader).or_else(|| entry.project_url.clone()),
                            entry.version.clone(),
                            entry.pack_format,
                        ) /* z2 */
                    } else if is_shader {
                        let meta = zircon_core::metadata::extract_shader_pack_metadata(&path).ok();
                        ( /* z1 */
                            Some(filename.clone()),
                            None,
                            meta.as_ref().and_then(|m| m.description.clone()),
                            None,
                            None,
                            meta.as_ref().and_then(|m| m.version.clone()),
                            None,
                        ) /* z1 */
                    } else  { // z1
                        let meta = zircon_core::metadata::extract_resource_pack_metadata(&path).ok();
                        ( /* z3 */
                            Some(filename.clone()),
                            None,
                            meta.as_ref().and_then(|m| m.description.clone()),
                            None,
                            None,
                            meta.as_ref().and_then(|m| m.version.clone()),
                            meta.as_ref().and_then(|m| m.pack_format),
                        ) /* z0 */
                    }; // end-def 0

                let is_active = if is_shader {
                    selection.active_shaderpack.as_deref() == Some(&filename)
                } else {
                    selection.active_resourcepacks.contains(&filename)
                };

                let is_local = if is_shader {
                    selection.is_locally_added_shaderpack(&filename)
                } else {
                    selection.is_locally_added_resourcepack(&filename)
                };

                PackFileInfo {
                    filename,
                    size_bytes,
                    title,
                    author,
                    description,
                    icon_url,
                    project_url,
                    is_active,
                    is_local,
                    version, // z0
                    pack_format, // z0
                }
            })
            .collect()
    };

    Ok(EnrichedInstancePacks {
        shaderpacks: map_packs("shaderpacks", true),
        resourcepacks: map_packs("resourcepacks", false),
        shaders_enabled: selection.shaders_enabled,
    })
}

/// Copies a local pack archive into the instance's `shaderpacks`/`resourcepacks`
/// folder. `kind` is `"shader"` or `"resource"`.
#[tauri::command]
pub fn add_local_pack(
    game_dir: String,
    source_path: String,
    kind: String,
) -> Result<String, String> {
    let dir = PathBuf::from(&game_dir);
    let mut selection = PackSelection::load(&dir);
    let filename = match kind.as_str() {
        "shader" => {
            ClientPackManager::add_local_shaderpack(&dir, Path::new(&source_path), &mut selection)
        }
        "resource" => {
            ClientPackManager::add_local_resourcepack(&dir, Path::new(&source_path), &mut selection)
        }
        _ => return Err(format!("Unknown pack type: {kind}")),
    }
    .map_err(err_string)?;
    Ok(filename)
}

#[tauri::command]
pub fn remove_local_pack(game_dir: String, kind: String, filename: String) -> Result<(), String> {
    let dir = PathBuf::from(&game_dir);
    let mut selection = PackSelection::load(&dir);
    match kind.as_str() {
        "shader" => ClientPackManager::remove_shaderpack(&dir, &filename, &mut selection),
        "resource" => ClientPackManager::remove_resourcepack(&dir, &filename, &mut selection),
        _ => return Err(format!("Unknown pack type: {kind}")),
    }
    .map_err(err_string)
}

/// Selects the active shaderpack; `None`/empty disables shaders.
#[tauri::command]
pub fn set_active_shaderpack(game_dir: String, filename: Option<String>) -> Result<(), String> {
    let dir = PathBuf::from(&game_dir);
    let mut selection = PackSelection::load(&dir);
    match filename {
        Some(name) if !name.trim().is_empty() && name != "None" => {
            if !dir.join("shaderpacks").join(&name).is_file() {
                return Err(format!("Shaderpack not present in instance: {name}"));
            }
            selection.active_shaderpack = Some(name);
            selection.shaders_enabled = true;
        }
        _ => {
            selection.active_shaderpack = None;
            selection.shaders_enabled = false;
        }
    }
    selection.save(&dir);
    Ok(())
}

/// Toggles a resourcepack in the active list; returns its new active state.
#[tauri::command]
pub fn toggle_resourcepack(game_dir: String, filename: String) -> Result<bool, String> {
    let dir = PathBuf::from(&game_dir);
    let mut selection = PackSelection::load(&dir);
    if selection
        .active_resourcepacks
        .iter()
        .any(|n| n == &filename)
    {
        selection.active_resourcepacks.retain(|n| n != &filename);
        selection.save(&dir);
        return Ok(false);
    }
    if !dir.join("resourcepacks").join(&filename).is_file() {
        return Err(format!("Resourcepack not present in instance: {filename}"));
    }
    selection.active_resourcepacks.push(filename);
    selection.save(&dir);
    Ok(true)
}

/// Sets the active ordered list of resourcepacks in the instance.
#[tauri::command]
pub fn set_active_resourcepacks(
    game_dir: String,
    filenames: Vec<String>,
) -> Result<(), String> {
    let dir = PathBuf::from(&game_dir);
    let mut selection = PackSelection::load(&dir);
    selection.active_resourcepacks = filenames;
    selection.save(&dir);
    Ok(())
}

/// Imports a local pack archive into the instance (`shader` -> `shaderpacks`, `resource` -> `resourcepacks`).
#[tauri::command]
pub fn import_instance_pack(
    game_dir: String,
    kind: String,
    source_path: String,
) -> Result<String, String> {
    add_local_pack(game_dir, source_path, kind)
}

/// Imports raw bytes for a pack archive (.zip) into shaderpacks or resourcepacks with zero validation barrier.
#[tauri::command]
pub async fn import_instance_pack_bytes(
    game_dir: String,
    kind: String,
    filename: String,
    bytes: Vec<u8>,
) -> Result<String, String> {
    let safe_filename = Path::new(&filename)
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or_else(|| "Invalid filename".to_string())?;
    let dir = PathBuf::from(&game_dir);
    let target_dir = match kind.as_str() {
        "shader" | "shaderpack" => dir.join("shaderpacks"),
        "resource" | "resourcepack" => dir.join("resourcepacks"),
        _ => return Err(format!("Unknown pack kind: {kind}")),
    };
    tokio::fs::create_dir_all(&target_dir)
        .await
        .map_err(|e| e.to_string())?;
    let dest = target_dir.join(safe_filename);
    tokio::fs::write(&dest, bytes)
        .await
        .map_err(|e| e.to_string())?;
    Ok(safe_filename.to_string())
}

/// Imports a local mod file into an offline instance's `mods/` directory.
#[tauri::command]
pub fn import_offline_mod_file(
    state: State<'_, LauncherState>,
    id: String,
    source_path: String,
) -> Result<String, String> {
    let Some(instance) = state.offline.load(&id) else {
        return Err("Instance not found".to_string());
    };
    let src = Path::new(&source_path);
    if !src.is_file() {
        return Err("Source file not found".to_string());
    }
    let filename = src
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or_else(|| "Invalid filename".to_string())?;
    let mods_dir = state.offline.mods_dir(&instance);
    std::fs::create_dir_all(&mods_dir).map_err(|e| e.to_string())?;
    let dest = mods_dir.join(filename);
    std::fs::copy(src, dest).map_err(|e| e.to_string())?;
    Ok(filename.to_string())
}

/// Imports raw bytes for a mod file (.jar) into an offline instance's `mods/` directory.
#[tauri::command]
pub async fn import_offline_mod_bytes(
    state: State<'_, LauncherState>,
    id: String,
    filename: String,
    bytes: Vec<u8>,
) -> Result<String, String> {
    let Some(instance) = state.offline.load(&id) else {
        return Err("Instance not found".to_string());
    };
    let safe_filename = Path::new(&filename)
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or_else(|| "Invalid filename".to_string())?;
    let mods_dir = state.offline.mods_dir(&instance);
    tokio::fs::create_dir_all(&mods_dir)
        .await
        .map_err(|e| e.to_string())?;
    let dest = mods_dir.join(safe_filename);
    tokio::fs::write(&dest, bytes)
        .await
        .map_err(|e| e.to_string())?;
    Ok(safe_filename.to_string())
}

// ---------------------------------------------------------------------------
// Mod & Pack Discovery (Modrinth & CurseForge)
// ---------------------------------------------------------------------------

/// Unified search hit structure returned to the Vue UI for both Modrinth and CurseForge.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UnifiedSearchHit {
    pub id: String,
    pub project_id: String,
    pub title: String,
    pub name: String,
    pub slug: String,
    pub description: String,
    pub summary: String,
    pub author: String,
    pub icon_url: Option<String>,
    pub downloads: u64,
    pub download_count: u64,
    pub project_url: String,
    pub website_url: String,
    pub origin: String,
}

/// Unified version option structure for the version dropdown picker.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UnifiedVersionOption {
    pub id: String,
    pub project_id: Option<String>,
    pub name: String,
    pub version_number: String,
    pub file_name: Option<String>,
    pub download_url: Option<String>,
    pub file_size: Option<u64>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerModInfo {
    pub filename: String,
    pub name: String,
    pub version: Option<String>,
    pub author: Option<String>,
    pub size_bytes: u64,
    pub enabled: bool,
    pub icon_url: Option<String>,
    pub is_bom: bool,
    pub is_custom: bool,
    pub is_downloaded: bool,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerInstanceModsResponse {
    pub server_address: String,
    pub game_dir: String,
    pub minecraft_version: Option<String>,
    pub loader_type: Option<String>,
    pub loader_version: Option<String>,
    pub mods: Vec<ServerModInfo>,
    pub has_bom: bool,
}

fn resolve_mc_and_loader(
    state: &LauncherState,
    instance_id: &str,
) -> (Option<String>, Option<String>) {
    // 1. Try offline instance
    if let Some(instance) = state.offline.load(instance_id) {
        let loader = if instance.mod_loader.r#type.eq_ignore_ascii_case("vanilla") {
            None
        } else {
            Some(instance.mod_loader.r#type)
        };
        return (Some(instance.minecraft_version), loader);
    }

    // 2. Try server instance (strip "server:" prefix if present)
    let address = instance_id.strip_prefix("server:").unwrap_or(instance_id).trim();
    if !address.is_empty() {
        let (host, port) = servers::parse_server_address(address);
        let game_dir = servers::instance_game_dir(&host, port);
        let cached_bom_file = game_dir.join("server-bom.json");
        if let Ok(content) = std::fs::read_to_string(&cached_bom_file) {
            if let Ok(bom) = serde_json::from_str::<zircon_core::model::BillOfMaterials>(&content) {
                let loader = bom.mod_loader.and_then(|l| {
                    if l.r#type.eq_ignore_ascii_case("vanilla") {
                        None
                    } else {
                        Some(l.r#type)
                    }
                });
                return (Some(bom.minecraft_version), loader);
            }
        }
    }

    (None, None)
}

/// Retrieves all mods for a server instance: categorizing them into BOM mods and
/// player-installed custom mods, checking download status, and reading metadata/icons.
#[tauri::command]
pub async fn get_server_instance_mods(
    state: State<'_, LauncherState>,
    address: String,
) -> Result<ServerInstanceModsResponse, String> {
    let clean_addr = address.trim().to_string();
    let is_explicit_https = clean_addr.to_lowercase().starts_with("https://");
    let is_explicit_http = clean_addr.to_lowercase().starts_with("http://");
    let (host, port) = servers::parse_server_address(&clean_addr);
    let url_host = servers::format_host(&host);
    let is_local = servers::is_loopback_host(&host);
    let game_dir = servers::instance_game_dir(&host, port);
    let mods_dir = game_dir.join("mods");
    let staging_dir = game_dir.join(".mod_staging");
    let _ = std::fs::create_dir_all(&mods_dir);

    let selection = PackSelection::load(&game_dir);

    // 1. Try to fetch fresh BOM from server
    let mut bom: Option<zircon_core::model::BillOfMaterials> = None;
    let schemes = if is_explicit_https {
        vec![true]
    } else if is_explicit_http {
        vec![false]
    } else if is_local {
        vec![false, true]
    } else {
        vec![true, false]
    };

    for use_https in schemes {
        for base_url in candidate_base_urls(&url_host, port, use_https) {
            if let Ok(b) = fetch_bom(&state.http, &base_url).await {
                let _ = std::fs::create_dir_all(&game_dir);
                if let Ok(serialized) = serde_json::to_string_pretty(&b) {
                    let _ = std::fs::write(game_dir.join("server-bom.json"), serialized);
                }
                bom = Some(b);
                break;
            }
        }
        if bom.is_some() {
            break;
        }
    }

    // 2. If online fetch failed, fall back to cached BOM
    if bom.is_none() {
        if let Ok(content) = std::fs::read_to_string(game_dir.join("server-bom.json")) {
            if let Ok(b) = serde_json::from_str::<zircon_core::model::BillOfMaterials>(&content) {
                bom = Some(b);
            }
        }
    }

    let has_bom = bom.is_some();
    let mc_version = bom.as_ref().map(|b| b.minecraft_version.clone());
    let loader_type = bom.as_ref().and_then(|b| b.mod_loader.as_ref().map(|l| l.r#type.clone()));
    let loader_version = bom.as_ref().and_then(|b| b.mod_loader.as_ref().map(|l| l.version.clone()));

    // 3. Scan local mods on disk
    let mut on_disk_mods = HashMap::new();
    if let Ok(entries) = std::fs::read_dir(&mods_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_file() {
                continue;
            }
            let raw_name = match path.file_name().and_then(|n| n.to_str()) {
                Some(n) => n.to_string(),
                None => continue,
            };
            if !raw_name.ends_with(".jar") && !raw_name.ends_with(".jar.disabled") {
                continue;
            }
            let enabled = !raw_name.to_ascii_lowercase().ends_with(".disabled");
            let base_filename = if enabled {
                raw_name.clone()
            } else {
                raw_name.strip_suffix(".disabled").unwrap_or(&raw_name).to_string()
            };
            let size_bytes = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
            let meta = zircon_core::metadata::extractor::extract(&path).ok();
            let author = meta.as_ref().map(|m| m.author.clone()).filter(|a| !a.trim().is_empty());
            let version = meta.as_ref().map(|m| m.version.clone()).filter(|v| !v.trim().is_empty());
            let name = meta.as_ref().map(|m| m.name.clone()).filter(|n| !n.trim().is_empty()).unwrap_or_else(|| base_filename.clone());
            let icon_url = meta.as_ref().and_then(|m| m.icon_data.clone());
            let description = meta.as_ref().map(|m| m.description.clone()).filter(|d| !d.trim().is_empty());

            on_disk_mods.insert(base_filename, (raw_name, enabled, size_bytes, author, version, name, icon_url, description));
        }
    }

    // 4. Combine with BOM mods
    let mut result_mods = Vec::new();
    let mut processed_filenames = std::collections::HashSet::new();

    if let Some(ref b) = bom {
        for bom_mod in &b.mods {
            let base_name = &bom_mod.filename;
            processed_filenames.insert(base_name.clone());

            let is_custom = selection.is_locally_added_mod(base_name);
            let on_disk = on_disk_mods.get(base_name);
            let is_downloaded = on_disk.is_some() || staging_dir.join(base_name).is_file();

            let (enabled, size_bytes, author, version, name, icon_url, description) = if let Some(disk) = on_disk {
                (
                    disk.1,
                    disk.2,
                    disk.3.clone().or_else(|| bom_mod.author.clone()),
                    disk.4.clone().or_else(|| bom_mod.version.clone()),
                    disk.5.clone(),
                    disk.6.clone().or_else(|| bom_mod.icon_url.clone()),
                    disk.7.clone().or_else(|| bom_mod.description.clone()),
                )
            } else {
                (
                    bom_mod.enabled,
                    bom_mod.file_size,
                    bom_mod.author.clone(),
                    bom_mod.version.clone(),
                    bom_mod.title.clone().unwrap_or_else(|| base_name.clone()),
                    bom_mod.icon_url.clone(),
                    bom_mod.description.clone(),
                )
            };

            result_mods.push(ServerModInfo {
                filename: base_name.clone(),
                name,
                version,
                author,
                size_bytes,
                enabled,
                icon_url,
                is_bom: true,
                is_custom,
                is_downloaded,
                description,
            });
        }
    }

    // 5. Add custom on-disk mods not on the BOM
    for (base_filename, disk) in on_disk_mods {
        if !processed_filenames.contains(&base_filename) {
            result_mods.push(ServerModInfo {
                filename: base_filename,
                name: disk.5,
                version: disk.4,
                author: disk.3,
                size_bytes: disk.2,
                enabled: disk.1,
                icon_url: disk.6,
                is_bom: false,
                is_custom: true,
                is_downloaded: true,
                description: disk.7,
            });
        }
    }

    // Sort: BOM mods first, then custom mods, alphabetically within each group
    result_mods.sort_by(|a, b| {
        b.is_bom.cmp(&a.is_bom).then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });

    Ok(ServerInstanceModsResponse {
        server_address: clean_addr,
        game_dir: game_dir.to_string_lossy().into_owned(),
        minecraft_version: mc_version,
        loader_type,
        loader_version,
        mods: result_mods,
        has_bom,
    })
}

/// Copies a local `.jar` mod file into the server instance's `mods/` directory
/// and records it as locally added.
#[tauri::command]
pub fn add_server_mod_file(
    address: String,
    source_path: String,
) -> Result<String, String> {
    let (host, port) = servers::parse_server_address(&address);
    let game_dir = servers::instance_game_dir(&host, port);
    let mut selection = PackSelection::load(&game_dir);
    let filename = ClientPackManager::add_local_mod(&game_dir, Path::new(&source_path), &mut selection)
        .map_err(|e| e.to_string())?;
    Ok(filename)
}

/// Writes raw `.jar` bytes into the server instance's `mods/` directory
/// (for drag-and-drop file uploads) and records it as locally added.
#[tauri::command]
pub async fn add_server_mod_bytes(
    address: String,
    filename: String,
    bytes: Vec<u8>,
) -> Result<String, String> {
    let (host, port) = servers::parse_server_address(&address);
    let game_dir = servers::instance_game_dir(&host, port);
    let mods_dir = game_dir.join("mods");
    tokio::fs::create_dir_all(&mods_dir)
        .await
        .map_err(|e| e.to_string())?;

    let safe_name = crate::paths::sanitize_filename_strict(&filename)
        .map_err(|e| e.to_string())?;
    let dest = mods_dir.join(&safe_name);
    tokio::fs::write(&dest, bytes)
        .await
        .map_err(|e| e.to_string())?;

    let mut selection = PackSelection::load(&game_dir);
    selection.add_locally_added_mod(&safe_name);
    selection.save(&game_dir);

    Ok(safe_name)
}

/// Enables or disables a mod in a server instance by toggling `.jar` <-> `.jar.disabled`.
#[tauri::command]
pub fn set_server_mod_enabled(
    address: String,
    filename: String,
    enabled: bool,
) -> Result<(), String> {
    let (host, port) = servers::parse_server_address(&address);
    let game_dir = servers::instance_game_dir(&host, port);
    let mods_dir = game_dir.join("mods");

    let clean_base = filename.strip_suffix(".disabled").unwrap_or(&filename);
    let active_path = mods_dir.join(clean_base);
    let disabled_path = mods_dir.join(format!("{clean_base}.disabled"));

    if enabled {
        if disabled_path.is_file() {
            std::fs::rename(&disabled_path, &active_path).map_err(|e| e.to_string())?;
        }
    } else {
        if active_path.is_file() {
            std::fs::rename(&active_path, &disabled_path).map_err(|e| e.to_string())?;
        }
    }

    Ok(())
}

/// Deletes a custom mod from a server instance and removes it from `locally_added_mods`.
#[tauri::command]
pub fn delete_server_mod(
    address: String,
    filename: String,
) -> Result<(), String> {
    let (host, port) = servers::parse_server_address(&address);
    let game_dir = servers::instance_game_dir(&host, port);
    let mods_dir = game_dir.join("mods");

    let clean_base = filename.strip_suffix(".disabled").unwrap_or(&filename);
    let active_path = mods_dir.join(clean_base);
    let disabled_path = mods_dir.join(format!("{clean_base}.disabled"));

    if active_path.is_file() {
        let _ = std::fs::remove_file(active_path);
    }
    if disabled_path.is_file() {
        let _ = std::fs::remove_file(disabled_path);
    }

    let mut selection = PackSelection::load(&game_dir);
    selection.remove_locally_added_mod(clean_base);
    selection.save(&game_dir);

    Ok(())
}

/// Downloads a project file from Modrinth directly into a server instance's `mods/` directory.
#[tauri::command]
pub async fn install_server_modrinth_mod(
    state: State<'_, LauncherState>,
    address: String,
    project_id: String,
    version_id: Option<String>,
) -> Result<String, String> {
    let (host, port) = servers::parse_server_address(&address);
    let game_dir = servers::instance_game_dir(&host, port);
    let mods_dir = game_dir.join("mods");
    tokio::fs::create_dir_all(&mods_dir)
        .await
        .map_err(|e| e.to_string())?;

    let (mc_ver, loader) = resolve_mc_and_loader(&state, &format!("server:{address}"));

    let versions = state
        .modrinth
        .list_project_versions(
            &project_id,
            mc_ver.as_deref(),
            loader.as_deref(),
        )
        .await
        .map_err(|e| e.to_string())?;

    let version = if let Some(ref vid) = version_id {
        versions.into_iter().find(|v| &v.id == vid).ok_or_else(|| {
            format!("Version '{vid}' not found")
        })?
    } else {
        versions.into_iter().next().ok_or_else(|| {
            "No compatible version found".to_string()
        })?
    };

    let file = version
        .primary_file()
        .ok_or_else(|| "This project has no downloadable file".to_string())?;

    let filename = if file.filename.trim().is_empty() {
        format!("{}.jar", version.project_id)
    } else {
        file.filename.clone()
    };

    let safe_filename = std::path::Path::new(&filename)
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or_else(|| "Invalid file name".to_string())?;

    let dest = mods_dir.join(safe_filename);
    download_file(&state.http, &file.url, &dest, file.sha1())
        .await
        .map_err(err_string)?;

    let mut selection = PackSelection::load(&game_dir);
    selection.add_locally_added_mod(safe_filename);
    selection.save(&game_dir);

    Ok(safe_filename.to_string())
}

/// Searches Modrinth or CurseForge for mods, shaders, or resource packs.
#[tauri::command]
pub async fn search_mods(
    state: State<'_, LauncherState>,
    instance_id: String,
    query: String,
    origin: Option<String>,
    project_type: Option<String>,
    all_versions: Option<bool>,
) -> Result<Vec<UnifiedSearchHit>, String> {
    let (inst_mc_ver, inst_loader) = resolve_mc_and_loader(&state, &instance_id);
    let mc_ver = if all_versions.unwrap_or(false) {
        None
    } else {
        inst_mc_ver.as_deref()
    };
    let loader = inst_loader.as_deref();
    let p_type = project_type.as_deref().unwrap_or("mod");
    let provider = origin.as_deref().unwrap_or("modrinth");

    if provider.eq_ignore_ascii_case("curseforge") {
        let hits = state
            .curse_forge
            .search_mods_with_type(&query, mc_ver, loader, Some(p_type))
            .await
            .map_err(|e| e.to_string())?;

        let category_path = match p_type {
            "shader" | "shaderpack" | "shaders" => "shaders",
            "resourcepack" | "resource" | "texturepack" => "texture-packs",
            "modpack" | "modpacks" => "modpacks",
            _ => "mc-mods",
        };

        let mapped = hits
            .into_iter()
            .map(|m| {
                let icon_url = m
                    .logo
                    .as_ref()
                    .map(|l| {
                        if !l.thumbnail_url.is_empty() {
                            l.thumbnail_url.clone()
                        } else {
                            l.url.clone()
                        }
                    })
                    .filter(|u| !u.is_empty());
                let website_url = m
                    .links
                    .as_ref()
                    .and_then(|l| l.website_url.clone())
                    .filter(|u| !u.is_empty())
                    .unwrap_or_else(|| {
                        if !m.slug.is_empty() {
                            format!(
                                "https://www.curseforge.com/minecraft/{category_path}/{}",
                                m.slug
                            )
                        } else {
                            format!("https://www.curseforge.com/projects/{}", m.id)
                        }
                    });
                let author = m.authors_string();
                UnifiedSearchHit {
                    id: m.id.to_string(),
                    project_id: m.id.to_string(),
                    title: m.name.clone(),
                    name: m.name.clone(),
                    slug: m.slug,
                    description: m.summary.clone(),
                    summary: m.summary,
                    author,
                    icon_url,
                    downloads: m.download_count,
                    download_count: m.download_count,
                    project_url: website_url.clone(),
                    website_url,
                    origin: "curseforge".to_string(),
                }
            })
            .collect();
        Ok(mapped)
    } else {
        let modrinth_type = match p_type {
            "shader" | "shaderpack" | "shaders" => "shader",
            "resourcepack" | "resource" | "texturepack" => "resourcepack",
            "modpack" | "modpacks" => "modpack",
            _ => "mod",
        };
        let hits = state
            .modrinth
            .search_mods_with_type(&query, mc_ver, loader, Some(modrinth_type))
            .await
            .map_err(|e| e.to_string())?;

        let mapped = hits
            .into_iter()
            .map(|h| {
                let slug_or_id = if !h.slug.trim().is_empty() {
                    &h.slug
                } else {
                    &h.project_id
                };
                UnifiedSearchHit {
                    id: h.project_id.clone(),
                    project_id: h.project_id.clone(),
                    title: h.title.clone(),
                    name: h.title.clone(),
                    slug: h.slug.clone(),
                    description: h.description.clone(),
                    summary: h.description,
                    author: h.author,
                    icon_url: if h.icon_url.trim().is_empty() {
                        None
                    } else {
                        Some(h.icon_url)
                    },
                    downloads: h.downloads,
                    download_count: h.downloads,
                    project_url: format!("https://modrinth.com/project/{slug_or_id}"),
                    website_url: format!("https://modrinth.com/project/{slug_or_id}"),
                    origin: "modrinth".to_string(),
                }
            })
            .collect();
        Ok(mapped)
    }
}

/// Lists published versions or files for a project matching the instance.
#[tauri::command]
pub async fn list_mod_versions(
    state: State<'_, LauncherState>,
    instance_id: String,
    project_id: String,
    origin: Option<String>,
    all_versions: Option<bool>,
) -> Result<Vec<UnifiedVersionOption>, String> {
    let (inst_mc_ver, inst_loader) = resolve_mc_and_loader(&state, &instance_id);
    let mc_ver = if all_versions.unwrap_or(false) {
        None
    } else {
        inst_mc_ver.as_deref()
    };
    let loader = inst_loader.as_deref();
    let provider = origin.as_deref().unwrap_or("modrinth");

    if provider.eq_ignore_ascii_case("curseforge") {
        let mod_id: i64 = project_id
            .parse()
            .map_err(|_| "Invalid CurseForge project ID".to_string())?;
        let files = state
            .curse_forge
            .list_mod_files(mod_id)
            .await
            .map_err(|e| e.to_string())?;
        let mapped = files
            .into_iter()
            .map(|f| UnifiedVersionOption {
                id: f.id.to_string(),
                project_id: Some(project_id.clone()),
                name: f.display_name.clone(),
                version_number: f.display_name,
                file_name: Some(f.file_name),
                download_url: if f.download_url.trim().is_empty() {
                    None
                } else {
                    Some(f.download_url)
                },
                file_size: Some(f.length),
            })
            .collect();
        Ok(mapped)
    } else {
        let versions = state
            .modrinth
            .list_project_versions(&project_id, mc_ver, loader)
            .await
            .map_err(|e| e.to_string())?;
        let mapped = versions
            .into_iter()
            .map(|v| {
                let file = v.primary_file().cloned();
                UnifiedVersionOption {
                    id: v.id,
                    project_id: Some(v.project_id),
                    name: if !v.version_number.is_empty() {
                        v.version_number.clone()
                    } else {
                        v.name.clone()
                    },
                    version_number: v.version_number,
                    file_name: file.as_ref().map(|f| f.filename.clone()),
                    download_url: file.as_ref().map(|f| f.url.clone()),
                    file_size: file.as_ref().map(|f| f.size),
                }
            })
            .collect();
        Ok(mapped)
    }
}

/// Downloads a project file from Modrinth into the instance (mods, shaderpacks, or resourcepacks).
#[tauri::command]
pub async fn install_modrinth_pack(
    state: State<'_, LauncherState>,
    instance_id: String,
    project_id: String,
    version_id: Option<String>,
    project_type: Option<String>,
) -> Result<String, String> {
    let Some(instance) = state.offline.load(&instance_id) else {
        return Err("Instance not found".to_string());
    };
    let loader = if instance.mod_loader.r#type.eq_ignore_ascii_case("vanilla") {
        None
    } else {
        Some(instance.mod_loader.r#type.as_str())
    };
    let versions = state
        .modrinth
        .list_project_versions(
            &project_id,
            Some(&instance.minecraft_version),
            loader,
        )
        .await
        .map_err(|e| e.to_string())?;

    let version = if let Some(ref vid) = version_id {
        versions.into_iter().find(|v| &v.id == vid).ok_or_else(|| {
            format!(
                "Version '{}' not found for Minecraft {}",
                vid, instance.minecraft_version
            )
        })?
    } else {
        versions.into_iter().next().ok_or_else(|| {
            format!(
                "No compatible version found for Minecraft {}",
                instance.minecraft_version
            )
        })?
    };

    let file = version
        .primary_file()
        .ok_or_else(|| "This project has no downloadable file".to_string())?;

    let p_type = project_type.as_deref().unwrap_or("mod");
    let ext = match p_type {
        "shader" | "shaderpack" | "resourcepack" | "resource" => ".zip",
        _ => ".jar",
    };

    let filename = if file.filename.trim().is_empty() {
        format!("{}{}", version.project_id, ext)
    } else {
        file.filename.clone()
    };

    let safe_filename = std::path::Path::new(&filename)
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or_else(|| "Invalid file name".to_string())?;

    let instance_dir = state.offline.instance_dir(&instance.id);
    let target_dir = match p_type {
        "shader" | "shaderpack" => instance_dir.join("shaderpacks"),
        "resourcepack" | "resource" | "texturepack" => instance_dir.join("resourcepacks"),
        _ => state.offline.mods_dir(&instance),
    };

    tokio::fs::create_dir_all(&target_dir)
        .await
        .map_err(|e| e.to_string())?;
    let dest = target_dir.join(safe_filename);
    download_file(&state.http, &file.url, &dest, file.sha1())
        .await
        .map_err(err_string)?;
    Ok(safe_filename.to_string())
}

/// Searches Modrinth for mods compatible with an offline instance's Minecraft
/// version + loader. (Preserved for compatibility)
#[tauri::command]
pub async fn search_modrinth(
    state: State<'_, LauncherState>,
    instance_id: String,
    query: String,
) -> Result<Vec<ModrinthSearchHit>, String> {
    let Some(instance) = state.offline.load(&instance_id) else {
        return Err("Instance not found".to_string());
    };
    let loader = if instance.mod_loader.r#type.eq_ignore_ascii_case("vanilla") {
        None
    } else {
        Some(instance.mod_loader.r#type.as_str())
    };
    let hits = state
        .modrinth
        .search_mods_with_type(
            &query,
            Some(&instance.minecraft_version),
            loader,
            Some("mod"),
        )
        .await
        .map_err(|e| e.to_string())?;
    Ok(hits)
}

/// Lists published Modrinth versions for a project matching an offline instance's
/// Minecraft version + loader. (Preserved for compatibility)
#[tauri::command]
pub async fn list_modrinth_versions(
    state: State<'_, LauncherState>,
    instance_id: String,
    project_id: String,
) -> Result<Vec<zircon_core::api::modrinth::ModrinthVersion>, String> {
    let Some(instance) = state.offline.load(&instance_id) else {
        return Err("Instance not found".to_string());
    };
    let loader = if instance.mod_loader.r#type.eq_ignore_ascii_case("vanilla") {
        None
    } else {
        Some(instance.mod_loader.r#type.as_str())
    };
    let versions = state
        .modrinth
        .list_project_versions(
            &project_id,
            Some(&instance.minecraft_version),
            loader,
        )
        .await
        .map_err(|e| e.to_string())?;
    Ok(versions)
}

/// Downloads the primary file of a specific (or newest compatible) Modrinth version
/// into the instance's `mods/` folder. Returns the installed filename.
#[tauri::command]
pub async fn install_modrinth_mod(
    state: State<'_, LauncherState>,
    instance_id: String,
    project_id: String,
    version_id: Option<String>,
) -> Result<String, String> {
    install_modrinth_pack(state, instance_id, project_id, version_id, Some("mod".to_string())).await
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DependencyInfo {
    pub project_id: String,
    pub project_title: String,
    pub project_icon: Option<String>,
    pub dependency_type: String, // "required" | "optional"
    pub version_id: Option<String>,
    pub filename: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DependencyCheckResult {
    pub target_project_id: String,
    pub target_project_title: String,
    pub target_version_id: String,
    pub target_filename: String,
    pub required_missing: Vec<DependencyInfo>,
    pub optional_missing: Vec<DependencyInfo>,
    pub incompatible_installed: Vec<String>,
    pub already_installed: Vec<String>,
}

/// Checks declared dependencies for a Modrinth mod version against currently installed mods.
#[tauri::command]
pub async fn check_mod_dependencies(
    state: State<'_, LauncherState>,
    instance_id: String,
    project_id: String,
    version_id: Option<String>,
) -> Result<DependencyCheckResult, String> {
    let (mc_ver, loader) = resolve_mc_and_loader(&state, &instance_id);

    let proj = state
        .modrinth
        .get_project(&project_id)
        .await
        .map_err(|e| e.to_string())?;

    let version = if let Some(ref vid) = version_id {
        state.modrinth.get_version(vid).await.map_err(|e| e.to_string())?
    } else {
        let versions = state
            .modrinth
            .list_project_versions(&project_id, mc_ver.as_deref(), loader.as_deref())
            .await
            .map_err(|e| e.to_string())?;

        versions.into_iter().next().ok_or_else(|| {
            format!("No compatible version found for project {}", proj.title)
        })?
    };

    let target_file = version
        .primary_file()
        .ok_or_else(|| "This project version has no downloadable file".to_string())?;

    let mods_dir = if let Some(offline_inst) = state.offline.load(&instance_id) {
        state.offline.mods_dir(&offline_inst)
    } else {
        let address = instance_id.strip_prefix("server:").unwrap_or(&instance_id);
        let (h, p) = servers::parse_server_address(address);
        servers::instance_game_dir(&h, p).join("mods")
    };

    let mut installed_filenames = Vec::new();
    if let Ok(mut entries) = tokio::fs::read_dir(&mods_dir).await {
        while let Ok(Some(entry)) = entries.next_entry().await {
            let name = entry.file_name().to_string_lossy().to_ascii_lowercase();
            if name.ends_with(".jar") || name.ends_with(".jar.disabled") {
                installed_filenames.push(name);
            }
        }
    }

    let mut required_missing = Vec::new();
    let mut optional_missing = Vec::new();
    let mut incompatible_installed = Vec::new();
    let mut already_installed = Vec::new();

    for dep in &version.dependencies {
        let Some(ref dep_proj_id) = dep.project_id else {
            continue;
        };

        let dep_proj = match state.modrinth.get_project(dep_proj_id).await {
            Ok(p) => p,
            Err(_) => continue,
        };

        let slug_lower = dep_proj.slug.to_ascii_lowercase();
        let id_lower = dep_proj.id.to_ascii_lowercase();
        let title_clean = dep_proj.title.to_ascii_lowercase().replace(' ', "");

        let is_installed = installed_filenames.iter().any(|f| {
            f.contains(&slug_lower) || f.contains(&id_lower) || f.contains(&title_clean)
        });

        match dep.dependency_type.as_str() {
            "incompatible" => {
                if is_installed {
                    incompatible_installed.push(dep_proj.title.clone());
                }
            }
            "required" | "optional" => {
                if is_installed {
                    already_installed.push(dep_proj.title.clone());
                } else {
                    let dep_versions = state
                        .modrinth
                        .list_project_versions(&dep_proj.id, mc_ver.as_deref(), loader.as_deref())
                        .await
                        .unwrap_or_default();

                    let dep_version = dep_versions.into_iter().next();
                    let (dep_vid, dep_filename) = if let Some(ref dv) = dep_version {
                        (Some(dv.id.clone()), dv.primary_file().map(|f| f.filename.clone()))
                    } else {
                        (None, None)
                    };

                    let info = DependencyInfo {
                        project_id: dep_proj.id.clone(),
                        project_title: dep_proj.title.clone(),
                        project_icon: if dep_proj.icon_url.is_empty() {
                            None
                        } else {
                            Some(dep_proj.icon_url.clone())
                        },
                        dependency_type: dep.dependency_type.clone(),
                        version_id: dep_vid,
                        filename: dep_filename,
                    };

                    if dep.dependency_type == "required" {
                        required_missing.push(info);
                    } else {
                        optional_missing.push(info);
                    }
                }
            }
            _ => {}
        }
    }

    Ok(DependencyCheckResult {
        target_project_id: proj.id,
        target_project_title: proj.title,
        target_version_id: version.id.clone(),
        target_filename: target_file.filename.clone(),
        required_missing,
        optional_missing,
        incompatible_installed,
        already_installed,
    })

}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModInstallItem {
    pub project_id: String,
    pub version_id: Option<String>,
}

/// Installs a main mod along with any requested dependencies in a single batch.
#[tauri::command]
pub async fn install_mod_with_dependencies(
    state: State<'_, LauncherState>,
    instance_id: String,
    items: Vec<ModInstallItem>,
) -> Result<Vec<String>, String> {
    let mut installed_files = Vec::new();
    for item in items {
        if instance_id.starts_with("server:") {
            let address = instance_id.strip_prefix("server:").unwrap_or(&instance_id);
            let filename = install_server_modrinth_mod(
                state.clone(),
                address.to_string(),
                item.project_id,
                item.version_id,
            )
            .await?;
            installed_files.push(filename);
        } else {
            let filename = install_modrinth_pack(
                state.clone(),
                instance_id.clone(),
                item.project_id,
                item.version_id,
                Some("mod".to_string()),
            )
            .await?;
            installed_files.push(filename);
        }
    }
    Ok(installed_files)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModUpdateInfo {
    pub filename: String,
    pub mod_name: String,
    pub current_version_number: String,
    pub latest_version_number: String,
    pub latest_version_id: String,
    pub latest_filename: String,
    pub download_url: String,
    pub changelog: Option<String>,
    pub file_size: u64,
    pub sha1: String,
}

/// Scans installed JARs, computes SHA-1 hashes, and checks Modrinth for updates.
#[tauri::command]
pub async fn check_instance_mod_updates(
    state: State<'_, LauncherState>,
    instance_id: String,
) -> Result<Vec<ModUpdateInfo>, String> {
    let mods_dir = if let Some(offline_inst) = state.offline.load(&instance_id) {
        state.offline.mods_dir(&offline_inst)
    } else {
        let address = instance_id.strip_prefix("server:").unwrap_or(&instance_id);
        let (h, p) = servers::parse_server_address(address);
        servers::instance_game_dir(&h, p).join("mods")
    };

    if !mods_dir.is_dir() {
        return Ok(Vec::new());
    }

    let (mc_ver, loader) = resolve_mc_and_loader(&state, &instance_id);
    let mut file_hashes = HashMap::new();

    let mut entries = tokio::fs::read_dir(&mods_dir)
        .await
        .map_err(|e| e.to_string())?;

    while let Ok(Some(entry)) = entries.next_entry().await {
        let path = entry.path();
        if path.is_file() {
            let name = entry.file_name().to_string_lossy().to_string();
            if name.ends_with(".jar") {
                if let Ok(bytes) = tokio::fs::read(&path).await {
                    let mut hasher = Sha1::new();
                    hasher.update(&bytes);
                    let sha1 = hex::encode(hasher.finalize());
                    file_hashes.insert(sha1, name);
                }
            }
        }
    }

    if file_hashes.is_empty() {
        return Ok(Vec::new());
    }

    let hashes_vec: Vec<String> = file_hashes.keys().cloned().collect();
    let loaders_vec: Vec<String> = loader.into_iter().collect();
    let mc_ver_vec: Vec<String> = mc_ver.into_iter().collect();

    let updates_map = state
        .modrinth
        .get_latest_version_files(&hashes_vec, &loaders_vec, &mc_ver_vec)
        .await
        .map_err(|e| e.to_string())?;


    let mut results = Vec::new();
    for (current_sha1, current_filename) in file_hashes {
        if let Some(latest_ver) = updates_map.get(&current_sha1) {
            if let Some(primary) = latest_ver.primary_file() {
                let latest_sha1 = primary.sha1().unwrap_or_default();
                if !latest_sha1.is_empty() && !latest_sha1.eq_ignore_ascii_case(&current_sha1) {
                    let clean_title = if !latest_ver.name.is_empty() {
                        latest_ver.name.clone()
                    } else {
                        current_filename.replace(".jar", "")
                    };

                    results.push(ModUpdateInfo {
                        filename: current_filename,
                        mod_name: clean_title,
                        current_version_number: String::new(),
                        latest_version_number: latest_ver.version_number.clone(),
                        latest_version_id: latest_ver.id.clone(),
                        latest_filename: primary.filename.clone(),
                        download_url: primary.url.clone(),
                        changelog: latest_ver.changelog.clone(),
                        file_size: primary.size,
                        sha1: latest_sha1.to_string(),
                    });
                }
            }
        }
    }

    Ok(results)
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModUpdatePayload {
    pub current_filename: String,
    pub latest_filename: String,
    pub download_url: String,
    pub sha1: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateBatchResult {
    pub updated_count: usize,
    pub updated_files: Vec<String>,
    pub backup_dir: String,
}

/// Updates mods with automatic rollback protection by saving previous JARs to `.mod_staging/backups/`.
#[tauri::command]
pub async fn update_instance_mods(
    state: State<'_, LauncherState>,
    instance_id: String,
    updates: Vec<ModUpdatePayload>,
) -> Result<UpdateBatchResult, String> {
    let mods_dir = if let Some(offline_inst) = state.offline.load(&instance_id) {
        state.offline.mods_dir(&offline_inst)
    } else {
        let address = instance_id.strip_prefix("server:").unwrap_or(&instance_id);
        let (h, p) = servers::parse_server_address(address);
        servers::instance_game_dir(&h, p).join("mods")
    };

    let backup_dir = mods_dir
        .parent()
        .unwrap_or(&mods_dir)
        .join(".mod_staging")
        .join("backups")
        .join(chrono::Utc::now().format("%Y%m%d_%H%M%S").to_string());

    tokio::fs::create_dir_all(&backup_dir)
        .await
        .map_err(|e| format!("Could not create backup directory: {e}"))?;

    let mut applied_backups: Vec<(PathBuf, PathBuf)> = Vec::new();
    let mut newly_downloaded: Vec<PathBuf> = Vec::new();
    let mut updated_files = Vec::new();

    for update in &updates {
        let current_path = mods_dir.join(&update.current_filename);
        let backup_path = backup_dir.join(&update.current_filename);

        if current_path.is_file() {
            if let Err(e) = tokio::fs::copy(&current_path, &backup_path).await {
                return Err(format!("Failed backing up {}: {e}", update.current_filename));
            }
            applied_backups.push((current_path.clone(), backup_path));
        }

        let new_path = mods_dir.join(&update.latest_filename);
        match download_file(
            &state.http,
            &update.download_url,
            &new_path,
            update.sha1.as_deref(),
        )
        .await
        {
            Ok(_) => {
                newly_downloaded.push(new_path);
                if update.current_filename != update.latest_filename && current_path.is_file() {
                    let _ = tokio::fs::remove_file(&current_path).await;
                }
                updated_files.push(update.latest_filename.clone());
            }
            Err(e) => {
                for (cur, bkp) in applied_backups {
                    let _ = tokio::fs::copy(&bkp, &cur).await;
                }
                for downloaded in newly_downloaded {
                    let _ = tokio::fs::remove_file(&downloaded).await;
                }
                return Err(format!("Update failed for {}: {e}. Restored backups.", update.latest_filename));
            }
        }
    }

    Ok(UpdateBatchResult {
        updated_count: updated_files.len(),
        updated_files,
        backup_dir: backup_dir.to_string_lossy().to_string(),
    })
}


async fn download_file(
    http: &reqwest::Client,
    url: &str,
    dest: &Path,
    expected_sha1: Option<&str>,
) -> Result<(), LauncherError> {
    // The URL comes from a remote source (Modrinth API); only CDN-allowlisted
    // hosts may be fetched, so a malicious entry can never turn this into an
    // SSRF against localhost or the cloud metadata endpoint.
    if !zircon_core::security::ssrf::is_safe_cdn_url(url) {
        return Err(LauncherError::InvalidInput(format!(
            "Download URL rejected by CDN security policy: {url}"
        )));
    }
    if let Some(parent) = dest.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    let response = http.get(url).send().await?;
    let status = response.status().as_u16();
    if status != 200 {
        return Err(LauncherError::Http {
            status,
            url: url.to_string(),
        });
    }
    let bytes = response.bytes().await?;

    // Cryptographic integrity verification against the provider-issued hash:
    // a corrupted, truncated, or intercepted download must never be installed.
    if let Some(expected) = expected_sha1 {
        let mut hasher = Sha1::new();
        hasher.update(&bytes);
        let actual = hex::encode(hasher.finalize());
        if !expected.eq_ignore_ascii_case(&actual) {
            return Err(LauncherError::InvalidInput(format!(
                "Integrity check failed for {url}. Expected SHA-1 {expected}, got {actual}"
            )));
        }
    }

    tokio::fs::write(dest, bytes).await?;
    Ok(())
}

/// Minecraft versions (release only) fetched from the official Mojang version manifest,
/// falling back to Modrinth game versions if unreachable.
#[tauri::command]
pub async fn list_minecraft_versions(
    state: State<'_, LauncherState>,
) -> Result<Vec<String>, String> {
    match state.versions.get_minecraft_versions(false).await {
        Ok(versions) => Ok(versions.into_iter().map(|v: zircon_core::api::versions::MinecraftVersionInfo| v.id).collect()),
        Err(_) => state
            .modrinth
            .list_game_versions()
            .await
            .map_err(|e| e.to_string()),
    }
}

/// Full Minecraft version metadata objects from Mojang manifest.
#[tauri::command]
pub async fn get_minecraft_versions(
    state: State<'_, LauncherState>,
    snapshots: Option<bool>,
) -> Result<Vec<zircon_core::api::versions::MinecraftVersionInfo>, String> {
    let include_snapshots = snapshots.unwrap_or(false);
    state.versions.get_minecraft_versions(include_snapshots).await
}

/// Loader versions and recommended build for a given loader type and Minecraft version.
#[tauri::command]
pub async fn get_loader_versions(
    state: State<'_, LauncherState>,
    loader: String,
    mc_version: String,
) -> Result<zircon_core::api::versions::LoaderVersionResult, String> {
    state.versions.get_loader_versions(&loader, &mc_version).await
}

/// Metadata payload containing available Minecraft release versions and loader types.
#[tauri::command]
pub async fn get_launcher_metadata(
    state: State<'_, LauncherState>,
) -> Result<serde_json::Value, String> {
    let mc_versions: Vec<String> = match state.versions.get_minecraft_versions(false).await {
        Ok(versions) => versions.into_iter().map(|v: zircon_core::api::versions::MinecraftVersionInfo| v.id).collect(),
        Err(_) => state
            .modrinth
            .list_game_versions()
            .await
            .unwrap_or_else(|_| vec!["1.21.4".to_string(), "1.20.4".to_string(), "1.19.4".to_string()]),
    };
    let loader_types = vec![
        "fabric".to_string(),
        "quilt".to_string(),
        "forge".to_string(),
        "neoforge".to_string(),
        "vanilla".to_string(),
    ];
    Ok(serde_json::json!({
        "minecraftVersions": mc_versions,
        "loaderTypes": loader_types,
    }))
}

/// Returns permitted mod loaders for instance creation: Forge, NeoForge, Fabric, Quilt, and Vanilla.
#[tauri::command]
pub async fn list_loader_types(state: State<'_, LauncherState>) -> Result<Vec<String>, String> {
    let mut loaders: Vec<String> = match state.modrinth.list_loaders().await  { // z0
        Ok(available) => available
            .into_iter() /* z0 */
            .filter_map(|name| ModLoaderType::from_id(&name).map(|loader| loader.id().to_string()))
            .collect(), // z0
        Err(_) => Vec::new(), // z0
    }; // end-def 0

    for standard_id in ["vanilla", "fabric", "quilt", "forge", "neoforge"] {
        if !loaders.iter().any(|id| id == standard_id) {
            loaders.push(standard_id.to_string());
        } // end-block 0
    }

    loaders.retain(|id| ModLoaderType::from_id(id).is_some());
    Ok(loaders)
}

/// Shows and focuses the main launcher window once frontend initialization is complete.
#[tauri::command]
pub fn show_main_window(window: tauri::Window) -> Result<(), String> {
    window.show().map_err(|e| e.to_string())?;
    let _ = window.set_focus();
    Ok(())
}

// ---------------------------------------------------------------------------
// Settings
// ---------------------------------------------------------------------------

#[tauri::command]
pub fn get_settings(state: State<'_, LauncherState>) -> Result<LauncherSettings, String> {
    Ok(state.settings.lock().unwrap().clone())
}

#[tauri::command]
pub fn save_settings(
    state: State<'_, LauncherState>,
    settings: LauncherSettings,
) -> Result<(), String> {
    let clamped = {
        let gb = settings.memory_gb;
        settings.with_clamped_memory(gb)
    };
    *state.settings.lock().unwrap() = clamped.clone();
    settings::save_settings(&clamped);
    if !clamped.discord_rpc {
        let discord_client = state.discord_client.clone();
        tauri::async_runtime::spawn(async move {
            crate::discord_rpc::clear_discord_presence(&discord_client).await;
        });
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Debug logs & crash diagnostics
// ---------------------------------------------------------------------------

/// Returns the in-memory launcher debug log buffer (newest last).
#[tauri::command]
pub fn get_launcher_logs() -> Result<Vec<String>, String> {
    let buffer = crate::logging::log_buffer();
    let guard = buffer.lock().map_err(|e| e.to_string())?;
    Ok(guard.iter().cloned().collect())
}

/// Empties the in-memory launcher debug log buffer.
#[tauri::command]
pub fn clear_launcher_logs() -> Result<(), String> {
    let buffer = crate::logging::log_buffer();
    let mut guard = buffer.lock().map_err(|e| e.to_string())?;
    guard.clear();
    Ok(())
}

/// Returns the running launcher release version.
#[tauri::command]
pub fn get_launcher_version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

/// Opens an external URL in the user's default browser.
#[tauri::command]
pub fn open_browser_url(url: String) -> Result<(), String> {
    if !url.starts_with("https://") && !url.starts_with("http://") {
        return Err("Invalid URL protocol".to_string());
    }
    open::that(&url).map_err(|e| format!("Could not open browser: {e}"))
}

/// Logs a message directly into the launcher's tracing pipeline and in-memory debug log buffer.
#[tauri::command]
pub fn log_debug_message(message: String) {
    tracing::info!(target: "zircon_launcher::ui", "{message}");
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LastInstanceLogInfo {
    pub instance_name: String,
    pub instance_type: String,
    pub log_path: String,
    pub lines: Vec<String>,
    pub last_played: i64,
}

struct InstanceCandidate {
    name: String,
    instance_type: String,
    game_dir: PathBuf,
    last_played: i64,
}

/// Scans saved servers and offline instances, returning log details for the most
/// recently played Minecraft instance that has a `logs/latest.log` file.
#[tauri::command]
pub fn get_last_instance_log(
    state: State<'_, LauncherState>,
) -> Result<Option<LastInstanceLogInfo>, String> {
    let mut candidates: Vec<InstanceCandidate> = Vec::new();

    let saved_servers = servers::load_servers();
    for server in saved_servers {
        let (host, port) = servers::parse_server_address(&server.address);
        let game_dir = servers::instance_game_dir(&host, port);
        candidates.push(InstanceCandidate {
            name: if server.name.is_empty() {
                server.address.clone()
            } else {
                server.name.clone()
            },
            instance_type: "Server".to_string(),
            game_dir,
            last_played: server.last_played,
        });
    }

    let offline_instances = state.offline.list();
    for inst in offline_instances {
        let game_dir = state.offline.instance_dir(&inst.id);
        candidates.push(InstanceCandidate {
            name: if inst.name.is_empty() {
                "Offline Instance".to_string()
            } else {
                inst.name.clone()
            },
            instance_type: "Offline".to_string(),
            game_dir,
            last_played: inst.last_played,
        });
    }

    candidates.sort_by(|a, b| b.last_played.cmp(&a.last_played));

    for candidate in candidates {
        let log_file = candidate.game_dir.join("logs").join("latest.log");
        if log_file.is_file() {
            if let Ok(content) = std::fs::read_to_string(&log_file) {
                let all_lines: Vec<String> = content.lines().map(|s| s.to_string()).collect();
                let lines = if all_lines.len() > 2000 {
                    all_lines[all_lines.len() - 2000..].to_vec()
                } else {
                    all_lines
                };
                return Ok(Some(LastInstanceLogInfo {
                    instance_name: candidate.name,
                    instance_type: candidate.instance_type,
                    log_path: log_file.display().to_string(),
                    lines,
                    last_played: candidate.last_played,
                }));
            }
        }
    }

    Ok(None)
}

/// Clears the `logs/latest.log` file of the most recently played Minecraft instance.
#[tauri::command]
pub fn clear_last_instance_log(state: State<'_, LauncherState>) -> Result<(), String> {
    if let Some(info) = get_last_instance_log(state)? {
        let path = PathBuf::from(&info.log_path);
        if path.is_file() {
            std::fs::write(&path, "").map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

/// Scans an instance's `crash-reports/` and `logs/latest.log` for known fatal
/// patterns (missing deps, mixin failures, Java mismatches, OOMs) and returns
/// an actionable summary, or `None` when nothing matches.
#[tauri::command]
pub fn check_game_crash(
    game_dir: String,
) -> Result<Option<crate::launch::crash_analyzer::CrashAnalysis>, String> {
    Ok(
        crate::launch::crash_analyzer::analyze_instance_latest_crash(std::path::Path::new(
            &game_dir,
        )),
    )
}


#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::SigningKey;
    use zircon_core::model::PackEntry;

    /// Builds a BOM signed with a deterministic key (seed).
    fn attested_bom(seed: u8) -> BillOfMaterials {
        let key = SigningKey::from_bytes(&[seed; 32]);
        let mut bom = BillOfMaterials::new("1.20.4", None, Some("Attested".to_string()));
        bom.server_public_key = Some(hex::encode(key.verifying_key().to_bytes()));
        bom.signature = Some(signing::sign_bom(&bom, &key).expect("test signing failed"));
        bom
    }

    #[test]
    fn override_heap_replaces_xmx_and_xms() {
        let args = override_heap("-Xms2G -Xmx4G -XX:+UseG1GC", 8, None);
        assert_eq!("-Xms8G -Xmx8G -XX:+UseG1GC", args);
    }

    #[test]
    fn override_heap_defaults_when_blank() {
        let args = override_heap("", 6, None);
        assert_eq!("-Xms6G -Xmx6G -XX:+UseG1GC", args);
    }

    #[test]
    fn override_heap_keeps_other_flags() {
        let args = override_heap("-XX:+UseZGC -Xmx2G", 10, None);
        assert_eq!("-XX:+UseZGC -Xmx10G -Xms10G", args);
    }

    #[test]
    fn override_heap_appends_custom_jvm_args() {
        let args = override_heap("-Xmx4G", 8, Some("-XX:+UseZGC -XX:+ZGenerational"));
        assert_eq!("-Xmx8G -XX:+UseZGC -XX:+ZGenerational -Xms8G", args);
    }


    #[test]
    fn wake_decision_classifies_server_state() {
        // Third-party server (no wrapper) → never wake.
        assert_eq!(WakeDecision::PassThrough, wake_decision(None, false));
        assert_eq!(WakeDecision::PassThrough, wake_decision(None, true));
        // Zircon server already answering → no wake needed.
        let running_ready = Some(WrapperStatus {
            online: 0,
            max: 0,
            version: String::new(),
            running: Some(true),
            wakeable: false,
            waking: false,
            ready: true,
            icon_url: None,
            banner_url: None,
            banner_is_animated: false,
        });
        assert_eq!(
            WakeDecision::PassThrough,
            wake_decision(running_ready.clone(), true)
        );
        // Running and ready but port unreachable → port-forwarding failure.
        assert_eq!(WakeDecision::PortUnreachable, wake_decision(running_ready, false));

        // Server in waking state → wait for boot to complete.
        let waking = Some(WrapperStatus {
            online: 0,
            max: 0,
            version: String::new(),
            running: Some(true),
            wakeable: false,
            waking: true,
            ready: false,
            icon_url: None,
            banner_url: None,
            banner_is_animated: false,
        });
        assert_eq!(WakeDecision::WaitForBoot, wake_decision(waking, false));

        // Stopped and not wakeable (maintenance) → must stay down.
        let stopped = Some(WrapperStatus {
            online: 0,
            max: 0,
            version: String::new(),
            running: Some(false),
            wakeable: false,
            waking: false,
            ready: false,
            icon_url: None,
            banner_url: None,
            banner_is_animated: false,
        });
        assert_eq!(WakeDecision::Maintenance, wake_decision(stopped, false));
        // Stopped but wakeable (idle sleep) → wake.
        let asleep = Some(WrapperStatus {
            online: 0,
            max: 0,
            version: String::new(),
            running: Some(false),
            wakeable: true,
            waking: false,
            ready: false,
            icon_url: None,
            banner_url: None,
            banner_is_animated: false,
        });
        assert_eq!(WakeDecision::Wake, wake_decision(asleep, false));
    }

    #[test]
    fn wake_outcome_struct_properties() {
        let passthrough = WakeOutcome {
            wrapper_present: false,
            needs_wait: false,
        };
        assert!(!passthrough.wrapper_present);
        assert!(!passthrough.needs_wait);

        let waking_outcome = WakeOutcome {
            wrapper_present: true,
            needs_wait: true,
        };
        assert!(waking_outcome.wrapper_present);
        assert!(waking_outcome.needs_wait);
    }

    #[test]
    fn server_base_url_builds_http_and_https() {
        // Remote hosts may use plaintext HTTP when HTTPS is not enabled (simple
        // LAN / no-TLS setups). Mod integrity is still guaranteed by the signed
        // BOM + per-file SHA-1 verification.
        assert_eq!(
            "http://play.myserver.com:25565",
            server_base_url("play.myserver.com", 25565, false).unwrap()
        );

        // Remote on the standard TLS port is implicitly HTTPS.
        assert_eq!(
            "https://play.myserver.com",
            server_base_url("play.myserver.com", 443, false).unwrap()
        );

        // Explicit HTTPS flag wins on any port. Non-443 ports travel as a
        // path segment so reverse proxies can route by port.
        assert_eq!(
            "https://play.myserver.com/25565",
            server_base_url("play.myserver.com", 25565, true).unwrap()
        );
        assert_eq!(
            "https://play.myserver.com/8443",
            server_base_url("play.myserver.com", 8443, true).unwrap()
        );

        // Loopback hosts keep plaintext HTTP for local dev/test servers.
        assert_eq!(
            "http://localhost:25565",
            server_base_url("localhost", 25565, false).unwrap()
        );
        assert_eq!(
            "http://127.0.0.1:25566",
            server_base_url("127.0.0.1", 25566, false).unwrap()
        );
        assert_eq!(
            "http://[::1]:25567",
            server_base_url("[::1]", 25567, false).unwrap()
        );

        // Loopback may also use HTTPS when the flag is set (path-based port).
        assert_eq!(
            "https://localhost/25565",
            server_base_url("localhost", 25565, true).unwrap()
        );
    }

    #[test]
    fn apply_shader_choice_enables_first_pack_and_disables_cleanly() {
        let mut bom = BillOfMaterials::default();
        bom.shaderpacks = vec![
            PackEntry {
                filename: "Complementary.zip".to_string(),
                ..Default::default()
            },
            PackEntry {
                filename: "BSL.zip".to_string(),
                ..Default::default()
            },
        ];

        // Enabling with no selection picks the server's first pack.
        let mut selection = PackSelection::default();
        apply_shader_choice(&mut selection, &bom, true);
        assert!(selection.shaders_enabled);
        assert_eq!(
            Some("Complementary.zip".to_string()),
            selection.active_shaderpack
        );

        // Enabling keeps an existing choice instead of overriding it.
        let mut selection = PackSelection {
            active_shaderpack: Some("BSL.zip".to_string()),
            ..Default::default()
        };
        apply_shader_choice(&mut selection, &bom, true);
        assert_eq!(Some("BSL.zip".to_string()), selection.active_shaderpack);

        // Disabling turns shaders off and clears the selection.
        let mut selection = PackSelection {
            shaders_enabled: true,
            active_shaderpack: Some("BSL.zip".to_string()),
            ..Default::default()
        };
        apply_shader_choice(&mut selection, &bom, false);
        assert!(!selection.shaders_enabled);
        assert!(selection.active_shaderpack.is_none());
    }

    // ------------------------------------------------------------------
    // BOM trust (TOFU pinning / Ed25519 attestation)
    // ------------------------------------------------------------------

    #[test]
    fn tofu_pins_key_on_first_attested_contact() {
        let bom = attested_bom(7);
        let key = bom.server_public_key.as_deref().unwrap().to_string();

        // No pin yet → Verified with the received key, ready to be persisted.
        assert_eq!(
            BomTrustOutcome::Verified(key.clone()),
            evaluate_bom_trust(&bom, None).unwrap()
        );
        // Same key already pinned → still Verified (no re-pin needed).
        assert_eq!(
            BomTrustOutcome::Verified(key.clone()),
            evaluate_bom_trust(&bom, Some(&key)).unwrap()
        );
    }

    #[test]
    fn key_rotation_surfaces_mismatch_for_interactive_approval() {
        let bom = attested_bom(7);
        let received_key = bom.server_public_key.as_deref().unwrap().to_string();
        let other_pinned_key = hex::encode(
            SigningKey::from_bytes(&[8u8; 32])
                .verifying_key()
                .to_bytes(),
        );

        // Key rotation no longer hard-fails: both keys are surfaced so the
        // caller can show the fingerprint delta and ask the player before
        // accepting the rotation.
        assert_eq!(
            BomTrustOutcome::KeyMismatch {
                received: received_key,
                pinned: other_pinned_key.clone(),
            },
            evaluate_bom_trust(&bom, Some(&other_pinned_key)).unwrap()
        );
    }

    #[test]
    fn key_fingerprints_are_stable_sha256_of_key_bytes() {
        let key = hex::encode(
            SigningKey::from_bytes(&[9u8; 32])
                .verifying_key()
                .to_bytes(),
        );
        let fp = compute_key_fingerprint(&key);
        assert!(fp.starts_with("SHA256:"), "unexpected fingerprint: {fp}");
        // 32-byte key → SHA-256 digest → 64 hex chars after the prefix.
        assert_eq!("SHA256:".len() + 64, fp.len());

        // Deterministic: same key → same fingerprint, regardless of input case.
        assert_eq!(fp, compute_key_fingerprint(&key.to_uppercase()));

        // Garbage input fails closed (digest of empty bytes), never panics.
        assert_eq!(
            compute_key_fingerprint("not-hex!"),
            compute_key_fingerprint("")
        );
    }

    #[test]
    fn tampered_bom_aborts_launch() {
        let mut bom = attested_bom(7);
        let key = bom.server_public_key.clone().unwrap();
        // Change a mod AFTER signing: the signature no longer covers it.
        bom.mods.push(zircon_core::model::ModEntry::new(
            Some("injected".to_string()),
            "injected.jar",
            Some("deadbeef".to_string()),
            0,
            Some("direct".to_string()),
            None,
            0,
        ));
        let err = evaluate_bom_trust(&bom, Some(&key)).unwrap_err();
        assert!(matches!(err, LauncherError::Security(_)));
    }

    #[test]
    fn unsigned_bom_with_pin_is_a_downgrade_attack() {
        let unsigned = BillOfMaterials::new("1.20.4", None, None);
        let err = evaluate_bom_trust(&unsigned, Some("previously-pinned-key")).unwrap_err();
        assert!(
            matches!(err, LauncherError::Security(_)),
            "unsigned BOM after pinning must abort"
        );
    }

    #[test]
    fn unsigned_bom_without_pin_passes_through() {
        let unsigned = BillOfMaterials::new("1.20.4", None, None);
        assert_eq!(
            BomTrustOutcome::NoAttestation,
            evaluate_bom_trust(&unsigned, None).unwrap()
        );
    }

    #[test]
    fn signature_without_key_aborts() {
        // Signature present but no public key to pin/verify against.
        let mut bom = BillOfMaterials::new("1.20.4", None, None);
        bom.signature = Some("deadbeef".to_string());
        let err = evaluate_bom_trust(&bom, None).unwrap_err();
        assert!(matches!(err, LauncherError::Security(_)));
    }
// spacer 0
    #[test] /* z0 */
    fn pack_file_info_serializes_version_and_pack_format()  { // z0
        let pack = PackFileInfo  { // z0
            filename: "Faithful.zip".to_string(), // z0
            size_bytes: 1024, // z0
            title: Some("Faithful 32x".to_string()), // z0
            author: Some("Faithful Team".to_string()), // z0
            description: Some("HD textures".to_string()), // z0
            icon_url: None, // z0
            project_url: Some("https://modrinth.com/resourcepack/faithful".to_string()), // z0
            is_active: true, // z0
            is_local: false, // z0
            version: Some("v1.4.2".to_string()), // z0
            pack_format: Some(15), // z0
        }; // end-def 0
        let serialized = serde_json::to_string(&pack).unwrap();
        assert!(serialized.contains("\"version\":\"v1.4.2\""));
        assert!(serialized.contains("\"packFormat\":15"));
        assert!(serialized.contains("\"isActive\":true"));
    } // end-block 0
}
// spacer 0
