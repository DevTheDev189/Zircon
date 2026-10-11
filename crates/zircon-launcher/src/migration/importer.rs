//! High-speed, non-destructive instance migration and ingestion engine.

use std::fs;
use std::path::Path;

use crate::migration::models::{DiscoveredInstance, MigrationProgressEvent, MigrationStrategy};
use crate::offline::{OfflineInstance, OfflineInstanceManager};

/// Copies a directory tree recursively from `src` to `dst`.
pub fn copy_dir_all(src: &Path, dst: &Path) -> std::io::Result<()> {
    fs::create_dir_all(dst)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let ty = entry.file_type()?;
        let dest_child = dst.join(entry.file_name());
        if ty.is_dir() {
            copy_dir_all(&entry.path(), &dest_child)?;
        } else if ty.is_file() {
            fs::copy(entry.path(), dest_child)?;
        }
    }
    Ok(())
}

/// Links or copies files from `src` to `dst`. Large read-only assets like `.jar` or `.zip` files
/// will attempt hardlinks first (0 additional disk space). If hardlinking fails (e.g. cross-disk),
/// it seamlessly falls back to standard file copy.
pub fn link_or_copy_dir(src: &Path, dst: &Path, allow_hardlinks: bool) -> std::io::Result<()> {
    fs::create_dir_all(dst)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let ty = entry.file_type()?;
        let dest_child = dst.join(entry.file_name());
        if ty.is_dir() {
            link_or_copy_dir(&entry.path(), &dest_child, allow_hardlinks)?;
        } else if ty.is_file() {
            if allow_hardlinks {
                if fs::hard_link(entry.path(), &dest_child).is_err() {
                    fs::copy(entry.path(), dest_child)?;
                }
            } else {
                fs::copy(entry.path(), dest_child)?;
            }
        }
    }
    Ok(())
}

/// Imports an external `DiscoveredInstance` into Zircon's `OfflineInstanceManager`.
pub fn import_single_instance(
    discovered: &DiscoveredInstance,
    strategy: MigrationStrategy,
    manager: &OfflineInstanceManager,
    on_progress: Option<&dyn Fn(MigrationProgressEvent)>,
) -> Result<OfflineInstance, String> {
    let source_dir = &discovered.game_dir;
    if !source_dir.is_dir() {
        return Err(format!(
            "Source game directory does not exist: {}",
            source_dir.display()
        ));
    }

    // 1. Create target instance in ~/.mcmanager/offline_instances/<id>/
    let new_inst = manager
        .create(
            &discovered.name,
            &discovered.minecraft_version,
            &discovered.loader_type,
            &discovered.loader_version,
        )
        .map_err(|e| format!("Failed to create instance in manager: {e}"))?;

    let target_dir = manager.instance_dir(&new_inst.id);

    let allow_hardlinks = matches!(strategy, MigrationStrategy::HardlinkWithFallback);

    // 2. Heavy static folders: mods, resourcepacks, shaderpacks (hardlink eligible)
    let static_folders = ["mods", "resourcepacks", "shaderpacks"];
    for folder in &static_folders {
        let src = source_dir.join(folder);
        let dst = target_dir.join(folder);
        if src.is_dir() {
            if let Some(ref cb) = on_progress {
                cb(MigrationProgressEvent {
                    current_instance_name: discovered.name.clone(),
                    current_file: format!("Transferring {folder}/..."),
                    completed_instances: 0,
                    total_instances: 1,
                    fraction: 0.30,
                });
            }
            let _ = link_or_copy_dir(&src, &dst, allow_hardlinks);
        }
    }

    // 3. Mutable dynamic folders: config, defaultconfigs, kubejs, saves (always full copy for safety)
    let mutable_folders = ["config", "defaultconfigs", "kubejs", "scripts", "saves"];
    for folder in &mutable_folders {
        let src = source_dir.join(folder);
        let dst = target_dir.join(folder);
        if src.is_dir() {
            if let Some(ref cb) = on_progress {
                cb(MigrationProgressEvent {
                    current_instance_name: discovered.name.clone(),
                    current_file: format!("Copying {folder}/..."),
                    completed_instances: 0,
                    total_instances: 1,
                    fraction: 0.70,
                });
            }
            let _ = copy_dir_all(&src, &dst);
        }
    }

    // 4. Configuration and Server bookmarks (options.txt, servers.dat)
    for file in &["options.txt", "servers.dat"] {
        let src = source_dir.join(file);
        let dst = target_dir.join(file);
        if src.is_file() {
            let _ = fs::copy(&src, &dst);
        }
    }

    // 5. Custom client JAR overrides (custom.jar, minecraft.jar)
    for jar in &["custom.jar", "minecraft.jar"] {
        let src = source_dir.join(jar);
        let dst = target_dir.join(jar);
        if src.is_file() {
            let _ = fs::copy(&src, &dst);
        }
    }

    if let Some(ref cb) = on_progress {
        cb(MigrationProgressEvent {
            current_instance_name: discovered.name.clone(),
            current_file: "Completed".to_string(),
            completed_instances: 1,
            total_instances: 1,
            fraction: 1.0,
        });
    }

    Ok(new_inst)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_import_single_instance_preserves_files_and_configs() {
        let temp = tempdir().unwrap();
        let source_root = temp.path().join("source_prism_inst");
        let source_game = source_root.join(".minecraft");
        fs::create_dir_all(source_game.join("mods")).unwrap();
        fs::create_dir_all(source_game.join("config")).unwrap();
        fs::create_dir_all(source_game.join("saves").join("TestWorld")).unwrap();

        fs::write(source_game.join("mods/sample.jar"), b"jar_content").unwrap();
        fs::write(source_game.join("config/sample.cfg"), b"setting=true").unwrap();
        fs::write(source_game.join("options.txt"), b"fov:90").unwrap();
        fs::write(source_game.join("servers.dat"), b"servers").unwrap();

        let discovered = DiscoveredInstance {
            id: "disc-1".to_string(),
            source_launcher: crate::migration::models::SourceLauncherType::Prism,
            name: "Imported Pack".to_string(),
            minecraft_version: "1.20.1".to_string(),
            loader_type: "fabric".to_string(),
            loader_version: "0.15.11".to_string(),
            source_path: source_root,
            game_dir: source_game,
            mod_count: 1,
            world_count: 1,
            last_played_ms: None,
            icon_base64: None,
        };

        let manager_root = temp.path().join("zircon_instances");
        let manager = OfflineInstanceManager::new(manager_root);

        let imported = import_single_instance(&discovered, MigrationStrategy::HardlinkWithFallback, &manager, None).unwrap();
        assert_eq!(imported.name, "Imported Pack");
        assert_eq!(imported.minecraft_version, "1.20.1");
        assert_eq!(imported.mod_loader.r#type, "fabric");

        let target_dir = manager.instance_dir(&imported.id);
        assert!(target_dir.join("mods/sample.jar").is_file());
        assert!(target_dir.join("config/sample.cfg").is_file());
        assert!(target_dir.join("saves/TestWorld").is_dir());
        assert!(target_dir.join("options.txt").is_file());
        assert!(target_dir.join("servers.dat").is_file());
    }
}
