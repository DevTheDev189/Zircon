//! JWT auth middleware for the admin API.
//!
//! Port of the `before("/api/*")` filter in `JavalinApp`: every `/api/*` route
//! requires a valid `Authorization: Bearer <jwt>` header, except the public
//! endpoints the launcher needs without an admin token.

use axum::extract::Request;
use axum::http::header::AUTHORIZATION;
use axum::middleware::Next;
use axum::response::Response;

use super::app::ApiError;
use crate::auth::jwt;

/// Extracted current admin username, inserted into request extensions.
#[derive(Debug, Clone)]
pub struct CurrentUser(pub String);

/// Axum middleware enforcing the bearer token on protected routes.
pub async fn require_auth(request: Request, next: Next) -> Result<Response, ApiError> {
    let token = request
        .headers()
        .get(AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .map(|t| t.trim());

    match token.and_then(jwt::validate_token) {
        Some(username) => {
            let mut request = request;
            request.extensions_mut().insert(CurrentUser(username));
            Ok(next.run(request).await)
        }
        None => Err(ApiError::Unauthorized(
            "Authentication required. Please log in.".to_string(),
        )),
    }
}

/// Extracts the bearer token username without failing (used by the public
/// `/api/auth/me`-style handlers that re-validate).
pub fn bearer_username(request: &Request) -> Option<String> {
    request
        .headers()
        .get(AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .map(|t| t.trim())
        .and_then(jwt::validate_token)
}
