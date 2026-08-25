//! Resource limits and guardrails for archive decompression and validation.
//!
//! Provides parameterized limits for uncompressed sizes, compression ratios,
//! entry counts, and recursion depth to defend against decompression bombs (Zip Bombs).

use std::fmt;
use std::io;
use std::path::{Component, Path, PathBuf};
use std::sync::{Arc, Mutex};

/// Default maximum total uncompressed bytes allowed during extraction (10 GB).
pub const DEFAULT_MAX_UNCOMPRESSED_BYTES: u64 = 10 * 1024 * 1024 * 1024; // 10,737,418,240 bytes

/// Default maximum allowed compression ratio (200:1).
///
/// Modpacks often include highly compressible text formats (JSON models,
/// language files, configuration files, XML/TOML, and repeating texture patterns).
/// A 200:1 ratio accommodates legitimate modpack assets while preventing exponential zip-bombs.
pub const DEFAULT_MAX_COMPRESSION_RATIO: u64 = 200;

/// Default maximum allowed file/directory entry count in an archive (50,000).
pub const DEFAULT_MAX_FILE_ENTRIES: usize = 50_000;

/// Default maximum allowed recursion depth for nested archives (e.g., zip-in-a-zip) (3).
pub const DEFAULT_MAX_RECURSION_DEPTH: usize = 3;

/// Default cap on metadata file byte inspection (2 MB).
pub const DEFAULT_MAX_METADATA_BYTES: u64 = 2 * 1024 * 1024;

/// Environment variable for maximum uncompressed bytes.
pub const ENV_ZIP_MAX_UNCOMPRESSED_BYTES: &str = "ZIP_MAX_UNCOMPRESSED_BYTES";

/// Environment variable for maximum compression ratio ceiling.
pub const ENV_ZIP_MAX_COMPRESSION_RATIO: &str = "ZIP_MAX_COMPRESSION_RATIO";

/// Environment variable for maximum archive file entry count.
pub const ENV_ZIP_MAX_FILE_ENTRIES: &str = "ZIP_MAX_FILE_ENTRIES";

/// Environment variable for maximum nested archive recursion depth.
pub const ENV_ZIP_MAX_RECURSION_DEPTH: &str = "ZIP_MAX_RECURSION_DEPTH";

/// Minimum uncompressed size (in bytes) before strictly enforcing the compression ratio limit.
/// This avoids false positives on tiny files (e.g. empty or 10-byte files that compress to 2 bytes).
pub const RATIO_ENFORCEMENT_THRESHOLD_BYTES: u64 = 64 * 1024; // 64 KB

/// Returns the configured maximum uncompressed bytes, reading `ZIP_MAX_UNCOMPRESSED_BYTES`
/// or falling back to [`DEFAULT_MAX_UNCOMPRESSED_BYTES`] (10 GB).
pub fn max_uncompressed_bytes() -> u64 {
    std::env::var(ENV_ZIP_MAX_UNCOMPRESSED_BYTES)
        .ok()
        .and_then(|val| val.trim().parse::<u64>().ok())
        .unwrap_or(DEFAULT_MAX_UNCOMPRESSED_BYTES)
}

/// Returns the configured maximum compression ratio, reading `ZIP_MAX_COMPRESSION_RATIO`
/// or falling back to [`DEFAULT_MAX_COMPRESSION_RATIO`] (200).
pub fn max_compression_ratio() -> u64 {
    std::env::var(ENV_ZIP_MAX_COMPRESSION_RATIO)
        .ok()
        .and_then(|val| val.trim().parse::<u64>().ok())
        .unwrap_or(DEFAULT_MAX_COMPRESSION_RATIO)
}

/// Returns the configured maximum file entries, reading `ZIP_MAX_FILE_ENTRIES`
/// or falling back to [`DEFAULT_MAX_FILE_ENTRIES`] (50,000).
pub fn max_file_entries() -> usize {
    std::env::var(ENV_ZIP_MAX_FILE_ENTRIES)
        .ok()
        .and_then(|val| val.trim().parse::<usize>().ok())
        .unwrap_or(DEFAULT_MAX_FILE_ENTRIES)
}

/// Returns the configured maximum recursion depth, reading `ZIP_MAX_RECURSION_DEPTH`
/// or falling back to [`DEFAULT_MAX_RECURSION_DEPTH`] (3).
pub fn max_recursion_depth() -> usize {
    std::env::var(ENV_ZIP_MAX_RECURSION_DEPTH)
        .ok()
        .and_then(|val| val.trim().parse::<usize>().ok())
        .unwrap_or(DEFAULT_MAX_RECURSION_DEPTH)
}

/// Configuration settings for archive extraction limits.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ArchiveLimits {
    pub max_uncompressed_bytes: u64,
    pub max_compression_ratio: u64,
    pub max_file_entries: usize,
    pub max_recursion_depth: usize,
}

impl Default for ArchiveLimits {
    fn default() -> Self {
        Self {
            max_uncompressed_bytes: DEFAULT_MAX_UNCOMPRESSED_BYTES,
            max_compression_ratio: DEFAULT_MAX_COMPRESSION_RATIO,
            max_file_entries: DEFAULT_MAX_FILE_ENTRIES,
            max_recursion_depth: DEFAULT_MAX_RECURSION_DEPTH,
        }
    }
}

impl ArchiveLimits {
    /// Loads archive limits dynamically from the environment, using defaults for unset values.
    pub fn from_env() -> Self {
        Self {
            max_uncompressed_bytes: max_uncompressed_bytes(),
            max_compression_ratio: max_compression_ratio(),
            max_file_entries: max_file_entries(),
            max_recursion_depth: max_recursion_depth(),
        }
    }

    /// Builder method to override `max_uncompressed_bytes`.
    pub fn with_max_uncompressed_bytes(mut self, bytes: u64) -> Self {
        self.max_uncompressed_bytes = bytes;
        self
    }

    /// Builder method to override `max_compression_ratio`.
    pub fn with_max_compression_ratio(mut self, ratio: u64) -> Self {
        self.max_compression_ratio = ratio;
        self
    }

    /// Builder method to override `max_file_entries`.
    pub fn with_max_file_entries(mut self, entries: usize) -> Self {
        self.max_file_entries = entries;
        self
    }

    /// Builder method to override `max_recursion_depth`.
    pub fn with_max_recursion_depth(mut self, depth: usize) -> Self {
        self.max_recursion_depth = depth;
        self
    }
}

/// Errors raised when an archive violates resource constraints or security checks.
#[derive(Debug)]
pub enum ArchiveError {
    /// Total uncompressed size exceeded the allowed maximum.
    ExceededMaxBytes { limit: u64, actual: u64 },
    /// Compression ratio exceeded the allowed maximum (decompression bomb guard).
    ExceededMaxRatio {
        ratio: u64,
        limit: u64,
        uncompressed: u64,
        compressed: u64,
    },
    /// Total file entry count exceeded the allowed maximum.
    ExceededMaxFiles { limit: usize, actual: usize },
    /// Nested archive recursion depth exceeded the allowed maximum.
    ExceededMaxRecursionDepth { depth: usize, limit: usize },
    /// Zip Slip or directory traversal attempt detected.
    ZipSlip(String),
    /// Symlink or hardlink entry rejected.
    SymlinkOrHardlink(String),
    /// Generic I/O error during extraction.
    Io(io::Error),
    /// Invalid or corrupt archive data.
    InvalidData(String),
}

impl fmt::Display for ArchiveError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ArchiveError::ExceededMaxBytes { limit, actual } => {
                write!(
                    f,
                    "Archive uncompressed size ({actual} bytes) exceeds maximum limit of {limit} bytes"
                )
            }
            ArchiveError::ExceededMaxRatio {
                ratio,
                limit,
                uncompressed,
                compressed,
            } => {
                write!(
                    f,
                    "Implausible compression ratio {ratio}:1 exceeds limit {limit}:1 ({uncompressed} uncompressed vs {compressed} compressed bytes - potential decompression bomb)"
                )
            }
            ArchiveError::ExceededMaxFiles { limit, actual } => {
                write!(
                    f,
                    "Archive file count ({actual}) exceeds maximum limit of {limit} entries"
                )
            }
            ArchiveError::ExceededMaxRecursionDepth { depth, limit } => {
                write!(
                    f,
                    "Nested archive recursion depth {depth} exceeds maximum limit of {limit}"
                )
            }
            ArchiveError::ZipSlip(path) => {
                write!(f, "Zip slip attempt detected: {path}")
            }
            ArchiveError::SymlinkOrHardlink(path) => {
                write!(f, "Refusing to extract symlink/hardlink entry: {path}")
            }
            ArchiveError::Io(err) => write!(f, "Archive I/O error: {err}"),
            ArchiveError::InvalidData(msg) => write!(f, "Corrupt or invalid archive: {msg}"),
        }
    }
}

impl std::error::Error for ArchiveError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            ArchiveError::Io(err) => Some(err),
            _ => None,
        }
    }
}

impl From<io::Error> for ArchiveError {
    fn from(err: io::Error) -> Self {
        ArchiveError::Io(err)
    }
}

impl From<ArchiveError> for io::Error {
    fn from(err: ArchiveError) -> Self {
        match err {
            ArchiveError::Io(io_err) => io_err,
            other => io::Error::new(io::ErrorKind::InvalidData, other.to_string()),
        }
    }
}

/// Shared internal state for tracking cumulative extraction across archive levels.
#[derive(Debug, Default)]
struct SharedState {
    cumulative_uncompressed_bytes: u64,
    cumulative_compressed_bytes: u64,
    cumulative_file_entries: usize,
}

/// A stateful extraction guard that enforces resource limits, tracks cumulative byte counts
/// across streaming reads, and prevents nested archive expansion attacks.
#[derive(Debug, Clone)]
pub struct ArchiveGuard {
    limits: ArchiveLimits,
    current_depth: usize,
    shared: Arc<Mutex<SharedState>>,
}

impl Default for ArchiveGuard {
    fn default() -> Self {
        Self::new(ArchiveLimits::default())
    }
}

impl ArchiveGuard {
    /// Creates a new guard with the specified limits.
    pub fn new(limits: ArchiveLimits) -> Self {
        Self {
            limits,
            current_depth: 0,
            shared: Arc::new(Mutex::new(SharedState::default())),
        }
    }

    /// Creates a new guard loading limits dynamically from environment variables.
    pub fn from_env() -> Self {
        Self::new(ArchiveLimits::from_env())
    }

    /// Returns the active limits for this guard.
    pub fn limits(&self) -> ArchiveLimits {
        self.limits
    }

    /// Returns the current nesting depth.
    pub fn current_depth(&self) -> usize {
        self.current_depth
    }

    /// Returns the cumulative uncompressed bytes extracted so far across all nested archives.
    pub fn cumulative_uncompressed_bytes(&self) -> u64 {
        self.shared
            .lock()
            .unwrap()
            .cumulative_uncompressed_bytes
    }

    /// Returns the cumulative compressed bytes read so far across all nested archives.
    pub fn cumulative_compressed_bytes(&self) -> u64 {
        self.shared
            .lock()
            .unwrap()
            .cumulative_compressed_bytes
    }

    /// Returns the cumulative file entries extracted so far across all nested archives.
    pub fn cumulative_file_entries(&self) -> usize {
        self.shared
            .lock()
            .unwrap()
            .cumulative_file_entries
    }

    /// Dynamic ratio ceiling based on file types commonly found in mod-packs.
    ///
    /// Mod-packs often include text formats (JSON models, language files, configs,
    /// scripts, blockstate data) that compress extremely well.
    /// This raises the ratio tolerance for known text/asset file types while keeping
    /// strict bounds for binary/executable formats.
    pub fn max_ratio_for_entry(&self, entry_name: &str) -> u64 {
        let lower = entry_name.to_lowercase();
        if lower.ends_with(".json")
            || lower.ends_with(".toml")
            || lower.ends_with(".yaml")
            || lower.ends_with(".yml")
            || lower.ends_with(".txt")
            || lower.ends_with(".mcmeta")
            || lower.ends_with(".lang")
            || lower.ends_with(".properties")
            || lower.ends_with(".cfg")
            || lower.ends_with(".xml")
            || lower.ends_with(".html")
            || lower.ends_with(".js")
            || lower.ends_with(".ts")
            || lower.ends_with(".csv")
        {
            self.limits.max_compression_ratio.max(500)
        } else {
            self.limits.max_compression_ratio
        }
    }

    /// Pre-checks an entry header before reading/extracting.
    ///
    /// Validates:
    /// 1. Declared uncompressed size against maximum limit.
    /// 2. Declared compression ratio against maximum ratio (if size > threshold).
    pub fn check_entry_header(
        &self,
        entry_name: &str,
        declared_uncompressed: u64,
        declared_compressed: u64,
    ) -> Result<(), ArchiveError> {
        let state = self.shared.lock().unwrap();
        let projected_total = state
            .cumulative_uncompressed_bytes
            .saturating_add(declared_uncompressed);

        if projected_total > self.limits.max_uncompressed_bytes {
            return Err(ArchiveError::ExceededMaxBytes {
                limit: self.limits.max_uncompressed_bytes,
                actual: projected_total,
            });
        }

        let max_ratio = self.max_ratio_for_entry(entry_name);

        // Validate compression ratio if uncompressed size is substantial
        if declared_uncompressed >= RATIO_ENFORCEMENT_THRESHOLD_BYTES && declared_compressed > 0 {
            let ratio = declared_uncompressed / declared_compressed;
            if ratio > max_ratio {
                return Err(ArchiveError::ExceededMaxRatio {
                    ratio,
                    limit: max_ratio,
                    uncompressed: declared_uncompressed,
                    compressed: declared_compressed,
                });
            }
        }

        Ok(())
    }

    /// Increments the file count and checks against `max_file_entries`.
    pub fn record_entry(&self) -> Result<(), ArchiveError> {
        let mut state = self.shared.lock().unwrap();
        state.cumulative_file_entries = state.cumulative_file_entries.saturating_add(1);
        if state.cumulative_file_entries > self.limits.max_file_entries {
            return Err(ArchiveError::ExceededMaxFiles {
                limit: self.limits.max_file_entries,
                actual: state.cumulative_file_entries,
            });
        }
        Ok(())
    }

    /// Iteratively tracks streamed uncompressed and compressed bytes chunk-by-chunk.
    ///
    /// Aborts immediately if:
    /// 1. Cumulative uncompressed bytes exceed `max_uncompressed_bytes`.
    /// 2. Real-time compression ratio exceeds `max_compression_ratio` once uncompressed bytes > 64 KB.
    pub fn track_stream_chunk(
        &self,
        uncompressed_chunk: u64,
        compressed_chunk: u64,
    ) -> Result<(), ArchiveError> {
        self.track_stream_chunk_with_ratio(
            uncompressed_chunk,
            compressed_chunk,
            self.limits.max_compression_ratio,
        )
    }

    /// Iteratively tracks streamed uncompressed and compressed bytes chunk-by-chunk with a custom ratio ceiling.
    pub fn track_stream_chunk_with_ratio(
        &self,
        uncompressed_chunk: u64,
        compressed_chunk: u64,
        allowed_ratio: u64,
    ) -> Result<(), ArchiveError> {
        let mut state = self.shared.lock().unwrap();
        state.cumulative_uncompressed_bytes = state
            .cumulative_uncompressed_bytes
            .saturating_add(uncompressed_chunk);
        state.cumulative_compressed_bytes = state
            .cumulative_compressed_bytes
            .saturating_add(compressed_chunk);

        if state.cumulative_uncompressed_bytes > self.limits.max_uncompressed_bytes {
            return Err(ArchiveError::ExceededMaxBytes {
                limit: self.limits.max_uncompressed_bytes,
                actual: state.cumulative_uncompressed_bytes,
            });
        }

        if state.cumulative_uncompressed_bytes >= RATIO_ENFORCEMENT_THRESHOLD_BYTES
            && state.cumulative_compressed_bytes > 0
        {
            let ratio = state.cumulative_uncompressed_bytes / state.cumulative_compressed_bytes;
            if ratio > allowed_ratio {
                return Err(ArchiveError::ExceededMaxRatio {
                    ratio,
                    limit: allowed_ratio,
                    uncompressed: state.cumulative_uncompressed_bytes,
                    compressed: state.cumulative_compressed_bytes,
                });
            }
        }

        Ok(())
    }

    /// Creates a child guard to enter a nested archive (e.g. zip within a zip).
    ///
    /// Shares the cumulative byte and entry counters while incrementing `current_depth`.
    /// Returns an error if `current_depth + 1 > max_recursion_depth`.
    pub fn enter_nested_archive(&self, _nested_name: &str) -> Result<Self, ArchiveError> {
        let next_depth = self.current_depth + 1;
        if next_depth > self.limits.max_recursion_depth {
            return Err(ArchiveError::ExceededMaxRecursionDepth {
                depth: next_depth,
                limit: self.limits.max_recursion_depth,
            });
        }
        Ok(Self {
            limits: self.limits,
            current_depth: next_depth,
            shared: Arc::clone(&self.shared),
        })
    }
}

/// Sanitizes an archive entry path to prevent Zip Slip and directory traversal attacks.
///
/// Rejects:
/// - Absolute paths (e.g. `/etc/passwd` or `C:\Windows`)
/// - Windows drive prefixes (e.g. `C:foo`)
/// - Parent directory traversal (`..`)
///
/// Returns normalized relative [`PathBuf`], or `None` if unsafe.
pub fn sanitize_entry_path(path: &Path) -> Option<PathBuf> {
    let mut result = PathBuf::new();
    for component in path.components() {
        match component {
            Component::Normal(part) => result.push(part),
            Component::CurDir => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => return None,
        }
    }
    if result.as_os_str().is_empty() {
        return None;
    }
    Some(result)
}

/// Exposed helper to check whether an archive entry path is safe.
pub fn is_safe_entry_path(path: &Path) -> bool {
    sanitize_entry_path(path).is_some()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_match_spec() {
        let limits = ArchiveLimits::default();
        assert_eq!(limits.max_uncompressed_bytes, 10 * 1024 * 1024 * 1024);
        assert_eq!(limits.max_compression_ratio, 200);
        assert_eq!(limits.max_file_entries, 50_000);
        assert_eq!(limits.max_recursion_depth, 3);
    }

    #[test]
    fn env_vars_override_defaults() {
        // Test with custom environment variable overrides
        std::env::set_var(ENV_ZIP_MAX_UNCOMPRESSED_BYTES, "5368709120"); // 5 GB
        std::env::set_var(ENV_ZIP_MAX_COMPRESSION_RATIO, "300");
        std::env::set_var(ENV_ZIP_MAX_FILE_ENTRIES, "10000");
        std::env::set_var(ENV_ZIP_MAX_RECURSION_DEPTH, "5");

        let limits = ArchiveLimits::from_env();
        assert_eq!(limits.max_uncompressed_bytes, 5_368_709_120);
        assert_eq!(limits.max_compression_ratio, 300);
        assert_eq!(limits.max_file_entries, 10_000);
        assert_eq!(limits.max_recursion_depth, 5);

        // Cleanup
        std::env::remove_var(ENV_ZIP_MAX_UNCOMPRESSED_BYTES);
        std::env::remove_var(ENV_ZIP_MAX_COMPRESSION_RATIO);
        std::env::remove_var(ENV_ZIP_MAX_FILE_ENTRIES);
        std::env::remove_var(ENV_ZIP_MAX_RECURSION_DEPTH);
    }

    #[test]
    fn path_sanitizer_rules() {
        assert!(is_safe_entry_path(Path::new("mods/example.jar")));
        assert!(is_safe_entry_path(Path::new("assets/textures/block.png")));
        assert!(is_safe_entry_path(Path::new("nested/dir/file.txt")));

        // Malicious paths
        assert!(!is_safe_entry_path(Path::new("../escape.txt")));
        assert!(!is_safe_entry_path(Path::new("mods/../../escape.txt")));
        assert!(!is_safe_entry_path(Path::new("/root/file.txt")));
        assert!(!is_safe_entry_path(Path::new("C:\\Windows\\System32\\calc.exe")));
        assert!(!is_safe_entry_path(Path::new("")));
        assert!(!is_safe_entry_path(Path::new(".")));
    }

    #[test]
    fn guard_tracks_cumulative_and_enforces_limits() {
        let limits = ArchiveLimits::default()
            .with_max_uncompressed_bytes(1000)
            .with_max_file_entries(5);
        let guard = ArchiveGuard::new(limits);

        assert!(guard.record_entry().is_ok());
        assert!(guard.track_stream_chunk(300, 100).is_ok());
        assert_eq!(guard.cumulative_uncompressed_bytes(), 300);
        assert_eq!(guard.cumulative_compressed_bytes(), 100);

        // Exceeding bytes limit
        let err = guard.track_stream_chunk(800, 100).unwrap_err();
        assert!(matches!(err, ArchiveError::ExceededMaxBytes { .. }));
    }

    #[test]
    fn guard_enforces_recursion_depth() {
        let limits = ArchiveLimits::default().with_max_recursion_depth(2);
        let root_guard = ArchiveGuard::new(limits);

        let depth_1 = root_guard.enter_nested_archive("level1.zip").unwrap();
        assert_eq!(depth_1.current_depth(), 1);

        let depth_2 = depth_1.enter_nested_archive("level2.zip").unwrap();
        assert_eq!(depth_2.current_depth(), 2);

        // Depth 3 should fail
        let err = depth_2.enter_nested_archive("level3.zip").unwrap_err();
        assert!(matches!(err, ArchiveError::ExceededMaxRecursionDepth { .. }));
    }
}
