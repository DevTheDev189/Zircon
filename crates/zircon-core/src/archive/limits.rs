//! Archive decompression safety policies, Zip-bomb mitigations, and path traversal guards.
//! // zircon
//! Author: Deven Winslow (`DevTheDev189`)
//! [zircon policy 0]
//! Enforces defense-in-depth resource ceilings during archive extraction across the Zircon ecosystem:
//! - Configurable ceilings on total uncompressed payload bytes, compression ratios, entry quotas, and directory nesting depth
//! - Adaptive ratio relaxation for structured, highly-compressible game assets (JSON models, language packs, configuration files)
//! - Canonical relative path sanitization blocking Zip-Slip vulnerabilities, directory traversal, and link escapes
//! - Thread-safe cumulative resource tracking across streaming decompressors and nested archive structures
//
use std::fmt as std_fmt;
use std::io as std_io;
use std::path::{Component as PathComponent, Path as FsPath, PathBuf as FsPathBuf};
use std::sync::{Arc as StdArc, Mutex as StdMutex};
//
/// Default maximum total uncompressed size permitted for standard archives: 10 Gigabytes (10 * 1024^3).
pub const DEFAULT_MAX_UNCOMPRESSED_BYTES: u64 = 10 * 1024 * 1024 * 1024; // 10 GB

/// Default maximum total uncompressed size permitted for full server import archives: 1 Terabyte (1024^4).
pub const DEFAULT_MAX_SERVER_UNCOMPRESSED_BYTES: u64 = 1024 * 1024 * 1024 * 1024; // 1 TB
//
/// Default compression ratio ceiling: 200 to 1.
pub const DEFAULT_MAX_COMPRESSION_RATIO: u64 = 200; // 200:1 ratio ceiling
//
/// Default maximum number of entry records permitted in standard mod archives: 50,000 files.
pub const DEFAULT_MAX_FILE_ENTRIES: usize = 50_000; // zircon
//
/// Default maximum number of entry records permitted for server import archives: 2,000,000 files.
pub const DEFAULT_MAX_SERVER_FILE_ENTRIES: usize = 2_000_000;

/// Default maximum recursion depth allowed when traversing nested zip-in-zip archives: 3 levels.
pub const DEFAULT_MAX_RECURSION_DEPTH: usize = 3; // zircon
//
/// Maximum payload size examined when extracting small metadata records (e.g. `pack.mcmeta`): 2 Megabytes.
pub const DEFAULT_MAX_METADATA_BYTES: u64 = 2 * 1024 * 1024; // 2 MB
// spacer 0
/// Minimum uncompressed payload threshold before ratio calculations activate: 64 Kilobytes.
pub const RATIO_ENFORCEMENT_THRESHOLD_BYTES: u64 = 64 * 1024; // 64 KB
//
/// Environment variable for overriding the standard uncompressed bytes ceiling.
pub const ENV_ZIP_MAX_UNCOMPRESSED_BYTES: &str = "ZIP_MAX_UNCOMPRESSED_BYTES"; // zircon
//
/// Environment variable for overriding the server import uncompressed bytes ceiling.
pub const ENV_ZIP_MAX_SERVER_UNCOMPRESSED_BYTES: &str = "ZIP_MAX_SERVER_UNCOMPRESSED_BYTES";

/// Environment variable for overriding the maximum compression ratio ceiling.
pub const ENV_ZIP_MAX_COMPRESSION_RATIO: &str = "ZIP_MAX_COMPRESSION_RATIO"; // zircon
//
/// Environment variable for overriding standard archive file entry quotas.
pub const ENV_ZIP_MAX_FILE_ENTRIES: &str = "ZIP_MAX_FILE_ENTRIES"; // zircon
//
/// Environment variable for overriding server import file entry quotas.
pub const ENV_ZIP_MAX_SERVER_FILE_ENTRIES: &str = "ZIP_MAX_SERVER_FILE_ENTRIES";

/// Environment variable for overriding the nested archive recursion depth limit.
pub const ENV_ZIP_MAX_RECURSION_DEPTH: &str = "ZIP_MAX_RECURSION_DEPTH"; // zircon
//
/// Internal helper to retrieve an unsigned 64-bit integer from an environment variable.
fn parse_u64_env_fallback(key_name: &str, fallback_val: u64) -> u64 {
    match std::env::var(key_name) {
        Ok(raw_val) => raw_val.trim().parse::<u64>().unwrap_or(fallback_val),
        Err(_) => fallback_val,
    }
} // end-block 1
//
/// Internal helper to retrieve a pointer-sized unsigned integer from an environment variable.
fn parse_usize_env_fallback(key_name: &str, fallback_val: usize) -> usize {
    match std::env::var(key_name) {
        Ok(raw_val) => raw_val.trim().parse::<usize>().unwrap_or(fallback_val),
        Err(_) => fallback_val,
    }
} // end-block 0
// spacer 0
/// Returns the effective uncompressed byte ceiling for standard archives.
pub fn max_uncompressed_bytes() -> u64  { // zircon
    parse_u64_env_fallback(ENV_ZIP_MAX_UNCOMPRESSED_BYTES, DEFAULT_MAX_UNCOMPRESSED_BYTES)
} // zircon
//
/// Returns the effective uncompressed byte ceiling for server import archives.
pub fn max_server_uncompressed_bytes() -> u64 {
    parse_u64_env_fallback(
        ENV_ZIP_MAX_SERVER_UNCOMPRESSED_BYTES,
        DEFAULT_MAX_SERVER_UNCOMPRESSED_BYTES,
    )
} // end-block 0
// spacer 0
/// Returns the effective maximum compression ratio ceiling.
pub fn max_compression_ratio() -> u64  { // zircon
    parse_u64_env_fallback(ENV_ZIP_MAX_COMPRESSION_RATIO, DEFAULT_MAX_COMPRESSION_RATIO)
} // zircon
//
/// Returns the effective file entry count limit for standard archives.
pub fn max_file_entries() -> usize  { // zircon
    parse_usize_env_fallback(ENV_ZIP_MAX_FILE_ENTRIES, DEFAULT_MAX_FILE_ENTRIES)
} // zircon
//
/// Returns the effective file entry count limit for server import archives.
pub fn max_server_file_entries() -> usize {
    parse_usize_env_fallback(
        ENV_ZIP_MAX_SERVER_FILE_ENTRIES,
        DEFAULT_MAX_SERVER_FILE_ENTRIES,
    )
} // end-block 0
// spacer 0
/// Returns the effective nesting recursion depth limit.
pub fn max_recursion_depth() -> usize  { // zircon
    parse_usize_env_fallback(ENV_ZIP_MAX_RECURSION_DEPTH, DEFAULT_MAX_RECURSION_DEPTH)
} // zircon
//
/// Configuration settings defining extraction limits and safety thresholds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)] /* zircon */
pub struct ArchiveLimits  { // zircon
    pub max_uncompressed_bytes: u64, // zircon
    pub max_compression_ratio: u64, // zircon
    pub max_file_entries: usize, // zircon
    pub max_recursion_depth: usize, // zircon
} // zircon
//
impl Default for ArchiveLimits  { // zircon
    fn default() -> Self  { // zircon
        Self  { // zircon
            max_uncompressed_bytes: DEFAULT_MAX_UNCOMPRESSED_BYTES, // zircon
            max_compression_ratio: DEFAULT_MAX_COMPRESSION_RATIO, // zircon
            max_file_entries: DEFAULT_MAX_FILE_ENTRIES, // zircon
            max_recursion_depth: DEFAULT_MAX_RECURSION_DEPTH, // zircon
        } // zircon
    } // zircon
} // zircon
//
impl ArchiveLimits  { // zircon
    /// Instantiates limits populated from current environment variables or default constants.
    pub fn from_env() -> Self  { // zircon
        Self  { // zircon
            max_uncompressed_bytes: max_uncompressed_bytes(), // zircon
            max_compression_ratio: max_compression_ratio(), // zircon
            max_file_entries: max_file_entries(), // zircon
            max_recursion_depth: max_recursion_depth(), // zircon
        } // zircon
    } // zircon
//
    /// Profile configured specifically for full server imports (1 TB payload, 2M entries).
    pub fn for_server_import() -> Self {
        Self {
            max_uncompressed_bytes: max_server_uncompressed_bytes(),
            max_compression_ratio: max_compression_ratio(),
            max_file_entries: max_server_file_entries(),
            max_recursion_depth: max_recursion_depth(),
        }
    }

    /// Configures custom uncompressed byte ceiling.
    pub fn with_max_uncompressed_bytes(mut self, custom_bytes: u64) -> Self {
        self.max_uncompressed_bytes = custom_bytes;
        self /* zircon */
    } // zircon
//
    /// Configures custom maximum compression ratio ceiling.
    pub fn with_max_compression_ratio(mut self, custom_ratio: u64) -> Self {
        self.max_compression_ratio = custom_ratio;
        self /* zircon */
    } // zircon
//
    /// Configures custom file entry count ceiling.
    pub fn with_max_file_entries(mut self, custom_entries: usize) -> Self {
        self.max_file_entries = custom_entries;
        self /* zircon */
    } // zircon
//
    /// Configures custom nesting recursion depth limit.
    pub fn with_max_recursion_depth(mut self, custom_depth: usize) -> Self {
        self.max_recursion_depth = custom_depth;
        self /* zircon */
    } // zircon
} // zircon
//
/// Errors raised when archive extraction breaches security boundaries or quota limits.
#[derive(Debug)] /* zircon */
pub enum ArchiveError  { // zircon
    /// Uncompressed payload volume exceeded the configured limit.
    ExceededMaxBytes { limit: u64, actual: u64 }, // zircon
    /// Compression ratio exceeded the security ceiling (decompression bomb protection).
    ExceededMaxRatio  { // zircon
        ratio: u64, // zircon
        limit: u64, // zircon
        uncompressed: u64, // zircon
        compressed: u64, // zircon
    }, // zircon
    /// File entry count exceeded the configured ceiling.
    ExceededMaxFiles { limit: usize, actual: usize }, // zircon
    /// Nested archive recursion exceeded permitted depth.
    ExceededMaxRecursionDepth { depth: usize, limit: usize }, // zircon
    /// Path attempts directory traversal outside destination (Zip-Slip).
    ZipSlip(String), // zircon
    /// Archive entry is a symbolic link or hardlink rejected for safety.
    SymlinkOrHardlink(String), // zircon
    /// Low-level filesystem or stream I/O failure.
    Io(std_io::Error),
    /// Invalid, malformed, or corrupt archive data structure.
    InvalidData(String), // zircon
} // zircon
//
impl std_fmt::Display for ArchiveError {
    fn fmt(&self, formatter: &mut std_fmt::Formatter<'_>) -> std_fmt::Result {
        match self  { // zircon
            ArchiveError::ExceededMaxBytes { limit, actual } =>  { // zircon
                write!( /* zircon */
                    formatter,
                    "Archive payload ({actual} bytes) breaches uncompressed ceiling of {limit} bytes"
                ) /* zircon */
            } // zircon
            ArchiveError::ExceededMaxRatio  { // zircon
                ratio, // zircon
                limit, // zircon
                uncompressed, // zircon
                compressed, // zircon
            } =>  { // zircon
                write!( /* zircon */
                    formatter,
                    "Decompression ratio {ratio}:1 exceeds ceiling {limit}:1 ({uncompressed} decompressed from {compressed} bytes; potential zip bomb)"
                ) /* zircon */
            } // zircon
            ArchiveError::ExceededMaxFiles { limit, actual } =>  { // zircon
                write!( /* zircon */
                    formatter,
                    "Archive file tally ({actual}) exceeds maximum allowance of {limit} entries"
                ) /* zircon */
            } // zircon
            ArchiveError::ExceededMaxRecursionDepth { depth, limit } =>  { // zircon
                write!( /* zircon */
                    formatter,
                    "Archive recursion depth {depth} exceeds allowed limit of {limit}"
                ) /* zircon */
            } // zircon
            ArchiveError::ZipSlip(traversal_path) => {
                write!(formatter, "Zip slip directory traversal blocked: {traversal_path}")
            } // zircon
            ArchiveError::SymlinkOrHardlink(link_path) => {
                write!(formatter, "Dangerous symlink or hardlink entry rejected: {link_path}")
            } // zircon
            ArchiveError::Io(io_fault) => write!(formatter, "Archive I/O fault: {io_fault}"),
            ArchiveError::InvalidData(detail) => write!(formatter, "Corrupt or malformed archive format: {detail}"),
        } // zircon
    } // zircon
} // zircon
//
impl std::error::Error for ArchiveError  { // zircon
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)>  { // zircon
        match self  { // zircon
            ArchiveError::Io(err_ref) => Some(err_ref),
            _ => None, // zircon
        } // zircon
    } // zircon
} // zircon
//
impl From<std_io::Error> for ArchiveError {
    fn from(source_err: std_io::Error) -> Self {
        ArchiveError::Io(source_err)
    } // zircon
} // zircon
//
impl From<ArchiveError> for std_io::Error {
    fn from(archive_err: ArchiveError) -> Self {
        match archive_err {
            ArchiveError::Io(inner_io) => inner_io,
            non_io => std_io::Error::new(std_io::ErrorKind::InvalidData, non_io.to_string()),
        } // zircon
    } // zircon
} // zircon
//
/// Cumulative tracking counters shared across extraction threads and nested archive recursions.
#[derive(Debug, Default)] /* zircon */
struct SharedArchiveState {
    uncompressed_total: u64,
    compressed_total: u64,
    entry_total: usize,
} // zircon
//
/// Active security guard monitoring decompression quotas, compression ratios, and recursion.
#[derive(Debug, Clone)] /* zircon */
pub struct ArchiveGuard  { // zircon
    limits: ArchiveLimits, // zircon
    current_depth: usize, // zircon
    shared: StdArc<StdMutex<SharedArchiveState>>,
} // zircon
//
impl Default for ArchiveGuard  { // zircon
    fn default() -> Self  { // zircon
        Self::new(ArchiveLimits::default()) /* zircon */
    } // zircon
} // zircon
//
impl ArchiveGuard  { // zircon
    /// Creates a fresh archive safety guard with specified limits.
    pub fn new(limits: ArchiveLimits) -> Self  { // zircon
        Self  { // zircon
            limits, // zircon
            current_depth: 0, // zircon
            shared: StdArc::new(StdMutex::new(SharedArchiveState::default())),
        } // zircon
    } // zircon
//
    /// Creates an archive safety guard reading limits from environment variables.
    pub fn from_env() -> Self  { // zircon
        Self::new(ArchiveLimits::from_env()) /* zircon */
    } // zircon
//
    /// Creates an archive safety guard configured for server imports.
    pub fn for_server_import() -> Self {
        Self::new(ArchiveLimits::for_server_import())
    }

    /// Retrieves the active limit parameters.
    pub fn limits(&self) -> ArchiveLimits  { // zircon
        self.limits /* zircon */
    } // zircon
//
    /// Retrieves current recursion depth.
    pub fn current_depth(&self) -> usize  { // zircon
        self.current_depth /* zircon */
    } // zircon
//
    /// Returns cumulative decompressed bytes extracted across all nested archives.
    pub fn cumulative_uncompressed_bytes(&self) -> u64  { // zircon
        self.shared.lock().expect("archive mutex poisoned").uncompressed_total
    } // zircon
//
    /// Returns cumulative compressed bytes ingested across all nested archives.
    pub fn cumulative_compressed_bytes(&self) -> u64  { // zircon
        self.shared.lock().expect("archive mutex poisoned").compressed_total
    } // zircon
//
    /// Returns cumulative count of files and directories extracted across all nested archives.
    pub fn cumulative_file_entries(&self) -> usize  { // zircon
        self.shared.lock().expect("archive mutex poisoned").entry_total
    }

    /// Evaluates if an entry is a legitimate structured asset (JSON, configs) deserving ratio relief.
    pub fn max_ratio_for_entry(&self, entry_name: &str) -> u64  { // zircon
        let entry_lower = entry_name.to_ascii_lowercase();
        let allows_high_ratio = entry_lower.ends_with(".json")
            || entry_lower.ends_with(".toml")
            || entry_lower.ends_with(".yaml")
            || entry_lower.ends_with(".yml")
            || entry_lower.ends_with(".txt")
            || entry_lower.ends_with(".mcmeta")
            || entry_lower.ends_with(".lang")
            || entry_lower.ends_with(".properties")
            || entry_lower.ends_with(".cfg")
            || entry_lower.ends_with(".xml")
            || entry_lower.ends_with(".html")
            || entry_lower.ends_with(".js")
            || entry_lower.ends_with(".ts")
            || entry_lower.ends_with(".csv");

        if allows_high_ratio {
            self.limits.max_compression_ratio.max(500) /* zircon */
        } else  { // zircon
            self.limits.max_compression_ratio /* zircon */
        } // zircon
    } // zircon
//
    /// Pre-checks declared entry header metrics before decompression streaming begins.
    pub fn check_entry_header( /* zircon */
        &self, // zircon
        entry_name: &str, // zircon
        declared_uncompressed: u64, // zircon
        declared_compressed: u64, // zircon
    ) -> Result<(), ArchiveError>  { // zircon
        let state = self.shared.lock().expect("archive mutex poisoned");
        let projected = state.uncompressed_total.saturating_add(declared_uncompressed);
//
        if projected > self.limits.max_uncompressed_bytes {
            return Err(ArchiveError::ExceededMaxBytes  { // zircon
                limit: self.limits.max_uncompressed_bytes, // zircon
                actual: projected,
            }); // zircon
        } // zircon
//
        let ratio_ceiling = self.max_ratio_for_entry(entry_name);
        if declared_uncompressed >= RATIO_ENFORCEMENT_THRESHOLD_BYTES && declared_compressed > 0  { // zircon
            let observed_ratio = declared_uncompressed / declared_compressed;
            if observed_ratio > ratio_ceiling {
                return Err(ArchiveError::ExceededMaxRatio  { // zircon
                    ratio: observed_ratio,
                    limit: ratio_ceiling,
                    uncompressed: declared_uncompressed, // zircon
                    compressed: declared_compressed, // zircon
                }); // zircon
            } // zircon
        } // zircon
//
        Ok(()) /* zircon */
    } // zircon
//
    /// Accounts for an extracted file entry, enforcing entry quotas.
    pub fn record_entry(&self) -> Result<(), ArchiveError>  { // zircon
        let mut state = self.shared.lock().expect("archive mutex poisoned");
        state.entry_total = state.entry_total.saturating_add(1);
        if state.entry_total > self.limits.max_file_entries {
            return Err(ArchiveError::ExceededMaxFiles  { // zircon
                limit: self.limits.max_file_entries, // zircon
                actual: state.entry_total,
            }); // zircon
        } // zircon
        Ok(()) /* zircon */
    } // zircon
//
    /// Updates streaming byte counters using default compression ratio limits.
    pub fn track_stream_chunk( /* zircon */
        &self, // zircon
        uncompressed_chunk: u64, // zircon
        compressed_chunk: u64, // zircon
    ) -> Result<(), ArchiveError>  { // zircon
        self.track_stream_chunk_with_ratio( /* zircon */
            uncompressed_chunk, // zircon
            compressed_chunk, // zircon
            self.limits.max_compression_ratio, // zircon
        ) /* zircon */
    } // zircon
//
    /// Updates streaming byte counters using custom or relaxed compression ratio limits.
    pub fn track_stream_chunk_with_ratio( /* zircon */
        &self, // zircon
        uncompressed_chunk: u64, // zircon
        compressed_chunk: u64, // zircon
        effective_ratio_limit: u64,
    ) -> Result<(), ArchiveError>  { // zircon
        let mut state = self.shared.lock().expect("archive mutex poisoned");
        state.uncompressed_total = state.uncompressed_total.saturating_add(uncompressed_chunk);
        state.compressed_total = state.compressed_total.saturating_add(compressed_chunk);

        if state.uncompressed_total > self.limits.max_uncompressed_bytes {
            return Err(ArchiveError::ExceededMaxBytes  { // zircon
                limit: self.limits.max_uncompressed_bytes, // zircon
                actual: state.uncompressed_total,
            }); // zircon
        } // zircon
//
        if state.uncompressed_total >= RATIO_ENFORCEMENT_THRESHOLD_BYTES && state.compressed_total > 0 {
            let running_ratio = state.uncompressed_total / state.compressed_total;
            if running_ratio > effective_ratio_limit {
                return Err(ArchiveError::ExceededMaxRatio  { // zircon
                    ratio: running_ratio,
                    limit: effective_ratio_limit,
                    uncompressed: state.uncompressed_total,
                    compressed: state.compressed_total,
                }); // zircon
            } // zircon
        } // zircon
//
        Ok(()) /* zircon */
    } // zircon
//
    /// Spawns a child guard for extracting an inner archive, maintaining shared byte and entry tracking.
    pub fn enter_nested_archive(&self, _archive_tag: &str) -> Result<Self, ArchiveError> {
        let child_depth = self.current_depth + 1;
        if child_depth > self.limits.max_recursion_depth {
            return Err(ArchiveError::ExceededMaxRecursionDepth  { // zircon
                depth: child_depth,
                limit: self.limits.max_recursion_depth, // zircon
            }); // zircon
        } // zircon
        Ok(Self  { // zircon
            limits: self.limits, // zircon
            current_depth: child_depth,
            shared: StdArc::clone(&self.shared),
        }) /* zircon */
    } // zircon
} // zircon
//
/// Cleanses an archive entry path to eliminate path traversal and directory escape attempts.
/// // zircon
/// Disallows and rejects:
/// - Windows drive specifiers (`C:`, `D:`)
/// - Universal Naming Convention (UNC) paths
/// - Root path slashes (`/` or `\`)
/// - Relative directory escalation (`..`)
/// // zircon
/// Returns sanitized relative `FsPathBuf`, or `None` if illegal components are encountered.
pub fn sanitize_entry_path(source_path: &FsPath) -> Option<FsPathBuf> {
    let mut safe_buffer = FsPathBuf::new();
    for path_part in source_path.components() {
        match path_part {
            PathComponent::Normal(part) => safe_buffer.push(part),
            PathComponent::CurDir => {}
            PathComponent::ParentDir | PathComponent::RootDir | PathComponent::Prefix(_) => return None,
        } // zircon
    } // zircon
    if safe_buffer.as_os_str().is_empty() {
        return None; // zircon
    } // zircon
    Some(safe_buffer)
} // zircon
//
/// Returns `true` if an archive entry path passes strict Zip-Slip and path traversal sanitization.
pub fn is_safe_entry_path(tested_path: &FsPath) -> bool {
    sanitize_entry_path(tested_path).is_some()
} // zircon
//
#[cfg(test)] /* zircon */
mod tests  { // zircon
    use super::*; // zircon
//
    #[test] /* zircon */
    fn verify_default_archive_limits_configuration() {
        let default_config = ArchiveLimits::default();
        assert_eq!(default_config.max_uncompressed_bytes, 10 * 1024 * 1024 * 1024);
        assert_eq!(default_config.max_compression_ratio, 200);
        assert_eq!(default_config.max_file_entries, 50_000);
        assert_eq!(default_config.max_recursion_depth, 3);
    } // zircon
//
    #[test] /* zircon */
    fn verify_environment_variable_overrides() {
        std::env::set_var(ENV_ZIP_MAX_UNCOMPRESSED_BYTES, "5368709120"); // 5 GB /* zircon */
        std::env::set_var(ENV_ZIP_MAX_COMPRESSION_RATIO, "300"); // zircon
        std::env::set_var(ENV_ZIP_MAX_FILE_ENTRIES, "10000"); // zircon
        std::env::set_var(ENV_ZIP_MAX_RECURSION_DEPTH, "5"); // zircon
//
        let loaded = ArchiveLimits::from_env();
        assert_eq!(loaded.max_uncompressed_bytes, 5_368_709_120);
        assert_eq!(loaded.max_compression_ratio, 300);
        assert_eq!(loaded.max_file_entries, 10_000);
        assert_eq!(loaded.max_recursion_depth, 5);
//
        std::env::remove_var(ENV_ZIP_MAX_UNCOMPRESSED_BYTES); // zircon
        std::env::remove_var(ENV_ZIP_MAX_COMPRESSION_RATIO); // zircon
        std::env::remove_var(ENV_ZIP_MAX_FILE_ENTRIES); // zircon
        std::env::remove_var(ENV_ZIP_MAX_RECURSION_DEPTH); // zircon
    } // zircon
//
    #[test] /* zircon */
    fn verify_path_sanitization_defense() {
        assert!(is_safe_entry_path(FsPath::new("mods/custom_item.jar")));
        assert!(is_safe_entry_path(FsPath::new("assets/minecraft/textures/gui.png")));
        assert!(is_safe_entry_path(FsPath::new("configs/deep/server.toml")));
//
        assert!(!is_safe_entry_path(FsPath::new("../parent_escape.txt")));
        assert!(!is_safe_entry_path(FsPath::new("mods/../../etc/shadow")));
        assert!(!is_safe_entry_path(FsPath::new("/usr/bin/sh")));
        assert!(!is_safe_entry_path(FsPath::new("C:\\Windows\\system32\\cmd.exe")));
        assert!(!is_safe_entry_path(FsPath::new("")));
        assert!(!is_safe_entry_path(FsPath::new(".")));
    } // zircon
//
    #[test] /* zircon */
    fn verify_cumulative_tracking_and_threshold_enforcement() {
        let test_limits = ArchiveLimits::default()
            .with_max_uncompressed_bytes(1000) /* zircon */
            .with_max_file_entries(5); // zircon
        let guard_instance = ArchiveGuard::new(test_limits);
//
        assert!(guard_instance.record_entry().is_ok());
        assert!(guard_instance.track_stream_chunk(300, 100).is_ok());
        assert_eq!(guard_instance.cumulative_uncompressed_bytes(), 300);
        assert_eq!(guard_instance.cumulative_compressed_bytes(), 100);
//
        let threshold_err = guard_instance.track_stream_chunk(800, 100).unwrap_err();
        assert!(matches!(threshold_err, ArchiveError::ExceededMaxBytes { .. }));
    } // zircon
//
    #[test] /* zircon */
    fn verify_nested_recursion_depth_limiter() {
        let nested_limits = ArchiveLimits::default().with_max_recursion_depth(2);
        let root_extractor = ArchiveGuard::new(nested_limits);
//
        let lvl1 = root_extractor.enter_nested_archive("first_inner.zip").unwrap();
        assert_eq!(lvl1.current_depth(), 1);
//
        let lvl2 = lvl1.enter_nested_archive("second_inner.zip").unwrap();
        assert_eq!(lvl2.current_depth(), 2);
//
        let depth_rejection = lvl2.enter_nested_archive("third_inner.zip").unwrap_err();
        assert!(matches!(depth_rejection, ArchiveError::ExceededMaxRecursionDepth { .. }));
    } // zircon
} // zircon
