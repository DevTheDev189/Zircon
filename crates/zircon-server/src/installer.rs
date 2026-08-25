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
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::Deserialize;
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

/// Returns `true` when the server matching the configured loader is already
/// installed — a `server.jar` for vanilla/fabric/quilt, or a launch args file
/// for the *configured* loader version for forge/neoforge.
pub fn is_installed(server_dir: &Path, server_jar: &Path, loader: &ModLoaderInfo) -> bool {
    match ModLoaderType::from_id(&loader.r#type) {
        Some(ModLoaderType::Forge) | Some(ModLoaderType::NeoForge) => {
            find_server_args_file(server_dir, &loader.version).is_some()
        }
        Some(ModLoaderType::Quilt) => {
            server_dir.join("quilt-server-launch.jar").is_file() || server_jar.is_file()
        }
        Some(ModLoaderType::Fabric) | Some(ModLoaderType::Vanilla) => server_jar.is_file(),
        None => false,
    }
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
    let loader_type = ModLoaderType::from_id(&loader.r#type).ok_or_else(|| {
        InstallError::Config(format!(
            "Invalid mod loader '{}'. Allowed loaders: {}",
            loader.r#type,
            ModLoaderType::ALLOWED_IDS.join(", ")
        ))
    })?;
    if is_installed(server_dir, server_jar, loader) {
        tracing::info!(
            "Server for {} is already installed",
            loader_type.id()
        );
    } else {
        tracing::info!(
            "No server installed for loader {} — installing...",
            loader_type.id()
        );
        match loader_type {
            ModLoaderType::Fabric => {
                install_fabric_like(server_jar, mc_version, loader, false).await?
            }
            ModLoaderType::Quilt => {
                install_quilt(server_dir, cache_dir, mc_version, loader).await?
            }
            ModLoaderType::Forge => {
                install_forge_like(server_dir, cache_dir, mc_version, loader, false).await?
            }
            ModLoaderType::NeoForge => {
                install_forge_like(server_dir, cache_dir, mc_version, loader, true).await?
            }
            ModLoaderType::Vanilla => install_vanilla(server_jar, mc_version).await?,
        }
        if !is_installed(server_dir, server_jar, loader) {
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

    fs::create_dir_all(cache_dir)?;
    let installer_jar = cache_dir.join(format!("quilt-installer-{installer_ver}.jar"));
    if !installer_jar.is_file() {
        tracing::info!("Downloading Quilt installer from {installer_url}");
        download(&installer_url, &installer_jar).await?;
    }

    fs::create_dir_all(server_dir)?;
    let java = java_bin();
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
    let exit_code = run_installer(
        &[
            java_bin().as_str(),
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
    let mut child = tokio::process::Command::new(command[0])
        .args(&command[1..])
        .current_dir(working_dir)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(InstallError::Io)?;

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

async fn get_json<T: serde::de::DeserializeOwned>(url: &str) -> Result<T, InstallError> {
    if !ssrf::is_safe_cdn_url(url) {
        return Err(InstallError::Http(format!(
            "Rejected metadata URL (host is not an allowed CDN): {url}"
        )));
    }
    let client = reqwest::Client::new();
    let response = client
        .get(url)
        .timeout(Duration::from_secs(120))
        .send()
        .await
        .map_err(|e| InstallError::Http(format!("GET {url} failed: {e}")))?;
    let status = response.status();
    let body = response
        .text()
        .await
        .map_err(|e| InstallError::Http(format!("GET {url} failed: {e}")))?;
    if !status.is_success() {
        return Err(InstallError::Http(format!(
            "GET {url} failed: HTTP {status}"
        )));
    }
    serde_json::from_str(&body)
        .map_err(|e| InstallError::Json(format!("Invalid JSON from {url}: {e}")))
}

async fn download(url: &str, target: &Path) -> Result<(), InstallError> {
    if !ssrf::is_safe_cdn_url(url) {
        return Err(InstallError::Http(format!(
            "Rejected download URL (host is not an allowed CDN): {url}"
        )));
    }
    let client = reqwest::Client::new();
    let response = client
        .get(url)
        .timeout(Duration::from_secs(10 * 60))
        .send()
        .await
        .map_err(|e| InstallError::Http(format!("Download {url} failed: {e}")))?;
    let status = response.status();
    if !status.is_success() {
        return Err(InstallError::Http(format!(
            "Download {url} failed: HTTP {status}"
        )));
    }
    let bytes = response
        .bytes()
        .await
        .map_err(|e| InstallError::Http(format!("Download {url} failed: {e}")))?;
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(target, bytes)?;
    Ok(())
}

// --------------------------------------------------------------------------
// helpers
// --------------------------------------------------------------------------

/// Resolves the java executable used to run server installers and the
/// Minecraft server itself.
///
/// Preference order:
/// 1. `MC_MANAGER_JAVA_HOME` — explicit override for this wrapper
/// 2. the newest JDK found in the standard install locations (modern Mojang
///    releases require Java 21+, and current NeoForge builds require 25)
/// 3. `$JAVA_HOME` — kept as a fallback for non-standard install locations
/// 4. `java` on PATH
pub fn java_bin() -> String {
    let exe = if is_windows() { "java.exe" } else { "java" };
    if let Some(candidate) = java_home_bin("MC_MANAGER_JAVA_HOME", exe) {
        return candidate;
    }
    if let Some(candidate) = find_newest_jdk(exe) {
        return candidate;
    }
    if let Some(candidate) = java_home_bin("JAVA_HOME", exe) {
        return candidate;
    }
    exe.to_string()
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
            let candidate = entry.path().join("bin").join(exe);
            if !candidate.is_file() {
                continue;
            }
            let (major, minor) = jdk_version_from_dir(&dir_name);
            let newer = best
                .as_ref()
                .map(|(best_major, best_minor, _)| {
                    major > *best_major || (major == *best_major && minor > *best_minor)
                })
                .unwrap_or(true);
            if newer {
                best = Some((major, minor, candidate.to_string_lossy().into_owned()));
            }
        }
    }
    best.map(|(_, _, path)| path)
}

/// Standard directories that JDK installers drop their homes into.
fn default_jdk_roots() -> Vec<PathBuf> {
    let mut roots = Vec::new();
    if is_windows() {
        for var in ["ProgramFiles", "ProgramFiles(x86)"] {
            if let Ok(pf) = std::env::var(var) {
                for sub in [
                    "Java",
                    "Eclipse Adoptium",
                    "Microsoft",
                    "Amazon Corretto",
                    "Zulu",
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
