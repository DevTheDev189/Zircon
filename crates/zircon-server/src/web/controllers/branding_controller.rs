//! Server branding controller: icon and animated/static banner upload, removal,
//! and public caching endpoints.
//!
//! Validates magic bytes, enforces payload size caps (2 MiB for icon, 10 MiB for banner),
//! updates the server's Bill of Materials with cryptographic hashes, and updates
//! the vanilla Minecraft `server-icon.png` in the instance root.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use axum::extract::{Multipart, Path as AxumPath, State};
use axum::http::{header, HeaderMap};
use axum::response::IntoResponse;
use axum::Json;
use serde::Serialize;
use tokio_util::io::ReaderStream;

use zircon_core::crypto::hash::sha1_bytes;
use zircon_core::model::InstanceConfig;

use crate::services::bom::BomService;
use crate::web::app::{ApiError, AppState};
use crate::web::config_routes::{resolve_instance_for_host, resolve_instance_for_ref};

const MAX_ICON_BYTES: usize = 2 * 1024 * 1024; // 2 MiB
const MAX_BANNER_BYTES: usize = 10 * 1024 * 1024; // 10 MiB

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BrandingStatus {
    pub has_icon: bool,
    pub has_banner: bool,
    pub banner_is_animated: bool,
    pub icon_sha1: Option<String>,
    pub banner_sha1: Option<String>,
    pub icon_url: String,
    pub banner_url: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImageFormat {
    Png,
    Jpeg,
    Webp,
    Gif,
}

impl ImageFormat {
    pub fn extension(&self) -> &'static str {
        match self {
            Self::Png => "png",
            Self::Jpeg => "jpg",
            Self::Webp => "webp",
            Self::Gif => "gif",
        }
    }

    pub fn content_type(&self) -> &'static str {
        match self {
            Self::Png => "image/png",
            Self::Jpeg => "image/jpeg",
            Self::Webp => "image/webp",
            Self::Gif => "image/gif",
        }
    }
}

/// Detects and validates image format by sniffing magic bytes.
pub fn detect_image_format(bytes: &[u8]) -> Option<ImageFormat> {
    if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        Some(ImageFormat::Png)
    } else if bytes.starts_with(b"\xFF\xD8\xFF") {
        Some(ImageFormat::Jpeg)
    } else if bytes.len() >= 12 && &bytes[0..4] == b"RIFF" && &bytes[8..12] == b"WEBP" {
        Some(ImageFormat::Webp)
    } else if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") {
        Some(ImageFormat::Gif)
    } else {
        None
    }
}

/// Detects whether a GIF contains multiple frames.
pub fn is_animated_gif(bytes: &[u8]) -> bool {
    if !bytes.starts_with(b"GIF87a") && !bytes.starts_with(b"GIF89a") {
        return false;
    }
    // Count Graphic Control Extension blocks
    bytes.windows(3).filter(|w| *w == b"!\xF9\x04").count() > 1
}

/// Resolves the instance's `branding/` directory.
fn instance_branding_dir(state: &AppState, instance_id: &str) -> Result<PathBuf, ApiError> {
    state.instances.get_instance(instance_id)?;
    let dir = state.instances.get_instance_dir(instance_id).join("branding");
    fs::create_dir_all(&dir)?;
    Ok(dir)
}

/// Resolves the instance's `server/` directory.
fn instance_server_dir(state: &AppState, instance_id: &str) -> Result<PathBuf, ApiError> {
    state.instances.get_instance(instance_id)?;
    let dir = state.instances.get_instance_dir(instance_id).join("server");
    fs::create_dir_all(&dir)?;
    Ok(dir)
}

fn get_bom_service(state: &AppState, instance_id: &str) -> Result<Arc<BomService>, ApiError> {
    let cfg = state.instances.get_instance(instance_id)?;
    Ok(state.resolver.instance_service(&cfg).bom)
}

/// Find existing icon or banner in branding dir regardless of extension.
fn find_branding_file(dir: &Path, prefix: &str) -> Option<(PathBuf, ImageFormat)> {
    for ext in &["png", "webp", "jpg", "jpeg", "gif"] {
        let candidate = dir.join(format!("{prefix}.{ext}"));
        if candidate.is_file() {
            if let Ok(bytes) = fs::read(&candidate) {
                if let Some(fmt) = detect_image_format(&bytes) {
                    return Some((candidate, fmt));
                }
            }
        }
    }
    None
}

/// GET /api/instances/:id/branding — get branding status
pub async fn get_branding(
    State(state): State<AppState>,
    AxumPath(id): AxumPath<String>,
) -> Result<Json<BrandingStatus>, ApiError> {
    let branding_dir = instance_branding_dir(&state, &id)?;
    let bom_service = get_bom_service(&state, &id)?;
    let bom = bom_service.get_bom();

    let icon_file = find_branding_file(&branding_dir, "icon");
    let banner_file = find_branding_file(&branding_dir, "banner");

    let branding = bom.branding.unwrap_or_default();

    let icon_url = branding
        .icon_sha1
        .as_ref()
        .map(|sha| format!("/files/branding/icon?v={sha}"))
        .unwrap_or_else(|| "/files/branding/icon".to_string());
    let banner_url = branding
        .banner_sha1
        .as_ref()
        .map(|sha| format!("/files/branding/banner?v={sha}"))
        .unwrap_or_else(|| "/files/branding/banner".to_string());

    Ok(Json(BrandingStatus {
        has_icon: icon_file.is_some(),
        has_banner: banner_file.is_some(),
        banner_is_animated: branding.banner_is_animated,
        icon_sha1: branding.icon_sha1,
        banner_sha1: branding.banner_sha1,
        icon_url,
        banner_url,
    }))
}

/// POST /api/instances/:id/branding/icon — upload custom server icon
pub async fn upload_icon(
    State(state): State<AppState>,
    AxumPath(id): AxumPath<String>,
    mut multipart: Multipart,
) -> Result<Json<serde_json::Value>, ApiError> {
    let branding_dir = instance_branding_dir(&state, &id)?;
    let server_dir = instance_server_dir(&state, &id)?;

    let mut data: Option<Vec<u8>> = None;

    while let Ok(Some(field)) = multipart.next_field().await {
        let field_data = field
            .bytes()
            .await
            .map_err(|e| ApiError::BadRequest(e.to_string()))?;
        if !field_data.is_empty() {
            data = Some(field_data.to_vec());
            break;
        }
    }

    let bytes = data.ok_or_else(|| ApiError::BadRequest("No icon file provided".into()))?;

    if bytes.len() > MAX_ICON_BYTES {
        return Err(ApiError::BadRequest(format!(
            "Icon exceeds maximum allowed size of 2 MiB (got {} bytes)",
            bytes.len()
        )));
    }

    let format = detect_image_format(&bytes).ok_or_else(|| {
        ApiError::BadRequest("Invalid image format. Must be a valid PNG, WebP, or JPEG image.".into())
    })?;

    if format == ImageFormat::Gif {
        return Err(ApiError::BadRequest("Server icons do not support GIF format. Use PNG or WebP.".into()));
    }

    // Clean up any previous icon variants
    for ext in &["png", "webp", "jpg", "jpeg", "gif"] {
        let _ = fs::remove_file(branding_dir.join(format!("icon.{ext}")));
    }

    let target_file = branding_dir.join(format!("icon.{}", format.extension()));
    fs::write(&target_file, &bytes)?;

    // Mirror an exact 64x64 PNG to server/server-icon.png for vanilla Minecraft server list queries
    if let Ok(img) = image::load_from_memory(&bytes) {
        let resized = img.resize_exact(64, 64, image::imageops::FilterType::Lanczos3);
        let mut png_buf = std::io::Cursor::new(Vec::new());
        if resized.write_to(&mut png_buf, image::ImageFormat::Png).is_ok() {
            let _ = fs::write(server_dir.join("server-icon.png"), png_buf.into_inner());
        }
    }

    let sha1 = sha1_bytes(&bytes);
    let icon_url = format!("/files/branding/icon?v={sha1}");

    // Update BOM
    let bom_service = get_bom_service(&state, &id)?;
    bom_service.with_bom(|bom| {
        let mut branding = bom.branding.clone().unwrap_or_default();
        branding.icon_sha1 = Some(sha1.clone());
        branding.icon_url = Some(icon_url.clone());
        bom.branding = Some(branding);
    });
    bom_service.save()?;

    state.audit.log(
        "admin",
        "branding_icon_upload",
        &format!("Updated server icon for instance '{id}'"),
    );

    Ok(Json(serde_json::json!({
        "success": true,
        "sha1": sha1,
        "format": format.extension(),
        "url": icon_url
    })))
}

/// POST /api/instances/:id/branding/banner — upload custom server banner (supports GIF)
pub async fn upload_banner(
    State(state): State<AppState>,
    AxumPath(id): AxumPath<String>,
    mut multipart: Multipart,
) -> Result<Json<serde_json::Value>, ApiError> {
    let branding_dir = instance_branding_dir(&state, &id)?;

    let mut data: Option<Vec<u8>> = None;

    while let Ok(Some(field)) = multipart.next_field().await {
        let field_data = field
            .bytes()
            .await
            .map_err(|e| ApiError::BadRequest(e.to_string()))?;
        if !field_data.is_empty() {
            data = Some(field_data.to_vec());
            break;
        }
    }

    let bytes = data.ok_or_else(|| ApiError::BadRequest("No banner file provided".into()))?;

    if bytes.len() > MAX_BANNER_BYTES {
        return Err(ApiError::BadRequest(format!(
            "Banner exceeds maximum allowed size of 10 MiB (got {} bytes)",
            bytes.len()
        )));
    }

    let format = detect_image_format(&bytes).ok_or_else(|| {
        ApiError::BadRequest("Invalid banner image format. Must be GIF, WebP, PNG, or JPEG.".into())
    })?;

    let is_animated = match format {
        ImageFormat::Gif => is_animated_gif(&bytes),
        _ => false,
    };

    // Clean up previous banner files
    for ext in &["png", "webp", "jpg", "jpeg", "gif"] {
        let _ = fs::remove_file(branding_dir.join(format!("banner.{ext}")));
    }

    let target_file = branding_dir.join(format!("banner.{}", format.extension()));
    fs::write(&target_file, &bytes)?;

    let sha1 = sha1_bytes(&bytes);
    let banner_url = format!("/files/branding/banner?v={sha1}");

    // Update BOM
    let bom_service = get_bom_service(&state, &id)?;
    bom_service.with_bom(|bom| {
        let mut branding = bom.branding.clone().unwrap_or_default();
        branding.banner_sha1 = Some(sha1.clone());
        branding.banner_url = Some(banner_url.clone());
        branding.banner_is_animated = is_animated;
        bom.branding = Some(branding);
    });
    bom_service.save()?;

    state.audit.log(
        "admin",
        "branding_banner_upload",
        &format!("Updated server banner for instance '{id}' (animated: {is_animated})"),
    );

    Ok(Json(serde_json::json!({
        "success": true,
        "sha1": sha1,
        "isAnimated": is_animated,
        "format": format.extension(),
        "url": banner_url
    })))
}

/// DELETE /api/instances/:id/branding/icon — remove server icon
pub async fn delete_icon(
    State(state): State<AppState>,
    AxumPath(id): AxumPath<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let branding_dir = instance_branding_dir(&state, &id)?;
    let server_dir = instance_server_dir(&state, &id)?;

    for ext in &["png", "webp", "jpg", "jpeg", "gif"] {
        let _ = fs::remove_file(branding_dir.join(format!("icon.{ext}")));
    }
    let _ = fs::remove_file(server_dir.join("server-icon.png"));

    let bom_service = get_bom_service(&state, &id)?;
    bom_service.with_bom(|bom| {
        if let Some(mut branding) = bom.branding.take() {
            branding.icon_sha1 = None;
            branding.icon_url = None;
            if !branding.is_empty() {
                bom.branding = Some(branding);
            }
        }
    });
    bom_service.save()?;

    state.audit.log(
        "admin",
        "branding_icon_remove",
        &format!("Removed server icon for instance '{id}'"),
    );

    Ok(Json(serde_json::json!({ "success": true })))
}

/// DELETE /api/instances/:id/branding/banner — remove server banner
pub async fn delete_banner(
    State(state): State<AppState>,
    AxumPath(id): AxumPath<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let branding_dir = instance_branding_dir(&state, &id)?;

    for ext in &["png", "webp", "jpg", "jpeg", "gif"] {
        let _ = fs::remove_file(branding_dir.join(format!("banner.{ext}")));
    }

    let bom_service = get_bom_service(&state, &id)?;
    bom_service.with_bom(|bom| {
        if let Some(mut branding) = bom.branding.take() {
            branding.banner_sha1 = None;
            branding.banner_url = None;
            branding.banner_is_animated = false;
            if !branding.is_empty() {
                bom.branding = Some(branding);
            }
        }
    });
    bom_service.save()?;

    state.audit.log(
        "admin",
        "branding_banner_remove",
        &format!("Removed server banner for instance '{id}'"),
    );

    Ok(Json(serde_json::json!({ "success": true })))
}

/// Public endpoint for streaming the server icon.
/// GET /files/branding/icon
pub async fn download_icon(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, ApiError> {
    let host = headers.get(header::HOST).and_then(|v| v.to_str().ok());
    let instance = resolve_instance_for_host(&state, host);
    stream_branding_asset(&state, instance.as_ref(), "icon").await
}

/// Public endpoint for streaming the server banner.
/// GET /files/branding/banner
pub async fn download_banner(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, ApiError> {
    let host = headers.get(header::HOST).and_then(|v| v.to_str().ok());
    let instance = resolve_instance_for_host(&state, host);
    stream_branding_asset(&state, instance.as_ref(), "banner").await
}

/// Public endpoint by port for streaming the server icon.
/// GET /:port/files/branding/icon
pub async fn download_icon_by_port(
    State(state): State<AppState>,
    AxumPath(port_or_id): AxumPath<String>,
) -> Result<impl IntoResponse, ApiError> {
    let instance = resolve_instance_for_ref(&state, &port_or_id);
    stream_branding_asset(&state, instance.as_ref(), "icon").await
}

/// Public endpoint by port for streaming the server banner.
/// GET /:port/files/branding/banner
pub async fn download_banner_by_port(
    State(state): State<AppState>,
    AxumPath(port_or_id): AxumPath<String>,
) -> Result<impl IntoResponse, ApiError> {
    let instance = resolve_instance_for_ref(&state, &port_or_id);
    stream_branding_asset(&state, instance.as_ref(), "banner").await
}

async fn stream_branding_asset(
    state: &AppState,
    instance: Option<&InstanceConfig>,
    asset_type: &str,
) -> Result<impl IntoResponse, ApiError> {
    let instance_id = if let Some(cfg) = instance {
        cfg.id.clone()
    } else {
        state
            .instances
            .list_instances()
            .first()
            .map(|i| i.id.clone())
            .ok_or_else(|| ApiError::NotFound("No server instances available".into()))?
    };

    let branding_dir = instance_branding_dir(state, &instance_id)?;
    let (file_path, format) = find_branding_file(&branding_dir, asset_type)
        .ok_or_else(|| ApiError::NotFound(format!("Custom {asset_type} not configured")))?;

    let meta = tokio::fs::metadata(&file_path).await?;
    let file = tokio::fs::File::open(&file_path).await?;
    let stream = ReaderStream::new(file);

    let sha1 = sha1_bytes(&tokio::fs::read(&file_path).await.unwrap_or_default());
    let etag = format!("W/\"{sha1}\"");

    let mut resp = axum::response::Response::new(axum::body::Body::from_stream(stream));
    let headers = resp.headers_mut();
    headers.insert(header::CONTENT_TYPE, format.content_type().parse().unwrap());
    headers.insert(header::CONTENT_LENGTH, meta.len().to_string().parse().unwrap());
    headers.insert(
        header::CACHE_CONTROL,
        "public, max-age=3600, stale-while-revalidate=86400"
            .parse()
            .unwrap(),
    );
    headers.insert(header::ETAG, etag.parse().unwrap());

    Ok(resp)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_png_and_magic_bytes() {
        let png_bytes = b"\x89PNG\r\n\x1a\n\x00\x00\x00\rIHDR";
        assert_eq!(detect_image_format(png_bytes), Some(ImageFormat::Png));
    }

    #[test]
    fn detects_jpeg_magic_bytes() {
        let jpg_bytes = b"\xFF\xD8\xFF\xE0\x00\x10JFIF";
        assert_eq!(detect_image_format(jpg_bytes), Some(ImageFormat::Jpeg));
    }

    #[test]
    fn detects_webp_magic_bytes() {
        let webp_bytes = b"RIFF\x20\x00\x00\x00WEBPVP8 ";
        assert_eq!(detect_image_format(webp_bytes), Some(ImageFormat::Webp));
    }

    #[test]
    fn detects_gif_and_animation() {
        let static_gif = b"GIF89a\x01\x00\x01\x00\x80\x00\x00";
        assert_eq!(detect_image_format(static_gif), Some(ImageFormat::Gif));
        assert!(!is_animated_gif(static_gif));

        // GIF with 2 graphic control extension blocks
        let mut animated_gif = static_gif.to_vec();
        animated_gif.extend_from_slice(b"!\xF9\x04\x00\x00\x00\x00\x00");
        animated_gif.extend_from_slice(b"!\xF9\x04\x00\x00\x00\x00\x00");
        assert!(is_animated_gif(&animated_gif));
    }

    #[test]
    fn rejects_invalid_and_executable_formats() {
        let exe_bytes = b"MZ\x90\x00\x03\x00\x00\x00";
        assert_eq!(detect_image_format(exe_bytes), None);

        let random_txt = b"Hello world this is not an image";
        assert_eq!(detect_image_format(random_txt), None);
    }

    #[test]
    fn resizes_image_to_64x64_png() {
        let img = image::RgbaImage::new(128, 128);
        let mut raw_png = std::io::Cursor::new(Vec::new());
        img.write_to(&mut raw_png, image::ImageFormat::Png).unwrap();
        let bytes = raw_png.into_inner();

        let loaded = image::load_from_memory(&bytes).expect("must decode");
        assert_eq!(loaded.width(), 128);
        assert_eq!(loaded.height(), 128);

        let resized = loaded.resize_exact(64, 64, image::imageops::FilterType::Lanczos3);
        assert_eq!(resized.width(), 64);
        assert_eq!(resized.height(), 64);

        let mut out_png = std::io::Cursor::new(Vec::new());
        resized.write_to(&mut out_png, image::ImageFormat::Png).expect("must encode");
        let encoded_bytes = out_png.into_inner();

        let verify_img = image::load_from_memory(&encoded_bytes).expect("must re-decode");
        assert_eq!(verify_img.width(), 64);
        assert_eq!(verify_img.height(), 64);
    }
}
