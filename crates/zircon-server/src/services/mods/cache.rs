use std::fs;
use std::path::PathBuf;
use zircon_core::api::curseforge::CurseForgeFile;
use zircon_core::model::ModEntry;

use super::*;

impl ModManagementService {
    /// Replaces the BOM entry for `entry.filename` with this entry and
    /// persists it, so provider metadata (id, icon, title) survives restarts.
    pub(crate) fn persist_entry(&self, entry: &ModEntry) -> std::io::Result<()> {
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
            return Err(ModError::Invalid(
                "CurseForge API key is not configured; cannot verify uploaded CurseForge file".to_string(),
            ));
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

                if fp_match && sha1_match {
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
        let Some(official_sha1) = file_match.sha1().map(str::trim).filter(|s| !s.is_empty()) else {
            return Err(ModError::Invalid(format!(
                "CurseForge file '{}' (ID {}) does not provide an official SHA-1 digest for verification.",
                entry.filename, file_match.id
            )));
        };
        let Some(local_sha1) = entry.sha1.as_deref().map(str::trim).filter(|s| !s.is_empty()) else {
            return Err(ModError::Invalid(format!(
                "Uploaded file '{}' is missing a local SHA-1 digest.",
                entry.filename
            )));
        };
        if !local_sha1.eq_ignore_ascii_case(official_sha1) {
            return Err(ModError::Invalid(format!(
                "Cryptographic verification failed for '{}': local SHA-1 ({}) does not match official CurseForge SHA-1 ({}).",
                entry.filename, local_sha1, official_sha1
            )));
        }
        entry.sha1 = Some(official_sha1.to_ascii_lowercase());

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
        // Strict origin check: if the mod originates from Modrinth or has a Modrinth CDN URL, NEVER enrich with CurseForge!
        if entry.origin.as_deref() == Some(ORIGIN_MODRINTH) {
            return false;
        }
        if let Some(url) = &entry.download_url {
            if url.contains("cdn.modrinth.com") || url.contains("modrinth.com") {
                return false;
            }
        }
        // If entry has a non-numeric ID (like Modrinth base62 IDs), it cannot be a CurseForge mod
        if let Some(id) = &entry.id {
            if !id.is_empty() && id.chars().any(|c| !c.is_ascii_digit()) {
                return false;
            }
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

        // Require CurseForge's official SHA-1 to match the local file's SHA-1 before promoting origin
        let Some(official_sha1) = file_match.sha1().map(str::trim).filter(|s| !s.is_empty()) else {
            tracing::warn!(
                "CurseForge file match for {} (id={}) has no official SHA-1; refusing Murmur3-only enrichment",
                entry.filename,
                file_match.id
            );
            return false;
        };
        let Some(local_sha1) = entry.sha1.as_deref().map(str::trim).filter(|s| !s.is_empty()) else {
            return false;
        };
        if !local_sha1.eq_ignore_ascii_case(official_sha1) {
            tracing::warn!(
                "Refusing CurseForge enrichment for {}: local SHA-1 ({}) != official SHA-1 ({})",
                entry.filename,
                local_sha1,
                official_sha1
            );
            return false;
        }
        entry.sha1 = Some(official_sha1.to_ascii_lowercase());

        if file_match.mod_id > 0 {
            if let Some(orig) = &entry.origin {
                if !orig.eq_ignore_ascii_case(ORIGIN_CURSEFORGE) && !orig.eq_ignore_ascii_case(ORIGIN_DIRECT) {
                    return false;
                }
            }
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
                if mod_info.categories.iter().any(|c| c.id == 424) {
                    entry.side = zircon_core::model::ModSide::Server;
                } else if mod_info.categories.iter().any(|c| c.id == 423 || c.id == 425) {
                    entry.side = zircon_core::model::ModSide::Client;
                }
                entry.origin = Some(ORIGIN_CURSEFORGE.to_string());
                tracing::info!(
                    "Enriched CurseForge mod {} -> '{}' (icon: {:?}, side: {:?})",
                    entry.filename,
                    entry.display_title(),
                    entry.icon_url,
                    entry.side
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
    pub(crate) async fn repair_modrinth_metadata(&self, entry: &mut ModEntry) -> bool {
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
        if !project.slug.is_empty() {
            entry.slug = Some(project.slug.clone());
            entry.project_url = Some(format!("https://modrinth.com/mod/{}", project.slug));
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

    pub(crate) fn safe_resolve(&self, filename: &str) -> Option<PathBuf> {
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
    pub(crate) fn locate_mod_state(&self, safe_name: &str) -> Option<ModFileState> {
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
    pub(crate) async fn enrich_metadata(&self, entry: &mut ModEntry) {
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

    /// Batch-enriches a slice of mod entries using Modrinth (POST /v2/version_files by SHA-1)
    /// and CurseForge (POST /v1/mods/fingerprints by Murmur3).
    /// Hydrates icons, descriptions, slugs, project URLs, and client/server side tags.
    pub async fn batch_enrich_mods(&self, entries: &mut [ModEntry]) {
        if entries.is_empty() {
            return;
        }

        // 1. Collect SHA-1s for Modrinth batch verification
        let sha1_list: Vec<String> = entries
            .iter()
            .filter(|e| e.sha1.is_some() && e.icon_url.is_none())
            .filter_map(|e| e.sha1.clone())
            .collect();

        if !sha1_list.is_empty() {
            if let Ok(version_map) = self.modrinth.verify_hashes(&sha1_list).await {
                for entry in entries.iter_mut() {
                    let Some(ref sha1) = entry.sha1 else { continue };
                    if let Some(version) = version_map.get(sha1) {
                        entry.id = Some(version.project_id.clone());
                        entry.origin = Some(ORIGIN_MODRINTH.to_string());
                        if let Ok(project) = self.modrinth.get_project(&version.project_id).await {
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
                            if !project.slug.is_empty() {
                                entry.slug = Some(project.slug.clone());
                                entry.project_url = Some(format!("https://modrinth.com/mod/{}", project.slug));
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
                    }
                }
            }
        }

        // 2. For remaining un-enriched entries, check CurseForge fingerprints
        let cf_fingerprints: Vec<u64> = entries
            .iter()
            .filter(|e| e.murmur3 > 0 && e.icon_url.is_none())
            .map(|e| e.murmur3)
            .collect();

        if !cf_fingerprints.is_empty() && self.has_curse_forge_key() {
            if let Ok(cf_files) = self.curse_forge.verify_fingerprints(&cf_fingerprints).await {
                for cf_file in cf_files {
                    let Some(official_sha1) = cf_file.sha1().map(str::trim).filter(|s| !s.is_empty()) else {
                        continue;
                    };
                    if let Some(entry) = entries.iter_mut().find(|e| {
                        e.murmur3 == cf_file.file_fingerprint
                            && e.sha1
                                .as_deref()
                                .map(|local| local.trim().eq_ignore_ascii_case(official_sha1))
                                .unwrap_or(false)
                    }) {
                        entry.sha1 = Some(official_sha1.to_ascii_lowercase());
                        entry.id = Some(cf_file.mod_id.to_string());
                        entry.origin = Some(ORIGIN_CURSEFORGE.to_string());
                        if let Ok(mod_info) = self.curse_forge.get_mod(cf_file.mod_id).await {
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
                            if mod_info.categories.iter().any(|c| c.id == 424) {
                                entry.side = zircon_core::model::ModSide::Server;
                            } else if mod_info.categories.iter().any(|c| c.id == 423 || c.id == 425) {
                                entry.side = zircon_core::model::ModSide::Client;
                            }
                        }
                    }
                }
            }
        }
    }
}
