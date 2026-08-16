//! Runs scheduled (automatic) backups for every instance according to each
//! instance's own backup schedule: `off` (manual only), `daily`, `weekly` or
//! `monthly`, at a configured local time of day.
//!
//! The scheduler wakes up every `POLL_INTERVAL_MINUTES` minutes and backs up
//! any instance whose next scheduled slot has been reached. The next slot is
//! derived from the most recent backup of *any* type, so a manual backup taken
//! at the scheduled time satisfies that slot instead of producing a redundant
//! automatic one. An instance with no backups yet is backed up immediately on
//! the first poll after its schedule is enabled.
//!
//! Port of `com.mcmanager.server.service.BackupSchedulerService`.

use std::sync::Arc;
use std::time::Duration;

use chrono::{DateTime, Local, Utc};
use zircon_core::model::backup::TRIGGER_SCHEDULED;
use zircon_core::model::InstanceConfig;

use super::backup::BackupService;
use crate::instance::ServerInstanceManager;

/// How often the scheduler wakes up to check whether backups are due.
pub const POLL_INTERVAL_MINUTES: u64 = 10;

/// Fallback time of day when an instance's `backupTime` is unset or unparseable.
pub fn default_backup_time() -> chrono::NaiveTime {
    chrono::NaiveTime::from_hms_opt(2, 0, 0).unwrap()
}

/// Runs scheduled (automatic) backups for every instance.
pub struct BackupSchedulerService {
    instance_manager: Arc<ServerInstanceManager>,
    backup_service: Arc<BackupService>,
}

impl BackupSchedulerService {
    pub fn new(
        instance_manager: Arc<ServerInstanceManager>,
        backup_service: Arc<BackupService>,
    ) -> Self {
        Self {
            instance_manager,
            backup_service,
        }
    }

    /// Starts the background scheduler loop.
    pub fn start(&self) -> tokio::task::JoinHandle<()> {
        let instance_manager = self.instance_manager.clone();
        let backup_service = self.backup_service.clone();
        tracing::info!(
            "Backup scheduler started: checks every {POLL_INTERVAL_MINUTES} min against per-instance schedules"
        );
        tokio::spawn(async move {
            // Initial delay of one minute lets the wrapper finish booting.
            tokio::time::sleep(Duration::from_secs(60)).await;
            loop {
                check_scheduled_backups(&instance_manager, &backup_service).await;
                tokio::time::sleep(Duration::from_secs(POLL_INTERVAL_MINUTES * 60)).await;
            }
        })
    }

    /// Runs one scheduled-backup pass over all instances. Exposed for tests so
    /// they can drive it without waiting for the real poll delay.
    pub async fn check_scheduled_backups(&self) {
        check_scheduled_backups(&self.instance_manager, &self.backup_service).await;
    }

    /// Returns `true` when the instance's schedule is enabled and its next
    /// scheduled slot has been reached (or it has never been backed up).
    pub fn backup_due(&self, config: &InstanceConfig) -> bool {
        if config
            .backup_frequency
            .eq_ignore_ascii_case(zircon_core::model::instance::BACKUP_OFF)
        {
            return false;
        }
        let backups = self.backup_service.list_backups(&config.id);
        if backups.is_empty() {
            // Never backed up: start the cadence on the first poll after enabling.
            return true;
        }
        let last =
            DateTime::<Utc>::from_timestamp_millis(backups[0].timestamp).unwrap_or_else(Utc::now);
        let next = next_scheduled_slot(
            &config.backup_frequency,
            parse_backup_time(&config.backup_time),
            last,
        );
        Utc::now() >= next
    }
}

/// Runs one scheduled-backup pass over all instances.
async fn check_scheduled_backups(
    instance_manager: &Arc<ServerInstanceManager>,
    backup_service: &Arc<BackupService>,
) {
    let instances = instance_manager.list_instances();
    for instance in instances {
        let scheduler =
            BackupSchedulerService::new(instance_manager.clone(), backup_service.clone());
        if scheduler.backup_due(&instance) {
            tracing::info!(
                "Running scheduled backup for instance '{}' ({})",
                instance.name,
                instance.id
            );
            if let Err(e) = backup_service
                .create_backup(&instance.id, TRIGGER_SCHEDULED)
                .await
            {
                // One failing instance must not block the others.
                tracing::error!("Scheduled backup failed for instance {}: {e}", instance.id);
            }
        }
    }
}

/// Computes the first configured slot strictly after `last`: the slot's own
/// date/time is anchored to the last backup and advanced by the frequency until
/// it passes it.
pub fn next_scheduled_slot(
    frequency: &str,
    time: chrono::NaiveTime,
    last: DateTime<Utc>,
) -> DateTime<Utc> {
    let local = last.with_timezone(&Local);
    let mut candidate = local
        .date_naive()
        .and_time(time)
        .and_local_timezone(Local)
        .single()
        .unwrap_or(local);
    while candidate <= local {
        candidate = advance(candidate, frequency);
    }
    candidate.with_timezone(&Utc)
}

fn advance(slot: DateTime<Local>, frequency: &str) -> DateTime<Local> {
    match frequency {
        zircon_core::model::instance::BACKUP_WEEKLY => slot + chrono::Duration::weeks(1),
        zircon_core::model::instance::BACKUP_MONTHLY => slot + chrono::Duration::days(30),
        _ => slot + chrono::Duration::days(1), // daily (and any unknown value as a safe fallback)
    }
}

fn parse_backup_time(time: &str) -> chrono::NaiveTime {
    if let Ok(parsed) = chrono::NaiveTime::parse_from_str(time, "%H:%M") {
        return parsed;
    }
    tracing::warn!("Invalid backupTime '{time}', defaulting to 02:00");
    default_backup_time()
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;

    fn temp_dir() -> std::path::PathBuf {
        crate::test_util::temp_dir("scheduler")
    }

    #[test]
    fn next_slot_is_after_last_and_aligned_to_frequency() {
        use chrono::Timelike;
        let base = Utc::now();

        // Daily: last at 23:30 → next is tomorrow 02:00.
        let time = chrono::NaiveTime::from_hms_opt(2, 0, 0).unwrap();
        let last = base.date_naive().and_hms_opt(23, 30, 0).unwrap().and_utc();
        let next = next_scheduled_slot("daily", time, last);
        assert!(next > last);
        assert_eq!(next.with_timezone(&Local).hour(), 2);
        assert_eq!(next.with_timezone(&Local).minute(), 0);

        // Weekly: last was yesterday, next is 7 days after the anchor day.
        let last = base - Duration::days(1);
        let next = next_scheduled_slot("weekly", time, last);
        assert!(next > last);
        assert_eq!(next.with_timezone(&Local).hour(), 2);

        // Monthly: next is roughly a month later.
        let next_month = next_scheduled_slot("monthly", time, base);
        assert!(next_month > base);
    }

    #[test]
    fn off_frequency_never_due() {
        let dir = temp_dir();
        let console = Arc::new(crate::process::console::ConsoleStreamHandler::new());
        let manager = Arc::new(ServerInstanceManager::new(&dir, console).unwrap());
        let instance = manager
            .create_instance("S", "1.20.4", "vanilla", "")
            .unwrap();
        let backup = Arc::new(BackupService::new(&dir, manager.clone()));
        let scheduler = BackupSchedulerService::new(manager, backup);

        let mut config = instance;
        config.backup_frequency = zircon_core::model::instance::BACKUP_OFF.to_string();
        assert!(!scheduler.backup_due(&config));

        config.backup_frequency = zircon_core::model::instance::BACKUP_DAILY.to_string();
        assert!(scheduler.backup_due(&config)); // never backed up → due immediately
        let _ = std::fs::remove_dir_all(&dir);
    }
}
