//! Strict security validator for Minecraft Resource Packs and Shaderpacks.
//!
//! Enforces a zero-trust, whitelist-only file policy. Any archive containing
//! executable binaries, scripts, disallowed file extensions, or path traversal
//! attempts is immediately rejected.

use std::collections::HashSet;
use std::fmt;
use std::io::{Read, Seek};
use std::path::Path;

use zip::ZipArchive;

use crate::archive::limits::{is_safe_entry_path, ArchiveError, ArchiveGuard};

/// Whitelisted file extensions permitted in Minecraft resource packs and shaderpacks.
pub const WHITELISTED_EXTENSIONS: &[&str] = &[
    // Textures & Images
    "png", "jpg", "jpeg", "tga", "bmp", "webp", "gif",
    // Audio
    "ogg", "mp3", "wav", "mus", "flac", "mid", "midi",
    // Configs, Models, Data, Markdown & Language files
    "json", "mcmeta", "txt", "properties", "lang", "nbt", "snbt", "bbmodel", "jem", "jpm", "obj",
    "mtl", "schem", "schematic", "dat", "ini", "toml", "yaml", "yml", "csv", "md", "markdown",
    // Shaders & GLSL code (used by Iris/Oculus, OptiFine, and vanilla core shaders)
    "fsh", "vsh", "gsh", "csh", "glsl", "inc", "comp", "frag", "vert", "geom",
    // Fonts
    "ttf", "otf", "woff", "woff2",
];

/// Safe unextended text filenames (case-insensitive) permitted at root or in directories.
const SAFE_UNEXTENDED_FILENAMES: &[&str] = &[
    "license",
    "licence",
    "credits",
    "readme",
    "copying",
    "notice",
    "authors",
    "changelog",
    "pack",
];

/// Errors returned when a pack fails strict security validation.
#[derive(Debug)]
pub enum PackSecurityError {
    Archive(ArchiveError),
    Zip(zip::result::ZipError),
    DisallowedFile {
        path: String,
        reason: String,
    },
    EmptyArchive,
}

impl fmt::Display for PackSecurityError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Archive(e) => write!(f, "Archive resource limit or structure error: {e}"),
            Self::Zip(e) => write!(f, "Corrupted or invalid ZIP archive: {e}"),
            Self::DisallowedFile { path, reason } => {
                write!(f, "Security violation in pack entry '{path}': {reason}")
            }
            Self::EmptyArchive => write!(f, "Pack archive contains no files"),
        }
    }
}

impl std::error::Error for PackSecurityError {}

impl From<ArchiveError> for PackSecurityError {
    fn from(e: ArchiveError) -> Self {
        Self::Archive(e)
    }
}

impl From<zip::result::ZipError> for PackSecurityError {
    fn from(e: zip::result::ZipError) -> Self {
        Self::Zip(e)
    }
}

/// Report detailing validation results of a safe resourcepack.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackValidationReport {
    pub file_count: usize,
    pub total_uncompressed_bytes: u64,
    pub extensions_found: Vec<String>,
}

/// Validates a ZIP reader against strict file extension whitelist and path safety rules.
pub fn validate_pack_archive<R: Read + Seek>(
    reader: R,
    guard: &ArchiveGuard,
) -> Result<PackValidationReport, PackSecurityError> {
    let mut zip = ZipArchive::new(reader)?;

    let num_entries = zip.len();
    if num_entries == 0 {
        return Err(PackSecurityError::EmptyArchive);
    }
    if num_entries > guard.limits().max_file_entries {
        return Err(PackSecurityError::Archive(ArchiveError::ExceededMaxFiles {
            limit: guard.limits().max_file_entries,
            actual: num_entries,
        }));
    }

    let whitelist: HashSet<&'static str> = WHITELISTED_EXTENSIONS.iter().copied().collect();
    let safe_stems: HashSet<&'static str> = SAFE_UNEXTENDED_FILENAMES.iter().copied().collect();
    let mut extensions_found = HashSet::new();
    let mut file_count = 0usize;
    let mut total_uncompressed: u64 = 0;
    let mut total_compressed: u64 = 0;

    for i in 0..num_entries {
        let entry = zip.by_index(i)?;
        let name = entry.name().to_string();
        let path = Path::new(&name);

        // 1. Check path traversal and zip-slip
        if !is_safe_entry_path(path) {
            return Err(PackSecurityError::DisallowedFile {
                path: name.clone(),
                reason: "Path traversal or dangerous component detected".to_string(),
            });
        }

        // 2. Directories are permitted if their path is safe
        if entry.is_dir() || name.ends_with('/') {
            continue;
        }

        // Skip harmless OS metadata files created by macOS/Windows zip tools
        if name.starts_with("__MACOSX/")
            || name.ends_with(".DS_Store")
            || name.ends_with("Thumbs.db")
            || name.ends_with(".gitattributes")
            || name.ends_with(".gitignore")
        {
            continue;
        }

        // 3. Size checks against bombs
        let size = entry.size();
        let comp_size = entry.compressed_size();
        total_uncompressed = total_uncompressed.saturating_add(size);
        total_compressed = total_compressed.saturating_add(comp_size);
        guard.check_entry_header(&name, size, comp_size)?;

        // 4. Strict extension whitelist check for all files
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .map(|s| s.to_ascii_lowercase());

        let file_stem = path
            .file_name()
            .and_then(|n| n.to_str())
            .map(|s| s.to_ascii_lowercase());

        match ext {
            Some(ext) => {
                if !whitelist.contains(ext.as_str()) {
                    return Err(PackSecurityError::DisallowedFile {
                        path: name.clone(),
                        reason: format!(
                            "Extension '.{ext}' is not in the allowed resource pack whitelist"
                        ),
                    });
                }
                extensions_found.insert(ext);
            }
            None => {
                // Check if file matches safe unextended text files (e.g. LICENSE, README)
                if let Some(stem) = file_stem {
                    if !safe_stems.contains(stem.as_str()) {
                        return Err(PackSecurityError::DisallowedFile {
                            path: name.clone(),
                            reason: "Files without a whitelisted extension are forbidden".to_string(),
                        });
                    }
                } else {
                    return Err(PackSecurityError::DisallowedFile {
                        path: name.clone(),
                        reason: "Files without a whitelisted extension are forbidden".to_string(),
                    });
                }
            }
        }

        file_count += 1;
    }

    if file_count == 0 {
        return Err(PackSecurityError::EmptyArchive);
    }

    if total_uncompressed > guard.limits().max_uncompressed_bytes {
        return Err(PackSecurityError::Archive(ArchiveError::ExceededMaxBytes {
            limit: guard.limits().max_uncompressed_bytes,
            actual: total_uncompressed,
        }));
    }

    let mut ext_list: Vec<String> = extensions_found.into_iter().collect();
    ext_list.sort();

    Ok(PackValidationReport {
        file_count,
        total_uncompressed_bytes: total_uncompressed,
        extensions_found: ext_list,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Cursor, Write};
    use zip::write::FileOptions;
    use zip::ZipWriter;

    fn create_test_zip(files: &[(&str, &[u8])]) -> Vec<u8> {
        let mut buffer = Vec::new();
        {
            let mut zip = ZipWriter::new(Cursor::new(&mut buffer));
            let options: FileOptions<'_, ()> = FileOptions::default();
            for &(name, content) in files {
                zip.start_file(name, options).unwrap();
                zip.write_all(content).unwrap();
            }
            zip.finish().unwrap();
        }
        buffer
    }

    #[test]
    fn valid_texture_pack_passes() {
        let zip_bytes = create_test_zip(&[
            ("pack.mcmeta", b"{\"pack\":{\"pack_format\":15,\"description\":\"Test\"}}"),
            ("pack.png", b"\x89PNG\r\n\x1a\n"),
            ("assets/minecraft/textures/block/stone.png", b"fake image"),
            ("assets/minecraft/sounds.json", b"{}"),
            ("assets/minecraft/sounds/ambient/cave.ogg", b"fake audio"),
        ]);
        let guard = ArchiveGuard::default();
        let report = validate_pack_archive(Cursor::new(zip_bytes), &guard).unwrap();
        assert_eq!(report.file_count, 5);
        assert!(report.extensions_found.contains(&"png".to_string()));
        assert!(report.extensions_found.contains(&"mcmeta".to_string()));
        assert!(report.extensions_found.contains(&"ogg".to_string()));
    }

    #[test]
    fn dangerous_jar_or_class_file_is_rejected() {
        let zip_bytes = create_test_zip(&[
            ("pack.mcmeta", b"{}"),
            ("assets/minecraft/malicious.jar", b"evil jar"),
        ]);
        let guard = ArchiveGuard::default();
        let err = validate_pack_archive(Cursor::new(zip_bytes), &guard).unwrap_err();
        match err {
            PackSecurityError::DisallowedFile { path, reason } => {
                assert_eq!(path, "assets/minecraft/malicious.jar");
                assert!(reason.contains("not in the allowed"));
            }
            _ => panic!("Expected DisallowedFile error, got: {:?}", err),
        }
    }

    #[test]
    fn dangerous_script_or_executable_is_rejected() {
        for bad_ext in &["exe", "bat", "sh", "py", "js", "vbs", "class", "dll", "so"] {
            let file_name = format!("assets/minecraft/payload.{bad_ext}");
            let zip_bytes = create_test_zip(&[
                ("pack.mcmeta", b"{}"),
                (&file_name, b"payload"),
            ]);
            let guard = ArchiveGuard::default();
            let err = validate_pack_archive(Cursor::new(zip_bytes), &guard).unwrap_err();
            assert!(matches!(err, PackSecurityError::DisallowedFile { .. }), "Failed to reject {}", bad_ext);
        }
    }

    #[test]
    fn file_without_extension_is_rejected() {
        let zip_bytes = create_test_zip(&[
            ("pack.mcmeta", b"{}"),
            ("assets/minecraft/noextension", b"unknown"),
        ]);
        let guard = ArchiveGuard::default();
        let err = validate_pack_archive(Cursor::new(zip_bytes), &guard).unwrap_err();
        assert!(matches!(err, PackSecurityError::DisallowedFile { .. }));
    }

    #[test]
    fn zip_slip_traversal_is_rejected() {
        let zip_bytes = create_test_zip(&[
            ("../../../evil.png", b"fake"),
        ]);
        let guard = ArchiveGuard::default();
        let err = validate_pack_archive(Cursor::new(zip_bytes), &guard).unwrap_err();
        assert!(matches!(err, PackSecurityError::DisallowedFile { .. }));
    }
}
