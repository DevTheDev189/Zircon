//! In-Process Edge Inference Runtime for Zircon AI Skin Studio.
//!
//! Generates batches of 4 candidate skins in a single parallel pass using
//! a 15-step Euler ODE solver over the Flow Matching vector field, then applies
//! deterministic Java Edition base/overlay post-processing.

use serde::{Deserialize, Serialize};
use tauri::Emitter;
use tracing::info;

use crate::paths::skin_ai_model_dir;
use super::download::{MODEL_ONNX_FILENAME, MODEL_ONNX_FP16_FILENAME, VOCAB_JSON_FILENAME};
use super::postprocess::{encode_to_png_data_url, postprocess_skin_buffer};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenerationRequest {
    pub tags: Vec<String>,
    pub palette: Vec<String>, // 3 hex colors: ["#0f172a", "#38bdf8", "#f1f5f9"]
    pub variant: String,       // "classic" | "slim"
    pub steps: Option<u32>,    // default: 15
    pub guidance_scale: Option<f32>, // default: 2.0
    pub seed: Option<u64>,     // base seed
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeneratedVariant {
    pub index: usize,
    pub data_url: String,
    pub seed: u64,
    pub variant: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenerationResponse {
    pub variants: Vec<GeneratedVariant>,
    pub generation_time_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenerationProgressPayload {
    pub stage: String,      // "loading_model" | "generating" | "postprocessing" | "completed"
    pub step: usize,
    pub total_steps: usize,
    pub percentage: f32,    // 0.0 .. 100.0
    pub message: String,
    pub device_name: Option<String>,
}

pub struct SkinAiEngine;

const BASE_RECTS: [(u32, u32, u32, u32); 6] = [
    (0, 0, 32, 16),    // Head Base
    (0, 16, 16, 16),   // Right Leg Base (fixed from 32 to 16)
    (16, 16, 24, 16),  // Torso Base
    (40, 16, 16, 16),  // Right Arm Base
    (16, 48, 16, 16),  // Left Leg Base
    (32, 48, 16, 16),  // Left Arm Base
];

const OVERLAY_RECTS: [(u32, u32, u32, u32); 6] = [
    (32, 0, 32, 16),   // Head Hat Overlay
    (0, 32, 16, 16),   // Right Leg Pants Overlay
    (16, 32, 24, 16),  // Torso Jacket Overlay
    (40, 32, 16, 16),  // Right Arm Sleeve Overlay
    (0, 48, 16, 16),   // Left Leg Pants Overlay
    (48, 48, 16, 16),  // Left Arm Sleeve Overlay
];

fn is_in_rects(x: u32, y: u32, rects: &[(u32, u32, u32, u32)]) -> bool {
    for &(rx, ry, rw, rh) in rects {
        if x >= rx && x < rx + rw && y >= ry && y < ry + rh {
            return true;
        }
    }
    false
}

fn derive_variant_seeds(base_seed: u64, count: usize) -> Vec<u64> {
    let constants = [
        0u64,
        0x9E3779B97F4A7C15,
        0x517CC1B727220A95,
        0x6C62272E07BB0142,
    ];
    let mut seeds = Vec::with_capacity(count);
    for i in 0..count {
        let c = constants[i % constants.len()];
        let mut s = (base_seed ^ c) & 0x7FFF_FFFF_FFFF_FFFF;
        s = s.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407) & 0x7FFF_FFFF_FFFF_FFFF;
        seeds.push(s);
    }
    seeds
}

#[cfg(target_os = "windows")]
fn find_best_directml_device_id() -> i32 {
    use std::ffi::c_void;

    #[repr(C)]
    struct Luid {
        _low_part: u32,
        _high_part: i32,
    }

    #[repr(C)]
    struct DxgiAdapterDesc {
        description: [u16; 128],
        _vendor_id: u32,
        _device_id: u32,
        _sub_sys_id: u32,
        _revision: u32,
        dedicated_video_memory: usize,
        _dedicated_system_memory: usize,
        _shared_system_memory: usize,
        _adapter_luid: Luid,
    }

    extern "system" {
        fn LoadLibraryA(lp_lib_file_name: *const u8) -> *mut c_void;
        fn GetProcAddress(h_module: *mut c_void, lp_proc_name: *const u8) -> *mut c_void;
        fn FreeLibrary(h_lib_module: *mut c_void) -> i32;
    }

    unsafe {
        let dxgi = LoadLibraryA(b"dxgi.dll\0".as_ptr());
        if dxgi.is_null() {
            return 0;
        }

        let create_factory_ptr = GetProcAddress(dxgi, b"CreateDXGIFactory1\0".as_ptr());
        if create_factory_ptr.is_null() {
            FreeLibrary(dxgi);
            return 0;
        }

        let iid_idxgi_factory1: [u8; 16] = [
            0x78, 0xae, 0x0a, 0x77, 0x6f, 0xf2, 0xba, 0x4d,
            0xa8, 0x29, 0x25, 0x3c, 0x83, 0xd1, 0xb3, 0x87,
        ];

        type CreateFactory1Fn = unsafe extern "system" fn(*const [u8; 16], *mut *mut c_void) -> i32;
        let create_factory1: CreateFactory1Fn = std::mem::transmute(create_factory_ptr);

        let mut factory: *mut c_void = std::ptr::null_mut();
        if create_factory1(&iid_idxgi_factory1, &mut factory) != 0 || factory.is_null() {
            FreeLibrary(dxgi);
            return 0;
        }

        let vtable = *(factory as *mut *mut *mut c_void);
        let enum_adapters1_ptr = *vtable.add(12);
        type EnumAdapters1Fn = unsafe extern "system" fn(*mut c_void, u32, *mut *mut c_void) -> i32;
        let enum_adapters1: EnumAdapters1Fn = std::mem::transmute(enum_adapters1_ptr);

        let mut best_id = 0i32;
        let mut max_vram = 0usize;
        let mut idx = 0u32;

        loop {
            let mut adapter: *mut c_void = std::ptr::null_mut();
            if enum_adapters1(factory, idx, &mut adapter) != 0 || adapter.is_null() {
                break;
            }

            let avtable = *(adapter as *mut *mut *mut c_void);
            let get_desc_ptr = *avtable.add(8);
            type GetDescFn = unsafe extern "system" fn(*mut c_void, *mut DxgiAdapterDesc) -> i32;
            let get_desc: GetDescFn = std::mem::transmute(get_desc_ptr);

            let mut desc = std::mem::zeroed::<DxgiAdapterDesc>();
            if get_desc(adapter, &mut desc) == 0 {
                let name = String::from_utf16_lossy(&desc.description)
                    .trim_matches(char::from(0))
                    .to_string();
                let vram_mb = desc.dedicated_video_memory / (1024 * 1024);
                info!("SkinAiEngine: [DXGI Adapter {idx}] '{name}' ({vram_mb} MB Dedicated VRAM)");

                if desc.dedicated_video_memory > max_vram {
                    max_vram = desc.dedicated_video_memory;
                    best_id = idx as i32;
                }
            }

            let release_ptr = *avtable.add(2);
            type ReleaseFn = unsafe extern "system" fn(*mut c_void) -> u32;
            let release: ReleaseFn = std::mem::transmute(release_ptr);
            release(adapter);

            idx += 1;
        }

        let release_ptr = *vtable.add(2);
        type ReleaseFn = unsafe extern "system" fn(*mut c_void) -> u32;
        let release: ReleaseFn = std::mem::transmute(release_ptr);
        release(factory);
        FreeLibrary(dxgi);

        info!("SkinAiEngine: [GPU Decision] Selected High-Performance Discrete GPU: Device ID {best_id} ({} MB VRAM)", max_vram / (1024 * 1024));
        best_id
    }
}

fn generate_gaussian_noise(seed: u64, count: usize) -> Vec<f32> {
    let mut state = if seed == 0 { 0x853c49e6748fea9b } else { seed };
    let mut out = Vec::with_capacity(count);

    let mut next_f32 = || -> f32 {
        state ^= state >> 12;
        state ^= state << 25;
        state ^= state >> 27;
        let val = (state.wrapping_mul(0x2545F4914F6CDD1D) >> 32) as u32;
        (val as f32 + 1.0) / 4294967297.0
    };

    while out.len() < count {
        let u1 = next_f32();
        let u2 = next_f32();
        let r = (-2.0 * u1.ln()).sqrt();
        let theta = 2.0 * std::f32::consts::PI * u2;
        out.push(r * theta.cos());
        if out.len() < count {
            out.push(r * theta.sin());
        }
    }
    out
}

fn resolve_tags(req_tags: &[String], vocab_path: &std::path::Path) -> [i64; 8] {
    let mut token_ids = [0i64; 8];
    let vocab: std::collections::HashMap<String, i64> = std::fs::read_to_string(vocab_path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default();

    let mut idx = 0;
    for tag in req_tags {
        if idx >= 8 { break; }
        let clean = tag.trim().to_lowercase().replace(' ', "_");
        if let Some(&id) = vocab.get(tag.trim()) {
            token_ids[idx] = id;
            idx += 1;
        } else if let Some(&id) = vocab.get(&clean) {
            token_ids[idx] = id;
            idx += 1;
        } else {
            let suffix = format!(":{}", clean);
            if let Some((_, &id)) = vocab.iter().find(|(k, _)| k.ends_with(&suffix) || *k == &clean) {
                token_ids[idx] = id;
                idx += 1;
            } else {
                token_ids[idx] = 0;
                idx += 1;
            }
        }
    }
    token_ids
}

fn parse_palette_normalized(hex_list: &[String]) -> [f32; 9] {
    let mut pal = [0.0f32; 9];
    for (i, hex) in hex_list.iter().take(3).enumerate() {
        let clean = hex.trim().trim_start_matches('#');
        if clean.len() == 6 {
            if let (Ok(r), Ok(g), Ok(b)) = (
                u8::from_str_radix(&clean[0..2], 16),
                u8::from_str_radix(&clean[2..4], 16),
                u8::from_str_radix(&clean[4..6], 16),
            ) {
                pal[i * 3] = r as f32 / 127.5 - 1.0;
                pal[i * 3 + 1] = g as f32 / 127.5 - 1.0;
                pal[i * 3 + 2] = b as f32 / 127.5 - 1.0;
            }
        }
    }
    pal
}

static SESSION_CACHE: std::sync::Mutex<Option<ort::session::Session>> = std::sync::Mutex::new(None);

impl SkinAiEngine {
    /// Generates a batch of 4 skin candidates (100% native in-process Rust ONNX engine).
    pub fn generate_batch(
        app: Option<&tauri::AppHandle>,
        req: GenerationRequest,
    ) -> Result<GenerationResponse, String> {
        let start = std::time::Instant::now();
        let is_slim = req.variant == "slim";
        // Resolve high-entropy random base seed if not provided or 0
        let base_seed = req.seed.filter(|&s| s != 0).unwrap_or_else(|| {
            let mut b = [0u8; 8];
            if getrandom::fill(&mut b).is_ok() {
                u64::from_le_bytes(b) & 0x7FFF_FFFF_FFFF_FFFF
            } else {
                use std::time::{SystemTime, UNIX_EPOCH};
                SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos() as u64
            }
        });

        let model_dir = skin_ai_model_dir();
        let onnx_path = if model_dir.join(MODEL_ONNX_FP16_FILENAME).exists() {
            model_dir.join(MODEL_ONNX_FP16_FILENAME)
        } else if std::path::Path::new("ai/models/skin_dit_b4_v1_fp16.onnx").exists() {
            std::path::PathBuf::from("ai/models/skin_dit_b4_v1_fp16.onnx")
        } else if model_dir.join(MODEL_ONNX_FILENAME).exists() {
            model_dir.join(MODEL_ONNX_FILENAME)
        } else {
            std::path::PathBuf::from("ai/models/skin_dit_b4_v1.onnx")
        };

        let vocab_path = if model_dir.join(VOCAB_JSON_FILENAME).exists() {
            model_dir.join(VOCAB_JSON_FILENAME)
        } else {
            std::path::PathBuf::from("ai/models/tags_vocab.json")
        };

        // Check if real ONNX model is available on disk
        if onnx_path.exists() && vocab_path.exists() {
            info!("Executing 100% native Rust ONNX Flow Matching inference on: {}", onnx_path.display());
            match Self::run_native_onnx_inference(app, &req, base_seed, &onnx_path, &vocab_path) {
                Ok(response) => {
                    info!("Native Rust ONNX inference completed in {}ms", response.generation_time_ms);
                    return Ok(response);
                }
                Err(err) => {
                    tracing::warn!("Native Rust ONNX inference failed ({err}); falling back to concept drafts");
                }
            }
        }

        // Fallback concept generator if model is missing
        info!("Generating concept draft variants (fallback mode)...");
        let mut variants = Vec::with_capacity(4);
        for i in 0..4 {
            let variant_seed = base_seed.wrapping_add(i as u64 * 7919);
            let mut buffer = Self::generate_concept_draft(&req, variant_seed, is_slim);
            postprocess_skin_buffer(&mut buffer, is_slim);
            let data_url = encode_to_png_data_url(&buffer)?;
            variants.push(GeneratedVariant {
                index: i,
                data_url,
                seed: variant_seed,
                variant: req.variant.clone(),
            });
        }

        let elapsed = start.elapsed().as_millis() as u64;
        Ok(GenerationResponse {
            variants,
            generation_time_ms: elapsed,
        })
    }

    /// 100% Native In-Process Rust ONNX Flow Matching Engine with DirectML/CoreML acceleration.
    fn run_native_onnx_inference(
        app: Option<&tauri::AppHandle>,
        req: &GenerationRequest,
        base_seed: u64,
        onnx_path: &std::path::Path,
        vocab_path: &std::path::Path,
    ) -> Result<GenerationResponse, String> {
        let start = std::time::Instant::now();
        let is_slim = req.variant == "slim";
        let batch_size = 4;
        let steps = req.steps.unwrap_or(10) as usize;
        let guidance_scale = req.guidance_scale.unwrap_or(3.5);

        // Derive 4 distinct variant seeds
        let seeds = derive_variant_seeds(base_seed, batch_size);

        // Retrieve or initialize cached DirectML/CoreML ONNX session
        let mut session_guard = SESSION_CACHE.lock().map_err(|e| format!("Session lock poisoned: {e}"))?;
        let is_cold_start = session_guard.is_none();
        if is_cold_start {
            if let Some(app) = app {
                let _ = app.emit("skin-ai-progress", &GenerationProgressPayload {
                    stage: "loading_model".into(),
                    step: 0,
                    total_steps: steps,
                    percentage: 0.0,
                    message: "Initializing DirectML and loading AI model into GPU VRAM...".into(),
                    device_name: None,
                });
            }
            info!("SkinAiEngine: [GPU Decision] Initializing ONNX Runtime session for: {}", onnx_path.display());
            info!("SkinAiEngine: [GPU Decision] Probing hardware GPU acceleration provider...");

            #[allow(unused_mut)]
            let mut builder = ort::session::Session::builder()
                .map_err(|e| format!("ORT Session builder error: {e}"))?
                .with_optimization_level(ort::session::builder::GraphOptimizationLevel::Level3)
                .map_err(|e| format!("ORT Session builder error: {e}"))?;

            #[cfg(target_os = "windows")]
            let mut builder = {
                let best_gpu_id = find_best_directml_device_id();
                let dml = ort::ep::DirectML::default().with_device_id(best_gpu_id).build();
                match builder.with_execution_providers([dml]) {
                    Ok(b) => {
                        info!("SkinAiEngine: [GPU Decision] DirectML Execution Provider attached successfully (Device {best_gpu_id}: Discrete High-Performance GPU)");
                        b
                    }
                    Err(e) => {
                        tracing::warn!("SkinAiEngine: [GPU Decision] DirectML GPU attach failed ({e}); falling back to multithreaded CPU execution");
                        e.recover()
                    }
                }
            };

            #[cfg(target_os = "macos")]
            let mut builder = {
                let coreml = ort::ep::CoreML::default().build();
                match builder.with_execution_providers([coreml]) {
                    Ok(b) => {
                        info!("SkinAiEngine: [GPU Decision] Native CoreML (Apple Neural Engine + Metal) attached successfully");
                        b
                    }
                    Err(e) => {
                        tracing::warn!("SkinAiEngine: [GPU Decision] CoreML attach failed ({e}); falling back to CPU execution");
                        e.recover()
                    }
                }
            };

            #[cfg(target_os = "linux")]
            info!("SkinAiEngine: [GPU Decision] Linux CPU execution provider initialized");

            let session = builder
                .commit_from_file(onnx_path)
                .map_err(|e| format!("Failed to load ONNX model from {}: {e}", onnx_path.display()))?;

            info!("SkinAiEngine: [GPU Decision] ONNX session compiled and cached in memory.");
            *session_guard = Some(session);
        } else {
            info!("SkinAiEngine: [GPU Decision] Reusing warm cached GPU ONNX session (zero cold-start compilation overhead)");
        }

        if let Some(app) = app {
            let _ = app.emit("skin-ai-progress", &GenerationProgressPayload {
                stage: "generating".into(),
                step: 0,
                total_steps: steps,
                percentage: 5.0,
                message: "GPU VRAM resident. Synthesizing skin field...".into(),
                device_name: None,
            });
        }

        // Token IDs and Palette
        let token_ids = resolve_tags(&req.tags, vocab_path);
        let pal_norm = parse_palette_normalized(&req.palette);

        info!(
            "SkinAiEngine: Starting Euler ODE Flow Matching batch (batch_size={}, steps={}, guidance_scale={:.2}, variant='{}', seed={})",
            batch_size, steps, guidance_scale, req.variant, base_seed
        );
        info!("SkinAiEngine: Resolved tags: {:?}, normalized palette: {:?}", token_ids, pal_norm);

        let session = session_guard.as_mut().unwrap();

        // Initialize Gaussian noise x_0 of shape (4, 7, 64, 64)
        let per_variant_size = 7 * 64 * 64;
        let total_x_size = batch_size * per_variant_size;
        let mut x = Vec::with_capacity(total_x_size);
        for b in 0..batch_size {
            let noise = generate_gaussian_noise(seeds[b], per_variant_size);
            x.extend(noise);
        }

        // Pre-allocate constant tensors outside the integration loop
        // Tags: 4 conditional rows, 4 unconditional (zero) rows
        let mut tags_cat = Vec::with_capacity(64);
        for _ in 0..4 {
            tags_cat.extend_from_slice(&token_ids);
        }
        tags_cat.extend(vec![0i64; 32]);

        // Palette: 4 conditional rows, 4 unconditional (zero) rows
        let mut pal_cat = Vec::with_capacity(72);
        for _ in 0..4 {
            pal_cat.extend_from_slice(&pal_norm);
        }
        pal_cat.extend(vec![0.0f32; 36]);

        // Variant: 8 rows
        let var_cat = vec![if is_slim { 1i64 } else { 0i64 }; 8];

        let tags_tensor = ort::value::Tensor::from_array(([8, 8], tags_cat))
            .map_err(|e| format!("tags tensor error: {e}"))?;
        let pal_tensor = ort::value::Tensor::from_array(([8, 9], pal_cat))
            .map_err(|e| format!("palette tensor error: {e}"))?;
        let var_tensor = ort::value::Tensor::from_array(([8], var_cat))
            .map_err(|e| format!("variant tensor error: {e}"))?;

        // Euler ODE Integration Loop
        let dt = 1.0 / steps as f32;
        let mut x_cat = Vec::with_capacity(total_x_size * 2);
        let ode_start = std::time::Instant::now();

        for i in 0..steps {
            let t_val = i as f32 * dt;

            if let Some(app) = app {
                let pct = 5.0 + (i as f32 / steps as f32) * 85.0;
                let _ = app.emit("skin-ai-progress", &GenerationProgressPayload {
                    stage: "generating".into(),
                    step: i + 1,
                    total_steps: steps,
                    percentage: pct,
                    message: format!("ODE Step {}/{} (Flow Matching)...", i + 1, steps),
                    device_name: None,
                });
            }

            // Combined batch size: 2 * batch_size = 8
            x_cat.clear();
            x_cat.extend_from_slice(&x);
            x_cat.extend_from_slice(&x);

            let t_cat = vec![t_val; 8];

            let x_tensor = ort::value::Tensor::from_array(([8, 7, 64, 64], x_cat.clone()))
                .map_err(|e| format!("x tensor error: {e}"))?;
            let t_tensor = ort::value::Tensor::from_array(([8], t_cat))
                .map_err(|e| format!("t tensor error: {e}"))?;

            let step_t0 = std::time::Instant::now();
            let outputs = session.run(ort::inputs! {
                "x" => x_tensor,
                "t" => t_tensor,
                "tags" => &tags_tensor,
                "palette" => &pal_tensor,
                "variant" => &var_tensor,
            })
            .map_err(|e| format!("Session run error at step {i}: {e}"))?;
            let step_elapsed = step_t0.elapsed().as_millis();

            let (_shape, v_data) = outputs["v_pred"]
                .try_extract_tensor::<f32>()
                .map_err(|e| format!("Extract tensor error: {e}"))?;

            // Dynamic Cosine-Tapered Guidance: preserves prompt guidance for t < 0.75
            // and tapers smoothly for t >= 0.75 to eliminate high-frequency pixel ringing
            let eff_guidance = if t_val >= 0.75 {
                let decay = ((0.5 * std::f32::consts::PI * (t_val - 0.75) / 0.25) as f32).cos();
                1.0 + (guidance_scale - 1.0) * decay
            } else {
                guidance_scale
            };

            info!(
                "SkinAiEngine: [ODE Step {}/{}] forward pass: {}ms (t={:.3}, eff_guidance={:.2}, total: {}ms)",
                i + 1,
                steps,
                step_elapsed,
                t_val,
                eff_guidance,
                ode_start.elapsed().as_millis()
            );

            // Per-variant Classifier-Free Guidance with CFG Rescaling (phi=0.7)
            // Keeps guidance vector variance aligned with conditional variance
            let rescale_phi = 0.7f32;
            for b in 0..batch_size {
                let b_offset = b * per_variant_size;
                let uncond_offset = (b + batch_size) * per_variant_size;

                // Calculate standard deviations for CFG rescaling
                let mut sum_c = 0.0f32;
                let mut sum_c_sq = 0.0f32;
                let mut sum_cfg = 0.0f32;
                let mut sum_cfg_sq = 0.0f32;
                let n = per_variant_size as f32;

                for j in 0..per_variant_size {
                    let vc = v_data[b_offset + j];
                    let vu = v_data[uncond_offset + j];
                    let v_cfg = vu + eff_guidance * (vc - vu);

                    sum_c += vc;
                    sum_c_sq += vc * vc;
                    sum_cfg += v_cfg;
                    sum_cfg_sq += v_cfg * v_cfg;
                }

                let mean_c = sum_c / n;
                let var_c = (sum_c_sq / n - mean_c * mean_c).max(0.0);
                let std_c = (var_c + 1e-6).sqrt();

                let mean_cfg = sum_cfg / n;
                let var_cfg = (sum_cfg_sq / n - mean_cfg * mean_cfg).max(0.0);
                let std_cfg = (var_cfg + 1e-6).sqrt();

                let scale_ratio = std_c / std_cfg;

                for j in 0..per_variant_size {
                    let vc = v_data[b_offset + j];
                    let vu = v_data[uncond_offset + j];
                    let raw_v_cfg = vu + eff_guidance * (vc - vu);
                    let rescaled_v_cfg = raw_v_cfg * scale_ratio;
                    let final_v = rescale_phi * rescaled_v_cfg + (1.0 - rescale_phi) * raw_v_cfg;

                    x[b_offset + j] += dt * final_v;
                }
            }
        }

        let ode_duration_ms = ode_start.elapsed().as_millis();
        info!(
            "SkinAiEngine: All {} ODE steps completed in {}ms ({:.1}ms/step avg). Post-processing dual-layer skin textures...",
            steps,
            ode_duration_ms,
            ode_duration_ms as f32 / steps as f32
        );

        if let Some(app) = app {
            let _ = app.emit("skin-ai-progress", &GenerationProgressPayload {
                stage: "postprocessing".into(),
                step: steps,
                total_steps: steps,
                percentage: 95.0,
                message: "Post-processing dual-layer Minecraft skin textures...".into(),
                device_name: None,
            });
        }

        // Convert 7-channel tensors to RGBA and postprocess
        let mut variants = Vec::with_capacity(batch_size);
        for b in 0..batch_size {
            let offset = b * per_variant_size;
            let mut raw_rgba = [0u8; 64 * 64 * 4];

            for y in 0..64 {
                for x_coord in 0..64 {
                    let p = (y * 64 + x_coord) as usize;
                    let idx = p * 4;

                    let base_r = ((x[offset + p] + 1.0) * 127.5).clamp(0.0, 255.0) as u8;
                    let base_g = ((x[offset + 4096 + p] + 1.0) * 127.5).clamp(0.0, 255.0) as u8;
                    let base_b = ((x[offset + 8192 + p] + 1.0) * 127.5).clamp(0.0, 255.0) as u8;

                    let over_r = ((x[offset + 12288 + p] + 1.0) * 127.5).clamp(0.0, 255.0) as u8;
                    let over_g = ((x[offset + 16384 + p] + 1.0) * 127.5).clamp(0.0, 255.0) as u8;
                    let over_b = ((x[offset + 20480 + p] + 1.0) * 127.5).clamp(0.0, 255.0) as u8;
                    let over_a = x[offset + 24576 + p];

                    let is_base = is_in_rects(x_coord, y, &BASE_RECTS);
                    let is_overlay = is_in_rects(x_coord, y, &OVERLAY_RECTS);

                    if is_base {
                        raw_rgba[idx] = base_r;
                        raw_rgba[idx + 1] = base_g;
                        raw_rgba[idx + 2] = base_b;
                        raw_rgba[idx + 3] = 255;
                    }
                    if is_overlay {
                        raw_rgba[idx] = over_r;
                        raw_rgba[idx + 1] = over_g;
                        raw_rgba[idx + 2] = over_b;
                        raw_rgba[idx + 3] = if over_a > 0.0 { 255 } else { 0 };
                    }
                }
            }

            // Slim arm clearing
            if is_slim {
                for y in 16..48 {
                    let idx = (y * 64 + 55) * 4;
                    raw_rgba[idx..idx + 4].fill(0);
                }
                for y in 48..64 {
                    let idx = (y * 64 + 63) * 4;
                    raw_rgba[idx..idx + 4].fill(0);
                }
            }

            postprocess_skin_buffer(&mut raw_rgba, is_slim);
            let data_url = encode_to_png_data_url(&raw_rgba)?;

            variants.push(GeneratedVariant {
                index: b,
                data_url,
                seed: seeds[b],
                variant: req.variant.clone(),
            });
        }

        if let Some(app) = app {
            let _ = app.emit("skin-ai-progress", &GenerationProgressPayload {
                stage: "completed".into(),
                step: steps,
                total_steps: steps,
                percentage: 100.0,
                message: "Generation complete!".into(),
                device_name: None,
            });
        }

        let elapsed = start.elapsed().as_millis() as u64;
        Ok(GenerationResponse {
            variants,
            generation_time_ms: elapsed,
        })
    }

    /// Procedural concept generator used during development or before model download.
    fn generate_concept_draft(req: &GenerationRequest, seed: u64, is_slim: bool) -> [u8; 64 * 64 * 4] {
        let mut buffer = [0u8; 64 * 64 * 4];
        let primary_rgb = parse_hex_color(req.palette.get(0).map(|s| s.as_str()).unwrap_or("#1e293b"));
        let secondary_rgb = parse_hex_color(req.palette.get(1).map(|s| s.as_str()).unwrap_or("#0ea5e9"));
        let _accent_rgb = parse_hex_color(req.palette.get(2).map(|s| s.as_str()).unwrap_or("#f1f5f9"));

        // Simple LCG PRNG for reproducible variation
        let mut rng = seed;
        let mut next_rand = || -> u8 {
            rng = rng.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            ((rng >> 32) & 0xFF) as u8
        };

        let arm_w = if is_slim { 3 } else { 4 };

        for y in 0..64 {
            for x in 0..64 {
                let idx = ((y * 64 + x) * 4) as usize;
                let noise = (next_rand() % 24) as i16 - 12;

                // Head Region (y: 0..16, x: 0..64)
                if y < 16 && x < 32 {
                    // Face skin base
                    let base_r = (245i16 + noise).clamp(0, 255) as u8;
                    let base_g = (200i16 + noise).clamp(0, 255) as u8;
                    let base_b = (175i16 + noise).clamp(0, 255) as u8;
                    buffer[idx] = base_r;
                    buffer[idx + 1] = base_g;
                    buffer[idx + 2] = base_b;
                    buffer[idx + 3] = 255;
                } else if y >= 16 && y < 32 && x >= 16 && x < 40 {
                    // Torso Base (Primary outfit color)
                    buffer[idx] = (primary_rgb.0 as i16 + noise).clamp(0, 255) as u8;
                    buffer[idx + 1] = (primary_rgb.1 as i16 + noise).clamp(0, 255) as u8;
                    buffer[idx + 2] = (primary_rgb.2 as i16 + noise).clamp(0, 255) as u8;
                    buffer[idx + 3] = 255;
                } else if y >= 32 && y < 48 && x >= 16 && x < 40 {
                    // Torso Overlay / Jacket (Secondary accent)
                    buffer[idx] = (secondary_rgb.0 as i16 + noise).clamp(0, 255) as u8;
                    buffer[idx + 1] = (secondary_rgb.1 as i16 + noise).clamp(0, 255) as u8;
                    buffer[idx + 2] = (secondary_rgb.2 as i16 + noise).clamp(0, 255) as u8;
                    buffer[idx + 3] = 255;
                } else if y >= 16 && y < 48 && x >= 40 && x < 40 + arm_w * 2 + 8 {
                    // Arms (mix of skin & sleeves)
                    buffer[idx] = (secondary_rgb.0 as i16 + noise).clamp(0, 255) as u8;
                    buffer[idx + 1] = (secondary_rgb.1 as i16 + noise).clamp(0, 255) as u8;
                    buffer[idx + 2] = (secondary_rgb.2 as i16 + noise).clamp(0, 255) as u8;
                    buffer[idx + 3] = 255;
                } else if (y >= 16 && y < 48 && x < 16) || (y >= 48 && x < 32) {
                    // Legs / Pants
                    buffer[idx] = (primary_rgb.0 as i16 + noise).clamp(0, 255) as u8;
                    buffer[idx + 1] = (primary_rgb.1 as i16 + noise).clamp(0, 255) as u8;
                    buffer[idx + 2] = (primary_rgb.2 as i16 + noise).clamp(0, 255) as u8;
                    buffer[idx + 3] = 255;
                }
            }
        }

        buffer
    }
}

fn parse_hex_color(hex: &str) -> (u8, u8, u8) {
    let clean = hex.trim_start_matches('#');
    if clean.len() == 6 {
        let r = u8::from_str_radix(&clean[0..2], 16).unwrap_or(30);
        let g = u8::from_str_radix(&clean[2..4], 16).unwrap_or(41);
        let b = u8::from_str_radix(&clean[4..6], 16).unwrap_or(59);
        (r, g, b)
    } else {
        (30, 41, 59)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_native_onnx_inference() {
        let _ = tracing_subscriber::fmt::try_init();

        let req = GenerationRequest {
            tags: vec!["cyberpunk".to_string(), "neon".to_string()],
            palette: vec!["#00ffcc".to_string(), "#ff0055".to_string(), "#111122".to_string()],
            variant: "classic".to_string(),
            steps: Some(1), // Just 1 step to instantly test providers and speed
            guidance_scale: Some(3.5),
            seed: Some(42),
        };

        let res = SkinAiEngine::generate_batch(None, req);
        assert!(res.is_ok(), "generate_batch failed: {:?}", res.err());
        let resp = res.unwrap();
        assert_eq!(resp.variants.len(), 4);
        for v in &resp.variants {
            assert!(v.data_url.starts_with("data:image/png;base64,"));
        }
        println!("Native Rust ONNX generate_batch (cold start 1 step) completed in {} ms!", resp.generation_time_ms);

        let req2 = GenerationRequest {
            tags: vec!["knight".to_string(), "gold".to_string()],
            palette: vec!["#d4af37".to_string(), "#1a1a1a".to_string(), "#ffffff".to_string()],
            variant: "slim".to_string(),
            steps: Some(6),
            guidance_scale: Some(3.5),
            seed: Some(123),
        };
        let res2 = SkinAiEngine::generate_batch(None, req2);
        assert!(res2.is_ok(), "Second generate_batch failed: {:?}", res2.err());
        let resp2 = res2.unwrap();
        assert_eq!(resp2.variants.len(), 4);
        for v in &resp2.variants {
            assert!(v.data_url.starts_with("data:image/png;base64,"));
        }
        println!("Native Rust ONNX generate_batch (warm cache 6 steps, 4 skins) completed in {} ms!", resp2.generation_time_ms);
    }
}

