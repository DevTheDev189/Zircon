use std::io;
use std::time::Duration;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::task::JoinHandle;
use zircon_core::model::InstanceConfig;

use super::*;

/// Backend host for the admin web server and the legacy MC server.
pub(crate) const BACKEND_HOST: &str = "127.0.0.1";

/// Max time a client may take to complete protocol detection before the socket
/// is dropped (Slowloris socket-starvation defense).
pub(crate) const PROTOCOL_DETECTION_TIMEOUT: Duration = Duration::from_secs(5);

/// Global cap on concurrently accepted TCP connections across all listeners
/// (public port + per-instance ports). Prevents a connection flood from
/// exhausting file descriptors (`EMFILE`) on the wrapper.
pub(crate) const MAX_GLOBAL_TCP_CONNS: usize = 1000;
/// Per-source-IP cap on concurrently accepted connections. Kept well below the
/// global cap so a single client (or a spoofed-IP flood) cannot crowd out
/// every other player or fill the global budget alone.
pub(crate) const MAX_CONNS_PER_IP: usize = 25;

/// Cap on buffered HTTP head bytes during `X-Zircon-Real-IP` injection.
pub(crate) const MAX_HTTP_HEAD_BYTES: usize = 16 * 1024;

/// Canonicalizes IPv4-mapped IPv6 addresses (`::ffff:a.b.c.d` -> `a.b.c.d`)
/// and masks global IPv6 addresses to their `/64` prefix so a single IPv6
/// `/64` host allocation cannot bypass `MAX_CONNS_PER_IP`.
fn normalize_quota_ip(ip: std::net::IpAddr) -> std::net::IpAddr {
    match ip.to_canonical() {
        std::net::IpAddr::V4(v4) => std::net::IpAddr::V4(v4),
        std::net::IpAddr::V6(v6) if v6.is_loopback() => std::net::IpAddr::V6(v6),
        std::net::IpAddr::V6(v6) => {
            let segs = v6.segments();
            std::net::IpAddr::V6(std::net::Ipv6Addr::new(
                segs[0], segs[1], segs[2], segs[3], 0, 0, 0, 0,
            ))
        }
    }
}

impl TcpMultiplexer {
    pub(crate) fn spawn_listener(&self, port: u16, fixed_instance: Option<InstanceConfig>) -> JoinHandle<()> {
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
                        let _ = socket.set_nodelay(true);
                        // Socket-exhaustion defense: cap concurrently accepted
                        // connections per source IP and globally so a flood
                        // cannot exhaust file descriptors (EMFILE) or crowd out
                        // every other player.
                        let peer_ip = socket
                            .peer_addr()
                            .map(|addr| normalize_quota_ip(addr.ip()))
                            .unwrap_or(std::net::IpAddr::V4(std::net::Ipv4Addr::LOCALHOST));
                        let this = this.clone();
                        let ip_conns = this.ip_conns.clone();

                        // Per-IP quota, checked before the global budget so one
                        // client cannot drain the global pool on its own.
                        {
                            let mut count = ip_conns.entry(peer_ip).or_insert(0);
                            if *count >= MAX_CONNS_PER_IP {
                                tracing::warn!(
                                    "TCP connection limit exceeded for {peer_ip}; closing socket"
                                );
                                drop(socket);
                                continue;
                            }
                            *count += 1;
                        }

                        // Global quota: an owned permit is held for the whole
                        // lifetime of the connection and released when its task
                        // ends (the permit is moved into the task below).
                        let Ok(permit) = this.global_limiter.clone().try_acquire_owned() else {
                            tracing::warn!(
                                "Global TCP connection capacity reached; dropping connection"
                            );
                            drop(socket);
                            // Revert the per-IP increment via the RAII guard.
                            drop(IpConnGuard {
                                ip: peer_ip,
                                ip_conns,
                            });
                            continue;
                        };

                        let fixed = fixed_instance.clone();
                        tokio::spawn(async move {
                            // The guard keeps the global permit and the per-IP
                            // counter slot for the connection's lifetime; both
                            // are released automatically when the task ends.
                            let _permit = permit;
                            let _ip_guard = IpConnGuard {
                                ip: peer_ip,
                                ip_conns,
                            };
                            if let Err(e) = this.handle_connection(socket, fixed).await {
                                tracing::warn!("Multiplexer connection error on port {port} -> backend: {e}");
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

    /// Transparent bidirectional proxy: forwards the buffered initial bytes,
    /// then pipes traffic both ways until either side disconnects. When the
    /// target is the admin web server, the real client IP is injected into the
    /// request head so the web layer can key rate limits on it.
    pub(crate) async fn proxy(&self, client: TcpStream, initial: Vec<u8>, port: u16) -> io::Result<()> {
        let mut client = client;
        let mut to_write = initial;
        if port == self.web_port {
            let peer_ip = client
                .peer_addr()
                .map(|addr| addr.ip().to_canonical())
                .unwrap_or(std::net::IpAddr::V4(std::net::Ipv4Addr::LOCALHOST));
            to_write = tokio::time::timeout(
                PROTOCOL_DETECTION_TIMEOUT,
                prepare_http_forward(to_write, &mut client, peer_ip),
            )
            .await
            .map_err(|_| io::Error::new(io::ErrorKind::TimedOut, "HTTP header read timed out"))??;
        }

        // Annotate the connect failure with the backend target so the accept
        // loop's warning says *which* backend refused — e.g. the web server
        // (25564, a real problem) vs a sleeping Minecraft instance's internal
        // port (expected until the launcher wakes it).
        let mut backend = tokio::time::timeout(
            PROTOCOL_DETECTION_TIMEOUT,
            TcpStream::connect((BACKEND_HOST, port)),
        )
        .await
        .map_err(|_| io::Error::new(io::ErrorKind::TimedOut, format!("connect to backend {BACKEND_HOST}:{port} timed out")))?
        .map_err(|e| {
            io::Error::new(
                e.kind(),
                format!("connect to backend {BACKEND_HOST}:{port}: {e}"),
            )
        })?;

        // Disable Nagle's algorithm on both legs of the proxy so small game packets
        // (movement, combat, clicks, position updates) are sent immediately with
        // sub-millisecond latency and zero packet-coalescing delays.
        let _ = client.set_nodelay(true);
        let _ = backend.set_nodelay(true);

        tokio::time::timeout(PROTOCOL_DETECTION_TIMEOUT, backend.write_all(&to_write))
            .await
            .map_err(|_| io::Error::new(io::ErrorKind::TimedOut, "write to backend timed out"))??;

        tokio::io::copy_bidirectional(&mut client, &mut backend).await?;
        Ok(())
    }
}

/// RAII guard that holds a per-IP connection slot: decrements (and cleans up)
/// the multiplexer's `ip_conns` counter when dropped, so the quota can never
/// leak — whether the connection task ends normally, the accept loop rejects
/// an over-budget socket, or a task panics.
struct IpConnGuard {
    ip: std::net::IpAddr,
    ip_conns: Arc<dashmap::DashMap<std::net::IpAddr, usize>>,
}

impl Drop for IpConnGuard {
    fn drop(&mut self) {
        if let Some(mut count) = self.ip_conns.get_mut(&self.ip) {
            if *count <= 1 {
                drop(count);
                self.ip_conns.remove(&self.ip);
            } else {
                *count -= 1;
            }
        }
    }
}

/// Fails closed with `InvalidData` when the HTTP header block cannot be
/// completed within `MAX_HTTP_HEAD_BYTES` so an oversized or stalled request
/// can never bypass header sanitization.
pub(crate) async fn prepare_http_forward(
    mut buf: Vec<u8>,
    client: &mut TcpStream,
    client_ip: std::net::IpAddr,
) -> io::Result<Vec<u8>> {
    while !buf.windows(4).any(|w| w == b"\r\n\r\n") && buf.len() < MAX_HTTP_HEAD_BYTES {
        let mut tmp = [0u8; 2048];
        let n = match client.read(&mut tmp).await {
            Ok(n) if n > 0 => n,
            _ => break, // EOF or read error: forward what we have
        };
        buf.extend_from_slice(&tmp[..n]);
    }
    let Some(head_end) = buf.windows(4).position(|w| w == b"\r\n\r\n") else {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "incomplete or oversized HTTP request header",
        ));
    };

    let head_str = String::from_utf8_lossy(&buf[..head_end]);
    let mut sanitized_lines = Vec::new();

    // WebSocket handshakes need their `Connection: Upgrade` header preserved;
    // forcing `Connection: close` here makes Axum/Hyper reject the upgrade
    // with HTTP 400 Bad Request. Only GET requests with Sec-WebSocket-Key are
    // valid RFC 6455 WebSocket upgrades.
    let is_get_request = head_str
        .split("\r\n")
        .next()
        .map(|req| req.starts_with("GET "))
        .unwrap_or(false);
    let has_ws_upgrade = head_str.split("\r\n").skip(1).any(|line| {
        let Some((name, val)) = line.split_once(':') else { return false };
        name.trim().eq_ignore_ascii_case("upgrade")
            && val.to_ascii_lowercase().contains("websocket")
    });
    let has_ws_key = head_str.split("\r\n").skip(1).any(|line| {
        let Some((name, _)) = line.split_once(':') else { return false };
        name.trim().eq_ignore_ascii_case("sec-websocket-key")
    });
    let is_ws = is_get_request && has_ws_upgrade && has_ws_key;

    for (i, line) in head_str.split("\r\n").enumerate() {
        if i == 0 {
            // Preserve the request line, e.g. "GET /index.html HTTP/1.1".
            sanitized_lines.push(line.to_string());
            continue;
        }
        // Reject obs-fold / bare-CR or malformed header lines without a colon.
        let Some((raw_name, _)) = line.split_once(':') else {
            continue;
        };
        if raw_name.starts_with([' ', '\t', '\r']) || line.contains('\r') {
            continue;
        }
        let name = raw_name.trim().to_ascii_lowercase();
        // Drop attacker-injected real-IP overrides and Connection/Upgrade headers.
        if name == "x-zircon-real-ip" || name == "connection" || (!is_ws && name == "upgrade") {
            continue;
        }
        sanitized_lines.push(line.to_string());
    }

    // Append our verified headers.
    sanitized_lines.push(format!("X-Zircon-Real-IP: {client_ip}"));
    if is_ws {
        sanitized_lines.push("Connection: Upgrade".to_string());
    } else {
        sanitized_lines.push("Connection: close".to_string());
    }

    let mut out = sanitized_lines.join("\r\n").into_bytes();
    out.extend_from_slice(b"\r\n\r\n");
    out.extend_from_slice(&buf[head_end + 4..]); // Append any body bytes already buffered
    Ok(out)
}


