//! Mod management: uploads, provider installs, BOM bookkeeping and version
//! re-sync.
//!
//! Port of `com.mcmanager.server.service.ModManagementService`.

use std::fmt;
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;

use uuid::Uuid;
use zircon_core::api::curseforge::CurseForgeApiClient;
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
        mut content: R,
        filename: &str,
        origin: Option<&str>,
    ) -> Result<ModEntry, ModError> {
        let safe_name = sanitize_filename(filename)?;
        let target = self.mods_dir.join(&safe_name);
        fs::create_dir_all(&self.mods_dir)?;

        let mut out = tokio::fs::File::create(&target).await?;
        tokio::io::copy(&mut content, &mut out).await?;
        drop(out);

        let size = fs::metadata(&target)?.len();
        let sha1 = hash::sha1_file(&target).await?;
        let murmur3_value = murmur3::curse_forge_fingerprint_of_file(&target)?;

        let normalized_origin = normalize_origin(origin);
        let id = match normalized_origin.as_str() {
            ORIGIN_MODRINTH | ORIGIN_CURSEFORGE => safe_name.clone(),
            _ => Uuid::new_v4().to_string(),
        };

        let entry = ModEntry::new(
            Some(id),
            safe_name.clone(),
            Some(sha1),
            murmur3_value,
            Some(normalized_origin.clone()),
            None,
            size,
        );

        self.bom_service.with_bom(|bom| {
            bom.mods.retain(|m| m.filename != safe_name);
            bom.mods.push(entry.clone());
        });
        self.bom_service.save()?;
        tracing::info!(
            "Added mod {} ({} bytes, {normalized_origin})",
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
        let reader = std::io::Cursor::new(bytes.to_vec());
        let entry = self.add_mod(reader, filename, Some(origin)).await?;
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
        let mut entry = self
            .install_from_url(&file.url, &file.filename, ORIGIN_MODRINTH)
            .await?;
        entry.id = Some(project_id.to_string());
        self.enrich_metadata(&mut entry).await;
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
        let reader = std::io::Cursor::new(bytes.to_vec());
        let mut archive = zip::ZipArchive::new(reader)
            .map_err(|e| ModError::Invalid(format!("Invalid .mrpack: {e}")))?;
        let index_entry = archive.by_name("modrinth.index.json").map_err(|_| {
            ModError::Invalid("Invalid .mrpack: missing modrinth.index.json".to_string())
        })?;
        let index: serde_json::Value = serde_json::from_reader(index_entry)
            .map_err(|e| ModError::Invalid(format!("Invalid modrinth.index.json: {e}")))?;

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
                                        self.enrich_metadata(&mut new_entry).await;

                                        self.bom_service.with_bom(|bom| {
                                            bom.mods.retain(|m| m.filename != mod_entry.filename);
                                            bom.mods.push(new_entry.clone());
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
            }

            if !found_compatible {
                self.bom_service.with_bom(|bom| {
                    if let Some(mod_entry) = bom
                        .mods
                        .iter_mut()
                        .find(|m| m.filename == mod_entry.filename)
                    {
                        mod_entry.compatible = false;
                        mod_entry.warning_message = Some(format!(
                            "Unverified for MC {new_mc_version} ({loader_type})"
                        ));
                    }
                });
                summary.incompatible_count += 1;
                summary.incompatible_mods.push(mod_entry.filename.clone());
            }
        }

        self.bom_service.save()?;
        tracing::info!(
            "Version sync for MC {new_mc_version} / {loader_type}: {} updated, {} incompatible",
            summary.updated_count,
            summary.incompatible_count
        );
        Ok(summary)
    }

    /// Lists every mod currently present in the BOM.
    pub fn list_mods(&self) -> Vec<ModEntry> {
        self.bom_service.get_bom().mods
    }

    /// Lists mods, opportunistically backfilling provider metadata (icon,
    /// title, real Modrinth project id) for legacy entries that were persisted
    /// before enrichment was written back to the BOM. Network round-trips only
    /// happen for entries whose stored id is a file name, so healthy mods are
    /// returned untouched.
    pub async fn list_mods_enriched(&self) -> Vec<ModEntry> {
        let mut mods = self.bom_service.get_bom().mods;
        let mut changed = false;
        for entry in mods.iter_mut() {
            if self.repair_modrinth_metadata(entry).await {
                changed = true;
            }
        }
        if changed {
            self.bom_service.with_bom(|bom| {
                bom.mods = mods.clone();
            });
            let _ = self.bom_service.save();
        }
        mods
    }

    /// Replaces the BOM entry for `entry.filename` with this entry and
    /// persists it, so provider metadata (id, icon, title) survives restarts.
    fn persist_entry(&self, entry: &ModEntry) -> std::io::Result<()> {
        self.bom_service.with_bom(|bom| {
            bom.mods.retain(|m| m.filename != entry.filename);
            bom.mods.push(entry.clone());
        });
        self.bom_service.save()
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
}
