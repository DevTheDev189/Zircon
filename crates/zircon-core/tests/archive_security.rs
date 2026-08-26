//! Comprehensive security & threshold integration tests for archive decompression safeguards.
//!
//! Tests:
//! - Phase 2: Parameterized thresholds (10 GB max uncompressed, 200:1 max ratio, env var overrides)
//! - Phase 3: Safe streaming iterative byte tracking & nested archive recursion bounds
//! - Phase 4: Legitimate mod-pack extraction & malicious zip-bomb rejection suites

use std::fs::{self, File};
use std::io::Write;
use std::path::PathBuf;

use zip::write::SimpleFileOptions;
use zip::ZipWriter;
use zircon_core::archive::limits::{
    ArchiveError, ArchiveGuard, ArchiveLimits, DEFAULT_MAX_COMPRESSION_RATIO,
    DEFAULT_MAX_FILE_ENTRIES, DEFAULT_MAX_RECURSION_DEPTH, DEFAULT_MAX_UNCOMPRESSED_BYTES,
    ENV_ZIP_MAX_COMPRESSION_RATIO, ENV_ZIP_MAX_FILE_ENTRIES, ENV_ZIP_MAX_RECURSION_DEPTH,
    ENV_ZIP_MAX_UNCOMPRESSED_BYTES,
};
use zircon_core::archive::zip::{
    extract_zip, extract_zip_with_options, validate_zip_structure, ZipExtractOptions,
};
use zircon_core::metadata::extractor::validate_mod_jar_structure;

fn temp_test_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("zircon-test-security-{}-{}", name, std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

// ======================================================================
// Phase 2: Thresholds & Parameterization Tests
// ======================================================================

#[test]
fn test_default_constants_and_env_parameterization() {
    assert_eq!(DEFAULT_MAX_UNCOMPRESSED_BYTES, 10 * 1024 * 1024 * 1024); // 10 GB
    assert_eq!(DEFAULT_MAX_COMPRESSION_RATIO, 200); // 200:1
    assert_eq!(DEFAULT_MAX_FILE_ENTRIES, 50_000);
    assert_eq!(DEFAULT_MAX_RECURSION_DEPTH, 3);

    // Verify dynamic override via environment variables
    std::env::set_var(ENV_ZIP_MAX_UNCOMPRESSED_BYTES, "1073741824"); // 1 GB
    std::env::set_var(ENV_ZIP_MAX_COMPRESSION_RATIO, "150");
    std::env::set_var(ENV_ZIP_MAX_FILE_ENTRIES, "5000");
    std::env::set_var(ENV_ZIP_MAX_RECURSION_DEPTH, "4");

    let limits = ArchiveLimits::from_env();
    assert_eq!(limits.max_uncompressed_bytes, 1_073_741_824);
    assert_eq!(limits.max_compression_ratio, 150);
    assert_eq!(limits.max_file_entries, 5_000);
    assert_eq!(limits.max_recursion_depth, 4);

    // Clean up
    std::env::remove_var(ENV_ZIP_MAX_UNCOMPRESSED_BYTES);
    std::env::remove_var(ENV_ZIP_MAX_COMPRESSION_RATIO);
    std::env::remove_var(ENV_ZIP_MAX_FILE_ENTRIES);
    std::env::remove_var(ENV_ZIP_MAX_RECURSION_DEPTH);
}

// ======================================================================
// Phase 3 & 4: Legitimate Mod-Pack Tests
// ======================================================================

#[test]
fn test_legitimate_modpack_assets_with_high_compression_ratio_succeeds() {
    // Mod-packs contain files that naturally compress well (e.g. repeated JSON structures,
    // repetitive lang keys, blockstate definitions). With the 200:1 ratio ceiling,
    // legitimate packs extract cleanly without false positives.
    let dir = temp_test_dir("legit-modpack");
    let zip_path = dir.join("modpack.zip");

    {
        let file = File::create(&zip_path).unwrap();
        let mut zip = ZipWriter::new(file);
        let opts = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);

        // 1. Mod metadata & config
        zip.start_file("modrinth.index.json", opts).unwrap();
        let index_json = r#"{
            "formatVersion": 1,
            "game": "minecraft",
            "versionId": "1.0.0",
            "name": "Super Modpack",
            "files": []
        }"#;
        zip.write_all(index_json.as_bytes()).unwrap();

        // 2. Large repetitive lang / model file (compresses at ~100:1, well within 200:1)
        zip.start_file("assets/mod/lang/en_us.json", opts).unwrap();
        let mut lang_content = String::from("{\n");
        for i in 0..3000 {
            lang_content.push_str(&format!("  \"item.mod.custom_item_{i}\": \"Custom Item Number {i}\",\n"));
        }
        lang_content.push_str("  \"item.mod.end\": \"End\"\n}\n");
        zip.write_all(lang_content.as_bytes()).unwrap();

        // 3. Mod configs and scripts
        zip.start_file("config/jei/jei-client.ini", opts).unwrap();
        zip.write_all(b"cheatItemsEnabled=false\neditModeEnabled=false\n").unwrap();

        zip.start_file("kubejs/server_scripts/recipes.js", opts).unwrap();
        zip.write_all(b"ServerEvents.recipes(event => { event.remove({ output: 'minecraft:stone' }); });").unwrap();

        // 4. Texture dummy data
        zip.start_file("assets/mod/textures/block/stone.png", opts).unwrap();
        zip.write_all(&[0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A]).unwrap();

        zip.finish().unwrap();
    }

    let guard = ArchiveGuard::from_env();
    let dest_dir = dir.join("extracted_pack");
    let file = File::open(&zip_path).unwrap();

    let stats = extract_zip(file, &dest_dir, &guard).unwrap();
    assert_eq!(stats.files_extracted, 5);
    assert!(dest_dir.join("modrinth.index.json").is_file());
    assert!(dest_dir.join("assets/mod/lang/en_us.json").is_file());
    assert!(dest_dir.join("config/jei/jei-client.ini").is_file());
    assert!(dest_dir.join("kubejs/server_scripts/recipes.js").is_file());
    assert!(dest_dir.join("assets/mod/textures/block/stone.png").is_file());

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn test_simulated_large_volume_modpack_header_validation_under_10gb() {
    // Tests that a large pack declaring 8 GB uncompressed uncompressed size (<10 GB limit)
    // passes structural validation without false positive.
    let dir = temp_test_dir("large-header-pass");
    let file_path = dir.join("large_pack.jar");

    {
        let file = File::create(&file_path).unwrap();
        let mut zip = ZipWriter::new(file);
        let opts = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);

        zip.start_file("fabric.mod.json", opts).unwrap();
        zip.write_all(b"{\"id\": \"large_mod\"}").unwrap();

        zip.finish().unwrap();
    }

    // Set custom limits for testing: 8 GB limit, archive is ~100 bytes
    let limits = ArchiveLimits::default().with_max_uncompressed_bytes(8 * 1024 * 1024 * 1024);
    let guard = ArchiveGuard::new(limits);

    let file = File::open(&file_path).unwrap();
    assert!(validate_zip_structure(file, &guard).is_ok());

    let _ = fs::remove_dir_all(&dir);
}

// ======================================================================
// Phase 4: Malicious Zip-Bomb Rejection Tests
// ======================================================================

#[test]
fn test_rejects_high_ratio_zip_bomb() {
    // A zip bomb with a massive repetitive block that compresses to tiny size (ratio > 500:1)
    let dir = temp_test_dir("ratio-bomb");
    let zip_path = dir.join("ratio_bomb.zip");

    {
        let file = File::create(&zip_path).unwrap();
        let mut zip = ZipWriter::new(file);
        let opts = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);

        zip.start_file("bomb.txt", opts).unwrap();
        // 2 MB of zeroes compresses to ~2 KB (ratio ~1000:1, exceeding 200:1 limit)
        let zeroes = vec![0u8; 2 * 1024 * 1024];
        zip.write_all(&zeroes).unwrap();
        zip.finish().unwrap();
    }

    // Default guard allows max 200:1 ratio
    let guard = ArchiveGuard::from_env();
    let dest_dir = dir.join("out");
    let file = File::open(&zip_path).unwrap();

    let err = extract_zip(file, &dest_dir, &guard).unwrap_err();
    assert!(
        matches!(err, ArchiveError::ExceededMaxRatio { .. }),
        "Expected ExceededMaxRatio, got: {err:?}"
    );

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn test_rejects_total_uncompressed_size_exceeding_limit() {
    // Test that an archive exceeding the max allowed uncompressed size is rejected.
    let dir = temp_test_dir("size-bomb");
    let zip_path = dir.join("size_bomb.zip");

    {
        let file = File::create(&zip_path).unwrap();
        let mut zip = ZipWriter::new(file);
        let opts = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);

        zip.start_file("file1.bin", opts).unwrap();
        zip.write_all(&vec![1u8; 500 * 1024]).unwrap(); // 500 KB

        zip.start_file("file2.bin", opts).unwrap();
        zip.write_all(&vec![2u8; 600 * 1024]).unwrap(); // 600 KB
        zip.finish().unwrap();
    }

    // Set custom max bytes limit to 1 MB (1,048,576 bytes); total archive is 1.1 MB
    let limits = ArchiveLimits::default().with_max_uncompressed_bytes(1024 * 1024);
    let guard = ArchiveGuard::new(limits);
    let dest_dir = dir.join("out");
    let file = File::open(&zip_path).unwrap();

    let err = extract_zip(file, &dest_dir, &guard).unwrap_err();
    assert!(
        matches!(err, ArchiveError::ExceededMaxBytes { .. }),
        "Expected ExceededMaxBytes, got: {err:?}"
    );

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn test_rejects_nested_recursive_zip_bomb() {
    // Test recursive nested archive attacks (zip containing zip containing zip ...)
    let dir = temp_test_dir("nested-bomb");
    let l1_path = dir.join("level1.zip");
    let l2_path = dir.join("level2.zip");
    let l3_path = dir.join("level3.zip");
    let l4_path = dir.join("level4.zip");

    let opts = SimpleFileOptions::default();

    // Create level 4
    {
        let mut zip = ZipWriter::new(File::create(&l4_path).unwrap());
        zip.start_file("payload.txt", opts).unwrap();
        zip.write_all(b"payload").unwrap();
        zip.finish().unwrap();
    }

    // Create level 3 containing level 4
    {
        let mut zip = ZipWriter::new(File::create(&l3_path).unwrap());
        zip.start_file("nested/level4.zip", opts).unwrap();
        zip.write_all(&fs::read(&l4_path).unwrap()).unwrap();
        zip.finish().unwrap();
    }

    // Create level 2 containing level 3
    {
        let mut zip = ZipWriter::new(File::create(&l2_path).unwrap());
        zip.start_file("nested/level3.zip", opts).unwrap();
        zip.write_all(&fs::read(&l3_path).unwrap()).unwrap();
        zip.finish().unwrap();
    }

    // Create level 1 containing level 2
    {
        let mut zip = ZipWriter::new(File::create(&l1_path).unwrap());
        zip.start_file("nested/level2.zip", opts).unwrap();
        zip.write_all(&fs::read(&l2_path).unwrap()).unwrap();
        zip.finish().unwrap();
    }

    // Max recursion depth = 2 (Level 1 -> Level 2 -> Level 3 will exceed max depth 2)
    let limits = ArchiveLimits::default().with_max_recursion_depth(2);
    let guard = ArchiveGuard::new(limits);
    let dest_dir = dir.join("out");
    let file = File::open(&l1_path).unwrap();

    let err = extract_zip_with_options(
        file,
        &dest_dir,
        &guard,
        ZipExtractOptions {
            extract_nested: true,
            overwrite: true,
        },
    )
    .unwrap_err();

    assert!(
        matches!(err, ArchiveError::ExceededMaxRecursionDepth { .. }),
        "Expected ExceededMaxRecursionDepth, got: {err:?}"
    );

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn test_rejects_zip_slip_path_traversal() {
    let dir = temp_test_dir("zip-slip-attack");
    let zip_path = dir.join("slip.zip");

    {
        let mut zip = ZipWriter::new(File::create(&zip_path).unwrap());
        let opts = SimpleFileOptions::default();
        zip.start_file("../../etc/passwd", opts).unwrap();
        zip.write_all(b"root:x:0:0:root:/root:/bin/bash").unwrap();
        zip.finish().unwrap();
    }

    let guard = ArchiveGuard::from_env();
    let dest_dir = dir.join("out");
    let file = File::open(&zip_path).unwrap();

    let err = extract_zip(file, &dest_dir, &guard).unwrap_err();
    assert!(
        matches!(err, ArchiveError::ZipSlip(_)),
        "Expected ZipSlip error, got: {err:?}"
    );

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn test_mod_jar_structure_validation_security_suite() {
    let dir = temp_test_dir("mod-jar-struct");

    // 1. Valid Fabric Mod Jar
    let valid_jar = dir.join("valid.jar");
    {
        let mut zip = ZipWriter::new(File::create(&valid_jar).unwrap());
        let opts = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
        zip.start_file("fabric.mod.json", opts).unwrap();
        zip.write_all(b"{\"id\": \"valid_mod\", \"version\": \"1.0.0\"}").unwrap();
        zip.finish().unwrap();
    }
    assert!(validate_mod_jar_structure(&valid_jar).is_ok());

    // 2. Mod Jar exceeding ratio ceiling
    let bomb_jar = dir.join("bomb.jar");
    {
        let mut zip = ZipWriter::new(File::create(&bomb_jar).unwrap());
        let opts = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
        zip.start_file("fabric.mod.json", opts).unwrap();
        // 1 MB of zeroes deflates to ~1 KB (> 1000:1 ratio)
        zip.write_all(&vec![0u8; 1024 * 1024]).unwrap();
        zip.finish().unwrap();
    }
    let err = validate_mod_jar_structure(&bomb_jar).unwrap_err();
    assert!(err.contains("implausible compression ratio"));

    let _ = fs::remove_dir_all(&dir);
}
