//! Constructs the Axum application: public routes, JWT-protected admin routes,
//! the WebSocket console streamer, static SPA assets and the SPA fallback.
//!
//! Port of `com.mcmanager.server.web.JavalinApp`.

use std::fmt;
use std::sync::Arc;

use axum::extract::{DefaultBodyLimit, Request};
use axum::http::{header, StatusCode};
use axum::middleware;
use axum::response::{IntoResponse, Response};
use axum::routing::{delete, get, post};
use axum::Router;
use tower_http::trace::TraceLayer;

use crate::auth::jwt;
use crate::auth::sessions::SessionRegistry;
use crate::config::ConfigService;
use crate::instance::ServerInstanceManager;
use crate::process::console::ConsoleStreamHandler;
use crate::process::manager::MinecraftProcessManager;
use crate::services::backup::BackupService;
use crate::services::bom::BomService;
use crate::services::mods::ModManagementService;
use crate::services::packs::PackManagementService;
use crate::services::resolver::ModServiceResolver;
use crate::tickets::JoinTicketManager;

use super::auth::require_auth;
use super::controllers::{
    auth_controller, backup_controller, bom_controller, console_controller, instance_controller,
    mod_controller, pack_controller, player_controller, stats_controller, system_controller,
};
use super::rate_limit::FixedWindowLimiter;

/// Shared application state handed to every handler.
#[derive(Clone)]
pub struct AppState {
    pub config: Arc<ConfigService>,
    pub auth: Arc<crate::auth::auth_service::AuthService>,
    pub instances: Arc<ServerInstanceManager>,
    pub console: Arc<ConsoleStreamHandler>,
    pub process_manager: Arc<MinecraftProcessManager>,
    pub backup: Arc<BackupService>,
    pub bom: Arc<BomService>,
    pub mods: Arc<ModManagementService>,
    pub packs: PackManagementService,
    pub resolver: Arc<ModServiceResolver>,
    pub tickets: Arc<JoinTicketManager>,
    pub curseforge_api_key: String,
    /// Server-side session registry (sign-out / password-change revocation).
    pub sessions: Arc<SessionRegistry>,
    /// Fixed-window limiter for authentication endpoints.
    pub login_limiter: Arc<FixedWindowLimiter>,
}

/// Errors mapped to HTTP responses.
#[derive(Debug)]
pub enum ApiError {
    BadRequest(String),
    Unauthorized(String),
    NotFound(String),
    Conflict(String),
    /// Authentication endpoints rate-limited (too many attempts).
    TooManyRequests(String),
    Internal(String),
    /// Provider/upstream failures (Modrinth/CurseForge/install steps).
    BadGateway(String),
}

impl fmt::Display for ApiError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ApiError::BadRequest(m)
            | ApiError::Unauthorized(m)
            | ApiError::NotFound(m)
            | ApiError::Conflict(m)
            | ApiError::TooManyRequests(m)
            | ApiError::Internal(m)
            | ApiError::BadGateway(m) => write!(f, "{m}"),
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, message) = match self {
            ApiError::BadRequest(m) => (StatusCode::BAD_REQUEST, m),
            ApiError::Unauthorized(m) => (StatusCode::UNAUTHORIZED, m),
            ApiError::NotFound(m) => (StatusCode::NOT_FOUND, m),
            ApiError::Conflict(m) => (StatusCode::CONFLICT, m),
            ApiError::TooManyRequests(m) => (StatusCode::TOO_MANY_REQUESTS, m),
            ApiError::Internal(m) => (StatusCode::INTERNAL_SERVER_ERROR, m),
            ApiError::BadGateway(m) => (StatusCode::BAD_GATEWAY, m),
        };
        (status, message).into_response()
    }
}

impl From<std::io::Error> for ApiError {
    fn from(e: std::io::Error) -> Self {
        ApiError::Internal(e.to_string())
    }
}

impl From<crate::instance::InstanceError> for ApiError {
    fn from(e: crate::instance::InstanceError) -> Self {
        match e {
            crate::instance::InstanceError::NotFound(m) => ApiError::NotFound(m),
            crate::instance::InstanceError::Conflict(m) => ApiError::Conflict(m),
            crate::instance::InstanceError::Invalid(m) => ApiError::BadRequest(m),
            crate::instance::InstanceError::Io(e) => ApiError::Internal(e.to_string()),
        }
    }
}

impl From<crate::services::mods::ModError> for ApiError {
    fn from(e: crate::services::mods::ModError) -> Self {
        match e {
            crate::services::mods::ModError::Invalid(m) => ApiError::BadRequest(m),
            crate::services::mods::ModError::Io(e) => ApiError::Internal(e.to_string()),
            crate::services::mods::ModError::Api(m) => ApiError::BadGateway(m),
        }
    }
}

impl From<crate::services::packs::PackError> for ApiError {
    fn from(e: crate::services::packs::PackError) -> Self {
        match e {
            crate::services::packs::PackError::Invalid(m) => ApiError::BadRequest(m),
            crate::services::packs::PackError::Io(e) => ApiError::Internal(e.to_string()),
            crate::services::packs::PackError::Api(m) => ApiError::BadGateway(m),
        }
    }
}

impl From<crate::services::backup::BackupError> for ApiError {
    fn from(e: crate::services::backup::BackupError) -> Self {
        match e {
            crate::services::backup::BackupError::NotFound(m) => ApiError::NotFound(m),
            crate::services::backup::BackupError::Invalid(m) => ApiError::BadRequest(m),
            crate::services::backup::BackupError::Io(e) => ApiError::Internal(e.to_string()),
        }
    }
}

/// Builds the full Axum router.
///
/// NOTE ON ROUTE SYNTAX: axum's docs describe `{param}` segments, but the
/// resolved `matchit 0.7.3` (axum's router) implements the older `:param`
/// syntax — `{param}` routes silently 404. Keep all dynamic segments in
/// `:param` form (`:id`, `:filename`, ...).
pub fn router(state: AppState) -> Router {
    // ----------------------------------------------------------------------
    // Public API (no admin token required)
    // ----------------------------------------------------------------------
    let public_api = Router::new()
        .route("/api/auth/login", post(auth_controller::login))
        .route(
            "/api/join-intent",
            post(instance_controller::register_join_intent),
        )
        .route(
            "/api/instances/:id/join-intent",
            post(instance_controller::register_join_intent),
        );

    // ----------------------------------------------------------------------
    // Protected admin API
    // ----------------------------------------------------------------------
    let protected_api = Router::new()
        // Auth
        .route("/api/auth/me", get(auth_controller::me))
        .route("/api/auth/profile", post(auth_controller::profile))
        .route(
            "/api/auth/change-password",
            post(auth_controller::change_password),
        )
        .route("/api/auth/logout", post(auth_controller::logout))
        // Stats
        .route("/api/stats", get(stats_controller::stats))
        // System self-update
        .route("/api/system/update/check", get(system_controller::check_update))
        .route("/api/system/update/apply", post(system_controller::apply_update))
        // Legacy single-server endpoints (serve the active instance's data)
        .route("/api/mods", get(mod_controller::list_mods))
        .route("/api/mods/upload", post(mod_controller::upload_mod))
        .route("/api/mods/:filename", delete(mod_controller::remove_mod))
        .route("/api/mods/search", get(mod_controller::search_mods))
        .route(
            "/api/mods/modrinth/versions",
            get(mod_controller::modrinth_versions),
        )
        .route(
            "/api/mods/curseforge/files",
            get(mod_controller::curseforge_files),
        )
        .route("/api/mods/install", post(mod_controller::install_mod))
        .route("/api/players/online", get(player_controller::online))
        .route(
            "/api/players/whitelist",
            get(player_controller::get_whitelist).post(player_controller::add_whitelist),
        )
        .route(
            "/api/players/whitelist/:name",
            delete(player_controller::remove_whitelist),
        )
        .route(
            "/api/players/bans",
            get(player_controller::get_bans).post(player_controller::add_ban),
        )
        .route(
            "/api/players/bans/:name",
            delete(player_controller::remove_ban),
        )
        .route(
            "/api/players/ops",
            get(player_controller::get_ops).post(player_controller::add_op),
        )
        .route(
            "/api/players/ops/:name",
            delete(player_controller::remove_op),
        )
        .route("/api/players/kick", post(player_controller::kick))
        .route("/api/players/command", post(player_controller::run_command))
        .route(
            "/api/config",
            get(config_routes::get_config).post(config_routes::update_config),
        )
        .route("/api/status", get(config_routes::get_status))
        .route("/api/server/start", post(config_routes::start_server))
        .route("/api/server/stop", post(config_routes::stop_server))
        // Multi-instance API
        .route(
            "/api/instances",
            get(instance_controller::list_instances).post(instance_controller::create_instance),
        )
        .route(
            "/api/instances/:id",
            get(instance_controller::get_instance)
                .patch(instance_controller::update_instance)
                .delete(instance_controller::delete_instance),
        )
        .route(
            "/api/instances/:id/start",
            post(instance_controller::start_instance),
        )
        .route(
            "/api/instances/:id/stop",
            post(instance_controller::stop_instance),
        )
        .route(
            "/api/instances/:id/restart",
            post(instance_controller::restart_instance),
        )
        .route(
            "/api/instances/:id/eula",
            get(instance_controller::get_eula).post(instance_controller::accept_eula),
        )
        .route(
            "/api/instances/:id/server-properties",
            get(instance_controller::get_server_properties)
                .post(instance_controller::save_server_properties),
        )
        .route(
            "/api/instances/:id/players/online",
            get(instance_controller::online_players),
        )
        .route(
            "/api/instances/:id/players/history",
            get(instance_controller::player_history),
        )
        .route(
            "/api/instances/:id/players/whitelist",
            get(instance_controller::get_whitelist).post(instance_controller::add_whitelist),
        )
        .route(
            "/api/instances/:id/players/whitelist/:name",
            delete(instance_controller::remove_whitelist),
        )
        .route(
            "/api/instances/:id/players/ops",
            get(instance_controller::get_ops).post(instance_controller::add_op),
        )
        .route(
            "/api/instances/:id/players/ops/:name",
            delete(instance_controller::remove_op),
        )
        .route(
            "/api/instances/:id/players/bans",
            get(instance_controller::get_bans).post(instance_controller::add_ban),
        )
        .route(
            "/api/instances/:id/players/bans/:name",
            delete(instance_controller::remove_ban),
        )
        .route(
            "/api/instances/:id/bom",
            get(instance_controller::get_instance_bom),
        )
        .route(
            "/api/instances/:id/mods",
            get(instance_controller::list_mods),
        )
        .route(
            "/api/instances/:id/mods/upload",
            post(instance_controller::upload_mod),
        )
        .route(
            "/api/instances/:id/mods/:filename",
            delete(instance_controller::remove_mod),
        )
        .route(
            "/api/instances/:id/mods/search",
            get(instance_controller::search_mods),
        )
        .route(
            "/api/instances/:id/mods/modrinth/versions",
            get(instance_controller::modrinth_versions),
        )
        .route(
            "/api/instances/:id/mods/curseforge/files",
            get(instance_controller::curseforge_files),
        )
        .route(
            "/api/instances/:id/mods/install",
            post(instance_controller::install_mod),
        )
        .route(
            "/api/instances/:id/modpacks/install",
            post(instance_controller::install_modpack),
        )
        .route(
            "/api/instances/:id/shaderpacks",
            get(instance_controller::list_shaderpacks),
        )
        .route(
            "/api/instances/:id/shaderpacks/upload",
            post(instance_controller::upload_shaderpack),
        )
        .route(
            "/api/instances/:id/shaderpacks/install",
            post(instance_controller::install_shaderpack),
        )
        .route(
            "/api/instances/:id/shaderpacks/:filename",
            delete(instance_controller::remove_shaderpack),
        )
        .route(
            "/api/instances/:id/resourcepacks",
            get(instance_controller::list_resourcepacks),
        )
        .route(
            "/api/instances/:id/resourcepacks/upload",
            post(instance_controller::upload_resourcepack),
        )
        .route(
            "/api/instances/:id/resourcepacks/install",
            post(instance_controller::install_resourcepack),
        )
        .route(
            "/api/instances/:id/resourcepacks/:filename",
            delete(instance_controller::remove_resourcepack),
        )
        .route(
            "/api/instances/:id/backups",
            get(backup_controller::list_backups).post(backup_controller::create_backup),
        )
        .route(
            "/api/instances/:id/backups/retention",
            post(backup_controller::set_retention),
        )
        .route(
            "/api/instances/:id/backups/:backup_id/restore",
            post(backup_controller::restore_backup),
        )
        .route_layer(middleware::from_fn_with_state(state.clone(), require_auth));

    // The console WebSocket authenticates with its first message (browsers
    // cannot set headers on the handshake, and a ?token= URL would leak into
    // logs), so it is deliberately NOT covered by the header-auth middleware.
    let console_router = Router::new().route("/api/console", get(console_controller::console_ws));

    // ----------------------------------------------------------------------
    // Client-facing legacy endpoints (public, outside /api)
    // ----------------------------------------------------------------------
    let client_routes = Router::new()
        .route("/status", get(config_routes::client_status))
        .route("/bom", get(bom_controller::get_bom))
        .route("/files/mods/:filename", get(mod_controller::download_mod))
        .route(
            "/files/shaderpacks/:filename",
            get(pack_controller::download_shaderpack),
        )
        .route(
            "/files/resourcepacks/:filename",
            get(pack_controller::download_resourcepack),
        );

    Router::new()
        .merge(public_api)
        .merge(protected_api)
        .merge(console_router)
        .merge(client_routes)
        .route("/", get(spa_index))
        .fallback(spa_fallback)
        .layer(TraceLayer::new_for_http())
        .layer(DefaultBodyLimit::max(512 * 1024 * 1024))
        .with_state(state)
}

/// Serves the bundled admin SPA (`index.html`).
async fn spa_index() -> impl IntoResponse {
    spa_response("/index.html")
}

/// SPA fallback: serve the requested static asset when it exists (JS, CSS,
/// SVG, HTML), otherwise `index.html` for unknown non-API GETs (deep links).
/// API paths return 404.
async fn spa_fallback(request: Request) -> impl IntoResponse {
    if request.method() == axum::http::Method::GET && !request.uri().path().starts_with("/api/") {
        let path = request.uri().path();
        // Serve real assets (js/css/svg/html) before falling back to the SPA
        // shell, so the browser never receives index.html for a script request.
        if static_file(path).is_some() {
            return spa_response(path).into_response();
        }
        return spa_response("/index.html").into_response();
    }
    (StatusCode::NOT_FOUND, "Not found").into_response()
}

fn spa_response(path: &str) -> Response {
    match static_file(path) {
        Some((content_type, content)) => (
            [
                (header::CONTENT_TYPE, content_type),
                (header::X_CONTENT_TYPE_OPTIONS, "nosniff"),
                (header::X_FRAME_OPTIONS, "DENY"),
                (header::REFERRER_POLICY, "no-referrer"),
                (header::CONTENT_SECURITY_POLICY, SPA_CSP),
            ],
            content,
        )
            .into_response(),
        None => (StatusCode::NOT_FOUND, "Not found").into_response(),
    }
}

/// Content-Security-Policy for the embedded SPA. The dashboard loads Vue and
/// Tailwind from CDNs and compiles in-DOM templates at runtime (which needs
/// `unsafe-eval`), so this can't be lock-tight — it still blocks arbitrary
/// external script origins, inline data: execution and clickjacking.
const SPA_CSP: &str = "default-src 'self'; \
    script-src 'self' https://cdn.tailwindcss.com https://unpkg.com 'unsafe-inline' 'unsafe-eval'; \
    style-src 'self' 'unsafe-inline' https://cdn.tailwindcss.com; \
    img-src 'self' data: https:; \
    font-src 'self' data:; \
    connect-src 'self' ws: wss:; \
    frame-ancestors 'none'; \
    base-uri 'self'; \
    form-action 'self'";

/// Embedded static assets for the admin SPA.
pub fn static_file(path: &str) -> Option<(&'static str, &'static str)> {
    let (content_type, content) = match path {
        "/index.html" | "/" => (
            "text/html; charset=utf-8",
            include_str!("../../assets/web/index.html"),
        ),
        "/app.js" => (
            "application/javascript; charset=utf-8",
            include_str!("../../assets/web/app.js"),
        ),
        "/styles.css" => (
            "text/css; charset=utf-8",
            include_str!("../../assets/web/styles.css"),
        ),
        "/zircon-icon.svg" => (
            "image/svg+xml",
            include_str!("../../assets/web/zircon-icon.svg"),
        ),
        "/zircon-title.svg" => (
            "image/svg+xml",
            include_str!("../../assets/web/zircon-title.svg"),
        ),
        "/js/auth.js" => (
            "application/javascript; charset=utf-8",
            include_str!("../../assets/web/js/auth.js"),
        ),
        "/js/backups.js" => (
            "application/javascript; charset=utf-8",
            include_str!("../../assets/web/js/backups.js"),
        ),
        "/js/console.js" => (
            "application/javascript; charset=utf-8",
            include_str!("../../assets/web/js/console.js"),
        ),
        "/js/core.js" => (
            "application/javascript; charset=utf-8",
            include_str!("../../assets/web/js/core.js"),
        ),
        "/js/instances.js" => (
            "application/javascript; charset=utf-8",
            include_str!("../../assets/web/js/instances.js"),
        ),
        "/js/mods.js" => (
            "application/javascript; charset=utf-8",
            include_str!("../../assets/web/js/mods.js"),
        ),
        "/js/packs.js" => (
            "application/javascript; charset=utf-8",
            include_str!("../../assets/web/js/packs.js"),
        ),
        "/js/players.js" => (
            "application/javascript; charset=utf-8",
            include_str!("../../assets/web/js/players.js"),
        ),
        "/js/settings.js" => (
            "application/javascript; charset=utf-8",
            include_str!("../../assets/web/js/settings.js"),
        ),
        _ => return None,
    };
    Some((content_type, content))
}

/// Issues a JWT for `username` and registers the session so it can be revoked
/// server-side on sign-out or password change.
pub fn issue_token(state: &AppState, username: &str) -> String {
    let token = jwt::generate_token(username);
    if let Some(claims) = jwt::decode_claims(&token) {
        state.sessions.register(&claims.jti, username, claims.exp);
    }
    token
}

use super::config_routes;
