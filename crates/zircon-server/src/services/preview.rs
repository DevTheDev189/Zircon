//! World Seed Preview Service for headless ephemeral generation and map rendering.

use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::Command;

use zircon_core::model::metadata::ModLoaderType;
use zircon_core::world::anvil::scan_region_surface;
use zircon_core::world::mods_filter::scan_terrain_mods;
use zircon_core::world::renderer::render_preview_map;
use zircon_core::world::{PreviewProgress, WorldPreviewRequest, WorldPreviewResult};

pub struct WorldPreviewService;

impl WorldPreviewService {
    /// Generates a preview for a seed within a given instance directory.
    /// In Approach B, chunks persist directly into the instance's real world storage (if clean)
    /// or a persistent seed cache directory (.cache/world-seeds/{seed}), so tiles remain cached.
    pub async fn generate_preview<F>(
        instance_dir: &Path,
        req: WorldPreviewRequest,
        mut progress_cb: F,
    ) -> Result<WorldPreviewResult, String>
    where
        F: FnMut(PreviewProgress) + Send + 'static,
    {
        let server_dir = instance_dir.join("server");
        let src_root = if server_dir.exists() {
            server_dir
        } else {
            instance_dir.to_path_buf()
        };

        // 1. Automatic EULA Enforcement on instance server root
        let eula_path = src_root.join("eula.txt");
        if !eula_path.exists() {
            let _ = fs::write(&eula_path, "eula=true\n");
        } else if let Ok(content) = fs::read_to_string(&eula_path) {
            if !content.contains("eula=true") {
                let _ = fs::write(&eula_path, "eula=true\n");
            }
        }

        // 2. Read instance configuration (Minecraft version & mod loader)
        let config_path = instance_dir.join("instance.json");
        let (mc_version, mod_loader) = if let Ok(content) = fs::read_to_string(&config_path) {
            if let Ok(cfg) = serde_json::from_str::<zircon_core::model::instance::InstanceConfig>(&content) {
                (cfg.minecraft_version, cfg.mod_loader)
            } else {
                ("1.21.1".to_string(), None)
            }
        } else {
            ("1.21.1".to_string(), None)
        };

        // 3. Resolve active seed in server.properties / level.dat
        let server_props = src_root.join("server.properties");
        let mut active_seed = if server_props.exists() {
            fs::read_to_string(&server_props)
                .ok()
                .and_then(|content| {
                    content
                        .lines()
                        .find(|l| l.starts_with("level-seed="))
                        .map(|l| l.trim_start_matches("level-seed=").trim().to_string())
                })
                .unwrap_or_default()
        } else {
            String::new()
        };

        if active_seed.is_empty() {
            for dat_path in &[
                src_root.join("world").join("level.dat"),
                instance_dir.join("world").join("level.dat"),
                instance_dir.join("server").join("world").join("level.dat"),
            ] {
                if dat_path.is_file() {
                    if let Ok(info) = zircon_core::metadata::read_level_dat(dat_path) {
                        if let Some(s) = info.seed {
                            active_seed = s.to_string();
                            break;
                        }
                    }
                }
            }
        }

        let requested_seed = req.seed.trim();
        let use_live = requested_seed.is_empty() || requested_seed == active_seed;

        tracing::info!(
            "[WorldGen] generate_preview called for instance '{:?}' - seed: '{}', active_seed: '{}', center: ({}, {}), radius: {}b",
            instance_dir.file_name().unwrap_or_default(),
            requested_seed,
            active_seed,
            req.center_x,
            req.center_z,
            req.radius_blocks
        );

        // 4. Check if live world already has generated region files
        let live_region_dir_opt = Self::find_region_dir(instance_dir, &src_root);

        if use_live {
            if let Some(ref live_region_dir) = live_region_dir_opt {
                let surface_map = scan_region_surface(live_region_dir, req.center_x, req.center_z, req.radius_blocks);
                tracing::info!(
                    "[WorldGen] Live region scan in '{:?}': found {} chunks",
                    live_region_dir,
                    surface_map.chunks_found
                );
                if surface_map.chunks_found > 0 {
                    progress_cb(PreviewProgress::Rendering {
                        message: format!("Loaded {} generated chunks from active world. Rendering map...", surface_map.chunks_found),
                    });
                    let seed_display = if !active_seed.is_empty() { active_seed } else { req.seed };
                    let result = render_preview_map(&surface_map, &seed_display, req.center_x, req.center_z, req.radius_blocks, req.zoom, 512);
                    progress_cb(PreviewProgress::Ready { result: result.clone() });
                    return Ok(result);
                }
            }
        } else {
            // Check persistent seed cache directory for alternate seed
            let seed_cache_dir = instance_dir.join(".cache").join("world-seeds").join(requested_seed);
            if let Some(cache_region_dir) = Self::find_region_dir(&seed_cache_dir, &seed_cache_dir) {
                let surface_map = scan_region_surface(&cache_region_dir, req.center_x, req.center_z, req.radius_blocks);
                tracing::info!(
                    "[WorldGen] Cache region scan in '{:?}': found {} chunks",
                    cache_region_dir,
                    surface_map.chunks_found
                );
                if surface_map.chunks_found > 0 {
                    progress_cb(PreviewProgress::Rendering {
                        message: format!("Loaded {} cached chunks for seed '{}'. Rendering map...", surface_map.chunks_found, requested_seed),
                    });
                    let result = render_preview_map(&surface_map, requested_seed, req.center_x, req.center_z, req.radius_blocks, req.zoom, 512);
                    progress_cb(PreviewProgress::Ready { result: result.clone() });
                    return Ok(result);
                }
            }
        }

        // 5. Ensure server binaries are installed before booting worker
        let loader_info = mod_loader.clone().unwrap_or_else(|| zircon_core::model::bom::ModLoaderInfo {
            r#type: "vanilla".to_string(),
            version: String::new(),
            loader_jar_url: None,
        });
        let server_jar = src_root.join("server.jar");
        let installer_cache_dir = instance_dir
            .parent()
            .and_then(|p| p.parent())
            .map(|p| p.join(".cache").join("installers"))
            .unwrap_or_else(|| instance_dir.join(".cache").join("installers"));

        if !crate::installer::is_installed(&src_root, &server_jar, &mc_version, &loader_info) {
            progress_cb(PreviewProgress::StartingWorker {
                message: format!("Installing {} server binaries for preview...", loader_info.r#type),
            });
            let _ = fs::create_dir_all(&installer_cache_dir);
            tracing::info!("[WorldGen] Ensuring server installed: loader={}, mc_ver={}", loader_info.r#type, mc_version);
            if let Err(e) = crate::installer::ensure_server_installed(
                &src_root,
                &server_jar,
                &installer_cache_dir,
                &mc_version,
                &loader_info,
            ).await {
                tracing::warn!("Failed to ensure server installed for preview: {e}");
            }
        }

        // Read configured internal port to restore later if modifying live server.properties
        let internal_port: u16 = if let Ok(content) = fs::read_to_string(&config_path) {
            serde_json::from_str::<zircon_core::model::instance::InstanceConfig>(&content)
                .ok()
                .map(|cfg| cfg.internal_mc_port as u16)
                .unwrap_or(25565)
        } else {
            25565
        };

        let worker_port = get_ephemeral_port();

        // 6. Determine target persistent generation directory:
        //    - If clean instance (no region files in live server world), generate directly in src_root
        //    - If instance already has active world and user previews alternate seed, generate in .cache/world-seeds/{seed}
        let is_clean_instance = live_region_dir_opt.is_none();
        let target_dir = if is_clean_instance {
            tracing::info!("[WorldGen] Clean instance detected -> generating directly into live src_root: {:?}", src_root);
            let _ = Self::prepare_server_properties(&src_root.join("server.properties"), requested_seed, worker_port);
            Self::sync_preview_mods(instance_dir, &src_root, &req);
            src_root.clone()
        } else {
            let seed_name = if !requested_seed.is_empty() {
                requested_seed
            } else {
                "default"
            };
            let cache_target = instance_dir.join(".cache").join("world-seeds").join(seed_name);
            tracing::info!("[WorldGen] Existing world detected -> caching alternate seed into: {:?}", cache_target);
            if let Err(e) = Self::setup_persistent_dir(instance_dir, &src_root, &cache_target, &req, worker_port) {
                let err_msg = format!("Failed to configure persistent seed cache: {e}");
                progress_cb(PreviewProgress::Failed { error: err_msg.clone() });
                return Err(err_msg);
            }
            cache_target
        };

        progress_cb(PreviewProgress::StartingWorker {
            message: "Booting headless engine with terrain generator...".to_string(),
        });

        // 7. Spawn headless persistent worker
        tracing::info!("[WorldGen] Spawning headless generation worker on port {}", worker_port);
        let gen_result = Self::run_worker(
            &src_root,
            &target_dir,
            &mc_version,
            &mod_loader,
            &req,
            &mut progress_cb,
        ).await;

        // If clean instance, restore internalMcPort in server.properties for future normal launches
        if is_clean_instance && internal_port > 0 {
            let _ = Self::restore_server_port(&src_root.join("server.properties"), internal_port);
        }

        // Note: In Approach B, we NEVER delete target_dir! The generated region files persist on disk!
        gen_result
    }

    pub fn ensure_server_seed(server_props_path: &Path, seed: &str) -> io::Result<()> {
        Self::prepare_server_properties(server_props_path, seed, 25565)
    }

    pub fn prepare_server_properties(server_props_path: &Path, seed: &str, port: u16) -> io::Result<()> {
        let mut key_values: Vec<(String, String)> = Vec::new();
        if server_props_path.exists() {
            if let Ok(content) = fs::read_to_string(server_props_path) {
                for line in content.lines() {
                    let line = line.trim();
                    if line.is_empty() || line.starts_with('#') {
                        continue;
                    }
                    if let Some((k, v)) = line.split_once('=') {
                        key_values.push((k.trim().to_string(), v.trim().to_string()));
                    }
                }
            }
        }

        let set_prop = |kvs: &mut Vec<(String, String)>, key: &str, val: &str| {
            if let Some(entry) = kvs.iter_mut().find(|(k, _)| k == key) {
                entry.1 = val.to_string();
            } else {
                kvs.push((key.to_string(), val.to_string()));
            }
        };

        if !seed.trim().is_empty() {
            set_prop(&mut key_values, "level-seed", seed.trim());
        }
        set_prop(&mut key_values, "server-port", &port.to_string());
        set_prop(&mut key_values, "server-ip", "127.0.0.1");
        set_prop(&mut key_values, "online-mode", "false");
        set_prop(&mut key_values, "level-name", "world");
        set_prop(&mut key_values, "sync-chunk-writes", "false");
        set_prop(&mut key_values, "pause-when-empty-seconds", "-1");
        set_prop(&mut key_values, "view-distance", "4");
        set_prop(&mut key_values, "simulation-distance", "3");
        set_prop(&mut key_values, "difficulty", "peaceful");
        set_prop(&mut key_values, "spawn-monsters", "false");
        set_prop(&mut key_values, "spawn-animals", "false");
        set_prop(&mut key_values, "spawn-npcs", "false");
        set_prop(&mut key_values, "generate-structures", "true");
        set_prop(&mut key_values, "max-players", "0");
        set_prop(&mut key_values, "enable-query", "false");
        set_prop(&mut key_values, "enable-rcon", "false");
        set_prop(&mut key_values, "enable-status", "false");
        set_prop(&mut key_values, "broadcast-rcon-to-ops", "false");
        set_prop(&mut key_values, "broadcast-console-to-ops", "false");
        set_prop(&mut key_values, "spawn-protection", "0");

        let mut out = String::new();
        for (k, v) in key_values {
            out.push_str(&format!("{k}={v}\n"));
        }
        fs::write(server_props_path, out)
    }

    pub fn restore_server_port(server_props_path: &Path, port: u16) -> io::Result<()> {
        if !server_props_path.exists() {
            return Ok(());
        }
        let content = fs::read_to_string(server_props_path).unwrap_or_default();
        let mut lines: Vec<String> = Vec::new();
        let mut updated = false;
        for line in content.lines() {
            if line.starts_with("server-port=") {
                lines.push(format!("server-port={port}"));
                updated = true;
            } else {
                lines.push(line.to_string());
            }
        }
        if !updated {
            lines.push(format!("server-port={port}"));
        }
        fs::write(server_props_path, lines.join("\n") + "\n")
    }

    pub(crate) fn sync_preview_mods(
        instance_dir: &Path,
        src_root: &Path,
        req: &WorldPreviewRequest,
    ) {
        let live_mods = src_root.join("mods");
        let parent_mods = instance_dir.join("mods");
        let needs_sync = !live_mods.exists() || fs::read_dir(&live_mods).map(|mut d| d.next().is_none()).unwrap_or(true);
        if needs_sync && parent_mods.is_dir() {
            let _ = fs::create_dir_all(&live_mods);
            if let Ok(candidates) = scan_terrain_mods(&parent_mods) {
                for cand in candidates {
                    let should_include = if let Some(selected) = &req.selected_mods {
                        selected.iter().any(|s| s.eq_ignore_ascii_case(&cand.file_name))
                    } else {
                        cand.is_likely_terrain
                    };

                    if should_include {
                        let target_mod = live_mods.join(&cand.file_name);
                        if !target_mod.exists() {
                            let _ = fs::copy(parent_mods.join(&cand.file_name), target_mod);
                        }
                    }
                }
            }
        }
    }

    pub(crate) fn setup_persistent_dir(
        instance_dir: &Path,
        src_root: &Path,
        dest_dir: &Path,
        req: &WorldPreviewRequest,
        worker_port: u16,
    ) -> io::Result<()> {
        fs::create_dir_all(dest_dir)?;

        // Copy server JARs and execution scripts
        for file_name in &[
            "server.jar",
            "fabric-server-launch.jar",
            "quilt-server-launch.jar",
            "user_jvm_args.txt",
            "win_args.txt",
            "unix_args.txt",
            "run.bat",
            "run.sh",
        ] {
            let p = src_root.join(file_name);
            let dest_p = dest_dir.join(file_name);
            if p.exists() && !dest_p.exists() {
                let _ = fs::copy(&p, &dest_p);
            }
        }

        // Copy libraries if present
        let libs = src_root.join("libraries");
        let dest_libs = dest_dir.join("libraries");
        if libs.exists() && !dest_libs.exists() {
            let _ = copy_dir_recursive(&libs, &dest_libs);
        }

        // Setup mods directory with terrain filtering
        let mods_src = if src_root.join("mods").is_dir() && fs::read_dir(src_root.join("mods")).map(|mut d| d.next().is_some()).unwrap_or(false) {
            src_root.join("mods")
        } else {
            instance_dir.join("mods")
        };
        let mods_dest = dest_dir.join("mods");
        fs::create_dir_all(&mods_dest)?;

        if mods_src.exists() {
            if let Ok(candidates) = scan_terrain_mods(&mods_src) {
                for cand in candidates {
                    let should_include = if let Some(selected) = &req.selected_mods {
                        selected.iter().any(|s| s.eq_ignore_ascii_case(&cand.file_name))
                    } else {
                        cand.is_likely_terrain
                    };

                    if should_include {
                        let target_mod = mods_dest.join(&cand.file_name);
                        if !target_mod.exists() {
                            let _ = fs::copy(mods_src.join(&cand.file_name), target_mod);
                        }
                    }
                }
            }
        }

        // Copy datapacks if present
        let dp_src = src_root.join("datapacks");
        let dest_dp = dest_dir.join("datapacks");
        if dp_src.exists() && !dest_dp.exists() {
            let _ = copy_dir_recursive(&dp_src, &dest_dp);
        }

        // Create eula.txt
        let mut eula = fs::File::create(dest_dir.join("eula.txt"))?;
        writeln!(eula, "eula=true")?;

        // Write optimized server.properties with dynamic ephemeral port
        Self::prepare_server_properties(&dest_dir.join("server.properties"), &req.seed, worker_port)
    }

    async fn run_worker<F>(
        src_root: &Path,
        target_dir: &Path,
        mc_version: &str,
        mod_loader: &Option<zircon_core::model::bom::ModLoaderInfo>,
        req: &WorldPreviewRequest,
        progress_cb: &mut F,
    ) -> Result<WorldPreviewResult, String>
    where
        F: FnMut(PreviewProgress),
    {
        // 1. Resolve exact Java binary based on Minecraft version (e.g. Java 21 for 1.20.5+)
        let java_version = crate::process::manager::detect_java_version(mc_version);
        let java_bin = crate::installer::java_bin_for_version(java_version);
        let mut cmd = Command::new(java_bin);
        cmd.current_dir(target_dir);
        cmd.arg("-Xmx1536M");
        cmd.arg("-Xms512M");
        cmd.arg("-Djava.awt.headless=true");
        cmd.arg("-Dmixin.debug.countInjections=false");

        // 2. ModLoader specific command line arguments (Forge, NeoForge, Quilt, Fabric, Vanilla)
        let loader = mod_loader
            .as_ref()
            .and_then(|info| ModLoaderType::from_id(&info.r#type))
            .unwrap_or(ModLoaderType::Vanilla);

        if loader.is_forge_like() {
            let loader_ver = mod_loader.as_ref().map(|l| l.version.as_str()).unwrap_or("");
            let user_jvm = target_dir.join("user_jvm_args.txt");
            if user_jvm.exists() {
                cmd.arg(format!("@{}", user_jvm.to_string_lossy()));
            } else if src_root.join("user_jvm_args.txt").exists() {
                cmd.arg(format!("@{}", src_root.join("user_jvm_args.txt").to_string_lossy()));
            }

            if let Some(args_file) = crate::installer::find_server_args_file(target_dir, loader_ver)
                .or_else(|| crate::installer::find_server_args_file(src_root, loader_ver))
            {
                cmd.arg(format!("@{}", args_file.to_string_lossy()));
            } else {
                let s_jar = target_dir.join("server.jar");
                if s_jar.exists() {
                    cmd.arg("-jar").arg(s_jar);
                }
            }
        } else if loader == ModLoaderType::Quilt && (target_dir.join("quilt-server-launch.jar").exists() || src_root.join("quilt-server-launch.jar").exists()) {
            let jar = if target_dir.join("quilt-server-launch.jar").exists() {
                target_dir.join("quilt-server-launch.jar")
            } else {
                src_root.join("quilt-server-launch.jar")
            };
            cmd.arg("-jar").arg(jar);
        } else if loader == ModLoaderType::Fabric && (target_dir.join("fabric-server-launch.jar").exists() || src_root.join("fabric-server-launch.jar").exists()) {
            let jar = if target_dir.join("fabric-server-launch.jar").exists() {
                target_dir.join("fabric-server-launch.jar")
            } else {
                src_root.join("fabric-server-launch.jar")
            };
            cmd.arg("-jar").arg(jar);
        } else if target_dir.join("server.jar").exists() || src_root.join("server.jar").exists() {
            let jar = if target_dir.join("server.jar").exists() {
                target_dir.join("server.jar")
            } else {
                src_root.join("server.jar")
            };
            cmd.arg("-jar").arg(jar);
        } else if target_dir.join("fabric-server-launch.jar").exists() || src_root.join("fabric-server-launch.jar").exists() {
            let jar = if target_dir.join("fabric-server-launch.jar").exists() {
                target_dir.join("fabric-server-launch.jar")
            } else {
                src_root.join("fabric-server-launch.jar")
            };
            cmd.arg("-jar").arg(jar);
        } else {
            // Fallback: If no server jar present (e.g. client mock / test suite), scan available files
            let region_dir = Self::find_region_dir(target_dir, target_dir)
                .or_else(|| Self::find_region_dir(src_root, src_root))
                .unwrap_or_else(|| target_dir.join("world/region"));
            let surface_map = scan_region_surface(&region_dir, req.center_x, req.center_z, req.radius_blocks);
            let result = render_preview_map(&surface_map, &req.seed, req.center_x, req.center_z, req.radius_blocks, req.zoom, 512);
            progress_cb(PreviewProgress::Ready { result: result.clone() });
            return Ok(result);
        }

        cmd.arg("nogui");
        cmd.stdin(std::process::Stdio::piped());
        cmd.stdout(std::process::Stdio::piped());
        cmd.stderr(std::process::Stdio::piped());

        tracing::info!("[WorldGen] Launching command: {:?}", cmd);
        let mut child = match cmd.spawn() {
            Ok(c) => {
                tracing::info!("[WorldGen] Headless preview worker process spawned (PID: {:?})", c.id());
                c
            },
            Err(e) => {
                tracing::warn!("Failed to spawn headless preview worker: {e}");
                let region_dir = Self::find_region_dir(target_dir, target_dir)
                    .or_else(|| Self::find_region_dir(src_root, src_root))
                    .unwrap_or_else(|| target_dir.join("world/region"));
                let surface_map = scan_region_surface(&region_dir, req.center_x, req.center_z, req.radius_blocks);
                let result = render_preview_map(&surface_map, &req.seed, req.center_x, req.center_z, req.radius_blocks, req.zoom, 512);
                progress_cb(PreviewProgress::Ready { result: result.clone() });
                return Ok(result);
            }
        };

        // Asynchronously drain and log stderr to prevent worker buffer-locking
        if let Some(stderr) = child.stderr.take() {
            tokio::spawn(async move {
                let mut err_reader = BufReader::new(stderr).lines();
                while let Ok(Some(err_line)) = err_reader.next_line().await {
                    tracing::info!("[preview worker stderr] {err_line}");
                }
            });
        }

        let mut stdin = match child.stdin.take() {
            Some(s) => s,
            None => {
                let region_dir = Self::find_region_dir(target_dir, target_dir)
                    .or_else(|| Self::find_region_dir(src_root, src_root))
                    .unwrap_or_else(|| target_dir.join("world/region"));
                let surface_map = scan_region_surface(&region_dir, req.center_x, req.center_z, req.radius_blocks);
                let result = render_preview_map(&surface_map, &req.seed, req.center_x, req.center_z, req.radius_blocks, req.zoom, 512);
                progress_cb(PreviewProgress::Ready { result: result.clone() });
                return Ok(result);
            }
        };

        let stdout = match child.stdout.take() {
            Some(s) => s,
            None => {
                let region_dir = Self::find_region_dir(target_dir, target_dir)
                    .or_else(|| Self::find_region_dir(src_root, src_root))
                    .unwrap_or_else(|| target_dir.join("world/region"));
                let surface_map = scan_region_surface(&region_dir, req.center_x, req.center_z, req.radius_blocks);
                let result = render_preview_map(&surface_map, &req.seed, req.center_x, req.center_z, req.radius_blocks, req.zoom, 512);
                progress_cb(PreviewProgress::Ready { result: result.clone() });
                return Ok(result);
            }
        };

        let mut reader = BufReader::new(stdout).lines();
        let timeout = Instant::now() + Duration::from_secs(120);
        let mut booted = false;
        let mut forceload_done = false;

        let radius_chunks = (req.radius_blocks >> 4).max(8);
        let min_x = req.center_x - req.radius_blocks;
        let min_z = req.center_z - req.radius_blocks;
        let max_x = req.center_x + req.radius_blocks;
        let max_z = req.center_z + req.radius_blocks;

        while Instant::now() < timeout {
            tokio::select! {
                line = reader.next_line() => {
                    match line {
                        Ok(Some(line_str)) => {
                            tracing::info!("[preview worker stdout] {line_str}");
                            if !booted && (line_str.contains("Done (") || line_str.contains("Done!") || line_str.contains("Done in ")) {
                                booted = true;
                                tracing::info!("[WorldGen] Headless server booted successfully! Sending gamerules and forceload add {} {} {} {}", min_x, min_z, max_x, max_z);
                                progress_cb(PreviewProgress::Forceloading {
                                    percent: 40,
                                    message: format!("Forceloading terrain area (+/- {radius_chunks} chunks)..."),
                                });
                                // Disable ticking boilerplate game rules to maximize chunk generation throughput
                                let _ = stdin.write_all(b"gamerule doDaylightCycle false\ngamerule doWeatherCycle false\ngamerule randomTickSpeed 0\ngamerule doMobSpawning false\ngamerule doPatrolSpawning false\ngamerule doTraderSpawning false\ngamerule doEntityDrops false\n").await;
                                let forceload_cmd = format!("forceload add {min_x} {min_z} {max_x} {max_z}\n");
                                let _ = stdin.write_all(forceload_cmd.as_bytes()).await;
                                let _ = stdin.flush().await;
                            } else if booted && !forceload_done && (line_str.contains("Marked chunk") || line_str.contains("Chunks are now forced") || line_str.contains("forceload")) {
                                forceload_done = true;
                                tracing::info!("[WorldGen] Forceload chunks confirmed! Flushing world chunks to disk and stopping worker...");
                                progress_cb(PreviewProgress::Forceloading {
                                    percent: 90,
                                    message: "Flushing chunk data to disk...".to_string(),
                                });
                                // Remove forceloaded marks so the world is not permanently forced for players, flush, and stop
                                let _ = stdin.write_all(b"forceload remove all\nsave-all flush\nstop\n").await;
                                let _ = stdin.flush().await;
                            }
                        }
                        _ => break,
                    }
                }
                _ = tokio::time::sleep(Duration::from_millis(100)) => {
                    if let Ok(Some(status)) = child.try_wait() {
                        tracing::info!("[WorldGen] Headless preview worker exited with status: {:?}", status);
                        break;
                    }
                }
            }
        }

        // Give child process up to 5 seconds to gracefully flush chunks before terminating
        let _ = tokio::time::timeout(Duration::from_secs(5), child.wait()).await;
        let _ = child.kill().await;

        tracing::info!("[WorldGen] Headless preview worker finished. Extracting region surfaces from {:?}", target_dir);
        progress_cb(PreviewProgress::Rendering {
            message: "Extracting region surfaces and rendering top-down map...".to_string(),
        });

        // Determine generated seed from level.dat if requested seed was empty
        let mut final_seed = req.seed.clone();
        if final_seed.trim().is_empty() {
            for dat_path in &[
                target_dir.join("world").join("level.dat"),
                src_root.join("world").join("level.dat"),
            ] {
                if dat_path.is_file() {
                    if let Ok(info) = zircon_core::metadata::read_level_dat(dat_path) {
                        if let Some(s) = info.seed {
                            final_seed = s.to_string();
                            break;
                        }
                    }
                }
            }
        }

        let region_dir = Self::find_region_dir(target_dir, target_dir)
            .or_else(|| Self::find_region_dir(src_root, src_root))
            .unwrap_or_else(|| target_dir.join("world/region"));
        let surface_map = scan_region_surface(&region_dir, req.center_x, req.center_z, req.radius_blocks);
        let result = render_preview_map(&surface_map, &final_seed, req.center_x, req.center_z, req.radius_blocks, req.zoom, 512);

        progress_cb(PreviewProgress::Ready {
            result: result.clone(),
        });

        Ok(result)
    }

    /// Finds the Minecraft overworld region directory across standard, Bukkit, and modern dimension structures.
    pub fn find_region_dir(instance_dir: &Path, src_root: &Path) -> Option<PathBuf> {
        let candidates = [
            src_root.join("world").join("dimensions").join("minecraft").join("overworld").join("region"),
            src_root.join("world").join("region"),
            instance_dir.join("world").join("dimensions").join("minecraft").join("overworld").join("region"),
            instance_dir.join("world").join("region"),
            instance_dir.join("server").join("world").join("dimensions").join("minecraft").join("overworld").join("region"),
            instance_dir.join("server").join("world").join("region"),
        ];

        for cand in &candidates {
            if cand.is_dir() && fs::read_dir(cand).map(|mut d| d.next().is_some()).unwrap_or(false) {
                return Some(cand.clone());
            }
        }

        for base in &[src_root.join("world"), instance_dir.join("world"), instance_dir.join("server/world")] {
            if base.is_dir() {
                if let Some(found) = find_region_recursive(base, 4) {
                    return Some(found);
                }
            }
        }

        None
    }
}

fn find_region_recursive(dir: &Path, max_depth: usize) -> Option<PathBuf> {
    if max_depth == 0 {
        return None;
    }
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                if path.file_name().map(|n| n == "region").unwrap_or(false) {
                    if fs::read_dir(&path).map(|mut d| d.next().is_some()).unwrap_or(false) {
                        return Some(path);
                    }
                }
                if let Some(sub) = find_region_recursive(&path, max_depth - 1) {
                    return Some(sub);
                }
            }
        }
    }
    None
}

fn copy_dir_recursive(src: &Path, dst: &Path) -> io::Result<()> {
    fs::create_dir_all(dst)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let path = entry.path();
        let target = dst.join(entry.file_name());
        if path.is_dir() {
            copy_dir_recursive(&path, &target)?;
        } else {
            fs::copy(&path, &target)?;
        }
    }
    Ok(())
}

pub(crate) fn get_ephemeral_port() -> u16 {
    std::net::TcpListener::bind("127.0.0.1:0")
        .and_then(|l| l.local_addr())
        .map(|a| a.port())
        .unwrap_or(29999)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_ensure_server_seed_creates_file_when_missing() {
        let dir = tempdir().unwrap();
        let props_path = dir.path().join("server.properties");
        assert!(!props_path.exists());

        WorldPreviewService::ensure_server_seed(&props_path, "123456789").unwrap();
        assert!(props_path.exists());

        let content = fs::read_to_string(&props_path).unwrap();
        assert!(content.contains("level-seed=123456789"));
        assert!(content.contains("level-name=world"));
    }

    #[test]
    fn test_ensure_server_seed_updates_existing_seed() {
        let dir = tempdir().unwrap();
        let props_path = dir.path().join("server.properties");
        fs::write(&props_path, "motd=A Minecraft Server\nlevel-seed=old_seed_value\npvp=true\n").unwrap();

        WorldPreviewService::ensure_server_seed(&props_path, "new_seed_999").unwrap();
        let content = fs::read_to_string(&props_path).unwrap();
        assert!(content.contains("level-seed=new_seed_999"));
        assert!(!content.contains("old_seed_value"));
        assert!(content.contains("motd=A Minecraft Server"));
        assert!(content.contains("pvp=true"));
    }

    #[test]
    fn test_setup_persistent_dir_copies_jars_and_eula() {
        let src_dir = tempdir().unwrap();
        let dest_dir = tempdir().unwrap();

        // Create mock server jar and user_jvm_args.txt
        fs::write(src_dir.path().join("server.jar"), b"mock jar").unwrap();
        fs::write(src_dir.path().join("user_jvm_args.txt"), b"-Xmx2G").unwrap();

        let req = WorldPreviewRequest {
            seed: "persisted_seed".to_string(),
            radius_blocks: 256,
            center_x: 0,
            center_z: 0,
            zoom: 1.0,
            selected_mods: None,
        };

        WorldPreviewService::setup_persistent_dir(
            src_dir.path(),
            src_dir.path(),
            dest_dir.path(),
            &req,
            25565,
        ).unwrap();

        assert!(dest_dir.path().join("server.jar").is_file());
        assert!(dest_dir.path().join("user_jvm_args.txt").is_file());
        assert!(dest_dir.path().join("eula.txt").is_file());
        assert_eq!(
            fs::read_to_string(dest_dir.path().join("eula.txt")).unwrap().trim(),
            "eula=true"
        );
        let props = fs::read_to_string(dest_dir.path().join("server.properties")).unwrap();
        assert!(props.contains("level-seed=persisted_seed"));
        assert!(props.contains("server-port=25565"));
        assert!(props.contains("server-ip=127.0.0.1"));
    }

    #[test]
    fn test_find_region_dir_locates_overworld_regions() {
        let dir = tempdir().unwrap();
        let region_path = dir.path().join("world").join("region");
        fs::create_dir_all(&region_path).unwrap();
        fs::write(region_path.join("r.0.0.mca"), b"dummy mca data").unwrap();

        let found = WorldPreviewService::find_region_dir(dir.path(), dir.path());
        assert!(found.is_some());
        assert_eq!(found.unwrap(), region_path);
    }
}
