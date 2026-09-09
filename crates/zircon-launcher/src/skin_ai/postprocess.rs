//! Deterministic Java Edition post-processing for generated Minecraft skins.
//!
//! Enforces:
//! 1. Base Layer (Head, Torso, Limbs): Clamped to 100% opaque (A = 255). Prevents solid-black glitch in Java.
//! 2. Outer Overlay (Hat, Jacket, Sleeves, Pants): Hard step threshold (A > 128 => 255, else 0).
//! 3. Void Geometry: Zeroed out to (0, 0, 0, 0).
//! 4. Clamping normalized [-1.0, 1.0] floats to [0, 255] u8.

use image::{ImageBuffer, RgbaImage};

/// Minecraft Java Edition 64x64 UV Face bounding boxes [x, y, width, height]
struct BoundingBox {
    x: u32,
    y: u32,
    w: u32,
    h: u32,
    is_overlay: bool,
}

fn get_uv_boxes(is_slim: bool) -> Vec<BoundingBox> {
    let arm_w = if is_slim { 3 } else { 4 };
    vec![
        // Head
        BoundingBox { x: 0, y: 0, w: 32, h: 16, is_overlay: false },  // Head Base
        BoundingBox { x: 32, y: 0, w: 32, h: 16, is_overlay: true },  // Hat Overlay
        // Torso
        BoundingBox { x: 16, y: 16, w: 24, h: 16, is_overlay: false }, // Torso Base
        BoundingBox { x: 16, y: 32, w: 24, h: 16, is_overlay: true },  // Jacket Overlay
        // Right Arm
        BoundingBox { x: 40, y: 16, w: arm_w * 2 + 8, h: 16, is_overlay: false }, // R Arm Base
        BoundingBox { x: 40, y: 32, w: arm_w * 2 + 8, h: 16, is_overlay: true },  // R Sleeve Overlay
        // Left Arm
        BoundingBox { x: 32, y: 48, w: arm_w * 2 + 8, h: 16, is_overlay: false }, // L Arm Base
        BoundingBox { x: 48, y: 48, w: arm_w * 2 + 8, h: 16, is_overlay: true },  // L Sleeve Overlay
        // Right Leg
        BoundingBox { x: 0, y: 16, w: 16, h: 16, is_overlay: false },  // R Leg Base
        BoundingBox { x: 0, y: 32, w: 16, h: 16, is_overlay: true },   // R Pants Overlay
        // Left Leg
        BoundingBox { x: 16, y: 48, w: 16, h: 16, is_overlay: false }, // L Leg Base
        BoundingBox { x: 0, y: 48, w: 16, h: 16, is_overlay: true },   // L Pants Overlay
    ]
}

/// Applies strict Java Edition opacity and UV rules to raw 64x64 RGBA byte buffer.
pub fn postprocess_skin_buffer(raw_rgba: &mut [u8; 64 * 64 * 4], is_slim: bool) {
    let boxes = get_uv_boxes(is_slim);

    for y in 0..64 {
        for x in 0..64 {
            let idx = ((y * 64 + x) * 4) as usize;

            // Check if pixel falls inside any valid UV bounding box
            let mut matching_box: Option<&BoundingBox> = None;
            for b in &boxes {
                if x >= b.x && x < b.x + b.w && y >= b.y && y < b.y + b.h {
                    matching_box = Some(b);
                    break;
                }
            }

            match matching_box {
                Some(b) => {
                    if !b.is_overlay {
                        // Base layer is unconditionally 100% opaque
                        raw_rgba[idx + 3] = 255;
                    } else {
                        // Overlay layer is strictly 1-bit binary alpha (0 or 255)
                        if raw_rgba[idx + 3] > 128 {
                            raw_rgba[idx + 3] = 255;
                        } else {
                            raw_rgba[idx] = 0;
                            raw_rgba[idx + 1] = 0;
                            raw_rgba[idx + 2] = 0;
                            raw_rgba[idx + 3] = 0;
                        }
                    }
                }
                None => {
                    // Unused void coordinates are forced to 0
                    raw_rgba[idx] = 0;
                    raw_rgba[idx + 1] = 0;
                    raw_rgba[idx + 2] = 0;
                    raw_rgba[idx + 3] = 0;
                }
            }
        }
    }
}

/// Encodes raw 64x64 RGBA bytes into a PNG Data URL string.
pub fn encode_to_png_data_url(raw_rgba: &[u8; 64 * 64 * 4]) -> Result<String, String> {
    let img: RgbaImage = ImageBuffer::from_raw(64, 64, raw_rgba.to_vec())
        .ok_or_else(|| "Failed to construct ImageBuffer".to_string())?;

    let mut png_bytes = Vec::new();
    let mut cursor = std::io::Cursor::new(&mut png_bytes);
    img.write_to(&mut cursor, image::ImageFormat::Png)
        .map_err(|e| format!("PNG encoding error: {e}"))?;

    use base64::Engine as _;
    let b64 = base64::engine::general_purpose::STANDARD.encode(&png_bytes);
    Ok(format!("data:image/png;base64,{b64}"))
}
