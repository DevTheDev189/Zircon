//! Router-level integration tests for the Axum admin API.
//! Verifies the Phase 3 checkpoint: all REST routes compile and the JWT
//! middleware properly gates protected endpoints — public routes work without a
//! token, protected routes reject unauthenticated requests with 401, and a
//! valid login token unlocks them.

use std::sync::Arc;
use std::time::Duration;

use axum::body::Body;
use axum::http::{header, Request, StatusCode};
use axum::Router;
use http_body_util::BodyExt;
use serde_json::json;
use tower::ServiceExt;

use zircon_server::audit::AuditLogger;
use zircon_server::auth::auth_service::AuthService;
use zircon_server::auth::jwt;
use zircon_server::auth::sessions::SessionRegistry;
use zircon_server::config::ConfigService;
use zircon_server::instance::ServerInstanceManager;
use zircon_server::process::console::ConsoleStreamHandler;
use zircon_server::process::manager::MinecraftProcessManager;
use zircon_server::services::backup::BackupService;
use zircon_server::services::bom::BomService;
use zircon_server::services::mods::ModManagementService;
use zircon_server::services::packs::PackManagementService;
use zircon_server::services::resolver::ModServiceResolver;
use zircon_server::tickets::JoinTicketManager;
use zircon_server::web::app::{router, AppState};
use zircon_server::web::rate_limit::FixedWindowLimiter;

const ADMIN_PASSWORD: &str = "test-password-123";

/// Unique temp dir per test (integration tests can't reach the lib's
/// `#[cfg(test)] test_util` module, so this mirrors it locally).
fn temp_dir(prefix: &str) -> std::path::PathBuf {
    let thread = std::thread::current()
        .name()
        .unwrap_or("test")
        .replace("::", "-");
    let dir = std::env::temp_dir().join(format!("zircon-{prefix}-{thread}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// Builds a fully wired router over a throwaway temp data dir. The initial
/// admin account is created with a known password.
fn test_app() -> Router {
    test_app_with_limits(10)
}

/// Like `test_app` but with a configurable login rate-limit window cap.
fn test_app_with_limits(max_attempts: u32) -> Router {
    let dir = temp_dir("web-auth");
    let config = Arc::new(
        ConfigService::load_with_data_dir(Some(dir.display().to_string())).expect("config load"),
    );
    let auth = Arc::new(AuthService::initialize(&config.data_dir).expect("auth init"));
    jwt::initialize(&config.data_dir).expect("jwt init");
    auth.set_password("admin", ADMIN_PASSWORD)
        .expect("set password");

    let console = Arc::new(ConsoleStreamHandler::new());
    let process_manager = Arc::new(MinecraftProcessManager::legacy(
        config.clone(),
        console.clone(),
    ));
    let instances = Arc::new(
        ServerInstanceManager::new(&config.data_dir, console.clone()).expect("instance manager"),
    );
    let bom = Arc::new(BomService::new(config.bom_file.clone(), None));
    let mods = Arc::new(ModManagementService::new(
        bom.clone(),
        config.mods_dir.clone(),
        "",
    ));
    let packs = PackManagementService::new(
        bom.clone(),
        config.data_dir.join("shaderpacks"),
        config.data_dir.join("resourcepacks"),
    );
    let resolver = Arc::new(ModServiceResolver::new(
        instances.clone(),
        bom.clone(),
        mods.clone(),
        packs.clone(),
        "",
    ));
    let backup = Arc::new(BackupService::new(&config.data_dir, instances.clone()));
    let tickets = Arc::new(JoinTicketManager::new());
    let audit = Arc::new(AuditLogger::new(&config.data_dir));

    let state = AppState {
        config,
        auth,
        instances,
        console,
        process_manager,
        backup,
        bom,
        mods,
        packs,
        resolver,
        tickets,
        curseforge_api_key: String::new(),
        sessions: Arc::new(SessionRegistry::new()),
        login_limiter: Arc::new(FixedWindowLimiter::new(
            Duration::from_secs(60),
            max_attempts,
        )),
        audit,
    };
    router(state)
}

/// Sends one request through the router and returns the status plus the parsed
/// JSON body (or `Null` for empty bodies).
async fn send(
    app: &Router,
    method: &str,
    uri: &str,
    token: Option<&str>,
    body: Option<serde_json::Value>,
) -> (StatusCode, serde_json::Value) {
    let mut builder = Request::builder().method(method).uri(uri);
    if let Some(token) = token {
        builder = builder.header(header::AUTHORIZATION, format!("Bearer {token}"));
    }
    let request = match body {
        Some(value) => {
            builder = builder.header(header::CONTENT_TYPE, "application/json");
            builder
                .body(Body::from(value.to_string()))
                .expect("request body")
        }
        None => builder.body(Body::empty()).expect("request"),
    };
    let response = app.clone().oneshot(request).await.expect("router response");
    let status = response.status();
    let bytes = response
        .into_body()
        .collect()
        .await
        .expect("collect body")
        .to_bytes();
    let json = if bytes.is_empty() {
        serde_json::Value::Null
    } else {
        serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::Null)
    };
    (status, json)
}

/// Logs in with the known test credentials and returns the issued token.
async fn login(app: &Router) -> String {
    let (status, body) = send(
        app,
        "POST",
        "/api/auth/login",
        None,
        Some(json!({ "username": "admin", "password": ADMIN_PASSWORD })),
    )
    .await;
    assert_eq!(StatusCode::OK, status);
    body["token"].as_str().expect("login token").to_string()
}

/// Generates the current TOTP code for a base32 secret, mirroring the server's
/// parameters (SHA1, 6 digits, 30s step, "Zircon Server" issuer).
fn totp_code(secret: &str) -> String {
    let secret_bytes = totp_rs::Secret::Encoded(secret.to_string())
        .to_bytes()
        .expect("valid base32 secret");
    let totp = totp_rs::TOTP::new(
        totp_rs::Algorithm::SHA1,
        6,
        1,
        30,
        secret_bytes,
        Some("Zircon Server".to_string()),
        "admin".to_string(),
    )
    .expect("totp build");
    totp.generate_current().expect("totp code")
}

// ---------------------------------------------------------------------------
// Public routes (no admin token required)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn public_login_issues_token() {
    let app = test_app();

    // Wrong credentials are rejected.
    let (status, _) = send(
        &app,
        "POST",
        "/api/auth/login",
        None,
        Some(json!({ "username": "admin", "password": "wrong-password" })),
    )
    .await;
    assert_eq!(StatusCode::UNAUTHORIZED, status);

    // Correct credentials issue a token without any prior Authorization header.
    let (status, body) = send(
        &app,
        "POST",
        "/api/auth/login",
        None,
        Some(json!({ "username": "admin", "password": ADMIN_PASSWORD })),
    )
    .await;
    assert_eq!(StatusCode::OK, status);
    let token = body["token"].as_str().expect("token");
    assert!(!token.is_empty());
    assert_eq!("admin", body["username"]);
}

#[tokio::test]
async fn change_password_requires_auth_and_current_password() {
    let app = test_app();

    // Admin-only route now: without a token it is rejected before the handler.
    let (status, _) = send(
        &app,
        "POST",
        "/api/auth/change-password",
        None,
        Some(json!({
            "username": "admin",
            "currentPassword": ADMIN_PASSWORD,
            "newPassword": "new-password-456"
        })),
    )
    .await;
    assert_eq!(StatusCode::UNAUTHORIZED, status);

    // With a token but the wrong current password -> 401.
    let token = login(&app).await;
    let (status, _) = send(
        &app,
        "POST",
        "/api/auth/change-password",
        Some(&token),
        Some(json!({
            "username": "admin",
            "currentPassword": "wrong",
            "newPassword": "new-password-456"
        })),
    )
    .await;
    assert_eq!(StatusCode::UNAUTHORIZED, status);

    // Correct current password -> 200 and a fresh token (old sessions die).
    let (status, body) = send(
        &app,
        "POST",
        "/api/auth/change-password",
        Some(&token),
        Some(json!({
            "username": "admin",
            "currentPassword": ADMIN_PASSWORD,
            "newPassword": "new-password-456"
        })),
    )
    .await;
    assert_eq!(StatusCode::OK, status);
    assert_eq!(true, body["ok"]);
    let new_token = body["token"].as_str().expect("fresh token").to_string();
    assert!(!new_token.is_empty());

    // The old token was revoked by the password change.
    let (status, _) = send(&app, "GET", "/api/auth/me", Some(&token), None).await;
    assert_eq!(StatusCode::UNAUTHORIZED, status);

    // The fresh token works.
    let (status, _) = send(&app, "GET", "/api/auth/me", Some(&new_token), None).await;
    assert_eq!(StatusCode::OK, status);

    // The new password now authenticates.
    let (status, _) = send(
        &app,
        "POST",
        "/api/auth/login",
        None,
        Some(json!({ "username": "admin", "password": "new-password-456" })),
    )
    .await;
    assert_eq!(StatusCode::OK, status);
}

#[tokio::test]
async fn join_intent_routes_are_public() {
    let app = test_app();

    let (status, body) = send(
        &app,
        "POST",
        "/api/join-intent",
        None,
        Some(json!({ "username": "Steve" })),
    )
    .await;
    assert_eq!(StatusCode::OK, status);
    assert_eq!(true, body["ok"]);
    assert!(body["expiresInSeconds"].as_i64().unwrap_or(0) > 0);

    // The per-instance variant is public too.
    let (status, _) = send(
        &app,
        "POST",
        "/api/instances/some-id/join-intent",
        None,
        Some(json!({ "uuid": "0f7d1a1e-8d5a-4f0a-8b9c-2a3b4c5d6e7f" })),
    )
    .await;
    assert_eq!(StatusCode::OK, status);
}

#[tokio::test]
async fn bom_is_public() {
    let app = test_app();
    let (status, body) = send(&app, "GET", "/bom", None, None).await;
    assert_eq!(StatusCode::OK, status);
    assert!(body["minecraftVersion"].is_string());
    assert!(body["mods"].is_array());
}

// ---------------------------------------------------------------------------
// Protected routes (Bearer token required)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn protected_routes_reject_missing_or_invalid_tokens() {
    let app = test_app();

    // No Authorization header at all.
    for (method, uri) in [
        ("GET", "/api/stats"),
        ("GET", "/api/instances"),
        ("GET", "/api/auth/me"),
        ("GET", "/api/players/online"),
        ("GET", "/api/mods"),
        ("GET", "/api/config"),
        ("GET", "/api/status"),
        ("GET", "/api/instances/unknown-id"),
        ("POST", "/api/server/start"),
        ("POST", "/api/players/kick"),
        ("POST", "/api/auth/logout"),
        ("POST", "/api/auth/change-password"),
        ("POST", "/api/instances"),
        ("GET", "/api/system/update/check"),
        ("POST", "/api/system/update/apply"),
    ] {
        let (status, _) = send(&app, method, uri, None, None).await;
        assert_eq!(
            StatusCode::UNAUTHORIZED,
            status,
            "expected 401 for {method} {uri}"
        );
    }

    // A malformed/garbage token is rejected just the same.
    for token in ["not-a-jwt", "", "Bearer", "eyJhbGciOiJIUzI1NiJ9.abc.def"] {
        let (status, _) = send(&app, "GET", "/api/stats", Some(token), None).await;
        assert_eq!(
            StatusCode::UNAUTHORIZED,
            status,
            "expected 401 for token {token:?}"
        );
    }
}

#[tokio::test]
async fn valid_token_grants_access() {
    let app = test_app();
    let token = login(&app).await;

    // Profile query.
    let (status, body) = send(&app, "GET", "/api/auth/me", Some(&token), None).await;
    assert_eq!(StatusCode::OK, status);
    assert_eq!("admin", body["username"]);

    // Instance listing + creation (POST with body).
    let (status, body) = send(&app, "GET", "/api/instances", Some(&token), None).await;
    assert_eq!(StatusCode::OK, status);
    assert!(body["instances"].is_array());

    let (status, body) = send(
        &app,
        "POST",
        "/api/instances",
        Some(&token),
        Some(json!({
            "name": "Survival",
            "mcVersion": "1.20.4",
            "loaderType": "vanilla",
            "loaderVersion": ""
        })),
    )
    .await;
    assert_eq!(StatusCode::CREATED, status);
    let id = body["id"].as_str().expect("instance id").to_string();

    // The created instance is visible in the listing.
    let (status, body) = send(&app, "GET", "/api/instances", Some(&token), None).await;
    assert_eq!(StatusCode::OK, status);
    let ids: Vec<&str> = body["instances"]
        .as_array()
        .expect("instances array")
        .iter()
        .filter_map(|i| i["id"].as_str())
        .collect();
    assert!(ids.contains(&id.as_str()), "listing contains {id}");

    // Stats sample works.
    let (status, body) = send(&app, "GET", "/api/stats", Some(&token), None).await;
    assert_eq!(StatusCode::OK, status);
    assert!(body["current"].is_object());
}

#[tokio::test]
async fn console_upgrade_is_not_gated_by_header_middleware() {
    let app = test_app();

    // The console WebSocket authenticates via its first message (the token is
    // never put in the URL), so the upgrade request itself is not blocked by
    // the header middleware. Without upgrade headers axum rejects the plain
    // GET with 400 — the point is that it is not a 401 from auth.
    let (status, _) = send(&app, "GET", "/api/console", None, None).await;
    assert_ne!(StatusCode::UNAUTHORIZED, status);

    let token = login(&app).await;
    let (status, _) = send(&app, "GET", "/api/console", Some(&token), None).await;
    assert_ne!(StatusCode::UNAUTHORIZED, status);
}

#[tokio::test]
async fn replayed_token_works_but_garbage_does_not() {
    let app = test_app();
    let token = login(&app).await;

    // Replaying the same token works...
    let (status, _) = send(&app, "GET", "/api/auth/me", Some(&token), None).await;
    assert_eq!(StatusCode::OK, status);

    // ...and garbage still does not.
    let (status, _) = send(&app, "GET", "/api/auth/me", Some("garbage"), None).await;
    assert_eq!(StatusCode::UNAUTHORIZED, status);
}

// ---------------------------------------------------------------------------
// Session revocation & rate limiting
// ---------------------------------------------------------------------------

#[tokio::test]
async fn logout_revokes_the_token_server_side() {
    let app = test_app();
    let token = login(&app).await;

    // Valid before sign-out.
    let (status, _) = send(&app, "GET", "/api/auth/me", Some(&token), None).await;
    assert_eq!(StatusCode::OK, status);

    // Sign out revokes the session.
    let (status, body) = send(&app, "POST", "/api/auth/logout", Some(&token), None).await;
    assert_eq!(StatusCode::OK, status);
    assert_eq!(true, body["ok"]);

    // The same token is now dead everywhere, including the console WebSocket.
    // The same token is now dead everywhere. (The console WebSocket is not
    // covered by the header middleware — it authenticates via its first
    // message — so the HTTP upgrade request is not a 401 here.)
    let (status, _) = send(&app, "GET", "/api/auth/me", Some(&token), None).await;
    assert_eq!(StatusCode::UNAUTHORIZED, status);
    let (status, _) = send(&app, "GET", "/api/stats", Some(&token), None).await;
    assert_eq!(StatusCode::UNAUTHORIZED, status);

    // A fresh login still works (only that one session was killed).
    let token2 = login(&app).await;
    let (status, _) = send(&app, "GET", "/api/auth/me", Some(&token2), None).await;
    assert_eq!(StatusCode::OK, status);
}

#[tokio::test]
async fn login_is_rate_limited_against_brute_force() {
    let app = test_app_with_limits(3);

    // Three failed attempts are allowed...
    for _ in 0..3 {
        let (status, _) = send(
            &app,
            "POST",
            "/api/auth/login",
            None,
            Some(json!({ "username": "admin", "password": "wrong" })),
        )
        .await;
        assert_eq!(StatusCode::UNAUTHORIZED, status);
    }

    // ...the fourth is throttled with 429 even with correct credentials.
    let (status, _) = send(
        &app,
        "POST",
        "/api/auth/login",
        None,
        Some(json!({ "username": "admin", "password": ADMIN_PASSWORD })),
    )
    .await;
    assert_eq!(StatusCode::TOO_MANY_REQUESTS, status);
}

#[tokio::test]
async fn two_factor_flow_gates_login_until_disabled() {
    let app = test_app();
    let token = login(&app).await;

    // Setup returns a base32 secret + otpauth URI.
    let (status, body) = send(&app, "POST", "/api/auth/2fa/setup", Some(&token), None).await;
    assert_eq!(StatusCode::OK, status);
    let secret = body["secret"].as_str().expect("totp secret").to_string();
    assert!(
        body["qrUrl"]
            .as_str()
            .unwrap_or("")
            .starts_with("otpauth://"),
        "qrUrl must be an otpauth URI"
    );

    // Enabling requires a live code: a wrong one is rejected.
    let (status, _) = send(
        &app,
        "POST",
        "/api/auth/2fa/enable",
        Some(&token),
        Some(json!({ "code": "000000" })),
    )
    .await;
    assert_eq!(StatusCode::UNAUTHORIZED, status);

    // The correct code activates 2FA.
    let code = totp_code(&secret);
    let (status, _) = send(
        &app,
        "POST",
        "/api/auth/2fa/enable",
        Some(&token),
        Some(json!({ "code": code })),
    )
    .await;
    assert_eq!(StatusCode::OK, status);

    // me reports 2FA enabled.
    let (status, body) = send(&app, "GET", "/api/auth/me", Some(&token), None).await;
    assert_eq!(StatusCode::OK, status);
    assert_eq!(true, body["totpEnabled"]);

    // Login now requires the code: password alone, or a wrong code, is 401.
    let (status, _) = send(
        &app,
        "POST",
        "/api/auth/login",
        None,
        Some(json!({ "username": "admin", "password": ADMIN_PASSWORD })),
    )
    .await;
    assert_eq!(StatusCode::UNAUTHORIZED, status);
    let (status, _) = send(
        &app,
        "POST",
        "/api/auth/login",
        None,
        Some(json!({
            "username": "admin",
            "password": ADMIN_PASSWORD,
            "totp_code": "000000"
        })),
    )
    .await;
    assert_eq!(StatusCode::UNAUTHORIZED, status);
    let (status, _) = send(
        &app,
        "POST",
        "/api/auth/login",
        None,
        Some(json!({
            "username": "admin",
            "password": ADMIN_PASSWORD,
            "totp_code": totp_code(&secret)
        })),
    )
    .await;
    assert_eq!(StatusCode::OK, status);

    // Disabling turns the gate back off.
    let (status, _) = send(&app, "POST", "/api/auth/2fa/disable", Some(&token), None).await;
    assert_eq!(StatusCode::OK, status);
    let (status, _) = send(
        &app,
        "POST",
        "/api/auth/login",
        None,
        Some(json!({ "username": "admin", "password": ADMIN_PASSWORD })),
    )
    .await;
    assert_eq!(StatusCode::OK, status);
}

#[tokio::test]
async fn session_cookie_authenticates_and_logout_clears_it() {
    let app = test_app();

    // Login returns a session cookie with the hardening flags.
    let request = Request::builder()
        .method("POST")
        .uri("/api/auth/login")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            json!({ "username": "admin", "password": ADMIN_PASSWORD }).to_string(),
        ))
        .unwrap();
    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(StatusCode::OK, response.status());
    let set_cookie = response
        .headers()
        .get(header::SET_COOKIE)
        .expect("Set-Cookie on login")
        .to_str()
        .unwrap()
        .to_string();
    assert!(set_cookie.contains("HttpOnly"), "cookie: {set_cookie}");
    assert!(
        set_cookie.contains("SameSite=Strict"),
        "cookie: {set_cookie}"
    );
    assert!(set_cookie.contains("Secure"), "cookie: {set_cookie}");
    assert!(set_cookie.contains("Path=/"), "cookie: {set_cookie}");
    let cookie_pair = set_cookie.split(';').next().unwrap().to_string();
    let _ = response.into_body().collect().await;

    // The cookie alone (no Authorization header) unlocks protected routes.
    let request = Request::builder()
        .method("GET")
        .uri("/api/auth/me")
        .header(header::COOKIE, &cookie_pair)
        .body(Body::empty())
        .unwrap();
    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(StatusCode::OK, response.status());
    let _ = response.into_body().collect().await;

    // Logout clears the cookie (expiry marker) and revokes the session.
    let request = Request::builder()
        .method("POST")
        .uri("/api/auth/logout")
        .header(header::COOKIE, &cookie_pair)
        .body(Body::empty())
        .unwrap();
    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(StatusCode::OK, response.status());
    let clear_cookie = response
        .headers()
        .get(header::SET_COOKIE)
        .expect("Set-Cookie on logout")
        .to_str()
        .unwrap()
        .to_string();
    assert!(
        clear_cookie.starts_with("zircon_session="),
        "{clear_cookie}"
    );
    assert!(clear_cookie.contains("Max-Age=0"), "{clear_cookie}");
    let _ = response.into_body().collect().await;

    // The same session token is now dead via cookie too.
    let request = Request::builder()
        .method("GET")
        .uri("/api/auth/me")
        .header(header::COOKIE, &cookie_pair)
        .body(Body::empty())
        .unwrap();
    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(StatusCode::UNAUTHORIZED, response.status());
}
