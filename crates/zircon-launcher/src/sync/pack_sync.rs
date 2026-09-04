//! Client pack synchronization engine: downloads every shaderpack/resourcepack
//! advertised in the server's Bill of Materials into `gameDir/shaderpacks` and
//! `gameDir/resourcepacks`, mirroring `ModSyncEngine`'s fetch-and-reconcile
//! shape but deliberately simpler:
//!
//! * No strict/trust-direct abort gating — packs are inert data files, not
//!   executable code, so a verification failure is only logged.
//! * No staging directory — presence in `shaderpacks`/`resourcepacks` never
//!   activates anything in Minecraft, unlike `mods/`.
//! * Reconciliation never deletes a file the caller marks as "keep" (a player's
//!   locally added pack), even if the server no longer lists it.
//!
//! Activation is never touched here — that's a purely local, per-player choice
//! applied at launch time.
//!
//! Port of `com.mcmanager.client.sync.PackSyncEngine`.

use std::collections::HashSet;
use std::path::Path;

use futures_util::StreamExt;
use tokio::io::AsyncWriteExt;
use tracing::{info, warn};

use zircon_core::model::{BillOfMaterials, PackEntry};

use super::mod_sync::{validate_entry_filename, HashVerifier};

/// Receives human-readable pack sync progress. Port of the Java
/// `PackSyncEngine.ProgressListener` — status only, the Java reports no
/// byte-count progress for packs. `Send + Sync` so spawned commands can hold
/// the listener across `.await` points.
pub trait PackProgressListener: Send + Sync {
    fn on_status(&self, message: &str);
}

fn emit_status(listener: Option<&dyn PackProgressListener>, message: &str) {
    if let Some(listener) = listener {
        listener.on_status(message);
    }
}

/// Result of a pack sync run. Port of the Java `PackSyncEngine.SyncResult`.
#[derive(Debug, Default)]
pub struct PackSyncResult {
    pub downloaded_shaderpacks: Vec<String>,
    pub downloaded_resourcepacks: Vec<String>,
    pub removed_shaderpacks: Vec<String>,
    pub removed_resourcepacks: Vec<String>,
}

/// Downloads every shaderpack/resourcepack advertised in the server's BOM into
/// `gameDir/shaderpacks` and `gameDir/resourcepacks`.
///
/// Port of `com.mcmanager.client.sync.PackSyncEngine`.
#[derive(Debug)]
pub struct PackSyncEngine {
    http: reqwest::Client,
}

impl Default for PackSyncEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl PackSyncEngine {
    pub fn new() -> Self {
        let http = reqwest::Client::builder()
            .connect_timeout(std::time::Duration::from_secs(15))
            .redirect(reqwest::redirect::Policy::limited(10))
            .build()
            .expect("failed to build reqwest client");
        Self { http }
    }

    /// Syncs both pack buckets against an already-fetched BOM (see the Java
    /// `MainController.fetchBom` flow — packs are reconciled from the BOM the
    /// caller already holds, unlike mods which are fetched here).
    ///
    /// `keep_shaderpacks` / `keep_resourcepacks` are local filenames that are
    /// never pruned even when absent from the BOM (a player's locally added
    /// packs). Per-pack failures are logged and skipped — the Java never
    /// aborts pack sync — so this returns a [`PackSyncResult`] rather than a
    /// `Result`, mirroring the Java `sync`.
    pub async fn sync(
        &self,
        bom: &BillOfMaterials,
        server_base_url: &str,
        game_dir: &Path,
        keep_shaderpacks: &[String],
        keep_resourcepacks: &[String],
        listener: Option<&dyn PackProgressListener>,
    ) -> PackSyncResult {
        let base = server_base_url
            .strip_suffix('/')
            .unwrap_or(server_base_url)
            .to_string();

        let mut result = PackSyncResult::default();
        self.sync_bucket(
            &base,
            &game_dir.join("shaderpacks"),
            &bom.shaderpacks,
            "/files/shaderpacks/",
            keep_shaderpacks,
            &mut result.downloaded_shaderpacks,
            &mut result.removed_shaderpacks,
            listener,
        )
        .await;
        self.sync_bucket(
            &base,
            &game_dir.join("resourcepacks"),
            &bom.resourcepacks,
            "/files/resourcepacks/",
            keep_resourcepacks,
            &mut result.downloaded_resourcepacks,
            &mut result.removed_resourcepacks,
            listener,
        )
        .await;
        result
    }

    /// Reconciles one pack bucket: download every BOM pack whose local copy is
    /// missing or mismatched, then prune files that are neither wanted by the
    /// BOM nor marked as "keep" (a player's locally added pack).
    ///
    /// Port of the Java `syncBucket`.
    #[allow(clippy::too_many_arguments)] // mirrors the Java syncBucket parameters
    async fn sync_bucket(
        &self,
        base: &str,
        dir: &Path,
        packs: &[PackEntry],
        url_prefix: &str,
        keep: &[String],
        downloaded: &mut Vec<String>,
        removed: &mut Vec<String>,
        listener: Option<&dyn PackProgressListener>,
    ) {
        if let Err(e) = std::fs::create_dir_all(dir) {
            warn!("Could not create pack directory {}: {}", dir.display(), e);
            return;
        }

        let wanted: Vec<String> = packs.iter().map(|p| p.filename.clone()).collect();

        for pack in packs {
            // Filenames come from the untrusted server BOM; never allow one to
            // escape the pack directory.
            if let Err(e) = validate_entry_filename(&pack.filename) {
                warn!("Skipping pack with {e}");
                continue;
            }
            let target = dir.join(&pack.filename);
            let guard = zircon_core::archive::limits::ArchiveGuard::default();
            if HashVerifier::matches_pack(&target, pack) {
                // Verify local cached file passes zero-trust security audit
                let is_safe = match std::fs::File::open(&target) {
                    Ok(f) => zircon_core::security::pack_validator::validate_pack_archive(f, &guard).is_ok(),
                    Err(_) => false,
                };
                if is_safe {
                    continue;
                } else {
                    let _ = std::fs::remove_file(&target);
                    warn!(
                        "Existing pack '{}' failed security audit and was purged",
                        pack.filename
                    );
                }
            }
            emit_status(listener, &format!("Downloading {}...", pack.filename));
            let url = format!("{base}{url_prefix}{}", url_encode(&pack.filename));
            match self.download(&url, &target).await {
                Ok(_) => {
                    // The downloaded archive must match the hash pinned in the
                    // BOM; a server serving something else is discarded.
                    if HashVerifier::matches_pack(&target, pack) {
                        let is_safe = match std::fs::File::open(&target) {
                            Ok(f) => zircon_core::security::pack_validator::validate_pack_archive(f, &guard).is_ok(),
                            Err(_) => false,
                        };
                        if is_safe {
                            downloaded.push(pack.filename.clone());
                        } else {
                            let _ = std::fs::remove_file(&target);
                            warn!(
                                "Pack '{}' failed client-side security whitelist audit and was discarded",
                                pack.filename
                            );
                        }
                    } else {
                        let _ = std::fs::remove_file(&target);
                        warn!(
                            "Pack download failed hash check and was discarded: {}",
                            pack.filename
                        );
                    }
                }
                Err(e) => warn!("Pack sync failed for {}: {}", pack.filename, e),
            }
        }

        match reconcile_pack_dir(dir, &wanted, keep) {
            Ok(pruned) => removed.extend(pruned),
            Err(e) => warn!(
                "Could not reconcile pack directory {}: {}",
                dir.display(),
                e
            ),
        }
    }

    /// Streams a file download to `target`. A non-2xx response becomes
    /// [`LauncherError::Http`].
    async fn download(&self, url: &str, target: &Path) -> Result<u64, crate::error::LauncherError> {
        let response = self.http.get(url).send().await?;
        let status = response.status();
        if !status.is_success() {
            return Err(crate::error::LauncherError::Http {
                status: status.as_u16(),
                url: url.to_string(),
            });
        }
        let mut out = tokio::fs::File::create(target).await?;
        let mut written: u64 = 0;
        let mut stream = response.bytes_stream();
        while let Some(chunk) = stream.next().await {
            let chunk = chunk?;
            out.write_all(&chunk).await?;
            written += chunk.len() as u64;
        }
        Ok(written)
    }
}

/// Prunes `.zip` files in `dir` that are neither wanted by the BOM nor in the
/// caller's `keep` set, returning the pruned filenames.
///
/// This is the pack equivalent of `mod_sync::reconcile`, deliberately simpler:
/// packs have no staging directory, and non-BOM files marked as "keep" (a
/// player's locally added packs) are never deleted. Port of the prune loop of
/// the Java `syncBucket`.
pub(crate) fn reconcile_pack_dir(
    dir: &Path,
    wanted: &[String],
    keep: &[String],
) -> std::io::Result<Vec<String>> {
    let wanted_set: HashSet<&str> = wanted.iter().map(|s| s.as_str()).collect();
    let keep_set: HashSet<&str> = keep.iter().map(|s| s.as_str()).collect();

    let mut removed = Vec::new();
    if !dir.is_dir() {
        return Ok(removed);
    }
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        if !entry.file_type()?.is_file() {
            continue;
        }
        let name = entry.file_name().to_string_lossy().into_owned();
        if !HashVerifier::is_zip(&name) {
            continue;
        }
        if !wanted_set.contains(name.as_str()) && !keep_set.contains(name.as_str()) {
            std::fs::remove_file(entry.path())?;
            removed.push(name.clone());
            info!("Pruned pack no longer offered by server: {}", name);
        }
    }
    Ok(removed)
}

/// Percent-encodes a filename for a URL path segment (same encoding as the mod
/// sync engine's `url_encode`).
fn url_encode(value: &str) -> String {
    url::form_urlencoded::byte_serialize(value.as_bytes()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

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
            remove_dir_all(&self.0);
        }
    }

    fn remove_dir_all(dir: &Path) {
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    remove_dir_all(&path);
                } else {
                    let _ = std::fs::remove_file(&path);
                }
            }
        }
        let _ = std::fs::remove_dir(dir);
    }

    #[test]
    fn reconcile_pack_dir_preserves_local_and_bom_packs() {
        let dir = TempDir::new("pack-reconcile");
        std::fs::write(dir.path().join("local-only.zip"), b"player pack").unwrap();
        std::fs::write(dir.path().join("bom-pack.zip"), b"server pack").unwrap();
        std::fs::write(dir.path().join("stale.zip"), b"old").unwrap();
        std::fs::write(dir.path().join("notes.txt"), b"not a pack").unwrap();

        let wanted = vec!["bom-pack.zip".to_string()];
        let keep = vec!["local-only.zip".to_string()];
        let removed = reconcile_pack_dir(dir.path(), &wanted, &keep).unwrap();

        assert_eq!(vec!["stale.zip".to_string()], removed);
        // The player's locally added pack must survive reconciliation.
        assert!(dir.path().join("local-only.zip").is_file());
        // The BOM pack present on disk must be kept (downloads land directly in
        // the pack dir — Java has no staging area for packs).
        assert!(dir.path().join("bom-pack.zip").is_file());
        assert!(!dir.path().join("stale.zip").exists());
        // Non-zip files are never touched.
        assert!(dir.path().join("notes.txt").is_file());
    }

    #[test]
    fn reconcile_pack_dir_keep_list_overrides_bom() {
        // A locally-added pack survives even when absent from the BOM entirely.
        let dir = TempDir::new("pack-keep");
        std::fs::write(dir.path().join("player-shader.zip"), b"mine").unwrap();

        let removed =
            reconcile_pack_dir(dir.path(), &[], &["player-shader.zip".to_string()]).unwrap();
        assert!(removed.is_empty());
        assert!(dir.path().join("player-shader.zip").is_file());
    }

    #[test]
    fn pack_sync_result_defaults() {
        let result = PackSyncResult::default();
        assert!(result.downloaded_shaderpacks.is_empty());
        assert!(result.downloaded_resourcepacks.is_empty());
        assert!(result.removed_shaderpacks.is_empty());
        assert!(result.removed_resourcepacks.is_empty());
    }
}
