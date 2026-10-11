use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use zircon_core::model::ModEntry;
use crate::services::bom::BomService;
use super::*;

    fn temp_dir() -> PathBuf {
        crate::test_util::temp_dir("mods")
    }

    #[tokio::test]
    async fn upload_replaces_existing_and_records_hash() {
        let dir = temp_dir();
        let mods_dir = dir.join("mods");
        let bom_file = dir.join("bom.json");
        let bom = Arc::new(BomService::new(
            bom_file,
            Some(zircon_core::model::BillOfMaterials::new(
                "1.20.4", None, None,
            )),
        ));
        let service = ModManagementService::new(bom.clone(), mods_dir, "");

        let data = b"fake jar contents".to_vec();
        let first = service
            .add_mod(std::io::Cursor::new(data.clone()), "my-mod.jar", None)
            .await
            .unwrap();
        assert_eq!("my-mod.jar", first.filename);
        assert_eq!(ORIGIN_DIRECT, first.origin.unwrap());
        assert!(!first.sha1.unwrap().is_empty());

        let second = service
            .add_mod(std::io::Cursor::new(data), "my-mod.jar", None)
            .await
            .unwrap();
        assert_eq!("my-mod.jar", second.filename);
        assert_eq!(1, service.list_mods().len());

        // Filename sanitization: path traversal is neutralized.
        let safe = sanitize_filename("../../evil.jar").unwrap();
        assert_eq!("evil.jar", safe);
        let _ = fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn remove_mod_deletes_file_and_bom_entry() {
        let dir = temp_dir();
        let mods_dir = dir.join("mods");
        let bom = Arc::new(BomService::new(
            dir.join("bom.json"),
            Some(zircon_core::model::BillOfMaterials::new(
                "1.20.4", None, None,
            )),
        ));
        let service = ModManagementService::new(bom, mods_dir.clone(), "");
        service
            .add_mod(std::io::Cursor::new(vec![1u8, 2, 3]), "mod.jar", None)
            .await
            .unwrap();

        assert!(service.remove_mod("mod.jar").unwrap());
        assert!(!mods_dir.join("mod.jar").exists());
        assert_eq!(0, service.list_mods().len());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn sanitize_filename_handles_traversal_and_extension() {
        assert_eq!("evil.jar", sanitize_filename("../../evil.jar").unwrap());
        assert_eq!("mod.jar", sanitize_filename("mod.jar").unwrap());
        assert_eq!("noext.jar", sanitize_filename("noext").unwrap());
        assert!(sanitize_filename("a b c").unwrap().ends_with(".jar"));
    }

    #[test]
    fn sanitize_filename_prefixes_windows_reserved_names() {
        // Reserved device names get a neutral `file_` prefix (original case is
        // preserved, so the collision with the Windows device is broken either
        // way).
        assert_eq!("file_CON.jar", sanitize_filename("CON.jar").unwrap());
        assert_eq!("file_nul.jar", sanitize_filename("nul.jar").unwrap());
        assert_eq!("file_COM1.jar", sanitize_filename("COM1.jar").unwrap());
        assert_eq!("file_LPT9.jar", sanitize_filename("LPT9.jar").unwrap());
        // Mixed/upper-case extensions are caught too (stem check is
        // case-insensitive; the original casing is preserved).
        assert_eq!("file_NUL.JAR", sanitize_filename("NUL.JAR").unwrap());
        // Ordinary names are untouched.
        assert_eq!("my_mod.jar", sanitize_filename("my_mod.jar").unwrap());
    }

    #[tokio::test]
    async fn enriched_metadata_is_persisted_to_bom() {
        let dir = temp_dir();
        let mods_dir = dir.join("mods");
        let bom = Arc::new(BomService::new(
            dir.join("bom.json"),
            Some(zircon_core::model::BillOfMaterials::new(
                "1.20.4", None, None,
            )),
        ));
        let service = ModManagementService::new(bom, mods_dir.clone(), "");

        // add_mod persists the raw entry (id = file name, no icon) — this is
        // the pre-fix behaviour that install_modrinth_version builds on.
        service
            .add_mod(
                std::io::Cursor::new(vec![1u8, 2, 3]),
                "sodium.jar",
                Some("modrinth"),
            )
            .await
            .unwrap();

        // Post-install enrichment must be written back, not left in memory.
        let mut entry = service.list_mods()[0].clone();
        entry.id = Some("AANobbMI".to_string());
        entry.title = Some("Sodium".to_string());
        entry.icon_url = Some("https://cdn.modrinth.com/data/AANobbMI/icon.png".to_string());
        service.persist_entry(&entry).unwrap();

        let reloaded = service.list_mods();
        assert_eq!(1, reloaded.len());
        assert_eq!(Some("AANobbMI".to_string()), reloaded[0].id);
        assert_eq!(Some("Sodium".to_string()), reloaded[0].title);
        assert_eq!(
            Some("https://cdn.modrinth.com/data/AANobbMI/icon.png".to_string()),
            reloaded[0].icon_url
        );

        // Survives a fresh load from disk (no in-memory cache involved).
        let bom2 = Arc::new(BomService::new(dir.join("bom.json"), None));
        let service2 = ModManagementService::new(bom2, mods_dir, "");
        let disk = service2.list_mods();
        assert_eq!(Some("AANobbMI".to_string()), disk[0].id);
        assert_eq!(
            Some("https://cdn.modrinth.com/data/AANobbMI/icon.png".to_string()),
            disk[0].icon_url
        );
        let _ = fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn test_set_mods_enabled_disk_and_bom_synchronization() {
        let sandbox = temp_dir();
        let mods_path = sandbox.join("mods");
        let bom_instance = Arc::new(BomService::new(
            sandbox.join("bom.json"),
            Some(zircon_core::model::BillOfMaterials::new("1.21.1", None, None)),
        ));
        let service = ModManagementService::new(bom_instance, mods_path.clone(), "");
        service
            .add_mod(std::io::Cursor::new(b"TEST_JAR_PAYLOAD"), "test_mod.jar", None)
            .await
            .expect("should install mod");
        assert!(mods_path.join("test_mod.jar").is_file());

        // Deactivate mod
        let toggled = service
            .set_mods_enabled(&["test_mod.jar".to_string()], false)
            .expect("disable mod should work");
        assert_eq!(toggled, vec!["test_mod.jar".to_string()]);
        assert!(!mods_path.join("test_mod.jar").exists());
        assert!(mods_path.join("test_mod.jar.disabled").is_file());
        assert!(!service.list_mods()[0].enabled);

        // Reactivate mod
        let reactivated = service
            .set_mods_enabled(&["test_mod.jar".to_string()], true)
            .expect("enable mod should work");
        assert_eq!(reactivated, vec!["test_mod.jar".to_string()]);
        assert!(mods_path.join("test_mod.jar").is_file());
        assert!(!mods_path.join("test_mod.jar.disabled").exists());
        assert!(service.list_mods()[0].enabled);

        let _ = fs::remove_dir_all(&sandbox);
    }

    #[tokio::test]
    async fn test_remove_mods_cleans_active_and_disabled_files() {
        let sandbox = temp_dir();
        let mods_path = sandbox.join("mods");
        let bom_instance = Arc::new(BomService::new(
            sandbox.join("bom.json"),
            Some(zircon_core::model::BillOfMaterials::new("1.21.1", None, None)),
        ));
        let service = ModManagementService::new(bom_instance, mods_path.clone(), "");
        service
            .add_mod(std::io::Cursor::new(b"MOD_A"), "alpha.jar", None)
            .await
            .unwrap();
        service
            .add_mod(std::io::Cursor::new(b"MOD_B"), "beta.jar", None)
            .await
            .unwrap();
        service
            .set_mods_enabled(&["beta.jar".to_string()], false)
            .unwrap();
        assert!(mods_path.join("beta.jar.disabled").is_file());

        let purged = service
            .remove_mods(&["alpha.jar".to_string(), "beta.jar".to_string()])
            .expect("remove alpha and beta");
        assert_eq!(purged.len(), 2);
        assert!(!mods_path.join("alpha.jar").exists());
        assert!(!mods_path.join("beta.jar.disabled").exists());
        assert!(service.list_mods().is_empty());

        let _ = fs::remove_dir_all(&sandbox);
    }

    #[tokio::test]
    async fn repair_skips_healthy_entries_without_network() {
        let dir = temp_dir();
        let bom = Arc::new(BomService::new(
            dir.join("bom.json"),
            Some(zircon_core::model::BillOfMaterials::new(
                "1.20.4", None, None,
            )),
        ));
        let service = ModManagementService::new(bom, dir.join("mods"), "");

        // Non-modrinth origins are never repaired.
        let mut direct = ModEntry::new(
            Some("550e8400-e29b-41d4-a716-446655440000".to_string()),
            "x.jar",
            None,
            0,
            Some(ORIGIN_DIRECT.to_string()),
            None,
            0,
        );
        assert!(!service.repair_modrinth_metadata(&mut direct).await);

        // Healthy modrinth entries (alphanumeric provider id) are skipped.
        let mut healthy = ModEntry::new(
            Some("AANobbMI".to_string()),
            "sodium.jar",
            None,
            0,
            Some(ORIGIN_MODRINTH.to_string()),
            None,
            0,
        );
        assert!(!service.repair_modrinth_metadata(&mut healthy).await);

        let _ = fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn set_mod_side_persists_to_bom() {
        let dir = temp_dir();
        let bom = Arc::new(BomService::new(
            dir.join("bom.json"),
            Some(zircon_core::model::BillOfMaterials::new("1.20.4", None, None)),
        ));
        let mods_dir = dir.join("mods");
        let service = ModManagementService::new(bom.clone(), mods_dir.clone(), "");

        let entry = service
            .add_mod(std::io::Cursor::new(b"data"), "sodium.jar", Some(ORIGIN_MODRINTH))
            .await
            .unwrap();
        assert_eq!(zircon_core::model::ModSide::Both, entry.side);

        let updated = service
            .set_mod_side("sodium.jar", zircon_core::model::ModSide::Client)
            .unwrap();
        assert_eq!(zircon_core::model::ModSide::Client, updated.side);

        // Verify it was persisted in the BOM
        let current_bom = bom.get_bom();
        assert_eq!(
            zircon_core::model::ModSide::Client,
            current_bom.mods[0].side
        );

        // Verify get_client_bom includes client-side mods
        let client_bom = bom.get_client_bom();
        assert_eq!(1, client_bom.mods.len());

        // Update to server-only and verify it gets filtered out of client_bom
        service
            .set_mod_side("sodium.jar", zircon_core::model::ModSide::Server)
            .unwrap();
        let server_bom = bom.get_bom();
        assert_eq!(1, server_bom.mods.len());
        let filtered_client_bom = bom.get_client_bom();
        assert_eq!(0, filtered_client_bom.mods.len());

        let _ = fs::remove_dir_all(&dir);
    }

