//! Tauri v2 IPC command bindings exposed to the Vue 3 webview.
//!
//! Refactored into domain-isolated modules for high compile speed and maintainability.

pub mod state;
pub mod auth;
pub mod servers;
pub mod launch;
pub mod offline;
pub mod worlds;
pub mod coop;
pub mod skins;
pub mod packs;
pub mod sync;
pub mod modpack;
pub mod system;

pub use state::*;
pub use auth::*;
pub use servers::*;
pub use launch::*;
pub use offline::*;
pub use worlds::*;
pub use coop::*;
pub use skins::*;
pub use packs::*;
pub use sync::*;
pub use modpack::*;
pub use system::*;

#[cfg(test)]
mod tests;
