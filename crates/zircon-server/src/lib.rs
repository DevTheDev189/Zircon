//! Zircon server manager daemon.
//!
//! Pure-Rust port of the Java `server-manager` Gradle module. Supervises
//! isolated Minecraft server instances, runs the Tokio TCP multiplexer with the
//! join-ticket gate, manages LZ4 backups, and exposes the Axum admin REST +
//! WebSocket API.

pub mod auth;
pub mod config;
pub mod installer;
pub mod instance;
pub mod multiplexer;
pub mod process;
pub mod services;
pub mod stats;
pub mod tickets;
pub mod web;

#[cfg(test)]
pub mod test_util {
    use std::path::PathBuf;

    /// Unique temp dir per test. The test thread name disambiguates parallel
    /// tests that would otherwise share a path keyed only by process id.
    pub fn temp_dir(prefix: &str) -> PathBuf {
        let thread = std::thread::current()
            .name()
            .unwrap_or("test")
            .replace("::", "-");
        let dir =
            std::env::temp_dir().join(format!("zircon-{prefix}-{thread}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }
}
