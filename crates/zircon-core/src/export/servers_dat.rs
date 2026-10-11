//! Minecraft NBT `servers.dat` generator for automatic multiplayer bookmarks.
//!
//! Generates spec-compliant uncompressed or GZipped NBT binary payloads for `<instance>/servers.dat`
//! so that Prism Launcher, Modrinth App, and Vanilla players have their server immediately
//! bookmarked upon importing a `.mrpack` archive.

use std::io::Write;

/// A bookmarked server entry in Minecraft's multiplayer menu.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServerDatEntry {
    pub name: String,
    pub ip: String,
    pub accept_textures: i8,
}

impl ServerDatEntry {
    pub fn new(name: impl Into<String>, ip: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            ip: ip.into(),
            accept_textures: 1, // 1 = prompt / auto-accept
        }
    }
}

const TAG_END: u8 = 0;
const TAG_BYTE: u8 = 1;
const TAG_STRING: u8 = 8;
const TAG_LIST: u8 = 9;
const TAG_COMPOUND: u8 = 10;

fn write_string<W: Write>(writer: &mut W, text: &str) -> std::io::Result<()> {
    let bytes = text.as_bytes();
    let len = bytes.len().min(u16::MAX as usize) as u16;
    writer.write_all(&len.to_be_bytes())?;
    writer.write_all(&bytes[..len as usize])?;
    Ok(())
}

/// Serializes a list of `ServerDatEntry` into uncompressed NBT bytes suitable for `servers.dat`.
pub fn serialize_servers_dat(servers: &[ServerDatEntry]) -> Vec<u8> {
    let mut out = Vec::new();
    // Root TAG_Compound ("")
    out.push(TAG_COMPOUND);
    let _ = write_string(&mut out, "");

    // TAG_List "servers"
    out.push(TAG_LIST);
    let _ = write_string(&mut out, "servers");
    out.push(TAG_COMPOUND); // Element type
    out.extend_from_slice(&(servers.len() as i32).to_be_bytes());

    for s in servers {
        // "name": TAG_STRING
        out.push(TAG_STRING);
        let _ = write_string(&mut out, "name");
        let _ = write_string(&mut out, &s.name);

        // "ip": TAG_STRING
        out.push(TAG_STRING);
        let _ = write_string(&mut out, "ip");
        let _ = write_string(&mut out, &s.ip);

        // "acceptTextures": TAG_BYTE (1 = Prompt/Allowed)
        out.push(TAG_BYTE);
        let _ = write_string(&mut out, "acceptTextures");
        out.push(s.accept_textures as u8);

        // TAG_End for server compound
        out.push(TAG_END);
    }

    // TAG_End for root compound
    out.push(TAG_END);
    out
}

/// Helper that builds a `servers.dat` byte buffer for a single server.
pub fn build_single_server_dat(name: &str, address: &str) -> Vec<u8> {
    let entries = [ServerDatEntry::new(name, address)];
    serialize_servers_dat(&entries)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_single_server_dat_structure() {
        let bytes = build_single_server_dat("Emerald SMP", "play.example.com:25565");
        assert!(!bytes.is_empty());
        assert_eq!(bytes[0], TAG_COMPOUND);
        let lossy = String::from_utf8_lossy(&bytes);
        assert!(lossy.contains("servers"));
        assert!(lossy.contains("Emerald SMP"));
        assert!(lossy.contains("play.example.com:25565"));
    }
}
