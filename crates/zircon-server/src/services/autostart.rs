//! Windows OS Autostart management via the user-level Registry Run key.
//!
//! Stores `ZirconServer` in `HKCU\Software\Microsoft\Windows\CurrentVersion\Run`
//! pointing to the running `zircon-server.exe` executable.
//!
//! This enables automatic boot when the user logs in, requiring no Administrator privileges
//! and avoiding manual `shell:startup` workarounds.

use std::env;
use std::process::Command;

const RUN_KEY: &str = r"HKCU\Software\Microsoft\Windows\CurrentVersion\Run";
const VALUE_NAME: &str = "ZirconServer";

/// Returns `true` if Zircon Server is configured to run on user login.
pub fn is_autostart_enabled() -> bool {
    #[cfg(target_os = "windows")]
    {
        let output = Command::new("reg")
            .args(["query", RUN_KEY, "/v", VALUE_NAME])
            .output();

        match output {
            Ok(out) => out.status.success(),
            Err(_) => false,
        }
    }

    #[cfg(not(target_os = "windows"))]
    {
        false
    }
}

/// Registers the current executable with Windows startup.
pub fn enable_autostart() -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        let current_exe = env::current_exe()
            .map_err(|e| format!("Failed to get current executable path: {e}"))?;
        let exe_str = current_exe.to_str()
            .ok_or_else(|| "Invalid UTF-8 in executable path".to_string())?;

        let command_str = format!("\"{exe_str}\"");

        let status = Command::new("reg")
            .args(["add", RUN_KEY, "/v", VALUE_NAME, "/t", "REG_SZ", "/d", &command_str, "/f"])
            .status()
            .map_err(|e| format!("Failed to execute reg.exe: {e}"))?;

        if status.success() {
            tracing::info!("Registered Windows startup entry for {command_str}");
            Ok(())
        } else {
            Err(format!("reg.exe exited with non-zero status: {status}"))
        }
    }

    #[cfg(not(target_os = "windows"))]
    {
        Err("Windows startup registration is only supported on Windows".to_string())
    }
}

/// Removes the Windows startup entry.
pub fn disable_autostart() -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        let status = Command::new("reg")
            .args(["delete", RUN_KEY, "/v", VALUE_NAME, "/f"])
            .status()
            .map_err(|e| format!("Failed to execute reg.exe: {e}"))?;

        if status.success() {
            tracing::info!("Removed Windows startup entry for ZirconServer");
            Ok(())
        } else {
            // If already deleted or not found, treat as Ok
            Ok(())
        }
    }

    #[cfg(not(target_os = "windows"))]
    {
        Ok(())
    }
}
