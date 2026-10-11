use std::io;
use std::time::Duration;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use zircon_core::model::{InstanceConfig, JoinGateMode};
use zircon_core::protocol::chat as protocol_chat;

use crate::multiplexer::detector::{self, ParseResult};
use crate::multiplexer::disconnect;

use super::*;

impl TcpMultiplexer {
    /// Inspects the first bytes of an incoming connection and routes it. The
    /// detection phase is bounded by `PROTOCOL_DETECTION_TIMEOUT` so a client
    /// that trickles bytes (Slowloris) can't hold the socket open forever.
    pub(crate) async fn handle_connection(
        &self,
        client: TcpStream,
        fixed_instance: Option<InstanceConfig>,
    ) -> io::Result<()> {
        self.handle_connection_with_timeout(client, fixed_instance, PROTOCOL_DETECTION_TIMEOUT)
            .await
    }

    /// `handle_connection` with an injectable detection timeout (tests use a
    /// short one instead of waiting the full 5 seconds).
    pub(crate) async fn handle_connection_with_timeout(
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

                        // Wake-from-sleep integration: detect sleeping instances and wake on demand
                        if let Some(instances) = &self.instances {
                            let matched_cfg = instances
                                .find_by_hostname(&handshake.hostname)
                                .or_else(|| fixed_instance.clone());

                            if let Some(cfg) = matched_cfg {
                                let is_sleeping = !instances.is_running(&cfg.id)
                                    && (instances.wakeable(&cfg.id) || instances.driver().is_some());

                                if is_sleeping {
                                    tracing::info!(
                                        "Instance '{}' is currently SLEEPING (state {}). Initiating wake flow...",
                                        cfg.name,
                                        handshake.next_state
                                    );

                                    // 1. Status Ping (next_state == 1)
                                    if handshake.next_state == 1 {
                                        let boot_info = instances.get_boot_progress(&cfg.id);
                                        let is_booting = boot_info.is_some() || instances.has_pending_join_intent(&cfg.id);
                                        let (frac, remaining_secs) = if let Some((elapsed, total, f)) = boot_info {
                                            let rem = total.saturating_sub(elapsed).max(3);
                                            (Some(f), Some(rem))
                                        } else if is_booting {
                                            (Some(0.25), Some(20))
                                        } else {
                                            (None, None)
                                        };

                                        let status_text = if is_booting {
                                            "⏳ Server is starting up... Connect to join!"
                                        } else {
                                            "Connect to wake up server!"
                                        };
                                        let wake_status = protocol_chat::build_wake_slp_json(
                                            handshake.protocol_version,
                                            &cfg.name,
                                            status_text,
                                            is_booting,
                                            frac,
                                            remaining_secs,
                                            None,
                                        );
                                        let packet = disconnect::create_status_response_packet(&wake_status);
                                        let _ = client.write_all(&packet).await;
                                        let _ = client.shutdown().await;
                                        return Ok(None);
                                    }

                                    // 2. Login (next_state == 2): validate ticket or access mode
                                    let server_url = handshake.hostname.split('\0').next().unwrap_or("").trim();
                                    let username = match detector::parse_login_start_username(&buf) {
                                        ParseResult::Incomplete => continue,
                                        ParseResult::NotMatch => {
                                            tracing::warn!("Rejecting unparseable Login Start frame during wake");
                                            let error_msg = disconnect::build_custom_error_message(server_url);
                                            let packet = disconnect::create_disconnect_packet(&error_msg);
                                            let _ = client.write_all(&packet).await;
                                            let _ = client.shutdown().await;
                                            return Ok(None);
                                        }
                                        ParseResult::Matched(u) => u,
                                    };

                                    let has_ticket = self.tickets.consume_ticket(&username);
                                    if !has_ticket {
                                        match cfg.join_gate_mode {
                                            JoinGateMode::StrictTicket => {
                                                tracing::info!(
                                                    "Rejected connection for '{username}' to sleeping server — StrictTicket mode requires Zircon companion launcher"
                                                );
                                                let error_msg = disconnect::build_custom_error_message(server_url);
                                                let packet = disconnect::create_disconnect_packet(&error_msg);
                                                let _ = client.write_all(&packet).await;
                                                let _ = client.shutdown().await;
                                                return Ok(None);
                                            }
                                            JoinGateMode::WhitelistOnly => {
                                                if !instances.is_username_whitelisted(&cfg.id, &username) {
                                                    tracing::info!(
                                                        "Rejected connection for '{username}' to sleeping server — player is not whitelisted"
                                                    );
                                                    let error_msg = protocol_chat::build_whitelist_rejected_json();
                                                    let packet = disconnect::create_disconnect_packet(&error_msg);
                                                    let _ = client.write_all(&packet).await;
                                                    let _ = client.shutdown().await;
                                                    return Ok(None);
                                                }
                                            }
                                            JoinGateMode::Universal => {
                                                tracing::info!(
                                                    "Universal client '{username}' triggered wake for instance '{}'",
                                                    cfg.name
                                                );
                                            }
                                        }
                                    }

                                    // Smart Already-Booting Guard:
                                    // If instance is already booting with >8s remaining, don't freeze socket for 20s.
                                    let already_booting = instances.get_boot_progress(&cfg.id);
                                    if let Some((elapsed, total, _)) = already_booting {
                                        let remaining_secs = total.saturating_sub(elapsed).max(3);
                                        if remaining_secs > 8 {
                                            let wake_msg = protocol_chat::build_wake_disconnect_json(&username, remaining_secs);
                                            let packet = disconnect::create_disconnect_packet(&wake_msg);
                                            let _ = client.write_all(&packet).await;
                                            let _ = client.shutdown().await;
                                            return Ok(None);
                                        }
                                    }

                                    // Register intent so idle shutdown doesn't stop it mid-boot
                                    instances.register_join_intent(&cfg.id);

                                    // Trigger instance / container start
                                    if let Err(e) = instances.start_instance(&cfg.id).await {
                                        tracing::error!("Wakeup failed for instance '{}': {e}", cfg.id);
                                    }

                                    // Tier 1: Silent Socket Stall (poll loopback port for up to 20s)
                                    let ready = instances
                                        .poll_port_ready(cfg.internal_mc_port as u16, Duration::from_secs(20))
                                        .await;
                                    if !ready {
                                        // Tier 2: Heavy boot. Send graceful disconnect packet instructing user to rejoin
                                        let remaining_secs = instances
                                            .get_boot_progress(&cfg.id)
                                            .map(|(elapsed, total, _)| total.saturating_sub(elapsed).max(5))
                                            .unwrap_or(15);
                                        let wake_msg = protocol_chat::build_wake_disconnect_json(&username, remaining_secs);
                                        let packet = disconnect::create_disconnect_packet(&wake_msg);
                                        let _ = client.write_all(&packet).await;
                                        let _ = client.shutdown().await;
                                        return Ok(None);
                                    }

                                    instances.mark_boot_complete(&cfg.id);
                                    instances.clear_pending_join_intent(&cfg.id);
                                    return Ok(Some((buf, cfg.internal_mc_port as u16)));
                                }
                            }
                        }

                        // Login connections to awake servers: check join gate mode
                        if handshake.next_state == 2 {
                            let server_url = handshake.hostname.split('\0').next().unwrap_or("").trim();
                            match detector::parse_login_start_username(&buf) {
                                ParseResult::Incomplete => continue, // Login Start not fully buffered yet
                                ParseResult::NotMatch => {
                                    tracing::warn!("Rejecting unparseable Login Start frame");
                                    let error_msg = disconnect::build_custom_error_message(server_url);
                                    let packet = disconnect::create_disconnect_packet(&error_msg);
                                    let _ = client.write_all(&packet).await;
                                    let _ = client.shutdown().await;
                                    return Ok(None);
                                }
                                ParseResult::Matched(username) => {
                                    let has_ticket = self.tickets.consume_ticket(&username);
                                    if !has_ticket {
                                        let matched_cfg = self.instances.as_ref().and_then(|inst| {
                                            inst.find_by_hostname(&handshake.hostname).or_else(|| fixed_instance.clone())
                                        });

                                        let is_strict = matched_cfg.as_ref()
                                            .map(|c| c.join_gate_mode == JoinGateMode::StrictTicket)
                                            .unwrap_or(false);

                                        if is_strict {
                                            tracing::info!(
                                                "Rejected connection for '{username}' — StrictTicket mode requires Zircon companion launcher"
                                            );
                                            let error_msg = disconnect::build_custom_error_message(server_url);
                                            let packet = disconnect::create_disconnect_packet(&error_msg);
                                            let _ = client.write_all(&packet).await;
                                            let _ = client.shutdown().await;
                                            return Ok(None);
                                        }
                                    }

                                    // The player has arrived — release the join-intent hold
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
    /// or when running as a headless Zircon Cloud fleet node.
    pub(crate) fn http_proxy_enabled(&self) -> bool {
        let cfg = self.config.get_config();
        if cfg.cloud.is_enabled() {
            return false;
        }
        cfg.http_proxy
    }

    /// Resolves the backend MC port for a connection.
    pub(crate) fn resolve_target_port(
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

}
