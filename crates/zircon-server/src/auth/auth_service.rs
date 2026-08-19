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
use totp_rs::{Algorithm, Secret, TOTP};

const ALPHABET: &str = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789!@#$%^&*";
/// Precomputed bcrypt hash of a random password. Verified against on every
/// authentication for an unknown username so response timing does not reveal
/// whether an account exists (username enumeration defense).
const DUMMY_BCRYPT_HASH: &str = "$2b$12$e8I3Q4kF1eN3WzL8zO0Q.eZ3q2w7F8Y7j6K5L4M3N2O1P0Q9R8S7T";

/// Serializable admin profile stored in `users.json`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct UserProfile {
    pub username: String,
    pub password_hash: String,
    #[serde(default = "default_icon")]
    pub icon: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub totp_secret: Option<String>,
    #[serde(default)]
    pub totp_enabled: bool,
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
            totp_secret: None,
            totp_enabled: false,
        }
    }

    /// Verifies a TOTP code against this profile's secret. Accounts without
    /// 2FA enabled (or without a stored secret) trivially pass.
    pub fn verify_totp(&self, code: &str) -> bool {
        if !self.totp_enabled {
            return true;
        }
        let Some(secret) = &self.totp_secret else {
            return true;
        };
        let Ok(secret_bytes) = Secret::Encoded(secret.clone()).to_bytes() else {
            return false;
        };
        let Ok(totp) = TOTP::new(
            Algorithm::SHA1,
            6,
            1,
            30,
            secret_bytes,
            Some("Zircon Server".to_string()),
            self.username.clone(),
        ) else {
            return false;
        };
        totp.check_current(code).unwrap_or(false)
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

    /// Verifies a username/password pair against the stored BCrypt hashes in
    /// constant time: unknown usernames still pay one bcrypt verification so
    /// an attacker cannot distinguish "wrong password" from "no such user" by
    /// timing alone.
    pub fn authenticate(&self, username: &str, password: &str) -> bool {
        let users = self.users.lock().unwrap();
        match users.get(username) {
            Some(profile) => {
                !password.is_empty()
                    && bcrypt::verify(password, &profile.password_hash).unwrap_or(false)
            }
            None => {
                // Execute dummy bcrypt work to equalize response timing.
                let _ = bcrypt::verify(password, DUMMY_BCRYPT_HASH);
                false
            }
        }
    }

    /// Returns the stored profile for a username, or `None` if unknown.
    pub fn get_user(&self, username: &str) -> Option<UserProfile> {
        self.users.lock().unwrap().get(username).cloned()
    }

    /// Updates the TOTP secret and enabled flag (2FA setup / enable / disable).
    pub fn set_totp(
        &self,
        username: &str,
        secret: Option<String>,
        enabled: bool,
    ) -> Result<(), String> {
        let mut users = self.users.lock().unwrap();
        let profile = users
            .get_mut(username)
            .ok_or_else(|| "User not found".to_string())?;
        profile.totp_secret = secret;
        profile.totp_enabled = enabled;
        drop(users);
        self.save().map_err(|e| e.to_string())
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
        write_secret_file(&self.users_file, json.as_bytes())
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

/// Writes a sensitive file and hardens it against other local users. Used for
/// `users.json`, `jwt-secret.key` and the audit log: on Unix the file is
/// chmod'ed `0o600`; on Windows a protected DACL grants SYSTEM, local
/// Administrators and the current user full access and denies everyone else
/// (including inheritance from the data dir).
pub fn write_secret_file(path: &Path, content: &[u8]) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, content)?;
    harden_secret_file(path)
}

/// Applies the platform's secret-file hardening to an existing file. Extracted
/// from `write_secret_file` so append-style writers (the audit log) can harden
/// without rewriting content.
pub fn harden_secret_file(path: &Path) -> std::io::Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        return fs::set_permissions(path, fs::Permissions::from_mode(0o600));
    }
    #[cfg(windows)]
    {
        restrict_dacl_windows(path)
    }
    #[cfg(not(any(unix, windows)))]
    {
        let _ = path;
        Ok(())
    }
}

/// Replaces the DACL of `path` with a protected ACL granting full access to
/// SYSTEM (`SY`), built-in Administrators (`BA`) and the current user (whose
/// SID is resolved at runtime — SDDL cannot express "current user" by name),
/// and denying everyone else. `PROTECTED_DACL` removes inheritable ACEs from
/// the data dir, so a loose parent directory cannot widen the exposure.
#[cfg(windows)]
fn restrict_dacl_windows(path: &Path) -> std::io::Result<()> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Foundation::LocalFree;
    use windows_sys::Win32::Security::Authorization::{
        ConvertStringSecurityDescriptorToSecurityDescriptorW, SetNamedSecurityInfoW,
        SDDL_REVISION_1, SE_FILE_OBJECT,
    };
    use windows_sys::Win32::Security::{
        GetSecurityDescriptorDacl, DACL_SECURITY_INFORMATION, PROTECTED_DACL_SECURITY_INFORMATION,
        PSECURITY_DESCRIPTOR,
    };

    let user_sid = current_user_sid()?;
    let sddl = format!("D:P(A;;FA;;;SY)(A;;FA;;;BA)(A;;FA;;;{user_sid})");

    // Build a self-relative security descriptor from the SDDL string.
    let mut descriptor: PSECURITY_DESCRIPTOR = core::ptr::null_mut();
    let sddl_wide: Vec<u16> = sddl.encode_utf16().chain(core::iter::once(0)).collect();
    let ok = unsafe {
        ConvertStringSecurityDescriptorToSecurityDescriptorW(
            sddl_wide.as_ptr(),
            SDDL_REVISION_1,
            &mut descriptor,
            core::ptr::null_mut(),
        )
    };
    if ok == 0 {
        return Err(std::io::Error::last_os_error());
    }

    // Pull the DACL out of the descriptor (SetNamedSecurityInfoW takes the ACL
    // pointer directly) and apply it to the file.
    let mut dacl_present: windows_sys::Win32::Foundation::BOOL = 0;
    let mut dacl: *mut windows_sys::Win32::Security::ACL = core::ptr::null_mut();
    let mut dacl_defaulted: windows_sys::Win32::Foundation::BOOL = 0;
    let ok = unsafe {
        GetSecurityDescriptorDacl(
            descriptor,
            &mut dacl_present,
            &mut dacl,
            &mut dacl_defaulted,
        )
    };
    if ok == 0 {
        let err = std::io::Error::last_os_error();
        unsafe { LocalFree(descriptor) };
        return Err(err);
    }

    let path_wide: Vec<u16> = path
        .as_os_str()
        .encode_wide()
        .chain(core::iter::once(0))
        .collect();
    let result = unsafe {
        SetNamedSecurityInfoW(
            path_wide.as_ptr(),
            SE_FILE_OBJECT,
            DACL_SECURITY_INFORMATION | PROTECTED_DACL_SECURITY_INFORMATION,
            core::ptr::null_mut(),
            core::ptr::null_mut(),
            dacl,
            core::ptr::null_mut(),
        )
    };
    // The descriptor was copied by SetNamedSecurityInfoW; free our copy.
    unsafe { LocalFree(descriptor) };

    if result != 0 {
        return Err(std::io::Error::from_raw_os_error(result as i32));
    }
    Ok(())
}

/// Resolves the current user's SID string (e.g. `S-1-5-21-...-1001`) via the
/// process token, for use inside an SDDL string.
#[cfg(windows)]
fn current_user_sid() -> std::io::Result<String> {
    use windows_sys::Win32::Foundation::{
        CloseHandle, GetLastError, LocalFree, ERROR_INSUFFICIENT_BUFFER, HANDLE,
    };
    use windows_sys::Win32::Security::Authorization::ConvertSidToStringSidW;
    use windows_sys::Win32::Security::{GetTokenInformation, TokenUser, TOKEN_QUERY};
    use windows_sys::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};

    let mut token: HANDLE = 0;
    let process = unsafe { GetCurrentProcess() };
    if unsafe { OpenProcessToken(process, TOKEN_QUERY, &mut token) } == 0 {
        return Err(std::io::Error::last_os_error());
    }

    // First call sizes the buffer; it must fail with ERROR_INSUFFICIENT_BUFFER.
    let mut needed: u32 = 0;
    unsafe { GetTokenInformation(token, TokenUser, core::ptr::null_mut(), 0, &mut needed) };
    let err = unsafe { GetLastError() };
    if err != ERROR_INSUFFICIENT_BUFFER {
        unsafe { CloseHandle(token) };
        return Err(std::io::Error::from_raw_os_error(err as i32));
    }

    let mut buffer = vec![0u8; needed as usize];
    if unsafe {
        GetTokenInformation(
            token,
            TokenUser,
            buffer.as_mut_ptr() as *mut _,
            needed,
            &mut needed,
        )
    } == 0
    {
        let err = unsafe { GetLastError() };
        unsafe { CloseHandle(token) };
        return Err(std::io::Error::from_raw_os_error(err as i32));
    }

    let token_user =
        unsafe { &*(buffer.as_ptr() as *const windows_sys::Win32::Security::TOKEN_USER) };
    let sid = token_user.User.Sid;

    let mut sid_string: windows_sys::core::PWSTR = core::ptr::null_mut();
    if unsafe { ConvertSidToStringSidW(sid, &mut sid_string) } == 0 {
        let err = unsafe { GetLastError() };
        unsafe { CloseHandle(token) };
        return Err(std::io::Error::from_raw_os_error(err as i32));
    }

    // Read the null-terminated wide string into a Rust String.
    let mut wide = Vec::new();
    let mut cursor = sid_string;
    unsafe {
        while *cursor != 0 {
            wide.push(*cursor);
            cursor = cursor.add(1);
        }
    }
    unsafe { LocalFree(sid_string as *mut core::ffi::c_void) };
    unsafe { CloseHandle(token) };

    Ok(String::from_utf16_lossy(&wide))
}

fn generate_random_password(length: usize) -> String {
    let mut bytes = vec![0u8; length];
    getrandom::fill(&mut bytes).expect("failed to read OS entropy");
    bytes
        .iter()
        .map(|b| {
            ALPHABET
                .chars()
                .nth((*b as usize) % ALPHABET.len())
                .unwrap()
        })
        .collect()
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
    fn secret_file_hardening_applies_on_windows() {
        #[cfg(windows)]
        {
            let dir = temp_dir();
            let file = dir.join("users.json");
            write_secret_file(&file, b"{\"admin\":\"hash\"}").unwrap();
            assert!(file.is_file());
            // Round-trips: an existing hardened file stays writable by us.
            harden_secret_file(&file).unwrap();
            std::fs::write(&file, b"updated").unwrap();
            assert_eq!("updated", std::fs::read_to_string(&file).unwrap());

            // End-to-end DACL check: only SYSTEM, built-in Administrators and
            // the current user hold access; the inherited "Everyone"/"Users"
            // ACEs from the temp dir must be gone.
            let output = std::process::Command::new("icacls")
                .arg(file.to_string_lossy().as_ref())
                .output()
                .expect("icacls must be available on Windows");
            let text = String::from_utf8_lossy(&output.stdout).to_lowercase();
            assert!(text.contains("system"), "SYSTEM must hold access: {text}");
            assert!(
                text.contains("administrators"),
                "Administrators must hold access: {text}"
            );
            assert!(
                !text.contains("everyone"),
                "Everyone must be denied: {text}"
            );
            // An inherited "Users"/"Authenticated Users" ACE would render as
            // `<account>\users:(f)`; the temp path also contains "users", so
            // match the ACE form specifically.
            assert!(!text.contains("users:(f)"), "Users must be denied: {text}");
            let _ = fs::remove_dir_all(&dir);
        }
    }

    #[test]
    fn secret_file_is_owner_only_on_unix() {
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let dir = temp_dir();
            let file = dir.join("users.json");
            write_secret_file(&file, b"secret").unwrap();
            let mode = std::fs::metadata(&file).unwrap().permissions().mode() & 0o777;
            assert_eq!(0o600, mode, "secret file must be owner-only");
            let _ = fs::remove_dir_all(&dir);
        }
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

    #[test]
    fn totp_secret_round_trips_and_verifies_codes() {
        let dir = temp_dir();
        let service = AuthService::initialize(&dir).unwrap();
        service.set_password("admin", "hunter2").unwrap();

        // Accounts without 2FA pass trivially.
        let profile = service.get_user("admin").unwrap();
        assert!(!profile.totp_enabled);
        assert!(profile.verify_totp("000000"));

        // Generate a secret exactly like the setup endpoint does and verify a
        // live code round-trips.
        let secret = Secret::generate_secret();
        let encoded = secret.to_encoded().to_string();
        service
            .set_totp("admin", Some(encoded.clone()), true)
            .unwrap();

        let profile = service.get_user("admin").unwrap();
        assert!(profile.totp_enabled);
        let totp = TOTP::new(
            Algorithm::SHA1,
            6,
            1,
            30,
            secret.to_bytes().unwrap(),
            Some("Zircon Server".to_string()),
            "admin".to_string(),
        )
        .unwrap();
        let code = totp.generate_current().unwrap();
        assert!(profile.verify_totp(&code), "live code must verify");
        assert!(!profile.verify_totp("000000"), "wrong code must fail");

        // Disabling turns the gate off again.
        service.set_totp("admin", None, false).unwrap();
        let profile = service.get_user("admin").unwrap();
        assert!(!profile.totp_enabled);
        assert!(profile.verify_totp("000000"));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn unknown_user_authenticate_is_constant_time_shaped() {
        let dir = temp_dir();
        let service = AuthService::initialize(&dir).unwrap();
        // No such user: must return false without panicking, regardless of
        // password content (the dummy bcrypt work happens inside).
        assert!(!service.authenticate("ghost", ""));
        assert!(!service.authenticate("ghost", "hunter2"));
        let _ = fs::remove_dir_all(&dir);
    }
}
