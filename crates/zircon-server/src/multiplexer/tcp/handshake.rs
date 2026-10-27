use std::io;
use std::time::Duration;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use zircon_core::model::{InstanceConfig, JoinGateMode};
use zircon_core::protocol::chat as protocol_chat;

use crate::multiplexer::detector::{self, ParseResult};
use crate::multiplexer::disconnect;

use super::*;

enum DetectionOutcome {
    Done,
    Proxy { buf: Vec<u8>, port: u16 },
    WakeAndProxy {
        buf: Vec<u8>,
        cfg: InstanceConfig,
        handshake: detector::Handshake,
        username: String,
        consumed_ticket: bool,
    },
}

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
                    return Ok(DetectionOutcome::Done); // EOF before any decision — nothing to do
                }
                if buf.len() + n > 4096 {
                    tracing::warn!("Protocol detection buffer limit (4096B) exceeded; dropping socket");
                    let _ = client.shutdown().await;
                    return Ok(DetectionOutcome::Done);
                }
                buf.extend_from_slice(&tmp[..n]);

                if detector::is_http_method(&buf) && self.http_proxy_enabled() {
                    return Ok(DetectionOutcome::Proxy { buf, port: self.web_port });
                }

                match detector::parse_handshake(&buf) {
                    ParseResult::Incomplete => {
                        if buf.len() >= 4096 {
                            tracing::warn!("Handshake incomplete within 4096B; dropping socket");
                            let _ = client.shutdown().await;
                            return Ok(DetectionOutcome::Done);
                        }
                        continue;
                    }
                    ParseResult::NotMatch => {
                        if !self.http_proxy_enabled() || self.instances.is_none() {
                            let port = self.resolve_target_port(None, &fixed_instance);
                            return Ok(DetectionOutcome::Proxy { buf, port });
                        }
                        tracing::warn!("Dropping connection: unrecognized non-Minecraft/non-HTTP frame");
                        let _ = client.shutdown().await;
                        return Ok(DetectionOutcome::Done);
                    }
                    ParseResult::Matched(handshake) => {
                        let matched_cfg =
                            self.resolve_target_instance(Some(&handshake), &fixed_instance);
                        let target_port = matched_cfg
                            .as_ref()
                            .map(|cfg| cfg.internal_mc_port as u16)
                            .unwrap_or(self.mc_port);

                        // Wake-from-sleep integration: detect sleeping instances and wake on demand
                        if let Some(instances) = &self.instances {
                            if let Some(cfg) = matched_cfg.clone() {
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
                                        let _ = client.flush().await;

                                        let mut ping_buf = [0u8; 64];
                                        while let Ok(Ok(n)) = tokio::time::timeout(Duration::from_millis(400), client.read(&mut ping_buf)).await {
                                            if n == 0 {
                                                break;
                                            }
                                            if n == 2 && ping_buf[0] == 0x01 && ping_buf[1] == 0x00 {
                                                continue;
                                            }
                                            let _ = client.write_all(&ping_buf[..n]).await;
                                            let _ = client.flush().await;
                                            break;
                                        }
                                        let _ = client.shutdown().await;
                                        return Ok(DetectionOutcome::Done);
                                    }

                                    // 2. Login (next_state == 2 or 3): validate ticket or access mode
                                    if handshake.next_state == 2 || handshake.next_state == 3 {
                                        let server_url = handshake.hostname.split('\0').next().unwrap_or("").trim();
                                        let username = match detector::parse_login_start_username(&buf) {
                                            ParseResult::Incomplete => continue,
                                            ParseResult::NotMatch => {
                                                tracing::warn!("Rejecting unparseable Login Start frame during wake");
                                                let error_msg = disconnect::build_custom_error_message(server_url);
                                                let packet = disconnect::create_disconnect_packet(&error_msg);
                                                let _ = client.write_all(&packet).await;
                                                let _ = client.shutdown().await;
                                                return Ok(DetectionOutcome::Done);
                                            }
                                            ParseResult::Matched(u) => u,
                                        };

                                        let peer_ip = client.peer_addr().ok().map(|a| a.ip());
                                        let has_ticket = self.tickets.consume_ticket_for_ip(&username, peer_ip);
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
                                                    return Ok(DetectionOutcome::Done);
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
                                                        return Ok(DetectionOutcome::Done);
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

                                        return Ok(DetectionOutcome::WakeAndProxy {
                                            buf,
                                            cfg,
                                            handshake,
                                            username,
                                            consumed_ticket: has_ticket,
                                        });
                                    }
                                }
                            }
                        }

                        // Login connections to awake servers: check join gate mode
                        if handshake.next_state == 2 || handshake.next_state == 3 {
                            let server_url = handshake.hostname.split('\0').next().unwrap_or("").trim();
                            match detector::parse_login_start_username(&buf) {
                                ParseResult::Incomplete => continue, // Login Start not fully buffered yet
                                ParseResult::NotMatch => {
                                    tracing::warn!("Rejecting unparseable Login Start frame");
                                    let error_msg = disconnect::build_custom_error_message(server_url);
                                    let packet = disconnect::create_disconnect_packet(&error_msg);
                                    let _ = client.write_all(&packet).await;
                                    let _ = client.shutdown().await;
                                    return Ok(DetectionOutcome::Done);
                                }
                                ParseResult::Matched(username) => {
                                    let peer_ip = client.peer_addr().ok().map(|a| a.ip());
                                    let has_ticket = self.tickets.consume_ticket_for_ip(&username, peer_ip);
                                    if !has_ticket {
                                        if let Some(cfg) = &matched_cfg {
                                            match cfg.join_gate_mode {
                                                JoinGateMode::StrictTicket => {
                                                    tracing::info!(
                                                        "Rejected connection for '{username}' — StrictTicket mode requires Zircon companion launcher"
                                                    );
                                                    let error_msg = disconnect::build_custom_error_message(server_url);
                                                    let packet = disconnect::create_disconnect_packet(&error_msg);
                                                    let _ = client.write_all(&packet).await;
                                                    let _ = client.shutdown().await;
                                                    return Ok(DetectionOutcome::Done);
                                                }
                                                JoinGateMode::WhitelistOnly => {
                                                    if let Some(instances) = &self.instances {
                                                        if !instances.is_username_whitelisted(&cfg.id, &username) {
                                                            tracing::info!(
                                                                "Rejected connection for '{username}' — player is not whitelisted"
                                                            );
                                                            let error_msg = protocol_chat::build_whitelist_rejected_json();
                                                            let packet = disconnect::create_disconnect_packet(&error_msg);
                                                            let _ = client.write_all(&packet).await;
                                                            let _ = client.shutdown().await;
                                                            return Ok(DetectionOutcome::Done);
                                                        }
                                                    }
                                                }
                                                JoinGateMode::Universal => {}
                                            }
                                        }
                                    }

                                    // The player has arrived — release the join-intent hold
                                    if let Some(instances) = &self.instances {
                                        if let Some(cfg) = &matched_cfg {
                                            instances.clear_pending_join_intent(&cfg.id);
                                        }
                                    }
                                }
                            }
                        }
                        return Ok(DetectionOutcome::Proxy { buf, port: target_port });
                    }
                }
            }
        };

        // Bounded detection stops Slowloris socket-starvation attacks: a client
        // that never completes a handshake is dropped after `detection_timeout`.
        match tokio::time::timeout(detection_timeout, detection_future).await {
            Ok(Ok(DetectionOutcome::Proxy { buf, port })) => self.proxy(client, buf, port).await,
            Ok(Ok(DetectionOutcome::WakeAndProxy {
                buf,
                cfg,
                handshake,
                username,
                consumed_ticket,
            })) => {
                self.handle_wake_and_proxy(client, buf, cfg, handshake, username, consumed_ticket)
                    .await
            }
            Ok(Ok(DetectionOutcome::Done)) => Ok(()),
            Ok(Err(e)) => Err(e),
            Err(_) => {
                tracing::warn!("Protocol detection timed out on socket; connection dropped");
                Ok(())
            }
        }
    }

    /// Executes the wake-from-sleep sequence outside the 5-second protocol
    /// detection timeout and restores any consumed one-time join ticket if the
    /// client is transferred or asked to reconnect while the instance finishes booting.
    async fn handle_wake_and_proxy(
        &self,
        mut client: TcpStream,
        buf: Vec<u8>,
        cfg: InstanceConfig,
        handshake: detector::Handshake,
        username: String,
        consumed_ticket: bool,
    ) -> io::Result<()> {
        let Some(instances) = &self.instances else {
            return Ok(());
        };

        let transfer_port = if cfg.external_mc_port > 0 {
            cfg.external_mc_port as u16
        } else {
            self.config.get_config().public_port as u16
        };

        let already_booting = instances.get_boot_progress(&cfg.id);
        if let Some((elapsed, total, _)) = already_booting {
            let remaining_secs = total.saturating_sub(elapsed).max(3);
            if remaining_secs > 8 {
                if consumed_ticket {
                    self.tickets.register_ticket(&username);
                }
                if handshake.protocol_version >= 766 {
                    let offline_uuid = uuid::Uuid::new_v3(
                        &uuid::Uuid::NAMESPACE_DNS,
                        format!("OfflinePlayer:{username}").as_bytes(),
                    );
                    let success_pkt =
                        disconnect::create_login_success_packet(&username, offline_uuid.as_bytes());
                    let _ = client.write_all(&success_pkt).await;
                    let _ = client.flush().await;

                    let mut ack_buf = [0u8; 32];
                    let _ = tokio::time::timeout(Duration::from_millis(500), client.read(&mut ack_buf)).await;

                    let target_host = handshake.hostname.split('\0').next().unwrap_or("localhost").trim();
                    let packet = disconnect::create_transfer_packet(target_host, transfer_port);
                    let _ = client.write_all(&packet).await;
                    let _ = client.shutdown().await;
                    return Ok(());
                }
                let wake_msg = protocol_chat::build_wake_disconnect_json(&username, remaining_secs);
                let packet = disconnect::create_disconnect_packet(&wake_msg);
                let _ = client.write_all(&packet).await;
                let _ = client.shutdown().await;
                return Ok(());
            }
        }

        instances.register_join_intent(&cfg.id);
        if let Err(e) = instances.start_instance(&cfg.id).await {
            tracing::error!("Wakeup failed for instance '{}': {e}", cfg.id);
        }

        let ready = instances
            .poll_port_ready(cfg.internal_mc_port as u16, Duration::from_secs(20))
            .await;
        if !ready {
            if consumed_ticket {
                self.tickets.register_ticket(&username);
            }
            if handshake.protocol_version >= 766 {
                let offline_uuid = uuid::Uuid::new_v3(
                    &uuid::Uuid::NAMESPACE_DNS,
                    format!("OfflinePlayer:{username}").as_bytes(),
                );
                let success_pkt =
                    disconnect::create_login_success_packet(&username, offline_uuid.as_bytes());
                let _ = client.write_all(&success_pkt).await;
                let _ = client.flush().await;

                let mut ack_buf = [0u8; 32];
                let _ = tokio::time::timeout(Duration::from_millis(500), client.read(&mut ack_buf)).await;

                let target_host = handshake.hostname.split('\0').next().unwrap_or("localhost").trim();
                let packet = disconnect::create_transfer_packet(target_host, transfer_port);
                let _ = client.write_all(&packet).await;
                let _ = client.shutdown().await;
                return Ok(());
            }

            let remaining_secs = instances
                .get_boot_progress(&cfg.id)
                .map(|(elapsed, total, _)| total.saturating_sub(elapsed).max(5))
                .unwrap_or(15);
            let wake_msg = protocol_chat::build_wake_disconnect_json(&username, remaining_secs);
            let packet = disconnect::create_disconnect_packet(&wake_msg);
            let _ = client.write_all(&packet).await;
            let _ = client.shutdown().await;
            return Ok(());
        }

        instances.mark_boot_complete(&cfg.id);
        instances.clear_pending_join_intent(&cfg.id);
        self.proxy(client, buf, cfg.internal_mc_port as u16).await
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

    /// Resolves the target `InstanceConfig` using the exact same precedence order
    /// for both security gate evaluation and backend port routing.
    pub(crate) fn resolve_target_instance(
        &self,
        handshake: Option<&detector::Handshake>,
        fixed_instance: &Option<InstanceConfig>,
    ) -> Option<InstanceConfig> {
        let instances = self.instances.as_ref()?;
        if let Some(fixed) = fixed_instance {
            return instances
                .find_by_internal_port(fixed.internal_mc_port as u16)
                .or_else(|| Some(fixed.clone()));
        }
        let public_port = self.config.get_config().public_port;
        if let Some(handshake) = handshake {
            if !handshake.hostname.is_empty() {
                if let Some(cfg) = instances.find_by_hostname(&handshake.hostname) {
                    return Some(cfg);
                }
            }
        }
        if let Some(cfg) = instances.find_by_external_port(public_port) {
            return Some(cfg);
        }
        instances.get_active_instance()
    }

    /// Resolves the backend MC port for a connection.
    pub(crate) fn resolve_target_port(
        &self,
        handshake: Option<&detector::Handshake>,
        fixed_instance: &Option<InstanceConfig>,
    ) -> u16 {
        self.resolve_target_instance(handshake, fixed_instance)
            .map(|cfg| cfg.internal_mc_port as u16)
            .unwrap_or(self.mc_port)
    }
}
