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

/// Manages shaderpacks and resourcepacks for one server/instance.
#[derive(Clone)]
pub struct PackManagementService {
    bom_service: Arc<BomService>,
    shaderpacks_dir: PathBuf,
    resourcepacks_dir: PathBuf,
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
        }
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
        self.add(content, filename, origin, &self.shaderpacks_dir, true)
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

    pub fn list_shaderpacks(&self) -> Vec<PackEntry> {
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
        self.add(content, filename, origin, &self.resourcepacks_dir, false)
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
        self.bom_service.get_bom().resourcepacks
    }

    pub fn get_resourcepack_file(&self, filename: &str) -> Option<PathBuf> {
        self.safe_resolve(filename, &self.resourcepacks_dir)
    }

    // ----------------------------------------------------------------------
    // Shared implementation
    // ----------------------------------------------------------------------

    async fn add<R: tokio::io::AsyncRead + Unpin>(
        &self,
        mut content: R,
        filename: &str,
        origin: Option<&str>,
        dir: &Path,
        shader: bool,
    ) -> Result<PackEntry, PackError> {
        let safe_name = sanitize_pack_filename(filename)?;
        let target = dir.join(&safe_name);
        fs::create_dir_all(dir)?;

        let mut out = tokio::fs::File::create(&target).await?;
        tokio::io::copy(&mut content, &mut out).await?;
        drop(out);

        let size = fs::metadata(&target)?.len();
        let sha1 = hash::sha1_file(&target).await?;
        let normalized_origin = if origin.unwrap_or("").eq_ignore_ascii_case(ORIGIN_MODRINTH) {
            ORIGIN_MODRINTH.to_string()
        } else {
            ORIGIN_DIRECT.to_string()
        };
        let id = if normalized_origin == ORIGIN_MODRINTH {
            safe_name.clone()
        } else {
            Uuid::new_v4().to_string()
        };

        let entry = PackEntry::new(
            Some(id),
            safe_name.clone(),
            Some(sha1),
            0,
            Some(normalized_origin.clone()),
            None,
            size,
        );

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
            .add_shaderpack(std::io::Cursor::new(vec![1, 2, 3]), "CoolShaders.zip", None)
            .await
            .unwrap();
        assert_eq!("CoolShaders.zip", entry.filename);
        assert_eq!(1, service.list_shaderpacks().len());
        assert!(service.get_shaderpack_file("CoolShaders.zip").is_some());

        // Replace by same name.
        service
            .add_shaderpack(std::io::Cursor::new(vec![4, 5, 6]), "CoolShaders.zip", None)
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
            .add_resourcepack(std::io::Cursor::new(vec![1]), "VanillaTweaks.zip", None)
            .await
            .unwrap();
        assert_eq!(1, service.list_resourcepacks().len());
        assert_eq!(0, service.list_shaderpacks().len());
        let _ = fs::remove_dir_all(&dir);
    }
}
