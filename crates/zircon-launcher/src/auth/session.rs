//! The persisted Microsoft authentication session: the Minecraft access token,
//! the Microsoft refresh token (for silent renewal) and the Minecraft profile
//! identity.
//!
//! Port of `com.mcmanager.client.auth.SessionData`.

use serde::{Deserialize, Deserializer, Serialize};
use zeroize::Zeroize;

/// The current wall-clock time in milliseconds since the Unix epoch.
pub(crate) fn now_millis() -> i64 {
    chrono::Utc::now().timestamp_millis()
}

/// A Minecraft/Microsoft authentication session.
///
/// Mirrors the Gson-serializable Java POJO: fields serialize to camelCase
/// (`accessToken`, `refreshToken`, `username`, `uuid`, `expiresAtMillis`,
/// `userType`). `userType` is always `"msa"` for sessions produced by
/// Microsoft auth.
///
/// The access and refresh tokens are secrets: they are zeroized in memory on
/// drop (see [`Zeroize`]) so a dumped process heap or a stale borrow cannot
/// recover them.
#[derive(Clone, Serialize, Deserialize, PartialEq, Zeroize)]
#[zeroize(drop)]
#[serde(rename_all = "camelCase")]
pub struct SessionData {
    pub access_token: String,
    pub refresh_token: String,
    #[zeroize(skip)]
    pub username: String,
    #[zeroize(skip)]
    pub uuid: String,
    #[zeroize(skip)]
    pub expires_at_millis: i64,
    /// Always `"msa"` — sessions are only ever produced by Microsoft auth.
    /// Deserializes to `""` when missing/null in JSON (like Gson's `null`),
    /// which [`SessionData::is_valid`] treats as `"msa"`.
    #[serde(default, deserialize_with = "de_nullable_string")]
    #[zeroize(skip)]
    pub user_type: String,
}

impl std::fmt::Debug for SessionData {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SessionData")
            .field("access_token", &"[REDACTED]")
            .field("refresh_token", &"[REDACTED]")
            .field("username", &self.username)
            .field("uuid", &self.uuid)
            .field("expires_at_millis", &self.expires_at_millis)
            .field("user_type", &self.user_type)
            .finish()
    }
}

/// Deserializes a string field that may be missing or explicitly `null`
/// (Gson writes `null` for absent Java fields; serde would otherwise reject it).
fn de_nullable_string<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: Deserializer<'de>,
{
    Ok(Option::<String>::deserialize(deserializer)?.unwrap_or_default())
}

impl Default for SessionData {
    fn default() -> Self {
        SessionData {
            access_token: String::new(),
            refresh_token: String::new(),
            username: String::new(),
            uuid: String::new(),
            expires_at_millis: 0,
            user_type: "msa".to_string(),
        }
    }
}

impl SessionData {
    /// Tokens are considered expired this many ms before they actually expire.
    const GRACE_MILLIS: i64 = 60_000;

    /// Returns `true` if the access token is expired (or about to expire within
    /// the grace period), `false` if it is still valid.
    pub fn is_expired(&self) -> bool {
        if self.expires_at_millis <= 0 {
            return true;
        }
        now_millis() > self.expires_at_millis.saturating_sub(Self::GRACE_MILLIS)
    }

    /// A usable session must have come from Microsoft auth: a real access token
    /// and `userType == "msa"`. Rejects hand-crafted caches (dummy tokens,
    /// legacy sessions) so the launcher can never launch without signing in.
    pub fn is_valid(&self) -> bool {
        let username = self.username.trim();
        if username.is_empty()
            || username.len() > 32
            || username.starts_with('-')
            || !username
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
        {
            return false;
        }
        let uuid = self.uuid.trim();
        if uuid.is_empty()
            || uuid.len() > 64
            || uuid.starts_with('-')
            || !uuid.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
        {
            return false;
        }
        let token = self.access_token.trim();
        if token.is_empty()
            || self.access_token == "0"
            || token.starts_with('-')
            || token.chars().any(|c| c.is_ascii_whitespace() || c.is_ascii_control())
        {
            return false;
        }
        let user_type = if self.user_type.is_empty() {
            "msa"
        } else {
            self.user_type.as_str()
        };
        user_type == "msa"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn session() -> SessionData {
        SessionData {
            access_token: "eyJhbGciOiJIUzI1NiJ9.test".to_string(),
            refresh_token: "M.R3_BAY.abc".to_string(),
            username: "Steve".to_string(),
            uuid: "0f7d1a1e-8d5a-4f0a-8b9c-2a3b4c5d6e7f".to_string(),
            expires_at_millis: now_millis() + 86_400_000,
            user_type: "msa".to_string(),
        }
    }

    #[test]
    fn default_is_msa() {
        assert_eq!("msa", SessionData::default().user_type);
    }

    #[test]
    fn is_expired_uses_grace_period() {
        let now = now_millis();
        let mut s = session();
        // More than the 60s grace left → still valid.
        s.expires_at_millis = now + 61_000;
        assert!(!s.is_expired());
        // Within the 60s grace → considered expired.
        s.expires_at_millis = now + 59_000;
        assert!(s.is_expired());
        // Already past expiry → expired.
        s.expires_at_millis = now - 60_000;
        assert!(s.is_expired());
    }

    #[test]
    fn is_valid_accepts_real_session() {
        assert!(session().is_valid());
    }

    #[test]
    fn is_valid_rejects_blank_or_dummy_tokens() {
        let mut s = session();
        s.access_token = String::new();
        assert!(!s.is_valid());
        s.access_token = "   ".to_string();
        assert!(!s.is_valid());
        s.access_token = "0".to_string();
        assert!(!s.is_valid());
        s.access_token = "real-token".to_string();
        assert!(s.is_valid());
    }

    #[test]
    fn is_valid_rejects_blank_username() {
        let mut s = session();
        s.username = "  ".to_string();
        assert!(!s.is_valid());
    }

    #[test]
    fn is_valid_defaults_missing_user_type_to_msa() {
        let mut s = session();
        s.user_type = "mojang".to_string();
        assert!(!s.is_valid());
        s.user_type = String::new();
        assert!(s.is_valid(), "empty userType defaults to msa");
    }

    #[test]
    fn serde_round_trip_uses_camel_case() {
        let s = session();
        let json = serde_json::to_string(&s).unwrap();
        assert!(json.contains("\"accessToken\""));
        assert!(json.contains("\"userType\":\"msa\""));
        let back: SessionData = serde_json::from_str(&json).unwrap();
        assert_eq!(s, back);
    }

    #[test]
    fn null_user_type_deserializes_like_gson() {
        // Gson maps `"userType": null` to a null field, which is_valid then
        // defaults to "msa"; serde maps it to "" with the same outcome.
        let json = r#"{"accessToken":"t","refreshToken":"r","username":"u","uuid":"id","expiresAtMillis":0,"userType":null}"#;
        let s: SessionData = serde_json::from_str(json).unwrap();
        assert_eq!("", s.user_type);
        assert!(s.is_valid());
    }

    #[test]
    fn zeroize_clears_tokens_but_keeps_identity() {
        use zeroize::Zeroize;
        let mut s = session();
        s.zeroize();
        // Secrets are wiped.
        assert!(s.access_token.is_empty());
        assert!(s.refresh_token.is_empty());
        // Non-secret identity fields are skipped.
        assert_eq!("Steve", s.username);
        assert_eq!("msa", s.user_type);
    }

    #[test]
    fn is_valid_rejects_flag_injection_and_unsafe_identity() {
        let mut s = session();
        s.username = "-Devil=true".to_string();
        assert!(!s.is_valid());
        s.username = "user name".to_string();
        assert!(!s.is_valid());
        s.username = "Valid_User-123".to_string();
        assert!(s.is_valid());

        s.uuid = "../traversal".to_string();
        assert!(!s.is_valid());
        s.uuid = "-flag".to_string();
        assert!(!s.is_valid());
        s.uuid = "0f7d1a1e-8d5a-4f0a-8b9c-2a3b4c5d6e7f".to_string();
        assert!(s.is_valid());

        s.access_token = "-flag".to_string();
        assert!(!s.is_valid());
        s.access_token = "token with spaces".to_string();
        assert!(!s.is_valid());
    }

    #[test]
    fn is_expired_handles_underflow() {
        let mut s = session();
        s.expires_at_millis = 0;
        assert!(s.is_expired());
        s.expires_at_millis = -1;
        assert!(s.is_expired());
        s.expires_at_millis = i64::MIN;
        assert!(s.is_expired());
    }

    #[test]
    fn debug_formatting_redacts_tokens() {
        let s = session();
        let debug_str = format!("{s:?}");
        assert!(!debug_str.contains("eyJhbGciOiJIUzI1NiJ9.test"));
        assert!(!debug_str.contains("M.R3_BAY.abc"));
        assert!(debug_str.contains("[REDACTED]"));
        assert!(debug_str.contains("Steve"));
    }
}
