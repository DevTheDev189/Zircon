//! Append-only audit logging for administrative actions.
//!
//! Every sensitive action (login, password change, console command, 2FA
//! changes) is appended to `audit.log` in the data dir. The file is kept
//! owner-only (`0o600`) on Unix and restricted to SYSTEM/Administrators/current
//! user on Windows, so other local users cannot read or tamper with the trail.
//! Entries are written under a mutex so concurrent handlers never interleave
//! partial lines.

use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use chrono::Utc;

use crate::auth::auth_service::harden_secret_file;

/// Writes timestamped audit entries to `<data_dir>/audit.log`.
pub struct AuditLogger {
    log_file: PathBuf,
    lock: Mutex<()>,
}

impl AuditLogger {
    pub fn new(data_dir: &Path) -> Self {
        let log_file = data_dir.join("audit.log");
        Self {
            log_file,
            lock: Mutex::new(()),
        }
    }

    /// Appends one audit entry: `[timestamp] [USER:<username>] [<action>] <details>`.
    pub fn log(&self, username: &str, action: &str, details: &str) {
        let _guard = self.lock.lock().unwrap();
        let timestamp = Utc::now().to_rfc3339();
        let entry = format!("[{timestamp}] [USER:{username}] [{action}] {details}\n");

        if let Ok(mut file) = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.log_file)
        {
            // The file may pre-exist from before hardening existed; re-apply
            // the platform's secret-file hardening on every write so an
            // unwatched local user can never read or tamper with the trail.
            if let Err(e) = harden_secret_file(&self.log_file) {
                tracing::warn!("Could not harden audit log permissions: {e}");
            }
            let _ = file.write_all(entry.as_bytes());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn writes_append_only_entries() {
        let dir = crate::test_util::temp_dir("audit");
        let logger = AuditLogger::new(&dir);
        logger.log("admin", "LOGIN_SUCCESS", "Authenticated");
        logger.log("admin", "CONSOLE_COMMAND", "say hello");

        let content = std::fs::read_to_string(dir.join("audit.log")).unwrap();
        assert!(content.contains("[USER:admin] [LOGIN_SUCCESS] Authenticated"));
        assert!(content.contains("[USER:admin] [CONSOLE_COMMAND] say hello"));
        assert_eq!(2, content.lines().count());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn file_is_owner_only_on_unix() {
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let dir = crate::test_util::temp_dir("audit-perms");
            let logger = AuditLogger::new(&dir);
            logger.log("admin", "LOGIN_FAILED", "bad password");
            let mode = std::fs::metadata(dir.join("audit.log"))
                .unwrap()
                .permissions()
                .mode()
                & 0o777;
            assert_eq!(0o600, mode, "audit.log must be owner-only");
            let _ = std::fs::remove_dir_all(&dir);
        }
    }
}
