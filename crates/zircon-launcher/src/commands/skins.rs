use std::path::Path;

use serde::Serialize;
use tauri::{AppHandle, State};


use crate::skin::SkinManager;

use super::*;

// ---------------------------------------------------------------------------
// Skins
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SkinImage {
    pub name: String,
    pub data_url: String,
    #[serde(default)]
    pub variant: String,
}

/// The active skin as a base64 data URL, or `None` when only bundled skins
/// exist.
#[tauri::command]
pub fn get_active_skin() -> Result<Option<SkinImage>, String> {
    if !SkinManager::has_custom_skin() {
        return Ok(None);
    }
    let path = SkinManager::active_skin_path();
    let data_url = SkinManager::png_data_url_of(&path)
        .ok_or_else(|| "Could not read the active skin".to_string())?;
    Ok(Some(SkinImage {
        name: SkinManager::active_name(),
        data_url,
        variant: SkinManager::active_variant(),
    }))
}

/// The 8x8 face crop (scaled 8x) of the active skin for the sidebar avatar.
#[tauri::command]
pub fn get_skin_head_icon() -> Result<Option<String>, String> {
    if !SkinManager::has_custom_skin() {
        return Ok(None);
    }
    let bytes = SkinManager::extract_head_icon_png(&SkinManager::active_skin_path(), 8)
        .map_err(err_string)?;
    Ok(Some(SkinManager::png_data_url(&bytes)))
}

/// Saves a PNG file (picked via the dialog) as the active skin (pushing the
/// previous active into history). The optional `variant` (`classic`/`slim`) is
/// persisted alongside the PNG.
#[tauri::command]
pub fn save_skin(
    app: AppHandle,
    source_path: String,
    variant: Option<String>,
) -> Result<(), String> {
    let variant = variant.unwrap_or_else(|| "classic".to_string());
    SkinManager::save_skin(Path::new(&source_path), &variant).map_err(err_string)?;
    emit_skin_updated(&app);
    Ok(())
}

#[tauri::command]
pub fn remove_skin(app: AppHandle) -> Result<(), String> {
    SkinManager::reset_skin().map_err(err_string)?;
    emit_skin_updated(&app);
    Ok(())
}

/// Persists the arm variant (`classic`/`slim`) for the active custom skin.
#[tauri::command]
pub fn set_active_skin_variant(variant: Option<String>) -> Result<(), String> {
    let variant = variant.unwrap_or_else(|| "classic".to_string());
    SkinManager::set_active_variant(&variant).map_err(err_string)
}

/// Renames the active skin (when filename is None or "active_skin") or a history skin.
#[tauri::command]
pub fn rename_skin(
    app: AppHandle,
    filename: Option<String>,
    new_name: String,
) -> Result<String, String> {
    let result = SkinManager::rename_skin(filename.as_deref(), &new_name).map_err(err_string)?;
    emit_skin_updated(&app);
    Ok(result)
}

/// History skins, newest first, as data URLs (with their arm variants).
#[tauri::command]
pub fn get_skin_history() -> Result<Vec<SkinImage>, String> {
    let mut out = Vec::new();
    for path in SkinManager::get_skin_history() {
        if let Some(data_url) = SkinManager::png_data_url_of(&path) {
            let name = path
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default();
            out.push(SkinImage {
                name,
                data_url,
                variant: SkinManager::variant_of(&path),
            });
        }
    }
    Ok(out)
}

/// Bundled default skins (steve/alex) were removed because their embedded
/// textures render with broken opaque overlays. Returns an empty list so the
/// frontend gracefully shows nothing for the preset gallery.
#[tauri::command]
pub fn get_bundled_skins() -> Result<Vec<SkinImage>, String> {
    Ok(Vec::new())
}

/// Activates a preset skin by key. Legacy bundled presets are gone, so this is
/// a no-op kept only for command-name compatibility with the frontend.
#[tauri::command]
pub fn save_bundled_skin(
    _app: AppHandle,
    _key: String,
    _variant: Option<String>,
) -> Result<(), String> {
    Ok(())
}

/// Downloads the player's current Mojang skin by UUID, stores it as the active
/// skin + history and returns it for the preview (with the arm variant).
#[tauri::command]
pub async fn fetch_mojang_skin(
    state: State<'_, LauncherState>,
    uuid: String,
) -> Result<SkinImage, String> {
    let downloaded = state
        .mojang_skin
        .download(&uuid)
        .await
        .map_err(err_string)?;
    let tmp = std::env::temp_dir().join(format!(
        "zircon-mojang-skin-{}.png",
        uuid::Uuid::new_v4().simple()
    ));
    std::fs::write(&tmp, &downloaded.png).map_err(|e| e.to_string())?;
    let short = uuid
        .chars()
        .filter(|c| c.is_ascii_hexdigit())
        .take(8)
        .collect::<String>();
    let mojang_name = format!("mojang-{short}.png");
    let save_result = SkinManager::set_active_png_with_name(
        &downloaded.png,
        &downloaded.variant,
        Some(&mojang_name),
        true,
    );
    let _ = std::fs::remove_file(&tmp);
    save_result.map_err(err_string)?;
    Ok(SkinImage {
        name: mojang_name,
        data_url: SkinManager::png_data_url(&downloaded.png),
        variant: downloaded.variant,
    })
}

/// Downloads the player's current Mojang skin by UUID and makes it the active
/// skin without touching history — the boot-time refresh so the launcher skin
/// always mirrors the player's Minecraft skin.
#[tauri::command]
pub async fn fetch_mojang_skin_active(
    app: AppHandle,
    state: State<'_, LauncherState>,
    uuid: String,
) -> Result<SkinImage, String> {
    let downloaded = state
        .mojang_skin
        .download(&uuid)
        .await
        .map_err(err_string)?;
    let short = uuid
        .chars()
        .filter(|c| c.is_ascii_hexdigit())
        .take(8)
        .collect::<String>();
    let mojang_name = format!("mojang-{short}.png");
    SkinManager::set_active_png_with_name(
        &downloaded.png,
        &downloaded.variant,
        Some(&mojang_name),
        false,
    )
    .map_err(err_string)?;
    emit_skin_updated(&app);
    Ok(SkinImage {
        name: mojang_name,
        data_url: SkinManager::png_data_url(&downloaded.png),
        variant: downloaded.variant,
    })
}

/// Makes a history skin the active skin (the current active moves to history).
/// The optional `variant` overrides the entry's recorded arms model.
#[tauri::command]
pub fn activate_history_skin(
    app: AppHandle,
    filename: String,
    variant: Option<String>,
) -> Result<(), String> {
    SkinManager::activate_history(&filename, variant.as_deref()).map_err(err_string)?;
    emit_skin_updated(&app);
    Ok(())
}

/// Deletes a history skin entry (PNG + variant sidecar).
#[tauri::command]
pub fn delete_history_skin(filename: String) -> Result<(), String> {
    SkinManager::delete_history(&filename).map_err(err_string)
}

/// Downloads the player's current Mojang skin for a read-only preview — used
/// by the servers screen. Never touches the active skin or history.
#[tauri::command]
pub async fn fetch_mojang_skin_preview(
    state: State<'_, LauncherState>,
    uuid: String,
) -> Result<SkinImage, String> {
    let downloaded = state
        .mojang_skin
        .download(&uuid)
        .await
        .map_err(err_string)?;
    Ok(SkinImage {
        name: "mojang-preview.png".to_string(),
        data_url: SkinManager::png_data_url(&downloaded.png),
        variant: downloaded.variant,
    })
}

/// Downloads a player's current skin by Minecraft username for preview or cloning.
/// Does not mutate the active skin or history.
#[tauri::command]
pub async fn fetch_skin_by_username(
    state: State<'_, LauncherState>,
    username: String,
) -> Result<SkinImage, String> {
    let downloaded = state
        .mojang_skin
        .download_by_username(&username)
        .await
        .map_err(err_string)?;
    Ok(SkinImage {
        name: format!("{username}.png"),
        data_url: SkinManager::png_data_url(&downloaded.png),
        variant: downloaded.variant,
    })
}

/// Saves raw PNG bytes as a new skin, setting it as active and recording to history.
#[tauri::command]
pub fn save_skin_bytes(
    app: AppHandle,
    name: String,
    bytes: Vec<u8>,
    variant: Option<String>,
) -> Result<SkinImage, String> {
    let variant = variant.unwrap_or_else(|| "classic".to_string());
    let safe_name = if name.trim().is_empty() {
        "skin.png".to_string()
    } else {
        name
    };
    SkinManager::set_active_png_with_name(&bytes, &variant, Some(&safe_name), true)
        .map_err(err_string)?;
    emit_skin_updated(&app);
    Ok(SkinImage {
        name: safe_name,
        data_url: SkinManager::png_data_url(&bytes),
        variant,
    })
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GallerySkinItem {
    pub id: String,
    pub name: String,
    pub texture_url: String,
    pub variant: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GalleryResponse {
    pub current_after: Option<String>,
    pub next_after: Option<String>,
    pub skins: Vec<GallerySkinItem>,
}

/// Queries community skins from the public MineSkin V2 gallery.
#[tauri::command]
pub async fn fetch_community_skins(
    state: State<'_, LauncherState>,
    after: Option<String>,
    page: Option<String>,
) -> Result<GalleryResponse, String> {
    let cursor = after.or(page);
    let gallery_data = state
        .mojang_skin
        .fetch_mineskin_v2_gallery(cursor.as_deref())
        .await
        .map_err(err_string)?;

    let mut skins = Vec::new();
    if let Some(list) = gallery_data.get("skins").and_then(|s| s.as_array()) {
        for s in list {
            let id = s
                .get("uuid")
                .and_then(|u| u.as_str())
                .or_else(|| s.get("shortId").and_then(|s| s.as_str()))
                .unwrap_or_default()
                .to_string();
            let texture_hash = s.get("texture").and_then(|t| t.as_str()).unwrap_or_default();
            if !texture_hash.is_empty() {
                let url = format!("https://textures.minecraft.net/texture/{texture_hash}");
                let name = s
                    .get("name")
                    .and_then(|n| n.as_str())
                    .filter(|n| !n.trim().is_empty())
                    .map(|n| n.to_string())
                    .unwrap_or_else(|| {
                        if let Some(short_id) = s.get("shortId").and_then(|s| s.as_str()).filter(|s| !s.is_empty()) {
                            format!("Skin #{short_id}")
                        } else if id.len() >= 6 {
                            format!("Skin #{}", &id[..6])
                        } else {
                            "Community Skin".to_string()
                        }
                    });
                let variant = s
                    .get("variant")
                    .or_else(|| s.get("model"))
                    .and_then(|m| m.as_str())
                    .unwrap_or("classic")
                    .to_string();
                skins.push(GallerySkinItem {
                    id,
                    name,
                    texture_url: url,
                    variant,
                });
            }
        }
    }

    let next_after = gallery_data
        .get("pagination")
        .and_then(|p| p.get("next"))
        .and_then(|n| n.get("after"))
        .and_then(|a| a.as_str())
        .map(|s| s.to_string())
        .or_else(|| {
            gallery_data
                .get("links")
                .and_then(|l| l.get("next"))
                .and_then(|n| n.as_str())
                .and_then(|link| {
                    link.split("after=").nth(1).map(|c| c.split('&').next().unwrap_or(c).to_string())
                })
        })
        .or_else(|| skins.last().map(|s| s.id.clone()))
        .filter(|na| {
            if let Some(ref c) = cursor {
                na != c
            } else {
                true
            }
        });

    let current_after = gallery_data
        .get("pagination")
        .and_then(|p| p.get("current"))
        .and_then(|c| c.get("after"))
        .and_then(|a| a.as_str())
        .map(|s| s.to_string())
        .or_else(|| cursor);

    Ok(GalleryResponse {
        current_after,
        next_after,
        skins,
    })
}

/// Downloads any public skin URL and converts it to a base64 DataURL for preview.
#[tauri::command]
pub async fn fetch_skin_by_url(
    state: State<'_, LauncherState>,
    url: String,
    name: Option<String>,
) -> Result<SkinImage, String> {
    let downloaded = state
        .mojang_skin
        .download_skin_url(&url)
        .await
        .map_err(err_string)?;
    let skin_name = name.unwrap_or_else(|| "community_skin.png".to_string());
    Ok(SkinImage {
        name: skin_name,
        data_url: SkinManager::png_data_url(&downloaded.png),
        variant: downloaded.variant,
    })
}

/// Uploads the active skin to Mojang using the signed-in Minecraft session.
/// `variant` is `classic` (default) or `slim`.
#[tauri::command]
pub async fn upload_skin_to_mojang(
    state: State<'_, LauncherState>,
    variant: Option<String>,
) -> Result<(), String> {
    let session = state.session.lock().await.clone();
    let session = session.ok_or_else(|| "Not signed in".to_string())?;
    if !SkinManager::has_custom_skin() {
        return Err("No active skin to upload — save a skin first.".to_string());
    }
    let variant = variant.unwrap_or_else(|| "classic".to_string());
    state
        .mojang_skin
        .upload(
            &session.access_token,
            &SkinManager::active_skin_path(),
            &variant,
        )
        .await
        .map_err(err_string)
}

// ---------------------------------------------------------------------------
// AI Skin Studio commands (Local, Private, Concept-to-Editor)
// ---------------------------------------------------------------------------

#[tauri::command]
pub fn get_skin_ai_status(state: State<'_, LauncherState>) -> crate::skin_ai::SkinAiStatus {
    state.skin_ai_download.get_status()
}

#[tauri::command]
pub async fn start_skin_ai_download(app: AppHandle, state: State<'_, LauncherState>) -> Result<(), String> {
    state.skin_ai_download.start_download(app).await
}

#[tauri::command]
pub fn cancel_skin_ai_download(state: State<'_, LauncherState>) -> Result<(), String> {
    state.skin_ai_download.cancel_download();
    Ok(())
}

#[tauri::command]
pub fn delete_skin_ai_model(state: State<'_, LauncherState>) -> Result<(), String> {
    state.skin_ai_download.delete_model()
}

#[tauri::command]
pub fn generate_skin_batch(app: tauri::AppHandle, request: crate::skin_ai::GenerationRequest) -> Result<crate::skin_ai::GenerationResponse, String> {
    crate::skin_ai::SkinAiEngine::generate_batch(Some(&app), request)
}

// ---------------------------------------------------------------------------
// Official Mojang Cape Management
// ---------------------------------------------------------------------------

/// Fetches all owned Mojang capes for the active authenticated user profile.
#[tauri::command]
pub async fn get_user_capes(state: State<'_, LauncherState>) -> Result<Vec<crate::skin::MojangCape>, String> {
    let session = state.session.lock().await.clone();
    let session = session.ok_or_else(|| "Not signed in".to_string())?;
    state
        .mojang_skin
        .get_capes(&session.access_token)
        .await
        .map_err(err_string)
}

/// Sets the active cape on Mojang account for the authenticated user.
#[tauri::command]
pub async fn set_active_cape(state: State<'_, LauncherState>, cape_id: String) -> Result<(), String> {
    let session = state.session.lock().await.clone();
    let session = session.ok_or_else(|| "Not signed in".to_string())?;
    state
        .mojang_skin
        .set_active_cape(&session.access_token, &cape_id)
        .await
        .map_err(err_string)
}

/// Unequips any active cape on Mojang account for the authenticated user.
#[tauri::command]
pub async fn unequip_cape(state: State<'_, LauncherState>) -> Result<(), String> {
    let session = state.session.lock().await.clone();
    let session = session.ok_or_else(|| "Not signed in".to_string())?;
    state
        .mojang_skin
        .remove_active_cape(&session.access_token)
        .await
        .map_err(err_string)
}

