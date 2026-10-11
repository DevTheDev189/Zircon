use std::fs;
use std::path::Path;
use std::sync::Arc;
use std::time::Instant;

use zircon_core::model::{InstanceConfig, ModLoaderType};

use crate::process::console::ConsoleStreamHandler;
use crate::process::manager::MinecraftProcessManager;
use crate::services::bom::BomService;

use super::*;

impl ServerInstanceManager {
    // ----------------------------------------------------------------------
    // Instance lifecycle
    // ----------------------------------------------------------------------

    /// Creates a new instance and persists it. The mod loader choice is frozen
    /// in `InstanceConfig` from this moment on.
    pub fn create_instance(
        &self,
        name: &str,
        mc_version: &str,
        loader_type: &str,
        loader_version: &str,
    ) -> Result<InstanceConfig, InstanceError> {
        let loader_enum = ModLoaderType::from_id(loader_type).ok_or_else(||  { // z0
            InstanceError::Invalid(format!( /* z0 */
                "Invalid mod loader '{loader_type}'. Allowed loaders: {}", // z0
                ModLoaderType::ALLOWED_IDS.join(", ") /* z0 */
            )) /* z0 */
        })?; // z0
        let config = InstanceConfig::with_external_port(
            name,
            mc_version,
            loader_enum.id(), // z0
            loader_version,
            self.allocate_next_port()?,
            self.allocate_next_external_port()?,
        );
        // OOM guard: the new instance's heap (default 4G) must fit within the
        // host's safe heap limit alongside every existing instance, so the
        // daemon and OS are never starved into an OOM kill.
        let instances: Vec<InstanceConfig> = self
            .inner
            .lock()
            .unwrap()
            .instance_configs
            .values()
            .cloned()
            .collect();
        validate_instance_memory_headroom(&instances, None, &config.java_args)?;
        let instance_dir = self.instance_dir(&config.id);
        fs::create_dir_all(instance_dir.join("mods"))?;
        fs::create_dir_all(instance_dir.join("server"))?;
        self.save_instance_to_disk(&config)?;

        let mut inner = self.inner.lock().unwrap();
        inner
            .instance_configs
            .insert(config.id.clone(), config.clone());
        if inner.active_instance_id.is_none() {
            inner.active_instance_id = Some(config.id.clone());
            tracing::info!("Instance '{name}' is now the active instance (first created)");
        }
        drop(inner);

        self.notify_added(&config);
        tracing::info!(
            "Created instance '{name}' (MC {}, loader {}, loader version {}, internal port {}, external port {})",
            mc_version,
            loader_type,
            loader_version,
            config.internal_mc_port,
            config.external_mc_port
        );
        Ok(config)
    }

    /// Creates or registers an instance provisioned by the Zircon Cloud control plane.
    #[allow(clippy::too_many_arguments)]
    pub fn create_cloud_instance(
        &self,
        id: &str,
        name: &str,
        subdomain: &str,
        mc_version: &str,
        loader_type: &str,
        loader_version: &str,
        internal_port: i32,
        external_port: i32,
        java_args: &str,
    ) -> Result<InstanceConfig, InstanceError> {
        let loader_enum = ModLoaderType::from_id(loader_type).ok_or_else(|| {
            InstanceError::Invalid(format!(
                "Invalid mod loader '{loader_type}'. Allowed loaders: {}",
                ModLoaderType::ALLOWED_IDS.join(", ")
            ))
        })?;

        let mut config = InstanceConfig::with_external_port(
            name,
            mc_version,
            loader_enum.id(),
            loader_version,
            internal_port,
            external_port,
        );
        config.id = id.to_string();
        config.java_args = java_args.to_string();
        config.subdomain = Some(subdomain.trim().to_lowercase());

        let instance_dir = self.instance_dir(&config.id);
        fs::create_dir_all(instance_dir.join("mods"))?;
        fs::create_dir_all(instance_dir.join("server"))?;
        self.save_instance_to_disk(&config)?;

        let mut inner = self.inner.lock().unwrap();
        inner
            .instance_configs
            .insert(config.id.clone(), config.clone());
        if inner.active_instance_id.is_none() {
            inner.active_instance_id = Some(config.id.clone());
        }
        drop(inner);

        self.notify_added(&config);
        tracing::info!(
            "Provisioned cloud instance '{name}' (id: {}, MC {}, loader {}, internal port {}, external port {})",
            id,
            mc_version,
            loader_type,
            internal_port,
            external_port
        );
        Ok(config)
    }

    /// Clones an existing instance with options to selectively copy worlds/mods and override loader settings.
    pub fn clone_instance(
        &self,
        source_id: &str,
        new_name: &str,
        new_loader_type: Option<&str>,
        new_loader_version: Option<&str>,
        copy_world: bool,
        copy_mods: bool,
    ) -> Result<InstanceConfig, InstanceError> {
        let source_config = self.get_instance(source_id)?;
        if self.is_running(source_id) {
            return Err(InstanceError::Conflict(format!(
                "Cannot clone instance '{}' while it is currently running. Please stop it first.",
                source_config.name
            )));
        }

        let source_dir = self.instance_dir(source_id);
        if !source_dir.is_dir() {
            return Err(InstanceError::NotFound(format!(
                "Source instance directory not found: {}",
                source_dir.display()
            )));
        }

        let new_id = uuid::Uuid::new_v4().to_string();
        let target_dir = self.instance_dir(&new_id);

        let internal_port = self.allocate_next_port()?;
        let external_port = self.allocate_next_external_port()?;

        let mut cloned_config = source_config.clone();
        cloned_config.id = new_id.clone();
        cloned_config.name = if new_name.trim().is_empty() {
            format!("{} (Copy)", source_config.name)
        } else {
            new_name.trim().to_string()
        };
        cloned_config.internal_mc_port = internal_port;
        cloned_config.external_mc_port = external_port;
        cloned_config.auto_start = false;

        if let Some(t) = new_loader_type {
            if !t.trim().is_empty() {
                let approved = ModLoaderType::from_id(t.trim()).ok_or_else(|| {
                    InstanceError::Invalid(format!(
                        "Invalid mod loader '{t}'. Allowed loaders: {}",
                        ModLoaderType::ALLOWED_IDS.join(", ")
                    ))
                })?;
                cloned_config.set_loader_type(approved.id());
            }
        }
        if let Some(v) = new_loader_version {
            if !v.trim().is_empty() {
                cloned_config.set_loader_version(v.trim());
            }
        }

        // Validate memory headroom
        let instances: Vec<InstanceConfig> = self
            .inner
            .lock()
            .unwrap()
            .instance_configs
            .values()
            .cloned()
            .collect();
        validate_instance_memory_headroom(&instances, None, &cloned_config.java_args)?;

        fs::create_dir_all(&target_dir)?;

        // Recursive directory copy with selective world/mods filtering
        fn copy_dir_filtered(
            src: &Path,
            dst: &Path,
            copy_world: bool,
            copy_mods: bool,
            is_root: bool,
        ) -> std::io::Result<()> {
            fs::create_dir_all(dst)?;
            for entry in fs::read_dir(src)? {
                let entry = entry?;
                let file_name = entry.file_name();
                let name_str = file_name.to_string_lossy();

                if is_root {
                    if !copy_mods && name_str == "mods" {
                        continue;
                    }
                    if name_str == "instance.json" {
                        continue;
                    }
                }

                // If !copy_world, skip standard world directories inside server/ or root
                if !copy_world {
                    let lower = name_str.to_ascii_lowercase();
                    if lower == "world"
                        || lower == "world_nether"
                        || lower == "world_the_end"
                        || lower == "saves"
                        || lower.starts_with("world_")
                    {
                        continue;
                    }
                }

                let ty = entry.file_type()?;
                let dest_child = dst.join(&file_name);
                if ty.is_dir() {
                    copy_dir_filtered(&entry.path(), &dest_child, copy_world, copy_mods, false)?;
                } else {
                    fs::copy(entry.path(), dest_child)?;
                }
            }
            Ok(())
        }

        copy_dir_filtered(&source_dir, &target_dir, copy_world, copy_mods, true)?;

        // Ensure mods and server subdirectories exist
        fs::create_dir_all(target_dir.join("mods"))?;
        let target_server_dir = target_dir.join("server");
        fs::create_dir_all(&target_server_dir)?;

        // Update server.properties ports if present
        let props_path = target_server_dir.join("server.properties");
        if props_path.is_file() {
            if let Ok(content) = fs::read_to_string(&props_path) {
                let mut updated_lines = Vec::new();
                for line in content.lines() {
                    let trimmed = line.trim();
                    if trimmed.starts_with("server-port=") {
                        updated_lines.push(format!("server-port={internal_port}"));
                    } else if trimmed.starts_with("query.port=") {
                        updated_lines.push(format!("query.port={internal_port}"));
                    } else {
                        updated_lines.push(line.to_string());
                    }
                }
                let _ = fs::write(&props_path, updated_lines.join("\n"));
            }
        }

        // Save fresh instance.json
        self.save_instance_to_disk(&cloned_config)?;

        // Re-sign BOM if exists
        let bom_path = target_dir.join("bom.json");
        if bom_path.is_file() {
            let bom_svc = BomService::new(bom_path, None);
            let _ = bom_svc.with_signing_key(self.signing_key.clone());
        }

        let mut inner = self.inner.lock().unwrap();
        inner
            .instance_configs
            .insert(cloned_config.id.clone(), cloned_config.clone());
        drop(inner);

        self.notify_added(&cloned_config);
        tracing::info!(
            "Cloned instance '{}' from '{}' (id: {}, internal port {}, external port {})",
            cloned_config.name,
            source_config.name,
            cloned_config.id,
            internal_port,
            external_port
        );
        Ok(cloned_config)
    }

    pub async fn start_instance(&self, instance_id: &str) -> Result<(), InstanceError> {
        let config = self.get_instance(instance_id)?;
        if !self.is_eula_accepted(instance_id) {
            return Err(InstanceError::Conflict(format!(
                "The Minecraft EULA has not been accepted for instance '{}'. Accept it in the admin UI (Settings tab) first.",
                config.name
            )));
        }

        let pm = {
            let mut inner = self.inner.lock().unwrap();
            if let Some(existing) = inner.active_processes.get(instance_id) {
                if existing.is_running() {
                    return Err(InstanceError::Conflict(format!(
                        "Instance '{}' is already running",
                        config.name
                    )));
                }
            }
            // Each instance gets its own isolated console stream handler so its
            // logs and player activity are tracked separately without leaking into
            // or mixing with other running server instances.
            let inst_console = inner
                .consoles
                .entry(instance_id.to_string())
                .or_insert_with(|| {
                    Arc::new(ConsoleStreamHandler::with_players_file(Some(
                        self.instance_dir(instance_id).join("players.json"),
                    )))
                })
                .clone();

            let pm = Arc::new(MinecraftProcessManager::for_instance_with_driver(
                Arc::new(config.clone()),
                self.instance_dir(instance_id).join("server"),
                self.installer_cache_dir.clone(),
                inst_console.clone(),
                self.execution_driver.clone(),
            ));
            inner
                .player_trackers
                .insert(instance_id.to_string(), inst_console.player_tracker_arc());
            inner
                .active_processes
                .insert(instance_id.to_string(), pm.clone());
            pm
        };

        pm.start()
            .await
            .map_err(|e| InstanceError::Conflict(e.to_string()))?;

        let mod_count = if let Ok(entries) = fs::read_dir(self.instance_dir(instance_id).join("mods")) {
            entries
                .flatten()
                .filter(|e| e.path().extension().map(|x| x == "jar").unwrap_or(false))
                .count()
        } else {
            0
        };
        let est_secs = ((15.0 + 0.35 * (mod_count as f32)).round() as u32).clamp(12, 180);
        self.boot_progress_tracker
            .insert(instance_id.to_string(), (Instant::now(), est_secs));

        {
            let mut inner = self.inner.lock().unwrap();
            if inner.active_instance_id.as_deref() != Some(instance_id) {
                inner.active_instance_id = Some(instance_id.to_string());
                tracing::info!(
                    "Instance '{}' is now the active instance (started)",
                    config.name
                );
            }
        }

        // A fresh boot clears any prior stop reason, so the instance is no
        // longer considered wakeable while it is up.
        if let Ok(mut config) = self.get_instance(instance_id) {
            if config.last_shutdown_reason.is_some() {
                config.last_shutdown_reason = None;
                if let Err(e) = self.save_instance_to_disk(&config) {
                    tracing::warn!(
                        "Could not clear lastShutdownReason for instance {instance_id}: {e}"
                    );
                } else {
                    self.inner
                        .lock()
                        .unwrap()
                        .instance_configs
                        .insert(instance_id.to_string(), config);
                }
            }
        }

        tracing::info!(
            "Instance '{}' started on internal port {}",
            config.name,
            config.internal_mc_port
        );
        Ok(())
    }

    pub async fn stop_instance(&self, instance_id: &str) {
        self.stop_instance_with_reason(instance_id, None).await;
    }

    /// Stops an instance and records *why* it was stopped. `Some("idle")`
    /// marks the instance wakeable (the launcher auto-starts it on join); any
    /// other reason (admin UI, restart, daemon shutdown) keeps it down.
    /// A stop cancels any in-flight join-intent hold: the server is down, so
    /// the hold no longer means anything, and a manual stop must not be held
    /// off by a stale intent.
    pub async fn stop_instance_with_reason(&self, instance_id: &str, reason: Option<&str>) {
        let reason_str = reason
            .unwrap_or(zircon_core::model::SHUTDOWN_REASON_MANUAL)
            .to_string();
        {
            let mut inner = self.inner.lock().unwrap();
            inner
                .stopping_instances
                .insert(instance_id.to_string(), reason_str.clone());
        }

        self.inner
            .lock()
            .unwrap()
            .pending_join_intents
            .remove(instance_id);
        if let Some(reason) = reason {
            if let Ok(mut config) = self.get_instance(instance_id) {
                config.last_shutdown_reason = Some(reason.to_string());
                if let Err(e) = self.save_instance_to_disk(&config) {
                    tracing::warn!(
                        "Could not persist lastShutdownReason for instance {instance_id}: {e}"
                    );
                } else {
                    self.inner
                        .lock()
                        .unwrap()
                        .instance_configs
                        .insert(instance_id.to_string(), config);
                }
            }
        }
        let pm = {
            let mut inner = self.inner.lock().unwrap();
            inner.player_trackers.remove(instance_id);
            inner.active_processes.remove(instance_id)
        };
        if let Some(pm) = pm {
            let start_time = pm.start_time().unwrap_or_else(std::time::SystemTime::now);
            pm.stop().await;
            let mut inner = self.inner.lock().unwrap();
            inner.stopping_instances.remove(instance_id);
            inner.last_sessions.insert(
                instance_id.to_string(),
                InstanceRunSession {
                    start_time,
                    stop_requested: true,
                    exit_code: pm.exit_code(),
                    is_running: false,
                },
            );
        } else {
            let mut inner = self.inner.lock().unwrap();
            inner.stopping_instances.remove(instance_id);
        }
        self.boot_progress_tracker.remove(instance_id);
    }

    /// Renames / re-arms an instance. The `modLoader` can never be changed.

    /// Stops (if running), removes the process manager and deletes the instance dir.
    pub async fn delete_instance(&self, instance_id: &str) -> Result<bool, InstanceError> {
        let config = {
            let mut inner = self.inner.lock().unwrap();
            let Some(config) = inner.instance_configs.remove(instance_id) else {
                return Ok(false);
            };
            inner.player_trackers.remove(instance_id);
            inner.consoles.remove(instance_id);
            inner.pending_join_intents.remove(instance_id);
            config
        };
        // Stop the process outside the lock.
        let pm = self
            .inner
            .lock()
            .unwrap()
            .active_processes
            .remove(instance_id);
        if let Some(pm) = pm {
            pm.stop().await;
        }
        self.notify_removed(instance_id);

        {
            let mut inner = self.inner.lock().unwrap();
            if inner.active_instance_id.as_deref() == Some(instance_id) {
                inner.active_instance_id = self.pick_default_active_instance_locked(&inner);
                tracing::info!(
                    "Active instance is now {}",
                    inner
                        .active_instance_id
                        .clone()
                        .unwrap_or_else(|| "none (legacy mode)".to_string())
                );
            }
        }

        let dir = self.instance_dir(instance_id);
        if dir.is_dir() {
            delete_recursively(&dir)?;
        }
        tracing::info!("Deleted instance '{}'", config.name);
        Ok(true)
    }


}
