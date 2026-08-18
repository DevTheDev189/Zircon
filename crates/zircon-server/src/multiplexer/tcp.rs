//! Binds the public Minecraft port (25565) and runs protocol detection on every
//! accepted connection, proxying HTTP to the admin web server and Minecraft
//! traffic to the internal port of the instance whose name/id matches the
//! handshake hostname (or the legacy single-server MC port when no instance
//! manager is wired).
//!
//! Additionally binds one dedicated player-facing port per instance so every
//! server has a fixed, memorable address; those listeners route straight to the
//! instance's internal port and are bound/unbound as instances are
//! created/deleted via `PortBindingListener`.
//!
//! Port of `com.mcmanager.server.multiplexer.TcpMultiplexer` + `ProxyHandler`.

use std::collections::HashMap;
use std::io;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::task::JoinHandle;
use zircon_core::model::InstanceConfig;

use crate::config::ConfigService;
use crate::instance::{PortBindingListener, ServerInstanceManager};
use crate::multiplexer::detector::{self, ParseResult};
use crate::multiplexer::disconnect;
use crate::tickets::JoinTicketManager;

/// Backend host for the admin web server and the legacy MC server.
const BACKEND_HOST: &str = "127.0.0.1";

/// Max time a client may take to complete protocol detection before the socket
/// is dropped (Slowloris socket-starvation defense).
const PROTOCOL_DETECTION_TIMEOUT: Duration = Duration::from_secs(5);

/// The Tokio TCP multiplexer.
#[derive(Clone)]
pub struct TcpMultiplexer {
    config: Arc<ConfigService>,
    instances: Option<Arc<ServerInstanceManager>>,
    tickets: Arc<JoinTicketManager>,
    web_port: u16,
    mc_port: u16,
    bindings: Arc<Mutex<HashMap<String, JoinHandle<()>>>>,
}

impl TcpMultiplexer {
    pub fn new(
        config: Arc<ConfigService>,
        instances: Option<Arc<ServerInstanceManager>>,
        tickets: Arc<JoinTicketManager>,
    ) -> Self {
        let cfg = config.get_config();
        Self {
            config,
            instances,
            tickets,
            web_port: cfg.web_port as u16,
            mc_port: cfg.mc_port as u16,
            bindings: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Binds the public port and one dedicated player-facing port per existing
    /// instance.
    pub fn start(&self) -> io::Result<()> {
        let public_port = self.config.get_config().public_port as u16;
        self.spawn_listener(public_port, None);
        if let Some(instances) = &self.instances {
            for instance in instances.list_instances() {
                self.bind_instance(&instance);
            }
        }
        Ok(())
    }

    /// Binds a dedicated player-facing port proxying to the instance's internal
    /// MC port.
    pub fn bind_instance(&self, config: &InstanceConfig) {
        if config.external_mc_port <= 0 {
            tracing::warn!(
                "Instance '{}' has no external port assigned; skipping port binding",
                config.name
            );
            return;
        }
        if config.external_mc_port == self.config.get_config().public_port {
            // The main multiplexer listener already owns this port and routes to
            // the active instance (hostname match or fallback).
            tracing::info!(
                "Instance '{}' uses the main multiplexer port {} (served by the public listener)",
                config.name,
                config.external_mc_port
            );
            return;
        }
        if self.bindings.lock().unwrap().contains_key(&config.id) {
            return; // already bound
        }
        tracing::info!(
            "Bound external port {} -> instance '{}' (internal {})",
            config.external_mc_port,
            config.name,
            config.internal_mc_port
        );
        let handle = self.spawn_listener(config.external_mc_port as u16, Some(config.clone()));
        self.bindings
            .lock()
            .unwrap()
            .insert(config.id.clone(), handle);
    }

    /// Unbinds the dedicated player-facing port of an instance, if bound.
    pub fn unbind_instance(&self, instance_id: &str) {
        if let Some(handle) = self.bindings.lock().unwrap().remove(instance_id) {
            handle.abort();
            tracing::info!("Unbound external port for instance {instance_id}");
        }
    }

    /// Stops all listeners.
    pub fn stop(&self) {
        let handles: Vec<JoinHandle<()>> = self
            .bindings
            .lock()
            .unwrap()
            .drain()
            .map(|(_, h)| h)
            .collect();
        for handle in handles {
            handle.abort();
        }
    }

    fn spawn_listener(&self, port: u16, fixed_instance: Option<InstanceConfig>) -> JoinHandle<()> {
        let this = self.clone();
        tokio::spawn(async move {
            // Bind both protocol families explicitly. A bare [::] bind is
            // dual-stack on most systems, but some Windows configurations make
            // it IPv6-only — silently refusing every IPv4 client on the LAN
            // while the wrapper looks perfectly healthy. When [::] is truly
            // dual-stack the 0.0.0.0 bind below fails with AddrInUse, which is
            // expected and means IPv4 is already covered.
            let ipv6_listener = match TcpListener::bind(("[::]", port)).await {
                Ok(listener) => Some(listener),
                Err(e) => {
                    tracing::warn!("Failed to bind IPv6 listener on port {port}: {e}");
                    None
                }
            };
            let ipv4_listener = match TcpListener::bind(("0.0.0.0", port)).await {
                Ok(listener) => Some(listener),
                Err(e) if e.kind() == io::ErrorKind::AddrInUse && ipv6_listener.is_some() => {
                    // The [::] socket is dual-stack and already accepts IPv4.
                    None
                }
                Err(e) => {
                    tracing::warn!("Failed to bind IPv4 listener on port {port}: {e}");
                    None
                }
            };
            if ipv4_listener.is_none() && ipv6_listener.is_none() {
                tracing::error!("Failed to bind TCP listeners on port {port} (IPv4 and IPv6)");
                return;
            }
            let families = match (ipv4_listener.is_some(), ipv6_listener.is_some()) {
                (true, true) => "IPv4 + IPv6",
                (true, false) => "IPv4",
                (false, true) => "IPv6",
                (false, false) => unreachable!(),
            };

            match &fixed_instance {
                Some(instance) => {
                    let http_desc = if this.http_proxy_enabled() {
                        format!("HTTP -> {}:{}", BACKEND_HOST, this.web_port)
                    } else {
                        "HTTP proxying disabled (TLS reverse proxy)".to_string()
                    };
                    tracing::info!(
                        "Multiplexer listening on 0.0.0.0:{port} ({families}, {http_desc}, MC -> instance '{}' internal {})",
                        instance.name,
                        instance.internal_mc_port
                    )
                }
                None => {
                    let mc_target = if this.instances.is_some() {
                        format!(
                            "MC -> instance-by-hostname (default {}:{})",
                            BACKEND_HOST, this.mc_port
                        )
                    } else {
                        format!("MC -> {}:{}", BACKEND_HOST, this.mc_port)
                    };
                    let http_desc = if this.http_proxy_enabled() {
                        format!("HTTP -> {}:{}", BACKEND_HOST, this.web_port)
                    } else {
                        "HTTP proxying disabled (TLS reverse proxy)".to_string()
                    };
                    tracing::info!(
                        "TCP multiplexer listening on 0.0.0.0:{port} ({families}, {http_desc}, {mc_target})"
                    );
                }
            }

            let mut ipv4_listener = ipv4_listener;
            let mut ipv6_listener = ipv6_listener;
            loop {
                let accepted = tokio::select! {
                    res = Self::accept_or_pending(&mut ipv4_listener) => res,
                    res = Self::accept_or_pending(&mut ipv6_listener) => res,
                };
                match accepted {
                    Ok((socket, _)) => {
                        let this = this.clone();
                        let fixed = fixed_instance.clone();
                        tokio::spawn(async move {
                            if let Err(e) = this.handle_connection(socket, fixed).await {
                                // Visible at the default log level: a dropped game
                                // connection is the classic "Failed to Quick Play"
                                // cause (e.g. backend instance not listening).
                                tracing::warn!("Connection error on port {port}: {e}");
                            }
                        });
                    }
                    Err(e) => {
                        tracing::warn!("Accept failed on port {port}: {e}");
                        tokio::time::sleep(Duration::from_millis(50)).await;
                    }
                }
            }
        })
    }

    /// Accepts on the listener, or never completes when the listener is absent
    /// so its `select!` branch stays inert.
    async fn accept_or_pending(
        listener: &mut Option<TcpListener>,
    ) -> io::Result<(TcpStream, std::net::SocketAddr)> {
        match listener.as_mut() {
            Some(listener) => listener.accept().await,
            None => std::future::pending().await,
        }
    }

    /// Inspects the first bytes of an incoming connection and routes it. The
    /// detection phase is bounded by `PROTOCOL_DETECTION_TIMEOUT` so a client
    /// that trickles bytes (Slowloris) can't hold the socket open forever.
    async fn handle_connection(
        &self,
        client: TcpStream,
        fixed_instance: Option<InstanceConfig>,
    ) -> io::Result<()> {
        self.handle_connection_with_timeout(client, fixed_instance, PROTOCOL_DETECTION_TIMEOUT)
            .await
    }

    /// `handle_connection` with an injectable detection timeout (tests use a
    /// short one instead of waiting the full 5 seconds).
    async fn handle_connection_with_timeout(
        &self,
        mut client: TcpStream,
        fixed_instance: Option<InstanceConfig>,
        detection_timeout: Duration,
    ) -> io::Result<()> {
        let detection_future = async {
            let mut buf: Vec<u8> = Vec::with_capacity(1024);
            let mut tmp = [0u8; 2048];
            loop {
                let n = client.read(&mut tmp).await?;
                if n == 0 {
                    return Ok(None); // EOF before any decision — nothing to do
                }
                buf.extend_from_slice(&tmp[..n]);

                if detector::is_http_method(&buf) && self.http_proxy_enabled() {
                    return Ok(Some((buf, self.web_port)));
                }

                match detector::parse_handshake(&buf) {
                    ParseResult::Incomplete => {
                        // Not enough bytes yet. Cap the buffer: anything larger
                        // than a plausible handshake is routed to the default MC
                        // backend.
                        if buf.len() > 4096 {
                            let port = self.resolve_target_port(None, &fixed_instance);
                            return Ok(Some((buf, port)));
                        }
                        continue;
                    }
                    ParseResult::NotMatch => {
                        let port = self.resolve_target_port(None, &fixed_instance);
                        return Ok(Some((buf, port)));
                    }
                    ParseResult::Matched(handshake) => {
                        let target_port =
                            self.resolve_target_port(Some(&handshake), &fixed_instance);

                        // Zircon join gate: login connections MUST present a
                        // valid one-time join ticket registered by the launcher
                        // right before launch.
                        if handshake.next_state == 2 {
                            match detector::parse_login_start_username(&buf) {
                                ParseResult::Incomplete => continue, // Login Start not fully buffered yet
                                ParseResult::NotMatch => {
                                    // FAIL-CLOSED: reject unparseable / forged
                                    // Login Start frames instead of proxying a
                                    // vanilla client straight to the backend.
                                    tracing::warn!("Rejecting unparseable Login Start frame");
                                    let packet = disconnect::create_disconnect_packet(
                                        disconnect::build_custom_error_message(),
                                    );
                                    let _ = client.write_all(&packet).await;
                                    let _ = client.shutdown().await;
                                    return Ok(None);
                                }
                                ParseResult::Matched(username) => {
                                    if !self.tickets.consume_ticket(&username) {
                                        tracing::info!(
                                            "Rejected connection for '{username}' — no active Zircon join ticket"
                                        );
                                        let packet = disconnect::create_disconnect_packet(
                                            disconnect::build_custom_error_message(),
                                        );
                                        let _ = client.write_all(&packet).await;
                                        let _ = client.shutdown().await;
                                        return Ok(None);
                                    }
                                    // The player has arrived — release the
                                    // join-intent hold so the idle window is
                                    // governed by real player activity from here.
                                    if let Some(instances) = &self.instances {
                                        if let Some(cfg) =
                                            instances.find_by_internal_port(target_port)
                                        {
                                            instances.clear_pending_join_intent(&cfg.id);
                                        }
                                    }
                                }
                            }
                        }
                        return Ok(Some((buf, target_port)));
                    }
                }
            }
        };

        // Bounded detection stops Slowloris socket-starvation attacks: a client
        // that never completes a handshake is dropped after `detection_timeout`.
        match tokio::time::timeout(detection_timeout, detection_future).await {
            Ok(Ok(Some((buf, port)))) => self.proxy(client, buf, port).await,
            Ok(Ok(None)) => Ok(()),
            Ok(Err(e)) => Err(e),
            Err(_) => {
                tracing::warn!("Protocol detection timed out on socket; connection dropped");
                Ok(())
            }
        }
    }

    /// Whether the multiplexer proxies HTTP traffic to the web server. Disabled
    /// when a TLS reverse proxy fronts the HTTP side (see `config.http_proxy`),
    /// so the admin panel is never reachable in plaintext on the MC ports.
    fn http_proxy_enabled(&self) -> bool {
        self.config.get_config().http_proxy
    }

    /// Resolves the backend MC port for a connection.
    fn resolve_target_port(
        &self,
        handshake: Option<&detector::Handshake>,
        fixed_instance: &Option<InstanceConfig>,
    ) -> u16 {
        let Some(instances) = &self.instances else {
            return self.mc_port; // legacy single-server mode
        };
        if let Some(fixed) = fixed_instance {
            // Dedicated per-instance port: the instance is already known.
            return fixed.internal_mc_port as u16;
        }
        let public_port = self.config.get_config().public_port;
        if let Some(handshake) = handshake {
            if let Some(cfg) = instances.find_by_hostname(&handshake.hostname) {
                return cfg.internal_mc_port as u16;
            }
        }
        // Unknown hostname (e.g. bare IP / localhost): route to the instance
        // owning the main port, falling back to the active instance.
        if let Some(cfg) = instances.find_by_external_port(public_port) {
            return cfg.internal_mc_port as u16;
        }
        if let Some(cfg) = instances.get_active_instance() {
            return cfg.internal_mc_port as u16;
        }
        self.mc_port
    }

    /// Transparent bidirectional proxy: forwards the buffered initial bytes,
    /// then pipes traffic both ways until either side disconnects.
    async fn proxy(&self, client: TcpStream, initial: Vec<u8>, port: u16) -> io::Result<()> {
        // Annotate the connect failure with the backend target so the accept
        // loop's warning says *which* backend refused — e.g. the web server
        // (25564, a real problem) vs a sleeping Minecraft instance's internal
        // port (expected until the launcher wakes it).
        let mut backend = TcpStream::connect((BACKEND_HOST, port))
            .await
            .map_err(|e| {
                io::Error::new(
                    e.kind(),
                    format!("connect to backend {BACKEND_HOST}:{port}: {e}"),
                )
            })?;
        backend.write_all(&initial).await?;
        let mut client = client;
        tokio::io::copy_bidirectional(&mut client, &mut backend).await?;
        Ok(())
    }
}

impl PortBindingListener for TcpMultiplexer {
    fn on_instance_added(&self, config: &InstanceConfig) {
        self.bind_instance(config);
    }

    fn on_instance_updated(&self, config: &InstanceConfig) {
        // Manual port changes: drop the old listener, bind the new one.
        self.unbind_instance(&config.id);
        self.bind_instance(config);
    }

    fn on_instance_removed(&self, instance_id: &str) {
        self.unbind_instance(instance_id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::multiplexer::varint::write_varint;
    use crate::process::console::ConsoleStreamHandler;

    /// Serializes the multiplexer tests: they bind real ports (25565+,
    /// 25700+) which would collide when the test runner executes them in
    /// parallel. An async mutex lets each test hold the guard across awaits.
    static MUX_TEST_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

    fn temp_dir() -> std::path::PathBuf {
        crate::test_util::temp_dir("mux")
    }

    fn config_at(dir: &std::path::Path) -> Arc<ConfigService> {
        Arc::new(
            ConfigService::load_with_data_dir(Some(dir.to_string_lossy().into_owned())).unwrap(),
        )
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
        let tickets = Arc::new(JoinTicketManager::new());
        let multiplexer = TcpMultiplexer::new(config.clone(), None, tickets);

        // Start a fake "web server" on the configured web port.
        let web_listener = TcpListener::bind(("127.0.0.1", config.get_config().web_port as u16))
            .await
            .unwrap();
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

        let main_port = 25565u16;
        let handle = multiplexer.spawn_listener(main_port, None);

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
        handle.abort();
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn http_proxy_can_be_disabled_for_reverse_proxy_deployments() {
        let _guard = MUX_TEST_LOCK.lock().await;
        let dir = temp_dir();
        let config = config_at(&dir);
        config.with_config(|c| c.http_proxy = false);
        let tickets = Arc::new(JoinTicketManager::new());
        let multiplexer = TcpMultiplexer::new(config.clone(), None, tickets);

        // An HTTP-looking request must NOT reach the web port when proxying is
        // disabled; it is treated as (invalid) Minecraft traffic and routed to
        // the MC backend instead. No web listener is bound — a proxy attempt
        // would fail with connection refused and surface in the test.
        let web_port = config.get_config().web_port as u16;
        let mc_port = config.get_config().mc_port as u16;
        assert_ne!(web_port, mc_port);

        let mc_listener = TcpListener::bind(("127.0.0.1", mc_port)).await.unwrap();
        let mc_handle = tokio::spawn(async move {
            let (mut socket, _) = mc_listener.accept().await.unwrap();
            let mut buf = [0u8; 256];
            let n = socket.read(&mut buf).await.unwrap();
            socket.write_all(b"\x00").await.unwrap(); // nonsense reply; bytes are what matter
            (String::from_utf8_lossy(&buf[..n]).to_string(), n)
        });

        let main_port = 25565u16;
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

        let port = instance.external_mc_port as u16;
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
}
