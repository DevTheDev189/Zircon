//! Microsoft OAuth 2.0 authentication: interactive PKCE login, the
//! XBL → XSTS → Minecraft token chain, session persistence and silent refresh.
//!
//! The client id is resolved from the `MC_MANAGER_CLIENT_ID` env var, then the
//! `~/.mcmanager/client_id.txt` file, then the embedded default. [`MicrosoftAuthService::login`]
//! refuses to start the OAuth flow while the resolved id is the
//! `REPLACE_WITH_AZURE_CLIENT_ID` sentinel.
//!
//! Port of `com.mcmanager.client.auth.MicrosoftAuthService`.

use std::path::PathBuf;
use std::time::Duration;

use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use serde_json::json;
use sha2::{Digest, Sha256};
use url::form_urlencoded::Serializer;

use crate::auth::callback::CallbackServer;
use crate::auth::session::{now_millis, SessionData};
use crate::error::LauncherError;
use crate::paths;

/// The Azure client id embedded in the binary so login works out of the box.
/// OAuth client ids for public clients are not secrets, so this is a plain
/// constant; the `MC_MANAGER_CLIENT_ID` env var and the
/// `~/.mcmanager/client_id.txt` file still override it.
pub const EMBEDDED_CLIENT_ID: &str = "37f881f0-0083-45af-b2c4-52a658fec513";

/// Sentinel value meaning "no real client id configured yet".
/// [`MicrosoftAuthService::login`] refuses to start the OAuth flow while the
/// resolved id equals this value.
pub const DEFAULT_CLIENT_ID: &str = "REPLACE_WITH_AZURE_CLIENT_ID";

/// Fixed redirect URI used by the non-interactive flows (silent refresh); the
/// interactive login uses a dynamically selected localhost port instead.
const REDIRECT_URI: &str = "http://localhost:8080/callback";

const AUTH_URL: &str = "https://login.live.com/oauth20_authorize.srf";
const TOKEN_URL: &str = "https://login.live.com/oauth20_token.srf";
const XBL_URL: &str = "https://user.auth.xboxlive.com/user/authenticate";
const XSTS_URL: &str = "https://xsts.auth.xboxlive.com/xsts/authorize";
const MC_LOGIN_URL: &str = "https://api.minecraftservices.com/authentication/login_with_xbox";
const MC_ENTITLEMENTS_URL: &str = "https://api.minecraftservices.com/entitlements/mcstore";
const MC_PROFILE_URL: &str = "https://api.minecraftservices.com/minecraft/profile";

/// Endpoints whose responses carry bearer tokens. Their bodies are never
/// logged or embedded in error messages, so tokens cannot leak into logs or
/// bug reports.
const SECRET_ENDPOINTS: [&str; 4] = [TOKEN_URL, XBL_URL, XSTS_URL, MC_LOGIN_URL];

/// Microsoft OAuth 2.0 PKCE authentication against login.live.com plus the
/// Xbox/Minecraft token chain and the persisted session cache.
///
/// The session (Minecraft access token + Microsoft refresh token) is stored in
/// the operating system's keychain (DPAPI / Keychain / kernel keyring) so other
/// local users and unrelated processes cannot read it; when the keychain is
/// unavailable it falls back to an owner-only file, and legacy plaintext files
/// are migrated to the keychain on first use.
#[derive(Debug)]
pub struct MicrosoftAuthService {
    client_id: String,
    cache_file: PathBuf,
    http: reqwest::Client,
    /// Prefer the OS keychain over the plaintext cache file. Disabled for the
    /// test constructor so tests stay hermetic (no real keychain access).
    use_keyring: bool,
}

impl MicrosoftAuthService {
    /// Resolves the client id (env var → `client_id.txt` → embedded default)
    /// and uses the default cache file (`~/.mcmanager/auth_cache.json`).
    pub fn new() -> Self {
        Self::new_with_client_id(resolve_client_id())
    }

    /// Uses the given client id with the default cache file.
    pub fn new_with_client_id(client_id: String) -> Self {
        Self::new_with_options(client_id, paths::auth_cache_file(), true)
    }

    /// Full constructor for tests: explicit client id and cache file, with the
    /// OS keychain disabled so tests never touch the real credential store.
    pub fn new_with_paths(client_id: String, cache_file: PathBuf) -> Self {
        Self::new_with_options(client_id, cache_file, false)
    }

    fn new_with_options(client_id: String, cache_file: PathBuf, use_keyring: bool) -> Self {
        // `build()` only fails on invalid configuration; the values below are
        // static, so the unwrap is unreachable in practice.
        let http = reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(20))
            .redirect(reqwest::redirect::Policy::limited(10))
            .build()
            .expect("failed to build HTTP client");
        MicrosoftAuthService {
            client_id,
            cache_file,
            http,
            use_keyring,
        }
    }
}

impl Default for MicrosoftAuthService {
    fn default() -> Self {
        Self::new()
    }
}

impl MicrosoftAuthService {
    /// Runs the full interactive browser flow and returns the authenticated
    /// session. Uses PKCE (S256) with a dynamically selected localhost port so
    /// concurrent launchers never fight over a fixed callback port.
    pub async fn login(&self) -> Result<SessionData, LauncherError> {
        if self.client_id == DEFAULT_CLIENT_ID {
            return Err(LauncherError::Auth(format!(
                "Microsoft client id not configured (resolved to the \
                 REPLACE_WITH_AZURE_CLIENT_ID placeholder). The launcher bundles a \
                 working client id — unset MC_MANAGER_CLIENT_ID and delete {} to use \
                 it, or set either one to a real Azure client id. The Azure app must \
                 allow localhost redirect URIs (http://localhost:<port>/callback).",
                paths::client_id_file().display()
            )));
        }

        // PKCE: the code verifier is a one-time secret; only its S256 challenge
        // is sent in the authorize URL, and the verifier is sent at token exchange.
        let code_verifier = Self::generate_code_verifier();
        let code_challenge = Self::generate_code_challenge(&code_verifier);
        // `state` binds the browser redirect back to this exact flow, so a
        // stale callback or a malicious local process cannot feed us a code.
        let state = Self::generate_state();

        let mut server = CallbackServer::start().await?;
        let redirect_uri = format!("http://localhost:{}/callback", server.port());
        let authorize_url = self.build_authorize_url(&redirect_uri, &code_challenge, &state);

        tracing::info!(
            "Opening browser for Microsoft login (client_id={}, redirect_uri={})",
            self.client_id,
            redirect_uri
        );
        tracing::debug!("Authorize URL: {authorize_url}");
        if let Err(e) = open::that(&authorize_url) {
            return Err(LauncherError::Auth(format!(
                "Could not open the browser automatically; open this URL manually:\n{authorize_url}\n({e})"
            )));
        }

        let code = server
            .await_code(Duration::from_secs(5 * 60), &state)
            .await?;
        self.complete_login(&code, Some(&code_verifier), &redirect_uri)
            .await
    }

    /// Continues the flow after the browser callback: MS token → XBL → XSTS →
    /// Minecraft token → profile. Persists the session to disk.
    pub async fn complete_login(
        &self,
        auth_code: &str,
        code_verifier: Option<&str>,
        redirect_uri: &str,
    ) -> Result<SessionData, LauncherError> {
        tracing::debug!("Step 1/5: exchanging auth code for Microsoft token...");
        let ms = self
            .exchange_code_for_ms_token(auth_code, code_verifier, redirect_uri)
            .await?;
        self.complete_login_with_ms_token(ms).await
    }

    /// Attempts to renew an expired session using its Microsoft refresh token.
    pub async fn refresh(&self, session: &SessionData) -> Result<SessionData, LauncherError> {
        if session.refresh_token.trim().is_empty() {
            return Err(LauncherError::Auth(
                "No refresh token available".to_string(),
            ));
        }
        let body = form(&[
            ("client_id", self.client_id.as_str()),
            ("redirect_uri", REDIRECT_URI),
            ("grant_type", "refresh_token"),
            ("refresh_token", session.refresh_token.as_str()),
            ("scope", "XboxLive.signin offline_access"),
        ]);
        let json = self
            .post_json(TOKEN_URL, &body, "application/x-www-form-urlencoded")
            .await?;
        let access_token = get_string(&json, "access_token").ok_or_else(|| {
            LauncherError::Auth("Token refresh failed: response missing access_token".to_string())
        })?;
        let refresh_token = get_string(&json, "refresh_token").unwrap_or_default();
        tracing::debug!("Token refresh OK");
        self.complete_login_with_ms_token(MsTokenResponse {
            access_token,
            refresh_token,
        })
        .await
    }

    /// Runs the shared MS token → XBL → XSTS → Minecraft login → profile chain
    /// and persists the resulting session.
    async fn complete_login_with_ms_token(
        &self,
        ms: MsTokenResponse,
    ) -> Result<SessionData, LauncherError> {
        tracing::debug!("Step 2/5: XBL authenticate...");
        let xbl_token = self.xbl_authenticate(&ms.access_token).await?;

        tracing::debug!("Step 3/5: XSTS authorize...");
        let xsts = self.xsts_authorize(&xbl_token).await?;

        tracing::debug!("Step 4/5: Minecraft login...");
        let identity_token = format!("XBL3.0 x={};{}", xsts.uhs, xsts.token);
        let mc = self.minecraft_login(&identity_token).await?;

        tracing::debug!("Step 5/5: fetching Minecraft profile...");
        let profile = self.fetch_profile(&mc.access_token).await?;

        let session = SessionData {
            access_token: mc.access_token,
            refresh_token: ms.refresh_token,
            username: profile.name,
            uuid: profile.id,
            expires_at_millis: now_millis() + mc.expires_in * 1000,
            user_type: "msa".to_string(),
        };
        self.save(&session)?;
        tracing::info!("Signed in as {}", session.username);
        Ok(session)
    }

    /// Loads the cached session, or `None` when the cache is missing, invalid
    /// (dummy token or non-msa session) or unreadable. Invalid entries are
    /// deleted so a broken cache never blocks a fresh login.
    ///
    /// The OS keychain is authoritative when it holds an entry; otherwise the
    /// legacy plaintext file is consulted, and a valid file entry is migrated
    /// into the keychain.
    pub fn load_cached(&self) -> Option<SessionData> {
        if self.use_keyring {
            if let Ok(entry) = Self::keyring_entry() {
                match entry.get_password() {
                    Ok(json) => match serde_json::from_str::<SessionData>(&json) {
                        Ok(data) if data.is_valid() => return Some(data),
                        Ok(_) => {
                            tracing::warn!("Ignoring invalid session in the OS keychain");
                            let _ = entry.delete_credential();
                            return None;
                        }
                        Err(e) => {
                            tracing::warn!("Could not parse the OS keychain session: {e}");
                            let _ = entry.delete_credential();
                            return None;
                        }
                    },
                    Err(keyring::Error::NoEntry) => { /* fall through to the file */ }
                    Err(e) => {
                        tracing::warn!(
                            "Could not read the session from the OS keychain ({e}); trying the cache file"
                        );
                    }
                }
            }
        }
        self.load_cache_file()
    }

    /// Loads the legacy plaintext cache file. A valid entry is migrated into
    /// the keychain (and the file removed) when keychain storage is enabled.
    fn load_cache_file(&self) -> Option<SessionData> {
        if !self.cache_file.is_file() {
            return None;
        }
        match std::fs::read_to_string(&self.cache_file) {
            Ok(content) => match serde_json::from_str::<SessionData>(&content) {
                Ok(data) => {
                    if data.is_valid() {
                        if self.use_keyring {
                            // Migrate the legacy cache into the keychain now
                            // that it is available.
                            let _ = self.save(&data);
                        }
                        Some(data)
                    } else {
                        tracing::warn!(
                            "Ignoring invalid auth cache (missing/dummy token or non-msa session)"
                        );
                        let _ = std::fs::remove_file(&self.cache_file);
                        None
                    }
                }
                Err(e) => {
                    tracing::warn!("Could not parse auth cache: {e}");
                    let _ = std::fs::remove_file(&self.cache_file);
                    None
                }
            },
            Err(e) => {
                tracing::warn!("Could not read auth cache: {e}");
                None
            }
        }
    }

    /// Persists the session: into the OS keychain when available, otherwise
    /// into an owner-only cache file. The session holds the Minecraft access
    /// token and the Microsoft refresh token, so a plaintext copy is avoided
    /// whenever the platform keychain is present.
    pub fn save(&self, session: &SessionData) -> Result<(), LauncherError> {
        let json = serde_json::to_string(session)?;
        if self.use_keyring {
            match Self::keyring_entry() {
                Ok(entry) => match entry.set_password(&json) {
                    Ok(()) => {
                        // Keychain is authoritative — drop any legacy plaintext copy.
                        let _ = std::fs::remove_file(&self.cache_file);
                        return Ok(());
                    }
                    Err(e) => {
                        tracing::warn!(
                            "Could not store the session in the OS keychain ({e}); falling back to a protected cache file"
                        );
                    }
                },
                Err(e) => {
                    tracing::warn!(
                        "Could not access the OS keychain ({e}); falling back to a protected cache file"
                    );
                }
            }
        }
        self.write_cache_file(&json)
    }

    /// Writes the session JSON to the cache file, restricted to the current
    /// user where the OS supports it.
    fn write_cache_file(&self, json: &str) -> Result<(), LauncherError> {
        if let Some(parent) = self.cache_file.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&self.cache_file, json)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            // Owner read/write only: keeps other local users (and malware
            // running as them) from reading the refresh token.
            let _ =
                std::fs::set_permissions(&self.cache_file, std::fs::Permissions::from_mode(0o600));
        }
        Ok(())
    }

    /// Deletes the cached session everywhere (keychain and legacy file); no
    /// error when nothing was stored.
    pub fn clear_cache(&self) -> Result<(), LauncherError> {
        if self.use_keyring {
            if let Ok(entry) = Self::keyring_entry() {
                let _ = entry.delete_credential();
            }
        }
        match std::fs::remove_file(&self.cache_file) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(e.into()),
        }
    }

    /// The keychain entry holding the session JSON. The service name is
    /// launcher-wide; a single user slot holds the one active session.
    fn keyring_entry() -> Result<keyring::Entry, LauncherError> {
        keyring::Entry::new("zircon-launcher", "auth-session")
            .map_err(|e| LauncherError::Auth(format!("Could not access the OS keychain: {e}")))
    }

    /// Returns `true` if the account owns Minecraft (best effort — never
    /// aborts login; any transport/parse failure reports `true` so the
    /// launcher never blocks on it).
    pub async fn check_entitlements(&self, mc_access_token: &str) -> bool {
        let response = match self
            .http
            .get(MC_ENTITLEMENTS_URL)
            .header(
                reqwest::header::AUTHORIZATION,
                format!("Bearer {mc_access_token}"),
            )
            .send()
            .await
        {
            Ok(response) => response,
            Err(e) => {
                tracing::warn!("Entitlements check failed: {e}");
                return true;
            }
        };
        match response.json::<serde_json::Value>().await {
            Ok(json) => json
                .get("items")
                .and_then(|items| items.as_array())
                .is_some_and(|items| !items.is_empty()),
            Err(e) => {
                tracing::warn!("Entitlements check failed: {e}");
                true
            }
        }
    }

    /// Builds the OAuth authorize URL. `code_challenge_method=S256` is fixed;
    /// the `scope` is `XboxLive.signin offline_access`, `prompt=login` forces a
    /// fresh interactive login, and `state` binds the redirect back to this
    /// flow so the callback can reject forged or stale requests.
    pub fn build_authorize_url(
        &self,
        redirect_uri: &str,
        code_challenge: &str,
        state: &str,
    ) -> String {
        format!(
            "{AUTH_URL}?client_id={}&response_type=code&redirect_uri={}&scope={}&code_challenge={}&code_challenge_method=S256&prompt=login&state={}",
            urlencode(&self.client_id),
            urlencode(redirect_uri),
            urlencode("XboxLive.signin offline_access"),
            urlencode(code_challenge),
            urlencode(state),
        )
    }

    /// 64 random chars from the RFC 7636 unreserved alphabet.
    pub fn generate_code_verifier() -> String {
        Self::random_alphabet_chars(64)
    }

    /// 32 random chars used as the OAuth `state` (defends the callback against
    /// forged or replayed redirects).
    pub fn generate_state() -> String {
        Self::random_alphabet_chars(32)
    }

    fn random_alphabet_chars(len: usize) -> String {
        const ALPHABET: &[u8] =
            b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-._~";
        let mut bytes = vec![0u8; len];
        getrandom::fill(&mut bytes).expect("failed to obtain randomness for the OAuth flow");
        bytes
            .iter()
            .map(|b| ALPHABET[(*b as usize) % ALPHABET.len()] as char)
            .collect()
    }

    /// S256 challenge = base64url(sha256(verifier)), unpadded.
    pub fn generate_code_challenge(verifier: &str) -> String {
        let digest = Sha256::digest(verifier.as_bytes());
        URL_SAFE_NO_PAD.encode(digest)
    }

    // ------------------------------------------------------------------
    // Token exchange steps
    // ------------------------------------------------------------------

    async fn exchange_code_for_ms_token(
        &self,
        code: &str,
        code_verifier: Option<&str>,
        redirect_uri: &str,
    ) -> Result<MsTokenResponse, LauncherError> {
        let mut pairs: Vec<(&str, &str)> = vec![
            ("client_id", self.client_id.as_str()),
            ("redirect_uri", redirect_uri),
            ("grant_type", "authorization_code"),
            ("code", code),
            ("scope", "XboxLive.signin offline_access"),
        ];
        if let Some(verifier) = code_verifier.filter(|v| !v.trim().is_empty()) {
            pairs.push(("code_verifier", verifier));
        }
        let body = form(&pairs);
        let json = self
            .post_json(TOKEN_URL, &body, "application/x-www-form-urlencoded")
            .await?;
        let access_token = get_string(&json, "access_token").ok_or_else(|| {
            LauncherError::Auth(
                "OAuth token exchange failed: response missing access_token".to_string(),
            )
        })?;
        let refresh_token = get_string(&json, "refresh_token").unwrap_or_default();
        tracing::debug!(
            "OAuth token exchange OK (refresh_token present: {})",
            !refresh_token.is_empty()
        );
        Ok(MsTokenResponse {
            access_token,
            refresh_token,
        })
    }

    async fn xbl_authenticate(&self, ms_access_token: &str) -> Result<String, LauncherError> {
        let body = json!({
            "RelyingParty": "http://auth.xboxlive.com",
            "TokenType": "JWT",
            "Properties": {
                "AuthMethod": "RPS",
                "SiteName": "user.auth.xboxlive.com",
                "RpsTicket": format!("d={ms_access_token}"),
            }
        });
        let json = self
            .post_json(XBL_URL, &body.to_string(), "application/json")
            .await?;
        let token = require(&json, "Token", "XBL authenticate")?;
        tracing::debug!("XBL authenticate OK");
        Ok(token)
    }

    async fn xsts_authorize(&self, xbl_token: &str) -> Result<XstsResponse, LauncherError> {
        let body = json!({
            "RelyingParty": "rp://api.minecraftservices.com/",
            "TokenType": "JWT",
            "Properties": {
                "SandboxId": "RETAIL",
                "UserTokens": [xbl_token],
            }
        });
        let json = self
            .post_json(XSTS_URL, &body.to_string(), "application/json")
            .await?;
        let token = require(&json, "Token", "XSTS authorize")?;
        let uhs = json
            .get("DisplayClaims")
            .and_then(|d| d.get("xui"))
            .and_then(|xui| xui.as_array())
            .and_then(|xui| xui.first())
            .and_then(|first| first.get("uhs"))
            .and_then(|uhs| uhs.as_str())
            .map(str::to_string)
            .ok_or_else(|| {
                LauncherError::Auth("XSTS response missing user hash (uhs)".to_string())
            })?;
        tracing::debug!("XSTS authorize OK (uhs={uhs})");
        Ok(XstsResponse { token, uhs })
    }

    async fn minecraft_login(
        &self,
        identity_token: &str,
    ) -> Result<McLoginResponse, LauncherError> {
        let body = json!({ "identityToken": identity_token });
        let (status, text) = self
            .post(MC_LOGIN_URL, &body.to_string(), "application/json")
            .await?;
        if !(200..300).contains(&status) {
            let raw = format!(
                "POST {MC_LOGIN_URL} failed: HTTP {status} {}",
                truncate(&text)
            );
            if raw.contains("Invalid app registration") {
                return Err(LauncherError::Auth(
                    "Minecraft rejected the login with 'Invalid app registration'. \
                     Two things must be true: (1) the Microsoft account owns Minecraft Java Edition, \
                     and (2) the Azure client ID is approved by Minecraft for authentication. \
                     If the account is correct, submit your client ID for review at \
                     https://aka.ms/mce-reviewappid — once approved (you receive an email), \
                     login works with no code change."
                        .to_string(),
                ));
            }
            return Err(LauncherError::Http {
                status,
                url: MC_LOGIN_URL.to_string(),
            });
        }
        let json: serde_json::Value = serde_json::from_str(&text).map_err(LauncherError::from)?;
        let access_token = require(&json, "access_token", "Minecraft login")?;
        let expires_in = json
            .get("expires_in")
            .and_then(|v| v.as_i64())
            .unwrap_or(86_400);
        tracing::debug!("Minecraft login OK (expires_in={expires_in}s)");
        Ok(McLoginResponse {
            access_token,
            expires_in,
        })
    }

    async fn fetch_profile(&self, mc_access_token: &str) -> Result<ProfileResponse, LauncherError> {
        let response = self
            .http
            .get(MC_PROFILE_URL)
            .header(
                reqwest::header::AUTHORIZATION,
                format!("Bearer {mc_access_token}"),
            )
            .send()
            .await?;
        let status = response.status().as_u16();
        let text = response.text().await?;
        tracing::debug!("GET {MC_PROFILE_URL} -> HTTP {status}: {}", truncate(&text));
        if status != 200 {
            return Err(LauncherError::Http {
                status,
                url: MC_PROFILE_URL.to_string(),
            });
        }
        let json: serde_json::Value = serde_json::from_str(&text).map_err(LauncherError::from)?;
        let id = json.get("id").and_then(|v| v.as_str()).map(str::to_string);
        let name = json
            .get("name")
            .and_then(|v| v.as_str())
            .map(str::to_string);
        match (id, name) {
            (Some(id), Some(name)) => {
                tracing::debug!("Minecraft profile OK (id={id}, name={name})");
                Ok(ProfileResponse { id, name })
            }
            _ => Err(LauncherError::Auth(format!(
                "Minecraft profile response missing 'id'/'name' (does the account own Minecraft?): {}",
                truncate(&text)
            ))),
        }
    }

    // ------------------------------------------------------------------
    // HTTP helpers
    // ------------------------------------------------------------------

    /// POSTs `body` with `content_type` and returns `(status, response text)`
    /// regardless of status so callers can surface API error bodies. Response
    /// bodies of the token-bearing endpoints are redacted from logs.
    async fn post(
        &self,
        url: &str,
        body: &str,
        content_type: &str,
    ) -> Result<(u16, String), LauncherError> {
        let response = self
            .http
            .post(url)
            .header(reqwest::header::CONTENT_TYPE, content_type)
            .header(reqwest::header::ACCEPT, "application/json")
            .body(body.to_string())
            .send()
            .await?;
        let status = response.status().as_u16();
        let text = response.text().await?;
        if SECRET_ENDPOINTS.contains(&url) {
            tracing::debug!("POST {url} -> HTTP {status} (body redacted)");
        } else {
            tracing::debug!("POST {url} -> HTTP {status}: {}", truncate(&text));
        }
        Ok((status, text))
    }

    /// POSTs `body` and parses a 2xx response body as JSON.
    async fn post_json(
        &self,
        url: &str,
        body: &str,
        content_type: &str,
    ) -> Result<serde_json::Value, LauncherError> {
        let (status, text) = self.post(url, body, content_type).await?;
        if !(200..300).contains(&status) {
            return Err(LauncherError::Http {
                status,
                url: url.to_string(),
            });
        }
        if text.trim().is_empty() {
            return Err(LauncherError::Auth(format!(
                "POST {url} returned an empty response body"
            )));
        }
        serde_json::from_str(&text).map_err(LauncherError::from)
    }
}

/// Resolves the client id: `MC_MANAGER_CLIENT_ID` env var, then the
/// `~/.mcmanager/client_id.txt` file, then the embedded default.
fn resolve_client_id() -> String {
    if let Ok(id) = std::env::var("MC_MANAGER_CLIENT_ID") {
        if !id.trim().is_empty() {
            return id;
        }
    }
    if let Ok(content) = std::fs::read_to_string(paths::client_id_file()) {
        let from_file = content.trim();
        if !from_file.is_empty() {
            return from_file.to_string();
        }
    }
    EMBEDDED_CLIENT_ID.to_string()
}

/// Extracts an optional string field.
fn get_string(json: &serde_json::Value, field: &str) -> Option<String> {
    json.get(field).and_then(|v| v.as_str()).map(str::to_string)
}

/// Extracts a required string field, mirroring the Java `require` helper. The
/// response body is deliberately NOT embedded in the error: the token-bearing
/// endpoints would leak credentials into error messages.
fn require(json: &serde_json::Value, field: &str, step: &str) -> Result<String, LauncherError> {
    get_string(json, field)
        .ok_or_else(|| LauncherError::Auth(format!("{step} failed: response missing '{field}'")))
}

/// Truncates long response bodies so error messages and debug logs stay readable.
fn truncate(s: &str) -> String {
    if s.len() > 500 {
        let mut out: String = s.chars().take(500).collect();
        out.push('…');
        out
    } else {
        s.to_string()
    }
}

/// Form-url-encodes key/value pairs (space → `+`, like Java's `URLEncoder`).
fn form(pairs: &[(&str, &str)]) -> String {
    let mut serializer = Serializer::new(String::new());
    for (key, value) in pairs {
        serializer.append_pair(key, value);
    }
    serializer.finish()
}

/// Percent-encodes a value for use in a URL query string (unreserved
/// characters `A-Za-z0-9-._~` pass through; space becomes `%20`).
fn urlencode(value: &str) -> String {
    url::form_urlencoded::byte_serialize(value.as_bytes()).collect()
}

/// Response DTOs for the token chain.
struct MsTokenResponse {
    access_token: String,
    refresh_token: String,
}

struct XstsResponse {
    token: String,
    uhs: String,
}

struct McLoginResponse {
    access_token: String,
    expires_in: i64,
}

struct ProfileResponse {
    id: String,
    name: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Creates an isolated cache path under the system temp dir.
    fn temp_cache() -> (PathBuf, PathBuf) {
        let dir = std::env::temp_dir().join(format!("zircon-msa-test-{}", uuid::Uuid::new_v4()));
        (dir.clone(), dir.join("auth_cache.json"))
    }

    fn valid_session() -> SessionData {
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
    fn build_authorize_url_contains_all_params() {
        let service = MicrosoftAuthService::new_with_client_id("abc123".to_string());
        let url = service.build_authorize_url(
            "http://localhost:45678/callback",
            "challenge-XYZ",
            "state-ABC",
        );
        assert!(url.starts_with("https://login.live.com/oauth20_authorize.srf?"));
        assert!(url.contains("client_id=abc123"));
        assert!(url.contains("response_type=code"));
        assert!(url.contains("code_challenge=challenge-XYZ"));
        assert!(url.contains("code_challenge_method=S256"));
        assert!(url.contains("prompt=login"));
        // redirect_uri/scope are percent-encoded; decode the query and check values.
        let params: Vec<(String, String)> = url::form_urlencoded::parse(
            url.split_once('?').map(|(_, q)| q).unwrap_or("").as_bytes(),
        )
        .into_owned()
        .collect();
        let get = |key: &str| {
            params
                .iter()
                .find(|(k, _)| k.as_str() == key)
                .map(|(_, v)| v.as_str())
        };
        assert_eq!(Some("abc123"), get("client_id"));
        assert_eq!(Some("code"), get("response_type"));
        assert_eq!(Some("http://localhost:45678/callback"), get("redirect_uri"));
        assert_eq!(Some("XboxLive.signin offline_access"), get("scope"));
        assert_eq!(Some("challenge-XYZ"), get("code_challenge"));
        assert_eq!(Some("S256"), get("code_challenge_method"));
        assert_eq!(Some("login"), get("prompt"));
        assert_eq!(Some("state-ABC"), get("state"));
    }

    #[test]
    fn state_has_expected_shape() {
        const ALPHABET: &str = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-._~";
        for _ in 0..8 {
            let state = MicrosoftAuthService::generate_state();
            assert_eq!(32, state.len());
            assert!(state.chars().all(|c| ALPHABET.contains(c)));
        }
    }

    #[test]
    fn cache_save_restricts_permissions_on_unix() {
        let (dir, cache) = temp_cache();
        let service =
            MicrosoftAuthService::new_with_paths("test-client".to_string(), cache.clone());
        service.save(&valid_session()).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(&cache).unwrap().permissions().mode();
            assert_eq!(0o600, mode & 0o777, "cache must be owner-only on unix");
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn code_challenge_matches_rfc_7636_example() {
        let verifier = "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk";
        assert_eq!(
            "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM",
            MicrosoftAuthService::generate_code_challenge(verifier)
        );
    }

    #[test]
    fn code_verifier_has_expected_shape() {
        const ALPHABET: &str = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-._~";
        for _ in 0..8 {
            let verifier = MicrosoftAuthService::generate_code_verifier();
            assert_eq!(64, verifier.len());
            assert!(verifier.chars().all(|c| ALPHABET.contains(c)));
        }
    }

    #[test]
    fn cache_save_load_round_trip() {
        let (dir, cache) = temp_cache();
        let service = MicrosoftAuthService::new_with_paths("test-client".to_string(), cache);
        let session = valid_session();
        service.save(&session).unwrap();
        assert_eq!(Some(session), service.load_cached());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn load_cached_returns_none_when_missing() {
        let (_dir, cache) = temp_cache();
        let service = MicrosoftAuthService::new_with_paths("test-client".to_string(), cache);
        assert!(service.load_cached().is_none());
    }

    #[test]
    fn load_cached_returns_none_for_garbage_file() {
        let (dir, cache) = temp_cache();
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(&cache, "this is not json {").unwrap();
        let service =
            MicrosoftAuthService::new_with_paths("test-client".to_string(), cache.clone());
        assert!(service.load_cached().is_none());
        assert!(!cache.exists(), "invalid cache file should be deleted");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn load_cached_rejects_invalid_session_and_deletes() {
        let (dir, cache) = temp_cache();
        std::fs::create_dir_all(&dir).unwrap();
        // (struct-update syntax can't move fields out of a `Drop` type, so the
        // invalid token is set in place)
        let mut session = valid_session();
        session.access_token = "0".to_string();
        std::fs::write(&cache, serde_json::to_string(&session).unwrap()).unwrap();
        let service =
            MicrosoftAuthService::new_with_paths("test-client".to_string(), cache.clone());
        assert!(service.load_cached().is_none());
        assert!(!cache.exists());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn clear_cache_tolerates_missing_file() {
        let (dir, cache) = temp_cache();
        let service =
            MicrosoftAuthService::new_with_paths("test-client".to_string(), cache.clone());
        service.clear_cache().unwrap(); // no file yet
        service.save(&valid_session()).unwrap();
        assert!(cache.exists());
        service.clear_cache().unwrap();
        assert!(!cache.exists());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
