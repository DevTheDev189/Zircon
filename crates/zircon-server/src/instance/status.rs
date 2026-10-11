use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use zircon_core::model::{clamp_idle_shutdown_minutes, InstanceConfig};

use crate::process::console::ConsoleStreamHandler;
use crate::process::manager::MinecraftProcessManager;

use super::*;

impl ServerInstanceManager {
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

    /// Polls an internal port on loopback until it accepts connections or `timeout` expires.
    pub async fn poll_port_ready(&self, port: u16, timeout: Duration) -> bool {
        let start = Instant::now();
        let poll_interval = Duration::from_millis(250);
        while start.elapsed() < timeout {
            if let Ok(Ok(stream)) = tokio::time::timeout(
                Duration::from_millis(500),
                tokio::net::TcpStream::connect(("127.0.0.1", port)),
            )
            .await
            {
                drop(stream);
                return true;
            }
            tokio::time::sleep(poll_interval).await;
        }
        false
    }

    /// Returns session information for the last run of the instance.
    pub fn get_last_session(&self, instance_id: &str) -> Option<InstanceRunSession> {
        let inner = self.inner.lock().unwrap();
        if let Some(pm) = inner.active_processes.get(instance_id) {
            let running = pm.is_running();
            return Some(InstanceRunSession {
                start_time: pm.start_time().unwrap_or_else(std::time::SystemTime::now),
                stop_requested: pm.stop_requested(),
                exit_code: pm.exit_code(),
                is_running: running,
            });
        }
        inner.last_sessions.get(instance_id).cloned()
    }

    /// Returns the shutdown reason if this instance is actively stopping/spinning down.
    pub fn stopping_reason(&self, instance_id: &str) -> Option<String> {
        self.inner
            .lock()
            .unwrap()
            .stopping_instances
            .get(instance_id)
            .cloned()
    }

    /// Whether this instance is actively stopping/spinning down.
    pub fn is_stopping(&self, instance_id: &str) -> bool {
        self.inner
            .lock()
            .unwrap()
            .stopping_instances
            .contains_key(instance_id)
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

    /// Returns the active instance ID if one is selected/running.
    pub fn active_instance_id(&self) -> Option<String> {
        self.inner.lock().unwrap().active_instance_id.clone()
    }

    /// Returns the TPS tracker for the currently active instance (or fallback to shared console tracker).
    pub fn active_tps_tracker(&self) -> Arc<crate::process::tps_tracker::TpsTracker> {
        let inner = self.inner.lock().unwrap();
        if let Some(active_id) = &inner.active_instance_id {
            if let Some(console) = inner.consoles.get(active_id) {
                return console.tps_tracker_arc();
            }
        }
        self.console.tps_tracker_arc()
    }

    /// Returns the TPS tracker for a specific instance by ID.
    pub fn get_tps_tracker(&self, instance_id: &str) -> Option<Arc<crate::process::tps_tracker::TpsTracker>> {
        let inner = self.inner.lock().unwrap();
        inner.consoles.get(instance_id).map(|c| c.tps_tracker_arc())
    }

    /// Returns live telemetry summaries across all registered server instances.
    pub fn all_instances_telemetry(&self) -> HashMap<String, InstanceTelemetrySummary> {
        let inner = self.inner.lock().unwrap();
        let mut result = HashMap::new();
        for (id, cfg) in &inner.instance_configs {
            let running = inner
                .active_processes
                .get(id)
                .map(|p| p.is_running())
                .unwrap_or(false);
            let player_count = inner
                .player_trackers
                .get(id)
                .map(|t| t.online_player_count())
                .unwrap_or(0);
            let (tps, mspt, ping) = inner
                .consoles
                .get(id)
                .map(|c| c.tps_tracker().current_metrics())
                .unwrap_or((None, None, None));
            result.insert(
                id.clone(),
                InstanceTelemetrySummary {
                    id: id.clone(),
                    name: cfg.name.clone(),
                    running,
                    tps: if running { tps } else { None },
                    mspt: if running { mspt } else { None },
                    ping_latency_ms: if running { ping } else { None },
                    player_count,
                },
            );
        }
        result
    }

    /// Whether a stopped instance may be auto-started by a launcher wakeup:
    /// true only when the idle-shutdown service put it to sleep. Manual stops
    /// (admin, restart, daemon shutdown) stay down until started manually.
    pub fn wakeable(&self, instance_id: &str) -> bool {
        self.get_instance(instance_id)
            .map(|cfg| cfg.wakeable())
            .unwrap_or(false)
    }

    /// Marks `instance_id` as waking. Returns `true` only when this call won
    /// the race (the instance was not already waking), so duplicate concurrent
    /// wakeup requests for the same instance are discarded instead of spawning
    /// parallel start attempts.
    pub fn mark_waking(&self, instance_id: &str) -> bool {
        self.in_progress_wakes.insert(instance_id.to_string())
    }

    /// Clears the waking marker once the start attempt completes (success or
    /// failure) so a later wakeup request can try again.
    pub fn unmark_waking(&self, instance_id: &str) {
        self.in_progress_wakes.remove(instance_id);
    }

    /// Returns `true` when a background wakeup task is actively starting `instance_id`.
    pub fn is_waking(&self, instance_id: &str) -> bool {
        self.in_progress_wakes.contains(instance_id)
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

    /// Seconds left before an idle shutdown fires for `instance_id`, or `None`
    /// when idle shutdown is disabled, the server is not up/joinable, a player
    /// is online, or a launcher has a fresh join intent. Mirrors the
    /// `IdleShutdownService` timing so the admin UI can show a live countdown
    /// (e.g. "Sleeps in 3m 42s"). `idle_since` is only `Some` once the server
    /// is ready, which covers the `is_server_ready` gate.
    pub fn get_idle_remaining_seconds(&self, instance_id: &str) -> Option<u64> {
        if self.is_stopping(instance_id) {
            return None;
        }
        let config = self.get_instance(instance_id).ok()?;
        if !config.idle_shutdown_enabled
            || !self.is_running(instance_id)
            || !self.is_server_ready(instance_id)
        {
            return None;
        }
        if self.get_online_player_count(instance_id) > 0
            || self.has_pending_join_intent(instance_id)
        {
            return None;
        }
        let idle_since = self.idle_since(instance_id)?;
        let window_secs = u64::from(clamp_idle_shutdown_minutes(config.idle_shutdown_minutes)) * 60;
        Some(window_secs.saturating_sub(idle_since.elapsed().as_secs()))
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

    /// Checks whether `username` is allowed by the instance's `whitelist.json`,
    /// or returns true if no whitelist file is present.
    pub fn is_username_whitelisted(&self, instance_id: &str, username: &str) -> bool {
        let instance_dir = self.get_instance_dir(instance_id);
        let file_path = if instance_dir.join("server/whitelist.json").is_file() {
            instance_dir.join("server/whitelist.json")
        } else if instance_dir.join("whitelist.json").is_file() {
            instance_dir.join("whitelist.json")
        } else {
            return true;
        };

        if let Ok(content) = std::fs::read_to_string(file_path) {
            zircon_core::model::is_username_in_whitelist(&content, username)
        } else {
            true
        }
    }

    pub fn get_process_manager(&self, instance_id: &str) -> Option<Arc<MinecraftProcessManager>> {
        self.inner
            .lock()
            .unwrap()
            .active_processes
            .get(instance_id)
            .cloned()
    }

    /// Gets or creates the isolated console stream handler for a specific instance.
    pub fn get_or_create_console(
        &self,
        instance_id: &str,
    ) -> Result<Arc<ConsoleStreamHandler>, InstanceError> {
        let mut inner = self.inner.lock().unwrap();
        if !inner.instance_configs.contains_key(instance_id) {
            return Err(InstanceError::NotFound(format!(
                "Instance '{instance_id}' not found"
            )));
        }
        if let Some(console) = inner.consoles.get(instance_id) {
            return Ok(console.clone());
        }
        let inst_console = Arc::new(ConsoleStreamHandler::with_players_file(Some(
            self.instance_dir(instance_id).join("players.json"),
        )));
        inner
            .player_trackers
            .insert(instance_id.to_string(), inst_console.player_tracker_arc());
        inner
            .consoles
            .insert(instance_id.to_string(), inst_console.clone());
        Ok(inst_console)
    }

    /// The shared console used for wrapper-level messages or single-instance legacy mode.
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

    pub fn get_boot_progress(&self, instance_id: &str) -> Option<(u32, u32, f32)> {
        let entry = self.boot_progress_tracker.get(instance_id)?;
        let (started_at, total_est_secs) = *entry.value();
        let elapsed = started_at.elapsed().as_secs() as u32;
        let linear = (elapsed as f32 / total_est_secs as f32).min(0.90);
        let fraction = if elapsed > total_est_secs {
            let overtime = (elapsed - total_est_secs) as f32;
            0.90 + 0.08 * (1.0 - (-overtime / 20.0).exp())
        } else {
            linear
        };
        Some((elapsed, total_est_secs, fraction))
    }

    /// Marks the boot phase complete for an instance (e.g. after internal port binds).
    pub fn mark_boot_complete(&self, instance_id: &str) {
        self.boot_progress_tracker.remove(instance_id);
    }

    /// Resolves the instance whose id or (normalized) name matches a handshake
    /// hostname, including subdomain prefixes (e.g. "emerald" for "emerald.zirconmc.net").
    pub fn find_by_hostname(&self, hostname: &str) -> Option<InstanceConfig> {
        let clean = hostname.split('\0').next().unwrap_or("").trim().to_lowercase();
        if clean.is_empty() {
            return None;
        }
        let subdomain = clean.split('.').next().unwrap_or(&clean);

        let inner = self.inner.lock().unwrap();
        inner
            .instance_configs
            .values()
            .find(|cfg| {
                if let Some(sub) = &cfg.subdomain {
                    let sub_lower = sub.to_lowercase();
                    if sub_lower == clean || sub_lower == subdomain {
                        return true;
                    }
                }
                if let Some(addr) = &cfg.public_address {
                    let addr_clean = addr.split(':').next().unwrap_or("").trim().to_lowercase();
                    if addr_clean == clean || addr_clean == subdomain {
                        return true;
                    }
                }
                let id = cfg.id.to_lowercase();
                let norm_name = normalize_name(&cfg.name);
                id == clean || norm_name == clean || id == subdomain || norm_name == subdomain
            })
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


}
