//! Serves shaderpack/resourcepack downloads to the client's sync engine,
//! mirroring `ModController::download_mod`. Resolved per request from the
//! `Host` header port so downloads follow the instance whose port the client
//! connected through.
//!
//! Port of `com.mcmanager.server.web.controller.PackFileController`.

use axum::extract::{Path, State};
use axum::http::{header, HeaderMap};
use axum::response::IntoResponse;

use tokio_util::io::ReaderStream;

use crate::services::packs::PackManagementService;
use crate::services::resolver::ModServiceResolver;
use crate::web::app::{ApiError, AppState};

fn resolve_packs(state: &AppState, headers: &HeaderMap) -> PackManagementService {
    let host = headers.get(header::HOST).and_then(|v| v.to_str().ok());
    if let Some(port) = ModServiceResolver::host_port(host) {
        if let Some(packs) = state.resolver.packs_by_external_port(port) {
            return packs;
        }
    }
    state.resolver.packs()
}

async fn stream_pack(
    state: &AppState,
    headers: &HeaderMap,
    filename: &str,
    shader: bool,
) -> Result<impl IntoResponse, ApiError> {
    let packs = resolve_packs(state, headers);
    let file = if shader {
        packs.get_shaderpack_file(filename)
    } else {
        packs.get_resourcepack_file(filename)
    };
    let Some(file) = file else {
        return Err(ApiError::NotFound("Pack not found".to_string()));
    };
    let size = tokio::fs::metadata(&file).await?.len();
    let stream = ReaderStream::new(tokio::fs::File::open(&file).await?);
    Ok((
        [(header::CONTENT_TYPE, "application/zip")],
        [
            (
                header::CONTENT_DISPOSITION,
                format!(
                    "attachment; filename=\"{}\"",
                    file.file_name().unwrap_or_default().to_string_lossy()
                ),
            ),
            (header::CONTENT_LENGTH, size.to_string()),
        ],
        axum::body::Body::from_stream(stream),
    ))
}

/// GET /files/shaderpacks/{filename}
pub async fn download_shaderpack(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(filename): Path<String>,
) -> Result<impl IntoResponse, ApiError> {
    stream_pack(&state, &headers, &filename, true).await
}

/// GET /files/resourcepacks/{filename}
pub async fn download_resourcepack(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(filename): Path<String>,
) -> Result<impl IntoResponse, ApiError> {
    stream_pack(&state, &headers, &filename, false).await
}
