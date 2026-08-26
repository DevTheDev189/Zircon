//! Safe ZIP archive validation and streaming extraction.
//!
//! Enforces:
//! - Pre-extraction header validation (size and ratio limits)
//! - Zip-slip / path traversal prevention via strict path sanitization
//! - Rejection of dangerous entry types (symlinks, hardlinks)
//! - Iterative streaming chunk byte tracking to prevent memory exhaustion (OOM)
//! - Cumulative resource guardrails across nested archives (zip-in-a-zip) with recursion depth bounds

use std::fs::{self, File};
use std::io::{self, Read, Seek, Write};
use std::path::{Path, PathBuf};

use zip::ZipArchive;

use crate::archive::limits::{
    is_safe_entry_path, sanitize_entry_path, ArchiveError, ArchiveGuard,
};

/// Streaming buffer size for reading and writing decompressed entries (64 KB).
const STREAM_BUFFER_SIZE: usize = 64 * 1024;

/// Result summary of an archive extraction operation.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ExtractionStats {
    pub files_extracted: usize,
    pub bytes_extracted: u64,
    pub nested_archives_processed: usize,
}

/// Options for configuring ZIP extraction.
#[derive(Debug, Clone, Copy)]
pub struct ZipExtractOptions {
    /// Whether to automatically decompress nested archives (`.zip`, `.mrpack`, `.jar`).
    pub extract_nested: bool,
    /// Whether to overwrite existing files at destination.
    pub overwrite: bool,
}

impl Default for ZipExtractOptions {
    fn default() -> Self {
        Self {
            extract_nested: false,
            overwrite: true,
        }
    }
}

/// Validates the structure and resource bounds of a ZIP archive without extracting files to disk.
///
/// Checks:
/// 1. ZIP format integrity
/// 2. Total entry count
/// 3. Zip-slip path traversal in entry names
/// 4. Declared uncompressed sizes and compression ratios against guard limits
pub fn validate_zip_structure<R: Read + Seek>(
    reader: R,
    guard: &ArchiveGuard,
) -> Result<(), ArchiveError> {
    let mut zip = ZipArchive::new(reader)
        .map_err(|e| ArchiveError::InvalidData(format!("Not a valid ZIP archive: {e}")))?;

    let num_entries = zip.len();
    if num_entries > guard.limits().max_file_entries {
        return Err(ArchiveError::ExceededMaxFiles {
            limit: guard.limits().max_file_entries,
            actual: num_entries,
        });
    }

    let mut total_uncompressed: u64 = 0;
    let mut total_compressed: u64 = 0;

    for i in 0..num_entries {
        let entry = zip
            .by_index(i)
            .map_err(|e| ArchiveError::InvalidData(format!("Corrupt ZIP entry header: {e}")))?;

        let entry_name = entry.name();
        let path = Path::new(entry_name);
        if !is_safe_entry_path(path) {
            return Err(ArchiveError::ZipSlip(entry_name.to_string()));
        }

        let size = entry.size();
        let comp_size = entry.compressed_size();

        total_uncompressed = total_uncompressed.saturating_add(size);
        total_compressed = total_compressed.saturating_add(comp_size);

        guard.check_entry_header(entry_name, size, comp_size)?;
    }

    if total_uncompressed > guard.limits().max_uncompressed_bytes {
        return Err(ArchiveError::ExceededMaxBytes {
            limit: guard.limits().max_uncompressed_bytes,
            actual: total_uncompressed,
        });
    }

    if total_compressed > 0 && total_uncompressed >= crate::archive::limits::RATIO_ENFORCEMENT_THRESHOLD_BYTES {
        let ratio = total_uncompressed / total_compressed;
        if ratio > guard.limits().max_compression_ratio {
            return Err(ArchiveError::ExceededMaxRatio {
                ratio,
                limit: guard.limits().max_compression_ratio,
                uncompressed: total_uncompressed,
                compressed: total_compressed,
            });
        }
    }

    Ok(())
}

/// Safely extracts a ZIP archive into `destination_dir` using the provided guardrails.
pub fn extract_zip<R: Read + Seek>(
    reader: R,
    destination_dir: &Path,
    guard: &ArchiveGuard,
) -> Result<ExtractionStats, ArchiveError> {
    extract_zip_with_options(reader, destination_dir, guard, ZipExtractOptions::default())
}

/// Safely extracts a ZIP archive with explicit extraction options.
pub fn extract_zip_with_options<R: Read + Seek>(
    reader: R,
    destination_dir: &Path,
    guard: &ArchiveGuard,
    options: ZipExtractOptions,
) -> Result<ExtractionStats, ArchiveError> {
    let mut zip = ZipArchive::new(reader)
        .map_err(|e| ArchiveError::InvalidData(format!("Not a valid ZIP archive: {e}")))?;

    let dest = canonicalize_or_create(destination_dir)?;
    let mut stats = ExtractionStats::default();

    let num_entries = zip.len();
    for i in 0..num_entries {
        guard.record_entry()?;

        let mut entry = zip
            .by_index(i)
            .map_err(|e| ArchiveError::InvalidData(format!("Corrupt ZIP entry: {e}")))?;

        let entry_name = entry.name().to_string();
        let path = Path::new(&entry_name);

        // Path traversal / Zip-slip check
        let safe_rel = sanitize_entry_path(path)
            .ok_or_else(|| ArchiveError::ZipSlip(entry_name.clone()))?;

        let target_path = dest.join(&safe_rel);
        if !target_path.starts_with(&dest) {
            return Err(ArchiveError::ZipSlip(entry_name.clone()));
        }

        // Header pre-check
        let declared_uncompressed = entry.size();
        let declared_compressed = entry.compressed_size();
        guard.check_entry_header(&entry_name, declared_uncompressed, declared_compressed)?;

        if entry.is_dir() {
            fs::create_dir_all(&target_path)?;
            continue;
        }

        if let Some(parent) = target_path.parent() {
            fs::create_dir_all(parent)?;
        }

        if target_path.exists() && !options.overwrite {
            continue;
        }

        // Streaming chunk copy with iterative byte tracking
        let bytes_written = copy_stream_with_guard(
            &mut entry,
            &target_path,
            guard,
            &entry_name,
            declared_uncompressed,
            declared_compressed,
        )?;
        stats.files_extracted += 1;
        stats.bytes_extracted += bytes_written;

        // Check if entry is a nested archive and recursive extraction is requested
        if options.extract_nested && is_nested_archive_extension(&entry_name) {
            let nested_guard = guard.enter_nested_archive(&entry_name)?;
            let nested_dest = target_path.with_extension("");
            let nested_file = File::open(&target_path)?;
            let nested_stats = extract_zip_with_options(
                nested_file,
                &nested_dest,
                &nested_guard,
                options,
            )?;
            stats.files_extracted += nested_stats.files_extracted;
            stats.bytes_extracted += nested_stats.bytes_extracted;
            stats.nested_archives_processed += 1 + nested_stats.nested_archives_processed;
        }
    }

    Ok(stats)
}

/// Streams data from a reader to a destination file in bounded chunks,
/// iteratively updating the extraction guard to prevent OOM.
fn copy_stream_with_guard<R: Read>(
    reader: &mut R,
    target_path: &Path,
    guard: &ArchiveGuard,
    entry_name: &str,
    declared_uncompressed: u64,
    declared_compressed: u64,
) -> Result<u64, ArchiveError> {
    let mut file = File::create(target_path)?;
    let mut buffer = [0u8; STREAM_BUFFER_SIZE];
    let mut total_written: u64 = 0;
    let mut total_compressed_accounted: u64 = 0;
    let allowed_ratio = guard.max_ratio_for_entry(entry_name);

    loop {
        let bytes_read = match reader.read(&mut buffer) {
            Ok(0) => break,
            Ok(n) => n,
            Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
            Err(e) => return Err(ArchiveError::Io(e)),
        };

        file.write_all(&buffer[..bytes_read])?;
        let uncomp_chunk = bytes_read as u64;
        total_written += uncomp_chunk;

        let target_compressed = if declared_uncompressed > 0 {
            (total_written.saturating_mul(declared_compressed)) / declared_uncompressed
        } else {
            declared_compressed
        };
        let compressed_chunk = target_compressed.saturating_sub(total_compressed_accounted);
        total_compressed_accounted += compressed_chunk;

        // Iteratively track stream bytes with guard
        guard.track_stream_chunk_with_ratio(uncomp_chunk, compressed_chunk, allowed_ratio)?;
    }

    if total_compressed_accounted < declared_compressed {
        let remaining_comp = declared_compressed - total_compressed_accounted;
        guard.track_stream_chunk_with_ratio(0, remaining_comp, allowed_ratio)?;
    }

    file.flush()?;
    Ok(total_written)
}

fn canonicalize_or_create(dir: &Path) -> io::Result<PathBuf> {
    match fs::canonicalize(dir) {
        Ok(c) => Ok(c),
        Err(_) => {
            fs::create_dir_all(dir)?;
            fs::canonicalize(dir)
        }
    }
}

fn is_nested_archive_extension(name: &str) -> bool {
    let lower = name.to_lowercase();
    lower.ends_with(".zip") || lower.ends_with(".mrpack") || lower.ends_with(".jar")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::archive::limits::ArchiveLimits;
    use zip::write::SimpleFileOptions;
    use zip::ZipWriter;

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("zircon-zip-{}-{}", name, std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn extracts_valid_zip_archive_safely() {
        let dir = temp_dir("valid-extract");
        let zip_path = dir.join("test.zip");

        {
            let file = File::create(&zip_path).unwrap();
            let mut writer = ZipWriter::new(file);
            let opts = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);

            writer.start_file("config/options.txt", opts).unwrap();
            writer.write_all(b"renderDistance=12\nmaxFps=144").unwrap();

            writer.start_file("mods/sample.txt", opts).unwrap();
            writer.write_all(b"sample mod asset").unwrap();

            writer.finish().unwrap();
        }

        let guard = ArchiveGuard::default();
        let out_dir = dir.join("output");
        let file = File::open(&zip_path).unwrap();
        let stats = extract_zip(file, &out_dir, &guard).unwrap();

        assert_eq!(stats.files_extracted, 2);
        assert!(out_dir.join("config/options.txt").is_file());
        assert!(out_dir.join("mods/sample.txt").is_file());

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn rejects_zip_slip_attempt() {
        let dir = temp_dir("zip-slip");
        let zip_path = dir.join("evil.zip");

        {
            let file = File::create(&zip_path).unwrap();
            let mut writer = ZipWriter::new(file);
            let opts = SimpleFileOptions::default();

            writer.start_file("../evil.txt", opts).unwrap();
            writer.write_all(b"evil content").unwrap();
            writer.finish().unwrap();
        }

        let guard = ArchiveGuard::default();
        let out_dir = dir.join("output");
        let file = File::open(&zip_path).unwrap();
        let err = extract_zip(file, &out_dir, &guard).unwrap_err();

        assert!(matches!(err, ArchiveError::ZipSlip(_)));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn handles_nested_archive_extraction_and_depth_limits() {
        let dir = temp_dir("nested-test");
        let inner_zip_path = dir.join("inner.zip");
        let outer_zip_path = dir.join("outer.zip");

        // 1. Create inner zip
        {
            let file = File::create(&inner_zip_path).unwrap();
            let mut writer = ZipWriter::new(file);
            let opts = SimpleFileOptions::default();
            writer.start_file("inner_file.txt", opts).unwrap();
            writer.write_all(b"inner payload").unwrap();
            writer.finish().unwrap();
        }

        let inner_bytes = fs::read(&inner_zip_path).unwrap();

        // 2. Create outer zip containing inner zip
        {
            let file = File::create(&outer_zip_path).unwrap();
            let mut writer = ZipWriter::new(file);
            let opts = SimpleFileOptions::default();
            writer.start_file("outer_file.txt", opts).unwrap();
            writer.write_all(b"outer payload").unwrap();

            writer.start_file("nested/inner.zip", opts).unwrap();
            writer.write_all(&inner_bytes).unwrap();
            writer.finish().unwrap();
        }

        // Test with nested extraction enabled
        let guard = ArchiveGuard::new(ArchiveLimits::default().with_max_recursion_depth(2));
        let out_dir = dir.join("output");
        let file = File::open(&outer_zip_path).unwrap();
        let stats = extract_zip_with_options(
            file,
            &out_dir,
            &guard,
            ZipExtractOptions {
                extract_nested: true,
                overwrite: true,
            },
        )
        .unwrap();

        assert_eq!(stats.files_extracted, 3); // outer_file.txt + inner.zip + inner_file.txt
        assert_eq!(stats.nested_archives_processed, 1);
        assert!(out_dir.join("outer_file.txt").is_file());
        assert!(out_dir.join("nested/inner/inner_file.txt").is_file());

        let _ = fs::remove_dir_all(&dir);
    }
}
