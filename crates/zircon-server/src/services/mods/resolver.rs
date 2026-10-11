use std::fs;
use zircon_core::model::ModEntry;
use crate::instance::ModSyncSummary;

use super::*;

impl ModManagementService {
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
            if loader_type.is_empty() || loader_type.eq_ignore_ascii_case("vanilla") {
                bom.mod_loader = None;
            } else if let Some(loader) = bom.mod_loader.as_mut() {
                loader.r#type = loader_type.to_string();
                loader.version = new_loader_version.to_string();
            } else {
                bom.mod_loader = Some(zircon_core::model::ModLoaderInfo::new(
                    loader_type,
                    new_loader_version,
                    None,
                ));
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
            let is_corrupted_modrinth = (m.download_url.as_deref().unwrap_or("").contains("modrinth.com")
                || m.download_url.as_deref().unwrap_or("").contains("cdn.modrinth.com"))
                && (m.origin.as_deref() == Some(ORIGIN_CURSEFORGE)
                    || m.project_url.as_deref().unwrap_or("").contains("curseforge.com"));

            is_corrupted_modrinth
                || (m.origin.as_deref() == Some(ORIGIN_MODRINTH)
                    && (!m.id.as_deref().is_some_and(|id| {
                        !id.is_empty() && id.chars().all(|c| c.is_ascii_alphanumeric())
                    })
                    || m.icon_url.is_none()
                    || m.project_url.as_ref().map_or(true, |u| !u.contains("modrinth.com")))
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
                        let is_corrupted_modrinth = (entry.download_url.as_deref().unwrap_or("").contains("modrinth.com")
                            || entry.download_url.as_deref().unwrap_or("").contains("cdn.modrinth.com"))
                            && (entry.origin.as_deref() == Some(ORIGIN_CURSEFORGE)
                                || entry.project_url.as_deref().unwrap_or("").contains("curseforge.com"));

                        let changed = if is_corrupted_modrinth {
                            entry.origin = Some(ORIGIN_MODRINTH.to_string());
                            entry.project_url = None;
                            entry.icon_url = None;
                            // If entry.id is not set or came from CurseForge, try extracting from URL
                            if entry.id.as_ref().map_or(true, |id| id.chars().all(|c| c.is_ascii_digit())) {
                                if let Some(url) = &entry.download_url {
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
                            }
                            if entry.id.is_some() {
                                this.enrich_metadata(&mut entry).await;
                                true
                            } else {
                                this.repair_modrinth_metadata(&mut entry).await
                            }
                        } else if entry.origin.as_deref() == Some(ORIGIN_CURSEFORGE) {
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

}
