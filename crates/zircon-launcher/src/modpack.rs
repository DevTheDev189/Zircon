//! Modpack installation pipeline for Modrinth (`.mrpack`) archives.
//!
//! Parses `modrinth.index.json`, extracts `overrides/` and `client-overrides/` directly
//! into the offline instance directory with zip-slip protection, filters client-compatible
//! files, and downloads them verifying SHA-1 and SHA-512 cryptographic checksums.

use std::collections::HashMap;
use std::io::{Cursor, Read};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use sha1::Digest;
use tauri::{AppHandle, Emitter};

use crate::error::LauncherError;
use crate::offline::{OfflineInstance, OfflineInstanceManager};

/// Modrinth index file manifest schema (formatVersion = 1).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModrinthIndex {
    pub format_version: u32,
    pub game: String,
    pub version_id: String,
    pub name: String,
    pub summary: Option<String>,
    pub files: Vec<ModrinthIndexFile>,
    pub dependencies: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModrinthIndexFile {
    pub path: String,
    pub hashes: HashMap<String, String>,
    #[serde(default)]
    pub env: Option<ModrinthIndexEnv>,
    pub downloads: Vec<String>,
    #[serde(default)]
    pub file_size: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ModrinthIndexEnv {
    #[serde(default)]
    pub client: Option<String>,
    #[serde(default)]
    pub server: Option<String>,
}

/// Information extracted from a Modrinth index for instance creation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModpackMetadata {
    pub name: String,
    pub mc_version: String,
    pub loader_type: String,
    pub loader_version: String,
}

/// Event payload emitted during modpack installation.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModpackProgressEvent {
    pub phase: String,
    pub current: usize,
    pub total: usize,
    pub fraction: f64,
    pub message: String,
}

/// Parses the `modrinth.index.json` from raw `.mrpack` archive bytes.
pub fn parse_modrinth_index(bytes: &[u8]) -> Result<ModrinthIndex, LauncherError> {
    let reader = Cursor::new(bytes);
    let mut zip = zip::ZipArchive::new(reader)
        .map_err(|e| LauncherError::InvalidInput(format!("Invalid .mrpack archive: {e}")))?;

    let mut index_entry = zip.by_name("modrinth.index.json").map_err(|_| {
        LauncherError::InvalidInput("Missing modrinth.index.json in modpack".to_string())
    })?;

    let mut content = String::new();
    index_entry
        .read_to_string(&mut content)
        .map_err(|e| LauncherError::InvalidInput(format!("Failed to read modrinth.index.json: {e}")))?;

    let index: ModrinthIndex = serde_json::from_str(&content)
        .map_err(|e| LauncherError::Parse(format!("Malformed modrinth.index.json: {e}")))?;

    if index.game != "minecraft" {
        return Err(LauncherError::InvalidInput(format!(
            "Unsupported game '{}' in modpack (expected 'minecraft')",
            index.game
        )));
    }

    Ok(index)
}

/// Resolves Minecraft version and mod loader metadata from the index dependencies.
pub fn extract_modpack_metadata(index: &ModrinthIndex, custom_name: Option<&str>) -> ModpackMetadata {
    let name = match custom_name {
        Some(n) if !n.trim().is_empty() => n.trim().to_string(),
        _ => index.name.clone(),
    };

    let mc_version = index
        .dependencies
        .get("minecraft")
        .cloned()
        .unwrap_or_else(|| "1.20.4".to_string());

    let mut loader_type = "fabric".to_string();
    let mut loader_version = String::new();

    if let Some(v) = index.dependencies.get("fabric-loader") {
        loader_type = "fabric".to_string();
        loader_version = v.clone();
    } else if let Some(v) = index.dependencies.get("quilt-loader") {
        loader_type = "quilt".to_string();
        loader_version = v.clone();
    } else if let Some(v) = index.dependencies.get("neoforge") {
        loader_type = "neoforge".to_string();
        loader_version = v.clone();
    } else if let Some(v) = index.dependencies.get("forge") {
        loader_type = "forge".to_string();
        loader_version = v.clone();
    }

    ModpackMetadata {
        name,
        mc_version,
        loader_type,
        loader_version,
    }
}

/// Checks whether a file path in a modpack is safe and relative (prevents zip-slip).
pub fn sanitize_rel_path(path: &str) -> Result<PathBuf, LauncherError> {
    let trimmed = path.trim().replace('\\', "/");
    if trimmed.is_empty()
        || trimmed.starts_with('/')
        || trimmed.contains("../")
        || trimmed.ends_with("/..")
        || trimmed == ".."
    {
        return Err(LauncherError::InvalidInput(format!(
            "Illegal path traversal attempt in modpack entry: {path}"
        )));
    }

    let p = PathBuf::from(trimmed);
    for comp in p.components() {
        match comp {
            std::path::Component::Normal(_) => {}
            _ => {
                return Err(LauncherError::InvalidInput(format!(
                    "Unsafe path component in modpack entry: {path}"
                )));
            }
        }
    }
    Ok(p)
}

/// Extracts `overrides/` and `client-overrides/` from the `.mrpack` archive into `dest_dir`.
pub fn extract_mrpack_overrides(bytes: &[u8], dest_dir: &Path) -> Result<usize, LauncherError> {
    let reader = Cursor::new(bytes);
    let mut zip = zip::ZipArchive::new(reader)
        .map_err(|e| LauncherError::InvalidInput(format!("Invalid archive: {e}")))?;

    let mut count = 0;
    for i in 0..zip.len() {
        let mut entry = zip
            .by_index(i)
            .map_err(|e| LauncherError::InvalidInput(format!("ZIP entry read error: {e}")))?;

        let entry_name = entry.name().replace('\\', "/");
        let rel_target = if let Some(stripped) = entry_name.strip_prefix("overrides/") {
            stripped
        } else if let Some(stripped) = entry_name.strip_prefix("client-overrides/") {
            stripped
        } else {
            continue;
        };

        if rel_target.trim().is_empty() {
            continue;
        }

        let safe_rel = sanitize_rel_path(rel_target)?;
        let target_path = dest_dir.join(safe_rel);

        if entry.is_dir() {
            std::fs::create_dir_all(&target_path)?;
        } else {
            if let Some(parent) = target_path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            let mut outfile = std::fs::File::create(&target_path)?;
            std::io::copy(&mut entry, &mut outfile)?;
            count += 1;
        }
    }

    Ok(count)
}

/// Returns true if this file should be downloaded for client gameplay.
pub fn is_client_file(file: &ModrinthIndexFile) -> bool {
    if let Some(env) = &file.env {
        if let Some(client) = &env.client {
            if client.eq_ignore_ascii_case("unsupported") {
                return false;
            }
        }
    }
    true
}

/// Verifies whether the downloaded data matches the declared SHA-1 or SHA-512 hashes.
pub fn verify_file_hashes(data: &[u8], hashes: &HashMap<String, String>) -> Result<(), LauncherError> {
    if let Some(expected_sha1) = hashes.get("sha1") {
        let mut hasher = sha1::Sha1::new();
        hasher.update(data);
        let actual_sha1 = hex::encode(hasher.finalize());
        if !actual_sha1.eq_ignore_ascii_case(expected_sha1) {
            return Err(LauncherError::Security(format!(
                "SHA-1 mismatch: expected {expected_sha1}, got {actual_sha1}"
            )));
        }
    } else if let Some(expected_sha512) = hashes.get("sha512") {
        use sha2::Digest as _;
        let mut hasher = sha2::Sha512::new();
        hasher.update(data);
        let actual_sha512 = hex::encode(hasher.finalize());
        if !actual_sha512.eq_ignore_ascii_case(expected_sha512) {
            return Err(LauncherError::Security(format!(
                "SHA-512 mismatch: expected {expected_sha512}, got {actual_sha512}"
            )));
        }
    }
    Ok(())
}

/// Full modpack install flow: creates the instance, extracts overrides, and downloads files.
pub async fn install_modpack(
    app: &AppHandle,
    manager: &OfflineInstanceManager,
    http: &reqwest::Client,
    mrpack_bytes: &[u8],
    custom_name: Option<&str>,
) -> Result<OfflineInstance, LauncherError> {
    emit_progress(app, "Reading modpack manifest...", 0, 1, 0.05, "Parsing modrinth.index.json");
    let index = parse_modrinth_index(mrpack_bytes)?;
    let meta = extract_modpack_metadata(&index, custom_name);

    emit_progress(
        app,
        "Creating instance...",
        0,
        1,
        0.10,
        &format!("Minecraft {} ({})", meta.mc_version, meta.loader_type),
    );
    let instance = manager.create(
        &meta.name,
        &meta.mc_version,
        &meta.loader_type,
        &meta.loader_version,
    )?;
    let instance_dir = manager.instance_dir(&instance.id);

    emit_progress(app, "Extracting configs and assets...", 0, 1, 0.15, "Applying overrides");
    extract_mrpack_overrides(mrpack_bytes, &instance_dir)?;

    let client_files: Vec<&ModrinthIndexFile> = index.files.iter().filter(|f| is_client_file(f)).collect();
    let total_files = client_files.len();

    emit_progress(
        app,
        "Downloading mods & assets...",
        0,
        total_files,
        0.20,
        &format!("0 / {total_files} files"),
    );

    for (idx, file) in client_files.into_iter().enumerate() {
        let safe_rel = sanitize_rel_path(&file.path)?;
        let dest = instance_dir.join(safe_rel);

        let Some(download_url) = file.downloads.first() else {
            continue;
        };

        let fraction = 0.20 + (0.75 * (idx as f64 / total_files.max(1) as f64));
        emit_progress(
            app,
            "Downloading mods & assets...",
            idx + 1,
            total_files,
            fraction,
            &format!("{}/{} - {}", idx + 1, total_files, file.path),
        );

        if !dest.is_file() {
            if let Some(parent) = dest.parent() {
                tokio::fs::create_dir_all(parent).await?;
            }
            let resp = http
                .get(download_url)
                .send()
                .await?;
            let bytes = resp
                .bytes()
                .await?;

            verify_file_hashes(&bytes, &file.hashes)?;
            tokio::fs::write(&dest, bytes).await?;
        }
    }

    emit_progress(app, "Installation complete!", total_files, total_files, 1.0, "Ready to launch");
    Ok(instance)
}

/// CurseForge manifest schema (formatVersion = 1).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CurseForgeManifest {
    pub minecraft: CurseForgeMinecraft,
    #[serde(default)]
    pub manifest_type: Option<String>,
    #[serde(default)]
    pub manifest_version: Option<u32>,
    pub name: String,
    #[serde(default)]
    pub version: Option<String>,
    #[serde(default)]
    pub author: Option<String>,
    #[serde(default)]
    pub files: Vec<CurseForgeFileRef>,
    #[serde(default = "default_overrides")]
    pub overrides: String,
}

fn default_overrides() -> String {
    "overrides".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CurseForgeMinecraft {
    pub version: String,
    #[serde(default)]
    pub mod_loaders: Vec<CurseForgeModLoaderRef>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CurseForgeModLoaderRef {
    pub id: String,
    #[serde(default)]
    pub primary: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CurseForgeFileRef {
    #[serde(rename = "projectID")]
    pub project_id: u32,
    #[serde(rename = "fileID")]
    pub file_id: u32,
    #[serde(default = "default_required")]
    pub required: bool,
}

fn default_required() -> bool {
    true
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModpackArchiveType {
    Modrinth,
    CurseForge,
}

pub fn detect_archive_type(bytes: &[u8]) -> Result<ModpackArchiveType, LauncherError> {
    let reader = Cursor::new(bytes);
    let mut zip = zip::ZipArchive::new(reader)
        .map_err(|e| LauncherError::InvalidInput(format!("Invalid zip archive: {e}")))?;

    if zip.by_name("modrinth.index.json").is_ok() {
        return Ok(ModpackArchiveType::Modrinth);
    }
    if zip.by_name("manifest.json").is_ok() {
        return Ok(ModpackArchiveType::CurseForge);
    }
    Err(LauncherError::InvalidInput(
        "Unsupported modpack archive: neither modrinth.index.json nor manifest.json found".to_string(),
    ))
}

pub fn parse_curseforge_manifest(bytes: &[u8]) -> Result<CurseForgeManifest, LauncherError> {
    let reader = Cursor::new(bytes);
    let mut zip = zip::ZipArchive::new(reader)
        .map_err(|e| LauncherError::InvalidInput(format!("Invalid CurseForge archive: {e}")))?;

    let mut entry = zip.by_name("manifest.json").map_err(|_| {
        LauncherError::InvalidInput("Missing manifest.json in CurseForge modpack".to_string())
    })?;

    let mut content = String::new();
    entry
        .read_to_string(&mut content)
        .map_err(|e| LauncherError::InvalidInput(format!("Failed to read manifest.json: {e}")))?;

    let manifest: CurseForgeManifest = serde_json::from_str(&content)
        .map_err(|e| LauncherError::Parse(format!("Malformed manifest.json: {e}")))?;

    Ok(manifest)
}

pub fn extract_curseforge_metadata(manifest: &CurseForgeManifest, custom_name: Option<&str>) -> ModpackMetadata {
    let name = match custom_name {
        Some(n) if !n.trim().is_empty() => n.trim().to_string(),
        _ => manifest.name.clone(),
    };

    let mc_version = manifest.minecraft.version.clone();

    let mut loader_type = "forge".to_string();
    let mut loader_version = String::new();

    let primary_loader = manifest
        .minecraft
        .mod_loaders
        .iter()
        .find(|l| l.primary)
        .or_else(|| manifest.minecraft.mod_loaders.first());

    if let Some(l) = primary_loader {
        let id_lower = l.id.to_ascii_lowercase();
        if let Some((loader, ver)) = id_lower.split_once('-') {
            loader_type = match loader {
                "fabric" => "fabric".to_string(),
                "quilt" => "quilt".to_string(),
                "neoforge" => "neoforge".to_string(),
                _ => "forge".to_string(),
            };
            loader_version = ver.to_string();
        } else {
            loader_version = l.id.clone();
        }
    }

    ModpackMetadata {
        name,
        mc_version,
        loader_type,
        loader_version,
    }
}

pub fn extract_curseforge_overrides(
    zip_bytes: &[u8],
    overrides_dir_name: &str,
    target_dir: &Path,
) -> Result<(), LauncherError> {
    let reader = Cursor::new(zip_bytes);
    let mut zip = zip::ZipArchive::new(reader)
        .map_err(|e| LauncherError::InvalidInput(format!("Invalid zip archive: {e}")))?;

    let prefix = format!("{}/", overrides_dir_name.trim_matches('/'));

    for i in 0..zip.len() {
        let mut file = zip.by_index(i).map_err(|e| {
            LauncherError::InvalidInput(format!("Failed reading zip index {i}: {e}"))
        })?;

        let name = file.name().to_string();
        if !name.starts_with(&prefix) {
            continue;
        }

        let rel_path = match name.strip_prefix(&prefix) {
            Some(p) if !p.trim().is_empty() => p,
            _ => continue,
        };

        let safe_rel = sanitize_rel_path(rel_path)?;
        let out_path = target_dir.join(safe_rel);

        if file.is_dir() {
            std::fs::create_dir_all(&out_path)?;
        } else {
            if let Some(parent) = out_path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            let mut out_file = std::fs::File::create(&out_path)?;
            std::io::copy(&mut file, &mut out_file)?;
        }
    }

    Ok(())
}

pub async fn install_curseforge_modpack(
    app: &AppHandle,
    manager: &OfflineInstanceManager,
    curse_forge: &zircon_core::api::curseforge::CurseForgeApiClient,
    http: &reqwest::Client,
    zip_bytes: &[u8],
    custom_name: Option<&str>,
) -> Result<OfflineInstance, LauncherError> {
    emit_progress(app, "Reading CurseForge manifest...", 0, 1, 0.05, "Parsing manifest.json");
    let manifest = parse_curseforge_manifest(zip_bytes)?;
    let meta = extract_curseforge_metadata(&manifest, custom_name);

    emit_progress(
        app,
        "Creating instance...",
        0,
        1,
        0.10,
        &format!("Creating {} ({})", meta.name, meta.mc_version),
    );

    let instance = manager.create(
        &meta.name,
        &meta.mc_version,
        &meta.loader_type,
        &meta.loader_version,
    )?;
    let instance_dir = manager.instance_dir(&instance.id);
    let mods_dir = instance_dir.join("mods");
    tokio::fs::create_dir_all(&mods_dir).await?;

    emit_progress(app, "Extracting configs and assets...", 0, 1, 0.15, "Applying overrides");
    extract_curseforge_overrides(zip_bytes, &manifest.overrides, &instance_dir)?;

    let total_files = manifest.files.len();
    emit_progress(
        app,
        "Resolving mod files from CurseForge...",
        0,
        total_files,
        0.20,
        &format!("0 / {total_files} mods"),
    );

    // Batch resolve CurseForge files (in chunks of 50)
    let mut resolved_files = Vec::new();
    for chunk in manifest.files.chunks(50) {
        let file_ids: Vec<i64> = chunk.iter().map(|f| f.file_id as i64).collect();
        if let Ok(files) = curse_forge.get_files(&file_ids).await {
            resolved_files.extend(files);
        } else {
            // Fallback to individual file fetching
            for file_ref in chunk {
                if let Ok(file) = curse_forge.get_mod_file(file_ref.project_id as i64, file_ref.file_id as i64).await {
                    resolved_files.push(file);
                }
            }
        }
    }

    for (idx, file) in resolved_files.into_iter().enumerate() {
        let dest = mods_dir.join(&file.file_name);
        let fraction = 0.20 + (0.75 * (idx as f64 / total_files.max(1) as f64));
        emit_progress(
            app,
            "Downloading mods...",
            idx + 1,
            total_files,
            fraction,
            &format!("{}/{} - {}", idx + 1, total_files, file.file_name),
        );

        if !dest.is_file() {
            let download_url = if !file.download_url.is_empty() {
                file.download_url
            } else {
                let p1 = file.id / 1000;
                let p2 = file.id % 1000;
                format!("https://edge.forgecdn.net/files/{p1}/{p2}/{}", file.file_name)
            };

            if let Ok(resp) = http.get(&download_url).send().await {
                if resp.status().is_success() {
                    if let Ok(bytes) = resp.bytes().await {
                        let _ = tokio::fs::write(&dest, bytes).await;
                    }
                }
            }
        }
    }

    emit_progress(app, "Installation complete!", total_files, total_files, 1.0, "Ready to launch");
    Ok(instance)
}

pub async fn install_modpack_archive(
    app: &AppHandle,
    manager: &OfflineInstanceManager,
    curse_forge: &zircon_core::api::curseforge::CurseForgeApiClient,
    http: &reqwest::Client,
    bytes: &[u8],
    custom_name: Option<&str>,
) -> Result<OfflineInstance, LauncherError> {
    match detect_archive_type(bytes)? {
        ModpackArchiveType::Modrinth => {
            install_modpack(app, manager, http, bytes, custom_name).await
        }

        ModpackArchiveType::CurseForge => {
            install_curseforge_modpack(app, manager, curse_forge, http, bytes, custom_name).await
        }
    }
}



fn emit_progress(
    app: &AppHandle,
    phase: &str,
    current: usize,
    total: usize,
    fraction: f64,
    message: &str,
) {
    let _ = app.emit(
        "modpack-progress",
        ModpackProgressEvent {
            phase: phase.to_string(),
            current,
            total,
            fraction: fraction.clamp(0.0, 1.0),
            message: message.to_string(),
        },
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_valid_modrinth_index() {
        let manifest = r#"{
            "formatVersion": 1,
            "game": "minecraft",
            "versionId": "1.0.0",
            "name": "Test Pack",
            "summary": "A test modpack",
            "files": [
                {
                    "path": "mods/example.jar",
                    "hashes": {
                        "sha1": "2fd4e1c67a2d28fced849ee1bb76e7391b93eb12"
                    },
                    "env": {
                        "client": "required",
                        "server": "required"
                    },
                    "downloads": ["https://cdn.example.com/example.jar"],
                    "fileSize": 1024
                }
            ],
            "dependencies": {
                "minecraft": "1.21.1",
                "fabric-loader": "0.16.5"
            }
        }"#;

        let mut buf = Vec::new();
        {
            let mut zip = zip::ZipWriter::new(Cursor::new(&mut buf));
            let opts = zip::write::SimpleFileOptions::default();
            zip.start_file("modrinth.index.json", opts).unwrap();
            std::io::Write::write_all(&mut zip, manifest.as_bytes()).unwrap();
            zip.finish().unwrap();
        }

        let index = parse_modrinth_index(&buf).unwrap();
        assert_eq!("Test Pack", index.name);
        assert_eq!("1.21.1", index.dependencies["minecraft"]);
        assert_eq!("0.16.5", index.dependencies["fabric-loader"]);

        let meta = extract_modpack_metadata(&index, None);
        assert_eq!("Test Pack", meta.name);
        assert_eq!("1.21.1", meta.mc_version);
        assert_eq!("fabric", meta.loader_type);
        assert_eq!("0.16.5", meta.loader_version);
    }

    #[test]
    fn sanitize_rel_path_blocks_traversal() {
        assert!(sanitize_rel_path("mods/test.jar").is_ok());
        assert!(sanitize_rel_path("config/sub/file.toml").is_ok());

        assert!(sanitize_rel_path("/absolute/path").is_err());
        assert!(sanitize_rel_path("../escape.jar").is_err());
        assert!(sanitize_rel_path("mods/../../escape.jar").is_err());
        assert!(sanitize_rel_path("..").is_err());
    }

    #[test]
    fn client_file_filtering() {
        let client_file = ModrinthIndexFile {
            path: "mods/client.jar".to_string(),
            hashes: HashMap::new(),
            env: Some(ModrinthIndexEnv {
                client: Some("required".to_string()),
                server: Some("unsupported".to_string()),
            }),
            downloads: vec![],
            file_size: 10,
        };
        assert!(is_client_file(&client_file));

        let server_only_file = ModrinthIndexFile {
            path: "mods/server.jar".to_string(),
            hashes: HashMap::new(),
            env: Some(ModrinthIndexEnv {
                client: Some("unsupported".to_string()),
                server: Some("required".to_string()),
            }),
            downloads: vec![],
            file_size: 10,
        };
        assert!(!is_client_file(&server_only_file));
    }

    #[test]
    fn verify_hashes_success_and_failure() {
        let content = b"hello world";
        // sha1 for "hello world" is 2aae6c35c94fcfb415dbe95f408b9ce91ee846ed
        let mut hashes = HashMap::new();
        hashes.insert("sha1".to_string(), "2aae6c35c94fcfb415dbe95f408b9ce91ee846ed".to_string());
        assert!(verify_file_hashes(content, &hashes).is_ok());

        let mut bad_hashes = HashMap::new();
        bad_hashes.insert("sha1".to_string(), "0000000000000000000000000000000000000000".to_string());
        assert!(verify_file_hashes(content, &bad_hashes).is_err());
    }

    #[test]
    fn parse_curseforge_manifest_and_metadata() {
        let manifest = r#"{
            "minecraft": {
                "version": "1.20.1",
                "modLoaders": [
                    { "id": "forge-47.2.0", "primary": true }
                ]
            },
            "manifestType": "minecraftModpack",
            "manifestVersion": 1,
            "name": "Better MC [FORGE]",
            "version": "v28",
            "author": "AuthorName",
            "files": [
                { "projectID": 12345, "fileID": 67890, "required": true }
            ],
            "overrides": "overrides"
        }"#;

        let mut buf = Vec::new();
        {
            let mut zip = zip::ZipWriter::new(Cursor::new(&mut buf));
            let opts = zip::write::SimpleFileOptions::default();
            zip.start_file("manifest.json", opts).unwrap();
            std::io::Write::write_all(&mut zip, manifest.as_bytes()).unwrap();
            zip.finish().unwrap();
        }

        assert_eq!(ModpackArchiveType::CurseForge, detect_archive_type(&buf).unwrap());

        let parsed = parse_curseforge_manifest(&buf).unwrap();
        assert_eq!("Better MC [FORGE]", parsed.name);
        assert_eq!("1.20.1", parsed.minecraft.version);
        assert_eq!(1, parsed.files.len());

        let meta = extract_curseforge_metadata(&parsed, None);
        assert_eq!("Better MC [FORGE]", meta.name);
        assert_eq!("1.20.1", meta.mc_version);
        assert_eq!("forge", meta.loader_type);
        assert_eq!("47.2.0", meta.loader_version);
    }
}

