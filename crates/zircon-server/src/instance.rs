//! Lifecycle manager for multiple isolated Zircon server instances.
//!
//! Each instance lives in `<data>/instances/<id>/` and owns its own
//! `instance.json` (metadata, loader LOCKED here), `bom.json`, `mods/` and
//! `server/` directory — cross-loader file pollution is impossible by
//! construction.
//!
//! The mod loader is frozen at creation: updates only ever mutate
//! name/javaArgs/autoStart and there is no API to change the loader of an
//! existing instance.
//!
//! Port of `com.mcmanager.server.instance.ServerInstanceManager`.

use std::collections::HashMap;
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::Mutex;
use std::time::Instant;

use zircon_core::model::{clamp_idle_shutdown_minutes, BillOfMaterials, InstanceConfig};

use crate::process::console::ConsoleStreamHandler;
use crate::process::manager::MinecraftProcessManager;
use crate::process::player_tracker::PlayerTracker;
use crate::services::bom::BomService;
use crate::services::mods::ModManagementService;

/// First automatically assigned internal MC port. Disjoint from the
/// player-facing range.
pub const MC_PORT_BASE: i32 = 25700;
/// Lowest player-facing (external) port an instance can be assigned.
pub const EXTERNAL_PORT_BASE: i32 = 25565;
/// Highest player-facing (external) port an instance can be assigned.
pub const EXTERNAL_PORT_MAX: i32 = 25665;

/// Standard Mojang eula.txt content; the server refuses to boot without it.
const EULA_TEXT: &str = "#By changing the settings below to TRUE you are indicating your agreement to our EULA (https://aka.ms/MinecraftEULA).\neula=true\n";

/// Implemented by the TCP multiplexer so per-instance external ports are bound
/// as instances are created, updated or deleted.
pub trait PortBindingListener: Send + Sync {
    fn on_instance_added(&self, config: &InstanceConfig);
    fn on_instance_updated(&self, config: &InstanceConfig);
    fn on_instance_removed(&self, instance_id: &str);
}

/// Errors raised by the instance manager.
#[derive(Debug)]
pub enum InstanceError {
    NotFound(String),
    Conflict(String),
    Invalid(String),
    Io(std::io::Error),
}

impl fmt::Display for InstanceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            InstanceError::NotFound(m) => write!(f, "{m}"),
            InstanceError::Conflict(m) => write!(f, "{m}"),
            InstanceError::Invalid(m) => write!(f, "{m}"),
            InstanceError::Io(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for InstanceError {}

impl From<std::io::Error> for InstanceError {
    fn from(e: std::io::Error) -> Self {
        InstanceError::Io(e)
    }
}

fn instance_io_error(e: InstanceError) -> std::io::Error {
    std::io::Error::other(e.to_string())
}

/// Shared interior state of the instance manager.
struct Inner {
    instance_configs: HashMap<String, InstanceConfig>,
    active_processes: HashMap<String, Arc<MinecraftProcessManager>>,
    player_trackers: HashMap<String, Arc<PlayerTracker>>,
    /// The instance whose data the client-facing legacy endpoints serve.
    active_instance_id: Option<String>,
    /// instance_id → expiry of a launcher's "a player is on their way" hold,
    /// set by `/api/join-intent`. The idle-shutdown service skips instances
    /// with a fresh intent, so a server cannot fall asleep while a client is
    /// still preparing to connect. Cleared when the player's login handshake
    /// is consumed by the multiplexer.
    pending_join_intents: HashMap<String, Instant>,
}

/// Lifecycle manager for multiple isolated Zircon server instances.
pub struct ServerInstanceManager {
    instances_dir: PathBuf,
    installer_cache_dir: PathBuf,
    console: Arc<ConsoleStreamHandler>,
    inner: Mutex<Inner>,
    port_binding_listener: Mutex<Option<Arc<dyn PortBindingListener>>>,
}

impl ServerInstanceManager {
    pub fn new(data_dir: &Path, console: Arc<ConsoleStreamHandler>) -> std::io::Result<Self> {
        let instances_dir = data_dir.join("instances");
        let installer_cache_dir = data_dir.join(".cache").join("installers");
        fs::create_dir_all(&instances_dir)?;
        fs::create_dir_all(&installer_cache_dir)?;

        let manager = Self {
            instances_dir,
            installer_cache_dir,
            console,
            inner: Mutex::new(Inner {
                instance_configs: HashMap::new(),
                active_processes: HashMap::new(),
                player_trackers: HashMap::new(),
                active_instance_id: None,
                pending_join_intents: HashMap::new(),
            }),
            port_binding_listener: Mutex::new(None),
        };
        manager.load_from_disk()?;
        Ok(manager)
    }

    // ----------------------------------------------------------------------
    // Instance lifecycle
    // ----------------------------------------------------------------------

    /// Creates a new instance and persists it. The mod loader choice is frozen
    /// in `InstanceConfig` from this moment on.
    pub fn create_instance(
        &self,
        name: &str,
        mc_version: &str,
        loader_type: &str,
        loader_version: &str,
    ) -> Result<InstanceConfig, InstanceError> {
        let config = InstanceConfig::with_external_port(
            name,
            mc_version,
            loader_type,
            loader_version,
            self.allocate_next_port()?,
            self.allocate_next_external_port()?,
        );
        let instance_dir = self.instance_dir(&config.id);
        fs::create_dir_all(instance_dir.join("mods"))?;
        fs::create_dir_all(instance_dir.join("server"))?;
        self.save_instance_to_disk(&config)?;

        let mut inner = self.inner.lock().unwrap();
        inner
            .instance_configs
            .insert(config.id.clone(), config.clone());
        if inner.active_instance_id.is_none() {
            inner.active_instance_id = Some(config.id.clone());
            tracing::info!("Instance '{name}' is now the active instance (first created)");
        }
        drop(inner);

        self.notify_added(&config);
        tracing::info!(
            "Created instance '{name}' (MC {}, loader {}, loader version {}, internal port {}, external port {})",
            mc_version,
            loader_type,
            loader_version,
            config.internal_mc_port,
            config.external_mc_port
        );
        Ok(config)
    }

    pub async fn start_instance(&self, instance_id: &str) -> Result<(), InstanceError> {
        let config = self.get_instance(instance_id)?;
        if !self.is_eula_accepted(instance_id) {
            return Err(InstanceError::Conflict(format!(
                "The Minecraft EULA has not been accepted for instance '{}'. Accept it in the admin UI (Settings tab) first.",
                config.name
            )));
        }

        let pm = {
            let mut inner = self.inner.lock().unwrap();
            if let Some(existing) = inner.active_processes.get(instance_id) {
                existing.clone()
            } else {
                // Each instance gets its own console so its player activity is
                // tracked separately; every line is forwarded to the shared
                // console so the WebSocket console keeps working. The per
                // instance players.json accumulates the ever-joined player log.
                let inst_console = Arc::new(ConsoleStreamHandler::with_players_file(Some(
                    self.instance_dir(instance_id).join("players.json"),
                )));
                let shared = self.console.clone();
                inst_console.add_listener(Box::new(move |line| shared.accept(line)));

                let pm = Arc::new(MinecraftProcessManager::for_instance(
                    Arc::new(config.clone()),
                    self.instance_dir(instance_id).join("server"),
                    self.installer_cache_dir.clone(),
                    inst_console.clone(),
                ));
                inner
                    .player_trackers
                    .insert(instance_id.to_string(), inst_console.player_tracker_arc());
                inner
                    .active_processes
                    .insert(instance_id.to_string(), pm.clone());
                pm
            }
        };

        pm.start()
            .await
            .map_err(|e| InstanceError::Conflict(e.to_string()))?;

        {
            let mut inner = self.inner.lock().unwrap();
            if inner.active_instance_id.as_deref() != Some(instance_id) {
                inner.active_instance_id = Some(instance_id.to_string());
                tracing::info!(
                    "Instance '{}' is now the active instance (started)",
                    config.name
                );
            }
        }

        // A fresh boot clears any prior stop reason, so the instance is no
        // longer considered wakeable while it is up.
        if let Ok(mut config) = self.get_instance(instance_id) {
            if config.last_shutdown_reason.is_some() {
                config.last_shutdown_reason = None;
                if let Err(e) = self.save_instance_to_disk(&config) {
                    tracing::warn!(
                        "Could not clear lastShutdownReason for instance {instance_id}: {e}"
                    );
                } else {
                    self.inner
                        .lock()
                        .unwrap()
                        .instance_configs
                        .insert(instance_id.to_string(), config);
                }
            }
        }

        tracing::info!(
            "Instance '{}' started on internal port {}",
            config.name,
            config.internal_mc_port
        );
        Ok(())
    }

    pub async fn stop_instance(&self, instance_id: &str) {
        self.stop_instance_with_reason(instance_id, None).await;
    }

    /// Stops an instance and records *why* it was stopped. `Some("idle")`
    /// marks the instance wakeable (the launcher auto-starts it on join); any
    /// other reason (admin UI, restart, daemon shutdown) keeps it down.
    /// A stop cancels any in-flight join-intent hold: the server is down, so
    /// the hold no longer means anything, and a manual stop must not be held
    /// off by a stale intent.
    pub async fn stop_instance_with_reason(&self, instance_id: &str, reason: Option<&str>) {
        self.inner
            .lock()
            .unwrap()
            .pending_join_intents
            .remove(instance_id);
        if let Some(reason) = reason {
            if let Ok(mut config) = self.get_instance(instance_id) {
                config.last_shutdown_reason = Some(reason.to_string());
                if let Err(e) = self.save_instance_to_disk(&config) {
                    tracing::warn!(
                        "Could not persist lastShutdownReason for instance {instance_id}: {e}"
                    );
                } else {
                    self.inner
                        .lock()
                        .unwrap()
                        .instance_configs
                        .insert(instance_id.to_string(), config);
                }
            }
        }
        let pm = {
            let mut inner = self.inner.lock().unwrap();
            inner.player_trackers.remove(instance_id);
            inner.active_processes.remove(instance_id)
        };
        if let Some(pm) = pm {
            pm.stop().await;
        }
    }

    /// Renames / re-arms an instance. The `modLoader` can never be changed.
    pub fn update_instance_config(
        &self,
        instance_id: &str,
        new_name: Option<&str>,
        new_java_args: Option<&str>,
    ) -> Result<(), InstanceError> {
        let mut config = self.get_instance(instance_id)?;
        if let Some(name) = new_name {
            if !name.trim().is_empty() {
                config.name = name.trim().to_string();
                self.sync_bom_title(instance_id, &config.name);
            }
        }
        if let Some(args) = new_java_args {
            let sanitized = sanitize_java_args(args);
            if has_invalid_heap_arg(&sanitized) {
                return Err(InstanceError::Invalid(
                    "javaArgs contain an invalid heap size or a forbidden JVM flag (e.g. -XmsNaNG, -javaagent); use plain numbers with an optional G/M/K suffix like -Xmx4G"
                        .to_string(),
                ));
            }
            config.java_args = sanitized;
        }
        self.save_instance_to_disk(&config)?;
        self.inner
            .lock()
            .unwrap()
            .instance_configs
            .insert(instance_id.to_string(), config);
        Ok(())
    }

    pub fn update_auto_start(
        &self,
        instance_id: &str,
        auto_start: bool,
    ) -> Result<(), InstanceError> {
        let mut config = self.get_instance(instance_id)?;
        config.auto_start = auto_start;
        self.save_instance_to_disk(&config)?;
        self.inner
            .lock()
            .unwrap()
            .instance_configs
            .insert(instance_id.to_string(), config);
        Ok(())
    }

    /// Updates the backup schedule (frequency + time of day) of an instance.
    pub fn update_backup_schedule(
        &self,
        instance_id: &str,
        frequency: Option<&str>,
        time: Option<&str>,
    ) -> Result<(), InstanceError> {
        let mut config = self.get_instance(instance_id)?;
        if let Some(f) = frequency {
            if !f.trim().is_empty() {
                config.backup_frequency = f.to_string();
            }
        }
        if let Some(t) = time {
            if !t.trim().is_empty() {
                config.backup_time = t.to_string();
            }
        }
        self.save_instance_to_disk(&config)?;
        self.inner
            .lock()
            .unwrap()
            .instance_configs
            .insert(instance_id.to_string(), config.clone());
        tracing::info!(
            "Instance '{}' backup schedule -> {} at {}",
            config.name,
            config.backup_frequency,
            config.backup_time
        );
        Ok(())
    }

    /// Sets how many backups are kept for an instance.
    pub fn update_backup_retention(
        &self,
        instance_id: &str,
        retention: i32,
    ) -> Result<(), InstanceError> {
        let mut config = self.get_instance(instance_id)?;
        config.backup_retention = retention;
        self.save_instance_to_disk(&config)?;
        self.inner
            .lock()
            .unwrap()
            .instance_configs
            .insert(instance_id.to_string(), config.clone());
        tracing::info!("Instance '{}' backup retention -> {retention}", config.name);
        Ok(())
    }

    /// Updates the idle-shutdown settings of an instance (the window is
    /// clamped to its allowed bounds).
    pub fn update_idle_shutdown_settings(
        &self,
        instance_id: &str,
        enabled: Option<bool>,
        minutes: Option<u32>,
    ) -> Result<(), InstanceError> {
        let mut config = self.get_instance(instance_id)?;
        if let Some(enabled) = enabled {
            config.idle_shutdown_enabled = enabled;
        }
        if let Some(minutes) = minutes {
            config.idle_shutdown_minutes = clamp_idle_shutdown_minutes(minutes);
        }
        self.save_instance_to_disk(&config)?;
        self.inner
            .lock()
            .unwrap()
            .instance_configs
            .insert(instance_id.to_string(), config.clone());
        tracing::info!(
            "Instance '{}' idle shutdown -> enabled={}, {} minutes",
            config.name,
            config.idle_shutdown_enabled,
            config.idle_shutdown_minutes
        );
        Ok(())
    }

    /// Applies a Minecraft / loader version change (and optionally a rename) to
    /// an instance, then re-syncs every installed mod against the new versions.
    /// The mod loader *type* stays locked — only its version string may change.
    pub async fn update_instance_versions(
        &self,
        instance_id: &str,
        new_mc_version: Option<&str>,
        new_loader_version: Option<&str>,
        new_name: Option<&str>,
    ) -> Result<ModSyncSummary, InstanceError> {
        let mut config = self.get_instance(instance_id)?;
        if let Some(name) = new_name {
            if !name.trim().is_empty() {
                config.name = name.trim().to_string();
            }
        }
        if let Some(mc) = new_mc_version {
            if !mc.trim().is_empty() {
                config.minecraft_version = mc.trim().to_string();
            }
        }
        if let Some(v) = new_loader_version {
            config.set_loader_version(v);
        }
        self.save_instance_to_disk(&config)?;
        if let Some(name) = new_name {
            if !name.trim().is_empty() {
                self.sync_bom_title(instance_id, &config.name);
            }
        }
        self.inner
            .lock()
            .unwrap()
            .instance_configs
            .insert(instance_id.to_string(), config.clone());

        let instance_dir = self.instance_dir(instance_id);
        let bom = Arc::new(BomService::new(
            instance_dir.join("bom.json"),
            Some(BillOfMaterials::new(
                config.minecraft_version.clone(),
                config.mod_loader.clone(),
                Some(config.name.clone()),
            )),
        ));
        let mods = ModManagementService::new(bom, instance_dir.join("mods"), "");
        let summary = mods
            .sync_mods_for_version_change(
                &config.minecraft_version,
                config.loader_type(),
                config.loader_version(),
            )
            .await
            .map_err(|e| InstanceError::Io(std::io::Error::other(e.to_string())))?;
        Ok(summary)
    }

    /// Stops (if running), removes the process manager and deletes the instance dir.
    pub async fn delete_instance(&self, instance_id: &str) -> Result<bool, InstanceError> {
        let config = {
            let mut inner = self.inner.lock().unwrap();
            let Some(config) = inner.instance_configs.remove(instance_id) else {
                return Ok(false);
            };
            inner.player_trackers.remove(instance_id);
            inner.pending_join_intents.remove(instance_id);
            config
        };
        // Stop the process outside the lock.
        let pm = self
            .inner
            .lock()
            .unwrap()
            .active_processes
            .remove(instance_id);
        if let Some(pm) = pm {
            pm.stop().await;
        }
        self.notify_removed(instance_id);

        {
            let mut inner = self.inner.lock().unwrap();
            if inner.active_instance_id.as_deref() == Some(instance_id) {
                inner.active_instance_id = self.pick_default_active_instance_locked(&inner);
                tracing::info!(
                    "Active instance is now {}",
                    inner
                        .active_instance_id
                        .clone()
                        .unwrap_or_else(|| "none (legacy mode)".to_string())
                );
            }
        }

        let dir = self.instance_dir(instance_id);
        if dir.is_dir() {
            delete_recursively(&dir)?;
        }
        tracing::info!("Deleted instance '{}'", config.name);
        Ok(true)
    }

    /// Returns `true` if the instance's `server/eula.txt` contains `eula=true`.
    pub fn is_eula_accepted(&self, instance_id: &str) -> bool {
        let eula = self
            .instance_dir(instance_id)
            .join("server")
            .join("eula.txt");
        if !eula.is_file() {
            return false;
        }
        match fs::read_to_string(&eula) {
            Ok(content) => content.lines().any(|line| {
                let line = line.trim();
                line.starts_with("eula=")
                    && line["eula=".len()..].trim().eq_ignore_ascii_case("true")
            }),
            Err(e) => {
                tracing::warn!("Could not read {}: {e}", eula.display());
                false
            }
        }
    }

    /// Writes `server/eula.txt` with `eula=true` (records the operator's consent).
    pub fn accept_eula(&self, instance_id: &str) -> Result<(), InstanceError> {
        let _ = self.get_instance(instance_id)?; // 404 for unknown ids
        let server_dir = self.instance_dir(instance_id).join("server");
        fs::create_dir_all(&server_dir)?;
        fs::write(server_dir.join("eula.txt"), EULA_TEXT)?;
        tracing::info!("EULA accepted for instance {instance_id}");
        Ok(())
    }

    // ----------------------------------------------------------------------
    // Queries
    // ----------------------------------------------------------------------

    pub fn get_instance(&self, instance_id: &str) -> Result<InstanceConfig, InstanceError> {
        self.inner
            .lock()
            .unwrap()
            .instance_configs
            .get(instance_id)
            .cloned()
            .ok_or_else(|| InstanceError::NotFound(format!("Instance not found: {instance_id}")))
    }

    pub fn list_instances(&self) -> Vec<InstanceConfig> {
        self.inner
            .lock()
            .unwrap()
            .instance_configs
            .values()
            .cloned()
            .collect()
    }

    pub fn is_running(&self, instance_id: &str) -> bool {
        self.inner
            .lock()
            .unwrap()
            .active_processes
            .get(instance_id)
            .map(|pm| pm.is_running())
            .unwrap_or(false)
    }

    /// The instance's currently online players (empty when not running).
    pub fn get_online_players(&self, instance_id: &str) -> Vec<String> {
        if !self.is_running(instance_id) {
            return Vec::new();
        }
        self.inner
            .lock()
            .unwrap()
            .player_trackers
            .get(instance_id)
            .map(|t| t.get_online_players())
            .unwrap_or_default()
    }

    pub fn get_online_player_count(&self, instance_id: &str) -> usize {
        self.get_online_players(instance_id).len()
    }

    /// Whether the instance's server has finished booting (printed the vanilla
    /// "Done (...)" line). Used by the idle-shutdown service so its timer only
    /// starts once the server is actually joinable.
    pub fn is_server_ready(&self, instance_id: &str) -> bool {
        self.inner
            .lock()
            .unwrap()
            .player_trackers
            .get(instance_id)
            .map(|t| t.is_ready())
            .unwrap_or(false)
    }

    /// Whether a stopped instance may be auto-started by a launcher wakeup:
    /// true only when the idle-shutdown service put it to sleep. Manual stops
    /// (admin, restart, daemon shutdown) stay down until started manually.
    pub fn wakeable(&self, instance_id: &str) -> bool {
        self.get_instance(instance_id)
            .map(|cfg| cfg.wakeable())
            .unwrap_or(false)
    }

    /// The instant from which idle time should be measured for an instance:
    /// the most recent join/leave/lost event, or boot completion for a server
    /// nobody has played on yet. `None` before the server is ready. Event-
    /// driven, so a player session that falls between idle-service polls still
    /// resets the idle window.
    pub fn idle_since(&self, instance_id: &str) -> Option<Instant> {
        self.inner
            .lock()
            .unwrap()
            .player_trackers
            .get(instance_id)
            .and_then(|t| t.idle_reference())
    }

    /// Defers an instance's idle shutdown by resetting its activity timestamp.
    /// Used when a launcher wakeup arrives while the server is already running
    /// (e.g. its status ping failed transiently during boot), so the server
    /// does not shut down under a player who is about to connect.
    pub fn defer_idle_shutdown(&self, instance_id: &str) {
        if let Some(tracker) = self.inner.lock().unwrap().player_trackers.get(instance_id) {
            tracker.touch_activity();
        }
    }

    /// Records that a launcher has committed to joining this instance: holds
    /// off idle shutdown until the intent expires (`join_intent_ttl`) or the
    /// player's login handshake is consumed by the multiplexer, and defers
    /// the window immediately so the server cannot sleep under a player who
    /// is about to connect.
    pub fn register_join_intent(&self, instance_id: &str) {
        let expiry = Instant::now() + crate::tickets::join_intent_ttl();
        self.inner
            .lock()
            .unwrap()
            .pending_join_intents
            .insert(instance_id.to_string(), expiry);
        self.defer_idle_shutdown(instance_id);
    }

    /// Whether a launcher has a fresh join intent for this instance (a player
    /// is on their way). The idle-shutdown service skips such instances.
    pub fn has_pending_join_intent(&self, instance_id: &str) -> bool {
        self.inner
            .lock()
            .unwrap()
            .pending_join_intents
            .get(instance_id)
            .map(|expiry| *expiry > Instant::now())
            .unwrap_or(false)
    }

    /// Clears a pending join intent — called when the player's connection is
    /// consumed at the multiplexer. The player has arrived, so real player
    /// activity (tracked by the console) governs the idle window from here.
    pub fn clear_pending_join_intent(&self, instance_id: &str) {
        self.inner
            .lock()
            .unwrap()
            .pending_join_intents
            .remove(instance_id);
    }

    pub fn get_process_manager(&self, instance_id: &str) -> Option<Arc<MinecraftProcessManager>> {
        self.inner
            .lock()
            .unwrap()
            .active_processes
            .get(instance_id)
            .cloned()
    }

    /// The shared console every instance's output is forwarded to (also streams
    /// over the admin WebSocket). Used to surface wrapper-level messages.
    pub fn console(&self) -> &Arc<ConsoleStreamHandler> {
        &self.console
    }

    /// Registers the component that binds per-instance external ports.
    pub fn set_port_binding_listener(&self, listener: Arc<dyn PortBindingListener>) {
        *self.port_binding_listener.lock().unwrap() = Some(listener);
    }

    /// Sets the instance's player-facing port manually (e.g. for reverse
    /// proxies). The port must be valid and unique. The multiplexer rebinds
    /// immediately.
    pub fn update_external_port(&self, instance_id: &str, port: i32) -> Result<(), InstanceError> {
        let mut config = self.get_instance(instance_id)?;
        if port <= 0 || port > 65535 {
            return Err(InstanceError::Invalid(
                "Port must be between 1 and 65535".to_string(),
            ));
        }
        {
            let inner = self.inner.lock().unwrap();
            for other in inner.instance_configs.values() {
                if other.id == instance_id {
                    continue;
                }
                if other.external_mc_port == port {
                    return Err(InstanceError::Conflict(format!(
                        "Port {port} is already used by instance '{}'",
                        other.name
                    )));
                }
                if other.internal_mc_port == port {
                    return Err(InstanceError::Conflict(format!(
                        "Port {port} is already used internally by instance '{}'",
                        other.name
                    )));
                }
            }
        }
        config.external_mc_port = port;
        self.save_instance_to_disk(&config)?;
        self.inner
            .lock()
            .unwrap()
            .instance_configs
            .insert(instance_id.to_string(), config.clone());
        self.notify_updated(&config);
        tracing::info!("Instance '{}' external port -> {port}", config.name);
        Ok(())
    }

    /// Resolves the instance whose id or (normalized) name matches a handshake
    /// hostname.
    pub fn find_by_hostname(&self, hostname: &str) -> Option<InstanceConfig> {
        let hostname = hostname.trim().to_lowercase();
        if hostname.is_empty() {
            return None;
        }
        let inner = self.inner.lock().unwrap();
        inner
            .instance_configs
            .values()
            .find(|cfg| cfg.id.to_lowercase() == hostname || normalize_name(&cfg.name) == hostname)
            .cloned()
    }

    /// The instance whose player-facing port matches, or `None`.
    pub fn find_by_external_port(&self, port: i32) -> Option<InstanceConfig> {
        let inner = self.inner.lock().unwrap();
        inner
            .instance_configs
            .values()
            .find(|cfg| cfg.external_mc_port == port)
            .cloned()
    }

    /// The instance whose loopback (internal) MC port matches, or `None`.
    /// Used by the multiplexer to map a proxied connection back to its
    /// instance.
    pub fn find_by_internal_port(&self, port: u16) -> Option<InstanceConfig> {
        let inner = self.inner.lock().unwrap();
        inner
            .instance_configs
            .values()
            .find(|cfg| cfg.internal_mc_port == i32::from(port))
            .cloned()
    }

    pub fn get_instance_dir(&self, instance_id: &str) -> PathBuf {
        self.instance_dir(instance_id)
    }

    pub fn get_instances_dir(&self) -> &Path {
        &self.instances_dir
    }

    /// Re-reads one instance's config from its `instance.json` on disk and
    /// replaces the in-memory copy. Used after a backup restore swaps the
    /// instance directory contents.
    pub fn reload_instance_from_disk(&self, instance_id: &str) -> Result<(), InstanceError> {
        let cfg_file = self.instance_dir(instance_id).join("instance.json");
        if !cfg_file.is_file() {
            return Err(InstanceError::Io(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                format!(
                    "instance.json not found after restore: {}",
                    cfg_file.display()
                ),
            )));
        }
        let content = fs::read_to_string(&cfg_file)?;
        let config: InstanceConfig = serde_json::from_str(&content).map_err(|e| {
            InstanceError::Invalid(format!(
                "Could not parse instance config after restore: {e}"
            ))
        })?;
        if config.id.is_empty() {
            return Err(InstanceError::Invalid(format!(
                "Could not parse instance config after restore: {}",
                cfg_file.display()
            )));
        }
        self.inner
            .lock()
            .unwrap()
            .instance_configs
            .insert(instance_id.to_string(), config.clone());
        tracing::info!(
            "Reloaded instance config from disk after restore: '{}'",
            config.name
        );
        Ok(())
    }

    /// The instance whose data the client-facing legacy endpoints serve, or
    /// `None` when the wrapper runs in pure legacy mode (no instances exist).
    pub fn get_active_instance(&self) -> Option<InstanceConfig> {
        let inner = self.inner.lock().unwrap();
        inner
            .active_instance_id
            .as_ref()
            .and_then(|id| inner.instance_configs.get(id))
            .cloned()
    }

    // ----------------------------------------------------------------------
    // persistence
    // ----------------------------------------------------------------------

    fn load_from_disk(&self) -> std::io::Result<()> {
        let mut loaded = 0usize;
        let mut inner = self.inner.lock().unwrap();
        if let Ok(entries) = fs::read_dir(&self.instances_dir) {
            for entry in entries.flatten() {
                let dir = entry.path();
                if !dir.is_dir() {
                    continue;
                }
                let cfg_file = dir.join("instance.json");
                if !cfg_file.is_file() {
                    continue;
                }
                match fs::read_to_string(&cfg_file)
                    .map_err(|e| std::io::Error::other(e.to_string()))
                    .and_then(|content| {
                        serde_json::from_str::<InstanceConfig>(&content)
                            .map_err(|e| std::io::Error::other(e.to_string()))
                    }) {
                    Ok(mut config) if !config.id.is_empty() => {
                        // Pre-external-port instances get a player-facing port
                        // assigned and persisted on first load.
                        if config.external_mc_port <= 0 {
                            config.external_mc_port = self
                                .allocate_next_external_port()
                                .unwrap_or(EXTERNAL_PORT_BASE);
                            self.save_instance_to_disk(&config)
                                .map_err(instance_io_error)?;
                        }
                        // Relocate legacy internal ports out of the player-facing range.
                        if config.internal_mc_port >= EXTERNAL_PORT_BASE
                            && config.internal_mc_port <= EXTERNAL_PORT_MAX
                        {
                            config.internal_mc_port =
                                self.allocate_next_port().unwrap_or(MC_PORT_BASE);
                            self.save_instance_to_disk(&config)
                                .map_err(instance_io_error)?;
                        }
                        tracing::info!(
                            "Loaded instance '{}' ({} {}, internal port {})",
                            config.name,
                            config.minecraft_version,
                            config.loader_type(),
                            config.internal_mc_port
                        );
                        inner.instance_configs.insert(config.id.clone(), config);
                        loaded += 1;
                    }
                    Err(e) => {
                        tracing::warn!("Could not parse {}, skipping: {e}", cfg_file.display());
                    }
                    _ => {}
                }
            }
        }
        tracing::info!(
            "Loaded {loaded} instance(s) from {}",
            self.instances_dir.display()
        );
        if inner.active_instance_id.is_none() {
            inner.active_instance_id = self.pick_default_active_instance_locked(&inner);
            if let Some(id) = &inner.active_instance_id {
                tracing::info!("Active instance for client sync: {id}");
            }
        }
        Ok(())
    }

    fn pick_default_active_instance_locked(&self, inner: &Inner) -> Option<String> {
        let mut ids: Vec<&String> = inner.instance_configs.keys().collect();
        ids.sort();
        ids.first().map(|s| (*s).clone())
    }

    fn save_instance_to_disk(&self, config: &InstanceConfig) -> Result<(), InstanceError> {
        let cfg_file = self.instance_dir(&config.id).join("instance.json");
        let json = serde_json::to_string_pretty(config)
            .map_err(|e| InstanceError::Invalid(format!("Could not serialize instance: {e}")))?;
        fs::write(cfg_file, json)?;
        Ok(())
    }

    /// Keeps the instance's published BOM title in sync with its (web-app) name.
    fn sync_bom_title(&self, instance_id: &str, name: &str) {
        let bom_file = self.instance_dir(instance_id).join("bom.json");
        if !bom_file.is_file() {
            return;
        }
        let result = fs::read_to_string(&bom_file)
            .map_err(|e| std::io::Error::other(e.to_string()))
            .and_then(|content| {
                let mut bom: BillOfMaterials = serde_json::from_str(&content)
                    .map_err(|e| std::io::Error::other(e.to_string()))?;
                bom.server_title = Some(name.to_string());
                let json = serde_json::to_string_pretty(&bom)
                    .map_err(|e| std::io::Error::other(e.to_string()))?;
                fs::write(&bom_file, json)
            });
        if let Err(e) = result {
            tracing::warn!("Could not update BOM title for instance '{name}': {e}");
        }
    }

    // ----------------------------------------------------------------------
    // helpers
    // ----------------------------------------------------------------------

    fn instance_dir(&self, instance_id: &str) -> PathBuf {
        self.instances_dir.join(instance_id)
    }

    fn notify_added(&self, config: &InstanceConfig) {
        if let Some(listener) = self.port_binding_listener.lock().unwrap().clone() {
            listener.on_instance_added(config);
        }
    }

    fn notify_updated(&self, config: &InstanceConfig) {
        if let Some(listener) = self.port_binding_listener.lock().unwrap().clone() {
            listener.on_instance_updated(config);
        }
    }

    fn notify_removed(&self, instance_id: &str) {
        if let Some(listener) = self.port_binding_listener.lock().unwrap().clone() {
            listener.on_instance_removed(instance_id);
        }
    }

    /// Picks the next free internal port above `MC_PORT_BASE`.
    fn allocate_next_port(&self) -> Result<i32, InstanceError> {
        let inner = self.inner.lock().unwrap();
        let max = inner
            .instance_configs
            .values()
            .map(|cfg| cfg.internal_mc_port)
            .max()
            .unwrap_or(MC_PORT_BASE - 1);
        Ok(max + 1)
    }

    /// Picks the lowest free player-facing port in
    /// `[EXTERNAL_PORT_BASE..EXTERNAL_PORT_MAX]`. 25565 is the shared web/main
    /// port — only the first instance may own it.
    fn allocate_next_external_port(&self) -> Result<i32, InstanceError> {
        let inner = self.inner.lock().unwrap();
        let start = if inner.instance_configs.is_empty() {
            EXTERNAL_PORT_BASE
        } else {
            EXTERNAL_PORT_BASE + 1
        };
        for port in start..=EXTERNAL_PORT_MAX {
            let used = inner
                .instance_configs
                .values()
                .any(|cfg| cfg.external_mc_port == port);
            if !used {
                return Ok(port);
            }
        }
        Err(InstanceError::Conflict(format!(
            "No free player-facing ports left in {start}-{EXTERNAL_PORT_MAX} \
             (free a port manually or lower EXTERNAL_PORT_MAX)"
        )))
    }
}

/// Deletes a file or directory tree; a no-op when the path does not exist.
pub fn delete_recursively(path: &Path) -> std::io::Result<()> {
    if !path.exists() {
        return Ok(());
    }
    if path.is_dir() {
        fs::remove_dir_all(path)
    } else {
        fs::remove_file(path)
    }
}

fn normalize_name(name: &str) -> String {
    let mut out = String::new();
    for c in name.trim().to_lowercase().chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c);
        } else {
            out.push('-');
        }
    }
    out
}

/// Allows only safe JVM flag characters; everything else is stripped.
/// Matches Java's `replaceAll("[^\\w.\\-+ ]", "")` (\\w = [A-Za-z0-9_]).
pub fn sanitize_java_args(java_args: &str) -> String {
    if java_args.trim().is_empty() {
        return "-Xms2G -Xmx4G".to_string();
    }
    let sanitized: String = java_args
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '.' | '-' | '+' | ' '))
        .collect();
    sanitized.trim().to_string()
}

/// JVM flags capable of native arbitrary code execution (`-javaagent`, remote
/// debugging, classpath boot-patching...). Any of these in instance `javaArgs`
/// is refused at save time — a compromised operator account must not be able
/// to weaponize a server launch.
const FORBIDDEN_JVM_PREFIXES: &[&str] = &[
    "-javaagent",
    "-agentlib",
    "-agentpath",
    "-xbootclasspath",
    "-xdebug",
    "-xrunjdwp",
    "-djava.security.manager",
];

/// Returns `true` when `args` contains a forbidden JVM flag (case-insensitive).
/// Uses substring matching so `-javaagent:foo.jar` and the sanitizer's
/// separator-stripped `-javaagentfoo.jar` are both caught.
pub fn contains_dangerous_jvm_args(args: &str) -> bool {
    let lower = args.to_ascii_lowercase();
    FORBIDDEN_JVM_PREFIXES.iter().any(|p| lower.contains(p))
}

/// Whether the args contain a forbidden JVM flag or an `-Xms`/`-Xmx` heap flag
/// whose value is not a positive number (optionally suffixed with G/M/K), e.g.
/// `-XmsNaNG`. The JVM would reject such args at launch (or, worse, load a
/// malicious agent), so they are refused at save time.
pub fn has_invalid_heap_arg(args: &str) -> bool {
    if contains_dangerous_jvm_args(args) {
        return true;
    }
    args.split_whitespace().any(|token| {
        let lower = token.to_ascii_lowercase();
        let value = if let Some(v) = lower.strip_prefix("-xmx") {
            Some(v)
        } else if let Some(v) = lower.strip_prefix("-xms") {
            Some(v)
        } else {
            None
        };
        match value {
            Some(v) => {
                let number = v.trim_end_matches(['g', 'm', 'k']);
                number.parse::<f64>().map(|n| !(n > 0.0)).unwrap_or(true)
            }
            None => false,
        }
    })
}

/// Summary of a mod re-sync after an instance version change.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModSyncSummary {
    pub updated_count: i32,
    pub incompatible_count: i32,
    pub updated_mods: Vec<String>,
    pub incompatible_mods: Vec<String>,
}

use serde::{Deserialize, Serialize};

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    use zircon_core::model::{SHUTDOWN_REASON_IDLE, SHUTDOWN_REASON_MANUAL};

    fn temp_dir() -> PathBuf {
        crate::test_util::temp_dir("instances")
    }

    #[test]
    fn creates_and_lists_instances_with_unique_ports() {
        let dir = temp_dir();
        let console = Arc::new(ConsoleStreamHandler::new());
        let manager = ServerInstanceManager::new(&dir, console).unwrap();

        let first = manager
            .create_instance("Alpha", "1.20.4", "fabric", "0.15.11")
            .unwrap();
        assert_eq!(8, first.id.len());
        assert_eq!(MC_PORT_BASE, first.internal_mc_port);
        assert_eq!(EXTERNAL_PORT_BASE, first.external_mc_port); // first owns 25565
        assert_eq!("fabric", first.loader_type());

        let second = manager
            .create_instance("Beta", "1.21", "neoforge", "21.1.0")
            .unwrap();
        assert_eq!(MC_PORT_BASE + 1, second.internal_mc_port);
        assert_eq!(EXTERNAL_PORT_BASE + 1, second.external_mc_port);

        assert_eq!(2, manager.list_instances().len());
        assert!(manager
            .get_instance_dir(&first.id)
            .join("instance.json")
            .is_file());
        assert!(manager.get_instance_dir(&first.id).join("mods").is_dir());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn loader_is_locked_but_version_can_change() {
        let dir = temp_dir();
        let console = Arc::new(ConsoleStreamHandler::new());
        let manager = ServerInstanceManager::new(&dir, console).unwrap();
        let instance = manager
            .create_instance("Locked", "1.20.4", "forge", "47.0.0")
            .unwrap();

        let mut config = manager.get_instance(&instance.id).unwrap();
        config.set_loader_version("47.1.0");
        manager.save_instance_to_disk(&config).unwrap();

        // The persisted file carries the new loader version while the loader
        // type stays locked.
        let json = fs::read_to_string(manager.get_instance_dir(&instance.id).join("instance.json"))
            .unwrap();
        let persisted: InstanceConfig = serde_json::from_str(&json).unwrap();
        assert_eq!("47.1.0", persisted.loader_version());
        assert_eq!("forge", persisted.loader_type());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn eula_flow() {
        let dir = temp_dir();
        let console = Arc::new(ConsoleStreamHandler::new());
        let manager = ServerInstanceManager::new(&dir, console).unwrap();
        let instance = manager
            .create_instance("Eula", "1.20.4", "vanilla", "")
            .unwrap();

        assert!(!manager.is_eula_accepted(&instance.id));
        manager.accept_eula(&instance.id).unwrap();
        assert!(manager.is_eula_accepted(&instance.id));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn hostname_lookup_matches_id_and_normalized_name() {
        let dir = temp_dir();
        let console = Arc::new(ConsoleStreamHandler::new());
        let manager = ServerInstanceManager::new(&dir, console).unwrap();
        let instance = manager
            .create_instance("My Cool Server", "1.20.4", "vanilla", "")
            .unwrap();

        assert_eq!(
            Some(instance.id.clone()),
            manager.find_by_hostname(&instance.id).map(|c| c.id)
        );
        assert_eq!(
            Some(instance.id.clone()),
            manager.find_by_hostname("my-cool-server").map(|c| c.id)
        );
        assert_eq!(None, manager.find_by_hostname("unknown"));
        let _ = fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn idle_stop_reason_is_persisted_and_marks_wakeable() {
        let dir = temp_dir();
        let console = Arc::new(ConsoleStreamHandler::new());
        let manager = ServerInstanceManager::new(&dir, console).unwrap();
        let instance = manager
            .create_instance("Sleepy", "1.20.4", "vanilla", "")
            .unwrap();

        // The idle service records its stop reason; the instance becomes
        // wakeable and the marker survives a reload from disk (wrapper restart).
        manager
            .stop_instance_with_reason(&instance.id, Some(SHUTDOWN_REASON_IDLE))
            .await;
        let config = manager.get_instance(&instance.id).unwrap();
        assert_eq!(
            Some(SHUTDOWN_REASON_IDLE.to_string()),
            config.last_shutdown_reason
        );
        assert!(manager.wakeable(&instance.id));
        manager.reload_instance_from_disk(&instance.id).unwrap();
        assert!(manager.wakeable(&instance.id));

        // A manual stop (e.g. admin maintenance) marks the instance non-wakeable.
        manager
            .stop_instance_with_reason(&instance.id, Some(SHUTDOWN_REASON_MANUAL))
            .await;
        assert!(!manager.wakeable(&instance.id));

        // A plain stop (daemon shutdown / restart) never clobbers an existing
        // idle marker, so slept servers stay wakeable across wrapper restarts.
        manager
            .stop_instance_with_reason(&instance.id, Some(SHUTDOWN_REASON_IDLE))
            .await;
        manager.stop_instance(&instance.id).await;
        assert!(manager.wakeable(&instance.id));
        let _ = fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn join_intent_holds_off_idle_shutdown_until_expiry_or_clear() {
        let dir = temp_dir();
        let console = Arc::new(ConsoleStreamHandler::new());
        let manager = ServerInstanceManager::new(&dir, console).unwrap();
        let instance = manager
            .create_instance("Held", "1.20.4", "vanilla", "")
            .unwrap();

        // No intent → the idle service is free to shut the instance down.
        assert!(!manager.has_pending_join_intent(&instance.id));

        // Registering an intent holds the instance against idle shutdown and
        // defers the idle window immediately.
        manager.register_join_intent(&instance.id);
        assert!(manager.has_pending_join_intent(&instance.id));

        // The hold expires when the launcher stops refreshing it — a crashed
        // launcher must not leave the server awake forever.
        {
            let mut inner = manager.inner.lock().unwrap();
            if let Some(expiry) = inner.pending_join_intents.get_mut(&instance.id) {
                *expiry = Instant::now() - Duration::from_secs(1);
            }
        }
        assert!(!manager.has_pending_join_intent(&instance.id));

        // A fresh intent is cleared when the player actually connects.
        manager.register_join_intent(&instance.id);
        assert!(manager.has_pending_join_intent(&instance.id));
        manager.clear_pending_join_intent(&instance.id);
        assert!(!manager.has_pending_join_intent(&instance.id));

        // Stopping the instance cancels any in-flight hold.
        manager.register_join_intent(&instance.id);
        manager.stop_instance(&instance.id).await;
        assert!(!manager.has_pending_join_intent(&instance.id));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn internal_port_lookup_finds_the_instance() {
        let dir = temp_dir();
        let console = Arc::new(ConsoleStreamHandler::new());
        let manager = ServerInstanceManager::new(&dir, console).unwrap();
        let instance = manager
            .create_instance("Internal", "1.20.4", "vanilla", "")
            .unwrap();

        assert_eq!(
            Some(instance.id.clone()),
            manager
                .find_by_internal_port(instance.internal_mc_port as u16)
                .map(|c| c.id)
        );
        assert_eq!(None, manager.find_by_internal_port(1));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn java_args_are_sanitized() {
        // Parity with the Java regex [^\w.\-+ ]: metacharacters are stripped
        // but plain words survive.
        assert_eq!("-Xmx4G rm -rf", sanitize_java_args("-Xmx4G; rm -rf /"));
        assert_eq!("-Xms2G -Xmx4G", sanitize_java_args("-Xms2G -Xmx4G"));
        assert_eq!("-Xms2G -Xmx4G", sanitize_java_args("  "));
    }

    #[test]
    fn invalid_heap_args_are_detected() {
        // NaN / undefined values (browser slider edge cases) must never reach
        // the JVM, which aborts at launch on them.
        assert!(has_invalid_heap_arg("-XmsNaNG -Xmx4G"));
        assert!(has_invalid_heap_arg("-XmxundefinedG"));
        assert!(has_invalid_heap_arg("-Xmx"));
        assert!(has_invalid_heap_arg("-Xms0G"));
        assert!(has_invalid_heap_arg("-Xmx-4G"));

        // Valid heap flags pass through.
        assert!(!has_invalid_heap_arg("-Xms2G -Xmx4G"));
        assert!(!has_invalid_heap_arg("-Xms512M -Xmx1g"));
        assert!(!has_invalid_heap_arg("-Xmx8"));
        assert!(!has_invalid_heap_arg(
            "-XX:MaxRAMPercentage=75.0 -XX:+UseG1GC"
        ));
    }

    #[test]
    fn dangerous_jvm_flags_are_detected_case_insensitively() {
        assert!(contains_dangerous_jvm_args("-javaagent:agent.jar"));
        assert!(contains_dangerous_jvm_args(
            "-agentlib:jdwp=transport=dt_socket"
        ));
        assert!(contains_dangerous_jvm_args("-agentpath:/tmp/evil.so"));
        assert!(contains_dangerous_jvm_args("-Xbootclasspath/a:evil.jar"));
        assert!(contains_dangerous_jvm_args("-Xdebug"));
        assert!(contains_dangerous_jvm_args("-Xrunjdwp:transport=dt_socket"));
        assert!(contains_dangerous_jvm_args("-Djava.security.manager"));
        // Case-insensitive and robust after the sanitizer strips separators.
        assert!(contains_dangerous_jvm_args("-JAVAAGENT:evil.jar"));
        assert!(contains_dangerous_jvm_args("-javaagentevil.jar"));

        // Legitimate flags pass.
        assert!(!contains_dangerous_jvm_args("-Xmx4G -XX:+UseG1GC"));
        assert!(!contains_dangerous_jvm_args("-Xms2G"));
        assert!(!contains_dangerous_jvm_args(""));

        // The invalid-args gate rejects dangerous flags even with valid heaps.
        assert!(has_invalid_heap_arg("-Xms2G -Xmx4G -javaagent:evil.jar"));
        assert!(has_invalid_heap_arg("-Xmx4G -Djava.security.manager"));
    }

    #[test]
    fn update_instance_config_rejects_invalid_heap_args() {
        let dir = temp_dir();
        let console = Arc::new(ConsoleStreamHandler::new());
        let manager = ServerInstanceManager::new(&dir, console).unwrap();
        let instance = manager
            .create_instance("HeapGuard", "1.20.4", "vanilla", "")
            .unwrap();

        let res =
            manager.update_instance_config(&instance.id, None, Some("-XmsNaNG -XmxundefinedG"));
        assert!(matches!(res, Err(InstanceError::Invalid(_))));
        // The stored args are untouched.
        assert_eq!(
            "-Xms2G -Xmx4G",
            manager.get_instance(&instance.id).unwrap().java_args
        );

        // A valid replacement still applies.
        manager
            .update_instance_config(&instance.id, None, Some("-Xms4G -Xmx8G"))
            .unwrap();
        assert_eq!(
            "-Xms4G -Xmx8G",
            manager.get_instance(&instance.id).unwrap().java_args
        );
        let _ = fs::remove_dir_all(&dir);
    }
}
