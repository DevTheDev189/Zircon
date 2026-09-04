//! Inspects the first bytes of an incoming connection on the public port and
//! decides where to proxy it:
//!
//! * **HTTP** (GET/POST/HEAD/PUT/DELETE/OPTIONS/PATCH, each with a trailing
//!   space to avoid false positives on the Minecraft protocol) → the admin web
//!   server on the internal web port.
//! * **Minecraft handshake** → the internal MC port of the instance whose
//!   id/name matches the handshake hostname, or the legacy default MC port.
//!
//! Login-state connections must present a one-time join ticket registered by
//! the Zircon launcher (see `crate::tickets`); without one the socket is
//! disconnected before reaching the game server. Server-list status pings and
//! legacy single-server mode are not gated.
//!
//! Port of `com.mcmanager.server.multiplexer.ProtocolDetector` (pure parsing
//! portion; the async connection loop lives in `multiplexer.rs`).

/// Require trailing space so e.g. "GET " never collides with MC binary packets.
const HTTP_PREFIXES: [&[u8]; 7] = [
    b"GET ",
    b"POST ",
    b"HEAD ",
    b"PUT ",
    b"DELETE ",
    b"OPTIONS ",
    b"PATCH ",
];

/// A parsed server-list-ping handshake packet.
#[derive(Debug, Clone, PartialEq)]
pub struct Handshake {
    pub hostname: String,
    /// Next protocol state: 1 = status ping, 2 = login.
    pub next_state: i32,
}

/// Result of a speculative protocol parse.
#[derive(Debug, Clone, PartialEq)]
pub enum ParseResult<T> {
    /// More bytes are needed before a decision can be made.
    Incomplete,
    /// The bytes can't be what we were looking for.
    NotMatch,
    /// A successful parse.
    Matched(T),
}

/// Returns `true` when the buffered bytes start with an HTTP method prefix.
pub fn is_http_method(buf: &[u8]) -> bool {
    if buf.len() < 5 {
        return false;
    }
    HTTP_PREFIXES.iter().any(|prefix| buf.starts_with(prefix))
}

/// Parses the server-list-ping handshake packet:
/// `[VarInt length][VarInt 0x00][VarInt protocol][VarInt addrLen][addr bytes][u16 port][VarInt nextState]`.
pub fn parse_handshake(buf: &[u8]) -> ParseResult<Handshake> {
    let Some((length, mut offset)) = varint_at(buf, 0) else {
        return ParseResult::Incomplete;
    };
    let Some((packet_id, bytes)) = varint_at(buf, offset) else {
        return ParseResult::Incomplete;
    };
    if packet_id != 0 {
        return ParseResult::NotMatch;
    }
    offset += bytes;

    let Some((_, bytes)) = varint_at(buf, offset) else {
        return ParseResult::Incomplete;
    };
    offset += bytes;

    let Some((addr_len, bytes)) = varint_at(buf, offset) else {
        return ParseResult::Incomplete;
    };
    if !(1..=255).contains(&addr_len) {
        return ParseResult::NotMatch;
    }
    offset += bytes;

    if buf.len() < offset + addr_len as usize {
        return ParseResult::Incomplete;
    }
    let hostname = String::from_utf8_lossy(&buf[offset..offset + addr_len as usize])
        .trim()
        .to_lowercase();
    offset += addr_len as usize;

    // u16 port
    if buf.len() < offset + 2 {
        return ParseResult::Incomplete;
    }
    offset += 2;

    let Some((next_state, _)) = varint_at(buf, offset) else {
        return ParseResult::Incomplete;
    };

    if hostname.is_empty() {
        return ParseResult::NotMatch;
    }
    let _ = length;
    ParseResult::Matched(Handshake {
        hostname,
        next_state,
    })
}

/// Parses the Login Start packet that follows a login-state handshake:
/// `[VarInt len][VarInt 0x00][VarInt nameLen][name bytes]` (the optional
/// trailing UUID, 1.19+, is intentionally ignored).
pub fn parse_login_start_username(buf: &[u8]) -> ParseResult<String> {
    // Skip the handshake frame: [VarInt length][length bytes].
    let Some((handshake_len, mut offset)) = varint_at(buf, 0) else {
        return ParseResult::Incomplete;
    };
    if handshake_len < 0 || buf.len() < offset + handshake_len as usize {
        return ParseResult::Incomplete;
    }
    offset += handshake_len as usize;

    // Login Start frame: [VarInt length][VarInt 0x00][VarInt nameLen][name bytes].
    let Some((packet_len, bytes)) = varint_at(buf, offset) else {
        return ParseResult::Incomplete;
    };
    offset += bytes;
    if packet_len < 0 || buf.len() < offset + packet_len as usize {
        return ParseResult::Incomplete;
    }
    let packet_end = offset + packet_len as usize;

    let Some((packet_id, bytes)) = varint_at_in(buf, offset, packet_end) else {
        return ParseResult::Incomplete;
    };
    offset += bytes;
    if packet_id != 0 {
        return ParseResult::NotMatch;
    }

    let Some((name_len, bytes)) = varint_at_in(buf, offset, packet_end) else {
        return ParseResult::Incomplete;
    };
    offset += bytes;
    if !(1..=16).contains(&name_len) || packet_end < offset + name_len as usize {
        return ParseResult::Incomplete;
    }

    let username = String::from_utf8_lossy(&buf[offset..offset + name_len as usize])
        .trim()
        .to_string();
    if username.is_empty() {
        return ParseResult::NotMatch;
    }
    ParseResult::Matched(username)
}

/// Reads a VarInt at `offset`, returning `(value, bytes_read)`.
fn varint_at(buf: &[u8], offset: usize) -> Option<(i32, usize)> {
    varint_at_in(buf, offset, buf.len())
}

/// Reads a VarInt at `offset` bounded by `limit`.
fn varint_at_in(buf: &[u8], offset: usize, limit: usize) -> Option<(i32, usize)> {
    use crate::multiplexer::varint::read_varint;
    let buf = &buf[..limit.min(buf.len())];
    if offset > buf.len() {
        return None;
    }
    read_varint(buf, offset).ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::multiplexer::varint::write_varint;

    fn handshake_frame(hostname: &str, next_state: i32) -> Vec<u8> {
        let mut payload = Vec::new();
        write_varint(&mut payload, 0); // packet id 0x00 (handshake)
        write_varint(&mut payload, 754); // protocol version
        write_varint(&mut payload, hostname.len() as i32);
        payload.extend_from_slice(hostname.as_bytes());
        payload.extend_from_slice(&25565u16.to_be_bytes());
        write_varint(&mut payload, next_state);

        let mut out = Vec::new();
        write_varint(&mut out, payload.len() as i32);
        out.extend_from_slice(&payload);
        out
    }

    fn login_start_frame(username: &str) -> Vec<u8> {
        let mut packet = Vec::new();
        write_varint(&mut packet, 0); // packet id
        write_varint(&mut packet, username.len() as i32);
        packet.extend_from_slice(username.as_bytes());
        let mut out = Vec::new();
        write_varint(&mut out, packet.len() as i32);
        out.extend_from_slice(&packet);
        out
    }

    #[test]
    fn http_detection_needs_five_bytes() {
        assert!(!is_http_method(b"GET"));
        assert!(!is_http_method(b"GET ")); // exactly 4 bytes: too short
        assert!(is_http_method(b"GET /index.html HTTP/1.1"));
        assert!(is_http_method(b"POST /api HTTP/1.1"));
        assert!(is_http_method(b"OPTIONS * HTTP/1.1"));
        assert!(is_http_method(b"PATCH /api/instances/1 HTTP/1.1"));
        assert!(!is_http_method(b"\x10\x00\x09localhost"));
    }

    #[test]
    fn parses_handshake_hostname_and_state() {
        let frame = handshake_frame("my-server", 2);
        match parse_handshake(&frame) {
            ParseResult::Matched(handshake) => {
                assert_eq!("my-server", handshake.hostname);
                assert_eq!(2, handshake.next_state);
            }
            other => panic!("expected match, got {other:?}"),
        }
    }

    #[test]
    fn incomplete_handshake_waits_for_more_bytes() {
        let frame = handshake_frame("my-server", 2);
        assert_eq!(ParseResult::Incomplete, parse_handshake(&frame[..1]));
        assert_eq!(
            ParseResult::Incomplete,
            parse_handshake(&frame[..frame.len() - 1])
        );
    }

    #[test]
    fn random_binary_is_not_a_handshake() {
        let junk = [0x01u8, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08];
        assert!(matches!(
            parse_handshake(&junk),
            ParseResult::NotMatch | ParseResult::Incomplete
        ));
    }

    #[test]
    fn login_start_parses_username() {
        let mut buf = handshake_frame("my-server", 2);
        buf.extend_from_slice(&login_start_frame("Steve"));
        match parse_login_start_username(&buf) {
            ParseResult::Matched(username) => assert_eq!("Steve", username),
            other => panic!("expected match, got {other:?}"),
        }
    }

    #[test]
    fn login_start_incomplete_without_full_frame() {
        let mut buf = handshake_frame("my-server", 2);
        let login = login_start_frame("Steve");
        buf.extend_from_slice(&login[..login.len() - 1]);
        assert_eq!(ParseResult::Incomplete, parse_login_start_username(&buf));
    }
}
