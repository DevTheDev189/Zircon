//! Zircon Launcher AI Skin Studio Engine.
//!
//! Submodules:
//! - `download`: On-demand model asset streaming, SHA-256 verification, and lifecycle.
//! - `engine`: Euler ODE Flow Matching batch inference.
//! - `postprocess`: Deterministic Java Edition alpha layer and UV boundary clamp.

pub mod download;
pub mod engine;
pub mod postprocess;

pub use download::{DownloadProgressPayload, ModelDownloadManager, SkinAiStatus};
pub use engine::{GeneratedVariant, GenerationRequest, GenerationResponse, SkinAiEngine};
pub use postprocess::{encode_to_png_data_url, postprocess_skin_buffer};
