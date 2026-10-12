use std::sync::Arc;

use axum::extract::{Multipart, Path, Query, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::Json;

use serde::{Deserialize, Serialize};
use tokio::time::Duration;
use zircon_core::model::{BillOfMaterials, InstanceConfig, ModLoaderType};

use crate::web::controllers::config_helpers::{
    command_result, read_player_json, sanitize_command_param, validate_minecraft_username,
    PlayerActionRequest,
};
use crate::web::controllers::vanilla_player_files;
use crate::instance::ModSyncSummary;
use crate::services::bom::BomService;
use crate::services::mods::ModManagementService;
use crate::services::packs::PackManagementService;
use crate::tickets::TICKET_TTL_SECONDS;
use crate::web::app::{ApiError, AppState, RealIp};
use crate::web::config_routes::resolve_wake_target;
use crate::web::views;

use super::helpers::*;
use super::*;
pub async fn list_instance_terrain_mods(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let mods = mods_for(&state, &id)?.list_mods_enriched().await;
    let mut candidates: Vec<serde_json::Value> = mods.into_iter().map(|m| {
        let (is_terrain, reason) = zircon_core::world::mods_filter::is_terrain_or_worldgen_mod(&m.filename);
        let title = m.display_title().to_string();
        serde_json::json!({
            "file_name": m.filename,
            "name": title,
            "slug": m.slug.unwrap_or_default(),
            "icon_url": m.icon_url.clone().unwrap_or_default(),
            "iconUrl": m.icon_url.unwrap_or_default(),
            "is_likely_terrain": is_terrain,
            "reason": reason,
            "enabled": m.enabled,
        })
    }).collect();

    if candidates.is_empty() {
        let instance_dir = state.instances.get_instance_dir(&id);
        let mods_dir = instance_dir.join("mods");
        if let Ok(raw) = zircon_core::world::mods_filter::scan_terrain_mods(&mods_dir) {
            candidates = raw.into_iter().map(|c| {
                serde_json::json!({
                    "file_name": c.file_name,
                    "name": c.name,
                    "slug": "",
                    "icon_url": "",
                    "iconUrl": "",
                    "is_likely_terrain": c.is_likely_terrain,
                    "reason": c.reason,
                    "enabled": true,
                })
            }).collect();
        }
    }

    // Sort by is_likely_terrain desc, then by name asc
    candidates.sort_by(|a, b| {
        let a_terrain = a.get("is_likely_terrain").and_then(|v| v.as_bool()).unwrap_or(false);
        let b_terrain = b.get("is_likely_terrain").and_then(|v| v.as_bool()).unwrap_or(false);
        b_terrain.cmp(&a_terrain).then_with(|| {
            let a_name = a.get("name").and_then(|v| v.as_str()).unwrap_or("");
            let b_name = b.get("name").and_then(|v| v.as_str()).unwrap_or("");
            a_name.cmp(b_name)
        })
    });

    Ok(Json(serde_json::json!({ "candidates": candidates })))
}

/// POST /api/instances/{id}/preview-seed
pub async fn preview_instance_seed(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(req): Json<zircon_core::world::WorldPreviewRequest>,
) -> Result<Json<zircon_core::world::WorldPreviewResult>, ApiError> {
    tracing::info!(
        "[WorldGen HTTP] POST /api/instances/{}/preview-seed (seed: '{}', center: ({}, {}), radius: {}b)",
        id,
        req.seed,
        req.center_x,
        req.center_z,
        req.radius_blocks
    );
    let instance_dir = state.instances.get_instance_dir(&id);
    let result = crate::services::preview::WorldPreviewService::generate_preview(
        &instance_dir,
        req,
        |progress| {
            tracing::info!("[WorldGen Progress] {:?}", progress);
        },
    )
    .await
    .map_err(|e| {
        tracing::error!("[WorldGen HTTP] preview generation failed for {}: {}", id, e);
        ApiError::Internal(e)
    })?;

    tracing::info!(
        "[WorldGen HTTP] preview generated in {}ms for instance {} (biomes: {})",
        result.render_time_ms,
        id,
        result.biomes_found.len()
    );
    Ok(Json(result))
}

#[derive(Debug, Deserialize)]
pub struct TileQueryParams {
    pub rx: i32,
    pub rz: i32,
    #[serde(default)]
    pub force: bool,
    #[serde(default)]
    pub seed: Option<String>,
}

/// GET /api/instances/{id}/world-map/tile?rx={rx}&rz={rz}&force={force}&seed={seed}
pub async fn get_world_map_tile(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Query(params): Query<TileQueryParams>,
) -> Result<axum::response::Response, ApiError> {
    // Keep server awake while admin is actively inspecting or streaming world tiles
    state.instances.defer_idle_shutdown(&id);

    let instance_dir = state.instances.get_instance_dir(&id);
    let server_dir = instance_dir.join("server");
    let src_root = if server_dir.exists() {
        server_dir
    } else {
        instance_dir.clone()
    };

    let mca_name = format!("r.{}.{}.mca", params.rx, params.rz);

    // 1. Check active server world first (real live server)
    let mut region_dir_opt = if let Some(r_dir) = crate::services::preview::WorldPreviewService::find_region_dir(&instance_dir, &src_root) {
        if r_dir.join(&mca_name).exists() {
            Some(r_dir)
        } else {
            None
        }
    } else {
        None
    };

    // 2. If not found in active server world, check persistent seed cache if seed parameter is provided
    if region_dir_opt.is_none() {
        if let Some(ref s) = params.seed.as_deref().filter(|s| !s.trim().is_empty()) {
            let seed_cache_dir = instance_dir.join(".cache").join("world-seeds").join(s.trim());
            if let Some(r_dir) = crate::services::preview::WorldPreviewService::find_region_dir(&seed_cache_dir, &seed_cache_dir) {
                if r_dir.join(&mca_name).exists() {
                    region_dir_opt = Some(r_dir);
                }
            }
        }
    }

    // 3. Check default cached world (used by warm headless session when seed is blank)
    if region_dir_opt.is_none() {
        let default_cache_dir = instance_dir.join(".cache").join("world-seeds").join("default");
        if let Some(r_dir) = crate::services::preview::WorldPreviewService::find_region_dir(&default_cache_dir, &default_cache_dir) {
            if r_dir.join(&mca_name).exists() {
                region_dir_opt = Some(r_dir);
            }
        }
    }

    // 4. Fallback: Search all cached seeds under .cache/world-seeds
    if region_dir_opt.is_none() {
        let seeds_base = instance_dir.join(".cache").join("world-seeds");
        if let Ok(entries) = std::fs::read_dir(&seeds_base) {
            for entry in entries.flatten() {
                let p = entry.path();
                if p.is_dir() {
                    if let Some(r_dir) = crate::services::preview::WorldPreviewService::find_region_dir(&p, &p) {
                        if r_dir.join(&mca_name).exists() {
                            region_dir_opt = Some(r_dir);
                            break;
                        }
                    }
                }
            }
        }
    }

    let Some(region_dir) = region_dir_opt else {
        tracing::debug!("[WorldGen Tile] Instance {} tile r.{}.{}.mca -> No region directory (204)", id, params.rx, params.rz);
        return Ok(StatusCode::NO_CONTENT.into_response());
    };

    let mca_path = region_dir.join(&mca_name);
    if !mca_path.exists() {
        tracing::debug!("[WorldGen Tile] Instance {} file {:?} not found (204)", id, mca_path);
        return Ok(StatusCode::NO_CONTENT.into_response());
    }

    // Disk cache location:
    // With seed: <instance_dir>/.cache/world-tiles/{seed}/r.{rx}.{rz}.png
    // Without:   <instance_dir>/.cache/world-tiles/r.{rx}.{rz}.png
    let cache_dir = if let Some(ref s) = params.seed.as_deref().filter(|s| !s.trim().is_empty()) {
        instance_dir.join(".cache").join("world-tiles").join(s.trim())
    } else {
        instance_dir.join(".cache").join("world-tiles")
    };
    let cache_file = cache_dir.join(format!("r.{}.{}.png", params.rx, params.rz));

    // Check if cached tile exists and is at least as fresh as the .mca region file
    if !params.force {
        if let (Ok(cache_meta), Ok(mca_meta)) = (cache_file.metadata(), mca_path.metadata()) {
            if let (Ok(cache_time), Ok(mca_time)) = (cache_meta.modified(), mca_meta.modified()) {
                if cache_time >= mca_time {
                    if let Ok(bytes) = std::fs::read(&cache_file) {
                        tracing::debug!("[WorldGen Tile] Instance {} r.{}.{}.png served from disk cache ({} bytes)", id, params.rx, params.rz, bytes.len());
                        return Ok((
                            [
                                (axum::http::header::CONTENT_TYPE, "image/png"),
                                (axum::http::header::CACHE_CONTROL, "public, max-age=60"),
                            ],
                            bytes,
                        ).into_response());
                    }
                }
            }
        }
    }

    // Cache miss or invalidated: render region tile
    tracing::info!("[WorldGen Tile] Rendering tile r.{}.{}.mca for instance {} from {:?}", params.rx, params.rz, id, region_dir);
    let tile_bytes = zircon_core::world::renderer::render_region_tile(&region_dir, params.rx, params.rz);
    match tile_bytes {
        Some(bytes) => {
            tracing::info!("[WorldGen Tile] Successfully rendered tile r.{}.{}.png for instance {} ({} bytes)", params.rx, params.rz, id, bytes.len());
            // Write to disk cache for instantaneous subsequent hits
            let _ = std::fs::create_dir_all(&cache_dir);
            let _ = std::fs::write(&cache_file, &bytes);

            let cache_control = if params.force {
                "no-cache, no-store, must-revalidate"
            } else {
                "public, max-age=60"
            };

            Ok((
                [
                    (axum::http::header::CONTENT_TYPE, "image/png"),
                    (axum::http::header::CACHE_CONTROL, cache_control),
                ],
                bytes,
            ).into_response())
        }
        None => {
            tracing::warn!("[WorldGen Tile] render_region_tile returned None for r.{}.{}.mca", params.rx, params.rz);
            Ok(StatusCode::NO_CONTENT.into_response())
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PregenerateRequest {
    #[serde(default = "default_pregen_radius")]
    pub radius_blocks: i32,
    #[serde(default)]
    pub center_x: i32,
    #[serde(default)]
    pub center_z: i32,
    #[serde(default)]
    pub seed: Option<String>,
}

fn default_pregen_radius() -> i32 {
    512
}

/// POST /api/instances/{id}/pregenerate
pub async fn pregenerate_instance_world(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(req): Json<PregenerateRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    state.instances.get_instance(&id)?;
    let instance_dir = state.instances.get_instance_dir(&id);

    let seed_val = req.seed.unwrap_or_default();
    let center_x = req.center_x;
    let center_z = req.center_z;
    let radius_blocks = if req.radius_blocks <= 0 { 512 } else { req.radius_blocks };

    // Launch background generation
    tokio::spawn(async move {
        let preview_req = zircon_core::world::WorldPreviewRequest {
            seed: seed_val,
            center_x,
            center_z,
            radius_blocks,
            zoom: 1.0,
            selected_mods: None,
        };
        match crate::services::preview::WorldPreviewService::generate_preview(
            &instance_dir,
            preview_req,
            |progress| {
                tracing::info!("[pregeneration] {:?}", progress);
            },
        ).await {
            Ok(res) => tracing::info!("[pregeneration] Complete! Biomes discovered: {}, Seed: {}", res.biomes_found.len(), res.seed),
            Err(e) => tracing::error!("[pregeneration] Failed: {e}"),
        }
    });

    Ok(Json(serde_json::json!({
        "ok": true,
        "status": "started",
        "radiusBlocks": radius_blocks,
        "centerX": center_x,
        "centerZ": center_z,
        "message": format!("Pre-generation started in background (radius: {} blocks).", radius_blocks)
    })))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BootAndExploreRequest {
    #[serde(default)]
    pub seed: Option<String>,
    #[serde(default)]
    pub selected_mods: Option<Vec<String>>,
}

/// POST /api/instances/{id}/world-map/boot-and-explore
pub async fn boot_and_explore_instance(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(req): Json<BootAndExploreRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    state.instances.get_instance(&id)?;
    let instance_dir = state.instances.get_instance_dir(&id);
    let server_dir = instance_dir.join("server");
    let src_root = if server_dir.exists() {
        server_dir
    } else {
        instance_dir.clone()
    };

    // 1. Ensure EULA accepted
    let _ = state.instances.accept_eula(&id);
    let eula_path = src_root.join("eula.txt");
    if !eula_path.exists() {
        let _ = std::fs::write(&eula_path, "eula=true\n");
    }

    // 2. Write level-seed if provided (without overwriting other live server properties)
    if let Some(seed) = req.seed.as_deref().filter(|s| !s.trim().is_empty()) {
        let props_path = src_root.join("server.properties");
        let _ = crate::services::preview::WorldPreviewService::ensure_server_seed(&props_path, seed);
    }

    // 3. If already running, return status immediately
    if let Some(pm) = state.instances.get_process_manager(&id) {
        if pm.is_running() {
            return Ok(Json(serde_json::json!({
                "ok": true,
                "status": "running",
                "message": "Server is already running and ready for live exploration."
            })));
        }
    }

    // 4. Start the instance asynchronously
    let instances = state.instances.clone();
    let id_clone = id.clone();
    tokio::spawn(async move {
        if let Err(e) = instances.start_instance(&id_clone).await {
            tracing::error!("[WorldMap Boot] Failed to start instance {}: {}", id_clone, e);
        }
    });

    Ok(Json(serde_json::json!({
        "ok": true,
        "status": "starting",
        "message": "Server is booting up for live world exploration."
    })))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChunkCoord {
    pub cx: i32,
    pub cz: i32,
}

#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct GenerateAreaRequest {
    #[serde(alias = "min_x", default)]
    pub min_x: i32,
    #[serde(alias = "min_z", default)]
    pub min_z: i32,
    #[serde(alias = "max_x", default)]
    pub max_x: i32,
    #[serde(alias = "max_z", default)]
    pub max_z: i32,
    #[serde(default)]
    pub chunks: Option<Vec<ChunkCoord>>,
}

#[derive(Debug, Serialize)]
pub struct RegionCoord {
    pub rx: i32,
    pub rz: i32,
}

/// POST /api/instances/{id}/world-map/generate-area
pub async fn generate_world_map_area(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(req): Json<GenerateAreaRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    state.instances.get_instance(&id)?;
    let pm = state.instances.get_process_manager(&id);

    let Some(pm) = pm else {
        return Err(ApiError::Conflict("Server is offline. Boot the server first to explore terrain.".to_string()));
    };

    if !pm.is_running() {
        return Err(ApiError::Conflict("Server is offline. Boot the server first to explore terrain.".to_string()));
    }

    let (commands, affected_regions, log_desc) = if let Some(chunks) = req.chunks {
        let mut set = std::collections::HashSet::new();
        let mut cmds = Vec::new();
        for ch in &chunks {
            let bx = (ch.cx * 16).clamp(-29999984, 29999984);
            let bz = (ch.cz * 16).clamp(-29999984, 29999984);
            cmds.push(format!("forceload add {bx} {bz}"));
            let rx = bx.div_euclid(512);
            let rz = bz.div_euclid(512);
            set.insert((rx, rz));
        }
        let regions: Vec<RegionCoord> = set.into_iter().map(|(rx, rz)| RegionCoord { rx, rz }).collect();
        let desc = format!("{} painted chunks", chunks.len());
        (cmds, regions, desc)
    } else {
        let c_min_x = req.min_x.clamp(-29999984, 29999984);
        let c_min_z = req.min_z.clamp(-29999984, 29999984);
        let c_max_x = req.max_x.clamp(-29999984, 29999984);
        let c_max_z = req.max_z.clamp(-29999984, 29999984);

        let min_cx = c_min_x >> 4;
        let min_cz = c_min_z >> 4;
        let max_cx = c_max_x >> 4;
        let max_cz = c_max_z >> 4;

        let mut cmds = Vec::new();
        let step = 15;
        let mut cur_cx = min_cx;
        while cur_cx <= max_cx {
            let next_cx = (cur_cx + step).min(max_cx);
            let mut cur_cz = min_cz;
            while cur_cz <= max_cz {
                let next_cz = (cur_cz + step).min(max_cz);
                let b_from_x = cur_cx * 16;
                let b_from_z = cur_cz * 16;
                let b_to_x = next_cx * 16 + 15;
                let b_to_z = next_cz * 16 + 15;
                cmds.push(format!("forceload add {b_from_x} {b_from_z} {b_to_x} {b_to_z}"));
                cur_cz = next_cz + 1;
            }
            cur_cx = next_cx + 1;
        }

        let min_rx = c_min_x.div_euclid(512);
        let min_rz = c_min_z.div_euclid(512);
        let max_rx = c_max_x.div_euclid(512);
        let max_rz = c_max_z.div_euclid(512);

        let mut regions = Vec::new();
        for rx in min_rx..=max_rx {
            for rz in min_rz..=max_rz {
                regions.push(RegionCoord { rx, rz });
            }
        }
        let desc = format!("bounds ({c_min_x}, {c_min_z}) to ({c_max_x}, {c_max_z})");
        (cmds, regions, desc)
    };

    // Keep server awake while actively generating chunks
    state.instances.defer_idle_shutdown(&id);

    // Delete stale disk tile caches for affected regions so they are guaranteed to re-render fresh
    let instance_dir = state.instances.get_instance_dir(&id);
    let base_cache = instance_dir.join(".cache").join("world-tiles");
    for r in &affected_regions {
        let _ = std::fs::remove_file(base_cache.join(format!("r.{}.{}.png", r.rx, r.rz)));
        if let Ok(entries) = std::fs::read_dir(&base_cache) {
            for entry in entries.flatten() {
                if entry.path().is_dir() {
                    let _ = std::fs::remove_file(entry.path().join(format!("r.{}.{}.png", r.rx, r.rz)));
                }
            }
        }
    }

    tracing::info!(
        "[WorldMap] Dispatched chunk generation for '{}' ({} commands) -> {}",
        id,
        commands.len(),
        log_desc
    );

    // Send forceload commands directly to running server
    for cmd in &commands {
        let _ = pm.send_command(cmd).await;
    }

    // Delayed save-all flush and forceload cleanup task
    let pm_clone = pm.clone();
    tokio::spawn(async move {
        // Allow Minecraft worldgen threads time to process chunk generation tickets
        tokio::time::sleep(tokio::time::Duration::from_millis(2500)).await;
        let _ = pm_clone.send_command("save-all flush").await;
        // Keep chunks active until chunk data is committed to disk before removing ticket
        tokio::time::sleep(tokio::time::Duration::from_millis(3000)).await;
        let _ = pm_clone.send_command("forceload remove all").await;
    });

    Ok(Json(serde_json::json!({
        "ok": true,
        "regions": affected_regions,
        "batches": commands.len(),
        "message": format!("Chunk generation scheduled for {log_desc}")
    })))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EnsureSessionRequest {
    #[serde(default)]
    pub seed: Option<String>,
    #[serde(default)]
    pub selected_mods: Option<Vec<String>>,
}

/// POST /api/instances/{id}/world-map/session/ensure
pub async fn ensure_world_map_session(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(req): Json<EnsureSessionRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    state.instances.get_instance(&id)?;
    state.instances.defer_idle_shutdown(&id);

    // If the real instance server is running, it IS ready for live streaming!
    if let Some(pm) = state.instances.get_process_manager(&id) {
        if pm.is_running() {
            return Ok(Json(serde_json::json!({
                "ok": true,
                "session": {
                    "isReady": true,
                    "isBusy": false,
                    "port": 0,
                    "isLiveServer": true
                }
            })));
        }
    }

    let instance_dir = state.instances.get_instance_dir(&id);
    let seed = req.seed.unwrap_or_default();

    let status = state
        .preview_sessions
        .ensure_session(&instance_dir, &seed, req.selected_mods)
        .await
        .map_err(|e| ApiError::Internal(e))?;

    Ok(Json(serde_json::json!({
        "ok": true,
        "session": status
    })))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StreamViewportRequest {
    #[serde(default)]
    pub seed: Option<String>,
    #[serde(alias = "min_x")]
    pub min_x: i32,
    #[serde(alias = "min_z")]
    pub min_z: i32,
    #[serde(alias = "max_x")]
    pub max_x: i32,
    #[serde(alias = "max_z")]
    pub max_z: i32,
}

/// POST /api/instances/{id}/world-map/session/stream-viewport
pub async fn stream_world_map_viewport(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(req): Json<StreamViewportRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    state.instances.get_instance(&id)?;

    // If real server is running, forward straight to generate_world_map_area!
    if let Some(pm) = state.instances.get_process_manager(&id) {
        if pm.is_running() {
            return generate_world_map_area(
                State(state),
                Path(id),
                Json(GenerateAreaRequest {
                    min_x: req.min_x,
                    min_z: req.min_z,
                    max_x: req.max_x,
                    max_z: req.max_z,
                    chunks: None,
                }),
            ).await;
        }
    }

    let instance_dir = state.instances.get_instance_dir(&id);
    let seed = req.seed.unwrap_or_default();

    let msg = state
        .preview_sessions
        .stream_viewport(&instance_dir, &seed, req.min_x, req.min_z, req.max_x, req.max_z)
        .await
        .map_err(|e| ApiError::Internal(e))?;

    Ok(Json(serde_json::json!({
        "ok": true,
        "message": msg
    })))
}

/// POST /api/instances/{id}/world-map/session/stop
pub async fn stop_world_map_session(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    state.preview_sessions.stop_session(&id).await;
    Ok(Json(serde_json::json!({
        "ok": true,
        "message": format!("Warm session stopped for instance {}", id)
    })))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResetWorldSeedRequest {
    pub seed: String,
    #[serde(default)]
    pub reboot: bool,
}

/// POST /api/instances/{id}/world-map/reset-world-seed
pub async fn reset_world_seed(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(req): Json<ResetWorldSeedRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    state.instances.get_instance(&id)?;
    let instance_dir = state.instances.get_instance_dir(&id);
    let server_dir = instance_dir.join("server");
    let src_root = if server_dir.exists() {
        server_dir
    } else {
        instance_dir.clone()
    };

    // 1. If running, stop the instance first
    let was_running = if let Some(pm) = state.instances.get_process_manager(&id) {
        if pm.is_running() {
            state.instances.stop_instance(&id).await;
            tokio::time::sleep(tokio::time::Duration::from_millis(1500)).await;
            true
        } else {
            false
        }
    } else {
        false
    };

    // 2. Wipe world directories
    let candidates = [
        src_root.join("world"),
        instance_dir.join("world"),
    ];
    for dir in &candidates {
        if dir.exists() {
            tracing::info!("[WorldMap Reset] Removing world directory {:?}", dir);
            let _ = std::fs::remove_dir_all(dir);
        }
    }

    // 3. Clear tile & seed caches
    let cache_dir = instance_dir.join(".cache").join("world-tiles");
    if cache_dir.exists() {
        let _ = std::fs::remove_dir_all(&cache_dir);
    }
    let seed_cache_dir = instance_dir.join(".cache").join("world-seeds");
    if seed_cache_dir.exists() {
        let _ = std::fs::remove_dir_all(&seed_cache_dir);
    }

    // 4. Update level-seed in server.properties
    let props_path = src_root.join("server.properties");
    let clean_seed = req.seed.trim();
    if props_path.exists() {
        if let Ok(content) = std::fs::read_to_string(&props_path) {
            let mut new_lines = Vec::new();
            let mut found = false;
            for line in content.lines() {
                if line.starts_with("level-seed=") {
                    new_lines.push(format!("level-seed={clean_seed}"));
                    found = true;
                } else {
                    new_lines.push(line.to_string());
                }
            }
            if !found {
                new_lines.push(format!("level-seed={clean_seed}"));
            }
            let _ = std::fs::write(&props_path, new_lines.join("\n") + "\n");
        }
    }

    // 5. If reboot requested or was running, restart the server with new world
    if req.reboot || was_running {
        let instances = state.instances.clone();
        let id_clone = id.clone();
        tokio::spawn(async move {
            tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
            let _ = instances.start_instance(&id_clone).await;
        });
    }

    Ok(Json(serde_json::json!({
        "ok": true,
        "seed": clean_seed,
        "rebooting": req.reboot || was_running,
        "message": format!("World wiped and reset with seed '{clean_seed}'.")
    })))
}

