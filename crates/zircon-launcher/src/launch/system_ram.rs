//! System RAM detection and adaptive Java heap size recommendation.

use serde::{Deserialize, Serialize};
use sysinfo::System;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SystemRamInfo {
    pub total_ram_gb: u32,
    pub recommended_ram_gb: u32,
}

/// Detects the machine's total physical RAM in GB (rounded).
pub fn get_system_ram_gb() -> u32 {
    let mut sys = System::new();
    sys.refresh_memory();
    let total_bytes = sys.total_memory();
    // total_memory is in bytes (sysinfo 0.31)
    let gb = (total_bytes as f64 / (1024.0 * 1024.0 * 1024.0)).round() as u64;
    gb.clamp(2, 256) as u32
}

/// Computes a safe, optimal default RAM allocation for Minecraft:
/// - <= 8 GB system RAM  -> 4 GB (or clamped if low)
/// - 12-24 GB system RAM -> 6 GB
/// - >= 32 GB system RAM -> 8 GB (capped at 8 GB by default to avoid GC pauses)
pub fn get_recommended_ram_gb(total_gb: u32) -> u32 {
    if total_gb <= 8 {
        4.min(total_gb.saturating_sub(2).max(2))
    } else if total_gb <= 24 {
        6
    } else {
        8
    }
}

/// Returns the system RAM information and recommended allocation.
pub fn get_system_ram_info() -> SystemRamInfo {
    let total = get_system_ram_gb();
    let recommended = get_recommended_ram_gb(total);
    SystemRamInfo {
        total_ram_gb: total,
        recommended_ram_gb: recommended,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_recommended_ram() {
        assert_eq!(get_recommended_ram_gb(4), 2);
        assert_eq!(get_recommended_ram_gb(8), 4);
        assert_eq!(get_recommended_ram_gb(16), 6);
        assert_eq!(get_recommended_ram_gb(32), 8);
        assert_eq!(get_recommended_ram_gb(64), 8);
    }
}
