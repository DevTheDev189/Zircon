//! Server-side session registry so "Sign out" (and password changes) actually
//! kill a session instead of leaving the token valid until it expires.
//!
//! Every issued JWT is registered here (keyed by its `jti` claim) with the
//! owning username and the token's absolute expiry:
//!
//! * **Sign-out** marks one `jti` as revoked — that token is dead immediately.
//! * **Password change** revokes *every* outstanding session for the user at
//!   once (no per-token enumeration needed), then a fresh token is minted.
//! * Expired entries are purged by the housekeeping task so memory stays
//!   bounded by roughly one entry per login within a 12h window.
//!
//! In-memory by design: this is a single-process admin daemon. A restart drops
//! the registry, which only means a pre-restart logout no longer blocks a token
//! whose 12h TTL outlived the restart — acceptable for a local admin tool.

use std::collections::HashMap;
use std::sync::Mutex;

struct Session {
    username: String,
    expires_at: i64,
    revoked: bool,
}

/// Live JWT sessions: `jti` → session metadata.
#[derive(Default)]
pub struct SessionRegistry {
    sessions: Mutex<HashMap<String, Session>>,
}

impl SessionRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Records a newly issued token. Existing entries (e.g. re-registration of
    /// a token that was already revoked) are left untouched.
    pub fn register(&self, jti: &str, username: &str, expires_at: i64) {
        if jti.is_empty() {
            return;
        }
        self.sessions
            .lock()
            .unwrap()
            .entry(jti.to_string())
            .or_insert_with(|| Session {
                username: username.to_string(),
                expires_at,
                revoked: false,
            });
    }

    /// Returns `true` when the token with `jti` was explicitly revoked and has
    /// not yet expired naturally. Unknown (never-registered) tokens — e.g.
    /// issued before the registry existed — are treated as valid, but an empty
    /// `jti` is **always** treated as revoked (fail closed): a token without a
    /// session identifier can never be tied to a revocable session, so it must
    /// not be accepted.
    pub fn is_revoked(&self, jti: &str) -> bool {
        if jti.trim().is_empty() {
            return true; // Fail closed
        }
        let sessions = self.sessions.lock().unwrap();
        match sessions.get(jti) {
            Some(session) => session.revoked && session.expires_at > now_seconds(),
            None => false,
        }
    }

    /// Revokes the session carrying `jti` (sign-out). Unknown jtis are still
    /// recorded as revoked so the lookup is consistent.
    pub fn revoke(&self, jti: &str, username: &str, expires_at: i64) {
        if jti.is_empty() {
            return;
        }
        let mut sessions = self.sessions.lock().unwrap();
        let session = sessions.entry(jti.to_string()).or_insert_with(|| Session {
            username: username.to_string(),
            expires_at,
            revoked: false,
        });
        session.revoked = true;
    }

    /// Revokes every outstanding session for `username`. Returns the number
    /// revoked. Used on password change so stolen tokens die immediately.
    pub fn revoke_user(&self, username: &str) -> usize {
        let mut sessions = self.sessions.lock().unwrap();
        let now = now_seconds();
        let mut count = 0;
        for session in sessions.values_mut() {
            if session.username == username && session.expires_at > now && !session.revoked {
                session.revoked = true;
                count += 1;
            }
        }
        count
    }

    /// Drops entries whose token has expired naturally. Called periodically by
    /// the housekeeping task.
    pub fn purge_expired(&self) {
        let now = now_seconds();
        self.sessions
            .lock()
            .unwrap()
            .retain(|_, s| s.expires_at > now);
    }

    /// Number of currently tracked sessions (tests / diagnostics).
    pub fn len(&self) -> usize {
        self.sessions.lock().unwrap().len()
    }
}

fn now_seconds() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64
}

#[cfg(test)]
mod tests {
    use super::*;

    fn future() -> i64 {
        now_seconds() + 1000
    }

    #[test]
    fn sign_out_revokes_only_that_session() {
        let registry = SessionRegistry::new();
        registry.register("aaa", "admin", future());
        registry.register("bbb", "admin", future());

        registry.revoke("aaa", "admin", future());
        assert!(registry.is_revoked("aaa"));
        assert!(!registry.is_revoked("bbb"));
    }

    #[test]
    fn empty_jti_is_revoked_but_unknown_jtis_are_not() {
        let registry = SessionRegistry::new();
        registry.revoke("", "admin", future());
        // Fail closed: a token with no session identifier is never valid.
        assert!(registry.is_revoked(""));
        assert!(registry.is_revoked("   "));
        // A never-issued (but well-formed) jti is not revoked.
        assert!(!registry.is_revoked("never-issued"));
    }

    #[test]
    fn password_change_revokes_all_user_sessions() {
        let registry = SessionRegistry::new();
        registry.register("a1", "admin", future());
        registry.register("a2", "admin", future());
        registry.register("s1", "steve", future());

        let count = registry.revoke_user("admin");
        assert_eq!(2, count);
        assert!(registry.is_revoked("a1"));
        assert!(registry.is_revoked("a2"));
        // Other users are untouched.
        assert!(!registry.is_revoked("s1"));

        // Revoking again finds nothing left.
        assert_eq!(0, registry.revoke_user("admin"));
    }

    #[test]
    fn expired_entries_are_purged_and_do_not_block() {
        let registry = SessionRegistry::new();
        registry.register("stale", "admin", now_seconds() - 1);
        registry.revoke("stale", "admin", now_seconds() - 1);
        assert!(!registry.is_revoked("stale"));
        registry.purge_expired();
        assert_eq!(0, registry.len());
    }

    #[test]
    fn re_registering_a_revoked_token_keeps_it_revoked() {
        let registry = SessionRegistry::new();
        registry.revoke("aaa", "admin", future());
        registry.register("aaa", "admin", future());
        assert!(registry.is_revoked("aaa"));
    }
}
