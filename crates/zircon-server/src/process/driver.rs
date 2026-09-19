use std::fmt::Debug;
use std::io;
use std::path::PathBuf;

use async_trait::async_trait;
use tokio::sync::mpsc;

/// Hardware and cgroup resource quotas enforced on a running server instance.
#[derive(Debug, Clone, PartialEq)]
pub struct ProcessLimits {
    /// Maximum RAM in megabytes (e.g. 7372 for 7.2GB Starter, 11776 for 11.5GB Pro).
    pub memory_limit_mb: u64,
    /// Fractional vCPU allocation (e.g. 2.0 or 4.0).
    pub cpu_cores: f32,
    /// Maximum process/task count inside the container to prevent fork-bombs.
    pub pids_limit: i64,
}

impl Default for ProcessLimits {
    fn default() -> Self {
        Self {
            memory_limit_mb: 7372, // ~7.2 GB
            cpu_cores: 2.5,
            pids_limit: 512,
        }
    }
}

impl ProcessLimits {
    /// Calculates memory limit in bytes for Docker cgroup v2.
    pub fn memory_bytes(&self) -> i64 {
        (self.memory_limit_mb * 1024 * 1024) as i64
    }

    /// Calculates CPU limit in nanoCPUs for Docker cgroup v2 (1 core = 1e9).
    pub fn nano_cpus(&self) -> i64 {
        (self.cpu_cores * 1e9) as i64
    }

    /// Helper to derive process limits from JVM heap arguments (e.g. -Xmx6G)
    /// allocating 20% headroom (min 1.2 GB) for off-heap, metaspace, and native memory.
    pub fn from_java_args(args: &str) -> Self {
        let mut heap_mb: u64 = 4096; // fallback 4GB
        for part in args.split_whitespace() {
            if let Some(rest) = part.strip_prefix("-Xmx") {
                let upper = rest.to_ascii_uppercase();
                if let Some(num) = upper.strip_suffix('G') {
                    if let Ok(g) = num.parse::<u64>() {
                        heap_mb = g * 1024;
                    }
                } else if let Some(num) = upper.strip_suffix('M') {
                    if let Ok(m) = num.parse::<u64>() {
                        heap_mb = m;
                    }
                }
            }
        }
        let memory_limit_mb = heap_mb + (heap_mb / 5).max(1200);
        let cpu_cores = if heap_mb >= 8192 { 4.0 } else { 2.5 };
        Self {
            memory_limit_mb,
            cpu_cores,
            pids_limit: 512,
        }
    }
}

/// Options required to launch an instance via a ProcessDriver.
#[derive(Debug, Clone)]
pub struct ProcessLaunchOptions {
    pub instance_id: String,
    pub server_dir: PathBuf,
    pub internal_port: u16,
    pub memory_args: String,
    pub java_version: u8, // 8, 17, 21
    pub limits: ProcessLimits,
}

/// Abstract driver controlling the lifecycle of a Minecraft server process,
/// enabling transparent switching between Native OS processes and Docker containers.
#[async_trait]
pub trait ProcessDriver: Send + Sync + Debug {
    /// Launches the server process or container.
    async fn start(&self, opts: ProcessLaunchOptions) -> io::Result<()>;

    /// Initiates a graceful shutdown (sending 'stop' or SIGTERM).
    async fn stop(&self, instance_id: &str, timeout_secs: u32) -> io::Result<()>;

    /// Immediately kills the instance if unresponsive.
    async fn kill(&self, instance_id: &str) -> io::Result<()>;

    /// Checks if the instance process or container is currently running.
    async fn is_running(&self, instance_id: &str) -> io::Result<bool>;

    /// Sends a text command into stdin of the running server.
    async fn send_command(&self, instance_id: &str, command: &str) -> io::Result<()>;

    /// Subscribes to the raw log stream (stdout/stderr).
    async fn stream_logs(
        &self,
        instance_id: &str,
        tx: mpsc::Sender<String>,
    ) -> io::Result<()>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_process_limits_arithmetic() {
        let limits = ProcessLimits {
            memory_limit_mb: 7372, // ~7.2 GB
            cpu_cores: 2.5,
            pids_limit: 512,
        };
        assert_eq!(limits.memory_bytes(), 7372 * 1024 * 1024);
        assert_eq!(limits.nano_cpus(), 2500000000);
    }

    #[test]
    fn test_from_java_args_parsing() {
        let limits = ProcessLimits::from_java_args("-Xms2G -Xmx6G");
        assert_eq!(limits.memory_limit_mb, 6144 + 1228); // 6GB + 20%
        assert_eq!(limits.cpu_cores, 2.5);

        let pro_limits = ProcessLimits::from_java_args("-Xms4G -Xmx10G");
        assert_eq!(pro_limits.memory_limit_mb, 10240 + 2048);
        assert_eq!(pro_limits.cpu_cores, 4.0);
    }
}
