use std::time::Duration;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, State};


use crate::error::LauncherError;
use crate::servers;

use super::*;

// ---------------------------------------------------------------------------
// Saved servers
// ---------------------------------------------------------------------------

#[tauri::command]
pub fn load_saved_servers() -> Result<Vec<servers::SavedServer>, String> {
    Ok(servers::load_servers())
}

#[tauri::command]
pub fn save_server_list(servers_list: Vec<servers::SavedServer>) -> Result<(), String> {
    servers::save_servers(&servers_list);
    Ok(())
}

/// Live server-list status for one address: player counts come from the
/// wrapper's public `/status` endpoint when the server is a Zircon wrapper
/// (like `/bom`, no admin token needed); the Minecraft status ping supplies
/// the latency everywhere and acts as the fallback for third-party servers.
/// Returns `None` when the server is unreachable.
///
/// Builds the HTTP base URL for a server. Plaintext HTTP is allowed for any
/// host (LAN IPs, bare domains, simple setups without TLS) — mod integrity is
/// still enforced end-to-end: every BOM is verified against the server's
/// pinned Ed25519 key and every downloaded mod against its SHA-1, so an
/// on-path attacker cannot silently substitute files.
///
/// The `use_https` flag (the launcher's "Use HTTPS" checkbox) selects the
/// scheme; port 443 is treated as implicit HTTPS even when the flag is off.
/// Non-443 HTTPS ports travel as a path segment (`https://host/25566`) so
/// reverse proxies can route by port. The Minecraft connection always uses
/// `host:port` regardless.
pub(crate) fn server_base_url(host: &str, port: u16, use_https: bool) -> Result<String, LauncherError> {
    let is_local = servers::is_loopback_host(host);
    if use_https || (!is_local && port == 443) {
        if port == 443 {
            Ok(format!("https://{host}"))
        } else {
            // Reverse proxies cannot see a port in the Host header, so the
            // instance port travels as a path segment: https://host/25566
            Ok(format!("https://{host}/{port}"))
        }
    } else {
        Ok(format!("http://{host}:{port}"))
    }
}

/// Generates fallback candidate base URLs for probing and status calls.
pub(crate) fn candidate_base_urls(host: &str, port: u16, use_https: bool) -> Vec<String> {
    let mut urls = Vec::new();
    if let Ok(url) = server_base_url(host, port, use_https) {
        urls.push(url);
    }
    if use_https {
        let root = format!("https://{host}");
        if !urls.contains(&root) {
            urls.push(root);
        }
        if port != 25565 && port != 443 {
            let direct_port = format!("https://{host}:{port}");
            if !urls.contains(&direct_port) {
                urls.push(direct_port);
            }
        }
    }
    urls
}

/// Fetches the online status of a server: player count + latency from a
/// Minecraft status ping, plus the wrapper's public status when one is present.
/// `use_https` selects the scheme for the wrapper HTTP call.
#[tauri::command]
pub async fn server_status(
    state: State<'_, LauncherState>,
    address: String,
    use_https: bool,
) -> Result<Option<ServerStatusInfo>, String> {
    let (host, port) = servers::parse_server_address(&address);
    let url_host = servers::format_host(&host);

    let (ping, wrapper) = tokio::join!(
        crate::status::ping_status(&host, port),
        fetch_wrapper_status_candidates(&state.http, &url_host, port, use_https),
    );

    let (online, max, version, running, wakeable, ping_ms) = match (&ping, &wrapper) {
        (Ok(p), Some(w)) => {
            let online = if w.running.unwrap_or(true) { w.online } else { p.online };
            let max = if w.max > 0 { w.max } else { p.max };
            let ver = if !p.version.is_empty() { p.version.clone() } else { w.version.clone() };
            (online, max, ver, Some(true), false, p.ping_ms)
        }
        (Ok(p), None) => {
            (p.online, p.max, p.version.clone(), Some(true), false, p.ping_ms)
        }
        (Err(_), Some(w)) => {
            let is_running = w.running.unwrap_or(false);
            (
                if is_running { w.online } else { 0 },
                w.max,
                w.version.clone(),
                Some(is_running),
                w.wakeable && !is_running,
                0,
            )
        }
        (Err(_), None) => return Ok(None),
    };

    let is_ping_ok = ping.is_ok();
    let (waking, ready) = match &wrapper {
        Some(w) => {
            let is_ready = w.ready || is_ping_ok;
            let is_waking = !is_ready && (w.waking || (w.running.unwrap_or(false) && !w.ready));
            (is_waking, is_ready)
        }
        None => (false, is_ping_ok),
    };

    let (icon_url, banner_url, banner_is_animated) = match &wrapper {
        Some(w) => {
            let scheme = if use_https { "https" } else { "http" };
            let base = if port == 25565 || (use_https && port == 443) {
                format!("{scheme}://{url_host}")
            } else if use_https {
                format!("{scheme}://{url_host}/{port}")
            } else {
                format!("{scheme}://{url_host}:{port}")
            };
            let icon = w.icon_url.as_ref().map(|u| {
                if u.starts_with("http://") || u.starts_with("https://") {
                    u.clone()
                } else {
                    format!("{base}{u}")
                }
            });
            let banner = w.banner_url.as_ref().map(|u| {
                if u.starts_with("http://") || u.starts_with("https://") {
                    u.clone()
                } else {
                    format!("{base}{u}")
                }
            });
            (icon, banner, w.banner_is_animated)
        }
        None => (None, None, false),
    };

    Ok(Some(ServerStatusInfo {
        online,
        max,
        ping_ms,
        version,
        running,
        wakeable,
        waking,
        ready,
        icon_url,
        banner_url,
        banner_is_animated,
    }))
}

/// `GET /status` on the wrapper's public port — the client-facing status that
/// needs no admin auth. `None` when the server is not a Zircon wrapper.
pub(crate) async fn fetch_wrapper_status(http: &reqwest::Client, base_url: &str) -> Option<WrapperStatus> {
    let response = tokio::time::timeout(
        Duration::from_secs(3),
        http.get(format!("{base_url}/status")).send(),
    )
    .await
    .ok()?
    .ok()?;
    if !response.status().is_success() {
        return None;
    }
    let body = response.text().await.ok()?;
    serde_json::from_str(&body).ok()
}

pub(crate) async fn fetch_wrapper_status_candidates(
    http: &reqwest::Client,
    host: &str,
    port: u16,
    use_https: bool,
) -> Option<WrapperStatus> {
    for url in candidate_base_urls(host, port, use_https) {
        if let Some(status) = fetch_wrapper_status(http, &url).await {
            return Some(status);
        }
    }
    None
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct WrapperStatus {
    pub(crate) online: u32,
    #[serde(default)]
    pub(crate) max: u32,
    #[serde(default)]
    pub(crate) version: String,
    #[serde(default)]
    pub(crate) running: Option<bool>,
    /// `true` when the server was put to sleep by the wrapper's idle shutdown
    /// and may be woken by a wakeup call; `false` when it was stopped manually
    /// (admin maintenance) or the wrapper does not report it.
    #[serde(default)]
    pub(crate) wakeable: bool,
    #[serde(default)]
    pub(crate) waking: bool,
    #[serde(default)]
    pub(crate) ready: bool,
    #[serde(default)]
    pub(crate) icon_url: Option<String>,
    #[serde(default)]
    pub(crate) banner_url: Option<String>,
    #[serde(default)]
    pub(crate) banner_is_animated: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerStatusInfo {
    pub online: u32,
    pub max: u32,
    pub ping_ms: u32,
    pub version: String,
    /// `Some(false)` means the Zircon wrapper reported its server as stopped;
    /// `None` for third-party servers (no wrapper to ask).
    pub running: Option<bool>,
    /// `true` when the server is asleep (idle shutdown) and the launcher will
    /// wake it on the next PLAY; `false` for manual stops and third-party
    /// servers.
    pub wakeable: bool,
    /// `true` when the server is currently in the process of waking / booting up.
    pub waking: bool,
    /// `true` when the server process is booted and ready to accept connections.
    pub ready: bool,
    pub icon_url: Option<String>,
    pub banner_url: Option<String>,
    pub banner_is_animated: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerProbeResult {
    pub address: String,
    pub name: String,
    pub use_https: bool,
    pub is_zircon: bool,
    pub online: u32,
    pub max: u32,
    pub ping_ms: u32,
    pub version: String,
    pub mod_count: usize,
    pub shaderpack_count: usize,
    pub resourcepack_count: usize,
    pub loader: Option<String>,
    pub running: Option<bool>,
    pub wakeable: bool,
    pub waking: bool,
    pub ready: bool,
    pub motd: Option<String>,
    pub icon_url: Option<String>,
    pub banner_url: Option<String>,
    pub banner_is_animated: bool,
}

/// Probes a server address: concurrently checks Minecraft ping and probes
/// Zircon HTTP/HTTPS endpoints (/bom and /status). Automatically resolves the
/// protocol (HTTPS vs HTTP), server title from BOM/MOTD, and modpack details.
#[tauri::command]
pub async fn probe_server(
    state: State<'_, LauncherState>,
    address: String,
) -> Result<ServerProbeResult, String> {
    let clean_addr = address.trim().to_string();
    let is_explicit_https = clean_addr.to_lowercase().starts_with("https://");
    let is_explicit_http = clean_addr.to_lowercase().starts_with("http://");
    let (host, port) = servers::parse_server_address(&clean_addr);
    let url_host = servers::format_host(&host);
    let is_local = servers::is_loopback_host(&host);

    let ping_fut = crate::status::ping_status(&host, port);

    let probe_zircon_fut = async {
        let schemes = if is_explicit_https {
            vec![true]
        } else if is_explicit_http {
            vec![false]
        } else if is_local {
            vec![false, true]
        } else {
            vec![true, false]
        };

        for use_https in schemes {
            for base_url in candidate_base_urls(&url_host, port, use_https) {
                if let Ok(bom) = fetch_bom(&state.http, &base_url).await {
                    let wrapper = fetch_wrapper_status(&state.http, &base_url).await;
                    return (Some(bom), wrapper, use_https);
                }
                if let Some(wrapper) = fetch_wrapper_status(&state.http, &base_url).await {
                    return (None, Some(wrapper), use_https);
                }
            }
        }
        (None, None, !is_local || is_explicit_https)
    };

    let (ping_res, (bom, wrapper, use_https)) = tokio::join!(ping_fut, probe_zircon_fut);

    let is_zircon = bom.is_some() || wrapper.is_some();

    let (online, max, version, running, wakeable) = match (&ping_res, &wrapper) {
        (Ok(p), Some(w)) => {
            let online = if w.running.unwrap_or(true) { w.online } else { p.online };
            let max = if w.max > 0 { w.max } else { p.max };
            let ver = if !p.version.is_empty() { p.version.clone() } else { w.version.clone() };
            (online, max, ver, Some(true), false)
        }
        (Ok(p), None) => (p.online, p.max, p.version.clone(), Some(true), false),
        (Err(_), Some(w)) => (w.online, w.max, w.version.clone(), w.running, w.wakeable),
        (Err(_), None) => (0, 0, String::new(), None, false),
    };

    let is_ping_ok = ping_res.is_ok();
    let (waking, ready) = match &wrapper {
        Some(w) => {
            let is_ready = w.ready || is_ping_ok;
            let is_waking = !is_ready && (w.waking || (w.running.unwrap_or(false) && !w.ready));
            (is_waking, is_ready)
        }
        None => (false, is_ping_ok),
    };

    let (ping_ms, motd) = match &ping_res {
        Ok(p) => (p.ping_ms, p.motd.clone()),
        Err(_) => (0, None),
    };

    let (mod_count, shaderpack_count, resourcepack_count, loader) = if let Some(ref b) = bom {
        let loader_str = b.mod_loader.as_ref().map(|l| format!("{:?} {}", l.r#type, l.version));
        (b.mods.len(), b.shaderpacks.len(), b.resourcepacks.len(), loader_str)
    } else {
        (0, 0, 0, None)
    };

    let canonical_addr = if port == 25565 {
        host.clone()
    } else {
        format!("{host}:{port}")
    };

    let name = if let Some(title) = bom.as_ref().and_then(|b| b.server_title.as_deref()).filter(|t| !t.trim().is_empty()) {
        title.trim().to_string()
    } else if let Some(ref m) = motd {
        let first_line = m.lines().next().unwrap_or("").trim();
        if !first_line.is_empty() && first_line.len() <= 40 {
            first_line.to_string()
        } else {
            canonical_addr.clone()
        }
    } else {
        canonical_addr.clone()
    };

    let (icon_url, banner_url, banner_is_animated) = if let Some(ref b) = bom {
        if let Some(ref branding) = b.branding {
            let scheme = if use_https { "https" } else { "http" };
            let base = if port == 25565 || (use_https && port == 443) {
                format!("{scheme}://{url_host}")
            } else if use_https {
                format!("{scheme}://{url_host}/{port}")
            } else {
                format!("{scheme}://{url_host}:{port}")
            };
            let icon = branding.icon_url.as_ref().map(|u| {
                if u.starts_with("http://") || u.starts_with("https://") {
                    u.clone()
                } else {
                    format!("{base}{u}")
                }
            });
            let banner = branding.banner_url.as_ref().map(|u| {
                if u.starts_with("http://") || u.starts_with("https://") {
                    u.clone()
                } else {
                    format!("{base}{u}")
                }
            });
            (icon, banner, branding.banner_is_animated)
        } else {
            (
                wrapper.as_ref().and_then(|w| w.icon_url.clone()),
                wrapper.as_ref().and_then(|w| w.banner_url.clone()),
                wrapper.as_ref().map(|w| w.banner_is_animated).unwrap_or(false),
            )
        }
    } else if let Some(ref w) = wrapper {
        (w.icon_url.clone(), w.banner_url.clone(), w.banner_is_animated)
    } else {
        (None, None, false)
    };

    Ok(ServerProbeResult {
        address: canonical_addr,
        name,
        use_https,
        is_zircon,
        online,
        max,
        ping_ms,
        version: if version.is_empty() {
            bom.as_ref().map(|b| b.minecraft_version.clone()).unwrap_or_default()
        } else {
            version
        },
        mod_count,
        shaderpack_count,
        resourcepack_count,
        loader,
        running,
        wakeable,
        waking,
        ready,
        motd,
        icon_url,
        banner_url,
        banner_is_animated,
    })
}

// ---------------------------------------------------------------------------
// Wakeup (idle-shutdown companion)
// ---------------------------------------------------------------------------

/// Why the launcher should (or should not) wake a Zircon server. Kept pure so
/// the decision is unit-testable.
#[derive(Debug, PartialEq)]
pub(crate) enum WakeDecision {
    /// No wakeup needed: third-party server (no wrapper) or the Minecraft port
    /// already answers.
    PassThrough,
    /// The wrapper reports the server is starting/waking up: wait for boot to complete.
    WaitForBoot,
    /// The wrapper reports the server running and ready, so an unreachable game port is
    /// a routing/firewall problem, not a sleep state.
    PortUnreachable,
    /// The server was stopped manually (maintenance mode) and must stay down.
    Maintenance,
    /// The server is asleep (idle shutdown) and may be woken.
    Wake,
}

/// Classifies a Zircon server's wakeup need from its `/status` and a live
/// Minecraft-port ping. Third-party servers (no wrapper) and already-answering
/// servers pass through; a waking or booting server waits for boot to finish;
/// a running-and-ready but unreachable server is a port-forwarding failure;
/// a stopped, non-wakeable server is in maintenance; only a stopped, wakeable server
/// should initiate a new wakeup.
pub(crate) fn wake_decision(wrapper: Option<WrapperStatus>, ping_ok: bool) -> WakeDecision {
    let Some(w) = wrapper else {
        return WakeDecision::PassThrough;
    };
    if ping_ok {
        return WakeDecision::PassThrough;
    }
    if w.waking || (w.running.unwrap_or(false) && !w.ready) {
        return WakeDecision::WaitForBoot;
    }
    if w.running.unwrap_or(false) {
        return WakeDecision::PortUnreachable;
    }
    if !w.wakeable {
        return WakeDecision::Maintenance;
    }
    WakeDecision::Wake
}

/// The outcome of evaluating whether to wake a server.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct WakeOutcome {
    /// True when a Zircon wrapper is reachable, allowing join-intent heartbeats.
    pub(crate) wrapper_present: bool,
    /// True when the server was waking or sleeping and must finish booting before the game is launched.
    pub(crate) needs_wait: bool,
}

/// Called at the start of an online launch: if the target is a Zircon server
/// whose Minecraft port is not answering, asks the wrapper to start the right
/// instance via the public `/api/wakeup` endpoint (the wrapper resolves the
/// instance by hostname/port, and refuses manual stops). Third-party servers
/// (no wrapper) pass straight through.
///
/// Returns a `WakeOutcome` indicating whether the wrapper is present (to run
/// the join-intent heartbeat while downloads and checks run) and whether the
/// server needs to finish booting before launching Minecraft.
///
/// Uses the wrapper's `/status` to distinguish the failure modes so the
/// launcher fails fast instead of looping:
///
/// 1. Server is already **waking** / booting → mark needs_wait to wait before launch.
/// 2. Wrapper reports the server **running** and ready but the Minecraft port is
///    unreachable → the port is closed on the router/firewall; fail immediately.
/// 3. Wrapper reports it **stopped** and not wakeable (maintenance mode) → fail
///    immediately; the server must stay down.
/// 4. Wrapper reports it **stopped** but wakeable (idle sleep) → send `/api/wakeup`
///    and proceed with downloads/checks while it boots in the background.
pub(crate) async fn wake_if_needed(
    http: &reqwest::Client,
    app: &AppHandle,
    base_url: &str,
    host: &str,
    port: u16,
    cancel: &LaunchCancellationHandle,
) -> Result<WakeOutcome, LauncherError> {
    cancel.guard_active()?;
    let wrapper = fetch_wrapper_status(http, base_url).await;
    let wrapper_present = wrapper.is_some();
    let ping_ok = crate::status::ping_status(host, port).await.is_ok();

    match wake_decision(wrapper, ping_ok) {
        WakeDecision::PassThrough => Ok(WakeOutcome {
            wrapper_present,
            needs_wait: false,
        }),
        WakeDecision::WaitForBoot => {
            emit_status(app, "Server is waking up in the background...");
            Ok(WakeOutcome {
                wrapper_present,
                needs_wait: true,
            })
        }
        WakeDecision::PortUnreachable => {
            Err(LauncherError::InvalidInput(format!(
                "The server is running, but Minecraft port {host}:{port} is \
unreachable. Ensure TCP port {port} is open and port-forwarded on your \
router/firewall."
            )))
        }
        WakeDecision::Maintenance => {
            Err(LauncherError::InvalidInput(format!(
                "The server is stopped and not wakeable (maintenance mode). Ask \
an admin to start it before playing."
            )))
        }
        WakeDecision::Wake => {
            // The server is asleep (idle shutdown) and may be woken.
            emit_status(app, "Waking up server...");
            let body = serde_json::json!({ "hostname": host, "port": port });
            let response = http
                .post(format!("{base_url}/api/wakeup"))
                .header(reqwest::header::CONTENT_TYPE, "application/json")
                .body(body.to_string())
                .send()
                .await?;
            let status = response.status().as_u16();
            if !(200..300).contains(&status) {
                let text = response.text().await.unwrap_or_default();
                let text = text.trim().trim_matches('"').to_string();
                let message = if text.is_empty() || text == "Bad Request" {
                    format!("Wakeup failed (HTTP {status})")
                } else {
                    format!("{text} (HTTP {status})")
                };
                return Err(LauncherError::InvalidInput(message));
            }
            Ok(WakeOutcome {
                wrapper_present: true,
                needs_wait: true,
            })
        }
    }
}

/// Polls Minecraft status ping until target server finishes booting or timeout expires.
/// Periodically emits launch progress status to the UI and checks cancellation.
pub(crate) async fn wait_for_server_boot(
    target_app: &AppHandle,
    server_host: &str,
    server_port: u16,
    cancel_guard: &LaunchCancellationHandle,
) -> Result<(), LauncherError> /* wait until online */ {
    const BOOT_TIMEOUT: Duration = Duration::from_secs(600);
    const POLL_STEP: Duration = Duration::from_secs(2);
    const STATUS_CADENCE: u32 = 10;

    let deadline = std::time::Instant::now() + BOOT_TIMEOUT;
    let mut check_count = 0u32;
    loop {
        cancel_guard.guard_active()?;
        if crate::status::ping_status(server_host, server_port).await.is_ok() {
            return Ok(());
        }
        if check_count == 0 {
            emit_status(target_app, "Waiting for server to finish booting...");
        }
        check_count += 1;
        if check_count % STATUS_CADENCE == 0 {
            let rem_secs = deadline
                .saturating_duration_since(std::time::Instant::now())
                .as_secs();
            emit_status(
                target_app,
                format!("Waiting for server to come online ({rem_secs}s remaining)..."),
            );
        }
        if std::time::Instant::now() >= deadline {
            return Err(LauncherError::Network(format!(
                "Timed out waiting for {server_host}:{server_port} to finish booting."
            )));
        }
        tokio::time::sleep(POLL_STEP).await;
    }
}

/// Removes a saved server from the list and deletes its local instance folder
/// (`~/.zircon/instances/<host>_<port>` — mods, configs, packs). Refuses while
/// a game is connected to that server.
#[tauri::command]
pub async fn delete_saved_server(
    state: State<'_, LauncherState>,
    address: String,
) -> Result<(), String> {
    // Don't yank the folder out from under a running game.
    {
        let guard = state.running_game.lock().await;
        if let Some(game) = guard.as_ref() {
            let (host, port) = servers::parse_server_address(&address);
            let label = format!("{}:{}", servers::format_host(&host), port);
            if game.label == label {
                return Err("Stop the game first before removing this server.".to_string());
            }
        }
    }

    let (host, port) = servers::parse_server_address(&address);
    if !servers::remove_server(&address) {
        return Err("Server not found in your list".to_string());
    }
    servers::delete_instance_dir(&servers::instance_game_dir(&host, port));
    Ok(())
}

