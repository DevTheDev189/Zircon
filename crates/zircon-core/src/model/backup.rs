//! Metadata record for one server instance backup: the `.tar.lz4` archive plus
//! an audit trail of what happened while it was created. Persisted as
//! `<backupId>.json` next to the archive under `<data>/backups/<instanceId>/`.
//!
//! Port of `com.mcmanager.core.model.BackupEntry`.

use serde::{Deserialize, Serialize};

pub const TRIGGER_MANUAL: &str = "manual";
pub const TRIGGER_SCHEDULED: &str = "scheduled";

pub const STATUS_IN_PROGRESS: &str = "in_progress";
pub const STATUS_COMPLETED: &str = "completed";
pub const STATUS_FAILED: &str = "failed";

/// Metadata record for one server instance backup.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct BackupEntry {
    pub id: String,
    pub instance_id: String,
    /// Archive file name inside the instance's backups folder.
    pub filename: String,
    /// Epoch millis when the backup was created.
    pub timestamp: i64,
    /// Archive size in bytes (0 while in progress or after a failure).
    pub size_bytes: u64,
    /// One of `TRIGGER_MANUAL` or `TRIGGER_SCHEDULED`.
    pub trigger_type: String,
    /// One of `STATUS_IN_PROGRESS`, `STATUS_COMPLETED`, `STATUS_FAILED`.
    pub status: String,
    /// Human-readable audit trail (flush commands, file counts, errors).
    #[serde(default)]
    pub logs: Vec<String>,
}

impl BackupEntry {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        id: impl Into<String>,
        instance_id: impl Into<String>,
        filename: impl Into<String>,
        timestamp: i64,
        trigger_type: impl Into<String>,
        status: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            instance_id: instance_id.into(),
            filename: filename.into(),
            timestamp,
            size_bytes: 0,
            trigger_type: trigger_type.into(),
            status: status.into(),
            logs: Vec::new(),
        }
    }

    pub fn add_log(&mut self, line: impl Into<String>) {
        self.logs.push(line.into());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_via_json() {
        let mut entry = BackupEntry::new(
            "b1",
            "inst-1",
            "b1.tar.lz4",
            1_700_000_000_000,
            TRIGGER_MANUAL,
            STATUS_COMPLETED,
        );
        entry.size_bytes = 42;
        entry.add_log("Archived 2 files");

        let json = serde_json::to_string(&entry).unwrap();
        let parsed: BackupEntry = serde_json::from_str(&json).unwrap();
        assert_eq!(entry, parsed);
        assert!(json.contains("\"triggerType\""));
        assert!(json.contains("\"sizeBytes\""));
    }
}
