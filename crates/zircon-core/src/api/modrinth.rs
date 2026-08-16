//! Client for the Modrinth API (v2).
//!
//! Modrinth requires a descriptive `User-Agent` header.
//!
//! Port of `com.mcmanager.core.api.ModrinthApiClient`.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use super::ApiError;

pub const BASE_URL: &str = "https://api.modrinth.com/v2";
pub const DEFAULT_USER_AGENT: &str = "McManager/1.0.0 (contact@example.com)";

/// Client for the Modrinth API (v2).
#[derive(Debug, Clone)]
pub struct ModrinthApiClient {
    client: reqwest::Client,
    user_agent: String,
}

impl Default for ModrinthApiClient {
    fn default() -> Self {
        Self::new()
    }
}

impl ModrinthApiClient {
    pub fn new() -> Self {
        Self::with_user_agent(DEFAULT_USER_AGENT)
    }

    pub fn with_user_agent(user_agent: &str) -> Self {
        let client = reqwest::Client::builder()
            .connect_timeout(std::time::Duration::from_secs(15))
            .redirect(reqwest::redirect::Policy::limited(10))
            .build()
            .expect("failed to build reqwest client");
        Self {
            client,
            user_agent: user_agent.to_string(),
        }
    }

    /// Batch-verifies SHA-1 hashes against Modrinth's file database.
    ///
    /// Returns a map of hash → `ModrinthVersion` for every hash Modrinth
    /// recognises. Hashes missing from the returned map are not known to
    /// Modrinth (and therefore not verified).
    pub async fn verify_hashes(
        &self,
        sha1_list: &[String],
    ) -> Result<HashMap<String, ModrinthVersion>, ApiError> {
        if sha1_list.is_empty() {
            return Ok(HashMap::new());
        }
        let body = serde_json::json!({
            "hashes": sha1_list,
            "algorithm": "sha1",
        });
        let text = self
            .post_json(&format!("{BASE_URL}/version_files"), &body)
            .await?;
        let result: HashMap<String, ModrinthVersion> = serde_json::from_str(&text)?;
        Ok(result)
    }

    /// Searches Modrinth for mods matching a query for the given game version
    /// and loader category.
    pub async fn search_mods(
        &self,
        query: &str,
        mc_version: Option<&str>,
        loader_type: Option<&str>,
    ) -> Result<Vec<ModrinthSearchHit>, ApiError> {
        self.search_mods_with_type(query, mc_version, loader_type, None)
            .await
    }

    /// Searches Modrinth, optionally restricting results to a `project_type`
    /// (e.g. `"mod"` or `"modpack"`).
    pub async fn search_mods_with_type(
        &self,
        query: &str,
        mc_version: Option<&str>,
        loader_type: Option<&str>,
        project_type: Option<&str>,
    ) -> Result<Vec<ModrinthSearchHit>, ApiError> {
        let mut url = format!("{BASE_URL}/search?query={}", form_encode(query));
        let mut facet_groups: Vec<String> = Vec::new();
        if let Some(v) = mc_version.filter(|v| !v.trim().is_empty()) {
            facet_groups.push(format!("[\"versions:{v}\"]"));
        }
        if let Some(l) = loader_type.filter(|l| !l.trim().is_empty()) {
            facet_groups.push(format!("[\"categories:{l}\"]"));
        }
        if let Some(p) = project_type.filter(|p| !p.trim().is_empty()) {
            facet_groups.push(format!("[\"project_type:{p}\"]"));
        }
        if !facet_groups.is_empty() {
            url.push_str(&format!(
                "&facets={}",
                form_encode(&format!("[{}]", facet_groups.join(",")))
            ));
        }
        url.push_str("&limit=25");

        let text = self.get(&url).await?;
        let response: SearchResponse = serde_json::from_str(&text)?;
        Ok(response.hits)
    }

    /// Fetches the stable (release) Minecraft versions known to Modrinth,
    /// newest first. Used to populate the launcher's game-version dropdown.
    pub async fn list_game_versions(&self) -> Result<Vec<String>, ApiError> {
        let text = self.get(&format!("{BASE_URL}/tag/game_version")).await?;
        let tags: Vec<TagEntry> = serde_json::from_str(&text)?;
        let mut releases: Vec<String> = tags
            .into_iter()
            .filter(|t| t.version_type.as_deref() == Some("release"))
            .filter_map(|t| t.version)
            .collect();
        releases.sort_by(|a, b| compare_versions(b, a)); // newest first
        Ok(releases)
    }

    /// Fetches the mod loader types Modrinth knows about (e.g. `"fabric"`,
    /// `"forge"`, `"neoforge"`, `"quilt"`).
    pub async fn list_loaders(&self) -> Result<Vec<String>, ApiError> {
        let text = self.get(&format!("{BASE_URL}/tag/loader")).await?;
        let tags: Vec<TagEntry> = serde_json::from_str(&text)?;
        let mut loaders: Vec<String> = tags.into_iter().filter_map(|t| t.name).collect();
        loaders.sort();
        Ok(loaders)
    }

    /// Lists published versions of a Modrinth project, optionally filtered by
    /// game version and loader. Used by the admin UI to pick a concrete
    /// version to install.
    pub async fn list_project_versions(
        &self,
        project_id: &str,
        mc_version: Option<&str>,
        loader_type: Option<&str>,
    ) -> Result<Vec<ModrinthVersion>, ApiError> {
        let mut url = format!("{BASE_URL}/project/{}/version", form_encode(project_id));
        let mut filters: Vec<String> = Vec::new();
        if let Some(v) = mc_version {
            filters.push(format!(
                "game_versions={}",
                form_encode(&format!("[\"{v}\"]"))
            ));
        }
        if let Some(l) = loader_type {
            filters.push(format!("loaders={}", form_encode(&format!("[\"{l}\"]"))));
        }
        if !filters.is_empty() {
            url.push('?');
            url.push_str(&filters.join("&"));
        }

        let text = self.get(&url).await?;
        let versions: Vec<ModrinthVersion> = serde_json::from_str(&text)?;
        Ok(versions)
    }

    /// Fetches the public metadata of a Modrinth project (title, description,
    /// icon, author). Used to enrich installed mod entries for the admin UI.
    pub async fn get_project(&self, project_id: &str) -> Result<ModrinthProject, ApiError> {
        let text = self
            .get(&format!("{BASE_URL}/project/{}", form_encode(project_id)))
            .await?;
        let project: ModrinthProject = serde_json::from_str(&text)?;
        Ok(project)
    }

    // ----------------------------------------------------------------------

    async fn get(&self, url: &str) -> Result<String, ApiError> {
        let response = self
            .client
            .get(url)
            .header("User-Agent", &self.user_agent)
            .send()
            .await?;
        let status = response.status();
        let text = response.text().await?;
        if !status.is_success() {
            return Err(ApiError::Status {
                status: status.as_u16(),
                body: text,
            });
        }
        Ok(text)
    }

    async fn post_json(&self, url: &str, body: &serde_json::Value) -> Result<String, ApiError> {
        let response = self
            .client
            .post(url)
            .header("User-Agent", &self.user_agent)
            .header("Content-Type", "application/json")
            .body(serde_json::to_string(body)?)
            .send()
            .await?;
        let status = response.status();
        let text = response.text().await?;
        if !status.is_success() {
            return Err(ApiError::Status {
                status: status.as_u16(),
                body: text,
            });
        }
        Ok(text)
    }
}

// --------------------------------------------------------------------------
// Response DTOs
//
// Modrinth's JSON uses snake_case field names (`project_id`, `icon_url`, ...),
// so deserialization reads snake_case while serialization keeps camelCase for
// Tauri IPC consumers.
// --------------------------------------------------------------------------

/// Response envelope of `GET /search`.
#[derive(Debug, Deserialize)]
struct SearchResponse {
    hits: Vec<ModrinthSearchHit>,
}

/// Public project metadata from `GET /project/{id}`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all(serialize = "camelCase", deserialize = "snake_case"))]
pub struct ModrinthProject {
    pub id: String,
    #[serde(default)]
    pub slug: String,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub icon_url: String,
    #[serde(default)]
    pub author: String,
    #[serde(default)]
    pub downloads: u64,
}

/// A specific version/file of a Modrinth project.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all(serialize = "camelCase", deserialize = "snake_case"))]
pub struct ModrinthVersion {
    pub id: String,
    pub project_id: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub version_number: String,
    #[serde(default)]
    pub game_versions: Vec<String>,
    #[serde(default)]
    pub loaders: Vec<String>,
    #[serde(default)]
    pub files: Vec<ModrinthFile>,
    #[serde(default)]
    pub url: String,
}

impl ModrinthVersion {
    /// The primary (downloadable) file of this version, or the first file.
    pub fn primary_file(&self) -> Option<&ModrinthFile> {
        self.files
            .iter()
            .find(|f| f.primary)
            .or_else(|| self.files.first())
    }
}

/// A downloadable file within a Modrinth version.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all(serialize = "camelCase", deserialize = "snake_case"))]
pub struct ModrinthFile {
    #[serde(default)]
    pub url: String,
    #[serde(default)]
    pub filename: String,
    #[serde(default)]
    pub hashes: HashMap<String, String>,
    #[serde(default)]
    pub size: u64,
    #[serde(default)]
    pub primary: bool,
}

impl ModrinthFile {
    pub fn sha1(&self) -> Option<&str> {
        self.hashes.get("sha1").map(|s| s.as_str())
    }
}

/// One hit from the Modrinth search endpoint.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all(serialize = "camelCase", deserialize = "snake_case"))]
pub struct ModrinthSearchHit {
    pub project_id: String,
    #[serde(default)]
    pub slug: String,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub author: String,
    #[serde(default)]
    pub downloads: u64,
    #[serde(default)]
    pub icon_url: String,
    #[serde(default)]
    pub versions: Vec<String>,
}

/// Internal shape of `GET /tag/game_version` and `GET /tag/loader` entries.
#[derive(Debug, Deserialize)]
struct TagEntry {
    #[serde(default)]
    version: Option<String>,
    #[serde(default)]
    version_type: Option<String>,
    #[serde(default)]
    name: Option<String>,
}

// --------------------------------------------------------------------------
// Helpers
// --------------------------------------------------------------------------

/// Form-url encoding matching Java's `URLEncoder` for the characters used in
/// query segments (quotes, brackets, colons, spaces, slashes...).
fn form_encode(value: &str) -> String {
    url::form_urlencoded::byte_serialize(value.as_bytes()).collect()
}

/// Numeric, dot/dash-separated version comparison (e.g. `1.21.4 > 1.8.9`,
/// `0.15.11 > 0.15.9`). Returns `Ordering` for `a` vs `b`.
fn compare_versions(a: &str, b: &str) -> std::cmp::Ordering {
    let pa: Vec<&str> = a.split(['.', '-']).collect();
    let pb: Vec<&str> = b.split(['.', '-']).collect();
    let n = pa.len().max(pb.len());
    for i in 0..n {
        let na = pa.get(i).map(|s| parse_version_segment(s)).unwrap_or(0);
        let nb = pb.get(i).map(|s| parse_version_segment(s)).unwrap_or(0);
        if na != nb {
            return na.cmp(&nb);
        }
    }
    std::cmp::Ordering::Equal
}

fn parse_version_segment(segment: &str) -> i64 {
    segment.parse::<i64>().unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_comparison_sorts_newest_first() {
        let mut versions = vec!["1.8.9", "1.21.4", "1.20.4", "0.15.9", "0.15.11"];
        versions.sort_by(|a, b| compare_versions(b, a));
        assert_eq!(
            vec!["1.21.4", "1.20.4", "1.8.9", "0.15.11", "0.15.9"],
            versions
        );
    }

    #[test]
    fn form_encoding_quotes_brackets_and_spaces() {
        let encoded = form_encode("[\"versions:1.20.4\"]");
        assert_eq!("%5B%22versions%3A1.20.4%22%5D", encoded);
    }

    #[tokio::test]
    #[ignore = "live network test"]
    async fn live_list_project_versions() {
        let client = ModrinthApiClient::new();
        let versions = client
            .list_project_versions("AANobbMI", Some("1.21.4"), Some("fabric"))
            .await
            .expect("live versions call failed");
        eprintln!(
            "got {} versions, first: {:?}",
            versions.len(),
            versions.first()
        );
        assert!(!versions.is_empty());
    }

    #[test]
    fn primary_file_prefers_primary_flag() {
        let version = ModrinthVersion {
            files: vec![
                ModrinthFile {
                    url: "second".to_string(),
                    primary: false,
                    ..Default::default()
                },
                ModrinthFile {
                    url: "primary".to_string(),
                    primary: true,
                    ..Default::default()
                },
            ],
            ..Default::default()
        };
        assert_eq!("primary", version.primary_file().unwrap().url);
    }

    #[test]
    fn parses_snake_case_search_hits() {
        // Fixture mirrors the real `GET /v2/search` response shape.
        let json = r#"{
            "hits": [{
                "project_id": "AANobbMI",
                "project_type": "mod",
                "slug": "sodium",
                "author": "jellysquid3",
                "title": "Sodium",
                "description": "A high-performance rendering engine replacement.",
                "categories": ["fabric"],
                "versions": ["1.21.4", "1.21.5"],
                "downloads": 207687903,
                "follows": 39873,
                "icon_url": "https://cdn.modrinth.com/data/AANobbMI/icon.png",
                "date_created": "2021-01-03T00:53:34.185936+00:00"
            }],
            "offset": 0,
            "limit": 25,
            "total_hits": 1
        }"#;
        let response: SearchResponse = serde_json::from_str(json).unwrap();
        assert_eq!(1, response.hits.len());
        let hit = &response.hits[0];
        assert_eq!("AANobbMI", hit.project_id);
        assert_eq!("sodium", hit.slug);
        assert_eq!(
            "https://cdn.modrinth.com/data/AANobbMI/icon.png",
            hit.icon_url
        );
        assert_eq!(vec!["1.21.4", "1.21.5"], hit.versions);
    }

    #[test]
    fn parses_snake_case_versions() {
        // Fixture mirrors the real `GET /v2/project/{id}/version` response.
        let json = r#"[
            {
                "id": "c3YkZvne",
                "project_id": "AANobbMI",
                "author_id": "TEZXhE2U",
                "name": "Sodium 0.6.13 for Fabric 1.21.4",
                "version_number": "mc1.21.4-0.6.13-fabric",
                "game_versions": ["1.21.4"],
                "loaders": ["fabric"],
                "version_type": "release",
                "status": "listed",
                "url": "https://modrinth.com/mod/sodium/version/c3YkZvne",
                "files": [{
                    "id": "Ya4LV6Qd",
                    "hashes": {"sha1": "c881d2db971207c396b5629632437f1520c0c478"},
                    "url": "https://cdn.modrinth.com/data/AANobbMI/versions/c3YkZvne/sodium-fabric-0.6.13+mc1.21.4.jar",
                    "filename": "sodium-fabric-0.6.13+mc1.21.4.jar",
                    "primary": true,
                    "size": 1306799
                }]
            }
        ]"#;
        let versions: Vec<ModrinthVersion> = serde_json::from_str(json).unwrap();
        assert_eq!("c3YkZvne", versions[0].id);
        assert_eq!("AANobbMI", versions[0].project_id);
        assert_eq!("mc1.21.4-0.6.13-fabric", versions[0].version_number);
        let file = versions[0].primary_file().unwrap();
        assert_eq!(
            "c881d2db971207c396b5629632437f1520c0c478",
            file.sha1().unwrap()
        );
    }

    #[test]
    fn parses_snake_case_project() {
        // Fixture mirrors the real `GET /v2/project/{id}` response.
        let json = r#"{
            "id": "AANobbMI",
            "slug": "sodium",
            "title": "Sodium",
            "description": "A high-performance rendering engine replacement.",
            "icon_url": "https://cdn.modrinth.com/data/AANobbMI/icon.png",
            "downloads": 207720463
        }"#;
        let project: ModrinthProject = serde_json::from_str(json).unwrap();
        assert_eq!("AANobbMI", project.id);
        assert_eq!(
            "https://cdn.modrinth.com/data/AANobbMI/icon.png",
            project.icon_url
        );
        assert_eq!(207720463, project.downloads);
    }

    #[test]
    fn parses_snake_case_game_version_tags() {
        // Fixture mirrors the real `GET /v2/tag/game_version` response.
        let json = r#"[
            {"version": "1.21.4", "version_type": "release", "major": true},
            {"version": "26.3-snapshot-1", "version_type": "snapshot", "major": false}
        ]"#;
        let tags: Vec<TagEntry> = serde_json::from_str(json).unwrap();
        assert_eq!(Some("release".to_string()), tags[0].version_type);
        assert_eq!(Some("snapshot".to_string()), tags[1].version_type);
    }
}
