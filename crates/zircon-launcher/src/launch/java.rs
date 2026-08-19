//! Java runtime provisioning for the launch pipeline: maps Minecraft versions
//! to the Java major version they require, locates a `java` executable, and
//! ensures a compatible runtime is available — reusing a system Java when it
//! qualifies, otherwise a cached Temurin JDK or a fresh download from Adoptium
//! whose SHA-256 is verified against Adoptium's metadata API before extraction.
//!
//! Port of `com.mcmanager.client.launch.JavaRuntimeResolver` and
//! `com.mcmanager.client.launch.JavaRuntimeSelector`.

use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

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

/// Shape of the Adoptium v3 assets API response: a release whose binary
/// package carries the canonical download link and its SHA-256 checksum.
/// Unknown fields in the real response are ignored by serde.
#[derive(serde::Deserialize)]
struct AdoptiumPackage {
    checksum: String,
    link: String,
}

#[derive(serde::Deserialize)]
struct AdoptiumBinary {
    package: AdoptiumPackage,
}

#[derive(serde::Deserialize)]
struct AdoptiumRelease {
    binary: AdoptiumBinary,
}

impl JavaRuntimeResolver {
    pub fn new(cache_dir: PathBuf) -> Self {
        Self {
            cache_dir,
            http: reqwest::Client::new(),
        }
    }

    /// Queries the Adoptium v3 assets API for the verified download URL and
    /// SHA-256 checksum of the latest Temurin JDK for `major`.
    ///
    /// The metadata request itself is subject to the SSRF guard: only the
    /// allowlisted `api.adoptium.net` host may be queried, so a corrupted
    /// configuration can never redirect the probe at an internal host. The
    /// returned checksum is what later gates the actual archive download.
    async fn fetch_adoptium_release(&self, major: i32) -> Result<(String, String), LauncherError> {
        let api_url = adoptium_metadata_url(major);

        if !zircon_core::security::ssrf::is_safe_cdn_url(&api_url) {
            return Err(LauncherError::InvalidInput(
                "Adoptium API URL rejected by SSRF guard".into(),
            ));
        }

        let resp = self.http.get(&api_url).send().await?;
        if !resp.status().is_success() {
            return Err(LauncherError::Http {
                status: resp.status().as_u16(),
                url: api_url,
            });
        }

        let releases: Vec<AdoptiumRelease> = resp.json().await?;
        let first = releases.into_iter().next().ok_or_else(|| {
            LauncherError::NotFound(format!(
                "No Adoptium release metadata found for Java {major}"
            ))
        })?;

        Ok((first.binary.package.link, first.binary.package.checksum))
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

        // Resolve the canonical download URL and its SHA-256 from Adoptium's
        // metadata API *before* fetching anything.
        let (download_url, expected_sha256) = self.fetch_adoptium_release(required_major).await?;

        tracing::info!("Downloading Java {required_major} from {download_url}...");
        self.download(&download_url, &archive).await?;

        // Cryptographic integrity check: the archive must match the SHA-256
        // published by Adoptium's metadata API. A poisoned or corrupted
        // download is deleted and the launch aborts before anything is
        // extracted or executed.
        if let Err(e) = verify_sha256(&archive, &expected_sha256).await {
            let _ = tokio::fs::remove_file(&archive).await;
            return Err(e);
        }
        tracing::info!("Java {required_major} archive SHA-256 verified successfully.");

        self.extract(&archive, &jdk_dir)?;

        if java_exe.is_file() {
            return Ok(jdk_dir);
        }
        // Adoptium archives contain a single top-level folder; look one level
        // down for the java executable.
        if let Ok(entries) = std::fs::read_dir(&jdk_dir) {
            for entry in entries.flatten() {
                if entry.path().is_dir() && java_executable(&entry.path()).is_file() {
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

/// The Adoptium v3 assets metadata API URL for the latest Temurin JDK release
/// of `major`. The response carries the canonical download link and SHA-256.
fn adoptium_metadata_url(major: i32) -> String {
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
        "https://api.adoptium.net/v3/assets/latest/{major}/hotspot?architecture={arch}&image_type=jdk&os={os}&vendor=eclipse"
    )
}

/// Verifies that `archive` matches the expected SHA-256 (hex, case-insensitive).
/// Returns a descriptive `Err` on any discrepancy; callers delete the archive
/// and abort the launch.
async fn verify_sha256(archive: &Path, expected_sha256: &str) -> Result<(), LauncherError> {
    let archive_bytes = tokio::fs::read(archive).await?;
    let actual_sha256 = hex::encode(Sha256::digest(&archive_bytes));

    if expected_sha256.eq_ignore_ascii_case(&actual_sha256) {
        Ok(())
    } else {
        Err(LauncherError::InvalidInput(format!(
            "Java runtime checksum mismatch! Expected SHA-256 {expected_sha256}, got {actual_sha256}"
        )))
    }
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

    /// The Adoptium metadata API shape we depend on: `[{ binary: { package:
    /// { checksum, link } } }]`. A wrong shape here silently breaks supply-chain
    /// verification, so the parse contract is pinned by a test.
    #[test]
    fn parses_adoptium_metadata_response() {
        let body = r#"[
            {
                "release_name": "jdk-21.0.5+11",
                "binary": {
                    "os": "windows",
                    "architecture": "x64",
                    "package": {
                        "checksum": "a1b2c3d4e5f60718293a4b5c6d7e8f90a1b2c3d4e5f60718293a4b5c6d7e8f90",
                        "link": "https://github.com/adoptium/temurin21-binaries/releases/download/jdk-21.0.5%2B11/OpenJDK21U-jdk_x64_windows_hotspot_21.0.5_11.zip",
                        "name": "OpenJDK21U-jdk_x64_windows_hotspot_21.0.5_11.zip",
                        "size": 190000000
                    }
                }
            }
        ]"#;
        let releases: Vec<AdoptiumRelease> = serde_json::from_str(body).unwrap();
        let first = releases.into_iter().next().expect("one release");
        assert!(first.binary.package.link.contains("github.com/adoptium"));
        assert_eq!(64, first.binary.package.checksum.len());
        assert!(first
            .binary
            .package
            .checksum
            .chars()
            .all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn adoptium_metadata_url_passes_ssrf_guard() {
        let url = adoptium_metadata_url(21);
        assert!(
            url.starts_with("https://api.adoptium.net/v3/assets/latest/21/hotspot?architecture=")
        );
        assert!(
            zircon_core::security::ssrf::is_safe_cdn_url(&url),
            "metadata URL must pass the CDN SSRF guard: {url}"
        );
    }

    #[tokio::test]
    async fn sha256_verification_rejects_corrupted_archive() {
        let dir =
            std::env::temp_dir().join(format!("zircon-java-sha256-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let archive = dir.join("fake-jdk.zip");
        let contents = b"not a real JDK archive";
        std::fs::write(&archive, contents).unwrap();

        let good = hex::encode(Sha256::digest(contents));
        let bad = hex::encode(Sha256::digest(b"something else entirely"));

        // Matching checksum (any case) passes.
        assert!(verify_sha256(&archive, &good).await.is_ok());
        assert!(verify_sha256(&archive, &good.to_uppercase()).await.is_ok());

        // Mismatch fails with a descriptive InvalidInput error.
        let err = verify_sha256(&archive, &bad).await.unwrap_err();
        assert!(matches!(err, LauncherError::InvalidInput(_)), "{err:?}");

        let _ = std::fs::remove_dir_all(&dir);
    }
}
