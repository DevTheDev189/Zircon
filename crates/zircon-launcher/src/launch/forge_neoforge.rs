//! End-to-end launch preparation for Forge and NeoForge: runs the official
//! headless installer into a dedicated install dir, parses the generated
//! `versions/<id>/<id>.json` profile, resolves the `inheritsFrom` chain against
//! the vanilla profile, stages the loader's libraries and artifacts into the
//! unified libraries dir, and resolves the profile's JVM and game arguments
//! (with `${token}` substitution). Also ports the installer plumbing it drives:
//! `ForgeInstaller`, `NeoForgeInstaller` and `LoaderInstallSupport`.
//!
//! Port of com.mcmanager.client.launch.ForgeLaunchResolver

use std::collections::{HashMap, HashSet};
use std::path::{Component, Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;

use crate::error::LauncherError;
use crate::launch::java::{java_executable, JavaRuntimeResolver, JavaRuntimeSelector};
use crate::launch::profile;
use crate::model::version::LibrarySpec;

/// Upper bound for the headless installer, mirroring the Java `INSTALL_TIMEOUT`
/// (15 minutes).
const INSTALL_TIMEOUT: Duration = Duration::from_secs(15 * 60);

/// Per-request download timeout, mirroring `LoaderInstallSupport` (10 minutes).
const DOWNLOAD_TIMEOUT: Duration = Duration::from_secs(10 * 60);

/// Everything the launcher needs beyond the vanilla resolution.
#[derive(Debug, Clone)]
pub struct ForgeLaunchData {
    pub main_class: String,
    pub jvm_args: Vec<String>,
    pub game_args: Vec<String>,
}

/// End-to-end launch preparation for Forge and NeoForge.
pub struct ForgeLaunchResolver;

impl ForgeLaunchResolver {
    pub fn new() -> Self {
        ForgeLaunchResolver
    }
}

impl Default for ForgeLaunchResolver {
    fn default() -> Self {
        Self::new()
    }
}

impl ForgeLaunchResolver {
    /// Prepares a Forge/NeoForge launch.
    ///
    /// * `cache_dir` — the launcher cache (holds `versions/`, `libraries/`,
    ///   `install/`).
    /// * `mc_version` — Minecraft version, e.g. `"1.20.4"`.
    /// * `loader_type` — the lowercase loader id: `"forge"` or `"neoforge"`.
    /// * `loader_version` — loader version, e.g. `"47.2.0"` (Forge) or
    ///   `"20.4.250"` (NeoForge).
    /// * `vanilla_version_json` — path to the vanilla version profile JSON
    ///   (already downloaded).
    /// * `vanilla_client_jar` — path to the vanilla client JAR (already
    ///   downloaded).
    /// * `natives_dir` — extracted natives dir (used only for token
    ///   substitution).
    /// * `classpath` — the in-progress classpath; the loader libraries are
    ///   appended. The patched client JAR is staged into `libraries/` but never
    ///   appended here (the classpath builder swaps it in for the vanilla
    ///   client JAR).
    #[allow(clippy::too_many_arguments)] // fixed cross-module contract with the classpath builder
    pub async fn resolve(
        &self,
        cache_dir: &Path,
        mc_version: &str,
        loader_type: &str,
        loader_version: &str,
        vanilla_version_json: &Path,
        vanilla_client_jar: &Path,
        natives_dir: &Path,
        classpath: &mut Vec<PathBuf>,
    ) -> Result<ForgeLaunchData, LauncherError> {
        // --- 1. headless installation ---
        let install_dir = cache_dir
            .join("install")
            .join(format!("{loader_type}-{mc_version}-{loader_version}"));
        prepare_install_dir(
            &install_dir,
            mc_version,
            vanilla_version_json,
            vanilla_client_jar,
        )?;
        let client = reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(15))
            .build()?;
        install_loader(
            &client,
            cache_dir,
            mc_version,
            loader_type,
            loader_version,
            &install_dir,
        )
        .await?;

        // --- 2. parse the generated version profile ---
        let profile_json =
            find_version_profile_json(&install_dir, mc_version).ok_or_else(|| {
                LauncherError::NotFound(format!(
                    "Loader profile JSON not found after installation in {}",
                    install_dir.display()
                ))
            })?;
        let content = std::fs::read_to_string(&profile_json)?;
        let root = profile::parse_profile(&content)?;
        tracing::info!(
            "Parsed loader profile '{}' (mainClass={}, inheritsFrom={})",
            root.id,
            root.main_class,
            root.inherits_from.as_deref().unwrap_or("")
        );

        // The loader version encodes its Minecraft version (e.g. NeoForge
        // 20.4.250 = MC 1.20.4). Refuse to launch when the server manifest
        // disagrees — a mismatched game jar + vanilla libraries crashes with
        // confusing NoSuchMethodError/classloader errors.
        validate_mc_version(root.inherits_from.as_deref(), mc_version, loader_version)?;

        // --- 3. resolve the inheritance chain (loader -> vanilla) ---
        let chain = profile::resolve_chain(&root, |parent_id| {
            let parent_json = if parent_id == mc_version && vanilla_version_json.is_file() {
                vanilla_version_json.to_path_buf()
            } else {
                cache_dir
                    .join("versions")
                    .join(parent_id)
                    .join(format!("{parent_id}.json"))
            };
            let content = std::fs::read_to_string(&parent_json)?;
            profile::parse_profile(&content)
        });

        // --- 4. libraries ---
        let libraries_dir = cache_dir.join("libraries");
        for lib in profile::merged_libraries(&chain) {
            let Some(artifact_path) = lib.artifact_path() else {
                tracing::warn!("Skipping library with unparseable coordinate: {}", lib.name);
                continue;
            };
            let target = absolute_normalized(&libraries_dir.join(&artifact_path));
            if is_non_empty_file(&target) {
                // Present on disk: add it to the classpath unless the vanilla
                // library loop already did (avoids duplicates while keeping
                // loader-only libs on the classpath across repeated launches).
                if !classpath_contains(classpath, &target) {
                    classpath.push(target);
                }
                continue;
            }
            stage_library(
                &client,
                &install_dir,
                &lib,
                &artifact_path,
                &target,
                classpath,
            )
            .await;
        }

        // --- 5. loader artifacts ---
        // The loader's own jars must NOT go on the classpath. FML's
        // RequiredSystemFiles treats game + loader classes found on -cp as a
        // merged IDE/dev environment and then runs NeoForgeDevDistCleaner, which
        // demands a Minecraft-Dists manifest attribute that production jars
        // don't carry (that attribute is generated by NeoGradle only) — aborting
        // with "NeoForge dev environment Minecraft jar does not have a
        // Minecraft-Dists attribute".
        //
        // Instead the artifacts are staged into the unified libraries dir under
        // their maven-relative paths so GameLocator.locateProductionMinecraft
        // can find them via -DlibraryDirectory (that path does not run the dist
        // cleaner). This covers both eras:
        //   *-universal.jar              -> the loader mod container
        //   *-client.jar                 -> patched game jar (Forge/NeoForge < 1.26)
        //   minecraft-client-patched.jar -> patched game jar (NeoForge 26+)
        //   client-<mcp>-srg/-extra.jar  -> split client partials (1.20.x-era)
        let loader_artifact_dir =
            loader_artifact_dir(&install_dir, loader_type, mc_version, loader_version);
        for universal_jar in find_jars(&loader_artifact_dir, "-universal.jar") {
            let _ = stage_loader_artifact(&libraries_dir, &install_dir, &universal_jar);
        }

        let patched_game_jar = find_jars(&loader_artifact_dir, "-client.jar")
            .into_iter()
            .next()
            .or_else(|| minecraft_client_patched(&install_dir, loader_version))
            .or_else(|| find_patched_client_jar(&install_dir, &root.id));
        if let Some(patched_game_jar) = patched_game_jar {
            stage_loader_artifact(&libraries_dir, &install_dir, &patched_game_jar);
            tracing::info!(
                "Staged patched game jar {} into libraries",
                patched_game_jar
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_else(|| patched_game_jar.display().to_string())
            );
        } else {
            tracing::warn!(
                "No patched game jar found — production locator will use the vanilla client jar"
            );
        }

        // 1.20.x-era: locateProductionMinecraft assembles the game from the
        // split srg + extra client partials in the libraries dir.
        stage_minecraft_client_partials(&install_dir, &libraries_dir);

        // --- 6. arguments ---
        let tokens = build_tokens(&libraries_dir, natives_dir, &root.id);
        let mut jvm_args = profile::resolve_jvm_arguments(&chain, &tokens);
        // The vanilla profile contributes "-cp ${classpath}"; the launcher
        // injects the classpath itself (the runner adds -cp uniformly for every
        // loader), so drop the template pair to avoid a duplicate -cp.
        remove_classpath_pair(&mut jvm_args);

        // This launcher always auto-connects the player to a server, so the
        // "is_quick_play_multiplayer" feature is enabled (the game arg template
        // ${quickPlayMultiplayer} is filled in at launch time with host:port).
        let enabled_features: HashSet<String> =
            HashSet::from(["is_quick_play_multiplayer".to_string()]);
        let game_args = profile::resolve_game_arguments(&chain, &tokens, &enabled_features);

        let main_class = root.main_class.clone();
        if main_class.trim().is_empty() {
            return Err(LauncherError::Parse(format!(
                "Profile {} declares no mainClass",
                root.id
            )));
        }

        tracing::info!(
            "Forge/NeoForge launch prepared: mainClass={}, jvmArgs={}, gameArgs={}",
            main_class,
            jvm_args.len(),
            game_args.len()
        );
        Ok(ForgeLaunchData {
            main_class,
            jvm_args,
            game_args,
        })
    }
}

// ---------------------------------------------------------------------------
// Installer download + execution (ports ForgeInstaller / NeoForgeInstaller and
// ProcessExecutionHelper)
// ---------------------------------------------------------------------------

/// Downloads the installer JAR (when not cached) and runs it headlessly against
/// `install_dir`, mirroring the per-loader `install(...)` strategy. Skips
/// entirely when the version profile JSON already exists.
async fn install_loader(
    client: &reqwest::Client,
    cache_dir: &Path,
    mc_version: &str,
    loader_type: &str,
    loader_version: &str,
    install_dir: &Path,
) -> Result<(), LauncherError> {
    let label = if loader_type == "neoforge" {
        "NeoForge"
    } else {
        "Forge"
    };
    if find_version_profile_json(install_dir, mc_version).is_some() {
        if loader_type == "neoforge" {
            tracing::info!(
                "NeoForge {} for MC {} is already installed.",
                loader_version,
                mc_version
            );
        } else {
            tracing::info!(
                "Forge {}-{} is already installed.",
                mc_version,
                loader_version
            );
        }
        return Ok(());
    }

    let (download_url, jar_name, flag) = installer_plan(loader_type, mc_version, loader_version)?;
    let installer_cache_dir = cache_dir.join(".installers");
    let installer_jar = installer_cache_dir.join(jar_name);
    if !is_non_empty_file(&installer_jar) {
        tracing::info!("Downloading {} installer from {}", label, download_url);
        download_if_missing(client, &download_url, &installer_jar).await?;
    }

    tracing::info!(
        "Running {} installer headlessly into {}...",
        label,
        install_dir.display()
    );
    let required_java = JavaRuntimeSelector::get_required_java_major_version(mc_version);
    // The installer is itself a Java process, so it cannot bootstrap its own
    // runtime. Provision Java up front — system Java, else the cached runtime,
    // else a one-time Adoptium download — exactly like the game launch does.
    // Falling back to a bare `java` on PATH would fail on machines with no
    // Java installed, blocking Forge/NeoForge users before they ever reach the
    // provisioning step.
    let java_home = JavaRuntimeResolver::new(cache_dir.to_path_buf())
        .resolve(required_java)
        .await?;
    let java = java_executable(&java_home);
    run_installer(
        &java,
        &installer_jar,
        flag,
        label,
        install_dir,
        &installer_cache_dir,
    )
    .await?;

    if find_version_profile_json(install_dir, mc_version).is_none() {
        return Err(LauncherError::Process(format!(
            "{label} installer reported success but produced no version profile"
        )));
    }
    if loader_type == "neoforge" {
        tracing::info!("NeoForge {} installed successfully.", loader_version);
    } else {
        tracing::info!(
            "Forge {}-{} installed successfully.",
            mc_version,
            loader_version
        );
    }
    Ok(())
}

/// The Maven URL, cached JAR file name and install flag per loader:
///
/// * Forge: `https://maven.minecraftforge.net/net/minecraftforge/forge/<mc>-<ver>/
///   forge-<mc>-<ver>-installer.jar` run with `--installClient`.
/// * NeoForge: `https://maven.neoforged.net/releases/net/neoforged/neoforge/<ver>/
///   neoforge-<ver>-installer.jar` run with `--install-client`.
fn installer_plan(
    loader_type: &str,
    mc_version: &str,
    loader_version: &str,
) -> Result<(String, String, &'static str), LauncherError> {
    match loader_type {
        "forge" => {
            let full_version = format!("{mc_version}-{loader_version}");
            Ok((
                format!(
                    "https://maven.minecraftforge.net/net/minecraftforge/forge/{full_version}/forge-{full_version}-installer.jar"
                ),
                format!("forge-{full_version}-installer.jar"),
                "--installClient",
            ))
        }
        "neoforge" => Ok((
            format!(
                "https://maven.neoforged.net/releases/net/neoforged/neoforge/{loader_version}/neoforge-{loader_version}-installer.jar"
            ),
            format!("neoforge-{loader_version}-installer.jar"),
            "--install-client",
        )),
        other => Err(LauncherError::InvalidInput(format!(
            "Unsupported mod loader type '{other}' — expected 'forge' or 'neoforge'"
        ))),
    }
}

/// Runs `java -jar <installer> <flag> <installDir>` with stdout+stderr captured
/// together. On a non-zero exit (or timeout) returns [`LauncherError::Process`]
/// with the captured output tail, mirroring `ProcessExecutionHelper.runProcess`
/// plus the installers' non-zero exit checks.
async fn run_installer(
    java: &Path,
    installer_jar: &Path,
    flag: &str,
    label: &str,
    install_dir: &Path,
    working_dir: &Path,
) -> Result<(), LauncherError> {
    tracing::info!(
        "Executing command: {} -jar {} {} {}",
        java.display(),
        installer_jar.display(),
        flag,
        install_dir.display()
    );
    let mut command = tokio::process::Command::new(java);
    command
        .arg("-jar")
        .arg(installer_jar)
        .arg(flag)
        .arg(install_dir)
        .current_dir(working_dir)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = command
        .spawn()
        .map_err(|e| LauncherError::Process(format!("Could not start {label} installer: {e}")))?;

    // Drain both pipes concurrently (mirroring the Java output pump) so a
    // chatty installer can never deadlock the pipe buffers while we wait.
    let mut stdout = child.stdout.take().expect("installer stdout is piped");
    let mut stderr = child.stderr.take().expect("installer stderr is piped");
    let stdout_task = tokio::spawn(async move {
        let mut buffer = Vec::new();
        let _ = tokio::io::copy(&mut stdout, &mut buffer).await;
        buffer
    });
    let stderr_task = tokio::spawn(async move {
        let mut buffer = Vec::new();
        let _ = tokio::io::copy(&mut stderr, &mut buffer).await;
        buffer
    });

    let status = match tokio::time::timeout(INSTALL_TIMEOUT, child.wait()).await {
        Ok(status) => status.map_err(|e| {
            LauncherError::Process(format!("Failed waiting for {label} installer: {e}"))
        })?,
        Err(_elapsed) => {
            let _ = child.kill().await;
            let _ = stdout_task.await;
            let _ = stderr_task.await;
            return Err(LauncherError::Process(format!(
                "{label} installer did not finish within {} minutes and was killed",
                INSTALL_TIMEOUT.as_secs() / 60
            )));
        }
    };

    let stdout_buffer = stdout_task.await.unwrap_or_default();
    let stderr_buffer = stderr_task.await.unwrap_or_default();
    let mut combined = stdout_buffer;
    combined.extend_from_slice(&stderr_buffer);
    let output = String::from_utf8_lossy(&combined);
    for line in output.lines() {
        tracing::info!("[Installer Output] {line}");
    }
    let exit_code = status.code().unwrap_or(-1);
    tracing::info!("Process finished with exit code: {exit_code}");
    if !status.success() {
        return Err(LauncherError::Process(format!(
            "{label} installer failed with exit code {exit_code}; captured output tail:\n{}",
            tail_of(&output, 40)
        )));
    }
    Ok(())
}

/// The last `max_lines` lines of `text`.
fn tail_of(text: &str, max_lines: usize) -> String {
    let lines: Vec<&str> = text.lines().collect();
    let start = lines.len().saturating_sub(max_lines);
    lines[start..].join("\n")
}

// ---------------------------------------------------------------------------
// Install directory preparation (ports LoaderInstallSupport)
// ---------------------------------------------------------------------------

/// Creates the minimum `.minecraft`-style layout the official installers accept
/// before they will run:
///
/// ```text
/// installDir/
///   launcher_profiles.json
///   versions/<mc>/<mc>.json
///   versions/<mc>/<mc>.jar
/// ```
fn prepare_install_dir(
    install_dir: &Path,
    mc_version: &str,
    vanilla_version_json: &Path,
    vanilla_client_jar: &Path,
) -> Result<(), LauncherError> {
    let versions_dir = install_dir.join("versions").join(mc_version);
    std::fs::create_dir_all(&versions_dir)?;

    let launcher_profiles = install_dir.join("launcher_profiles.json");
    if !launcher_profiles.exists() {
        std::fs::write(
            &launcher_profiles,
            "{\"profiles\":{},\"settings\":{},\"version\":3}",
        )?;
    }
    let version_json = versions_dir.join(format!("{mc_version}.json"));
    if !version_json.exists() && vanilla_version_json.is_file() {
        std::fs::copy(vanilla_version_json, &version_json)?;
    }
    let client_jar = versions_dir.join(format!("{mc_version}.jar"));
    if !client_jar.exists() && vanilla_client_jar.is_file() {
        std::fs::copy(vanilla_client_jar, &client_jar)?;
    }
    Ok(())
}

/// The loader's artifact directory inside an install dir, e.g.
/// `libraries/net/neoforged/neoforge/<loaderVersion>` (NeoForge) or
/// `libraries/net/minecraftforge/forge/<mc>-<loaderVersion>` (Forge).
fn loader_artifact_dir(
    install_dir: &Path,
    loader_type: &str,
    mc_version: &str,
    loader_version: &str,
) -> PathBuf {
    if loader_type == "neoforge" {
        install_dir
            .join("libraries/net/neoforged/neoforge")
            .join(loader_version)
    } else {
        install_dir
            .join("libraries/net/minecraftforge/forge")
            .join(format!("{mc_version}-{loader_version}"))
    }
}

/// Lists JARs directly inside `dir` whose file name ends with `suffix`
/// (e.g. `-universal.jar`, `-client.jar`).
fn find_jars(dir: &Path, suffix: &str) -> Vec<PathBuf> {
    if !dir.is_dir() {
        return Vec::new();
    }
    match std::fs::read_dir(dir) {
        Ok(entries) => entries
            .flatten()
            .map(|entry| entry.path())
            .filter(|path| path.is_file())
            .filter(|path| {
                path.file_name()
                    .map(|name| name.to_string_lossy().ends_with(suffix))
                    .unwrap_or(false)
            })
            .collect(),
        Err(e) => {
            tracing::warn!(
                "Could not scan {} for *{} jars: {}",
                dir.display(),
                suffix,
                e
            );
            Vec::new()
        }
    }
}

// ---------------------------------------------------------------------------
// Locating installer output (ports LoaderInstallSupport)
// ---------------------------------------------------------------------------

/// Locates the loader's version profile JSON in a prepared install dir, e.g.
/// `versions/neoforge-20.4.250/neoforge-20.4.250.json`.
///
/// The profile directory name is not predictable across loader generations
/// (NeoForge pre-1.20.2 used `<mc>-forge-<ver>`, later builds use
/// `neoforge-<ver>`), so any non-vanilla version directory with a matching
/// `<name>.json` is accepted; the largest file wins.
fn find_version_profile_json(install_dir: &Path, mc_version: &str) -> Option<PathBuf> {
    let versions_dir = install_dir.join("versions");
    if !versions_dir.is_dir() {
        return None;
    }
    let mut best: Option<PathBuf> = None;
    let mut best_size: u64 = 0;
    match std::fs::read_dir(&versions_dir) {
        Ok(entries) => {
            for entry in entries.flatten() {
                let dir = entry.path();
                if !dir.is_dir() {
                    continue;
                }
                let Some(name) = dir.file_name().map(|n| n.to_string_lossy().to_string()) else {
                    continue;
                };
                if name == mc_version {
                    continue;
                }
                let profile = dir.join(format!("{name}.json"));
                if !profile.is_file() {
                    continue;
                }
                let size = std::fs::metadata(&profile).map(|m| m.len()).unwrap_or(0);
                if size > best_size {
                    best_size = size;
                    best = Some(profile);
                }
            }
        }
        Err(e) => {
            tracing::warn!(
                "Could not scan version profiles in {}: {}",
                versions_dir.display(),
                e
            );
            return None;
        }
    }
    best
}

/// Locates the patched client JAR the installer produced, e.g.
/// `libraries/net/neoforged/neoforge/20.4.250/neoforge-20.4.250-client.jar`.
///
/// Modern Forge/NeoForge put the runnable game jar in the libraries directory
/// with a `-client` classifier; some older Forge builds write
/// `versions/<id>/<id>.jar` instead, which is checked first.
fn find_patched_client_jar(install_dir: &Path, profile_id: &str) -> Option<PathBuf> {
    let legacy = install_dir
        .join("versions")
        .join(profile_id)
        .join(format!("{profile_id}.jar"));
    if legacy.is_file() {
        return Some(legacy);
    }
    let libraries_dir = install_dir.join("libraries");
    if !libraries_dir.is_dir() {
        return None;
    }
    let mut pending: Vec<PathBuf> = vec![libraries_dir.clone()];
    while let Some(dir) = pending.pop() {
        match std::fs::read_dir(&dir) {
            Ok(entries) => {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.is_dir() {
                        pending.push(path);
                    } else if path.is_file()
                        && path
                            .file_name()
                            .map(|n| n.to_string_lossy().ends_with("-client.jar"))
                            .unwrap_or(false)
                    {
                        return Some(path);
                    }
                }
            }
            Err(e) => {
                tracing::warn!(
                    "Could not scan libraries in {}: {}",
                    libraries_dir.display(),
                    e
                );
                return None;
            }
        }
    }
    None
}

// ---------------------------------------------------------------------------
// Library / artifact staging
// ---------------------------------------------------------------------------

/// Ensures a loader library is present in the unified libraries dir — copied
/// from the installer output when available, otherwise downloaded from the
/// profile's artifact URL — and appends it to the classpath. Failures are
/// logged and the library skipped (mirroring the Java try/catch-warn).
async fn stage_library(
    client: &reqwest::Client,
    install_dir: &Path,
    lib: &LibrarySpec,
    artifact_path: &str,
    target: &Path,
    classpath: &mut Vec<PathBuf>,
) {
    let installed = install_dir.join("libraries").join(artifact_path);
    if installed.is_file() {
        if let Err(e) = (|| -> std::io::Result<()> {
            if let Some(parent) = target.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::copy(&installed, target)?;
            Ok(())
        })() {
            tracing::warn!("Could not stage library {}: {}", lib.name, e);
            return;
        }
        classpath.push(target.to_path_buf());
        return;
    }
    match lib.download_url() {
        Some(url) => {
            if let Err(e) = download_if_missing(client, &url, target).await {
                tracing::warn!("Could not stage library {}: {}", lib.name, e);
                return;
            }
            classpath.push(target.to_path_buf());
        }
        None => tracing::warn!(
            "Library {} has no download URL and was not installed — skipping",
            lib.name
        ),
    }
}

/// Copies a loader artifact (e.g. the `-universal.jar`) from the installer
/// output into the unified libraries dir under its maven-relative path,
/// returning the staged copy. Locators that scan the library directory (and
/// FML's classpath scanner) can both find it there.
fn stage_loader_artifact(
    libraries_dir: &Path,
    install_dir: &Path,
    artifact: &Path,
) -> Option<PathBuf> {
    let relative = relativize(&install_dir.join("libraries"), artifact);
    let target = absolute_normalized(&libraries_dir.join(relative));
    if is_non_empty_file(&target) {
        return Some(target);
    }
    let parent = target.parent()?;
    if let Err(e) = std::fs::create_dir_all(parent) {
        tracing::warn!(
            "Could not stage loader artifact {}: {}",
            artifact.display(),
            e
        );
        return None;
    }
    if let Err(e) = std::fs::copy(artifact, &target) {
        tracing::warn!(
            "Could not stage loader artifact {}: {}",
            artifact.display(),
            e
        );
        return None;
    }
    Some(target)
}

/// Stages the 1.20.x-era split client partials (`client-<mcp>-srg.jar` and
/// `client-<mcp>-extra.jar`) from the installer output into the unified
/// libraries dir. FML's `locateProductionMinecraft` assembles the game from
/// these when no `minecraft-client-patched` artifact exists for the loader
/// version.
fn stage_minecraft_client_partials(install_dir: &Path, libraries_dir: &Path) {
    let client_dir = install_dir.join("libraries/net/minecraft/client");
    if !client_dir.is_dir() {
        return;
    }
    let mcp_dirs: Vec<PathBuf> = match std::fs::read_dir(&client_dir) {
        Ok(entries) => entries.flatten().map(|entry| entry.path()).collect(),
        Err(e) => {
            tracing::warn!(
                "Could not scan client partials in {}: {}",
                client_dir.display(),
                e
            );
            return;
        }
    };
    for mcp_dir in mcp_dirs {
        for suffix in ["-srg.jar", "-extra.jar"] {
            for partial in find_jars(&mcp_dir, suffix) {
                let _ = stage_loader_artifact(libraries_dir, install_dir, &partial);
            }
        }
    }
}

/// NeoForge 26+ publishes the patched game jar as a dedicated artifact:
/// `libraries/net/neoforged/minecraft-client-patched/<ver>/minecraft-client-patched-<ver>.jar`.
fn minecraft_client_patched(install_dir: &Path, loader_version: &str) -> Option<PathBuf> {
    let patched = install_dir
        .join("libraries/net/neoforged/minecraft-client-patched")
        .join(loader_version)
        .join(format!("minecraft-client-patched-{loader_version}.jar"));
    patched.is_file().then_some(patched)
}

// ---------------------------------------------------------------------------
// Downloads
// ---------------------------------------------------------------------------

/// Downloads `url` to `target`, skipping the download when the file already
/// exists and is non-empty. Writes to a temp file first and renames it into
/// place so an interrupted download never leaves a truncated file that the
/// size>0 check would otherwise accept.
async fn download_if_missing(
    client: &reqwest::Client,
    url: &str,
    target: &Path,
) -> Result<(), LauncherError> {
    if is_non_empty_file(target) {
        return Ok(());
    }
    let parent = target.parent().ok_or_else(|| {
        LauncherError::InvalidInput(format!(
            "Cannot download to {}: no parent directory",
            target.display()
        ))
    })?;
    tokio::fs::create_dir_all(parent).await?;
    let response = client.get(url).timeout(DOWNLOAD_TIMEOUT).send().await?;
    let status = response.status();
    if !status.is_success() {
        return Err(LauncherError::Http {
            status: status.as_u16(),
            url: url.to_string(),
        });
    }
    let bytes = response.bytes().await?;
    let file_name = target
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "download".to_string());
    let temp = parent.join(format!(".{file_name}.part"));
    tokio::fs::write(&temp, &bytes).await?;
    if target.exists() {
        tokio::fs::remove_file(target).await?;
    }
    tokio::fs::rename(&temp, target).await?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Tokens / arguments
// ---------------------------------------------------------------------------

/// Builds the `${token}` substitution map for the profile argument resolvers,
/// mirroring the Java `tokens` map exactly.
pub(crate) fn build_tokens(
    libraries_dir: &Path,
    natives_dir: &Path,
    version_name: &str,
) -> HashMap<String, String> {
    let mut tokens = HashMap::new();
    tokens.insert(
        "library_directory".to_string(),
        libraries_dir.to_string_lossy().to_string(),
    );
    tokens.insert(
        "classpath_separator".to_string(),
        if cfg!(windows) { ";" } else { ":" }.to_string(),
    );
    tokens.insert("version_name".to_string(), version_name.to_string());
    tokens.insert("launcher_name".to_string(), "mcmanager".to_string());
    tokens.insert("launcher_version".to_string(), "1.0.0".to_string());
    tokens.insert(
        "natives_directory".to_string(),
        natives_dir.to_string_lossy().to_string(),
    );
    tokens
}

/// Removes the first `-cp ${classpath}` template pair from resolved JVM args
/// (the runner injects `-cp` uniformly, so a second one would be a duplicate).
fn remove_classpath_pair(jvm_args: &mut Vec<String>) {
    if let Some(index) = jvm_args
        .windows(2)
        .position(|pair| pair[0] == "-cp" && pair[1] == "${classpath}")
    {
        jvm_args.drain(index..index + 2);
    }
}

/// The loader version encodes its Minecraft version (e.g. NeoForge 20.4.250 =
/// MC 1.20.4). Refuse to launch when the server manifest disagrees — a
/// mismatched game jar + vanilla libraries crashes with confusing
/// NoSuchMethodError/classloader errors.
fn validate_mc_version(
    profile_mc_version: Option<&str>,
    mc_version: &str,
    loader_version: &str,
) -> Result<(), LauncherError> {
    if let Some(profile_mc_version) = profile_mc_version {
        if !profile_mc_version.trim().is_empty() && profile_mc_version != mc_version {
            return Err(LauncherError::InvalidInput(format!(
                "Loader version {loader_version} targets Minecraft {profile_mc_version} but the server manifest declares {mc_version}. Fix 'minecraftVersion' and 'modLoader.version' in the server config so they describe the same Minecraft version."
            )));
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Path helpers
// ---------------------------------------------------------------------------

/// Java `Path.toAbsolutePath().normalize()` — lexical, never touches the
/// filesystem (so it also works for paths that do not exist yet).
fn absolute_normalized(path: &Path) -> PathBuf {
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("."))
            .join(path)
    };
    let mut normalized = PathBuf::new();
    for component in absolute.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                normalized.pop();
            }
            other => normalized.push(other.as_os_str()),
        }
    }
    normalized
}

/// Java `Path.relativize` semantics: the relative path from `base` to `path`,
/// emitting `..` components even when the two share no common prefix.
fn relativize(base: &Path, path: &Path) -> PathBuf {
    let base_components: Vec<Component<'_>> = base.components().collect();
    let path_components: Vec<Component<'_>> = path.components().collect();
    let mut common = 0;
    while common < base_components.len()
        && common < path_components.len()
        && base_components[common] == path_components[common]
    {
        common += 1;
    }
    let mut relative = PathBuf::new();
    for _ in common..base_components.len() {
        relative.push("..");
    }
    for component in &path_components[common..] {
        relative.push(component.as_os_str());
    }
    relative
}

/// `true` when `path` is a regular file with at least one byte.
fn is_non_empty_file(path: &Path) -> bool {
    path.is_file()
        && std::fs::metadata(path)
            .map(|meta| meta.len() > 0)
            .unwrap_or(false)
}

/// Whether `target` (normalized absolute) is already on the classpath, where
/// existing entries are compared by their normalized absolute form too.
fn classpath_contains(classpath: &[PathBuf], target: &Path) -> bool {
    let target = absolute_normalized(target);
    classpath
        .iter()
        .any(|entry| absolute_normalized(entry) == target)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn removes_duplicate_cp_classpath_pair() {
        let mut args = vec![
            "-Xmx2G".to_string(),
            "-cp".to_string(),
            "${classpath}".to_string(),
            "net.minecraft.client.main.Main".to_string(),
        ];
        remove_classpath_pair(&mut args);
        assert_eq!(vec!["-Xmx2G", "net.minecraft.client.main.Main"], args);
    }

    #[test]
    fn removes_only_the_first_cp_pair() {
        let mut args = vec![
            "-cp".to_string(),
            "${classpath}".to_string(),
            "-cp".to_string(),
            "${classpath}".to_string(),
        ];
        remove_classpath_pair(&mut args);
        assert_eq!(vec!["-cp", "${classpath}"], args);
    }

    #[test]
    fn leaves_jvm_args_untouched_without_cp_pair() {
        let mut args = vec!["-Xmx2G".to_string(), "-Dfoo=bar".to_string()];
        remove_classpath_pair(&mut args);
        assert_eq!(vec!["-Xmx2G", "-Dfoo=bar"], args);
    }

    #[test]
    fn detects_mismatched_minecraft_version() {
        let err = validate_mc_version(Some("1.20.4"), "1.21.1", "20.4.250").unwrap_err();
        let message = match err {
            LauncherError::InvalidInput(m) => m,
            other => panic!("expected InvalidInput, got {other:?}"),
        };
        assert!(message.contains("1.20.4"), "message: {message}");
        assert!(message.contains("1.21.1"), "message: {message}");
    }

    #[test]
    fn accepts_matching_or_absent_inherits_from() {
        assert!(validate_mc_version(Some("1.20.4"), "1.20.4", "47.2.0").is_ok());
        assert!(validate_mc_version(None, "1.20.4", "47.2.0").is_ok());
        assert!(validate_mc_version(Some("  "), "1.20.4", "47.2.0").is_ok());
    }

    #[test]
    fn token_map_contains_launcher_tokens() {
        let libraries = PathBuf::from("cache").join("libraries");
        let natives = PathBuf::from("cache").join("natives");
        let tokens = build_tokens(&libraries, &natives, "neoforge-20.4.250");
        assert_eq!(libraries.to_string_lossy(), tokens["library_directory"]);
        assert_eq!(natives.to_string_lossy(), tokens["natives_directory"]);
        assert_eq!("neoforge-20.4.250", tokens["version_name"]);
        assert_eq!("mcmanager", tokens["launcher_name"]);
        assert_eq!("1.0.0", tokens["launcher_version"]);
        assert_eq!(
            if cfg!(windows) { ";" } else { ":" },
            tokens["classpath_separator"]
        );
    }

    #[test]
    fn relativize_matches_java_semantics() {
        let libraries = Path::new("install/libraries");
        let artifact = Path::new(
            "install/libraries/net/minecraftforge/forge/1.20.1-47.2.0/forge-1.20.1-47.2.0-universal.jar",
        );
        let expected: PathBuf = Path::new("net")
            .join("minecraftforge")
            .join("forge")
            .join("1.20.1-47.2.0")
            .join("forge-1.20.1-47.2.0-universal.jar");
        assert_eq!(expected, relativize(libraries, artifact));

        // Paths sharing no common prefix still relativize (with `..` hops),
        // which is how the legacy versions/<id>/<id>.jar gets staged.
        let legacy = Path::new("install/versions/neoforge-20.4.250/neoforge-20.4.250.jar");
        let expected: PathBuf = Path::new("..")
            .join("versions")
            .join("neoforge-20.4.250")
            .join("neoforge-20.4.250.jar");
        assert_eq!(expected, relativize(libraries, legacy));
    }

    #[test]
    fn absolute_normalized_collapses_dotdot_lexically() {
        let cwd = std::env::current_dir().unwrap();
        let raw = cwd.join("libraries/../libraries/net/x.jar");
        assert_eq!(cwd.join("libraries/net/x.jar"), absolute_normalized(&raw));
    }

    #[test]
    fn classpath_dedup_compares_normalized_paths() {
        let cwd = std::env::current_dir().unwrap();
        let target = cwd.join("libraries/net/a.jar");
        let classpath = vec![cwd.join("libraries/../libraries/net/a.jar")];
        assert!(classpath_contains(&classpath, &target));
    }

    #[test]
    fn find_version_profile_json_picks_largest_loader_profile() {
        let base =
            std::env::temp_dir().join(format!("zircon-forge-profile-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        let install_dir = base.join("install");
        let vanilla = install_dir.join("versions").join("1.20.4");
        let loader_a = install_dir.join("versions").join("neoforge-20.4.250");
        let loader_b = install_dir.join("versions").join("forge-47.2.0");
        std::fs::create_dir_all(&vanilla).unwrap();
        std::fs::create_dir_all(&loader_a).unwrap();
        std::fs::create_dir_all(&loader_b).unwrap();
        std::fs::write(vanilla.join("1.20.4.json"), r#"{"id":"1.20.4"}"#).unwrap();
        std::fs::write(
            loader_a.join("neoforge-20.4.250.json"),
            r#"{"id":"neoforge-20.4.250","mainClass":"a"}"#,
        )
        .unwrap();
        std::fs::write(
            loader_b.join("forge-47.2.0.json"),
            r#"{"id":"forge-47.2.0","mainClass":"bb","inheritsFrom":"1.20.4","libraries":[]}"#,
        )
        .unwrap();

        let found =
            find_version_profile_json(&install_dir, "1.20.4").expect("a loader profile json");
        assert_eq!(
            "forge-47.2.0.json",
            found.file_name().unwrap().to_string_lossy()
        );
        assert!(find_version_profile_json(&base.join("nowhere"), "1.20.4").is_none());

        let _ = std::fs::remove_dir_all(&base);
    }
}
