//! Tauri v2 build glue: validates `tauri.conf.json` and the capabilities
//! directory at compile time and wires the platform bundle resources.

fn main() {
    tauri_build::build()
}
