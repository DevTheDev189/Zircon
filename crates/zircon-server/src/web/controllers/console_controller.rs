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

use crate::auth::jwt;
use crate::auth::sessions::SessionRegistry;
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
            // Inbound: client messages → server stdin commands.
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

/// Extracts the JWT from a first-message auth payload (`AUTH <jwt>`), or
/// `None` for anything else (a raw command, `__CLEAR__`, empty...).
fn parse_auth_message(message: &str) -> Option<&str> {
    message
        .strip_prefix("AUTH ")
        .map(str::trim)
        .filter(|t| !t.is_empty())
}

/// Validates a console-auth token: it must decode and must not be revoked.
/// Separated from the handler so the security decision is unit-testable.
fn validate_console_auth(token: Option<&str>, sessions: &SessionRegistry) -> bool {
    let Some(token) = token else {
        return false;
    };
    let Some(claims) = jwt::decode_claims(token) else {
        return false;
    };
    !sessions.is_revoked(&claims.jti)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::jwt;
    use crate::auth::sessions::SessionRegistry;

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
    fn rejects_garbage_but_accepts_fresh_tokens() {
        let dir = temp_dir();
        jwt::initialize(&dir).unwrap();
        let sessions = SessionRegistry::new();

        assert!(!validate_console_auth(None, &sessions));
        assert!(!validate_console_auth(Some(""), &sessions));
        assert!(!validate_console_auth(Some("garbage"), &sessions));

        // A freshly issued token is accepted, then revoked → rejected.
        let token = jwt::generate_token("admin");
        let claims = jwt::decode_claims(&token).unwrap();
        sessions.register(&claims.jti, "admin", claims.exp);
        assert!(validate_console_auth(Some(&token), &sessions));

        sessions.revoke(&claims.jti, "admin", claims.exp);
        assert!(!validate_console_auth(Some(&token), &sessions));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
