//! Loads, caches and persists the `BillOfMaterials`. The BOM is the single
//! source of truth the client launcher syncs against, so every mutation goes
//! through this type and is written to `bom.json` immediately.
//!
//! Port of `com.mcmanager.server.service.BomService`.

use std::fs;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use ed25519_dalek::SigningKey;
use zircon_core::crypto::signing;
use zircon_core::model::BillOfMaterials;

/// Loads, caches and persists the `BillOfMaterials`.
pub struct BomService {
    bom_file: PathBuf,
    default_bom: Option<BillOfMaterials>,
    bom: Mutex<Option<BillOfMaterials>>,
    /// The wrapper's Ed25519 signing key (from
    /// `ConfigService::load_or_create_signing_key`). When present, every BOM
    /// written to disk carries `signature` + `server_public_key`; launchers
    /// refuse unsigned or wrongly-signed BOMs once they have pinned a key.
    signing_key: Option<Arc<SigningKey>>,
}

impl BomService {
    /// Instance-scoped BOM stored at `<instance>/bom.json`.
    pub fn new(bom_file: PathBuf, default_bom: Option<BillOfMaterials>) -> Self {
        Self {
            bom_file,
            default_bom,
            bom: Mutex::new(None),
            signing_key: None,
        }
    }

    /// Attaches the server's signing key so `save_bom` attests every write.
    /// Tests and keyless deployments simply omit this.
    pub fn with_signing_key(mut self, signing_key: Option<Arc<SigningKey>>) -> Self {
        self.signing_key = signing_key;
        self
    }

    /// Returns a clone of the current BOM, loading it (or creating a default)
    /// on first access.
    pub fn get_bom(&self) -> BillOfMaterials {
        let mut guard = self.bom.lock().unwrap();
        if guard.is_none() {
            *guard = Some(self.load());
        }
        guard.as_ref().unwrap().clone()
    }

    /// Mutates the current BOM in place (loading it first).
    pub fn with_bom<R>(&self, f: impl FnOnce(&mut BillOfMaterials) -> R) -> R {
        let mut guard = self.bom.lock().unwrap();
        if guard.is_none() {
            *guard = Some(self.load());
        }
        f(guard.as_mut().unwrap())
    }

    /// Persists the current BOM to disk.
    pub fn save(&self) -> std::io::Result<()> {
        let bom = self.get_bom();
        self.save_bom(&bom)
    }

    pub fn has_bom_file(&self) -> bool {
        self.bom_file.is_file()
    }

    fn load(&self) -> BillOfMaterials {
        if self.bom_file.is_file() {
            match fs::read_to_string(&self.bom_file)
                .map_err(|e| std::io::Error::other(e.to_string()))
                .and_then(|content| {
                    serde_json::from_str::<BillOfMaterials>(&content)
                        .map_err(|e| std::io::Error::other(e.to_string()))
                }) {
                Ok(parsed) => {
                    tracing::info!(
                        "Loaded BOM: {} mods for MC {}",
                        parsed.mods.len(),
                        parsed.minecraft_version
                    );
                    // Self-heal legacy unsigned BOM files: attach the
                    // attestation fields on first load so deployments that
                    // predate BOM signing become signed without any admin
                    // action (launchers with a pinned key reject unsigned BOMs).
                    if self.signing_key.is_some() && parsed.signature.is_none() {
                        if let Err(e) = self.save_bom(&parsed) {
                            tracing::warn!("Could not sign existing BOM on load: {e}");
                        }
                    }
                    return parsed;
                }
                Err(e) => {
                    tracing::warn!(
                        "Could not parse {}, recreating: {e}",
                        self.bom_file.display()
                    );
                }
            }
        }
        match &self.default_bom {
            Some(default) => {
                if let Err(e) = self.save_bom(default) {
                    tracing::warn!("Could not write default BOM: {e}");
                }
                default.clone()
            }
            None => {
                tracing::warn!("No default BOM configured for {}", self.bom_file.display());
                BillOfMaterials::default()
            }
        }
    }

    fn save_bom(&self, bom: &BillOfMaterials) -> std::io::Result<()> {
        // Attest the BOM: embed the public key and sign the canonical digest
        // before writing. Signing happens after attaching the public key — the
        // digest strips both attestation fields, so the order is irrelevant to
        // the signature, and launchers recompute it identically.
        let mut signed_bom = bom.clone();
        if let Some(signing_key) = &self.signing_key {
            let pubkey_hex = hex::encode(signing_key.verifying_key().to_bytes());
            signed_bom.server_public_key = Some(pubkey_hex);
            let sig = signing::sign_bom(&signed_bom, signing_key)
                .map_err(|e| std::io::Error::other(e))?;
            signed_bom.signature = Some(sig);
        }
        let json = serde_json::to_string_pretty(&signed_bom)
            .map_err(|e| std::io::Error::other(e.to_string()))?;
        fs::write(&self.bom_file, json)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir() -> PathBuf {
        crate::test_util::temp_dir("bom")
    }

    #[test]
    fn default_bom_is_persisted_on_first_access() {
        let dir = temp_dir();
        let file = dir.join("bom.json");
        let default = BillOfMaterials::new("1.20.4", None, Some("Server".to_string()));
        let service = BomService::new(file.clone(), Some(default));

        let bom = service.get_bom();
        assert_eq!("1.20.4", bom.minecraft_version);
        assert!(file.is_file());

        service.with_bom(|b| {
            b.mods.push(zircon_core::model::ModEntry::new(
                Some("m".to_string()),
                "m.jar",
                None,
                0,
                None,
                None,
                1,
            ));
        });
        service.save().unwrap();

        let reloaded = BomService::new(file.clone(), None).get_bom();
        assert_eq!(1, reloaded.mods.len());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn legacy_unsigned_bom_is_signed_on_first_load() {
        use ed25519_dalek::SigningKey;
        use zircon_core::crypto::signing::verify_bom_signature;

        let dir = temp_dir();
        let file = dir.join("bom.json");
        // Pre-existing unsigned BOM (written before signing existed).
        let legacy = BillOfMaterials::new("1.20.4", None, Some("Legacy".to_string()));
        fs::write(&file, serde_json::to_string_pretty(&legacy).unwrap()).unwrap();

        let key = Arc::new(SigningKey::from_bytes(&[5u8; 32]));
        let service = BomService::new(file.clone(), None).with_signing_key(Some(key));
        let loaded = service.get_bom();

        assert_eq!("1.20.4", loaded.minecraft_version);
        // The on-disk copy was re-signed during the load.
        let on_disk: BillOfMaterials =
            serde_json::from_str(&fs::read_to_string(&file).unwrap()).unwrap();
        let pubkey = on_disk.server_public_key.as_deref().expect("public key");
        assert!(on_disk.signature.is_some(), "signature attached on load");
        assert!(verify_bom_signature(&on_disk, pubkey));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn saved_bom_is_signed_and_verifies_against_embedded_key() {
        use ed25519_dalek::SigningKey;
        use zircon_core::crypto::signing::verify_bom_signature;

        let dir = temp_dir();
        let file = dir.join("bom.json");
        let key = Arc::new(SigningKey::from_bytes(&[9u8; 32]));
        let service = BomService::new(file.clone(), None).with_signing_key(Some(key.clone()));
        service.with_bom(|b| {
            b.minecraft_version = "1.21.4".to_string();
            b.mods.push(zircon_core::model::ModEntry::new(
                Some("sodium".to_string()),
                "sodium-0.5.8.jar",
                Some("abc".to_string()),
                0,
                Some("modrinth".to_string()),
                Some("https://cdn.example/sodium.jar".to_string()),
                512000,
            ));
        });
        service.save().unwrap();

        // Every save embeds the public key and a valid self-signature.
        let on_disk: BillOfMaterials =
            serde_json::from_str(&fs::read_to_string(&file).unwrap()).unwrap();
        let pubkey = on_disk
            .server_public_key
            .as_deref()
            .expect("public key embedded");
        assert!(on_disk.signature.is_some(), "signature embedded");
        assert_eq!(pubkey, hex::encode(key.verifying_key().to_bytes()));
        assert!(
            verify_bom_signature(&on_disk, pubkey),
            "on-disk BOM must verify against its embedded key"
        );

        // Tampering with a mod breaks the signature (launcher refuses).
        let mut tampered = on_disk.clone();
        tampered.mods[0].sha1 = Some("deadbeef".to_string());
        assert!(!verify_bom_signature(&tampered, pubkey));

        // Without a signing key, BOMs stay unsigned.
        let unsigned_service = BomService::new(dir.join("unsigned.json"), None);
        unsigned_service.with_bom(|b| b.minecraft_version = "1.20.4".to_string());
        unsigned_service.save().unwrap();
        let unsigned: BillOfMaterials =
            serde_json::from_str(&fs::read_to_string(dir.join("unsigned.json")).unwrap()).unwrap();
        assert!(unsigned.signature.is_none());
        assert!(unsigned.server_public_key.is_none());
        let _ = fs::remove_dir_all(&dir);
    }
}
