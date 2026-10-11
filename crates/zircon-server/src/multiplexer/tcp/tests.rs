use std::sync::Arc;
use std::time::Duration;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use zircon_core::model::InstanceConfig;

use crate::config::ConfigService;
use crate::instance::ServerInstanceManager;
use crate::multiplexer::varint::write_varint;
use crate::process::console::ConsoleStreamHandler;
use crate::tickets::JoinTicketManager;

use super::*;

static MUX_TEST_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());


    fn temp_dir() -> std::path::PathBuf {
        crate::test_util::temp_dir("mux")
    }

    fn config_at(dir: &std::path::Path) -> Arc<ConfigService> {
        Arc::new(
            ConfigService::load_with_data_dir(Some(dir.to_string_lossy().into_owned())).unwrap(),
        )
    }

    /// Allocates an ephemeral test port on localhost.
    async fn free_port() -> u16 {
        let socket = TcpListener::bind(("127.0.0.1", 0)).await.expect("ephemeral port binding failed");
        let assigned_port = socket.local_addr().expect("address retrieval").port();
        drop(socket);
        assigned_port
    }

    /// Builds a valid handshake frame: [VarInt len][VarInt 0x00][protocol][host][u16 port][nextState].
    fn handshake_frame(hostname: &str, next_state: i32) -> Vec<u8> {
        let mut payload = Vec::new();
        write_varint(&mut payload, 0); // packet id
        write_varint(&mut payload, 754); // protocol
        write_varint(&mut payload, hostname.len() as i32);
        payload.extend_from_slice(hostname.as_bytes());
        payload.extend_from_slice(&25565u16.to_be_bytes());
        write_varint(&mut payload, next_state);
        let mut out = Vec::new();
        write_varint(&mut out, payload.len() as i32);
        out.extend_from_slice(&payload);
        out
    }

    /// Builds a login start frame: [VarInt len][VarInt 0x00][VarInt nameLen][name].
    fn login_start_frame(username: &str) -> Vec<u8> {
        let mut payload = Vec::new();
        write_varint(&mut payload, 0); // packet id
        write_varint(&mut payload, username.len() as i32);
        payload.extend_from_slice(username.as_bytes());
        let mut out = Vec::new();
        write_varint(&mut out, payload.len() as i32);
        out.extend_from_slice(&payload);
        out
    }

    #[tokio::test]
    async fn http_traffic_is_proxied_to_the_web_port() {
        let _guard = MUX_TEST_LOCK.lock().await;
        let dir = temp_dir();
        let config = config_at(&dir);
        let web_port = free_port().await;
        config.with_config(|c| c.web_port = web_port as i32);
        let tickets = Arc::new(JoinTicketManager::new());
        let multiplexer = TcpMultiplexer::new(config.clone(), None, tickets);

        // Start a fake "web server" on the configured web port.
        let web_listener = TcpListener::bind(("127.0.0.1", web_port)).await.unwrap();
        let web_handle = tokio::spawn(async move {
            let (mut socket, _) = web_listener.accept().await.unwrap();
            let mut buf = [0u8; 256];
            let n = socket.read(&mut buf).await.unwrap();
            socket
                .write_all(b"HTTP/1.1 200 OK\r\ncontent-length: 2\r\n\r\nok")
                .await
                .unwrap();
            (String::from_utf8_lossy(&buf[..n]).to_string(), n)
        });

        let main_port = free_port().await;
        let handle = multiplexer.spawn_listener(main_port, None);
        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

        let mut client = TcpStream::connect(("127.0.0.1", main_port)).await.unwrap();
        client
            .write_all(b"GET /index.html HTTP/1.1\r\nHost: localhost\r\n\r\n")
            .await
            .unwrap();
        let mut response = Vec::new();
        client.read_to_end(&mut response).await.unwrap();
        assert!(String::from_utf8_lossy(&response).contains("200 OK"));

        let (request, _) = web_handle.await.unwrap();
        assert!(request.starts_with("GET /index.html"));
        // The real client IP is forwarded (the test client connects from
        // loopback) and keep-alive is disabled so every proxied request
        // carries the header.
        assert!(
            request.contains("X-Zircon-Real-IP: 127.0.0.1"),
            "proxied request must carry the real client IP: {request:?}"
        );
        assert!(
            request.contains("Connection: close"),
            "proxied request must disable keep-alive: {request:?}"
        );
        handle.abort();
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn strips_spoofed_x_zircon_real_ip_and_connection_headers() {
        // The head is already complete in the buffer, so `prepare_http_forward`
        // never reads from the socket — a connected-but-silent pair suffices.
        let listener = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
        let addr = listener.local_addr().unwrap();
        let mut client = TcpStream::connect(addr).await.unwrap();

        // An attacker prefaces the request with forged trusted headers in
        // varying cases (headers are case-insensitive per RFC 9110).
        let spoofed = b"GET /index.html HTTP/1.1\r\n\
            Host: localhost\r\n\
            X-Zircon-Real-IP: 203.0.113.66\r\n\
            x-zircon-real-ip: 198.51.100.7\r\n\
            Connection: keep-alive\r\n\
            cOnNeCtIoN: keep-alive\r\n\
            User-Agent: minecraft\r\n\
            \r\n\
            body-bytes"
            .to_vec();

        let rewritten = prepare_http_forward(
            spoofed,
            &mut client,
            std::net::IpAddr::V4(std::net::Ipv4Addr::new(192, 0, 2, 1)),
        )
        .await;
        let text = String::from_utf8_lossy(&rewritten);

        assert!(text.starts_with("GET /index.html HTTP/1.1\r\n"));
        // The trusted header appears exactly once, carrying the socket's real
        // IP; the spoofed values are gone.
        assert_eq!(
            1,
            text.matches("X-Zircon-Real-IP").count(),
            "spoofed real-IP header must be stripped: {text:?}"
        );
        assert!(text.contains("X-Zircon-Real-IP: 192.0.2.1\r\n"));
        assert!(!text.contains("203.0.113.66"));
        assert!(!text.contains("198.51.100.7"));
        // Attacker keep-alive overrides are stripped; the proxy injects exactly
        // one `Connection: close`.
        assert_eq!(
            1,
            text.matches("Connection").count(),
            "spoofed connection headers must be stripped: {text:?}"
        );
        assert!(text.contains("Connection: close\r\n"));
        // Legitimate headers and any already-buffered body bytes are preserved.
        assert!(text.contains("Host: localhost\r\n"));
        assert!(text.contains("User-Agent: minecraft\r\n"));
        assert!(text.ends_with("body-bytes"));
    }

    #[tokio::test]
    async fn http_proxy_can_be_disabled_for_reverse_proxy_deployments() {
        let _guard = MUX_TEST_LOCK.lock().await;
        let dir = temp_dir();
        let config = config_at(&dir);
        config.with_config(|c| c.http_proxy = false);

        // An HTTP-looking request must NOT reach the web port when proxying is
        // disabled; it is treated as (invalid) Minecraft traffic and routed to
        // the MC backend instead. No web listener is bound — a proxy attempt
        // would fail with connection refused and surface in the test.
        let web_port = free_port().await;
        let mut mc_port = free_port().await;
        while mc_port == web_port {
            mc_port = free_port().await;
        }
        config.with_config(|c| {
            c.web_port = web_port as i32;
            c.mc_port = mc_port as i32;
        });
        let tickets = Arc::new(JoinTicketManager::new());
        let multiplexer = TcpMultiplexer::new(config.clone(), None, tickets);
        assert_ne!(web_port, mc_port);

        let mc_listener = TcpListener::bind(("127.0.0.1", mc_port)).await.unwrap();
        let mc_handle = tokio::spawn(async move {
            let (mut socket, _) = mc_listener.accept().await.unwrap();
            let mut buf = [0u8; 256];
            let n = socket.read(&mut buf).await.unwrap();
            socket.write_all(b"\x00").await.unwrap(); // nonsense reply; bytes are what matter
            (String::from_utf8_lossy(&buf[..n]).to_string(), n)
        });

        let main_port = free_port().await;
        let handle = multiplexer.spawn_listener(main_port, None);

        let mut client = TcpStream::connect(("127.0.0.1", main_port)).await.unwrap();
        client
            .write_all(b"GET /index.html HTTP/1.1\r\nHost: localhost\r\n\r\n")
            .await
            .unwrap();
        let mut response = Vec::new();
        let _ = client.read_to_end(&mut response).await;

        let (received, _) = mc_handle.await.unwrap();
        assert!(
            received.starts_with("GET "),
            "HTTP bytes must be routed to the MC backend, got: {received:?}"
        );
        handle.abort();
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn per_ip_connection_quota_drops_excess_connections() {
        let _guard = MUX_TEST_LOCK.lock().await;
        let dir = temp_dir();
        let config = config_at(&dir);
        let multiplexer =
            TcpMultiplexer::new(config.clone(), None, Arc::new(JoinTicketManager::new()));
        let main_port = config.get_config().public_port as u16;
        let handle = multiplexer.spawn_listener(main_port, None);

        // Fill the per-IP quota with idle sockets: they send nothing, so the
        // connection handlers stay alive in protocol detection and keep their
        // quota slots.
        let mut held = Vec::new();
        for _ in 0..MAX_CONNS_PER_IP {
            held.push(TcpStream::connect(("127.0.0.1", main_port)).await.unwrap());
        }

        // Give the accept loop time to accept and count every held socket.
        tokio::time::sleep(Duration::from_millis(200)).await;

        // The next connection from the same IP is dropped immediately (the
        // server closes it without writing anything).
        let mut excess = TcpStream::connect(("127.0.0.1", main_port)).await.unwrap();
        let mut byte = [0u8; 1];
        let read = tokio::time::timeout(Duration::from_secs(2), excess.read(&mut byte)).await;
        let read = read.expect("excess connection must be dropped promptly");
        assert!(
            matches!(read, Ok(0) | Err(_)),
            "over-quota connection should be closed by the server, got {read:?}"
        );

        // Dropping the held sockets releases their slots: the per-IP counter
        // drains and the map entry is cleaned up rather than left at zero.
        drop(held);
        tokio::time::sleep(Duration::from_millis(200)).await;
        assert!(
            multiplexer.ip_conns.is_empty(),
            "counters must drain on close"
        );

        handle.abort();
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// The flag wiring itself — deterministic, no sockets. The end-to-end
    /// proxy tests above exercise the full path where the platform allows it.
    #[test]
    fn http_proxy_flag_controls_the_routing_decision() {
        let dir = temp_dir();
        let config = config_at(&dir);
        let multiplexer =
            TcpMultiplexer::new(config.clone(), None, Arc::new(JoinTicketManager::new()));

        // Default (and legacy config files without the field): proxying on.
        assert!(multiplexer.http_proxy_enabled());

        // Reverse-proxy deployments turn it off.
        config.with_config(|c| c.http_proxy = false);
        assert!(!multiplexer.http_proxy_enabled());

        // Cloud fleet mode also turns it off unconditionally.
        config.with_config(|c| {
            c.http_proxy = true;
            c.cloud.enabled = true;
        });
        assert!(!multiplexer.http_proxy_enabled());

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn vanilla_login_without_ticket_is_disconnected() {
        let _guard = MUX_TEST_LOCK.lock().await;
        let dir = temp_dir();
        let config = config_at(&dir);
        let console = Arc::new(ConsoleStreamHandler::new());
        let instances = Arc::new(ServerInstanceManager::new(&dir, console).unwrap());
        let instance = instances
            .create_instance("Main", "1.20.4", "vanilla", "")
            .unwrap();

        let tickets = Arc::new(JoinTicketManager::new());
        let multiplexer =
            TcpMultiplexer::new(config.clone(), Some(instances.clone()), tickets.clone());

        // Register a ticket for "Steve" only.
        tickets.register_ticket("Steve");

        instances
            .update_join_gate_mode(&instance.id, zircon_core::model::instance::JoinGateMode::StrictTicket)
            .unwrap();

        let port = instance.external_mc_port as u16;
        let handle = multiplexer.spawn_listener(port, None);

        let mut frame = handshake_frame("main", 2); // login state
        frame.extend_from_slice(&login_start_frame("Alex"));

        // Alex has no ticket → expect a Disconnect frame carrying the Zircon
        // message back.
        let mut client = TcpStream::connect(("127.0.0.1", port)).await.unwrap();
        client.write_all(&frame).await.unwrap();
        let mut response = Vec::new();
        client.read_to_end(&mut response).await.unwrap();
        assert!(
            String::from_utf8_lossy(&response).contains("Zircon Client Required"),
            "expected a disconnect frame, got {:?}",
            response
        );
        handle.abort();
        let _ = std::fs::remove_dir_all(&dir);
    }

    // KNOWN FLAKY under a fully parallel `cargo test` run (default test
    // threading): the client's `read_to_end()` can race the backend task's
    // write and observe an empty response under heavy CPU contention from
    // the rest of the suite running concurrently. Passes reliably in
    // isolation and with `--test-threads=1` (confirmed: 136/136, no
    // failures). This is a pre-existing timing sensitivity in the test
    // itself (two independent tokio tasks with no synchronization beyond
    // the socket), not a port collision — left unfixed as out of scope;
    // a real fix would need the client to retry/wait rather than assume the
    // backend has already written by the time it reads.
    #[tokio::test]
    async fn ticketed_login_is_proxied_to_the_instance_backend() {
        let _guard = MUX_TEST_LOCK.lock().await;
        let dir = temp_dir();
        let config = config_at(&dir);
        let console = Arc::new(ConsoleStreamHandler::new());
        let instances = Arc::new(ServerInstanceManager::new(&dir, console).unwrap());
        let instance = instances
            .create_instance("Main", "1.20.4", "vanilla", "")
            .unwrap();

        // `create_instance` deterministically assigns the first instance
        // `EXTERNAL_PORT_BASE` (25565), which collides with a real Zircon
        // server that might already be running on this machine. This test
        // exercises the `None`-fixed-instance path, which routes by looking
        // up the *registered* external port, so reassign it to a genuinely
        // free one via the public API (keeps the in-memory registry and the
        // `port` variable below in sync).
        let external_port = free_port().await;
        instances
            .update_external_port(&instance.id, external_port as i32)
            .unwrap();

        let tickets = Arc::new(JoinTicketManager::new());
        tickets.register_ticket("Steve");
        let multiplexer = TcpMultiplexer::new(config.clone(), Some(instances.clone()), tickets);

        // A fake MC backend on the instance's internal port.
        let backend = TcpListener::bind(("127.0.0.1", instance.internal_mc_port as u16))
            .await
            .unwrap();
        let backend_handle = tokio::spawn(async move {
            let (mut socket, _) = backend.accept().await.unwrap();
            let mut buf = [0u8; 64];
            let n = socket.read(&mut buf).await.unwrap();
            socket.write_all(b"hello-backend").await.unwrap();
            (buf[..n].to_vec(), n)
        });

        let port = external_port;
        let handle = multiplexer.spawn_listener(port, None);

        let mut frame = handshake_frame("main", 2);
        frame.extend_from_slice(&login_start_frame("Steve"));
        let mut client = TcpStream::connect(("127.0.0.1", port)).await.unwrap();
        client.write_all(&frame).await.unwrap();
        let mut response = Vec::new();
        client.read_to_end(&mut response).await.unwrap();
        assert_eq!(b"hello-backend".to_vec(), response);

        let (received, _) = backend_handle.await.unwrap();
        assert!(!received.is_empty());
        handle.abort();
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn per_instance_port_routes_to_fixed_instance() {
        let _guard = MUX_TEST_LOCK.lock().await;
        let dir = temp_dir();
        let config = config_at(&dir);
        let console = Arc::new(ConsoleStreamHandler::new());
        let instances = Arc::new(ServerInstanceManager::new(&dir, console).unwrap());
        let instance = instances
            .create_instance("Alpha", "1.20.4", "vanilla", "")
            .unwrap();

        let tickets = Arc::new(JoinTicketManager::new());
        let multiplexer = TcpMultiplexer::new(config.clone(), Some(instances.clone()), tickets);

        // A fake MC backend on the instance's internal port.
        let backend = TcpListener::bind(("127.0.0.1", instance.internal_mc_port as u16))
            .await
            .unwrap();
        let backend_handle = tokio::spawn(async move {
            let (mut socket, _) = backend.accept().await.unwrap();
            let mut buf = [0u8; 64];
            let n = socket.read(&mut buf).await.unwrap();
            socket.write_all(b"hello-backend").await.unwrap();
            (buf[..n].to_vec(), n)
        });

        let port = instance.external_mc_port as u16;
        let handle = multiplexer.spawn_listener(port, Some(instance.clone()));

        // A status ping handshake (next state 1) — no ticket gate for status.
        let frame = handshake_frame("alpha", 1);
        let mut client = TcpStream::connect(("127.0.0.1", port)).await.unwrap();
        client.write_all(&frame).await.unwrap();
        let mut response = Vec::new();
        client.read_to_end(&mut response).await.unwrap();
        assert_eq!(b"hello-backend".to_vec(), response);

        let (received, _) = backend_handle.await.unwrap();
        assert!(!received.is_empty());
        handle.abort();
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn malformed_login_start_is_rejected_fail_closed() {
        let _guard = MUX_TEST_LOCK.lock().await;
        let dir = temp_dir();
        let config = config_at(&dir);
        let console = Arc::new(ConsoleStreamHandler::new());
        let instances = Arc::new(ServerInstanceManager::new(&dir, console).unwrap());
        let instance = instances
            .create_instance("Main", "1.20.4", "vanilla", "")
            .unwrap();

        let tickets = Arc::new(JoinTicketManager::new());
        let multiplexer = TcpMultiplexer::new(config.clone(), Some(instances.clone()), tickets);

        // A fake MC backend on the instance's internal port.
        let backend = TcpListener::bind(("127.0.0.1", instance.internal_mc_port as u16))
            .await
            .unwrap();

        let port = instance.external_mc_port as u16;
        let handle = multiplexer.spawn_listener(port, None);

        // Login-state handshake followed by a bogus "login start" frame whose
        // packet id is 0x01 (not 0x00) — `parse_login_start_username` reports
        // NotMatch, and the gate must now fail closed instead of proxying.
        let mut frame = handshake_frame("main", 2);
        let mut bogus = Vec::new();
        write_varint(&mut bogus, 1); // packet id 0x01
        write_varint(&mut bogus, 1);
        bogus.extend_from_slice(b"x");
        let mut payload = Vec::new();
        write_varint(&mut payload, bogus.len() as i32);
        payload.extend_from_slice(&bogus);
        frame.extend_from_slice(&payload);

        let mut client = TcpStream::connect(("127.0.0.1", port)).await.unwrap();
        client.write_all(&frame).await.unwrap();
        let mut response = Vec::new();
        client.read_to_end(&mut response).await.unwrap();
        assert!(
            String::from_utf8_lossy(&response).contains("Zircon Client Required"),
            "expected a fail-closed disconnect, got {:?}",
            response
        );

        // The forged frame must never be forwarded to the backend.
        let backend_conn = tokio::time::timeout(Duration::from_millis(300), backend.accept()).await;
        assert!(
            backend_conn.is_err(),
            "backend must not receive a proxied connection when login start is unparseable"
        );
        handle.abort();
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn stalled_connection_is_dropped_after_detection_timeout() {
        let dir = temp_dir();
        let config = config_at(&dir);
        let multiplexer =
            TcpMultiplexer::new(config.clone(), None, Arc::new(JoinTicketManager::new()));

        let listener = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
        let addr = listener.local_addr().unwrap();
        let mux_handle = tokio::spawn(async move {
            let (server, _) = listener.accept().await.unwrap();
            multiplexer
                .handle_connection_with_timeout(server, None, Duration::from_millis(100))
                .await
        });

        // Trickle an incomplete handshake: VarInt 127 promises 127 more bytes
        // that are never sent, so detection can never finish and must time out.
        let mut client = TcpStream::connect(addr).await.unwrap();
        client.write_all(&[0x7f]).await.unwrap();

        // The detection timeout must drop the socket: the read ends with EOF.
        let mut buf = [0u8; 16];
        let n = tokio::time::timeout(Duration::from_secs(5), client.read(&mut buf))
            .await
            .expect("socket must be closed after the detection timeout")
            .expect("read must not error");
        assert_eq!(0, n, "expected EOF after detection timeout");

        assert!(mux_handle.await.unwrap().is_ok());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn sleeping_instance_ping_returns_waking_status() {
        let _guard = MUX_TEST_LOCK.lock().await;
        let dir = temp_dir();
        let config = config_at(&dir);
        let console = Arc::new(ConsoleStreamHandler::new());
        let instances = Arc::new(ServerInstanceManager::new(&dir, console).unwrap());
        let instance = instances
            .create_instance("Emerald", "1.20.4", "fabric", "")
            .unwrap();

        // Put instance to sleep with idle reason
        instances
            .stop_instance_with_reason(&instance.id, Some(zircon_core::model::SHUTDOWN_REASON_IDLE))
            .await;

        let external_port = free_port().await;
        instances
            .update_external_port(&instance.id, external_port as i32)
            .unwrap();

        let tickets = Arc::new(JoinTicketManager::new());
        let multiplexer = TcpMultiplexer::new(config.clone(), Some(instances.clone()), tickets);
        let handle = multiplexer.spawn_listener(external_port, None);

        // Send status ping (next_state == 1) to "emerald.zirconmc.net"
        let frame = handshake_frame("emerald.zirconmc.net", 1);
        let mut client = TcpStream::connect(("127.0.0.1", external_port)).await.unwrap();
        client.write_all(&frame).await.unwrap();
        let mut response = Vec::new();
        client.read_to_end(&mut response).await.unwrap();

        let response_str = String::from_utf8_lossy(&response);
        assert!(response_str.contains("Sleeping"), "got: {response_str}");
        assert!(response_str.contains("Connect to wake up server!"), "got: {response_str}");

        handle.abort();
        let _ = std::fs::remove_dir_all(&dir);
    }

