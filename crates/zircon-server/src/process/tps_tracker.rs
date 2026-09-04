//! Real-time TPS and MSPT telemetry parsing and tracking.
//!
//! Captures server performance telemetry from standard server commands (`tick query`,
//! `/forge tps`, `/spark tps`) and status pings across Vanilla, Forge, NeoForge,
//! Fabric, and Spark.

use std::collections::VecDeque;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use regex::Regex;

pub const HISTORY_LIMIT: usize = 60;

/// Telemetry state holding current tick metrics and rolling history.
#[derive(Debug)]
struct TrackerState {
    tps: Option<f64>,
    mspt: Option<f64>,
    ping_latency_ms: Option<u64>,
    last_updated: Option<Instant>,
    tps_history: VecDeque<f64>,
    mspt_history: VecDeque<f64>,
}

/// Tracks real-time TPS & MSPT by inspecting console output lines and ping probes.
pub struct TpsTracker {
    state: Mutex<TrackerState>,
    active_monitoring_until: Mutex<Option<Instant>>,
    vanilla_tick_regex: Regex,
    vanilla_avg_regex: Regex,
    forge_tps_regex: Regex,
    spark_tps_regex: Regex,
    spark_mspt_regex: Regex,
}

impl TpsTracker {
    pub fn new() -> Self {
        Self {
            state: Mutex::new(TrackerState {
                tps: None,
                mspt: None,
                ping_latency_ms: None,
                last_updated: None,
                tps_history: VecDeque::with_capacity(HISTORY_LIMIT),
                mspt_history: VecDeque::with_capacity(HISTORY_LIMIT),
            }),
            active_monitoring_until: Mutex::new(None),
            // Matches: "The tick rate is 20.0 ticks per second (average tick time: 24.3ms)"
            // or "The tick rate is 20.0 ticks per second (average tick time: 24.3 ms)"
            vanilla_tick_regex: Regex::new(
                r"(?i)tick rate is\s+(?P<tps>[\d\.]+)\s+ticks per second(?:\s*\(average tick time:\s*(?P<mspt>[\d\.]+)\s*ms\))?",
            ).expect("invalid vanilla tick regex"),
            // Matches: "average tick time: 18.5ms"
            vanilla_avg_regex: Regex::new(
                r"(?i)average tick time:\s*(?P<mspt>[\d\.]+)\s*ms",
            ).expect("invalid vanilla avg regex"),
            // Matches Forge/NeoForge: "Mean tick time: 12.3 ms. Mean TPS: 20.0"
            // or "Dim 0 : Mean tick time: 15.2 ms. Mean TPS: 20.000"
            forge_tps_regex: Regex::new(
                r"(?i)Mean tick time:\s*(?P<mspt>[\d\.]+)\s*ms\.\s*Mean TPS:\s*(?P<tps>[\d\.]+)",
            ).expect("invalid forge tps regex"),
            // Matches Spark/Paper/Spigot TPS: "TPS from last 5s, 10s, 1m, 5m, 15m: 20.0, 20.0, 20.0"
            // or "TPS from last 1m: 19.98" or "TPS: 20.0"
            spark_tps_regex: Regex::new(
                r"(?i)TPS(?: from last [^:]*)?:\s*\*?(?P<tps>[\d\.]+)",
            ).expect("invalid spark tps regex"),
            // Matches Spark MSPT: "MSPT from last 5s, 10s, 1m: 15.2, 14.8"
            // or "MSPT: 16.1"
            spark_mspt_regex: Regex::new(
                r"(?i)MSPT(?: from last [^:]*)?:\s*\*?(?P<mspt>[\d\.]+)",
            ).expect("invalid spark mspt regex"),
        }
    }

    /// Feeds one console line into the parser. Returns true if telemetry was matched.
    pub fn on_line(&self, line: &str) -> bool {
        let mut matched = false;
        let mut parsed_tps: Option<f64> = None;
        let mut parsed_mspt: Option<f64> = None;

        if let Some(caps) = self.vanilla_tick_regex.captures(line) {
            if let Some(m) = caps.name("tps") {
                if let Ok(val) = m.as_str().parse::<f64>() {
                    parsed_tps = Some(val.clamp(0.0, 20.0));
                    matched = true;
                }
            }
            if let Some(m) = caps.name("mspt") {
                if let Ok(val) = m.as_str().parse::<f64>() {
                    parsed_mspt = Some(val.max(0.0));
                    matched = true;
                }
            }
        } else if let Some(caps) = self.forge_tps_regex.captures(line) {
            if let Some(m) = caps.name("tps") {
                if let Ok(val) = m.as_str().parse::<f64>() {
                    parsed_tps = Some(val.clamp(0.0, 20.0));
                    matched = true;
                }
            }
            if let Some(m) = caps.name("mspt") {
                if let Ok(val) = m.as_str().parse::<f64>() {
                    parsed_mspt = Some(val.max(0.0));
                    matched = true;
                }
            }
        } else {
            if let Some(caps) = self.spark_tps_regex.captures(line) {
                if let Some(m) = caps.name("tps") {
                    if let Ok(val) = m.as_str().parse::<f64>() {
                        parsed_tps = Some(val.clamp(0.0, 20.0));
                        matched = true;
                    }
                }
            }
            if let Some(caps) = self.spark_mspt_regex.captures(line) {
                if let Some(m) = caps.name("mspt") {
                    if let Ok(val) = m.as_str().parse::<f64>() {
                        parsed_mspt = Some(val.max(0.0));
                        matched = true;
                    }
                }
            } else if let Some(caps) = self.vanilla_avg_regex.captures(line) {
                if let Some(m) = caps.name("mspt") {
                    if let Ok(val) = m.as_str().parse::<f64>() {
                        parsed_mspt = Some(val.max(0.0));
                        matched = true;
                    }
                }
            }
        }

        if matched {
            self.record_sample(parsed_tps, parsed_mspt, None);
        }

        matched
    }

    /// Records a new sample into current values and rolling history buffers.
    pub fn record_sample(&self, tps: Option<f64>, mspt: Option<f64>, ping_ms: Option<u64>) {
        let mut state = self.state.lock().unwrap();
        if let Some(t) = tps {
            state.tps = Some(round1(t));
            state.tps_history.push_back(round1(t));
            while state.tps_history.len() > HISTORY_LIMIT {
                state.tps_history.pop_front();
            }
        }
        if let Some(m) = mspt {
            state.mspt = Some(round1(m));
            state.mspt_history.push_back(round1(m));
            while state.mspt_history.len() > HISTORY_LIMIT {
                state.mspt_history.pop_front();
            }
        }
        if let Some(p) = ping_ms {
            state.ping_latency_ms = Some(p);
        }
        state.last_updated = Some(Instant::now());
    }

    /// Records local TCP / Minecraft status ping latency in milliseconds.
    pub fn record_ping(&self, latency_ms: u64) {
        let mut state = self.state.lock().unwrap();
        state.ping_latency_ms = Some(latency_ms);
        // If we don't have a direct tick query TPS yet, estimate baseline health
        // from ping: < 35ms -> 20.0 TPS, < 75ms -> 19.5 TPS, otherwise lower
        if state.tps.is_none() {
            let estimated_tps = if latency_ms < 35 {
                20.0
            } else if latency_ms < 75 {
                19.5
            } else if latency_ms < 150 {
                18.0
            } else {
                15.0
            };
            state.tps = Some(estimated_tps);
            state.tps_history.push_back(estimated_tps);
            while state.tps_history.len() > HISTORY_LIMIT {
                state.tps_history.pop_front();
            }
        }
        state.last_updated = Some(Instant::now());
    }

    /// Retrieves current (tps, mspt, ping_ms).
    pub fn current_metrics(&self) -> (Option<f64>, Option<f64>, Option<u64>) {
        let state = self.state.lock().unwrap();
        (state.tps, state.mspt, state.ping_latency_ms)
    }

    /// Rolling TPS history (up to 60 samples).
    pub fn tps_history(&self) -> Vec<f64> {
        let state = self.state.lock().unwrap();
        state.tps_history.iter().copied().collect()
    }

    /// Rolling MSPT history (up to 60 samples).
    pub fn mspt_history(&self) -> Vec<f64> {
        let state = self.state.lock().unwrap();
        state.mspt_history.iter().copied().collect()
    }

    /// Returns true if telemetry has not been updated within `max_age`.
    pub fn is_stale(&self, max_age: Duration) -> bool {
        let state = self.state.lock().unwrap();
        match state.last_updated {
            Some(time) => time.elapsed() > max_age,
            None => true,
        }
    }

    /// Marks telemetry as actively requested by a connected admin viewing the dashboard.
    /// Extends active probing for 15 seconds.
    pub fn request_active_monitoring(&self) {
        let mut until = self.active_monitoring_until.lock().unwrap();
        *until = Some(Instant::now() + Duration::from_secs(15));
    }

    /// Whether a connected dashboard client has recently requested telemetry.
    pub fn is_active_monitoring_requested(&self) -> bool {
        let until = self.active_monitoring_until.lock().unwrap();
        match *until {
            Some(time) => Instant::now() < time,
            None => false,
        }
    }

    /// Extracts the actual log payload after standard Minecraft log prefixes.
    fn extract_log_payload(line: &str) -> &str {
        let trimmed = line.trim();
        if let Some(idx) = trimmed.find("]: ") {
            &trimmed[idx + 3..]
        } else if let Some(idx) = trimmed.find("[Server thread/INFO]: ") {
            &trimmed[idx + 22..]
        } else if let Some(idx) = trimmed.find("[main/INFO]: ") {
            &trimmed[idx + 13..]
        } else {
            trimmed
        }
    }

    /// Returns true if a line is an automated probe command echo, response, or command-error
    /// that should be suppressed from the human-facing console log.
    pub fn is_telemetry_probe_line(&self, line: &str) -> bool {
        let payload = Self::extract_log_payload(line).trim();
        if payload.is_empty() {
            return false;
        }

        // Direct command echos or error pointers
        if payload == "tick query"
            || payload == "/tick query"
            || payload == "forge tps"
            || payload == "/forge tps"
            || payload == "spark tps"
            || payload == "/spark tps"
            || payload.ends_with("tick query<--[HERE]")
            || payload.ends_with("forge tps<--[HERE]")
            || payload.ends_with("spark tps<--[HERE]")
        {
            return true;
        }

        // Vanilla tick query response lines
        if payload.starts_with("The tick rate is")
            || payload.starts_with("[Server: The tick rate is")
            || payload.starts_with("Target tick rate:")
            || payload.starts_with("[Server: Target tick rate:")
            || payload.contains("average tick time:")
        {
            return true;
        }

        // Forge TPS response lines
        if (payload.contains("Mean tick time:") && payload.contains("Mean TPS:"))
            || payload.starts_with("Dim ")
            || payload.starts_with("Overall :")
        {
            return true;
        }

        // Command syntax errors caused by automated probes
        if payload.contains("Unknown or incomplete command")
            || payload.contains("Incorrect argument for command")
        {
            return true;
        }

        false
    }

    /// Resets telemetry state when server stops.
    pub fn reset(&self) {
        let mut state = self.state.lock().unwrap();
        state.tps = None;
        state.mspt = None;
        state.ping_latency_ms = None;
        state.last_updated = None;
        state.tps_history.clear();
        state.mspt_history.clear();
    }
}

impl Default for TpsTracker {
    fn default() -> Self {
        Self::new()
    }
}

fn round1(val: f64) -> f64 {
    (val * 10.0).round() / 10.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_vanilla_tick_query() {
        let tracker = TpsTracker::new();
        let matched = tracker.on_line("The tick rate is 20.0 ticks per second (average tick time: 24.3ms)");
        assert!(matched);
        let (tps, mspt, _) = tracker.current_metrics();
        assert_eq!(tps, Some(20.0));
        assert_eq!(mspt, Some(24.3));
    }

    #[test]
    fn parses_forge_tps() {
        let tracker = TpsTracker::new();
        let matched = tracker.on_line("Dim  0 : Mean tick time: 14.8 ms. Mean TPS: 20.000");
        assert!(matched);
        let (tps, mspt, _) = tracker.current_metrics();
        assert_eq!(tps, Some(20.0));
        assert_eq!(mspt, Some(14.8));
    }

    #[test]
    fn parses_spark_tps_and_mspt() {
        let tracker = TpsTracker::new();
        assert!(tracker.on_line("TPS from last 5s, 10s, 1m, 5m, 15m: 19.85, 19.92, 20.0"));
        assert!(tracker.on_line("MSPT from last 5s, 10s, 1m: 28.4, 26.1, 24.0"));
        let (tps, mspt, _) = tracker.current_metrics();
        assert_eq!(tps, Some(19.9));
        assert_eq!(mspt, Some(28.4));
    }

    #[test]
    fn rolling_history_caps_at_limit() {
        let tracker = TpsTracker::new();
        for i in 0..80 {
            tracker.record_sample(Some(15.0 + (i as f64 % 5.0)), Some(30.0), None);
        }
        assert_eq!(tracker.tps_history().len(), HISTORY_LIMIT);
    }
}
