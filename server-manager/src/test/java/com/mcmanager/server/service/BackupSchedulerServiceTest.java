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
import java.time.Instant;
import java.time.LocalTime;
import java.time.ZoneId;
import java.time.ZonedDateTime;
import java.util.List;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertTrue;

class BackupSchedulerServiceTest {

    @TempDir
    Path tempDir;

    private ServerInstanceManager newInstanceManager() throws IOException {
        return new ServerInstanceManager(tempDir, new ConsoleStreamHandler());
    }

    private InstanceConfig newInstance(ServerInstanceManager manager) throws IOException {
        InstanceConfig cfg = manager.createInstance("Test Instance", "1.21.4", "vanilla", "");
        Files.writeString(manager.getInstanceDir(cfg.getId()).resolve("level.dat"), "data");
        return cfg;
    }

    @Test
    void createsScheduledBackupWhenNoneExists() throws IOException {
        ServerInstanceManager manager = newInstanceManager();
        InstanceConfig cfg = newInstance(manager);
        cfg.setBackupFrequency(InstanceConfig.BACKUP_DAILY);

        BackupService backupService = new BackupService(tempDir, manager);
        BackupSchedulerService scheduler = new BackupSchedulerService(manager, backupService);

        scheduler.checkScheduledBackups();

        List<BackupEntry> backups = backupService.listBackups(cfg.getId());
        assertEquals(1, backups.size());
        assertEquals(BackupEntry.TRIGGER_SCHEDULED, backups.get(0).getTriggerType());
        assertEquals(BackupEntry.STATUS_COMPLETED, backups.get(0).getStatus());
    }

    @Test
    void skipsBackupWhenSlotHasNotElapsed() throws IOException {
        ServerInstanceManager manager = newInstanceManager();
        InstanceConfig cfg = newInstance(manager);
        cfg.setBackupFrequency(InstanceConfig.BACKUP_DAILY);

        BackupService backupService = new BackupService(tempDir, manager);
        BackupSchedulerService scheduler = new BackupSchedulerService(manager, backupService);

        scheduler.checkScheduledBackups(); // creates the initial backup
        // The next slot is a full cadence away, so an immediate second pass must not.
        scheduler.checkScheduledBackups();

        assertEquals(1, backupService.listBackups(cfg.getId()).size());
    }

    @Test
    void offScheduleNeverBacksUp() throws IOException {
        ServerInstanceManager manager = newInstanceManager();
        InstanceConfig cfg = newInstance(manager); // default schedule is "off"

        BackupService backupService = new BackupService(tempDir, manager);
        BackupSchedulerService scheduler = new BackupSchedulerService(manager, backupService);

        scheduler.checkScheduledBackups();

        assertTrue(backupService.listBackups(cfg.getId()).isEmpty());
    }

    @Test
    void nextScheduledSlotAdvancesByFrequency() {
        ZoneId zone = ZoneId.systemDefault();
        Instant last = ZonedDateTime.of(2025, 3, 3, 3, 0, 0, 0, zone) // Monday 03:00
                .toInstant();

        assertEquals(ZonedDateTime.of(2025, 3, 4, 3, 0, 0, 0, zone).toInstant(),
                BackupSchedulerService.nextScheduledSlot(InstanceConfig.BACKUP_DAILY,
                        LocalTime.of(3, 0), last).toInstant());
        assertEquals(ZonedDateTime.of(2025, 3, 10, 3, 0, 0, 0, zone).toInstant(),
                BackupSchedulerService.nextScheduledSlot(InstanceConfig.BACKUP_WEEKLY,
                        LocalTime.of(3, 0), last).toInstant());
        assertEquals(ZonedDateTime.of(2025, 4, 3, 3, 0, 0, 0, zone).toInstant(),
                BackupSchedulerService.nextScheduledSlot(InstanceConfig.BACKUP_MONTHLY,
                        LocalTime.of(3, 0), last).toInstant());
    }

    @Test
    void nextScheduledSlotMovesToNextPeriodWhenTimeAlreadyPassed() {
        ZoneId zone = ZoneId.systemDefault();
        // Last backup Monday 03:00, slot at 02:00 -> Tuesday 02:00 is the next slot.
        Instant last = ZonedDateTime.of(2025, 3, 3, 3, 0, 0, 0, zone).toInstant();

        assertEquals(ZonedDateTime.of(2025, 3, 4, 2, 0, 0, 0, zone).toInstant(),
                BackupSchedulerService.nextScheduledSlot(InstanceConfig.BACKUP_DAILY,
                        LocalTime.of(2, 0), last).toInstant());
    }

    @Test
    void monthlySlotClampsToMonthLength() {
        ZoneId zone = ZoneId.systemDefault();
        // Jan 31 + one month has no 31st in February (2025 is not a leap year).
        Instant last = ZonedDateTime.of(2025, 1, 31, 3, 0, 0, 0, zone).toInstant();

        assertEquals(ZonedDateTime.of(2025, 2, 28, 3, 0, 0, 0, zone).toInstant(),
                BackupSchedulerService.nextScheduledSlot(InstanceConfig.BACKUP_MONTHLY,
                        LocalTime.of(3, 0), last).toInstant());
    }

    @Test
    void invalidBackupTimeFallsBackToDefault() throws IOException {
        ServerInstanceManager manager = newInstanceManager();
        InstanceConfig cfg = newInstance(manager);
        cfg.setBackupFrequency(InstanceConfig.BACKUP_DAILY);
        cfg.setBackupTime("not-a-time");

        BackupService backupService = new BackupService(tempDir, manager);
        BackupSchedulerService scheduler = new BackupSchedulerService(manager, backupService);

        // An unparseable time must not crash the pass; the instance is due because
        // it has never been backed up.
        scheduler.checkScheduledBackups();
        assertFalse(backupService.listBackups(cfg.getId()).isEmpty());
    }
}
