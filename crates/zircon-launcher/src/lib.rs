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
pub mod discord_rpc;
pub mod error;
pub mod launch;
pub mod logging;
pub mod modpack;
pub mod model;

pub mod offline;
pub mod pack_selection;
pub mod paths;
pub mod servers;
pub mod settings;
pub mod skin;
pub mod status;
pub mod sync;
pub mod worlds;
pub mod export;
pub mod coop;


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
            commands::list_accounts,
            commands::switch_account,
            commands::remove_account,
            commands::load_saved_servers,

            commands::save_server_list,
            commands::server_status,
            commands::probe_server,
            commands::delete_saved_server,
            commands::launch_server,
            commands::respond_shader_choice,
            commands::stop_game,
            commands::get_game_status,
            commands::list_offline_instances,
            commands::create_offline_instance,
            commands::clone_offline_instance,
            commands::delete_offline_instance,
            commands::get_offline_instance_dir,
            commands::open_instance_folder,
            commands::install_modrinth_modpack,
            commands::import_local_mrpack,
            commands::export_offline_instance_mrpack,
            commands::export_to_zircon_server,
            commands::list_instance_worlds,
            commands::backup_instance_world,
            commands::list_instance_world_backups,
            commands::restore_instance_world_backup,
            commands::delete_instance_world_backup,
            commands::list_instance_screenshots,
            commands::delete_instance_screenshot,
            commands::start_coop_session,
            commands::stop_coop_session,
            commands::get_coop_session_status,
            commands::resolve_coop_code,
            commands::coop_preflight,
            commands::coop_sync_mods,
            commands::launch_offline_instance,


            // Offline mod management
            commands::list_offline_mods,
            commands::add_offline_mod,
            commands::delete_offline_mod,
            commands::set_offline_mod_enabled, // toggle enable/disable
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
            commands::fetch_skin_by_username,
            commands::fetch_community_skins,
            commands::fetch_skin_by_url,
            commands::save_skin_bytes,
            commands::activate_history_skin,
            commands::delete_history_skin,
            commands::rename_skin,
            commands::upload_skin_to_mojang,
            commands::list_instance_packs,
            commands::list_instance_packs_detailed,
            commands::open_external_url,
            commands::open_browser_url,
            commands::add_local_pack,
            commands::import_instance_pack,
            commands::import_instance_pack_bytes,
            commands::remove_local_pack,
            commands::set_active_shaderpack,
            commands::set_active_resourcepacks,
            commands::toggle_resourcepack,
            commands::import_offline_mod_file,
            commands::import_offline_mod_bytes,
            commands::get_server_instance_mods,
            commands::add_server_mod_file,
            commands::add_server_mod_bytes,
            commands::set_server_mod_enabled,
            commands::delete_server_mod,
            commands::install_server_modrinth_mod,
            commands::search_mods,
            commands::list_mod_versions,
            commands::install_modrinth_pack,
            commands::search_modrinth,
            commands::list_modrinth_versions,
            commands::install_modrinth_mod,
            commands::check_mod_dependencies,
            commands::install_mod_with_dependencies,
            commands::check_instance_mod_updates,
            commands::update_instance_mods,
            commands::list_minecraft_versions,

            commands::get_minecraft_versions,
            commands::list_loader_types,
            commands::get_loader_versions,
            commands::get_launcher_metadata,
            commands::get_settings,
            commands::save_settings,
            commands::show_main_window,
            commands::get_launcher_version,
            commands::log_debug_message,
            commands::get_launcher_logs,
            commands::clear_launcher_logs,
            commands::get_last_instance_log,
            commands::clear_last_instance_log,
            commands::check_game_crash,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Zircon launcher");
}
