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
        Self::new_from_context(
            LaunchContext {
                server_dir: server_dir.clone(),
                server_jar: server_dir.join("server.jar"),
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
        let loader = ModLoaderType::from_id(&self.context.loader_info.r#type);
        if let Some(loader) = loader {
            if loader.is_forge_like() {
                // Forge/NeoForge servers launch through the installer-generated
                // @args file (module path + JVM args + main class). Paths inside
                // the file are relative to the server dir, which is the CWD.
                let args_file = installer::find_server_args_file(
                    &self.context.server_dir,
                    &self.context.loader_info.version,
                )
                .ok_or_else(|| {
                    ProcessError::Install(
                        "Forge/NeoForge server args file not found after installation".to_string(),
                    )
                })?;
                let rel = args_file
                    .strip_prefix(&self.context.server_dir)
                    .unwrap_or(&args_file)
                    .to_string_lossy()
                    .into_owned();
                launch_args.push(format!("@{rel}"));
            } else if loader == ModLoaderType::Quilt
                && self
                    .context
                    .server_dir
                    .join("quilt-server-launch.jar")
                    .is_file()
            {
                // Quilt servers install to `quilt-server-launch.jar` (unlike
                // Fabric's combined `server.jar`).
                launch_args.push("-jar".to_string());
                launch_args.push(
                    self.context
                        .server_dir
                        .join("quilt-server-launch.jar")
                        .to_string_lossy()
                        .into_owned(),
                );
            } else {
                launch_args.push("-jar".to_string());
                launch_args.push(self.context.server_jar.to_string_lossy().into_owned());
            }
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

#[cfg(test)]
mod tests {
    use super::*;

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
