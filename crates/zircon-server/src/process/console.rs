//! Fan-out point for Minecraft server console output. Every line printed by
//! the server process is broadcast to registered subscribers (WebSocket
//! sessions) and kept in a small ring buffer so late-joining clients see
//! recent history.
//!
//! Port of `com.mcmanager.server.process.ConsoleStreamHandler`.

use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use tokio::sync::broadcast;

use super::player_tracker::PlayerTracker;
use super::tps_tracker::TpsTracker;

pub const HISTORY_SIZE: usize = 1000;

/// Log levels the console can filter history by.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogLevel {
    Warn,
    Error,
}

/// Legacy synchronous listener receiving every console line (e.g. instance
/// console → shared console).
type ConsoleListener = Box<dyn Fn(String) + Send + Sync>;

/// Fan-out point for Minecraft server console output.
pub struct ConsoleStreamHandler {
    tx: broadcast::Sender<String>,
    history: Mutex<VecDeque<String>>,
    /// Legacy synchronous listeners (e.g. instance console → shared console).
    sync_listeners: Mutex<Vec<ConsoleListener>>,
    player_tracker: Arc<PlayerTracker>,
    tps_tracker: Arc<TpsTracker>,
}

impl ConsoleStreamHandler {
    /// No persistence: legacy single-server wiring.
    pub fn new() -> Self {
        Self::with_players_file(None)
    }

    /// `players_file`: optional path for the ever-joined player log.
    pub fn with_players_file(players_file: Option<PathBuf>) -> Self {
        let (tx, _) = broadcast::channel(512);
        Self {
            tx,
            history: Mutex::new(VecDeque::with_capacity(HISTORY_SIZE)),
            sync_listeners: Mutex::new(Vec::new()),
            player_tracker: Arc::new(PlayerTracker::new(players_file)),
            tps_tracker: Arc::new(TpsTracker::new()),
        }
    }

    /// Subscribes to live console lines.
    pub fn subscribe(&self) -> broadcast::Receiver<String> {
        self.tx.subscribe()
    }

    /// Registers a synchronous listener receiving every console line.
    pub fn add_listener(&self, listener: ConsoleListener) {
        self.sync_listeners.lock().unwrap().push(listener);
    }

    /// Feeds a raw console line into the tracker and fans it out.
    pub fn accept(&self, line: String) {
        self.player_tracker.on_line(&line);
        let was_telemetry = self.tps_tracker.on_line(&line);
        let is_probe_noise = self.tps_tracker.is_telemetry_probe_line(&line);

        // Suppress background probe echoes and telemetry responses from spamming user console
        if was_telemetry || is_probe_noise {
            return;
        }

        {
            let mut history = self.history.lock().unwrap();
            history.push_back(line.clone());
            while history.len() > HISTORY_SIZE {
                history.pop_front();
            }
        }

        let _ = self.tx.send(line.clone());
        let listeners = self.sync_listeners.lock().unwrap();
        for listener in listeners.iter() {
            listener(line.clone());
        }
    }

    /// Returns the most recent lines, oldest first.
    pub fn recent_history(&self, max_lines: usize) -> Vec<String> {
        let history = self.history.lock().unwrap();
        history
            .iter()
            .rev()
            .take(max_lines)
            .rev()
            .cloned()
            .collect()
    }

    /// Returns the most recent lines matching the provided filter, oldest first.
    pub fn recent_filtered_history(
        &self,
        max_lines: usize,
        filter: impl Fn(&str) -> bool,
    ) -> Vec<String> {
        let history = self.history.lock().unwrap();
        let mut matched: Vec<String> = history.iter().filter(|l| filter(l)).cloned().collect();
        if matched.len() > max_lines {
            matched.drain(..matched.len() - max_lines);
        }
        matched
    }

    /// Retrieves the most recent console lines that match ANY of the specified
    /// log levels. If no levels are provided, returns all lines.
    pub fn recent_history_by_level(&self, max_lines: usize, levels: &[LogLevel]) -> Vec<String> {
        if levels.is_empty() {
            return self.recent_history(max_lines);
        }
        self.recent_filtered_history(max_lines, |line| {
            let upper = line.to_uppercase();
            if levels.contains(&LogLevel::Error)
                && (upper.contains("ERROR")
                    || upper.contains("EXCEPTION")
                    || upper.starts_with("\tAT ")
                    || upper.starts_with("CAUSED BY: "))
            {
                return true;
            }
            if levels.contains(&LogLevel::Warn)
                && (upper.contains("WARN") || upper.contains("WARNING"))
            {
                return true;
            }
            false
        })
    }

    /// Clears the console history.
    pub fn clear_history(&self) {
        self.history.lock().unwrap().clear();
    }

    pub fn player_tracker(&self) -> &PlayerTracker {
        &self.player_tracker
    }

    /// Cloned handle to the shared player tracker.
    pub fn player_tracker_arc(&self) -> Arc<PlayerTracker> {
        self.player_tracker.clone()
    }

    pub fn tps_tracker(&self) -> &TpsTracker {
        &self.tps_tracker
    }

    /// Cloned handle to the shared TPS & MSPT telemetry tracker.
    pub fn tps_tracker_arc(&self) -> Arc<TpsTracker> {
        self.tps_tracker.clone()
    }
}

impl Default for ConsoleStreamHandler {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ring_buffer_keeps_most_recent_lines() {
        let console = ConsoleStreamHandler::new();
        for i in 0..2500 {
            console.accept(format!("line {i}"));
        }
        let recent = console.recent_history(5);
        assert_eq!(
            vec![
                "line 2495",
                "line 2496",
                "line 2497",
                "line 2498",
                "line 2499"
            ],
            recent
        );
        assert_eq!(HISTORY_SIZE, console.history.lock().unwrap().len());
    }

    #[test]
    fn clear_empties_history() {
        let console = ConsoleStreamHandler::new();
        console.accept("hello".to_string());
        console.clear_history();
        assert!(console.recent_history(10).is_empty());
    }

    #[test]
    fn level_filtering() {
        let console = ConsoleStreamHandler::new();
        console.accept("[INFO] Server started".to_string());
        console.accept("[WARN] Something suspicious".to_string());
        console.accept("ERROR: boom".to_string());
        console.accept("java.lang.Exception at com.example.Thing".to_string());

        let errors = console.recent_history_by_level(10, &[LogLevel::Error]);
        assert_eq!(2, errors.len());
        assert!(errors[0].contains("ERROR: boom"));
        assert!(errors[1].contains("Exception"));

        let warns = console.recent_history_by_level(10, &[LogLevel::Warn]);
        assert_eq!(1, warns.len());
        assert!(warns[0].contains("[WARN]"));

        // No levels → everything.
        assert_eq!(4, console.recent_history_by_level(10, &[]).len());
    }

    #[test]
    fn broadcast_subscribers_receive_lines() {
        let console = ConsoleStreamHandler::new();
        let mut rx = console.subscribe();
        console.accept("hello broadcast".to_string());
        assert_eq!("hello broadcast", rx.try_recv().unwrap());
    }
}
