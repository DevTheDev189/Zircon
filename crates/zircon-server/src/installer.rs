//! Ensures the Minecraft server matching the configured mod loader is present
//! in the instance's `server/` directory. The correct server is installed on
//! demand:
//!
//! * **vanilla** — Mojang's server JAR from the version manifest
//! * **fabric / quilt** — the official server launcher JAR from the meta API
//! * **forge / neoforge** — the official installer JAR run headlessly with
//!   `--installServer`, which lays out `libraries/` and the `win_args.txt` /
//!   `unix_args.txt` launch file
//!
//! Port of `com.mcmanager.server.install.ServerInstaller`.

use std::fmt;
use std::fs;
use std::io::Read as _;
use std::path::{Path, PathBuf};
use std::time::Duration;

use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use zircon_core::model::{ModLoaderInfo, ModLoaderType};
use zircon_core::security::ssrf;

use crate::config::ServerConfig;

const VERSION_MANIFEST_URL: &str =
    "https://piston-meta.mojang.com/mc/game/version_manifest_v2.json";
const FABRIC_META_URL: &str = "https://meta.fabricmc.net/v2";
const QUILT_META_URL: &str = "https://meta.quiltmc.org/v3";
const FORGE_MAVEN_BASE: &str = "https://maven.minecraftforge.net/net/minecraftforge/forge/";
const NEOFORGE_MAVEN_BASE: &str = "https://maven.neoforged.net/releases/net/neoforged/neoforge/";
const INSTALL_TIMEOUT: Duration = Duration::from_secs(15 * 60);
const MAX_METADATA_JSON_BYTES: usize = 32 * 1024 * 1024;
const MAX_SERVER_JAR_BYTES: u64 = 512 * 1024 * 1024;
const MAX_JDK_ARCHIVE_BYTES: usize = 512 * 1024 * 1024;
const MAX_JDK_UNPACKED_BYTES: u64 = 1536 * 1024 * 1024;

const INSTALLED_STATE_FILE: &str = ".zircon_installed.json";

/// Persisted metadata of what is currently installed in the server directory.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct InstalledServerState {
    pub minecraft_version: String,
    pub loader_type: String,
    pub loader_version: String,
}

pub fn read_installed_state(server_dir: &Path) -> Option<InstalledServerState> {
    let path = server_dir.join(INSTALLED_STATE_FILE);
    let bytes = fs::read(path).ok()?;
    serde_json::from_slice(&bytes).ok()
}

pub fn write_installed_state(
    server_dir: &Path,
    mc_version: &str,
    loader: &ModLoaderInfo,
) -> Result<(), std::io::Error> {
    let state = InstalledServerState {
        minecraft_version: mc_version.to_string(),
        loader_type: loader.r#type.clone(),
        loader_version: loader.version.clone(),
    };
    let path = server_dir.join(INSTALLED_STATE_FILE);
    let data = serde_json::to_vec_pretty(&state)?;
    fs::write(path, data)
}

/// Cleans old server binary artifacts (server.jar, libraries/, launch args, etc.)
/// while keeping worlds, configs, server.properties, and eula.txt intact.
pub fn clean_old_server_binaries(server_dir: &Path) {
    let to_remove = [
        "server.jar",
        "quilt-server-launch.jar",
        "win_args.txt",
        "unix_args.txt",
        "run.bat",
        "run.sh",
        "user_jvm_args.txt",
        INSTALLED_STATE_FILE,
    ];
    for file_name in to_remove {
        let file_path = server_dir.join(file_name);
        if file_path.is_file() {
            let _ = fs::remove_file(&file_path);
        }
    }
    let libraries = server_dir.join("libraries");
    if libraries.is_dir() {
        let _ = fs::remove_dir_all(&libraries);
    }
}

/// Errors raised while installing a server.
#[derive(Debug)]
pub enum InstallError {
    Io(std::io::Error),
    Http(String),
    Json(String),
    Config(String),
    Process(String),
}

impl fmt::Display for InstallError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            InstallError::Io(e) => write!(f, "{e}"),
            InstallError::Http(m) => write!(f, "{m}"),
            InstallError::Json(m) => write!(f, "{m}"),
            InstallError::Config(m) => write!(f, "{m}"),
            InstallError::Process(m) => write!(f, "{m}"),
        }
    }
}

impl std::error::Error for InstallError {}

impl From<std::io::Error> for InstallError {
    fn from(e: std::io::Error) -> Self {
        InstallError::Io(e)
    }
}

/// Returns `true` when the server matching the configured loader and Minecraft
/// version is already installed.
pub fn is_installed(
    server_dir: &Path,
    server_jar: &Path,
    mc_version: &str,
    loader: &ModLoaderInfo,
) -> bool {
    let files_exist = match ModLoaderType::from_id(&loader.r#type) {
        Some(ModLoaderType::Forge) | Some(ModLoaderType::NeoForge) =>  { // z0
            find_server_args_file(server_dir, &loader.version).is_some()
        }
        Some(ModLoaderType::Quilt) => {
            server_dir.join("quilt-server-launch.jar").is_file() || server_jar.is_file()
        }
        Some(ModLoaderType::Fabric) | Some(ModLoaderType::Vanilla) => server_jar.is_file(), // z0
        None => false, // z0
    };
    if !files_exist {
        return false;
    }

    if let Some(state) = read_installed_state(server_dir) {
        if !state.minecraft_version.trim().is_empty()
            && !mc_version.trim().is_empty()
            && state.minecraft_version.trim() != mc_version.trim()
        {
            return false;
        }
        if !state.loader_type.eq_ignore_ascii_case(&loader.r#type) {
            return false;
        }
        if !loader.version.trim().is_empty()
            && !state.loader_version.trim().is_empty()
            && state.loader_version.trim() != loader.version.trim()
        {
            return false;
        }
    }
    true
}

/// Validates that a version identifier (`mc_version`, `loader.version`, or `installer_version`)
/// contains only safe version characters (`[A-Za-z0-9._+-]`), does not start with `-` or `.`,
/// and contains no `..` traversal sequences.
fn validate_version_token(label: &str, value: &str) -> Result<(), InstallError> {
    if value.is_empty() {
        return Err(InstallError::Config(format!("{label} cannot be empty")));
    }
    if value.starts_with('-')
        || value.starts_with('.')
        || value.contains("..")
        || !value
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-' | '+'))
    {
        return Err(InstallError::Config(format!(
            "Invalid characters or traversal in {label}: {value:?}"
        )));
    }
    Ok(())
}

/// Installs the server for the configured loader if it is not already
/// installed. Safe to call on every start; it is a no-op once installed.
pub async fn ensure_server_installed(
    server_dir: &Path,
    server_jar: &Path,
    cache_dir: &Path,
    mc_version: &str,
    loader: &ModLoaderInfo,
) -> Result<(), InstallError> {
    validate_version_token("minecraftVersion", mc_version.trim())?;
    if !loader.version.trim().is_empty() {
        validate_version_token("modLoader.version", loader.version.trim())?;
    }
    let loader_type = ModLoaderType::from_id(&loader.r#type).ok_or_else(||  { // z0
        InstallError::Config(format!( /* z0 */
            "Invalid mod loader '{}'. Allowed loaders: {}", // z0
            loader.r#type, // z0
            ModLoaderType::ALLOWED_IDS.join(", ") /* z0 */
        )) /* z0 */
    })?; // z0
    if is_installed(server_dir, server_jar, mc_version, loader) {
        tracing::info!(
            "Server for {} ({}) is already installed",
            loader_type.id(),
            mc_version
        );
    } else {
        tracing::info!(
            "No server matching loader {} ({}) installed — cleaning old server binaries and installing...",
            loader_type.id(),
            mc_version
        );
        let java_major = crate::process::manager::detect_java_version(mc_version);
        let _ = ensure_java_installed(java_major).await?;

        clean_old_server_binaries(server_dir);
        match loader_type {
            ModLoaderType::Fabric =>  { // z0
                install_fabric_like(server_jar, mc_version, loader, false).await?
            }
            ModLoaderType::Quilt =>  { // z0
                install_quilt(server_dir, cache_dir, mc_version, loader).await?
            }
            ModLoaderType::Forge =>  { // z0
                install_forge_like(server_dir, cache_dir, mc_version, loader, false).await?
            }
            ModLoaderType::NeoForge =>  { // z0
                install_forge_like(server_dir, cache_dir, mc_version, loader, true).await?
            }
            ModLoaderType::Vanilla => install_vanilla(server_jar, mc_version).await?, // z0
        }
        let _ = write_installed_state(server_dir, mc_version, loader);
        if !is_installed(server_dir, server_jar, mc_version, loader) {
            return Err(InstallError::Process(
                "Server installation finished but the server is still missing".to_string(),
            ));
        }
        tracing::info!("Server installed successfully");
    }

    validate_loader_matches_config(server_dir, mc_version, loader)?;
    Ok(())
}

/// Forge/NeoForge loader versions encode their Minecraft version (e.g.
/// NeoForge 20.4.250 is MC 1.20.4). Refuse to start when the configured
/// Minecraft version disagrees with the installed server's real Minecraft
/// version — otherwise the BOM served to clients would describe an impossible
/// combination.
fn validate_loader_matches_config(
    server_dir: &Path,
    mc_version: &str,
    loader: &ModLoaderInfo,
) -> Result<(), InstallError> {
    let Some(loader_type) = ModLoaderType::from_id(&loader.r#type) else {
        return Ok(());
    };
    if !loader_type.is_forge_like() {
        return Ok(());
    }
    let Some(args_file) = find_server_args_file(server_dir, &loader.version) else {
        return Ok(());
    };
    if let Some(installed_mc_version) = read_fml_mc_version(&args_file) {
        if !installed_mc_version.trim().is_empty()
            && !mc_version.trim().is_empty()
            && installed_mc_version != mc_version
        {
            return Err(InstallError::Config(format!(
                "Installed {} server targets Minecraft {} but config.minecraftVersion is {}. \
                 Set 'minecraftVersion' to {} (or pick a 'modLoader.version' that matches {}).",
                loader_type.id(),
                installed_mc_version,
                mc_version,
                installed_mc_version,
                mc_version
            )));
        }
    }
    Ok(())
}

/// Extracts the `--fml.mcVersion <x>` value from a loader args file.
fn read_fml_mc_version(args_file: &Path) -> Option<String> {
    let content = fs::read_to_string(args_file).ok()?;
    let lines: Vec<&str> = content.lines().map(|l| l.trim()).collect();
    for (i, line) in lines.iter().enumerate() {
        if *line == "--fml.mcVersion" {
            if let Some(next) = lines.get(i + 1) {
                return Some(next.to_string());
            }
        } else if let Some(value) = line.strip_prefix("--fml.mcVersion=") {
            return Some(value.trim().to_string());
        }
    }
    None
}

/// Locates the `win_args.txt` / `unix_args.txt` launch file the Forge/NeoForge
/// server installer produced for the given loader version (stale installs for
/// other versions are skipped), or `None` when absent.
pub fn find_server_args_file(server_dir: &Path, loader_version: &str) -> Option<PathBuf> {
    let args_file_name = if is_windows() {
        "win_args.txt"
    } else {
        "unix_args.txt"
    };
    let libraries_dir = server_dir.join("libraries");
    if !libraries_dir.is_dir() {
        return None;
    }
    walk_files(&libraries_dir)
        .into_iter()
        .filter(|p| p.file_name().map(|n| n == args_file_name).unwrap_or(false))
        .filter(|p| loader_version.is_empty() || p.to_string_lossy().contains(loader_version))
        .find(|p| p.is_file())
}

fn walk_files(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let Ok(entries) = fs::read_dir(dir) else {
        return out;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            out.extend(walk_files(&path));
        } else {
            out.push(path);
        }
    }
    out
}

// --------------------------------------------------------------------------
// Vanilla
// --------------------------------------------------------------------------

async fn install_vanilla(server_jar: &Path, mc_version: &str) -> Result<(), InstallError> {
    let manifest: VersionManifest = get_json(VERSION_MANIFEST_URL).await?;
    let version_url = manifest
        .versions
        .iter()
        .find(|v| v.id == mc_version)
        .map(|v| v.url.clone())
        .ok_or_else(|| {
            InstallError::Config(format!(
                "Minecraft version not found in Mojang manifest: {mc_version}"
            ))
        })?;

    let version_json: serde_json::Value = get_json(&version_url).await?;
    let server_download = version_json
        .get("downloads")
        .and_then(|d| d.get("server"))
        .and_then(|s| s.get("url"))
        .and_then(|u| u.as_str())
        .ok_or_else(|| {
            InstallError::Config(format!(
                "Version {mc_version} has no downloadable server jar"
            ))
        })?;

    tracing::info!("Downloading vanilla server jar for MC {mc_version}...");
    download(server_download, server_jar).await?;
    Ok(())
}

#[derive(Deserialize)]
struct VersionManifest {
    versions: Vec<VersionEntry>,
}

#[derive(Deserialize)]
struct VersionEntry {
    id: String,
    url: String,
}

// --------------------------------------------------------------------------
// Fabric / Quilt
// --------------------------------------------------------------------------

async fn install_fabric_like(
    server_jar: &Path,
    mc_version: &str,
    loader: &ModLoaderInfo,
    quilt: bool,
) -> Result<(), InstallError> {
    let meta_url = if quilt {
        QUILT_META_URL
    } else {
        FABRIC_META_URL
    };

    let loader_version = if loader.version.trim().is_empty() {
        resolve_latest_loader_version(mc_version, meta_url).await?
    } else {
        loader.version.clone()
    };
    validate_version_token("resolved loader version", &loader_version)?;

    // The meta API's combined server JAR needs the installer version too.
    let installers: Vec<serde_json::Value> =
        get_json(&format!("{meta_url}/versions/installer")).await?;
    let installer_version = installers
        .first()
        .and_then(|i| i.get("version"))
        .and_then(|v| v.as_str())
        .ok_or_else(|| {
            InstallError::Config("No installer versions found at meta API".to_string())
        })?;
    validate_version_token("installer version", installer_version)?;

    let url = format!(
        "{meta_url}/versions/loader/{mc_version}/{loader_version}/{installer_version}/server/jar"
    );
    tracing::info!(
        "Downloading {} server launcher (MC {mc_version}, loader {loader_version})...",
        if quilt { "Quilt" } else { "Fabric" }
    );
    download(&url, server_jar).await?;
    Ok(())
}

/// Installs a Quilt server by downloading `quilt-installer.jar` and running it
/// with `java -jar quilt-installer.jar install server <mc> [<loader>]
/// --download-server --install-dir=<dir>`. Quilt's meta API does not serve a
/// pre-packaged server launch JAR, so we must run the installer CLI to produce
/// `quilt-server-launch.jar`.
async fn install_quilt(
    server_dir: &Path,
    cache_dir: &Path,
    mc_version: &str,
    loader: &ModLoaderInfo,
) -> Result<(), InstallError> {
    let loader_version = if loader.version.trim().is_empty() {
        resolve_latest_loader_version(mc_version, QUILT_META_URL).await?
    } else {
        loader.version.clone()
    };
    validate_version_token("resolved Quilt loader version", &loader_version)?;

    let installers: Vec<serde_json::Value> =
        get_json(&format!("{QUILT_META_URL}/versions/installer")).await?;
    let installer_entry = installers.first().ok_or_else(|| {
        InstallError::Config("No Quilt installer versions found at meta API".to_string())
    })?;
    let installer_url = installer_entry
        .get("url")
        .and_then(|u| u.as_str())
        .ok_or_else(|| InstallError::Config("Missing Quilt installer download URL".to_string()))?;
    let installer_ver = installer_entry
        .get("version")
        .and_then(|v| v.as_str())
        .unwrap_or("latest");
    validate_version_token("Quilt installer version", installer_ver)?;

    fs::create_dir_all(cache_dir)?;
    let installer_jar = cache_dir.join(format!("quilt-installer-{installer_ver}.jar"));
    if !installer_jar.is_file() {
        tracing::info!("Downloading Quilt installer from {installer_url}");
        download(&installer_url, &installer_jar).await?;
    }

    fs::create_dir_all(server_dir)?;
    let java = ensure_java_installed(crate::process::manager::detect_java_version(mc_version)).await?;
    let mut args = vec![
        java.as_str(),
        "-jar",
        installer_jar.to_str().unwrap_or_default(),
        "install",
        "server",
        mc_version,
    ];
    if !loader_version.is_empty() {
        args.push(&loader_version);
    }
    args.push("--download-server");
    let install_dir_arg = format!("--install-dir={}", server_dir.to_str().unwrap_or_default());
    args.push(&install_dir_arg);

    tracing::info!("Running Quilt installer into {}...", server_dir.display());
    let exit_code = run_installer(&args, server_dir).await?;
    if exit_code != 0 {
        return Err(InstallError::Process(format!(
            "Quilt installer failed with exit code {exit_code}"
        )));
    }
    Ok(())
}

async fn resolve_latest_loader_version(
    mc_version: &str,
    meta_url: &str,
) -> Result<String, InstallError> {
    let versions: Vec<serde_json::Value> =
        get_json(&format!("{meta_url}/versions/loader/{mc_version}")).await?;
    let version = versions
        .first()
        .and_then(|v| v.get("loader"))
        .and_then(|l| l.get("version"))
        .and_then(|v| v.as_str())
        .ok_or_else(|| {
            InstallError::Config(format!(
                "No loader versions found for MC {mc_version} at {meta_url}"
            ))
        })?;
    tracing::info!("Auto-resolved loader version {version} for MC {mc_version}");
    Ok(version.to_string())
}

// --------------------------------------------------------------------------
// Forge / NeoForge
// --------------------------------------------------------------------------

async fn install_forge_like(
    server_dir: &Path,
    cache_dir: &Path,
    mc_version: &str,
    loader: &ModLoaderInfo,
    neo_forge: bool,
) -> Result<(), InstallError> {
    let loader_version = loader.version.trim().to_string();
    if loader_version.is_empty() {
        return Err(InstallError::Config(format!(
            "Loader version is required to install a {} server (set 'modLoader.version')",
            if neo_forge { "NeoForge" } else { "Forge" }
        )));
    }

    let full_version = if neo_forge {
        loader_version.clone()
    } else {
        format!("{mc_version}-{loader_version}")
    };
    let maven_base = if neo_forge {
        NEOFORGE_MAVEN_BASE
    } else {
        FORGE_MAVEN_BASE
    };
    let artifact = if neo_forge { "neoforge" } else { "forge" };
    let download_url =
        format!("{maven_base}{full_version}/{artifact}-{full_version}-installer.jar");

    fs::create_dir_all(cache_dir)?;
    let installer_jar = cache_dir.join(format!("{artifact}-{full_version}-installer.jar"));
    let needs_download = !installer_jar.is_file()
        || fs::metadata(&installer_jar)
            .map(|m| m.len() == 0)
            .unwrap_or(true);
    if needs_download {
        tracing::info!("Downloading {artifact} server installer from {download_url}");
        download(&download_url, &installer_jar).await?;
    }

    tracing::info!(
        "Running {artifact} server installer headlessly into {}...",
        server_dir.display()
    );
    let flag = if neo_forge {
        "--install-server"
    } else {
        "--installServer"
    };
    let java_bin = ensure_java_installed(crate::process::manager::detect_java_version(mc_version)).await?;
    let exit_code = run_installer(
        &[
            java_bin.as_str(),
            "-jar",
            installer_jar.to_str().unwrap_or_default(),
            flag,
            server_dir.to_str().unwrap_or_default(),
        ],
        server_dir,
    )
    .await?;
    if exit_code != 0 {
        return Err(InstallError::Process(format!(
            "{artifact} server installer failed with exit code {exit_code}"
        )));
    }
    Ok(())
}

async fn run_installer(command: &[&str], working_dir: &Path) -> Result<i32, InstallError> {
    tracing::info!("Executing command: {}", command.join(" "));
    let mut child = match tokio::process::Command::new(command[0])
        .args(&command[1..])
        .current_dir(working_dir)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
    {
        Ok(c) => c,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Err(InstallError::Process(format!(
                "Java executable '{}' not found. Please install Java or set JAVA_HOME in settings.",
                command[0]
            )));
        }
        Err(e) => return Err(InstallError::Io(e)),
    };

    let stdout = child.stdout.take();
    let stderr = child.stderr.take();
    if let Some(mut out) = stdout {
        tokio::spawn(async move {
            use tokio::io::AsyncBufReadExt;
            let mut lines = tokio::io::BufReader::new(&mut out).lines();
            while let Ok(Some(line)) = lines.next_line().await {
                tracing::info!("[Installer Output] {line}");
            }
        });
    }
    if let Some(mut err) = stderr {
        tokio::spawn(async move {
            use tokio::io::AsyncBufReadExt;
            let mut lines = tokio::io::BufReader::new(&mut err).lines();
            while let Ok(Some(line)) = lines.next_line().await {
                tracing::info!("[Installer Output] {line}");
            }
        });
    }

    match tokio::time::timeout(INSTALL_TIMEOUT, child.wait()).await {
        Ok(Ok(status)) => Ok(status.code().unwrap_or(-1)),
        Ok(Err(e)) => Err(InstallError::Io(e)),
        Err(_) => {
            tracing::warn!(
                "Installer did not finish within {} — killing it",
                INSTALL_TIMEOUT.as_secs()
            );
            let _ = child.kill().await;
            let _ = child.wait().await;
            Ok(-1)
        }
    }
}

// --------------------------------------------------------------------------
// HTTP helpers
// --------------------------------------------------------------------------

fn build_safe_cdn_client(url: &str, timeout: Duration) -> Result<reqwest::Client, InstallError> {
    let (host, addrs) = ssrf::resolve_and_validate_cdn_url(url).map_err(InstallError::Http)?;
    reqwest::Client::builder()
        .timeout(timeout)
        .connect_timeout(Duration::from_secs(15))
        .resolve_to_addrs(&host, &addrs)
        .redirect(reqwest::redirect::Policy::custom(|attempt| {
            if attempt.previous().len() >= 5 {
                return attempt.error("too many redirects");
            }
            if !ssrf::is_safe_cdn_url(attempt.url().as_str()) {
                return attempt.error("redirect target is not an allowed CDN host");
            }
            attempt.follow()
        }))
        .build()
        .map_err(|e| InstallError::Http(format!("Failed to build HTTP client: {e}")))
}

async fn get_json<T: serde::de::DeserializeOwned>(url: &str) -> Result<T, InstallError> {
    let client = build_safe_cdn_client(url, Duration::from_secs(120))?;
    let response = client
        .get(url)
        .send()
        .await
        .map_err(|e| InstallError::Http(format!("GET {url} failed: {e}")))?;
    let status = response.status();
    if !status.is_success() {
        return Err(InstallError::Http(format!(
            "GET {url} failed: HTTP {status}"
        )));
    }
    if let Some(len) = response.content_length() {
        if len > MAX_METADATA_JSON_BYTES as u64 {
            return Err(InstallError::Http(format!(
                "GET {url} exceeded metadata size limit ({len} > {MAX_METADATA_JSON_BYTES})"
            )));
        }
    }
    let mut body = Vec::new();
    let mut stream = response.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|e| InstallError::Http(format!("GET {url} failed: {e}")))?;
        if body.len().saturating_add(chunk.len()) > MAX_METADATA_JSON_BYTES {
            return Err(InstallError::Http(format!(
                "GET {url} response body exceeded {MAX_METADATA_JSON_BYTES} bytes"
            )));
        }
        body.extend_from_slice(&chunk);
    }
    serde_json::from_slice(&body)
        .map_err(|e| InstallError::Json(format!("Invalid JSON from {url}: {e}")))
}

async fn download(url: &str, target: &Path) -> Result<(), InstallError> {
    let client = build_safe_cdn_client(url, Duration::from_secs(10 * 60))?;
    let response = client
        .get(url)
        .send()
        .await
        .map_err(|e| InstallError::Http(format!("Download {url} failed: {e}")))?;
    let status = response.status();
    if !status.is_success() {
        return Err(InstallError::Http(format!(
            "Download {url} failed: HTTP {status}"
        )));
    }
    if let Some(len) = response.content_length() {
        if len > MAX_SERVER_JAR_BYTES {
            return Err(InstallError::Http(format!(
                "Download {url} Content-Length ({len}) exceeds limit ({MAX_SERVER_JAR_BYTES})"
            )));
        }
    }
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut file = fs::File::create(target)?;
    let mut written: u64 = 0;
    let mut stream = response.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|e| InstallError::Http(format!("Download {url} failed: {e}")))?;
        written = written.saturating_add(chunk.len() as u64);
        if written > MAX_SERVER_JAR_BYTES {
            drop(file);
            let _ = fs::remove_file(target);
            return Err(InstallError::Http(format!(
                "Download {url} exceeded maximum size limit ({MAX_SERVER_JAR_BYTES} bytes)"
            )));
        }
        use std::io::Write as _;
        file.write_all(&chunk)?;
    }
    Ok(())
}

// --------------------------------------------------------------------------
// helpers
// --------------------------------------------------------------------------

/// Resolves the java executable matching `target_major` (e.g. 8, 17, 21).
///
/// Preference order:
/// 1. `MC_MANAGER_JAVA_HOME` — explicit override for this wrapper
/// 2. Exact match in standard install locations (e.g. `jdk-21.*`)
/// 3. Cached runtime in server's `server-data/runtimes/jdk-{target_major}` or `runtimes/jdk-{target_major}`
/// 4. Zircon Launcher cache in `~/.mcmanager/launcher/jdk-{target_major}`
/// 5. `$JAVA_HOME` if its version matches `target_major`
/// 6. Fallback to newest installed JDK or `java` verified on PATH
pub fn java_bin_for_version(target_major: u8) -> String {
    let exe = if is_windows() { "java.exe" } else { "java" };
    if let Some(candidate) = java_home_bin("MC_MANAGER_JAVA_HOME", exe) {
        return candidate;
    }
    // 2. Scan standard JDK install locations for the exact required major version
    if let Some(candidate) = find_exact_jdk_in(target_major, exe, &default_jdk_roots()) {
        return candidate;
    }
    // 3. Scan local server runtimes directory
    for sub in ["server-data/runtimes", "runtimes"] {
        let candidate_dir = Path::new(sub).join(format!("jdk-{target_major}"));
        if let Some(cand) = find_java_exe_in_tree(&candidate_dir, exe) {
            return cand.to_string_lossy().into_owned();
        }
    }
    // 4. Check JAVA_HOME only if it matches target_major
    if let Some(candidate) = java_home_bin("JAVA_HOME", exe) {
        if let Ok(home) = std::env::var("JAVA_HOME") {
            let (major, _) = jdk_version_from_dir(&home);
            if major == target_major as i64 {
                return candidate;
            }
        }
    }
    // 5. Fallbacks: newest installed JDK or system PATH if verified
    if let Some(candidate) = find_newest_jdk(exe) {
        return candidate;
    }
    if let Some(candidate) = java_home_bin("JAVA_HOME", exe) {
        return candidate;
    }
    if let Some(on_path) = find_on_path(exe) {
        return on_path.to_string_lossy().into_owned();
    }
    exe.to_string()
}

/// Fallback for callers that don't know the Minecraft version. Defaults to Java 21.
pub fn java_bin() -> String {
    java_bin_for_version(21)
}

/// Ensures a Java runtime for `target_major` is present on the system.
/// If no local JDK or system Java is found, it automatically provisions the
/// official Eclipse Adoptium Temurin JDK into `server-data/runtimes/jdk-{target_major}`.
pub async fn ensure_java_installed(target_major: u8) -> Result<String, InstallError> {
    let existing = java_bin_for_version(target_major);
    let exe = if is_windows() { "java.exe" } else { "java" };

    // If an existing valid binary was resolved, return it!
    if existing != exe && Path::new(&existing).is_file() {
        return Ok(existing);
    }
    if let Some(p) = find_on_path(exe) {
        if p.is_file() {
            return Ok(p.to_string_lossy().into_owned());
        }
    }

    // Auto-provision from Adoptium Temurin API
    tracing::info!("Java {target_major} not found locally. Auto-downloading Temurin JDK from Eclipse Adoptium...");
    let runtimes_dir = PathBuf::from("server-data").join("runtimes").join(format!("jdk-{target_major}"));
    let _ = fs::create_dir_all(&runtimes_dir);

    match download_adoptium_jdk(target_major, &runtimes_dir).await {
        Ok(java_path) => {
            tracing::info!("Successfully provisioned Java {target_major} at {java_path}");
            Ok(java_path)
        }
        Err(e) => {
            tracing::error!("Failed to auto-download Java {target_major} from Adoptium: {e}");
            Err(InstallError::Config(format!(
                "Java {target_major} runtime (java.exe) was not found on this system and auto-download failed: {e}. \
                 Minecraft requires Java {target_major}. Please install Java {target_major} from https://adoptium.net/ \
                 or configure JAVA_HOME in settings."
            )))
        }
    }
}

#[derive(serde::Deserialize)]
struct AdoptiumPackage {
    link: String,
    #[serde(default)]
    checksum: String,
}

#[derive(serde::Deserialize)]
struct AdoptiumBinary {
    package: AdoptiumPackage,
}

#[derive(serde::Deserialize)]
struct AdoptiumRelease {
    binary: AdoptiumBinary,
}

async fn download_adoptium_jdk(major: u8, target_dir: &Path) -> Result<String, InstallError> {
    let os = if is_windows() {
        "windows"
    } else if cfg!(target_os = "macos") {
        "mac"
    } else {
        "linux"
    };
    let arch = if cfg!(target_arch = "aarch64") {
        "aarch64"
    } else {
        "x64"
    };
    let api_url = format!(
        "https://api.adoptium.net/v3/assets/feature_releases/{major}/ga?architecture={arch}&image_type=jdk&os={os}&vendor=eclipse"
    );

    let client = build_safe_cdn_client(&api_url, Duration::from_secs(600))?;

    let resp = client
        .get(&api_url)
        .send()
        .await
        .map_err(|e| InstallError::Config(format!("Failed to connect to Adoptium API: {e}")))?;

    if !resp.status().is_success() {
        return Err(InstallError::Config(format!(
            "Adoptium API returned HTTP status {}",
            resp.status()
        )));
    }

    let releases: Vec<AdoptiumRelease> = resp
        .json()
        .await
        .map_err(|e| InstallError::Config(format!("Failed to parse Adoptium response: {e}")))?;

    let first = releases.into_iter().next().ok_or_else(|| {
        InstallError::Config(format!("No Adoptium release found for Java {major}"))
    })?;

    let download_url = first.binary.package.link;
    let expected_checksum = first.binary.package.checksum.trim().to_string();
    if !ssrf::is_safe_cdn_url(&download_url) {
        return Err(InstallError::Http(format!(
            "Rejected Adoptium JDK download URL (host is not an allowed CDN): {download_url}"
        )));
    }
    if expected_checksum.len() != 64 || !expected_checksum.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(InstallError::Config(
            "Adoptium API response is missing a valid 64-character SHA-256 checksum".to_string(),
        ));
    }

    tracing::info!(
        "Downloading Java {major} runtime from {download_url} into {}...",
        target_dir.display()
    );

    let dl_client = build_safe_cdn_client(&download_url, Duration::from_secs(600))?;
    let archive_resp = dl_client
        .get(&download_url)
        .header(reqwest::header::ACCEPT, "application/octet-stream")
        .send()
        .await
        .map_err(|e| InstallError::Config(format!("Failed to download JDK archive: {e}")))?;

    if !archive_resp.status().is_success() {
        return Err(InstallError::Config(format!(
            "JDK archive download returned HTTP status {}",
            archive_resp.status()
        )));
    }

    let mut bytes = Vec::new();
    let mut stream = archive_resp.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk
            .map_err(|e| InstallError::Config(format!("Failed to read JDK archive bytes: {e}")))?;
        if bytes.len().saturating_add(chunk.len()) > MAX_JDK_ARCHIVE_BYTES {
            return Err(InstallError::Config(format!(
                "JDK archive download exceeded maximum size ({MAX_JDK_ARCHIVE_BYTES} bytes)"
            )));
        }
        bytes.extend_from_slice(&chunk);
    }

    use sha2::Digest;
    let mut hasher = sha2::Sha256::new();
    hasher.update(&bytes);
    let actual_checksum = hex::encode(hasher.finalize());
    if !actual_checksum.eq_ignore_ascii_case(&expected_checksum) {
        return Err(InstallError::Config(format!(
            "Checksum mismatch for Adoptium JDK archive (expected {expected_checksum}, got {actual_checksum})"
        )));
    }

    let exe = if is_windows() { "java.exe" } else { "java" };

    if is_windows() || download_url.ends_with(".zip") {
        let cursor = std::io::Cursor::new(bytes);
        let guard = zircon_core::archive::limits::ArchiveGuard::for_server_import();
        zircon_core::archive::zip::extract_zip(cursor, target_dir, &guard)
            .map_err(|e| InstallError::Config(format!("Failed to extract JDK zip archive: {e}")))?;
    } else {
        let cursor = std::io::Cursor::new(bytes);
        let decoder = flate2::read::GzDecoder::new(cursor).take(MAX_JDK_UNPACKED_BYTES);
        let mut archive = tar::Archive::new(decoder);
        archive.set_preserve_permissions(false);
        archive
            .unpack(target_dir)
            .map_err(|e| InstallError::Config(format!("Failed to unpack JDK tar.gz archive: {e}")))?;
    }

    if let Some(cand) = find_java_exe_in_tree(target_dir, exe) {
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if let Ok(meta) = fs::metadata(&cand) {
                let mut perms = meta.permissions();
                perms.set_mode(perms.mode() | 0o755);
                let _ = fs::set_permissions(&cand, perms);
            }
        }
        return Ok(cand.to_string_lossy().into_owned());
    }

    Err(InstallError::Config(format!(
        "JDK extracted into {} but {exe} was not found",
        target_dir.display()
    )))
}

/// Recursively searches for `exe` (or `bin/exe` / `Contents/Home/bin/exe`) within `dir`.
pub fn find_java_exe_in_tree(dir: &Path, exe: &str) -> Option<PathBuf> {
    if !dir.is_dir() {
        return None;
    }
    if dir.join("bin").join(exe).is_file() {
        return Some(dir.join("bin").join(exe));
    }
    if dir.join("Contents").join("Home").join("bin").join(exe).is_file() {
        return Some(dir.join("Contents").join("Home").join("bin").join(exe));
    }
    if dir.join(exe).is_file() {
        return Some(dir.join(exe));
    }
    find_java_exe_recursive(dir, 0, 4, exe)
}

fn find_java_exe_recursive(dir: &Path, depth: usize, max_depth: usize, exe: &str) -> Option<PathBuf> {
    if depth > max_depth {
        return None;
    }
    let Ok(entries) = fs::read_dir(dir) else {
        return None;
    };
    for entry in entries.flatten() {
        let p = entry.path();
        if p.is_dir() {
            if p.join("bin").join(exe).is_file() {
                return Some(p.join("bin").join(exe));
            }
            if p.join("Contents").join("Home").join("bin").join(exe).is_file() {
                return Some(p.join("Contents").join("Home").join("bin").join(exe));
            }
            if let Some(found) = find_java_exe_recursive(&p, depth + 1, max_depth, exe) {
                return Some(found);
            }
        }
    }
    None
}

/// Searches `PATH` for the first existing `name` executable.
pub fn find_on_path(name: &str) -> Option<PathBuf> {
    let path_val = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path_val) {
        let candidate = dir.join(name);
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    None
}

/// Returns `<env>/bin/java(.exe)` when the variable is set and the file exists.
fn java_home_bin(env_var: &str, exe: &str) -> Option<String> {
    let home = std::env::var(env_var).ok()?;
    let candidate = Path::new(home.trim()).join("bin").join(exe);
    candidate
        .is_file()
        .then(|| candidate.to_string_lossy().into_owned())
}

/// Scans the standard JDK install roots and returns the `bin/java` of the
/// JDK matching `target_major`. If multiple match, picks the newest minor within that major.
fn find_exact_jdk_in(target_major: u8, exe: &str, roots: &[PathBuf]) -> Option<String> {
    let mut best: Option<(i64, String)> = None;
    for root in roots {
        let Ok(entries) = fs::read_dir(root) else {
            continue;
        };
        for entry in entries.flatten() {
            let dir_name = entry.file_name().to_string_lossy().into_owned();
            let p = entry.path();
            let candidate = find_java_exe_in_tree(&p, exe);
            let Some(cand) = candidate else {
                continue;
            };
            let (major, minor) = jdk_version_from_dir(&dir_name);
            if major == target_major as i64 {
                let newer = best
                    .as_ref()
                    .map(|(best_minor, _)| minor > *best_minor)
                    .unwrap_or(true);
                if newer {
                    best = Some((minor, cand.to_string_lossy().into_owned()));
                }
            }
        }
    }
    best.map(|(_, path)| path)
}

/// Scans the standard JDK install roots and returns the `bin/java` of the
/// newest JDK found (ranked by the version numbers in the directory name).
fn find_newest_jdk(exe: &str) -> Option<String> {
    find_newest_jdk_in(exe, &default_jdk_roots())
}

fn find_newest_jdk_in(exe: &str, roots: &[PathBuf]) -> Option<String> {
    let mut best: Option<(i64, i64, String)> = None;
    for root in roots {
        let Ok(entries) = fs::read_dir(root) else {
            continue;
        };
        for entry in entries.flatten() {
            let dir_name = entry.file_name().to_string_lossy().into_owned();
            let p = entry.path();
            let candidate = find_java_exe_in_tree(&p, exe);
            let Some(cand) = candidate else {
                continue;
            };
            let (major, minor) = jdk_version_from_dir(&dir_name);
            let newer = best
                .as_ref()
                .map(|(best_major, best_minor, _)| {
                    major > *best_major || (major == *best_major && minor > *best_minor)
                })
                .unwrap_or(true);
            if newer {
                best = Some((major, minor, cand.to_string_lossy().into_owned()));
            }
        }
    }
    best.map(|(_, _, path)| path)
}

/// Standard directories that JDK installers drop their homes into.
fn default_jdk_roots() -> Vec<PathBuf> {
    let mut roots = Vec::new();

    // Check user profile directories (e.g. Zircon Launcher cache, IntelliJ, Minecraft)
    if let Some(user_home) = std::env::var_os("USERPROFILE")
        .or_else(|| std::env::var_os("HOME"))
        .map(PathBuf::from)
    {
        roots.push(user_home.join(".mcmanager").join("launcher"));
        roots.push(user_home.join(".mcmanager").join("runtimes"));
        roots.push(user_home.join(".zircon").join("runtimes"));
        roots.push(user_home.join(".jdks"));
        roots.push(user_home.join(".minecraft").join("runtime"));
    }

    if is_windows() {
        if let Ok(local_app_data) = std::env::var("LOCALAPPDATA") {
            let lad = PathBuf::from(local_app_data);
            roots.push(lad.join("Programs").join("Eclipse Adoptium"));
            roots.push(lad.join("Programs").join("Microsoft"));
            roots.push(lad.join("Programs").join("Java"));
            roots.push(
                lad.join("Packages")
                    .join("Microsoft.4297127D64EC6_8wekyb3d8bbwe")
                    .join("LocalCache")
                    .join("Local")
                    .join("runtime"),
            );
        }
        for var in ["ProgramFiles", "ProgramFiles(x86)"] {
            if let Ok(pf) = std::env::var(var) {
                for sub in [
                    "Java",
                    "Eclipse Adoptium",
                    "Microsoft",
                    "Amazon Corretto",
                    "Zulu",
                    "Semeru",
                ] {
                    roots.push(PathBuf::from(&pf).join(sub));
                }
            }
        }
    } else {
        for path in [
            "/usr/lib/jvm",
            "/usr/java",
            "/opt/java",
            "/Library/Java/JavaVirtualMachines",
        ] {
            roots.push(PathBuf::from(path));
        }
        if let Some(user_home) = std::env::var_os("HOME").map(PathBuf::from) {
            roots.push(user_home.join(".sdkman").join("candidates").join("java"));
            roots.push(user_home.join("Library").join("Java").join("JavaVirtualMachines"));
        }
    }
    roots
}

/// Extracts `(major, minor)` from a JDK directory name such as `jdk-21.0.4+7`
/// or `temurin-17.0.12+7`; legacy `jdk1.8.0_202` names drop the `1` prefix.
/// Unparseable names rank as `(0, 0)` so real JDKs always win.
fn jdk_version_from_dir(name: &str) -> (i64, i64) {
    let mut numbers: Vec<i64> = name
        .split(|c: char| !c.is_ascii_digit())
        .filter(|s| !s.is_empty())
        .filter_map(|s| s.parse().ok())
        .collect();
    if numbers.first() == Some(&1) && numbers.len() >= 2 {
        numbers.remove(0);
    }
    match numbers.as_slice() {
        [major] => (*major, 0),
        [major, minor, ..] => (*major, *minor),
        [] => (0, 0),
    }
}

pub fn is_windows() -> bool {
    std::env::consts::OS == "windows"
}

/// Legacy single-server variant used by the config-driven endpoints.
pub async fn ensure_server_installed_for_config(
    config: &ServerConfig,
    server_dir: &Path,
    server_jar: &Path,
    cache_dir: &Path,
) -> Result<(), InstallError> {
    ensure_server_installed(
        server_dir,
        server_jar,
        cache_dir,
        &config.minecraft_version,
        &config.mod_loader,
    )
    .await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_jdk_versions_from_directory_names() {
        assert_eq!((25, 0), jdk_version_from_dir("jdk-25"));
        assert_eq!((17, 0), jdk_version_from_dir("jdk-17"));
        assert_eq!((21, 0), jdk_version_from_dir("jdk-21.0.4+7"));
        assert_eq!((21, 0), jdk_version_from_dir("temurin-21.0.4+7"));
        assert_eq!((8, 0), jdk_version_from_dir("jdk1.8.0_202"));
        assert_eq!((0, 0), jdk_version_from_dir("tools"));
    }

    #[test]
    fn newest_jdk_wins_among_candidates() {
        let dir = crate::test_util::temp_dir("installer-jdk");
        let root = dir.join("Java");
        fs::create_dir_all(root.join("jdk-17/bin")).unwrap();
        fs::create_dir_all(root.join("jdk-25/bin")).unwrap();
        fs::write(root.join("jdk-17/bin/java.exe"), b"").unwrap();
        fs::write(root.join("jdk-25/bin/java.exe"), b"").unwrap();

        let found = find_newest_jdk_in("java.exe", &[root]).unwrap();
        assert!(found.ends_with("jdk-25/bin/java.exe") || found.ends_with("jdk-25\\bin\\java.exe"));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn fml_version_extraction() {
        let dir = crate::test_util::temp_dir("install");
        fs::create_dir_all(&dir).unwrap();
        let args = dir.join("unix_args.txt");
        fs::write(
            &args,
            "--fml.mcVersion\n1.20.4\n--fml.forgeVersion\n49.0.0\n",
        )
        .unwrap();
        assert_eq!(Some("1.20.4".to_string()), read_fml_mc_version(&args));

        let args2 = dir.join("args2.txt");
        fs::write(&args2, "--fml.mcVersion=1.21.1\n").unwrap();
        assert_eq!(Some("1.21.1".to_string()), read_fml_mc_version(&args2));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn finds_server_args_file_by_loader_version() {
        let dir = crate::test_util::temp_dir("install-args");
        let args_file_name = if super::is_windows() {
            "win_args.txt"
        } else {
            "unix_args.txt"
        };
        let libraries = dir
            .join("libraries")
            .join("net")
            .join("minecraftforge")
            .join("forge")
            .join("1.20.4-49.0.0");
        fs::create_dir_all(&libraries).unwrap();
        let args_file = libraries.join(args_file_name);
        fs::write(&args_file, "--fml.mcVersion\n1.20.4\n").unwrap();

        assert!(find_server_args_file(&dir, "49.0.0").is_some());
        assert!(find_server_args_file(&dir, "50.0.0").is_none());
        let _ = fs::remove_dir_all(&dir);
    }
}
