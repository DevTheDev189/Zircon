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
use axum::extract::State;
use axum::response::IntoResponse;
use futures_util::{SinkExt, StreamExt};

use crate::auth::auth_service::AuthService;
use crate::auth::jwt;
use crate::auth::sessions::SessionRegistry;
use crate::process::console::ConsoleStreamHandler;
use crate::process::manager::MinecraftProcessManager;
use crate::web::app::AppState;

/// WebSocket upgrade route `/api/console`.
pub async fn console_ws(State(state): State<AppState>, ws: WebSocketUpgrade) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_console_socket(socket, state))
}

async fn handle_console_socket(socket: WebSocket, state: AppState) {
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

    // Every console action is now attributable to the authenticated admin.
    state.audit.log(
        &user,
        "WS_CONSOLE_CONNECT",
        "WebSocket console session established",
    );
    let mut broadcast_rx = state.console.subscribe();

    // Replay recent history so the UI is not blank on connect (last 500 lines).
    for line in state.console.recent_history(500) {
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
                            &state.console,
                            &state.process_manager,
                            &user,
                            &text,
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
                            "WebSocket console disconnected",
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
    process_manager: &MinecraftProcessManager,
    user: &str,
    text: &str,
) -> InboundResult {
    match classify_inbound(text) {
        InboundAction::Clear => {
            audit.log(user, "CONSOLE_CLEAR", "Console history cleared");
            console.clear_history();
            InboundResult::Notify("__CLEAR__".to_string())
        }
        InboundAction::Command(command) => {
            audit.log(user, "CONSOLE_COMMAND", &command);
            match process_manager.send_command(&command).await {
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
    use crate::process::console::ConsoleStreamHandler;
    use crate::process::manager::MinecraftProcessManager;
    use std::sync::Arc;

    fn temp_dir() -> std::path::PathBuf {
        crate::test_util::temp_dir("console")
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
        let process_manager = MinecraftProcessManager::legacy(config, console.clone());
        let audit = AuditLogger::new(&dir);

        // `__CLEAR__` is audited under the authenticated username and echoed.
        let result =
            apply_inbound_message(&audit, &console, &process_manager, "alice", "__CLEAR__").await;
        assert_eq!(InboundResult::Notify("__CLEAR__".to_string()), result);
        assert!(console.recent_history(10).is_empty());

        // A command is audited before it is executed (the server is not
        // running here, so it is reported back — the audit entry still lands).
        let result =
            apply_inbound_message(&audit, &console, &process_manager, "alice", "say hello").await;
        assert!(matches!(result, InboundResult::Notify(_)));

        // Empty payloads are ignored without touching the audit trail.
        let result = apply_inbound_message(&audit, &console, &process_manager, "alice", "  ").await;
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
}
