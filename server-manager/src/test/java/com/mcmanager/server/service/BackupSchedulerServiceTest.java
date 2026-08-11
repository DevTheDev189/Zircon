package com.mcmanager.server.service;

import com.mcmanager.core.model.BackupEntry;
import com.mcmanager.core.model.InstanceConfig;
import com.mcmanager.server.instance.ServerInstanceManager;
import com.mcmanager.server.process.ConsoleStreamHandler;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.io.TempDir;

import java.io.IOException;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.List;

import static org.junit.jupiter.api.Assertions.assertEquals;

class BackupSchedulerServiceTest {

    @TempDir
    Path tempDir;

    private ServerInstanceManager newInstanceManager() throws IOException {
        return new ServerInstanceManager(tempDir, new ConsoleStreamHandler());
    }

    @Test
    void createsScheduledBackupWhenNoneExists() throws IOException {
        ServerInstanceManager manager = newInstanceManager();
        InstanceConfig cfg = manager.createInstance("Test World", "1.21.4", "vanilla", "");
        Files.writeString(manager.getInstanceDir(cfg.getId()).resolve("level.dat"), "data");

        BackupService backupService = new BackupService(tempDir, manager);
        BackupSchedulerService scheduler = new BackupSchedulerService(manager, backupService);

        scheduler.checkScheduledBackups();

        List<BackupEntry> backups = backupService.listBackups(cfg.getId());
        assertEquals(1, backups.size());
        assertEquals(BackupEntry.TRIGGER_SCHEDULED, backups.get(0).getTriggerType());
        assertEquals(BackupEntry.STATUS_COMPLETED, backups.get(0).getStatus());
    }

    @Test
    void skipsBackupWhenOneWasJustCreated() throws IOException {
        ServerInstanceManager manager = newInstanceManager();
        InstanceConfig cfg = manager.createInstance("Test World", "1.21.4", "vanilla", "");
        Files.writeString(manager.getInstanceDir(cfg.getId()).resolve("level.dat"), "data");

        BackupService backupService = new BackupService(tempDir, manager);
        BackupSchedulerService scheduler = new BackupSchedulerService(manager, backupService);

        scheduler.checkScheduledBackups();
        // A second pass immediately after must not create another backup
        // (the interval-since-last-backup gate).
        scheduler.checkScheduledBackups();

        assertEquals(1, backupService.listBackups(cfg.getId()).size());
    }
}
