//! VarInt encoder/decoder for Minecraft protocol bytes.
//!
//! A VarInt is an unsigned base-128 variable-length integer: 7 bits per byte,
//! most significant bit set on every byte except the last, max 5 bytes.

/// Error produced when reading a VarInt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VarIntError {
    /// The buffer ended before the VarInt was complete.
    Incomplete,
    /// The VarInt exceeded the 5-byte protocol limit.
    TooLong,
}

impl std::fmt::Display for VarIntError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            VarIntError::Incomplete => write!(f, "incomplete VarInt: buffer too short"),
            VarIntError::TooLong => write!(f, "malformed VarInt: more than 5 bytes"),
        }
    }
}

impl std::error::Error for VarIntError {}

/// Reads a VarInt from `buf` starting at `offset`.
///
/// Returns `Ok((value, bytes_read))` on success, `Err(VarIntError)` when the
/// buffer is too short (incomplete) or the varint is malformed (>5 bytes).
pub fn read_varint(buf: &[u8], offset: usize) -> Result<(i32, usize), VarIntError> {
    let mut value: i32 = 0;
    let mut bytes = 0;
    loop {
        if offset + bytes >= buf.len() {
            return Err(VarIntError::Incomplete);
        }
        let b = buf[offset + bytes];
        if bytes >= 5 {
            return Err(VarIntError::TooLong);
        }
        value |= i32::from(b & 0x7F) << (7 * bytes);
        bytes += 1;
        if b & 0x80 == 0 {
            break;
        }
    }
    Ok((value, bytes))
}

/// Encodes `value` as a VarInt into `out`. Negative values use the standard
/// two's-complement 5-byte encoding (matching Java's `value >>>= 7`).
pub fn write_varint(out: &mut Vec<u8>, value: i32) {
    let mut v = value as u32;
    loop {
        let b = (v & 0x7F) as u8;
        v >>= 7;
        if v != 0 {
            out.push(b | 0x80);
        } else {
            out.push(b);
            break;
        }
    }
}

/// Size in bytes of the VarInt encoding of `value`.
pub fn varint_len(value: i32) -> usize {
    let mut out = Vec::with_capacity(5);
    write_varint(&mut out, value);
    out.len()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_common_values() {
        for value in [0, 1, 2, 127, 128, 255, 2097151, 2147483647, -1] {
            let mut encoded = Vec::new();
            write_varint(&mut encoded, value);
            let (decoded, _) = read_varint(&encoded, 0).unwrap();
            assert_eq!(value, decoded, "round trip of {value}");
        }
    }

    #[test]
    fn known_encodings() {
        let mut out = Vec::new();
        write_varint(&mut out, 0);
        assert_eq!(vec![0x00], out);

        out.clear();
        write_varint(&mut out, 127);
        assert_eq!(vec![0x7F], out);

        out.clear();
        write_varint(&mut out, 128);
        assert_eq!(vec![0x80, 0x01], out);

        out.clear();
        write_varint(&mut out, 255);
        assert_eq!(vec![0xFF, 0x01], out);
    }

    #[test]
    fn incomplete_and_malformed_are_rejected() {
        assert!(read_varint(&[0x80], 0).is_err()); // needs a continuation byte
        assert!(read_varint(&[], 0).is_err());
        // 6 continuation bytes → malformed.
        let malformed = [0x80u8, 0x80, 0x80, 0x80, 0x80, 0x01];
        assert!(read_varint(&malformed, 0).is_err());
    }

    #[test]
    fn reads_from_offset() {
        let buf = [0x01u8, 0x7F];
        let (value, bytes) = read_varint(&buf, 1).unwrap();
        assert_eq!(127, value);
        assert_eq!(1, bytes);
    }
}
