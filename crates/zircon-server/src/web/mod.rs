//! Axum admin web API: REST controllers, JWT auth middleware, WebSocket
//! console streaming and the bundled SPA.

pub mod app;
pub mod auth;
pub mod config_routes;
pub mod controllers;
pub mod views;

pub use app::{router, AppState};
