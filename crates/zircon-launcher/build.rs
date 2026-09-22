//! Tauri v2 build glue: validates `tauri.conf.json` and the capabilities
//! directory at compile time and wires the platform bundle resources.

fn main() {
    println!("cargo:rerun-if-env-changed=CURSEFORGE_API_KEY");
    tauri_build::build()
}
