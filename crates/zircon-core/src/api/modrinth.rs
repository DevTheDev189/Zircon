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
        let root: serde_json::Value = serde_json::from_str(&text)?;
        let hits: Vec<ModrinthSearchHit> = root
            .get("hits")
            .and_then(|h| serde_json::from_value(h.clone()).ok())
            .unwrap_or_default();
        Ok(hits)
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
// --------------------------------------------------------------------------

/// Public project metadata from `GET /project/{id}`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
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
#[serde(rename_all = "camelCase")]
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
#[serde(rename_all = "camelCase")]
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
#[serde(rename_all = "camelCase")]
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
#[serde(rename_all = "camelCase")]
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
}
