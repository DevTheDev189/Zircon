//! 32-bit MurmurHash3 implementation matching CurseForge's file fingerprint
//! algorithm.
//!
//! CurseForge computes fingerprints by first stripping four ASCII whitespace
//! bytes (Tab `0x09`, LF `0x0A`, CR `0x0D`, Space `0x20`) from the file bytes,
//! then running standard MurmurHash3 (x86, 32-bit) with seed `1`. The result
//! is returned as an unsigned value masked to 32 bits.
//!
//! Port of `com.mcmanager.core.crypto.MurmurHash3` (verified byte-for-byte
//! against the official MurmurHash3 x86-32 test vectors).

/// Seed used by CurseForge for file fingerprints.
pub const CURSEFORGE_SEED: u32 = 1;

/// Whitespace byte values ignored by CurseForge before hashing.
const TAB: u8 = 0x09;
const LF: u8 = 0x0A;
const CR: u8 = 0x0D;
const SPACE: u8 = 0x20;

/// CurseForge fingerprint of a byte slice: strips the four whitespace byte
/// values, then runs 32-bit MurmurHash3 with seed 1.
///
/// Returns an unsigned value in the range `[0, 0xFFFF_FFFF]`.
pub fn curse_forge_fingerprint(data: &[u8]) -> u64 {
    let stripped = strip_whitespace(data);
    murmur3_x86_32(&stripped, CURSEFORGE_SEED)
}

/// Convenience overload that reads a file from disk and computes its
/// CurseForge fingerprint.
pub fn curse_forge_fingerprint_of_file(path: &std::path::Path) -> std::io::Result<u64> {
    let data = std::fs::read(path)?;
    Ok(curse_forge_fingerprint(&data))
}

/// Standard 32-bit MurmurHash3 (x86 variant) over the given data.
///
/// Returns an unsigned value in the range `[0, 0xFFFF_FFFF]`.
pub fn murmur3_x86_32(data: &[u8], seed: u32) -> u64 {
    const C1: u32 = 0xcc9e_2d51;
    const C2: u32 = 0x1b87_3593;

    let length = data.len() as u32;
    let mut h1 = seed;
    let rounded_end = (length & 0xffff_fffc) as usize;

    // Process 4-byte blocks.
    for chunk in data[..rounded_end].chunks_exact(4) {
        let mut k1 = u32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]);
        k1 = k1.wrapping_mul(C1);
        k1 = k1.rotate_left(15);
        k1 = k1.wrapping_mul(C2);

        h1 ^= k1;
        h1 = h1.rotate_left(13);
        h1 = h1.wrapping_mul(5).wrapping_add(0xe654_6b64);
    }

    // Tail bytes (Java's fall-through switch on length & 3).
    let tail = (length & 3) as usize;
    let mut k1 = 0u32;
    if tail >= 3 {
        k1 ^= (data[rounded_end + 2] as u32) << 16;
    }
    if tail >= 2 {
        k1 ^= (data[rounded_end + 1] as u32) << 8;
    }
    if tail >= 1 {
        k1 ^= data[rounded_end] as u32;
        k1 = k1.wrapping_mul(C1);
        k1 = k1.rotate_left(15);
        k1 = k1.wrapping_mul(C2);
        h1 ^= k1;
    }

    h1 ^= length;
    h1 = fmix32(h1);

    (h1 as u64) & 0xFFFF_FFFF
}

fn fmix32(mut h: u32) -> u32 {
    h ^= h >> 16;
    h = h.wrapping_mul(0x85eb_ca6b);
    h ^= h >> 13;
    h = h.wrapping_mul(0xc2b2_ae35);
    h ^= h >> 16;
    h
}

fn strip_whitespace(data: &[u8]) -> Vec<u8> {
    let count = data.iter().filter(|&&b| !is_stripped_byte(b)).count();
    if count == data.len() {
        return data.to_vec();
    }
    let mut out = Vec::with_capacity(count);
    for &b in data {
        if !is_stripped_byte(b) {
            out.push(b);
        }
    }
    out
}

#[inline]
fn is_stripped_byte(b: u8) -> bool {
    b == TAB || b == LF || b == CR || b == SPACE
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Official MurmurHash3 x86-32 test vectors (seed 0).
    #[test]
    fn known_vectors_seed_zero() {
        assert_hash("", 0, 0x0000_0000);
        assert_hash("hello", 0, 0x248b_fa47);
        assert_hash("hello, world", 0, 0x149b_bb7f);
        assert_hash(
            "The quick brown fox jumps over the lazy dog",
            0,
            0x2e4f_f723,
        );
    }

    fn assert_hash(input: &str, seed: u32, expected: u64) {
        assert_eq!(
            expected,
            murmur3_x86_32(input.as_bytes(), seed),
            "murmur3(\"{}\", seed {})",
            input,
            seed
        );
    }

    #[test]
    fn curse_forge_fingerprint_strips_whitespace() {
        let body = "PK\u{3}\u{4} some zip content \r\nwith\twhitespace and  spaces ";
        let raw = body.as_bytes();
        let stripped: String = body
            .chars()
            .filter(|c| *c != '\r' && *c != '\n' && *c != '\t' && *c != ' ')
            .collect();

        assert_eq!(
            murmur3_x86_32(stripped.as_bytes(), CURSEFORGE_SEED),
            curse_forge_fingerprint(raw),
            "fingerprint must ignore 0x09/0x0A/0x0D/0x20 bytes"
        );
    }

    #[test]
    fn curse_forge_fingerprint_of_whitespace_only_equals_empty() {
        let whitespace = [b' ', b'\t', b'\r', b'\n', b' ', b'\t'];
        assert_eq!(
            murmur3_x86_32(&[], CURSEFORGE_SEED),
            curse_forge_fingerprint(&whitespace)
        );
    }

    #[test]
    fn fingerprint_is_unsigned() {
        let fp = curse_forge_fingerprint(b"arbitrary content that should produce a high bit");
        assert_eq!(fp, fp & 0xFFFF_FFFF);
        assert!(fp <= u64::from(u32::MAX));
    }

    #[test]
    fn matches_java_reference_seed_one() {
        // Reference values generated by running the original Java
        // implementation (shared-core MurmurHash3, seed 1).
        assert_eq!(0x514e_28b7, murmur3_x86_32(b"", 1));
        assert_eq!(0x588a_dce8, murmur3_x86_32(b"a", 1));
        assert_eq!(0xaa75_e9ff, murmur3_x86_32(b"abc", 1));
        assert_eq!(0xbb4a_bcad, murmur3_x86_32(b"hello", 1));
        assert_eq!(0x6f5c_b2e9, murmur3_x86_32(b"hello, world", 1));
        assert_eq!(
            0x78e6_9e27,
            murmur3_x86_32(b"The quick brown fox jumps over the lazy dog", 1)
        );
    }

    #[test]
    fn curse_forge_fingerprint_matches_java_reference() {
        // Fingerprint (whitespace-stripped, seed 1) generated by the Java
        // implementation for the same byte input.
        let body = "PK\u{3}\u{4} some zip content \r\nwith\twhitespace";
        assert_eq!(2_252_075_348u64, curse_forge_fingerprint(body.as_bytes()));
    }
}
