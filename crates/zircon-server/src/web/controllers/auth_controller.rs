//! Admin auth endpoints: login, profile query, change password, profile update.
//!
//! Port of the auth routes in `com.mcmanager.server.web.JavalinApp`.

use axum::extract::{Request, State};
use axum::Json;

use serde::Deserialize;

use crate::web::app::{issue_token, ApiError, AppState};
use crate::web::auth::bearer_username;

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

/// POST /api/auth/login
pub async fn login(
    State(state): State<AppState>,
    Json(body): Json<LoginRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    if !state.auth.authenticate(&body.username, &body.password) {
        return Err(ApiError::Unauthorized(
            "Invalid username or password".to_string(),
        ));
    }
    let token = issue_token(&body.username);
    Ok(Json(
        serde_json::json!({ "token": token, "username": body.username }),
    ))
}

/// POST /api/auth/change-password
pub async fn change_password(
    State(state): State<AppState>,
    Json(body): Json<ChangePasswordRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    if body.username.is_empty() || body.current_password.is_empty() || body.new_password.is_empty()
    {
        return Err(ApiError::BadRequest(
            "username, currentPassword and newPassword are required".to_string(),
        ));
    }
    match state
        .auth
        .change_password(&body.username, &body.current_password, &body.new_password)
    {
        Ok(true) => Ok(Json(serde_json::json!({ "ok": true }))),
        Ok(false) => Err(ApiError::Unauthorized(
            "Invalid username or current password".to_string(),
        )),
        Err(e) => Err(ApiError::BadRequest(e)),
    }
}

/// GET /api/auth/me — current user profile (username + icon for the admin header).
pub async fn me(
    State(state): State<AppState>,
    request: Request,
) -> Result<Json<serde_json::Value>, ApiError> {
    let username = bearer_username(&request)
        .ok_or_else(|| ApiError::Unauthorized("Unauthorized".to_string()))?;
    let user = state
        .auth
        .get_user(&username)
        .ok_or_else(|| ApiError::NotFound("User not found".to_string()))?;
    Ok(Json(
        serde_json::json!({ "username": user.username, "icon": user.icon }),
    ))
}

/// POST /api/auth/profile — atomic profile update (rename / change password / change icon).
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
        Ok(true) => Ok(Json(serde_json::json!({ "ok": true }))),
        Ok(false) => Err(ApiError::Unauthorized("Invalid credentials".to_string())),
        Err(e) => Err(ApiError::BadRequest(e)),
    }
}
