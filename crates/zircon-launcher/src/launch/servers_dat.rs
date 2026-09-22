//! Automatic pre-population and management of Minecraft's `<gameDir>/servers.dat`.
//!
//! Minecraft stores bookmarked multiplayer servers in NBT format inside `servers.dat`.
//! This module safely ensures the current Zircon server is pre-populated in `servers.dat`
//! so that if the player disconnects from `--quickPlayMultiplayer`, the server is readily
//! available in their multiplayer menu with zero manual IP typing.
//!
//! Preserves any existing servers already bookmarked by the player.

use std::fs::File;
use std::io::{Cursor, Read, Write};
use std::path::Path;
use flate2::read::GzDecoder;

/// A bookmarked server in Minecraft's multiplayer menu.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServerDatEntry {
    pub name: String,
    pub ip: String,
    pub accept_textures: i8,
}

/// NBT tag identifiers
const TAG_END: u8 = 0;
const TAG_BYTE: u8 = 1;
const TAG_SHORT: u8 = 2;
const TAG_INT: u8 = 3;
const TAG_LONG: u8 = 4;
const TAG_FLOAT: u8 = 5;
const TAG_DOUBLE: u8 = 6;
const TAG_BYTE_ARRAY: u8 = 7;
const TAG_STRING: u8 = 8;
const TAG_LIST: u8 = 9;
const TAG_COMPOUND: u8 = 10;
const TAG_INT_ARRAY: u8 = 11;
const TAG_LONG_ARRAY: u8 = 12;

/// Reads raw bytes from `servers.dat`, handling optional GZip compression.
fn read_servers_dat_bytes(file_path: &Path) -> std::io::Result<Vec<u8>> {
    let mut file = File::open(file_path)?;
    let mut header = [0u8; 2];
    let n = file.read(&mut header)?;
    if n == 2 && header[0] == 0x1f && header[1] == 0x8b {
        // GZip compressed
        let gz_file = File::open(file_path)?;
        let mut decoder = GzDecoder::new(gz_file);
        let mut decompressed = Vec::new();
        decoder.read_to_end(&mut decompressed)?;
        Ok(decompressed)
    } else {
        // Raw uncompressed NBT
        let mut raw = Vec::new();
        File::open(file_path)?.read_to_end(&mut raw)?;
        Ok(raw)
    }
}

/// Reads a UTF-8 string with a 2-byte big-endian length prefix.
fn read_string<R: Read>(reader: &mut R) -> std::io::Result<String> {
    let mut len_bytes = [0u8; 2];
    reader.read_exact(&mut len_bytes)?;
    let len = u16::from_be_bytes(len_bytes) as usize;
    let mut buf = vec![0u8; len];
    reader.read_exact(&mut buf)?;
    Ok(String::from_utf8_lossy(&buf).to_string())
}

/// Writes a UTF-8 string with a 2-byte big-endian length prefix.
fn write_string<W: Write>(writer: &mut W, text: &str) -> std::io::Result<()> {
    let bytes = text.as_bytes();
    let len = bytes.len().min(u16::MAX as usize) as u16;
    writer.write_all(&len.to_be_bytes())?;
    writer.write_all(&bytes[..len as usize])?;
    Ok(())
}

/// Skips an unnamed NBT payload of given tag type.
fn skip_nbt_payload<R: Read>(reader: &mut R, tag_type: u8) -> std::io::Result<()> {
    match tag_type {
        TAG_BYTE => {
            let mut b = [0u8; 1];
            reader.read_exact(&mut b)?;
        }
        TAG_SHORT => {
            let mut b = [0u8; 2];
            reader.read_exact(&mut b)?;
        }
        TAG_INT | TAG_FLOAT => {
            let mut b = [0u8; 4];
            reader.read_exact(&mut b)?;
        }
        TAG_LONG | TAG_DOUBLE => {
            let mut b = [0u8; 8];
            reader.read_exact(&mut b)?;
        }
        TAG_BYTE_ARRAY => {
            let mut len_b = [0u8; 4];
            reader.read_exact(&mut len_b)?;
            let len = i32::from_be_bytes(len_b);
            if len > 0 {
                let mut buf = vec![0u8; len as usize];
                reader.read_exact(&mut buf)?;
            }
        }
        TAG_STRING => {
            let _ = read_string(reader)?;
        }
        TAG_LIST => {
            let mut elem_type = [0u8; 1];
            reader.read_exact(&mut elem_type)?;
            let mut len_b = [0u8; 4];
            reader.read_exact(&mut len_b)?;
            let len = i32::from_be_bytes(len_b);
            for _ in 0..len {
                skip_nbt_payload(reader, elem_type[0])?;
            }
        }
        TAG_COMPOUND => {
            loop {
                let mut child_tag = [0u8; 1];
                reader.read_exact(&mut child_tag)?;
                if child_tag[0] == TAG_END {
                    break;
                }
                let _name = read_string(reader)?;
                skip_nbt_payload(reader, child_tag[0])?;
            }
        }
        TAG_INT_ARRAY => {
            let mut len_b = [0u8; 4];
            reader.read_exact(&mut len_b)?;
            let len = i32::from_be_bytes(len_b);
            for _ in 0..len {
                let mut b = [0u8; 4];
                reader.read_exact(&mut b)?;
            }
        }
        TAG_LONG_ARRAY => {
            let mut len_b = [0u8; 4];
            reader.read_exact(&mut len_b)?;
            let len = i32::from_be_bytes(len_b);
            for _ in 0..len {
                let mut b = [0u8; 8];
                reader.read_exact(&mut b)?;
            }
        }
        _ => {}
    }
    Ok(())
}

/// Parses an existing `servers.dat` buffer into a list of `ServerDatEntry`.
pub fn parse_servers_dat(data: &[u8]) -> std::io::Result<Vec<ServerDatEntry>> {
    let mut cursor = Cursor::new(data);
    let mut root_type = [0u8; 1];
    cursor.read_exact(&mut root_type)?;
    if root_type[0] != TAG_COMPOUND {
        return Ok(Vec::new());
    }
    // Root name
    let _ = read_string(&mut cursor)?;

    let mut servers = Vec::new();

    loop {
        let mut tag_type = [0u8; 1];
        if cursor.read_exact(&mut tag_type).is_err() || tag_type[0] == TAG_END {
            break;
        }
        let tag_name = read_string(&mut cursor)?;
        if tag_name == "servers" && tag_type[0] == TAG_LIST {
            let mut elem_type = [0u8; 1];
            cursor.read_exact(&mut elem_type)?;
            let mut len_b = [0u8; 4];
            cursor.read_exact(&mut len_b)?;
            let count = i32::from_be_bytes(len_b);

            if elem_type[0] == TAG_COMPOUND && count > 0 {
                for _ in 0..count {
                    let mut s_name = String::new();
                    let mut s_ip = String::new();
                    let mut s_accept_textures: i8 = 1;

                    loop {
                        let mut field_type = [0u8; 1];
                        cursor.read_exact(&mut field_type)?;
                        if field_type[0] == TAG_END {
                            break;
                        }
                        let field_name = read_string(&mut cursor)?;
                        match (field_name.as_str(), field_type[0]) {
                            ("name", TAG_STRING) => {
                                s_name = read_string(&mut cursor)?;
                            }
                            ("ip", TAG_STRING) => {
                                s_ip = read_string(&mut cursor)?;
                            }
                            ("acceptTextures", TAG_BYTE) => {
                                let mut b = [0u8; 1];
                                cursor.read_exact(&mut b)?;
                                s_accept_textures = b[0] as i8;
                            }
                            _ => {
                                skip_nbt_payload(&mut cursor, field_type[0])?;
                            }
                        }
                    }

                    if !s_ip.trim().is_empty() {
                        servers.push(ServerDatEntry {
                            name: s_name,
                            ip: s_ip,
                            accept_textures: s_accept_textures,
                        });
                    }
                }
            }
        } else {
            skip_nbt_payload(&mut cursor, tag_type[0])?;
        }
    }

    Ok(servers)
}

/// Serializes a list of `ServerDatEntry` to uncompressed NBT bytes suitable for `servers.dat`.
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

/// Ensures that the specified server is present in `<gameDir>/servers.dat`.
///
/// If `servers.dat` does not exist, it is created.
/// If it exists, existing servers are preserved. If the server is not present,
/// it is prepended to the top of the list so it appears first in the multiplayer menu.
pub fn ensure_server_entry(
    game_dir: &Path,
    server_name: &str,
    server_address: &str,
) -> std::io::Result<()> {
    let target_ip = server_address.trim();
    if target_ip.is_empty() {
        return Ok(());
    }

    let servers_dat_path = game_dir.join("servers.dat");
    let mut servers = if servers_dat_path.is_file() {
        match read_servers_dat_bytes(&servers_dat_path) {
            Ok(bytes) => parse_servers_dat(&bytes).unwrap_or_default(),
            Err(_) => Vec::new(),
        }
    } else {
        Vec::new()
    };

    // Normalize ip comparison: check if target_ip matches existing
    let already_present = servers.iter().any(|s| {
        s.ip.trim().eq_ignore_ascii_case(target_ip)
            || (target_ip.ends_with(":25565") && s.ip.trim().eq_ignore_ascii_case(&target_ip[..target_ip.len() - 6]))
            || (!target_ip.contains(':') && s.ip.trim().eq_ignore_ascii_case(&format!("{target_ip}:25565")))
    });

    if already_present {
        tracing::debug!("Server '{target_ip}' is already bookmarked in servers.dat");
        return Ok(());
    }

    // Prepend to top of the list
    servers.insert(
        0,
        ServerDatEntry {
            name: server_name.trim().to_string(),
            ip: target_ip.to_string(),
            accept_textures: 1,
        },
    );

    let bytes = serialize_servers_dat(&servers);
    if let Some(parent) = servers_dat_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let tmp_path = game_dir.join(format!("servers.dat.tmp.{}", std::process::id()));
    std::fs::write(&tmp_path, bytes)?;
    std::fs::rename(&tmp_path, &servers_dat_path)?;
    tracing::info!("Pre-populated '{server_name}' ({target_ip}) in servers.dat");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_serialize_and_parse_servers_dat() {
        let original = vec![
            ServerDatEntry {
                name: "Zircon SMP".to_string(),
                ip: "play.zircon.mc:25565".to_string(),
                accept_textures: 1,
            },
            ServerDatEntry {
                name: "Hypixel".to_string(),
                ip: "mc.hypixel.net".to_string(),
                accept_textures: 1,
            },
        ];

        let bytes = serialize_servers_dat(&original);
        let parsed = parse_servers_dat(&bytes).expect("Failed to parse serialized NBT");

        assert_eq!(parsed.len(), 2);
        assert_eq!(parsed[0].name, "Zircon SMP");
        assert_eq!(parsed[0].ip, "play.zircon.mc:25565");
        assert_eq!(parsed[1].name, "Hypixel");
        assert_eq!(parsed[1].ip, "mc.hypixel.net");
    }

    #[test]
    fn test_ensure_server_entry_idempotent() {
        let temp_dir = std::env::temp_dir().join(format!("zircon_servers_dat_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&temp_dir).unwrap();

        // 1. Initial creation
        ensure_server_entry(&temp_dir, "My Server", "127.0.0.1:25565").unwrap();
        let bytes = read_servers_dat_bytes(&temp_dir.join("servers.dat")).unwrap();
        let parsed = parse_servers_dat(&bytes).unwrap();
        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed[0].name, "My Server");
        assert_eq!(parsed[0].ip, "127.0.0.1:25565");

        // 2. Second call with same IP (different formatting/default port)
        ensure_server_entry(&temp_dir, "My Server Updated Name", "127.0.0.1").unwrap();
        let bytes2 = read_servers_dat_bytes(&temp_dir.join("servers.dat")).unwrap();
        let parsed2 = parse_servers_dat(&bytes2).unwrap();
        assert_eq!(parsed2.len(), 1); // Not duplicated!

        // 3. Add a distinct second server
        ensure_server_entry(&temp_dir, "Friends Server", "192.168.1.100:25565").unwrap();
        let bytes3 = read_servers_dat_bytes(&temp_dir.join("servers.dat")).unwrap();
        let parsed3 = parse_servers_dat(&bytes3).unwrap();
        assert_eq!(parsed3.len(), 2);
        assert_eq!(parsed3[0].name, "Friends Server"); // Prepended to top
        assert_eq!(parsed3[1].name, "My Server");

        let _ = std::fs::remove_dir_all(&temp_dir);
    }
}
