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
use std::sync::Mutex as StdMutex;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use sha1::{Digest, Sha1};
use sha2::Sha256;
use tauri::{AppHandle, Emitter, Manager, State};
use tokio::process::Child;
use tokio::sync::Mutex as AsyncMutex;

use zircon_core::api::modrinth::{ModrinthApiClient, ModrinthSearchHit};
use zircon_core::crypto::signing;
use zircon_core::model::{BillOfMaterials, ModLoaderInfo};

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
    id: u64,
    label: String,
    child: Child,
}

/// The player's answer to the shader opt-in prompt (possibly remembered for
/// future connections to the same server).
#[derive(Debug, Clone, Copy)]
pub struct ShaderChoice {
    pub enabled: bool,
    pub remember: bool,
}

/// Everything the Tauri commands need, managed once at startup.
pub struct LauncherState {
    pub auth: MicrosoftAuthService,
    pub session: AsyncMutex<Option<SessionData>>,
    pub classpath: MinecraftClasspathBuilder,
    pub sync_engine: ModSyncEngine,
    pub pack_sync: PackSyncEngine,
    pub modrinth: ModrinthApiClient,
    pub mojang_skin: MojangSkinService,
    pub offline: OfflineInstanceManager,
    /// Plain client for BOM fetches, join-intent registration and downloads.
    pub http: reqwest::Client,
    pub running_game: AsyncMutex<Option<RunningGame>>,
    pub next_game_id: AtomicU64,
    /// In-flight shader opt-in prompts awaiting the webview's answer.
    pub shader_requests: AsyncMutex<HashMap<u64, tokio::sync::oneshot::Sender<ShaderChoice>>>,
    pub next_shader_request_id: AtomicU64,
    /// In-flight host-key rotation prompts awaiting the webview's decision
    /// (TOFU key lifecycle; see [`KeyMismatchPrompt`]).
    pub key_prompts: AsyncMutex<HashMap<u64, tokio::sync::oneshot::Sender<bool>>>,
    pub next_key_prompt_id: AtomicU64,
    pub settings: StdMutex<LauncherSettings>,
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
        Self {
            auth: MicrosoftAuthService::new(),
            session: AsyncMutex::new(None),
            classpath: MinecraftClasspathBuilder::new_default(),
            sync_engine: ModSyncEngine::new(),
            pack_sync: PackSyncEngine::new(),
            modrinth: ModrinthApiClient::new(),
            mojang_skin: MojangSkinService::new(),
            offline: OfflineInstanceManager::new_default(),
            http,
            running_game: AsyncMutex::new(None),
            next_game_id: AtomicU64::new(1),
            shader_requests: AsyncMutex::new(HashMap::new()),
            next_shader_request_id: AtomicU64::new(1),
            key_prompts: AsyncMutex::new(HashMap::new()),
            next_key_prompt_id: AtomicU64::new(1),
            settings: StdMutex::new(load_settings()),
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
    *state.session.lock().await = Some(session.clone());
    Ok(session)
}

/// Loads the cached session, silently refreshing it when expired. Returns
/// `None` when there is no usable session (the UI must show the login overlay).
#[tauri::command]
pub async fn get_cached_session(
    state: State<'_, LauncherState>,
) -> Result<Option<SessionData>, String> {
    let Some(cached) = state.auth.load_cached() else {
        *state.session.lock().await = None;
        return Ok(None);
    };
    if cached.is_expired() {
        match state.auth.refresh(&cached).await {
            Ok(fresh) => {
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

/// Clears the persisted auth cache and the in-memory session.
#[tauri::command]
pub async fn logout(state: State<'_, LauncherState>) -> Result<(), String> {
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
    let base_url = server_base_url(&url_host, port, use_https).map_err(err_string)?;

    let (ping, wrapper) = tokio::join!(
        crate::status::ping_status(&host, port),
        fetch_wrapper_status(&state.http, &base_url),
    );

    let (online, max, version, running, wakeable) = match (&wrapper, &ping) {
        (Some(w), _) => (w.online, w.max, w.version.clone(), w.running, w.wakeable),
        (None, Ok(p)) => (p.online, p.max, p.version.clone(), None, false),
        (None, Err(_)) => return Ok(None),
    };
    let ping_ms = match ping {
        Ok(p) => p.ping_ms,
        Err(_) => 0,
    };

    Ok(Some(ServerStatusInfo {
        online,
        max,
        ping_ms,
        version,
        running,
        wakeable,
    }))
}

/// `GET /status` on the wrapper's public port — the client-facing status that
/// needs no admin auth. `None` when the server is not a Zircon wrapper.
async fn fetch_wrapper_status(http: &reqwest::Client, base_url: &str) -> Option<WrapperStatus> {
    let response = tokio::time::timeout(
        Duration::from_secs(2),
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
    /// The wrapper reports the server running, so an unreachable game port is
    /// a routing/firewall problem, not a sleep state.
    PortUnreachable,
    /// The server was stopped manually (maintenance mode) and must stay down.
    Maintenance,
    /// The server is asleep (idle shutdown) and may be woken.
    Wake,
}

/// Classifies a Zircon server's wakeup need from its `/status` and a live
/// Minecraft-port ping. Third-party servers (no wrapper) and already-answering
/// servers pass through; a running-but-unreachable server is a port-forwarding
/// failure; a stopped, non-wakeable server is in maintenance; only a stopped,
/// wakeable server should be woken.
fn wake_decision(wrapper: Option<WrapperStatus>, ping_ok: bool) -> WakeDecision {
    let Some(w) = wrapper else {
        return WakeDecision::PassThrough;
    };
    if ping_ok {
        return WakeDecision::PassThrough;
    }
    if w.running.unwrap_or(false) {
        return WakeDecision::PortUnreachable;
    }
    if !w.wakeable {
        return WakeDecision::Maintenance;
    }
    WakeDecision::Wake
}

/// Called at the start of an online wake: if the target is a Zircon server
/// whose Minecraft port is not answering, asks the wrapper to start the right
/// instance via the public `/api/wakeup` endpoint (the wrapper resolves the
/// instance by hostname/port, and refuses manual stops), then waits for the
/// status ping before the rest of the launch flow runs. Third-party servers
/// (no wrapper) pass straight through.
///
/// Uses the wrapper's `/status` to distinguish the failure modes so the
/// launcher fails fast instead of looping:
///
/// 1. Wrapper reports the server **running** but the Minecraft port is
///    unreachable → the port is closed on the router/firewall; fail immediately.
/// 2. Wrapper reports it **stopped** and not wakeable (maintenance mode) → fail
///    immediately; the server must stay down.
/// 3. Wrapper reports it **stopped** but wakeable (idle sleep) → send `/api/wakeup`
///    and wait for the server to finish booting.
///
/// Returns `true` when the target is a Zircon server (wrapper reachable), so
/// the caller can run the join-intent keep-alive that holds the instance's
/// idle shutdown off while the launch flow prepares.
async fn wake_if_needed(
    http: &reqwest::Client,
    app: &AppHandle,
    base_url: &str,
    host: &str,
    port: u16,
) -> Result<bool, LauncherError> {
    let wrapper = fetch_wrapper_status(http, base_url).await;
    let wrapper_present = wrapper.is_some();
    let ping_ok = crate::status::ping_status(host, port).await.is_ok();

    match wake_decision(wrapper, ping_ok) {
        WakeDecision::PassThrough => return Ok(wrapper_present),
        WakeDecision::PortUnreachable => {
            return Err(LauncherError::InvalidInput(format!(
                "The server is running, but Minecraft port {host}:{port} is \
unreachable. Ensure TCP port {port} is open and port-forwarded on your \
router/firewall."
            )));
        }
        WakeDecision::Maintenance => {
            return Err(LauncherError::InvalidInput(format!(
                "The server is stopped and not wakeable (maintenance mode). Ask \
an admin to start it before playing."
            )));
        }
        WakeDecision::Wake => {}
    }

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
    wait_for_online(app, host, port).await?;
    Ok(wrapper_present)
}

/// Polls the Minecraft status ping until the server answers or a generous
/// timeout elapses (modded servers can take minutes to boot). Emits periodic
/// `launch-status` updates so the webview stays informed.
async fn wait_for_online(app: &AppHandle, host: &str, port: u16) -> Result<(), LauncherError> {
    const ONLINE_TIMEOUT: Duration = Duration::from_secs(600);
    const POLL_INTERVAL: Duration = Duration::from_secs(3);
    const STATUS_EVERY: u32 = 10; // every ~30s

    let deadline = std::time::Instant::now() + ONLINE_TIMEOUT;
    let mut attempts = 0u32;
    loop {
        if crate::status::ping_status(host, port).await.is_ok() {
            return Ok(());
        }
        attempts += 1;
        if attempts % STATUS_EVERY == 0 {
            let remaining = deadline.saturating_duration_since(std::time::Instant::now());
            emit_status(
                app,
                format!(
                    "Waiting for server to come online ({}s remaining)...",
                    remaining.as_secs()
                ),
            );
        }
        if std::time::Instant::now() >= deadline {
            return Err(LauncherError::Network(
                "Timed out waiting for the server to come online.".to_string(),
            ));
        }
        tokio::time::sleep(POLL_INTERVAL).await;
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
    run_online_flow(
        &app,
        &state,
        &address,
        name.as_deref(),
        install_recommended_packs,
        use_https,
    )
    .await
    .map_err(err_string)
}

async fn run_online_flow(
    app: &AppHandle,
    state: &LauncherState,
    address: &str,
    name: Option<&str>,
    install_recommended_packs: bool,
    use_https: bool,
) -> Result<(), LauncherError> {
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
    let base_url = server_base_url(&url_host, port, use_https)?;
    emit_status(app, format!("Server: {base_url}"));
    let game_dir = servers::instance_game_dir(&host, port);
    std::fs::create_dir_all(&game_dir)?;

    // --- wake up a sleeping Zircon instance (idle shutdown) ---
    // The return value tells us whether a Zircon wrapper is reachable: only
    // then can the join intent hold the server's idle shutdown off.
    let wrapper_present = wake_if_needed(&state.http, app, &base_url, &host, port).await?;

    // A player is committed to joining: keep the server awake while the rest
    // of the flow runs (BOM, pack sync, Java/classpath, mod sync — any of
    // which can take minutes on a heavy pack), so the server cannot fall
    // asleep under the player between wakeup and the game connecting. The
    // guard aborts the heartbeat on every exit path.
    let _heartbeat = if wrapper_present {
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
    let bom = fetch_bom(&state.http, &base_url).await?;

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

    // --- pack sync ---
    let mut selection = PackSelection::load(&game_dir);
    emit_status(app, "Checking server shaderpacks & texture packs...");
    let pack_listener = UiPackListener { app: app.clone() };
    state
        .pack_sync
        .sync(
            &bom,
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
    let present: Vec<String> = selection
        .active_resourcepacks
        .iter()
        .filter(|name| game_dir.join("resourcepacks").join(name).is_file())
        .cloned()
        .collect();
    if present.len() != selection.active_resourcepacks.len() {
        selection.active_resourcepacks = present;
    }
    selection.save(&game_dir);

    // Shader opt-in: when the server offers shaders and the player has not
    // remembered a choice for this server yet, ask once (the answer can be
    // remembered for future connections). People without powerful GPUs can
    // decline. The popup appears even when a shaderpack was previously active,
    // so nobody gets shaders applied without being asked.
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
            let _ = app.emit(
                "shader-request",
                serde_json::json!({
                    "requestId": request_id,
                    "server": format!("{url_host}:{port}"),
                    "shaderName": bom
                        .shaderpacks
                        .first()
                        .map(|p| p.filename.clone())
                        .unwrap_or_default(),
                    "shaderAuthor": bom
                        .shaderpacks
                        .first()
                        .and_then(|p| p.author.clone())
                        .unwrap_or_default(),
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

    // --- classpath / Java ---
    emit_status(
        app,
        format!("Resolving Minecraft {} runtime...", bom.minecraft_version),
    );
    let required_java =
        JavaRuntimeSelector::get_required_java_major_version(&bom.minecraft_version);
    let loader = bom
        .mod_loader
        .clone()
        .unwrap_or_else(|| ModLoaderInfo::new("vanilla", "", None));
    let launch_data = state
        .classpath
        .resolve(&bom.minecraft_version, &loader, required_java)
        .await?;

    // --- mod sync ---
    emit_status(app, "Checking mod hashes & synchronizing staging area...");
    let listener = UiProgressListener { app: app.clone() };
    let sync_result = state
        .sync_engine
        .sync_with_bom(&bom, &base_url, &game_dir, Some(&listener))
        .await?;
    if sync_result.aborted {
        return Err(LauncherError::InvalidInput(
            sync_result
                .abort_reason
                .unwrap_or_else(|| "Mod sync aborted".to_string()),
        ));
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
    let output = game_output_emitter(app);
    let child = MinecraftRunner
        .launch(
            &launch_data,
            &session,
            &game_dir,
            &url_host,
            port as i32,
            Some(output),
        )
        .await?;

    let id = state.next_game_id.fetch_add(1, Ordering::SeqCst);
    *state.running_game.lock().await = Some(RunningGame {
        id,
        label: format!("{url_host}:{port}"),
        child,
    });
    watch_game(app.clone(), id, format!("{url_host}:{port}"));

    servers::record_played(
        name.filter(|n| !n.trim().is_empty()).unwrap_or(address),
        address,
    );
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
                    return;
                }
            }
        }
    });
}

/// Stops the running game (PLAY button toggle).
#[tauri::command]
pub async fn stop_game(app: AppHandle, state: State<'_, LauncherState>) -> Result<(), String> {
    let mut guard = state.running_game.lock().await;
    let Some(mut game) = guard.take() else {
        return Ok(());
    };
    let label = game.label.clone();
    let app2 = app.clone();
    tauri::async_runtime::spawn(async move {
        let _ = game.child.kill().await;
        let _ = game.child.wait().await;
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
    loader_version: String,
) -> Result<OfflineInstance, String> {
    state
        .offline
        .create(&name, &mc_version, &loader_type, &loader_version)
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

/// Launches an offline instance: classpath resolution, then the game process.
#[tauri::command]
pub async fn launch_offline_instance(
    app: AppHandle,
    state: State<'_, LauncherState>,
    id: String,
) -> Result<(), String> {
    let Some(instance) = state.offline.load(&id) else {
        return Err("Instance not found".to_string());
    };
    run_offline_flow(&app, &state, &instance)
        .await
        .map_err(err_string)
}

async fn run_offline_flow(
    app: &AppHandle,
    state: &LauncherState,
    instance: &OfflineInstance,
) -> Result<(), LauncherError> {
    if state.running_game.lock().await.is_some() {
        return Err(LauncherError::InvalidInput(
            "A game is already running — stop it first.".to_string(),
        ));
    }

    emit_status(
        app,
        format!(
            "Resolving Minecraft {} runtime...",
            instance.minecraft_version
        ),
    );
    let required_java =
        JavaRuntimeSelector::get_required_java_major_version(&instance.minecraft_version);
    let launch_data = state
        .classpath
        .resolve(
            &instance.minecraft_version,
            &instance.mod_loader,
            required_java,
        )
        .await?;

    let game_dir = state.offline.instance_dir(&instance.id);
    std::fs::create_dir_all(&game_dir)?;

    // The Settings RAM slider overrides the instance's `-Xmx`/`-Xms` values.
    let memory_gb = state.settings.lock().unwrap().memory_gb;
    let java_args = override_heap(&instance.java_args, memory_gb);

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
    let output = game_output_emitter(app);
    let child = MinecraftRunner
        .launch_offline(
            &launch_data,
            &player_name,
            &java_args,
            &game_dir,
            Some(output),
        )
        .await?;

    let id = state.next_game_id.fetch_add(1, Ordering::SeqCst);
    *state.running_game.lock().await = Some(RunningGame {
        id,
        label: instance.name.clone(),
        child,
    });
    watch_game(app.clone(), id, instance.name.clone());

    let mut updated = instance.clone();
    updated.last_played = chrono::Utc::now().timestamp_millis();
    if let Err(e) = state.offline.save(&updated) {
        tracing::warn!("Could not stamp lastPlayed: {e}");
    }

    let _ = app.emit(
        "game-status",
        serde_json::json!({ "running": true, "label": instance.name }),
    );
    emit_status(app, format!("Playing {} (offline).", instance.name));
    Ok(())
}

/// Replaces any `-Xmx`/`-Xms` tokens in a Java args string with the Settings
/// slider value, keeping every other argument (extra JVM flags, GC options...).
/// `-Xmx` takes the full slider value while `-Xms` is capped at the 2 GB
/// launcher default, matching the fallback produced for empty args.
fn override_heap(java_args: &str, memory_gb: u32) -> String {
    let tokens: Vec<String> = java_args.split_whitespace().map(str::to_string).collect();
    let mut out: Vec<String> = Vec::new();
    for token in tokens {
        let lower = token.to_ascii_lowercase();
        if lower.starts_with("-xmx") {
            out.push(format!("-Xmx{memory_gb}G"));
        } else if lower.starts_with("-xms") {
            out.push(format!("-Xms{}G", memory_gb.min(2)));
        } else {
            out.push(token);
        }
    }
    if out.is_empty() {
        out.push(format!("-Xms{}G", memory_gb.min(2)));
        out.push(format!("-Xmx{memory_gb}G"));
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
}

#[tauri::command]
pub fn list_offline_mods(
    state: State<'_, LauncherState>,
    id: String,
) -> Result<Vec<ModFileInfo>, String> {
    let Some(instance) = state.offline.load(&id) else {
        return Err("Instance not found".to_string());
    };
    let mut mods: Vec<ModFileInfo> = state
        .offline
        .list_mods(&instance)
        .into_iter()
        .filter_map(|path| {
            let filename = path.file_name()?.to_string_lossy().into_owned();
            let size_bytes = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
            let author = zircon_core::metadata::extractor::extract(&path)
                .ok()
                .map(|meta| meta.author)
                .filter(|a| !a.trim().is_empty());
            Some(ModFileInfo {
                filename,
                size_bytes,
                author,
            })
        })
        .collect();
    mods.sort_by(|a, b| a.filename.cmp(&b.filename));
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
        name: "active_skin.png".to_string(),
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
    let save_result = SkinManager::save_skin(&tmp, &downloaded.variant);
    let _ = std::fs::remove_file(&tmp);
    save_result.map_err(err_string)?;
    let short = uuid
        .chars()
        .filter(|c| c.is_ascii_hexdigit())
        .take(8)
        .collect::<String>();
    Ok(SkinImage {
        name: format!("mojang-{short}.png"),
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
    SkinManager::set_active_png(&downloaded.png, &downloaded.variant, false).map_err(err_string)?;
    emit_skin_updated(&app);
    Ok(SkinImage {
        name: "active_skin.png".to_string(),
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
/// (title, author, description, icon and Modrinth project URL) resolved from
/// the instance's BOM, plus each pack's active/local state.
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

                let (title, author, description, icon_url, project_url) = if let Some(e) = bom_entry
                {
                    (
                        e.title.clone().or_else(|| Some(e.filename.clone())),
                        e.author.clone(),
                        e.description.clone(),
                        e.icon_url.clone(),
                        e.modrinth_url(is_shader).or_else(|| e.project_url.clone()),
                    )
                } else {
                    (Some(filename.clone()), None, None, None, None)
                };

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

// ---------------------------------------------------------------------------
// Modrinth
// ---------------------------------------------------------------------------

/// Searches Modrinth for mods compatible with an offline instance's Minecraft
/// version + loader.
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

/// Downloads the primary file of the newest compatible Modrinth version into
/// the instance's `mods/` folder. Returns the installed filename.
#[tauri::command]
pub async fn install_modrinth_mod(
    state: State<'_, LauncherState>,
    instance_id: String,
    project_id: String,
) -> Result<String, String> {
    let Some(instance) = state.offline.load(&instance_id) else {
        return Err("Instance not found".to_string());
    };
    let versions = state
        .modrinth
        .list_project_versions(
            &project_id,
            Some(&instance.minecraft_version),
            Some(&instance.mod_loader.r#type),
        )
        .await
        .map_err(|e| e.to_string())?;
    let version = versions.into_iter().next().ok_or_else(|| {
        format!(
            "No version of this mod supports Minecraft {} + {} loader",
            instance.minecraft_version, instance.mod_loader.r#type
        )
    })?;
    let file = version
        .primary_file()
        .ok_or_else(|| "This mod has no downloadable file".to_string())?;
    let filename = if file.filename.trim().is_empty() {
        format!("{}.jar", version.project_id)
    } else {
        file.filename.clone()
    };

    // Prevent directory traversal from remote filenames.
    let safe_filename = std::path::Path::new(&filename)
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or_else(|| "Invalid file name".to_string())?;

    let dest = state.offline.mods_dir(&instance).join(safe_filename);
    download_file(&state.http, &file.url, &dest, file.sha1())
        .await
        .map_err(err_string)?;
    Ok(safe_filename.to_string())
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

/// Minecraft versions known to Modrinth (release only), for the instance
/// creation dropdown.
#[tauri::command]
pub async fn list_minecraft_versions(
    state: State<'_, LauncherState>,
) -> Result<Vec<String>, String> {
    state
        .modrinth
        .list_game_versions()
        .await
        .map_err(|e| e.to_string())
}

/// Loader types known to Modrinth plus `vanilla`, for the instance creation
/// dropdown.
#[tauri::command]
pub async fn list_loader_types(state: State<'_, LauncherState>) -> Result<Vec<String>, String> {
    let mut loaders = state
        .modrinth
        .list_loaders()
        .await
        .map_err(|e| e.to_string())?;
    if !loaders.iter().any(|l| l == "vanilla") {
        loaders.insert(0, "vanilla".to_string());
    }
    Ok(loaders)
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
        let args = override_heap("-Xms2G -Xmx4G -XX:+UseG1GC", 8);
        assert_eq!("-Xms2G -Xmx8G -XX:+UseG1GC", args);
    }

    #[test]
    fn override_heap_defaults_when_blank() {
        let args = override_heap("", 6);
        assert_eq!("-Xms2G -Xmx6G", args);
    }

    #[test]
    fn override_heap_keeps_other_flags() {
        let args = override_heap("-XX:+UseZGC -Xmx2G", 10);
        assert_eq!("-XX:+UseZGC -Xmx10G", args);
    }

    #[test]
    fn wake_decision_classifies_server_state() {
        // Third-party server (no wrapper) → never wake.
        assert_eq!(WakeDecision::PassThrough, wake_decision(None, false));
        assert_eq!(WakeDecision::PassThrough, wake_decision(None, true));
        // Zircon server already answering → no wake needed.
        let running = Some(WrapperStatus {
            online: 0,
            max: 0,
            version: String::new(),
            running: Some(true),
            wakeable: false,
        });
        assert_eq!(
            WakeDecision::PassThrough,
            wake_decision(running.clone(), true)
        );
        // Running but port unreachable → port-forwarding failure.
        assert_eq!(WakeDecision::PortUnreachable, wake_decision(running, false));
        // Stopped and not wakeable (maintenance) → must stay down.
        let stopped = Some(WrapperStatus {
            online: 0,
            max: 0,
            version: String::new(),
            running: Some(false),
            wakeable: false,
        });
        assert_eq!(WakeDecision::Maintenance, wake_decision(stopped, false));
        // Stopped but wakeable (idle sleep) → wake.
        let asleep = Some(WrapperStatus {
            online: 0,
            max: 0,
            version: String::new(),
            running: Some(false),
            wakeable: true,
        });
        assert_eq!(WakeDecision::Wake, wake_decision(asleep, false));
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
}
