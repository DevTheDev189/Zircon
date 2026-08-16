//! JWT auth middleware for the admin API.
//!
//! Port of the `before("/api/*")` filter in `JavalinApp`: every `/api/*` route
//! requires a valid `Authorization: Bearer <jwt>` header, except the public
//! endpoints the launcher needs without an admin token.
//!
//! Browser WebSockets cannot set HTTP headers, so the console stream also
//! accepts the token as a `?token=` query parameter.

use axum::extract::Request;
use axum::http::header::AUTHORIZATION;
use axum::middleware::Next;
use axum::response::Response;

use super::app::ApiError;
use crate::auth::jwt;

/// Extracted current admin username, inserted into request extensions.
#[derive(Debug, Clone)]
pub struct CurrentUser(pub String);

/// Token from the `Authorization: Bearer` header, falling back to the
/// `?token=` query parameter (browser WebSockets cannot set headers).
fn bearer_token(request: &Request) -> Option<String> {
    if let Some(token) = request
        .headers()
        .get(AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
    {
        let token = token.trim();
        if !token.is_empty() {
            return Some(token.to_string());
        }
    }
    request
        .uri()
        .query()
        .and_then(|q| {
            url::form_urlencoded::parse(q.as_bytes())
                .find(|(k, _)| k == "token")
                .map(|(_, v)| v.into_owned())
        })
        .filter(|t| !t.is_empty())
}

/// Axum middleware enforcing the bearer token on protected routes.
pub async fn require_auth(request: Request, next: Next) -> Result<Response, ApiError> {
    match bearer_token(&request)
        .as_deref()
        .and_then(jwt::validate_token)
    {
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
    bearer_token(request)
        .as_deref()
        .and_then(jwt::validate_token)
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
    fn bearer_header_is_preferred() {
        let req = request_with(
            "/api/console?token=query-token",
            Some(("Authorization", "Bearer header-token")),
        );
        assert_eq!(Some("header-token".to_string()), bearer_token(&req));
    }

    #[test]
    fn query_token_fallback_for_websockets() {
        let req = request_with("/api/console?token=abc123", None);
        assert_eq!(Some("abc123".to_string()), bearer_token(&req));
    }

    #[test]
    fn no_credentials_yields_none() {
        let req = request_with("/api/console", None);
        assert_eq!(None, bearer_token(&req));
    }
}
