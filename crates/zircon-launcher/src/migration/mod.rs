//! Migration and discovery engine for external Minecraft launchers.

pub mod detectors;
pub mod importer;
pub mod models;

pub use detectors::*;
pub use importer::*;
pub use models::*;

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use crate::offline::{OfflineInstance, OfflineInstanceManager};

/// Scans all supported Minecraft launchers across default paths or custom-provided paths.
pub fn scan_all_launchers(
    custom_paths: Option<&HashMap<String, PathBuf>>,
) -> Vec<MigrationScanResult> {
    let get_custom = |key: &str| -> Option<&Path> {
        custom_paths.and_then(|map| map.get(key).map(|p| p.as_path()))
    };

    let mut results = Vec::new();

    // 1. Prism Launcher / PolyMC / MultiMC
    results.push(scan_prism_instances(get_custom("prism")));

    // 2. Modrinth App (Theseus)
    results.push(scan_modrinth_instances(get_custom("modrinth")));

    // 3. CurseForge
    results.push(scan_curseforge_instances(get_custom("curseforge")));

    // 4. Official Minecraft Launcher (Vanilla)
    results.push(scan_vanilla_instances(get_custom("vanilla")));

    // 5. ATLauncher
    results.push(scan_atlauncher_instances(get_custom("atlauncher")));

    results
}

/// Imports multiple instances, reporting aggregate progress.
pub fn import_multiple_instances<F>(
    instances: &[DiscoveredInstance],
    strategy: MigrationStrategy,
    manager: &OfflineInstanceManager,
    on_progress: F,
) -> Result<Vec<OfflineInstance>, String>
where
    F: Fn(MigrationProgressEvent),
{
    let total = instances.len();
    if total == 0 {
        return Ok(Vec::new());
    }

    let mut imported = Vec::with_capacity(total);

    for (idx, inst) in instances.iter().enumerate() {
        let base_fraction = idx as f32 / total as f32;
        let per_instance_fraction = 1.0 / total as f32;

        let inst_name = inst.name.clone();
        let sub_progress = |sub_event: MigrationProgressEvent| {
            let overall_fraction = base_fraction + (sub_event.fraction * per_instance_fraction);
            on_progress(MigrationProgressEvent {
                current_instance_name: inst_name.clone(),
                current_file: sub_event.current_file,
                completed_instances: idx + if sub_event.fraction >= 0.99 { 1 } else { 0 },
                total_instances: total,
                fraction: overall_fraction.min(1.0),
            });
        };

        match import_single_instance(inst, strategy.clone(), manager, Some(&sub_progress)) {
            Ok(new_inst) => {
                imported.push(new_inst);
            }
            Err(e) => {
                tracing::warn!("Failed to import instance '{}': {}", inst.name, e);
            }
        }
    }

    Ok(imported)
}
