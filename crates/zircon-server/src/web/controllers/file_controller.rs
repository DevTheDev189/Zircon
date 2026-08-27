//! File management REST controller: directory listing, text file viewing/editing,
//! uploading, creating, copying, deleting, and BOM config sync toggling.
//!
//! Enforces zero-trust instance sandboxing: all file operations are strictly jailed
//! within the instance's `<data>/instances/<id>/server/` directory.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::SystemTime;

use axum::extract::{Multipart, Path as AxumPath, Query, Request, State};
use axum::http::{header, HeaderMap};
use axum::response::IntoResponse;
use axum::Json;
use serde::{Deserialize, Serialize};
use tokio_util::io::ReaderStream;

use zircon_core::crypto::hash::sha1_bytes;
use zircon_core::model::{ConfigFileEntry, InstanceConfig};
use zircon_core::security::path_validator::{
    has_allowed_config_extension, sanitize_relative_path, validate_config_relative_path,
};

use crate::services::bom::BomService;
use crate::web::app::{ApiError, AppState};
use crate::web::config_routes::{resolve_instance_for_host, resolve_instance_for_ref};

const MAX_TEXT_FILE_BYTES: u64 = 10 * 1024 * 1024; // 10 MiB limit for editor

#[derive(Debug, Deserialize)]
pub struct ListFilesQuery {
    pub path: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct GetContentQuery {
    pub path: String,
}

#[derive(Debug, Deserialize)]
pub struct SaveContentRequest {
    pub path: String,
    pub content: String,
}

#[derive(Debug, Deserialize)]
pub struct CreateRequest {
    pub path: String,
    #[serde(default)]
    pub is_dir: bool,
    pub content: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct DeleteRequest {
    pub path: String,
}

#[derive(Debug, Deserialize)]
pub struct CopyMoveRequest {
    pub from: String,
    pub to: String,
}

#[derive(Debug, Deserialize)]
pub struct SyncToggleRequest {
    pub path: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileItem {
    pub name: String,
    pub path: String,
    pub is_dir: bool,
    pub size: u64,
    pub modified: u64,
    pub extension: Option<String>,
    pub is_synced_config: bool,
    pub can_sync_config: bool,
}

/// Resolves the instance's `server/` root directory.
fn instance_server_dir(state: &AppState, instance_id: &str) -> Result<PathBuf, ApiError> {
    state.instances.get_instance(instance_id)?; // 404 if missing
    let server_dir = state.instances.get_instance_dir(instance_id).join("server");
    fs::create_dir_all(&server_dir)?;
    Ok(server_dir)
}

/// Helper to get the BomService for an instance.
fn get_bom_service(state: &AppState, instance_id: &str) -> Result<Arc<BomService>, ApiError> {
    let cfg = state.instances.get_instance(instance_id)?;
    Ok(state.resolver.instance_service(&cfg).bom)
}

/// Strictly resolves a safe path within the instance `server/` directory, preventing traversal.
fn resolve_safe_path(server_dir: &Path, rel_path: &str) -> Result<PathBuf, ApiError> {
    let sanitized = sanitize_relative_path(rel_path)
        .map_err(|e| ApiError::BadRequest(format!("Invalid path: {e}")))?;

    let target = if sanitized.is_empty() {
        server_dir.to_path_buf()
    } else {
        server_dir.join(&sanitized)
    };

    // Ensure lexical canonical jail boundary check
    let canonical_root = server_dir
        .canonicalize()
        .map_err(|e| ApiError::Internal(format!("Failed to canonicalize server dir: {e}")))?;

    if target.exists() {
        let canonical_target = target
            .canonicalize()
            .map_err(|e| ApiError::Internal(format!("Failed to canonicalize target: {e}")))?;
        if !canonical_target.starts_with(&canonical_root) {
            return Err(ApiError::BadRequest("Path traversal outside instance jail".into()));
        }
        Ok(canonical_target)
    } else {
        // If target doesn't exist yet, ensure its parent is within root
        if let Some(parent) = target.parent() {
            if parent.exists() {
                let canonical_parent = parent
                    .canonicalize()
                    .map_err(|e| ApiError::Internal(format!("Failed to canonicalize parent: {e}")))?;
                if !canonical_parent.starts_with(&canonical_root) {
                    return Err(ApiError::BadRequest("Path traversal outside instance jail".into()));
                }
            }
        }
        Ok(target)
    }
}

/// GET /api/instances/:id/files?path=... — list directory contents
pub async fn list_files(
    State(state): State<AppState>,
    AxumPath(id): AxumPath<String>,
    Query(query): Query<ListFilesQuery>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let server_dir = instance_server_dir(&state, &id)?;
    let rel_req = query.path.as_deref().unwrap_or("");
    let target_dir = resolve_safe_path(&server_dir, rel_req)?;

    if !target_dir.is_dir() {
        return Err(ApiError::NotFound(format!("Directory not found: {rel_req}")));
    }

    let bom_service = get_bom_service(&state, &id)?;
    let bom = bom_service.get_bom();
    let read_dir = fs::read_dir(&target_dir)?;
    let mut items = Vec::new();

    let server_canon = server_dir.canonicalize()?;

    for entry in read_dir.flatten() {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();

        // Skip hidden/system files
        if name.starts_with(".tmp.") {
            continue;
        }

        let is_dir = path.is_dir();
        let metadata = match entry.metadata() {
            Ok(m) => m,
            Err(_) => continue,
        };

        let size = if is_dir { 0 } else { metadata.len() };
        let modified = metadata
            .modified()
            .ok()
            .and_then(|t| t.duration_since(SystemTime::UNIX_EPOCH).ok())
            .map(|d| d.as_secs())
            .unwrap_or(0);

        let extension = if is_dir {
            None
        } else {
            path.extension().and_then(|e| e.to_str()).map(|s| s.to_string())
        };

        // Compute relative path from server_dir
        let rel_from_server = match path.canonicalize() {
            Ok(canon) => match canon.strip_prefix(&server_canon) {
                Ok(p) => p.to_string_lossy().replace('\\', "/"),
                Err(_) => name.clone(),
            },
            Err(_) => {
                if rel_req.is_empty() {
                    name.clone()
                } else {
                    format!("{rel_req}/{name}")
                }
            }
        };

        // Config sync status: check if file has an allowed config extension and check if in bom.configs
        let config_rel_path = rel_from_server.strip_prefix("config/").unwrap_or(&rel_from_server);
        let can_sync_config = !is_dir && has_allowed_config_extension(&name);
        let is_synced_config = can_sync_config
            && bom
                .configs
                .iter()
                .any(|c| c.path == config_rel_path || c.path == rel_from_server);

        items.push(FileItem {
            name,
            path: rel_from_server,
            is_dir,
            size,
            modified,
            extension,
            is_synced_config,
            can_sync_config,
        });
    }

    // Sort: directories first, then alphabetically
    items.sort_by(|a, b| match (a.is_dir, b.is_dir) {
        (true, false) => std::cmp::Ordering::Less,
        (false, true) => std::cmp::Ordering::Greater,
        _ => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
    });

    let current_rel = target_dir
        .strip_prefix(&server_canon)
        .map(|p| p.to_string_lossy().replace('\\', "/"))
        .unwrap_or_default();

    Ok(Json(serde_json::json!({
        "currentPath": current_rel,
        "files": items
    })))
}

/// GET /api/instances/:id/files/content?path=... — read text file content
pub async fn get_file_content(
    State(state): State<AppState>,
    AxumPath(id): AxumPath<String>,
    Query(query): Query<GetContentQuery>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let server_dir = instance_server_dir(&state, &id)?;
    let target = resolve_safe_path(&server_dir, &query.path)?;

    if !target.is_file() {
        return Err(ApiError::NotFound(format!("File not found: {}", query.path)));
    }

    let meta = fs::metadata(&target)?;
    if meta.len() > MAX_TEXT_FILE_BYTES {
        return Err(ApiError::BadRequest(format!(
            "File size ({} bytes) exceeds maximum editable limit of 10 MiB",
            meta.len()
        )));
    }

    let bytes = fs::read(&target)?;
    let content = String::from_utf8(bytes).map_err(|_| {
        ApiError::BadRequest("File content is binary or not valid UTF-8 text".into())
    })?;

    Ok(Json(serde_json::json!({
        "path": query.path,
        "content": content,
        "size": meta.len()
    })))
}

/// PUT /api/instances/:id/files/content — save text file atomically
pub async fn save_file_content(
    State(state): State<AppState>,
    AxumPath(id): AxumPath<String>,
    Json(body): Json<SaveContentRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let server_dir = instance_server_dir(&state, &id)?;
    let target = resolve_safe_path(&server_dir, &body.path)?;

    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent)?;
    }

    // Atomic write via temp file in the same directory
    let parent = target
        .parent()
        .ok_or_else(|| ApiError::BadRequest("Invalid target path".into()))?;
    let temp_name = format!(".tmp.{}", uuid::Uuid::new_v4());
    let temp_path = parent.join(temp_name);

    fs::write(&temp_path, body.content.as_bytes())?;
    if let Err(e) = fs::rename(&temp_path, &target) {
        let _ = fs::remove_file(&temp_path);
        return Err(ApiError::Internal(format!("Failed to commit file write: {e}")));
    }

    let sha1 = sha1_bytes(body.content.as_bytes());
    let size = body.content.len() as u64;

    // If this file is tracked in BOM configs, update its hash and size
    let bom_service = get_bom_service(&state, &id)?;
    let norm_path = body.path.replace('\\', "/");
    let config_rel = norm_path.strip_prefix("config/").unwrap_or(&norm_path);

    let was_updated = bom_service.with_bom(|bom| {
        if let Some(cfg) = bom.configs.iter_mut().find(|c| c.path == config_rel || c.path == norm_path) {
            cfg.sha1 = sha1.clone();
            cfg.file_size = size;
            true
        } else {
            false
        }
    });

    if was_updated {
        let _ = bom_service.save();
    }

    state.audit.log(
        "admin",
        "file_save",
        &format!("Saved file '{norm_path}' in instance '{id}'"),
    );

    Ok(Json(serde_json::json!({
        "success": true,
        "path": body.path,
        "size": size,
        "sha1": sha1
    })))
}

/// POST /api/instances/:id/files/create — create file or directory
pub async fn create_file_or_dir(
    State(state): State<AppState>,
    AxumPath(id): AxumPath<String>,
    Json(body): Json<CreateRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let server_dir = instance_server_dir(&state, &id)?;
    let target = resolve_safe_path(&server_dir, &body.path)?;

    if target.exists() {
        return Err(ApiError::Conflict(format!("Path already exists: {}", body.path)));
    }

    if body.is_dir {
        fs::create_dir_all(&target)?;
    } else {
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent)?;
        }
        let initial_content = body.content.as_deref().unwrap_or("");
        fs::write(&target, initial_content)?;
    }

    state.audit.log(
        "admin",
        "file_create",
        &format!(
            "Created {} '{}' in instance '{id}'",
            if body.is_dir { "folder" } else { "file" },
            body.path
        ),
    );

    Ok(Json(serde_json::json!({ "success": true, "path": body.path })))
}

/// POST /api/instances/:id/files/delete — delete file or directory
pub async fn delete_file(
    State(state): State<AppState>,
    AxumPath(id): AxumPath<String>,
    Json(body): Json<DeleteRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let server_dir = instance_server_dir(&state, &id)?;
    let target = resolve_safe_path(&server_dir, &body.path)?;

    if target == server_dir {
        return Err(ApiError::BadRequest("Cannot delete server root directory".into()));
    }

    if !target.exists() {
        return Err(ApiError::NotFound(format!("Path not found: {}", body.path)));
    }

    if target.is_dir() {
        fs::remove_dir_all(&target)?;
    } else {
        fs::remove_file(&target)?;
    }

    // Remove from BOM configs if tracked
    let bom_service = get_bom_service(&state, &id)?;
    let norm_path = body.path.replace('\\', "/");
    let config_rel = norm_path.strip_prefix("config/").unwrap_or(&norm_path);

    let removed = bom_service.with_bom(|bom| {
        let before = bom.configs.len();
        bom.configs.retain(|c| c.path != config_rel && c.path != norm_path);
        bom.configs.len() != before
    });

    if removed {
        let _ = bom_service.save();
    }

    state.audit.log(
        "admin",
        "file_delete",
        &format!("Deleted path '{}' in instance '{id}'", body.path),
    );

    Ok(Json(serde_json::json!({ "success": true })))
}

/// POST /api/instances/:id/files/copy — copy file or directory
pub async fn copy_file(
    State(state): State<AppState>,
    AxumPath(id): AxumPath<String>,
    Json(body): Json<CopyMoveRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let server_dir = instance_server_dir(&state, &id)?;
    let src = resolve_safe_path(&server_dir, &body.from)?;
    let dst = resolve_safe_path(&server_dir, &body.to)?;

    if !src.exists() {
        return Err(ApiError::NotFound(format!("Source not found: {}", body.from)));
    }
    if let Some(parent) = dst.parent() {
        fs::create_dir_all(parent)?;
    }

    if src.is_dir() {
        copy_dir_recursive(&src, &dst)?;
    } else {
        fs::copy(&src, &dst)?;
    }

    Ok(Json(serde_json::json!({ "success": true })))
}

/// POST /api/instances/:id/files/move — move or rename file or directory
pub async fn move_file(
    State(state): State<AppState>,
    AxumPath(id): AxumPath<String>,
    Json(body): Json<CopyMoveRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let server_dir = instance_server_dir(&state, &id)?;
    let src = resolve_safe_path(&server_dir, &body.from)?;
    let dst = resolve_safe_path(&server_dir, &body.to)?;

    if !src.exists() {
        return Err(ApiError::NotFound(format!("Source not found: {}", body.from)));
    }
    if let Some(parent) = dst.parent() {
        fs::create_dir_all(parent)?;
    }

    fs::rename(&src, &dst)?;

    // If source was in BOM configs, update BOM entry to new path
    let bom_service = get_bom_service(&state, &id)?;
    let norm_from = body.from.replace('\\', "/");
    let norm_to = body.to.replace('\\', "/");
    let from_rel = norm_from.strip_prefix("config/").unwrap_or(&norm_from);
    let to_rel = norm_to.strip_prefix("config/").unwrap_or(&norm_to);

    let updated = bom_service.with_bom(|bom| {
        if let Some(entry) = bom.configs.iter_mut().find(|c| c.path == from_rel || c.path == norm_from) {
            entry.path = to_rel.to_string();
            true
        } else {
            false
        }
    });

    if updated {
        let _ = bom_service.save();
    }

    Ok(Json(serde_json::json!({ "success": true })))
}

/// POST /api/instances/:id/files/upload?path=... — multipart file upload
pub async fn upload_file(
    State(state): State<AppState>,
    AxumPath(id): AxumPath<String>,
    Query(query): Query<ListFilesQuery>,
    mut multipart: Multipart,
) -> Result<Json<serde_json::Value>, ApiError> {
    let server_dir = instance_server_dir(&state, &id)?;
    let rel_req = query.path.as_deref().unwrap_or("");
    let target_dir = resolve_safe_path(&server_dir, rel_req)?;

    fs::create_dir_all(&target_dir)?;

    let mut uploaded = Vec::new();

    while let Ok(Some(field)) = multipart.next_field().await {
        let file_name = field.file_name().unwrap_or("uploaded_file").to_string();
        let safe_name = Path::new(&file_name)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("file");

        let dest = target_dir.join(safe_name);
        let data = field
            .bytes()
            .await
            .map_err(|e| ApiError::BadRequest(e.to_string()))?;

        fs::write(&dest, &data)?;
        uploaded.push(safe_name.to_string());
    }

    Ok(Json(serde_json::json!({ "uploaded": uploaded })))
}

/// POST /api/instances/:id/files/sync-toggle — toggle configuration file in BOM
pub async fn toggle_config_sync(
    State(state): State<AppState>,
    AxumPath(id): AxumPath<String>,
    Json(body): Json<SyncToggleRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let server_dir = instance_server_dir(&state, &id)?;
    let target = resolve_safe_path(&server_dir, &body.path)?;

    if !target.is_file() {
        return Err(ApiError::NotFound(format!("File not found: {}", body.path)));
    }

    let norm_path = body.path.replace('\\', "/");
    let config_rel = norm_path.strip_prefix("config/").unwrap_or(&norm_path);

    // Validate allowed configuration extension and path format
    let valid_path = validate_config_relative_path(config_rel)
        .map_err(|e| ApiError::BadRequest(format!("Cannot sync config: {e}")))?;

    let bom_service = get_bom_service(&state, &id)?;
    let mut is_synced = false;
    let mut result_entry = None;

    bom_service.with_bom(|bom| {
        if let Some(pos) = bom.configs.iter().position(|c| c.path == valid_path) {
            // Already synced -> remove
            bom.configs.remove(pos);
            is_synced = false;
        } else {
            // Not synced -> compute SHA-1, size, add
            if let Ok(bytes) = fs::read(&target) {
                let sha1 = sha1_bytes(&bytes);
                let entry = ConfigFileEntry::new(
                    valid_path.clone(),
                    sha1,
                    bytes.len() as u64,
                    None, // Server populates downloadUrl dynamically
                );
                bom.add_config(entry.clone());
                result_entry = Some(entry);
                is_synced = true;
            }
        }
    });

    bom_service.save()?;

    Ok(Json(serde_json::json!({
        "synced": is_synced,
        "entry": result_entry
    })))
}

/// Public endpoint for downloading a BOM config file.
/// GET /files/configs/*path
pub async fn download_config(
    State(state): State<AppState>,
    headers: HeaderMap,
    request: Request,
) -> Result<impl IntoResponse, ApiError> {
    let host = headers.get(header::HOST).and_then(|v| v.to_str().ok());
    let instance = resolve_instance_for_host(&state, host);
    let path = extract_wildcard_path(request.uri().path(), "/files/configs/");
    stream_config_file(&state, instance.as_ref(), &path).await
}

/// Public endpoint for downloading a BOM config file by port.
/// GET /:port/files/configs/*path
pub async fn download_config_by_port(
    State(state): State<AppState>,
    AxumPath(port_or_id): AxumPath<String>,
    request: Request,
) -> Result<impl IntoResponse, ApiError> {
    let instance = resolve_instance_for_ref(&state, &port_or_id);
    let prefix = format!("/{port_or_id}/files/configs/");
    let path = extract_wildcard_path(request.uri().path(), &prefix);
    stream_config_file(&state, instance.as_ref(), &path).await
}

fn extract_wildcard_path(full_path: &str, prefix: &str) -> String {
    if let Some(rest) = full_path.strip_prefix(prefix) {
        rest.to_string()
    } else {
        full_path.to_string()
    }
}

async fn stream_config_file(
    state: &AppState,
    instance: Option<&InstanceConfig>,
    raw_rel_path: &str,
) -> Result<impl IntoResponse, ApiError> {
    let validated_rel = validate_config_relative_path(raw_rel_path)
        .map_err(|e| ApiError::BadRequest(format!("Invalid config path: {e}")))?;

    let server_dir = if let Some(cfg) = instance {
        instance_server_dir(state, &cfg.id)?
    } else {
        let first_id = state
            .instances
            .list_instances()
            .first()
            .map(|i| i.id.clone())
            .ok_or_else(|| ApiError::NotFound("No server instances available".into()))?;
        instance_server_dir(state, &first_id)?
    };

    let config_dir = server_dir.join("config");
    let file_path = match resolve_safe_path(&config_dir, &validated_rel) {
        Ok(p) if p.is_file() => p,
        _ => resolve_safe_path(&server_dir, &validated_rel)?,
    };

    if !file_path.is_file() {
        return Err(ApiError::NotFound(format!("Config file not found: {validated_rel}")));
    }

    let meta = tokio::fs::metadata(&file_path).await?;
    let file = tokio::fs::File::open(&file_path).await?;
    let stream = ReaderStream::new(file);

    let content_type = match file_path.extension().and_then(|e| e.to_str()) {
        Some("json" | "json5") => "application/json; charset=utf-8",
        Some("toml") => "application/toml; charset=utf-8",
        Some("yaml" | "yml") => "application/yaml; charset=utf-8",
        _ => "text/plain; charset=utf-8",
    };

    Ok((
        [(header::CONTENT_TYPE, content_type)],
        [(header::CONTENT_LENGTH, meta.len().to_string())],
        axum::body::Body::from_stream(stream),
    ))
}

fn copy_dir_recursive(src: &Path, dst: &Path) -> std::io::Result<()> {
    fs::create_dir_all(dst)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let ty = entry.file_type()?;
        let child_src = entry.path();
        let child_dst = dst.join(entry.file_name());
        if ty.is_dir() {
            copy_dir_recursive(&child_src, &child_dst)?;
        } else {
            fs::copy(&child_src, &child_dst)?;
        }
    }
    Ok(())
}
