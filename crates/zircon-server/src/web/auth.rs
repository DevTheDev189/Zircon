//! JWT auth middleware for the admin API.
//!
//! Port of the `before("/api/*")` filter in `JavalinApp`: every `/api/*` route
//! requires a valid `Authorization: Bearer <jwt>` header, except the public
//! endpoints the launcher needs without an admin token.
//!
//! The middleware also rejects revoked tokens (see `crate::auth::sessions`) so
//! a signed-out or password-changed session dies immediately, and tokens whose
//! account no longer exists (renamed/deleted users).
//!
//! Browser WebSockets cannot set HTTP headers, so the console stream
//! authenticates with the token as its first message (see
//! `crate::web::controllers::console_controller`).

use axum::extract::Request;
use axum::extract::{FromRequestParts, State};
use axum::http::header::AUTHORIZATION;
use axum::http::request::Parts;
use axum::middleware::Next;
use axum::response::Response;

use super::app::ApiError;
use crate::auth::jwt;
use crate::web::app::AppState;

/// Validated admin identity, inserted into request extensions by `require_auth`
/// and extractable by handlers via `CurrentUser`.
#[derive(Debug, Clone)]
pub struct CurrentUser {
    pub username: String,
    /// Token ID (`jti` claim) — used to revoke exactly this session.
    pub jti: String,
    /// Token expiry (unix seconds) — bounds how long a revocation must stick.
    pub exp: i64,
}

#[axum::async_trait]
impl<S> FromRequestParts<S> for CurrentUser {
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        parts
            .extensions
            .get::<CurrentUser>()
            .cloned()
            .ok_or_else(|| {
                ApiError::Unauthorized("Authentication required. Please log in.".to_string())
            })
    }
}

/// Token from the `Authorization: Bearer` header.
fn bearer_token(request: &Request) -> Option<String> {
    request
        .headers()
        .get(AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .map(str::trim)
        .filter(|t| !t.is_empty())
        .map(str::to_string)
}

/// Axum middleware enforcing the bearer token on protected routes.
pub async fn require_auth(
    State(state): State<AppState>,
    request: Request,
    next: Next,
) -> Result<Response, ApiError> {
    let Some(token) = bearer_token(&request) else {
        return Err(ApiError::Unauthorized(
            "Authentication required. Please log in.".to_string(),
        ));
    };
    let Some(claims) = jwt::decode_claims(&token) else {
        return Err(ApiError::Unauthorized(
            "Authentication required. Please log in.".to_string(),
        ));
    };
    if state.sessions.is_revoked(&claims.jti) {
        return Err(ApiError::Unauthorized(
            "Session has been terminated. Please log in again.".to_string(),
        ));
    }
    if state.auth.get_user(&claims.sub).is_none() {
        // The account was renamed or deleted since the token was issued.
        return Err(ApiError::Unauthorized(
            "Account no longer exists. Please log in again.".to_string(),
        ));
    }
    let mut request = request;
    request.extensions_mut().insert(CurrentUser {
        username: claims.sub,
        jti: claims.jti,
        exp: claims.exp,
    });
    Ok(next.run(request).await)
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;

    fn request_with(uri: &str, header: Option<(&str, &str)>) -> Request {
        let mut builder = Request::builder().uri(uri);
        if let Some((name, value)) = header {
            builder = builder.header(name, value);
        }
        builder.body(Body::empty()).unwrap()
    }

    #[test]
    fn bearer_header_is_required() {
        let req = request_with("/api/console?token=query-token", None);
        // Query-string tokens are no longer accepted anywhere — the console
        // WebSocket authenticates via its first message instead, so bearer
        // tokens never appear in URLs (which get logged).
        assert_eq!(None, bearer_token(&req));

        let req = request_with(
            "/api/console",
            Some(("Authorization", "Bearer header-token")),
        );
        assert_eq!(Some("header-token".to_string()), bearer_token(&req));
    }

    #[test]
    fn no_credentials_yields_none() {
        let req = request_with("/api/console", None);
        assert_eq!(None, bearer_token(&req));
    }
}
