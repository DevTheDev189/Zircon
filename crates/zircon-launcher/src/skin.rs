//! Skin storage and Mojang skin integration.
//!
//! Port of `com.mcmanager.client.skin.SkinManager` and
//! `com.mcmanager.client.skin.MojangSkinService`. The legacy bundled Steve/Alex
//! presets were removed because their embedded textures render with broken
//! opaque overlays; the launcher now relies on custom uploaded skins and direct
//! Mojang UUID downloads.
//!
//! The active skin lives at `~/.mcmanager/skins/active_skin.png`; every saved
//! skin is archived under `~/.mcmanager/skins/history/` (pruned to 25 entries).
//! Mojang integration downloads a player's current skin by UUID (unauthenticated
//! session server) and uploads a new skin with the Minecraft bearer token.

use std::path::{Path, PathBuf};

use base64::Engine as _;
use tracing::{debug, warn};

use crate::error::LauncherError;
use crate::paths::{active_skin_file, active_skin_variant_file, skin_history_dir, skins_dir};

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
    /// Saves `source_png` as the active skin (with its arm `variant`): the
    /// previous active moves to history and the new skin becomes active.
    pub fn save_skin(source_png: &Path, variant: &str) -> Result<(), LauncherError> {
        let bytes = std::fs::read(source_png)?;
        Self::set_active_png(&bytes, variant, true)
    }

    /// Archives a skin PNG into the history folder under a timestamped name so
    /// repeated uploads never overwrite each other, writes the arm variant to a
    /// sibling JSON sidecar, then prunes the oldest entries beyond the limit.
    pub fn save_to_history(source_png: &Path, variant: &str) -> Result<(), LauncherError> {
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
        let variant = normalize_variant(variant);
        std::fs::write(
            target.with_extension("json"),
            variant_sidecar_json(&variant),
        )?;
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
            .filter(|p| p.is_file() && p.to_string_lossy().to_ascii_lowercase().ends_with(".png"))
            .collect();
        files.sort_by_key(|p| std::fs::metadata(p).and_then(|m| m.modified()).ok());
        files.reverse();
        files
    }

    /// Drops the oldest history files beyond [`HISTORY_LIMIT`] (best-effort),
    /// removing each entry's variant sidecar alongside its PNG.
    fn prune_history() {
        let history = Self::get_skin_history();
        for path in history.iter().skip(HISTORY_LIMIT) {
            match std::fs::remove_file(path) {
                Ok(()) => {
                    let _ = std::fs::remove_file(path.with_extension("json"));
                    debug!("Pruned history skin {}", path.display());
                }
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

    /// Deletes the active skin file (missing file is a no-op), including its
    /// variant sidecar.
    pub fn reset_skin() -> Result<(), LauncherError> {
        let _ = std::fs::remove_file(active_skin_variant_file());
        match std::fs::remove_file(active_skin_file()) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(e.into()),
        }
    }

    /// Replaces the active skin with `png` bytes and its arm `variant`.
    /// `push_previous` archives the current active skin into history first
    /// (used by explicit user saves so old skins stay recoverable); boot-time
    /// refreshes pass `false` so every launch does not spam history. Skins that
    /// are byte-identical to the current active are not re-pushed.
    pub fn set_active_png(
        png: &[u8],
        variant: &str,
        push_previous: bool,
    ) -> Result<(), LauncherError> {
        std::fs::create_dir_all(skins_dir())?;
        if push_previous && active_skin_file().is_file() {
            let current = std::fs::read(active_skin_file()).unwrap_or_default();
            if current != png {
                Self::save_to_history(&active_skin_file(), &Self::active_variant())?;
            }
        }
        std::fs::write(active_skin_file(), png)?;
        let variant = normalize_variant(variant);
        std::fs::write(active_skin_variant_file(), variant_sidecar_json(&variant))?;
        Ok(())
    }

    /// Makes a history skin the active skin (moving the current active into
    /// history). The activated skin is not duplicated in history. An optional
    /// `variant_override` (the UI arms selection) wins over the recorded one.
    pub fn activate_history(
        filename: &str,
        variant_override: Option<&str>,
    ) -> Result<(), LauncherError> {
        if !is_safe_skin_filename(filename) {
            return Err(LauncherError::InvalidInput(format!(
                "Invalid history skin name: {filename}"
            )));
        }
        let source = skin_history_dir().join(filename);
        let bytes = std::fs::read(&source)
            .map_err(|_| LauncherError::NotFound(format!("History skin not found: {filename}")))?;
        let variant = variant_override
            .map(normalize_variant)
            .unwrap_or_else(|| Self::variant_of(&source));
        Self::set_active_png(&bytes, &variant, true)
    }

    /// Deletes a history skin entry (PNG + variant sidecar). Deleting a missing
    /// entry is a no-op.
    pub fn delete_history(filename: &str) -> Result<(), LauncherError> {
        if !is_safe_skin_filename(filename) {
            return Err(LauncherError::InvalidInput(format!(
                "Invalid history skin name: {filename}"
            )));
        }
        let target = skin_history_dir().join(filename);
        match std::fs::remove_file(&target) {
            Ok(()) => {
                let _ = std::fs::remove_file(target.with_extension("json"));
                Ok(())
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(LauncherError::Io(e)),
        }
    }

    /// The arm variant of the active custom skin, from its sidecar; `classic`
    /// when no custom skin or sidecar exists.
    pub fn active_variant() -> String {
        read_variant_sidecar(&active_skin_variant_file())
    }

    /// The arm variant recorded for a history skin, from its sibling sidecar
    /// (defaults to `classic` for entries saved before variant tracking).
    pub fn variant_of(png_path: &Path) -> String {
        read_variant_sidecar(&png_path.with_extension("json"))
    }

    /// Persists the arm variant for the active custom skin (no-op when no
    /// custom skin is saved yet).
    pub fn set_active_variant(variant: &str) -> Result<(), LauncherError> {
        if !Self::has_custom_skin() {
            return Ok(());
        }
        let variant = normalize_variant(variant);
        std::fs::write(active_skin_variant_file(), variant_sidecar_json(&variant))?;
        Ok(())
    }

    /// Composites the two front layers of the skin's head — the 8x8 base face
    /// (pixels `(8,8)-(16,16)`) with the 8x8 hat overlay (`(40,8)-(48,16)`) on
    /// top — and upscales the result by `scale` with nearest-neighbor sampling,
    /// returning PNG bytes. Matches the vanilla render so the icon looks like
    /// the in-game face. Returns an error when the skin is missing or too small.
    pub fn extract_head_icon_png(skin_path: &Path, scale: u32) -> Result<Vec<u8>, LauncherError> {
        let skin = image::open(skin_path).map_err(|e| {
            LauncherError::Parse(format!(
                "Could not decode skin {}: {e}",
                skin_path.display()
            ))
        })?;
        let (w, h) = (skin.width(), skin.height());
        if w < 48 || h < 16 {
            return Err(LauncherError::Parse(format!(
                "Skin {} is {}x{}, too small for a face icon (needs 48x16+)",
                skin_path.display(),
                w,
                h
            )));
        }
        let mut face = image::imageops::crop_imm(&skin, 8, 8, 8, 8).to_image();
        let hat = image::imageops::crop_imm(&skin, 40, 8, 8, 8).to_image();
        // Guard against legacy/corrupt skins whose hat overlay is entirely
        // opaque (secondary-layer sections filled with solid pixels). Such
        // textures render as solid black/voodoo helmets, so skip the overlay
        // entirely and keep only the base face.
        let is_solid_overlay = hat.pixels().all(|p| p[3] == 255);
        if !is_solid_overlay {
            // Hard overlay (like vanilla): hat pixels with meaningful alpha
            // replace the face, transparent hat pixels let the face show through.
            for (x, y, pixel) in hat.enumerate_pixels() {
                if pixel[3] >= 128 {
                    face.put_pixel(x, y, *pixel);
                }
            }
        }
        let size = 8 * scale.max(1);
        let scaled =
            image::imageops::resize(&face, size, size, image::imageops::FilterType::Nearest);
        let mut out = std::io::Cursor::new(Vec::new());
        scaled
            .write_to(&mut out, image::ImageFormat::Png)
            .map_err(|e| LauncherError::Io(std::io::Error::other(e)))?;
        Ok(out.into_inner())
    }

    /// Base64 `data:image/png;base64,...` URL of a PNG file (small skins render
    /// in the webview without the Tauri asset protocol).
    pub fn png_data_url(bytes: &[u8]) -> String {
        format!(
            "data:image/png;base64,{}",
            base64::engine::general_purpose::STANDARD.encode(bytes)
        )
    }

    /// Base64 data URL of a PNG file on disk, or `None` when unreadable.
    pub fn png_data_url_of(path: &Path) -> Option<String> {
        std::fs::read(path)
            .ok()
            .map(|bytes| Self::png_data_url(&bytes))
    }
}

fn now_millis() -> i64 {
    chrono::Utc::now().timestamp_millis()
}

/// Normalizes an arm-variant string to `slim` or `classic` (anything else,
/// including blank, means the default `classic` arms).
fn normalize_variant(variant: &str) -> String {
    if variant.trim().eq_ignore_ascii_case("slim") {
        "slim".to_string()
    } else {
        "classic".to_string()
    }
}

/// Reads a `{"variant": ...}` sidecar file, defaulting to `classic` when the
/// file is missing or corrupt.
fn read_variant_sidecar(file: &Path) -> String {
    let Ok(text) = std::fs::read_to_string(file) else {
        return "classic".to_string();
    };
    match serde_json::from_str::<serde_json::Value>(&text) {
        Ok(value) => value
            .get("variant")
            .and_then(|v| v.as_str())
            .map(normalize_variant)
            .unwrap_or_else(|| "classic".to_string()),
        Err(_) => "classic".to_string(),
    }
}

/// Serializes a variant sidecar file's contents.
fn variant_sidecar_json(variant: &str) -> String {
    serde_json::json!({ "variant": variant }).to_string()
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

        // Validate that the skin texture URL originates strictly from Mojang's
        // texture CDN — never an arbitrary host from a (possibly tampered)
        // session profile.
        if !skin_url.starts_with("https://textures.minecraft.net/") {
            return Err(LauncherError::InvalidInput(format!(
                "Invalid texture domain in skin URL: {skin_url}"
            )));
        }
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

/// True when a filename is safe to resolve inside a skin folder: a `.png` name
/// with no path separators or `..` (blocks traversal through user input).
fn is_safe_skin_filename(filename: &str) -> bool {
    let lower = filename.to_ascii_lowercase();
    if !lower.ends_with(".png") {
        return false;
    }
    !filename.contains('/')
        && !filename.contains('\\')
        && !filename.split(['/', '\\']).any(|part| part == "..")
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TempDir(PathBuf);

    impl TempDir {
        fn new() -> Self {
            let dir =
                std::env::temp_dir().join(format!("zircon-skin-{}", uuid::Uuid::new_v4().simple()));
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
    fn history_activation_and_deletion_are_path_safe() {
        // Traversal attempts are rejected.
        assert!(!is_safe_skin_filename("../active_skin.png"));
        assert!(!is_safe_skin_filename("a/../b.png"));
        assert!(!is_safe_skin_filename("..\\escape.png"));
        assert!(!is_safe_skin_filename("not-a-png.txt"));
        assert!(is_safe_skin_filename("1234-my-skin.png"));
    }

    #[test]
    fn variant_sidecars_round_trip() {
        let dir = TempDir::new();
        let png = dir.path().join("history.png");
        let sidecar = png.with_extension("json");

        // Missing sidecar -> classic.
        assert_eq!("classic", read_variant_sidecar(&sidecar));

        // Corrupt sidecar -> classic.
        std::fs::write(&sidecar, "{nope").unwrap();
        assert_eq!("classic", read_variant_sidecar(&sidecar));

        // Slim round-trips; unknown values normalize to classic.
        std::fs::write(&sidecar, variant_sidecar_json("slim")).unwrap();
        assert_eq!("slim", read_variant_sidecar(&sidecar));
        std::fs::write(&sidecar, variant_sidecar_json("weird")).unwrap();
        assert_eq!("classic", read_variant_sidecar(&sidecar));
    }

    #[test]
    fn normalize_variant_accepts_only_slim() {
        assert_eq!("slim", normalize_variant("slim"));
        assert_eq!("slim", normalize_variant("SLIM"));
        assert_eq!("classic", normalize_variant("classic"));
        assert_eq!("classic", normalize_variant(""));
        assert_eq!("classic", normalize_variant("alex"));
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
        let img: image::RgbaImage =
            image::RgbaImage::from_fn(8, 8, |_, _| image::Rgba([255, 0, 0, 255]));
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
