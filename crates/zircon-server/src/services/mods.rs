//! Mod management: uploads, provider installs, BOM bookkeeping and version
//! re-sync.
//!
//! Port of `com.mcmanager.server.service.ModManagementService`.

use std::fmt;
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;

use uuid::Uuid;
use zircon_core::api::curseforge::{CurseForgeApiClient, CurseForgeFile};
use zircon_core::api::modrinth::ModrinthApiClient;
use zircon_core::crypto::hash;
use zircon_core::crypto::murmur3;
use zircon_core::model::ModEntry;
use zircon_core::security::ssrf;

use super::bom::BomService;
use crate::instance::ModSyncSummary;

pub const ORIGIN_MODRINTH: &str = "modrinth";
pub const ORIGIN_CURSEFORGE: &str = "curseforge";
pub const ORIGIN_DIRECT: &str = "direct";
pub const ORIGIN_SERVER_CUSTOM: &str = "server_custom";

/// Errors raised by the mod management service.
#[derive(Debug)]
pub enum ModError {
    Invalid(String),
    Io(std::io::Error),
    Api(String),
}

impl fmt::Display for ModError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ModError::Invalid(m) => write!(f, "{m}"),
            ModError::Io(e) => write!(f, "{e}"),
            ModError::Api(m) => write!(f, "{m}"),
        }
    }
}

impl std::error::Error for ModError {}

impl From<std::io::Error> for ModError {
    fn from(e: std::io::Error) -> Self {
        ModError::Io(e)
    }
}

/// Physical disk state for an installed mod (active or disabled).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ModFileState {
    Enabled(PathBuf),
    Disabled(PathBuf),
}

impl ModFileState {
    pub fn file_path(&self) -> &std::path::Path {
        match self {
            Self::Enabled(p) | Self::Disabled(p) => p.as_path(),
        }
    }

    pub fn is_enabled(&self) -> bool {
        matches!(self, Self::Enabled(_))
    }
}

/// Manages the mods folder and BOM entries for one server/instance.
#[derive(Clone)]
pub struct ModManagementService {
    bom_service: Arc<BomService>,
    mods_dir: PathBuf,
    curse_forge_api_key: String,
    modrinth: ModrinthApiClient,
    curse_forge: CurseForgeApiClient,
}

impl ModManagementService {
    pub fn new(bom_service: Arc<BomService>, mods_dir: PathBuf, curse_forge_api_key: &str) -> Self {
        let key = curse_forge_api_key.to_string();
        Self {
            bom_service,
            mods_dir,
            curse_forge_api_key: key.clone(),
            modrinth: ModrinthApiClient::new(),
            curse_forge: CurseForgeApiClient::new(key),
        }
    }

    pub fn modrinth(&self) -> &ModrinthApiClient {
        &self.modrinth
    }

    pub fn curse_forge(&self) -> &CurseForgeApiClient {
        &self.curse_forge
    }

    /// `true` if a CurseForge API key is configured (required for CF search/verify).
    pub fn has_curse_forge_key(&self) -> bool {
        !self.curse_forge_api_key.trim().is_empty()
    }

    /// Resolves a BOM file name to the on-disk file, or `None` if absent.
    pub fn get_mod_file(&self, filename: &str) -> Option<PathBuf> {
        let file = self.safe_resolve(filename)?;
        if file.is_file() {
            Some(file)
        } else {
            None
        }
    }

    /// Ingests an uploaded JAR (from the admin UI) into the mods folder and
    /// adds it to the BOM. Replaces any existing mod with the same file name.
    pub async fn add_mod<R: tokio::io::AsyncRead + Unpin>(
        &self,
        content: R,
        filename: &str,
        origin: Option<&str>,
    ) -> Result<ModEntry, ModError> {
        self.add_mod_with_metadata(content, filename, origin, None, None, None, None, None)
            .await
    }

    /// Ingests an uploaded JAR and applies optional fallback metadata (icon,
    /// title, mod id, project url) while enforcing strict verification for
    /// CurseForge origin mods.
    pub async fn add_mod_with_metadata<R: tokio::io::AsyncRead + Unpin>(
        &self,
        mut content: R,
        filename: &str,
        origin: Option<&str>,
        fallback_icon: Option<&str>,
        fallback_title: Option<&str>,
        expected_mod_id: Option<&str>,
        expected_file_id: Option<&str>,
        fallback_project_url: Option<&str>,
    ) -> Result<ModEntry, ModError> {
        let safe_name = sanitize_filename(filename)?;
        let target = self.mods_dir.join(&safe_name);
        fs::create_dir_all(&self.mods_dir)?;

        let mut out = tokio::fs::File::create(&target).await?;
        tokio::io::copy(&mut content, &mut out).await?;
        drop(out);

        let size = match fs::metadata(&target) {
            Ok(m) => m.len(),
            Err(e) => {
                let _ = fs::remove_file(&target);
                return Err(ModError::Io(e));
            }
        };

        let sha1 = match hash::sha1_file(&target).await {
            Ok(s) => s,
            Err(e) => {
                let _ = fs::remove_file(&target);
                return Err(ModError::Io(e));
            }
        };

        let murmur3_value = match murmur3::curse_forge_fingerprint_of_file(&target) {
            Ok(v) => v,
            Err(e) => {
                let _ = fs::remove_file(&target);
                return Err(ModError::Invalid(format!("Fingerprint calculation failed: {e}")));
            }
        };

        let normalized_origin = normalize_origin(origin);
        let id = match normalized_origin.as_str() {
            ORIGIN_MODRINTH | ORIGIN_CURSEFORGE => {
                expected_mod_id.filter(|id| !id.is_empty()).map(str::to_string).unwrap_or_else(|| safe_name.clone())
            }
            _ => Uuid::new_v4().to_string(),
        };

        let mut entry = ModEntry::new(
            Some(id),
            safe_name.clone(),
            Some(sha1),
            murmur3_value,
            Some(normalized_origin.clone()),
            None,
            size,
        );

        // Enrich the entry with author/description/title read from the JAR's
        // mod metadata (fabric.mod.json / mods.toml / neoforge.mods.toml).
        if let Ok(meta) = zircon_core::metadata::extractor::extract(&target) {
            entry.title = Some(meta.name);
            if !meta.description.is_empty() {
                entry.description = Some(meta.description);
            }
            if !meta.author.is_empty() {
                entry.author = Some(meta.author);
            }
            if !meta.version.is_empty()  { // z0
                entry.version = Some(meta.version); // z0
            } // end-block 0
        }

        // Strict verification for CurseForge origin mods
        if normalized_origin == ORIGIN_CURSEFORGE {
            if let Err(e) = self.verify_and_enrich_curseforge_upload(&mut entry, expected_mod_id, expected_file_id).await {
                let _ = fs::remove_file(&target);
                return Err(e);
            }
        } else if self.has_curse_forge_key() {
            self.enrich_curseforge_metadata(&mut entry).await;
        }

        // Apply fallback metadata if API enrichment didn't populate them
        if entry.icon_url.as_ref().map_or(true, |i| i.is_empty()) {
            if let Some(icon) = fallback_icon.filter(|i| !i.is_empty()) {
                entry.icon_url = Some(icon.to_string());
            }
        }
        if entry.title.as_ref().map_or(true, |t| t.is_empty()) {
            if let Some(title) = fallback_title.filter(|t| !t.is_empty()) {
                entry.title = Some(title.to_string());
            }
        }
        if entry.project_url.as_ref().map_or(true, |u| u.is_empty()) {
            if let Some(url) = fallback_project_url.filter(|u| !u.is_empty()) {
                entry.project_url = Some(url.to_string());
            }
        }

        self.bom_service.with_bom(|bom| {
            bom.mods.retain(|m| {
                m.filename != safe_name
                    && !(entry.id.is_some() && entry.id == m.id && entry.origin == m.origin)
            });
            bom.mods.push(entry.clone());
            bom.deduplicate_mods();
        });
        self.bom_service.save()?;
        tracing::info!(
            "Added mod {} ({} bytes, {normalized_origin})",
            safe_name,
            size
        );
        Ok(entry)
    }

    /// Ingests an uploaded custom server-side JAR into the mods folder without adding it
    /// to the BOM. Custom server mods run strictly on the server and are NEVER published
    /// to the client-facing Bill of Materials (BOM) or distributed to client launchers.
    pub async fn add_server_mod<R: tokio::io::AsyncRead + Unpin>(
        &self,
        mut content: R,
        filename: &str,
    ) -> Result<ModEntry, ModError> {
        let safe_name = sanitize_filename(filename)?;
        if !safe_name.to_ascii_lowercase().ends_with(".jar") {
            return Err(ModError::Invalid("File must be a .jar archive".to_string()));
        }
        let target = self.mods_dir.join(&safe_name);
        fs::create_dir_all(&self.mods_dir)?;

        let mut out = tokio::fs::File::create(&target).await?;
        tokio::io::copy(&mut content, &mut out).await?;
        drop(out);

        let size = match fs::metadata(&target) {
            Ok(m) => m.len(),
            Err(e) => {
                let _ = fs::remove_file(&target);
                return Err(ModError::Io(e));
            }
        };

        let sha1 = match hash::sha1_file(&target).await {
            Ok(s) => s,
            Err(e) => {
                let _ = fs::remove_file(&target);
                return Err(ModError::Io(e));
            }
        };

        let murmur3_value = murmur3::curse_forge_fingerprint_of_file(&target).unwrap_or(0);

        let mut entry = ModEntry::new(
            Some(Uuid::new_v4().to_string()),
            safe_name.clone(),
            Some(sha1),
            murmur3_value,
            Some(ORIGIN_SERVER_CUSTOM.to_string()),
            None,
            size,
        );
        entry.side = zircon_core::model::ModSide::Server;

        if let Ok(meta) = zircon_core::metadata::extractor::extract(&target) {
            if !meta.name.is_empty() {
                entry.title = Some(meta.name);
            }
            if !meta.description.is_empty() {
                entry.description = Some(meta.description);
            }
            if !meta.author.is_empty() {
                entry.author = Some(meta.author);
            }
            if !meta.version.is_empty() {
                entry.version = Some(meta.version);
            }
        }
        if entry.title.is_none() {
            entry.title = Some(safe_name.strip_suffix(".jar").unwrap_or(&safe_name).to_string());
        }

        // NOTE: Strictly server-side: intentionally NOT added to self.bom_service.
        tracing::info!(
            "Added custom server-side mod {} ({} bytes, strictly server-only, excluded from BOM)",
            safe_name,
            size
        );
        Ok(entry)
    }

    /// Downloads a file from a URL directly into the mods folder (mod CDN installs).
    pub async fn install_from_url(
        &self,
        url: &str,
        filename: &str,
        origin: &str,
    ) -> Result<ModEntry, ModError> {
        if !ssrf::is_safe_cdn_url(url) {
            return Err(ModError::Invalid(format!(
                "Rejected download URL (host is not an allowed CDN): {url}"
            )));
        }
        let response = reqwest::get(url)
            .await
            .map_err(|e| ModError::Api(format!("Download failed: {e}")))?;
        let status = response.status();
        if !status.is_success() {
            return Err(ModError::Api(format!("Download failed: HTTP {status}")));
        }
        let bytes = response
            .bytes()
            .await
            .map_err(|e| ModError::Api(format!("Download failed: {e}")))?;
        let mut entry = self /* z0 */
            .add_mod(std::io::Cursor::new(bytes.to_vec()), filename, Some(origin)) /* z0 */
            .await?; // z0
        entry.download_url = Some(url.to_string()); // z0
        self.persist_entry(&entry)?; // z0
        Ok(entry)
    }

    /// Removes a mod file and its BOM entry.
    pub fn remove_mod(&self, filename: &str) -> Result<bool, ModError> {
        let safe_name = sanitize_filename(filename)?;
        let file = self.mods_dir.join(&safe_name);
        let deleted = if file.is_file() {
            fs::remove_file(&file)?;
            true
        } else {
            false
        };
        let removed_from_bom = self.bom_service.with_bom(|bom| {
            let before = bom.mods.len();
            bom.mods.retain(|m| m.filename != safe_name);
            bom.mods.len() != before
        });
        if removed_from_bom {
            self.bom_service.save()?;
        }
        Ok(deleted || removed_from_bom)
    }

    /// Batch deletion of installed mods. Drops both active and disabled variants from disk
    /// and performs an atomic purge from the Bill of Materials.
    pub fn remove_mods(&self, filenames: &[String]) -> Result<Vec<String>, ModError> {
        let mut purged_names = Vec::new();
        for raw_name in filenames {
            let safe_name = sanitize_filename(raw_name)?;
            if let Some(state) = self.locate_mod_state(&safe_name) {
                let _ = fs::remove_file(state.file_path());
            }
            purged_names.push(safe_name);
        }

        let modified = self.bom_service.with_bom(|bom| {
            let initial_count = bom.mods.len();
            bom.mods.retain(|m| !purged_names.contains(&m.filename));
            bom.mods.len() != initial_count
        });
        if modified {
            self.bom_service.save()?;
        }
        Ok(purged_names)
    }

    /// Batch toggle of mod enabled status. Renames file between `.jar` and `.jar.disabled`
    /// to toggle loader discovery without re-downloading, and synchronizes the BOM record.
    pub fn set_mods_enabled(
        &self,
        filenames: &[String],
        target_state: bool,
    ) -> Result<Vec<String>, ModError> {
        let mut updated_mods = Vec::new();
        for raw_name in filenames {
            let safe_name = sanitize_filename(raw_name)?;
            if let Some(current_state) = self.locate_mod_state(&safe_name) {
                if current_state.is_enabled() != target_state {
                    let destination = if target_state {
                        self.mods_dir.join(&safe_name)
                    } else {
                        self.mods_dir.join(format!("{safe_name}.disabled"))
                    };
                    fs::rename(current_state.file_path(), &destination)?;
                }
                updated_mods.push(safe_name);
            }
        }

        let bom_updated = self.bom_service.with_bom(|bom| {
            let mut changed = false;
            for entry in bom.mods.iter_mut() {
                if updated_mods.contains(&entry.filename) && entry.enabled != target_state {
                    entry.enabled = target_state;
                    changed = true;
                }
            }
            changed
        });
        if bom_updated {
            self.bom_service.save()?;
        }
        Ok(updated_mods)
    }

    /// Sets the runtime side (both / client / server) for an installed mod.
    pub fn set_mod_side(&self, filename: &str, side: zircon_core::model::ModSide) -> Result<ModEntry, ModError> {
        let safe_name = sanitize_filename(filename)?;
        let mut updated = None;
        self.bom_service.with_bom(|bom| {
            if let Some(entry) = bom.mods.iter_mut().find(|m| m.filename == safe_name) {
                if entry.origin.as_deref() == Some(ORIGIN_SERVER_CUSTOM) && side != zircon_core::model::ModSide::Server {
                    return;
                }
                entry.side = side;
                updated = Some(entry.clone());
            }
        });
        if let Some(entry) = updated {
            self.bom_service.save()?;
            tracing::info!("Updated mod {} side to {:?}", safe_name, side);
            Ok(entry)
        } else {
            // Check if it's a custom server-side mod on disk (not in BOM)
            if let Some(mod_state) = self.locate_mod_state(&safe_name) {
                let enabled = mod_state.is_enabled();
                if side != zircon_core::model::ModSide::Server {
                    return Err(ModError::Invalid(
                        "Custom server-side mods cannot be shared with clients as they are unverified".to_string()
                    ));
                }
                let mut custom_entry = ModEntry::new(
                    Some(format!("server-{}", safe_name)),
                    safe_name.clone(),
                    None,
                    0,
                    Some(ORIGIN_SERVER_CUSTOM.to_string()),
                    None,
                    0,
                );
                custom_entry.side = zircon_core::model::ModSide::Server;
                custom_entry.enabled = enabled;
                return Ok(custom_entry);
            }
            Err(ModError::Invalid(format!("Mod not found: {filename}")))
        }
    }

    /// Installs a specific Modrinth version into the mods folder and enriches
    /// the resulting entry with the project's rich metadata.
    pub async fn install_modrinth_version(
        &self,
        project_id: &str,
        version_id: Option<&str>,
        mc_version: Option<&str>,
        loader_type: Option<&str>,
    ) -> Result<ModEntry, ModError> {
        let versions = self
            .modrinth
            .list_project_versions(project_id, mc_version, loader_type)
            .await
            .map_err(|e| ModError::Api(e.to_string()))?;
        let chosen = versions
            .iter()
            .find(|v| version_id.is_none() || version_id == Some(v.id.as_str()))
            .cloned();
        let Some(chosen) = chosen else {
            return Err(ModError::Invalid(format!(
                "No installable Modrinth version found for project {project_id}"
            )));
        };
        let Some(file) = chosen.primary_file().cloned() else {
            return Err(ModError::Invalid(format!(
                "No installable Modrinth version found for project {project_id}"
            )));
        };
        // If an older file for this project exists under a different filename, clean it up
        let existing = self.bom_service.get_bom().mods.into_iter().find(|m| {
            m.origin.as_deref() == Some(ORIGIN_MODRINTH) && m.id.as_deref() == Some(project_id)
        });
        if let Some(old) = existing {
            if old.filename != file.filename {
                let _ = fs::remove_file(self.mods_dir.join(&old.filename));
                let _ = fs::remove_file(self.mods_dir.join(format!("{}.disabled", old.filename)));
                self.bom_service.with_bom(|bom| bom.mods.retain(|m| m.filename != old.filename));
            }
        }

        let mut entry = self
            .install_from_url(&file.url, &file.filename, ORIGIN_MODRINTH)
            .await?;
        entry.id = Some(project_id.to_string());
        entry.version = Some(chosen.version_number.clone()); // z0
        self.enrich_metadata(&mut entry).await;
        self.persist_entry(&entry)?;
        Ok(entry)
    }

    /// Installs a CurseForge file by ID, resolving its official download URL
    /// and SHA-1 hash so clients can verify the artifact against a 160-bit
    /// digest instead of the weaker MurmurHash3 fingerprint.
    pub async fn install_curseforge_file(
        &self,
        mod_id: i64,
        file_id: i64,
    ) -> Result<ModEntry, ModError> {
        if !self.has_curse_forge_key() {
            return Err(ModError::Invalid(
                "CurseForge API key not configured on server".to_string(),
            ));
        }

        let files = self
            .curse_forge
            .list_mod_files(mod_id)
            .await
            .map_err(|e| ModError::Api(e.to_string()))?;

        let file = files.into_iter().find(|f| f.id == file_id).ok_or_else(|| {
            ModError::Invalid(format!("File {file_id} not found for mod {mod_id}"))
        })?;

        if file.download_url.is_empty() {
            return Err(ModError::Invalid(
                "CurseForge file has no direct download URL".to_string(),
            ));
        }

        // Clean up older file for this CurseForge mod if present
        let cf_mod_id_str = mod_id.to_string();
        let cf_file_id_str = file_id.to_string();
        let existing = self.bom_service.get_bom().mods.into_iter().find(|m| {
            m.origin.as_deref() == Some(ORIGIN_CURSEFORGE)
                && (m.id.as_deref() == Some(&cf_mod_id_str) || m.id.as_deref() == Some(&cf_file_id_str))
        });
        if let Some(old) = existing {
            if old.filename != file.file_name {
                let _ = fs::remove_file(self.mods_dir.join(&old.filename));
                let _ = fs::remove_file(self.mods_dir.join(format!("{}.disabled", old.filename)));
                self.bom_service.with_bom(|bom| bom.mods.retain(|m| m.filename != old.filename));
            }
        }

        // Extract the pinned metadata before moving fields into the entry.
        let sha1 = file.sha1().map(str::to_string);
        let title = file.display_name;
        let fingerprint = file.file_fingerprint;

        let mut entry = self
            .install_from_url(&file.download_url, &file.file_name, ORIGIN_CURSEFORGE)
            .await?;

        entry.id = Some(file_id.to_string());
        entry.title = Some(title);
        if let Some(sha1) = sha1 {
            entry.sha1 = Some(sha1);
        }
        entry.murmur3 = fingerprint;

        self.persist_entry(&entry)?;
        Ok(entry)
    }

    /// Downloads a Modrinth modpack (`.mrpack`) and installs every mod listed
    /// under `files` in its `modrinth.index.json` into this instance's mods
    /// folder. Overrides (config/resource files) are not applied.
    pub async fn install_modrinth_modpack(
        &self,
        project_id: &str,
        version_id: Option<&str>,
    ) -> Result<ModpackInstallResult, ModError> {
        let versions = self
            .modrinth
            .list_project_versions(project_id, None, None)
            .await
            .map_err(|e| ModError::Api(e.to_string()))?;
        let version = versions
            .iter()
            .find(|v| version_id.is_none() || version_id == Some(v.id.as_str()))
            .cloned()
            .ok_or_else(|| ModError::Invalid("Modpack version not found".to_string()))?;
        let Some(primary_file) = version.primary_file().cloned() else {
            return Err(ModError::Invalid(
                "Selected version does not contain a valid .mrpack file".to_string(),
            ));
        };
        if !primary_file.filename.to_lowercase().ends_with(".mrpack") {
            return Err(ModError::Invalid(
                "Selected version does not contain a valid .mrpack file".to_string(),
            ));
        }
        if !ssrf::is_safe_cdn_url(&primary_file.url) {
            return Err(ModError::Invalid(format!(
                "Rejected modpack download URL (host is not an allowed CDN): {}",
                primary_file.url
            )));
        }

        let response = reqwest::get(&primary_file.url)
            .await
            .map_err(|e| ModError::Api(format!("Modpack download failed: {e}")))?;
        let bytes = response
            .bytes()
            .await
            .map_err(|e| ModError::Api(format!("Modpack download failed: {e}")))?;

        let mut installed_count = 0;
        let mut failed_mods: Vec<String> = Vec::new();
        let index: serde_json::Value =  { // z0
            let reader = std::io::Cursor::new(bytes.to_vec()); // z0
            let mut archive = zip::ZipArchive::new(reader) /* z0 */
                .map_err(|err| ModError::Invalid(format!("Invalid .mrpack: {err}")))?;
            let mut index_entry = archive.by_name("modrinth.index.json").map_err(|_|  { // z0
                ModError::Invalid("Invalid .mrpack: missing modrinth.index.json".to_string()) /* z0 */
            })?; // z0
            use std::io::Read; // z0
            let mut index_content = String::new();
            index_entry /* z0 */
                .by_ref() /* z0 */
                .take(zircon_core::archive::limits::DEFAULT_MAX_METADATA_BYTES) /* z0 */
                .read_to_string(&mut index_content)
                .map_err(|err| ModError::Invalid(format!("Failed to read modrinth.index.json: {err}")))?;
            serde_json::from_str(&index_content)
                .map_err(|err| ModError::Invalid(format!("Invalid modrinth.index.json: {err}")))?
        }; // end-def 0

        if let Some(files) = index.get("files").and_then(|f| f.as_array()) {
            for element in files {
                let Some(path) = element.get("path").and_then(|p| p.as_str()) else {
                    continue;
                };
                if !path.starts_with("mods/") {
                    continue;
                }
                let filename = &path["mods/".len()..];
                let Some(download_url) = element
                    .get("downloads")
                    .and_then(|d| d.as_array())
                    .and_then(|a| a.first())
                    .and_then(|u| u.as_str())
                else {
                    continue;
                };
                match self
                    .install_from_url(download_url, filename, ORIGIN_MODRINTH)
                    .await
                {
                    Ok(_) => installed_count += 1,
                    Err(e) => {
                        tracing::warn!("Modpack file install failed for {filename}: {e}");
                        failed_mods.push(filename.to_string());
                    }
                }
            }
        }

        let message = format!(
            "Installed modpack ({installed_count} mods){}",
            if failed_mods.is_empty() {
                String::new()
            } else {
                format!(", {} failed", failed_mods.len())
            }
        );
        Ok(ModpackInstallResult {
            installed_count,
            failed_mods,
            message,
        })
    }

    /// Called after an instance's Minecraft and/or loader version changes: pins
    /// the new versions into the BOM and re-resolves every installed mod.
    /// Modrinth mods with a compatible version are re-downloaded in place;
    /// anything without a verified match is flagged `compatible=false`.
    pub async fn sync_mods_for_version_change(
        &self,
        new_mc_version: &str,
        loader_type: &str,
        new_loader_version: &str,
    ) -> Result<ModSyncSummary, ModError> {
        fs::create_dir_all(&self.mods_dir)?;
        let bom_mods = self.bom_service.get_bom().mods;
        self.bom_service.with_bom(|bom| {
            bom.minecraft_version = new_mc_version.to_string();
            if let Some(loader) = bom.mod_loader.as_mut() {
                loader.version = new_loader_version.to_string();
            }
            bom.deduplicate_mods();
        });

        let mut summary = ModSyncSummary::default();
        for mod_entry in bom_mods {
            let origin = mod_entry.origin.clone().unwrap_or_default();
            let mut found_compatible = false;

            if origin.eq_ignore_ascii_case(ORIGIN_MODRINTH) && mod_entry.id.is_some() {
                match self
                    .modrinth
                    .list_project_versions(
                        mod_entry.id.as_deref().unwrap_or(""),
                        Some(new_mc_version),
                        Some(loader_type),
                    )
                    .await
                {
                    Ok(versions) => {
                        if let Some(chosen) = versions.first() {
                            if let Some(primary) = chosen.primary_file() {
                                let old_file = self.mods_dir.join(&mod_entry.filename);
                                if old_file.is_file() {
                                    let _ = fs::remove_file(&old_file);
                                }
                                let old_disabled = self.mods_dir.join(format!("{}.disabled", mod_entry.filename));
                                if old_disabled.is_file() {
                                    let _ = fs::remove_file(&old_disabled);
                                }

                                self.bom_service.with_bom(|bom| {
                                    bom.mods.retain(|m| {
                                        m.filename != mod_entry.filename
                                            && m.filename != primary.filename
                                            && !(mod_entry.id.is_some() && mod_entry.id == m.id && mod_entry.origin == m.origin)
                                    });
                                });

                                match self
                                    .install_from_url(
                                        &primary.url,
                                        &primary.filename,
                                        ORIGIN_MODRINTH,
                                    )
                                    .await
                                {
                                    Ok(mut new_entry) => {
                                        new_entry.id = mod_entry.id.clone();
                                        new_entry.title = mod_entry.title.clone();
                                        new_entry.icon_url = mod_entry.icon_url.clone();
                                        new_entry.author = mod_entry.author.clone();
                                        new_entry.description = mod_entry.description.clone();
                                        new_entry.compatible = true;
                                        new_entry.warning_message = None;
                                        new_entry.enabled = mod_entry.enabled;
                                        self.enrich_metadata(&mut new_entry).await;

                                        // Enforce disabled status if the mod was deactivated prior to resync
                                        if !new_entry.enabled {
                                            let live_path = self.mods_dir.join(&new_entry.filename);
                                            let hidden_path = self.mods_dir.join(format!("{}.disabled", new_entry.filename));
                                            if live_path.is_file() {
                                                let _ = fs::rename(&live_path, &hidden_path);
                                            }
                                        }

                                        self.bom_service.with_bom(|bom| {
                                            bom.mods.retain(|m| {
                                                m.filename != mod_entry.filename
                                                    && m.filename != new_entry.filename
                                                    && !(new_entry.id.is_some() && new_entry.id == m.id && new_entry.origin == m.origin)
                                            });
                                            bom.mods.push(new_entry.clone());
                                            bom.deduplicate_mods();
                                        });
                                        found_compatible = true;
                                        summary.updated_count += 1;
                                        summary.updated_mods.push(new_entry.filename.clone());
                                    }
                                    Err(e) => {
                                        tracing::warn!(
                                            "Auto-update failed for Modrinth mod {}: {e}",
                                            mod_entry.filename
                                        );
                                    }
                                }
                            }
                        }
                    }
                    Err(e) => {
                        tracing::warn!(
                            "Auto-update failed for Modrinth mod {}: {e}",
                            mod_entry.filename
                        );
                    }
                }
            } else if origin.eq_ignore_ascii_case(ORIGIN_CURSEFORGE) && mod_entry.id.is_some() && self.has_curse_forge_key() {
                if let Ok(cf_mod_id) = mod_entry.id.as_deref().unwrap_or("").parse::<i64>() {
                    match self.curse_forge.list_mod_files(cf_mod_id).await {
                        Ok(files) => {
                            let matched = files.into_iter().find(|f| {
                                let match_mc = f.game_versions.iter().any(|v| v == new_mc_version);
                                let match_loader = loader_type == "vanilla"
                                    || f.game_versions.iter().any(|v| v.eq_ignore_ascii_case(loader_type));
                                match_mc && match_loader
                            });
                            if let Some(chosen_file) = matched {
                                if !chosen_file.download_url.is_empty() && !chosen_file.file_name.is_empty() {
                                    let old_file = self.mods_dir.join(&mod_entry.filename);
                                    if old_file.is_file() {
                                        let _ = fs::remove_file(&old_file);
                                    }
                                    let old_disabled = self.mods_dir.join(format!("{}.disabled", mod_entry.filename));
                                    if old_disabled.is_file() {
                                        let _ = fs::remove_file(&old_disabled);
                                    }

                                    self.bom_service.with_bom(|bom| {
                                        bom.mods.retain(|m| {
                                            m.filename != mod_entry.filename
                                                && m.filename != chosen_file.file_name
                                                && !(mod_entry.id.is_some() && mod_entry.id == m.id && mod_entry.origin == m.origin)
                                        });
                                    });

                                    match self.install_from_url(&chosen_file.download_url, &chosen_file.file_name, ORIGIN_CURSEFORGE).await {
                                        Ok(mut new_entry) => {
                                            new_entry.id = mod_entry.id.clone();
                                            new_entry.title = mod_entry.title.clone();
                                            new_entry.icon_url = mod_entry.icon_url.clone();
                                            new_entry.author = mod_entry.author.clone();
                                            new_entry.description = mod_entry.description.clone();
                                            new_entry.compatible = true;
                                            new_entry.warning_message = None;
                                            new_entry.enabled = mod_entry.enabled;

                                            if !new_entry.enabled {
                                                let active = self.mods_dir.join(&new_entry.filename);
                                                let disabled = self.mods_dir.join(format!("{}.disabled", new_entry.filename));
                                                if active.is_file() {
                                                    let _ = fs::rename(&active, &disabled);
                                                }
                                            }

                                            self.bom_service.with_bom(|bom| {
                                                bom.mods.retain(|m| {
                                                    m.filename != mod_entry.filename
                                                        && m.filename != new_entry.filename
                                                        && !(new_entry.id.is_some() && new_entry.id == m.id && new_entry.origin == m.origin)
                                                });
                                                bom.mods.push(new_entry.clone());
                                                bom.deduplicate_mods();
                                            });
                                            found_compatible = true;
                                            summary.updated_count += 1;
                                            summary.updated_mods.push(new_entry.filename.clone());
                                        }
                                        Err(e) => {
                                            tracing::warn!("Auto-update failed for CurseForge mod {}: {e}", mod_entry.filename);
                                        }
                                    }
                                }
                            }
                        }
                        Err(e) => {
                            tracing::warn!("Auto-update check failed for CurseForge mod {}: {e}", mod_entry.filename);
                        }
                    }
                }
            }

            if !found_compatible {
                self.bom_service.with_bom(|bom| {
                    if let Some(entry) = bom
                        .mods
                        .iter_mut()
                        .find(|m| m.filename == mod_entry.filename)
                    {
                        entry.compatible = false;
                        entry.warning_message = Some(format!(
                            "Unverified for MC {new_mc_version} ({loader_type})"
                        ));
                    }
                });
                summary.incompatible_count += 1;
                summary.incompatible_mods.push(mod_entry.filename.clone());
            }
        }

        self.bom_service.with_bom(|bom| {
            bom.deduplicate_mods();
        });
        self.bom_service.save()?;

        // Clean up any orphaned files on disk in mods_dir that are neither in the BOM nor valid custom server mods
        if let Ok(entries) = fs::read_dir(&self.mods_dir) {
            let valid_names: std::collections::HashSet<String> = self
                .bom_service
                .get_bom()
                .mods
                .into_iter()
                .map(|m| m.filename.to_ascii_lowercase())
                .collect();
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_file() {
                    let name = path.file_name().unwrap_or_default().to_string_lossy();
                    let base_name = name.strip_suffix(".disabled").unwrap_or(&name);
                    if base_name.ends_with(".jar") && !valid_names.contains(&base_name.to_ascii_lowercase()) {
                        // Keep valid JAR archives as custom server mods
                        if zircon_core::metadata::extractor::extract(&path).is_err() {
                            tracing::info!("Purging invalid/orphaned mod file on disk: {}", name);
                            let _ = fs::remove_file(&path);
                        }
                    }
                }
            }
        }

        tracing::info!(
            "Version sync for MC {new_mc_version} / {loader_type}: {} updated, {} incompatible",
            summary.updated_count,
            summary.incompatible_count
        );
        Ok(summary)
    }

    /// Discovers any custom server-side mod JARs on disk in `mods_dir` that are not listed in the BOM.
    pub fn discover_custom_server_mods(&self, known_bom_mods: &[ModEntry]) -> Vec<ModEntry> {
        let mut custom_mods = Vec::new();
        if let Ok(entries) = fs::read_dir(&self.mods_dir) {
            let bom_filenames: std::collections::HashSet<String> = known_bom_mods
                .iter()
                .map(|m| m.filename.to_ascii_lowercase())
                .collect();
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_file() {
                    let file_name = path.file_name().unwrap_or_default().to_string_lossy();
                    let (base_name, enabled) = if let Some(stripped) = file_name.strip_suffix(".disabled") {
                        (stripped.to_string(), false)
                    } else {
                        (file_name.to_string(), true)
                    };
                    if base_name.to_ascii_lowercase().ends_with(".jar")
                        && !bom_filenames.contains(&base_name.to_ascii_lowercase())
                    {
                        let size = fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
                        let mut custom_entry = ModEntry::new(
                            Some(format!("server-{}", base_name)),
                            base_name.clone(),
                            None,
                            0,
                            Some(ORIGIN_SERVER_CUSTOM.to_string()),
                            None,
                            size,
                        );
                        custom_entry.side = zircon_core::model::ModSide::Server;
                        custom_entry.enabled = enabled;
                        if let Ok(meta) = zircon_core::metadata::extractor::extract(&path) {
                            if !meta.name.is_empty() {
                                custom_entry.title = Some(meta.name);
                            }
                            if !meta.description.is_empty() {
                                custom_entry.description = Some(meta.description);
                            }
                            if !meta.author.is_empty() {
                                custom_entry.author = Some(meta.author);
                            }
                            if !meta.version.is_empty() {
                                custom_entry.version = Some(meta.version);
                            }
                        }
                        if custom_entry.title.is_none() {
                            custom_entry.title = Some(
                                base_name
                                    .strip_suffix(".jar")
                                    .unwrap_or(&base_name)
                                    .to_string(),
                            );
                        }
                        custom_mods.push(custom_entry);
                    }
                }
            }
        }
        custom_mods
    }

    /// Lists every mod currently installed (BOM mods + custom server-side mods).
    pub fn list_mods(&self) -> Vec<ModEntry> {
        let mut mods = self.bom_service.get_bom().mods;
        let custom = self.discover_custom_server_mods(&mods);
        mods.extend(custom);
        mods
    }

    /// Lists mods, opportunistically backfilling provider metadata (icon,
    /// title, real Modrinth project id) for legacy entries that were persisted
    /// before enrichment was written back to the BOM. Network round-trips only
    /// happen for entries that actually need repair; the repairs run in parallel
    /// (bounded concurrency) so a single offline incident cannot block the whole
    /// list. Once repaired, the enriched metadata is cached back into `bom.json`
    /// so subsequent loads require zero external requests.
    pub async fn list_mods_enriched(&self) -> Vec<ModEntry> {
        let mods = self.bom_service.get_bom().mods;

        // Fast path: check if any entry needs Modrinth or CurseForge repair
        let needs_repair = mods.iter().any(|m| {
            (m.origin.as_deref() == Some(ORIGIN_MODRINTH)
                && !m.id.as_deref().is_some_and(|id| {
                    !id.is_empty() && id.chars().all(|c| c.is_ascii_alphanumeric())
                })
                && m.icon_url.is_none()
                && m.sha1.is_some())
            || (m.origin.as_deref() == Some(ORIGIN_CURSEFORGE)
                && m.icon_url.is_none()
                && m.murmur3 > 0)
        });
        let mut result = if !needs_repair {
            mods
        } else {
            // Parallelize the repair work with bounded concurrency, then reorder
            // back by the original index so callers see a stable list.
            use futures_util::stream::{self, StreamExt};
            let results: Vec<(usize, ModEntry, bool)> = stream::iter(mods.into_iter().enumerate())
                .map(|(idx, mut entry)| {
                    let this = self.clone();
                    async move {
                        let changed = if entry.origin.as_deref() == Some(ORIGIN_CURSEFORGE) {
                            this.enrich_curseforge_metadata(&mut entry).await
                        } else {
                            this.repair_modrinth_metadata(&mut entry).await
                        };
                        (idx, entry, changed)
                    }
                })
                .buffer_unordered(8)
                .collect()
                .await;

            let mut updated = vec![ModEntry::default(); results.len()];
            let mut changed = false;
            for (idx, entry, entry_changed) in results {
                if entry_changed {
                    changed = true;
                }
                updated[idx] = entry;
            }

            if changed {
                self.bom_service.with_bom(|bom| {
                    bom.mods = updated.clone();
                    bom.deduplicate_mods();
                });
                let _ = self.bom_service.save();
            }
            updated
        };

        // Attach custom server-side mods
        let custom = self.discover_custom_server_mods(&result);
        result.extend(custom);
        result
    }

    /// Replaces the BOM entry for `entry.filename` with this entry and
    /// persists it, so provider metadata (id, icon, title) survives restarts.
    fn persist_entry(&self, entry: &ModEntry) -> std::io::Result<()> {
        self.bom_service.with_bom(|bom| {
            bom.mods.retain(|m| {
                m.filename != entry.filename
                    && !(entry.id.is_some() && entry.id == m.id && entry.origin == m.origin)
            });
            bom.mods.push(entry.clone());
            bom.deduplicate_mods();
        });
        self.bom_service.save()
    }

    /// Strictly verifies a CurseForge uploaded file against official CurseForge
    /// records, comparing SHA-1, checking that it matches the expected mod ID,
    /// and populating rich metadata.
    pub async fn verify_and_enrich_curseforge_upload(
        &self,
        entry: &mut ModEntry,
        expected_mod_id: Option<&str>,
        expected_file_id: Option<&str>,
    ) -> Result<(), ModError> {
        if !self.has_curse_forge_key() {
            return Ok(());
        }
        let murmur3 = entry.murmur3;
        if murmur3 == 0 {
            return Err(ModError::Invalid(
                "Uploaded file fingerprint calculation failed (empty file or invalid format)".to_string(),
            ));
        }

        let parsed_mod_id: Option<i64> = expected_mod_id
            .filter(|s| !s.trim().is_empty())
            .and_then(|s| s.trim().parse().ok());
        let parsed_file_id: Option<i64> = expected_file_id
            .filter(|s| !s.trim().is_empty())
            .and_then(|s| s.trim().parse().ok());

        // 1. If both mod_id and file_id are provided, try direct file metadata lookup first
        let mut file_match: Option<CurseForgeFile> = None;
        if let (Some(m_id), Some(f_id)) = (parsed_mod_id, parsed_file_id) {
            if let Ok(direct_file) = self.curse_forge.get_mod_file(m_id, f_id).await {
                let fp_match = direct_file.file_fingerprint == murmur3;
                let sha1_match = direct_file
                    .sha1()
                    .zip(entry.sha1.as_deref())
                    .map(|(official, local)| local.trim().eq_ignore_ascii_case(official.trim()))
                    .unwrap_or(false);
                let len_match = direct_file.length > 0 && direct_file.length == entry.file_size;

                if fp_match || sha1_match || len_match {
                    file_match = Some(direct_file);
                }
            }
        }

        // 2. If not found via direct file lookup, use batch fingerprint verification
        if file_match.is_none() {
            let matches = self
                .curse_forge
                .verify_fingerprints(&[murmur3])
                .await
                .map_err(|e| ModError::Api(format!("CurseForge fingerprint verification failed: {e}")))?;

            if !matches.is_empty() {
                // Find the best match among candidates
                let matched = if let Some(target_fid) = parsed_file_id {
                    matches.iter().find(|m| m.id == target_fid).cloned()
                } else {
                    None
                };

                let matched = matched.or_else(|| {
                    if let Some(target_mid) = parsed_mod_id {
                        matches.iter().find(|m| m.mod_id == target_mid).cloned()
                    } else {
                        None
                    }
                });

                let matched = matched.or_else(|| {
                    if let Some(local_sha) = entry.sha1.as_deref() {
                        matches.iter().find(|m| {
                            m.sha1()
                                .map(|s| s.trim().eq_ignore_ascii_case(local_sha.trim()))
                                .unwrap_or(false)
                        }).cloned()
                    } else {
                        None
                    }
                });

                file_match = matched.or_else(|| matches.into_iter().next());
            }
        }

        let Some(file_match) = file_match else {
            return Err(ModError::Invalid(format!(
                "File verification failed: CurseForge does not recognize '{}' as an official mod file.",
                entry.filename
            )));
        };

        // 3. Strict mod match check
        if let Some(expected_id_num) = parsed_mod_id {
            if file_match.mod_id > 0 && file_match.mod_id != expected_id_num {
                return Err(ModError::Invalid(format!(
                    "Mod mismatch: Uploaded file is for mod ID {}, but you are installing mod ID {}. Please upload the correct file.",
                    file_match.mod_id, expected_id_num
                )));
            }
        }

        // 4. SHA-1 verification & recording
        if let Some(official_sha1) = file_match.sha1() {
            if let Some(local_sha1) = &entry.sha1 {
                if !local_sha1.trim().eq_ignore_ascii_case(official_sha1.trim()) {
                    tracing::warn!(
                        "CurseForge file {} ({}) SHA-1 differs from official metadata (official: {}, local: {}). Murmur3 fingerprint ({}) verified.",
                        entry.filename, file_match.id, official_sha1, local_sha1, murmur3
                    );
                }
            }
            entry.sha1 = Some(official_sha1.to_string());
        }

        // 5. Fetch rich mod metadata from CurseForge
        if file_match.mod_id > 0 {
            entry.id = Some(file_match.mod_id.to_string());
            if let Ok(mod_info) = self.curse_forge.get_mod(file_match.mod_id).await {
                if !mod_info.name.is_empty() {
                    entry.title = Some(mod_info.name.clone());
                }
                if !mod_info.summary.is_empty() {
                    entry.description = Some(mod_info.summary.clone());
                }
                let icon = mod_info.logo.as_ref().and_then(|l| {
                    if !l.thumbnail_url.is_empty() {
                        Some(l.thumbnail_url.clone())
                    } else if !l.url.is_empty() {
                        Some(l.url.clone())
                    } else {
                        None
                    }
                });
                if let Some(icon) = icon {
                    entry.icon_url = Some(icon);
                }
                let authors = mod_info.authors_string();
                if !authors.is_empty() {
                    entry.author = Some(authors);
                }
                let website = mod_info
                    .links
                    .as_ref()
                    .and_then(|l| l.website_url.clone())
                    .unwrap_or_else(|| {
                        if !mod_info.slug.is_empty() {
                            format!(
                                "https://www.curseforge.com/minecraft/mc-mods/{}",
                                mod_info.slug
                            )
                        } else {
                            format!("https://www.curseforge.com/projects/{}", mod_info.id)
                        }
                    });
                if !website.is_empty() {
                    entry.project_url = Some(website);
                }
                entry.origin = Some(ORIGIN_CURSEFORGE.to_string());
                tracing::info!(
                    "Strictly verified and enriched CurseForge mod {} -> '{}' (id: {})",
                    entry.filename,
                    entry.display_title(),
                    file_match.mod_id
                );
            }
        }

        Ok(())
    }

    /// Enriches a CurseForge mod entry by querying CurseForge's fingerprint API,
    /// verifying the file against the official CurseForge record, comparing SHA-1,
    /// and populating the official title, summary, icon_url, author, and project_url.
    pub async fn enrich_curseforge_metadata(&self, entry: &mut ModEntry) -> bool {
        if !self.has_curse_forge_key() {
            return false;
        }
        let murmur3 = entry.murmur3;
        if murmur3 == 0 {
            return false;
        }
        let Ok(matches) = self.curse_forge.verify_fingerprints(&[murmur3]).await else {
            return false;
        };
        let Some(file_match) = matches.into_iter().next() else {
            tracing::info!("CurseForge fingerprint {murmur3} not matched to a known file");
            return false;
        };

        // If CurseForge has an official SHA-1 for this file, verify it
        if let Some(official_sha1) = file_match.sha1() {
            if let Some(local_sha1) = &entry.sha1 {
                if !local_sha1.eq_ignore_ascii_case(official_sha1) {
                    tracing::warn!(
                        "Uploaded mod {} SHA-1 mismatch: local={}, official={}",
                        entry.filename,
                        local_sha1,
                        official_sha1
                    );
                } else {
                    tracing::info!(
                        "Uploaded mod {} SHA-1 verified successfully against CurseForge: {}",
                        entry.filename,
                        official_sha1
                    );
                }
            }
            entry.sha1 = Some(official_sha1.to_string());
        }

        if file_match.mod_id > 0 {
            entry.id = Some(file_match.mod_id.to_string());
            if let Ok(mod_info) = self.curse_forge.get_mod(file_match.mod_id).await {
                if !mod_info.name.is_empty() {
                    entry.title = Some(mod_info.name.clone());
                }
                if !mod_info.summary.is_empty() {
                    entry.description = Some(mod_info.summary.clone());
                }
                let icon = mod_info.logo.as_ref().and_then(|l| {
                    if !l.thumbnail_url.is_empty() {
                        Some(l.thumbnail_url.clone())
                    } else if !l.url.is_empty() {
                        Some(l.url.clone())
                    } else {
                        None
                    }
                });
                if let Some(icon) = icon {
                    entry.icon_url = Some(icon);
                }
                let authors = mod_info.authors_string();
                if !authors.is_empty() {
                    entry.author = Some(authors);
                }
                let website = mod_info.links.as_ref().and_then(|l| l.website_url.clone()).unwrap_or_else(|| {
                    if !mod_info.slug.is_empty() {
                        format!("https://www.curseforge.com/minecraft/mc-mods/{}", mod_info.slug)
                    } else {
                        format!("https://www.curseforge.com/projects/{}", mod_info.id)
                    }
                });
                if !website.is_empty() {
                    entry.project_url = Some(website);
                }
                entry.origin = Some(ORIGIN_CURSEFORGE.to_string());
                tracing::info!(
                    "Enriched CurseForge mod {} -> '{}' (icon: {:?})",
                    entry.filename,
                    entry.display_title(),
                    entry.icon_url
                );
                return true;
            }
        }
        false
    }

    /// One-shot repair for a Modrinth entry stored in the pre-fix format
    /// (`id` = file name, no icon): resolves the real project id via the file
    /// SHA-1 and backfills title/description/icon. Returns whether the entry
    /// changed. Never touches healthy entries or other origins.
    async fn repair_modrinth_metadata(&self, entry: &mut ModEntry) -> bool {
        if entry.origin.as_deref() != Some(ORIGIN_MODRINTH) {
            return false;
        }
        // Healthy entries carry an alphanumeric provider id (Modrinth ids are
        // base62) plus their metadata — skip without a network call.
        if entry
            .id
            .as_deref()
            .is_some_and(|id| !id.is_empty() && id.chars().all(|c| c.is_ascii_alphanumeric()))
        {
            return false;
        }
        if entry.icon_url.is_some() {
            return false;
        }
        let Some(sha1) = entry.sha1.clone() else {
            return false;
        };
        let project_id = match self.modrinth.verify_hashes(&[sha1.clone()]).await {
            Ok(found) => found.get(&sha1).map(|v| v.project_id.clone()),
            Err(_) => None,
        };
        let Some(project_id) = project_id else {
            return false;
        };
        let Ok(project) = self.modrinth.get_project(&project_id).await else {
            return false;
        };
        entry.id = Some(project_id);
        if !project.title.is_empty() {
            entry.title = Some(project.title);
        }
        if !project.description.is_empty() {
            entry.description = Some(project.description);
        }
        if !project.icon_url.is_empty() {
            entry.icon_url = Some(project.icon_url);
        }
        true
    }

    /// Lists the files physically present in the mods folder.
    pub fn list_mod_files(&self) -> std::io::Result<Vec<PathBuf>> {
        let mut files = Vec::new();
        if let Ok(entries) = fs::read_dir(&self.mods_dir) {
            for entry in entries.flatten() {
                if entry.path().is_file() {
                    files.push(entry.path());
                }
            }
        }
        Ok(files)
    }

    // ----------------------------------------------------------------------
    // helpers
    // ----------------------------------------------------------------------

    fn safe_resolve(&self, filename: &str) -> Option<PathBuf> {
        let safe_name = sanitize_filename(filename).ok()?;
        // safe_name is sanitized (no separators), so the join cannot escape.
        let resolved = self.mods_dir.join(&safe_name);
        if resolved.starts_with(&self.mods_dir) {
            Some(resolved)
        } else {
            None
        }
    }

    /// Inspects the instance mods directory to find whether a target mod is active
    /// (`<name>.jar`) or dormant (`<name>.jar.disabled`).
    fn locate_mod_state(&self, safe_name: &str) -> Option<ModFileState> {
        let active_path = self.mods_dir.join(safe_name);
        if active_path.starts_with(&self.mods_dir) && active_path.is_file() {
            return Some(ModFileState::Enabled(active_path));
        }
        let disabled_path = self.mods_dir.join(format!("{safe_name}.disabled"));
        if disabled_path.starts_with(&self.mods_dir) && disabled_path.is_file() {
            return Some(ModFileState::Disabled(disabled_path));
        }
        None
    }

    /// Best-effort metadata enrichment: fetches the provider project page for
    /// the entry's id and fills in title/description/icon/author. Never throws.
    async fn enrich_metadata(&self, entry: &mut ModEntry) {
        let Some(id) = entry.id.clone() else { return };
        if !entry
            .origin
            .as_deref()
            .unwrap_or("")
            .eq_ignore_ascii_case(ORIGIN_MODRINTH)
        {
            return;
        }
        match self.modrinth.get_project(&id).await {
            Ok(project) => {
                entry.slug = Some(project.slug.clone());
                entry.project_url = Some(format!("https://modrinth.com/mod/{}", project.slug));
                if !project.title.is_empty() {
                    entry.title = Some(project.title);
                }
                if !project.description.is_empty() {
                    entry.description = Some(project.description);
                }
                if !project.icon_url.is_empty() {
                    entry.icon_url = Some(project.icon_url);
                }
                if !project.author.is_empty() {
                    entry.author = Some(project.author);
                }
                if let Some(ref server_side) = project.server_side {
                    if server_side.eq_ignore_ascii_case("unsupported") {
                        entry.side = zircon_core::model::ModSide::Client;
                    }
                }
                if let Some(ref client_side) = project.client_side {
                    if client_side.eq_ignore_ascii_case("unsupported") {
                        entry.side = zircon_core::model::ModSide::Server;
                    }
                }
            }
            Err(e) => {
                tracing::warn!("Could not enrich metadata for mod {}: {e}", entry.filename);
            }
        }
    }
}

/// Windows device names that are reserved even with an extension (`CON`, `NUL`,
/// `COM1`...). Uploading a file with one of these names would create an
/// unreadable/undeletable entry on Windows, so they are prefixed defensively.
const WINDOWS_RESERVED: &[&str] = &[
    "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8",
    "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
];

/// Strips path separators and control characters so uploads cannot escape the
/// mods dir.
pub fn sanitize_filename(filename: &str) -> Result<String, ModError> {
    if filename.is_empty() {
        return Err(ModError::Invalid("filename is required".to_string()));
    }
    let mut base: String = filename.replace('\\', "/");
    if let Some(slash) = base.rfind('/') {
        base = base[slash + 1..].to_string();
    }
    let sanitized: String = base
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-') {
                c
            } else {
                '_'
            }
        })
        .collect();
    let mut base = if sanitized.trim().is_empty() {
        format!("mod-{}.jar", &Uuid::new_v4().simple().to_string()[..8])
    } else {
        sanitized
    };
    if !base.to_lowercase().ends_with(".jar") {
        base = format!("{base}.jar");
    }

    // Windows reserved device names, regardless of extension casing.
    let upper = base.to_ascii_uppercase();
    let stem = upper.strip_suffix(".JAR").unwrap_or(&upper).to_string();
    if WINDOWS_RESERVED.contains(&stem.as_str()) {
        base = format!("file_{base}");
    }
    Ok(base)
}

fn normalize_origin(origin: Option<&str>) -> String {
    match origin {
        Some(o) => match o.to_lowercase().as_str() {
            "modrinth" => ORIGIN_MODRINTH.to_string(),
            "curseforge" => ORIGIN_CURSEFORGE.to_string(),
            _ => ORIGIN_DIRECT.to_string(),
        },
        None => ORIGIN_DIRECT.to_string(),
    }
}

/// Result of a modpack installation.
#[derive(Debug, Clone, Default)]
pub struct ModpackInstallResult {
    pub installed_count: i32,
    pub failed_mods: Vec<String>,
    pub message: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir() -> PathBuf {
        crate::test_util::temp_dir("mods")
    }

    #[tokio::test]
    async fn upload_replaces_existing_and_records_hash() {
        let dir = temp_dir();
        let mods_dir = dir.join("mods");
        let bom_file = dir.join("bom.json");
        let bom = Arc::new(BomService::new(
            bom_file,
            Some(zircon_core::model::BillOfMaterials::new(
                "1.20.4", None, None,
            )),
        ));
        let service = ModManagementService::new(bom.clone(), mods_dir, "");

        let data = b"fake jar contents".to_vec();
        let first = service
            .add_mod(std::io::Cursor::new(data.clone()), "my-mod.jar", None)
            .await
            .unwrap();
        assert_eq!("my-mod.jar", first.filename);
        assert_eq!(ORIGIN_DIRECT, first.origin.unwrap());
        assert!(!first.sha1.unwrap().is_empty());

        let second = service
            .add_mod(std::io::Cursor::new(data), "my-mod.jar", None)
            .await
            .unwrap();
        assert_eq!("my-mod.jar", second.filename);
        assert_eq!(1, service.list_mods().len());

        // Filename sanitization: path traversal is neutralized.
        let safe = sanitize_filename("../../evil.jar").unwrap();
        assert_eq!("evil.jar", safe);
        let _ = fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn remove_mod_deletes_file_and_bom_entry() {
        let dir = temp_dir();
        let mods_dir = dir.join("mods");
        let bom = Arc::new(BomService::new(
            dir.join("bom.json"),
            Some(zircon_core::model::BillOfMaterials::new(
                "1.20.4", None, None,
            )),
        ));
        let service = ModManagementService::new(bom, mods_dir.clone(), "");
        service
            .add_mod(std::io::Cursor::new(vec![1u8, 2, 3]), "mod.jar", None)
            .await
            .unwrap();

        assert!(service.remove_mod("mod.jar").unwrap());
        assert!(!mods_dir.join("mod.jar").exists());
        assert_eq!(0, service.list_mods().len());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn sanitize_filename_handles_traversal_and_extension() {
        assert_eq!("evil.jar", sanitize_filename("../../evil.jar").unwrap());
        assert_eq!("mod.jar", sanitize_filename("mod.jar").unwrap());
        assert_eq!("noext.jar", sanitize_filename("noext").unwrap());
        assert!(sanitize_filename("a b c").unwrap().ends_with(".jar"));
    }

    #[test]
    fn sanitize_filename_prefixes_windows_reserved_names() {
        // Reserved device names get a neutral `file_` prefix (original case is
        // preserved, so the collision with the Windows device is broken either
        // way).
        assert_eq!("file_CON.jar", sanitize_filename("CON.jar").unwrap());
        assert_eq!("file_nul.jar", sanitize_filename("nul.jar").unwrap());
        assert_eq!("file_COM1.jar", sanitize_filename("COM1.jar").unwrap());
        assert_eq!("file_LPT9.jar", sanitize_filename("LPT9.jar").unwrap());
        // Mixed/upper-case extensions are caught too (stem check is
        // case-insensitive; the original casing is preserved).
        assert_eq!("file_NUL.JAR", sanitize_filename("NUL.JAR").unwrap());
        // Ordinary names are untouched.
        assert_eq!("my_mod.jar", sanitize_filename("my_mod.jar").unwrap());
    }

    #[tokio::test]
    async fn enriched_metadata_is_persisted_to_bom() {
        let dir = temp_dir();
        let mods_dir = dir.join("mods");
        let bom = Arc::new(BomService::new(
            dir.join("bom.json"),
            Some(zircon_core::model::BillOfMaterials::new(
                "1.20.4", None, None,
            )),
        ));
        let service = ModManagementService::new(bom, mods_dir.clone(), "");

        // add_mod persists the raw entry (id = file name, no icon) — this is
        // the pre-fix behaviour that install_modrinth_version builds on.
        service
            .add_mod(
                std::io::Cursor::new(vec![1u8, 2, 3]),
                "sodium.jar",
                Some("modrinth"),
            )
            .await
            .unwrap();

        // Post-install enrichment must be written back, not left in memory.
        let mut entry = service.list_mods()[0].clone();
        entry.id = Some("AANobbMI".to_string());
        entry.title = Some("Sodium".to_string());
        entry.icon_url = Some("https://cdn.modrinth.com/data/AANobbMI/icon.png".to_string());
        service.persist_entry(&entry).unwrap();

        let reloaded = service.list_mods();
        assert_eq!(1, reloaded.len());
        assert_eq!(Some("AANobbMI".to_string()), reloaded[0].id);
        assert_eq!(Some("Sodium".to_string()), reloaded[0].title);
        assert_eq!(
            Some("https://cdn.modrinth.com/data/AANobbMI/icon.png".to_string()),
            reloaded[0].icon_url
        );

        // Survives a fresh load from disk (no in-memory cache involved).
        let bom2 = Arc::new(BomService::new(dir.join("bom.json"), None));
        let service2 = ModManagementService::new(bom2, mods_dir, "");
        let disk = service2.list_mods();
        assert_eq!(Some("AANobbMI".to_string()), disk[0].id);
        assert_eq!(
            Some("https://cdn.modrinth.com/data/AANobbMI/icon.png".to_string()),
            disk[0].icon_url
        );
        let _ = fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn test_set_mods_enabled_disk_and_bom_synchronization() {
        let sandbox = temp_dir();
        let mods_path = sandbox.join("mods");
        let bom_instance = Arc::new(BomService::new(
            sandbox.join("bom.json"),
            Some(zircon_core::model::BillOfMaterials::new("1.21.1", None, None)),
        ));
        let service = ModManagementService::new(bom_instance, mods_path.clone(), "");
        service
            .add_mod(std::io::Cursor::new(b"TEST_JAR_PAYLOAD"), "test_mod.jar", None)
            .await
            .expect("should install mod");
        assert!(mods_path.join("test_mod.jar").is_file());

        // Deactivate mod
        let toggled = service
            .set_mods_enabled(&["test_mod.jar".to_string()], false)
            .expect("disable mod should work");
        assert_eq!(toggled, vec!["test_mod.jar".to_string()]);
        assert!(!mods_path.join("test_mod.jar").exists());
        assert!(mods_path.join("test_mod.jar.disabled").is_file());
        assert!(!service.list_mods()[0].enabled);

        // Reactivate mod
        let reactivated = service
            .set_mods_enabled(&["test_mod.jar".to_string()], true)
            .expect("enable mod should work");
        assert_eq!(reactivated, vec!["test_mod.jar".to_string()]);
        assert!(mods_path.join("test_mod.jar").is_file());
        assert!(!mods_path.join("test_mod.jar.disabled").exists());
        assert!(service.list_mods()[0].enabled);

        let _ = fs::remove_dir_all(&sandbox);
    }

    #[tokio::test]
    async fn test_remove_mods_cleans_active_and_disabled_files() {
        let sandbox = temp_dir();
        let mods_path = sandbox.join("mods");
        let bom_instance = Arc::new(BomService::new(
            sandbox.join("bom.json"),
            Some(zircon_core::model::BillOfMaterials::new("1.21.1", None, None)),
        ));
        let service = ModManagementService::new(bom_instance, mods_path.clone(), "");
        service
            .add_mod(std::io::Cursor::new(b"MOD_A"), "alpha.jar", None)
            .await
            .unwrap();
        service
            .add_mod(std::io::Cursor::new(b"MOD_B"), "beta.jar", None)
            .await
            .unwrap();
        service
            .set_mods_enabled(&["beta.jar".to_string()], false)
            .unwrap();
        assert!(mods_path.join("beta.jar.disabled").is_file());

        let purged = service
            .remove_mods(&["alpha.jar".to_string(), "beta.jar".to_string()])
            .expect("remove alpha and beta");
        assert_eq!(purged.len(), 2);
        assert!(!mods_path.join("alpha.jar").exists());
        assert!(!mods_path.join("beta.jar.disabled").exists());
        assert!(service.list_mods().is_empty());

        let _ = fs::remove_dir_all(&sandbox);
    }

    #[tokio::test]
    async fn repair_skips_healthy_entries_without_network() {
        let dir = temp_dir();
        let bom = Arc::new(BomService::new(
            dir.join("bom.json"),
            Some(zircon_core::model::BillOfMaterials::new(
                "1.20.4", None, None,
            )),
        ));
        let service = ModManagementService::new(bom, dir.join("mods"), "");

        // Non-modrinth origins are never repaired.
        let mut direct = ModEntry::new(
            Some("550e8400-e29b-41d4-a716-446655440000".to_string()),
            "x.jar",
            None,
            0,
            Some(ORIGIN_DIRECT.to_string()),
            None,
            0,
        );
        assert!(!service.repair_modrinth_metadata(&mut direct).await);

        // Healthy modrinth entries (alphanumeric provider id) are skipped.
        let mut healthy = ModEntry::new(
            Some("AANobbMI".to_string()),
            "sodium.jar",
            None,
            0,
            Some(ORIGIN_MODRINTH.to_string()),
            None,
            0,
        );
        assert!(!service.repair_modrinth_metadata(&mut healthy).await);

        let _ = fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn set_mod_side_persists_to_bom() {
        let dir = temp_dir();
        let bom = Arc::new(BomService::new(
            dir.join("bom.json"),
            Some(zircon_core::model::BillOfMaterials::new("1.20.4", None, None)),
        ));
        let mods_dir = dir.join("mods");
        let service = ModManagementService::new(bom.clone(), mods_dir.clone(), "");

        let entry = service
            .add_mod(std::io::Cursor::new(b"data"), "sodium.jar", Some(ORIGIN_MODRINTH))
            .await
            .unwrap();
        assert_eq!(zircon_core::model::ModSide::Both, entry.side);

        let updated = service
            .set_mod_side("sodium.jar", zircon_core::model::ModSide::Client)
            .unwrap();
        assert_eq!(zircon_core::model::ModSide::Client, updated.side);

        // Verify it was persisted in the BOM
        let current_bom = bom.get_bom();
        assert_eq!(
            zircon_core::model::ModSide::Client,
            current_bom.mods[0].side
        );

        // Verify get_client_bom includes client-side mods
        let client_bom = bom.get_client_bom();
        assert_eq!(1, client_bom.mods.len());

        // Update to server-only and verify it gets filtered out of client_bom
        service
            .set_mod_side("sodium.jar", zircon_core::model::ModSide::Server)
            .unwrap();
        let server_bom = bom.get_bom();
        assert_eq!(1, server_bom.mods.len());
        let filtered_client_bom = bom.get_client_bom();
        assert_eq!(0, filtered_client_bom.mods.len());

        let _ = fs::remove_dir_all(&dir);
    }
}
