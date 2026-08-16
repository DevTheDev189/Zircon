//! System metrics endpoint (`GET /api/stats`).
//!
//! Port of the stats route in `com.mcmanager.server.web.JavalinApp`.

use axum::extract::State;
use axum::Json;

use crate::stats;
use crate::web::app::AppState;

/// GET /api/stats — current sample + rolling history.
pub async fn stats(State(state): State<AppState>) -> Json<serde_json::Value> {
    let snapshot = stats::get_metrics_snapshot(&state.config.data_dir);
    Json(serde_json::to_value(snapshot).unwrap_or_default())
}
