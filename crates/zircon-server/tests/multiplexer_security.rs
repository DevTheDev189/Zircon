//! Transport-layer security integration tests (Phase 5 matrix): the TCP
//! multiplexer's real-IP handling and the wakeup endpoint's rate limiting.
//!
//! `strips_incoming_x_zircon_real_ip` runs the FULL stack — a real web router
//! bound on a loopback port behind a real `TcpMultiplexer` listener — and
//! proves a client-supplied `X-Zircon-Real-IP` header cannot influence the
//! rate-limiter keying: every proxied request is keyed on the actual socket
//! IP. `wakeup_endpoint_rate_limited` proves the 31st wakeup from one IP is
//! rejected with HTTP 429.

mod common;

use std::sync::Arc;
use std::time::Duration;

use serde_json::json;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

use zircon_server::config::ConfigService;
use zircon_server::multiplexer::tcp::TcpMultiplexer;
use zircon_server::tickets::JoinTicketManager;

use common::temp_dir;

/// Serializes the tests in this binary: they bind real ports.
static PORT_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

/// Non-default ports so this binary never collides with the lib's unit tests
/// (which bind 25564/25565/25700+ under their own lock).
const WEB_PORT: u16 = 27864;
const PUBLIC_PORT: u16 = 27865;

/// Spins up the full stack: the Axum router bound on `WEB_PORT` (loopback-only
/// web server with `ConnectInfo`), and the `TcpMultiplexer` listening on
/// `PUBLIC_PORT` and proxying HTTP to the web port.
async fn start_stack(max_join_intents: u32) -> (Arc<TcpMultiplexer>, tokio::task::JoinHandle<()>) {
    let dir = temp_dir("mux-sec");
    let config = Arc::new(
        ConfigService::load_with_data_dir(Some(dir.display().to_string())).expect("config load"),
    );
    config.with_config(|c| {
        c.web_port = WEB_PORT as i32;
        c.public_port = PUBLIC_PORT as i32;
    });

    let app = common::test_app_with_limits(10, max_join_intents);
    let listener = tokio::net::TcpListener::bind(("127.0.0.1", WEB_PORT))
        .await
        .expect("bind web port");
    let web_handle = tokio::spawn(async move {
        axum::serve(
            listener,
            app.into_make_service_with_connect_info::<std::net::SocketAddr>(),
        )
        .await
        .expect("web server failed");
    });

    let tickets = Arc::new(JoinTicketManager::new());
    let multiplexer = Arc::new(TcpMultiplexer::new(config.clone(), None, tickets));
    multiplexer.start().expect("start multiplexer");
    (multiplexer, web_handle)
}

/// Sends one raw HTTP POST to the multiplexer's public port and returns the
/// response status code. `spoof` optionally injects an attacker-supplied
/// `X-Zircon-Real-IP` header.
async fn raw_wakeup_post(port: u16, spoof: Option<&str>) -> u16 {
    let mut stream = tokio::time::timeout(
        Duration::from_secs(5),
        TcpStream::connect(("127.0.0.1", port)),
    )
    .await
    .expect("connect to multiplexer")
    .expect("connect to multiplexer");

    let body = json!({ "hostname": "localhost", "port": 25565 }).to_string();
    let mut request = format!(
        "POST /api/wakeup HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\n\
         Content-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n",
        body.len()
    );
    if let Some(spoof) = spoof {
        request.push_str(&format!("X-Zircon-Real-IP: {spoof}\r\n"));
    }
    request.push_str("\r\n");
    request.push_str(&body);

    stream
        .write_all(request.as_bytes())
        .await
        .expect("write request");
    let mut response = Vec::new();
    let _ = tokio::time::timeout(Duration::from_secs(5), stream.read_to_end(&mut response)).await;
    let text = String::from_utf8_lossy(&response);
    let status_line = text.lines().next().unwrap_or("");
    status_line
        .split_whitespace()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(0)
}

/// A proxied request must reflect the actual socket IP, ignoring any client
/// header. Proven via the rate limiter: 30 requests from the same socket IP —
/// each carrying a DIFFERENT spoofed `X-Zircon-Real-IP` — must all count
/// against ONE bucket, so the 31st request is rejected with 429. If the
/// spoofed header were honored, every request would key a fresh bucket and
/// none would trip the limit.
#[tokio::test]
async fn strips_incoming_x_zircon_real_ip() {
    let _guard = PORT_LOCK.lock().await;
    let (multiplexer, web_handle) = start_stack(30).await;

    let mut statuses = Vec::new();
    for i in 0..31u32 {
        let spoof = format!("203.0.113.{i}");
        statuses.push(raw_wakeup_post(PUBLIC_PORT, Some(&spoof)).await);
    }
    assert_eq!(
        429,
        statuses[30],
        "31st request must trip the shared socket-IP bucket (spoofed headers ignored): {statuses:?}"
    );
    // The requests actually got proxied and answered (not dropped).
    assert!(
        statuses.iter().take(30).all(|s| *s != 0),
        "requests must be proxied through to the web server: {statuses:?}"
    );

    multiplexer.stop();
    web_handle.abort();
}

/// The 31st wakeup request from the same IP returns HTTP 429 (the wakeup
/// endpoint shares the join-intent limiter: 30 per 60s window per real IP).
#[tokio::test]
async fn wakeup_endpoint_rate_limited() {
    let app = common::test_app_with_limits(10, 30);
    let body = json!({ "hostname": "localhost", "port": 25565 });

    let mut last: u16 = 0;
    for _ in 0..31 {
        let (status, _) = common::send_from(
            &app,
            "127.0.0.1",
            "POST",
            "/api/wakeup",
            None,
            Some(body.clone()),
        )
        .await;
        last = status.as_u16();
    }
    assert_eq!(
        429, last,
        "31st wakeup from the same IP must be rate limited"
    );
}
