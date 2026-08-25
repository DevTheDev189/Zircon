//! Packs and unpacks server instance directories as LZ4-compressed TAR
//! archives (`.tar.lz4`).
//!
//! Compression streams each file through the LZ4 frame format. Extraction
//! rejects any entry that escapes the destination directory ("zip-slip" / path
//! traversal), refuses symlinks and hardlinks (Tar-slip), and caps both the
//! entry count and total uncompressed size to blunt decompression bombs, so
//! archives can be restored into a live instance folder safely.
//!
//! Port of `com.mcmanager.core.util.Lz4ArchiveUtil`.

use std::fs::File;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use lz4_flex::frame::{FrameDecoder, FrameEncoder};
use tar::{Archive, Builder, EntryType, Header};

/// Running counters captured by the file-tree walk during packing.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PackStats {
    pub file_count: u64,
    pub uncompressed_bytes: u64,
}

/// Packs the contents of `source_dir` into an LZ4-compressed TAR archive.
///
/// * `source_dir` — the directory whose contents are archived
/// * `target_archive` — the archive file to write (e.g. `backup.tar.lz4`)
/// * `exclude_dir` — optional directory tree inside `source_dir` to skip
///   entirely; used to keep pre-existing backup archives from being nested
///   inside a new one.
/// * `audit_logs` — receives human-readable progress notes (file count,
///   timing, compression ratio).
pub fn compress_directory(
    source_dir: &Path,
    target_archive: &Path,
    exclude_dir: Option<&Path>,
    audit_logs: &mut Vec<String>,
) -> io::Result<PackStats> {
    let start = std::time::Instant::now();
    let mut stats = PackStats::default();

    let file_out = File::create(target_archive)?;
    let lz4_out = FrameEncoder::new(file_out);
    let mut tar_out = Builder::new(lz4_out);
    // Use POSIX PAX headers for long paths, like the Java LONGFILE_POSIX mode.
    tar_out.mode(tar::HeaderMode::Complete);

    let mut stack = vec![source_dir.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let entries = match std::fs::read_dir(&dir) {
            Ok(entries) => entries,
            Err(e) if e.kind() == io::ErrorKind::NotFound => continue,
            Err(e) => return Err(e),
        };
        for entry in entries {
            let entry = entry?;
            let path = entry.path();
            if is_excluded(&path, exclude_dir) {
                continue;
            }
            let file_type = entry.file_type()?;
            if file_type.is_dir() {
                stack.push(path);
            } else if file_type.is_file() {
                append_file(&mut tar_out, &path, source_dir, &mut stats)?;
            }
            // Symlinks and other special files are intentionally skipped.
        }
    }

    // Flush the tar trailer + LZ4 frame.
    tar_out.finish()?;
    let lz4_out = tar_out.into_inner()?;
    let mut file_out = lz4_out.finish()?;
    file_out.flush()?;

    let archive_size = std::fs::metadata(target_archive)?.len();
    let elapsed = start.elapsed();
    let ratio = if stats.uncompressed_bytes > 0 && archive_size > 0 {
        stats.uncompressed_bytes as f64 / archive_size as f64
    } else {
        1.0
    };
    audit_logs.push(format!(
        "Archived {} files ({} bytes) in {} ms. Compressed size: {:.2} MB (ratio {:.2}:1)",
        stats.file_count,
        stats.uncompressed_bytes,
        elapsed.as_millis(),
        archive_size as f64 / (1024.0 * 1024.0),
        ratio
    ));

    Ok(stats)
}

fn is_excluded(path: &Path, exclude_dir: Option<&Path>) -> bool {
    match exclude_dir {
        Some(excl) => path.starts_with(excl),
        None => false,
    }
}

fn append_file(
    tar_out: &mut Builder<FrameEncoder<File>>,
    path: &Path,
    source_dir: &Path,
    stats: &mut PackStats,
) -> io::Result<()> {
    let entry_name = path
        .strip_prefix(source_dir)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/");

    let metadata = std::fs::metadata(path)?;
    let mut header = Header::new_gnu();
    header.set_entry_type(EntryType::Regular);
    header.set_size(metadata.len());
    header.set_mode(0o644);
    if let Ok(mtime) = metadata.modified() {
        if let Ok(dur) = mtime.duration_since(std::time::UNIX_EPOCH) {
            header.set_mtime(dur.as_secs());
        }
    }
    header.set_cksum();

    let mut file = File::open(path)?;
    tar_out.append_data(&mut header, &entry_name, &mut file)?;

    stats.file_count += 1;
    stats.uncompressed_bytes += metadata.len();
    Ok(())
}

pub use crate::archive::limits::is_safe_entry_path;
use crate::archive::limits::{
    max_file_entries, max_uncompressed_bytes, sanitize_entry_path,
    DEFAULT_MAX_FILE_ENTRIES, DEFAULT_MAX_UNCOMPRESSED_BYTES,
};

pub const MAX_TOTAL_EXTRACT_BYTES: u64 = DEFAULT_MAX_UNCOMPRESSED_BYTES; // 10 GB
pub const MAX_FILE_ENTRIES: usize = DEFAULT_MAX_FILE_ENTRIES;

/// Decompresses a `.tar.lz4` archive into `destination_dir`, overwriting files
/// that already exist. Aborts the whole extraction when any entry:
///
/// * is a symlink or hardlink (Tar-slip / link escape),
/// * would escape the destination directory (absolute paths or `..`
///   traversal), or
/// * pushes the archive past the entry-count or total-size bomb limits.
pub fn extract_archive(archive_file: &Path, destination_dir: &Path) -> io::Result<()> {
    let file_in = File::open(archive_file)?;
    let lz4_in = FrameDecoder::new(file_in);
    let mut tar_in = Archive::new(lz4_in);

    let dest = canonicalize_or_create(destination_dir)?;

    let max_bytes = max_uncompressed_bytes();
    let max_entries = max_file_entries();

    let mut entry_count = 0;
    let mut total_uncompressed: u64 = 0;

    let entries = tar_in.entries()?;
    for entry in entries {
        entry_count += 1;
        if entry_count > max_entries {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("Archive exceeds maximum allowed entry count ({max_entries}) (decompression bomb defense)"),
            ));
        }

        let mut entry = entry?;
        let entry_type = entry.header().entry_type();

        // Prevent Symlink / Hardlink directory escape attacks.
        if entry_type.is_symlink() || entry_type.is_hard_link() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "Refusing to extract symlink/hardlink entry: {}",
                    entry.path()?.display()
                ),
            ));
        }

        let path = entry.path()?;
        let safe_path = sanitize_entry_path(&path).ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("Zip slip attempt detected: {}", path.display()),
            )
        })?;

        let target = dest.join(&safe_path);
        // Defense in depth: the resolved target must stay inside dest.
        if !target.starts_with(&dest) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("Zip slip attempt detected: {}", path.display()),
            ));
        }

        if entry_type.is_dir() {
            std::fs::create_dir_all(&target)?;
        } else {
            // Header pre-check if available
            if let Ok(declared_size) = entry.header().size() {
                if total_uncompressed.saturating_add(declared_size) > max_bytes {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        format!("Archive exceeds maximum allowed uncompressed size ({max_bytes} bytes)"),
                    ));
                }
            }

            if let Some(parent) = target.parent() {
                std::fs::create_dir_all(parent)?;
            }
            let mut out = File::create(&target)?;
            let written = io::copy(&mut entry, &mut out)?;
            total_uncompressed += written;

            if total_uncompressed > max_bytes {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("Archive exceeds maximum allowed uncompressed size ({max_bytes} bytes)"),
                ));
            }
        }
    }
    Ok(())
}

fn canonicalize_or_create(destination_dir: &Path) -> io::Result<PathBuf> {
    match std::fs::canonicalize(destination_dir) {
        Ok(c) => Ok(c),
        Err(_) => {
            std::fs::create_dir_all(destination_dir)?;
            std::fs::canonicalize(destination_dir)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("zircon-{}-{}", name, std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn round_trips_directory_structure_and_contents() {
        let dir = temp_dir("archive-roundtrip");
        let source = dir.join("world");
        std::fs::create_dir_all(source.join("region")).unwrap();
        std::fs::write(source.join("level.dat"), "level data").unwrap();
        std::fs::write(source.join("region").join("r.0.0.mca"), "chunk data").unwrap();

        let archive = dir.join("backup.tar.lz4");
        let mut logs = Vec::new();
        compress_directory(&source, &archive, None, &mut logs).unwrap();

        assert!(archive.is_file());
        assert!(std::fs::metadata(&archive).unwrap().len() > 0);
        assert!(!logs.is_empty());
        assert!(logs[0].contains("2 files"));

        let restored = dir.join("restored");
        extract_archive(&archive, &restored).unwrap();
        assert_eq!(
            "level data",
            std::fs::read_to_string(restored.join("level.dat")).unwrap()
        );
        assert_eq!(
            "chunk data",
            std::fs::read_to_string(restored.join("region").join("r.0.0.mca")).unwrap()
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn excludes_directory_tree_from_archive() {
        let dir = temp_dir("archive-exclude");
        let source = dir.join("instance");
        std::fs::create_dir_all(source.join("server")).unwrap();
        std::fs::write(source.join("server").join("server.jar"), "jar bytes").unwrap();
        std::fs::write(source.join("bom.json"), "{}").unwrap();

        let archive = dir.join("backup.tar.lz4");
        compress_directory(
            &source,
            &archive,
            Some(&source.join("server")),
            &mut Vec::new(),
        )
        .unwrap();

        let restored = dir.join("restored");
        extract_archive(&archive, &restored).unwrap();
        assert!(restored.join("bom.json").exists());
        assert!(!restored.join("server").exists());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn archives_everything_without_exclude() {
        let dir = temp_dir("archive-all");
        let source = dir.join("instance");
        std::fs::create_dir_all(&source).unwrap();
        std::fs::write(source.join("a.txt"), "a").unwrap();

        let archive = dir.join("backup.tar.lz4");
        compress_directory(&source, &archive, None, &mut Vec::new()).unwrap();

        let restored = dir.join("restored");
        extract_archive(&archive, &restored).unwrap();
        assert!(restored.join("a.txt").exists());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn path_sanitizer_rejects_traversal() {
        assert!(is_safe_entry_path(Path::new("level.dat")));
        assert!(is_safe_entry_path(Path::new("region/r.0.0.mca")));
        assert!(!is_safe_entry_path(Path::new("../evil.txt")));
        assert!(!is_safe_entry_path(Path::new("/etc/passwd")));
        assert!(!is_safe_entry_path(Path::new("a/../../evil.txt")));
        assert!(!is_safe_entry_path(Path::new("C:\\windows\\system32")));
    }

    #[test]
    fn rejects_zip_slip_entries() {
        // Hand-craft a malicious archive containing a "../evil.txt" entry.
        // (the tar crate refuses to write such names, so the header is
        // assembled manually to simulate an archive from an untrusted source.)
        let dir = temp_dir("archive-slip");
        let archive = dir.join("evil.tar.lz4");
        write_raw_tar_archive_with_slip(&archive);

        let dest = dir.join("dest");
        let err = extract_archive(&archive, &dest).unwrap_err();
        assert!(
            err.to_string().contains("Zip slip"),
            "unexpected error: {err}"
        );
        // Nothing may have been written outside the destination.
        assert!(!dir.join("evil.txt").exists());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn rejects_symlink_and_hardlink_entries() {
        // Untrusted archives may carry symlink (typeflag '2') or hardlink
        // (typeflag '1') entries pointing outside the extraction root
        // (Tar-slip). The builder never writes links, so the headers are
        // assembled manually.
        for (typeflag, label) in [(b'2', "symlink"), (b'1', "hardlink")] {
            let dir = temp_dir("archive-link");
            let archive = dir.join("evil-link.tar.lz4");
            write_raw_tar_archive_with_entry(&archive, b"evil-link", typeflag);

            let dest = dir.join("dest");
            let err = extract_archive(&archive, &dest).unwrap_err();
            assert!(
                err.to_string().contains("symlink/hardlink"),
                "unexpected error for {label}: {err}"
            );
            let _ = std::fs::remove_dir_all(&dir);
        }
    }

    /// Writes a minimal single-header TAR (no data), LZ4-framed. `typeflag`
    /// selects the entry type (e.g. `b'1'` hardlink, `b'2'` symlink).
    fn write_raw_tar_archive_with_entry(archive: &Path, name: &[u8], typeflag: u8) {
        let mut header = [0u8; 512];
        header[..name.len()].copy_from_slice(name);
        header[100..108].copy_from_slice(b"0000644\0");
        header[108..116].copy_from_slice(b"0000000\0");
        header[116..124].copy_from_slice(b"0000000\0");
        // File size in octal (0 bytes of data).
        header[124..136].copy_from_slice(b"00000000000\0");
        header[136..148].copy_from_slice(b"00000000000\0");
        // Checksum field left as spaces while computing, then patched in.
        for b in &mut header[148..156] {
            *b = b' ';
        }
        header[156] = typeflag;
        header[257..263].copy_from_slice(b"ustar\0");
        header[263..265].copy_from_slice(b"00");

        let sum: u32 = header.iter().map(|&b| u32::from(b)).sum();
        let checksum = format!("{sum:06o}\0 ");
        header[148..156].copy_from_slice(checksum.as_bytes());

        let file = File::create(archive).unwrap();
        let mut lz4 = FrameEncoder::new(file);
        lz4.write_all(&header).unwrap();
        lz4.write_all(&[0u8; 1024]).unwrap(); // two zero blocks = end of archive
        lz4.finish().unwrap();
    }

    /// Writes a minimal single-file TAR with a `../evil.txt` entry, LZ4-framed.
    fn write_raw_tar_archive_with_slip(archive: &Path) {
        let mut header = [0u8; 512];
        let name = b"../evil.txt";
        header[..name.len()].copy_from_slice(name);
        header[100..108].copy_from_slice(b"0000644\0");
        header[108..116].copy_from_slice(b"0000000\0");
        header[116..124].copy_from_slice(b"0000000\0");
        // File size in octal (4 bytes of data).
        header[124..136].copy_from_slice(b"00000000004\0");
        header[136..148].copy_from_slice(b"00000000000\0");
        // Checksum field left as spaces while computing, then patched in.
        for b in &mut header[148..156] {
            *b = b' ';
        }
        header[156] = b'0'; // typeflag: regular file
        header[257..263].copy_from_slice(b"ustar\0");
        header[263..265].copy_from_slice(b"00");

        let sum: u32 = header.iter().map(|&b| u32::from(b)).sum();
        let checksum = format!("{sum:06o}\0 ");
        header[148..156].copy_from_slice(checksum.as_bytes());

        let mut data = [0u8; 512];
        data[..4].copy_from_slice(b"boom");

        let file = File::create(archive).unwrap();
        let mut lz4 = FrameEncoder::new(file);
        lz4.write_all(&header).unwrap();
        lz4.write_all(&data).unwrap();
        lz4.write_all(&[0u8; 1024]).unwrap(); // two zero blocks = end of archive
        lz4.finish().unwrap();
    }
}
