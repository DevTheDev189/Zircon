package com.mcmanager.server.service;

import com.mcmanager.core.model.BackupEntry;
import com.mcmanager.core.model.InstanceConfig;
import com.mcmanager.server.instance.ServerInstanceManager;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

import java.time.Instant;
import java.time.LocalTime;
import java.time.ZoneId;
import java.time.ZonedDateTime;
import java.time.format.DateTimeParseException;
import java.util.List;
import java.util.concurrent.Executors;
import java.util.concurrent.ScheduledExecutorService;
import java.util.concurrent.TimeUnit;

/**
 * Runs scheduled (automatic) backups for every instance according to each
 * instance's own backup schedule: {@code off} (manual only), {@code daily},
 * {@code weekly} or {@code monthly}, at a configured local time of day.
 *
 * <p>The scheduler wakes up every {@link #POLL_INTERVAL_MINUTES} minutes and
 * backs up any instance whose next scheduled slot has been reached. The next
 * slot is derived from the most recent backup of <em>any</em> type, so a manual
 * backup taken at the scheduled time satisfies that slot instead of producing
 * a redundant automatic one. An instance with no backups yet is backed up
 * immediately on the first poll after its schedule is enabled, then settles
 * into the configured cadence. Retention pruning in {@link BackupService}
 * bounds how many archives are kept per instance.
 */
public class BackupSchedulerService {

    private static final Logger log = LoggerFactory.getLogger(BackupSchedulerService.class);

    /** How often the scheduler wakes up to check whether backups are due. */
    public static final long POLL_INTERVAL_MINUTES = 10;

    /** Fallback time of day when an instance's {@code backupTime} is unset or unparseable. */
    private static final LocalTime DEFAULT_BACKUP_TIME = LocalTime.of(2, 0);

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
        log.info("Backup scheduler started: checks every {} min against per-instance schedules",
                POLL_INTERVAL_MINUTES);
    }

    /**
     * Runs one scheduled-backup pass over all instances. Package-private so
     * tests can drive it without waiting for the real poll delay.
     */
    void checkScheduledBackups() {
        try {
            for (InstanceConfig inst : instanceManager.listInstances()) {
                try {
                    if (backupDue(inst)) {
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

    /**
     * @return {@code true} when the instance's schedule is enabled and its next
     *         scheduled slot has been reached (or it has never been backed up).
     */
    boolean backupDue(InstanceConfig config) {
        String frequency = config.getBackupFrequency();
        if (frequency == null || InstanceConfig.BACKUP_OFF.equalsIgnoreCase(frequency)) {
            return false;
        }
        List<BackupEntry> backups = backupService.listBackups(config.getId());
        if (backups.isEmpty()) {
            // Never backed up: start the cadence on the first poll after enabling.
            return true;
        }
        Instant last = Instant.ofEpochMilli(backups.get(0).getTimestamp());
        ZonedDateTime next = nextScheduledSlot(frequency, parseBackupTime(config.getBackupTime()), last);
        return !ZonedDateTime.now().isBefore(next);
    }

    /**
     * Computes the first configured slot strictly after {@code last}: the
     * slot's own date/time is anchored to the last backup and advanced by the
     * frequency until it passes it. Package-private so tests can assert the
     * exact slot math without waiting for real time.
     *
     * @param frequency one of {@code daily}, {@code weekly}, {@code monthly}
     * @param time      local time of day the slot fires at
     * @param last      the most recent backup instant (never {@code null})
     */
    static ZonedDateTime nextScheduledSlot(String frequency, LocalTime time, Instant last) {
        ZoneId zone = ZoneId.systemDefault();
        ZonedDateTime anchor = ZonedDateTime.ofInstant(last, zone);
        ZonedDateTime candidate = anchor.toLocalDate().atTime(time).atZone(zone);
        while (!candidate.isAfter(anchor)) {
            candidate = advance(candidate, frequency);
        }
        return candidate;
    }

    private static ZonedDateTime advance(ZonedDateTime slot, String frequency) {
        return switch (frequency) {
            case InstanceConfig.BACKUP_WEEKLY -> slot.plusWeeks(1);
            case InstanceConfig.BACKUP_MONTHLY -> slot.plusMonths(1);
            default -> slot.plusDays(1); // daily (and any unknown value as a safe fallback)
        };
    }

    private static LocalTime parseBackupTime(String time) {
        if (time != null) {
            try {
                return LocalTime.parse(time);
            } catch (DateTimeParseException e) {
                log.warn("Invalid backupTime '{}', defaulting to {}", time, DEFAULT_BACKUP_TIME);
            }
        }
        return DEFAULT_BACKUP_TIME;
    }

    public void stop() {
        scheduler.shutdown();
        log.info("Backup scheduler stopped.");
    }
}
