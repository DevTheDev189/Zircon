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

/// The in-game message shown when a connection is rejected by the join gate.
pub fn build_custom_error_message(server_url: &str) -> String {
    let clean_url = server_url.trim();
    let display_url = if clean_url.is_empty() {
        "this server's address"
    } else {
        clean_url
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
}
