//! Launches and supervises a Minecraft server subprocess.
//!
//! Supports two wiring styles:
//! * the legacy single-server layout, derived from `ConfigService`
//!   (`<data>/server`, global `mcPort`);
//! * isolated Zircon instances, derived from an `InstanceConfig` whose server
//!   lives in `<data>/instances/<id>/server` and binds its own internal port.
//!
//! The server is told to bind the internal port (`server-port`) on loopback
//! only, so the TCP multiplexer on the public port can proxy to it. stdout is
//! piped to a `ConsoleStreamHandler` and commands can be written back to the
//! process stdin.
//!
//! Port of `com.mcmanager.server.process.MinecraftProcessManager`.

use std::fmt;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::Mutex;

use tokio::io::AsyncWriteExt;
use zircon_core::model::{InstanceConfig, ModLoaderInfo, ModLoaderType};

use crate::config::{ConfigService, ServerProperties};
use crate::installer;
use crate::process::console::ConsoleStreamHandler;

/// Immutable launch description captured at construction time.
#[derive(Debug, Clone)]
struct LaunchContext {
    server_dir: PathBuf,
    server_jar: PathBuf,
    mods_dir: PathBuf,
    installer_cache_dir: PathBuf,
    minecraft_version: String,
    loader_info: ModLoaderInfo,
    java_args: String,
    mc_port: i32,
    public_port: i32,
}

/// Shared mutable process state.
struct ProcessInner {
    running: bool,
    stop_requested: bool,
    exit_code: i32,
    start_time: Option<std::time::SystemTime>,
    stdin: Option<tokio::process::ChildStdin>,
}

/// Errors raised by process management.
#[derive(Debug)]
pub enum ProcessError {
    AlreadyRunning,
    NotRunning,
    Install(String),
    Io(std::io::Error),
}

impl fmt::Display for ProcessError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ProcessError::AlreadyRunning => write!(f, "Server is already running"),
            ProcessError::NotRunning => write!(f, "Server is not running"),
            ProcessError::Install(m) => write!(f, "{m}"),
            ProcessError::Io(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for ProcessError {}

impl From<std::io::Error> for ProcessError {
    fn from(e: std::io::Error) -> Self {
        ProcessError::Io(e)
    }
}

/// Launches and supervises one Minecraft server subprocess.
pub struct MinecraftProcessManager {
    context: LaunchContext,
    console: Arc<ConsoleStreamHandler>,
    inner: Arc<Mutex<ProcessInner>>,
    /// Set when the wrapper requests a force kill (graceful stop timed out).
    kill_tx: tokio::sync::watch::Sender<bool>,
}

impl MinecraftProcessManager {
    /// Legacy single-server wiring (existing tests and controllers keep working).
    pub fn legacy(config: Arc<ConfigService>, console: Arc<ConsoleStreamHandler>) -> Self {
        let cfg = config.get_config();
        Self::new_from_context(
            LaunchContext {
                server_dir: config.server_dir.clone(),
                server_jar: config.server_jar.clone(),
                mods_dir: config.mods_dir.clone(),
                installer_cache_dir: config.data_dir.join(".cache").join("installers"),
                minecraft_version: cfg.minecraft_version,
                loader_info: cfg.mod_loader.clone(),
                java_args: cfg.java_args,
                mc_port: cfg.mc_port,
                public_port: cfg.public_port,
            },
            console,
        )
    }

    /// Multi-instance wiring: the process manager is bound to one isolated
    /// instance.
    pub fn for_instance(
        config: Arc<InstanceConfig>,
        server_dir: PathBuf,
        installer_cache_dir: PathBuf,
        console: Arc<ConsoleStreamHandler>,
    ) -> Self {
        let loader_info = config
            .mod_loader
            .clone()
            .unwrap_or_else(|| ModLoaderInfo::new("vanilla", "", None));
        let mods_dir = server_dir
            .parent()
            .map(|p| p.join("mods"))
            .unwrap_or_else(|| server_dir.join("mods"));
        Self::new_from_context(
            LaunchContext {
                server_dir: server_dir.clone(),
                server_jar: server_dir.join("server.jar"),
                mods_dir,
                installer_cache_dir,
                minecraft_version: config.minecraft_version.clone(),
                loader_info,
                java_args: config.java_args.clone(),
                mc_port: config.internal_mc_port,
                public_port: config.external_mc_port,
            },
            console,
        )
    }

    fn new_from_context(context: LaunchContext, console: Arc<ConsoleStreamHandler>) -> Self {
        let (kill_tx, _) = tokio::sync::watch::channel(false);
        Self {
            context,
            console,
            inner: Arc::new(Mutex::new(ProcessInner {
                running: false,
                stop_requested: false,
                exit_code: -1,
                start_time: None,
                stdin: None,
            })),
            kill_tx,
        }
    }

    pub fn is_running(&self) -> bool {
        self.inner.lock().unwrap().running
    }

    pub fn exit_code(&self) -> i32 {
        self.inner.lock().unwrap().exit_code
    }

    pub fn stop_requested(&self) -> bool {
        self.inner.lock().unwrap().stop_requested
    }

    pub fn start_time(&self) -> Option<std::time::SystemTime> {
        self.inner.lock().unwrap().start_time
    }

    pub fn is_abnormal_termination(&self) -> bool {
        let guard = self.inner.lock().unwrap();
        !guard.running && (!guard.stop_requested || guard.exit_code != 0)
    }

    /// Starts the server. Returns immediately; the process is supervised in the
    /// background and its console output is streamed to the console handler.
    /// The server matching the configured mod loader is installed on demand.
    pub async fn start(&self) -> Result<(), ProcessError> {
        {
            let inner = self.inner.lock().unwrap();
            if inner.running {
                return Err(ProcessError::AlreadyRunning);
            }
        }

        // Install the server matching the configured mod loader before launch.
        installer::ensure_server_installed(
            &self.context.server_dir,
            &self.context.server_jar,
            &self.context.installer_cache_dir,
            &self.context.minecraft_version,
            &self.context.loader_info,
        )
        .await
        .map_err(|e| ProcessError::Install(e.to_string()))?;

        // Reconcile managed mods into server/mods directory before booting JVM.
        // Client-only mods are excluded to protect the dedicated server JVM.
        let target_mods_dir = self.context.server_dir.join("mods");
        let bom_path = self
            .context
            .mods_dir
            .parent()
            .map(|p| p.join("bom.json"))
            .unwrap_or_else(|| self.context.mods_dir.join("bom.json"));
        let client_only_mods = get_client_only_mods(&bom_path);
        let synced_count = sync_mods(&self.context.mods_dir, &target_mods_dir, &client_only_mods)?;
        tracing::info!(
            "Synced {} mod(s) (excluded {} client-only) from {:?} to {:?}",
            synced_count,
            client_only_mods.len(),
            self.context.mods_dir,
            target_mods_dir
        );

        // Pin the server to its internal port on loopback only.
        let props_file = self.context.server_dir.join("server.properties");
        let mut props = if props_file.is_file() {
            ServerProperties::load(&props_file)?
        } else {
            ServerProperties::default()
        };
        props.set("server-port", &self.context.mc_port.to_string());
        props.set("server-ip", "127.0.0.1");
        props.save(&props_file)?;

        let mut command = tokio::process::Command::new(installer::java_bin());
        // Untrusted mod code runs inside this JVM: scrub the environment so
        // host secrets (AWS_ACCESS_KEY_ID, GITHUB_TOKEN, ...) can never leak
        // into the server process. Keep only what the JVM needs to function.
        command.env_clear();
        command.envs(std::env::vars().filter(|(k, _)| {
            let upper = k.to_ascii_uppercase();
            matches!(
                upper.as_str(),
                "PATH" | "SYSTEMROOT" | "USERPROFILE" | "HOME" | "TMP" | "TEMP"
            )
        }));
        let mut launch_args: Vec<String> = self
            .context
            .java_args
            .split_whitespace()
            .map(|s| s.to_string())
            .collect();
        let loader = ModLoaderType::from_id(&self.context.loader_info.r#type) /* z0 */
            .unwrap_or(ModLoaderType::Vanilla); // z0
        if loader.is_forge_like()  { // z0
            let args_file = installer::find_server_args_file( /* z0 */
                &self.context.server_dir, // z0
                &self.context.loader_info.version, // z0
            ) /* z0 */
            .ok_or_else(||  { // z0
                ProcessError::Install( /* z0 */
                    "Forge/NeoForge server args file not found after installation".to_string(), // z0
                )
            })?; // z0
            let rel = args_file /* z0 */
                .strip_prefix(&self.context.server_dir) /* z0 */
                .unwrap_or(&args_file) /* z0 */
                .to_string_lossy() /* z0 */
                .into_owned(); // z0
            launch_args.push(format!("@{rel}")); // z0
        } else if loader == ModLoaderType::Quilt /* z0 */
            && self /* z0 */
                .context /* z0 */
                .server_dir /* z0 */
                .join("quilt-server-launch.jar") /* z0 */
                .is_file() /* z0 */
         { // z0
            launch_args.push("-jar".to_string()); // z0
            launch_args.push( /* z0 */
                self.context /* z0 */
                    .server_dir
                    .join("quilt-server-launch.jar")
                    .to_string_lossy() /* z0 */
                    .into_owned(), // z0
            ); // z0
        } else {
            if !self.context.server_jar.is_file() {
                return Err(ProcessError::Io(std::io::Error::new(
                    std::io::ErrorKind::NotFound,
                    format!(
                        "No server.jar found at {}. Drop the vanilla/fabric server JAR into {}",
                        self.context.server_jar.display(),
                        self.context.server_dir.display()
                    ),
                )));
            }
            launch_args.push("-jar".to_string());
            launch_args.push(self.context.server_jar.to_string_lossy().into_owned());
        }
        launch_args.push("nogui".to_string());
        launch_args.push("--port".to_string());
        launch_args.push(self.context.mc_port.to_string());
        command
            .args(&launch_args)
            .current_dir(&self.context.server_dir)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped());

        tracing::info!(
            "Launching: {}",
            std::iter::once(
                command
                    .as_std()
                    .get_program()
                    .to_string_lossy()
                    .into_owned()
            )
            .chain(launch_args)
            .collect::<Vec<_>>()
            .join(" ")
        );

        let mut child = command.spawn()?;
        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| std::io::Error::other("failed to take child stdin"))?;
        let stdout = child.stdout.take();
        let stderr = child.stderr.take();
        let exit_code = child.id().map(|_| -1).unwrap_or(-1);

        {
            let mut inner = self.inner.lock().unwrap();
            inner.running = true;
            inner.stop_requested = false;
            inner.exit_code = exit_code;
            inner.start_time = Some(std::time::SystemTime::now());
            inner.stdin = Some(stdin);
        }

        let public_port_text = if self.context.public_port > 0 {
            format!(" (public port {})", self.context.public_port)
        } else {
            String::new()
        };
        self.console.accept(format!(
            "[wrapper] Starting Minecraft server on internal port {}{}",
            self.context.mc_port, public_port_text
        ));

        // Pump stdout/stderr lines into the console.
        let console = self.console.clone();
        if let Some(mut out) = stdout {
            tokio::spawn(async move {
                use tokio::io::AsyncBufReadExt;
                let mut lines = tokio::io::BufReader::new(&mut out).lines();
                while let Ok(Some(line)) = lines.next_line().await {
                    console.accept(line);
                }
            });
        }
        let console_err = self.console.clone();
        if let Some(mut err) = stderr {
            tokio::spawn(async move {
                use tokio::io::AsyncBufReadExt;
                let mut lines = tokio::io::BufReader::new(&mut err).lines();
                while let Ok(Some(line)) = lines.next_line().await {
                    console_err.accept(line);
                }
            });
        }

        // Telemetry probe task: periodically queries tick performance & status ping.
        let console_telemetry = self.console.clone();
        let inner_telemetry = self.inner.clone();
        let mc_port = self.context.mc_port;
        let mc_version = self.context.minecraft_version.clone();
        let loader_type = self.context.loader_info.r#type.to_lowercase();
        tokio::spawn(async move {
            // Initial delay while server starts up and initializes world
            tokio::time::sleep(std::time::Duration::from_secs(8)).await;
            while inner_telemetry.lock().map(|g| g.running).unwrap_or(false) {
                // Only perform probes if a dashboard client is actively monitoring stats
                if console_telemetry.tps_tracker().is_active_monitoring_requested() {
                    // Measure TCP handshake latency to internal loopback port
                    let start = std::time::Instant::now();
                    if let Ok(stream) = tokio::time::timeout(
                        std::time::Duration::from_millis(1500),
                        tokio::net::TcpStream::connect(("127.0.0.1", mc_port as u16)),
                    )
                    .await
                    {
                        if let Ok(mut s) = stream {
                            let elapsed_ms = start.elapsed().as_millis() as u64;
                            console_telemetry.tps_tracker().record_ping(elapsed_ms);
                            let _ = s.shutdown().await;
                        }
                    }

                    // Issue non-intrusive tick query command to stdin:
                    // NeoForge, Fabric, Quilt, Vanilla, and all modern 1.20.3+ servers use `tick query`.
                    // Only legacy Forge (1.12–1.20.2) uses `forge tps`.
                    let is_legacy_forge = loader_type == "forge"
                        && !mc_version.starts_with("1.20.3")
                        && !mc_version.starts_with("1.20.4")
                        && !mc_version.starts_with("1.20.5")
                        && !mc_version.starts_with("1.20.6")
                        && !mc_version.starts_with("1.21")
                        && !mc_version.starts_with("1.22")
                        && !mc_version.starts_with("26.");

                    let cmd = if is_legacy_forge {
                        "forge tps"
                    } else {
                        "tick query"
                    };

                    let stdin_opt = {
                        let mut guard = inner_telemetry.lock().unwrap();
                        if guard.running {
                            guard.stdin.take()
                        } else {
                            None
                        }
                    };

                    if let Some(mut stdin) = stdin_opt {
                        let write_res = stdin.write_all(format!("{cmd}\n").as_bytes()).await;
                        if write_res.is_ok() {
                            let _ = stdin.flush().await;
                        }
                        if let Ok(mut guard) = inner_telemetry.lock() {
                            guard.stdin = Some(stdin);
                        }
                    }
                }

                tokio::time::sleep(std::time::Duration::from_secs(5)).await;
            }
        });

        // Monitor task: waits for exit; can be interrupted by stop() to force-kill.
        let inner = self.inner.clone();
        let console = self.console.clone();
        let mut kill_rx = self.kill_tx.subscribe();
        tokio::spawn(async move {
            let outcome = tokio::select! {
                res = child.wait() => res,
                _ = kill_rx.changed() => {
                    // Force-kill requested by stop(). The wait future is dropped
                    // when this branch wins, releasing the borrow on child.
                    let _ = child.kill().await;
                    child.wait().await
                }
            };
            let code = outcome.map(|s| s.code().unwrap_or(-1)).unwrap_or(-1);
            let mut guard = inner.lock().unwrap();
            guard.exit_code = code;
            guard.running = false;
            let stop_requested = guard.stop_requested;
            drop(guard);
            console.player_tracker().reset();
            console.tps_tracker().reset();
            if stop_requested {
                console.accept(format!(
                    "[wrapper] Minecraft server stopped (exit code {code})"
                ));
            } else {
                console.accept(format!(
                    "[wrapper] Minecraft server exited unexpectedly with code {code}"
                ));
                tracing::warn!("Minecraft server exited with code {code}");
            }
        });

        Ok(())
    }

    /// Writes a command to the server's stdin (e.g. "say hello").
    pub async fn send_command(&self, command: &str) -> Result<(), ProcessError> {
        // Take the stdin handle out of the shared state so the write can be
        // awaited without holding the std Mutex — `MutexGuard` is `!Send`, so
        // holding it across an await would both deadlock-risk concurrent
        // commands and break axum's requirement that handler futures be `Send`.
        // (Do not drop a `write_all` future un-awaited: the command would be
        // silently discarded.)
        let mut stdin = {
            let mut inner = self.inner.lock().unwrap();
            if !inner.running {
                return Err(ProcessError::NotRunning);
            }
            inner.stdin.take().ok_or(ProcessError::NotRunning)?
        };
        let write_result = async {
            stdin.write_all(format!("{command}\n").as_bytes()).await?;
            stdin.flush().await
        }
        .await;
        {
            let mut inner = self.inner.lock().unwrap();
            inner.stdin = Some(stdin);
        }
        write_result?;
        tracing::debug!("Sent command: {command}");
        Ok(())
    }

    /// Sends `stop`, waits for a graceful exit, then force-kills if needed.
    pub async fn stop(&self) {
        {
            let mut inner = self.inner.lock().unwrap();
            if !inner.running {
                return;
            }
            inner.stop_requested = true;
        }
        self.console
            .accept("[wrapper] Stopping Minecraft server...".to_string());
        let _ = self.send_command("stop").await;

        // Wait up to 15s for a graceful exit, then force-kill.
        let mut waited = 0u32;
        while self.is_running() && waited < 150 {
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            waited += 1;
        }
        if self.is_running() {
            tracing::warn!("Server did not stop gracefully, force killing");
            let _ = self.kill_tx.send(true);
            // Wait for the monitor to reap the child.
            let mut waited = 0u32;
            while self.is_running() && waited < 50 {
                tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                waited += 1;
            }
        }
        self.inner.lock().unwrap().running = false;
    }
}

/// Reads the BOM file if present to discover which mods are marked as client-only.
fn get_client_only_mods(bom_path: &std::path::Path) -> std::collections::HashSet<String> {
    let mut client_only = std::collections::HashSet::new();
    if bom_path.is_file() {
        if let Ok(content) = std::fs::read_to_string(bom_path) {
            if let Ok(bom) = serde_json::from_str::<zircon_core::model::BillOfMaterials>(&content) {
                for m in bom.mods {
                    if m.side == zircon_core::model::ModSide::Client {
                        client_only.insert(m.filename);
                    }
                }
            }
        }
    }
    client_only
}

/// Reconciles target mods directory (`<server_dir>/mods`) with source mods directory (`<mods_dir>`).
/// Purges stale files in target and copies missing/modified files from source.
/// Client-only mods are skipped and purged from target to protect the dedicated server JVM.
fn sync_mods(
    source_mods_dir: &std::path::Path,
    target_mods_dir: &std::path::Path,
    client_only_mods: &std::collections::HashSet<String>,
) -> Result<usize, ProcessError> {
    std::fs::create_dir_all(target_mods_dir)?;

    let mut source_files = std::collections::HashSet::new();

    if source_mods_dir.is_dir() {
        let entries = std::fs::read_dir(source_mods_dir)?;
        for entry in entries {
            let entry = entry?;
            let path = entry.path();
            if path.is_file() {
                if let Some(filename_os) = path.file_name() {
                    let filename = filename_os.to_string_lossy().to_string();
                    if client_only_mods.contains(&filename) {
                        tracing::debug!("Skipping client-only mod {:?} for server runtime", filename);
                        continue;
                    }

                    source_files.insert(filename_os.to_os_string());
                    let target_path = target_mods_dir.join(filename_os);

                    let needs_copy = if !target_path.is_file() {
                        true
                    } else {
                        let src_len = entry.metadata()?.len();
                        let tgt_len = std::fs::metadata(&target_path)?.len();
                        src_len != tgt_len
                    };

                    if needs_copy {
                        std::fs::copy(&path, &target_path)?;
                        tracing::info!("Synced mod file {:?} to server mods directory", filename);
                    }
                }
            }
        }
    }

    if target_mods_dir.is_dir() {
        let entries = std::fs::read_dir(target_mods_dir)?;
        for entry in entries {
            let entry = entry?;
            let path = entry.path();
            if path.is_file() {
                if let Some(filename) = path.file_name() {
                    if !source_files.contains(filename) {
                        tracing::info!("Purging stale/client-only server mod file {:?}", filename);
                        std::fs::remove_file(&path)?;
                    }
                }
            }
        }
    }

    Ok(source_files.len())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sync_mods_copies_and_purges_correctly() {
        let dir = crate::test_util::temp_dir("sync_mods");
        let src = dir.join("instance_mods");
        let tgt = dir.join("server_mods");
        std::fs::create_dir_all(&src).unwrap();
        std::fs::create_dir_all(&tgt).unwrap();

        // 1. Create source mods
        std::fs::write(src.join("mod_a.jar"), b"mod_a_data_v1").unwrap();
        std::fs::write(src.join("mod_b.jar"), b"mod_b_data").unwrap();

        // Create stale mod in target
        std::fs::write(tgt.join("stale_mod.jar"), b"old").unwrap();

        // Sync with empty client_only
        let client_only = std::collections::HashSet::new();
        let count = sync_mods(&src, &tgt, &client_only).unwrap();
        assert_eq!(count, 2);
        assert!(tgt.join("mod_a.jar").is_file());
        assert!(tgt.join("mod_b.jar").is_file());
        assert!(!tgt.join("stale_mod.jar").exists());

        // 2. Update a mod file size
        std::fs::write(src.join("mod_a.jar"), b"mod_a_data_v2_longer").unwrap();
        sync_mods(&src, &tgt, &client_only).unwrap();
        assert_eq!(
            std::fs::read(tgt.join("mod_a.jar")).unwrap(),
            b"mod_a_data_v2_longer"
        );

        // 3. Exclude a mod via client_only filter
        let mut client_only_filter = std::collections::HashSet::new();
        client_only_filter.insert("mod_a.jar".to_string());
        let count_client = sync_mods(&src, &tgt, &client_only_filter).unwrap();
        assert_eq!(count_client, 1); // Only mod_b counted
        assert!(!tgt.join("mod_a.jar").exists()); // Purged from server runtime
        assert!(tgt.join("mod_b.jar").is_file());

        // 4. Remove a mod from source
        std::fs::remove_file(src.join("mod_b.jar")).unwrap();
        let count2 = sync_mods(&src, &tgt, &client_only).unwrap();
        assert_eq!(count2, 1); // Only mod_a remains in src
        assert!(!tgt.join("mod_b.jar").exists());

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn send_command_when_not_running_fails() {
        let console = Arc::new(ConsoleStreamHandler::new());
        let config = Arc::new(InstanceConfig::new("Test", "1.20.4", "vanilla", "", 25700));
        let pm = MinecraftProcessManager::for_instance(
            config,
            PathBuf::from("."),
            PathBuf::from("."),
            console,
        );
        assert!(!pm.is_running());
        assert!(matches!(
            pm.send_command("say hi").await,
            Err(ProcessError::NotRunning)
        ));
    }
}
