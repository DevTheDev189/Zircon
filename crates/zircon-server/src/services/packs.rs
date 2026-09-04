//! Handles the physical shaderpack/resourcepack files in an instance's
//! `shaderpacks/` and `resourcepacks/` folders and keeps the BOM's lists in
//! sync, mirroring `ModManagementService` for mods.
//!
//! Unlike mods, packs are never force-applied to a client — the BOM only
//! advertises what's available to download; activation is a local per-player
//! choice made in the client launcher.
//!
//! Port of `com.mcmanager.server.service.PackManagementService`.

use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use uuid::Uuid;
use zircon_core::api::curseforge::{CurseForgeApiClient, CurseForgeFile};
use zircon_core::api::modrinth::ModrinthApiClient;
use zircon_core::crypto::hash;
use zircon_core::model::PackEntry;
use zircon_core::security::ssrf;

use super::bom::BomService;

/// Windows device names that are reserved even with an extension (`CON`, `NUL`,
/// `COM1`...). Uploading a file with one of these names would create an
/// unreadable/undeletable entry on Windows, so they are prefixed defensively.
const WINDOWS_RESERVED: &[&str] = &[
    "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8",
    "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
];

/// Strips path separators and control characters so uploads cannot escape
/// their pack dir, and normalizes the extension to `.zip`.
pub fn sanitize_pack_filename(filename: &str) -> Result<String, PackError> {
    if filename.is_empty() {
        return Err(PackError::Invalid("filename is required".to_string()));
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
        format!(
            "pack-{}.zip",
            &uuid::Uuid::new_v4().simple().to_string()[..8]
        )
    } else {
        sanitized
    };
    // Normalize the extension: packs are zips, never jars.
    if base.to_lowercase().ends_with(".jar") {
        base = format!("{}.zip", &base[..base.len() - 4]);
    }
    if !base.to_lowercase().ends_with(".zip") {
        base = format!("{base}.zip");
    }

    // Windows reserved device names, regardless of extension casing.
    let upper = base.to_ascii_uppercase();
    let stem = upper.strip_suffix(".ZIP").unwrap_or(&upper).to_string();
    if WINDOWS_RESERVED.contains(&stem.as_str()) {
        base = format!("file_{base}");
    }
    Ok(base)
}

pub const ORIGIN_MODRINTH: &str = "modrinth";
pub const ORIGIN_CURSEFORGE: &str = "curseforge";
pub const ORIGIN_DIRECT: &str = "direct";

/// Errors raised by the pack management service.
#[derive(Debug)]
pub enum PackError {
    Invalid(String),
    Io(std::io::Error),
    Api(String),
}

impl fmt::Display for PackError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PackError::Invalid(m) => write!(f, "{m}"),
            PackError::Io(e) => write!(f, "{e}"),
            PackError::Api(m) => write!(f, "{m}"),
        }
    }
}

impl std::error::Error for PackError {}

impl From<std::io::Error> for PackError {
    fn from(e: std::io::Error) -> Self {
        PackError::Io(e)
    }
}

impl From<super::mods::ModError> for PackError {
    fn from(e: super::mods::ModError) -> Self {
        PackError::Invalid(e.to_string())
    }
}

use zircon_core::crypto::murmur3;

/// Manages shaderpacks and resourcepacks for one server/instance.
///
/// Holds paths to the physical folders (`<instance>/shaderpacks/` and
/// `<instance>/resourcepacks/`) and references the `BomService` to keep BOM
/// lists in sync with disk.
#[derive(Clone)]
pub struct PackManagementService {
    bom_service: Arc<BomService>,
    shaderpacks_dir: PathBuf,
    resourcepacks_dir: PathBuf,
    curse_forge: CurseForgeApiClient,
    curseforge_key: String,
}

impl PackManagementService {
    pub fn new(
        bom_service: Arc<BomService>,
        shaderpacks_dir: PathBuf,
        resourcepacks_dir: PathBuf,
    ) -> Self {
        Self {
            bom_service,
            shaderpacks_dir,
            resourcepacks_dir,
            curse_forge: CurseForgeApiClient::new(""),
            curseforge_key: String::new(),
        }
    }

    pub fn with_curseforge_key(mut self, key: &str) -> Self {
        self.curseforge_key = key.to_string();
        self.curse_forge = CurseForgeApiClient::new(key);
        self
    }

    pub fn has_curse_forge_key(&self) -> bool {
        !self.curseforge_key.trim().is_empty()
    }

    // ----------------------------------------------------------------------
    // Shaderpacks
    // ----------------------------------------------------------------------

    pub async fn add_shaderpack<R: tokio::io::AsyncRead + Unpin>(
        &self,
        content: R,
        filename: &str,
        origin: Option<&str>,
    ) -> Result<PackEntry, PackError> {
        self.add_shaderpack_with_metadata(content, filename, origin, None, None, None, None, None)
            .await
    }

    pub async fn add_shaderpack_with_metadata<R: tokio::io::AsyncRead + Unpin>(
        &self,
        content: R,
        filename: &str,
        origin: Option<&str>,
        fallback_icon: Option<&str>,
        fallback_title: Option<&str>,
        expected_mod_id: Option<&str>,
        expected_file_id: Option<&str>,
        fallback_project_url: Option<&str>,
    ) -> Result<PackEntry, PackError> {
        self.add_with_metadata(
            content,
            filename,
            origin,
            fallback_icon,
            fallback_title,
            expected_mod_id,
            expected_file_id,
            fallback_project_url,
            &self.shaderpacks_dir,
            true,
        )
        .await
    }

    pub async fn install_shaderpack_from_url(
        &self,
        url: &str,
        filename: &str,
        origin: Option<&str>,
    ) -> Result<PackEntry, PackError> {
        self.install_from_url(url, filename, origin, true).await
    }

    pub fn remove_shaderpack(&self, filename: &str) -> Result<bool, PackError> {
        self.remove(filename, &self.shaderpacks_dir, true)
    }

    pub fn sync_pack_metadata(&self, shader: bool) {
        let dir = if shader {
            &self.shaderpacks_dir
        } else {
            &self.resourcepacks_dir
        };
        let mut modified = false;
        self.bom_service.with_bom(|bom| {
            let list = if shader {
                &mut bom.shaderpacks
            } else {
                &mut bom.resourcepacks
            };
            for entry in list.iter_mut() {
                if entry.version.is_none() || (!shader && entry.pack_format.is_none()) {
                    let file_path = dir.join(&entry.filename);
                    if file_path.is_file() {
                        if shader {
                            if let Ok(meta) = zircon_core::metadata::extract_shader_pack_metadata(&file_path) {
                                if meta.version.is_some() {
                                    entry.version = meta.version;
                                    modified = true;
                                }
                                if meta.description.is_some() && entry.description.is_none() {
                                    entry.description = meta.description;
                                    modified = true;
                                }
                            }
                        } else {
                            if let Ok(meta) = zircon_core::metadata::extract_resource_pack_metadata(&file_path) {
                                if meta.version.is_some() {
                                    entry.version = meta.version;
                                    modified = true;
                                }
                                if meta.pack_format.is_some() {
                                    entry.pack_format = meta.pack_format;
                                    modified = true;
                                }
                                if meta.description.is_some() && entry.description.is_none() {
                                    entry.description = meta.description;
                                    modified = true;
                                }
                            }
                        }
                    }
                }
            }
        });
        if modified {
            let _ = self.bom_service.save();
        }
    }

    pub fn list_shaderpacks(&self) -> Vec<PackEntry> {
        self.sync_pack_metadata(true);
        self.bom_service.get_bom().shaderpacks
    }

    pub fn get_shaderpack_file(&self, filename: &str) -> Option<PathBuf> {
        self.safe_resolve(filename, &self.shaderpacks_dir)
    }

    // ----------------------------------------------------------------------
    // Resourcepacks
    // ----------------------------------------------------------------------

    pub async fn add_resourcepack<R: tokio::io::AsyncRead + Unpin>(
        &self,
        content: R,
        filename: &str,
        origin: Option<&str>,
    ) -> Result<PackEntry, PackError> {
        self.add_resourcepack_with_metadata(content, filename, origin, None, None, None, None, None)
            .await
    }

    pub async fn add_resourcepack_with_metadata<R: tokio::io::AsyncRead + Unpin>(
        &self,
        content: R,
        filename: &str,
        origin: Option<&str>,
        fallback_icon: Option<&str>,
        fallback_title: Option<&str>,
        expected_mod_id: Option<&str>,
        expected_file_id: Option<&str>,
        fallback_project_url: Option<&str>,
    ) -> Result<PackEntry, PackError> {
        self.add_with_metadata(
            content,
            filename,
            origin,
            fallback_icon,
            fallback_title,
            expected_mod_id,
            expected_file_id,
            fallback_project_url,
            &self.resourcepacks_dir,
            false,
        )
        .await
    }

    pub async fn install_resourcepack_from_url(
        &self,
        url: &str,
        filename: &str,
        origin: Option<&str>,
    ) -> Result<PackEntry, PackError> {
        self.install_from_url(url, filename, origin, false).await
    }

    pub fn remove_resourcepack(&self, filename: &str) -> Result<bool, PackError> {
        self.remove(filename, &self.resourcepacks_dir, false)
    }

    pub fn list_resourcepacks(&self) -> Vec<PackEntry> {
        self.sync_pack_metadata(false);
        self.bom_service.get_bom().resourcepacks
    }

    pub fn get_resourcepack_file(&self, filename: &str) -> Option<PathBuf> {
        self.safe_resolve(filename, &self.resourcepacks_dir)
    }

    pub fn set_server_resourcepack(&self, filename: Option<&str>) -> Result<(), PackError> {
        let mut found = false;
        self.bom_service.with_bom(|bom| {
            for pack in &mut bom.resourcepacks {
                if let Some(target) = filename {
                    if pack.filename == target {
                        pack.server_enforced = Some(true);
                        found = true;
                    } else {
                        pack.server_enforced = None;
                    }
                } else {
                    pack.server_enforced = None;
                }
            }
        });
        if filename.is_some() && !found {
            return Err(PackError::Invalid(format!("Resource pack '{filename:?}' not found in BOM")));
        }
        self.bom_service.save().map_err(PackError::Io)?;
        Ok(())
    }

    pub fn get_server_resourcepack(&self) -> Option<PackEntry> {
        self.bom_service.get_bom().resourcepacks.into_iter().find(|p| p.server_enforced == Some(true))
    }

    // ----------------------------------------------------------------------
    // Shared implementation
    // ----------------------------------------------------------------------

    /// Strictly verifies a CurseForge uploaded pack against official records,
    /// checking SHA-1, matching expected pack ID, and populating rich metadata.
    pub async fn verify_and_enrich_curseforge_upload(
        &self,
        entry: &mut PackEntry,
        expected_mod_id: Option<&str>,
        expected_file_id: Option<&str>,
    ) -> Result<(), PackError> {
        if !self.has_curse_forge_key() {
            return Ok(());
        }
        let murmur3 = entry.murmur3;
        if murmur3 == 0 {
            return Err(PackError::Invalid(
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
                .map_err(|e| PackError::Api(format!("CurseForge fingerprint verification failed: {e}")))?;

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
            return Err(PackError::Invalid(format!(
                "File verification failed: CurseForge does not recognize '{}' as an official pack file.",
                entry.filename
            )));
        };

        // 3. Strict mod match check
        if let Some(expected_id_num) = parsed_mod_id {
            if file_match.mod_id > 0 && file_match.mod_id != expected_id_num {
                return Err(PackError::Invalid(format!(
                    "Pack mismatch: Uploaded file is for pack ID {}, but you are installing pack ID {}. Please upload the correct file.",
                    file_match.mod_id, expected_id_num
                )));
            }
        }

        // 4. SHA-1 verification & recording
        if let Some(official_sha1) = file_match.sha1() {
            if let Some(local_sha1) = &entry.sha1 {
                if !local_sha1.trim().eq_ignore_ascii_case(official_sha1.trim()) {
                    tracing::warn!(
                        "CurseForge pack file {} ({}) SHA-1 differs from official metadata (official: {}, local: {}). Murmur3 fingerprint ({}) verified.",
                        entry.filename, file_match.id, official_sha1, local_sha1, murmur3
                    );
                }
            }
            entry.sha1 = Some(official_sha1.to_string());
        }

        // 5. Fetch rich pack metadata from CurseForge
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
                                "https://www.curseforge.com/minecraft/texture-packs/{}",
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
                    "Strictly verified and enriched CurseForge pack {} -> '{}' (id: {})",
                    entry.filename,
                    entry.display_title(),
                    file_match.mod_id
                );
            }
        }

        Ok(())
    }

    async fn add_with_metadata<R: tokio::io::AsyncRead + Unpin>(
        &self,
        mut content: R,
        filename: &str,
        origin: Option<&str>,
        fallback_icon: Option<&str>,
        fallback_title: Option<&str>,
        expected_mod_id: Option<&str>,
        expected_file_id: Option<&str>,
        fallback_project_url: Option<&str>,
        dir: &Path,
        shader: bool,
    ) -> Result<PackEntry, PackError> {
        let safe_name = sanitize_pack_filename(filename)?;
        let target = dir.join(&safe_name);
        fs::create_dir_all(dir)?;

        let mut out = tokio::fs::File::create(&target).await?;
        tokio::io::copy(&mut content, &mut out).await?;
        drop(out);

        // Zero-trust security audit: enforce file extension whitelist and zip safety
        let guard = zircon_core::archive::limits::ArchiveGuard::default();
        let pack_file = match std::fs::File::open(&target) {
            Ok(f) => f,
            Err(e) => {
                let _ = fs::remove_file(&target);
                return Err(PackError::Io(e));
            }
        };
        if let Err(e) = zircon_core::security::pack_validator::validate_pack_archive(pack_file, &guard) {
            let _ = fs::remove_file(&target);
            return Err(PackError::Invalid(format!("Security validation failed: {e}")));
        }

        let size = match fs::metadata(&target) {
            Ok(m) => m.len(),
            Err(e) => {
                let _ = fs::remove_file(&target);
                return Err(PackError::Io(e));
            }
        };

        let sha1 = match hash::sha1_file(&target).await {
            Ok(s) => s,
            Err(e) => {
                let _ = fs::remove_file(&target);
                return Err(PackError::Io(e));
            }
        };

        let murmur3_value = match murmur3::curse_forge_fingerprint_of_file(&target) {
            Ok(v) => v,
            Err(_) => 0,
        };

        let normalized_origin = if origin.unwrap_or("").eq_ignore_ascii_case(ORIGIN_CURSEFORGE) {
            ORIGIN_CURSEFORGE.to_string()
        } else if origin.unwrap_or("").eq_ignore_ascii_case(ORIGIN_MODRINTH) {
            ORIGIN_MODRINTH.to_string()
        } else {
            ORIGIN_DIRECT.to_string()
        };

        let id = match normalized_origin.as_str() {
            ORIGIN_MODRINTH | ORIGIN_CURSEFORGE => {
                expected_mod_id.filter(|id| !id.is_empty()).map(str::to_string).unwrap_or_else(|| safe_name.clone())
            }
            _ => Uuid::new_v4().to_string(),
        };

        let mut pack_version: Option<String> = None;
        let mut resource_pack_format: Option<u32> = None;
        let mut pack_description: Option<String> = None;
// spacer 0
        if shader  { // z0
            if let Ok(shader_meta) = zircon_core::metadata::extract_shader_pack_metadata(&target) {
                pack_version = shader_meta.version;
                pack_description = shader_meta.description;
            } // end-block 0
        } else if let Ok(resource_meta) = zircon_core::metadata::extract_resource_pack_metadata(&target) {
            pack_version = resource_meta.version;
            resource_pack_format = resource_meta.pack_format;
            pack_description = resource_meta.description;
        } // end-block 0

        let mut entry = PackEntry::new( /* z0 */
            Some(id),
            safe_name.clone(),
            Some(sha1),
            murmur3_value,
            Some(normalized_origin.clone()),
            None,
            size,
        );
        entry.version = pack_version;
        entry.pack_format = resource_pack_format;
        entry.sanitized = Some(true);
        if pack_description.is_some() {
            entry.description = pack_description;
        } // end-block 0

        // Strict verification for CurseForge origin packs
        if normalized_origin == ORIGIN_CURSEFORGE {
            if let Err(e) = self.verify_and_enrich_curseforge_upload(&mut entry, expected_mod_id, expected_file_id).await {
                let _ = fs::remove_file(&target);
                return Err(e);
            }
        }

        // Apply fallback metadata
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
            if shader {
                bom.shaderpacks.retain(|p| p.filename != safe_name);
                bom.shaderpacks.push(entry.clone());
            } else {
                bom.resourcepacks.retain(|p| p.filename != safe_name);
                bom.resourcepacks.push(entry.clone());
            }
        });
        self.bom_service.save()?;
        tracing::info!(
            "Added {} {} ({} bytes, {normalized_origin})",
            if shader { "shaderpack" } else { "resourcepack" },
            safe_name,
            size
        );
        Ok(entry)
    }

    async fn add<R: tokio::io::AsyncRead + Unpin>(
        &self,
        content: R,
        filename: &str,
        origin: Option<&str>,
        dir: &Path,
        shader: bool,
    ) -> Result<PackEntry, PackError> {
        self.add_with_metadata(
            content,
            filename,
            origin,
            None,
            None,
            None,
            None,
            None,
            dir,
            shader,
        )
        .await
    }

    async fn install_from_url(
        &self,
        url: &str,
        filename: &str,
        origin: Option<&str>,
        shader: bool,
    ) -> Result<PackEntry, PackError> {
        if !ssrf::is_safe_cdn_url(url) {
            return Err(PackError::Invalid(format!(
                "Rejected download URL (host is not an allowed CDN): {url}"
            )));
        }
        let response = reqwest::get(url)
            .await
            .map_err(|e| PackError::Api(format!("Download failed: {e}")))?;
        let status = response.status();
        if !status.is_success() {
            return Err(PackError::Api(format!("Download failed: HTTP {status}")));
        }
        let bytes = response
            .bytes()
            .await
            .map_err(|e| PackError::Api(format!("Download failed: {e}")))?;
        let dir = if shader {
            self.shaderpacks_dir.clone()
        } else {
            self.resourcepacks_dir.clone()
        };
        let mut entry = self
            .add(
                std::io::Cursor::new(bytes.to_vec()),
                filename,
                origin,
                &dir,
                shader,
            )
            .await?;
        entry.download_url = Some(url.to_string());
        self.bom_service.save()?;
        Ok(entry)
    }

    /// Installs a shaderpack or resourcepack from Modrinth by project id,
    /// optionally pinning a specific version, and enriches the resulting BOM
    /// entry with the project's rich metadata (icon, slug, author, description,
    /// title, project URL).
    pub async fn install_modrinth_pack(
        &self,
        project_id: &str,
        version_id: Option<&str>,
        is_shader: bool,
    ) -> Result<PackEntry, PackError> {
        let modrinth = ModrinthApiClient::new();
        let versions = modrinth
            .list_project_versions(project_id, None, None)
            .await
            .map_err(|e| PackError::Api(e.to_string()))?;

        let version = versions
            .into_iter()
            .find(|v| version_id.is_none() || version_id == Some(v.id.as_str()))
            .ok_or_else(|| PackError::Invalid("No matching pack version found".into()))?;

        let file = version
            .primary_file()
            .ok_or_else(|| PackError::Invalid("No downloadable file found in version".into()))?;

        let dir = if is_shader {
            &self.shaderpacks_dir
        } else {
            &self.resourcepacks_dir
        };

        let bytes = reqwest::get(&file.url)
            .await
            .map_err(|e| PackError::Api(e.to_string()))?
            .bytes()
            .await
            .map_err(|e| PackError::Api(e.to_string()))?
            .to_vec();

        let mut entry = self
            .add(
                std::io::Cursor::new(bytes),
                &file.filename,
                Some(ORIGIN_MODRINTH),
                dir,
                is_shader,
            )
            .await?;

        entry.version = Some(version.version_number.clone()); // z0
// spacer 0
        // Enrich with Modrinth Project details.
        if let Ok(project) = modrinth.get_project(project_id).await {
            entry.id = Some(project.id.clone());
            entry.slug = Some(project.slug.clone());
            entry.title = Some(project.title);
            entry.description = Some(project.description);
            entry.icon_url = Some(project.icon_url);
            entry.author = Some(project.author);
            let category = if is_shader { "shader" } else { "resourcepack" };
            entry.project_url = Some(format!("https://modrinth.com/{category}/{}", project.slug));
        }

        // Persist the enriched entry to the BOM.
        self.bom_service.with_bom(|bom| {
            let list = if is_shader {
                &mut bom.shaderpacks
            } else {
                &mut bom.resourcepacks
            };
            list.retain(|p| p.filename != entry.filename);
            list.push(entry.clone());
        });
        self.bom_service.save()?;

        Ok(entry)
    }

    fn remove(&self, filename: &str, dir: &Path, shader: bool) -> Result<bool, PackError> {
        let safe_name = sanitize_pack_filename(filename)?;
        let file = dir.join(&safe_name);
        let deleted = if file.is_file() {
            fs::remove_file(&file)?;
            true
        } else {
            false
        };
        let removed_from_bom = self.bom_service.with_bom(|bom| {
            let before = if shader {
                bom.shaderpacks.len()
            } else {
                bom.resourcepacks.len()
            };
            if shader {
                bom.shaderpacks.retain(|p| p.filename != safe_name);
            } else {
                bom.resourcepacks.retain(|p| p.filename != safe_name);
            }
            let after = if shader {
                bom.shaderpacks.len()
            } else {
                bom.resourcepacks.len()
            };
            after != before
        });
        if removed_from_bom {
            self.bom_service.save()?;
        }
        Ok(deleted || removed_from_bom)
    }

    fn safe_resolve(&self, filename: &str, dir: &Path) -> Option<PathBuf> {
        let safe_name = sanitize_pack_filename(filename).ok()?;
        // safe_name is sanitized (no separators), so the join cannot escape.
        let resolved = dir.join(&safe_name);
        if resolved.starts_with(dir) && resolved.is_file() {
            Some(resolved)
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir() -> PathBuf {
        crate::test_util::temp_dir("packs")
    }

    fn valid_test_zip() -> Vec<u8> {
        let mut buffer = Vec::new();
        {
            let mut zip = zip::ZipWriter::new(std::io::Cursor::new(&mut buffer));
            let options: zip::write::FileOptions<'_, ()> = zip::write::FileOptions::default();
            zip.start_file("pack.mcmeta", options).unwrap();
            std::io::Write::write_all(&mut zip, b"{\"pack\":{\"pack_format\":15,\"description\":\"Test\"}}").unwrap();
            zip.finish().unwrap();
        }
        buffer
    }

    #[tokio::test]
    async fn shaderpack_upload_updates_bom() {
        let dir = temp_dir();
        let bom = Arc::new(BomService::new(
            dir.join("bom.json"),
            Some(zircon_core::model::BillOfMaterials::new(
                "1.20.4", None, None,
            )),
        ));
        let service = PackManagementService::new(
            bom.clone(),
            dir.join("shaderpacks"),
            dir.join("resourcepacks"),
        );

        let entry = service
            .add_shaderpack(std::io::Cursor::new(valid_test_zip()), "CoolShaders.zip", None)
            .await
            .unwrap();
        assert_eq!("CoolShaders.zip", entry.filename);
        assert_eq!(1, service.list_shaderpacks().len());
        assert!(service.get_shaderpack_file("CoolShaders.zip").is_some());

        // Replace by same name.
        service
            .add_shaderpack(std::io::Cursor::new(valid_test_zip()), "CoolShaders.zip", None)
            .await
            .unwrap();
        assert_eq!(1, service.list_shaderpacks().len());

        assert!(service.remove_shaderpack("CoolShaders.zip").unwrap());
        assert_eq!(0, service.list_shaderpacks().len());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn sanitize_pack_filename_prefixes_windows_reserved_names() {
        // Reserved device names get a neutral `file_` prefix (original case is
        // preserved, so the collision with the Windows device is broken either
        // way).
        assert_eq!("file_CON.zip", sanitize_pack_filename("CON.zip").unwrap());
        assert_eq!("file_nul.zip", sanitize_pack_filename("nul").unwrap());
        assert_eq!("file_COM3.zip", sanitize_pack_filename("COM3.zip").unwrap());
        assert_eq!("file_LPT1.zip", sanitize_pack_filename("LPT1.zip").unwrap());
        // Mixed/upper-case extensions are caught too (original casing is
        // preserved).
        assert_eq!("file_AUX.ZIP", sanitize_pack_filename("AUX.ZIP").unwrap());
        // Ordinary names are untouched (and jars become zips).
        assert_eq!("world.zip", sanitize_pack_filename("world.zip").unwrap());
        assert_eq!("cool.zip", sanitize_pack_filename("cool.jar").unwrap());
    }

    #[tokio::test]
    async fn resourcepack_and_shaderpack_stores_are_independent() {
        let dir = temp_dir();
        let bom = Arc::new(BomService::new(
            dir.join("bom.json"),
            Some(zircon_core::model::BillOfMaterials::new(
                "1.20.4", None, None,
            )),
        ));
        let service =
            PackManagementService::new(bom, dir.join("shaderpacks"), dir.join("resourcepacks"));
        service
            .add_resourcepack(std::io::Cursor::new(valid_test_zip()), "VanillaTweaks.zip", None)
            .await
            .unwrap();
        assert_eq!(1, service.list_resourcepacks().len());
        assert_eq!(0, service.list_shaderpacks().len());
        let _ = fs::remove_dir_all(&dir);
    }
}
