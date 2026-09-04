//! Discord Rich Presence (RPC) client for Zircon Launcher.
//!
//! Connects to the local Discord desktop client via local IPC (Named Pipes on
//! Windows, Unix Domain Sockets on Linux/macOS) and synchronizes the active
//! Minecraft session.

use std::time::Duration;
use serde::{Deserialize, Serialize};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tracing::{debug, warn};

/// Default Discord application client ID for Zircon Launcher.
pub const ZIRCON_DISCORD_CLIENT_ID: &str = "1345900000000000000";

/// Opcodes in Discord IPC protocol.
const OP_HANDSHAKE: u32 = 0;
const OP_FRAME: u32 = 1;
const OP_CLOSE: u32 = 2;

/// Timestamps for the activity duration.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct ActivityTimestamps {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end: Option<i64>,
}

/// Asset images and tooltips for Discord presence.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct ActivityAssets {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub large_image: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub large_text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub small_image: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub small_text: Option<String>,
}

/// Discord Rich Presence activity model.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct Activity {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub state: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timestamps: Option<ActivityTimestamps>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub assets: Option<ActivityAssets>,
}

impl Activity {
    /// Constructs a session activity with standard Zircon branding.
    pub fn new(
        details: impl Into<String>,
        state: impl Into<String>,
        start_epoch_secs: Option<i64>,
        loader: Option<&str>,
    ) -> Self {
        let (small_image, small_text) = match loader {
            Some(l) if !l.trim().is_empty() => {
                let norm = l.trim().to_lowercase();
                let badge = match norm.as_str() {
                    "fabric" => ("fabric".to_string(), "Fabric".to_string()),
                    "forge" => ("forge".to_string(), "Forge".to_string()),
                    "neoforge" | "neo_forge" => ("neoforge".to_string(), "NeoForge".to_string()),
                    "quilt" => ("quilt".to_string(), "Quilt".to_string()),
                    _ => ("vanilla".to_string(), "Vanilla".to_string()),
                };
                (Some(badge.0), Some(badge.1))
            }
            _ => (Some("vanilla".to_string()), Some("Vanilla".to_string())),
        };

        Self {
            details: Some(details.into()),
            state: Some(state.into()),
            timestamps: start_epoch_secs.map(|s| ActivityTimestamps {
                start: Some(s),
                end: None,
            }),
            assets: Some(ActivityAssets {
                large_image: Some("zircon_logo".to_string()),
                large_text: Some("Zircon Launcher".to_string()),
                small_image,
                small_text,
            }),
        }
    }
}

/// Local IPC stream abstraction across Windows named pipes and Unix domain sockets.
enum IpcStream {
    #[cfg(windows)]
    Windows(tokio::net::windows::named_pipe::NamedPipeClient),
    #[cfg(unix)]
    Unix(tokio::net::UnixStream),
}

impl IpcStream {
    async fn read_exact(&mut self, buf: &mut [u8]) -> Result<(), std::io::Error> {
        match self {
            #[cfg(windows)]
            IpcStream::Windows(pipe) => pipe.read_exact(buf).await.map(|_| ()),
            #[cfg(unix)]
            IpcStream::Unix(stream) => stream.read_exact(buf).await.map(|_| ()),
        }
    }

    async fn write_all(&mut self, buf: &[u8]) -> Result<(), std::io::Error> {
        match self {
            #[cfg(windows)]
            IpcStream::Windows(pipe) => {
                pipe.write_all(buf).await?;
                pipe.flush().await
            }
            #[cfg(unix)]
            IpcStream::Unix(stream) => {
                stream.write_all(buf).await?;
                stream.flush().await
            }
        }
    }
}

/// Asynchronous Discord Rich Presence client.
pub struct DiscordRpcClient {
    stream: IpcStream,
    pid: u32,
}

impl DiscordRpcClient {
    /// Attempts to connect to an active local Discord client.
    pub async fn connect(client_id: &str) -> Result<Self, String> {
        let pid = std::process::id();
        let stream = Self::open_ipc_stream().await?;
        let mut client = Self { stream, pid };

        // Send Handshake
        let handshake_payload = serde_json::json!({
            "v": 1,
            "client_id": client_id
        });
        client
            .send_packet(OP_HANDSHAKE, &handshake_payload.to_string())
            .await
            .map_err(|e| format!("Failed to send Discord RPC handshake: {e}"))?;

        // Read Handshake response with a 3-second timeout
        let read_fut = client.read_packet();
        match tokio::time::timeout(Duration::from_secs(3), read_fut).await {
            Ok(Ok((op, payload))) => {
                debug!("Discord RPC connected: op={op}, payload={payload}");
                Ok(client)
            }
            Ok(Err(e)) => Err(format!("Discord RPC handshake read error: {e}")),
            Err(_) => Err("Timed out waiting for Discord RPC handshake response".to_string()),
        }
    }

    /// Sets or updates the active Discord Rich Presence.
    pub async fn set_activity(&mut self, activity: Activity) -> Result<(), String> {
        let nonce = uuid::Uuid::new_v4().to_string();
        let payload = serde_json::json!({
            "cmd": "SET_ACTIVITY",
            "args": {
                "pid": self.pid,
                "activity": activity
            },
            "nonce": nonce
        });

        self.send_packet(OP_FRAME, &payload.to_string())
            .await
            .map_err(|e| format!("Failed to set Discord activity: {e}"))
    }

    /// Clears the active Discord presence.
    pub async fn clear_activity(&mut self) -> Result<(), String> {
        let nonce = uuid::Uuid::new_v4().to_string();
        let payload = serde_json::json!({
            "cmd": "SET_ACTIVITY",
            "args": {
                "pid": self.pid,
                "activity": serde_json::Value::Null
            },
            "nonce": nonce
        });

        self.send_packet(OP_FRAME, &payload.to_string())
            .await
            .map_err(|e| format!("Failed to clear Discord activity: {e}"))
    }

    /// Closes the connection to Discord RPC.
    pub async fn close(&mut self) -> Result<(), String> {
        let payload = serde_json::json!({});
        let _ = self.send_packet(OP_CLOSE, &payload.to_string()).await;
        Ok(())
    }

    async fn send_packet(&mut self, opcode: u32, payload: &str) -> Result<(), std::io::Error> {
        let payload_bytes = payload.as_bytes();
        let length = payload_bytes.len() as u32;

        let mut header = [0u8; 8];
        header[0..4].copy_from_slice(&opcode.to_le_bytes());
        header[4..8].copy_from_slice(&length.to_le_bytes());

        self.stream.write_all(&header).await?;
        self.stream.write_all(payload_bytes).await?;
        Ok(())
    }

    async fn read_packet(&mut self) -> Result<(u32, String), std::io::Error> {
        let mut header = [0u8; 8];
        self.stream.read_exact(&mut header).await?;

        let opcode = u32::from_le_bytes(header[0..4].try_into().unwrap());
        let length = u32::from_le_bytes(header[4..8].try_into().unwrap()) as usize;

        let mut buffer = vec![0u8; length];
        self.stream.read_exact(&mut buffer).await?;

        let text = String::from_utf8(buffer)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        Ok((opcode, text))
    }

    #[cfg(windows)]
    async fn open_ipc_stream() -> Result<IpcStream, String> {
        use tokio::net::windows::named_pipe::ClientOptions;
        for i in 0..10 {
            let pipe_name = format!(r"\\.\pipe\discord-ipc-{i}");
            if let Ok(client) = ClientOptions::new().open(&pipe_name) {
                return Ok(IpcStream::Windows(client));
            }
        }
        Err("No active Discord IPC named pipe found (\\.\\pipe\\discord-ipc-0..9)".to_string())
    }

    #[cfg(unix)]
    async fn open_ipc_stream() -> Result<IpcStream, String> {
        let mut search_paths = Vec::new();
        if let Ok(runtime_dir) = std::env::var("XDG_RUNTIME_DIR") {
            search_paths.push(std::path::PathBuf::from(runtime_dir));
        }
        if let Ok(tmpdir) = std::env::var("TMPDIR") {
            let tmp_buf = std::path::PathBuf::from(tmpdir);
            search_paths.push(tmp_buf.clone());
            search_paths.push(tmp_buf.join("app/com.discordapp.Discord"));
        }
        search_paths.push(std::path::PathBuf::from("/tmp"));

        for base in search_paths {
            for i in 0..10 {
                let sock_path = base.join(format!("discord-ipc-{i}"));
                if sock_path.exists() {
                    if let Ok(stream) = tokio::net::UnixStream::connect(&sock_path).await {
                        return Ok(IpcStream::Unix(stream));
                    }
                }
            }
        }
        Err("No active Discord IPC unix domain socket found".to_string())
    }
}

/// Asynchronously attempts to initialize presence if enabled in launcher settings.
/// Never panics or fails the caller.
pub async fn update_discord_presence(
    client_slot: &tokio::sync::Mutex<Option<DiscordRpcClient>>,
    activity: Activity,
) {
    let mut guard = client_slot.lock().await;
    if guard.is_none() {
        match DiscordRpcClient::connect(ZIRCON_DISCORD_CLIENT_ID).await {
            Ok(client) => {
                *guard = Some(client);
            }
            Err(e) => {
                debug!("Discord RPC not connected: {e}");
                return;
            }
        }
    }

    if let Some(client) = guard.as_mut() {
        if let Err(e) = client.set_activity(activity).await {
            warn!("Failed to update Discord presence: {e}");
            // Reset client so it reconnects on next attempt
            guard.take();
        }
    }
}

/// Asynchronously clears Discord presence and closes the IPC session.
pub async fn clear_discord_presence(client_slot: &tokio::sync::Mutex<Option<DiscordRpcClient>>) {
    let mut guard = client_slot.lock().await;
    if let Some(mut client) = guard.take() {
        let _ = client.clear_activity().await;
        let _ = client.close().await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn activity_builder_server() {
        let act = Activity::new(
            "Playing on Survival Hub",
            "Minecraft 1.21.4 (Fabric)",
            Some(1700000000),
            Some("fabric"),
        );

        assert_eq!(act.details.as_deref(), Some("Playing on Survival Hub"));
        assert_eq!(act.state.as_deref(), Some("Minecraft 1.21.4 (Fabric)"));
        assert_eq!(
            act.timestamps,
            Some(ActivityTimestamps {
                start: Some(1700000000),
                end: None
            })
        );
        let assets = act.assets.unwrap();
        assert_eq!(assets.large_image.as_deref(), Some("zircon_logo"));
        assert_eq!(assets.large_text.as_deref(), Some("Zircon Launcher"));
        assert_eq!(assets.small_image.as_deref(), Some("fabric"));
        assert_eq!(assets.small_text.as_deref(), Some("Fabric"));
    }

    #[test]
    fn activity_builder_offline() {
        let act = Activity::new(
            "Playing Offline: Modded 1.20",
            "Minecraft 1.20.1 (NeoForge)",
            Some(1700000000),
            Some("neoforge"),
        );

        assert_eq!(
            act.details.as_deref(),
            Some("Playing Offline: Modded 1.20")
        );
        assert_eq!(act.state.as_deref(), Some("Minecraft 1.20.1 (NeoForge)"));
        let assets = act.assets.unwrap();
        assert_eq!(assets.small_image.as_deref(), Some("neoforge"));
        assert_eq!(assets.small_text.as_deref(), Some("NeoForge"));
    }

    #[test]
    fn activity_json_serialization() {
        let act = Activity::new(
            "Playing on test.server.net",
            "Minecraft 1.21.4",
            Some(1700000000),
            None,
        );
        let json = serde_json::to_string(&act).unwrap();
        assert!(json.contains("Playing on test.server.net"));
        assert!(json.contains("zircon_logo"));
        assert!(json.contains("vanilla"));
    }
}
