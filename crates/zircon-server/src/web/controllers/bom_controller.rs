//! Publishes the `BillOfMaterials` that the client launcher syncs against.
//!
//! In multi-instance mode the BOM is resolved from the `Host` header port of
//! the request: a client connecting to an instance's dedicated port (e.g.
//! `localhost:25566`) receives exactly that instance's BOM, and the shared main
//! port (25565) falls back to the active instance.
//!
//! Port of `com.mcmanager.server.web.controller.BomController`.

use std::sync::Arc;

use axum::extract::{Path, Request, State};
use axum::http::header::HOST;
use axum::response::IntoResponse;

use crate::services::bom::BomService;
use crate::web::app::{ApiError, AppState};
use crate::web::config_routes::{
    resolve_instance_for_host, resolve_instance_for_ref, resolve_instance_for_ref_with_host,
};

/// GET /bom — full BOM as JSON for the instance owning the request's port.
pub async fn get_bom(
    State(state): State<AppState>,
    request: Request,
) -> Result<impl IntoResponse, ApiError> {
    let host = request.headers().get(HOST).and_then(|v| v.to_str().ok());
    let bom = bom_service_for_host(&state, host).get_client_bom();
    let json = serde_json::to_string_pretty(&bom).map_err(|e| ApiError::Internal(e.to_string()))?;
    Ok(([("content-type", "application/json; charset=utf-8")], json))
}

/// GET /{port}/bom — BOM for the instance owning the path port (or id), used
/// by HTTPS reverse proxies whose `Host` header carries no port. When the path
/// port is default 25565, host-header virtual host routing takes precedence.
pub async fn get_bom_by_port(
    State(state): State<AppState>,
    Path(port_or_id): Path<String>,
    request: Request,
) -> Result<impl IntoResponse, ApiError> {
    let host = request.headers().get(HOST).and_then(|v| v.to_str().ok());
    let bom = resolve_bom_for_ref_with_host(&state, &port_or_id, host).get_client_bom();
    let json = serde_json::to_string_pretty(&bom).map_err(|e| ApiError::Internal(e.to_string()))?;
    Ok(([("content-type", "application/json; charset=utf-8")], json))
}

/// Resolves the BOM service for a path-based `:port`/instance-id reference:
/// an owned external port, an instance's internal port, an instance id, then
/// the default (active/legacy) store.
#[allow(dead_code)]
pub(crate) fn resolve_bom_for_ref(state: &AppState, port_or_id: &str) -> Arc<BomService> {
    resolve_bom_for_ref_with_host(state, port_or_id, None)
}

pub(crate) fn resolve_bom_for_ref_with_host(
    state: &AppState,
    port_or_id: &str,
    host: Option<&str>,
) -> Arc<BomService> {
    if let Some(instance) = resolve_instance_for_ref_with_host(state, port_or_id, host) {
        return state.resolver.instance_service(&instance).bom;
    }
    state.resolver.bom()
}

/// The BOM service of the instance owning the request's port, else the active
/// instance (or the legacy single-server store).
pub(crate) fn bom_service_for_host(state: &AppState, host: Option<&str>) -> Arc<BomService> {
    if let Some(instance) = resolve_instance_for_host(state, host) {
        return state.resolver.instance_service(&instance).bom;
    }
    state.resolver.bom()
}
