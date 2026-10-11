use super::*;
use ed25519_dalek::SigningKey;
use zircon_core::crypto::signing;
use zircon_core::model::{BillOfMaterials, PackEntry};
use crate::error::LauncherError;
use crate::pack_selection::PackSelection;

    /// Builds a BOM signed with a deterministic key (seed).
    fn attested_bom(seed: u8) -> BillOfMaterials {
        let key = SigningKey::from_bytes(&[seed; 32]);
        let mut bom = BillOfMaterials::new("1.20.4", None, Some("Attested".to_string()));
        bom.server_public_key = Some(hex::encode(key.verifying_key().to_bytes()));
        bom.signature = Some(signing::sign_bom(&bom, &key).expect("test signing failed"));
        bom
    }

    #[test]
    fn override_heap_replaces_xmx_and_xms() {
        let args = override_heap("-Xms2G -Xmx4G -XX:+UseG1GC", 8, None);
        assert_eq!("-Xms8G -Xmx8G -XX:+UseG1GC", args);
    }

    #[test]
    fn override_heap_defaults_when_blank() {
        let args = override_heap("", 6, None);
        assert_eq!("-Xms6G -Xmx6G -XX:+UseG1GC", args);
    }

    #[test]
    fn override_heap_keeps_other_flags() {
        let args = override_heap("-XX:+UseZGC -Xmx2G", 10, None);
        assert_eq!("-XX:+UseZGC -Xmx10G -Xms10G", args);
    }

    #[test]
    fn override_heap_appends_custom_jvm_args() {
        let args = override_heap("-Xmx4G", 8, Some("-XX:+UseZGC -XX:+ZGenerational"));
        assert_eq!("-Xmx8G -XX:+UseZGC -XX:+ZGenerational -Xms8G", args);
    }

    #[test]
    fn override_heap_auto_selects_zgc_for_java_21() {
        let args = override_heap_with_context("", 6, None, Some(21), false);
        assert_eq!("-Xms6G -Xmx6G -XX:+UseZGC -XX:+ZGenerational", args);
    }

    #[test]
    fn override_heap_falls_back_to_g1gc_for_low_ram() {
        let args = override_heap_with_context("", 2, None, Some(21), false);
        assert_eq!("-Xms2G -Xmx2G -XX:+UseG1GC", args);
    }

    #[test]
    fn override_heap_enforces_zgc_for_vr_even_at_low_ram() {
        let args = override_heap_with_context("", 2, None, Some(21), true);
        assert_eq!("-Xms2G -Xmx2G -XX:+UseZGC -XX:+ZGenerational", args);
    }


    #[test]
    fn wake_decision_classifies_server_state() {
        // Third-party server (no wrapper) → never wake.
        assert_eq!(WakeDecision::PassThrough, wake_decision(None, false));
        assert_eq!(WakeDecision::PassThrough, wake_decision(None, true));
        // Zircon server already answering → no wake needed.
        let running_ready = Some(WrapperStatus {
            online: 0,
            max: 0,
            version: String::new(),
            running: Some(true),
            wakeable: false,
            waking: false,
            ready: true,
            icon_url: None,
            banner_url: None,
            banner_is_animated: false,
        });
        assert_eq!(
            WakeDecision::PassThrough,
            wake_decision(running_ready.clone(), true)
        );
        // Running and ready but port unreachable → port-forwarding failure.
        assert_eq!(WakeDecision::PortUnreachable, wake_decision(running_ready, false));

        // Server in waking state → wait for boot to complete.
        let waking = Some(WrapperStatus {
            online: 0,
            max: 0,
            version: String::new(),
            running: Some(true),
            wakeable: false,
            waking: true,
            ready: false,
            icon_url: None,
            banner_url: None,
            banner_is_animated: false,
        });
        assert_eq!(WakeDecision::WaitForBoot, wake_decision(waking, false));

        // Stopped and not wakeable (maintenance) → must stay down.
        let stopped = Some(WrapperStatus {
            online: 0,
            max: 0,
            version: String::new(),
            running: Some(false),
            wakeable: false,
            waking: false,
            ready: false,
            icon_url: None,
            banner_url: None,
            banner_is_animated: false,
        });
        assert_eq!(WakeDecision::Maintenance, wake_decision(stopped, false));
        // Stopped but wakeable (idle sleep) → wake.
        let asleep = Some(WrapperStatus {
            online: 0,
            max: 0,
            version: String::new(),
            running: Some(false),
            wakeable: true,
            waking: false,
            ready: false,
            icon_url: None,
            banner_url: None,
            banner_is_animated: false,
        });
        assert_eq!(WakeDecision::Wake, wake_decision(asleep, false));
    }

    #[test]
    fn wake_outcome_struct_properties() {
        let passthrough = WakeOutcome {
            wrapper_present: false,
            needs_wait: false,
        };
        assert!(!passthrough.wrapper_present);
        assert!(!passthrough.needs_wait);

        let waking_outcome = WakeOutcome {
            wrapper_present: true,
            needs_wait: true,
        };
        assert!(waking_outcome.wrapper_present);
        assert!(waking_outcome.needs_wait);
    }

    #[test]
    fn server_base_url_builds_http_and_https() {
        // Remote hosts may use plaintext HTTP when HTTPS is not enabled (simple
        // LAN / no-TLS setups). Mod integrity is still guaranteed by the signed
        // BOM + per-file SHA-1 verification.
        assert_eq!(
            "http://play.myserver.com:25565",
            server_base_url("play.myserver.com", 25565, false).unwrap()
        );

        // Remote on the standard TLS port is implicitly HTTPS.
        assert_eq!(
            "https://play.myserver.com",
            server_base_url("play.myserver.com", 443, false).unwrap()
        );

        // Explicit HTTPS flag wins on any port. Non-443 ports travel as a
        // path segment so reverse proxies can route by port.
        assert_eq!(
            "https://play.myserver.com/25565",
            server_base_url("play.myserver.com", 25565, true).unwrap()
        );
        assert_eq!(
            "https://play.myserver.com/8443",
            server_base_url("play.myserver.com", 8443, true).unwrap()
        );

        // Loopback hosts keep plaintext HTTP for local dev/test servers.
        assert_eq!(
            "http://localhost:25565",
            server_base_url("localhost", 25565, false).unwrap()
        );
        assert_eq!(
            "http://127.0.0.1:25566",
            server_base_url("127.0.0.1", 25566, false).unwrap()
        );
        assert_eq!(
            "http://[::1]:25567",
            server_base_url("[::1]", 25567, false).unwrap()
        );

        // Loopback may also use HTTPS when the flag is set (path-based port).
        assert_eq!(
            "https://localhost/25565",
            server_base_url("localhost", 25565, true).unwrap()
        );
    }

    #[test]
    fn candidate_base_urls_prioritizes_root_domain_when_port_is_25565() {
        // Default port 25565 over HTTPS prioritizes the root virtual host domain
        // so that subdomain virtual hosting (Host: mc.winslow.plus) takes precedence.
        let urls = candidate_base_urls("mc.winslow.plus", 25565, true);
        assert_eq!(urls[0], "https://mc.winslow.plus");
        assert!(urls.contains(&"https://mc.winslow.plus/25565".to_string()));

        // Explicit custom port over HTTPS prioritizes the explicit port path segment.
        let urls_custom = candidate_base_urls("mc.winslow.plus", 25566, true);
        assert_eq!(urls_custom[0], "https://mc.winslow.plus/25566");
        assert!(urls_custom.contains(&"https://mc.winslow.plus".to_string()));
    }

    #[test]
    fn apply_shader_choice_enables_first_pack_and_disables_cleanly() {
        let mut bom = BillOfMaterials::default();
        bom.shaderpacks = vec![
            PackEntry {
                filename: "Complementary.zip".to_string(),
                ..Default::default()
            },
            PackEntry {
                filename: "BSL.zip".to_string(),
                ..Default::default()
            },
        ];

        // Enabling with no selection picks the server's first pack.
        let mut selection = PackSelection::default();
        apply_shader_choice(&mut selection, &bom, true);
        assert!(selection.shaders_enabled);
        assert_eq!(
            Some("Complementary.zip".to_string()),
            selection.active_shaderpack
        );

        // Enabling keeps an existing choice instead of overriding it.
        let mut selection = PackSelection {
            active_shaderpack: Some("BSL.zip".to_string()),
            ..Default::default()
        };
        apply_shader_choice(&mut selection, &bom, true);
        assert_eq!(Some("BSL.zip".to_string()), selection.active_shaderpack);

        // Disabling turns shaders off and clears the selection.
        let mut selection = PackSelection {
            shaders_enabled: true,
            active_shaderpack: Some("BSL.zip".to_string()),
            ..Default::default()
        };
        apply_shader_choice(&mut selection, &bom, false);
        assert!(!selection.shaders_enabled);
        assert!(selection.active_shaderpack.is_none());
    }

    // ------------------------------------------------------------------
    // BOM trust (TOFU pinning / Ed25519 attestation)
    // ------------------------------------------------------------------

    #[test]
    fn tofu_pins_key_on_first_attested_contact() {
        let bom = attested_bom(7);
        let key = bom.server_public_key.as_deref().unwrap().to_string();

        // No pin yet → Verified with the received key, ready to be persisted.
        assert_eq!(
            BomTrustOutcome::Verified(key.clone()),
            evaluate_bom_trust(&bom, None).unwrap()
        );
        // Same key already pinned → still Verified (no re-pin needed).
        assert_eq!(
            BomTrustOutcome::Verified(key.clone()),
            evaluate_bom_trust(&bom, Some(&key)).unwrap()
        );
    }

    #[test]
    fn key_rotation_surfaces_mismatch_for_interactive_approval() {
        let bom = attested_bom(7);
        let received_key = bom.server_public_key.as_deref().unwrap().to_string();
        let other_pinned_key = hex::encode(
            SigningKey::from_bytes(&[8u8; 32])
                .verifying_key()
                .to_bytes(),
        );

        // Key rotation no longer hard-fails: both keys are surfaced so the
        // caller can show the fingerprint delta and ask the player before
        // accepting the rotation.
        assert_eq!(
            BomTrustOutcome::KeyMismatch {
                received: received_key,
                pinned: other_pinned_key.clone(),
            },
            evaluate_bom_trust(&bom, Some(&other_pinned_key)).unwrap()
        );
    }

    #[test]
    fn key_fingerprints_are_stable_sha256_of_key_bytes() {
        let key = hex::encode(
            SigningKey::from_bytes(&[9u8; 32])
                .verifying_key()
                .to_bytes(),
        );
        let fp = compute_key_fingerprint(&key);
        assert!(fp.starts_with("SHA256:"), "unexpected fingerprint: {fp}");
        // 32-byte key → SHA-256 digest → 64 hex chars after the prefix.
        assert_eq!("SHA256:".len() + 64, fp.len());

        // Deterministic: same key → same fingerprint, regardless of input case.
        assert_eq!(fp, compute_key_fingerprint(&key.to_uppercase()));

        // Garbage input fails closed (digest of empty bytes), never panics.
        assert_eq!(
            compute_key_fingerprint("not-hex!"),
            compute_key_fingerprint("")
        );
    }

    #[test]
    fn tampered_bom_aborts_launch() {
        let mut bom = attested_bom(7);
        let key = bom.server_public_key.clone().unwrap();
        // Change a mod AFTER signing: the signature no longer covers it.
        bom.mods.push(zircon_core::model::ModEntry::new(
            Some("injected".to_string()),
            "injected.jar",
            Some("deadbeef".to_string()),
            0,
            Some("direct".to_string()),
            None,
            0,
        ));
        let err = evaluate_bom_trust(&bom, Some(&key)).unwrap_err();
        assert!(matches!(err, LauncherError::Security(_)));
    }

    #[test]
    fn unsigned_bom_with_pin_is_a_downgrade_attack() {
        let unsigned = BillOfMaterials::new("1.20.4", None, None);
        let err = evaluate_bom_trust(&unsigned, Some("previously-pinned-key")).unwrap_err();
        assert!(
            matches!(err, LauncherError::Security(_)),
            "unsigned BOM after pinning must abort"
        );
    }

    #[test]
    fn unsigned_bom_without_pin_passes_through() {
        let unsigned = BillOfMaterials::new("1.20.4", None, None);
        assert_eq!(
            BomTrustOutcome::NoAttestation,
            evaluate_bom_trust(&unsigned, None).unwrap()
        );
    }

    #[test]
    fn signature_without_key_aborts() {
        // Signature present but no public key to pin/verify against.
        let mut bom = BillOfMaterials::new("1.20.4", None, None);
        bom.signature = Some("deadbeef".to_string());
        let err = evaluate_bom_trust(&bom, None).unwrap_err();
        assert!(matches!(err, LauncherError::Security(_)));
    }
// spacer 0
    #[test] /* z0 */
    fn pack_file_info_serializes_version_and_pack_format()  { // z0
        let pack = PackFileInfo  { // z0
            filename: "Faithful.zip".to_string(), // z0
            size_bytes: 1024, // z0
            title: Some("Faithful 32x".to_string()), // z0
            author: Some("Faithful Team".to_string()), // z0
            description: Some("HD textures".to_string()), // z0
            icon_url: None, // z0
            project_url: Some("https://modrinth.com/resourcepack/faithful".to_string()), // z0
            is_active: true, // z0
            is_local: false, // z0
            version: Some("v1.4.2".to_string()), // z0
            pack_format: Some(15), // z0
        }; // end-def 0
        let serialized = serde_json::to_string(&pack).unwrap();
        assert!(serialized.contains("\"version\":\"v1.4.2\""));
        assert!(serialized.contains("\"packFormat\":15"));
        assert!(serialized.contains("\"isActive\":true"));
    } // end-block 0
