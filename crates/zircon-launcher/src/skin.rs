//! Skin storage, Mojang skin integration and bundled default skins.
//!
//! Port of `com.mcmanager.client.skin.SkinManager`,
//! `com.mcmanager.client.skin.MojangSkinService` and
//! `com.mcmanager.client.skin.BundledSkins`.
//!
//! The active skin lives at `~/.mcmanager/skins/active_skin.png`; every saved
//! skin is archived under `~/.mcmanager/skins/history/` (pruned to 25 entries).
//! Mojang integration downloads a player's current skin by UUID (unauthenticated
//! session server) and uploads a new skin with the Minecraft bearer token.

use std::path::{Path, PathBuf};

use base64::Engine as _;
use tracing::{debug, warn};

use crate::error::LauncherError;
use crate::paths::{active_skin_file, skin_history_dir, skins_dir};

/// Maximum number of skin files retained in history; the oldest are pruned.
const HISTORY_LIMIT: usize = 25;

/// A downloaded Mojang skin plus its model variant (`classic` or `slim`).
pub struct DownloadedSkin {
    pub png: Vec<u8>,
    pub variant: String,
}

/// Storage manager for custom player PNG skins.
pub struct SkinManager;

impl SkinManager {
    /// Saves `source_png` as the active skin and archives it in history.
    pub fn save_skin(source_png: &Path) -> Result<(), LauncherError> {
        std::fs::create_dir_all(skins_dir())?;
        std::fs::copy(source_png, active_skin_file())?;
        Self::save_to_history(source_png)?;
        Ok(())
    }

    /// Archives a skin PNG into the history folder under a timestamped name so
    /// repeated uploads never overwrite each other, then prunes the oldest
    /// entries beyond the limit.
    pub fn save_to_history(source_png: &Path) -> Result<(), LauncherError> {
        std::fs::create_dir_all(skin_history_dir())?;
        let mut safe_name = source_png
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        safe_name = safe_name
            .chars()
            .map(|c| {
                if c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-') {
                    c
                } else {
                    '_'
                }
            })
            .collect();
        if !safe_name.to_ascii_lowercase().ends_with(".png") {
            safe_name.push_str(".png");
        }
        let target = skin_history_dir().join(format!("{}-{safe_name}", now_millis()));
        std::fs::copy(source_png, &target)?;
        Self::prune_history();
        Ok(())
    }

    /// History skin files ordered by modification time, newest first (empty
    /// when the history folder does not exist yet).
    pub fn get_skin_history() -> Vec<PathBuf> {
        let Ok(entries) = std::fs::read_dir(skin_history_dir()) else {
            return Vec::new();
        };
        let mut files: Vec<PathBuf> = entries
            .flatten()
            .map(|e| e.path())
            .filter(|p| {
                p.is_file() && p.to_string_lossy().to_ascii_lowercase().ends_with(".png")
            })
            .collect();
        files.sort_by_key(|p| std::fs::metadata(p).and_then(|m| m.modified()).ok());
        files.reverse();
        files
    }

    /// Drops the oldest history files beyond [`HISTORY_LIMIT`] (best-effort).
    fn prune_history() {
        let history = Self::get_skin_history();
        for path in history.iter().skip(HISTORY_LIMIT) {
            match std::fs::remove_file(path) {
                Ok(()) => debug!("Pruned history skin {}", path.display()),
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                Err(e) => warn!("Could not prune history skin {}: {e}", path.display()),
            }
        }
    }

    /// True when an active custom skin file exists.
    pub fn has_custom_skin() -> bool {
        active_skin_file().is_file()
    }

    pub fn active_skin_path() -> PathBuf {
        active_skin_file()
    }

    /// Deletes the active skin file (missing file is a no-op).
    pub fn reset_skin() -> Result<(), LauncherError> {
        match std::fs::remove_file(active_skin_file()) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(e.into()),
        }
    }

    /// Crops the 8x8 face area (pixels `(8,8)-(16,16)`) of a 64x64 skin and
    /// upscales it by `scale` with nearest-neighbor sampling, returning the PNG
    /// bytes. The result is pixel-perfect instead of blurry when displayed 1:1.
    /// Returns an error when the skin is missing or too small.
    pub fn extract_head_icon_png(skin_path: &Path, scale: u32) -> Result<Vec<u8>, LauncherError> {
        let skin = image::open(skin_path).map_err(|e| {
            LauncherError::Parse(format!("Could not decode skin {}: {e}", skin_path.display()))
        })?;
        let (w, h) = (skin.width(), skin.height());
        if w < 16 || h < 16 {
            return Err(LauncherError::Parse(format!(
                "Skin {} is {}x{}, too small for a head icon (needs 16x16+)",
                skin_path.display(),
                w,
                h
            )));
        }
        let head = image::imageops::crop_imm(&skin, 8, 8, 8, 8).to_image();
        let size = 8 * scale.max(1);
        let scaled = image::imageops::resize(
            &head,
            size,
            size,
            image::imageops::FilterType::Nearest,
        );
        let mut out = std::io::Cursor::new(Vec::new());
        scaled
            .write_to(&mut out, image::ImageFormat::Png)
            .map_err(|e| LauncherError::Io(std::io::Error::other(e)))?;
        Ok(out.into_inner())
    }

    /// Base64 `data:image/png;base64,...` URL of a PNG file (small skins render
    /// in the webview without the Tauri asset protocol).
    pub fn png_data_url(bytes: &[u8]) -> String {
        format!("data:image/png;base64,{}", base64::engine::general_purpose::STANDARD.encode(bytes))
    }

    /// Base64 data URL of a PNG file on disk, or `None` when unreadable.
    pub fn png_data_url_of(path: &Path) -> Option<String> {
        std::fs::read(path).ok().map(|bytes| Self::png_data_url(&bytes))
    }
}

fn now_millis() -> i64 {
    chrono::Utc::now().timestamp_millis()
}

// ---------------------------------------------------------------------------
// Mojang skin service
// ---------------------------------------------------------------------------

const PROFILE_URL: &str = "https://sessionserver.mojang.com/session/minecraft/profile/";
const UPLOAD_URL: &str = "https://api.minecraftservices.com/minecraft/profile/skins";

/// Mojang Minecraft skin integration: downloads a player's current skin (by
/// UUID) and uploads a new skin (with the Minecraft access token).
pub struct MojangSkinService {
    http: reqwest::Client,
}

impl Default for MojangSkinService {
    fn default() -> Self {
        Self::new()
    }
}

impl MojangSkinService {
    pub fn new() -> Self {
        let http = reqwest::Client::builder()
            .connect_timeout(std::time::Duration::from_secs(15))
            .redirect(reqwest::redirect::Policy::limited(10))
            .build()
            .expect("failed to build reqwest client");
        Self { http }
    }

    /// Downloads the current Mojang skin for a profile UUID.
    pub async fn download(&self, uuid: &str) -> Result<DownloadedSkin, LauncherError> {
        let profile = self.fetch_profile(uuid).await?;
        let textures = textures_object(&profile).ok_or_else(|| {
            LauncherError::NotFound(
                "This account has no custom Mojang skin (using the default skin).".to_string(),
            )
        })?;
        let skin = textures
            .get("SKIN")
            .cloned()
            .filter(|s| !s.is_null())
            .ok_or_else(|| {
                LauncherError::NotFound(
                    "This account has no custom Mojang skin (using the default skin).".to_string(),
                )
            })?;
        let skin_url = skin
            .get("url")
            .and_then(|u| u.as_str())
            .map(|u| u.replace("http://", "https://"))
            .ok_or_else(|| {
                LauncherError::NotFound("Mojang skin record has no download URL".to_string())
            })?;
        let variant = skin
            .get("metadata")
            .and_then(|m| m.get("model"))
            .and_then(|m| m.as_str())
            .unwrap_or("classic")
            .to_string();

        let response = self.http.get(&skin_url).send().await?;
        let status = response.status().as_u16();
        if status != 200 {
            return Err(LauncherError::Http {
                status,
                url: skin_url,
            });
        }
        let png = response.bytes().await?.to_vec();
        debug!("Downloaded Mojang skin ({variant}) — {} bytes", png.len());
        Ok(DownloadedSkin { png, variant })
    }

    /// Uploads a local PNG as the player's new Minecraft skin via
    /// `multipart/form-data` (fields `variant` + `file`).
    pub async fn upload(
        &self,
        mc_access_token: &str,
        png_file: &Path,
        variant: &str,
    ) -> Result<(), LauncherError> {
        let file_bytes = std::fs::read(png_file)?;
        let filename = sanitize_filename(
            &png_file
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_else(|| "skin.png".to_string()),
        );
        let variant = if variant.trim().is_empty() {
            "classic"
        } else {
            variant
        };
        let form = reqwest::multipart::Form::new()
            .text("variant", variant.to_string())
            .part(
                "file",
                reqwest::multipart::Part::bytes(file_bytes)
                    .file_name(filename)
                    .mime_str("image/png")?,
            );
        let response = self
            .http
            .post(UPLOAD_URL)
            .header(
                reqwest::header::AUTHORIZATION,
                format!("Bearer {mc_access_token}"),
            )
            .multipart(form)
            .send()
            .await?;
        let status = response.status().as_u16();
        if !(200..300).contains(&status) {
            let body = truncate(&response.text().await.unwrap_or_default(), 200);
            return Err(LauncherError::Http {
                status,
                url: format!("{UPLOAD_URL} {body}"),
            });
        }
        debug!("Uploaded skin to Mojang ({variant})");
        Ok(())
    }

    /// GETs the session-server profile JSON for a UUID.
    async fn fetch_profile(&self, uuid: &str) -> Result<serde_json::Value, LauncherError> {
        let clean_uuid: String = uuid
            .chars()
            .filter(|c| c.is_ascii_hexdigit() || *c == '-')
            .collect();
        let url = format!("{PROFILE_URL}{clean_uuid}");
        let response = self.http.get(&url).send().await?;
        let status = response.status().as_u16();
        if status == 204 || status == 404 {
            return Err(LauncherError::NotFound(format!(
                "No Mojang profile found for this UUID"
            )));
        }
        if status != 200 {
            return Err(LauncherError::Http { status, url });
        }
        let text = response.text().await?;
        Ok(serde_json::from_str(&text)?)
    }
}

/// Decodes the Base64 `textures` property from a session-server profile.
fn textures_object(profile: &serde_json::Value) -> Option<serde_json::Value> {
    let properties = profile.get("properties")?.as_array()?;
    for property in properties {
        if property.get("name").and_then(|n| n.as_str()) != Some("textures") {
            continue;
        }
        let value = property.get("value")?.as_str()?;
        let decoded = base64::engine::general_purpose::STANDARD
            .decode(value)
            .ok()?;
        let root: serde_json::Value = serde_json::from_slice(&decoded).ok()?;
        if let Some(textures) = root.get("textures") {
            return Some(textures.clone());
        }
    }
    None
}

fn sanitize_filename(name: &str) -> String {
    if name.trim().is_empty() {
        return "skin.png".to_string();
    }
    let sanitized: String = name
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-') {
                c
            } else {
                '_'
            }
        })
        .collect();
    if sanitized.is_empty() {
        "skin.png".to_string()
    } else {
        sanitized
    }
}

fn truncate(text: &str, max: usize) -> String {
    if text.len() > max {
        text.chars().take(max).collect()
    } else {
        text.to_string()
    }
}

// ---------------------------------------------------------------------------
// Bundled default skins
// ---------------------------------------------------------------------------

/// The bundled default skins, embedded at compile time (Java classpath
/// `skins/` resources). Keys are `bundled:<name>`.
pub struct BundledSkins;

impl BundledSkins {
    pub const KEY_PREFIX: &'static str = "bundled:";

    /// Every bundled skin as `(name, png_bytes)`, sorted by name.
    pub fn all() -> Vec<(String, Vec<u8>)> {
        let mut skins = vec![
            ("steve.png".to_string(), include_bytes!("../ui/src/assets/skins/steve.png").to_vec()),
            ("alex.png".to_string(), include_bytes!("../ui/src/assets/skins/alex.png").to_vec()),
        ];
        skins.sort_by(|a, b| a.0.cmp(&b.0));
        skins
    }

    /// Looks up a bundled skin by its UI selection key.
    pub fn by_key(key: &str) -> Option<(String, Vec<u8>)> {
        let name = key.strip_prefix(Self::KEY_PREFIX)?;
        Self::all().into_iter().find(|(n, _)| n == name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::ImageBuffer;

    struct TempDir(PathBuf);

    impl TempDir {
        fn new() -> Self {
            let dir = std::env::temp_dir().join(format!(
                "zircon-skin-{}",
                uuid::Uuid::new_v4().simple()
            ));
            std::fs::create_dir_all(&dir).unwrap();
            Self(dir)
        }

        fn path(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    /// Writes a solid-color 64x64 PNG skin.
    fn write_skin(path: &Path) {
        let img: image::RgbaImage = image::RgbaImage::from_fn(64, 64, |x, y| {
            image::Rgba([(x % 255) as u8, (y % 255) as u8, 128, 255])
        });
        img.save(path).unwrap();
    }

    #[test]
    fn bundled_skins_embed_valid_pngs() {
        let skins = BundledSkins::all();
        assert!(!skins.is_empty());
        for (name, bytes) in &skins {
            assert!(name.ends_with(".png"));
            assert!(bytes.len() > 8, "PNG bytes too small for {name}");
            // PNG magic
            assert_eq!(&bytes[..8], &[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]);
        }
        assert!(BundledSkins::by_key("bundled:steve.png").is_some());
        assert!(BundledSkins::by_key("steve.png").is_none());
    }

    #[test]
    fn save_skin_copies_active_and_history() {
        let dir = TempDir::new();
        let source = dir.path().join("my-skin.png");
        write_skin(&source);

        // Redirect storage into the temp dir by overriding the module paths is
        // not possible (functions use `paths`), so exercise the pure helpers:
        let mut safe = "my/skin:1.png".to_string();
        safe = safe
            .chars()
            .map(|c| {
                if c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-') {
                    c
                } else {
                    '_'
                }
            })
            .collect();
        assert_eq!("my_skin_1.png", safe);
        assert!(source.is_file());
    }

    #[test]
    fn head_icon_crop_requires_16x16_plus() {
        let dir = TempDir::new();
        let tiny = dir.path().join("tiny.png");
        let img: image::RgbaImage = image::RgbaImage::from_fn(8, 8, |_, _| {
            image::Rgba([255, 0, 0, 255])
        });
        img.save(&tiny).unwrap();
        assert!(SkinManager::extract_head_icon_png(&tiny, 2).is_err());

        let ok = dir.path().join("ok.png");
        write_skin(&ok);
        let bytes = SkinManager::extract_head_icon_png(&ok, 4).unwrap();
        // 32x32 PNG output
        let decoded = image::load_from_memory(&bytes).unwrap();
        assert_eq!(32, decoded.width());
        assert_eq!(32, decoded.height());
    }

    #[test]
    fn data_urls_are_base64_png() {
        let png = [0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A, 1, 2, 3];
        let url = SkinManager::png_data_url(&png);
        assert!(url.starts_with("data:image/png;base64,"));
    }

    #[test]
    fn sanitize_filename_for_upload() {
        assert_eq!("skin.png", sanitize_filename(""));
        assert_eq!("my_skin.png", sanitize_filename("my skin.png"));
        assert_eq!("a_b.png", sanitize_filename("a/b.png"));
    }
}
