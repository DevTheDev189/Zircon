//! External mod API clients (Modrinth & CurseForge).

pub mod curseforge;
pub mod modrinth;

use std::fmt;

/// Error raised by the provider API clients.
#[derive(Debug)]
pub enum ApiError {
    /// Transport / decoding failure.
    Http(reqwest::Error),
    /// Non-2xx HTTP response with the response body attached for diagnostics.
    Status { status: u16, body: String },
    /// The response body could not be decoded.
    Json(serde_json::Error),
}

impl fmt::Display for ApiError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ApiError::Http(e) => write!(f, "HTTP request failed: {e}"),
            ApiError::Status { status, body } => {
                write!(f, "API returned HTTP {status}: {body}")
            }
            ApiError::Json(e) => write!(f, "failed to decode API response: {e}"),
        }
    }
}

impl std::error::Error for ApiError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            ApiError::Http(e) => Some(e),
            ApiError::Json(e) => Some(e),
            _ => None,
        }
    }
}

impl From<reqwest::Error> for ApiError {
    fn from(e: reqwest::Error) -> Self {
        ApiError::Http(e)
    }
}

impl From<serde_json::Error> for ApiError {
    fn from(e: serde_json::Error) -> Self {
        ApiError::Json(e)
    }
}
