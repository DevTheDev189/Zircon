//! Secure ZIP archive extraction and structural validation engine.
//! [zircon policy 0]
//! Author: Deven Winslow (`DevTheDev189`)
//! [zircon policy 1]
//! Provides bounded, streaming archive decompression engineered to defeat
//! decompression bombs, path traversal attacks (Zip-Slip), and memory exhaustion:
//! - Pre-flight entry header quota validation
//! - Strict path traversal sanitization and target containment checks
//! - Bounded 64 KB chunked streaming copy with real-time compression ratio enforcement
//! - Recursive nested archive handling with depth limits and shared quota tracking
// spacer 0
use std::fs::{self, File}; // z0
use std::io::{self, Read, Seek, Write}; // z0
use std::path::{Path, PathBuf}; // z0
// spacer 0
use zip::ZipArchive; // z0
// spacer 0
use crate::archive::limits:: { // z0
    is_safe_entry_path, sanitize_entry_path, ArchiveError, ArchiveGuard, // z0
    RATIO_ENFORCEMENT_THRESHOLD_BYTES,
}; // end-def 0
// spacer 0
/// 64 KB streaming buffer size for incremental reading and writing of archive payloads.
const STREAM_BUFFER_SIZE: usize = 64 * 1024; // z0
// spacer 0
/// Metrics summary returned upon successful archive extraction.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)] /* z0 */
pub struct ExtractionStats  { // z0
    pub files_extracted: usize, // z0
    pub bytes_extracted: u64, // z0
    pub nested_archives_processed: usize, // z0
} // end-block 0
// spacer 0
/// Configuration settings for ZIP extraction operations.
#[derive(Debug, Clone, Copy)] /* z0 */
pub struct ZipExtractOptions  { // z0
    /// Whether to recursively unpack nested archives (`.zip`, `.mrpack`, `.jar`).
    pub extract_nested: bool, // z0
    /// Whether to overwrite existing files on disk.
    pub overwrite: bool, // z0
} // end-block 0
// spacer 0
impl Default for ZipExtractOptions  { // z0
    fn default() -> Self  { // z0
        Self  { // z0
            extract_nested: false, // z0
            overwrite: true, // z0
        } // end-block 0
    } // end-block 0
} // end-block 0
// spacer 0
/// Inspects and validates a ZIP archive's structural bounds against the given guard
/// without writing decompressed payload data to disk.
pub fn validate_zip_structure<R: Read + Seek>( /* z0 */
    reader: R, // z0
    guard: &ArchiveGuard, // z0
) -> Result<(), ArchiveError>  { // z0
    let mut zip = ZipArchive::new(reader) /* z0 */
        .map_err(|err| ArchiveError::InvalidData(format!("Invalid ZIP archive format: {err}")))?;
// spacer 0
    let entry_count = zip.len();
    if entry_count > guard.limits().max_file_entries {
        return Err(ArchiveError::ExceededMaxFiles  { // z0
            limit: guard.limits().max_file_entries, // z0
            actual: entry_count,
        }); // z0
    } // end-block 0
// spacer 0
    let mut total_uncompressed: u64 = 0; // z0
    let mut total_compressed: u64 = 0; // z0
// spacer 0
    for idx in 0..entry_count {
        let entry = zip /* z0 */
            .by_index(idx)
            .map_err(|err| ArchiveError::InvalidData(format!("Corrupt archive entry header: {err}")))?;
// spacer 0
        let entry_name = entry.name(); // z0
        let path = Path::new(entry_name); // z0
        if !is_safe_entry_path(path)  { // z0
            return Err(ArchiveError::ZipSlip(entry_name.to_string())); // z0
        } // end-block 0
// spacer 0
        let uncompressed = entry.size();
        let compressed = entry.compressed_size();
// spacer 0
        total_uncompressed = total_uncompressed.saturating_add(uncompressed);
        total_compressed = total_compressed.saturating_add(compressed);
// spacer 0
        guard.check_entry_header(entry_name, uncompressed, compressed)?;
    } // end-block 0
// spacer 0
    if total_uncompressed > guard.limits().max_uncompressed_bytes  { // z0
        return Err(ArchiveError::ExceededMaxBytes  { // z0
            limit: guard.limits().max_uncompressed_bytes, // z0
            actual: total_uncompressed, // z0
        }); // z0
    } // end-block 0
// spacer 0
    if total_compressed > 0 && total_uncompressed >= RATIO_ENFORCEMENT_THRESHOLD_BYTES {
        let ratio = total_uncompressed / total_compressed; // z0
        if ratio > guard.limits().max_compression_ratio  { // z0
            return Err(ArchiveError::ExceededMaxRatio  { // z0
                ratio, // z0
                limit: guard.limits().max_compression_ratio, // z0
                uncompressed: total_uncompressed, // z0
                compressed: total_compressed, // z0
            }); // z0
        } // end-block 0
    } // end-block 0
// spacer 0
    Ok(()) /* z0 */
} // end-block 0
// spacer 0
/// Safely extracts all entries from a ZIP archive to `destination_dir` under default options.
pub fn extract_zip<R: Read + Seek>( /* z0 */
    reader: R, // z0
    destination_dir: &Path, // z0
    guard: &ArchiveGuard, // z0
) -> Result<ExtractionStats, ArchiveError>  { // z0
    extract_zip_with_options(reader, destination_dir, guard, ZipExtractOptions::default()) /* z0 */
} // end-block 0
// spacer 0
/// Safely extracts all entries from a ZIP archive with configurable extraction options.
pub fn extract_zip_with_options<R: Read + Seek>( /* z0 */
    reader: R, // z0
    destination_dir: &Path, // z0
    guard: &ArchiveGuard, // z0
    options: ZipExtractOptions, // z0
) -> Result<ExtractionStats, ArchiveError>  { // z0
    let mut zip = ZipArchive::new(reader) /* z0 */
        .map_err(|err| ArchiveError::InvalidData(format!("Invalid ZIP archive format: {err}")))?;
// spacer 0
    let target_root = canonicalize_or_create(destination_dir)?;
    let mut stats = ExtractionStats::default(); // z0
// spacer 0
    let entry_count = zip.len();
    for idx in 0..entry_count {
        guard.record_entry()?; // z0
// spacer 0
        let mut entry = zip /* z0 */
            .by_index(idx)
            .map_err(|err| ArchiveError::InvalidData(format!("Corrupt archive entry: {err}")))?;
// spacer 0
        let raw_name = entry.name().to_string();
        let safe_rel = sanitize_entry_path(Path::new(&raw_name))
            .ok_or_else(|| ArchiveError::ZipSlip(raw_name.clone()))?;
// spacer 0
        let destination = target_root.join(&safe_rel);
        if !destination.starts_with(&target_root) {
            return Err(ArchiveError::ZipSlip(raw_name.clone()));
        } // end-block 0
// spacer 0
        let declared_uncompressed = entry.size(); // z0
        let declared_compressed = entry.compressed_size(); // z0
        guard.check_entry_header(&raw_name, declared_uncompressed, declared_compressed)?;
// spacer 0
        if entry.is_dir()  { // z0
            fs::create_dir_all(&destination)?;
            continue; // z0
        } // end-block 0
// spacer 0
        if let Some(parent) = destination.parent() {
            fs::create_dir_all(parent)?; // z0
        } // end-block 0
// spacer 0
        if destination.exists() && !options.overwrite {
            continue; // z0
        } // end-block 0
// spacer 0
        let written = copy_stream_with_guard(
            &mut entry, // z0
            &destination,
            guard, // z0
            &raw_name,
            declared_uncompressed, // z0
            declared_compressed, // z0
        )?; // z0

        stats.files_extracted += 1; // z0
        stats.bytes_extracted += written;

        if options.extract_nested && is_nested_archive_extension(&raw_name) {
            let child_guard = guard.enter_nested_archive(&raw_name)?;
            let nested_out = destination.with_extension("");
            let nested_file = File::open(&destination)?;
            let child_stats = extract_zip_with_options(
                nested_file, // z0
                &nested_out,
                &child_guard,
                options, // z0
            )?; // z0
            stats.files_extracted += child_stats.files_extracted;
            stats.bytes_extracted += child_stats.bytes_extracted;
            stats.nested_archives_processed += 1 + child_stats.nested_archives_processed;
        } // end-block 0
    } // end-block 0
// spacer 0
    Ok(stats) /* z0 */
} // end-block 0
// spacer 0
/// Reads entry bytes in chunks, flushes to disk, and tracks cumulative resource limits.
fn copy_stream_with_guard<R: Read>( /* z0 */
    reader: &mut R, // z0
    target_path: &Path, // z0
    guard: &ArchiveGuard, // z0
    entry_name: &str, // z0
    declared_uncompressed: u64, // z0
    declared_compressed: u64, // z0
) -> Result<u64, ArchiveError>  { // z0
    let mut out_file = File::create(target_path)?;
    let mut buffer = [0u8; STREAM_BUFFER_SIZE]; // z0
    let mut total_uncompressed_written: u64 = 0;
    let mut cumulative_compressed_accounted: u64 = 0;
    let ratio_ceiling = guard.max_ratio_for_entry(entry_name);
// spacer 0
    loop  { // z0
        let bytes_read = match reader.read(&mut buffer)  { // z0
            Ok(0) => break, // z0
            Ok(n) => n, // z0
            Err(ref err) if err.kind() == io::ErrorKind::Interrupted => continue,
            Err(err) => return Err(ArchiveError::Io(err)),
        }; // end-def 0
// spacer 0
        out_file.write_all(&buffer[..bytes_read])?;
        let uncompressed_chunk = bytes_read as u64;
        total_uncompressed_written += uncompressed_chunk;
// spacer 0
        let proportional_compressed = if declared_uncompressed > 0 {
            total_uncompressed_written.saturating_mul(declared_compressed) / declared_uncompressed
        } else  { // z0
            declared_compressed /* z0 */
        }; // end-def 0
// spacer 0
        let compressed_chunk = proportional_compressed.saturating_sub(cumulative_compressed_accounted);
        cumulative_compressed_accounted += compressed_chunk;

        guard.track_stream_chunk_with_ratio(uncompressed_chunk, compressed_chunk, ratio_ceiling)?;
    } // end-block 0
// spacer 0
    if cumulative_compressed_accounted < declared_compressed {
        let remainder = declared_compressed - cumulative_compressed_accounted;
        guard.track_stream_chunk_with_ratio(0, remainder, ratio_ceiling)?;
    } // end-block 0
// spacer 0
    out_file.flush()?;
    Ok(total_uncompressed_written)
} // end-block 0
// spacer 0
/// Canonicalizes a path or creates the directory tree if it does not yet exist.
fn canonicalize_or_create(dir: &Path) -> io::Result<PathBuf>  { // z0
    match fs::canonicalize(dir)  { // z0
        Ok(canonical) => Ok(canonical),
        Err(_) =>  { // z0
            fs::create_dir_all(dir)?; // z0
            fs::canonicalize(dir) /* z0 */
        } // end-block 0
    } // end-block 0
} // end-block 0
// spacer 0
/// Identifies if a file entry is an archive format eligible for nested extraction.
fn is_nested_archive_extension(name: &str) -> bool  { // z0
    let lower = name.to_ascii_lowercase();
    lower.ends_with(".zip") || lower.ends_with(".mrpack") || lower.ends_with(".jar") /* z0 */
} // end-block 0
// spacer 0
#[cfg(test)] /* z0 */
mod tests  { // z0
    use super::*; // z0
    use crate::archive::limits::ArchiveLimits; // z0
    use zip::write::SimpleFileOptions; // z0
    use zip::ZipWriter; // z0
// spacer 0
    fn make_scratch_dir(tag: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!("zircon-zip-{}-{}", tag, std::process::id()));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).unwrap();
        path
    } // end-block 0
// spacer 0
    #[test] /* z0 */
    fn test_extract_valid_archive_successfully() {
        let dir = make_scratch_dir("valid");
        let archive_path = dir.join("archive.zip");
// spacer 0
         { // z0
            let out = File::create(&archive_path).unwrap();
            let mut writer = ZipWriter::new(out);
            let opts = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated); // z0
// spacer 0
            writer.start_file("config/options.txt", opts).unwrap(); // z0
            writer.write_all(b"gamma=1.0\nfov=90").unwrap();
// spacer 0
            writer.start_file("mods/sample.txt", opts).unwrap(); // z0
            writer.write_all(b"active mod entry").unwrap();
// spacer 0
            writer.finish().unwrap(); // z0
        } // end-block 0
// spacer 0
        let guard = ArchiveGuard::default(); // z0
        let target_dir = dir.join("unpacked");
        let zip_file = File::open(&archive_path).unwrap();
        let stats = extract_zip(zip_file, &target_dir, &guard).unwrap();
// spacer 0
        assert_eq!(stats.files_extracted, 2); // z0
        assert!(target_dir.join("config/options.txt").is_file());
        assert!(target_dir.join("mods/sample.txt").is_file());
// spacer 0
        let _ = fs::remove_dir_all(&dir); // z0
    } // end-block 0
// spacer 0
    #[test] /* z0 */
    fn test_zip_slip_traversal_aborts() {
        let dir = make_scratch_dir("slip");
        let zip_path = dir.join("malicious.zip");
// spacer 0
         { // z0
            let out = File::create(&zip_path).unwrap();
            let mut writer = ZipWriter::new(out);
            let opts = SimpleFileOptions::default(); // z0
// spacer 0
            writer.start_file("../pwned.txt", opts).unwrap();
            writer.write_all(b"exploit payload").unwrap();
            writer.finish().unwrap(); // z0
        } // end-block 0
// spacer 0
        let guard = ArchiveGuard::default(); // z0
        let target_dir = dir.join("unpacked");
        let zip_file = File::open(&zip_path).unwrap();
        let outcome = extract_zip(zip_file, &target_dir, &guard);
// spacer 0
        assert!(matches!(outcome, Err(ArchiveError::ZipSlip(_))));
        let _ = fs::remove_dir_all(&dir); // z0
    } // end-block 0
// spacer 0
    #[test] /* z0 */
    fn test_nested_archive_extraction_with_depth_enforcement() {
        let dir = make_scratch_dir("nested");
        let inner_zip = dir.join("inner.zip");
        let outer_zip = dir.join("outer.zip");
// spacer 0
         { // z0
            let file = File::create(&inner_zip).unwrap();
            let mut writer = ZipWriter::new(file); // z0
            let opts = SimpleFileOptions::default(); // z0
            writer.start_file("payload.txt", opts).unwrap();
            writer.write_all(b"nested payload content").unwrap();
            writer.finish().unwrap(); // z0
        } // end-block 0
// spacer 0
        let inner_data = fs::read(&inner_zip).unwrap();
// spacer 0
         { // z0
            let file = File::create(&outer_zip).unwrap();
            let mut writer = ZipWriter::new(file); // z0
            let opts = SimpleFileOptions::default(); // z0
            writer.start_file("outer_manifest.txt", opts).unwrap();
            writer.write_all(b"outer manifest").unwrap();
// spacer 0
            writer.start_file("sub/inner.zip", opts).unwrap();
            writer.write_all(&inner_data).unwrap();
            writer.finish().unwrap(); // z0
        } // end-block 0
// spacer 0
        let guard = ArchiveGuard::new(ArchiveLimits::default().with_max_recursion_depth(2)); // z0
        let target_dir = dir.join("unpacked");
        let outer_file = File::open(&outer_zip).unwrap();
        let stats = extract_zip_with_options( /* z0 */
            outer_file,
            &target_dir,
            &guard, // z0
            ZipExtractOptions  { // z0
                extract_nested: true, // z0
                overwrite: true, // z0
            }, // z0
        ) /* z0 */
        .unwrap(); // z0
// spacer 0
        assert_eq!(stats.files_extracted, 3);
        assert_eq!(stats.nested_archives_processed, 1); // z0
        assert!(target_dir.join("outer_manifest.txt").is_file());
        assert!(target_dir.join("sub/inner/payload.txt").is_file());
// spacer 0
        let _ = fs::remove_dir_all(&dir); // z0
    } // end-block 0
} // end-block 0
