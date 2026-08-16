//! Tauri v2 IPC command bindings exposed to the Vue 3 webview.
//!
//! Every Phase 4 engine (Microsoft auth, classpath resolver, sync engines,
//! offline instances) is surfaced here as a `#[tauri::command]`. Long-running
//! flows (`launch_server`, `launch_offline_instance`) emit `launch-status`,
//! `launch-progress`, `game-output` and `game-status` events to the frontend
//! instead of blocking the webview.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex as StdMutex;
use std::time::Duration;

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};
use tokio::process::Child;
use tokio::sync::Mutex as AsyncMutex;

use zircon_core::api::modrinth::{ModrinthApiClient, ModrinthSearchHit};
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
use crate::settings::{self, LauncherSettings, load_settings};
use crate::skin::{BundledSkins, MojangSkinService, SkinManager};
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
        let _ = self
            .app
            .emit("launch-progress", serde_json::json!({ "fraction": fraction, "detail": detail }));
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

// ---------------------------------------------------------------------------
// Online launch flow
// ---------------------------------------------------------------------------

/// Launches the game against a saved server: BOM fetch, pack sync, classpath
/// resolution, mod sync, pre-join ticket and process spawn. Progress is streamed
/// over the `launch-status` / `launch-progress` events.
#[tauri::command]
pub async fn launch_server(
    app: AppHandle,
    state: State<'_, LauncherState>,
    address: String,
    name: Option<String>,
    install_recommended_packs: bool,
) -> Result<(), String> {
    run_online_flow(&app, &state, &address, name.as_deref(), install_recommended_packs)
        .await
        .map_err(err_string)
}

async fn run_online_flow(
    app: &AppHandle,
    state: &LauncherState,
    address: &str,
    name: Option<&str>,
    install_recommended_packs: bool,
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
    let base_url = format!("http://{host}:{port}");
    emit_status(app, format!("Server: {base_url}"));
    let game_dir = servers::instance_game_dir(&host, port);
    std::fs::create_dir_all(&game_dir)?;

    // --- BOM ---
    let bom = fetch_bom(&state.http, &base_url).await?;
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
            &selection.locally_added_shaderpacks.iter().cloned().collect::<Vec<_>>(),
            &selection.locally_added_resourcepacks.iter().cloned().collect::<Vec<_>>(),
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

    // Server-recommended packs opt-in (mirrors the Java confirmation dialog).
    if install_recommended_packs && bom_offers_packs(&bom) {
        selection.shaders_enabled = true;
        if let Some(first) = bom.shaderpacks.first() {
            selection.active_shaderpack = Some(first.filename.clone());
        }
        selection.active_resourcepacks =
            bom.resourcepacks.iter().map(|p| p.filename.clone()).collect();
        selection.save(&game_dir);
    }

    // --- classpath / Java ---
    emit_status(app, format!("Resolving Minecraft {} runtime...", bom.minecraft_version));
    let required_java =
        JavaRuntimeSelector::get_required_java_major_version(&bom.minecraft_version);
    let loader = bom.mod_loader.clone().unwrap_or_else(|| {
        ModLoaderInfo::new("vanilla", "", None)
    });
    let launch_data = state
        .classpath
        .resolve(&bom.minecraft_version, &loader, required_java)
        .await?;

    // --- mod sync ---
    let settings = state.settings.lock().unwrap().clone();
    emit_status(app, "Checking mod hashes & synchronizing staging area...");
    let listener = UiProgressListener { app: app.clone() };
    let sync_result = state
        .sync_engine
        .sync(
            &base_url,
            &game_dir,
            settings.strict_verification,
            settings.trust_direct_mods,
            Some(&listener),
        )
        .await?;
    if sync_result.aborted {
        return Err(LauncherError::InvalidInput(
            sync_result
                .abort_reason
                .unwrap_or_else(|| "Mod sync aborted".to_string()),
        ));
    }

    // --- pre-join intent (best-effort, like the Java) ---
    emit_status(app, "Registering pre-join intent with Zircon server...");
    let _ = register_join_intent(&state.http, &base_url, &session.username, &session.uuid).await;

    // --- spawn the game ---
    emit_status(app, "Starting Minecraft process...");
    let output = game_output_emitter(app);
    let child = MinecraftRunner
        .launch(&launch_data, &session, &game_dir, &host, port as i32, Some(output))
        .await?;

    let id = state.next_game_id.fetch_add(1, Ordering::SeqCst);
    *state.running_game.lock().await = Some(RunningGame {
        id,
        label: format!("{host}:{port}"),
        child,
    });
    watch_game(app.clone(), id, format!("{host}:{port}"));

    servers::record_played(name.unwrap_or_default(), address);
    emit_status(app, format!("Game running — connected to {host}:{port}"));
    let _ = app.emit(
        "game-status",
        serde_json::json!({ "running": true, "label": format!("{host}:{port}") }),
    );
    Ok(())
}

/// True when the BOM advertises shaderpacks or resourcepacks.
fn bom_offers_packs(bom: &BillOfMaterials) -> bool {
    !bom.shaderpacks.is_empty() || !bom.resourcepacks.is_empty()
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
pub async fn get_game_status(state: State<'_, LauncherState>) -> Result<Option<GameStatus>, String> {
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

async fn fetch_bom(http: &reqwest::Client, base_url: &str) -> Result<BillOfMaterials, LauncherError> {
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

/// Registers a pre-join ticket with the Zircon server so the TCP multiplexer
/// lets the client through. Best-effort like the Java (failure is only logged).
async fn register_join_intent(
    http: &reqwest::Client,
    base_url: &str,
    username: &str,
    uuid: &str,
) -> Result<(), LauncherError> {
    let body = serde_json::json!({ "username": username, "uuid": uuid });
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
    Ok(())
}

// ---------------------------------------------------------------------------
// Offline instances
// ---------------------------------------------------------------------------

#[tauri::command]
pub fn list_offline_instances(state: State<'_, LauncherState>) -> Result<Vec<OfflineInstance>, String> {
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
    Ok(state.offline.instance_dir(&instance.id).display().to_string())
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
        format!("Resolving Minecraft {} runtime...", instance.minecraft_version),
    );
    let required_java = JavaRuntimeSelector::get_required_java_major_version(&instance.minecraft_version);
    let launch_data = state
        .classpath
        .resolve(&instance.minecraft_version, &instance.mod_loader, required_java)
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

    emit_status(app, format!("Starting offline instance '{}'...", instance.name));
    let output = game_output_emitter(app);
    let child = MinecraftRunner
        .launch_offline(&launch_data, &player_name, &java_args, &game_dir, Some(output))
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
fn override_heap(java_args: &str, memory_gb: u32) -> String {
    let tokens: Vec<String> = java_args
        .split_whitespace()
        .map(str::to_string)
        .collect();
    let mut out: Vec<String> = Vec::new();
    for token in tokens {
        let lower = token.to_ascii_lowercase();
        if lower.starts_with("-xmx") || lower.starts_with("-xms") {
            out.push(format!("-Xmx{memory_gb}G"));
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
pub struct ModFileInfo {
    pub filename: String,
    pub size_bytes: u64,
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
            Some(ModFileInfo { filename, size_bytes })
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
        variant: String::new(),
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

/// Saves a PNG file (picked via the dialog) as the active skin + history.
#[tauri::command]
pub fn save_skin(source_path: String) -> Result<(), String> {
    SkinManager::save_skin(Path::new(&source_path)).map_err(err_string)
}

#[tauri::command]
pub fn remove_skin() -> Result<(), String> {
    SkinManager::reset_skin().map_err(err_string)
}

/// History skins, newest first, as data URLs.
#[tauri::command]
pub fn get_skin_history() -> Result<Vec<SkinImage>, String> {
    let mut out = Vec::new();
    for path in SkinManager::get_skin_history() {
        if let Some(data_url) = SkinManager::png_data_url_of(&path) {
            let name = path
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default();
            out.push(SkinImage { name, data_url, variant: String::new() });
        }
    }
    Ok(out)
}

/// Bundled default skins (steve/alex) as data URLs.
#[tauri::command]
pub fn get_bundled_skins() -> Result<Vec<SkinImage>, String> {
    Ok(BundledSkins::all()
        .into_iter()
        .map(|(name, bytes)| SkinImage {
            name,
            data_url: SkinManager::png_data_url(&bytes),
            variant: String::new(),
        })
        .collect())
}

/// Activates a bundled skin by key (`bundled:<name>`), like picking it in the
/// JavaFX gallery and saving.
#[tauri::command]
pub fn save_bundled_skin(key: String) -> Result<(), String> {
    let Some((name, bytes)) = BundledSkins::by_key(&key) else {
        return Err("Unknown bundled skin".to_string());
    };
    let tmp = std::env::temp_dir().join(format!("zircon-bundled-{name}"));
    std::fs::write(&tmp, &bytes).map_err(|e| e.to_string())?;
    let result = SkinManager::save_skin(&tmp);
    let _ = std::fs::remove_file(&tmp);
    result.map_err(err_string)
}

/// Downloads the player's current Mojang skin by UUID, stores it as the active
/// skin + history and returns it for the preview (with the arm variant).
#[tauri::command]
pub async fn fetch_mojang_skin(
    state: State<'_, LauncherState>,
    uuid: String,
) -> Result<SkinImage, String> {
    let downloaded = state.mojang_skin.download(&uuid).await.map_err(err_string)?;
    let tmp = std::env::temp_dir().join(format!(
        "zircon-mojang-skin-{}.png",
        uuid::Uuid::new_v4().simple()
    ));
    std::fs::write(&tmp, &downloaded.png).map_err(|e| e.to_string())?;
    let save_result = SkinManager::save_skin(&tmp);
    let _ = std::fs::remove_file(&tmp);
    save_result.map_err(err_string)?;
    let short = uuid.chars().filter(|c| c.is_ascii_hexdigit()).take(8).collect::<String>();
    Ok(SkinImage {
        name: format!("mojang-{short}.png"),
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
        .upload(&session.access_token, &SkinManager::active_skin_path(), &variant)
        .await
        .map_err(err_string)
}

// ---------------------------------------------------------------------------
// Packs (shaderpacks / resourcepacks)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize)]
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
        locally_added_shaderpacks: selection.locally_added_shaderpacks.iter().cloned().collect(),
        locally_added_resourcepacks: selection
            .locally_added_resourcepacks
            .iter()
            .cloned()
            .collect(),
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
        "shader" => ClientPackManager::add_local_shaderpack(&dir, Path::new(&source_path), &mut selection),
        "resource" => {
            ClientPackManager::add_local_resourcepack(&dir, Path::new(&source_path), &mut selection)
        }
        _ => return Err(format!("Unknown pack type: {kind}")),
    }
    .map_err(err_string)?;
    Ok(filename)
}

#[tauri::command]
pub fn remove_local_pack(
    game_dir: String,
    kind: String,
    filename: String,
) -> Result<(), String> {
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
pub fn set_active_shaderpack(
    game_dir: String,
    filename: Option<String>,
) -> Result<(), String> {
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
    if selection.active_resourcepacks.iter().any(|n| n == &filename) {
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
        .search_mods_with_type(&query, Some(&instance.minecraft_version), loader, Some("mod"))
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
        .list_project_versions(&project_id, Some(&instance.minecraft_version), Some(&instance.mod_loader.r#type))
        .await
        .map_err(|e| e.to_string())?;
    let version = versions
        .into_iter()
        .next()
        .ok_or_else(|| format!("No version of this mod supports Minecraft {} + {} loader", instance.minecraft_version, instance.mod_loader.r#type))?;
    let file = version
        .primary_file()
        .ok_or_else(|| "This mod has no downloadable file".to_string())?;
    let filename = if file.filename.trim().is_empty() {
        format!("{}.jar", version.project_id)
    } else {
        file.filename.clone()
    };
    let dest = state.offline.mods_dir(&instance).join(&filename);
    download_file(&state.http, &file.url, &dest)
        .await
        .map_err(err_string)?;
    Ok(filename)
}

async fn download_file(
    http: &reqwest::Client,
    url: &str,
    dest: &Path,
) -> Result<(), LauncherError> {
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
    tokio::fs::write(dest, bytes).await?;
    Ok(())
}

/// Minecraft versions known to Modrinth (release only), for the instance
/// creation dropdown.
#[tauri::command]
pub async fn list_minecraft_versions(state: State<'_, LauncherState>) -> Result<Vec<String>, String> {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn override_heap_replaces_xmx_and_xms() {
        let args = override_heap("-Xms2G -Xmx4G -XX:+UseG1GC", 8);
        assert_eq!("-Xmx8G -Xmx8G -XX:+UseG1GC", args);
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
}
