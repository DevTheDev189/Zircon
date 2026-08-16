//! Fabric/Quilt loader meta resolution: newest-stable loader version lookup and
//! the loader profile (`libraries` + `mainClass`) merged into the vanilla
//! launch, with loader-provided libraries replacing vanilla ones by
//! `group:artifact` — Fabric's Knot loader refuses to start with duplicate
//! classes on the classpath.
//!
//! Port of com.mcmanager.client.launch.MinecraftClasspathBuilder.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::error::LauncherError;
use crate::launch::classpath::{
    group_and_artifact, maven_path, rules_allow, FABRIC_META_URL, QUILT_META_URL,
};

/// Queries the loader meta API for the newest stable loader version of a
/// Minecraft version (responses are ordered newest-first).
///
/// Returns `None` when no loader version exists for the Minecraft version.
pub async fn resolve_latest_loader_version(
    mc_version: &str,
    loader_type: &str,
) -> Result<Option<String>, LauncherError> {
    let client = reqwest::Client::new();
    resolve_latest_loader_version_with(&client, mc_version, loader_type).await
}

/// Resolves the loader profile for `mc_version` + `loader_version`, downloads
/// its libraries into `libraries_dir` and records them in `library_by_artifact`
/// keyed by `group:artifact` (loader libraries override vanilla ones with the
/// same key). Returns the profile's `mainClass`.
///
/// When `loader_version` is `None` (or blank) the newest stable loader version
/// is auto-resolved from the meta API. `cache_dir` is accepted for symmetry
/// with the Java signature but is not used — the profile is always fetched
/// fresh from the meta API.
///
/// This synchronous entry point is called from the async launcher pipeline: it
/// parks the calling task on the ambient tokio multi-thread runtime while the
/// meta requests run. Callers outside a runtime (or on a single-threaded one)
/// get a dedicated runtime instead.
pub fn resolve_loader_profile(
    mc_version: &str,
    loader_type: &str,
    loader_version: Option<&str>,
    cache_dir: &Path,
    library_by_artifact: &mut BTreeMap<String, PathBuf>,
    libraries_dir: &Path,
) -> Result<String, LauncherError> {
    let work = async {
        resolve_loader_profile_async(
            mc_version,
            loader_type,
            loader_version,
            cache_dir,
            library_by_artifact,
            libraries_dir,
        )
        .await
    };
    match tokio::runtime::Handle::try_current() {
        Ok(handle) if handle.runtime_flavor() == tokio::runtime::RuntimeFlavor::MultiThread => {
            tokio::task::block_in_place(|| handle.block_on(work))
        }
        _ => {
            let runtime = tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()
                .map_err(|e| {
                    LauncherError::Process(format!("failed to start loader meta runtime: {e}"))
                })?;
            runtime.block_on(work)
        }
    }
}

/// The async core of [`resolve_loader_profile`].
async fn resolve_loader_profile_async(
    mc_version: &str,
    loader_type: &str,
    loader_version: Option<&str>,
    _cache_dir: &Path,
    library_by_artifact: &mut BTreeMap<String, PathBuf>,
    libraries_dir: &Path,
) -> Result<String, LauncherError> {
    let client = reqwest::Client::new();

    // Be forgiving: auto-resolve the newest stable loader for this MC version
    // when the BOM left the version empty.
    let loader_version = match loader_version {
        Some(version) if !version.trim().is_empty() => version.to_string(),
        _ => {
            let resolved =
                resolve_latest_loader_version_with(&client, mc_version, loader_type).await?;
            match resolved {
                Some(version) => {
                    tracing::info!(
                        "Auto-resolved {loader_type} loader version {version} for MC {mc_version}"
                    );
                    version
                }
                None => {
                    return Err(LauncherError::NotFound(format!(
                        "Loader version is empty in the BOM for {loader_type} and could not be \
                         auto-resolved from {}",
                        loader_base_url(loader_type)
                    )));
                }
            }
        }
    };

    let base = loader_base_url(loader_type);
    let url = format!("{base}/versions/loader/{mc_version}/{loader_version}/profile/json");
    let profile_json = get(&client, &url).await?;
    let profile: serde_json::Value = serde_json::from_str(&profile_json)?;

    merge_profile_libraries(&profile, library_by_artifact, libraries_dir, &client).await?;

    let main_class = profile
        .get("mainClass")
        .and_then(|m| m.as_str())
        .ok_or_else(|| LauncherError::Parse("loader profile missing mainClass".to_string()))?
        .to_string();
    tracing::info!(
        "{loader_type} loader profile resolved: mainClass={main_class}, {} libraries",
        library_by_artifact.len()
    );
    Ok(main_class)
}

/// Downloads every allowed library of a loader profile into `libraries_dir`
/// and records `group:artifact -> jar` in `library_by_artifact`, overriding any
/// vanilla library with the same key.
async fn merge_profile_libraries(
    profile: &serde_json::Value,
    library_by_artifact: &mut BTreeMap<String, PathBuf>,
    libraries_dir: &Path,
    client: &reqwest::Client,
) -> Result<(), LauncherError> {
    let Some(libraries) = profile.get("libraries").and_then(|l| l.as_array()) else {
        return Ok(());
    };
    for element in libraries {
        if !rules_allow(element) {
            continue;
        }
        let Some(name) = element.get("name").and_then(|n| n.as_str()) else {
            continue;
        };
        // Port of `lib.has("url") ? url : "https://maven.fabricmc.net/"`.
        let repo = element
            .get("url")
            .and_then(|u| u.as_str())
            .unwrap_or("https://maven.fabricmc.net/");
        let rel = maven_path(name);
        let jar = libraries_dir.join(&rel);
        download_if_missing(client, &format!("{repo}{rel}"), &jar).await?;
        // Loader-provided libraries take precedence over vanilla ones with the
        // same group:artifact (e.g. a newer ASM required by Fabric Loader).
        library_by_artifact.insert(group_and_artifact(name), jar);
    }
    Ok(())
}

/// The meta API root for a loader type (`quilt` -> Quilt, anything else ->
/// Fabric).
fn loader_base_url(loader_type: &str) -> &'static str {
    if loader_type.eq_ignore_ascii_case("quilt") {
        QUILT_META_URL
    } else {
        FABRIC_META_URL
    }
}

/// Extracts the newest loader version from a `GET {base}/versions/loader/{mc}`
/// response body (an array ordered newest-first).
fn parse_latest_loader_version(body: &str) -> Option<String> {
    let versions: serde_json::Value = serde_json::from_str(body).ok()?;
    let first = versions.as_array()?.first()?;
    first
        .get("loader")?
        .get("version")?
        .as_str()
        .map(str::to_string)
}

async fn resolve_latest_loader_version_with(
    client: &reqwest::Client,
    mc_version: &str,
    loader_type: &str,
) -> Result<Option<String>, LauncherError> {
    let base = loader_base_url(loader_type);
    let body = get(client, &format!("{base}/versions/loader/{mc_version}")).await?;
    Ok(parse_latest_loader_version(&body))
}

/// GETs `url` and returns the response body as text.
async fn get(client: &reqwest::Client, url: &str) -> Result<String, LauncherError> {
    let response = client.get(url).send().await?;
    if !response.status().is_success() {
        return Err(LauncherError::Http {
            status: response.status().as_u16(),
            url: url.to_string(),
        });
    }
    Ok(response.text().await?)
}

/// Downloads `url` to `target` unless a non-empty regular file already exists
/// there.
async fn download_if_missing(
    client: &reqwest::Client,
    url: &str,
    target: &Path,
) -> Result<(), LauncherError> {
    if target.is_file() && std::fs::metadata(target).map(|m| m.len()).unwrap_or(0) > 0 {
        return Ok(());
    }
    if let Some(parent) = target.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    let response = client.get(url).send().await?;
    if !response.status().is_success() {
        return Err(LauncherError::Http {
            status: response.status().as_u16(),
            url: url.to_string(),
        });
    }
    let bytes = response.bytes().await?;
    tokio::fs::write(target, &bytes).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicUsize, Ordering};

    /// Unique, self-cleaning temp directory for deterministic tests.
    struct TempDir(PathBuf);

    impl TempDir {
        fn new(tag: &str) -> Self {
            static COUNTER: AtomicUsize = AtomicUsize::new(0);
            let n = COUNTER.fetch_add(1, Ordering::Relaxed);
            let dir = std::env::temp_dir().join(format!(
                "zircon-fabric-quilt-{tag}-{}-{n}",
                std::process::id()
            ));
            let _ = std::fs::remove_dir_all(&dir);
            std::fs::create_dir_all(&dir).unwrap();
            TempDir(dir)
        }

        fn path(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    /// Pre-stages an empty-but-non-empty jar under `libraries_dir` at the
    /// Maven path of `name` so downloads are skipped (no network in tests).
    fn stage_jars(libraries_dir: &Path, names: &[&str]) {
        for name in names {
            let rel = maven_path(name);
            let path = libraries_dir.join(rel);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(&path, b"fixture-jar").unwrap();
        }
    }

    #[test]
    fn parses_newest_loader_version_from_meta_response() {
        let body = r#"[
            { "loader": { "version": "0.16.9", "stable": true }, "intermediary": { "version": "4.1.0" } },
            { "loader": { "version": "0.15.11", "stable": true }, "intermediary": { "version": "4.1.0" } }
        ]"#;
        // Newest-first: the first entry wins.
        assert_eq!(
            Some("0.16.9".to_string()),
            parse_latest_loader_version(body)
        );
    }

    #[test]
    fn parses_no_loader_version_for_empty_or_malformed_responses() {
        assert_eq!(None, parse_latest_loader_version("[]"));
        assert_eq!(None, parse_latest_loader_version("not json"));
        assert_eq!(
            None,
            parse_latest_loader_version(r#"[{ "intermediary": { "version": "4.1.0" } }]"#)
        );
    }

    #[tokio::test]
    async fn loader_profile_libraries_override_vanilla_by_group_artifact() {
        let dir = TempDir::new("merge");
        let libraries_dir = dir.path().join("libraries");
        std::fs::create_dir_all(&libraries_dir).unwrap();

        // Pre-stage the loader jars so download_if_missing skips the network.
        let loader_rel = maven_path("net.fabricmc:fabric-loader:0.15.11");
        let asm_rel = maven_path("org.ow2.asm:asm:9.5");
        stage_jars(
            &libraries_dir,
            &[
                "net.fabricmc:fabric-loader:0.15.11",
                "org.ow2.asm:asm:9.5",
                "com.example:windows-only:1.0",
            ],
        );

        // Vanilla already pulled a shared dependency at an older version.
        let mut library_by_artifact: BTreeMap<String, PathBuf> = BTreeMap::new();
        library_by_artifact.insert(
            "org.ow2.asm:asm".to_string(),
            libraries_dir.join(maven_path("org.ow2.asm:asm:9.2")),
        );

        // Local fixture of a Fabric loader profile JSON.
        let profile = serde_json::json!({
            "id": "fabric-loader-0.15.11-1.20.4",
            "mainClass": "net.fabricmc.loader.impl.launch.knot.KnotClient",
            "libraries": [
                { "name": "net.fabricmc:fabric-loader:0.15.11", "url": "https://maven.fabricmc.net/" },
                { "name": "org.ow2.asm:asm:9.5" },
                {
                    "name": "com.example:windows-only:1.0",
                    "rules": [{ "action": "allow", "os": { "name": "windows" } }]
                }
            ]
        });

        let client = reqwest::Client::new();
        merge_profile_libraries(&profile, &mut library_by_artifact, &libraries_dir, &client)
            .await
            .unwrap();

        // The loader version replaced the vanilla one for the shared
        // group:artifact key...
        assert_eq!(
            libraries_dir.join(&asm_rel),
            *library_by_artifact.get("org.ow2.asm:asm").unwrap()
        );
        // ...and the loader-only library was added.
        assert_eq!(
            libraries_dir.join(&loader_rel),
            *library_by_artifact
                .get("net.fabricmc:fabric-loader")
                .unwrap()
        );
        // Rules-gated libraries follow the host (deterministic assertion).
        assert_eq!(
            cfg!(target_os = "windows"),
            library_by_artifact.contains_key("com.example:windows-only")
        );
    }
}
