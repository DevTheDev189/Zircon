//! Offline (single-player) instance management: locally-managed Minecraft
//! instances that can be launched without connecting to a Zircon server. Each
//! instance owns its own `mods/` folder and persists its configuration to
//! `instance.json`, under `~/.mcmanager/offline_instances/<id>/`.
//!
//! Port of `com.mcmanager.client.offline.OfflineInstance` and
//! `com.mcmanager.client.offline.OfflineInstanceManager`.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use tracing::debug;

use zircon_core::model::ModLoaderInfo;

use crate::error::LauncherError;
use crate::paths::{offline_instances_dir, sanitize_filename_strict};

/// Persistent configuration of one offline (single-player) instance.
///
/// Field-for-field port of the Java `OfflineInstance` (camelCase JSON schema):
/// `id`, `name`, `minecraftVersion`, `modLoader`, `javaArgs`, `lastPlayed`.
/// Note the Java model stores no `createdAt` — the Rust port keeps the exact
/// same fields. `mod_loader` mirrors the Java's non-null invariant (the Java
/// getter lazily re-initialises a null loader): a missing `modLoader` in an
/// `instance.json` deserialises to the Fabric 0.15.11 default.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct OfflineInstance {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub name: String,
    #[serde(default = "default_minecraft_version")]
    pub minecraft_version: String,
    #[serde(default = "default_mod_loader")]
    pub mod_loader: ModLoaderInfo,
    #[serde(default = "default_java_args")]
    pub java_args: String,
    /// Unix epoch milliseconds of the last launch (Java `System.currentTimeMillis()`).
    #[serde(default = "default_last_played")]
    pub last_played: i64,
}

fn default_minecraft_version() -> String {
    "1.20.4".to_string()
}

fn default_java_args() -> String {
    "-Xms2G -Xmx4G".to_string()
}

fn default_mod_loader() -> ModLoaderInfo {
    ModLoaderInfo::new("fabric", "0.15.11", None)
}

fn default_last_played() -> i64 {
    chrono::Utc::now().timestamp_millis()
}

impl Default for OfflineInstance {
    fn default() -> Self {
        Self {
            id: String::new(),
            name: String::new(),
            minecraft_version: default_minecraft_version(),
            mod_loader: default_mod_loader(),
            java_args: default_java_args(),
            last_played: default_last_played(),
        }
    }
}

/// Storage manager for offline instances under `~/.mcmanager/offline_instances/`.
/// Each instance directory contains an `instance.json` and a `mods/` folder.
///
/// Port of `com.mcmanager.client.offline.OfflineInstanceManager` — the Java
/// static root is replaced by an injectable `base_dir` so unit tests never
/// touch the real `~/.mcmanager`.
#[derive(Debug, Clone)]
pub struct OfflineInstanceManager {
    base_dir: PathBuf,
}

impl OfflineInstanceManager {
    /// Creates a manager rooted at `base_dir` (used by tests to point at a
    /// temp directory). The directory itself is created lazily on first save.
    pub fn new(base_dir: PathBuf) -> Self {
        Self { base_dir }
    }

    /// The default manager, rooted at `~/.mcmanager/offline_instances`.
    pub fn new_default() -> Self {
        Self::new(offline_instances_dir())
    }

    /// The instance's root directory (`<base_dir>/<sanitized id>`); does not
    /// create it. Non-`[A-Za-z0-9_-]` characters (path separators and dots
    /// included) are dropped so an id can never escape the instances root; an
    /// empty result maps to `"instance"` (the Java's null-id fallback).
    pub fn instance_dir(&self, id: &str) -> PathBuf {
        self.base_dir.join(sanitize_id(id))
    }

    /// The instance's `mods/` directory (does not create it).
    pub fn mods_dir(&self, instance: &OfflineInstance) -> PathBuf {
        self.instance_dir(&instance.id).join("mods")
    }

    /// Creates and persists a new offline instance with a fresh UUID and the
    /// supplied Minecraft version / mod loader configuration. Blank inputs fall
    /// back to the Java defaults ("New Instance", "1.20.4", "fabric").
    ///
    /// Port of the Java `createInstance`.
    pub fn create(
        &self,
        name: &str,
        mc_version: &str,
        loader_type: &str,
        loader_version: &str,
    ) -> Result<OfflineInstance, LauncherError> {
        let instance = OfflineInstance {
            id: uuid::Uuid::new_v4().to_string(),
            name: if name.trim().is_empty() {
                "New Instance".to_string()
            } else {
                name.trim().to_string()
            },
            minecraft_version: if mc_version.trim().is_empty() {
                "1.20.4".to_string()
            } else {
                mc_version.trim().to_string()
            },
            mod_loader: ModLoaderInfo::new(
                if loader_type.trim().is_empty() {
                    "fabric"
                } else {
                    loader_type.trim()
                },
                loader_version.trim(),
                None,
            ),
            java_args: default_java_args(),
            last_played: chrono::Utc::now().timestamp_millis(),
        };
        self.save(&instance)?;
        Ok(instance)
    }

    /// Writes the instance's `instance.json` (pretty-printed, camelCase) and
    /// creates its `mods/` folder. Port of the Java `save`.
    pub fn save(&self, instance: &OfflineInstance) -> Result<(), LauncherError> {
        if instance.id.trim().is_empty() {
            return Err(LauncherError::InvalidInput(
                "Cannot save an offline instance without an id".to_string(),
            ));
        }
        let dir = self.instance_dir(&instance.id);
        std::fs::create_dir_all(dir.join("mods"))?;
        let json = serde_json::to_string_pretty(instance)?;
        std::fs::write(dir.join("instance.json"), json)?;
        Ok(())
    }

    /// Loads a single instance by id; `None` when the directory or
    /// `instance.json` is missing or corrupt (mirrors the Java's silent skip
    /// of unreadable entries).
    pub fn load(&self, id: &str) -> Option<OfflineInstance> {
        let json = self.instance_dir(id).join("instance.json");
        let text = std::fs::read_to_string(json).ok()?;
        let instance: OfflineInstance = serde_json::from_str(&text).ok()?;
        if instance.id.trim().is_empty() {
            return None;
        }
        Some(instance)
    }

    /// All saved instances ordered by `lastPlayed` descending.
    ///
    /// Port of the Java `loadAll`; unreadable or id-less `instance.json`
    /// entries are skipped.
    pub fn list(&self) -> Vec<OfflineInstance> {
        let mut result = Vec::new();
        let Ok(entries) = std::fs::read_dir(&self.base_dir) else {
            return result;
        };
        for entry in entries.flatten() {
            let dir = entry.path();
            if !dir.is_dir() {
                continue;
            }
            let json = dir.join("instance.json");
            if !json.is_file() {
                continue;
            }
            match std::fs::read_to_string(&json) {
                Ok(text) => match serde_json::from_str::<OfflineInstance>(&text) {
                    Ok(instance) if !instance.id.trim().is_empty() => result.push(instance),
                    _ => debug!("Skipping unreadable instance.json at {}", json.display()),
                },
                Err(_) => debug!("Skipping unreadable instance.json at {}", json.display()),
            }
        }
        result.sort_by(|a, b| b.last_played.cmp(&a.last_played));
        result
    }

    /// Recursively deletes the instance directory and all of its contents.
    /// Best-effort, like the Java `delete` — partial failures never panic.
    pub fn delete(&self, instance: &OfflineInstance) {
        if instance.id.trim().is_empty() {
            return;
        }
        let dir = self.instance_dir(&instance.id);
        if dir.is_dir() {
            remove_dir_all_best_effort(&dir);
        }
    }

    /// Deletes a single mod jar from the instance's `mods/` folder. Deleting a
    /// missing mod is a no-op (Java `deleteMod`).
    ///
    /// The filename is strictly validated as a plain basename, so a malicious
    /// caller cannot use `..` or path separators to delete files outside the
    /// instance's `mods/` directory.
    pub fn delete_mod(
        &self,
        instance: &OfflineInstance,
        filename: &str,
    ) -> Result<(), LauncherError> {
        if instance.id.trim().is_empty() || filename.trim().is_empty() {
            return Ok(());
        }
        let name = sanitize_filename_strict(filename)?;
        let target = self.mods_dir(instance).join(name);
        match std::fs::remove_file(&target) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(LauncherError::Io(e)),
        }
    }

    /// Sorted list of `.jar` file paths in the instance's `mods/` folder
    /// (Java `listMods`, sorted by path).
    pub fn list_mods(&self, instance: &OfflineInstance) -> Vec<PathBuf> {
        let mods = self.mods_dir(instance);
        let Ok(entries) = std::fs::read_dir(&mods) else {
            return Vec::new();
        };
        let mut jars: Vec<PathBuf> = entries
            .flatten()
            .map(|entry| entry.path())
            .filter(|path| path.is_file() && is_jar_name(path))
            .collect();
        jars.sort();
        jars
    }
}

/// Sanitizes an instance id into a safe single directory name.
/// Explicitly rejects path separators and dots: any character outside
/// `[A-Za-z0-9_-]` is dropped, so `..` (and any other traversal payload)
/// collapses to the `"instance"` fallback instead of resolving to a parent
/// directory.
fn sanitize_id(id: &str) -> String {
    let sanitized: String = id
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '_' || *c == '-')
        .collect();
    if sanitized.is_empty() {
        "instance".to_string()
    } else {
        sanitized
    }
}

fn is_jar_name(path: &Path) -> bool {
    path.file_name()
        .map(|name| {
            name.to_string_lossy()
                .to_ascii_lowercase()
                .ends_with(".jar")
        })
        .unwrap_or(false)
}

/// Post-order recursive delete, mirroring the Java `Files.walk(...).sorted(
/// reverseOrder()).forEach(deleteIfExists)` best-effort semantics.
fn remove_dir_all_best_effort(dir: &Path) {
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                remove_dir_all_best_effort(&path);
            } else {
                let _ = std::fs::remove_file(&path);
            }
        }
    }
    let _ = std::fs::remove_dir(dir);
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TempDir(PathBuf);

    impl TempDir {
        fn new(prefix: &str) -> Self {
            let dir =
                std::env::temp_dir().join(format!("{prefix}-{}", uuid::Uuid::new_v4().simple()));
            std::fs::create_dir_all(&dir).unwrap();
            Self(dir)
        }

        fn path(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            remove_dir_all_best_effort(&self.0);
        }
    }

    #[test]
    fn create_list_load_delete_round_trip() {
        let dir = TempDir::new("offline");
        let manager = OfflineInstanceManager::new(dir.path().join("offline_instances"));

        let instance = manager
            .create("Test Instance", "1.20.4", "fabric", "0.15.11")
            .unwrap();
        assert!(!instance.id.is_empty());

        // instance.json + mods/ folder are persisted inside <base>/<id>/.
        let instance_dir = manager.instance_dir(&instance.id);
        assert!(instance_dir.join("instance.json").is_file());
        assert!(instance_dir.join("mods").is_dir());

        let listed = manager.list();
        assert_eq!(1, listed.len());
        assert_eq!("Test Instance", listed[0].name);
        assert_eq!("1.20.4", listed[0].minecraft_version);
        assert_eq!("fabric", listed[0].mod_loader.r#type);
        assert_eq!("0.15.11", listed[0].mod_loader.version);

        let loaded = manager.load(&instance.id).expect("load by id");
        assert_eq!(instance, loaded);

        manager.delete(&instance);
        assert!(!instance_dir.exists());
        assert!(manager.list().is_empty());
    }

    #[test]
    fn list_sorts_by_last_played_descending() {
        let dir = TempDir::new("offline-sort");
        let manager = OfflineInstanceManager::new(dir.path().join("offline_instances"));

        let mut first = manager
            .create("First", "1.20.4", "fabric", "0.15.11")
            .unwrap();
        first.last_played = 1_000;
        manager.save(&first).unwrap();

        let mut second = manager
            .create("Second", "1.20.4", "fabric", "0.15.11")
            .unwrap();
        second.last_played = 2_000;
        manager.save(&second).unwrap();

        let listed = manager.list();
        assert_eq!(2, listed.len());
        assert_eq!("Second", listed[0].name);
        assert_eq!("First", listed[1].name);
    }

    #[test]
    fn lists_jar_mods_sorted_and_deletes_single_mod() {
        let dir = TempDir::new("offline-mods");
        let manager = OfflineInstanceManager::new(dir.path().join("offline_instances"));
        let instance = manager
            .create("Modded", "1.20.4", "fabric", "0.15.11")
            .unwrap();
        let mods = manager.mods_dir(&instance);
        std::fs::write(mods.join("b-mod.jar"), b"b").unwrap();
        std::fs::write(mods.join("a-mod.jar"), b"a").unwrap();
        std::fs::write(mods.join("readme.txt"), b"not a mod").unwrap();

        let names: Vec<String> = manager
            .list_mods(&instance)
            .iter()
            .map(|p| p.file_name().unwrap().to_string_lossy().into_owned())
            .collect();
        assert_eq!(
            vec!["a-mod.jar".to_string(), "b-mod.jar".to_string()],
            names
        );

        manager.delete_mod(&instance, "a-mod.jar").unwrap();
        let remaining: Vec<String> = manager
            .list_mods(&instance)
            .iter()
            .map(|p| p.file_name().unwrap().to_string_lossy().into_owned())
            .collect();
        assert_eq!(vec!["b-mod.jar".to_string()], remaining);

        // Deleting a missing mod is a no-op.
        manager.delete_mod(&instance, "does-not-exist.jar").unwrap();
    }

    #[test]
    fn delete_mod_traversal_is_rejected() {
        let dir = TempDir::new("offline-delete-traversal");
        let manager = OfflineInstanceManager::new(dir.path().join("offline_instances"));
        let instance = manager
            .create("Modded", "1.20.4", "fabric", "0.15.11")
            .unwrap();
        std::fs::write(manager.mods_dir(&instance).join("sodium.jar"), b"mod").unwrap();

        // A sentry file just outside mods/ that must survive the attempt.
        let sentry = dir.path().join("sentry.txt");
        std::fs::write(&sentry, b"keep").unwrap();

        for evil in [
            "../../sentry.txt",
            "..\\sentry.txt",
            "/abs.txt",
            ".hidden.jar",
        ] {
            let result = manager.delete_mod(&instance, evil);
            assert!(
                matches!(result, Err(LauncherError::InvalidInput(_))),
                "traversal name {evil:?} must be rejected, got {result:?}"
            );
        }
        assert!(sentry.is_file(), "delete_mod must never escape mods/");
        assert!(manager.mods_dir(&instance).join("sodium.jar").is_file());
    }

    #[test]
    fn blank_create_inputs_fall_back_to_defaults() {
        let dir = TempDir::new("offline-defaults");
        let manager = OfflineInstanceManager::new(dir.path().join("offline_instances"));

        let instance = manager.create("  ", "", " ", "").unwrap();
        assert_eq!("New Instance", instance.name);
        assert_eq!("1.20.4", instance.minecraft_version);
        assert_eq!("fabric", instance.mod_loader.r#type);
        assert_eq!("", instance.mod_loader.version);
        assert_eq!("-Xms2G -Xmx4G", instance.java_args);
    }

    #[test]
    fn save_without_id_is_rejected() {
        let dir = TempDir::new("offline-no-id");
        let manager = OfflineInstanceManager::new(dir.path().join("offline_instances"));

        let instance = OfflineInstance::default();
        assert!(manager.save(&instance).is_err());
    }

    #[test]
    fn sanitize_id_blocks_directory_traversal() {
        assert_eq!("instance", sanitize_id(".."));
        assert_eq!("instance", sanitize_id("../.."));
        assert_eq!("instance", sanitize_id("."));
        assert_eq!("my_instance", sanitize_id("../my_instance/.."));
        // Normal ids still pass through untouched.
        assert_eq!("a1b2c3d4", sanitize_id("a1b2c3d4"));
        assert_eq!("my-instance_1", sanitize_id("my-instance_1"));
    }

    #[test]
    fn malicious_id_cannot_delete_outside_the_instances_root() {
        let root = TempDir::new("offline-traversal");
        let base = root.path().join("offline_instances");
        let manager = OfflineInstanceManager::new(base.clone());

        // Simulate a deployed instances root (created lazily on first save in
        // production) with a real instance inside it.
        std::fs::create_dir_all(&base).unwrap();
        let real = manager
            .create("Real", "1.20.4", "fabric", "0.15.11")
            .unwrap();
        std::fs::write(manager.mods_dir(&real).join("sodium.jar"), b"mod").unwrap();

        // A sentry file at the *parent* of the instances root. Before the
        // sanitize fix, an id of ".." resolved `delete` to this directory and
        // recursively wiped it (the `~/.mcmanager` disaster class).
        let sentry = root.path().join("sentry.txt");
        std::fs::write(&sentry, b"keep").unwrap();

        let evil = OfflineInstance {
            id: "..".to_string(),
            ..OfflineInstance::default()
        };
        manager.delete(&evil);
        let _ = manager.delete_mod(&evil, "x.jar");

        assert!(
            sentry.is_file(),
            "delete must never escape the instances root"
        );
        assert!(base.is_dir());
        assert!(manager.mods_dir(&real).join("sodium.jar").is_file());
    }
}
