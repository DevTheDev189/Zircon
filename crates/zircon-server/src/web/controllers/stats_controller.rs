//! System metrics endpoint (`GET /api/stats`).
//!
//! Port of the stats route in `com.mcmanager.server.web.JavalinApp`.

use axum::extract::{Query, State};
use axum::Json;
use serde::Deserialize;

use crate::stats;
use crate::web::app::AppState;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StatsQuery {
    pub instance_id: Option<String>,
}

/// GET /api/stats — current sample + rolling history + live tick telemetry.
pub async fn stats(
    Query(query): Query<StatsQuery>,
    State(state): State<AppState>,
) -> Json<serde_json::Value> {
    let target_id = query
        .instance_id
        .clone()
        .or_else(|| state.instances.active_instance_id());

    // Check if the requested instance (or active instance) is running
    let is_running = match target_id.as_deref() {
        Some(id) => state.instances.is_running(id),
        None => false,
    };

    let is_sleeping = match target_id.as_deref() {
        Some(id) if !is_running => state.instances.wakeable(id),
        _ => false,
    };

    let server_pid = match target_id.as_deref() {
        Some(id) if is_running => state.instances.get_instance_pid(id),
        _ => None,
    };

    let server_configured_mem = match target_id.as_deref() {
        Some(id) => state.instances.get_instance_configured_memory_bytes(id),
        None => 4 * 1024 * 1024 * 1024,
    };

    let (tps, mspt, ping_ms, tps_history, mspt_history) = if is_running {
        let tps_tracker = target_id
            .as_deref()
            .and_then(|id| state.instances.get_tps_tracker(id))
            .unwrap_or_else(|| state.instances.active_tps_tracker());

        // Mark telemetry as actively monitored by a client
        tps_tracker.request_active_monitoring();

        let (t, m, p) = tps_tracker.current_metrics();
        let th = tps_tracker.tps_history();
        let mh = tps_tracker.mspt_history();
        (t, m, p, th, mh)
    } else {
        (None, None, None, Vec::new(), Vec::new())
    };

    let all_instances_telemetry = state.instances.all_instances_telemetry();

    let snapshot = stats::get_metrics_snapshot_with_telemetry(
        &state.config.data_dir,
        tps,
        mspt,
        ping_ms,
        tps_history,
        mspt_history,
        target_id,
        all_instances_telemetry,
        server_pid,
        server_configured_mem,
        is_running,
        is_sleeping,
    );
    Json(serde_json::to_value(snapshot).unwrap_or_default())
}
