//! Warm Headless Preview Session Pool and Real-Time Viewport Streaming.
//!
//! Keeps headless JVM Minecraft instances warm in memory while users pan around
//! the Slippy World Map Studio. Commands (`forceload add <min> <max>`) are dynamically
//! piped into the warm process's stdin. An idle watchdog gracefully terminates instances
//! after 60 seconds of inactivity to conserve RAM and CPU.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin, Command};
use tokio::sync::Mutex;

use zircon_core::model::metadata::ModLoaderType;
use zircon_core::world::WorldPreviewRequest;

use std::sync::atomic::{AtomicBool, Ordering};

use super::preview::{get_ephemeral_port, WorldPreviewService};

/// Maximum idle duration before warm headless instance is gracefully flushed and stopped.
pub const WARM_SESSION_IDLE_TIMEOUT: Duration = Duration::from_secs(60);

/// Represents an active warm headless preview server session.
pub struct WarmPreviewSession {
    pub instance_id: String,
    pub seed: String,
    pub target_dir: PathBuf,
    pub port: u16,
    pub last_activity: Instant,
    pub is_ready: Arc<AtomicBool>,
    pub is_busy: bool,
    pub stdin: Arc<Mutex<ChildStdin>>,
    pub child: Arc<Mutex<Child>>,
}

/// Global thread-safe manager for warm preview sessions across instances.
#[derive(Clone)]
pub struct WarmSessionManager {
    sessions: Arc<Mutex<HashMap<String, Arc<Mutex<WarmPreviewSession>>>>>,
}

impl WarmSessionManager {
    pub fn new() -> Self {
        let manager = Self {
            sessions: Arc::new(Mutex::new(HashMap::new())),
        };

        // Start background idle watchdog
        let sessions_clone = manager.sessions.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(5));
            loop {
                interval.tick().await;
                Self::prune_idle_sessions(&sessions_clone).await;
            }
        });

        manager
    }

    /// Background task that stops and cleans up sessions that have been idle for >60s.
    async fn prune_idle_sessions(sessions: &Arc<Mutex<HashMap<String, Arc<Mutex<WarmPreviewSession>>>>>) {
        let mut to_stop = Vec::new();
        {
            let map = sessions.lock().await;
            let now = Instant::now();
            for (id, session_lock) in map.iter() {
                let session = session_lock.lock().await;
                if now.duration_since(session.last_activity) >= WARM_SESSION_IDLE_TIMEOUT {
                    to_stop.push(id.clone());
                }
            }
        }

        for id in to_stop {
            tracing::info!("[WarmSession] Idle timeout reached for instance '{}' (60s). Gracefully stopping...", id);
            let session_opt = {
                let mut map = sessions.lock().await;
                map.remove(&id)
            };

            if let Some(session_lock) = session_opt {
                let mut session = session_lock.lock().await;
                let _ = Self::terminate_session(&mut session).await;
            }
        }
    }

    /// Terminates a warm session gracefully: sends stop, waits up to 5s, kills if stuck.
    async fn terminate_session(session: &mut WarmPreviewSession) {
        tracing::info!("[WarmSession] Terminating warm session for '{}'", session.instance_id);
        {
            let mut stdin = session.stdin.lock().await;
            let _ = stdin.write_all(b"forceload remove all\nsave-all flush\nstop\n").await;
            let _ = stdin.flush().await;
        }

        let mut child = session.child.lock().await;
        let _ = tokio::time::timeout(Duration::from_secs(5), child.wait()).await;
        let _ = child.kill().await;
        session.is_ready.store(false, Ordering::SeqCst);
        session.is_busy = false;
    }

    /// Explicitly stops a warm session for an instance (e.g. user leaves World tab or switches instance).
    pub async fn stop_session(&self, instance_id: &str) {
        let session_opt = {
            let mut map = self.sessions.lock().await;
            map.remove(instance_id)
        };

        if let Some(session_lock) = session_opt {
            let mut session = session_lock.lock().await;
            Self::terminate_session(&mut session).await;
        }
    }

    /// Ensures a warm session is running for the given instance and seed.
    /// If none exists, boots one asynchronously in the background.
    pub async fn ensure_session(
        &self,
        instance_dir: &Path,
        seed: &str,
        selected_mods: Option<Vec<String>>,
    ) -> Result<WarmSessionStatus, String> {
        let instance_id = instance_dir
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("default")
            .to_string();

        let mut map = self.sessions.lock().await;
        if let Some(session_lock) = map.get(&instance_id) {
            let mut session = session_lock.lock().await;
            // If seed changed, terminate the old session and start fresh
            if session.seed != seed && !seed.trim().is_empty() {
                tracing::info!(
                    "[WarmSession] Seed changed from '{}' to '{}' for instance '{}'. Restarting session...",
                    session.seed,
                    seed,
                    instance_id
                );
                Self::terminate_session(&mut session).await;
            } else {
                session.last_activity = Instant::now();
                return Ok(WarmSessionStatus {
                    is_ready: session.is_ready.load(Ordering::SeqCst),
                    is_busy: session.is_busy,
                    port: session.port,
                });
            }
        }

        // Spawn new warm session
        tracing::info!("[WarmSession] Starting new warm headless session for instance '{}' (seed: '{}')", instance_id, seed);
        let session = Self::spawn_session(instance_dir, seed, selected_mods).await?;
        let status = WarmSessionStatus {
            is_ready: session.is_ready.load(Ordering::SeqCst),
            is_busy: session.is_busy,
            port: session.port,
        };

        map.insert(instance_id, Arc::new(Mutex::new(session)));
        Ok(status)
    }

    /// Streams a viewport bounding box into the warm headless server.
    pub async fn stream_viewport(
        &self,
        instance_dir: &Path,
        seed: &str,
        min_x: i32,
        min_z: i32,
        max_x: i32,
        max_z: i32,
    ) -> Result<String, String> {
        let instance_id = instance_dir
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("default");

        // Ensure session exists
        let session_lock = {
            let map = self.sessions.lock().await;
            map.get(instance_id).cloned()
        };

        let session_lock = match session_lock {
            Some(s) => s,
            None => {
                // Ensure session starts if not already running
                self.ensure_session(instance_dir, seed, None).await?;
                let map = self.sessions.lock().await;
                map.get(instance_id).cloned().ok_or_else(|| "Session failed to initialize".to_string())?
            }
        };

        let mut session = session_lock.lock().await;
        session.last_activity = Instant::now();

        if !session.is_ready.load(Ordering::SeqCst) {
            return Ok("Worker is still booting; command queued for initialization.".to_string());
        }

        // Clamp viewport request to prevent runaway chunk floods or out-of-world coordinates
        let c_min_x = min_x.clamp(-29999984, 29999984);
        let c_min_z = min_z.clamp(-29999984, 29999984);
        let c_max_x = max_x.clamp(-29999984, 29999984);
        let c_max_z = max_z.clamp(-29999984, 29999984);

        // Convert block coordinates to chunk coordinates
        let min_cx = c_min_x >> 4;
        let min_cz = c_min_z >> 4;
        let max_cx = c_max_x >> 4;
        let max_cz = c_max_z >> 4;

        // Split viewport into batches of at most 16x16 chunks (<= 256 chunks) to adhere to Minecraft's limit
        let mut commands = Vec::new();
        let step = 15; // 0..=15 is 16 chunks
        let mut cur_cx = min_cx;
        while cur_cx <= max_cx {
            let next_cx = (cur_cx + step).min(max_cx);
            let mut cur_cz = min_cz;
            while cur_cz <= max_cz {
                let next_cz = (cur_cz + step).min(max_cz);
                let b_from_x = cur_cx * 16;
                let b_from_z = cur_cz * 16;
                let b_to_x = next_cx * 16 + 15;
                let b_to_z = next_cz * 16 + 15;
                commands.push(format!("forceload add {b_from_x} {b_from_z} {b_to_x} {b_to_z}\n"));
                cur_cz = next_cz + 1;
            }
            cur_cx = next_cx + 1;
        }

        tracing::info!(
            "[WarmSession] Viewport stream for '{}': dispatched {} forceload batch(es) for ({}, {}) -> ({}, {})",
            instance_id,
            commands.len(),
            c_min_x,
            c_min_z,
            c_max_x,
            c_max_z
        );

        let mut stdin = session.stdin.lock().await;
        for cmd in &commands {
            stdin
                .write_all(cmd.as_bytes())
                .await
                .map_err(|e| format!("Failed to write forceload to worker: {e}"))?;
        }
        stdin
            .flush()
            .await
            .map_err(|e| format!("Failed to flush stdin: {e}"))?;

        // Schedule delayed save-all flush so chunks have a tick window to generate and write to disk
        let stdin_clone = session.stdin.clone();
        tokio::spawn(async move {
            tokio::time::sleep(tokio::time::Duration::from_millis(1500)).await;
            let mut sin = stdin_clone.lock().await;
            let _ = sin.write_all(b"save-all flush\n").await;
            let _ = sin.flush().await;
        });

        Ok(format!(
            "Dispatched chunk generation for bounds ({min_x}, {min_z}) to ({max_x}, {max_z}) in {} batch(es)",
            commands.len()
        ))
    }

    /// Spawns the underlying JVM process and sets up stdout listener for boot detection.
    async fn spawn_session(
        instance_dir: &Path,
        seed: &str,
        selected_mods: Option<Vec<String>>,
    ) -> Result<WarmPreviewSession, String> {
        let server_dir = instance_dir.join("server");
        let src_root = if server_dir.exists() {
            server_dir
        } else {
            instance_dir.to_path_buf()
        };

        let config_path = instance_dir.join("instance.json");
        let (mc_version, mod_loader) = if let Ok(content) = fs::read_to_string(&config_path) {
            if let Ok(cfg) = serde_json::from_str::<zircon_core::model::instance::InstanceConfig>(&content) {
                (cfg.minecraft_version, cfg.mod_loader)
            } else {
                ("1.21.1".to_string(), None)
            }
        } else {
            ("1.21.1".to_string(), None)
        };

        let worker_port = get_ephemeral_port();
        let requested_seed = seed.trim();

        // Check if clean instance (no region files in live server world)
        let live_region_dir_opt = WorldPreviewService::find_region_dir(instance_dir, &src_root);
        let is_clean_instance = live_region_dir_opt.is_none();

        let req = WorldPreviewRequest {
            seed: requested_seed.to_string(),
            center_x: 0,
            center_z: 0,
            radius_blocks: 512,
            zoom: 1.0,
            selected_mods,
        };

        let target_dir = if is_clean_instance {
            let _ = WorldPreviewService::prepare_server_properties(&src_root.join("server.properties"), requested_seed, worker_port);
            WorldPreviewService::sync_preview_mods(instance_dir, &src_root, &req);
            src_root.clone()
        } else {
            let seed_name = if !requested_seed.is_empty() { requested_seed } else { "default" };
            let cache_target = instance_dir.join(".cache").join("world-seeds").join(seed_name);
            WorldPreviewService::setup_persistent_dir(instance_dir, &src_root, &cache_target, &req, worker_port)
                .map_err(|e| format!("Failed to configure persistent seed cache: {e}"))?;
            cache_target
        };

        // Command assembly
        let java_version = crate::process::manager::detect_java_version(&mc_version);
        let java_bin = crate::installer::java_bin_for_version(java_version);
        let mut cmd = Command::new(java_bin);
        cmd.current_dir(&target_dir);
        cmd.arg("-Xmx1536M");
        cmd.arg("-Xms512M");
        cmd.arg("-Djava.awt.headless=true");
        cmd.arg("-Dmixin.debug.countInjections=false");

        let loader = mod_loader
            .as_ref()
            .and_then(|info| ModLoaderType::from_id(&info.r#type))
            .unwrap_or(ModLoaderType::Vanilla);

        if loader.is_forge_like() {
            let loader_ver = mod_loader.as_ref().map(|l| l.version.as_str()).unwrap_or("");
            let user_jvm = target_dir.join("user_jvm_args.txt");
            if user_jvm.exists() {
                cmd.arg(format!("@{}", user_jvm.to_string_lossy()));
            } else if src_root.join("user_jvm_args.txt").exists() {
                cmd.arg(format!("@{}", src_root.join("user_jvm_args.txt").to_string_lossy()));
            }

            if let Some(args_file) = crate::installer::find_server_args_file(&target_dir, loader_ver)
                .or_else(|| crate::installer::find_server_args_file(&src_root, loader_ver))
            {
                cmd.arg(format!("@{}", args_file.to_string_lossy()));
            } else {
                let s_jar = target_dir.join("server.jar");
                if s_jar.exists() {
                    cmd.arg("-jar").arg(s_jar);
                }
            }
        } else if loader == ModLoaderType::Quilt && (target_dir.join("quilt-server-launch.jar").exists() || src_root.join("quilt-server-launch.jar").exists()) {
            let jar = if target_dir.join("quilt-server-launch.jar").exists() {
                target_dir.join("quilt-server-launch.jar")
            } else {
                src_root.join("quilt-server-launch.jar")
            };
            cmd.arg("-jar").arg(jar);
        } else if loader == ModLoaderType::Fabric && (target_dir.join("fabric-server-launch.jar").exists() || src_root.join("fabric-server-launch.jar").exists()) {
            let jar = if target_dir.join("fabric-server-launch.jar").exists() {
                target_dir.join("fabric-server-launch.jar")
            } else {
                src_root.join("fabric-server-launch.jar")
            };
            cmd.arg("-jar").arg(jar);
        } else if target_dir.join("server.jar").exists() || src_root.join("server.jar").exists() {
            let jar = if target_dir.join("server.jar").exists() {
                target_dir.join("server.jar")
            } else {
                src_root.join("server.jar")
            };
            cmd.arg("-jar").arg(jar);
        } else if target_dir.join("fabric-server-launch.jar").exists() || src_root.join("fabric-server-launch.jar").exists() {
            let jar = if target_dir.join("fabric-server-launch.jar").exists() {
                target_dir.join("fabric-server-launch.jar")
            } else {
                src_root.join("fabric-server-launch.jar")
            };
            cmd.arg("-jar").arg(jar);
        } else {
            return Err("No server binary found for headless session".to_string());
        }

        cmd.arg("nogui");
        cmd.stdin(std::process::Stdio::piped());
        cmd.stdout(std::process::Stdio::piped());
        cmd.stderr(std::process::Stdio::piped());

        tracing::info!("[WarmSession] Launching headless instance: {:?}", cmd);
        let mut child = cmd
            .spawn()
            .map_err(|e| format!("Failed to spawn headless preview process: {e}"))?;

        let stdin = child.stdin.take().ok_or_else(|| "Failed to capture stdin".to_string())?;
        let stdout = child.stdout.take().ok_or_else(|| "Failed to capture stdout".to_string())?;
        let stderr = child.stderr.take();

        // Drain stderr asynchronously
        if let Some(err_pipe) = stderr {
            tokio::spawn(async move {
                let mut reader = BufReader::new(err_pipe).lines();
                while let Ok(Some(line)) = reader.next_line().await {
                    tracing::info!("[warm preview stderr] {line}");
                }
            });
        }

        let instance_id = instance_dir
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("default")
            .to_string();

        let is_ready = Arc::new(AtomicBool::new(false));
        let is_ready_clone = is_ready.clone();

        let session = WarmPreviewSession {
            instance_id: instance_id.clone(),
            seed: requested_seed.to_string(),
            target_dir,
            port: worker_port,
            last_activity: Instant::now(),
            is_ready,
            is_busy: false,
            stdin: Arc::new(Mutex::new(stdin)),
            child: Arc::new(Mutex::new(child)),
        };

        // Asynchronously monitor stdout for boot completion and disable heavy gamerules
        let stdin_clone = session.stdin.clone();
        tokio::spawn(async move {
            let mut reader = BufReader::new(stdout).lines();
            let mut ready_signaled = false;
            while let Ok(Some(line)) = reader.next_line().await {
                tracing::info!("[warm preview stdout] {line}");
                if !ready_signaled && (line.contains("Done (") || line.contains("Done!") || line.contains("Done in ")) {
                    ready_signaled = true;
                    tracing::info!("[WarmSession] Instance '{}' warm server is READY for streaming!", instance_id);
                    is_ready_clone.store(true, Ordering::SeqCst);
                    let mut sin = stdin_clone.lock().await;
                    let _ = sin.write_all(b"gamerule doDaylightCycle false\ngamerule doWeatherCycle false\ngamerule randomTickSpeed 0\ngamerule doMobSpawning false\ngamerule doPatrolSpawning false\ngamerule doTraderSpawning false\ngamerule doEntityDrops false\n").await;
                    let _ = sin.flush().await;
                }
            }
            tracing::info!("[WarmSession] Instance '{}' warm server stdout closed.", instance_id);
        });

        Ok(session)
    }
}

/// Status payload returned to client endpoints.
#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WarmSessionStatus {
    pub is_ready: bool,
    pub is_busy: bool,
    pub port: u16,
}
