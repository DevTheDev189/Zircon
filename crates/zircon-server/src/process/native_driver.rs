use std::collections::HashMap;
use std::io;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use async_trait::async_trait;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin, Command};
use tokio::sync::{mpsc, watch};
use tracing::{info, warn};

use crate::installer;
use super::driver::{ProcessDriver, ProcessLaunchOptions};

struct RunningChild {
    child: Child,
    stdin: Option<ChildStdin>,
    kill_tx: watch::Sender<bool>,
    log_broadcaster: tokio::sync::broadcast::Sender<String>,
}

/// Native OS process driver using `tokio::process::Command` for local development
/// and self-hosting without Docker requirements.
#[derive(Clone)]
pub struct NativeDriver {
    children: Arc<Mutex<HashMap<String, RunningChild>>>,
}

impl std::fmt::Debug for NativeDriver {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let count = self.children.lock().map(|c| c.len()).unwrap_or(0);
        f.debug_struct("NativeDriver")
            .field("active_children", &count)
            .finish()
    }
}

impl Default for NativeDriver {
    fn default() -> Self {
        Self::new()
    }
}

impl NativeDriver {
    pub fn new() -> Self {
        Self {
            children: Arc::new(Mutex::new(HashMap::new())),
        }
    }
}

#[async_trait]
impl ProcessDriver for NativeDriver {
    async fn start(&self, opts: ProcessLaunchOptions) -> io::Result<()> {
        {
            let mut children = self.children.lock().unwrap();
            if let Some(child) = children.get_mut(&opts.instance_id) {
                if let Ok(None) = child.child.try_wait() {
                    return Err(io::Error::new(
                        io::ErrorKind::AlreadyExists,
                        format!("Instance '{}' is already running", opts.instance_id),
                    ));
                }
            }
        }

        let java_bin = installer::java_bin_for_version(opts.java_version);
        let mut cmd = Command::new(java_bin);
        cmd.current_dir(&opts.server_dir);

        // Sanitize environment variables
        cmd.env_clear();
        cmd.envs(std::env::vars().filter(|(k, _)| {
            let upper = k.to_ascii_uppercase();
            matches!(
                upper.as_str(),
                "PATH" | "SYSTEMROOT" | "USERPROFILE" | "HOME" | "TMP" | "TEMP"
            )
        }));

        let mut args: Vec<String> = opts
            .memory_args
            .split_whitespace()
            .map(|s| s.to_string())
            .collect();

        // Determine JAR target
        let quilt_jar = opts.server_dir.join("quilt-server-launch.jar");
        let fabric_jar = opts.server_dir.join("fabric-server-launch.jar");
        let server_jar = opts.server_dir.join("server.jar");

        if quilt_jar.is_file() {
            args.push("-jar".to_string());
            args.push(quilt_jar.to_string_lossy().into_owned());
        } else if fabric_jar.is_file() {
            args.push("-jar".to_string());
            args.push(fabric_jar.to_string_lossy().into_owned());
        } else if server_jar.is_file() {
            args.push("-jar".to_string());
            args.push(server_jar.to_string_lossy().into_owned());
        }

        args.push("nogui".to_string());
        args.push("--port".to_string());
        args.push(opts.internal_port.to_string());

        cmd.args(&args);
        cmd.stdin(std::process::Stdio::piped());
        cmd.stdout(std::process::Stdio::piped());
        cmd.stderr(std::process::Stdio::piped());

        info!(
            "NativeDriver spawning instance '{}': {:?}",
            opts.instance_id, cmd
        );

        let mut child = cmd.spawn()?;
        let stdin = child.stdin.take();
        let stdout = child.stdout.take();
        let stderr = child.stderr.take();

        let (kill_tx, _) = watch::channel(false);
        let (log_tx, _) = tokio::sync::broadcast::channel(256);

        // Forward stdout into broadcast channel
        if let Some(out) = stdout {
            let log_tx_out = log_tx.clone();
            tokio::spawn(async move {
                let mut reader = BufReader::new(out).lines();
                while let Ok(Some(line)) = reader.next_line().await {
                    let _ = log_tx_out.send(line);
                }
            });
        }

        // Forward stderr into broadcast channel
        if let Some(err) = stderr {
            let log_tx_err = log_tx.clone();
            tokio::spawn(async move {
                let mut reader = BufReader::new(err).lines();
                while let Ok(Some(line)) = reader.next_line().await {
                    let _ = log_tx_err.send(line);
                }
            });
        }

        let instance_id = opts.instance_id.clone();
        let children_map = self.children.clone();
        children_map.lock().unwrap().insert(
            instance_id,
            RunningChild {
                child,
                stdin,
                kill_tx,
                log_broadcaster: log_tx,
            },
        );

        Ok(())
    }

    async fn stop(&self, instance_id: &str, timeout_secs: u32) -> io::Result<()> {
        let _ = self.send_command(instance_id, "stop").await;

        let poll_interval = Duration::from_millis(100);
        let max_polls = timeout_secs * 10;
        for _ in 0..max_polls {
            if !self.is_running(instance_id).await? {
                return Ok(());
            }
            tokio::time::sleep(poll_interval).await;
        }

        warn!("Instance '{instance_id}' failed to stop gracefully in {timeout_secs}s; killing");
        self.kill(instance_id).await
    }

    async fn kill(&self, instance_id: &str) -> io::Result<()> {
        let mut children = self.children.lock().unwrap();
        if let Some(child_info) = children.get_mut(instance_id) {
            let _ = child_info.kill_tx.send(true);
            child_info.child.start_kill()
        } else {
            Ok(())
        }
    }

    async fn is_running(&self, instance_id: &str) -> io::Result<bool> {
        let mut children = self.children.lock().unwrap();
        if let Some(child_info) = children.get_mut(instance_id) {
            match child_info.child.try_wait() {
                Ok(None) => Ok(true),
                Ok(Some(_)) => {
                    children.remove(instance_id);
                    Ok(false)
                }
                Err(e) => Err(e),
            }
        } else {
            Ok(false)
        }
    }

    async fn send_command(&self, instance_id: &str, command: &str) -> io::Result<()> {
        let stdin_opt = {
            let mut children = self.children.lock().unwrap();
            children.get_mut(instance_id).and_then(|c| c.stdin.take())
        };

        if let Some(mut stdin) = stdin_opt {
            let formatted = format!("{command}\n");
            let res = stdin.write_all(formatted.as_bytes()).await;
            let flush_res = stdin.flush().await;

            // Put stdin back
            if let Some(c) = self.children.lock().unwrap().get_mut(instance_id) {
                c.stdin = Some(stdin);
            }

            res?;
            flush_res?;
            Ok(())
        } else {
            Err(io::Error::new(
                io::ErrorKind::NotConnected,
                format!("Instance '{instance_id}' stdin not available or not running"),
            ))
        }
    }

    async fn stream_logs(&self, instance_id: &str, tx: mpsc::Sender<String>) -> io::Result<()> {
        let rx_opt = {
            let children = self.children.lock().unwrap();
            children.get(instance_id).map(|c| c.log_broadcaster.subscribe())
        };

        if let Some(mut rx) = rx_opt {
            tokio::spawn(async move {
                while let Ok(line) = rx.recv().await {
                    if tx.send(line).await.is_err() {
                        break;
                    }
                }
            });
            Ok(())
        } else {
            Err(io::Error::new(
                io::ErrorKind::NotFound,
                format!("Instance '{instance_id}' logs not found"),
            ))
        }
    }
}
