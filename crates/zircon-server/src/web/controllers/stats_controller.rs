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
    // Check if the requested instance (or active instance) is running
    let is_running = match query.instance_id.as_deref() {
        Some(id) => state.instances.is_running(id),
        None => state.instances.active_instance_id().is_some(),
    };

    let (tps, mspt, ping_ms, tps_history, mspt_history) = if is_running {
        let tps_tracker = query
            .instance_id
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

    let active_instance_id = query.instance_id.or_else(|| state.instances.active_instance_id());
    let all_instances_telemetry = state.instances.all_instances_telemetry();

    let snapshot = stats::get_metrics_snapshot_with_telemetry(
        &state.config.data_dir,
        tps,
        mspt,
        ping_ms,
        tps_history,
        mspt_history,
        active_instance_id,
        all_instances_telemetry,
    );
    Json(serde_json::to_value(snapshot).unwrap_or_default())
}
