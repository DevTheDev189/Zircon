//! Constructs the Axum application: public routes, JWT-protected admin routes,
//! the WebSocket console streamer, static SPA assets and the SPA fallback.
//!
//! Port of `com.mcmanager.server.web.JavalinApp`.

use std::fmt;
use std::sync::Arc;

use axum::extract::{ConnectInfo, DefaultBodyLimit, FromRequestParts, Request};
use axum::http::{header, StatusCode};
use axum::middleware;
use axum::response::{IntoResponse, Response};
use axum::routing::{delete, get, post};
use axum::Router;
use tower_http::trace::TraceLayer;

use std::net::{IpAddr, Ipv4Addr, SocketAddr};

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
    /// Server-level Ed25519 key for signing per-instance BOMs; shares the pin
    /// launchers TOFU on via the legacy `/bom` endpoint.
    pub signing_key: Option<Arc<ed25519_dalek::SigningKey>>,
    /// Server-side session registry (sign-out / password-change revocation).
    pub sessions: Arc<SessionRegistry>,
    /// Fixed-window limiter for authentication endpoints.
    pub login_limiter: Arc<FixedWindowLimiter>,
    /// Fixed-window limiter for the public join-intent endpoint, keyed by real
    /// client IP so a single attacker cannot fill the ticket store and block
    /// legitimate player joins.
    pub join_intent_limiter: Arc<FixedWindowLimiter>,
    /// Append-only audit log for sensitive administrative actions.
    pub audit: Arc<crate::audit::AuditLogger>,
}

/// The client IP for rate limiting and other per-source decisions.
///
/// Every remote connection reaches the admin web server through the TCP
/// multiplexer, which terminates on loopback — so the direct peer address is
/// `127.0.0.1` even for internet clients. The multiplexer injects the real
/// client IP via the `X-Zircon-Real-IP` header; because the web server binds
/// loopback-only, only that trusted proxy can set the header, so it is honored
/// **only** when the direct peer is loopback. Any other caller (a LAN client
/// that reached the panel directly) is keyed by its real peer address, and a
/// request with no peer information falls back to `127.0.0.1`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RealIp(pub IpAddr);

#[axum::async_trait]
impl<S> FromRequestParts<S> for RealIp {
    type Rejection = std::convert::Infallible;

    async fn from_request_parts(
        parts: &mut axum::http::request::Parts,
        _state: &S,
    ) -> Result<Self, Self::Rejection> {
        let direct = parts
            .extensions
            .get::<ConnectInfo<SocketAddr>>()
            .map(|c| c.0.ip());
        let ip = match direct {
            Some(peer) if peer.is_loopback() => parts
                .headers
                .get("x-zircon-real-ip")
                .and_then(|v| v.to_str().ok())
                .and_then(|s| s.trim().parse().ok())
                .unwrap_or(peer),
            Some(peer) => peer,
            None => IpAddr::V4(Ipv4Addr::LOCALHOST),
        };
        Ok(RealIp(ip))
    }
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
        )
        // Path-based port routing for HTTPS reverse proxies: the launcher's
        // base URL may carry the instance port as a path segment, so the
        // wakeup / join-intent endpoints are reachable at /:port/api/... too.
        // Both handlers resolve the instance from the request body.
        .route("/:port/api/wakeup", post(config_routes::wakeup_server))
        .route(
            "/:port/api/join-intent",
            post(instance_controller::register_join_intent),
        )
        .route("/api/wakeup", post(config_routes::wakeup_server));

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
        .route("/api/auth/2fa/setup", post(auth_controller::setup_2fa))
        .route("/api/auth/2fa/enable", post(auth_controller::enable_2fa))
        .route("/api/auth/2fa/disable", post(auth_controller::disable_2fa))
        // System self-update
        .route(
            "/api/system/update/check",
            get(system_controller::check_update),
        )
        .route(
            "/api/system/update/apply",
            post(system_controller::apply_update),
        )
        // Stats
        .route("/api/stats", get(stats_controller::stats))
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
            get(config_routes::get_config)
                .post(config_routes::update_config)
                .put(config_routes::update_config)
                .patch(config_routes::update_config),
        )
        .route(
            "/api/settings",
            get(config_routes::get_config)
                .post(config_routes::update_config)
                .put(config_routes::update_config)
                .patch(config_routes::update_config),
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
                .put(instance_controller::update_instance)
                .post(instance_controller::update_instance)
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
                .post(instance_controller::save_server_properties)
                .put(instance_controller::save_server_properties),
        )
        .route(
            "/api/instances/:id/settings",
            get(instance_controller::get_instance)
                .patch(instance_controller::update_instance)
                .put(instance_controller::update_instance)
                .post(instance_controller::update_instance),
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
        )
        // Path-based port routing: HTTPS reverse proxies cannot carry a port in
        // the Host header (e.g. https://domain.net), so instance ports are
        // encoded as the first path segment instead.
        .route("/:port/status", get(config_routes::client_status_by_port))
        .route("/:port/bom", get(bom_controller::get_bom_by_port))
        .route(
            "/:port/files/mods/:filename",
            get(mod_controller::download_mod_by_port),
        )
        .route(
            "/:port/files/shaderpacks/:filename",
            get(pack_controller::download_shaderpack_by_port),
        )
        .route(
            "/:port/files/resourcepacks/:filename",
            get(pack_controller::download_resourcepack_by_port),
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

/// Content-Security-Policy for the embedded SPA. The dashboard is fully
/// pre-compiled: Vue's runtime-only build is vendored locally, the template is
/// compiled to a render function at build time (no `unsafe-eval`), and the
/// Tailwind utilities are generated into `styles.css` at build time (no CDN
/// JIT, no `unsafe-inline` scripts). Only same-origin scripts may run; inline
/// styles remain allowed for Vue's dynamic style bindings.
const SPA_CSP: &str = "default-src 'self'; \
    script-src 'self'; \
    style-src 'self' 'unsafe-inline'; \
    img-src 'self' data: https:; \
    font-src 'self' data:; \
    connect-src 'self' ws: wss:; \
    frame-ancestors 'none'; \
    base-uri 'self'; \
    form-action 'self'; \
    object-src 'none'";

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
        "/js/render.js" => (
            "application/javascript; charset=utf-8",
            include_str!("../../assets/web/js/render.js"),
        ),
        "/js/settings.js" => (
            "application/javascript; charset=utf-8",
            include_str!("../../assets/web/js/settings.js"),
        ),
        "/js/vue.runtime.global.prod.js" => (
            "application/javascript; charset=utf-8",
            include_str!("../../assets/web/js/vue.runtime.global.prod.js"),
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

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::Request;

    async fn real_ip(peer: Option<IpAddr>, header: Option<&str>) -> IpAddr {
        let mut request = Request::builder()
            .uri("/")
            .body(Body::empty())
            .expect("request");
        if let Some(header) = header {
            request
                .headers_mut()
                .insert("x-zircon-real-ip", header.parse().expect("header value"));
        }
        if let Some(ip) = peer {
            request
                .extensions_mut()
                .insert(ConnectInfo(SocketAddr::new(ip, 12345)));
        }
        let (mut parts, _) = request.into_parts();
        RealIp::from_request_parts(&mut parts, &())
            .await
            .expect("infallible")
            .0
    }

    #[tokio::test]
    async fn trusts_x_real_ip_only_from_loopback_peers() {
        // The multiplexer path: loopback peer + real IP header -> header wins.
        let via_proxy = "203.0.113.7".parse::<IpAddr>().unwrap();
        assert_eq!(
            via_proxy,
            real_ip(Some("127.0.0.1".parse().unwrap()), Some("203.0.113.7")).await
        );
        assert_eq!(
            via_proxy,
            real_ip(Some("::1".parse().unwrap()), Some("203.0.113.7")).await
        );

        // A non-loopback peer (direct LAN client) is keyed by its real address;
        // a spoofed header is ignored.
        let lan = "192.168.1.50".parse::<IpAddr>().unwrap();
        assert_eq!(lan, real_ip(Some(lan), Some("203.0.113.7")).await);

        // Loopback peer without a header falls back to the peer address.
        assert_eq!(
            "127.0.0.1".parse::<IpAddr>().unwrap(),
            real_ip(Some("127.0.0.1".parse().unwrap()), None).await
        );

        // A malformed header is ignored.
        assert_eq!(
            "127.0.0.1".parse::<IpAddr>().unwrap(),
            real_ip(Some("127.0.0.1".parse().unwrap()), Some("not-an-ip")).await
        );

        // No peer information at all (e.g. unit-test oneshot requests) falls
        // back to loopback.
        assert_eq!(
            "127.0.0.1".parse::<IpAddr>().unwrap(),
            real_ip(None, None).await
        );
    }

    #[test]
    fn spa_csp_is_zero_eval_and_self_only() {
        // Phase 4 hardening: the dashboard is pre-compiled and self-hosted, so
        // the CSP must not allow any external origin, inline scripts, or
        // unsafe-eval — an XSS can no longer escalate to arbitrary code.
        assert!(
            !SPA_CSP.contains("unsafe-eval"),
            "CSP must not allow unsafe-eval: {SPA_CSP}"
        );
        // Inline styles stay allowed (Vue's style bindings); inline *scripts*
        // must not be. Check the script-src directive specifically.
        let script_src = SPA_CSP
            .split(';')
            .find(|d| d.trim_start().starts_with("script-src"))
            .expect("script-src directive");
        assert!(
            !script_src.contains("unsafe-inline"),
            "script-src must not allow inline scripts: {script_src}"
        );
        assert!(SPA_CSP.contains("script-src 'self'"), "CSP: {SPA_CSP}");
        assert!(
            !SPA_CSP.contains("https://") && !SPA_CSP.contains("http://"),
            "CSP must not whitelist external origins: {SPA_CSP}"
        );
        assert!(SPA_CSP.contains("object-src 'none'"), "CSP: {SPA_CSP}");
        assert!(SPA_CSP.contains("frame-ancestors 'none'"), "CSP: {SPA_CSP}");
    }

    #[test]
    fn spa_assets_are_self_contained() {
        // No CDN fetches may remain: the page must load only same-origin
        // scripts, and the render/Vue files must exist for static_file.
        let index = include_str!("../../assets/web/index.html");
        assert!(
            !index.contains("https://"),
            "CDN script in index.html: {index}"
        );
        assert!(
            static_file("/js/vue.runtime.global.prod.js").is_some(),
            "vendored Vue runtime must be embedded"
        );
        assert!(
            static_file("/js/render.js").is_some(),
            "precompiled render must be embedded"
        );
        // The generated render defines ZirconRender and the app uses it.
        let render = include_str!("../../assets/web/js/render.js");
        assert!(render.contains("window.ZirconRender"));
        let app_js = include_str!("../../assets/web/app.js");
        assert!(app_js.contains("render: ZirconRender"));
    }
}
