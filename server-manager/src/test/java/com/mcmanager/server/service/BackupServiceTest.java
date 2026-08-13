package com.mcmanager.server.service;

import com.mcmanager.core.model.BackupEntry;
import com.mcmanager.core.model.InstanceConfig;
import com.mcmanager.server.instance.ServerInstanceManager;
import com.mcmanager.server.process.ConsoleStreamHandler;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.io.TempDir;

import java.io.FileNotFoundException;
import java.io.IOException;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.List;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertThrows;
import static org.junit.jupiter.api.Assertions.assertTrue;

class BackupServiceTest {

    @TempDir
    Path tempDir;

    private ServerInstanceManager newInstanceManager() throws IOException {
        return new ServerInstanceManager(tempDir, new ConsoleStreamHandler());
    }

    private BackupService newService(ServerInstanceManager manager) {
        return new BackupService(tempDir, manager);
    }

    @Test
    void createBackupWritesArchiveAndMetadata() throws IOException {
        ServerInstanceManager manager = newInstanceManager();
        InstanceConfig cfg = manager.createInstance("Test Instance", "1.21.4", "vanilla", "");
        Path worldDir = manager.getInstanceDir(cfg.getId()).resolve("world");
        Files.createDirectories(worldDir);
        Files.writeString(worldDir.resolve("level.dat"), "world data");

        BackupService service = newService(manager);
        BackupEntry entry = service.createBackup(cfg.getId(), BackupEntry.TRIGGER_MANUAL);

        assertEquals(BackupEntry.STATUS_COMPLETED, entry.getStatus());
        assertEquals(BackupEntry.TRIGGER_MANUAL, entry.getTriggerType());
        assertTrue(entry.getSizeBytes() > 0);
        assertFalse(entry.getLogs().isEmpty());
        assertTrue(entry.getLogs().get(0).contains("Starting backup"));

        Path backupsDir = tempDir.resolve("backups").resolve(cfg.getId());
        assertTrue(Files.isRegularFile(backupsDir.resolve(entry.getFilename())));
        assertTrue(Files.isRegularFile(backupsDir.resolve(entry.getId() + ".json")));
        // The archive must contain the world file.
        List<BackupEntry> listed = service.listBackups(cfg.getId());
        assertEquals(1, listed.size());
        assertEquals(entry.getId(), listed.get(0).getId());
    }

    @Test
    void restoreReplacesInstanceStateAndDropsRollback() throws IOException {
        ServerInstanceManager manager = newInstanceManager();
        InstanceConfig cfg = manager.createInstance("Test Instance", "1.21.4", "vanilla", "");
        Path instanceDir = manager.getInstanceDir(cfg.getId());
        Files.writeString(instanceDir.resolve("level.dat"), "original");

        BackupService service = newService(manager);
        BackupEntry entry = service.createBackup(cfg.getId(), BackupEntry.TRIGGER_MANUAL);

        // Mutate the instance after the backup was taken.
        Files.writeString(instanceDir.resolve("level.dat"), "corrupted");
        Files.writeString(instanceDir.resolve("new-file.txt"), "extra");

        service.restoreBackup(cfg.getId(), entry.getId());

        assertEquals("original", Files.readString(instanceDir.resolve("level.dat")));
        assertFalse(Files.exists(instanceDir.resolve("new-file.txt")));
        // The temporary rollback snapshot must be gone after a successful restore.
        Path rollbackDir = tempDir.resolve("backups").resolve(cfg.getId())
                .resolve("rollback").resolve(entry.getId());
        assertFalse(Files.exists(rollbackDir));
    }

    @Test
    void prunesOldestBackupsBeyondRetentionLimit() throws IOException {
        ServerInstanceManager manager = newInstanceManager();
        InstanceConfig cfg = manager.createInstance("Test Instance", "1.21.4", "vanilla", "");
        Files.writeString(manager.getInstanceDir(cfg.getId()).resolve("level.dat"), "data");

        BackupService service = newService(manager);
        int target = InstanceConfig.DEFAULT_BACKUP_RETENTION + 3;
        for (int i = 0; i < target; i++) {
            service.createBackup(cfg.getId(), BackupEntry.TRIGGER_SCHEDULED);
        }

        List<BackupEntry> backups = service.listBackups(cfg.getId());
        assertEquals(InstanceConfig.DEFAULT_BACKUP_RETENTION, backups.size());
        // Every remaining entry has both its archive and metadata on disk.
        Path backupsDir = tempDir.resolve("backups").resolve(cfg.getId());
        for (BackupEntry b : backups) {
            assertTrue(Files.isRegularFile(backupsDir.resolve(b.getFilename())));
        }
    }

    @Test
    void restoreUnknownBackupThrows() throws IOException {
        ServerInstanceManager manager = newInstanceManager();
        InstanceConfig cfg = manager.createInstance("Test Instance", "1.21.4", "vanilla", "");

        BackupService service = newService(manager);
        assertThrows(FileNotFoundException.class,
                () -> service.restoreBackup(cfg.getId(), "backup-does-not-exist"));
    }

    @Test
    void failedBackupRecordsFailureAndDropsPartialArchive() throws IOException {
        ServerInstanceManager manager = newInstanceManager();
        InstanceConfig cfg = manager.createInstance("Test Instance", "1.21.4", "vanilla", "");
        // Removing the instance directory forces the archive walk to fail.
        Path instanceDir = manager.getInstanceDir(cfg.getId());
        try (var walk = Files.walk(instanceDir)) {
            for (Path p : walk.sorted(java.util.Comparator.reverseOrder()).toList()) {
                Files.deleteIfExists(p);
            }
        }

        BackupService service = newService(manager);
        BackupEntry entry = service.createBackup(cfg.getId(), BackupEntry.TRIGGER_MANUAL);

        assertEquals(BackupEntry.STATUS_FAILED, entry.getStatus());
        assertTrue(entry.getLogs().stream().anyMatch(l -> l.contains("ERROR")));

        Path backupsDir = tempDir.resolve("backups").resolve(cfg.getId());
        assertTrue(Files.isRegularFile(backupsDir.resolve(entry.getId() + ".json")));
        assertFalse(Files.exists(backupsDir.resolve(entry.getFilename())));
    }

    @Test
    void setRetentionPrunesOldBackupsAndPersists() throws IOException {
        ServerInstanceManager manager = newInstanceManager();
        InstanceConfig cfg = manager.createInstance("Test Instance", "1.21.4", "vanilla", "");
        Files.writeString(manager.getInstanceDir(cfg.getId()).resolve("level.dat"), "data");

        BackupService service = newService(manager);
        for (int i = 0; i < 5; i++) {
            service.createBackup(cfg.getId(), BackupEntry.TRIGGER_MANUAL);
        }
        assertEquals(5, service.listBackups(cfg.getId()).size());

        int deleted = service.setRetention(cfg.getId(), 2);

        assertEquals(3, deleted);
        List<BackupEntry> backups = service.listBackups(cfg.getId());
        assertEquals(2, backups.size());
        // The new limit is persisted on the instance config.
        assertEquals(2, manager.getInstance(cfg.getId()).getBackupRetention());
        // Only the retained archives remain on disk.
        Path backupsDir = tempDir.resolve("backups").resolve(cfg.getId());
        for (BackupEntry b : backups) {
            assertTrue(Files.isRegularFile(backupsDir.resolve(b.getFilename())));
        }
    }

    @Test
    void setRetentionAboveCurrentCountDeletesNothing() throws IOException {
        ServerInstanceManager manager = newInstanceManager();
        InstanceConfig cfg = manager.createInstance("Test Instance", "1.21.4", "vanilla", "");
        Files.writeString(manager.getInstanceDir(cfg.getId()).resolve("level.dat"), "data");

        BackupService service = newService(manager);
        for (int i = 0; i < 2; i++) {
            service.createBackup(cfg.getId(), BackupEntry.TRIGGER_MANUAL);
        }

        int deleted = service.setRetention(cfg.getId(), 5);

        assertEquals(0, deleted);
        assertEquals(2, service.listBackups(cfg.getId()).size());
        assertEquals(5, manager.getInstance(cfg.getId()).getBackupRetention());
    }

    @Test
    void createBackupRespectsConfiguredRetention() throws IOException {
        ServerInstanceManager manager = newInstanceManager();
        InstanceConfig cfg = manager.createInstance("Test Instance", "1.21.4", "vanilla", "");
        Files.writeString(manager.getInstanceDir(cfg.getId()).resolve("level.dat"), "data");
        cfg.setBackupRetention(2);

        BackupService service = newService(manager);
        for (int i = 0; i < 5; i++) {
            service.createBackup(cfg.getId(), BackupEntry.TRIGGER_SCHEDULED);
        }

        assertEquals(2, service.listBackups(cfg.getId()).size());
    }

    @Test
    void setRetentionRejectsOutOfRangeValues() throws IOException {
        ServerInstanceManager manager = newInstanceManager();
        InstanceConfig cfg = manager.createInstance("Test Instance", "1.21.4", "vanilla", "");

        BackupService service = newService(manager);
        assertThrows(IllegalArgumentException.class,
                () -> service.setRetention(cfg.getId(), 0));
        assertThrows(IllegalArgumentException.class,
                () -> service.setRetention(cfg.getId(), 101));
    }
}
