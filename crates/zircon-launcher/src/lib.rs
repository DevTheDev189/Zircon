//! Zircon companion launcher.
//!
//! Pure-Rust port of the Java `client-launcher` Gradle module. Phase 4 delivers
//! the client-side launch pipeline: Microsoft OAuth PKCE authentication, the
//! vanilla/loader classpath and asset resolver, Java runtime provisioning, the
//! game process runner and the mod/pack sync engines. Phase 5 adds the Tauri v2
//! shell (`commands.rs`) and the Vue 3 frontend under `ui/` on top of this
//! library.

pub mod auth;
pub mod commands;
pub mod error;
pub mod launch;
pub mod logging;
pub mod model;
pub mod offline;
pub mod pack_selection;
pub mod paths;
pub mod servers;
pub mod settings;
pub mod skin;
pub mod status;
pub mod sync;

/// Boots the Tauri application: registers the plugins, manages the shared
/// launcher state and exposes every IPC command from [`commands`].
///
/// Logging goes to stderr; set `RUST_LOG` (e.g. `RUST_LOG=debug`) for more
/// verbose output when running from a terminal. Every event is also mirrored
/// into an in-memory ring buffer for the Settings tab's debug log viewer.
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    use tracing_subscriber::prelude::*;
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .with(tracing_subscriber::fmt::layer())
        .with(logging::InMemoryLogLayer)
        .init();
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .manage(commands::LauncherState::new())
        .invoke_handler(tauri::generate_handler![
            commands::login_microsoft,
            commands::get_cached_session,
            commands::get_session,
            commands::logout,
            commands::load_saved_servers,
            commands::save_server_list,
            commands::server_status,
            commands::delete_saved_server,
            commands::launch_server,
            commands::respond_shader_choice,
            commands::stop_game,
            commands::get_game_status,
            commands::list_offline_instances,
            commands::create_offline_instance,
            commands::delete_offline_instance,
            commands::get_offline_instance_dir,
            commands::launch_offline_instance,
            commands::list_offline_mods,
            commands::delete_offline_mod,
            commands::add_offline_mod,
            commands::get_active_skin,
            commands::get_skin_head_icon,
            commands::save_skin,
            commands::set_active_skin_variant,
            commands::remove_skin,
            commands::get_skin_history,
            commands::get_bundled_skins,
            commands::save_bundled_skin,
            commands::fetch_mojang_skin,
            commands::fetch_mojang_skin_active,
            commands::fetch_mojang_skin_preview,
            commands::activate_history_skin,
            commands::delete_history_skin,
            commands::upload_skin_to_mojang,
            commands::list_instance_packs,
            commands::list_instance_packs_detailed,
            commands::open_external_url,
            commands::add_local_pack,
            commands::remove_local_pack,
            commands::set_active_shaderpack,
            commands::toggle_resourcepack,
            commands::search_modrinth,
            commands::install_modrinth_mod,
            commands::list_minecraft_versions,
            commands::list_loader_types,
            commands::get_settings,
            commands::save_settings,
            commands::get_launcher_logs,
            commands::clear_launcher_logs,
            commands::check_game_crash,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Zircon launcher");
}
