//! Per-instance idle shutdown: when enabled, an instance that has had no
//! players online for its configured window is shut down gracefully, freeing
//! CPU/RAM while nobody is playing. The launcher wakes it back up (POST
//! /api/wakeup) when a player wants to join.
//!
//! Idle time is measured **event-driven** from the tracker's last join/leave
//! event (or boot completion for a server nobody has joined yet) rather than
//! from polls observing an empty server. A short session that falls entirely
//! between two polls must still reset the timer — otherwise the window could
//! expire mid-session and shut the server down under a playing player.
//!
//! The service only polls to *check* the elapsed window; the window itself is
//! anchored to real player events (`PlayerTracker::idle_reference`).
//!
//! Port of the concept from the Java codebase's scheduled services
//! (`BackupSchedulerService` is the structural sibling).

use std::sync::Arc;
use std::time::{Duration, Instant};

use zircon_core::model::{clamp_idle_shutdown_minutes, SHUTDOWN_REASON_IDLE};

use crate::instance::ServerInstanceManager;

/// How often the service wakes up to check for idle instances.
pub const POLL_INTERVAL_SECONDS: u64 = 30;

/// Whether an idle shutdown is due: the instance is up and joinable, no
/// players are online, no launcher has a fresh join intent (a player is on
/// their way), and the window has elapsed since the reference instant (last
/// player event, or boot completion). Kept pure so it is unit-testable.
pub fn idle_due(
    running: bool,
    ready: bool,
    players: usize,
    idle_since: Option<Instant>,
    window: Duration,
    now: Instant,
    pending_join_intent: bool,
) -> bool {
    if pending_join_intent || !running || !ready || players > 0 {
        return false;
    }
    match idle_since {
        Some(since) => now.duration_since(since) >= window,
        None => false,
    }
}

/// Runs idle shutdowns for every instance with the feature enabled.
#[derive(Clone)]
pub struct IdleShutdownService {
    instance_manager: Arc<ServerInstanceManager>,
}

impl IdleShutdownService {
    pub fn new(instance_manager: Arc<ServerInstanceManager>) -> Self {
        Self { instance_manager }
    }

    /// Starts the background idle-check loop.
    pub fn start(&self) -> tokio::task::JoinHandle<()> {
        let this = self.clone();
        tracing::info!("Idle shutdown service started: checks every {POLL_INTERVAL_SECONDS}s");
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(Duration::from_secs(POLL_INTERVAL_SECONDS)).await;
                this.check_idle_shutdowns().await;
            }
        })
    }

    /// Runs one idle-check pass over all instances. Exposed for tests so they
    /// can drive it without waiting for the real poll delay.
    pub async fn check_idle_shutdowns(&self) {
        let instances = self.instance_manager.list_instances();
        let now = Instant::now();
        for config in instances {
            if !config.idle_shutdown_enabled {
                continue;
            }
            let id = config.id.clone();
            let window = Duration::from_secs(
                u64::from(clamp_idle_shutdown_minutes(config.idle_shutdown_minutes)) * 60,
            );
            let due = idle_due(
                self.instance_manager.is_running(&id),
                self.instance_manager.is_server_ready(&id),
                self.instance_manager.get_online_player_count(&id),
                self.instance_manager.idle_since(&id),
                window,
                now,
                self.instance_manager.has_pending_join_intent(&id),
            );
            if !due {
                continue;
            }
            // Re-check right before stopping: a join intent that lands between
            // the poll and the stop must not race the shutdown and kill the
            // server under a player who is about to connect.
            if self.instance_manager.has_pending_join_intent(&id) {
                continue;
            }
            tracing::info!(
                "Instance '{}' had no players for {} minutes — shutting down (idle)",
                config.name,
                config.idle_shutdown_minutes
            );
            self.instance_manager.console().accept(format!(
                "[wrapper] No players for {} minutes — shutting down (idle)",
                config.idle_shutdown_minutes
            ));
            self.instance_manager
                .stop_instance_with_reason(&id, Some(SHUTDOWN_REASON_IDLE))
                .await;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn idle_window_starts_only_when_ready_and_running() {
        let t0 = Instant::now();
        let window = Duration::from_secs(5 * 60);

        // Booting (not ready) or down — never idle, even past the window.
        assert!(!idle_due(
            true,
            false,
            0,
            Some(t0),
            window,
            t0 + window,
            false
        ));
        assert!(!idle_due(
            false,
            false,
            0,
            Some(t0),
            window,
            t0 + window,
            false
        ));
        assert!(!idle_due(
            false,
            true,
            0,
            Some(t0),
            window,
            t0 + window,
            false
        ));

        // Ready, empty, reference set → due only after the window.
        assert!(!idle_due(
            true,
            true,
            0,
            Some(t0),
            window,
            t0 + Duration::from_secs(60),
            false
        ));
        assert!(idle_due(
            true,
            true,
            0,
            Some(t0),
            window,
            t0 + window + Duration::from_secs(1),
            false
        ));
    }

    #[test]
    fn players_online_never_due() {
        let t0 = Instant::now();
        let window = Duration::from_secs(5 * 60);
        // Even far past the window, an online player keeps the server up.
        assert!(!idle_due(
            true,
            true,
            1,
            Some(t0),
            window,
            t0 + window + Duration::from_secs(600),
            false
        ));
    }

    #[test]
    fn no_reference_means_not_due() {
        // No boot-complete / player event observed yet (tracker reference
        // still None) — the window cannot be evaluated.
        let t0 = Instant::now();
        assert!(!idle_due(
            true,
            true,
            0,
            None,
            Duration::from_secs(5 * 60),
            t0 + Duration::from_secs(3600),
            false
        ));
    }

    #[test]
    fn window_uses_the_configured_minutes() {
        let t0 = Instant::now();
        let window = Duration::from_secs(5 * 60);
        assert!(!idle_due(
            true,
            true,
            0,
            Some(t0),
            window,
            t0 + Duration::from_secs(299),
            false
        ));
        assert!(idle_due(
            true,
            true,
            0,
            Some(t0),
            window,
            t0 + Duration::from_secs(301),
            false
        ));
    }

    #[test]
    fn pending_join_intent_holds_off_shutdown() {
        let t0 = Instant::now();
        let window = Duration::from_secs(5 * 60);
        // A launcher is on its way: even far past the window with no players,
        // the instance must not be shut down.
        assert!(!idle_due(
            true,
            true,
            0,
            Some(t0),
            window,
            t0 + window + Duration::from_secs(600),
            true
        ));
        // Booting or down with a pending intent is also never due.
        assert!(!idle_due(
            false,
            true,
            0,
            Some(t0),
            window,
            t0 + window,
            true
        ));
        assert!(!idle_due(
            true,
            false,
            0,
            Some(t0),
            window,
            t0 + window,
            true
        ));
    }
}
