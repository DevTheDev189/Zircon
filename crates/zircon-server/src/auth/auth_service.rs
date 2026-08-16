//! Admin credential store: `users.json` (username → `UserProfile`) in the data
//! dir. A profile holds the BCrypt password hash plus display metadata
//! (`icon`) so the admin UI can personalize the header.
//!
//! On first run a random 16-character admin password is generated, stored as a
//! BCrypt hash and printed to stdout — the operator copies it into the admin
//! web UI, then should change it. Passwords are never stored in plain text.
//!
//! Files written by older versions (plain `{"user": "hash"}` maps) are
//! migrated to the profile schema transparently on load.
//!
//! Port of `com.mcmanager.server.auth.AuthService`.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use serde::{Deserialize, Serialize};

const ALPHABET: &str = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789!@#$%^&*";

/// Serializable admin profile stored in `users.json`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct UserProfile {
    pub username: String,
    pub password_hash: String,
    #[serde(default = "default_icon")]
    pub icon: String,
}

fn default_icon() -> String {
    "emerald".to_string()
}

impl UserProfile {
    pub fn new(
        username: impl Into<String>,
        password_hash: impl Into<String>,
        icon: Option<String>,
    ) -> Self {
        let icon = icon
            .filter(|i| !i.trim().is_empty())
            .unwrap_or_else(default_icon);
        Self {
            username: username.into(),
            password_hash: password_hash.into(),
            icon,
        }
    }
}

/// Admin credential store backed by `users.json`.
pub struct AuthService {
    users_file: PathBuf,
    users: Mutex<BTreeMap<String, UserProfile>>,
}

impl AuthService {
    /// Ensures `users.json` exists, creating the initial `admin` account with a
    /// random password (printed to stdout) when it does not.
    pub fn initialize(data_dir: &Path) -> std::io::Result<Self> {
        fs::create_dir_all(data_dir)?;
        let users_file = data_dir.join("users.json");
        let users = if users_file.exists() {
            load(&users_file)?
        } else {
            let initial_password = generate_random_password(16);
            let hashed_password = bcrypt::hash(&initial_password, 12)
                .map_err(|e| std::io::Error::other(e.to_string()))?;
            let mut fresh = BTreeMap::new();
            fresh.insert(
                "admin".to_string(),
                UserProfile::new("admin", hashed_password, None),
            );
            let service = Self {
                users_file: users_file.clone(),
                users: Mutex::new(fresh),
            };
            service.save()?;
            println!("=================================================");
            println!("  ZIRCON SERVER CREATED INITIAL ADMIN USER");
            println!("  Username: admin");
            println!("  Password: {initial_password}");
            println!("  Please log in and change your password!");
            println!("=================================================");
            tracing::info!("Created initial admin user; password printed to stdout");
            return Ok(service);
        };
        Ok(Self {
            users_file,
            users: Mutex::new(users),
        })
    }

    /// Verifies a username/password pair against the stored BCrypt hashes.
    pub fn authenticate(&self, username: &str, password: &str) -> bool {
        let users = self.users.lock().unwrap();
        match users.get(username) {
            Some(profile) => {
                !password.is_empty()
                    && bcrypt::verify(password, &profile.password_hash).unwrap_or(false)
            }
            None => false,
        }
    }

    /// Returns the stored profile for a username, or `None` if unknown.
    pub fn get_user(&self, username: &str) -> Option<UserProfile> {
        self.users.lock().unwrap().get(username).cloned()
    }

    /// Atomically updates a profile: optionally renames the account, changes
    /// the password and/or updates the display icon. The current password is
    /// always required as proof of identity.
    ///
    /// Returns `Ok(true)` on success, `Ok(false)` if credentials were wrong.
    pub fn update_profile(
        &self,
        current_username: &str,
        new_username: Option<&str>,
        current_password: &str,
        new_password: Option<&str>,
        new_icon: Option<&str>,
    ) -> Result<bool, String> {
        if !self.authenticate(current_username, current_password) {
            return Ok(false);
        }
        let mut users = self.users.lock().unwrap();
        let Some(profile) = users.get(current_username).cloned() else {
            return Ok(false);
        };

        let target_user = match new_username {
            Some(name) if !name.trim().is_empty() => name.trim().to_string(),
            _ => current_username.to_string(),
        };

        if !target_user.eq_ignore_ascii_case(current_username) && users.contains_key(&target_user) {
            return Err(format!("Username '{target_user}' is already taken"));
        }

        let mut profile = profile;
        if let Some(password) = new_password {
            if !password.is_empty() {
                if password.len() < 8 {
                    return Err("New password must be at least 8 characters".to_string());
                }
                profile.password_hash = bcrypt::hash(password, 12)
                    .map_err(|e| format!("Could not hash password: {e}"))?;
            }
        }

        if let Some(icon) = new_icon {
            if !icon.trim().is_empty() {
                profile.icon = icon.trim().to_string();
            }
        }

        if !target_user.eq_ignore_ascii_case(current_username) {
            users.remove(current_username);
            profile.username = target_user.clone();
            users.insert(target_user.clone(), profile);
        } else {
            users.insert(target_user.clone(), profile);
        }
        drop(users);
        self.save().map_err(|e| e.to_string())?;
        tracing::info!("Profile updated for user {target_user}");
        Ok(true)
    }

    /// Changes a password after verifying the current one.
    pub fn change_password(
        &self,
        username: &str,
        current_password: &str,
        new_password: &str,
    ) -> Result<bool, String> {
        self.update_profile(username, None, current_password, Some(new_password), None)
    }

    /// Sets a password without verification (used by tests / initial setup).
    pub fn set_password(&self, username: &str, new_password: &str) -> std::io::Result<()> {
        let mut users = self.users.lock().unwrap();
        let profile = users
            .entry(username.to_string())
            .or_insert_with(|| UserProfile::new(username, "", None));
        profile.password_hash =
            bcrypt::hash(new_password, 12).map_err(|e| std::io::Error::other(e.to_string()))?;
        drop(users);
        self.save()
    }

    fn save(&self) -> std::io::Result<()> {
        let users = self.users.lock().unwrap();
        let json = serde_json::to_string_pretty(&*users)
            .map_err(|e| std::io::Error::other(e.to_string()))?;
        fs::write(&self.users_file, json)
    }
}

/// Loads `users.json`, migrating the legacy `{"user": "<bcrypt hash>"}` schema
/// to the profile schema when needed.
fn load(file: &Path) -> std::io::Result<BTreeMap<String, UserProfile>> {
    let content = fs::read_to_string(file)?;

    // Preferred schema: {"user": {"username":..., "passwordHash":..., "icon":...}}.
    if let Ok(parsed) = serde_json::from_str::<BTreeMap<String, UserProfile>>(&content) {
        if !parsed.is_empty() {
            return Ok(parsed);
        }
    }
    // Legacy schema: {"user": "<bcrypt hash>"} — migrate in place.
    if let Ok(legacy) = serde_json::from_str::<BTreeMap<String, String>>(&content) {
        let migrated: BTreeMap<String, UserProfile> = legacy
            .into_iter()
            .filter(|(_, hash)| !hash.trim().is_empty())
            .map(|(name, hash)| (name.clone(), UserProfile::new(name, hash, None)))
            .collect();
        if !migrated.is_empty() {
            tracing::info!(
                "Migrated {} legacy user(s) to the profile schema",
                migrated.len()
            );
            return Ok(migrated);
        }
    }
    Ok(BTreeMap::new())
}

fn generate_random_password(length: usize) -> String {
    use rand_like::Rng;
    // bcrypt's rand is not exposed; use a simple OS entropy source.
    let mut bytes = [0u8; 32];
    getrandom(&mut bytes);
    let mut rng = rand_like::ChaCha8Rng::from_seed(bytes);
    (0..length)
        .map(|_| {
            ALPHABET
                .chars()
                .nth(rng.random_range(ALPHABET.len()))
                .unwrap()
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Minimal internal RNG to avoid an extra dependency: ChaCha8 from `rand_core`
// is not available, so implement a tiny splitmix64-based PRNG seeded from the
// OS. This is only used to generate the one-time initial admin password.
// ---------------------------------------------------------------------------

mod rand_like {
    pub trait Rng {
        fn random_range(&mut self, range: usize) -> usize;
    }

    pub struct ChaCha8Rng {
        state: u64,
    }

    impl ChaCha8Rng {
        pub fn from_seed(seed: [u8; 32]) -> Self {
            // Fold the 32 seed bytes into a u64 (splitmix64 style mixing).
            let mut h: u64 = 0;
            for (i, byte) in seed.iter().enumerate() {
                h ^= u64::from(*byte) << ((i % 8) * 8);
            }
            Self { state: h }
        }
    }

    impl Rng for ChaCha8Rng {
        fn random_range(&mut self, range: usize) -> usize {
            // splitmix64 next
            self.state = self.state.wrapping_add(0x9E37_79B9_7F4A_7C15);
            let mut z = self.state;
            z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
            z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
            z = z ^ (z >> 31);
            (z as usize) % range
        }
    }
}

fn getrandom(bytes: &mut [u8]) {
    getrandom::fill(bytes).expect("failed to read OS entropy");
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir() -> PathBuf {
        crate::test_util::temp_dir("auth")
    }

    #[test]
    fn initial_admin_is_created_and_authenticates() {
        let dir = temp_dir();
        let service = AuthService::initialize(&dir).unwrap();
        let admin = service.get_user("admin").expect("admin user");
        assert_eq!("emerald", admin.icon);
        // We cannot know the random password; verify via set_password round trip.
        service
            .set_password("admin", "correct horse battery staple")
            .unwrap();
        assert!(service.authenticate("admin", "correct horse battery staple"));
        assert!(!service.authenticate("admin", "wrong"));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn profile_update_renames_and_changes_password() {
        let dir = temp_dir();
        let service = AuthService::initialize(&dir).unwrap();
        service.set_password("admin", "oldpass123").unwrap();

        let ok = service
            .update_profile(
                "admin",
                Some("root"),
                "oldpass123",
                Some("newpass456"),
                Some("ruby"),
            )
            .unwrap();
        assert!(ok);
        assert!(!service.authenticate("admin", "newpass456"));
        assert!(service.authenticate("root", "newpass456"));
        assert_eq!("ruby", service.get_user("root").unwrap().icon);

        // Wrong current password fails.
        let ok = service
            .update_profile("root", None, "wrong", Some("x"), None)
            .unwrap();
        assert!(!ok);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn rejects_short_passwords_and_taken_usernames() {
        let dir = temp_dir();
        let service = AuthService::initialize(&dir).unwrap();
        service.set_password("admin", "oldpass123").unwrap();
        service.set_password("other", "otherpass").unwrap();

        assert!(service
            .update_profile("admin", None, "oldpass123", Some("short"), None)
            .is_err());
        assert!(service
            .update_profile("admin", Some("other"), "oldpass123", None, None)
            .is_err());
        let _ = fs::remove_dir_all(&dir);
    }
}
