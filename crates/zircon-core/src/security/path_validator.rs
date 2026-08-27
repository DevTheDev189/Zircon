//! Path and extension validation for secure file management and BOM config distribution.
//!
//! Enforces zero-trust sandboxing:
//! - Rejects path traversal (`..`, leading `/` or `\`, null bytes, Windows alternate data streams `:`).
//! - Enforces whitelisted non-executable text/data extensions for BOM configuration synchronization.

use std::path::{Component, Path};

/// Allowed file extensions for synchronized configuration files.
pub const ALLOWED_CONFIG_EXTENSIONS: &[&str] = &[
    "toml", "json", "json5", "cfg", "txt", "csv", "yaml", "yml", "properties", "ini", "snbt",
    "conf", "hocon",
];

/// Forbidden filename components or patterns.
pub fn is_forbidden_segment(segment: &str) -> bool {
    let lower = segment.to_ascii_lowercase();
    segment.is_empty()
        || segment == "."
        || segment == ".."
        || segment.contains('\0')
        || segment.contains(':')
        || lower.ends_with(".disabled")
        || lower == ".git"
}

/// Checks if an extension (case-insensitive, without leading dot) is in the allowed config list.
pub fn is_allowed_config_extension(ext: &str) -> bool {
    let lower = ext.trim_start_matches('.').to_ascii_lowercase();
    ALLOWED_CONFIG_EXTENSIONS.iter().any(|&e| e == lower)
}

/// Checks whether a given filename has an allowed configuration extension.
pub fn has_allowed_config_extension(filename: &str) -> bool {
    Path::new(filename)
        .extension()
        .and_then(|ext| ext.to_str())
        .map(is_allowed_config_extension)
        .unwrap_or(false)
}

/// Normalizes and validates a relative path inside a sandboxed jail (e.g. `config/` or `server/`).
///
/// Ensures:
/// 1. No absolute paths or prefixes (like `C:` or `\\`).
/// 2. No `..` component traversal.
/// 3. Normalizes all path separators to forward slashes `/`.
pub fn sanitize_relative_path(input: &str) -> Result<String, String> {
    if input.is_empty() {
        return Ok(String::new());
    }

    if input.contains('\0') {
        return Err("Path contains null byte".to_string());
    }

    let raw_path = Path::new(input);
    let mut clean_components = Vec::new();

    for component in raw_path.components() {
        match component {
            Component::Normal(c) => {
                let seg = c
                    .to_str()
                    .ok_or_else(|| "Invalid UTF-8 in path".to_string())?;
                if seg.contains(':') {
                    return Err("Path contains forbidden stream separator ':'".to_string());
                }
                if seg == ".." || seg == "." {
                    return Err("Path traversal segment detected".to_string());
                }
                clean_components.push(seg);
            }
            Component::CurDir => continue,
            Component::ParentDir => return Err("Parent directory traversal '..' forbidden".to_string()),
            Component::RootDir | Component::Prefix(_) => {
                return Err("Absolute paths and drive prefixes forbidden".to_string())
            }
        }
    }

    Ok(clean_components.join("/"))
}

/// Validates a relative config path for inclusion in the BOM and client synchronization.
///
/// Validates both the relative path structure and ensures the file extension is on the whitelist.
pub fn validate_config_relative_path(path: &str) -> Result<String, String> {
    let sanitized = sanitize_relative_path(path)?;
    if sanitized.is_empty() {
        return Err("Config path cannot be empty".to_string());
    }

    if !has_allowed_config_extension(&sanitized) {
        return Err(format!(
            "File extension not permitted for configuration sync. Allowed extensions: {}",
            ALLOWED_CONFIG_EXTENSIONS.join(", ")
        ));
    }

    Ok(sanitized)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_allowed_extensions() {
        assert!(has_allowed_config_extension("create-common.toml"));
        assert!(has_allowed_config_extension("options.txt"));
        assert!(has_allowed_config_extension("jei/jei-client.ini"));
        assert!(has_allowed_config_extension("sub/dir/config.json5"));
        assert!(has_allowed_config_extension("server.properties"));

        assert!(!has_allowed_config_extension("malicious.jar"));
        assert!(!has_allowed_config_extension("script.bat"));
        assert!(!has_allowed_config_extension("exploit.exe"));
        assert!(!has_allowed_config_extension("payload.sh"));
        assert!(!has_allowed_config_extension("no_ext"));
    }

    #[test]
    fn test_sanitize_relative_path() {
        assert_eq!(
            "foo/bar/baz.toml",
            sanitize_relative_path("foo/bar/baz.toml").unwrap()
        );
        assert_eq!(
            "foo/bar/baz.toml",
            sanitize_relative_path("foo\\bar\\baz.toml").unwrap()
        );
        assert_eq!(
            "foo/baz.toml",
            sanitize_relative_path("./foo/./baz.toml").unwrap()
        );

        assert!(sanitize_relative_path("../secret.txt").is_err());
        assert!(sanitize_relative_path("foo/../../secret.txt").is_err());
        assert!(sanitize_relative_path("/etc/passwd").is_err());
        assert!(sanitize_relative_path("C:\\Windows\\system32").is_err());
        assert!(sanitize_relative_path("foo:stream").is_err());
    }

    #[test]
    fn test_validate_config_relative_path() {
        assert_eq!(
            "jei/recipe-lookup.toml",
            validate_config_relative_path("jei/recipe-lookup.toml").unwrap()
        );
        assert!(validate_config_relative_path("jei/recipe-lookup.jar").is_err());
        assert!(validate_config_relative_path("../jei/recipe-lookup.toml").is_err());
    }
}
