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
            let listener = match TcpListener::bind(("0.0.0.0", port)).await {
                Ok(listener) => listener,
                Err(e) => {
                    tracing::error!("Failed to bind TCP listener on port {port}: {e}");
                    return;
                }
            };
            match &fixed_instance {
                Some(instance) => tracing::info!(
                    "Multiplexer listening on 0.0.0.0:{port} (HTTP -> {}:{}, MC -> instance '{}' internal {})",
                    BACKEND_HOST,
                    this.web_port,
                    instance.name,
                    instance.internal_mc_port
                ),
                None => {
                    let mc_target = if this.instances.is_some() {
                        format!("MC -> instance-by-hostname (default {}:{})", BACKEND_HOST, this.mc_port)
                    } else {
                        format!("MC -> {}:{}", BACKEND_HOST, this.mc_port)
                    };
                    tracing::info!(
                        "TCP multiplexer listening on 0.0.0.0:{port} (HTTP -> {}:{}, {mc_target})",
                        BACKEND_HOST,
                        this.web_port
                    );
                }
            }

            loop {
                match listener.accept().await {
                    Ok((socket, _)) => {
                        let this = this.clone();
                        let fixed = fixed_instance.clone();
                        tokio::spawn(async move {
                            if let Err(e) = this.handle_connection(socket, fixed).await {
                                tracing::debug!("Connection error on port {port}: {e}");
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

    /// Inspects the first bytes of an incoming connection and routes it.
    async fn handle_connection(
        &self,
        mut client: TcpStream,
        fixed_instance: Option<InstanceConfig>,
    ) -> io::Result<()> {
        let mut buf: Vec<u8> = Vec::with_capacity(1024);
        let mut tmp = [0u8; 2048];
        loop {
            let n = client.read(&mut tmp).await?;
            if n == 0 {
                return Ok(()); // EOF before any decision — nothing to do
            }
            buf.extend_from_slice(&tmp[..n]);

            if detector::is_http_method(&buf) {
                return self.proxy(client, buf, self.web_port).await;
            }

            match detector::parse_handshake(&buf) {
                ParseResult::Incomplete => {
                    // Not enough bytes yet. Cap the buffer: anything larger than
                    // a plausible handshake is routed to the default MC backend.
                    if buf.len() > 4096 {
                        let port = self.resolve_target_port(None, &fixed_instance);
                        return self.proxy(client, buf, port).await;
                    }
                    continue;
                }
                ParseResult::NotMatch => {
                    let port = self.resolve_target_port(None, &fixed_instance);
                    return self.proxy(client, buf, port).await;
                }
                ParseResult::Matched(handshake) => {
                    let target_port = self.resolve_target_port(Some(&handshake), &fixed_instance);

                    // Zircon join gate: login connections must present a one-time
                    // ticket registered by the launcher right before launch.
                    if handshake.next_state == 2 {
                        match detector::parse_login_start_username(&buf) {
                            ParseResult::Incomplete => continue, // Login Start not fully buffered yet
                            ParseResult::NotMatch => {}
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
                                    return Ok(());
                                }
                            }
                        }
                    }
                    return self.proxy(client, buf, target_port).await;
                }
            }
        }
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
        let mut backend = TcpStream::connect((BACKEND_HOST, port)).await?;
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
}
