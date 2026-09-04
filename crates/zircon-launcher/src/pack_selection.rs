//! A player's local, per-instance choice of shaderpacks/resourcepacks and the
//! local file manager for packs added directly (drag-and-drop / file picker).
//!
//! Port of `com.mcmanager.client.pack.PackSelection` and
//! `com.mcmanager.client.pack.ClientPackManager`.
//!
//! `PackSelection` is persisted at `<gameDir>/pack-selection.json` following
//! the same silent-catch Gson load/save pattern as `SavedServer`, scoped to a
//! single instance's game directory. `ClientPackManager` copies a picked file
//! into the instance's `shaderpacks/`/`resourcepacks/` folder and records the
//! filename in the selection's locally-added sets so `PackSyncEngine` never
//! prunes a pack the server doesn't list.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use tracing::warn;

use crate::error::LauncherError;
use crate::paths::sanitize_filename_strict;

const FILE_NAME: &str = "pack-selection.json";

/// The player's local pack selection for one instance.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PackSelection {
    #[serde(default)]
    pub shaders_enabled: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub active_shaderpack: Option<String>,
    #[serde(default)]
    pub active_resourcepacks: Vec<String>,
    #[serde(default)]
    pub locally_added_shaderpacks: BTreeSet<String>,
    #[serde(default)]
    pub locally_added_resourcepacks: BTreeSet<String>,
    #[serde(default)]
    pub locally_added_mods: BTreeSet<String>,
    /// The player answered the per-server shader prompt once; don't ask again.
    #[serde(default)]
    pub remember_shaders_choice: bool,
    /// The remembered shader answer (`true` = enable shaders on connect).
    #[serde(default)]
    pub shaders_auto_enabled: bool,
}

impl PackSelection {
    pub fn file_for(game_dir: &Path) -> PathBuf {
        game_dir.join(FILE_NAME)
    }

    /// Loads the selection for a game directory; missing/corrupt files yield a
    /// fresh selection (Java `PackSelection.load` silent catch). Java Gson
    /// null-fields are normalised to empty collections on load.
    pub fn load(game_dir: &Path) -> Self {
        let file = Self::file_for(game_dir);
        if !file.is_file() {
            return Self::default();
        }
        match std::fs::read_to_string(&file) {
            Ok(text) => match serde_json::from_str::<PackSelection>(&text) {
                Ok(mut loaded) => {
                    loaded.active_resourcepacks = loaded
                        .active_resourcepacks
                        .into_iter()
                        .filter(|n| !n.trim().is_empty())
                        .collect();
                    loaded
                }
                Err(e) => {
                    warn!("Could not parse {}: {e}", file.display());
                    Self::default()
                }
            },
            Err(e) => {
                warn!("Could not read {}: {e}", file.display());
                Self::default()
            }
        }
    }

    /// Persists the selection to `<gameDir>/pack-selection.json`. Best-effort,
    /// like the Java `save`.
    pub fn save(&self, game_dir: &Path) {
        if let Err(e) = std::fs::create_dir_all(game_dir) {
            warn!("Could not create {}: {e}", game_dir.display());
            return;
        }
        match serde_json::to_string_pretty(self) {
            Ok(json) => {
                if let Err(e) = std::fs::write(Self::file_for(game_dir), json) {
                    warn!("Could not write pack selection: {e}");
                }
            }
            Err(e) => warn!("Could not serialize pack selection: {e}"),
        }
    }

    /// True when the given shaderpack filename is locally added (never pruned).
    pub fn is_locally_added_shaderpack(&self, filename: &str) -> bool {
        self.locally_added_shaderpacks.contains(filename)
    }

    /// True when the given resourcepack filename is locally added (never pruned).
    pub fn is_locally_added_resourcepack(&self, filename: &str) -> bool {
        self.locally_added_resourcepacks.contains(filename)
    }

    /// True when the given mod filename (or base name) is locally added (never pruned).
    pub fn is_locally_added_mod(&self, filename: &str) -> bool {
        let base = filename.strip_suffix(".disabled").unwrap_or(filename);
        self.locally_added_mods.contains(filename) || self.locally_added_mods.contains(base)
    }

    pub fn add_locally_added_mod(&mut self, filename: impl Into<String>) {
        let name = filename.into();
        let base = name.strip_suffix(".disabled").unwrap_or(&name).to_string();
        self.locally_added_mods.insert(base);
    }

    pub fn remove_locally_added_mod(&mut self, filename: &str) {
        let base = filename.strip_suffix(".disabled").unwrap_or(filename);
        self.locally_added_mods.remove(filename);
        self.locally_added_mods.remove(base);
    }
}

/// Local file management for shaderpacks/resourcepacks a player adds directly,
/// independent of anything the server offers.
pub struct ClientPackManager;

impl ClientPackManager {
    /// Copies `source` into `gameDir/mods`, records it as locally added
    /// and persists the selection. Returns the sanitized filename.
    pub fn add_local_mod(
        game_dir: &Path,
        source: &Path,
        selection: &mut PackSelection,
    ) -> Result<String, LauncherError> {
        let filename = copy_in(game_dir.join("mods"), source)?;
        selection.add_locally_added_mod(&filename);
        selection.save(game_dir);
        Ok(filename)
    }

    /// Copies `source` into `gameDir/shaderpacks`, records it as locally added
    /// and persists the selection. Returns the sanitized filename.
    pub fn add_local_shaderpack(
        game_dir: &Path,
        source: &Path,
        selection: &mut PackSelection,
    ) -> Result<String, LauncherError> {
        let filename = copy_in(game_dir.join("shaderpacks"), source)?;
        selection.locally_added_shaderpacks.insert(filename.clone());
        selection.save(game_dir);
        Ok(filename)
    }

    /// Copies `source` into `gameDir/resourcepacks`, records it as locally
    /// added and persists the selection. Returns the sanitized filename.
    pub fn add_local_resourcepack(
        game_dir: &Path,
        source: &Path,
        selection: &mut PackSelection,
    ) -> Result<String, LauncherError> {
        let filename = copy_in(game_dir.join("resourcepacks"), source)?;
        selection
            .locally_added_resourcepacks
            .insert(filename.clone());
        selection.save(game_dir);
        Ok(filename)
    }

    /// Deletes a shaderpack from disk and its selection entries; clearing the
    /// active selection when it was the active pack. The filename is strictly
    /// validated so a caller cannot escape `gameDir/shaderpacks`.
    pub fn remove_shaderpack(
        game_dir: &Path,
        filename: &str,
        selection: &mut PackSelection,
    ) -> Result<(), LauncherError> {
        let name = sanitize_filename_strict(filename)?;
        delete_if_exists(&game_dir.join("shaderpacks").join(&name));
        selection.locally_added_shaderpacks.remove(&name);
        if selection.active_shaderpack.as_deref() == Some(name.as_str()) {
            selection.active_shaderpack = None;
        }
        selection.save(game_dir);
        Ok(())
    }

    /// Deletes a resourcepack from disk and all of its selection entries. The
    /// filename is strictly validated so a caller cannot escape
    /// `gameDir/resourcepacks`.
    pub fn remove_resourcepack(
        game_dir: &Path,
        filename: &str,
        selection: &mut PackSelection,
    ) -> Result<(), LauncherError> {
        let name = sanitize_filename_strict(filename)?;
        delete_if_exists(&game_dir.join("resourcepacks").join(&name));
        selection.locally_added_resourcepacks.remove(&name);
        selection.active_resourcepacks.retain(|n| n != &name);
        selection.save(game_dir);
        Ok(())
    }
}

fn copy_in(dir: PathBuf, source: &Path) -> Result<String, LauncherError> {
    let guard = zircon_core::archive::limits::ArchiveGuard::default();
    let file = std::fs::File::open(source)?;
    zircon_core::security::pack_validator::validate_pack_archive(file, &guard)
        .map_err(|e| LauncherError::Security(format!("Pack failed security audit: {e}")))?;

    std::fs::create_dir_all(&dir)?;
    let filename = sanitize_pack_filename(source);
    let target = dir.join(&filename);
    std::fs::copy(source, &target)?;
    Ok(filename)
}

fn delete_if_exists(path: &Path) {
    match std::fs::remove_file(path) {
        Ok(()) => {}
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => warn!("Could not delete {}: {e}", path.display()),
    }
}

/// Sanitizes a pack filename: strips directory components, replaces unsafe
/// characters, forces a `.zip` extension, and falls back to a random name.
/// Port of the Java `ClientPackManager.sanitize`.
fn sanitize_pack_filename(source: &Path) -> String {
    let raw = source
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let base = raw.replace('\\', "/");
    let base = base.rsplit('/').next().unwrap_or(&base).to_string();
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
    let sanitized = sanitized.trim().to_string();
    let mut name = if sanitized.is_empty() {
        format!("pack-{}", &uuid::Uuid::new_v4().simple().to_string()[..8])
    } else {
        sanitized
    };
    if !name.to_ascii_lowercase().ends_with(".zip") {
        name.push_str(".zip");
    }
    name
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::servers::instance_game_dir;

    struct TempDir(PathBuf);

    impl TempDir {
        fn new() -> Self {
            let dir = std::env::temp_dir()
                .join(format!("zircon-packs-{}", uuid::Uuid::new_v4().simple()));
            std::fs::create_dir_all(&dir).unwrap();
            Self(dir)
        }

        fn path(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn write_zip(path: &Path) {
        let file = std::fs::File::create(path).unwrap();
        let mut zip = zip::ZipWriter::new(file);
        let options: zip::write::FileOptions<'_, ()> = zip::write::FileOptions::default();
        zip.start_file("pack.mcmeta", options).unwrap();
        std::io::Write::write_all(&mut zip, b"{\"pack\":{\"pack_format\":15,\"description\":\"Test\"}}").unwrap();
        zip.finish().unwrap();
    }

    #[test]
    fn pack_selection_save_load_round_trip() {
        let dir = TempDir::new();
        let selection = PackSelection {
            shaders_enabled: true,
            active_shaderpack: Some("shader.zip".to_string()),
            active_resourcepacks: vec!["pack.zip".to_string()],
            ..Default::default()
        };
        selection.save(dir.path());

        let loaded = PackSelection::load(dir.path());
        assert!(loaded.shaders_enabled);
        assert_eq!(Some("shader.zip".to_string()), loaded.active_shaderpack);
        assert_eq!(vec!["pack.zip".to_string()], loaded.active_resourcepacks);
    }

    #[test]
    fn load_missing_or_corrupt_returns_default() {
        let dir = TempDir::new();
        let selection = PackSelection::load(dir.path());
        assert!(!selection.shaders_enabled);
        assert!(selection.active_shaderpack.is_none());

        std::fs::write(PackSelection::file_for(dir.path()), "{bad").unwrap();
        let selection = PackSelection::load(dir.path());
        assert!(selection.active_resourcepacks.is_empty());
    }

    #[test]
    fn add_local_pack_copies_and_records() {
        let dir = TempDir::new();
        let game = dir.path().join("game");
        let source = dir.path().join("My Shader Pack.zip");
        write_zip(&source);

        let mut selection = PackSelection::default();
        let filename =
            ClientPackManager::add_local_shaderpack(&game, &source, &mut selection).unwrap();
        assert_eq!("My_Shader_Pack.zip", filename);
        assert!(game.join("shaderpacks").join(&filename).is_file());
        assert!(selection.is_locally_added_shaderpack(&filename));

        // Reloading from disk preserves the locally-added set.
        let reloaded = PackSelection::load(&game);
        assert!(reloaded.is_locally_added_shaderpack(&filename));
    }

    #[test]
    fn sanitize_forces_zip_extension_and_removes_dirs() {
        let dir = TempDir::new();
        // `!` is a legal Windows filename char that must be sanitized; `:` is
        // not legal on Windows so it cannot be used in a real test file.
        let sub = dir.path().join("sub dir");
        std::fs::create_dir_all(&sub).unwrap();
        let source = sub.join("weird!name");
        write_zip(&source);

        let game = dir.path().join("game2");
        let mut selection = PackSelection::default();
        let filename =
            ClientPackManager::add_local_resourcepack(&game, &source, &mut selection).unwrap();
        assert_eq!("weird_name.zip", filename);
        assert!(game.join("resourcepacks").join(&filename).is_file());
        assert!(selection.is_locally_added_resourcepack(&filename));
    }

    #[test]
    fn remove_pack_clears_disk_and_selection() {
        let dir = TempDir::new();
        let game = dir.path().join("game3");
        std::fs::create_dir_all(game.join("shaderpacks")).unwrap();
        std::fs::write(game.join("shaderpacks").join("s.zip"), b"x").unwrap();

        let mut selection = PackSelection {
            active_shaderpack: Some("s.zip".to_string()),
            locally_added_shaderpacks: BTreeSet::from(["s.zip".to_string()]),
            ..Default::default()
        };
        ClientPackManager::remove_shaderpack(&game, "s.zip", &mut selection).unwrap();
        assert!(!game.join("shaderpacks").join("s.zip").exists());
        assert!(selection.active_shaderpack.is_none());
        assert!(!selection.is_locally_added_shaderpack("s.zip"));
    }

    #[test]
    fn remove_pack_rejects_traversal_filenames() {
        let dir = TempDir::new();
        let game = dir.path().join("game-traversal");
        std::fs::create_dir_all(game.join("shaderpacks")).unwrap();
        std::fs::create_dir_all(game.join("resourcepacks")).unwrap();

        // A sentry file outside the pack dirs that must survive.
        let sentry = dir.path().join("sentry.txt");
        std::fs::write(&sentry, b"keep").unwrap();

        let mut selection = PackSelection::default();
        for evil in [
            "../../sentry.txt",
            "..\\sentry.txt",
            "/abs.zip",
            ".hidden.zip",
        ] {
            let shader = ClientPackManager::remove_shaderpack(&game, evil, &mut selection);
            assert!(
                matches!(shader, Err(LauncherError::InvalidInput(_))),
                "shaderpack traversal {evil:?} must be rejected, got {shader:?}"
            );
            let resource = ClientPackManager::remove_resourcepack(&game, evil, &mut selection);
            assert!(
                matches!(resource, Err(LauncherError::InvalidInput(_))),
                "resourcepack traversal {evil:?} must be rejected, got {resource:?}"
            );
        }
        assert!(
            sentry.is_file(),
            "pack deletion must never escape the pack dirs"
        );
    }

    #[test]
    fn instance_game_dir_is_reusable_path_helper() {
        // sanity: the shared helper still produces the Java-style path
        let dir = instance_game_dir("localhost", 25565);
        assert!(dir.to_string_lossy().ends_with("localhost_25565"));
    }
}
