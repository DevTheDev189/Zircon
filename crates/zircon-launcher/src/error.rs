//! Unified error type for the launcher.
//!
//! Every module (auth, launch, sync, offline) reports failures through
//! [`LauncherError`] so the Tauri shell (Phase 5) has a single error surface to
//! render, log, and translate into user-facing messages.

use std::fmt;

/// Errors produced by the launcher pipeline.
#[derive(Debug)]
pub enum LauncherError {
    /// Network-level failures (DNS, connect, TLS, timeouts) from `reqwest`.
    Network(String),
    /// A non-2xx HTTP response.
    Http { status: u16, url: String },
    /// Filesystem failures.
    Io(std::io::Error),
    /// JSON parsing / serialization failures.
    Json(serde_json::Error),
    /// Authentication-flow failures (Microsoft/XBL/XSTS/Minecraft steps).
    Auth(String),
    /// Malformed remote data (version manifests, profiles, asset indexes).
    Parse(String),
    /// Invalid user input or unsupported configuration.
    InvalidInput(String),
    /// Security-trust failures: host-key mismatch, BOM attestation failure.
    Security(String),
    /// Child-process failures (installers, the game itself).
    Process(String),
    /// A required artifact/entity could not be found.
    NotFound(String),
}

impl fmt::Display for LauncherError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LauncherError::Network(m) => write!(f, "network error: {m}"),
            LauncherError::Http { status, url } => write!(f, "HTTP {status} from {url}"),
            LauncherError::Io(e) => write!(f, "I/O error: {e}"),
            LauncherError::Json(e) => write!(f, "JSON error: {e}"),
            LauncherError::Auth(m) => write!(f, "authentication error: {m}"),
            LauncherError::Parse(m) => write!(f, "parse error: {m}"),
            LauncherError::InvalidInput(m) => write!(f, "invalid input: {m}"),
            LauncherError::Security(m) => write!(f, "security error: {m}"),
            LauncherError::Process(m) => write!(f, "process error: {m}"),
            LauncherError::NotFound(m) => write!(f, "not found: {m}"),
        }
    }
}

impl std::error::Error for LauncherError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            LauncherError::Io(e) => Some(e),
            LauncherError::Json(e) => Some(e),
            _ => None,
        }
    }
}

impl From<std::io::Error> for LauncherError {
    fn from(e: std::io::Error) -> Self {
        LauncherError::Io(e)
    }
}

impl From<serde_json::Error> for LauncherError {
    fn from(e: serde_json::Error) -> Self {
        LauncherError::Json(e)
    }
}

impl From<reqwest::Error> for LauncherError {
    fn from(e: reqwest::Error) -> Self {
        if e.is_timeout() || e.is_connect() {
            LauncherError::Network(e.to_string())
        } else if let Some(status) = e.status() {
            LauncherError::Http {
                status: status.as_u16(),
                url: e.url().map(|u| u.to_string()).unwrap_or_default(),
            }
        } else {
            LauncherError::Network(e.to_string())
        }
    }
}
