//! Zircon shared core domain.
//!
//! Pure-Rust port of the Java `shared-core` Gradle module. Contains the data
//! models shared between the server manager and the client launcher, hash
//! utilities (SHA-1/SHA-256 and CurseForge MurmurHash3 fingerprints), LZ4+TAR
//! archive helpers, mod JAR metadata extraction, Modrinth/CurseForge API
//! clients and the SSRF-safe CDN URL validator.

pub mod api;
pub mod archive;
pub mod crypto;
pub mod metadata;
pub mod model;
pub mod security;

/// Current BOM JSON schema version. Bump when breaking field changes are made.
pub const CURRENT_SCHEMA_VERSION: i32 = 1;
