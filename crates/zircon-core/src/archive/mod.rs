//! Archive utilities and decompression guardrails.

pub mod limits;
pub mod lz4_tar;
pub mod zip;

pub use limits::{
    max_compression_ratio, max_file_entries, max_recursion_depth, max_uncompressed_bytes,
    ArchiveError, ArchiveGuard, ArchiveLimits, DEFAULT_MAX_COMPRESSION_RATIO,
    DEFAULT_MAX_FILE_ENTRIES, DEFAULT_MAX_RECURSION_DEPTH, DEFAULT_MAX_UNCOMPRESSED_BYTES,
    ENV_ZIP_MAX_COMPRESSION_RATIO, ENV_ZIP_MAX_FILE_ENTRIES, ENV_ZIP_MAX_RECURSION_DEPTH,
    ENV_ZIP_MAX_UNCOMPRESSED_BYTES,
};
pub use zip::{extract_zip, extract_zip_with_options, validate_zip_structure, ZipExtractOptions};
