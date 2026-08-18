//! JWT auth middleware for the admin API.
//!
//! Every protected route requires a valid JWT presented either as an
//! `Authorization: Bearer <jwt>` header (launcher, curl, SPA API calls) or as
//! the `zircon_session` HttpOnly cookie (browser sessions issued at login).
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
use axum_extra::extract::cookie::CookieJar;

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

/// Token from the `Authorization: Bearer` header, falling back to the
/// `zircon_session` HttpOnly cookie (browser sessions).
fn extract_token(request: &Request) -> Option<String> {
    if let Some(auth_header) = request.headers().get(AUTHORIZATION) {
        if let Ok(val) = auth_header.to_str() {
            if let Some(t) = val.strip_prefix("Bearer ") {
                let trimmed = t.trim();
                if !trimmed.is_empty() {
                    return Some(trimmed.to_string());
                }
            }
        }
    }
    let jar = CookieJar::from_headers(request.headers());
    jar.get("zircon_session").map(|c| c.value().to_string())
}

/// Axum middleware enforcing the bearer token or session cookie on protected
/// routes.
pub async fn require_auth(
    State(state): State<AppState>,
    request: Request,
    next: Next,
) -> Result<Response, ApiError> {
    let Some(token) = extract_token(&request) else {
        return Err(ApiError::Unauthorized(
            "Authentication required.".to_string(),
        ));
    };

    let Some(claims) = jwt::decode_claims(&token) else {
        return Err(ApiError::Unauthorized(
            "Invalid or expired session.".to_string(),
        ));
    };

    if state.sessions.is_revoked(&claims.jti) {
        return Err(ApiError::Unauthorized(
            "Session has been revoked.".to_string(),
        ));
    }

    if state.auth.get_user(&claims.sub).is_none() {
        return Err(ApiError::Unauthorized(
            "Account no longer exists.".to_string(),
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
    fn bearer_header_is_preferred_over_cookie() {
        let req = request_with(
            "/api/console",
            Some(("Authorization", "Bearer header-token")),
        );
        assert_eq!(Some("header-token".to_string()), extract_token(&req));
    }

    #[test]
    fn session_cookie_is_accepted_when_no_header() {
        let req = request_with(
            "/api/console",
            Some(("Cookie", "zircon_session=cookie-token")),
        );
        assert_eq!(Some("cookie-token".to_string()), extract_token(&req));
    }

    #[test]
    fn no_credentials_yields_none() {
        let req = request_with("/api/console", None);
        assert_eq!(None, extract_token(&req));
    }

    #[test]
    fn malformed_bearer_is_rejected() {
        let req = request_with("/api/console", Some(("Authorization", "Bearer ")));
        assert_eq!(None, extract_token(&req));
    }
}
