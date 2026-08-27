//! WebSocket endpoint `/api/console`. Server console lines are streamed to
//! every connected session; messages sent by a client are written to the server
//! stdin as commands (this is how the admin UI sends "whitelist add X" etc.).
//!
//! Browsers cannot set HTTP headers on the WebSocket handshake, and a token in
//! the URL (`?token=`) would leak into access logs, proxies and history — so
//! the client authenticates with the token as its **first message**:
//! `AUTH <jwt>`. The connection is closed when that message is missing or the
//! token is invalid/revoked.
//!
//! Port of `com.mcmanager.server.web.controller.ConsoleController`.

use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{Path, Query, State};
use axum::http::HeaderMap;
use axum::response::IntoResponse;
use futures_util::{SinkExt, StreamExt};

use std::sync::Arc;

use crate::auth::auth_service::AuthService;
use crate::auth::jwt;
use crate::auth::sessions::SessionRegistry;
use crate::instance::ServerInstanceManager;
use crate::process::console::ConsoleStreamHandler;
use crate::process::manager::MinecraftProcessManager;
use crate::web::app::{ApiError, AppState};

/// Query parameters for the WebSocket console upgrade route.
#[derive(Debug, serde::Deserialize, Default)]
pub struct ConsoleQuery {
    pub instance: Option<String>,
}

/// WebSocket upgrade route `/api/console`.
///
/// CSWSH defense: the `Origin` header is validated during the HTTP upgrade
/// handshake. Browsers always send `Origin` on WebSocket connects, so a page
/// on an attacker-controlled site can never hijack the console — its origin is
/// rejected with 401 before the socket is upgraded. Clients that omit the
/// header entirely (the Tauri shell, other non-browser tooling) are unaffected:
/// they still authenticate with their first message.
pub async fn console_ws(
    State(state): State<AppState>,
    Query(query): Query<ConsoleQuery>,
    headers: HeaderMap,
    ws: WebSocketUpgrade,
) -> Result<impl IntoResponse, ApiError> {
    validate_origin_or_error(&headers, &state)?;
    Ok(ws.on_upgrade(move |socket| handle_console_socket(socket, state, query.instance)))
}

/// WebSocket upgrade route `/api/instances/:id/console`.
///
/// Connects directly to the specified server instance's console.
pub async fn instance_console_ws(
    State(state): State<AppState>,
    Path(id): Path<String>,
    headers: HeaderMap,
    ws: WebSocketUpgrade,
) -> Result<impl IntoResponse, ApiError> {
    validate_origin_or_error(&headers, &state)?;
    Ok(ws.on_upgrade(move |socket| handle_console_socket(socket, state, Some(id))))
}

/// Validates the `Origin` header during WebSocket upgrades (CSWSH defense).
fn validate_origin_or_error(headers: &HeaderMap, state: &AppState) -> Result<(), ApiError> {
    if let Some(origin_header) = headers.get("origin").and_then(|o| o.to_str().ok()) {
        let config = state.config.get_config();
        let is_allowed = is_allowed_origin(origin_header, config.web_port, config.public_port);
        if !is_allowed {
            state.audit.log(
                "ANONYMOUS",
                "CSWSH_BLOCKED",
                &format!("Blocked unauthorized WebSocket upgrade from origin: {origin_header}"),
            );
            return Err(ApiError::Unauthorized(
                "Cross-Origin WebSocket request denied".into(),
            ));
        }
    }
    Ok(())
}

/// Whether a WebSocket handshake `Origin` is trusted.
///
/// CSWSH defense-in-depth on top of the first-message JWT authentication:
/// browser clients behind reverse proxies (arbitrary hostnames, ports 80/443)
/// must be able to connect, while origins that are neither loopback, private
/// LAN, nor a matching web/public port are still rejected. The embedded Tauri
/// frontend schemes and non-browser clients that omit `Origin` (or send
/// `null`) are always accepted.
fn is_allowed_origin(origin: &str, web_port: i32, public_port: i32) -> bool {
    let clean = origin.trim().to_lowercase();
    // Non-browser clients (Tauri shell, CLI tooling) may omit Origin entirely
    // or send the literal "null" (e.g. sandboxed iframes / file:// pages).
    // They still authenticate with their first message.
    if clean.is_empty() || clean == "null" {
        return true;
    }
    // Embedded Tauri frontend schemes (Windows/Linux: `tauri://localhost`,
    // macOS: `http://tauri.localhost`).
    if clean == "tauri://localhost"
        || clean == "http://tauri.localhost"
        || clean.starts_with("tauri://")
    {
        return true;
    }

    let Ok(parsed) = url::Url::parse(&clean) else {
        return false;
    };
    let Some(host) = parsed.host_str() else {
        return false;
    };

    // Reverse proxies forward the origin's default port (80/443) when the
    // host header carries no port, and the admin UI can also be served on the
    // web/public ports through a proxy that keeps the port.
    let port = parsed
        .port()
        .unwrap_or(if parsed.scheme() == "https" { 443 } else { 80 });
    if port == web_port as u16 || port == public_port as u16 || port == 443 || port == 80 {
        return true;
    }

    // Loopback and private LAN hosts are accepted regardless of port: an
    // attacker-controlled page can never be served from these hosts.
    let is_private = host == "localhost"
        || host == "127.0.0.1"
        || host == "::1"
        || host.starts_with("192.168.")
        || host.starts_with("10.")
        || (host.starts_with("172.")
            && host
                .split('.')
                .nth(1)
                .and_then(|octet| octet.parse::<u8>().ok())
                .is_some_and(|o| (16..=31).contains(&o)));
    is_private
}

async fn handle_console_socket(
    socket: WebSocket,
    state: AppState,
    target_instance_id: Option<String>,
) {
    let (mut sender, mut receiver) = socket.split();

    // The first inbound message must authenticate: "AUTH <jwt>". Nothing is
    // streamed until this succeeds, so an unauthenticated connection learns
    // nothing about the console. A 5-second deadline stops Slowloris-style
    // sockets that connect and never send the auth message.
    let first_msg = tokio::time::timeout(std::time::Duration::from_secs(5), receiver.next()).await;

    let username = match first_msg {
        Ok(Some(Ok(Message::Text(text)))) => {
            authenticate_console_user(parse_auth_message(&text), &state.sessions, &state.auth)
        }
        _ => None,
    };

    let Some(user) = username else {
        let _ = sender
            .send(Message::Text(
                "[wrapper] Authentication failed or timed out — connection closed.".to_string(),
            ))
            .await;
        let _ = sender.close().await;
        return;
    };

    // Determine target console stream handler
    let target_console: Arc<ConsoleStreamHandler> = if let Some(ref id) = target_instance_id {
        match state.instances.get_or_create_console(id) {
            Ok(c) => c,
            Err(e) => {
                let _ = sender
                    .send(Message::Text(format!("[wrapper] {e}")))
                    .await;
                let _ = sender.close().await;
                return;
            }
        }
    } else if let Some(active_cfg) = state.instances.get_active_instance() {
        state
            .instances
            .get_or_create_console(&active_cfg.id)
            .unwrap_or_else(|_| state.console.clone())
    } else {
        state.console.clone()
    };

    // Every console action is now attributable to the authenticated admin.
    state.audit.log(
        &user,
        "WS_CONSOLE_CONNECT",
        &format!(
            "WebSocket console session established{}",
            target_instance_id
                .as_deref()
                .map(|id| format!(" for instance {id}"))
                .unwrap_or_default()
        ),
    );
    let mut broadcast_rx = target_console.subscribe();

    // Replay recent history so the UI is not blank on connect (last 500 lines).
    for line in target_console.recent_history(500) {
        if sender.send(Message::Text(line)).await.is_err() {
            return;
        }
    }

    loop {
        tokio::select! {
            // Outbound: console lines → client.
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
            // Inbound: client messages → audit trail + server stdin commands.
            msg = receiver.next() => {
                match msg {
                    Some(Ok(Message::Text(text))) => {
                        match apply_inbound_message(
                            &state.audit,
                            &target_console,
                            &state.instances,
                            &state.process_manager,
                            &user,
                            &text,
                            target_instance_id.as_deref(),
                        )
                        .await
                        {
                            InboundResult::Ok => {}
                            InboundResult::Notify(message) => {
                                if sender.send(Message::Text(message)).await.is_err() {
                                    break;
                                }
                            }
                        }
                    }
                    Some(Ok(Message::Close(_))) | None => {
                        state.audit.log(
                            &user,
                            "WS_CONSOLE_DISCONNECT",
                            &format!(
                                "WebSocket console disconnected{}",
                                target_instance_id
                                    .as_deref()
                                    .map(|id| format!(" for instance {id}"))
                                    .unwrap_or_default()
                            ),
                        );
                        break;
                    }
                    Some(Err(_)) => break,
                    _ => break,
                }
            }
        }
    }
}

/// Outcome of applying one inbound console message.
#[derive(Debug, PartialEq, Eq)]
enum InboundResult {
    /// Message handled; the connection stays open.
    Ok,
    /// The client must be sent this reply, then the connection stays open.
    Notify(String),
}

/// One inbound console message, classified before any side effect runs so the
/// audit trail can record it under the authenticated username.
#[derive(Debug, PartialEq, Eq)]
enum InboundAction {
    Clear,
    Command(String),
    Nothing,
}

fn classify_inbound(text: &str) -> InboundAction {
    let trimmed = text.trim();
    if trimmed == "__CLEAR__" {
        InboundAction::Clear
    } else if trimmed.is_empty() {
        InboundAction::Nothing
    } else {
        InboundAction::Command(trimmed.to_string())
    }
}

/// Applies one inbound console message: audit-logs it under `user`, then
/// performs the side effect. `__CLEAR__` is echoed back to the client so every
/// connected session clears its view; a failed command is reported back.
/// Separated from the socket loop so the audit/identity binding is testable.
async fn apply_inbound_message(
    audit: &crate::audit::AuditLogger,
    console: &ConsoleStreamHandler,
    instances: &ServerInstanceManager,
    process_manager: &MinecraftProcessManager,
    user: &str,
    text: &str,
    target_instance_id: Option<&str>,
) -> InboundResult {
    match classify_inbound(text) {
        InboundAction::Clear => {
            audit.log(user, "CONSOLE_CLEAR", "Console history cleared");
            console.clear_history();
            InboundResult::Notify("__CLEAR__".to_string())
        }
        InboundAction::Command(command) => {
            audit.log(user, "CONSOLE_COMMAND", &command);

            // Route command to the specified instance's process manager when
            // targeted, or active instance's process manager when in multi-instance mode,
            // falling back to any instance currently running, then to the legacy process manager.
            let target_pm: Option<Arc<MinecraftProcessManager>> =
                if let Some(id) = target_instance_id {
                    instances.get_process_manager(id)
                } else if let Some(active_cfg) = instances.get_active_instance() {
                    instances.get_process_manager(&active_cfg.id)
                } else {
                    instances
                        .list_instances()
                        .into_iter()
                        .find(|inst| instances.is_running(&inst.id))
                        .and_then(|inst| instances.get_process_manager(&inst.id))
                };

            let send_result = if let Some(pm) = target_pm {
                pm.send_command(&command).await
            } else {
                process_manager.send_command(&command).await
            };

            match send_result {
                Ok(()) => InboundResult::Ok,
                Err(e) => InboundResult::Notify(format!("[wrapper] {e}")),
            }
        }
        InboundAction::Nothing => InboundResult::Ok,
    }
}

/// Extracts the JWT from a first-message auth payload (`AUTH <jwt>`), or
/// `None` for anything else (a raw command, `__CLEAR__`, empty...).
fn parse_auth_message(message: &str) -> Option<&str> {
    message
        .strip_prefix("AUTH ")
        .map(str::trim)
        .filter(|t| !t.is_empty())
}

/// Authenticates a console session from its first message: the token must
/// decode to a JWT whose subject is a real user and whose `jti` is not
/// revoked. Returns the authenticated username so every action can be bound
/// to it in the audit trail.
fn authenticate_console_user(
    token: Option<&str>,
    sessions: &SessionRegistry,
    auth: &AuthService,
) -> Option<String> {
    let token = token?;
    let claims = jwt::decode_claims(token)?;
    if sessions.is_revoked(&claims.jti) {
        return None;
    }
    if auth.get_user(&claims.sub).is_none() {
        return None;
    }
    Some(claims.sub)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audit::AuditLogger;
    use crate::auth::auth_service::AuthService;
    use crate::auth::jwt;
    use crate::auth::sessions::SessionRegistry;
    use crate::config::ConfigService;
    use crate::instance::ServerInstanceManager;
    use crate::process::console::ConsoleStreamHandler;
    use crate::process::manager::MinecraftProcessManager;
    use std::sync::Arc;

    fn temp_dir() -> std::path::PathBuf {
        crate::test_util::temp_dir("console")
    }

    #[test]
    fn origin_validation_accepts_admin_and_tauri_origins() {
        let (web, public) = (25564, 25565);
        // Admin UI on the web port (http/https, loopback hosts).
        assert!(is_allowed_origin(
            &format!("http://127.0.0.1:{web}"),
            web,
            public
        ));
        assert!(is_allowed_origin(
            &format!("http://localhost:{web}"),
            web,
            public
        ));
        assert!(is_allowed_origin(
            &format!("https://127.0.0.1:{web}"),
            web,
            public
        ));
        assert!(is_allowed_origin(
            &format!("https://localhost:{web}"),
            web,
            public
        ));
        // Origins proxied via the public port.
        assert!(is_allowed_origin(
            &format!("http://127.0.0.1:{public}"),
            web,
            public
        ));
        assert!(is_allowed_origin(
            &format!("http://localhost:{public}"),
            web,
            public
        ));
        // Embedded Tauri frontend schemes.
        assert!(is_allowed_origin("tauri://localhost", web, public));
        assert!(is_allowed_origin("http://tauri.localhost", web, public));
        // Case and surrounding whitespace are normalized.
        assert!(is_allowed_origin(
            &format!("  HTTP://LOCALHOST:{web}  "),
            web,
            public
        ));
    }

    #[test]
    fn origin_validation_accepts_loopback_lan_and_proxy_origins() {
        let (web, public) = (25564, 25565);
        // Loopback and private LAN origins regardless of port.
        assert!(is_allowed_origin("http://127.0.0.1:9999", web, public));
        assert!(is_allowed_origin("http://127.0.0.1:25566", web, public));
        assert!(is_allowed_origin("http://192.168.1.50:8080", web, public));
        assert!(is_allowed_origin("http://10.0.0.7:25564", web, public));
        assert!(is_allowed_origin("http://172.20.0.3:80", web, public));
        // Non-browser / sandboxed clients that omit or null the Origin.
        assert!(is_allowed_origin("", web, public));
        assert!(is_allowed_origin("null", web, public));
        // Reverse-proxy hostnames on default HTTP(S) ports.
        assert!(is_allowed_origin("https://mc.example.com", web, public));
        assert!(is_allowed_origin("http://mc.example.com", web, public));
        assert!(is_allowed_origin(
            "https://mc.example.com:25564",
            web,
            public
        ));
        assert!(is_allowed_origin(
            "http://mc.example.com:25565",
            web,
            public
        ));
    }

    #[test]
    fn origin_validation_rejects_cross_site_and_lookalikes() {
        let (web, public) = (25564, 25565);
        // Non-loopback, non-LAN host on an unrelated port is rejected.
        assert!(!is_allowed_origin(
            "http://evil.example.com:4444",
            web,
            public
        ));
        // Unparseable values are rejected.
        assert!(!is_allowed_origin("not a url", web, public));
        // Exact-match only: lookalike hosts must never pass.
        assert!(!is_allowed_origin(
            "http://127.0.0.1:25564.evil.com",
            web,
            public
        ));
        assert!(!is_allowed_origin(
            "http://localhost:25564.evil.com",
            web,
            public
        ));
        // Public (non-private) 172.x ranges are not treated as LAN.
        assert!(!is_allowed_origin("http://172.32.0.1:8080", web, public));
        assert!(!is_allowed_origin("http://172.15.0.1:8080", web, public));
    }

    #[test]
    fn parses_auth_messages() {
        assert_eq!(Some("abc"), parse_auth_message("AUTH abc"));
        assert_eq!(Some("abc"), parse_auth_message("AUTH  abc "));
        assert_eq!(None, parse_auth_message("AUTH"));
        assert_eq!(None, parse_auth_message("AUTH "));
        assert_eq!(None, parse_auth_message("__CLEAR__"));
        assert_eq!(None, parse_auth_message("say hello"));
        assert_eq!(None, parse_auth_message(""));
    }

    #[test]
    fn classifies_inbound_messages() {
        assert_eq!(InboundAction::Clear, classify_inbound("__CLEAR__"));
        assert_eq!(InboundAction::Clear, classify_inbound("  __CLEAR__  "));
        assert_eq!(
            InboundAction::Command("say hello".to_string()),
            classify_inbound("say hello")
        );
        assert_eq!(InboundAction::Nothing, classify_inbound(""));
        assert_eq!(InboundAction::Nothing, classify_inbound("   "));
    }

    #[test]
    fn rejects_garbage_but_accepts_fresh_tokens() {
        let dir = temp_dir();
        jwt::initialize(&dir).unwrap();
        let auth = AuthService::initialize(&dir).unwrap();
        let sessions = SessionRegistry::new();

        assert!(authenticate_console_user(None, &sessions, &auth).is_none());
        assert!(authenticate_console_user(Some(""), &sessions, &auth).is_none());
        assert!(authenticate_console_user(Some("garbage"), &sessions, &auth).is_none());

        // A freshly issued token for a real user is accepted and the username
        // is recovered for the audit trail...
        let token = jwt::generate_token("admin");
        let claims = jwt::decode_claims(&token).unwrap();
        sessions.register(&claims.jti, "admin", claims.exp);
        assert_eq!(
            Some("admin".to_string()),
            authenticate_console_user(Some(&token), &sessions, &auth)
        );

        // ...revoking it kills the session...
        sessions.revoke(&claims.jti, "admin", claims.exp);
        assert!(authenticate_console_user(Some(&token), &sessions, &auth).is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn rejects_tokens_for_unknown_users() {
        let dir = temp_dir();
        jwt::initialize(&dir).unwrap();
        let auth = AuthService::initialize(&dir).unwrap();
        let sessions = SessionRegistry::new();

        // A cryptographically valid token for a user that no longer exists
        // must not authenticate (deleted-account edge case).
        let token = jwt::generate_token("ghost");
        let claims = jwt::decode_claims(&token).unwrap();
        sessions.register(&claims.jti, "ghost", claims.exp);
        assert!(authenticate_console_user(Some(&token), &sessions, &auth).is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn clear_and_command_actions_are_audited_with_the_username() {
        let dir = temp_dir();
        let config =
            Arc::new(ConfigService::load_with_data_dir(Some(dir.display().to_string())).unwrap());
        let console = Arc::new(ConsoleStreamHandler::new());
        let instances = Arc::new(ServerInstanceManager::new(&dir, console.clone()).unwrap());
        let process_manager = MinecraftProcessManager::legacy(config, console.clone());
        let audit = AuditLogger::new(&dir);

        // `__CLEAR__` is audited under the authenticated username and echoed.
        let result = apply_inbound_message(
            &audit,
            &console,
            &instances,
            &process_manager,
            "alice",
            "__CLEAR__",
            None,
        )
        .await;
        assert_eq!(InboundResult::Notify("__CLEAR__".to_string()), result);
        assert!(console.recent_history(10).is_empty());

        // A command is audited before it is executed (the server is not
        // running here, so it is reported back — the audit entry still lands).
        let result = apply_inbound_message(
            &audit,
            &console,
            &instances,
            &process_manager,
            "alice",
            "say hello",
            None,
        )
        .await;
        assert!(matches!(result, InboundResult::Notify(_)));

        // Empty payloads are ignored without touching the audit trail.
        let result = apply_inbound_message(
            &audit,
            &console,
            &instances,
            &process_manager,
            "alice",
            "  ",
            None,
        )
        .await;
        assert_eq!(InboundResult::Ok, result);

        let content = std::fs::read_to_string(dir.join("audit.log")).unwrap();
        assert!(content.contains("[USER:alice] [CONSOLE_CLEAR] Console history cleared"));
        assert!(content.contains("[USER:alice] [CONSOLE_COMMAND] say hello"));
        assert!(
            !content.contains("[USER:ADMIN_WS]"),
            "audit entries must carry the real username, not a placeholder"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn instances_have_isolated_consoles() {
        let dir = temp_dir();
        let console = Arc::new(ConsoleStreamHandler::new());
        let instances = Arc::new(ServerInstanceManager::new(&dir, console.clone()).unwrap());

        let inst1 = instances
            .create_instance("Server1", "1.21.4", "fabric", "0.16.9")
            .unwrap();
        let inst2 = instances
            .create_instance("Server2", "1.21.4", "fabric", "0.16.9")
            .unwrap();

        let console1 = instances.get_or_create_console(&inst1.id).unwrap();
        let console2 = instances.get_or_create_console(&inst2.id).unwrap();

        console1.accept("[Server1] Log line 1".to_string());
        console2.accept("[Server2] Log line 2".to_string());

        let history1 = console1.recent_history(10);
        let history2 = console2.recent_history(10);
        let shared_history = console.recent_history(10);

        assert_eq!(vec!["[Server1] Log line 1"], history1);
        assert_eq!(vec!["[Server2] Log line 2"], history2);
        // Shared console does not get contaminated with instance logs
        assert!(shared_history.is_empty());

        let _ = std::fs::remove_dir_all(&dir);
    }
}
