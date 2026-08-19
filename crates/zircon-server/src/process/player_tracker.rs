//! Derives the set of online players by parsing the vanilla server's console
//! messages ("X joined the game", "X left the game", "X lost connection: ...").
//! This is intentionally tolerant of log format changes: unmatched lines are
//! simply ignored.
//!
//! When constructed with a `players.json` path, it also maintains the
//! persistent "players who have ever joined" log: the file is loaded at
//! startup, each join appends/updates an entry (name, first/last seen, join
//! count), and the file is rewritten on every change so the log survives
//! restarts and is visible to the admin UI even while the server is offline.
//!
//! Port of `com.mcmanager.server.process.PlayerTracker` / `PlayerHistoryEntry`.

use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

const JOINED: &str = " joined the game";
const LEFT: &str = " left the game";
const LOST: &str = " lost connection:";
/// Vanilla's boot-complete line, e.g. `[Server thread/INFO]: Done (5.2s)! For help, type "help"`.
const DONE_MARKER: &str = "Done (";
const DONE_SUFFIX: &str = "For help, type";

/// One entry of the persistent "players who have ever joined" log.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PlayerHistoryEntry {
    pub name: String,
    /// Epoch millis of the first join ever observed.
    pub first_joined: i64,
    /// Epoch millis of the most recent join.
    pub last_joined: i64,
    /// Number of times the player has joined.
    pub join_count: i32,
}

/// Tracks online players and the ever-joined log from console lines.
pub struct PlayerTracker {
    online: Mutex<HashSet<String>>,
    /// players_file: nullable → no persistence (legacy single-server).
    players_file: Option<PathBuf>,
    history: Mutex<HashMap<String, PlayerHistoryEntry>>,
    /// Set once the server has finished booting (the "Done (...)" line). The
    /// idle-shutdown service uses it so its timer only starts once the server
    /// is actually joinable — a fresh boot must not count as "idle".
    ready: Mutex<bool>,
    /// Monotonic instant of boot completion (first "Done (...)" line).
    ready_at: Mutex<Option<Instant>>,
    /// Monotonic instant of the most recent activity: a join/leave/lost event
    /// or an external keep-alive (e.g. a launcher wakeup call while the server
    /// is already running). The idle window is measured from this (or
    /// `ready_at` if nobody has joined yet), so a session that falls entirely
    /// between two idle-service polls still resets the timer.
    last_activity_at: Mutex<Option<Instant>>,
}

impl PlayerTracker {
    pub fn new(players_file: Option<PathBuf>) -> Self {
        let history = match &players_file {
            Some(file) => {
                let mut map = HashMap::new();
                for entry in Self::load_history(file) {
                    if !entry.name.trim().is_empty() {
                        map.insert(entry.name.to_lowercase(), entry);
                    }
                }
                map
            }
            None => HashMap::new(),
        };
        Self {
            online: Mutex::new(HashSet::new()),
            players_file,
            history: Mutex::new(history),
            ready: Mutex::new(false),
            ready_at: Mutex::new(None),
            last_activity_at: Mutex::new(None),
        }
    }

    pub fn on_line(&self, line: &str) {
        if line.is_empty() {
            return;
        }

        // Only parse lines from the official Minecraft Server thread. Chat
        // lines arrive on that thread too, so they are further filtered below.
        let content = if let Some(idx) = line.find("[Server thread/INFO]: ") {
            &line[idx + 22..]
        } else if let Some(idx) = line.find("[Server thread/INFO] [minecraft/MinecraftServer]: ") {
            &line[idx + 50..]
        } else {
            return;
        };

        // Ignore in-game player chat messages (`<Steve> ...`) and bracket-led
        // lines (e.g. `/tellraw` output, advancement titles) — they must never
        // fake a join/leave event or the boot marker.
        if content.starts_with('<') || content.starts_with('[') {
            return;
        }

        // Boot completion detection
        if content.starts_with(DONE_MARKER) && content.contains(DONE_SUFFIX) {
            *self.ready.lock().unwrap() = true;
            let mut ready_at = self.ready_at.lock().unwrap();
            if ready_at.is_none() {
                *ready_at = Some(Instant::now());
            }
            return;
        }

        let mut name: Option<&str> = None;
        let mut remove = false;

        if let Some(idx) = content.find(JOINED) {
            name = Some(&content[..idx]);
        } else if let Some(idx) = content.find(LEFT) {
            name = Some(&content[..idx]);
            remove = true;
        } else if let Some(idx) = content.find(LOST) {
            name = Some(&content[..idx]);
            remove = true;
        }

        if let Some(raw_name) = name {
            let player = raw_name.trim();
            // Validate strictly against Minecraft username format so log
            // spoofing can never register an arbitrary string.
            if (1..=16).contains(&player.len())
                && player
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '_')
            {
                // Every join/leave/lost event is a moment of player activity.
                *self.last_activity_at.lock().unwrap() = Some(Instant::now());
                let mut online = self.online.lock().unwrap();
                if remove {
                    online.remove(player);
                } else {
                    online.insert(player.to_string());
                    self.record_join(player);
                }
            }
        }
    }

    pub fn get_online_players(&self) -> Vec<String> {
        let mut players: Vec<String> = self.online.lock().unwrap().iter().cloned().collect();
        players.sort();
        players
    }

    pub fn online_player_count(&self) -> usize {
        self.online.lock().unwrap().len()
    }

    /// The ever-joined log, most recently active players first.
    pub fn get_history(&self) -> Vec<PlayerHistoryEntry> {
        let history = self.history.lock().unwrap();
        let mut entries: Vec<PlayerHistoryEntry> = history.values().cloned().collect();
        entries.sort_by(|a, b| b.last_joined.cmp(&a.last_joined));
        entries
    }

    /// Whether the server has printed its boot-complete "Done (...)" line.
    /// Starts false on construction (each instance rebuilds its tracker on
    /// start) and stays true for the lifetime of the process.
    pub fn is_ready(&self) -> bool {
        *self.ready.lock().unwrap()
    }

    /// The instant from which idle time should be measured: the most recent
    /// activity (join/leave/lost event or keep-alive), or boot completion for
    /// a server nobody has played on yet. `None` before the server has
    /// finished booting.
    pub fn idle_reference(&self) -> Option<Instant> {
        let ready_at = *self.ready_at.lock().unwrap();
        let last_activity = *self.last_activity_at.lock().unwrap();
        match (ready_at, last_activity) {
            (Some(ready), Some(activity)) => Some(activity.max(ready)),
            (Some(ready), None) => Some(ready),
            (None, activity) => activity,
        }
    }

    /// Treats an external keep-alive (e.g. a launcher wakeup call while the
    /// server is already running) as activity: moves the idle reference
    /// forward so an idle shutdown is deferred — "kicking the can down the
    /// road" without disabling the feature.
    pub fn touch_activity(&self) {
        *self.last_activity_at.lock().unwrap() = Some(Instant::now());
    }

    /// Loads a persisted ever-joined log, tolerating a missing or corrupt file.
    pub fn load_history(players_file: &PathBuf) -> Vec<PlayerHistoryEntry> {
        if !players_file.is_file() {
            return Vec::new();
        }
        match fs::read_to_string(players_file)
            .map_err(|e| std::io::Error::other(e.to_string()))
            .and_then(|json| {
                serde_json::from_str::<Vec<PlayerHistoryEntry>>(&json)
                    .map_err(|e| std::io::Error::other(e.to_string()))
            }) {
            Ok(parsed) => parsed,
            Err(e) => {
                tracing::warn!(
                    "Could not read player history {}, starting empty: {e}",
                    players_file.display()
                );
                Vec::new()
            }
        }
    }

    /// Upserts the ever-joined entry for a player and persists the log.
    fn record_join(&self, name: &str) {
        let Some(players_file) = &self.players_file else {
            return; // no persistence configured → don't accumulate history
        };
        let now = now_millis();
        {
            let mut history = self.history.lock().unwrap();
            let key = name.to_lowercase();
            let entry = history.entry(key).or_insert_with(|| PlayerHistoryEntry {
                name: name.to_string(),
                first_joined: now,
                last_joined: 0,
                join_count: 0,
            });
            entry.name = name.to_string();
            if entry.last_joined == 0 {
                entry.first_joined = now;
            }
            entry.last_joined = now;
            entry.join_count += 1;
        }
        self.save_history(players_file);
    }

    fn save_history(&self, players_file: &PathBuf) {
        let history = self.history.lock().unwrap();
        let mut entries: Vec<PlayerHistoryEntry> = history.values().cloned().collect();
        entries.sort_by(|a, b| b.last_joined.cmp(&a.last_joined));
        match serde_json::to_string_pretty(&entries)
            .map_err(|e| std::io::Error::other(e.to_string()))
            .and_then(|json| fs::write(players_file, json))
        {
            Ok(()) => {}
            Err(e) => {
                tracing::warn!(
                    "Could not persist player history to {}: {e}",
                    players_file.display()
                );
            }
        }
    }
}

fn now_millis() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir() -> PathBuf {
        crate::test_util::temp_dir("players")
    }

    #[test]
    fn tracks_online_players_with_vanilla_prefixes() {
        let tracker = PlayerTracker::new(None);
        tracker.on_line("[Server thread/INFO]: Steve joined the game");
        tracker.on_line("[Server thread/INFO]: Alex joined the game");
        assert_eq!(2, tracker.online_player_count());

        tracker.on_line("[Server thread/INFO]: Steve left the game");
        assert_eq!(1, tracker.online_player_count());
        assert!(tracker.get_online_players().contains(&"Alex".to_string()));

        tracker.on_line("[Server thread/INFO]: Alex lost connection: disconnected");
        assert_eq!(0, tracker.online_player_count());
    }

    #[test]
    fn unmatched_lines_are_ignored() {
        let tracker = PlayerTracker::new(None);
        tracker.on_line("[Server thread/INFO]: Done (5.2s)! For help, type \"help\"");
        tracker.on_line("some random output");
        assert_eq!(0, tracker.online_player_count());
    }

    #[test]
    fn ready_flag_tracks_boot_completion() {
        let tracker = PlayerTracker::new(None);
        assert!(!tracker.is_ready());
        tracker.on_line("[Server thread/INFO]: Preparing level \"world\"");
        assert!(!tracker.is_ready());
        tracker.on_line("[Server thread/INFO]: Done (7.3s)! For help, type \"help\"");
        assert!(tracker.is_ready());
        // A fresh tracker (new server boot) starts unready again.
        assert!(!PlayerTracker::new(None).is_ready());
    }

    #[test]
    fn touch_activity_defers_the_idle_reference() {
        let tracker = PlayerTracker::new(None);
        tracker.on_line("[Server thread/INFO]: Done (5.2s)! For help, type \"help\"");
        let after_boot = tracker.idle_reference().expect("boot reference");

        // A keep-alive (e.g. wakeup while running) moves the reference forward,
        // deferring any idle shutdown without disabling it.
        tracker.touch_activity();
        let after_keepalive = tracker.idle_reference().expect("keepalive reference");
        assert!(after_keepalive >= after_boot);
    }

    #[test]
    fn idle_reference_anchors_to_boot_then_player_events() {
        let tracker = PlayerTracker::new(None);
        // Nothing observed yet — no idle reference.
        assert!(tracker.idle_reference().is_none());

        // Boot completion becomes the reference for a server nobody joined.
        tracker.on_line("[Server thread/INFO]: Done (5.2s)! For help, type \"help\"");
        let after_boot = tracker.idle_reference().expect("boot reference");

        // A join moves the reference forward (session started).
        tracker.on_line("[Server thread/INFO]: Steve joined the game");
        let after_join = tracker.idle_reference().expect("join reference");
        assert!(after_join >= after_boot);

        // A leave moves it forward again — this is the moment the idle window
        // starts, even if no poll runs at that instant.
        tracker.on_line("[Server thread/INFO]: Steve left the game");
        let after_leave = tracker.idle_reference().expect("leave reference");
        assert!(after_leave >= after_join);
    }

    #[test]
    fn history_is_persisted() {
        let dir = temp_dir();
        let players_file = dir.join("players.json");
        let tracker = PlayerTracker::new(Some(players_file.clone()));
        tracker.on_line("[Server thread/INFO]: Steve joined the game");
        tracker.on_line("[Server thread/INFO]: Steve joined the game");
        tracker.on_line("[Server thread/INFO]: Alex joined the game");

        let history = PlayerTracker::load_history(&players_file);
        assert_eq!(2, history.len());
        let steve = history.iter().find(|e| e.name == "Steve").unwrap();
        assert_eq!(2, steve.join_count);
        let alex = history.iter().find(|e| e.name == "Alex").unwrap();
        assert_eq!(1, alex.join_count);
        // Most recently active first.
        assert!(history[0].name == "Alex" || history[0].name == "Steve");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn chat_lines_cannot_fake_ready_or_join_events() {
        let tracker = PlayerTracker::new(None);
        // A chat message embedding the boot marker must not flip `ready`.
        tracker.on_line("<Attacker> [Server thread/INFO]: Done (1s)");
        assert!(!tracker.is_ready());
        assert_eq!(0, tracker.online_player_count());

        // A chat message whose content starts with '<' is ignored entirely,
        // even when it contains a fake join marker.
        tracker.on_line("[Server thread/INFO]: <Attacker> Steve joined the game");
        assert_eq!(0, tracker.online_player_count());

        // Bracket-led lines (e.g. /tellraw output) are ignored too.
        tracker.on_line("[Server thread/INFO]: [{\"text\":\"Alex joined the game\"}]");
        assert_eq!(0, tracker.online_player_count());
        assert!(!tracker.is_ready());
    }

    #[test]
    fn non_server_thread_lines_are_ignored() {
        let tracker = PlayerTracker::new(None);
        tracker.on_line("[User Authenticator #1/INFO]: Steve joined the game");
        tracker.on_line("[Netty Server IO #3/INFO]: Alex left the game");
        assert_eq!(0, tracker.online_player_count());
        assert!(!tracker.is_ready());
    }

    #[test]
    fn invalid_usernames_are_rejected() {
        let tracker = PlayerTracker::new(None);
        // Too long for a Minecraft username.
        tracker.on_line("[Server thread/INFO]: VeryLongUsernameExceeds16 joined the game");
        // Contains spaces — not a valid username.
        tracker.on_line("[Server thread/INFO]: bad name with spaces joined the game");
        assert_eq!(0, tracker.online_player_count());

        // A legitimately formatted name still tracks normally.
        tracker.on_line("[Server thread/INFO]: Notch joined the game");
        assert_eq!(1, tracker.online_player_count());
        assert!(tracker.get_online_players().contains(&"Notch".to_string()));
    }
}
