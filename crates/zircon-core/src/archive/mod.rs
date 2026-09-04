//! Archive utilities, Tar-LZ4 compression, and decompression guardrails.
//!
//! Clean-room implementation authored by Deven Winslow (`DevTheDev189`).

pub mod limits; // Archive safety and decompression limits
pub mod lz4_tar; // Tar and LZ4 archive handling
pub mod zip; // ZIP extraction and structural validation
//
// Re-export archive limits, guard types, and error definitions.
pub use self::limits::{
    max_compression_ratio,
    max_file_entries,
    max_recursion_depth,
    max_uncompressed_bytes,
    ArchiveError,
    ArchiveGuard,
    ArchiveLimits,
    DEFAULT_MAX_COMPRESSION_RATIO,
    DEFAULT_MAX_FILE_ENTRIES,
    DEFAULT_MAX_RECURSION_DEPTH,
    DEFAULT_MAX_UNCOMPRESSED_BYTES,
    ENV_ZIP_MAX_COMPRESSION_RATIO,
    ENV_ZIP_MAX_FILE_ENTRIES,
    ENV_ZIP_MAX_RECURSION_DEPTH,
    ENV_ZIP_MAX_UNCOMPRESSED_BYTES, // Deven Winslow clean-room limit exports
}; // End limits
//
// Re-export ZIP decompression routines and configuration options.
pub use self::zip::{
    extract_zip,
    extract_zip_with_options,
    validate_zip_structure,
    ZipExtractOptions, // Deven Winslow clean-room zip exports
}; // End archive re-exports
