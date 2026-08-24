//! Auth-related security integration tests (Phase 5 matrix).

mod common;

use axum::http::StatusCode;
use serde_json::json;

use zircon_server::auth::jwt;

use common::{send_from, temp_dir, test_app_with_limits};

/// A protected route that requires a valid bearer token.
const PROTECTED_URI: &str = "/api/instances";

/// The signing secret the JWT tests pin: `jwt-secret.key` stores base64(secret).
/// base64 of 32 × 0x07 — reused by `jwt::initialize` via the temp data dir and
/// by the forger below, so both sides sign/verify with the same key.
const PINNED_SECRET: [u8; 32] = [0x07; 32];
const PINNED_SECRET_B64: &str = "BwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwc=";

/// Builds a router whose JWT signing secret is the pinned one.
fn app_with_pinned_jwt_secret() -> axum::Router {
    let dir = temp_dir("auth-sec");
    std::fs::write(dir.join("jwt-secret.key"), PINNED_SECRET_B64).unwrap();
    jwt::initialize(&dir).unwrap();
    let _ = std::fs::remove_dir_all(&dir);
    // `jwt::initialize` cached the secret in its `OnceLock`; the app itself
    // doesn't need a real data dir for the token gate.
    test_app_with_limits(10, 30)
}

/// Signs a JWT with the pinned secret and the given claims, skipping the
/// mandatory `jti` claim (the Phase 1.5 regression: tokens without `jti` must
/// fail validation and be rejected with 401, so a hand-forged token can never
/// bypass session revocation).
fn forge_token(claims: serde_json::Value) -> String {
    let header = jsonwebtoken::Header::default();
    jsonwebtoken::encode(
        &header,
        &claims,
        &jsonwebtoken::EncodingKey::from_secret(&PINNED_SECRET),
    )
    .expect("forge token")
}

fn claims_without_jti(now: i64) -> serde_json::Value {
    json!({
        "sub": "admin",
        "iat": now,
        "exp": now + 3600,
    })
}

fn claims_with_jti(now: i64) -> serde_json::Value {
    json!({
        "sub": "admin",
        "jti": "test-session-id",
        "iat": now,
        "exp": now + 3600,
    })
}

#[tokio::test]
async fn jwt_missing_jti_rejected() {
    let app = app_with_pinned_jwt_secret();
    let now = chrono::Utc::now().timestamp();

    // A token signed with the right key but WITHOUT the mandatory jti claim
    // must not authenticate.
    let forged = forge_token(claims_without_jti(now));
    assert!(
        jwt::decode_claims(&forged).is_none(),
        "token without jti must fail claims deserialization"
    );
    let (status, _) = send_from(&app, "127.0.0.1", "GET", PROTECTED_URI, Some(&forged), None).await;
    assert_eq!(
        StatusCode::UNAUTHORIZED,
        status,
        "missing jti must yield 401"
    );

    // Control: a token WITH a jti claim (signed with the same key, jti not
    // revoked) passes the gate — proving the rejection above is specifically
    // the missing `jti`, not the signature.
    let with_jti = forge_token(claims_with_jti(now));
    let (status, _) = send_from(
        &app,
        "127.0.0.1",
        "GET",
        PROTECTED_URI,
        Some(&with_jti),
        None,
    )
    .await;
    assert_eq!(
        StatusCode::OK,
        status,
        "token with a valid (non-revoked) jti must pass the gate"
    );
}
