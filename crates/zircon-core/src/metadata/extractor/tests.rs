use super::*;
use std::fs::File;
use std::path::PathBuf;

    struct ZipEntry<'a>(&'static str, &'a str);

    fn make_jar(name: &str, entries: &[ZipEntry<'_>]) -> PathBuf {
        // Unique per-jar directory so parallel tests never collide.
        let dir =
            std::env::temp_dir().join(format!("zircon-metadata-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join(name);
        let f = File::create(&file).unwrap();
        let mut zip = zip::ZipWriter::new(f);
        let options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated);
        for ZipEntry(entry_name, content) in entries {
            zip.start_file(*entry_name, options).unwrap();
            std::io::Write::write_all(&mut zip, content.as_bytes()).unwrap();
        }
        zip.finish().unwrap();
        file
    }

    #[test]
    fn extracts_fabric_metadata() {
        let jar = make_jar(
            "fabric-mod.jar",
            &[ZipEntry(
                "fabric.mod.json",
                r#"{
                    "id": "sodium",
                    "name": "Sodium",
                    "version": "0.5.8",
                    "description": "Fast rendering",
                    "environment": "client"
                }"#,
            )],
        );

        let meta = extract(&jar).unwrap();
        assert_eq!("sodium", meta.id);
        assert_eq!("Sodium", meta.name);
        assert_eq!("0.5.8", meta.version);
        assert_eq!("Fast rendering", meta.description);
        assert_eq!(ModLoaderType::Fabric, meta.loader_type);
        assert_eq!("client", meta.normalized_environment());
        assert_eq!("", meta.author); // no authors field
        let _ = std::fs::remove_dir_all(jar.parent().unwrap());
    }

    #[test]
    fn extracts_fabric_authors_from_strings_and_objects() {
        let jar = make_jar(
            "fabric-authors.jar",
            &[ZipEntry(
                "fabric.mod.json",
                r#"{
                    "id": "authorsmod",
                    "authors": [
                        "jellysquid3",
                        { "name": "grum", "contact": {} }
                    ]
                }"#,
            )],
        );

        let meta = extract(&jar).unwrap();
        assert_eq!("jellysquid3, grum", meta.author);
        let _ = std::fs::remove_dir_all(jar.parent().unwrap());
    }

    #[test]
    fn extracts_fabric_metadata_with_defaults() {
        let jar = make_jar(
            "fabric-min.jar",
            &[ZipEntry("fabric.mod.json", r#"{"id": "minimal"}"#)],
        );

        let meta = extract(&jar).unwrap();
        assert_eq!("minimal", meta.id);
        assert_eq!("minimal", meta.name); // name falls back to id
        assert_eq!("0.0.0", meta.version);
        assert_eq!(ModLoaderType::Fabric, meta.loader_type);
        assert_eq!("both", meta.normalized_environment());
        let _ = std::fs::remove_dir_all(jar.parent().unwrap());
    }

    #[test]
    fn fabric_environment_object_form() {
        let jar = make_jar(
            "fabric-env.jar",
            &[ZipEntry(
                "fabric.mod.json",
                r#"{"id": "envmod", "environment": {"client": "*"}}"#,
            )],
        );
        let meta = extract(&jar).unwrap();
        assert_eq!("client", meta.normalized_environment());
        let _ = std::fs::remove_dir_all(jar.parent().unwrap());
    }

    #[test]
    fn extracts_forge_toml_metadata() {
        let jar = make_jar(
            "forge-mod.jar",
            &[ZipEntry(
                "META-INF/mods.toml",
                r#"modLoader="javafml"
loaderVersion="[47,)"
license="MIT"

[[mods]]
modId="jei"
version="15.2.0.27"
displayName="Just Enough Items"
description="Show recipes in your inventory"
authors="mezz, snowshock"
"#,
            )],
        );

        let meta = extract(&jar).unwrap();
        assert_eq!("jei", meta.id);
        assert_eq!("Just Enough Items", meta.name);
        assert_eq!("15.2.0.27", meta.version);
        assert_eq!("Show recipes in your inventory", meta.description);
        assert_eq!("mezz, snowshock", meta.author);
        assert_eq!(ModLoaderType::Forge, meta.loader_type);
        assert_eq!("both", meta.normalized_environment());
        let _ = std::fs::remove_dir_all(jar.parent().unwrap());
    }

    #[test]
    fn extracts_neoforge_toml_metadata() {
        let jar = make_jar(
            "neoforge-mod.jar",
            &[ZipEntry(
                "META-INF/neoforge.mods.toml",
                r#"modLoader="javafml"
loaderVersion="[2,)"
license="MIT"

[[mods]]
modId="example"
version="1.0.0"
displayName="Example Mod"
"#,
            )],
        );

        let meta = extract(&jar).unwrap();
        assert_eq!("example", meta.id);
        assert_eq!("Example Mod", meta.name);
        assert_eq!("1.0.0", meta.version);
        assert_eq!(ModLoaderType::NeoForge, meta.loader_type);
        let _ = std::fs::remove_dir_all(jar.parent().unwrap());
    }

    #[test]
    fn numeric_version_string_in_toml() {
        let jar = make_jar(
            "numeric-version.jar",
            &[ZipEntry(
                "META-INF/mods.toml",
                r#"[[mods]]
modId="num"
version=42
"#,
            )],
        );
        let meta = extract(&jar).unwrap();
        assert_eq!("42", meta.version);
        let _ = std::fs::remove_dir_all(jar.parent().unwrap());
    }

    #[test]
    fn neoforge_metadata_wins_over_forge_when_both_present() {
        let jar = make_jar(
            "dual-toml.jar",
            &[
                ZipEntry(
                    "META-INF/mods.toml",
                    r#"[[mods]]
modId="forge-only"
version="1.0.0"
"#,
                ),
                ZipEntry(
                    "META-INF/neoforge.mods.toml",
                    r#"[[mods]]
modId="neoforge-only"
version="2.0.0"
"#,
                ),
            ],
        );

        let meta = extract(&jar).unwrap();
        assert_eq!("neoforge-only", meta.id);
        assert_eq!(ModLoaderType::NeoForge, meta.loader_type);
        let _ = std::fs::remove_dir_all(jar.parent().unwrap());
    }

    #[test]
    fn rejects_jar_without_metadata() {
        let jar = make_jar(
            "empty.jar",
            &[ZipEntry("META-INF/MANIFEST.MF", "Manifest-Version: 1.0\n")],
        );
        let err = extract(&jar).unwrap_err();
        assert!(err.to_string().contains("Unknown or unparseable mod jar"));
        let _ = std::fs::remove_dir_all(jar.parent().unwrap());
    }

    #[test]
    fn rejects_missing_id() {
        let jar = make_jar(
            "no-id.jar",
            &[ZipEntry("fabric.mod.json", r#"{"name": "No Id"}"#)],
        );
        let err = extract(&jar).unwrap_err();
        assert!(err.to_string().contains("missing required field 'id'"));
        let _ = std::fs::remove_dir_all(jar.parent().unwrap());
    }

    #[test]
    fn caps_metadata_read_at_2mb_preventing_zip_bomb() {
        // A fabric.mod.json far larger than the 2 MB cap. Without the cap this
        // JSON parses fine; with the cap the read is truncated mid-string so
        // parsing fails, proving the extractor never decompresses the whole
        // entry (the uncompressed ZIP bomb scenario).
        let padding = "a".repeat(MAX_METADATA_BYTES as usize + 1);
        let meta = format!(r#"{{"id": "bomb", "padding": "{padding}"}}"#);
        let jar = make_jar("zip-bomb.jar", &[ZipEntry("fabric.mod.json", &meta)]);

        let err = extract(&jar).unwrap_err();
        assert!(
            matches!(err, MetadataError::Invalid(_)),
            "oversized metadata should fail as truncated/invalid, got {err:?}"
        );
        let _ = std::fs::remove_dir_all(jar.parent().unwrap());
    }

    // ------------------------------------------------------------------
    // validate_mod_jar_structure
    // ------------------------------------------------------------------

    #[test]
    fn structure_accepts_valid_fabric_and_forge_jars() {
        let fabric = make_jar(
            "struct-fabric.jar",
            &[
                ZipEntry("fabric.mod.json", r#"{"id": "ok"}"#),
                ZipEntry("com/example/Mod.class", "class bytes"),
            ],
        );
        assert!(validate_mod_jar_structure(&fabric).is_ok());
        let _ = std::fs::remove_dir_all(fabric.parent().unwrap());

        let forge = make_jar(
            "struct-forge.jar",
            &[
                ZipEntry("META-INF/mods.toml", "[[mods]]\nmodId=\"ok\"\n"),
                ZipEntry("META-INF/MANIFEST.MF", "Manifest-Version: 1.0\n"),
            ],
        );
        assert!(validate_mod_jar_structure(&forge).is_ok());
        let _ = std::fs::remove_dir_all(forge.parent().unwrap());

        let neoforge = make_jar(
            "struct-neoforge.jar",
            &[ZipEntry(
                "META-INF/neoforge.mods.toml",
                "[[mods]]\nmodId=\"ok\"\n",
            )],
        );
        assert!(validate_mod_jar_structure(&neoforge).is_ok());
        let _ = std::fs::remove_dir_all(neoforge.parent().unwrap());
    }

    #[test]
    fn structure_rejects_jar_without_mod_metadata() {
        // A jar that only ships a manifest is not a mod the loader can use.
        let jar = make_jar(
            "struct-no-meta.jar",
            &[ZipEntry("META-INF/MANIFEST.MF", "Manifest-Version: 1.0\n")],
        );
        let err = validate_mod_jar_structure(&jar).unwrap_err();
        assert!(err.contains("no mod metadata"), "unhelpful error: {err}");
        let _ = std::fs::remove_dir_all(jar.parent().unwrap());
    }

    #[test]
    fn structure_rejects_non_zip_files() {
        let dir = std::env::temp_dir().join(format!("zircon-struct-notzip-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("fake.jar");
        std::fs::write(&file, b"this is definitely not a zip archive").unwrap();

        let err = validate_mod_jar_structure(&file).unwrap_err();
        assert!(
            err.contains("not a valid ZIP archive"),
            "unhelpful error: {err}"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn structure_rejects_implausible_compression_ratio() {
        // A single entry claiming to be 100 MB uncompressed while only 1 MB is
        // actually stored in the zip (deflated). The header check sees the
        // declared sizes and must reject before anything is extracted.
        let dir = std::env::temp_dir().join(format!("zircon-struct-bomb-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("bomb.jar");
        {
            let f = File::create(&file).unwrap();
            let mut zip = zip::ZipWriter::new(f);
            let options = zip::write::SimpleFileOptions::default()
                .compression_method(zip::CompressionMethod::Deflated);
            zip.start_file("fabric.mod.json", options).unwrap();
            std::io::Write::write_all(&mut zip, b"{\"id\": \"bomb\"}").unwrap();
            zip.finish().unwrap();
        }
        // Patch the central directory's uncompressed size for the single entry
        // to a huge value (100 MB) so the declared ratio exceeds the cap.
        patch_uncompressed_size(&file, 100 * 1024 * 1024);

        let err = validate_mod_jar_structure(&file).unwrap_err();
        assert!(err.contains("compression ratio"), "unhelpful error: {err}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Rewrites the uncompressed-size field (bytes 24..28) of the single
    /// central-directory entry in a zip file. Enough for tests: the entry
    /// header layout is fixed and the CRC stays unchecked by the header pass.
    fn patch_uncompressed_size(file: &std::path::Path, size: u32) {
        let bytes = std::fs::read(file).unwrap();
        // Find the EOCD and walk back to the central directory start.
        let eocd = bytes.windows(4).rposition(|w| w == b"PK\x05\x06").unwrap();
        let cd_size = u32::from_le_bytes(bytes[eocd + 12..eocd + 16].try_into().unwrap());
        let cd_offset = u32::from_le_bytes(bytes[eocd + 16..eocd + 20].try_into().unwrap());
        let cd_start = cd_offset as usize;
        // First entry header: signature(4) version(2+2) flags(2) method(2)
        // time(2) date(2) crc(4) compSize(4) uncompSize(4) -> offset + 24.
        let signature = &bytes[cd_start..cd_start + 4];
        assert_eq!(signature, b"PK\x01\x02", "expected a central directory");
        let mut patched = bytes.clone();
        patched[cd_start + 24..cd_start + 28].copy_from_slice(&size.to_le_bytes());
        std::fs::write(file, patched).unwrap();
        let _ = cd_size;
    }

    #[test]
    fn archive_safety_accepts_jar_without_mod_metadata() {
        let jar = make_jar(
            "safe-no-meta.jar",
            &[ZipEntry("META-INF/MANIFEST.MF", "Manifest-Version: 1.0\n")],
        );
        assert!(validate_archive_safety(&jar).is_ok());
        let _ = std::fs::remove_dir_all(jar.parent().unwrap());
    }

    #[test]
    fn classify_detects_legacy_forge_and_client_jar_and_coremod() {
        let legacy = make_jar(
            "legacy-forge.jar",
            &[ZipEntry(
                "mcmod.info",
                r#"[{"modid": "ironchest", "name": "Iron Chest", "version": "1.12.2-7.0.67.844"}]"#,
            )],
        );
        assert_eq!(
            classify_jar_artifact(&legacy).unwrap(),
            ArtifactClassification::LegacyForge
        );
        assert!(validate_mod_jar_structure(&legacy).is_ok());
        let meta = extract(&legacy).unwrap();
        assert_eq!(meta.id, "ironchest");
        assert_eq!(meta.name, "Iron Chest");
        assert_eq!(meta.version, "1.12.2-7.0.67.844");
        let _ = std::fs::remove_dir_all(legacy.parent().unwrap());

        let client = make_jar(
            "client.jar",
            &[ZipEntry(
                "META-INF/MANIFEST.MF",
                "Manifest-Version: 1.0\nMain-Class: net.minecraft.client.main.Main\n",
            )],
        );
        assert_eq!(
            classify_jar_artifact(&client).unwrap(),
            ArtifactClassification::MinecraftClientJar {
                main_class: "net.minecraft.client.main.Main".to_string()
            }
        );
        let err = validate_mod_jar_structure(&client).unwrap_err();
        assert!(err.contains("file is a Minecraft client JAR"));
        let _ = std::fs::remove_dir_all(client.parent().unwrap());

        let coremod = make_jar(
            "coremod.jar",
            &[ZipEntry(
                "META-INF/MANIFEST.MF",
                "Manifest-Version: 1.0\nFMLCorePlugin: com.example.CorePlugin\n",
            )],
        );
        assert_eq!(
            classify_jar_artifact(&coremod).unwrap(),
            ArtifactClassification::CoremodOrTweaker {
                identifier: "FMLCorePlugin:com.example.CorePlugin".to_string()
            }
        );
        assert!(validate_mod_jar_structure(&coremod).is_ok());
        let _ = std::fs::remove_dir_all(coremod.parent().unwrap());
    }

    #[test]
    fn classify_and_validate_jar_in_jar_and_library_mods() {
        let dir = tempfile::tempdir().unwrap();
        let inner_path = dir.path().join("inner.jar");
        {
            let file = std::fs::File::create(&inner_path).unwrap();
            let mut zip = zip::ZipWriter::new(file);
            let options = zip::write::SimpleFileOptions::default();
            zip.start_file("META-INF/neoforge.mods.toml", options).unwrap();
            std::io::Write::write_all(
                &mut zip,
                b"[[mods]]\nmodId = \"kotlinforforge\"\nversion = \"5.12.0\"\ndisplayName = \"Kotlin For Forge\"\n",
            )
            .unwrap();
            zip.finish().unwrap();
        }
        let inner_bytes = std::fs::read(&inner_path).unwrap();

        let outer_path = dir.path().join("kotlinforforge-5.12.0-all.jar");
        {
            let file = std::fs::File::create(&outer_path).unwrap();
            let mut zip = zip::ZipWriter::new(file);
            let options = zip::write::SimpleFileOptions::default();
            zip.start_file("META-INF/MANIFEST.MF", options).unwrap();
            std::io::Write::write_all(&mut zip, b"Manifest-Version: 1.0\nFMLModType: LIBRARY\n").unwrap();
            zip.start_file("META-INF/jarjar/metadata.json", options).unwrap();
            std::io::Write::write_all(&mut zip, b"{\"jars\": []}\n").unwrap();
            zip.start_file("META-INF/jarjar/kffmod-5.12.0.jar", options).unwrap();
            std::io::Write::write_all(&mut zip, &inner_bytes).unwrap();
            zip.finish().unwrap();
        }

        assert_eq!(
            classify_jar_artifact(&outer_path).unwrap(),
            ArtifactClassification::NeoForge
        );
        assert!(validate_mod_jar_structure(&outer_path).is_ok());

        let meta = extract(&outer_path).unwrap();
        assert_eq!(meta.id, "kotlinforforge");
        assert_eq!(meta.name, "Kotlin For Forge");
        assert_eq!(meta.version, "5.12.0");
    }
