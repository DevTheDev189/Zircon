
use tauri::State;


use crate::auth::session::SessionData;

use super::*;

// ---------------------------------------------------------------------------
// Auth
// ---------------------------------------------------------------------------

/// Runs the interactive Microsoft OAuth PKCE login (opens the browser).
#[tauri::command]
pub async fn login_microsoft(state: State<'_, LauncherState>) -> Result<SessionData, String> {
    let session = state.auth.login().await.map_err(err_string)?;
    let _ = state.accounts.register_active_account(&session, None);
    *state.session.lock().await = Some(session.clone());
    Ok(session)
}

/// Lists all saved Microsoft accounts.
#[tauri::command]
pub async fn list_accounts(
    state: State<'_, LauncherState>,
) -> Result<Vec<crate::auth::accounts::AccountProfile>, String> {
    let current_session = state.session.lock().await.clone();
    let data = state.accounts.load_accounts(current_session.as_ref());
    Ok(data.accounts)
}

/// Switches active Microsoft account by UUID.
#[tauri::command]
pub async fn switch_account(
    state: State<'_, LauncherState>,
    uuid: String,
) -> Result<SessionData, String> {
    let mut session = state.accounts.switch_account(&uuid).map_err(err_string)?;
    if session.is_expired() {
        match state.auth.refresh(&session).await {
            Ok(fresh) => {
                let _ = state.accounts.store_session_credentials(&fresh);
                session = fresh;
            }
            Err(e) => {
                return Err(format!("Session expired and could not be refreshed: {e}"));
            }
        }
    }
    *state.session.lock().await = Some(session.clone());
    Ok(session)
}

/// Removes a saved Microsoft account by UUID.
#[tauri::command]
pub async fn remove_account(
    state: State<'_, LauncherState>,
    uuid: String,
) -> Result<Option<SessionData>, String> {
    let next_session = state.accounts.remove_account(&uuid).map_err(err_string)?;
    *state.session.lock().await = next_session.clone();
    Ok(next_session)
}

/// Loads the cached session, silently refreshing it when expired. Returns
/// `None` when there is no usable session (the UI must show the login overlay).
#[tauri::command]
pub async fn get_cached_session(
    state: State<'_, LauncherState>,
) -> Result<Option<SessionData>, String> {
    let accounts = state.accounts.load_accounts(None);
    let cached = if let Some(ref active_uuid) = accounts.active_uuid {
        state.accounts.load_session_for_uuid(active_uuid)
    } else {
        state.auth.load_cached()
    };

    let Some(cached) = cached else {
        *state.session.lock().await = None;
        return Ok(None);
    };

    if cached.is_expired() {
        match state.auth.refresh(&cached).await {
            Ok(fresh) => {
                let _ = state.accounts.store_session_credentials(&fresh);
                *state.session.lock().await = Some(fresh.clone());
                Ok(Some(fresh))
            }
            Err(_) => {
                // Expired with no working refresh token → force a fresh login.
                *state.session.lock().await = None;
                Ok(None)
            }
        }
    } else {
        *state.session.lock().await = Some(cached.clone());
        Ok(Some(cached))
    }
}

/// The currently signed-in session, or `None`.
#[tauri::command]
pub async fn get_session(state: State<'_, LauncherState>) -> Result<Option<SessionData>, String> {
    Ok(state.session.lock().await.clone())
}

/// Clears the active auth cache and removes the active account from accounts.json.
#[tauri::command]
pub async fn logout(state: State<'_, LauncherState>) -> Result<(), String> {
    let current_uuid = state.session.lock().await.as_ref().map(|s| s.uuid.clone());
    if let Some(uuid) = current_uuid {
        let _ = state.accounts.remove_account(&uuid);
    }
    state.auth.clear_cache().map_err(err_string)?;
    *state.session.lock().await = None;
    Ok(())
}


