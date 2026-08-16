//! Shared data models: Bill of Materials, instance config, backups, metadata.

pub mod backup;
pub mod bom;
pub mod instance;
pub mod metadata;

pub use backup::*;
pub use bom::*;
pub use instance::*;
pub use metadata::*;
