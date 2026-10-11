//! Minecraft JSON chat payloads and wire frame builders for Disconnect and SLP packets.

/// Writes a 32-bit signed integer as a Minecraft protocol VarInt into `buf`.
pub fn write_varint(buf: &mut Vec<u8>, mut value: i32) {
    loop {
        if (value & !0x7F) == 0 {
            buf.push(value as u8);
            return;
        }
        buf.push(((value & 0x7F) | 0x80) as u8);
        value = (value as u32 >> 7) as i32;
    }
}

/// Creates a complete, framed Minecraft `Disconnect (Login)` packet (Packet ID 0x00)
/// carrying `json_message`.
///
/// Frame layout: `[VarInt frameLen][VarInt 0x00][VarInt msgLen][msg bytes]`.
pub fn create_login_disconnect_frame(json_message: &str) -> Vec<u8> {
    let msg_bytes = json_message.as_bytes();

    let mut packet = Vec::with_capacity(msg_bytes.len() + 8);
    write_varint(&mut packet, 0x00); // Packet ID for Login Disconnect
    write_varint(&mut packet, msg_bytes.len() as i32); // String length
    packet.extend_from_slice(msg_bytes);

    let mut frame = Vec::with_capacity(packet.len() + 5);
    write_varint(&mut frame, packet.len() as i32); // Frame length
    frame.extend_from_slice(&packet);
    frame
}

/// Creates a complete, framed Minecraft `Status Response` packet (Packet ID 0x00 in status state)
/// carrying the SLP JSON response.
pub fn create_status_response_frame(json_status: &str) -> Vec<u8> {
    create_login_disconnect_frame(json_status)
}

/// Constructs a rich Minecraft chat JSON payload notifying a player that their connection
/// triggered a server wake-from-sleep sequence.
pub fn build_wake_disconnect_json(username: &str, est_seconds: u32) -> String {
    let payload = serde_json::json!({
        "text": "§e⚡ [Zircon] Waking Server From Sleep!\n\n",
        "bold": true,
        "extra": [
            {
                "text": format!("Server boot triggered by {username}.\n"),
                "color": "gray",
                "bold": false
            },
            {
                "text": format!("Booting modded environment (estimated: ~{est_seconds}s)...\n\n"),
                "color": "white",
                "bold": false
            },
            {
                "text": "Please click 'Back to Server List' and rejoin in a few moments.",
                "color": "green",
                "bold": true
            }
        ]
    });

    serde_json::to_string(&payload).unwrap_or_else(|_| {
        format!("§e⚡ [Zircon] Waking Server From Sleep! Rejoin in ~{est_seconds}s.")
    })
}

/// Constructs a rich Minecraft chat JSON payload notifying a player that they are not whitelisted.
pub fn build_whitelist_rejected_json() -> String {
    let payload = serde_json::json!({
        "text": "§c⚡ [Zircon] Access Denied\n\n",
        "bold": true,
        "extra": [
            {
                "text": "You are not on the whitelist for this server.\n\n",
                "color": "red",
                "bold": false
            },
            {
                "text": "Please contact the server administrator to gain access.",
                "color": "gray",
                "bold": false
            }
        ]
    });

    serde_json::to_string(&payload).unwrap_or_else(|_| {
        "§c[Zircon] You are not on the whitelist for this server.".to_string()
    })
}

/// Renders a colorized Unicode progress bar suitable for Minecraft MOTD display.
///
/// Example: `[██████░░░░]` with green filled blocks and gray empty blocks.
pub fn render_ascii_progress_bar(fraction: f32, width: usize) -> String {
    let clamped = fraction.clamp(0.0, 1.0);
    let filled = ((clamped * width as f32).round() as usize).min(width);
    let empty = width.saturating_sub(filled);
    format!("§a{}§7{}", "█".repeat(filled), "░".repeat(empty))
}

/// Constructs a dynamic SLP (Server List Ping) status JSON payload for sleeping or booting servers.
///
/// Features:
/// - Real-time ASCII progress bar in MOTD when booting.
/// - Remaining seconds countdown in version string and MOTD.
/// - Rich player sample tooltips (visible on hover over player count).
/// - Optional base64 PNG favicon.
pub fn build_wake_slp_json(
    protocol: i32,
    server_title: &str,
    status_text: &str,
    is_booting: bool,
    progress_fraction: Option<f32>,
    remaining_secs: Option<u32>,
    favicon_base64: Option<&str>,
) -> String {
    let title = if server_title.trim().is_empty() {
        "Zircon Modded Server"
    } else {
        server_title.trim()
    };

    let (version_name, motd_line1, motd_line2, sample) = if is_booting {
        let line1 = format!("§b✦ {title} §7[§eBooting§7]");
        if let (Some(frac), Some(secs)) = (progress_fraction, remaining_secs) {
            let pct = (frac.clamp(0.0, 1.0) * 100.0).round() as u32;
            let bar = render_ascii_progress_bar(frac, 10);
            let v_name = format!("⏳ Booting {pct}% (~{secs}s)");
            let line2 = format!("{bar} §e{pct}% §7(~{secs}s remaining)");
            let tooltip_sample = vec![
                serde_json::json!({
                    "name": format!("§eStatus: §fBooting (~{secs}s)"),
                    "id": "00000000-0000-0000-0000-000000000000"
                }),
                serde_json::json!({
                    "name": format!("§eProgress: §a{pct}%"),
                    "id": "00000000-0000-0000-0000-000000000001"
                }),
                serde_json::json!({
                    "name": "§7Refresh server list for updates",
                    "id": "00000000-0000-0000-0000-000000000002"
                }),
            ];
            (v_name, line1, line2, tooltip_sample)
        } else {
            let v_name = "⏳ Booting Server...".to_string();
            let line2 = format!("§f{status_text}");
            (v_name, line1, line2, Vec::new())
        }
    } else {
        let v_name = "💤 Sleeping (Click to Wake)".to_string();
        let line1 = format!("§b✦ {title} §7[§eSleeping§7]");
        let line2 = format!("§f{status_text}");
        let tooltip_sample = vec![
            serde_json::json!({
                "name": "§b✦ Zircon Sleeping Server",
                "id": "00000000-0000-0000-0000-000000000000"
            }),
            serde_json::json!({
                "name": "§aConnect to automatically wake!",
                "id": "00000000-0000-0000-0000-000000000001"
            }),
        ];
        (v_name, line1, line2, tooltip_sample)
    };

    let mut payload = serde_json::json!({
        "version": {
            "name": version_name,
            "protocol": protocol
        },
        "players": {
            "max": 20,
            "online": 0,
            "sample": sample
        },
        "description": {
            "text": format!("{motd_line1}\n{motd_line2}")
        }
    });

    if let Some(fav) = favicon_base64 {
        if !fav.trim().is_empty() {
            let uri = if fav.starts_with("data:image/") {
                fav.to_string()
            } else {
                format!("data:image/png;base64,{fav}")
            };
            payload["favicon"] = serde_json::Value::String(uri);
        }
    }

    serde_json::to_string(&payload).unwrap_or_else(|_| {
        r#"{"description":{"text":"§e[Zircon] Server is sleeping... Connect to wake!"}}"#.to_string()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_login_disconnect_frame() {
        let json = r#"{"text":"test"}"#;
        let frame = create_login_disconnect_frame(json);
        assert!(!frame.is_empty());
        assert!(frame.len() > json.len());
        // Frame should contain the payload text
        let s = String::from_utf8_lossy(&frame);
        assert!(s.contains("test"));
    }

    #[test]
    fn test_wake_disconnect_json_contents() {
        let msg = build_wake_disconnect_json("Steve", 15);
        assert!(msg.contains("Waking Server From Sleep"));
        assert!(msg.contains("Steve"));
        assert!(msg.contains("15s"));
    }

    #[test]
    fn test_whitelist_rejected_json_contents() {
        let msg = build_whitelist_rejected_json();
        assert!(msg.contains("Access Denied"));
        assert!(msg.contains("whitelist"));
    }

    #[test]
    fn test_render_ascii_progress_bar() {
        let bar_0 = render_ascii_progress_bar(0.0, 10);
        assert_eq!(bar_0, "§a§7░░░░░░░░░░");

        let bar_50 = render_ascii_progress_bar(0.5, 10);
        assert_eq!(bar_50, "§a█████§7░░░░░");

        let bar_100 = render_ascii_progress_bar(1.0, 10);
        assert_eq!(bar_100, "§a██████████§7");
    }

    #[test]
    fn test_build_wake_slp_json_contents() {
        let slp_sleeping = build_wake_slp_json(765, "My SMP", "Connect to start server!", false, None, None, None);
        assert!(slp_sleeping.contains("Sleeping (Click to Wake)"));
        assert!(slp_sleeping.contains("My SMP"));
        assert!(slp_sleeping.contains("Connect to automatically wake!"));

        let slp_booting = build_wake_slp_json(
            765,
            "My SMP",
            "Starting Fabric...",
            true,
            Some(0.60),
            Some(35),
            Some("fake_favicon_b64"),
        );
        assert!(slp_booting.contains("Booting 60% (~35s)"));
        assert!(slp_booting.contains("35s remaining"));
        assert!(slp_booting.contains("data:image/png;base64,fake_favicon_b64"));
    }
}
