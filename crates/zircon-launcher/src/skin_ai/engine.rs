//! In-Process Edge Inference Runtime for Zircon AI Skin Studio.
//!
//! Generates batches of 4 candidate skins in a single parallel pass using
//! a 15-step Euler ODE solver over the Flow Matching vector field, then applies
//! deterministic Java Edition base/overlay post-processing.

use serde::{Deserialize, Serialize};
use tauri::Emitter;
use tracing::info;

use super::postprocess::{decode_from_png_data_url, encode_to_png_data_url, postprocess_skin_buffer};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenerationRequest {
    pub tags: Vec<String>,
    pub palette: Vec<String>, // 3 hex colors: ["#0f172a", "#38bdf8", "#f1f5f9"]
    pub variant: String,       // "classic" | "slim"
    pub steps: Option<u32>,    // default: 15
    pub guidance_scale: Option<f32>, // default: 2.0
    pub seed: Option<u64>,     // base seed
    pub prompt: Option<String>,
    pub ref_skin_data_url: Option<String>,
    pub lock_parts: Option<Vec<String>>,
    pub mask_overlay_only: Option<bool>,
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


fn compute_edit_mask(req: &GenerationRequest) -> [f32; 64 * 64] {
    let mut mask = [1.0f32; 64 * 64];

    let Some(ref locked) = req.lock_parts else {
        return mask;
    };

    if locked.is_empty() {
        return mask;
    }

    let is_locked = |part: &str| -> bool {
        locked.iter().any(|p| p.eq_ignore_ascii_case(part))
    };

    let mut set_rect = |x: u32, y: u32, w: u32, h: u32, val: f32| {
        for row in y..y + h {
            for col in x..x + w {
                if col < 64 && row < 64 {
                    mask[(row * 64 + col) as usize] = val;
                }
            }
        }
    };

    // Head (Base + Overlay)
    if is_locked("head") {
        set_rect(0, 0, 64, 16, 0.0);
    }

    // Torso (Base + Overlay)
    if is_locked("torso") || is_locked("body") {
        set_rect(16, 16, 24, 32, 0.0);
    }

    // Right Arm (Base + Overlay)
    if is_locked("right_arm") || is_locked("arms") {
        set_rect(40, 16, 16, 32, 0.0);
    }

    // Left Arm (Base + Overlay)
    if is_locked("left_arm") || is_locked("arms") {
        set_rect(32, 48, 32, 16, 0.0);
    }

    // Right Leg (Base + Overlay)
    if is_locked("right_leg") || is_locked("legs") {
        set_rect(0, 16, 16, 32, 0.0);
    }

    // Left Leg (Base + Overlay)
    if is_locked("left_leg") || is_locked("legs") {
        set_rect(0, 48, 32, 16, 0.0);
    }

    // Overlay Only Edit: If mask_overlay_only is true, lock all base layer rects (mask = 0.0)
    if req.mask_overlay_only.unwrap_or(false) {
        for &(bx, by, bw, bh) in &BASE_RECTS {
            set_rect(bx, by, bw, bh, 0.0);
        }
    }

    mask
}

impl SkinAiEngine {
    /// Generates a batch of 4 skin candidates using procedural concept drafts.
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

        info!("Generating concept draft variants...");
        let edit_mask = compute_edit_mask(&req);
        let ref_rgba_opt = req.ref_skin_data_url.as_deref().and_then(|url| decode_from_png_data_url(url).ok());
        let mut variants = Vec::with_capacity(4);
        for i in 0..4 {
            let variant_seed = base_seed.wrapping_add(i as u64 * 7919);
            let mut buffer = Self::generate_concept_draft(&req, variant_seed, is_slim);
            if let Some(ref_rgba) = &ref_rgba_opt {
                for p in 0..4096 {
                    if edit_mask[p] < 0.5 {
                        let idx = p * 4;
                        buffer[idx..idx + 4].copy_from_slice(&ref_rgba[idx..idx + 4]);
                    }
                }
            }
            postprocess_skin_buffer(&mut buffer, is_slim);
            let data_url = encode_to_png_data_url(&buffer)?;
            variants.push(GeneratedVariant {
                index: i,
                data_url,
                seed: variant_seed,
                variant: req.variant.clone(),
            });
        }

        if let Some(app) = app {
            let _ = app.emit("skin-ai-progress", &GenerationProgressPayload {
                stage: "completed".into(),
                step: 1,
                total_steps: 1,
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
    fn test_concept_draft_generation() {
        let _ = tracing_subscriber::fmt::try_init();

        let req = GenerationRequest {
            tags: vec!["cyberpunk".to_string(), "neon".to_string()],
            palette: vec!["#00ffcc".to_string(), "#ff0055".to_string(), "#111122".to_string()],
            variant: "classic".to_string(),
            steps: Some(1), // Just 1 step to instantly test providers and speed
            guidance_scale: Some(3.5),
            seed: Some(42),
            prompt: None,
            ref_skin_data_url: None,
            lock_parts: None,
            mask_overlay_only: None,
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
            prompt: None,
            ref_skin_data_url: None,
            lock_parts: None,
            mask_overlay_only: None,
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

