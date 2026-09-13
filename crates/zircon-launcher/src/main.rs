// Prevents an extra console window on Windows in release builds.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

// Force discrete high-performance GPU on hybrid laptops (NVIDIA Optimus & AMD PowerXpress)
#[cfg(target_os = "windows")]
#[no_mangle]
pub static NvOptimusEnablement: u32 = 0x00000001;

#[cfg(target_os = "windows")]
#[no_mangle]
pub static AmdPowerXpressRequestHighPerformance: i32 = 1;

fn main() {
    zircon_launcher_lib::run()
}

