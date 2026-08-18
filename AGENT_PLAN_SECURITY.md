# Threat Model & Architecture Overview

Because Zircon is open-source and anyone can run a server or connect with a client, the security architecture operates under a **Zero-Trust Threat Model**:

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                            ZERO-TRUST THREAT MODEL                          │
├─────────────────────────────────────────────────────────────────────────────┤
│ 1. Hostile Server vs. Honest Player:                                        │
│    A rogue server publishes a malicious BOM to push malware to players.     │
│    -> Defense: Fail-closed mod verification (Modrinth/CurseForge hashes),   │
│       strict HTTPS whitelist (SSRF check), path-sanitized mod folders,      │
│       and client subprocess environment scrubbing (env_clear).              │
│                                                                             │
│ 2. Hostile Player vs. Honest Server:                                        │
│    An attacker attempts protocol bypass, credential brute-force, or DoS.   │
│    -> Defense: Fail-closed join ticket gate, Slowloris read timeouts,       │
│       constant-time bcrypt checks, TOTP 2FA, and username-keyed rate limits.│
│                                                                             │
│ 3. Compromised Operator vs. Host Machine:                                   │
│    An attacker with stolen admin credentials attempts host takeover.        │
│    -> Defense: Blocklist dangerous JVM flags (-javaagent), scrub subprocess │
│       environment variables, enforce 0o600 secret permissions, and write to │
│       an append-only audit.log.                                             │
│                                                                             │
│ 4. Supply Chain & Update Delivery:                                          │
│    Updates served from Cloudflare R2 bucket (https://zirconmc.net/updates/). │
│    -> Defense: Launcher verified via Ed25519 signatures (Tauri v2),         │
│       Server verified via SHA-256 hash checks + HTTPS domain validation.    │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

# MASTER AGENT PLAN: Complete Security Hardening & Auto-Updater

---

## Phase 1: Core SSRF, Path Traversal & Archive Hardening (`zircon-core`)

### [x] Step 1.1: Enforce Strict HTTPS, Default Ports, and Whitelist in SSRF Validator
**File:** `zircon-core/src/security/ssrf.rs`
* **Reasoning:** A malicious server or actor might supply URLs with `http://` (allowing MITM tampering), custom ports (for intranet port scanning), or non-whitelisted domains. Enforce `https://`, reject explicit port numbers, and add `zirconmc.net`.

```rust
//! SSRF protection for outbound mod/pack/update downloads.

pub const ALLOWED_CDN_DOMAINS: &[&str] = &[
    "zirconmc.net",
    "cdn.modrinth.com",
    "edge.forgecdn.net",
    "media.forgecdn.net",
    "maven.neoforged.net",
    "maven.minecraftforge.net",
    "meta.fabricmc.net",
    "meta.quiltmc.org",
    "piston-meta.mojang.com",
    "piston-data.mojang.com",
    "launcher.mojang.com",
    "launchermeta.mojang.com",
];

/// Returns `true` if the URL is strictly HTTPS, has no custom port, and its host is allowed.
pub fn is_safe_cdn_url(url: &str) -> bool {
    let parsed = match url::Url::parse(url) {
        Ok(parsed) => parsed,
        Err(_) => return false,
    };

    // 1. Enforce HTTPS only (no plaintext HTTP or file/ftp schemes)
    if parsed.scheme() != "https" {
        return false;
    }

    // 2. Disallow custom ports (prevent internal network scanning)
    if parsed.port().is_some() {
        return false;
    }

    let host = match parsed.host_str() {
        Some(host) if !host.is_empty() => host,
        _ => return false,
    };

    let host_lower = host.to_ascii_lowercase();
    ALLOWED_CDN_DOMAINS
        .iter()
        .any(|allowed| host_lower == *allowed || host_lower.ends_with(&format!(".{allowed}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_allowed_https_domains() {
        assert!(is_safe_cdn_url("https://zirconmc.net/updates/server/latest.json"));
        assert!(is_safe_cdn_url("https://cdn.modrinth.com/data/abc/1.0.jar"));
        assert!(is_safe_cdn_url("https://edge.forgecdn.net/files/123/456/mod.jar"));
    }

    #[test]
    fn rejects_insecure_schemes_and_ports() {
        assert!(!is_safe_cdn_url("http://cdn.modrinth.com/data/abc/1.0.jar")); // No HTTP
        assert!(!is_safe_cdn_url("https://cdn.modrinth.com:8443/data/abc/1.0.jar")); // No custom ports
        assert!(!is_safe_cdn_url("file:///etc/passwd"));
        assert!(!is_safe_cdn_url("http://169.254.169.254/latest/meta-data/"));
    }
}
```

---

### [x] Step 1.2: Reject Symlinks, Hardlinks & Decompression Bombs
**File:** `zircon-core/src/archive/lz4_tar.rs`
* **Reasoning:** Untrusted archives could extract symlinks pointing outside the target directory (Tar-slip) or contain billions of zeroes (decompression bomb). Reject symlinks and hardlinks explicitly and cap total extraction size.

Update `extract_archive` in `zircon-core/src/archive/lz4_tar.rs`:

```rust
const MAX_TOTAL_EXTRACT_BYTES: u64 = 10 * 1024 * 1024 * 1024; // 10 GB
const MAX_FILE_ENTRIES: usize = 50_000;

pub fn extract_archive(archive_file: &Path, destination_dir: &Path) -> io::Result<()> {
    let file_in = File::open(archive_file)?;
    let lz4_in = FrameDecoder::new(file_in);
    let mut tar_in = Archive::new(lz4_in);

    let dest = canonicalize_or_create(destination_dir)?;

    let mut entry_count = 0;
    let mut total_uncompressed: u64 = 0;

    let entries = tar_in.entries()?;
    for entry in entries {
        entry_count += 1;
        if entry_count > MAX_FILE_ENTRIES {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Archive exceeds maximum allowed entry count (decompression bomb defense)",
            ));
        }

        let mut entry = entry?;
        let entry_type = entry.header().entry_type();

        // Prevent Symlink / Hardlink directory escape attacks
        if entry_type.is_symlink() || entry_type.is_hard_link() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("Refusing to extract symlink/hardlink entry: {}", entry.path()?.display()),
            ));
        }

        let path = entry.path()?;
        let safe_path = sanitize_entry_path(&path).ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("Zip slip attempt detected: {}", path.display()),
            )
        })?;

        let target = dest.join(&safe_path);
        if !target.starts_with(&dest) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("Zip slip attempt detected: {}", path.display()),
            ));
        }

        if entry_type.is_dir() {
            std::fs::create_dir_all(&target)?;
        } else {
            if let Some(parent) = target.parent() {
                std::fs::create_dir_all(parent)?;
            }
            let mut out = File::create(&target)?;
            let written = io::copy(&mut entry, &mut out)?;
            total_uncompressed += written;

            if total_uncompressed > MAX_TOTAL_EXTRACT_BYTES {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "Archive exceeds maximum allowed uncompressed size",
                ));
            }
        }
    }
    Ok(())
}
```

---

## Phase 2: Server Multiplexer & Protocol Gate Hardening (`zircon-server`)

### [x] Step 2.1: Fix Join-Ticket Gate Fail-Open & Add Slowloris Detection Timeout
**File:** `zircon-server/src/multiplexer/tcp.rs`
* **Reasoning:** In `handle_connection`, if `handshake.next_state == 2` and `parse_login_start_username` returned `NotMatch`, the code previously did nothing and fell through to proxy the connection, allowing vanilla clients to bypass the launcher requirement. Make it fail-closed and enforce a 5-second detection timeout.

Update `handle_connection` in `zircon-server/src/multiplexer/tcp.rs`:

```rust
async fn handle_connection(
    &self,
    mut client: TcpStream,
    fixed_instance: Option<InstanceConfig>,
) -> io::Result<()> {
    let detection_future = async {
        let mut buf: Vec<u8> = Vec::with_capacity(1024);
        let mut tmp = [0u8; 2048];
        loop {
            let n = client.read(&mut tmp).await?;
            if n == 0 {
                return Ok(None);
            }
            buf.extend_from_slice(&tmp[..n]);

            if detector::is_http_method(&buf) && self.http_proxy_enabled() {
                return Ok(Some((buf, self.web_port)));
            }

            match detector::parse_handshake(&buf) {
                ParseResult::Incomplete => {
                    if buf.len() > 4096 {
                        let port = self.resolve_target_port(None, &fixed_instance);
                        return Ok(Some((buf, port)));
                    }
                    continue;
                }
                ParseResult::NotMatch => {
                    let port = self.resolve_target_port(None, &fixed_instance);
                    return Ok(Some((buf, port)));
                }
                ParseResult::Matched(handshake) => {
                    let target_port = self.resolve_target_port(Some(&handshake), &fixed_instance);

                    // Zircon join gate: login connections MUST present a valid join ticket
                    if handshake.next_state == 2 {
                        match detector::parse_login_start_username(&buf) {
                            ParseResult::Incomplete => continue,
                            ParseResult::NotMatch => {
                                // FAIL-CLOSED: Reject unparseable / forged login start frames
                                tracing::warn!("Rejecting unparseable Login Start frame");
                                let packet = disconnect::create_disconnect_packet(
                                    disconnect::build_custom_error_message(),
                                );
                                let _ = client.write_all(&packet).await;
                                let _ = client.shutdown().await;
                                return Ok(None);
                            }
                            ParseResult::Matched(username) => {
                                if !self.tickets.consume_ticket(&username) {
                                    tracing::info!(
                                        "Rejected connection for '{username}' — no active Zircon join ticket"
                                    );
                                    let packet = disconnect::create_disconnect_packet(
                                        disconnect::build_custom_error_message(),
                                    );
                                    let _ = client.write_all(&packet).await;
                                    let _ = client.shutdown().await;
                                    return Ok(None);
                                }
                                if let Some(instances) = &self.instances {
                                    if let Some(cfg) = instances.find_by_internal_port(target_port) {
                                        instances.clear_pending_join_intent(&cfg.id);
                                    }
                                }
                            }
                        }
                    }
                    return Ok(Some((buf, target_port)));
                }
            }
        }
    };

    // 5-second timeout stops Slowloris socket starvation attacks
    match tokio::time::timeout(Duration::from_secs(5), detection_future).await {
        Ok(Ok(Some((buf, port)))) => self.proxy(client, buf, port).await,
        Ok(Ok(None)) => Ok(()),
        Ok(Err(e)) => Err(e),
        Err(_) => {
            tracing::warn!("Protocol detection timed out on socket; connection dropped");
            Ok(())
        }
    }
}
```

---

## Phase 3: Server Auth, 2FA, Session Cookies & Audit Logging (`zircon-server`)

### [x] Step 3.1: Add Dependencies in `zircon-server/Cargo.toml`
**File:** `zircon-server/Cargo.toml`

```toml
[dependencies]
zircon-core = { path = "../zircon-core" }

tokio = { workspace = true, features = ["full"] }
axum = { workspace = true }
axum-extra = { version = "0.9", features = ["cookie"] }
tower-http = { workspace = true }
serde.workspace = true
serde_json.workspace = true
reqwest = { workspace = true }
bcrypt.workspace = true
jsonwebtoken.workspace = true
totp-rs = { version = "5.5", features = ["gen_secret"] }
self_replace = "1.5"
semver = "1.0"
sysinfo.workspace = true
tracing.workspace = true
tracing-subscriber.workspace = true
dashmap.workspace = true
bytes.workspace = true
uuid.workspace = true
chrono.workspace = true
url.workspace = true
zip.workspace = true
getrandom = "0.3"
tokio-util = { workspace = true, features = ["io"] }
futures-util = "0.3"
sha2 = { workspace = true }
hex = { workspace = true }
```

---

### [x] Step 3.2: Create Tamper-Evident Audit Logging Module
**File:** `zircon-server/src/audit.rs` (NEW)
* **Reasoning:** In an open-source server environment where multiple admins or operators may exist, every sensitive action (login, password change, command execution, mod upload, backup restore) must be written to an append-only, `0o600` protected file (`audit.log`).

```rust
//! Append-only audit logging for administrative actions.

use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use chrono::Utc;

pub struct AuditLogger {
    log_file: PathBuf,
    lock: Mutex<()>,
}

impl AuditLogger {
    pub fn new(data_dir: &Path) -> Self {
        let log_file = data_dir.join("audit.log");
        Self {
            log_file,
            lock: Mutex::new(()),
        }
    }

    pub fn log(&self, username: &str, action: &str, details: &str) {
        let _guard = self.lock.lock().unwrap();
        let timestamp = Utc::now().to_rfc3339();
        let entry = format!("[{timestamp}] [USER:{username}] [{action}] {details}\n");

        if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(&self.log_file) {
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let _ = std::fs::set_permissions(&self.log_file, std::fs::Permissions::from_mode(0o600));
            }
            let _ = file.write_all(entry.as_bytes());
        }
    }
}
```

Register `pub mod audit;` in `zircon-server/src/lib.rs`.

---

### [x] Step 3.3: Constant-Time Bcrypt Authentication & TOTP 2FA
**File:** `zircon-server/src/auth/auth_service.rs`
* Prevent timing attacks by running a dummy bcrypt verification on missing users.
* Add TOTP secret generation, verification, and enabling/disabling.
* Restrict `users.json` to owner-only permissions (`0o600`) on Unix.

```rust
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use serde::{Deserialize, Serialize};
use totp_rs::{Algorithm, Secret, TOTP};

const ALPHABET: &str = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789!@#$%^&*";
const DUMMY_BCRYPT_HASH: &str = "$2b$12$e8I3Q4kF1eN3WzL8zO0Q.eZ3q2w7F8Y7j6K5L4M3N2O1P0Q9R8S7T";

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

pub struct AuthService {
    users_file: PathBuf,
    users: Mutex<BTreeMap<String, UserProfile>>,
}

impl AuthService {
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
            println!("  ZIRCON SERVER INITIAL ADMIN CREDENTIALS");
            println!("  Username: admin");
            println!("  Password: {initial_password}");
            println!("  Please log in and set up TOTP 2FA!");
            println!("=================================================");
            return Ok(service);
        };
        Ok(Self {
            users_file,
            users: Mutex::new(users),
        })
    }

    /// Constant-time authentication preventing username enumeration
    pub fn authenticate(&self, username: &str, password: &str) -> bool {
        let users = self.users.lock().unwrap();
        match users.get(username) {
            Some(profile) => {
                !password.is_empty()
                    && bcrypt::verify(password, &profile.password_hash).unwrap_or(false)
            }
            None => {
                // Execute dummy bcrypt work to equalize response timing
                let _ = bcrypt::verify(password, DUMMY_BCRYPT_HASH);
                false
            }
        }
    }

    pub fn get_user(&self, username: &str) -> Option<UserProfile> {
        self.users.lock().unwrap().get(username).cloned()
    }

    pub fn set_totp(&self, username: &str, secret: Option<String>, enabled: bool) -> Result<(), String> {
        let mut users = self.users.lock().unwrap();
        let profile = users.get_mut(username).ok_or_else(|| "User not found".to_string())?;
        profile.totp_secret = secret;
        profile.totp_enabled = enabled;
        drop(users);
        self.save().map_err(|e| e.to_string())
    }

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
        Ok(true)
    }

    pub fn change_password(
        &self,
        username: &str,
        current_password: &str,
        new_password: &str,
    ) -> Result<bool, String> {
        self.update_profile(username, None, current_password, Some(new_password), None)
    }

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

fn load(file: &Path) -> std::io::Result<BTreeMap<String, UserProfile>> {
    let content = fs::read_to_string(file)?;
    if let Ok(parsed) = serde_json::from_str::<BTreeMap<String, UserProfile>>(&content) {
        if !parsed.is_empty() {
            return Ok(parsed);
        }
    }
    if let Ok(legacy) = serde_json::from_str::<BTreeMap<String, String>>(&content) {
        let migrated: BTreeMap<String, UserProfile> = legacy
            .into_iter()
            .filter(|(_, hash)| !hash.trim().is_empty())
            .map(|(name, hash)| (name.clone(), UserProfile::new(name, hash, None)))
            .collect();
        if !migrated.is_empty() {
            return Ok(migrated);
        }
    }
    Ok(BTreeMap::new())
}

pub fn write_secret_file(path: &Path, content: &[u8]) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, content)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(path, fs::Permissions::from_mode(0o600));
    }
    Ok(())
}

fn generate_random_password(length: usize) -> String {
    let mut bytes = vec![0u8; length];
    getrandom::fill(&mut bytes).expect("failed to read OS entropy");
    bytes
        .iter()
        .map(|b| ALPHABET.chars().nth((*b as usize) % ALPHABET.len()).unwrap())
        .collect()
}
```

---

### [x] Step 3.4: Protect `jwt-secret.key` with Unix `0o600` Permissions
**File:** `zircon-server/src/auth/jwt.rs`
Update `initialize` to use `write_secret_file`:

```rust
use crate::auth::auth_service::write_secret_file;

pub fn initialize(data_dir: &Path) -> std::io::Result<()> {
    fs::create_dir_all(data_dir)?;
    let secret_file = data_dir.join("jwt-secret.key");
    let secret_bytes: Vec<u8> = if secret_file.is_file() {
        let content = fs::read_to_string(&secret_file)?;
        let decoded = base64_decode(content.trim());
        if decoded.is_empty() {
            return Err(std::io::Error::other("jwt-secret.key is empty or invalid base64"));
        }
        decoded
    } else {
        let secret = generate_secret();
        write_secret_file(&secret_file, base64_encode(&secret).as_bytes())?;
        tracing::info!("Generated new JWT signing secret at {}", secret_file.display());
        secret
    };
    let _ = SECRET.set(secret_bytes);
    Ok(())
}
```

---

### [x] Step 3.5: User-Scoped Rate Limiting & HttpOnly Cookie Session Handler
**File:** `zircon-server/src/web/controllers/auth_controller.rs`

```rust
//! Admin auth endpoints with TOTP 2FA, HttpOnly cookies, and per-user rate limiting.

use std::net::SocketAddr;
use axum::extract::{ConnectInfo, State};
use axum::http::StatusCode;
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

fn limiter_key(username: &str, client: &Option<ConnectInfo<SocketAddr>>) -> String {
    let u = username.trim().to_lowercase();
    if !u.is_empty() {
        format!("user:{u}")
    } else {
        client
            .as_ref()
            .map(|c| format!("ip:{}", c.0.ip()))
            .unwrap_or_else(|| "ip:unknown".to_string())
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
        state.audit.log(&body.username, "LOGIN_FAILED", "Invalid credentials");
        return Err(ApiError::Unauthorized("Invalid username or password".to_string()));
    }

    let user = state.auth.get_user(&body.username).ok_or_else(|| {
        ApiError::Unauthorized("User record not found".to_string())
    })?;

    if user.totp_enabled {
        let code = body.totp_code.as_deref().unwrap_or("");
        if !user.verify_totp(code) {
            state.audit.log(&body.username, "LOGIN_FAILED", "Invalid TOTP code");
            return Err(ApiError::Unauthorized("Invalid TOTP two-factor code".to_string()));
        }
    }

    state.login_limiter.reset(&key);
    let token = issue_token(&state, &body.username);
    state.audit.log(&body.username, "LOGIN_SUCCESS", "Authenticated successfully");

    let mut cookie = Cookie::new("zircon_session", token.clone());
    cookie.set_http_only(true);
    cookie.set_same_site(SameSite::Strict);
    cookie.set_secure(true);
    cookie.set_path("/");

    let updated_jar = jar.add(cookie);
    Ok((updated_jar, Json(serde_json::json!({ "token": token, "username": body.username }))))
}

/// POST /api/auth/logout
pub async fn logout(
    State(state): State<AppState>,
    jar: CookieJar,
    user: CurrentUser,
) -> Result<(CookieJar, Json<serde_json::Value>), ApiError> {
    state.sessions.revoke(&user.jti, &user.username, user.exp);
    state.audit.log(&user.username, "LOGOUT", "Session terminated");
    let updated_jar = jar.remove(Cookie::from("zircon_session"));
    Ok((updated_jar, Json(serde_json::json!({ "ok": true }))))
}

/// GET /api/auth/me
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

/// POST /api/auth/2fa/setup — Generates a new TOTP secret & QR URI
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
    ).map_err(|e| ApiError::Internal(e.to_string()))?;

    let url = totp.get_url();
    state.auth.set_totp(&user.username, Some(secret_encoded.clone()), false)
        .map_err(ApiError::Internal)?;

    Ok(Json(serde_json::json!({
        "secret": secret_encoded,
        "qrUrl": url
    })))
}

/// POST /api/auth/2fa/enable — Verifies and enables 2FA
pub async fn enable_2fa(
    State(state): State<AppState>,
    user: CurrentUser,
    Json(body): Json<TotpEnableRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let profile = state.auth.get_user(&user.username)
        .ok_or_else(|| ApiError::NotFound("User not found".to_string()))?;

    let Some(secret) = &profile.totp_secret else {
        return Err(ApiError::BadRequest("2FA not initialized. Run setup first.".to_string()));
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
        return Err(ApiError::Unauthorized("Invalid confirmation code".to_string()));
    }

    state.auth.set_totp(&user.username, Some(secret.clone()), true)
        .map_err(ApiError::Internal)?;
    state.audit.log(&user.username, "2FA_ENABLED", "Two-factor authentication activated");

    Ok(Json(serde_json::json!({ "ok": true, "totpEnabled": true })))
}

/// POST /api/auth/2fa/disable
pub async fn disable_2fa(
    State(state): State<AppState>,
    user: CurrentUser,
) -> Result<Json<serde_json::Value>, ApiError> {
    state.auth.set_totp(&user.username, None, false).map_err(ApiError::Internal)?;
    state.audit.log(&user.username, "2FA_DISABLED", "Two-factor authentication disabled");
    Ok(Json(serde_json::json!({ "ok": true, "totpEnabled": false })))
}

/// POST /api/auth/change-password
pub async fn change_password(
    State(state): State<AppState>,
    jar: CookieJar,
    client: Option<ConnectInfo<SocketAddr>>,
    Json(body): Json<ChangePasswordRequest>,
) -> Result<(CookieJar, Json<serde_json::Value>), ApiError> {
    if body.username.is_empty() || body.current_password.is_empty() || body.new_password.is_empty() {
        return Err(ApiError::BadRequest("All fields are required".to_string()));
    }
    let key = limiter_key(&body.username, &client);
    rate_limited(&state, &key)?;

    match state.auth.change_password(&body.username, &body.current_password, &body.new_password) {
        Ok(true) => {
            state.login_limiter.reset(&key);
            state.sessions.revoke_user(&body.username);
            state.audit.log(&body.username, "PASSWORD_CHANGED", "Password successfully changed; sessions revoked");
            let token = issue_token(&state, &body.username);

            let mut cookie = Cookie::new("zircon_session", token.clone());
            cookie.set_http_only(true);
            cookie.set_same_site(SameSite::Strict);
            cookie.set_secure(true);
            cookie.set_path("/");

            Ok((jar.add(cookie), Json(serde_json::json!({ "ok": true, "token": token }))))
        }
        Ok(false) => Err(ApiError::Unauthorized("Invalid current password".to_string())),
        Err(e) => Err(ApiError::BadRequest(e)),
    }
}

/// POST /api/auth/profile
pub async fn profile(
    State(state): State<AppState>,
    jar: CookieJar,
    Json(body): Json<ProfileUpdateRequest>,
) -> Result<(CookieJar, Json<serde_json::Value>), ApiError> {
    if body.current_username.is_empty() || body.current_password.is_empty() {
        return Err(ApiError::BadRequest("Current credentials required".to_string()));
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

            state.audit.log(&body.current_username, "PROFILE_UPDATED", &format!("Target: {target}"));

            if body.new_password.as_deref().is_some_and(|p| !p.is_empty()) {
                state.sessions.revoke_user(target);
                let token = issue_token(&state, target);

                let mut cookie = Cookie::new("zircon_session", token.clone());
                cookie.set_http_only(true);
                cookie.set_same_site(SameSite::Strict);
                cookie.set_secure(true);
                cookie.set_path("/");

                jar = jar.add(cookie);
                response["token"] = serde_json::json!(token);
            }
            Ok((jar, Json(response)))
        }
        Ok(false) => Err(ApiError::Unauthorized("Invalid credentials".to_string())),
        Err(e) => Err(ApiError::BadRequest(e)),
    }
}
```

---

### [x] Step 3.6: Update Auth Middleware to Support Dual Extraction (Cookie / Bearer)
**File:** `zircon-server/src/web/auth.rs`

```rust
use axum::extract::Request;
use axum::extract::{FromRequestParts, State};
use axum::http::header::AUTHORIZATION;
use axum::http::request::Parts;
use axum::middleware::Next;
use axum::response::Response;
use axum_extra::extract::cookie::CookieJar;

use super::app::ApiError;
use crate::auth::jwt;
use crate::web::app::AppState;

#[derive(Debug, Clone)]
pub struct CurrentUser {
    pub username: String,
    pub jti: String,
    pub exp: i64,
}

#[axum::async_trait]
impl<S> FromRequestParts<S> for CurrentUser {
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        parts
            .extensions
            .get::<CurrentUser>()
            .cloned()
            .ok_or_else(|| {
                ApiError::Unauthorized("Authentication required. Please log in.".to_string())
            })
    }
}

fn extract_token(request: &Request) -> Option<String> {
    if let Some(auth_header) = request.headers().get(AUTHORIZATION) {
        if let Ok(val) = auth_header.to_str() {
            if let Some(t) = val.strip_prefix("Bearer ") {
                let trimmed = t.trim();
                if !trimmed.is_empty() {
                    return Some(trimmed.to_string());
                }
            }
        }
    }
    let jar = CookieJar::from_headers(request.headers());
    jar.get("zircon_session").map(|c| c.value().to_string())
}

pub async fn require_auth(
    State(state): State<AppState>,
    request: Request,
    next: Next,
) -> Result<Response, ApiError> {
    let Some(token) = extract_token(&request) else {
        return Err(ApiError::Unauthorized("Authentication required.".to_string()));
    };

    let Some(claims) = jwt::decode_claims(&token) else {
        return Err(ApiError::Unauthorized("Invalid or expired session.".to_string()));
    };

    if state.sessions.is_revoked(&claims.jti) {
        return Err(ApiError::Unauthorized("Session has been revoked.".to_string()));
    }

    if state.auth.get_user(&claims.sub).is_none() {
        return Err(ApiError::Unauthorized("Account no longer exists.".to_string()));
    }

    let mut request = request;
    request.extensions_mut().insert(CurrentUser {
        username: claims.sub,
        jti: claims.jti,
        exp: claims.exp,
    });
    Ok(next.run(request).await)
}
```

---

### [x] Step 3.7: Add Timeout to WebSocket Console Authentication
**File:** `zircon-server/src/web/controllers/console_controller.rs`
Update `handle_console_socket`:

```rust
async fn handle_console_socket(socket: WebSocket, state: AppState) {
    let (mut sender, mut receiver) = socket.split();

    // 5-second deadline to send the first AUTH message
    let first_msg = tokio::time::timeout(
        std::time::Duration::from_secs(5),
        receiver.next(),
    )
    .await;

    let authenticated = match first_msg {
        Ok(Some(Ok(Message::Text(text)))) => {
            validate_console_auth(parse_auth_message(&text), &state.sessions)
        }
        _ => false,
    };

    if !authenticated {
        let _ = sender
            .send(Message::Text(
                "[wrapper] Authentication failed or timed out — connection closed.".to_string(),
            ))
            .await;
        let _ = sender.close().await;
        return;
    }

    let mut broadcast_rx = state.console.subscribe();

    for line in state.console.recent_history(500) {
        if sender.send(Message::Text(line)).await.is_err() {
            return;
        }
    }

    loop {
        tokio::select! {
            msg = broadcast_rx.recv() => {
                match msg {
                    Ok(line) => {
                        if sender.send(Message::Text(line)).await.is_err() {
                            break;
                        }
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {
                        if sender.send(Message::Text("[wrapper] Console stream lagged; reconnecting...".to_string())).await.is_err() {
                            break;
                        }
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                }
            }
            msg = receiver.next() => {
                match msg {
                    Some(Ok(Message::Text(text))) => {
                        if text.trim() == "__CLEAR__" {
                            state.console.clear_history();
                            if sender.send(Message::Text("__CLEAR__".to_string())).await.is_err() {
                                break;
                            }
                            continue;
                        }
                        state.audit.log("ADMIN_WS", "CONSOLE_COMMAND", text.trim());
                        match state.process_manager.send_command(text.trim()).await {
                            Ok(()) => {}
                            Err(e) => {
                                if sender.send(Message::Text(format!("[wrapper] {e}"))).await.is_err() {
                                    break;
                                }
                            }
                        }
                    }
                    Some(Ok(Message::Close(_))) | None => break,
                    Some(Err(_)) => break,
                    _ => {}
                }
            }
        }
    }
}
```

---

## Phase 4: Subprocess, File & Environment Hardening

### [x] Step 4.1: Block Dangerous JVM Flags (`-javaagent`, `-agentlib`, etc.)
**File:** `zircon-server/src/instance.rs`
Update `has_invalid_heap_arg` to block flags capable of native arbitrary code execution:

```rust
const FORBIDDEN_JVM_PREFIXES: &[&str] = &[
    "-javaagent",
    "-agentlib",
    "-agentpath",
    "-xbootclasspath",
    "-xdebug",
    "-xrunjdwp",
    "-djava.security.manager",
];

pub fn contains_dangerous_jvm_args(args: &str) -> bool {
    let lower = args.to_ascii_lowercase();
    FORBIDDEN_JVM_PREFIXES.iter().any(|p| lower.contains(p))
}

pub fn has_invalid_heap_arg(args: &str) -> bool {
    if contains_dangerous_jvm_args(args) {
        return true;
    }
    args.split_whitespace().any(|token| {
        let lower = token.to_ascii_lowercase();
        let value = if let Some(v) = lower.strip_prefix("-xmx") {
            Some(v)
        } else if let Some(v) = lower.strip_prefix("-xms") {
            Some(v)
        } else {
            None
        };
        match value {
            Some(v) => {
                let number = v.trim_end_matches(['g', 'm', 'k']);
                number.parse::<f64>().map(|n| !(n > 0.0)).unwrap_or(true)
            }
            None => false,
        }
    })
}
```

---

### [x] Step 4.2: Sanitize Windows Reserved Device Names
**File:** `zircon-server/src/services/mods.rs` and `zircon-server/src/services/packs.rs`
Add reserved name check in `sanitize_filename` & `sanitize_pack_filename`:

```rust
const WINDOWS_RESERVED: &[&str] = &[
    "CON", "PRN", "AUX", "NUL",
    "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8", "COM9",
    "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
];

pub fn sanitize_filename(filename: &str) -> Result<String, ModError> {
    if filename.is_empty() {
        return Err(ModError::Invalid("filename is required".to_string()));
    }
    let mut base: String = filename.replace('\\', "/");
    if let Some(slash) = base.rfind('/') {
        base = base[slash + 1..].to_string();
    }
    let sanitized: String = base
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-') {
                c
            } else {
                '_'
            }
        })
        .collect();
    let mut base = if sanitized.trim().is_empty() {
        format!("mod-{}.jar", &Uuid::new_v4().simple().to_string()[..8])
    } else {
        sanitized
    };
    if !base.to_lowercase().ends_with(".jar") {
        base = format!("{base}.jar");
    }

    let stem = base.trim_end_matches(".jar").to_ascii_uppercase();
    if WINDOWS_RESERVED.contains(&stem.as_str()) {
        base = format!("file_{base}");
    }
    Ok(base)
}
```

---

### [x] Step 4.3: Scrub Subprocess Environment Variables on Java Launch
**File:** `zircon-server/src/process/manager.rs` and `zircon-launcher/src/launch/runner.rs`
* **Reasoning:** Minecraft mod code executes inside the spawned JVM. Clear process environment to avoid leaking host secrets (`AWS_ACCESS_KEY_ID`, `GITHUB_TOKEN`, etc.).

```rust
let mut command = tokio::process::Command::new(installer::java_bin());
command.env_clear();
command.envs(std::env::vars().filter(|(k, _)| {
    let upper = k.to_ascii_uppercase();
    matches!(upper.as_str(), "PATH" | "SYSTEMROOT" | "USERPROFILE" | "HOME" | "TMP" | "TEMP")
}));
```

---

## Phase 5: Server Self-Updater Engine (`zircon-server`)

### [x] Step 5.1: Create `zircon-server/src/updater.rs`
```rust
//! In-place binary self-updater for zircon-server against Cloudflare R2.

use std::env;
use std::io::{Cursor, Read};
use std::process::Command;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use zircon_core::security::ssrf;

pub const CURRENT_SERVER_VERSION: &str = env!("CARGO_PKG_VERSION");
pub const SERVER_UPDATE_URL: &str = "https://zirconmc.net/updates/server/latest.json";

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerUpdateManifest {
    pub version: String,
    pub release_date: String,
    pub notes: Option<String>,
    pub platforms: std::collections::HashMap<String, PlatformArtifact>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PlatformArtifact {
    pub url: String,
    pub sha256: String,
    #[serde(rename = "binName")]
    pub bin_name: String,
}

pub struct ServerUpdater {
    client: reqwest::Client,
}

impl Default for ServerUpdater {
    fn default() -> Self {
        Self::new()
    }
}

impl ServerUpdater {
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(30))
                .build()
                .unwrap(),
        }
    }

    pub async fn check_update(&self) -> Result<Option<ServerUpdateManifest>, String> {
        let resp = self.client.get(SERVER_UPDATE_URL)
            .send()
            .await
            .map_err(|e| format!("Failed to check update: {e}"))?;

        if !resp.status().is_success() {
            return Ok(None);
        }

        let manifest: ServerUpdateManifest = resp.json().await
            .map_err(|e| format!("Invalid update manifest: {e}"))?;

        let current = semver::Version::parse(CURRENT_SERVER_VERSION).map_err(|e| e.to_string())?;
        let target = semver::Version::parse(&manifest.version).map_err(|e| e.to_string())?;

        if target > current {
            Ok(Some(manifest))
        } else {
            Ok(None)
        }
    }

    pub async fn apply_update(&self, manifest: &ServerUpdateManifest) -> Result<(), String> {
        let platform_key = if cfg!(target_os = "windows") {
            "windows-x86_64"
        } else if cfg!(target_os = "linux") {
            "linux-x86_64"
        } else if cfg!(target_os = "macos") {
            "macos-x86_64"
        } else {
            return Err("Unsupported OS platform for auto-update".into());
        };

        let artifact = manifest.platforms.get(platform_key)
            .ok_or_else(|| format!("No release available for platform {platform_key}"))?;

        if !ssrf::is_safe_cdn_url(&artifact.url) {
            return Err(format!("Untrusted update source host: {}", artifact.url));
        }

        let bytes = self.client.get(&artifact.url)
            .send()
            .await
            .map_err(|e| format!("Download failed: {e}"))?
            .bytes()
            .await
            .map_err(|e| format!("Read failed: {e}"))?;

        let mut hasher = Sha256::new();
        hasher.update(&bytes);
        let calculated_hash = hex::encode(hasher.finalize());
        if !calculated_hash.eq_ignore_ascii_case(&artifact.sha256) {
            return Err(format!(
                "Checksum mismatch! Expected {}, got {}",
                artifact.sha256, calculated_hash
            ));
        }

        let cursor = Cursor::new(bytes);
        let mut zip = zip::ZipArchive::new(cursor).map_err(|e| format!("Corrupt zip: {e}"))?;
        let mut new_bin_bytes = Vec::new();
        let mut file = zip.by_name(&artifact.bin_name)
            .map_err(|_| format!("Binary '{}' not found inside archive", artifact.bin_name))?;
        file.read_to_end(&mut new_bin_bytes).map_err(|e| e.to_string())?;

        let temp_bin_path = std::env::temp_dir().join(format!("zircon_update_{}", manifest.version));
        std::fs::write(&temp_bin_path, &new_bin_bytes).map_err(|e| e.to_string())?;

        self_replace::self_replace(&temp_bin_path)
            .map_err(|e| format!("Failed to swap executable: {e}"))?;
        let _ = std::fs::remove_file(temp_bin_path);

        tracing::info!("Server binary updated to v{}.", manifest.version);
        Ok(())
    }

    pub fn restart_process() -> Result<(), std::io::Error> {
        let current_exe = env::current_exe()?;
        let args: Vec<String> = env::args().skip(1).collect();

        Command::new(current_exe)
            .args(&args)
            .spawn()?;

        std::process::exit(0);
    }
}
```

---

### [x] Step 5.2: Create System Controller & App State Wiring
**File:** `zircon-server/src/web/controllers/system_controller.rs`

```rust
use axum::extract::State;
use axum::Json;
use serde_json::json;

use crate::updater::{ServerUpdater, CURRENT_SERVER_VERSION};
use crate::web::app::{ApiError, AppState};

/// GET /api/system/update/check
pub async fn check_update() -> Result<Json<serde_json::Value>, ApiError> {
    let updater = ServerUpdater::new();
    let update = updater.check_update().await.map_err(ApiError::Internal)?;
    Ok(Json(json!({
        "currentVersion": CURRENT_SERVER_VERSION,
        "updateAvailable": update.is_some(),
        "manifest": update
    })))
}

/// POST /api/system/update/apply
pub async fn apply_update(
    State(state): State<AppState>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let updater = ServerUpdater::new();
    let Some(manifest) = updater.check_update().await.map_err(ApiError::Internal)? else {
        return Err(ApiError::BadRequest("No updates available".into()));
    };

    state.audit.log("ADMIN", "SERVER_UPDATE_APPLY", &format!("Target version: {}", manifest.version));

    for inst in state.instances.list_instances() {
        state.instances.stop_instance(&inst.id).await;
    }

    updater.apply_update(&manifest).await.map_err(ApiError::Internal)?;

    tokio::spawn(async move {
        tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
        let _ = ServerUpdater::restart_process();
    });

    Ok(Json(json!({ "ok": true, "message": "Server updated. Restarting..." })))
}
```

* In `zircon-server/src/lib.rs`, register `pub mod updater;`.
* In `zircon-server/src/web/controllers/mod.rs`, register `pub mod system_controller;`.
* In `zircon-server/src/web/app.rs`, add `audit: Arc<AuditLogger>` to `AppState`, instantiate it in `main.rs`, and mount:
  ```rust
  .route("/api/system/update/check", get(system_controller::check_update))
  .route("/api/system/update/apply", post(system_controller::apply_update))
  .route("/api/auth/2fa/setup", post(auth_controller::setup_2fa))
  .route("/api/auth/2fa/enable", post(auth_controller::enable_2fa))
  .route("/api/auth/2fa/disable", post(auth_controller::disable_2fa))
  ```

---

## Phase 6: Client Launcher Hardening & Auto-Update (`zircon-launcher`)

### [x] Step 6.1: Update `zircon-launcher/Cargo.toml`
```toml
[dependencies]
tauri-plugin-updater = "2"
tauri-plugin-process = "2"
zeroize = { version = "1.8", features = ["derive"] }
```

---

### [x] Step 6.2: Apply Memory Zeroization to Sensitive Session Tokens
**File:** `zircon-launcher/src/auth/session.rs`

```rust
use serde::{Deserialize, Deserializer, Serialize};
use zeroize::Zeroize;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Zeroize)]
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
    #[zeroize(skip)]
    pub user_type: String,
}
```

---

### [x] Step 6.3: Validate Server IP and Scrub Subprocess Environment
**File:** `zircon-launcher/src/launch/runner.rs`

```rust
// Validate server_ip syntax before building launch args
if let Some(host) = server_ip {
    if !host.is_empty() && url::Host::parse(host).is_err() {
        return Err(LauncherError::InvalidInput(format!("Invalid server address: {host}")));
    }
}
```

---

### [x] Step 6.4: Register Tauri Plugins in `zircon-launcher/src/lib.rs`
```rust
tauri::Builder::default()
    .plugin(tauri_plugin_dialog::init())
    .plugin(tauri_plugin_updater::Builder::new().build())
    .plugin(tauri_plugin_process::init())
    .manage(commands::LauncherState::new())
```

---

### [x] Step 6.5: Configure Tauri Updater & Vue UI Hook
1. In `zircon-launcher/ui/package.json`, add:
   ```json
   "@tauri-apps/plugin-updater": "^2.0.0",
   "@tauri-apps/plugin-process": "^2.0.0"
   ```
2. In `zircon-launcher/src-tauri/tauri.conf.json`:
   ```json
   {
     "bundle": {
       "createUpdaterArtifacts": true
     },
     "plugins": {
       "updater": {
         "pubkey": "<YOUR_GENERATED_PUBLIC_KEY>",
         "endpoints": [
           "https://zirconmc.net/updates/launcher/latest.json"
         ]
       }
     }
   }
   ```

---

## Phase 7: Build Automation & Manifest Generation (`build.bat`)

**File:** `build.bat`

```bat
@echo off
setlocal enabledelayedexpansion
cd /d "%~dp0"

set VERSION=0.1.0
set DOMAIN=https://zirconmc.net

echo.
echo === [1/3] Building Server Release ===
cargo build --release -p zircon-server
if errorlevel 1 (
    echo FAILED: server build
    exit /b 1
)

echo.
echo === [2/3] Creating Server Archive & Manifest ===
if not exist "dist-run\zircon-server" mkdir "dist-run\zircon-server"
copy /Y "target\release\zircon-server.exe" "dist-run\zircon-server\zircon-server.exe"
if not exist "dist-run\zircon-server\server-data\.keep" mkdir "dist-run\zircon-server\server-data\.keep"
> "dist-run\zircon-server\server-data\.keep\readme.txt" echo placeholder

if exist "dist-run\zircon-server-windows-x86_64.zip" del /Q "dist-run\zircon-server-windows-x86_64.zip"
powershell -NoProfile -Command "Compress-Archive -Path dist-run/zircon-server/* -DestinationPath dist-run/zircon-server-windows-x86_64.zip -Force"

for /f %%i in ('certutil -hashfile dist-run\zircon-server-windows-x86_64.zip SHA256 ^| findstr /v "hash"') do set HASH=%%i
set HASH=%HASH: =%

(
  echo {
  echo   "version": "%VERSION%",
  echo   "releaseDate": "%DATE% %TIME%",
  echo   "notes": "Zircon Server Release v%VERSION%",
  echo   "platforms": {
  echo     "windows-x86_64": {
  echo       "url": "%DOMAIN%/updates/server/v%VERSION%/zircon-server-windows-x86_64.zip",
  echo       "sha256": "%HASH%",
  echo       "binName": "zircon-server.exe"
  echo     }
  echo   }
  echo }
) > dist-run\server-latest.json

echo.
echo === [3/3] Building Tauri Launcher with Updater Artifacts ===
cd /d "%~dp0zircon-launcher"
call npx --yes @tauri-apps/cli build
if errorlevel 1 (
    echo FAILED: launcher build
    exit /b 1
)
cd /d "%~dp0"

echo.
echo ===========================================================================
echo Build completed successfully!
echo Upload artifacts to your Cloudflare R2 bucket:
echo   dist-run\server-latest.json                      -> /updates/server/latest.json
echo   dist-run\zircon-server-windows-x86_64.zip        -> /updates/server/v%VERSION%/zircon-server-windows-x86_64.zip
echo   target\release\bundle\updater\latest.json        -> /updates/launcher/latest.json
echo   target\release\bundle\msi\*.zip / *.sig          -> /updates/launcher/
echo   target\release\bundle\nsis\*.zip / *.sig         -> /updates/launcher/
echo ===========================================================================
endlocal
```

---

## Phase 8: Verification, Tests, Commit & Push

1. [ ] Run all workspace tests:
   ```bash
   cargo test --workspace
   ```
2. [ ] Verify release build compilation:
   ```bash
   cargo check --workspace --release
   ```
3. [ ] Stage and commit changes:
   ```bash
   git add zircon-core/ zircon-server/ zircon-launcher/ build.bat
   git commit -m "security: full architectural zero-trust hardening, 2FA, and r2 auto-updater"
   ```
4. [ ] Push to remote repository:
   ```bash
   git push origin HEAD
   ```

---

## Post-Execution Setup

1. **Generate Tauri Signing Key**:
   ```bash
   npx @tauri-apps/cli signer generate -w ~/.tauri/zircon.key
   ```
   Paste the output public key into `zircon-launcher/src-tauri/tauri.conf.json`.
2. **Set Build Environment**:
   ```cmd
   set TAURI_SIGNING_PRIVATE_KEY=<YOUR_PRIVATE_KEY>
   ```
3. **Execute `build.bat`** and upload artifacts to the `/updates/` prefix in your R2 bucket.