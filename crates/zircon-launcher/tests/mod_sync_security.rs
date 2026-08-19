//! Mod-sync security integration tests (Phase 5 matrix): strict verification
//! of CurseForge-origin mods and the Ed25519 BOM attestation / TOFU trust
//! decision.

use std::path::PathBuf;
use std::time::Duration;

use ed25519_dalek::SigningKey;
use zircon_core::crypto::signing;
use zircon_core::model::{BillOfMaterials, ModEntry};
use zircon_launcher::commands::{evaluate_bom_trust, BomTrustOutcome};
use zircon_launcher::error::LauncherError;
use zircon_launcher::sync::mod_sync::ModSyncEngine;

fn temp_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "zircon-mod-sync-sec-{tag}-{}-{}",
        std::process::id(),
        uuid::Uuid::new_v4().simple()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// Must fail sync when the server claims a CurseForge origin for an unknown
/// SHA-1: the fingerprint alone is never trusted and an unknown hash cannot be
/// confirmed against the public database, so the sync aborts before anything
/// is installed. Deterministic regardless of network state — Modrinth either
/// reports the hash as unknown (fail-closed) or is unreachable (also
/// fail-closed).
#[tokio::test]
async fn rejects_unverified_curseforge_origin() {
    let mut bom = BillOfMaterials::new("1.20.4", None, Some("Test Server".to_string()));
    bom.add_mod(ModEntry::new(
        Some("cf-file-12345".to_string()),
        "mystery-mod.jar",
        // Deliberately not a hash any public database knows.
        Some("0000000000000000000000000000000000000000".to_string()),
        987654321,
        Some("curseforge".to_string()),
        Some("http://127.0.0.1:1/files/mods/mystery-mod.jar".to_string()),
        1024,
    ));

    let game_dir = temp_dir("curseforge");
    let engine = ModSyncEngine::new();
    let result = tokio::time::timeout(
        Duration::from_secs(60),
        engine.sync_with_bom(&bom, "http://127.0.0.1:1", &game_dir, None),
    )
    .await
    .expect("sync must terminate within the timeout")
    .expect("sync runs");

    assert!(
        result.aborted,
        "unknown-SHA-1 CurseForge mod must abort the sync"
    );
    let reason = result.abort_reason.unwrap_or_default();
    assert!(
        reason.contains("mystery-mod.jar"),
        "abort reason must name the offending mod: {reason}"
    );
    let _ = std::fs::remove_dir_all(&game_dir);
}

/// TOFU + Ed25519 BOM attestation: pins on first contact, surfaces key
/// rotation for interactive approval (fingerprint delta) and rejects a BOM
/// that was tampered with after signing (mod list changed).
#[test]
fn verifies_ed25519_bom_signature_tofu() {
    let key_a = SigningKey::from_bytes(&[1u8; 32]);
    let key_b = SigningKey::from_bytes(&[2u8; 32]);
    let pub_a = hex::encode(key_a.verifying_key().to_bytes());
    let pub_b = hex::encode(key_b.verifying_key().to_bytes());

    let mut bom = BillOfMaterials::new("1.20.4", None, Some("Attested".to_string()));
    bom.server_public_key = Some(pub_a.clone());
    bom.signature = Some(signing::sign_bom(&bom, &key_a).expect("test signing failed"));

    // First contact (no pin yet): the presented key is verified and returned
    // for the caller to persist as the TOFU pin.
    match evaluate_bom_trust(&bom, None) {
        Ok(BomTrustOutcome::Verified(key)) => assert_eq!(pub_a, key),
        other => panic!("expected Verified on first contact, got {other:?}"),
    }

    // A matching pin re-verifies cleanly.
    match evaluate_bom_trust(&bom, Some(&pub_a)) {
        Ok(BomTrustOutcome::Verified(key)) => assert_eq!(pub_a, key),
        other => panic!("expected Verified with matching pin, got {other:?}"),
    }

    // Key rotation: the launcher pinned key B but the server now presents
    // key A. Instead of a silent crash the mismatch is surfaced with both
    // keys so the player can approve or reject the rotation interactively.
    match evaluate_bom_trust(&bom, Some(&pub_b)) {
        Ok(BomTrustOutcome::KeyMismatch { received, pinned }) => {
            assert_eq!(pub_a, received, "received key must be the presented one");
            assert_eq!(
                pub_b, pinned,
                "pinned key must be the previously trusted one"
            );
        }
        other => panic!("expected KeyMismatch on rotation, got {other:?}"),
    }

    // Tampering after signing (a mod is injected into the signed list) breaks
    // the signature even though the embedded key still matches.
    let mut tampered = bom.clone();
    tampered.mods.push(ModEntry::new(
        Some("injected".to_string()),
        "injected.jar",
        Some("deadbeef".to_string()),
        0,
        Some("direct".to_string()),
        None,
        0,
    ));
    let err = evaluate_bom_trust(&tampered, Some(&pub_a)).unwrap_err();
    assert!(
        matches!(err, LauncherError::Security(_)),
        "tampered BOM must be rejected, got {err:?}"
    );
}
