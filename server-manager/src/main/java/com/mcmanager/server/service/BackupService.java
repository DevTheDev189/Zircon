package com.mcmanager.server.service;

import com.google.gson.Gson;
import com.google.gson.GsonBuilder;
import com.mcmanager.core.model.BackupEntry;
import com.mcmanager.core.model.InstanceConfig;
import com.mcmanager.core.util.Lz4ArchiveUtil;
import com.mcmanager.server.instance.ServerInstanceManager;
import com.mcmanager.server.process.MinecraftProcessManager;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

import java.io.FileNotFoundException;
import java.io.IOException;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.Comparator;
import java.util.List;
import java.util.concurrent.atomic.AtomicLong;

/**
 * LZ4-compressed backup engine for isolated server instances.
 *
 * <p>Backups are stored outside the instance directory, under
 * {@code <data>/backups/<instanceId>/}, so archiving an instance folder can
 * never nest a previous archive inside a new one. Each backup is a
 * {@code .tar.lz4} file plus a sidecar {@code .json} metadata record that
 * doubles as an audit trail (flush commands issued, file counts, errors).
 *
 * <p>Backing up a live server first sends {@code save-off} + {@code save-all}
 * so the archive captures a consistent world state, then {@code save-on} in a
 * {@code finally} block so autosaving always resumes. Restoring stops the
 * instance, moves the current state aside to a temporary rollback folder,
 * extracts the archive, and only discards the rollback once extraction
 * succeeded.
 */
public class BackupService {

    private static final Logger log = LoggerFactory.getLogger(BackupService.class);
    private static final Gson GSON = new GsonBuilder().setPrettyPrinting().create();

    /** How long to wait after {@code save-all} for the chunk flush to hit disk. */
    private static final long SAVE_FLUSH_WAIT_MS = 2500;

    /** Monotonic sequence so two backups started within the same millisecond stay distinct. */
    private static final AtomicLong BACKUP_SEQ = new AtomicLong();

    private final Path globalBackupsDir;
    private final ServerInstanceManager instanceManager;

    public BackupService(Path dataDir, ServerInstanceManager instanceManager) {
        this.globalBackupsDir = dataDir.resolve("backups");
        this.instanceManager = instanceManager;
        try {
            Files.createDirectories(globalBackupsDir);
        } catch (IOException e) {
            log.error("Failed to create backups directory", e);
        }
    }

    /**
     * Creates a backup of an instance: flushes/pauses autosave when the server
     * is live, streams the instance folder into an LZ4-compressed TAR archive,
     * persists a JSON metadata/audit record, and prunes old backups beyond the
     * retention limit.
     *
     * @return the backup entry (status {@code completed} or {@code failed});
     *         the metadata file is written either way so failures stay visible
     *         in the audit trail
     */
    public synchronized BackupEntry createBackup(String instanceId, String triggerType) throws IOException {
        InstanceConfig config = instanceManager.getInstance(instanceId);
        Path instanceDir = instanceManager.getInstanceDir(instanceId);
        Path instanceBackupsDir = globalBackupsDir.resolve(instanceId);
        Files.createDirectories(instanceBackupsDir);

        String backupId = newBackupId();
        String filename = backupId + ".tar.lz4";
        Path targetArchive = instanceBackupsDir.resolve(filename);
        Path metadataFile = instanceBackupsDir.resolve(backupId + ".json");

        List<String> auditLogs = new ArrayList<>();
        auditLogs.add("Starting backup for instance: " + config.getName() + " (" + instanceId + ")");
        auditLogs.add("Trigger type: " + triggerType);

        MinecraftProcessManager pm = instanceManager.getProcessManager(instanceId);
        boolean wasRunning = pm != null && pm.isRunning();

        BackupEntry entry = new BackupEntry();
        entry.setId(backupId);
        entry.setInstanceId(instanceId);
        entry.setFilename(filename);
        entry.setTimestamp(System.currentTimeMillis());
        entry.setTriggerType(triggerType);
        entry.setStatus(BackupEntry.STATUS_IN_PROGRESS);

        // 1. Flush chunks and pause autosave while the server is live, so the
        //    archive captures a consistent world state. A console hiccup must
        //    not block the backup, so failures here degrade to "offline" mode.
        if (wasRunning) {
            auditLogs.add("Server is running. Sending 'save-off' and 'save-all' commands...");
            try {
                pm.sendCommand("save-off");
                pm.sendCommand("save-all");
                Thread.sleep(SAVE_FLUSH_WAIT_MS);
            } catch (InterruptedException e) {
                Thread.currentThread().interrupt();
            } catch (RuntimeException e) {
                auditLogs.add("WARNING: could not pause autosave: " + e.getMessage());
                log.warn("Could not pause autosave for instance {}", instanceId, e);
            }
        } else {
            auditLogs.add("Server is offline. Proceeding directly with archive.");
        }

        // 2. Stream the instance dir into an LZ4-compressed TAR archive.
        try {
            Lz4ArchiveUtil.compressDirectory(instanceDir, targetArchive, null, auditLogs);
            entry.setStatus(BackupEntry.STATUS_COMPLETED);
            entry.setSizeBytes(Files.size(targetArchive));
            auditLogs.add("Backup file written successfully: " + filename);
        } catch (Exception e) {
            entry.setStatus(BackupEntry.STATUS_FAILED);
            auditLogs.add("ERROR during compression: " + e.getMessage());
            log.error("Backup failed for instance " + instanceId, e);
            // Drop the partial archive so a corrupt file can never be restored.
            try {
                Files.deleteIfExists(targetArchive);
            } catch (IOException deleteError) {
                log.warn("Could not delete partial backup archive {}", targetArchive, deleteError);
            }
        } finally {
            // 3. Always resume autosave if the server is still alive.
            if (wasRunning) {
                try {
                    pm.sendCommand("save-on");
                    auditLogs.add("Resumed server auto-saving ('save-on').");
                } catch (RuntimeException e) {
                    auditLogs.add("WARNING: could not resume auto-saving: " + e.getMessage());
                    log.warn("Could not resume autosave for instance {}", instanceId, e);
                }
            }
        }

        entry.setLogs(auditLogs);
        Files.writeString(metadataFile, GSON.toJson(entry), StandardCharsets.UTF_8);

        // 4. Enforce the retention policy configured for this instance.
        pruneOldBackups(instanceId, config.getBackupRetention());

        return entry;
    }

    /**
     * Persists a new retention limit for an instance and immediately prunes any
     * backups beyond it.
     *
     * @return how many backups were deleted by the new limit
     */
    public synchronized int setRetention(String instanceId, int retention) throws IOException {
        if (retention < InstanceConfig.MIN_BACKUP_RETENTION
                || retention > InstanceConfig.MAX_BACKUP_RETENTION) {
            throw new IllegalArgumentException("retention must be between "
                    + InstanceConfig.MIN_BACKUP_RETENTION + " and "
                    + InstanceConfig.MAX_BACKUP_RETENTION);
        }
        instanceManager.updateBackupRetention(instanceId, retention);

        List<BackupEntry> backups = listBackups(instanceId);
        int toDelete = Math.max(0, backups.size() - retention);
        if (toDelete > 0) {
            pruneOldBackups(instanceId, retention);
            log.info("Retention for instance {} set to {} — pruned {} old backup(s)",
                    instanceId, retention, toDelete);
        }
        return toDelete;
    }

    /**
     * Restores a backup into an instance directory. The server is stopped
     * first; the pre-restore state is moved to a temporary rollback folder and
     * is either discarded on success or moved back if extraction fails.
     * Afterwards the instance config is re-read from disk so the wrapper sees
     * the restored {@code instance.json}.
     */
    public synchronized void restoreBackup(String instanceId, String backupId) throws IOException {
        instanceManager.getInstance(instanceId); // throws IllegalArgumentException for unknown ids
        Path instanceBackupsDir = globalBackupsDir.resolve(instanceId);
        Path archiveFile = instanceBackupsDir.resolve(backupId + ".tar.lz4");
        if (!Files.isRegularFile(archiveFile)) {
            throw new FileNotFoundException("Backup archive not found: " + backupId);
        }

        // 1. Safely stop the server and wait for it to exit before touching files.
        if (instanceManager.isRunning(instanceId)) {
            log.info("Stopping instance {} before restore", instanceId);
            instanceManager.stopInstance(instanceId);
        }

        Path instanceDir = instanceManager.getInstanceDir(instanceId);

        // 2. Move the current (possibly broken) state aside. A rename on the
        //    same volume is cheap even for large worlds, and keeps a snapshot
        //    to fall back to if extraction fails part-way.
        Path rollbackDir = instanceBackupsDir.resolve("rollback").resolve(backupId);
        if (Files.exists(instanceDir)) {
            deleteRecursively(rollbackDir);
            Files.createDirectories(rollbackDir.getParent());
            Files.move(instanceDir, rollbackDir);
            log.info("Moved pre-restore instance state to rollback folder: {}", rollbackDir);
        }

        // 3. Extract the archive into a fresh instance directory.
        Files.createDirectories(instanceDir);
        try {
            Lz4ArchiveUtil.extractArchive(archiveFile, instanceDir);
        } catch (IOException e) {
            // Undo: discard the partial extraction and put the old state back.
            deleteRecursively(instanceDir);
            if (Files.exists(rollbackDir)) {
                Files.move(rollbackDir, instanceDir);
            }
            throw new IOException("Restore failed; instance state has been rolled back: "
                    + e.getMessage(), e);
        }

        // Success: the rollback snapshot was only temporary.
        deleteRecursively(rollbackDir);

        // 4. Re-read the restored instance config so in-memory state matches disk.
        try {
            instanceManager.reloadInstanceFromDisk(instanceId);
        } catch (IOException e) {
            log.warn("Could not reload instance config after restore: {}", e.getMessage());
        }
        log.info("Restored backup {} into instance {}", backupId, instanceId);
    }

    /** All backup metadata records for an instance, newest first. */
    public List<BackupEntry> listBackups(String instanceId) {
        Path instanceBackupsDir = globalBackupsDir.resolve(instanceId);
        if (!Files.isDirectory(instanceBackupsDir)) {
            return List.of();
        }

        List<BackupEntry> list = new ArrayList<>();
        try (var stream = Files.list(instanceBackupsDir)) {
            for (Path p : stream.filter(p -> p.getFileName().toString().endsWith(".json")).toList()) {
                try {
                    BackupEntry entry = GSON.fromJson(Files.readString(p, StandardCharsets.UTF_8),
                            BackupEntry.class);
                    if (entry != null && entry.getId() != null) {
                        list.add(entry);
                    }
                } catch (IOException | RuntimeException e) {
                    log.warn("Could not read backup metadata {}", p, e);
                }
            }
        } catch (IOException e) {
            log.warn("Could not list backups for instance {}", instanceId, e);
        }

        list.sort(Comparator.comparingLong(BackupEntry::getTimestamp).reversed());
        return list;
    }

    private void pruneOldBackups(String instanceId, int maxKeep) {
        List<BackupEntry> backups = listBackups(instanceId);
        if (backups.size() <= maxKeep) {
            return;
        }
        Path instanceBackupsDir = globalBackupsDir.resolve(instanceId);
        for (int i = maxKeep; i < backups.size(); i++) {
            BackupEntry old = backups.get(i);
            try {
                Files.deleteIfExists(instanceBackupsDir.resolve(old.getFilename()));
                Files.deleteIfExists(instanceBackupsDir.resolve(old.getId() + ".json"));
                log.info("Pruned old backup: {} ({})", old.getId(), old.getTriggerType());
            } catch (IOException e) {
                log.warn("Failed to prune backup {}", old.getId(), e);
            }
        }
    }

    private static String newBackupId() {
        return "backup-" + System.currentTimeMillis() + "-" + BACKUP_SEQ.incrementAndGet();
    }

    /** Deletes a file or directory tree; a no-op when the path does not exist. */
    private static void deleteRecursively(Path path) throws IOException {
        if (!Files.exists(path)) {
            return;
        }
        try (var walk = Files.walk(path)) {
            for (Path p : walk.sorted(Comparator.reverseOrder()).toList()) {
                Files.deleteIfExists(p);
            }
        }
    }
}
