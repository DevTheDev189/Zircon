//! Admin auth endpoints: login, logout, profile query, change password, profile
//! update.
//!
//! Port of the auth routes in `com.mcmanager.server.web.JavalinApp`.

use std::net::SocketAddr;

use axum::extract::{ConnectInfo, State};
use axum::Json;

use serde::Deserialize;

use crate::web::app::{issue_token, ApiError, AppState};
use crate::web::auth::CurrentUser;

#[derive(Debug, Deserialize)]
pub struct LoginRequest {
    pub username: String,
    pub password: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChangePasswordRequest {
    pub username: String,
    pub current_password: String,
    pub new_password: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfileUpdateRequest {
    pub current_username: String,
    pub new_username: Option<String>,
    pub current_password: String,
    pub new_password: Option<String>,
    pub icon: Option<String>,
}

/// Key for the login rate limiter. Remote clients reach the web server through
/// the loopback multiplexer, so they share the `127.0.0.1` bucket — the limiter
/// is effectively a global cap, which is the intended brute-force defense.
/// Direct connections use their real peer IP.
fn limiter_key(client: &Option<ConnectInfo<SocketAddr>>) -> String {
    client
        .as_ref()
        .map(|c| c.0.ip().to_string())
        .unwrap_or_else(|| "unknown".to_string())
}

fn rate_limited(state: &AppState, key: &str) -> Result<(), ApiError> {
    match state.login_limiter.check(key) {
        Ok(()) => Ok(()),
        Err(retry_after) => Err(ApiError::TooManyRequests(format!(
            "Too many login attempts. Retry in {retry_after}s."
        ))),
    }
}

/// POST /api/auth/login
pub async fn login(
    State(state): State<AppState>,
    client: Option<ConnectInfo<SocketAddr>>,
    Json(body): Json<LoginRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let key = limiter_key(&client);
    rate_limited(&state, &key)?;
    if !state.auth.authenticate(&body.username, &body.password) {
        return Err(ApiError::Unauthorized(
            "Invalid username or password".to_string(),
        ));
    }
    state.login_limiter.reset(&key);
    let token = issue_token(&state, &body.username);
    Ok(Json(
        serde_json::json!({ "token": token, "username": body.username }),
    ))
}

/// POST /api/auth/logout — revokes the presented token server-side so the
/// session dies immediately instead of lingering until its 12h expiry.
pub async fn logout(
    State(state): State<AppState>,
    user: CurrentUser,
) -> Result<Json<serde_json::Value>, ApiError> {
    state.sessions.revoke(&user.jti, &user.username, user.exp);
    tracing::info!("Revoked session for user {}", user.username);
    Ok(Json(serde_json::json!({ "ok": true })))
}

/// POST /api/auth/change-password — admin-only; a successful change revokes
/// every existing session and returns a fresh token for the caller.
pub async fn change_password(
    State(state): State<AppState>,
    client: Option<ConnectInfo<SocketAddr>>,
    Json(body): Json<ChangePasswordRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    if body.username.is_empty() || body.current_password.is_empty() || body.new_password.is_empty()
    {
        return Err(ApiError::BadRequest(
            "username, currentPassword and newPassword are required".to_string(),
        ));
    }
    let key = limiter_key(&client);
    rate_limited(&state, &key)?;
    match state
        .auth
        .change_password(&body.username, &body.current_password, &body.new_password)
    {
        Ok(true) => {
            state.login_limiter.reset(&key);
            // Kill every outstanding session (including this one — the caller
            // adopts the fresh token minted below). Stolen tokens die now.
            let revoked = state.sessions.revoke_user(&body.username);
            tracing::info!(
                "Password changed for {}; revoked {revoked} session(s)",
                body.username
            );
            let token = issue_token(&state, &body.username);
            Ok(Json(serde_json::json!({ "ok": true, "token": token })))
        }
        Ok(false) => Err(ApiError::Unauthorized(
            "Invalid username or current password".to_string(),
        )),
        Err(e) => Err(ApiError::BadRequest(e)),
    }
}

/// GET /api/auth/me — current user profile (username + icon for the admin header).
pub async fn me(
    State(state): State<AppState>,
    user: CurrentUser,
) -> Result<Json<serde_json::Value>, ApiError> {
    let profile = state
        .auth
        .get_user(&user.username)
        .ok_or_else(|| ApiError::NotFound("User not found".to_string()))?;
    Ok(Json(
        serde_json::json!({ "username": profile.username, "icon": profile.icon }),
    ))
}

/// POST /api/auth/profile — atomic profile update (rename / change password /
/// change icon). Changing the password revokes every session and mints a fresh
/// token (returned so the UI keeps working).
pub async fn profile(
    State(state): State<AppState>,
    Json(body): Json<ProfileUpdateRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    if body.current_username.is_empty() || body.current_password.is_empty() {
        return Err(ApiError::BadRequest(
            "currentUsername and currentPassword are required".to_string(),
        ));
    }
    match state.auth.update_profile(
        &body.current_username,
        body.new_username.as_deref(),
        &body.current_password,
        body.new_password.as_deref(),
        body.icon.as_deref(),
    ) {
        Ok(true) => {
            let mut response = serde_json::json!({ "ok": true });
            let password_changed = body.new_password.as_deref().is_some_and(|p| !p.is_empty());
            if password_changed {
                // The account may have been renamed; derive the final username
                // the same way AuthService does.
                let target = body
                    .new_username
                    .as_deref()
                    .map(str::trim)
                    .filter(|n| !n.is_empty())
                    .unwrap_or(&body.current_username);
                let revoked = state.sessions.revoke_user(target);
                tracing::info!("Password changed for {target}; revoked {revoked} session(s)");
                response["token"] = serde_json::json!(issue_token(&state, target));
            }
            Ok(Json(response))
        }
        Ok(false) => Err(ApiError::Unauthorized("Invalid credentials".to_string())),
        Err(e) => Err(ApiError::BadRequest(e)),
    }
}
