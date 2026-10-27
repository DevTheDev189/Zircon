//! Builds Minecraft protocol frames, specifically the clientbound
//! `Disconnect (Login)` packet (packet ID `0x00` in the login state) that the
//! connection gate writes before closing a rejected socket.
//!
//! Frame layout: `[VarInt frameLen][VarInt 0x00][VarInt msgLen][msg bytes]`.
//!
//! Port of `com.mcmanager.server.multiplexer.MinecraftDisconnectUtil`.

use super::varint::write_varint;

/// Creates a framed Minecraft `Disconnect (Login)` packet carrying a chat JSON
/// message.
pub fn create_disconnect_packet(json_message: &str) -> Vec<u8> {
    let message_bytes = json_message.as_bytes();

    let mut packet = Vec::with_capacity(message_bytes.len() + 5);
    write_varint(&mut packet, 0x00); // Packet ID for Login Disconnect
    write_varint(&mut packet, message_bytes.len() as i32); // String length
    packet.extend_from_slice(message_bytes); // String payload

    let mut frame = Vec::with_capacity(packet.len() + 5);
    write_varint(&mut frame, packet.len() as i32); // Total frame length
    frame.extend_from_slice(&packet);
    frame
}

/// Creates a framed Minecraft `Status Response` packet (packet ID `0x00` in status state)
/// carrying the SLP JSON response.
pub fn create_status_response_packet(json_status: &str) -> Vec<u8> {
    create_disconnect_packet(json_status)
}

/// Creates a framed Minecraft 1.20.5+ (Protocol 766+) `ClientboundTransferPacket` (packet ID `0x0B`).
///
/// Instructs modern clients (Vanilla, Prism, CurseForge, Modrinth App on MC 1.20.5+) to seamlessly
/// transfer to the target host and port without a disconnect screen.
///
/// Frame layout: `[VarInt frameLen][VarInt 0x0B][VarInt hostLen][host bytes][VarInt port]`.
pub fn create_transfer_packet(host: &str, port: u16) -> Vec<u8> {
    let host_bytes = host.as_bytes();
    let mut packet = Vec::with_capacity(host_bytes.len() + 10);
    write_varint(&mut packet, 0x0B); // Packet ID for Clientbound Transfer
    write_varint(&mut packet, host_bytes.len() as i32); // Host string length
    packet.extend_from_slice(host_bytes); // Host string payload
    write_varint(&mut packet, port as i32); // Target port as VarInt

    let mut frame = Vec::with_capacity(packet.len() + 5);
    write_varint(&mut frame, packet.len() as i32); // Total frame length
    frame.extend_from_slice(&packet);
    frame
}

/// Creates a framed Minecraft `Login Success` packet (packet ID `0x02` in login state).
///
/// Advancing the client to Configuration state allows modern 1.20.5+ clients to receive
/// `ClientboundTransferPacket (0x0B)` or limbo holding packets without crashing with a Netty DecoderException.
///
/// Frame layout: `[VarInt frameLen][VarInt 0x02][UUID 16 bytes][VarInt usernameLen][username bytes][VarInt 0]`.
pub fn create_login_success_packet(username: &str, uuid_bytes: &[u8; 16]) -> Vec<u8> {
    let username_bytes = username.as_bytes();
    let mut packet = Vec::with_capacity(16 + username_bytes.len() + 10);
    write_varint(&mut packet, 0x02); // Packet ID for Login Success
    packet.extend_from_slice(uuid_bytes); // 16-byte UUID
    write_varint(&mut packet, username_bytes.len() as i32); // Username string length
    packet.extend_from_slice(username_bytes); // Username payload
    write_varint(&mut packet, 0); // Number of properties (0)

    let mut frame = Vec::with_capacity(packet.len() + 5);
    write_varint(&mut frame, packet.len() as i32);
    frame.extend_from_slice(&packet);
    frame
}

/// The in-game message shown when a connection is rejected by the join gate.
pub fn build_custom_error_message(server_url: &str) -> String {
    let sanitized: String = server_url
        .trim()
        .chars()
        .filter(|c| !c.is_control() && *c != '§')
        .take(128)
        .collect();
    let display_url = if sanitized.is_empty() {
        "this server's address"
    } else {
        sanitized.as_str()
    };

    let payload = serde_json::json!({
        "text": "⚡ Zircon Client Required\n\n",
        "color": "red",
        "bold": true,
        "extra": [
            {
                "text": "You must use the official Zircon Launcher to join this server.\n\n",
                "color": "gray",
                "bold": false
            },
            {
                "text": "How to connect:\n",
                "color": "gold",
                "bold": true
            },
            {
                "text": "1. Download the Zircon Launcher at ",
                "color": "white",
                "bold": false
            },
            {
                "text": "https://zirconmc.net/downloads.html\n",
                "color": "aqua",
                "underlined": true,
                "bold": false
            },
            {
                "text": "2. Log in with your Microsoft account\n",
                "color": "white",
                "bold": false
            },
            {
                "text": "3. Add ",
                "color": "white",
                "bold": false
            },
            {
                "text": display_url,
                "color": "gold",
                "bold": true
            },
            {
                "text": " in the UI and click PLAY",
                "color": "white",
                "bold": false
            }
        ]
    });

    serde_json::to_string(&payload).unwrap_or_else(|_| {
        r#"{"text":"⚡ Zircon Client Required\n\nDownload at https://zirconmc.net/downloads.html","color":"red"}"#.to_string()
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::multiplexer::varint::read_varint;

    #[test]
    fn disconnect_frame_layout_is_valid() {
        let frame = create_disconnect_packet(r#"{"text":"bye"}"#);
        // [VarInt len][VarInt 0x00][VarInt msgLen][msg]
        let (frame_len, bytes) = read_varint(&frame, 0).unwrap();
        let offset = bytes;
        assert_eq!(frame.len() as i32 - offset as i32, frame_len);
        let (packet_id, bytes) = read_varint(&frame, offset).unwrap();
        let offset = offset + bytes;
        assert_eq!(0, packet_id);
        let (msg_len, bytes) = read_varint(&frame, offset).unwrap();
        let offset = offset + bytes;
        assert_eq!(r#"{"text":"bye"}"#.len() as i32, msg_len);
        let msg = String::from_utf8(frame[offset..].to_vec()).unwrap();
        assert_eq!(r#"{"text":"bye"}"#, msg);
    }

    #[test]
    fn error_message_contains_zircon_requirement_and_instructions() {
        let message = build_custom_error_message("mc.zirconmc.net");
        assert!(message.contains("Zircon Client Required"));
        assert!(message.contains("https://zirconmc.net/downloads.html"));
        assert!(message.contains("mc.zirconmc.net"));
    }

    #[test]
    fn status_response_frame_layout_is_valid() {
        let status_json = r#"{"description":{"text":"Waking up..."}}"#;
        let frame = create_status_response_packet(status_json);
        let (frame_len, bytes) = read_varint(&frame, 0).unwrap();
        let offset = bytes;
        assert_eq!(frame.len() as i32 - offset as i32, frame_len);
        let (packet_id, bytes) = read_varint(&frame, offset).unwrap();
        let offset = offset + bytes;
        assert_eq!(0, packet_id);
        let (msg_len, bytes) = read_varint(&frame, offset).unwrap();
        let offset = offset + bytes;
        assert_eq!(status_json.len() as i32, msg_len);
        let payload = String::from_utf8(frame[offset..].to_vec()).unwrap();
        assert_eq!(status_json, payload);
    }
}
