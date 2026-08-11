package com.mcmanager.server.service;

import com.mcmanager.core.model.BackupEntry;
import com.mcmanager.core.model.InstanceConfig;
import com.mcmanager.server.instance.ServerInstanceManager;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

import java.util.List;
import java.util.concurrent.Executors;
import java.util.concurrent.ScheduledExecutorService;
import java.util.concurrent.TimeUnit;

/**
 * Runs scheduled (automatic) backups for every instance on a fixed cadence.
 *
 * <p>The scheduler wakes up every {@link #POLL_INTERVAL_MINUTES} minutes and
 * backs up any instance whose most recent backup is older than
 * {@link #SCHEDULED_BACKUP_INTERVAL_MINUTES} minutes (the "interval has elapsed
 * since last backup" gate). Manual backups count as the most recent backup too,
 * so a freshly taken manual backup suppresses the next scheduled run instead of
 * double-archiving the same state. Retention pruning in {@link BackupService}
 * bounds how many archives are kept per instance.
 */
public class BackupSchedulerService {

    private static final Logger log = LoggerFactory.getLogger(BackupSchedulerService.class);

    /** How often the scheduler wakes up to check whether backups are due. */
    public static final long POLL_INTERVAL_MINUTES = 10;

    /** Minimum gap between two backups of the same instance. */
    public static final long SCHEDULED_BACKUP_INTERVAL_MINUTES = 10;

    private final ScheduledExecutorService scheduler;
    private final ServerInstanceManager instanceManager;
    private final BackupService backupService;

    public BackupSchedulerService(ServerInstanceManager instanceManager, BackupService backupService) {
        this.instanceManager = instanceManager;
        this.backupService = backupService;
        this.scheduler = Executors.newSingleThreadScheduledExecutor(r -> {
            Thread thread = new Thread(r, "backup-scheduler");
            thread.setDaemon(true);
            return thread;
        });
    }

    public void start() {
        // Initial delay of one minute lets the wrapper finish booting before the
        // first check; fixed-delay (not fixed-rate) scheduling ensures a slow
        // archive of a large world never queues up overlapping poll passes.
        scheduler.scheduleWithFixedDelay(this::checkScheduledBackups, 1,
                POLL_INTERVAL_MINUTES, TimeUnit.MINUTES);
        log.info("Backup scheduler started: poll every {} min, "
                        + "minimum interval between backups {} min",
                POLL_INTERVAL_MINUTES, SCHEDULED_BACKUP_INTERVAL_MINUTES);
    }

    /**
     * Runs one scheduled-backup pass over all instances. Package-private so
     * tests can drive it without waiting for the real poll delay.
     */
    void checkScheduledBackups() {
        try {
            for (InstanceConfig inst : instanceManager.listInstances()) {
                try {
                    if (backupDue(inst.getId())) {
                        log.info("Running scheduled backup for instance '{}' ({})",
                                inst.getName(), inst.getId());
                        backupService.createBackup(inst.getId(), BackupEntry.TRIGGER_SCHEDULED);
                    }
                } catch (Exception e) {
                    // One failing instance must not block the others.
                    log.error("Scheduled backup failed for instance {}", inst.getId(), e);
                }
            }
        } catch (Exception e) {
            log.error("Scheduled backup pass failed", e);
        }
    }

    /** @return {@code true} when the instance has no backup yet or its newest one is older than the interval. */
    private boolean backupDue(String instanceId) {
        List<BackupEntry> backups = backupService.listBackups(instanceId);
        if (backups.isEmpty()) {
            return true;
        }
        long newest = backups.get(0).getTimestamp();
        return System.currentTimeMillis() - newest
                >= TimeUnit.MINUTES.toMillis(SCHEDULED_BACKUP_INTERVAL_MINUTES);
    }

    public void stop() {
        scheduler.shutdown();
        log.info("Backup scheduler stopped.");
    }
}
