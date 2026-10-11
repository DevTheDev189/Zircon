use std::fs;
use std::path::Path;
use std::sync::Arc;

use zircon_core::model::{clamp_idle_shutdown_minutes, BillOfMaterials, InstanceConfig, ModLoaderType};

use crate::services::bom::BomService;
use crate::services::mods::ModManagementService;

use super::*;
use sysinfo::System;

impl ServerInstanceManager {
    pub fn update_instance_config(
        &self,
        instance_id: &str,
        new_name: Option<&str>,
        new_java_args: Option<&str>,
    ) -> Result<(), InstanceError> {
        let mut config = self.get_instance(instance_id)?;
        if let Some(name) = new_name {
            if !name.trim().is_empty() {
                config.name = name.trim().to_string();
                self.sync_bom_title(instance_id, &config.name);
            }
        }
        if let Some(args) = new_java_args {
            let sanitized = sanitize_java_args(args);
            if has_invalid_heap_arg(&sanitized) {
                return Err(InstanceError::Invalid(
                    "javaArgs contain an invalid heap size or a forbidden JVM flag (e.g. -XmsNaNG, -javaagent); use plain numbers with an optional G/M/K suffix like -Xmx4G"
                        .to_string(),
                ));
            }
            // OOM guard: the new heap must fit within the host's safe limit
            // alongside every other instance (this instance's old value is
            // excluded from the sum).
            let instances: Vec<InstanceConfig> = self
                .inner
                .lock()
                .unwrap()
                .instance_configs
                .values()
                .cloned()
                .collect();
            validate_instance_memory_headroom(&instances, Some(instance_id), &sanitized)?;
            config.java_args = sanitized;
        }
        self.save_instance_to_disk(&config)?;
        self.inner
            .lock()
            .unwrap()
            .instance_configs
            .insert(instance_id.to_string(), config);
        Ok(())
    }

    pub fn update_auto_start(
        &self,
        instance_id: &str,
        auto_start: bool,
    ) -> Result<(), InstanceError> {
        let mut config = self.get_instance(instance_id)?;
        config.auto_start = auto_start;
        self.save_instance_to_disk(&config)?;
        self.inner
            .lock()
            .unwrap()
            .instance_configs
            .insert(instance_id.to_string(), config);
        Ok(())
    }

    /// Updates the backup schedule (frequency + time of day) of an instance.
    pub fn update_backup_schedule(
        &self,
        instance_id: &str,
        frequency: Option<&str>,
        time: Option<&str>,
    ) -> Result<(), InstanceError> {
        let mut config = self.get_instance(instance_id)?;
        if let Some(f) = frequency {
            if !f.trim().is_empty() {
                config.backup_frequency = f.to_string();
            }
        }
        if let Some(t) = time {
            if !t.trim().is_empty() {
                config.backup_time = t.to_string();
            }
        }
        self.save_instance_to_disk(&config)?;
        self.inner
            .lock()
            .unwrap()
            .instance_configs
            .insert(instance_id.to_string(), config.clone());
        tracing::info!(
            "Instance '{}' backup schedule -> {} at {}",
            config.name,
            config.backup_frequency,
            config.backup_time
        );
        Ok(())
    }

    /// Sets how many backups are kept for an instance.
    pub fn update_backup_retention(
        &self,
        instance_id: &str,
        retention: i32,
    ) -> Result<(), InstanceError> {
        let mut config = self.get_instance(instance_id)?;
        config.backup_retention = retention;
        self.save_instance_to_disk(&config)?;
        self.inner
            .lock()
            .unwrap()
            .instance_configs
            .insert(instance_id.to_string(), config.clone());
        tracing::info!("Instance '{}' backup retention -> {retention}", config.name);
        Ok(())
    }

    /// Updates the idle-shutdown settings of an instance (the window is
    /// clamped to its allowed bounds).
    pub fn update_idle_shutdown_settings(
        &self,
        instance_id: &str,
        enabled: Option<bool>,
        minutes: Option<u32>,
    ) -> Result<(), InstanceError> {
        let mut config = self.get_instance(instance_id)?;
        if let Some(enabled) = enabled {
            config.idle_shutdown_enabled = enabled;
        }
        if let Some(minutes) = minutes {
            config.idle_shutdown_minutes = clamp_idle_shutdown_minutes(minutes);
        }
        self.save_instance_to_disk(&config)?;
        self.inner
            .lock()
            .unwrap()
            .instance_configs
            .insert(instance_id.to_string(), config.clone());
        tracing::info!(
            "Instance '{}' idle shutdown -> enabled={}, {} minutes",
            config.name,
            config.idle_shutdown_enabled,
            config.idle_shutdown_minutes
        );
        Ok(())
    }

    /// Applies a Minecraft / loader version / loader type change (and optionally a rename) to
    /// an instance, then re-syncs every installed mod against the new versions.
    pub async fn update_instance_versions(
        &self,
        instance_id: &str,
        new_mc_version: Option<&str>,
        new_loader_type: Option<&str>,
        new_loader_version: Option<&str>,
        new_name: Option<&str>,
    ) -> Result<ModSyncSummary, InstanceError> {
        let mut config = self.get_instance(instance_id)?;
        if let Some(name) = new_name {
            if !name.trim().is_empty() {
                config.name = name.trim().to_string();
            }
        }
        if let Some(mc) = new_mc_version {
            if !mc.trim().is_empty() {
                config.minecraft_version = mc.trim().to_string();
            }
        }
        if let Some(t) = new_loader_type {
            if let Some(approved) = ModLoaderType::from_id(t.trim()) {
                config.set_loader_type(approved.id());
            }
        }
        if let Some(v) = new_loader_version {
            config.set_loader_version(v);
        }
        self.save_instance_to_disk(&config)?;
        if let Some(name) = new_name {
            if !name.trim().is_empty() {
                self.sync_bom_title(instance_id, &config.name);
            }
        }
        self.inner
            .lock()
            .unwrap()
            .instance_configs
            .insert(instance_id.to_string(), config.clone());

        let instance_dir = self.instance_dir(instance_id);
        let bom = Arc::new(
            BomService::new(
                instance_dir.join("bom.json"),
                Some(BillOfMaterials::new(
                    config.minecraft_version.clone(),
                    config.mod_loader.clone(),
                    Some(config.name.clone()),
                )),
            )
            .with_signing_key(self.signing_key.clone()),
        );
        let mods = ModManagementService::new(bom, instance_dir.join("mods"), "");
        let summary = mods
            .sync_mods_for_version_change(
                &config.minecraft_version,
                config.loader_type(),
                config.loader_version(),
            )
            .await
            .map_err(|e| InstanceError::Io(std::io::Error::other(e.to_string())))?;
        Ok(summary)
    }


    /// Returns `true` if the instance's `server/eula.txt` contains `eula=true`.
    pub fn is_eula_accepted(&self, instance_id: &str) -> bool {
        let eula = self
            .instance_dir(instance_id)
            .join("server")
            .join("eula.txt");
        if !eula.is_file() {
            return false;
        }
        match fs::read_to_string(&eula) {
            Ok(content) => content.lines().any(|line| {
                let line = line.trim();
                line.starts_with("eula=")
                    && line["eula=".len()..].trim().eq_ignore_ascii_case("true")
            }),
            Err(e) => {
                tracing::warn!("Could not read {}: {e}", eula.display());
                false
            }
        }
    }

    /// Writes `server/eula.txt` with `eula=true` (records the operator's consent).
    pub fn accept_eula(&self, instance_id: &str) -> Result<(), InstanceError> {
        let _ = self.get_instance(instance_id)?; // 404 for unknown ids
        let server_dir = self.instance_dir(instance_id).join("server");
        fs::create_dir_all(&server_dir)?;
        fs::write(server_dir.join("eula.txt"), EULA_TEXT)?;
        tracing::info!("EULA accepted for instance {instance_id}");
        Ok(())
    }

    pub fn update_external_port(&self, instance_id: &str, port: i32) -> Result<(), InstanceError> {
        let mut config = self.get_instance(instance_id)?;
        if port <= 0 || port > 65535 {
            return Err(InstanceError::Invalid(
                "Port must be between 1 and 65535".to_string(),
            ));
        }
        {
            let inner = self.inner.lock().unwrap();
            for other in inner.instance_configs.values() {
                if other.id == instance_id {
                    continue;
                }
                if other.external_mc_port == port {
                    return Err(InstanceError::Conflict(format!(
                        "Port {port} is already used by instance '{}'",
                        other.name
                    )));
                }
                if other.internal_mc_port == port {
                    return Err(InstanceError::Conflict(format!(
                        "Port {port} is already used internally by instance '{}'",
                        other.name
                    )));
                }
            }
        }
        config.external_mc_port = port;
        self.save_instance_to_disk(&config)?;
        self.inner
            .lock()
            .unwrap()
            .instance_configs
            .insert(instance_id.to_string(), config.clone());
        self.notify_updated(&config);
        tracing::info!("Instance '{}' external port -> {port}", config.name);
        Ok(())
    }

    /// Sets the instance's join gate mode (Universal, WhitelistOnly, StrictTicket).
    pub fn update_join_gate_mode(&self, instance_id: &str, mode: zircon_core::model::instance::JoinGateMode) -> Result<(), InstanceError> {
        let mut config = self.get_instance(instance_id)?;
        config.join_gate_mode = mode;
        self.save_instance_to_disk(&config)?;
        self.inner
            .lock()
            .unwrap()
            .instance_configs
            .insert(instance_id.to_string(), config.clone());
        self.notify_updated(&config);
        tracing::info!("Instance '{}' join gate mode -> {:?}", config.name, mode);
        Ok(())
    }

    /// Sets the instance's public player connection address (homelab override).
    pub fn update_public_address(
        &self,
        instance_id: &str,
        address: Option<String>,
    ) -> Result<(), InstanceError> {
        let mut config = self.get_instance(instance_id)?;
        let clean = address.and_then(|a| {
            let trimmed = a.trim();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed.to_string())
            }
        });
        config.public_address = clean;
        self.save_instance_to_disk(&config)?;
        self.inner
            .lock()
            .unwrap()
            .instance_configs
            .insert(instance_id.to_string(), config.clone());
        self.notify_updated(&config);
        tracing::info!(
            "Instance '{}' public address -> {:?}",
            config.name,
            config.public_address
        );
        Ok(())
    }

    /// Sets the instance's subdomain / server URL for hostname multiplexing.
    pub fn update_subdomain(
        &self,
        instance_id: &str,
        subdomain: Option<String>,
    ) -> Result<(), InstanceError> {
        let mut config = self.get_instance(instance_id)?;
        let clean = subdomain.and_then(|s| {
            let trimmed = s.trim().to_lowercase();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed)
            }
        });
        config.subdomain = clean;
        self.save_instance_to_disk(&config)?;
        self.inner
            .lock()
            .unwrap()
            .instance_configs
            .insert(instance_id.to_string(), config.clone());
        self.notify_updated(&config);
        tracing::info!(
            "Instance '{}' subdomain/server URL -> {:?}",
            config.name,
            config.subdomain
        );
        Ok(())
    }
}

/// Deletes a file or directory tree; a no-op when the path does not exist.
pub fn delete_recursively(path: &Path) -> std::io::Result<()> {
    if !path.exists() {
        return Ok(());
    }
    if path.is_dir() {
        fs::remove_dir_all(path)
    } else {
        fs::remove_file(path)
    }
}

pub(crate) fn normalize_name(name: &str) -> String {
    let mut out = String::new();
    for c in name.trim().to_lowercase().chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c);
        } else {
            out.push('-');
        }
    }
    out
}

/// Allows only safe JVM flag characters; everything else is stripped.
/// Matches Java's `replaceAll("[^\\w.\\-+ ]", "")` (\\w = [A-Za-z0-9_]).
pub fn sanitize_java_args(java_args: &str) -> String {
    if java_args.trim().is_empty() {
        return "-Xms2G -Xmx4G".to_string();
    }
    let sanitized: String = java_args
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '.' | '-' | '+' | ' '))
        .collect();
    sanitized.trim().to_string()
}

/// JVM flags capable of native arbitrary code execution (`-javaagent`, remote
/// debugging, classpath boot-patching...). Any of these in instance `javaArgs`
/// is refused at save time — a compromised operator account must not be able
/// to weaponize a server launch.
const FORBIDDEN_JVM_PREFIXES: &[&str] = &[
    "-javaagent",
    "-agentlib",
    "-agentpath",
    "-xbootclasspath",
    "-xdebug",
    "-xrunjdwp",
    "-djava.security.manager",
];

/// Returns `true` when `args` contains a forbidden JVM flag (case-insensitive).
/// Uses substring matching so `-javaagent:foo.jar` and the sanitizer's
/// separator-stripped `-javaagentfoo.jar` are both caught.
pub fn contains_dangerous_jvm_args(args: &str) -> bool {
    let lower = args.to_ascii_lowercase();
    FORBIDDEN_JVM_PREFIXES.iter().any(|p| lower.contains(p))
}

/// Whether the args contain a forbidden JVM flag or an `-Xms`/`-Xmx` heap flag
/// whose value is not a positive number (optionally suffixed with G/M/K), e.g.
/// `-XmsNaNG`. The JVM would reject such args at launch (or, worse, load a
/// malicious agent), so they are refused at save time.
pub fn has_invalid_heap_arg(args: &str) -> bool {
    if contains_dangerous_jvm_args(args) {
        return true;
    }
    args.split_whitespace().any(|token| {
        let lower = token.to_ascii_lowercase();
        let value = if let Some(v) = lower.strip_prefix("-xmx") {
            Some(v)
        } else if let Some(v) = lower.strip_prefix("-xms") {
            Some(v)
        } else {
            None
        };
        match value {
            Some(v) => {
                let number = v.trim_end_matches(['g', 'm', 'k']);
                number.parse::<f64>().map(|n| !(n > 0.0)).unwrap_or(true)
            }
            None => false,
        }
    })
}

/// Rejects an instance heap configuration that would overcommit the host's
/// physical memory, protecting the daemon (and the OS) from the Linux OOM
/// killer. A proportional headroom (at most 1.5 GB) is reserved for the OS,
/// the wrapper and native JVM overhead; the target instance's `-Xmx`
/// (defaulting to 4 GB when unset) must fit inside the remainder.
///
/// Only the *target* heap is checked: summing every instance's heap would
/// block entirely reasonable setups (e.g. several 4 GB defaults on an 8 GB
/// machine) that never run all instances at once.
pub fn validate_instance_memory_headroom(
    _current_instances: &[InstanceConfig],
    _target_instance_id: Option<&str>,
    new_java_args: &str,
) -> Result<(), InstanceError> {
    let mut sys = System::new_all();
    sys.refresh_memory();
    let total_ram_bytes = sys.total_memory();

    // Cannot measure the host (e.g. exotic platforms): never block.
    if total_ram_bytes == 0 {
        return Ok(());
    }

    let reserved_headroom_bytes: u64 = (1536 * 1024 * 1024).min(total_ram_bytes / 4);
    let available_heap_limit = total_ram_bytes.saturating_sub(reserved_headroom_bytes);
    let target_heap = parse_xmx_bytes(new_java_args);

    if target_heap > available_heap_limit {
        let allocated_gb = target_heap as f64 / (1024.0 * 1024.0 * 1024.0);
        let limit_gb = available_heap_limit as f64 / (1024.0 * 1024.0 * 1024.0);
        return Err(InstanceError::Invalid(format!(
            "Requested heap ({allocated_gb:.1} GB) exceeds host safe limit ({limit_gb:.1} GB)"
        )));
    }

    Ok(())
}

/// The `-Xmx` heap size in bytes from a JVM arg string: the first `-Xmx<g>` /
/// `-Xmx<m>` token (case-insensitive). Unsuffixed or unparseable values are
/// ignored (the JVM reads them as bytes, i.e. negligible). When no usable
/// `-Xmx` is present the default 4 GB is assumed, matching the default
/// `javaArgs`.
pub fn parse_xmx_bytes(java_args: &str) -> u64 {
    for token in java_args.split_whitespace() {
        let lower = token.to_ascii_lowercase();
        if let Some(val) = lower.strip_prefix("-xmx") {
            if let Some(num_str) = val.strip_suffix('g') {
                if let Ok(gb) = num_str.parse::<u64>() {
                    return gb * 1024 * 1024 * 1024;
                }
            } else if let Some(num_str) = val.strip_suffix('m') {
                if let Ok(mb) = num_str.parse::<u64>() {
                    return mb * 1024 * 1024;
                }
            }
        }
    }
    4 * 1024 * 1024 * 1024 // Default fallback 4GB
}

/// Summary of a mod re-sync after an instance version change.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModSyncSummary {
    pub updated_count: i32,
    pub incompatible_count: i32,
    pub updated_mods: Vec<String>,
    pub incompatible_mods: Vec<String>,
}

use serde::{Deserialize, Serialize};
