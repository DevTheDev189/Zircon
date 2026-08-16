//! WebSocket endpoint `/api/console`. Server console lines are streamed to
//! every connected session; messages sent by a client are written to the server
//! stdin as commands (this is how the admin UI sends "whitelist add X" etc.).
//!
//! Port of `com.mcmanager.server.web.controller.ConsoleController`.

use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::State;
use axum::response::IntoResponse;
use futures_util::{SinkExt, StreamExt};

use crate::web::app::AppState;

/// WebSocket upgrade route `/api/console`.
pub async fn console_ws(State(state): State<AppState>, ws: WebSocketUpgrade) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_console_socket(socket, state))
}

async fn handle_console_socket(socket: WebSocket, state: AppState) {
    let (mut sender, mut receiver) = socket.split();
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
