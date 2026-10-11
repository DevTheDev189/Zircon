//! Local Wi-Fi / LAN mDNS & Multicast Auto-Discovery Engine.
//!
//! Solves the classic Windows firewall and subnet discovery failure by running
//! a resilient dual-protocol listener in Zircon Launcher:
//! 1. Standard Minecraft LAN Multicast: Listens on UDP `224.0.2.60:4445`.
//!    Parses `[MOTD]...[/MOTD][AD]...[/AD]` broadcasted by any open LAN world.
//! 2. Zircon Enhanced Beacon: Listens on UDP port `25564` (and multicast) for
//!    structured JSON packets containing host display name, modloader type,
//!    Minecraft version, and P2P co-op join codes.
//! 3. Maintains an active TTL cache (pruning stale sessions after 6s) and emits
//!    reactive `lan-servers-changed` events to the UI "Nearby Friends" card deck.

use std::collections::HashMap;
use std::net::{Ipv4Addr, SocketAddr};
use std::sync::Arc;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter};
use tokio::sync::RwLock;

/// Information about a discovered local LAN world or co-op session.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DiscoveredLanServer {
    pub id: String,                 // unique key: "host_ip:port"
    pub address: String,            // "192.168.1.105:25565"
    pub host_address: String,       // "192.168.1.105"
    pub port: u16,                  // 25565
    pub motd: String,               // "Alex's World"
    pub host_username: Option<String>,
    pub mc_version: Option<String>,
    pub loader_type: Option<String>,
    pub p2p_port: Option<u16>,
    pub join_code: Option<String>,
    pub is_zircon_enhanced: bool,
    pub last_seen_epoch_ms: i64,
}

/// Zircon Enhanced LAN broadcast beacon payload.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LanBeaconPayload {
    pub zircon: bool,
    pub host_username: String,
    pub world_name: String,
    pub mc_version: String,
    pub loader_type: String,
    pub game_port: u16,
    pub p2p_port: Option<u16>,
    pub join_code: Option<String>,
}

/// Shared engine managing LAN discovery and active server cache.
pub struct LanDiscoveryEngine {
    servers: Arc<RwLock<HashMap<String, DiscoveredLanServer>>>,
    app: AppHandle,
}

impl LanDiscoveryEngine {
    /// Initializes and starts the LAN discovery background listeners and pruner.
    pub fn start(app: AppHandle) -> Arc<Self> {
        let engine = Arc::new(Self {
            servers: Arc::new(RwLock::new(HashMap::new())),
            app: app.clone(),
        });

        // 1. Standard Minecraft Multicast Listener (224.0.2.60:4445)
        let eng_mc = engine.clone();
        tauri::async_runtime::spawn(async move {
            eng_mc.run_minecraft_multicast_listener().await;
        });

        // 2. Zircon Enhanced Broadcast Listener (Port 25564)
        let eng_zc = engine.clone();
        tauri::async_runtime::spawn(async move {
            eng_zc.run_zircon_broadcast_listener().await;
        });

        // 3. Periodic TTL Cache Pruner (every 2 seconds)
        let eng_prune = engine.clone();
        tauri::async_runtime::spawn(async move {
            loop {
                tokio::time::sleep(Duration::from_secs(2)).await;
                eng_prune.prune_expired().await;
            }
        });

        engine
    }

    /// Returns a snapshot of all currently active discovered LAN games.
    pub async fn get_servers(&self) -> Vec<DiscoveredLanServer> {
        let map = self.servers.read().await;
        let mut list: Vec<DiscoveredLanServer> = map.values().cloned().collect();
        list.sort_by(|a, b| b.last_seen_epoch_ms.cmp(&a.last_seen_epoch_ms));
        list
    }

    /// Broadcasts a Zircon enhanced announcement beacon across local subnets.
    pub async fn broadcast_beacon(payload: LanBeaconPayload) -> Result<(), String> {
        let socket = tokio::net::UdpSocket::bind("0.0.0.0:0")
            .await
            .map_err(|e| format!("Failed to bind UDP socket for beacon: {e}"))?;
        let _ = socket.set_broadcast(true);

        let data = serde_json::to_vec(&payload)
            .map_err(|e| format!("Failed to serialize beacon payload: {e}"))?;

        // Send to local broadcast on 25564
        let _ = socket.send_to(&data, "255.255.255.255:25564").await;
        // Also send to Minecraft multicast 224.0.2.60:4445
        let _ = socket.send_to(&data, "224.0.2.60:4445").await;

        Ok(())
    }

    async fn record_server(&self, mut server: DiscoveredLanServer) {
        server.last_seen_epoch_ms = chrono::Utc::now().timestamp_millis();
        let id = server.id.clone();

        let mut map = self.servers.write().await;
        map.insert(id, server);
        let list: Vec<DiscoveredLanServer> = map.values().cloned().collect();
        drop(map);

        let _ = self.app.emit("lan-servers-changed", list);
    }

    async fn prune_expired(&self) {
        let now = chrono::Utc::now().timestamp_millis();
        let timeout_ms = 6000; // 6 seconds without a beacon -> expired

        let mut map = self.servers.write().await;
        let before_count = map.len();
        map.retain(|_, s| now - s.last_seen_epoch_ms < timeout_ms);

        if map.len() != before_count {
            let list: Vec<DiscoveredLanServer> = map.values().cloned().collect();
            drop(map);
            let _ = self.app.emit("lan-servers-changed", list);
        }
    }

    async fn run_minecraft_multicast_listener(&self) {
        let multi_addr = Ipv4Addr::new(224, 0, 2, 60);
        let port = 4445;

        let socket = match bind_reusable_udp_socket(port) {
            Ok(sock) => {
                let _ = sock.join_multicast_v4(&multi_addr, &Ipv4Addr::UNSPECIFIED);
                tokio::net::UdpSocket::from_std(sock).ok()
            }
            Err(e) => {
                tracing::warn!("Could not bind multicast socket on port {port}: {e}");
                None
            }
        };

        let Some(sock) = socket else { return };
        tracing::info!("Listening for Minecraft LAN broadcasts on 224.0.2.60:4445");

        let mut buf = [0u8; 4096];
        loop {
            match sock.recv_from(&mut buf).await {
                Ok((len, src)) => {
                    let slice = &buf[..len];
                    // Check if this packet is actually a Zircon enhanced JSON beacon
                    if let Ok(beacon) = serde_json::from_slice::<LanBeaconPayload>(slice) {
                        if beacon.zircon {
                            let server = DiscoveredLanServer {
                                id: format!("{}:{}", src.ip(), beacon.game_port),
                                address: format!("{}:{}", src.ip(), beacon.game_port),
                                host_address: src.ip().to_string(),
                                port: beacon.game_port,
                                motd: beacon.world_name,
                                host_username: Some(beacon.host_username),
                                mc_version: Some(beacon.mc_version),
                                loader_type: Some(beacon.loader_type),
                                p2p_port: beacon.p2p_port,
                                join_code: beacon.join_code,
                                is_zircon_enhanced: true,
                                last_seen_epoch_ms: chrono::Utc::now().timestamp_millis(),
                            };
                            self.record_server(server).await;
                            continue;
                        }
                    }

                    // Otherwise parse standard Minecraft LAN packet: [MOTD]name[/MOTD][AD]port[/AD]
                    if let Ok(text) = std::str::from_utf8(slice) {
                        if let Some(server) = parse_standard_minecraft_lan(text, src) {
                            self.record_server(server).await;
                        }
                    }
                }
                Err(e) => {
                    tracing::debug!("Error reading multicast packet: {e}");
                    tokio::time::sleep(Duration::from_millis(500)).await;
                }
            }
        }
    }

    async fn run_zircon_broadcast_listener(&self) {
        let port = 25564;
        let socket = match bind_reusable_udp_socket(port) {
            Ok(sock) => tokio::net::UdpSocket::from_std(sock).ok(),
            Err(e) => {
                tracing::debug!("Could not bind Zircon LAN beacon socket on port {port}: {e}");
                None
            }
        };

        let Some(sock) = socket else { return };
        tracing::info!("Listening for Zircon enhanced LAN beacons on UDP port {port}");

        let mut buf = [0u8; 4096];
        loop {
            match sock.recv_from(&mut buf).await {
                Ok((len, src)) => {
                    let slice = &buf[..len];
                    if let Ok(beacon) = serde_json::from_slice::<LanBeaconPayload>(slice) {
                        if beacon.zircon {
                            let server = DiscoveredLanServer {
                                id: format!("{}:{}", src.ip(), beacon.game_port),
                                address: format!("{}:{}", src.ip(), beacon.game_port),
                                host_address: src.ip().to_string(),
                                port: beacon.game_port,
                                motd: beacon.world_name,
                                host_username: Some(beacon.host_username),
                                mc_version: Some(beacon.mc_version),
                                loader_type: Some(beacon.loader_type),
                                p2p_port: beacon.p2p_port,
                                join_code: beacon.join_code,
                                is_zircon_enhanced: true,
                                last_seen_epoch_ms: chrono::Utc::now().timestamp_millis(),
                            };
                            self.record_server(server).await;
                        }
                    }
                }
                Err(_) => {
                    tokio::time::sleep(Duration::from_millis(500)).await;
                }
            }
        }
    }
}

/// Parses Minecraft LAN packet: `[MOTD]World Name - Host[/MOTD][AD]Port[/AD]`
fn parse_standard_minecraft_lan(raw: &str, src: SocketAddr) -> Option<DiscoveredLanServer> {
    let motd_start = raw.find("[MOTD]")? + 6;
    let motd_end = raw.find("[/MOTD]")?;
    if motd_start >= motd_end {
        return None;
    }
    let motd = raw[motd_start..motd_end].trim().to_string();

    let ad_start = raw.find("[AD]")? + 4;
    let ad_end = raw.find("[/AD]")?;
    if ad_start >= ad_end {
        return None;
    }
    let port_str = raw[ad_start..ad_end].trim();
    let port: u16 = port_str.parse().ok()?;

    // Standard Minecraft format often includes "World Name - PlayerName"
    let (clean_motd, host_user) = if let Some(dash_idx) = motd.rfind(" - ") {
        let (w, u) = motd.split_at(dash_idx);
        (w.trim().to_string(), Some(u[3..].trim().to_string()))
    } else {
        (motd.clone(), None)
    };

    Some(DiscoveredLanServer {
        id: format!("{}:{port}", src.ip()),
        address: format!("{}:{port}", src.ip()),
        host_address: src.ip().to_string(),
        port,
        motd: clean_motd,
        host_username: host_user,
        mc_version: None,
        loader_type: None,
        p2p_port: None,
        join_code: None,
        is_zircon_enhanced: false,
        last_seen_epoch_ms: chrono::Utc::now().timestamp_millis(),
    })
}

/// Binds a UDP socket with SO_REUSEADDR enabled for collision-free multi-app sharing.
fn bind_reusable_udp_socket(port: u16) -> std::io::Result<std::net::UdpSocket> {
    let sock = std::net::UdpSocket::bind(format!("0.0.0.0:{port}"))?;
    sock.set_nonblocking(true)?;

    #[cfg(target_os = "windows")]
    {
        use std::os::windows::io::AsRawSocket;
        extern "system" {
            fn setsockopt(
                s: usize,
                level: i32,
                optname: i32,
                optval: *const i8,
                optlen: i32,
            ) -> i32;
        }
        let raw = sock.as_raw_socket() as usize;
        let optval: i32 = 1;
        unsafe {
            let _ = setsockopt(
                raw,
                0xffff, // SOL_SOCKET
                0x0004, // SO_REUSEADDR
                &optval as *const i32 as *const i8,
                std::mem::size_of::<i32>() as i32,
            );
        }
    }

    Ok(sock)
}
