//! Entry point of the server manager: wires up configuration, admin auth, the
//! multi-instance engine, the mod/BOM services, the Minecraft subprocess
//! manager, the Axum admin API and the TCP protocol multiplexer on the public
//! port.
//!
//! Port of `com.mcmanager.server.Main`.

use std::sync::Arc;
use std::time::Duration;

use tokio::net::TcpListener;
use zircon_server::auth::auth_service::AuthService;
use zircon_server::auth::jwt;
use zircon_server::auth::sessions::SessionRegistry;
use zircon_server::config::ConfigService;
use zircon_server::instance::ServerInstanceManager;
use zircon_server::multiplexer::tcp::TcpMultiplexer;
use zircon_server::process::console::ConsoleStreamHandler;
use zircon_server::process::manager::MinecraftProcessManager;
use zircon_server::services::backup::BackupService;
use zircon_server::services::bom::BomService;
use zircon_server::services::mods::ModManagementService;
use zircon_server::services::packs::PackManagementService;
use zircon_server::services::resolver::ModServiceResolver;
use zircon_server::services::scheduler::BackupSchedulerService;
use zircon_server::tickets::JoinTicketManager;
use zircon_server::web::app::AppState;
use zircon_server::web::rate_limit::FixedWindowLimiter;
use zircon_server::web::router;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info,zircon_server=info")),
        )
        .init();

    let config = Arc::new(ConfigService::load()?);

    // Admin auth: creates users.json + a random initial admin password on first
    // run (printed to stdout) and the JWT signing secret.
    let auth = Arc::new(AuthService::initialize(&config.data_dir)?);
    jwt::initialize(&config.data_dir)?;

    let console = Arc::new(ConsoleStreamHandler::new());
    let process_manager = Arc::new(MinecraftProcessManager::legacy(
        config.clone(),
        console.clone(),
    ));

    // Multi-instance engine (isolated <data>/instances/<id>/ dirs).
    let instances = Arc::new(ServerInstanceManager::new(
        &config.data_dir,
        console.clone(),
    )?);

    let bom = Arc::new(BomService::new(config.bom_file.clone(), None));
    let mods = Arc::new(ModManagementService::new(
        bom.clone(),
        config.mods_dir.clone(),
        &config.get_config().curseforge_api_key,
    ));
    let packs = PackManagementService::new(
        bom.clone(),
        config.data_dir.join("shaderpacks"),
        config.data_dir.join("resourcepacks"),
    );
    let resolver = Arc::new(ModServiceResolver::new(
        instances.clone(),
        bom.clone(),
        mods.clone(),
        packs.clone(),
        &config.get_config().curseforge_api_key,
    ));

    // LZ4-compressed backups + the automatic scheduler.
    let backup = Arc::new(BackupService::new(&config.data_dir, instances.clone()));
    let scheduler = BackupSchedulerService::new(instances.clone(), backup.clone());
    let scheduler_handle = scheduler.start();

    let tickets = Arc::new(JoinTicketManager::new());

    // Auth hardening: server-side session revocation + a global cap on failed
    // login attempts (15 min window, 10 attempts).
    let sessions = Arc::new(SessionRegistry::new());
    let login_limiter = Arc::new(FixedWindowLimiter::new(Duration::from_secs(15 * 60), 10));

    let state = AppState {
        config: config.clone(),
        auth,
        instances: instances.clone(),
        console: console.clone(),
        process_manager,
        backup,
        bom,
        mods,
        packs,
        resolver,
        tickets: tickets.clone(),
        curseforge_api_key: config.get_config().curseforge_api_key,
        sessions: sessions.clone(),
        login_limiter: login_limiter.clone(),
    };

    // Axum admin API (binds 127.0.0.1:<webPort>; reachable through the
    // multiplexer's public port too). ConnectInfo is needed so auth handlers
    // can key rate limits on the client address.
    let app = router(state.clone());
    let web_port = config.get_config().web_port;
    let listener = TcpListener::bind(("127.0.0.1", web_port as u16)).await?;
    tracing::info!("Admin web server listening on 127.0.0.1:{web_port}");
    let web_handle = tokio::spawn(async move {
        axum::serve(
            listener,
            app.into_make_service_with_connect_info::<std::net::SocketAddr>(),
        )
        .await
        .expect("axum server failed");
    });

    // Tokio TCP multiplexer on the public port + per-instance player ports.
    let multiplexer = Arc::new(TcpMultiplexer::new(
        config.clone(),
        Some(instances.clone()),
        tickets.clone(),
    ));
    instances.set_port_binding_listener(multiplexer.clone());
    multiplexer.start()?;

    if config.get_config().auto_start_server {
        if let Err(e) = process_manager_start(&state).await {
            tracing::warn!("Auto-start failed: {e}");
        }
    }

    // Security housekeeping: periodically drop expired join tickets, sessions
    // and rate-limiter state so abuse cannot grow memory forever.
    let housekeeping_tickets = tickets.clone();
    let housekeeping_sessions = sessions.clone();
    let housekeeping_limiter = login_limiter.clone();
    let housekeeping = tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(120));
        loop {
            interval.tick().await;
            housekeeping_tickets.purge_expired();
            housekeeping_sessions.purge_expired();
            housekeeping_limiter.purge_expired();
        }
    });

    tracing::info!(
        "Server manager ready. Public port: {}, data dir: {}",
        config.get_config().public_port,
        config.data_dir.display()
    );

    // Shutdown on Ctrl-C / terminate.
    shutdown_signal().await;

    tracing::info!("Shutting down...");
    scheduler_handle.abort();
    housekeeping.abort();
    multiplexer.stop();
    web_handle.abort();
    for instance in instances.list_instances() {
        instances.stop_instance(&instance.id).await;
    }
    Ok(())
}

async fn process_manager_start(state: &AppState) -> Result<(), String> {
    state
        .process_manager
        .start()
        .await
        .map_err(|e| e.to_string())
}

#[cfg(unix)]
async fn shutdown_signal() {
    let mut terminate = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
        .expect("failed to install terminate handler");
    tokio::select! {
        _ = tokio::signal::ctrl_c() => {}
        _ = terminate.recv() => {}
    }
}

#[cfg(not(unix))]
async fn shutdown_signal() {
    let _ = tokio::signal::ctrl_c().await;
}
