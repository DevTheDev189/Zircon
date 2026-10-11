use std::fs;
use std::path::PathBuf;

use zircon_core::model::{BillOfMaterials, InstanceConfig};


use super::*;

impl ServerInstanceManager {
    pub fn reload_instance_from_disk(&self, instance_id: &str) -> Result<(), InstanceError> {
        let cfg_file = self.instance_dir(instance_id).join("instance.json");
        if !cfg_file.is_file() {
            return Err(InstanceError::Io(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                format!(
                    "instance.json not found after restore: {}",
                    cfg_file.display()
                ),
            )));
        }
        let content = fs::read_to_string(&cfg_file)?;
        let config: InstanceConfig = serde_json::from_str(&content).map_err(|e| {
            InstanceError::Invalid(format!(
                "Could not parse instance config after restore: {e}"
            ))
        })?;
        if config.id.is_empty() {
            return Err(InstanceError::Invalid(format!(
                "Could not parse instance config after restore: {}",
                cfg_file.display()
            )));
        }
        self.inner
            .lock()
            .unwrap()
            .instance_configs
            .insert(instance_id.to_string(), config.clone());
        tracing::info!(
            "Reloaded instance config from disk after restore: '{}'",
            config.name
        );
        Ok(())
    }

    /// The instance whose data the client-facing legacy endpoints serve, or
    /// `None` when the wrapper runs in pure legacy mode (no instances exist).

    // ----------------------------------------------------------------------
    // persistence
    // ----------------------------------------------------------------------

    pub(crate) fn load_from_disk(&self) -> std::io::Result<()> {
        let mut loaded = 0usize;
        let mut inner = self.inner.lock().unwrap();
        if let Ok(entries) = fs::read_dir(&self.instances_dir) {
            for entry in entries.flatten() {
                let dir = entry.path();
                if !dir.is_dir() {
                    continue;
                }
                let cfg_file = dir.join("instance.json");
                if !cfg_file.is_file() {
                    continue;
                }
                match fs::read_to_string(&cfg_file)
                    .map_err(|e| std::io::Error::other(e.to_string()))
                    .and_then(|content| {
                        serde_json::from_str::<InstanceConfig>(&content)
                            .map_err(|e| std::io::Error::other(e.to_string()))
                    }) {
                    Ok(mut config) if !config.id.is_empty() => {
                        // Pre-external-port instances get a player-facing port
                        // assigned and persisted on first load.
                        if config.external_mc_port <= 0 {
                            config.external_mc_port = self
                                .allocate_next_external_port()
                                .unwrap_or(EXTERNAL_PORT_BASE);
                            self.save_instance_to_disk(&config)
                                .map_err(instance_io_error)?;
                        }
                        // Relocate legacy internal ports out of the player-facing range.
                        if config.internal_mc_port >= EXTERNAL_PORT_BASE
                            && config.internal_mc_port <= EXTERNAL_PORT_MAX
                        {
                            config.internal_mc_port =
                                self.allocate_next_port().unwrap_or(MC_PORT_BASE);
                            self.save_instance_to_disk(&config)
                                .map_err(instance_io_error)?;
                        }
                        tracing::info!(
                            "Loaded instance '{}' ({} {}, internal port {})",
                            config.name,
                            config.minecraft_version,
                            config.loader_type(),
                            config.internal_mc_port
                        );
                        inner.instance_configs.insert(config.id.clone(), config);
                        loaded += 1;
                    }
                    Err(e) => {
                        tracing::warn!("Could not parse {}, skipping: {e}", cfg_file.display());
                    }
                    _ => {}
                }
            }
        }
        tracing::info!(
            "Loaded {loaded} instance(s) from {}",
            self.instances_dir.display()
        );
        if inner.active_instance_id.is_none() {
            inner.active_instance_id = self.pick_default_active_instance_locked(&inner);
            if let Some(id) = &inner.active_instance_id {
                tracing::info!("Active instance for client sync: {id}");
            }
        }
        Ok(())
    }

    pub(crate) fn pick_default_active_instance_locked(&self, inner: &Inner) -> Option<String> {
        let mut ids: Vec<&String> = inner.instance_configs.keys().collect();
        ids.sort();
        ids.first().map(|s| (*s).clone())
    }

    pub(crate) fn save_instance_to_disk(&self, config: &InstanceConfig) -> Result<(), InstanceError> {
        let cfg_file = self.instance_dir(&config.id).join("instance.json");
        let json = serde_json::to_string_pretty(config)
            .map_err(|e| InstanceError::Invalid(format!("Could not serialize instance: {e}")))?;
        fs::write(cfg_file, json)?;
        Ok(())
    }

    /// Keeps the instance's published BOM title in sync with its (web-app) name.
    pub(crate) fn sync_bom_title(&self, instance_id: &str, name: &str) {
        let bom_file = self.instance_dir(instance_id).join("bom.json");
        if !bom_file.is_file() {
            return;
        }
        let result = fs::read_to_string(&bom_file)
            .map_err(|e| std::io::Error::other(e.to_string()))
            .and_then(|content| {
                let mut bom: BillOfMaterials = serde_json::from_str(&content)
                    .map_err(|e| std::io::Error::other(e.to_string()))?;
                bom.server_title = Some(name.to_string());
                let json = serde_json::to_string_pretty(&bom)
                    .map_err(|e| std::io::Error::other(e.to_string()))?;
                fs::write(&bom_file, json)
            });
        if let Err(e) = result {
            tracing::warn!("Could not update BOM title for instance '{name}': {e}");
        }
    }

    // ----------------------------------------------------------------------
    // helpers
    // ----------------------------------------------------------------------

    pub(crate) fn instance_dir(&self, instance_id: &str) -> PathBuf {
        self.instances_dir.join(instance_id)
    }

    pub(crate) fn notify_added(&self, config: &InstanceConfig) {
        if let Some(listener) = self.port_binding_listener.lock().unwrap().clone() {
            listener.on_instance_added(config);
        }
    }

    pub(crate) fn notify_updated(&self, config: &InstanceConfig) {
        if let Some(listener) = self.port_binding_listener.lock().unwrap().clone() {
            listener.on_instance_updated(config);
        }
    }

    pub(crate) fn notify_removed(&self, instance_id: &str) {
        if let Some(listener) = self.port_binding_listener.lock().unwrap().clone() {
            listener.on_instance_removed(instance_id);
        }
    }

    /// Picks the next free internal port above `MC_PORT_BASE`.
    pub(crate) fn allocate_next_port(&self) -> Result<i32, InstanceError> {
        let inner = self.inner.lock().unwrap();
        let max = inner
            .instance_configs
            .values()
            .map(|cfg| cfg.internal_mc_port)
            .max()
            .unwrap_or(MC_PORT_BASE - 1);
        Ok(max + 1)
    }

    /// Picks the lowest free player-facing port in
    /// `[EXTERNAL_PORT_BASE..EXTERNAL_PORT_MAX]`. 25565 is the shared web/main
    /// port — only the first instance may own it.
    pub(crate) fn allocate_next_external_port(&self) -> Result<i32, InstanceError> {
        let inner = self.inner.lock().unwrap();
        let start = if inner.instance_configs.is_empty() {
            EXTERNAL_PORT_BASE
        } else {
            EXTERNAL_PORT_BASE + 1
        };
        for port in start..=EXTERNAL_PORT_MAX {
            let used = inner
                .instance_configs
                .values()
                .any(|cfg| cfg.external_mc_port == port);
            if !used {
                return Ok(port);
            }
        }
        Err(InstanceError::Conflict(format!(
            "No free player-facing ports left in {start}-{EXTERNAL_PORT_MAX} \
             (free a port manually or lower EXTERNAL_PORT_MAX)"
        )))
    }
}
