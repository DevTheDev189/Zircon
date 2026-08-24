//! System endpoints: server self-update check & apply.
//!
//! Both routes are admin-only (mounted inside the JWT-protected router). The
//! update applies in place and then relaunches the daemon, so the caller gets
//! an immediate `ok` before the process restarts.

use axum::extract::State;
use axum::Json;
use serde_json::json;

use crate::updater::{ServerUpdater, CURRENT_SERVER_VERSION};
use crate::web::app::{ApiError, AppState};
use crate::web::auth::CurrentUser;

/// GET /api/system/update/check — reports the running version and whether a
/// newer release is available (with the manifest for UI display).
pub async fn check_update() -> Result<Json<serde_json::Value>, ApiError> {
    let updater = ServerUpdater::new();
    let update = updater.check_update().await.map_err(ApiError::Internal)?;
    Ok(Json(json!({
        "currentVersion": CURRENT_SERVER_VERSION,
        "updateAvailable": update.is_some(),
        "manifest": update
    })))
}

/// POST /api/system/update/apply — stops all instances, swaps in the verified
/// binary and relaunches the daemon with the original arguments. The action is
/// recorded in the audit log under the authenticated admin's identity.
pub async fn apply_update(
    State(state): State<AppState>,
    user: CurrentUser,
) -> Result<Json<serde_json::Value>, ApiError> {
    let updater = ServerUpdater::new();
    let Some(manifest) = updater.check_update().await.map_err(ApiError::Internal)? else {
        return Err(ApiError::BadRequest("No updates available".into()));
    };

    state.audit.log(
        &user.username,
        "SERVER_UPDATE_APPLY",
        &format!("Target version: {}", manifest.version),
    );

    // Gracefully stop every instance so the swap never happens mid-world-write.
    for inst in state.instances.list_instances() {
        state.instances.stop_instance(&inst.id).await;
    }

    updater
        .apply_update(&manifest)
        .await
        .map_err(ApiError::Internal)?;

    // Give the HTTP response time to flush before the process exits.
    tokio::spawn(async move {
        tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
        let _ = ServerUpdater::restart_process();
    });

    Ok(Json(
        json!({ "ok": true, "message": "Server updated. Restarting..." }),
    ))
}
