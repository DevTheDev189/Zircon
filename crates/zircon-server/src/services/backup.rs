//! LZ4-compressed backup engine for isolated server instances.
//!
//! Backups are stored outside the instance directory, under
//! `<data>/backups/<instanceId>/`, so archiving an instance folder can never
//! nest a previous archive inside a new one. Each backup is a `.tar.lz4` file
//! plus a sidecar `.json` metadata record that doubles as an audit trail.
//!
//! Backing up a live server first announces the operation in-game, waits 10
//! seconds, gracefully stops the instance, archives the now-offline directory
//! (a cold backup that avoids OS file-sharing violations on locked files), and
//! restarts the instance if it was running beforehand. This is also what avoids
//! the ghost JSON entries left behind when a live archive previously failed.
//! Restoring stops the instance, moves the current state aside to a temporary
//! rollback folder, extracts the archive, and only discards the rollback once
//! extraction succeeded.
//!
//! Port of `com.mcmanager.server.service.BackupService`.

use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use zircon_core::archive::lz4_tar;
use zircon_core::model::BackupEntry;

use crate::instance::{delete_recursively, ServerInstanceManager};

/// How long to announce the upcoming cold backup in-game before stopping the
/// server. Players see this countdown so they can finish what they are doing.
const BACKUP_NOTICE_WAIT_SECS: u64 = 10;

/// Monotonic sequence so two backups started within the same millisecond stay
/// distinct.
static BACKUP_SEQ: AtomicI64 = AtomicI64::new(0);

/// Errors raised by the backup service.
#[derive(Debug)]
pub enum BackupError {
    NotFound(String),
    Invalid(String),
    Io(std::io::Error),
}

impl fmt::Display for BackupError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BackupError::NotFound(m) => write!(f, "{m}"),
            BackupError::Invalid(m) => write!(f, "{m}"),
            BackupError::Io(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for BackupError {}

impl From<std::io::Error> for BackupError {
    fn from(e: std::io::Error) -> Self {
        BackupError::Io(e)
    }
}

impl From<crate::instance::InstanceError> for BackupError {
    fn from(e: crate::instance::InstanceError) -> Self {
        match e {
            crate::instance::InstanceError::NotFound(m) => BackupError::NotFound(m),
            crate::instance::InstanceError::Conflict(m)
            | crate::instance::InstanceError::Invalid(m) => BackupError::Invalid(m),
            crate::instance::InstanceError::Io(e) => BackupError::Io(e),
        }
    }
}

/// LZ4-compressed backup engine for isolated server instances.
pub struct BackupService {
    global_backups_dir: PathBuf,
    instance_manager: Arc<ServerInstanceManager>,
}

impl BackupService {
    pub fn new(data_dir: &Path, instance_manager: Arc<ServerInstanceManager>) -> Self {
        let global_backups_dir = data_dir.join("backups");
        if let Err(e) = fs::create_dir_all(&global_backups_dir) {
            tracing::error!("Failed to create backups directory: {e}");
        }
        Self {
            global_backups_dir,
            instance_manager,
        }
    }

    /// Creates a backup of an instance, using the safe stop-backup-restart
    /// workflow: if the instance is live it first announces the operation
    /// in-game, waits a short countdown, gracefully stops the server, archives
    /// the now-offline folder as an LZ4-compressed TAR, restarts the server if
    /// it was running beforehand, and prunes old backups beyond the retention
    /// limit.
    ///
    /// Because archiving only ever runs while the server is stopped, locked
    /// files (on Windows) can no longer cause failed archives or ghost JSON
    /// metadata entries. If the archive step fails, the instance is still
    /// restarted and no partial archive is left behind.
    pub async fn create_backup(
        &self,
        instance_id: &str,
        trigger_type: &str,
    ) -> Result<BackupEntry, BackupError> {
        let config = self.instance_manager.get_instance(instance_id)?;
        let instance_dir = self.instance_manager.get_instance_dir(instance_id);
        let instance_backups_dir = self.global_backups_dir.join(instance_id);
        fs::create_dir_all(&instance_backups_dir)?;

        let backup_id = new_backup_id();
        let filename = format!("{backup_id}.tar.lz4");
        let target_archive = instance_backups_dir.join(&filename);
        let metadata_file = instance_backups_dir.join(format!("{backup_id}.json"));

        let mut audit_logs: Vec<String> = Vec::new();
        audit_logs.push(format!(
            "Starting backup for instance: {} ({instance_id})",
            config.name
        ));
        audit_logs.push(format!("Trigger type: {trigger_type}"));

        // Only stop the server if it is currently running; otherwise proceed
        // straight to the cold archive.
        let was_running = self.instance_manager.is_running(instance_id);
        if was_running {
            audit_logs.push("Server is running. Announcing backup and waiting 10s...".to_string());
            if let Some(pm) = self.instance_manager.get_process_manager(instance_id) {
                let _ = pm
                    .send_command("say [Server is backing up, please check back in about 1 minute]")
                    .await;
            }

            tokio::time::sleep(std::time::Duration::from_secs(BACKUP_NOTICE_WAIT_SECS)).await;

            audit_logs.push("Stopping server for cold backup...".to_string());
            self.instance_manager.stop_instance(instance_id).await;
        } else {
            audit_logs.push("Server is offline. Proceeding directly with archive.".to_string());
        }

        let mut entry = BackupEntry::new(
            backup_id.clone(),
            instance_id,
            filename.clone(),
            now_millis(),
            trigger_type,
            zircon_core::model::backup::STATUS_IN_PROGRESS,
        );

        // 2. Compress the (now offline) directory.
        {
            let audit_logs_shared: Arc<std::sync::Mutex<Vec<String>>> =
                Arc::new(std::sync::Mutex::new(audit_logs));
            let closure_logs = audit_logs_shared.clone();
            let closure_instance = instance_dir.clone();
            let closure_target = target_archive.clone();
            let compress_result = tokio::task::spawn_blocking(move || {
                let mut logs = closure_logs.lock().unwrap();
                lz4_tar::compress_directory(&closure_instance, &closure_target, None, &mut logs)
            })
            .await;
            audit_logs = audit_logs_shared.lock().unwrap().clone();

            match compress_result {
                Ok(Ok(_stats)) => {
                    entry.status = zircon_core::model::backup::STATUS_COMPLETED.to_string();
                    entry.size_bytes = fs::metadata(&target_archive)?.len();
                    audit_logs.push(format!("Backup file written successfully: {filename}"));
                }
                Ok(Err(e)) => {
                    entry.status = zircon_core::model::backup::STATUS_FAILED.to_string();
                    audit_logs.push(format!("ERROR during compression: {e}"));
                    tracing::error!("Backup failed for instance {instance_id}: {e}");
                    // Drop the partial archive so a corrupt file can never be restored.
                    if let Err(delete_error) = fs::remove_file(&target_archive) {
                        tracing::warn!(
                            "Could not delete partial backup archive {}: {delete_error}",
                            target_archive.display()
                        );
                    }
                }
                Err(e) => {
                    entry.status = zircon_core::model::backup::STATUS_FAILED.to_string();
                    audit_logs.push(format!("ERROR during compression: {e}"));
                }
            }
        }

        // 3. Always restart the server if it was running prior to the backup, so
        // the workflow restores the instance even when the archive failed.
        if was_running {
            audit_logs.push("Restarting server...".to_string());
            match self.instance_manager.start_instance(instance_id).await {
                Ok(()) => audit_logs.push("Server restarted successfully.".to_string()),
                Err(e) => {
                    audit_logs.push(format!("WARNING: Failed to auto-restart instance: {e}"));
                    tracing::error!("Failed to restart instance {instance_id} after backup: {e}");
                }
            }
        }

        // 4. Finished archiving: write the metadata regardless.
        entry.logs = audit_logs;
        let json = serde_json::to_string_pretty(&entry).map_err(|e| {
            BackupError::Invalid(format!("Could not serialize backup metadata: {e}"))
        })?;
        fs::write(&metadata_file, json)?;

        // Do not leave broken ghost entries on disk if compression completely
        // failed — surface the error to the caller instead.
        if entry.status == zircon_core::model::backup::STATUS_FAILED {
            let _ = fs::remove_file(&metadata_file);
            return Err(BackupError::Io(std::io::Error::other(format!(
                "Backup failed: {}",
                entry.logs.last().cloned().unwrap_or_default()
            ))));
        }

        // 5. Enforce the retention policy configured for this instance.
        self.prune_old_backups(instance_id, config.backup_retention)?;

        Ok(entry)
    }

    /// Persists a new retention limit for an instance and immediately prunes
    /// any backups beyond it.
    ///
    /// Returns how many backups were deleted by the new limit.
    pub fn set_retention(&self, instance_id: &str, retention: i32) -> Result<i32, BackupError> {
        if !(zircon_core::model::instance::MIN_BACKUP_RETENTION
            ..=zircon_core::model::instance::MAX_BACKUP_RETENTION)
            .contains(&retention)
        {
            return Err(BackupError::Invalid(format!(
                "retention must be between {} and {}",
                zircon_core::model::instance::MIN_BACKUP_RETENTION,
                zircon_core::model::instance::MAX_BACKUP_RETENTION
            )));
        }
        self.instance_manager
            .update_backup_retention(instance_id, retention)?;

        let backups = self.list_backups(instance_id);
        let to_delete = backups.len().saturating_sub(retention as usize);
        if to_delete > 0 {
            self.prune_old_backups(instance_id, retention)?;
            tracing::info!("Retention for instance {instance_id} set to {retention} — pruned {to_delete} old backup(s)");
        }
        Ok(to_delete as i32)
    }

    /// Restores a backup into an instance directory. The server is stopped
    /// first; the pre-restore state is moved to a temporary rollback folder and
    /// is either discarded on success or moved back if extraction fails.
    /// Afterwards the instance config is re-read from disk.
    pub async fn restore_backup(
        &self,
        instance_id: &str,
        backup_id: &str,
    ) -> Result<(), BackupError> {
        self.instance_manager.get_instance(instance_id)?;
        let instance_backups_dir = self.global_backups_dir.join(instance_id);
        let archive_file = instance_backups_dir.join(format!("{backup_id}.tar.lz4"));
        if !archive_file.is_file() {
            return Err(BackupError::NotFound(format!(
                "Backup archive not found: {backup_id}"
            )));
        }

        // 1. Safely stop the server and wait for it to exit before touching files.
        if self.instance_manager.is_running(instance_id) {
            tracing::info!("Stopping instance {instance_id} before restore");
            self.instance_manager.stop_instance(instance_id).await;
        }

        let instance_dir = self.instance_manager.get_instance_dir(instance_id);

        // 2. Move the current (possibly broken) state aside.
        let rollback_dir = instance_backups_dir.join("rollback").join(backup_id);
        if instance_dir.exists() {
            delete_recursively(&rollback_dir)?;
            if let Some(parent) = rollback_dir.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::rename(&instance_dir, &rollback_dir)?;
            tracing::info!(
                "Moved pre-restore instance state to rollback folder: {}",
                rollback_dir.display()
            );
        }

        // 3. Extract the archive into a fresh instance directory.
        fs::create_dir_all(&instance_dir)?;
        let extract_result = tokio::task::spawn_blocking({
            let archive_file = archive_file.clone();
            let instance_dir = instance_dir.clone();
            move || lz4_tar::extract_archive(&archive_file, &instance_dir)
        })
        .await;

        if let Err(e) = extract_result {
            // Undo: discard the partial extraction and put the old state back.
            delete_recursively(&instance_dir)?;
            if rollback_dir.exists() {
                fs::rename(&rollback_dir, &instance_dir)?;
            }
            return Err(BackupError::Io(std::io::Error::other(format!(
                "Restore failed; instance state has been rolled back: {e}"
            ))));
        }

        // Success: the rollback snapshot was only temporary.
        delete_recursively(&rollback_dir)?;

        // 4. Re-read the restored instance config so in-memory state matches disk.
        if let Err(e) = self.instance_manager.reload_instance_from_disk(instance_id) {
            tracing::warn!("Could not reload instance config after restore: {e}");
        }
        tracing::info!("Restored backup {backup_id} into instance {instance_id}");
        Ok(())
    }

    /// All backup metadata records for an instance, newest first.
    pub fn list_backups(&self, instance_id: &str) -> Vec<BackupEntry> {
        let instance_backups_dir = self.global_backups_dir.join(instance_id);
        if !instance_backups_dir.is_dir() {
            return Vec::new();
        }
        let mut list: Vec<BackupEntry> = Vec::new();
        if let Ok(entries) = fs::read_dir(&instance_backups_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().map(|e| e == "json").unwrap_or(false) {
                    match fs::read_to_string(&path)
                        .map_err(|e| std::io::Error::other(e.to_string()))
                        .and_then(|content| {
                            serde_json::from_str::<BackupEntry>(&content)
                                .map_err(|e| std::io::Error::other(e.to_string()))
                        }) {
                        Ok(entry) if !entry.id.is_empty() => list.push(entry),
                        Ok(_) => {}
                        Err(e) => {
                            tracing::warn!("Could not read backup metadata {}: {e}", path.display())
                        }
                    }
                }
            }
        }
        list.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
        list
    }

    fn prune_old_backups(&self, instance_id: &str, max_keep: i32) -> std::io::Result<()> {
        let backups = self.list_backups(instance_id);
        if backups.len() <= max_keep as usize {
            return Ok(());
        }
        let instance_backups_dir = self.global_backups_dir.join(instance_id);
        for old in backups.iter().skip(max_keep as usize) {
            let archive = instance_backups_dir.join(&old.filename);
            let metadata = instance_backups_dir.join(format!("{}.json", old.id));
            if archive.is_file() {
                let _ = fs::remove_file(&archive);
            }
            if metadata.is_file() {
                let _ = fs::remove_file(&metadata);
            }
            tracing::info!("Pruned old backup: {} ({})", old.id, old.trigger_type);
        }
        Ok(())
    }
}

fn new_backup_id() -> String {
    let seq = BACKUP_SEQ.fetch_add(1, Ordering::SeqCst);
    format!("backup-{}-{seq}", now_millis())
}

fn now_millis() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::process::console::ConsoleStreamHandler;

    fn temp_dir() -> PathBuf {
        crate::test_util::temp_dir("backup")
    }

    #[tokio::test]
    async fn backup_and_restore_round_trip() {
        let dir = temp_dir();
        let console = Arc::new(ConsoleStreamHandler::new());
        let manager = Arc::new(ServerInstanceManager::new(&dir, console).unwrap());
        let instance = manager
            .create_instance("World", "1.20.4", "vanilla", "")
            .unwrap();
        let instance_dir = manager.get_instance_dir(&instance.id);
        fs::create_dir_all(instance_dir.join("world")).unwrap();
        fs::write(instance_dir.join("world").join("level.dat"), "world data").unwrap();
        fs::write(
            instance_dir.join("instance.json"),
            serde_json::to_string(&instance).unwrap(),
        )
        .unwrap();

        let backup_service = BackupService::new(&dir, manager.clone());
        let entry = backup_service
            .create_backup(&instance.id, "manual")
            .await
            .unwrap();
        assert_eq!("completed", entry.status);
        assert!(entry.size_bytes > 0);
        assert!(!entry.logs.is_empty());

        // Mutate the world, then restore.
        fs::write(instance_dir.join("world").join("level.dat"), "corrupted").unwrap();
        backup_service
            .restore_backup(&instance.id, &entry.id)
            .await
            .unwrap();
        assert_eq!(
            "world data",
            fs::read_to_string(instance_dir.join("world").join("level.dat")).unwrap()
        );

        let backups = backup_service.list_backups(&instance.id);
        assert_eq!(1, backups.len());
        let _ = fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn retention_prunes_oldest_backups() {
        let dir = temp_dir();
        let console = Arc::new(ConsoleStreamHandler::new());
        let manager = Arc::new(ServerInstanceManager::new(&dir, console).unwrap());
        let instance = manager
            .create_instance("Retention", "1.20.4", "vanilla", "")
            .unwrap();
        let instance_dir = manager.get_instance_dir(&instance.id);
        fs::write(
            instance_dir.join("instance.json"),
            serde_json::to_string(&instance).unwrap(),
        )
        .unwrap();
        fs::write(instance_dir.join("a.txt"), "a").unwrap();

        let backup_service = BackupService::new(&dir, manager.clone());
        let mut ids = Vec::new();
        for _ in 0..3 {
            let entry = backup_service
                .create_backup(&instance.id, "manual")
                .await
                .unwrap();
            ids.push(entry.id);
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        assert_eq!(3, backup_service.list_backups(&instance.id).len());

        let deleted = backup_service.set_retention(&instance.id, 1).unwrap();
        assert_eq!(2, deleted);
        let remaining = backup_service.list_backups(&instance.id);
        assert_eq!(1, remaining.len());
        assert_eq!(ids[2], remaining[0].id); // newest survives
        let _ = fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn restore_of_missing_backup_fails() {
        let dir = temp_dir();
        let console = Arc::new(ConsoleStreamHandler::new());
        let manager = Arc::new(ServerInstanceManager::new(&dir, console).unwrap());
        let instance = manager
            .create_instance("X", "1.20.4", "vanilla", "")
            .unwrap();
        let backup_service = BackupService::new(&dir, manager.clone());
        let err = backup_service
            .restore_backup(&instance.id, "backup-nope")
            .await
            .unwrap_err();
        assert!(err.to_string().contains("Backup archive not found"));
        let _ = fs::remove_dir_all(&dir);
    }
}
