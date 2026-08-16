//! Real-time host metrics for the admin UI "System Stats" tab: CPU
//! (system-wide and process), memory usage, and free disk space on the data
//! dir. Every call to `sample` appends to a rolling 60-entry history so the
//! frontend can render sparklines or track trends.
//!
//! Port of `com.mcmanager.server.stats.SystemMetricsService`.

use std::path::Path;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use sysinfo::{Disks, System};

const HISTORY_LIMIT: usize = 60;

/// One immutable measurement. Values are already rounded for display.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MetricPoint {
    pub timestamp: i64,
    pub system_cpu_load: f64,
    pub process_cpu_load: f64,
    pub used_memory_bytes: u64,
    pub max_memory_bytes: u64,
    pub total_disk_bytes: u64,
    pub free_disk_bytes: u64,
}

struct MetricsState {
    history: Vec<MetricPoint>,
    system: System,
}

static STATE: Mutex<Option<MetricsState>> = Mutex::new(None);

/// Takes one measurement of the host, appends it to the history and returns it.
pub fn sample(data_dir: &Path) -> MetricPoint {
    let mut guard = STATE.lock().unwrap();
    let state = guard.get_or_insert_with(|| MetricsState {
        history: Vec::with_capacity(HISTORY_LIMIT),
        system: System::new_all(),
    });

    state.system.refresh_all();

    let sys_cpu = percent_of(f64::from(state.system.global_cpu_usage()) / 100.0);
    let pid = sysinfo::get_current_pid().ok();
    let proc_cpu = percent_of(
        pid.and_then(|p| state.system.process(p))
            .map(|p| f64::from(p.cpu_usage()) / 100.0)
            .unwrap_or(0.0),
    );

    let total_mem = state.system.total_memory();
    let used_mem = state.system.used_memory();

    let (total_disk, free_disk) = disk_space(data_dir);

    let point = MetricPoint {
        timestamp: now_millis(),
        system_cpu_load: round1(sys_cpu),
        process_cpu_load: round1(proc_cpu),
        used_memory_bytes: used_mem,
        max_memory_bytes: total_mem,
        total_disk_bytes: total_disk,
        free_disk_bytes: free_disk,
    };

    state.history.push(point.clone());
    if state.history.len() > HISTORY_LIMIT {
        state.history.remove(0);
    }
    point
}

/// The latest measurement plus the rolling history.
pub fn get_metrics_snapshot(data_dir: &Path) -> MetricsSnapshot {
    let current = sample(data_dir);
    let history = STATE
        .lock()
        .unwrap()
        .as_ref()
        .map(|s| s.history.clone())
        .unwrap_or_default();
    MetricsSnapshot { current, history }
}

/// Snapshot payload served by `GET /api/stats`.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MetricsSnapshot {
    pub current: MetricPoint,
    pub history: Vec<MetricPoint>,
}

fn disk_space(data_dir: &Path) -> (u64, u64) {
    let disks = Disks::new_with_refreshed_list();
    for disk in &disks {
        if data_dir.starts_with(disk.mount_point()) {
            return (disk.total_space(), disk.available_space());
        }
    }
    disks
        .first()
        .map(|d| (d.total_space(), d.available_space()))
        .unwrap_or((0, 0))
}

/// Converts a load fraction (0..1, or negative/NaN when unavailable) to a percent.
fn percent_of(load: f64) -> f64 {
    if !load.is_finite() || load < 0.0 {
        return 0.0;
    }
    load.min(1.0) * 100.0
}

fn round1(value: f64) -> f64 {
    (value * 10.0).round() / 10.0
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

    #[test]
    fn samples_are_well_formed_and_rolling() {
        let dir = std::env::temp_dir();
        let first = sample(&dir);
        assert!(first.timestamp > 0);
        assert!(first.max_memory_bytes > 0);
        assert!(first.total_disk_bytes > 0);

        for _ in 0..70 {
            sample(&dir);
        }
        let snapshot = get_metrics_snapshot(&dir);
        assert!(snapshot.history.len() <= HISTORY_LIMIT);
        assert_eq!(HISTORY_LIMIT, snapshot.history.len());
    }
}
