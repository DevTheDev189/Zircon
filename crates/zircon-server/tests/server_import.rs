//! Integration tests for server import pipeline: ZIP upload, NBT inspection,
//! Bukkit dimension conversion, downgrade enforcer, and instance BOM generation.

use std::fs::File;
use std::io::Write;
use std::sync::Arc;

use flate2::write::GzEncoder;
use flate2::Compression;
use zip::write::SimpleFileOptions;
use zip::ZipWriter;

use zircon_server::instance::ServerInstanceManager;
use zircon_server::process::console::ConsoleStreamHandler;
use zircon_server::services::import::{ImportCommitRequest, ServerImportService};

fn build_synthetic_level_dat(data_version: i32, level_name: &str, seed: i64) -> Vec<u8> {
    let mut raw = Vec::new();
    raw.push(10); // TAG_Compound
    raw.extend_from_slice(&(0u16).to_be_bytes());

    raw.push(10); // TAG_Compound
    let data_str = "Data";
    raw.extend_from_slice(&(data_str.len() as u16).to_be_bytes());
    raw.extend_from_slice(data_str.as_bytes());

    raw.push(3); // TAG_Int
    let dv_str = "DataVersion";
    raw.extend_from_slice(&(dv_str.len() as u16).to_be_bytes());
    raw.extend_from_slice(dv_str.as_bytes());
    raw.extend_from_slice(&data_version.to_be_bytes());

    raw.push(8); // TAG_String
    let ln_str = "LevelName";
    raw.extend_from_slice(&(ln_str.len() as u16).to_be_bytes());
    raw.extend_from_slice(ln_str.as_bytes());
    raw.extend_from_slice(&(level_name.len() as u16).to_be_bytes());
    raw.extend_from_slice(level_name.as_bytes());

    raw.push(4); // TAG_Long
    let seed_str = "RandomSeed";
    raw.extend_from_slice(&(seed_str.len() as u16).to_be_bytes());
    raw.extend_from_slice(seed_str.as_bytes());
    raw.extend_from_slice(&seed.to_be_bytes());

    raw.push(0); // End "Data"
    raw.push(0); // End root

    let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
    encoder.write_all(&raw).unwrap();
    encoder.finish().unwrap()
}

fn create_test_server_zip(zip_path: &std::path::Path) {
    let file = File::create(zip_path).unwrap();
    let mut zip = ZipWriter::new(file);
    let options = SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Stored);

    // server.properties
    zip.start_file("server.properties", options).unwrap();
    zip.write_all(b"motd=Awesome Imported Server\nlevel-name=world\nserver-port=25565\n").unwrap();

    // world/level.dat (MC 1.21.1, DataVersion 3955)
    let level_dat_bytes = build_synthetic_level_dat(3955, "ImportedWorld", 987654321);
    zip.start_file("world/level.dat", options).unwrap();
    zip.write_all(&level_dat_bytes).unwrap();

    // Overworld region
    zip.start_file("world/region/r.0.0.mca", options).unwrap();
    zip.write_all(b"chunk data").unwrap();

    // Player data
    zip.start_file("world/playerdata/00000000-0000-0000-0000-000000000001.dat", options).unwrap();
    zip.write_all(b"player inventory").unwrap();

    // Bukkit Nether layout
    zip.start_file("world_nether/region/r.-1.-1.mca", options).unwrap();
    zip.write_all(b"nether chunk data").unwrap();

    // Config & Whitelist
    zip.start_file("whitelist.json", options).unwrap();
    zip.write_all(b"[]").unwrap();

    zip.start_file("config/test_mod.json", options).unwrap();
    zip.write_all(b"{\"enabled\": true}").unwrap();

    zip.finish().unwrap();
}

#[tokio::test]
async fn test_import_pipeline_end_to_end() {
    let temp_dir = tempfile::tempdir().unwrap();
    let data_dir = temp_dir.path();

    let console = Arc::new(ConsoleStreamHandler::new());
    let instances = Arc::new(ServerInstanceManager::new(data_dir, console).unwrap());
    let import_service = ServerImportService::new(data_dir, instances.clone(), None).unwrap();

    let zip_path = data_dir.join("test_server.zip");
    create_test_server_zip(&zip_path);

    // 1. Stage and Analyze
    let report = import_service.stage_and_analyze(&zip_path).expect("stage and analyze");
    assert_eq!(report.suggested_name, "Awesome Imported Server");
    assert_eq!(report.minecraft_version.as_deref(), Some("1.21.1"));
    assert_eq!(report.data_version, Some(3955));
    assert!(report.bukkit_dimensions_detected);
    assert!(report.server_properties_found);

    let world = report.world.expect("world summary");
    assert_eq!(world.overworld_chunks, 1);
    assert_eq!(world.nether_chunks, 1);
    assert_eq!(world.player_count, 1);

    // 2. Commit import
    let commit_req = ImportCommitRequest {
        import_id: report.import_id,
        name: Some("My Imported Server".to_string()),
        mc_version: Some("1.21.1".to_string()),
        loader_type: Some("fabric".to_string()),
        loader_version: None,
        java_args: None,
        external_port: None,
        convert_dimensions: Some(true),
    };

    let instance = import_service.commit_import(commit_req).expect("commit import");
    assert_eq!(instance.name, "My Imported Server");
    assert_eq!(instance.minecraft_version, "1.21.1");
    assert_eq!(instance.loader_type(), "fabric");

    // 3. Verify directory contents & dimension normalization
    let inst_dir = instances.get_instance_dir(&instance.id);
    let server_dir = inst_dir.join("server");

    assert!(server_dir.join("server.properties").is_file());
    assert!(server_dir.join("whitelist.json").is_file());
    assert!(server_dir.join("config").join("test_mod.json").is_file());

    let world_dir = server_dir.join("world");
    assert!(world_dir.join("level.dat").is_file());
    assert!(world_dir.join("region").join("r.0.0.mca").is_file());
    assert!(world_dir.join("playerdata").join("00000000-0000-0000-0000-000000000001.dat").is_file());

    // Bukkit Nether converted to world/DIM-1/region
    assert!(world_dir.join("DIM-1").join("region").join("r.-1.-1.mca").is_file());

    // BOM file generated
    assert!(inst_dir.join("bom.json").is_file());
}

#[tokio::test]
async fn test_import_downgrade_rejection() {
    let temp_dir = tempfile::tempdir().unwrap();
    let data_dir = temp_dir.path();

    let console = Arc::new(ConsoleStreamHandler::new());
    let instances = Arc::new(ServerInstanceManager::new(data_dir, console).unwrap());
    let import_service = ServerImportService::new(data_dir, instances.clone(), None).unwrap();

    let zip_path = data_dir.join("test_downgrade_server.zip");
    create_test_server_zip(&zip_path);

    let report = import_service.stage_and_analyze(&zip_path).expect("stage and analyze");

    // Attempting to downgrade a 1.21.1 world (DataVersion 3955) to 1.20.4 (DataVersion 3465) must fail
    let commit_req = ImportCommitRequest {
        import_id: report.import_id,
        name: Some("Downgraded Server".to_string()),
        mc_version: Some("1.20.4".to_string()),
        loader_type: Some("fabric".to_string()),
        loader_version: None,
        java_args: None,
        external_port: None,
        convert_dimensions: Some(true),
    };

    let result = import_service.commit_import(commit_req);
    assert!(result.is_err(), "Downgrade must be rejected");
    let err_msg = result.unwrap_err().to_string();
    assert!(err_msg.contains("Downgrade detected"), "Error should explain downgrade: {err_msg}");
}

#[tokio::test]
async fn test_import_to_modern_26_unified_layout() {
    let temp_dir = tempfile::tempdir().unwrap();
    let data_dir = temp_dir.path();

    let console = Arc::new(ConsoleStreamHandler::new());
    let instances = Arc::new(ServerInstanceManager::new(data_dir, console).unwrap());
    let import_service = ServerImportService::new(data_dir, instances.clone(), None).unwrap();

    let zip_path = data_dir.join("test_server_to_26.zip");
    create_test_server_zip(&zip_path);

    let report = import_service.stage_and_analyze(&zip_path).expect("stage and analyze");
    assert!(report.migration_notice.is_some());

    let commit_req = ImportCommitRequest {
        import_id: report.import_id,
        name: Some("26.1.2 Modern Server".to_string()),
        mc_version: Some("26.1.2".to_string()),
        loader_type: Some("fabric".to_string()),
        loader_version: None,
        java_args: None,
        external_port: None,
        convert_dimensions: Some(true),
    };

    let instance = import_service.commit_import(commit_req).expect("commit import to 26.1.2");
    assert_eq!(instance.minecraft_version, "26.1.2");

    let inst_dir = instances.get_instance_dir(&instance.id);
    let world_dir = inst_dir.join("server").join("world");

    // Unified 26.x directory hierarchy
    assert!(world_dir.join("dimensions").join("minecraft").join("overworld").join("region").join("r.0.0.mca").is_file());
    assert!(world_dir.join("dimensions").join("minecraft").join("the_nether").join("region").join("r.-1.-1.mca").is_file());
    assert!(world_dir.join("players").join("data").join("00000000-0000-0000-0000-000000000001.dat").is_file());

    // Legacy directories migrated away
    assert!(!world_dir.join("region").exists());
    assert!(!world_dir.join("playerdata").exists());
}

