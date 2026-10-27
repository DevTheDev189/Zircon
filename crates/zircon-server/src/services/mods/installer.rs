use std::fs;
use zircon_core::crypto::hash;
use zircon_core::crypto::murmur3;
use zircon_core::model::ModEntry;
use zircon_core::security::ssrf;

use super::*;

impl ModManagementService {
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

        let sha256 = hash::sha256_file(&target).await.ok();
        entry.sha256 = sha256.clone();
        if let (Some(h), Ok(cdn_domain)) = (&sha256, std::env::var("R2_PUBLIC_CDN_DOMAIN")) {
            let r2_configured = !std::env::var("R2_ACCESS_KEY_ID").unwrap_or_default().is_empty()
                && !std::env::var("R2_SECRET_ACCESS_KEY").unwrap_or_default().is_empty();
            let clean_domain = cdn_domain.trim_start_matches("https://").trim_end_matches('/');
            if r2_configured && !clean_domain.is_empty() {
                entry.download_url = Some(format!("https://{clean_domain}/objects/{h}.jar"));
            }
        }

        // Enrich the entry with author/description/title read from the JAR's
        // mod metadata (fabric.mod.json / mods.toml / neoforge.mods.toml).
        if let Ok(meta) = zircon_core::metadata::extractor::extract(&target) {
            match meta.normalized_environment() {
                "client" => entry.side = zircon_core::model::ModSide::Client,
                "server" => entry.side = zircon_core::model::ModSide::Server,
                _ => entry.side = zircon_core::model::ModSide::Both,
            }
            if !meta.name.is_empty() {
                entry.title = Some(meta.name);
            }
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
        } else if normalized_origin == ORIGIN_MODRINTH {
            self.enrich_metadata(&mut entry).await;
        } else if self.has_curse_forge_key() && normalized_origin == ORIGIN_DIRECT {
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
        entry.sha256 = hash::sha256_file(&target).await.ok();
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
        if entry.id.is_none() && origin.eq_ignore_ascii_case(ORIGIN_MODRINTH) {
            if let Some(idx) = url.find("cdn.modrinth.com/data/") {
                let rest = &url[idx + "cdn.modrinth.com/data/".len()..];
                if let Some(slash) = rest.find('/') {
                    let pid = &rest[..slash];
                    if !pid.is_empty() && pid.chars().all(|c| c.is_ascii_alphanumeric()) {
                        entry.id = Some(pid.to_string());
                    }
                }
            }
        }
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

        self.install_mrpack_bytes(&bytes).await
    }

    /// Ingests and installs an uploaded or downloaded .mrpack archive from bytes.
    pub async fn install_mrpack_bytes(
        &self,
        bytes: &[u8],
    ) -> Result<ModpackInstallResult, ModError> {
        let mut installed_count = 0;
        let mut failed_mods: Vec<String> = Vec::new();
        let index: serde_json::Value =  { // z0
            let reader = std::io::Cursor::new(bytes.to_vec()); // z0
            let mut archive = zip::ZipArchive::new(reader) /* z0 */
                .map_err(|err| ModError::Invalid(format!("Invalid .mrpack: {err}")))?;
            let mut index_entry = archive.by_name("modrinth.index.json").map_err(|_|  { // z0
                ModError::Invalid("Invalid .mrpack: missing modrinth.index.json".to_string()) /* z0 */
            })?; // z0
            let max_meta = zircon_core::archive::limits::DEFAULT_MAX_METADATA_BYTES;
            if index_entry.size() > max_meta {
                return Err(ModError::Invalid(format!(
                    "modrinth.index.json exceeds maximum metadata size ({max_meta} bytes)"
                )));
            }
            use std::io::Read; // z0
            let mut index_content = String::new();
            index_entry /* z0 */
                .by_ref() /* z0 */
                .take(max_meta + 1) /* z0 */
                .read_to_string(&mut index_content)
                .map_err(|err| ModError::Invalid(format!("Failed to read modrinth.index.json: {err}")))?;
            if index_content.len() as u64 > max_meta {
                return Err(ModError::Invalid("modrinth.index.json exceeded metadata quota".to_string()));
            }
            serde_json::from_str(&index_content)
                .map_err(|err| ModError::Invalid(format!("Invalid modrinth.index.json: {err}")))?
        }; // end-def 0

        if let Some(files) = index.get("files").and_then(|f| f.as_array()) {
            for element in files {
                // Environment check: skip client-only mods (e.g. Sodium, Iris) on dedicated server
                if let Some(env) = element.get("env") {
                    if let Some(server) = env.get("server").and_then(|s| s.as_str()) {
                        if server.eq_ignore_ascii_case("unsupported") {
                            let path = element.get("path").and_then(|p| p.as_str()).unwrap_or("unknown");
                            tracing::info!("Skipping client-only modpack file on dedicated server: {path}");
                            continue;
                        }
                    }
                }

                let Some(path) = element.get("path").and_then(|p| p.as_str()) else {
                    continue;
                };
                if !path.starts_with("mods/") {
                    continue;
                }
                let Ok(filename) = sanitize_filename(&path["mods/".len()..]) else {
                    failed_mods.push(path.to_string());
                    continue;
                };
                let expected_sha1 = element
                    .get("hashes")
                    .and_then(|h| h.get("sha1"))
                    .and_then(|s| s.as_str());
                let Some(download_url) = element
                    .get("downloads")
                    .and_then(|d| d.as_array())
                    .and_then(|a| a.first())
                    .and_then(|u| u.as_str())
                else {
                    continue;
                };
                match self
                    .install_from_url(download_url, &filename, ORIGIN_MODRINTH)
                    .await
                {
                    Ok(installed_entry) => {
                        if let Some(exp) = expected_sha1 {
                            if installed_entry.sha1.as_deref().map(|a| a.eq_ignore_ascii_case(exp)) != Some(true) {
                                let _ = self.remove_mod(&filename);
                                tracing::warn!("SHA-1 mismatch for modpack file {filename} (expected {exp})");
                                failed_mods.push(filename);
                                continue;
                            }
                        }
                        installed_count += 1;
                    }
                    Err(e) => {
                        tracing::warn!("Modpack file install failed for {filename}: {e}");
                        failed_mods.push(filename);
                    }
                }
            }
        }

        // Extract bundled overrides/ (configs, scripts, defaultoptions) into instance root
        if let Some(instance_dir) = self.mods_dir.parent() {
            let guard = zircon_core::archive::ArchiveGuard::for_modpack_import();
            let cursor = std::io::Cursor::new(bytes.to_vec());
            match zircon_core::archive::extract_mrpack_overrides(cursor, instance_dir, &guard) {
                Ok(stats) => {
                    tracing::info!(
                        "Extracted {} override files from .mrpack into {}",
                        stats.files_extracted,
                        instance_dir.display()
                    );
                }
                Err(e) => {
                    tracing::warn!("Failed to extract .mrpack overrides: {e}");
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

    /// Ingests and auto-unbundles a modpack archive (.zip or .mrpack) without failing with HTTP 400.
    ///
    /// - Uses `ArchiveGuard::for_modpack_import()` (zero false-positive decompression bomb aborts)
    /// - Extracts overrides (configs, scripts, overrides/mods/) into the instance directory
    /// - Unpacks any nested mod JAR files into `mods/`
    /// - Ingests unbundled JARs into the BOM, extracting manifests and enriching metadata via
    ///   Modrinth (batch SHA-1 `POST /v2/version_files`) and CurseForge (Murmur3 fingerprints).
    pub async fn unbundle_modpack_archive(
        &self,
        bytes: &[u8],
        filename: &str,
    ) -> Result<Vec<ModEntry>, ModError> {
        let guard = zircon_core::archive::ArchiveGuard::for_modpack_import();
        let instance_dir = self.mods_dir.parent().unwrap_or(&self.mods_dir).to_path_buf();
        fs::create_dir_all(&self.mods_dir)?;

        let mut zip = zip::ZipArchive::new(std::io::Cursor::new(bytes))
            .map_err(|e| ModError::Invalid(format!("Cannot open archive '{filename}': {e}")))?;

        let is_curseforge = zip.by_name("manifest.json").is_ok();
        let is_modrinth = zip.by_name("modrinth.index.json").is_ok();

        // 1. Extract overrides if present
        if is_curseforge {
            let cursor = std::io::Cursor::new(bytes.to_vec());
            zircon_core::archive::curseforge_pack::extract_curseforge_overrides(
                cursor,
                "overrides",
                &instance_dir,
                &guard,
            )
            .map_err(|e| ModError::Invalid(format!("CurseForge override extraction failed: {e}")))?;
        } else if is_modrinth {
            let cursor = std::io::Cursor::new(bytes.to_vec());
            zircon_core::archive::extract_mrpack_overrides(
                cursor,
                &instance_dir,
                &guard,
            )
            .map_err(|e| ModError::Invalid(format!("Modrinth override extraction failed: {e}")))?;
        }

        // 2. Extract any nested .jar files anywhere in the archive directly into self.mods_dir
        let mut extracted_filenames = Vec::new();
        for idx in 0..zip.len() {
            guard
                .record_entry()
                .map_err(|e| ModError::Invalid(format!("Archive entry limit exceeded: {e}")))?;
            let mut entry = match zip.by_index(idx) {
                Ok(e) => e,
                Err(_) => continue,
            };
            let entry_name = entry.name().to_string();
            let entry_lower = entry_name.to_ascii_lowercase();

            if entry.is_dir() || entry.is_symlink() || entry_name.contains("__MACOSX") {
                continue;
            }

            let declared_uncompressed = entry.size();
            let declared_compressed = entry.compressed_size();
            guard
                .check_entry_header(&entry_name, declared_uncompressed, declared_compressed)
                .map_err(|e| ModError::Invalid(format!("Archive quota violation in '{entry_name}': {e}")))?;

            if entry_lower.ends_with(".jar") {
                let base_name = std::path::Path::new(&entry_name)
                    .file_name()
                    .and_then(|f| f.to_str())
                    .unwrap_or(&entry_name);
                if let Ok(safe_name) = sanitize_filename(base_name) {
                    let dest = self.mods_dir.join(&safe_name);
                    if let Ok(mut out) = fs::File::create(&dest) {
                        use std::io::Read;
                        let max_rem = guard
                            .limits()
                            .max_uncompressed_bytes
                            .saturating_sub(guard.cumulative_uncompressed_bytes());
                        let mut bounded = (&mut entry).take(max_rem.saturating_add(1));
                        let written = std::io::copy(&mut bounded, &mut out).map_err(ModError::Io)?;
                        if let Err(e) = guard.track_stream_chunk_with_ratio(
                            written,
                            declared_compressed.max(1),
                            guard.max_ratio_for_entry(&entry_name),
                        ) {
                            let _ = fs::remove_file(&dest);
                            return Err(ModError::Invalid(format!("Archive quota exceeded: {e}")));
                        }
                        extracted_filenames.push(safe_name);
                    }
                }
            } else if !is_curseforge && !is_modrinth {
                let entry_path = std::path::Path::new(&entry_name);
                if let Some(first_comp) = entry_path.components().next() {
                    let first_str = first_comp.as_os_str().to_string_lossy().to_ascii_lowercase();
                    if first_str == "config" || first_str == "scripts" || first_str == "defaultconfigs" || first_str == "kubejs" {
                        if let Some(safe_rel) = zircon_core::archive::limits::sanitize_entry_path(entry_path) {
                            let dest = instance_dir.join(&safe_rel);
                            if let Some(parent) = dest.parent() {
                                let _ = fs::create_dir_all(parent);
                            }
                            if let Ok(mut out) = fs::File::create(&dest) {
                                use std::io::Read;
                                let max_rem = guard
                                    .limits()
                                    .max_uncompressed_bytes
                                    .saturating_sub(guard.cumulative_uncompressed_bytes());
                                let mut bounded = (&mut entry).take(max_rem.saturating_add(1));
                                let written = std::io::copy(&mut bounded, &mut out).map_err(ModError::Io)?;
                                if let Err(e) = guard.track_stream_chunk_with_ratio(
                                    written,
                                    declared_compressed.max(1),
                                    guard.max_ratio_for_entry(&entry_name),
                                ) {
                                    let _ = fs::remove_file(&dest);
                                    return Err(ModError::Invalid(format!("Archive quota exceeded: {e}")));
                                }
                            }
                        }
                    }
                }
            }
        }

        // 3. For Modrinth mrpack with remote files, install them
        if is_modrinth {
            let _ = self.install_mrpack_bytes(bytes).await;
        }

        // 4. Ingest and enrich all extracted JARs
        let mut new_entries: Vec<ModEntry> = Vec::new();
        for fname in &extracted_filenames {
            let target = self.mods_dir.join(fname);
            if !target.is_file() {
                continue;
            }

            let size = fs::metadata(&target).map(|m| m.len()).unwrap_or(0);
            let sha1 = hash::sha1_file(&target).await.ok();
            let murmur3_val = murmur3::curse_forge_fingerprint_of_file(&target).unwrap_or(0);

            let mut entry = ModEntry::new(
                Some(Uuid::new_v4().to_string()),
                fname.clone(),
                sha1,
                murmur3_val,
                Some(ORIGIN_DIRECT.to_string()),
                None,
                size,
            );

            // Tier 1: Ground-truth embedded manifest extraction
            if let Ok(meta) = zircon_core::metadata::extractor::extract(&target) {
                match meta.normalized_environment() {
                    "client" => entry.side = zircon_core::model::ModSide::Client,
                    "server" => entry.side = zircon_core::model::ModSide::Server,
                    _ => entry.side = zircon_core::model::ModSide::Both,
                }
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

            new_entries.push(entry);
        }

        // 5. Batch reverse-search enrichment (Modrinth batch SHA-1 + CurseForge Murmur3)
        self.batch_enrich_mods(&mut new_entries).await;

        // 6. Persist to BOM
        if !new_entries.is_empty() {
            let entries_to_add = new_entries.clone();
            self.bom_service.with_bom(|bom| {
                for new_entry in entries_to_add {
                    bom.mods.retain(|m| m.filename != new_entry.filename);
                    bom.mods.push(new_entry);
                }
                bom.deduplicate_mods();
            });
            let _ = self.bom_service.save();
        }

        Ok(new_entries)
    }

}
