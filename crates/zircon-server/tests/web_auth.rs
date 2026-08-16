//! Router-level integration tests for the Axum admin API.
//! Verifies the Phase 3 checkpoint: all REST routes compile and the JWT
//! middleware properly gates protected endpoints — public routes work without a
//! token, protected routes reject unauthenticated requests with 401, and a
//! valid login token unlocks them.

use std::sync::Arc;

use axum::body::Body;
use axum::http::{header, Request, StatusCode};
use axum::Router;
use http_body_util::BodyExt;
use serde_json::json;
use tower::ServiceExt;

use zircon_server::auth::auth_service::AuthService;
use zircon_server::auth::jwt;
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
async fn change_password_is_public_but_requires_current_password() {
    let app = test_app();

    // Wrong current password -> 401 (public route, but proof of knowledge).
    let (status, _) = send(
        &app,
        "POST",
        "/api/auth/change-password",
        None,
        Some(json!({
            "username": "admin",
            "currentPassword": "wrong",
            "newPassword": "new-password-456"
        })),
    )
    .await;
    assert_eq!(StatusCode::UNAUTHORIZED, status);

    // Correct current password -> 200 without any token.
    let (status, body) = send(
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
    assert_eq!(StatusCode::OK, status);
    assert_eq!(true, body["ok"]);

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
        ("GET", "/api/console"),
        ("POST", "/api/server/start"),
        ("POST", "/api/players/kick"),
        ("POST", "/api/instances"),
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
async fn console_ws_requires_auth() {
    let app = test_app();

    // Without a token the upgrade is rejected by the middleware before the
    // WebSocket handler even looks at the request.
    let (status, _) = send(&app, "GET", "/api/console", None, None).await;
    assert_eq!(StatusCode::UNAUTHORIZED, status);

    // With a token the handler responds (400 without proper upgrade headers is
    // expected from axum's WebSocketUpgrade extractor — the point is the
    // request is no longer blocked by auth).
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
