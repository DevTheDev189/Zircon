//! REST endpoints for the mod manager tab of the admin UI: file listing,
//! uploads, downloads, provider search and remote installs.
//!
//! Port of `com.mcmanager.server.web.controller.ModController`.

use axum::extract::{Multipart, Path, Query, State};
use axum::http::{header, HeaderMap, StatusCode};
use axum::response::IntoResponse;
use axum::Json;

use serde::Deserialize;
use tokio_util::io::ReaderStream;

use crate::services::mods::ModManagementService;
use crate::services::resolver::ModServiceResolver;
use crate::web::app::{ApiError, AppState};
use crate::web::views;

/// Resolves the mod service for the instance owning the request port, else the
/// active instance.
fn resolve_mods(state: &AppState, headers: &HeaderMap) -> ModManagementService {
    let host = headers.get(header::HOST).and_then(|v| v.to_str().ok());
    if let Some(port) = ModServiceResolver::host_port(host) {
        if let Some(mods) = state.resolver.mods_by_external_port(port) {
            return mods;
        }
    }
    state.resolver.mods()
}

/// GET /api/mods — list of installed mods from the BOM.
pub async fn list_mods(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Json<serde_json::Value> {
    let mods = resolve_mods(&state, &headers).list_mods();
    let hits: Vec<serde_json::Value> = mods.iter().map(views::mod_entry_to_map).collect();
    Json(serde_json::json!({ "mods": hits }))
}

/// GET /files/mods/{filename} — download a hosted mod JAR.
pub async fn download_mod(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(filename): Path<String>,
) -> Result<impl IntoResponse, ApiError> {
    let mods = resolve_mods(&state, &headers);
    let file = mods
        .get_mod_file(&filename)
        .ok_or_else(|| ApiError::NotFound(format!("Mod not found: {filename}")))?;
    let size = tokio::fs::metadata(&file).await?.len();
    let stream = ReaderStream::new(tokio::fs::File::open(&file).await?);
    Ok((
        [(header::CONTENT_TYPE, "application/java-archive")],
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

/// POST /api/mods/upload (multipart, field "file") — add a local JAR to the server.
pub async fn upload_mod(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(params): Query<OriginParam>,
    mut multipart: Multipart,
) -> Result<(StatusCode, Json<serde_json::Value>), ApiError> {
    let Some((filename, bytes)) = take_upload(&mut multipart).await? else {
        return Err(ApiError::BadRequest(
            "No file uploaded (form field 'file')".to_string(),
        ));
    };
    let entry = resolve_mods(&state, &headers)
        .add_mod(
            std::io::Cursor::new(bytes),
            &filename,
            params.origin.as_deref(),
        )
        .await?;
    Ok((StatusCode::CREATED, Json(views::mod_entry_to_map(&entry))))
}

/// DELETE /api/mods/{filename} — remove a mod.
pub async fn remove_mod(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(filename): Path<String>,
) -> Result<StatusCode, ApiError> {
    let removed = resolve_mods(&state, &headers).remove_mod(&filename)?;
    if !removed {
        return Err(ApiError::NotFound("Mod not found".to_string()));
    }
    Ok(StatusCode::NO_CONTENT)
}

/// Query params for search / version listing.
#[derive(Debug, Default, Deserialize)]
pub struct SearchParams {
    pub query: Option<String>,
    #[serde(rename = "mcVersion")]
    pub mc_version: Option<String>,
    pub loader: Option<String>,
    pub origin: Option<String>,
    #[serde(rename = "type")]
    pub project_type: Option<String>,
}

/// Query param for uploads: `?origin=...`.
#[derive(Debug, Default, Deserialize)]
pub struct OriginParam {
    pub origin: Option<String>,
}

/// GET /api/mods/search?query=&mcVersion=&loader=&origin= — search Modrinth/CurseForge.
pub async fn search_mods(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(params): Query<SearchParams>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let mods = resolve_mods(&state, &headers);
    let query = params.query.unwrap_or_default();
    let origin = params.origin.as_deref().unwrap_or("modrinth");
    let mut result = serde_json::Map::new();

    if origin.eq_ignore_ascii_case("curseforge") {
        if !mods.has_curse_forge_key() {
            result.insert(
                "origin".to_string(),
                serde_json::Value::String("curseforge".to_string()),
            );
            result.insert("hits".to_string(), serde_json::Value::Array(vec![]));
            result.insert(
                "notice".to_string(),
                serde_json::Value::String(
                    "CurseForge API key not configured on the server. Add one in the Settings tab (or set MC_MANAGER_CURSEFORGE_API_KEY) to search CurseForge.".to_string(),
                ),
            );
            return Ok(Json(serde_json::Value::Object(result)));
        }
        let hits = mods
            .curse_forge()
            .search_mods(&query, params.mc_version.as_deref())
            .await
            .map_err(|e| ApiError::BadGateway(e.to_string()))?;
        result.insert(
            "origin".to_string(),
            serde_json::Value::String("curseforge".to_string()),
        );
        result.insert(
            "hits".to_string(),
            serde_json::Value::Array(hits.iter().map(views::curseforge_mod_to_map).collect()),
        );
    } else {
        let hits = mods
            .modrinth()
            .search_mods_with_type(
                &query,
                params.mc_version.as_deref(),
                params.loader.as_deref(),
                params.project_type.as_deref(),
            )
            .await
            .map_err(|e| ApiError::BadGateway(e.to_string()))?;
        result.insert(
            "origin".to_string(),
            serde_json::Value::String("modrinth".to_string()),
        );
        result.insert(
            "hits".to_string(),
            serde_json::Value::Array(hits.iter().map(views::modrinth_hit_to_map).collect()),
        );
    }
    Ok(Json(serde_json::Value::Object(result)))
}

/// Query params for the Modrinth version picker.
#[derive(Debug, Default, Deserialize)]
pub struct VersionParams {
    pub project_id: Option<String>,
    #[serde(rename = "mcVersion")]
    pub mc_version: Option<String>,
    pub loader: Option<String>,
}

/// GET /api/mods/modrinth/versions?projectId=&mcVersion=&loader= — version picker.
pub async fn modrinth_versions(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(params): Query<VersionParams>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let Some(project_id) = params.project_id else {
        return Err(ApiError::BadRequest("projectId is required".to_string()));
    };
    let mods = resolve_mods(&state, &headers);
    let versions = mods
        .modrinth()
        .list_project_versions(
            &project_id,
            params.mc_version.as_deref(),
            params.loader.as_deref(),
        )
        .await
        .map_err(|e| ApiError::BadGateway(e.to_string()))?;
    let mapped: Vec<serde_json::Value> = versions
        .iter()
        .map(views::modrinth_version_to_map)
        .collect();
    Ok(Json(serde_json::json!({ "versions": mapped })))
}

/// Query params for the CurseForge file picker.
#[derive(Debug, Default, Deserialize)]
pub struct CurseForgeParams {
    pub mod_id: Option<String>,
}

/// GET /api/mods/curseforge/files?modId= — file picker for a CurseForge mod.
pub async fn curseforge_files(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(params): Query<CurseForgeParams>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let mod_id = params
        .mod_id
        .ok_or_else(|| ApiError::BadRequest("modId is required".to_string()))?;
    let mod_id: i64 = mod_id
        .parse()
        .map_err(|_| ApiError::BadRequest("modId must be a number".to_string()))?;
    let mods = resolve_mods(&state, &headers);
    if !mods.has_curse_forge_key() {
        return Err(ApiError::BadRequest(
            "CurseForge API key not configured on the server".to_string(),
        ));
    }
    let files = mods
        .curse_forge()
        .list_mod_files(mod_id)
        .await
        .map_err(|e| ApiError::BadGateway(e.to_string()))?;
    let mapped: Vec<serde_json::Value> = files.iter().map(views::curseforge_file_to_map).collect();
    Ok(Json(serde_json::json!({ "files": mapped })))
}

/// POST /api/mods/install
pub async fn install_mod(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<InstallRequest>,
) -> Result<(StatusCode, Json<serde_json::Value>), ApiError> {
    let Some(origin) = &body.origin else {
        return Err(ApiError::BadRequest("origin is required".to_string()));
    };
    let mods = resolve_mods(&state, &headers);
    let entry = match origin.to_lowercase().as_str() {
        "modrinth" => {
            let (Some(project_id), Some(version_id)) = (&body.project_id, &body.version_id) else {
                return Err(ApiError::BadRequest(
                    "projectId and versionId are required for modrinth".to_string(),
                ));
            };
            mods.install_modrinth_version(project_id, Some(version_id), None, None)
                .await?
        }
        "curseforge" => {
            let (Some(download_url), Some(filename)) = (&body.download_url, &body.filename) else {
                return Err(ApiError::BadRequest(
                    "downloadUrl and filename are required for curseforge".to_string(),
                ));
            };
            let mut entry = mods
                .install_from_url(download_url, filename, "curseforge")
                .await?;
            if let Some(file_id) = &body.file_id {
                entry.id = Some(file_id.to_string());
            }
            entry
        }
        _ => {
            return Err(ApiError::BadRequest(
                "origin must be 'modrinth' or 'curseforge'".to_string(),
            ))
        }
    };
    Ok((StatusCode::CREATED, Json(views::mod_entry_to_map(&entry))))
}

/// POST /api/mods/install request body.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstallRequest {
    pub origin: Option<String>,
    pub project_id: Option<String>,
    pub version_id: Option<String>,
    pub download_url: Option<String>,
    pub filename: Option<String>,
    pub file_id: Option<String>,
}

/// Reads the first `file` multipart field fully into memory.
pub(crate) async fn take_upload(
    multipart: &mut Multipart,
) -> Result<Option<(String, Vec<u8>)>, ApiError> {
    loop {
        let part = match multipart.next_field().await {
            Ok(Some(part)) => part,
            Ok(None) => return Ok(None),
            Err(e) => return Err(ApiError::BadRequest(e.to_string())),
        };
        if part.name() == Some("file") {
            let filename = part.file_name().unwrap_or("upload.jar").to_string();
            let bytes = part
                .bytes()
                .await
                .map_err(|e| ApiError::BadRequest(e.to_string()))?;
            return Ok(Some((filename, bytes.to_vec())));
        }
    }
}
