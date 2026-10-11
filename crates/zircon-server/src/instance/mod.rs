use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;

use zircon_core::model::InstanceConfig;

use crate::process::console::ConsoleStreamHandler;
use crate::process::driver::ProcessDriver;
use crate::process::manager::MinecraftProcessManager;
use crate::process::player_tracker::PlayerTracker;

use std::fmt;
use std::sync::Mutex;
use ed25519_dalek::SigningKey;

pub mod lifecycle;
pub mod config;
pub mod status;
pub mod persistence;

pub use config::*;

#[cfg(test)]
mod tests;

/// First automatically assigned internal MC port. Disjoint from the
/// player-facing range.
pub const MC_PORT_BASE: i32 = 25700;
/// Lowest player-facing (external) port an instance can be assigned.
pub const EXTERNAL_PORT_BASE: i32 = 25565;
/// Highest player-facing (external) port an instance can be assigned.
pub const EXTERNAL_PORT_MAX: i32 = 25665;

/// Standard Mojang eula.txt content; the server refuses to boot without it.
const EULA_TEXT: &str = "#By changing the settings below to TRUE you are indicating your agreement to our EULA (https://aka.ms/MinecraftEULA).\neula=true\n";

/// Summary of an individual instance's real-time tick and player metrics.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstanceTelemetrySummary {
    pub id: String,
    pub name: String,
    pub running: bool,
    pub tps: Option<f64>,
    pub mspt: Option<f64>,
    pub ping_latency_ms: Option<u64>,
    pub player_count: usize,
}

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

/// Execution / termination status of the last process session for an instance.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstanceRunSession {
    pub start_time: std::time::SystemTime,
    pub stop_requested: bool,
    pub exit_code: i32,
    pub is_running: bool,
}

/// Shared interior state of the instance manager.
pub(crate) struct Inner {
    instance_configs: HashMap<String, InstanceConfig>,
    active_processes: HashMap<String, Arc<MinecraftProcessManager>>,
    player_trackers: HashMap<String, Arc<PlayerTracker>>,
    consoles: HashMap<String, Arc<ConsoleStreamHandler>>,
    last_sessions: HashMap<String, InstanceRunSession>,
    stopping_instances: HashMap<String, String>,
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
    /// Server-level Ed25519 key used to sign instance BOMs on every write.
    signing_key: Option<Arc<SigningKey>>,
    inner: Mutex<Inner>,
    port_binding_listener: Mutex<Option<Arc<dyn PortBindingListener>>>,
    /// Execution driver used to spawn instances (Native or Docker).
    execution_driver: Option<Arc<dyn ProcessDriver>>,
    /// instance_id → a wake/start attempt is in flight. Guards the public
    /// wakeup path from spawning duplicate start tasks for the same instance;
    /// the entry is removed when the start attempt completes (success or
    /// failure) so a later wakeup can retry.
    in_progress_wakes: dashmap::DashSet<String>,
    /// instance_id → (boot_start_time, estimated_duration_secs)
    boot_progress_tracker: Arc<dashmap::DashMap<String, (Instant, u32)>>,
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
            signing_key: None,
            inner: Mutex::new(Inner {
                instance_configs: HashMap::new(),
                active_processes: HashMap::new(),
                player_trackers: HashMap::new(),
                consoles: HashMap::new(),
                last_sessions: HashMap::new(),
                stopping_instances: HashMap::new(),
                active_instance_id: None,
                pending_join_intents: HashMap::new(),
            }),
            port_binding_listener: Mutex::new(None),
            execution_driver: None,
            in_progress_wakes: dashmap::DashSet::new(),
            boot_progress_tracker: Arc::new(dashmap::DashMap::new()),
        };
        manager.load_from_disk()?;
        Ok(manager)
    }

    /// Attaches an execution driver (e.g. DockerDriver or NativeDriver).
    pub fn with_driver(mut self, driver: Option<Arc<dyn ProcessDriver>>) -> Self {
        self.execution_driver = driver;
        self
    }

    /// Returns the attached execution driver, if any.
    pub fn driver(&self) -> Option<Arc<dyn ProcessDriver>> {
        self.execution_driver.clone()
    }

    /// Attaches the server's BOM signing key so instance BOMs are attested on
    /// every write (same key as the legacy store).
    pub fn with_signing_key(mut self, signing_key: Option<Arc<SigningKey>>) -> Self {
        self.signing_key = signing_key;
        self
    }


}
