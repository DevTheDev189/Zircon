//! Issues and validates admin JWTs (HMAC SHA-256, 12h TTL).
//!
//! The signing secret is generated once, persisted to `jwt-secret.key` in the
//! data dir, and reused across restarts so tokens stay valid.
//!
//! Every token carries a random `jti` (JWT ID) so a session can be revoked
//! server-side on sign-out or password change (see `crate::auth::revocation`).
//!
//! Port of `com.mcmanager.server.auth.JwtUtil`.

use std::fs;
use std::path::Path;
use std::sync::OnceLock;
use std::time::{SystemTime, UNIX_EPOCH};

use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};

const TTL_SECONDS: i64 = 12 * 60 * 60;

/// Claims carried by an admin JWT.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Claims {
    pub sub: String,
    /// Random per-token ID so sessions can be individually revoked. `default`
    /// keeps tokens issued before this field existed valid across upgrades.
    #[serde(default)]
    pub jti: String,
    pub iat: i64,
    pub exp: i64,
}

/// Loads (or creates) the persistent signing secret. Call once at startup.
pub fn initialize(data_dir: &Path) -> std::io::Result<()> {
    fs::create_dir_all(data_dir)?;
    let secret_file = data_dir.join("jwt-secret.key");
    let secret_bytes: Vec<u8> = if secret_file.is_file() {
        let content = fs::read_to_string(&secret_file)?;
        let decoded = base64_decode(content.trim());
        if decoded.is_empty() {
            return Err(std::io::Error::other(
                "jwt-secret.key is empty or invalid base64",
            ));
        }
        decoded
    } else {
        let secret = generate_secret();
        fs::write(&secret_file, base64_encode(&secret))?;
        tracing::info!(
            "Generated new JWT signing secret at {}",
            secret_file.display()
        );
        secret
    };
    let _ = SECRET.set(secret_bytes);
    Ok(())
}

static SECRET: OnceLock<Vec<u8>> = OnceLock::new();

fn secret() -> &'static [u8] {
    SECRET.get_or_init(|| {
        panic!("JwtUtil::initialize(Path) must be called before issuing tokens");
    })
}

/// Issues a new token for `username`, valid for 12 hours.
pub fn generate_token(username: &str) -> String {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;
    let claims = Claims {
        sub: username.to_string(),
        jti: uuid::Uuid::new_v4().to_string(),
        iat: now,
        exp: now + TTL_SECONDS,
    };
    encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(secret()),
    )
    .expect("failed to sign JWT")
}

/// Decodes and verifies a token, returning its claims (or `None` if
/// invalid/expired). The caller is responsible for checking revocation.
pub fn decode_claims(token: &str) -> Option<Claims> {
    decode::<Claims>(
        token,
        &DecodingKey::from_secret(secret()),
        &Validation::default(),
    )
    .ok()
    .map(|data| data.claims)
}

/// Returns the token subject (username), or `None` if the token is
/// invalid/expired.
pub fn validate_token(token: &str) -> Option<String> {
    decode_claims(token).map(|claims| claims.sub)
}

fn generate_secret() -> Vec<u8> {
    // 32 random bytes from the OS (same helper as AuthService).
    let mut bytes = vec![0u8; 32];
    fill_random(&mut bytes);
    bytes
}

fn fill_random(bytes: &mut [u8]) {
    getrandom::fill(bytes).expect("failed to read OS entropy");
}

fn base64_encode(bytes: &[u8]) -> String {
    const TABLE: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let b = [
            chunk[0],
            chunk.get(1).copied().unwrap_or(0),
            chunk.get(2).copied().unwrap_or(0),
        ];
        let n = ((b[0] as u32) << 16) | ((b[1] as u32) << 8) | b[2] as u32;
        out.push(TABLE[(n >> 18) as usize & 63] as char);
        out.push(TABLE[(n >> 12) as usize & 63] as char);
        out.push(if chunk.len() > 1 {
            TABLE[(n >> 6) as usize & 63] as char
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            TABLE[n as usize & 63] as char
        } else {
            '='
        });
    }
    out
}

fn base64_decode(input: &str) -> Vec<u8> {
    let mut out = Vec::with_capacity(input.len() / 4 * 3);
    let mut buf = 0u32;
    let mut bits = 0u32;
    for c in input.bytes().filter(|b| !b.is_ascii_whitespace()) {
        let v = match c {
            b'A'..=b'Z' => c - b'A',
            b'a'..=b'z' => c - b'a' + 26,
            b'0'..=b'9' => c - b'0' + 52,
            b'+' => 62,
            b'/' => 63,
            b'=' => break,
            _ => return Vec::new(),
        };
        buf = (buf << 6) | u32::from(v);
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((buf >> bits) as u8);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64_round_trip() {
        let data = b"hello world\x00\x01\x02";
        let encoded = base64_encode(data);
        assert_eq!(data.to_vec(), base64_decode(&encoded));
    }

    #[test]
    fn token_round_trip() {
        let dir = crate::test_util::temp_dir("jwt");
        initialize(&dir).unwrap();

        let token = generate_token("admin");
        assert_eq!(Some("admin".to_string()), validate_token(&token));
        assert_eq!(None, validate_token("garbage"));
        assert_eq!(None, validate_token(""));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn secret_is_persistent_across_initializations() {
        let dir = crate::test_util::temp_dir("jwt-persist");
        fs::write(dir.join("jwt-secret.key"), base64_encode(&[7u8; 32])).unwrap();
        initialize(&dir).unwrap();
        let token = generate_token("admin");
        assert_eq!(Some("admin".to_string()), validate_token(&token));
        let _ = fs::remove_dir_all(&dir);
    }
}
