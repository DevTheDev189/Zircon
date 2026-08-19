//! Shared helpers for the zircon-server integration test binaries. Mirrors the
//! lib's `#[cfg(test)] test_util` (not reachable from integration tests) and
//! the router wiring used by `tests/web_auth.rs`, so every security test spins
//! up the same fully-wired app.
#![allow(dead_code)] // each test binary uses a different subset of helpers

use std::sync::Arc;
use std::time::Duration;

use axum::body::Body;
use axum::extract::ConnectInfo;
use axum::http::{header, Request, StatusCode};
use axum::Router;
use http_body_util::BodyExt;
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

pub const ADMIN_PASSWORD: &str = "test-password-123";

/// Unique temp dir per test (integration tests can't reach the lib's
/// `#[cfg(test)] test_util` module, so this mirrors it locally).
pub fn temp_dir(prefix: &str) -> std::path::PathBuf {
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
pub fn test_app() -> Router {
    test_app_with_limits(10, 30)
}

/// Like `test_app` but with configurable rate-limit window caps (login
/// attempts per IP+user, join-intent registrations per IP).
pub fn test_app_with_limits(max_attempts: u32, max_join_intents: u32) -> Router {
    let dir = temp_dir("common");
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
        None,
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
        signing_key: None,
        sessions: Arc::new(SessionRegistry::new()),
        login_limiter: Arc::new(FixedWindowLimiter::new(
            Duration::from_secs(60),
            max_attempts,
        )),
        join_intent_limiter: Arc::new(FixedWindowLimiter::new(
            Duration::from_secs(60),
            max_join_intents,
        )),
        audit,
    };
    router(state)
}

/// Sends one request through the router and returns the status plus the parsed
/// JSON body (or `Null` for empty bodies).
pub async fn send(
    app: &Router,
    method: &str,
    uri: &str,
    token: Option<&str>,
    body: Option<serde_json::Value>,
) -> (StatusCode, serde_json::Value) {
    send_from(app, "127.0.0.1", method, uri, token, body).await
}

/// Like `send` but pins the peer address via `ConnectInfo` so rate-limit
/// keying can be exercised per source IP.
pub async fn send_from(
    app: &Router,
    ip: &str,
    method: &str,
    uri: &str,
    token: Option<&str>,
    body: Option<serde_json::Value>,
) -> (StatusCode, serde_json::Value) {
    send_from_with_headers(app, ip, method, uri, token, body, &[]).await
}

/// `send_from` with arbitrary extra headers (e.g. a spoofed
/// `X-Zircon-Real-IP`).
pub async fn send_from_with_headers(
    app: &Router,
    ip: &str,
    method: &str,
    uri: &str,
    token: Option<&str>,
    body: Option<serde_json::Value>,
    extra_headers: &[(&str, &str)],
) -> (StatusCode, serde_json::Value) {
    let mut builder = Request::builder().method(method).uri(uri);
    if let Some(token) = token {
        builder = builder.header(header::AUTHORIZATION, format!("Bearer {token}"));
    }
    for (name, value) in extra_headers {
        builder = builder.header(*name, *value);
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
    let mut request = request;
    let addr: std::net::SocketAddr = format!("{ip}:12345").parse().expect("socket addr");
    request.extensions_mut().insert(ConnectInfo(addr));
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
