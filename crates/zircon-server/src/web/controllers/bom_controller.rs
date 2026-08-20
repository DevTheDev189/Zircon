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
use crate::services::resolver::ModServiceResolver;
use crate::web::app::{ApiError, AppState};

/// GET /bom — full BOM as JSON for the instance owning the request's port.
pub async fn get_bom(
    State(state): State<AppState>,
    request: Request,
) -> Result<impl IntoResponse, ApiError> {
    let host = request.headers().get(HOST).and_then(|v| v.to_str().ok());
    let bom = bom_service_for_host(&state, host).get_bom();
    let json = serde_json::to_string_pretty(&bom).map_err(|e| ApiError::Internal(e.to_string()))?;
    Ok(([("content-type", "application/json; charset=utf-8")], json))
}

/// GET /{port}/bom — BOM for the instance owning the path port (or id), used
/// by HTTPS reverse proxies whose `Host` header carries no port.
pub async fn get_bom_by_port(
    State(state): State<AppState>,
    Path(port_or_id): Path<String>,
) -> Result<impl IntoResponse, ApiError> {
    let bom = resolve_bom_for_ref(&state, &port_or_id).get_bom();
    let json = serde_json::to_string_pretty(&bom).map_err(|e| ApiError::Internal(e.to_string()))?;
    Ok(([("content-type", "application/json; charset=utf-8")], json))
}

/// Resolves the BOM service for a path-based `:port`/instance-id reference:
/// an owned external port, an instance's internal port, an instance id, then
/// the default (active/legacy) store.
pub(crate) fn resolve_bom_for_ref(state: &AppState, port_or_id: &str) -> Arc<BomService> {
    if let Ok(port) = port_or_id.parse::<i32>() {
        if let Some(bom) = state.resolver.bom_by_external_port(port) {
            return bom;
        }
        if let Some(cfg) = state.instances.find_by_internal_port(port as u16) {
            return state.resolver.instance_service(&cfg).bom;
        }
        return state.resolver.bom();
    }
    if let Ok(cfg) = state.instances.get_instance(port_or_id) {
        return state.resolver.instance_service(&cfg).bom;
    }
    state.resolver.bom()
}

/// The BOM service of the instance owning the request's port, else the active
/// instance (or the legacy single-server store).
pub(crate) fn bom_service_for_host(state: &AppState, host: Option<&str>) -> Arc<BomService> {
    if let Some(port) = ModServiceResolver::host_port(host) {
        if let Some(bom) = state.resolver.bom_by_external_port(port) {
            return bom;
        }
    }
    state.resolver.bom()
}
