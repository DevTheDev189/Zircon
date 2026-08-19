//! Admin auth endpoints with TOTP 2FA, HttpOnly session cookies, per-user rate
//! limiting and audit logging.
//!
//! Port of the auth routes in `com.mcmanager.server.web.JavalinApp`.

use std::net::SocketAddr;

use axum::extract::{ConnectInfo, State};
use axum::Json;
use axum_extra::extract::cookie::{Cookie, CookieJar, SameSite};
use serde::Deserialize;
use totp_rs::{Algorithm, Secret, TOTP};

use crate::web::app::{issue_token, ApiError, AppState};
use crate::web::auth::CurrentUser;

#[derive(Debug, Deserialize)]
pub struct LoginRequest {
    pub username: String,
    pub password: String,
    #[serde(default)]
    pub totp_code: Option<String>,
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

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TotpEnableRequest {
    pub code: String,
}

/// Rate-limit key. Buckets are per `IP + username` so a brute-forcer spraying
/// failed attempts against `admin` from one address can only lock out that
/// address — never the administrator's own IP. Unknown-username probes still
/// get an IP-scoped bucket.
fn limiter_key(username: &str, client: &Option<ConnectInfo<SocketAddr>>) -> String {
    let ip = client
        .as_ref()
        .map(|c| c.0.ip().to_string())
        .unwrap_or_else(|| "127.0.0.1".to_string());
    let u = username.trim().to_lowercase();
    if !u.is_empty() {
        format!("ip:{ip}:user:{u}")
    } else {
        format!("ip:{ip}:user:unknown")
    }
}

fn rate_limited(state: &AppState, key: &str) -> Result<(), ApiError> {
    match state.login_limiter.check(key) {
        Ok(()) => Ok(()),
        Err(retry_after) => Err(ApiError::TooManyRequests(format!(
            "Too many login attempts. Retry in {retry_after}s."
        ))),
    }
}

/// Builds the HttpOnly, SameSite=Strict, Secure session cookie carrying `token`.
fn session_cookie(token: String) -> Cookie<'static> {
    let mut cookie = Cookie::new("zircon_session", token);
    cookie.set_http_only(true);
    cookie.set_same_site(SameSite::Strict);
    cookie.set_secure(true);
    cookie.set_path("/");
    cookie
}

/// POST /api/auth/login
pub async fn login(
    State(state): State<AppState>,
    jar: CookieJar,
    client: Option<ConnectInfo<SocketAddr>>,
    Json(body): Json<LoginRequest>,
) -> Result<(CookieJar, Json<serde_json::Value>), ApiError> {
    let key = limiter_key(&body.username, &client);
    rate_limited(&state, &key)?;

    if !state.auth.authenticate(&body.username, &body.password) {
        state
            .audit
            .log(&body.username, "LOGIN_FAILED", "Invalid credentials");
        return Err(ApiError::Unauthorized(
            "Invalid username or password".to_string(),
        ));
    }

    let user = state
        .auth
        .get_user(&body.username)
        .ok_or_else(|| ApiError::Unauthorized("User record not found".to_string()))?;

    // TOTP 2FA: when enabled, a valid code is mandatory even with the right
    // password.
    if user.totp_enabled {
        let code = body.totp_code.as_deref().unwrap_or("");
        if !user.verify_totp(code) {
            state
                .audit
                .log(&body.username, "LOGIN_FAILED", "Invalid TOTP code");
            return Err(ApiError::Unauthorized(
                "Invalid TOTP two-factor code".to_string(),
            ));
        }
    }

    state.login_limiter.reset(&key);
    let token = issue_token(&state, &body.username);
    state.audit.log(
        &body.username,
        "LOGIN_SUCCESS",
        "Authenticated successfully",
    );

    let updated_jar = jar.add(session_cookie(token.clone()));
    Ok((
        updated_jar,
        Json(serde_json::json!({ "token": token, "username": body.username })),
    ))
}

/// POST /api/auth/logout — revokes the presented session server-side and clears
/// the session cookie so the browser doesn't keep replaying a dead token.
pub async fn logout(
    State(state): State<AppState>,
    jar: CookieJar,
    user: CurrentUser,
) -> Result<(CookieJar, Json<serde_json::Value>), ApiError> {
    state.sessions.revoke(&user.jti, &user.username, user.exp);
    state
        .audit
        .log(&user.username, "LOGOUT", "Session terminated");
    let updated_jar = jar.remove(Cookie::from("zircon_session"));
    Ok((updated_jar, Json(serde_json::json!({ "ok": true }))))
}

/// GET /api/auth/me — current user profile (username + icon + 2FA state).
pub async fn me(
    State(state): State<AppState>,
    user: CurrentUser,
) -> Result<Json<serde_json::Value>, ApiError> {
    let profile = state
        .auth
        .get_user(&user.username)
        .ok_or_else(|| ApiError::NotFound("User not found".to_string()))?;
    Ok(Json(serde_json::json!({
        "username": profile.username,
        "icon": profile.icon,
        "totpEnabled": profile.totp_enabled
    })))
}

/// POST /api/auth/2fa/setup — generates a new TOTP secret + QR URI. The secret
/// is stored but not yet enforced until `enable` confirms a live code.
pub async fn setup_2fa(
    State(state): State<AppState>,
    user: CurrentUser,
) -> Result<Json<serde_json::Value>, ApiError> {
    let secret = Secret::generate_secret();
    let secret_encoded = secret.to_encoded().to_string();
    let totp = TOTP::new(
        Algorithm::SHA1,
        6,
        1,
        30,
        secret.to_bytes().unwrap(),
        Some("Zircon Server".to_string()),
        user.username.clone(),
    )
    .map_err(|e| ApiError::Internal(e.to_string()))?;

    let url = totp.get_url();
    state
        .auth
        .set_totp(&user.username, Some(secret_encoded.clone()), false)
        .map_err(ApiError::Internal)?;

    Ok(Json(serde_json::json!({
        "secret": secret_encoded,
        "qrUrl": url
    })))
}

/// POST /api/auth/2fa/enable — verifies a code against the pending secret and
/// only then activates 2FA for the account.
pub async fn enable_2fa(
    State(state): State<AppState>,
    user: CurrentUser,
    Json(body): Json<TotpEnableRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let profile = state
        .auth
        .get_user(&user.username)
        .ok_or_else(|| ApiError::NotFound("User not found".to_string()))?;

    let Some(secret) = &profile.totp_secret else {
        return Err(ApiError::BadRequest(
            "2FA not initialized. Run setup first.".to_string(),
        ));
    };

    let Ok(secret_bytes) = Secret::Encoded(secret.clone()).to_bytes() else {
        return Err(ApiError::Internal("Invalid secret format".to_string()));
    };

    let Ok(totp) = TOTP::new(
        Algorithm::SHA1,
        6,
        1,
        30,
        secret_bytes,
        Some("Zircon Server".to_string()),
        user.username.clone(),
    ) else {
        return Err(ApiError::Internal("Failed to build TOTP".to_string()));
    };

    if !totp.check_current(&body.code).unwrap_or(false) {
        return Err(ApiError::Unauthorized(
            "Invalid confirmation code".to_string(),
        ));
    }

    state
        .auth
        .set_totp(&user.username, Some(secret.clone()), true)
        .map_err(ApiError::Internal)?;
    state.audit.log(
        &user.username,
        "2FA_ENABLED",
        "Two-factor authentication activated",
    );

    Ok(Json(serde_json::json!({ "ok": true, "totpEnabled": true })))
}

/// POST /api/auth/2fa/disable
pub async fn disable_2fa(
    State(state): State<AppState>,
    user: CurrentUser,
) -> Result<Json<serde_json::Value>, ApiError> {
    state
        .auth
        .set_totp(&user.username, None, false)
        .map_err(ApiError::Internal)?;
    state.audit.log(
        &user.username,
        "2FA_DISABLED",
        "Two-factor authentication disabled",
    );
    Ok(Json(
        serde_json::json!({ "ok": true, "totpEnabled": false }),
    ))
}

/// POST /api/auth/change-password — a successful change revokes every existing
/// session (including the caller's) and mints a fresh token + cookie.
pub async fn change_password(
    State(state): State<AppState>,
    jar: CookieJar,
    client: Option<ConnectInfo<SocketAddr>>,
    Json(body): Json<ChangePasswordRequest>,
) -> Result<(CookieJar, Json<serde_json::Value>), ApiError> {
    if body.username.is_empty() || body.current_password.is_empty() || body.new_password.is_empty()
    {
        return Err(ApiError::BadRequest("All fields are required".to_string()));
    }
    let key = limiter_key(&body.username, &client);
    rate_limited(&state, &key)?;

    match state
        .auth
        .change_password(&body.username, &body.current_password, &body.new_password)
    {
        Ok(true) => {
            state.login_limiter.reset(&key);
            state.sessions.revoke_user(&body.username);
            state.audit.log(
                &body.username,
                "PASSWORD_CHANGED",
                "Password successfully changed; sessions revoked",
            );
            let token = issue_token(&state, &body.username);
            Ok((
                jar.add(session_cookie(token.clone())),
                Json(serde_json::json!({ "ok": true, "token": token })),
            ))
        }
        Ok(false) => Err(ApiError::Unauthorized(
            "Invalid current password".to_string(),
        )),
        Err(e) => Err(ApiError::BadRequest(e)),
    }
}

/// POST /api/auth/profile — atomic profile update (rename / change password /
/// change icon). Changing the password revokes every session and mints a fresh
/// token (returned so the UI keeps working).
pub async fn profile(
    State(state): State<AppState>,
    jar: CookieJar,
    Json(body): Json<ProfileUpdateRequest>,
) -> Result<(CookieJar, Json<serde_json::Value>), ApiError> {
    if body.current_username.is_empty() || body.current_password.is_empty() {
        return Err(ApiError::BadRequest(
            "Current credentials required".to_string(),
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
            let mut jar = jar;
            let mut response = serde_json::json!({ "ok": true });
            let target = body
                .new_username
                .as_deref()
                .map(str::trim)
                .filter(|n| !n.is_empty())
                .unwrap_or(&body.current_username);

            state.audit.log(
                &body.current_username,
                "PROFILE_UPDATED",
                &format!("Target: {target}"),
            );

            if body.new_password.as_deref().is_some_and(|p| !p.is_empty()) {
                state.sessions.revoke_user(target);
                let token = issue_token(&state, target);
                jar = jar.add(session_cookie(token.clone()));
                response["token"] = serde_json::json!(token);
            }
            Ok((jar, Json(response)))
        }
        Ok(false) => Err(ApiError::Unauthorized("Invalid credentials".to_string())),
        Err(e) => Err(ApiError::BadRequest(e)),
    }
}
