//! Streaming SHA-1 / SHA-256 helpers. Files (potentially large mod JARs) are
//! hashed through an 8 KiB buffer so memory usage stays flat regardless of
//! file size.
//!
//! Port of `com.mcmanager.core.crypto.HashUtil`.

use std::io;
use std::path::Path;

use sha1::Digest;
use tokio::io::AsyncReadExt;

/// Read buffer size (8 KiB), matching the Java implementation.
pub const BUFFER_SIZE: usize = 8192;

/// Computes the lower-case hex SHA-1 of a file.
pub async fn sha1_file(path: &Path) -> io::Result<String> {
    let mut hasher = sha1::Sha1::new();
    hash_file_streaming(path, |chunk| hasher.update(chunk)).await?;
    Ok(hex::encode(hasher.finalize()))
}

/// Computes the lower-case hex SHA-256 of a file.
pub async fn sha256_file(path: &Path) -> io::Result<String> {
    let mut hasher = sha2::Sha256::new();
    hash_file_streaming(path, |chunk| hasher.update(chunk)).await?;
    Ok(hex::encode(hasher.finalize()))
}

/// Streams a file through an 8 KiB buffer, feeding every chunk to `update`.
async fn hash_file_streaming<F>(path: &Path, mut update: F) -> io::Result<()>
where
    F: FnMut(&[u8]),
{
    let mut file = tokio::fs::File::open(path).await?;
    let mut buffer = [0u8; BUFFER_SIZE];
    loop {
        let n = file.read(&mut buffer).await?;
        if n == 0 {
            break;
        }
        update(&buffer[..n]);
    }
    Ok(())
}

/// Lower-case hex encoding of a byte slice.
pub fn to_hex(bytes: &[u8]) -> String {
    hex::encode(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write as _;

    fn known_sha1(data: &[u8]) -> String {
        hex::encode(sha1::Sha1::digest(data))
    }

    fn known_sha256(data: &[u8]) -> String {
        hex::encode(sha2::Sha256::digest(data))
    }

    #[tokio::test]
    async fn sha1_matches_known_digest() {
        // Unique temp dir per test: the tests run in parallel and share a
        // process id, so a shared dir name would race on cleanup.
        let dir = std::env::temp_dir().join(format!("zircon-hash-sha1-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("data.bin");
        let content: Vec<u8> = (0..20_000u32).map(|i| (i * 31) as u8).collect(); // larger than 8192 buffer
        let mut f = std::fs::File::create(&file).unwrap();
        f.write_all(&content).unwrap();
        drop(f);

        assert_eq!(known_sha1(&content), sha1_file(&file).await.unwrap());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn sha256_matches_known_digest() {
        let dir = std::env::temp_dir().join(format!("zircon-hash-sha256-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("data2.bin");
        std::fs::write(&file, b"hello world").unwrap();

        assert_eq!(
            known_sha256(b"hello world"),
            sha256_file(&file).await.unwrap()
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn hashes_empty_file() {
        let dir = std::env::temp_dir().join(format!("zircon-hash-empty-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("empty.bin");
        std::fs::write(&file, []).unwrap();

        assert_eq!(known_sha1(b""), sha1_file(&file).await.unwrap());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
