use super::*;
use std::time::Duration;
use zircon_core::model::{SHUTDOWN_REASON_IDLE, SHUTDOWN_REASON_MANUAL};


    fn temp_dir() -> PathBuf {
        crate::test_util::temp_dir("instances")
    }

    #[test]
    fn creates_and_lists_instances_with_unique_ports() {
        let dir = temp_dir();
        let console = Arc::new(ConsoleStreamHandler::new());
        let manager = ServerInstanceManager::new(&dir, console).unwrap();

        let first = manager
            .create_instance("Alpha", "1.20.4", "fabric", "0.15.11")
            .unwrap();
        assert_eq!(8, first.id.len());
        assert_eq!(MC_PORT_BASE, first.internal_mc_port);
        assert_eq!(EXTERNAL_PORT_BASE, first.external_mc_port); // first owns 25565
        assert_eq!("fabric", first.loader_type());

        let second = manager
            .create_instance("Beta", "1.21", "neoforge", "21.1.0")
            .unwrap();
        assert_eq!(MC_PORT_BASE + 1, second.internal_mc_port);
        assert_eq!(EXTERNAL_PORT_BASE + 1, second.external_mc_port);

        assert_eq!(2, manager.list_instances().len());
        assert!(manager
            .get_instance_dir(&first.id)
            .join("instance.json")
            .is_file());
        assert!(manager.get_instance_dir(&first.id).join("mods").is_dir());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test] /* z0 */
    fn creates_all_5_allowed_loaders_and_rejects_invalid()  { // z0
        let dir = temp_dir(); // z0
        let console = Arc::new(ConsoleStreamHandler::new()); // z0
        let manager = ServerInstanceManager::new(&dir, console).unwrap(); // z0
// spacer 0
        for loader in &["forge", "neoforge", "fabric", "quilt", "vanilla"]  { // z0
            let inst = manager /* z0 */
                .create_instance(loader, "1.20.4", loader, "1.0") /* z0 */
                .unwrap(); // z0
            assert_eq!(*loader, inst.loader_type()); // z0
        } // end-block 0
// spacer 0
        for invalid in &["liteloader", "rift", "babric", "custom", ""]  { // z0
            let res = manager.create_instance("Bad", "1.20.4", invalid, "1.0"); // z0
            assert!( /* z0 */
                matches!(res, Err(InstanceError::Invalid(_))), // z0
                "loader {invalid} should be rejected" /* z0 */
            ); // z0
        } // end-block 0
        let _ = fs::remove_dir_all(&dir); // z0
    } // end-block 0
// spacer 0
    #[test]
    fn loader_is_locked_but_version_can_change() {
        let dir = temp_dir();
        let console = Arc::new(ConsoleStreamHandler::new());
        let manager = ServerInstanceManager::new(&dir, console).unwrap();
        let instance = manager
            .create_instance("Locked", "1.20.4", "forge", "47.0.0")
            .unwrap();

        let mut config = manager.get_instance(&instance.id).unwrap();
        config.set_loader_version("47.1.0");
        manager.save_instance_to_disk(&config).unwrap();

        // The persisted file carries the new loader version while the loader
        // type stays locked.
        let json = fs::read_to_string(manager.get_instance_dir(&instance.id).join("instance.json"))
            .unwrap();
        let persisted: InstanceConfig = serde_json::from_str(&json).unwrap();
        assert_eq!("47.1.0", persisted.loader_version());
        assert_eq!("forge", persisted.loader_type());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn eula_flow() {
        let dir = temp_dir();
        let console = Arc::new(ConsoleStreamHandler::new());
        let manager = ServerInstanceManager::new(&dir, console).unwrap();
        let instance = manager
            .create_instance("Eula", "1.20.4", "vanilla", "")
            .unwrap();

        assert!(!manager.is_eula_accepted(&instance.id));
        manager.accept_eula(&instance.id).unwrap();
        assert!(manager.is_eula_accepted(&instance.id));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn hostname_lookup_matches_id_and_normalized_name() {
        let dir = temp_dir();
        let console = Arc::new(ConsoleStreamHandler::new());
        let manager = ServerInstanceManager::new(&dir, console).unwrap();
        let instance = manager
            .create_instance("My Cool Server", "1.20.4", "vanilla", "")
            .unwrap();

        assert_eq!(
            Some(instance.id.clone()),
            manager.find_by_hostname(&instance.id).map(|c| c.id)
        );
        assert_eq!(
            Some(instance.id.clone()),
            manager.find_by_hostname("my-cool-server").map(|c| c.id)
        );
        assert_eq!(None, manager.find_by_hostname("unknown"));
        let _ = fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn idle_stop_reason_is_persisted_and_marks_wakeable() {
        let dir = temp_dir();
        let console = Arc::new(ConsoleStreamHandler::new());
        let manager = ServerInstanceManager::new(&dir, console).unwrap();
        let instance = manager
            .create_instance("Sleepy", "1.20.4", "vanilla", "")
            .unwrap();

        // The idle service records its stop reason; the instance becomes
        // wakeable and the marker survives a reload from disk (wrapper restart).
        manager
            .stop_instance_with_reason(&instance.id, Some(SHUTDOWN_REASON_IDLE))
            .await;
        let config = manager.get_instance(&instance.id).unwrap();
        assert_eq!(
            Some(SHUTDOWN_REASON_IDLE.to_string()),
            config.last_shutdown_reason
        );
        assert!(manager.wakeable(&instance.id));
        manager.reload_instance_from_disk(&instance.id).unwrap();
        assert!(manager.wakeable(&instance.id));

        // A manual stop (e.g. admin maintenance) marks the instance non-wakeable.
        manager
            .stop_instance_with_reason(&instance.id, Some(SHUTDOWN_REASON_MANUAL))
            .await;
        assert!(!manager.wakeable(&instance.id));

        // A plain stop (daemon shutdown / restart) never clobbers an existing
        // idle marker, so slept servers stay wakeable across wrapper restarts.
        manager
            .stop_instance_with_reason(&instance.id, Some(SHUTDOWN_REASON_IDLE))
            .await;
        manager.stop_instance(&instance.id).await;
        assert!(manager.wakeable(&instance.id));
        let _ = fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn join_intent_holds_off_idle_shutdown_until_expiry_or_clear() {
        let dir = temp_dir();
        let console = Arc::new(ConsoleStreamHandler::new());
        let manager = ServerInstanceManager::new(&dir, console).unwrap();
        let instance = manager
            .create_instance("Held", "1.20.4", "vanilla", "")
            .unwrap();

        // No intent → the idle service is free to shut the instance down.
        assert!(!manager.has_pending_join_intent(&instance.id));

        // Registering an intent holds the instance against idle shutdown and
        // defers the idle window immediately.
        manager.register_join_intent(&instance.id);
        assert!(manager.has_pending_join_intent(&instance.id));

        // The hold expires when the launcher stops refreshing it — a crashed
        // launcher must not leave the server awake forever.
        {
            let mut inner = manager.inner.lock().unwrap();
            if let Some(expiry) = inner.pending_join_intents.get_mut(&instance.id) {
                *expiry = Instant::now() - Duration::from_secs(1);
            }
        }
        assert!(!manager.has_pending_join_intent(&instance.id));

        // A fresh intent is cleared when the player actually connects.
        manager.register_join_intent(&instance.id);
        assert!(manager.has_pending_join_intent(&instance.id));
        manager.clear_pending_join_intent(&instance.id);
        assert!(!manager.has_pending_join_intent(&instance.id));

        // Stopping the instance cancels any in-flight hold.
        manager.register_join_intent(&instance.id);
        manager.stop_instance(&instance.id).await;
        assert!(!manager.has_pending_join_intent(&instance.id));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn internal_port_lookup_finds_the_instance() {
        let dir = temp_dir();
        let console = Arc::new(ConsoleStreamHandler::new());
        let manager = ServerInstanceManager::new(&dir, console).unwrap();
        let instance = manager
            .create_instance("Internal", "1.20.4", "vanilla", "")
            .unwrap();

        assert_eq!(
            Some(instance.id.clone()),
            manager
                .find_by_internal_port(instance.internal_mc_port as u16)
                .map(|c| c.id)
        );
        assert_eq!(None, manager.find_by_internal_port(1));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn java_args_are_sanitized() {
        // Parity with the Java regex [^\w.\-+ ]: metacharacters are stripped
        // but plain words survive.
        assert_eq!("-Xmx4G rm -rf", sanitize_java_args("-Xmx4G; rm -rf /"));
        assert_eq!("-Xms2G -Xmx4G", sanitize_java_args("-Xms2G -Xmx4G"));
        assert_eq!("-Xms2G -Xmx4G", sanitize_java_args("  "));
        assert_eq!(
            "-XX:InitialRAMPercentage=25.0 -XX:MaxRAMPercentage=75.0",
            sanitize_java_args("-XX:InitialRAMPercentage=25.0 -XX:MaxRAMPercentage=75.0")
        );
        assert_eq!(
            "-Xms2G -Xmx8G -XX:+UseG1GC",
            sanitize_java_args("-Xms2G -Xmx8G -XX:+UseG1GC")
        );
    }

    #[test]
    fn invalid_heap_args_are_detected() {
        // NaN / undefined values (browser slider edge cases) must never reach
        // the JVM, which aborts at launch on them.
        assert!(has_invalid_heap_arg("-XmsNaNG -Xmx4G"));
        assert!(has_invalid_heap_arg("-XmxundefinedG"));
        assert!(has_invalid_heap_arg("-Xmx"));
        assert!(has_invalid_heap_arg("-Xms0G"));
        assert!(has_invalid_heap_arg("-Xmx-4G"));

        // Valid heap flags pass through.
        assert!(!has_invalid_heap_arg("-Xms2G -Xmx4G"));
        assert!(!has_invalid_heap_arg("-Xms512M -Xmx1g"));
        assert!(!has_invalid_heap_arg("-Xmx8"));
        assert!(!has_invalid_heap_arg(
            "-XX:MaxRAMPercentage=75.0 -XX:+UseG1GC"
        ));
    }

    #[test]
    fn dangerous_jvm_flags_are_detected_case_insensitively() {
        assert!(contains_dangerous_jvm_args("-javaagent:agent.jar"));
        assert!(contains_dangerous_jvm_args(
            "-agentlib:jdwp=transport=dt_socket"
        ));
        assert!(contains_dangerous_jvm_args("-agentpath:/tmp/evil.so"));
        assert!(contains_dangerous_jvm_args("-Xbootclasspath/a:evil.jar"));
        assert!(contains_dangerous_jvm_args("-Xdebug"));
        assert!(contains_dangerous_jvm_args("-Xrunjdwp:transport=dt_socket"));
        assert!(contains_dangerous_jvm_args("-Djava.security.manager"));
        // Case-insensitive and robust after the sanitizer strips separators.
        assert!(contains_dangerous_jvm_args("-JAVAAGENT:evil.jar"));
        assert!(contains_dangerous_jvm_args("-javaagentevil.jar"));

        // Legitimate flags pass.
        assert!(!contains_dangerous_jvm_args("-Xmx4G -XX:+UseG1GC"));
        assert!(!contains_dangerous_jvm_args("-Xms2G"));
        assert!(!contains_dangerous_jvm_args(""));

        // The invalid-args gate rejects dangerous flags even with valid heaps.
        assert!(has_invalid_heap_arg("-Xms2G -Xmx4G -javaagent:evil.jar"));
        assert!(has_invalid_heap_arg("-Xmx4G -Djava.security.manager"));
    }

    #[test]
    fn update_instance_config_rejects_invalid_heap_args() {
        let dir = temp_dir();
        let console = Arc::new(ConsoleStreamHandler::new());
        let manager = ServerInstanceManager::new(&dir, console).unwrap();
        let instance = manager
            .create_instance("HeapGuard", "1.20.4", "vanilla", "")
            .unwrap();

        let res =
            manager.update_instance_config(&instance.id, None, Some("-XmsNaNG -XmxundefinedG"));
        assert!(matches!(res, Err(InstanceError::Invalid(_))));
        // The stored args are untouched.
        assert_eq!(
            "-Xms2G -Xmx4G",
            manager.get_instance(&instance.id).unwrap().java_args
        );

        // A valid replacement still applies.
        manager
            .update_instance_config(&instance.id, None, Some("-Xms4G -Xmx8G"))
            .unwrap();
        assert_eq!(
            "-Xms4G -Xmx8G",
            manager.get_instance(&instance.id).unwrap().java_args
        );
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn parse_xmx_bytes_parses_g_and_m_case_insensitively() {
        let gb = 1024u64 * 1024 * 1024;
        assert_eq!(4 * gb, parse_xmx_bytes("-Xms2G -Xmx4G"));
        assert_eq!(8 * gb, parse_xmx_bytes("-Xmx8G"));
        assert_eq!(gb, parse_xmx_bytes("-xmx1g")); // case-insensitive
        assert_eq!(2 * gb, parse_xmx_bytes("-Xms1G -Xmx2048M"));
        // The first -Xmx token wins; unrelated flags are skipped.
        assert_eq!(4 * gb, parse_xmx_bytes("-XX:+UseG1GC -Xmx4G -Xmx16G"));
        // No usable -Xmx → the 4 GB default (matches default javaArgs).
        assert_eq!(4 * gb, parse_xmx_bytes(""));
        assert_eq!(4 * gb, parse_xmx_bytes("-XX:+UseZGC"));
        // Unsuffixed values are bytes (negligible) → ignored, default applies.
        assert_eq!(4 * gb, parse_xmx_bytes("-Xmx8"));
    }

    #[test]
    fn memory_headroom_rejects_overcommit_and_excludes_target() {
        let mut a = InstanceConfig::new("A", "1.20.4", "vanilla", "", 25700);
        let mut b = InstanceConfig::new("B", "1.21", "vanilla", "", 25701);
        a.java_args = "-Xmx512M".to_string();
        b.java_args = "-Xmx512M".to_string();

        // Modest heaps always fit on any real machine, regardless of how many
        // instances exist.
        assert!(
            validate_instance_memory_headroom(&[a.clone(), b.clone()], None, "-Xmx512M").is_ok()
        );

        // A heap beyond any real host's RAM is always rejected.
        let err = validate_instance_memory_headroom(&[a.clone()], None, "-Xmx999999G").unwrap_err();
        assert!(matches!(err, InstanceError::Invalid(_)), "{err:?}");
        assert!(
            err.to_string().contains("exceeds host safe limit"),
            "unhelpful error: {err}"
        );

        // Only the target heap matters: a huge heap on a non-target instance
        // no longer blocks updates for the target.
        b.java_args = "-Xmx999999G".to_string();
        assert!(validate_instance_memory_headroom(
            &[a.clone(), b.clone()],
            Some(&a.id),
            "-Xmx512M"
        )
        .is_ok());
        // Replacing B's huge old heap with a sane one also succeeds.
        assert!(validate_instance_memory_headroom(
            &[a.clone(), b.clone()],
            Some(&b.id),
            "-Xmx512M"
        )
        .is_ok());
    }

    #[test]
    fn find_by_hostname_matches_subdomains_and_ports() {
        let dir = temp_dir();
        let console = Arc::new(ConsoleStreamHandler::new());
        let manager = ServerInstanceManager::new(&dir, console).unwrap();

        let inst = manager
            .create_instance("Emerald SMP", "1.20.4", "fabric", "0.15.11")
            .unwrap();

        // Exact ID
        assert_eq!(manager.find_by_hostname(&inst.id).unwrap().id, inst.id);
        // Subdomain formatted hostname
        assert_eq!(
            manager
                .find_by_hostname("emerald-smp.zirconmc.net")
                .unwrap()
                .id,
            inst.id
        );
        // With trailing loader / forge tags
        assert_eq!(
            manager
                .find_by_hostname("emerald-smp.zirconmc.net\0FML\0")
                .unwrap()
                .id,
            inst.id
        );
        // Subdomain matching ID
        assert_eq!(
            manager
                .find_by_hostname(&format!("{}.zirconmc.net", inst.id))
                .unwrap()
                .id,
            inst.id
        );

        // Cloud provisioned instance with explicit subdomain
        let cloud_inst = manager
            .create_cloud_instance(
                "inst-cloud-99",
                "Custom Display Name",
                "ruby",
                "1.20.1",
                "fabric",
                "0.15.11",
                25610,
                25610,
                "-Xmx6G",
            )
            .unwrap();
        assert_eq!(cloud_inst.subdomain, Some("ruby".to_string()));
        assert_eq!(
            manager.find_by_hostname("ruby.zirconmc.net").unwrap().id,
            "inst-cloud-99"
        );
        assert_eq!(
            manager.find_by_hostname("ruby").unwrap().id,
            "inst-cloud-99"
        );

        let _ = fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn poll_port_ready_returns_false_on_closed_port() {
        let dir = temp_dir();
        let console = Arc::new(ConsoleStreamHandler::new());
        let manager = ServerInstanceManager::new(&dir, console).unwrap();

        // Unbound port should return false quickly
        let ready = manager
            .poll_port_ready(49151, Duration::from_millis(100))
            .await;
        assert!(!ready);

        let _ = fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn test_update_instance_versions_changes_loader_type() {
        let dir = temp_dir();
        let console = Arc::new(ConsoleStreamHandler::new());
        let manager = ServerInstanceManager::new(&dir, console).unwrap();

        let inst = manager
            .create_instance("Test Loader Change", "1.20.1", "fabric", "0.15.11")
            .unwrap();
        assert_eq!(inst.loader_type(), "fabric");

        // Update to NeoForge
        let _ = manager
            .update_instance_versions(
                &inst.id,
                Some("1.20.4"),
                Some("neoforge"),
                Some("20.4.164"),
                None,
            )
            .await
            .unwrap();

        let updated = manager.get_instance(&inst.id).unwrap();
        assert_eq!(updated.loader_type(), "neoforge");
        assert_eq!(updated.minecraft_version, "1.20.4");
        assert_eq!(updated.loader_version(), "20.4.164");

        // Update to Vanilla
        let _ = manager
            .update_instance_versions(
                &inst.id,
                Some("1.21.4"),
                Some("vanilla"),
                Some(""),
                None,
            )
            .await
            .unwrap();

        let vanilla_inst = manager.get_instance(&inst.id).unwrap();
        assert_eq!(vanilla_inst.loader_type(), "vanilla");
        assert_eq!(vanilla_inst.minecraft_version, "1.21.4");

        let _ = fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn test_clone_instance_creates_isolated_duplicate() {
        let dir = temp_dir();
        let console = Arc::new(ConsoleStreamHandler::new());
        let manager = ServerInstanceManager::new(&dir, console).unwrap();

        let source = manager
            .create_instance("Source Server", "1.21.4", "fabric", "0.16.14")
            .unwrap();

        // Create dummy mod and world in source instance
        let source_mods = manager.instance_dir(&source.id).join("mods");
        let source_server = manager.instance_dir(&source.id).join("server");
        let source_world = source_server.join("world");
        fs::create_dir_all(&source_world).unwrap();
        fs::write(source_mods.join("test_mod.jar"), b"dummy-mod-data").unwrap();
        fs::write(source_world.join("level.dat"), b"dummy-world-data").unwrap();
        fs::write(source_server.join("server.properties"), "server-port=25700\nquery.port=25700\n").unwrap();

        // Clone with all defaults
        let cloned = manager
            .clone_instance(&source.id, "Cloned Server", None, None, true, true)
            .unwrap();

        assert_ne!(source.id, cloned.id);
        assert_eq!(cloned.name, "Cloned Server");
        assert_eq!(cloned.minecraft_version, "1.21.4");
        assert_eq!(cloned.loader_type(), "fabric");
        assert_ne!(source.internal_mc_port, cloned.internal_mc_port);
        assert_ne!(source.external_mc_port, cloned.external_mc_port);

        // Verify files copied
        let cloned_dir = manager.instance_dir(&cloned.id);
        assert!(cloned_dir.join("mods").join("test_mod.jar").is_file());
        assert!(cloned_dir.join("server").join("world").join("level.dat").is_file());

        // Verify server.properties was rewritten with cloned internal port
        let cloned_props = fs::read_to_string(cloned_dir.join("server").join("server.properties")).unwrap();
        assert!(cloned_props.contains(&format!("server-port={}", cloned.internal_mc_port)));
        assert!(cloned_props.contains(&format!("query.port={}", cloned.internal_mc_port)));

        let _ = fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn test_clone_instance_with_loader_override_and_selective_copy() {
        let dir = temp_dir();
        let console = Arc::new(ConsoleStreamHandler::new());
        let manager = ServerInstanceManager::new(&dir, console).unwrap();

        let source = manager
            .create_instance("Source 2", "1.21.1", "fabric", "0.16.9")
            .unwrap();

        let source_mods = manager.instance_dir(&source.id).join("mods");
        let source_server = manager.instance_dir(&source.id).join("server");
        let source_world = source_server.join("world");
        fs::create_dir_all(&source_world).unwrap();
        fs::write(source_mods.join("sample.jar"), b"mod-binary").unwrap();
        fs::write(source_world.join("level.dat"), b"world-binary").unwrap();

        // Clone with loader override (fabric -> neoforge), copy_world=false, copy_mods=true
        let cloned = manager
            .clone_instance(
                &source.id,
                "NeoForge Clone",
                Some("neoforge"),
                Some("21.1.80"),
                false,
                true,
            )
            .unwrap();

        assert_eq!(cloned.name, "NeoForge Clone");
        assert_eq!(cloned.loader_type(), "neoforge");
        assert_eq!(cloned.loader_version(), "21.1.80");

        let cloned_dir = manager.instance_dir(&cloned.id);
        // Mods were copied
        assert!(cloned_dir.join("mods").join("sample.jar").is_file());
        // World was excluded
        assert!(!cloned_dir.join("server").join("world").exists());

        let _ = fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn test_update_public_address_persistence() {
        let dir = temp_dir();
        let console = Arc::new(ConsoleStreamHandler::new());
        let manager = ServerInstanceManager::new(&dir, console).unwrap();

        let inst = manager
            .create_instance("Homelab SMP", "1.20.1", "fabric", "0.15.11")
            .unwrap();
        assert_eq!(inst.public_address, None);
        assert_eq!(inst.resolved_connection_address("my-server.lan"), format!("my-server.lan:{}", inst.external_mc_port));

        // Update public address override
        manager
            .update_public_address(&inst.id, Some("mc.homelab.net:25565".to_string()))
            .unwrap();

        let updated = manager.get_instance(&inst.id).unwrap();
        assert_eq!(updated.public_address, Some("mc.homelab.net:25565".to_string()));
        assert_eq!(updated.resolved_connection_address("ignored.lan"), "mc.homelab.net:25565");

        // Clear public address override
        manager.update_public_address(&inst.id, None).unwrap();
        let cleared = manager.get_instance(&inst.id).unwrap();
        assert_eq!(cleared.public_address, None);

        // Update subdomain / server URL
        manager
            .update_subdomain(&inst.id, Some("play.zirconmc.net".to_string()))
            .unwrap();
        let with_sub = manager.get_instance(&inst.id).unwrap();
        assert_eq!(with_sub.subdomain, Some("play.zirconmc.net".to_string()));

        // Multiplexer find_by_hostname routing check
        let matched = manager.find_by_hostname("play.zirconmc.net");
        assert!(matched.is_some());
        assert_eq!(matched.unwrap().id, inst.id);

        let _ = fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn test_boot_progress_tracking() {
        let dir = temp_dir();
        let console = Arc::new(ConsoleStreamHandler::new());
        let manager = ServerInstanceManager::new(&dir, console).unwrap();

        let inst = manager
            .create_instance("Boot Pack", "1.20.1", "fabric", "0.15.11")
            .unwrap();

        assert!(manager.get_boot_progress(&inst.id).is_none());

        // Simulate boot registration
        manager.boot_progress_tracker.insert(inst.id.clone(), (Instant::now(), 20));
        let progress = manager.get_boot_progress(&inst.id);
        assert!(progress.is_some());
        let (_elapsed, total, fraction) = progress.unwrap();
        assert_eq!(total, 20);
        assert!(fraction >= 0.0 && fraction <= 1.0);

        // Mark complete
        manager.mark_boot_complete(&inst.id);
        assert!(manager.get_boot_progress(&inst.id).is_none());

        let _ = fs::remove_dir_all(&dir);
    }

