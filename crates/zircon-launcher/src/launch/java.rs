//! Java runtime provisioning for the launch pipeline: maps Minecraft versions
//! to the Java major version they require, locates a `java` executable, and
//! ensures a compatible runtime is available — reusing a system Java when it
//! qualifies, otherwise a cached Temurin JDK or a fresh download from Adoptium.
//!
//! Port of `com.mcmanager.client.launch.JavaRuntimeResolver` and
//! `com.mcmanager.client.launch.JavaRuntimeSelector`.

use std::path::{Path, PathBuf};

use crate::error::LauncherError;
use crate::paths;

/// Maps Minecraft versions to the Java major version they require.
///
/// Fallback for cases where the vanilla version profile's
/// `javaVersion.majorVersion` is not available (e.g. picking a JVM to run a
/// loader installer). The mapping mirrors the Java selector exactly:
///
/// * `< 1.17` — Java 8
/// * `1.17` — Java 16
/// * `1.18 .. 1.20.4` — Java 17
/// * `1.20.5+` — Java 21
///
/// Unparseable versions default to 17.
pub struct JavaRuntimeSelector;

impl JavaRuntimeSelector {
    /// Required Java major version per Minecraft version.
    pub fn get_required_java_major_version(minecraft_version: &str) -> i32 {
        let parts: Vec<&str> = minecraft_version.split('.').collect();
        if parts.is_empty() || parts[0].is_empty() {
            return 17;
        }
        // Non-"1.x" version schemes (e.g. "26.2") are modern by construction;
        // the 1.x-era mappings below would otherwise read them as ancient
        // ("26.2" would parse minor=2 -> Java 8). Snapshot ids like "25w06a"
        // don't parse as a number and fall through to the old logic.
        if let Ok(major) = parts[0].parse::<i32>() {
            if major != 1 {
                return 21;
            }
        }
        if parts.len() < 2 {
            return 17;
        }
        let minor = safe_parse(parts[1]);
        let patch = if parts.len() > 2 {
            safe_parse(parts[2])
        } else {
            0
        };

        if minor < 17 {
            8 // MC < 1.17
        } else if minor == 17 {
            16 // MC 1.17
        } else if minor < 20 || (minor == 20 && patch < 5) {
            17 // MC 1.18 - 1.20.4
        } else {
            21 // MC 1.20.5+
        }
    }

    /// A `java` executable for the given major version.
    ///
    /// Prefers `$JAVA_HOME` when set; otherwise a runtime previously provisioned
    /// under the launcher cache; otherwise a bare `java` resolved from PATH.
    ///
    /// Simplification vs. the Java: there is no "current JVM" to introspect, so
    /// `$JAVA_HOME` is trusted to satisfy the requirement without a version
    /// probe here (the [`JavaRuntimeResolver`] probes properly), and the final
    /// fallback is a bare `java` from PATH instead of the current JVM's home.
    pub fn get_java_executable_path(major_version: i32) -> PathBuf {
        if let Some(java_home) = std::env::var_os("JAVA_HOME") {
            return java_executable(Path::new(&java_home));
        }
        let cached = paths::launcher_dir().join(format!("jdk-{major_version}"));
        let candidate = java_executable(&cached);
        if candidate.is_file() {
            return candidate;
        }
        // Best effort: the resolvers/provisioners will fix this up later.
        if cfg!(target_os = "windows") {
            PathBuf::from("java.exe")
        } else {
            PathBuf::from("java")
        }
    }
}

/// `<javaHome>/bin/java(.exe)` for the current platform.
pub fn java_executable(java_home: &Path) -> PathBuf {
    let exe = if cfg!(target_os = "windows") {
        "java.exe"
    } else {
        "java"
    };
    java_home.join("bin").join(exe)
}

/// Ensures a Java runtime with the required major version is available.
///
/// Resolution order (mirroring `JavaRuntimeResolver.resolve`):
/// 1. A system Java (`$JAVA_HOME`, else `java` on PATH) whose probed major
///    satisfies the requirement.
/// 2. A cached runtime at `cache_dir/jdk-{major}`.
/// 3. A fresh Temurin JDK downloaded from Adoptium and extracted into
///    `cache_dir/jdk-{major}`.
pub struct JavaRuntimeResolver {
    cache_dir: PathBuf,
    http: reqwest::Client,
}

impl JavaRuntimeResolver {
    pub fn new(cache_dir: PathBuf) -> Self {
        Self {
            cache_dir,
            http: reqwest::Client::new(),
        }
    }

    /// Returns a Java home whose major version is `>= required_major`.
    pub async fn resolve(&self, required_major: i32) -> Result<PathBuf, LauncherError> {
        if let Some(home) = sufficient_system_java(required_major).await {
            tracing::info!(
                "Using system Java (major >= {required_major}) at {}",
                home.display()
            );
            return Ok(home);
        }

        let jdk_dir = self.cache_dir.join(format!("jdk-{required_major}"));
        let java_exe = java_executable(&jdk_dir);
        if java_exe.is_file() {
            tracing::info!("Using cached Java runtime at {}", jdk_dir.display());
            return Ok(jdk_dir);
        }

        tracing::info!(
            "Downloading Java {} runtime from Adoptium (this can take a few minutes)...",
            required_major
        );
        // Adoptium serves .zip on Windows and .tar.gz on Linux/macOS.
        let archive_ext = if cfg!(target_os = "windows") {
            "zip"
        } else {
            "tar.gz"
        };
        let archive = self
            .cache_dir
            .join(format!("jdk-{required_major}.{archive_ext}"));
        self.download(&adoptium_url(required_major), &archive)
            .await?;
        self.extract(&archive, &jdk_dir)?;

        if java_exe.is_file() {
            return Ok(jdk_dir);
        }
        // Adoptium archives contain a single top-level folder; look one level down.
        if let Ok(entries) = std::fs::read_dir(&jdk_dir) {
            for entry in entries.flatten() {
                if entry.path().is_dir() {
                    return Ok(entry.path());
                }
            }
        }
        Err(LauncherError::Process(format!(
            "Java runtime downloaded but java executable not found under {}",
            jdk_dir.display()
        )))
    }

    async fn download(&self, url: &str, target: &Path) -> Result<(), LauncherError> {
        // The Adoptium binary endpoint only negotiates `application/octet-stream`;
        // requesting `application/zip` yields HTTP 406. The archive format (zip vs
        // tar.gz) is chosen by Adoptium based on the platform, not the Accept header.
        let accept = "application/octet-stream";
        tracing::info!("Requesting {url} with Accept: {accept}");
        let mut response = self
            .http
            .get(url)
            .header(reqwest::header::ACCEPT, accept)
            .send()
            .await?;
        if response.status() == reqwest::StatusCode::NOT_ACCEPTABLE {
            // A few Adoptium deployments reject any explicit Accept value; retry
            // with reqwest's default (`*/*`), which the endpoint also accepts.
            tracing::warn!(
                "Adoptium returned 406 for Accept: {accept}; retrying without the header"
            );
            response = self.http.get(url).send().await?;
        }
        if !response.status().is_success() {
            return Err(LauncherError::Http {
                status: response.status().as_u16(),
                url: url.to_string(),
            });
        }
        let bytes = response.bytes().await?;
        if let Some(parent) = target.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        tokio::fs::write(target, &bytes).await?;
        Ok(())
    }

    fn extract(&self, archive: &Path, target_dir: &Path) -> Result<(), LauncherError> {
        std::fs::create_dir_all(target_dir)?;
        if archive.to_string_lossy().ends_with(".tar.gz") {
            self.extract_tar_gz(archive, target_dir)?;
        } else {
            self.extract_zip(archive, target_dir)?;
        }
        std::fs::remove_file(archive)?;
        Ok(())
    }

    fn extract_zip(&self, archive: &Path, target_dir: &Path) -> Result<(), LauncherError> {
        let file = std::fs::File::open(archive)?;
        let mut zip = zip::ZipArchive::new(file).map_err(zip_error)?;
        for i in 0..zip.len() {
            let mut entry = zip.by_index(i).map_err(zip_error)?;
            let name = entry.name().to_string();
            // Guard against zip-slip: reject traversal or absolute entry names.
            if name.contains("..") || Path::new(&name).is_absolute() {
                return Err(LauncherError::InvalidInput(format!(
                    "unsafe path in Java runtime archive: {name}"
                )));
            }
            let out_path = target_dir.join(&name);
            if entry.is_dir() {
                std::fs::create_dir_all(&out_path)?;
            } else {
                if let Some(parent) = out_path.parent() {
                    std::fs::create_dir_all(parent)?;
                }
                let mut out = std::fs::File::create(&out_path)?;
                std::io::copy(&mut entry, &mut out)?;
            }
        }
        Ok(())
    }

    /// Extracts a `.tar.gz` archive into `target_dir`.
    ///
    /// Uses `unpack`, which never writes outside `target_dir` (tar-slip
    /// protection) and preserves Unix permissions such as the executable bit on
    /// `bin/java`.
    fn extract_tar_gz(&self, archive: &Path, target_dir: &Path) -> Result<(), LauncherError> {
        let file = std::fs::File::open(archive)?;
        let decoder = flate2::read::GzDecoder::new(file);
        let mut tar = tar::Archive::new(decoder);
        tar.unpack(target_dir)?;
        Ok(())
    }
}

fn zip_error(e: zip::result::ZipError) -> LauncherError {
    LauncherError::Io(std::io::Error::new(std::io::ErrorKind::InvalidData, e))
}

/// Probes a `java` executable for its major version by running `java -version`.
/// Probe failures and unparseable output are treated as insufficient.
async fn probe_java_major(java_exe: &Path) -> Option<i32> {
    let output = tokio::process::Command::new(java_exe)
        .arg("-version")
        .output()
        .await
        .ok()?;
    let stderr = String::from_utf8_lossy(&output.stderr);
    parse_java_version_output(&stderr)
}

/// Extracts the major version from `java -version` output.
///
/// Matches the first dotted number sequence after the word "version" and
/// applies the legacy scheme (`1.8.0_392` is Java 8). Returns `None` when no
/// version token is found.
pub(crate) fn parse_java_version_output(output: &str) -> Option<i32> {
    let version_pos = output.find("version")?;
    let rest = &output[version_pos + "version".len()..];
    let start = rest.find(|c: char| c.is_ascii_digit())?;

    let mut numbers: Vec<i32> = Vec::new();
    let mut current = String::new();
    for ch in rest[start..].chars() {
        if ch.is_ascii_digit() {
            current.push(ch);
        } else {
            if !current.is_empty() {
                numbers.push(current.parse().ok()?);
                current.clear();
            }
            if ch != '.' {
                break;
            }
        }
    }
    if !current.is_empty() {
        numbers.push(current.parse().ok()?);
    }

    let first = *numbers.first()?;
    if first == 1 {
        // Legacy scheme: "1.8.0_392" is Java 8.
        numbers.get(1).copied()
    } else {
        Some(first)
    }
}

/// A system Java satisfying `required_major`, if any: `$JAVA_HOME` when set,
/// else `java` on PATH. When PATH is used, the returned home is a heuristic:
/// the parent of the directory containing the executable.
async fn sufficient_system_java(required_major: i32) -> Option<PathBuf> {
    if let Some(java_home) = std::env::var_os("JAVA_HOME") {
        let home = PathBuf::from(java_home);
        let exe = java_executable(&home);
        if probe_java_major(&exe)
            .await
            .is_some_and(|m| m >= required_major)
        {
            return Some(home);
        }
    } else {
        let name = if cfg!(target_os = "windows") {
            "java.exe"
        } else {
            "java"
        };
        if let Some(exe) = find_on_path(name) {
            if probe_java_major(&exe)
                .await
                .is_some_and(|m| m >= required_major)
            {
                if let Some(home) = exe.parent().and_then(Path::parent) {
                    return Some(home.to_path_buf());
                }
            }
        }
    }
    None
}

/// Searches `PATH` for the first existing `name` entry.
fn find_on_path(name: &str) -> Option<PathBuf> {
    for dir in std::env::split_paths(&std::env::var_os("PATH")?) {
        let candidate = dir.join(name);
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    None
}

/// The Adoptium Temurin JDK download URL for the current platform.
fn adoptium_url(major: i32) -> String {
    let os = if cfg!(target_os = "windows") {
        "windows"
    } else if cfg!(target_os = "macos") {
        "mac"
    } else {
        "linux"
    };
    let arch = if cfg!(target_arch = "aarch64") {
        "aarch64"
    } else {
        "x64"
    };
    format!(
        "https://api.adoptium.net/v3/binary/latest/{major}/ga/{os}/{arch}/jdk/hotspot/normal/eclipse"
    )
}

fn safe_parse(value: &str) -> i32 {
    value.parse().unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn required_java_major_version_mapping() {
        let cases = [
            ("1.16.5", 8),
            ("1.17", 16),
            ("1.17.1", 16),
            ("1.18", 17),
            ("1.20.4", 17),
            ("1.20.5", 21),
            ("1.21.1", 21),
            // Non-1.x schemes (e.g. the "26.2" branding) are modern.
            ("26.2", 21),
            ("25w06a", 17),
            ("", 17),
        ];
        for (minecraft, expected) in cases {
            assert_eq!(
                expected,
                JavaRuntimeSelector::get_required_java_major_version(minecraft),
                "minecraft version {minecraft}"
            );
        }
    }

    #[test]
    fn java_executable_platform_specific() {
        let exe = java_executable(Path::new("/opt/jdk-21"));
        if cfg!(target_os = "windows") {
            assert_eq!(PathBuf::from("/opt/jdk-21/bin/java.exe"), exe);
        } else {
            assert_eq!(PathBuf::from("/opt/jdk-21/bin/java"), exe);
        }
    }

    #[test]
    fn parses_major_from_java_version_output() {
        assert_eq!(
            Some(21),
            parse_java_version_output("openjdk version \"21.0.1\" 2023-10-17")
        );
        assert_eq!(
            Some(8),
            parse_java_version_output("java version \"1.8.0_392\"")
        );
        assert_eq!(
            Some(17),
            parse_java_version_output("java version \"17.0.9\"")
        );
        assert_eq!(None, parse_java_version_output("not a java runtime"));
        assert_eq!(None, parse_java_version_output(""));
    }
}
