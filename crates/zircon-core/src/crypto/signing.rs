//! Ed25519 BOM attestation: RFC 8785 canonical digest, signing and verification.
//!
//! The server signs the canonical digest of every `BillOfMaterials` it writes
//! with its persistent Ed25519 signing key; the launcher pins the server's
//! public key on first use (TOFU) and refuses to trust a BOM whose signature
//! does not verify against the pinned key. This makes the authoritative mod
//! list (names, pinned hashes, URLs) tamper-evident end to end — a compromised
//! or rogue wrapper can no longer silently swap mods or inject a different
//! list.
//!
//! Signatures are hex-encoded in the BOM JSON (`signature`); the public key is
//! embedded as `server_publicKey` so first-contact pinning has something to
//! pin.
//!
//! Digests are computed over the RFC 8785 (JCS) canonical form of the BOM via
//! `serde_jcs`, not raw `serde_json` output. JCS guarantees a single byte
//! representation for any JSON value — lexicographical key sorting, IEEE 754
//! number canonicalization, minimal string escaping, and rejection of
//! non-finite floats — so the digest is identical on every platform and every
//! library release, even when the wire JSON arrived with different key order,
//! whitespace, or escaping.

use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use sha2::{Digest, Sha256};

use crate::model::BillOfMaterials;

/// Computes the deterministic RFC 8785 (JCS) SHA-256 digest of a BOM.
///
/// The attestation fields (`signature`, `server_public_key`) are stripped
/// before hashing so the digest is independent of the key/signature currently
/// attached and can be computed identically on signer and verifier. Serializing
/// through `serde_jcs` produces the canonical JCS byte stream (lexicographical
/// key sort, IEEE 754 number canon, minimal escaping), which is stable across
/// platforms and serde/serde_json releases.
pub fn canonical_bom_digest(bom: &BillOfMaterials) -> Result<Vec<u8>, String> {
    let mut cloned = bom.clone();
    cloned.signature = None;
    cloned.server_public_key = None;

    // Convert to canonical JSON bytes per RFC 8785 (lexicographical key sort,
    // IEEE 754 float canon). Errors — e.g. non-finite floats — are surfaced
    // instead of panicking so callers can fail closed.
    let canonical_bytes =
        serde_jcs::to_vec(&cloned).map_err(|e| format!("JCS canonicalization failed: {e}"))?;

    let mut hasher = Sha256::new();
    hasher.update(&canonical_bytes);
    Ok(hasher.finalize().to_vec())
}

/// Signs the canonical BOM digest and returns the hex-encoded Ed25519
/// signature (64 bytes → 128 hex chars).
pub fn sign_bom(bom: &BillOfMaterials, signing_key: &SigningKey) -> Result<String, String> {
    let digest = canonical_bom_digest(bom)?;
    let signature = signing_key.sign(&digest);
    Ok(hex::encode(signature.to_bytes()))
}

/// Verifies the BOM's embedded `signature` against `pubkey_hex` (the hex
/// Ed25519 public key). Fails closed: a missing/malformed signature, public key
/// or digest mismatch all return `false`.
pub fn verify_bom_signature(bom: &BillOfMaterials, pubkey_hex: &str) -> bool {
    let Some(sig_hex) = &bom.signature else {
        return false;
    };
    let Ok(pubkey_bytes) = hex::decode(pubkey_hex) else {
        return false;
    };
    let Ok(pubkey_array): Result<[u8; 32], _> = pubkey_bytes.try_into() else {
        return false;
    };
    let Ok(verifying_key) = VerifyingKey::from_bytes(&pubkey_array) else {
        return false;
    };
    let Ok(sig_bytes) = hex::decode(sig_hex) else {
        return false;
    };
    let Ok(sig_array): Result<[u8; 64], _> = sig_bytes.try_into() else {
        return false;
    };
    let signature = Signature::from_bytes(&sig_array);
    let Ok(digest) = canonical_bom_digest(bom) else {
        return false;
    };

    verifying_key.verify(&digest, &signature).is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{BillOfMaterials, ModLoaderInfo};

    /// A deterministic signing key so tests never need an RNG.
    fn test_key(seed: u8) -> SigningKey {
        SigningKey::from_bytes(&[seed; 32])
    }

    fn signed_bom(seed: u8) -> BillOfMaterials {
        let key = test_key(seed);
        let mut bom = BillOfMaterials::new(
            "1.20.4",
            Some(ModLoaderInfo::new("fabric", "0.15.11", None)),
            Some("Attested Server".to_string()),
        );
        bom.server_public_key = Some(hex::encode(key.verifying_key().to_bytes()));
        bom.signature = Some(sign_bom(&bom, &key).expect("test signing failed"));
        bom
    }

    #[test]
    fn sign_then_verify_round_trips() {
        let key = test_key(7);
        let bom = signed_bom(7);
        let pubkey = hex::encode(key.verifying_key().to_bytes());
        assert!(verify_bom_signature(&bom, &pubkey));
        // The embedded server_public_key field matches the key that signed.
        assert_eq!(Some(pubkey.clone()), bom.server_public_key);
        assert!(bom.signature.is_some());
    }

    #[test]
    fn tampered_bom_fails_verification() {
        let key = test_key(7);
        let mut bom = signed_bom(7);
        // Flip a mod field — the digest changes, the old signature no longer
        // covers it.
        bom.mods.push(crate::model::ModEntry::new(
            Some("injected".to_string()),
            "injected.jar",
            Some("deadbeef".to_string()),
            0,
            Some("direct".to_string()),
            None,
            0,
        ));
        let pubkey = hex::encode(key.verifying_key().to_bytes());
        assert!(!verify_bom_signature(&bom, &pubkey));
    }

    #[test]
    fn wrong_key_fails_verification() {
        let bom = signed_bom(7);
        // A different key (seed 8) cannot validate seed-7's signature.
        let other_pubkey = hex::encode(test_key(8).verifying_key().to_bytes());
        assert!(!verify_bom_signature(&bom, &other_pubkey));
    }

    #[test]
    fn missing_or_malformed_attestation_fails_closed() {
        // Unsigned BOM: no signature → false.
        let mut unsigned = BillOfMaterials::new("1.20.4", None, None);
        unsigned.server_public_key = Some(hex::encode(test_key(1).verifying_key().to_bytes()));
        assert!(!verify_bom_signature(
            &unsigned,
            unsigned.server_public_key.as_deref().unwrap()
        ));

        // Garbage hex in either field → false.
        let mut bom = signed_bom(7);
        bom.signature = Some("not-hex!".to_string());
        let pubkey = hex::encode(test_key(7).verifying_key().to_bytes());
        assert!(!verify_bom_signature(&bom, &pubkey));

        // Bad/empty/short public key input → false.
        let bom = signed_bom(7);
        assert!(!verify_bom_signature(&bom, "zz"));
        assert!(!verify_bom_signature(&bom, ""));
        assert!(!verify_bom_signature(&bom, &pubkey[..62])); // wrong length
    }

    #[test]
    fn canonical_digest_is_stable_across_attestation_fields() {
        // Re-encoding through JSON must not change the digest: the launcher
        // parses the wire BOM, the server signed the same content.
        let key = test_key(3);
        let mut bom = signed_bom(3);
        let digest_a = canonical_bom_digest(&bom).expect("digest");

        // Attaching a different signature/key does not change the content digest.
        bom.signature = Some(sign_bom(&bom, &key).expect("test signing failed"));
        bom.server_public_key = Some(hex::encode(key.verifying_key().to_bytes()));
        let digest_b = canonical_bom_digest(&bom).expect("digest");
        assert_eq!(digest_a, digest_b);

        // Round-tripping through JSON preserves the digest.
        let json = serde_json::to_string(&bom).unwrap();
        let reparsed: BillOfMaterials = serde_json::from_str(&json).unwrap();
        assert_eq!(digest_a, canonical_bom_digest(&reparsed).expect("digest"));

        // Content changes move the digest.
        bom.server_title = Some("Different".to_string());
        assert_ne!(digest_a, canonical_bom_digest(&bom).expect("digest"));
    }

    #[test]
    fn jcs_normalizes_key_order_whitespace_and_escaping() {
        // The same semantic BOM can arrive on the wire in different byte
        // representations: keys in any order, extra whitespace, and \uXXXX
        // escapes instead of literal characters. JCS must reduce all of them
        // to one digest — the whole point of RFC 8785.
        let canonical_json = r#"{
            "schemaVersion": 1,
            "minecraftVersion": "1.20.4",
            "modLoader": { "version": "0.15.11", "type": "fabric" },
            "mods": [],
            "shaderpacks": [],
            "resourcepacks": [],
            "serverTitle": "Caf\u00e9"
        }"#;
        let shuffled_json = r#"{"serverTitle":"Café","resourcepacks":[],"shaderpacks":[],"mods":[],"modLoader":{"type":"fabric","version":"0.15.11"},"minecraftVersion":"1.20.4","schemaVersion":1}"#;

        let a: BillOfMaterials = serde_json::from_str(canonical_json).unwrap();
        let b: BillOfMaterials = serde_json::from_str(shuffled_json).unwrap();
        assert_eq!(
            canonical_bom_digest(&a).expect("digest"),
            canonical_bom_digest(&b).expect("digest"),
            "semantically identical BOMs must share one JCS digest"
        );

        // The canonical form is key-sorted lexicographically — "configs"
        // leads ('c' < 'm'), not the struct-declaration order ("schemaVersion" first) that
        // plain serde_json emits.
        let canonical = serde_jcs::to_string(&a).unwrap();
        assert!(
            canonical.starts_with(r#"{"configs":[],"minecraftVersion":"1.20.4","modLoader":"#),
            "JCS must sort keys lexicographically: {canonical}"
        );
        let struct_order = serde_json::to_string(&a).unwrap();
        assert!(
            struct_order.starts_with(r#"{"schemaVersion":1,"#),
            "serde_json emits struct-declaration order: {struct_order}"
        );
        assert_ne!(canonical, struct_order);
    }
}
