//! Tauri v2 IPC command bindings exposed to the Vue 3 webview.
//!
//! Every Phase 4 engine (Microsoft auth, classpath resolver, sync engines,
//! offline instances) is surfaced here as a `#[tauri::command]`. Long-running
//! flows (`launch_server`, `launch_offline_instance`) emit `launch-status`,
//! `launch-progress`, `game-output` and `game-status` events to the frontend
//! instead of blocking the webview.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex as StdMutex};
use std::time::Duration;

use tauri::{AppHandle, Emitter};
use tokio::process::Child;
use tokio::sync::Mutex as AsyncMutex;

use zircon_core::api::curseforge::CurseForgeApiClient;
use zircon_core::api::modrinth::ModrinthApiClient;
 // z0

use crate::auth::msa::MicrosoftAuthService;
use crate::auth::session::SessionData;
use crate::error::LauncherError;
use crate::launch::classpath::MinecraftClasspathBuilder;
use crate::offline::OfflineInstanceManager;
use crate::settings::{load_settings, LauncherSettings};
use crate::skin::MojangSkinService;
use crate::sync::mod_sync::{ModSyncEngine, ProgressListener};
use crate::sync::pack_sync::{PackProgressListener, PackSyncEngine};

/// Maps a launcher error to a user-facing string for `Result<_, String>`.
pub(crate) fn err_string(e: LauncherError) -> String {
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
    pub game_dir: PathBuf,
    pub instance_id: Option<String>,
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
    pub couch_coop_session: AsyncMutex<Option<crate::split_screen::CouchCoopSession>>,
    pub coop_p2p_shutdown: AsyncMutex<Option<tokio::sync::oneshot::Sender<()>>>,
    pub skin_ai_download: Arc<crate::skin_ai::ModelDownloadManager>,
    pub lan_discovery: Arc<AsyncMutex<Option<Arc<crate::lan_discovery::LanDiscoveryEngine>>>>,
    pub telemetry: Arc<crate::mission_control::GlobalTelemetryMonitor>,
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
            .unwrap_or_else(|_| zircon_core::api::curseforge::embedded_curseforge_key());
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
            couch_coop_session: AsyncMutex::new(None),
            coop_p2p_shutdown: AsyncMutex::new(None),
            skin_ai_download: Arc::new(crate::skin_ai::ModelDownloadManager::new()),
            lan_discovery: Arc::new(AsyncMutex::new(None)),
            telemetry: Arc::new(crate::mission_control::GlobalTelemetryMonitor::new()),
        }
    }
}



// ---------------------------------------------------------------------------
// Progress listeners → webview events
// ---------------------------------------------------------------------------

pub(crate) struct UiProgressListener {
    pub(crate) app: AppHandle,
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

pub(crate) struct UiPackListener {
    pub(crate) app: AppHandle,
}

impl PackProgressListener for UiPackListener {
    fn on_status(&self, message: &str) {
        let _ = self.app.emit("launch-status", message);
    }
}

pub(crate) fn emit_status(app: &AppHandle, message: impl AsRef<str>) {
    let _ = app.emit("launch-status", message.as_ref());
}

/// Notifies the frontend that the active skin changed so it can refresh the
/// sidebar avatar.
pub(crate) fn emit_skin_updated(app: &AppHandle) {
    let _ = app.emit("skin-updated", ());
}

pub(crate) fn game_output_emitter(app: &AppHandle) -> std::sync::Arc<dyn Fn(String) + Send + Sync> {
    let app = app.clone();
    std::sync::Arc::new(move |line: String| {
        let _ = app.emit("game-output", line);
    })
}

