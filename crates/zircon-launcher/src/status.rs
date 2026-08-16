//! Minecraft server-list status ping — the query every client uses to render
//! the multiplayer screen: a TCP handshake with next-state 1, a status request,
//! the JSON status response, and a ping-pong packet for a real latency reading.
//!
//! This works both against the Zircon wrapper's public port (the multiplexer
//! forwards status handshakes straight to the backend instance) and against any
//! vanilla/modded server (Hypixel, Wynncraft, ...), so the server list can show
//! live player counts and pings for every entry.

use std::time::{Duration, Instant};

use serde::Deserialize;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

use crate::error::LauncherError;

/// A parsed server-list status response.
#[derive(Debug, Clone, Default)]
pub struct ServerStatus {
    pub online: u32,
    pub max: u32,
    pub version: String,
    /// Round-trip latency in milliseconds.
    pub ping_ms: u32,
}

const CONNECT_TIMEOUT: Duration = Duration::from_secs(3);
const READ_TIMEOUT: Duration = Duration::from_secs(3);

/// Pings `host:port` and returns the parsed status. Errors when the server is
/// unreachable, times out, or sends an unreadable response.
pub async fn ping_status(host: &str, port: u16) -> Result<ServerStatus, LauncherError> {
    let mut stream = tokio::time::timeout(CONNECT_TIMEOUT, TcpStream::connect((host, port)))
        .await
        .map_err(|_| LauncherError::Network(format!("status ping to {host}:{port} timed out")))?
        .map_err(LauncherError::from)?;

    // Handshake: [0x00][protocol][host len][host][port][next state 1 = status].
    let mut handshake = Vec::new();
    write_varint(&mut handshake, 0x00);
    write_varint(&mut handshake, -1); // any protocol — we only want status
    let host_bytes = host.as_bytes();
    write_varint(&mut handshake, host_bytes.len() as i32);
    handshake.extend_from_slice(host_bytes);
    handshake.extend_from_slice(&port.to_be_bytes());
    write_varint(&mut handshake, 0x01); // next state: status
    write_frame(&mut stream, &handshake).await?;

    // Status request: [0x00] with an empty body.
    write_frame(&mut stream, &[0x00]).await?;

    // Status response; the time to first byte doubles as a latency fallback for
    // servers that close the connection without answering the ping.
    let started = Instant::now();
    let frame = read_frame(&mut stream).await?;
    let status_rtt_ms = started.elapsed().as_millis() as u32;
    let json = parse_status_frame(&frame)?;
    let status: StatusJson = serde_json::from_slice(json)
        .map_err(|e| LauncherError::Parse(format!("invalid server status JSON: {e}")))?;

    // Ping-pong for a clean RTT. Servers that close right after status fall
    // back to the status-response RTT.
    let payload = (started.elapsed().as_nanos() as u64).to_be_bytes();
    let ping_started = Instant::now();
    let ping_ms = match write_ping_and_read(&mut stream, &payload).await {
        Ok(()) => ping_started.elapsed().as_millis() as u32,
        Err(_) => status_rtt_ms,
    };

    Ok(ServerStatus {
        online: status.players.online,
        max: status.players.max,
        version: status
            .version
            .as_ref()
            .map(|v| v.name.clone())
            .unwrap_or_default(),
        ping_ms,
    })
}

/// Sends one length-prefixed packet.
async fn write_frame(stream: &mut TcpStream, packet: &[u8]) -> Result<(), LauncherError> {
    let mut frame = Vec::with_capacity(packet.len() + 5);
    write_varint(&mut frame, packet.len() as i32);
    frame.extend_from_slice(packet);
    tokio::time::timeout(READ_TIMEOUT, stream.write_all(&frame))
        .await
        .map_err(|_| LauncherError::Network("status ping write timed out".to_string()))?
        .map_err(LauncherError::from)
}

/// Reads one length-prefixed packet, bounded to a sane size.
async fn read_frame(stream: &mut TcpStream) -> Result<Vec<u8>, LauncherError> {
    // VarInt frame length, one byte at a time.
    let mut len_bytes = Vec::new();
    loop {
        let byte = read_exact(stream, 1).await?[0];
        len_bytes.push(byte);
        if byte & 0x80 == 0 {
            break;
        }
        if len_bytes.len() > 5 {
            return Err(LauncherError::Parse(
                "status frame length exceeds VarInt bounds".to_string(),
            ));
        }
    }
    let (frame_len, _) = read_varint(&len_bytes)
        .ok_or_else(|| LauncherError::Parse("invalid status frame length".to_string()))?;
    if frame_len < 0 || frame_len as usize > (1 << 20) {
        return Err(LauncherError::Parse("status frame too large".to_string()));
    }
    read_exact(stream, frame_len as usize).await
}

/// Reads exactly `n` bytes (or fails when the connection closes early).
async fn read_exact(stream: &mut TcpStream, n: usize) -> Result<Vec<u8>, LauncherError> {
    let mut out = vec![0u8; n];
    let mut read = 0;
    while read < n {
        let got = tokio::time::timeout(READ_TIMEOUT, stream.read(&mut out[read..]))
            .await
            .map_err(|_| LauncherError::Network("status ping read timed out".to_string()))?
            .map_err(LauncherError::from)?;
        if got == 0 {
            return Err(LauncherError::Network(
                "server closed the connection".to_string(),
            ));
        }
        read += got;
    }
    Ok(out)
}

/// Sends the `0x01` ping packet and waits for the echoed response.
async fn write_ping_and_read(
    stream: &mut TcpStream,
    payload: &[u8; 8],
) -> Result<(), LauncherError> {
    let mut packet = Vec::with_capacity(9);
    write_varint(&mut packet, 0x01);
    packet.extend_from_slice(payload);
    write_frame(stream, &packet).await?;
    let frame = read_frame(stream).await?;
    // Response is [packet id 0x01][8-byte echo].
    if frame.len() == 9 && frame[0] == 0x01 {
        Ok(())
    } else {
        Err(LauncherError::Parse("unexpected ping response".to_string()))
    }
}

/// Parses a status response frame body: [packet id][json length][json].
fn parse_status_frame(frame: &[u8]) -> Result<&[u8], LauncherError> {
    let mut cursor = 0;
    let (packet_id, used) = read_varint(frame)
        .ok_or_else(|| LauncherError::Parse("missing status packet id".to_string()))?;
    cursor += used;
    if packet_id != 0x00 {
        return Err(LauncherError::Parse(format!(
            "unexpected status packet id {packet_id}"
        )));
    }
    let (json_len, used) = read_varint(&frame[cursor..])
        .ok_or_else(|| LauncherError::Parse("missing status JSON length".to_string()))?;
    cursor += used;
    if json_len < 0 || cursor + json_len as usize > frame.len() {
        return Err(LauncherError::Parse(
            "status JSON length out of bounds".to_string(),
        ));
    }
    Ok(&frame[cursor..cursor + json_len as usize])
}

/// Writes a Minecraft VarInt (LEB128, max 5 bytes).
fn write_varint(out: &mut Vec<u8>, value: i32) {
    let mut value = value as u32;
    loop {
        let mut byte = (value & 0x7f) as u8;
        value >>= 7;
        if value != 0 {
            byte |= 0x80;
        }
        out.push(byte);
        if value == 0 {
            break;
        }
    }
}

/// Reads a VarInt from the start of `buf`, returning `(value, bytes_consumed)`.
fn read_varint(buf: &[u8]) -> Option<(i32, usize)> {
    let mut value: i32 = 0;
    let mut shift = 0;
    for (i, &byte) in buf.iter().take(5).enumerate() {
        value |= ((byte & 0x7f) as i32) << shift;
        if byte & 0x80 == 0 {
            return Some((value, i + 1));
        }
        shift += 7;
    }
    None
}

/// Shape of the server-list status JSON (`players`, `version`).
#[derive(Debug, Deserialize, Default)]
struct StatusJson {
    #[serde(default)]
    players: PlayersJson,
    #[serde(default)]
    version: Option<VersionJson>,
}

#[derive(Debug, Deserialize, Default)]
struct PlayersJson {
    #[serde(default)]
    online: u32,
    #[serde(default)]
    max: u32,
}

#[derive(Debug, Deserialize)]
struct VersionJson {
    #[serde(default)]
    name: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn varint_round_trips_common_values() {
        for value in [0, 1, 127, 128, 255, 300, 25565, 2147483647, -1] {
            let mut buf = Vec::new();
            write_varint(&mut buf, value);
            let (decoded, used) = read_varint(&buf).expect("valid varint");
            assert_eq!(value, decoded);
            assert_eq!(buf.len(), used);
        }
    }

    #[test]
    fn varint_encoding_matches_known_bytes() {
        let mut buf = Vec::new();
        write_varint(&mut buf, 0);
        assert_eq!(vec![0x00], buf);

        buf.clear();
        write_varint(&mut buf, 127);
        assert_eq!(vec![0x7f], buf);

        buf.clear();
        write_varint(&mut buf, 128);
        assert_eq!(vec![0x80, 0x01], buf);
    }

    #[test]
    fn truncated_varint_is_rejected() {
        // 0x80 with no continuation byte.
        assert!(read_varint(&[0x80]).is_none());
        assert!(read_varint(&[0x80, 0x80, 0x80, 0x80, 0x80]).is_none());
    }

    #[test]
    fn status_frame_parses_packet_and_json() {
        let json = br#"{"players":{"online":12,"max":50}}"#;
        let mut frame = Vec::new();
        write_varint(&mut frame, 0x00);
        write_varint(&mut frame, json.len() as i32);
        frame.extend_from_slice(json);

        let parsed = parse_status_frame(&frame).expect("valid frame");
        assert_eq!(json, parsed);
        let status: StatusJson = serde_json::from_slice(parsed).unwrap();
        assert_eq!(12, status.players.online);
        assert_eq!(50, status.players.max);
    }

    #[test]
    fn status_frame_rejects_unknown_packet_id() {
        let json = b"{}";
        let mut frame = Vec::new();
        write_varint(&mut frame, 0x02);
        write_varint(&mut frame, json.len() as i32);
        frame.extend_from_slice(json);
        assert!(parse_status_frame(&frame).is_err());
    }
}
