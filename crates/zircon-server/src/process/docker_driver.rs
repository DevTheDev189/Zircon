use std::collections::HashMap;
use std::io;
use async_trait::async_trait;
use bollard::container::{
    AttachContainerOptions, Config, CreateContainerOptions,
    InspectContainerOptions, KillContainerOptions, LogsOptions,
    StartContainerOptions, StopContainerOptions,
};
use bollard::models::{HostConfig, PortBinding};
use bollard::Docker;
use futures_util::StreamExt;
use tokio::io::AsyncWriteExt;
use tokio::sync::mpsc;
use tracing::{error, info, warn};

use super::driver::{ProcessDriver, ProcessLaunchOptions};

#[derive(Debug, Clone)]
pub struct DockerDriver {
    client: Docker,
    network_name: Option<String>,
}

impl DockerDriver {
    /// Connects to the local Docker daemon (via Unix socket on Linux/macOS or named pipe on Windows).
    pub fn new() -> io::Result<Self> {
        let client = Docker::connect_with_local_defaults()
            .map_err(|e| io::Error::new(io::ErrorKind::Other, format!("Docker connect failed: {e}")))?;
        Ok(Self {
            client,
            network_name: None,
        })
    }

    pub fn with_network(mut self, network_name: Option<String>) -> Self {
        self.network_name = network_name;
        self
    }

    pub fn container_name(instance_id: &str) -> String {
        format!("zircon-inst-{instance_id}")
    }

    pub fn select_image(java_version: u8) -> &'static str {
        match java_version {
            8 => "zircon/runtime:java8-temurin",
            17 => "zircon/runtime:java17-temurin",
            _ => "zircon/runtime:java21-temurin",
        }
    }
}

#[async_trait]
impl ProcessDriver for DockerDriver {
    async fn start(&self, opts: ProcessLaunchOptions) -> io::Result<()> {
        let name = Self::container_name(&opts.instance_id);

        // Check if existing stopped container exists; if so, start it
        if let Ok(inspect) = self.client.inspect_container(&name, None::<InspectContainerOptions>).await {
            if let Some(state) = inspect.state {
                if state.running.unwrap_or(false) {
                    info!("Container {name} is already running");
                    return Ok(());
                }
            }
            // Exists but stopped: trigger fast wake
            info!("Waking stopped container {name}");
            self.client
                .start_container(&name, None::<StartContainerOptions<String>>)
                .await
                .map_err(|e| io::Error::new(io::ErrorKind::Other, format!("Failed to wake container: {e}")))?;
            return Ok(());
        }

        // Port bindings: bind strictly to 127.0.0.1 on the host
        let mut port_bindings = HashMap::new();
        port_bindings.insert(
            "25565/tcp".to_string(),
            Some(vec![PortBinding {
                host_ip: Some("127.0.0.1".to_string()),
                host_port: Some(opts.internal_port.to_string()),
            }]),
        );

        // Memory limit in bytes and nano_cpus
        let memory_bytes = opts.limits.memory_bytes();
        let nano_cpus = opts.limits.nano_cpus();

        // Generational ZGC uncommit flags for modern Java
        let jvm_uncommit_flags = if opts.java_version >= 21 {
            "-XX:+UseZGC -XX:+ZGenerational -XX:+UnlockExperimentalVMOptions -XX:ZUncommit=true -XX:ZUncommitDelay=60"
        } else {
            "-XX:+UnlockExperimentalVMOptions"
        };
        let java_opts = format!("{} {}", opts.memory_args, jvm_uncommit_flags);

        let host_config = HostConfig {
            binds: Some(vec![
                format!("{}:/data:rw", opts.server_dir.display()),
            ]),
            port_bindings: Some(port_bindings),
            memory: Some(memory_bytes),
            memory_swap: Some(memory_bytes), // Disable swap thrashing inside container
            nano_cpus: Some(nano_cpus),
            pids_limit: Some(opts.limits.pids_limit),
            network_mode: self.network_name.clone(),
            security_opt: Some(vec!["no-new-privileges:true".to_string()]),
            cap_drop: Some(vec!["ALL".to_string()]),
            cap_add: Some(vec![
                "CHOWN".to_string(),
                "SETUID".to_string(),
                "SETGID".to_string(),
            ]),
            ..Default::default()
        };

        let image = Self::select_image(opts.java_version);
        let config = Config {
            image: Some(image.to_string()),
            open_stdin: Some(true),
            stdin_once: Some(false),
            attach_stdin: Some(true),
            attach_stdout: Some(true),
            attach_stderr: Some(true),
            tty: Some(true),
            env: Some(vec![
                format!("JAVA_OPTS={java_opts}"),
                "EULA=true".to_string(),
            ]),
            host_config: Some(host_config),
            ..Default::default()
        };

        self.client
            .create_container(
                Some(CreateContainerOptions {
                    name: name.clone(),
                    platform: None,
                }),
                config,
            )
            .await
            .map_err(|e| io::Error::new(io::ErrorKind::Other, format!("Create container failed: {e}")))?;

        self.client
            .start_container(&name, None::<StartContainerOptions<String>>)
            .await
            .map_err(|e| io::Error::new(io::ErrorKind::Other, format!("Start container failed: {e}")))?;

        info!("Successfully provisioned and started container {name}");
        Ok(())
    }

    async fn stop(&self, instance_id: &str, timeout_secs: u32) -> io::Result<()> {
        let name = Self::container_name(instance_id);
        info!("Stopping container {name} (grace period: {timeout_secs}s)");
        self.client
            .stop_container(&name, Some(StopContainerOptions { t: timeout_secs as i64 }))
            .await
            .map_err(|e| io::Error::new(io::ErrorKind::Other, format!("Stop container failed: {e}")))
    }

    async fn kill(&self, instance_id: &str) -> io::Result<()> {
        let name = Self::container_name(instance_id);
        warn!("Force killing container {name} with SIGKILL");
        self.client
            .kill_container(&name, Some(KillContainerOptions { signal: "SIGKILL" }))
            .await
            .map_err(|e| io::Error::new(io::ErrorKind::Other, format!("Kill container failed: {e}")))
    }

    async fn is_running(&self, instance_id: &str) -> io::Result<bool> {
        let name = Self::container_name(instance_id);
        match self.client.inspect_container(&name, None::<InspectContainerOptions>).await {
            Ok(inspect) => Ok(inspect.state.and_then(|s| s.running).unwrap_or(false)),
            Err(bollard::errors::Error::DockerResponseServerError { status_code: 404, .. }) => Ok(false),
            Err(e) => Err(io::Error::new(io::ErrorKind::Other, format!("Inspect failed: {e}"))),
        }
    }

    async fn send_command(&self, instance_id: &str, command: &str) -> io::Result<()> {
        let name = Self::container_name(instance_id);
        let options = AttachContainerOptions::<String> {
            stdin: Some(true),
            stream: Some(true),
            ..Default::default()
        };

        let results = self
            .client
            .attach_container(&name, Some(options))
            .await
            .map_err(|e| io::Error::new(io::ErrorKind::Other, format!("Attach failed: {e}")))?;

        let mut input = results.input;
        let formatted = format!("{command}\n");
        input
            .write_all(formatted.as_bytes())
            .await
            .map_err(|e| io::Error::new(io::ErrorKind::Other, format!("Write to stdin failed: {e}")))?;
        input
            .flush()
            .await
            .map_err(|e| io::Error::new(io::ErrorKind::Other, format!("Flush stdin failed: {e}")))?;
        Ok(())
    }

    async fn stream_logs(&self, instance_id: &str, tx: mpsc::Sender<String>) -> io::Result<()> {
        let name = Self::container_name(instance_id);
        let options = LogsOptions::<String> {
            follow: true,
            stdout: true,
            stderr: true,
            tail: "100".to_string(),
            ..Default::default()
        };

        let mut stream = self.client.logs(&name, Some(options));
        tokio::spawn(async move {
            while let Some(msg) = stream.next().await {
                match msg {
                    Ok(log_output) => {
                        let text = log_output.to_string();
                        if tx.send(text).await.is_err() {
                            break;
                        }
                    }
                    Err(e) => {
                        error!("Error reading container logs for {name}: {e}");
                        break;
                    }
                }
            }
        });
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_container_name_formatting() {
        assert_eq!(
            DockerDriver::container_name("emerald-01"),
            "zircon-inst-emerald-01"
        );
    }

    #[test]
    fn test_select_image_by_java_version() {
        assert_eq!(
            DockerDriver::select_image(8),
            "zircon/runtime:java8-temurin"
        );
        assert_eq!(
            DockerDriver::select_image(17),
            "zircon/runtime:java17-temurin"
        );
        assert_eq!(
            DockerDriver::select_image(21),
            "zircon/runtime:java21-temurin"
        );
        assert_eq!(
            DockerDriver::select_image(22),
            "zircon/runtime:java21-temurin"
        );
    }
}
