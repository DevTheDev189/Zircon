//! Multi-account Microsoft profile management and credential persistence.
//!
//! Stores user accounts in `~/.mcmanager/accounts.json` while securing OAuth
//! tokens in the OS Keyring keyed per account UUID (`auth-session-{uuid}` and
//! `auth-refresh-{uuid}`).

use std::path::PathBuf;
use keyring::Entry;
use serde::{Deserialize, Serialize};

use crate::auth::session::{now_millis, SessionData};
use crate::error::LauncherError;
use crate::paths;

pub const KEYRING_SERVICE: &str = "zircon-launcher";
pub const KEYRING_SESSION_USER: &str = "auth-session";
pub const KEYRING_REFRESH_USER: &str = "auth-refresh";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AccountProfile {
    pub uuid: String,
    pub username: String,
    pub avatar_data_url: Option<String>,
    pub last_used: i64,
    pub is_active: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct AccountsData {
    pub active_uuid: Option<String>,
    pub accounts: Vec<AccountProfile>,
}

#[derive(Debug, Clone)]
pub struct AccountManager {
    accounts_file: PathBuf,
    use_keyring: bool,
}

impl Default for AccountManager {
    fn default() -> Self {
        Self::new()
    }
}

impl AccountManager {
    pub fn new() -> Self {
        Self {
            accounts_file: paths::accounts_file(),
            use_keyring: true,
        }
    }

    pub fn new_with_file(accounts_file: PathBuf, use_keyring: bool) -> Self {
        Self {
            accounts_file,
            use_keyring,
        }
    }

    fn keyring_entry(user: &str) -> Result<Entry, LauncherError> {
        Entry::new(KEYRING_SERVICE, user).map_err(|e| LauncherError::Auth(e.to_string()))
    }

    fn account_cache_file(&self, uuid: &str) -> PathBuf {
        paths::mcmanager_dir().join(format!("auth_cache_{}.json", uuid.replace('-', "")))
    }

    /// Loads all accounts, automatically migrating any existing legacy single-session
    /// when `accounts.json` does not exist yet.
    pub fn load_accounts(&self, legacy_session: Option<&SessionData>) -> AccountsData {
        if self.accounts_file.is_file() {
            if let Ok(content) = std::fs::read_to_string(&self.accounts_file) {
                if let Ok(mut data) = serde_json::from_str::<AccountsData>(&content) {
                    if !data.accounts.is_empty() {
                        // Ensure an active account is marked if active_uuid is set
                        if let Some(ref active_uuid) = data.active_uuid {
                            for acc in &mut data.accounts {
                                acc.is_active = &acc.uuid == active_uuid;
                            }
                        } else if let Some(first) = data.accounts.first_mut() {
                            first.is_active = true;
                            data.active_uuid = Some(first.uuid.clone());
                        }
                        return data;
                    }
                }
            }
        }

        // Migrate existing legacy session into accounts.json if available
        let mut data = AccountsData::default();
        if let Some(session) = legacy_session {
            if session.is_valid() {
                let profile = AccountProfile {
                    uuid: session.uuid.clone(),
                    username: session.username.clone(),
                    avatar_data_url: None,
                    last_used: now_millis(),
                    is_active: true,
                };
                data.active_uuid = Some(session.uuid.clone());
                data.accounts.push(profile);
                let _ = self.save_accounts(&data);
                // Also mirror credentials into dedicated per-uuid slot
                let _ = self.store_session_credentials(session);
            }
        }
        data
    }

    /// Persists `accounts.json`.
    pub fn save_accounts(&self, data: &AccountsData) -> Result<(), LauncherError> {
        if let Some(parent) = self.accounts_file.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let json = serde_json::to_string_pretty(data)?;
        std::fs::write(&self.accounts_file, json.as_bytes())
            .map_err(|e| LauncherError::Io(e))?;

        Ok(())
    }

    /// Stores credentials for an account (into keyring per-UUID, plus fallback cache file,
    /// and mirrors to the legacy active slot).
    pub fn store_session_credentials(&self, session: &SessionData) -> Result<(), LauncherError> {
        let uuid = &session.uuid;
        let uuid_clean = uuid.replace('-', "");
        let session_slot = format!("auth-session-{uuid_clean}");
        let refresh_slot = format!("auth-refresh-{uuid_clean}");

        if self.use_keyring {
            let mut session_without_refresh = session.clone();
            session_without_refresh.refresh_token.clear();
            let session_json = serde_json::to_string(&session_without_refresh)?;

            if let Ok(entry) = Self::keyring_entry(&session_slot) {
                let _ = entry.set_password(&session_json);
            }
            if let Ok(entry) = Self::keyring_entry(&refresh_slot) {
                let _ = entry.set_password(&session.refresh_token);
            }

            // Mirror to active legacy slot
            if let Ok(entry) = Self::keyring_entry(KEYRING_SESSION_USER) {
                let _ = entry.set_password(&session_json);
            }
            if let Ok(entry) = Self::keyring_entry(KEYRING_REFRESH_USER) {
                let _ = entry.set_password(&session.refresh_token);
            }
        }

        // Also write file backups
        let file_slot = self.account_cache_file(uuid);
        let json = serde_json::to_string(session)?;
        let _ = std::fs::write(&file_slot, json.as_bytes());
        let _ = std::fs::write(paths::auth_cache_file(), json.as_bytes());

        Ok(())
    }


    /// Loads the stored `SessionData` for a specific UUID.
    pub fn load_session_for_uuid(&self, uuid: &str) -> Option<SessionData> {
        let uuid_clean = uuid.replace('-', "");
        let session_slot = format!("auth-session-{uuid_clean}");
        let refresh_slot = format!("auth-refresh-{uuid_clean}");

        if self.use_keyring {
            if let Ok(entry) = Self::keyring_entry(&session_slot) {
                if let Ok(json) = entry.get_password() {
                    if let Ok(mut data) = serde_json::from_str::<SessionData>(&json) {
                        if data.refresh_token.is_empty() {
                            if let Ok(ref_entry) = Self::keyring_entry(&refresh_slot) {
                                if let Ok(token) = ref_entry.get_password() {
                                    data.refresh_token = token;
                                }
                            }
                        }
                        if data.is_valid() {
                            return Some(data);
                        }
                    }
                }
            }
        }

        // Fallback to per-account file
        let file = self.account_cache_file(uuid);
        if file.is_file() {
            if let Ok(content) = std::fs::read_to_string(&file) {
                if let Ok(data) = serde_json::from_str::<SessionData>(&content) {
                    if data.is_valid() {
                        return Some(data);
                    }
                }
            }
        }

        // Fallback to default auth cache file if UUID matches
        if let Ok(content) = std::fs::read_to_string(paths::auth_cache_file()) {
            if let Ok(data) = serde_json::from_str::<SessionData>(&content) {
                if data.is_valid() && data.uuid == uuid {
                    return Some(data);
                }
            }
        }

        None
    }

    /// Registers or updates an account profile and sets it as the active account.
    pub fn register_active_account(
        &self,
        session: &SessionData,
        avatar_data_url: Option<String>,
    ) -> Result<AccountsData, LauncherError> {
        let mut data = self.load_accounts(Some(session));
        let now = now_millis();

        let mut found = false;
        for acc in &mut data.accounts {
            if acc.uuid == session.uuid {
                acc.username = session.username.clone();
                if avatar_data_url.is_some() {
                    acc.avatar_data_url = avatar_data_url.clone();
                }
                acc.last_used = now;
                acc.is_active = true;
                found = true;
            } else {
                acc.is_active = false;
            }
        }

        if !found {
            data.accounts.push(AccountProfile {
                uuid: session.uuid.clone(),
                username: session.username.clone(),
                avatar_data_url,
                last_used: now,
                is_active: true,
            });
        }

        data.active_uuid = Some(session.uuid.clone());
        self.store_session_credentials(session)?;
        self.save_accounts(&data)?;
        Ok(data)
    }

    /// Switches the active account to the specified UUID and returns its stored session.
    pub fn switch_account(&self, target_uuid: &str) -> Result<SessionData, LauncherError> {
        let mut data = self.load_accounts(None);
        let account = data
            .accounts
            .iter_mut()
            .find(|a| a.uuid == target_uuid)
            .ok_or_else(|| LauncherError::Auth(format!("Account '{target_uuid}' not found")))?;

        account.is_active = true;
        account.last_used = now_millis();
        data.active_uuid = Some(target_uuid.to_string());

        for other in &mut data.accounts {
            if other.uuid != target_uuid {
                other.is_active = false;
            }
        }

        let session = self
            .load_session_for_uuid(target_uuid)
            .ok_or_else(|| LauncherError::Auth("No stored credentials found for account".into()))?;

        // Mirror switched session into active default slot
        self.store_session_credentials(&session)?;
        self.save_accounts(&data)?;

        Ok(session)
    }

    /// Removes an account by UUID. Returns the newly active session (if any).
    pub fn remove_account(&self, target_uuid: &str) -> Result<Option<SessionData>, LauncherError> {
        let mut data = self.load_accounts(None);
        let was_active = data.active_uuid.as_deref() == Some(target_uuid);
        data.accounts.retain(|a| a.uuid != target_uuid);

        // Delete keyring entries
        let uuid_clean = target_uuid.replace('-', "");
        if self.use_keyring {
            let _ = Self::keyring_entry(&format!("auth-session-{uuid_clean}"))
                .map(|e| e.delete_credential());
            let _ = Self::keyring_entry(&format!("auth-refresh-{uuid_clean}"))
                .map(|e| e.delete_credential());
        }
        let _ = std::fs::remove_file(self.account_cache_file(target_uuid));

        let next_session = if was_active {
            if let Some(next_acc) = data.accounts.first_mut() {
                next_acc.is_active = true;
                data.active_uuid = Some(next_acc.uuid.clone());
                let session = self.load_session_for_uuid(&next_acc.uuid);
                if let Some(ref s) = session {
                    let _ = self.store_session_credentials(s);
                }
                session
            } else {
                data.active_uuid = None;
                if self.use_keyring {
                    let _ = Self::keyring_entry(KEYRING_SESSION_USER).map(|e| e.delete_credential());
                    let _ = Self::keyring_entry(KEYRING_REFRESH_USER).map(|e| e.delete_credential());
                }
                let _ = std::fs::remove_file(paths::auth_cache_file());
                None
            }
        } else {
            // Active session remains unchanged
            data.active_uuid
                .as_ref()
                .and_then(|u| self.load_session_for_uuid(u))
        };

        self.save_accounts(&data)?;
        Ok(next_session)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_session(uuid: &str, username: &str) -> SessionData {
        SessionData {
            access_token: "test-token".to_string(),
            refresh_token: "test-refresh".to_string(),
            username: username.to_string(),
            uuid: uuid.to_string(),
            expires_at_millis: now_millis() + 3600_000,
            user_type: "msa".to_string(),
        }
    }

    #[test]
    fn account_manager_lifecycle_round_trip() {
        let temp_dir = std::env::temp_dir().join(format!("accounts_test_{}", now_millis()));
        let _ = std::fs::create_dir_all(&temp_dir);
        let accounts_file = temp_dir.join("accounts.json");
        let manager = AccountManager::new_with_file(accounts_file, false);


        let s1 = test_session("1111-2222", "PlayerOne");
        let s2 = test_session("3333-4444", "PlayerTwo");

        // Register first account
        let data1 = manager.register_active_account(&s1, None).unwrap();
        assert_eq!(1, data1.accounts.len());
        assert_eq!(Some("1111-2222".to_string()), data1.active_uuid);
        assert!(data1.accounts[0].is_active);

        // Register second account
        let data2 = manager.register_active_account(&s2, None).unwrap();
        assert_eq!(2, data2.accounts.len());
        assert_eq!(Some("3333-4444".to_string()), data2.active_uuid);
        assert!(data2.accounts.iter().find(|a| a.uuid == "3333-4444").unwrap().is_active);
        assert!(!data2.accounts.iter().find(|a| a.uuid == "1111-2222").unwrap().is_active);

        // Switch back to first account
        let switched = manager.switch_account("1111-2222").unwrap();
        assert_eq!("PlayerOne", switched.username);
        let loaded = manager.load_accounts(None);
        assert_eq!(Some("1111-2222".to_string()), loaded.active_uuid);

        // Remove active account
        let next = manager.remove_account("1111-2222").unwrap();
        assert_eq!(Some("PlayerTwo".to_string()), next.map(|s| s.username.clone()));
        let loaded2 = manager.load_accounts(None);

        assert_eq!(1, loaded2.accounts.len());
        assert_eq!(Some("3333-4444".to_string()), loaded2.active_uuid);
        let _ = std::fs::remove_dir_all(temp_dir);
    }
}

