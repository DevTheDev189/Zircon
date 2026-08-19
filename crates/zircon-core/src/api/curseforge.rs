//! Client for the CurseForge API (v1).
//!
//! All requests require the `x-api-key` header. CurseForge identifies files by
//! MurmurHash3 "fingerprints" (see `crypto::murmur3`).
//!
//! Port of `com.mcmanager.core.api.CurseForgeApiClient`.

use serde::{Deserialize, Serialize};

use super::ApiError;

pub const BASE_URL: &str = "https://api.curseforge.com/v1";
pub const MINECRAFT_GAME_ID: i64 = 432;

/// Client for the CurseForge API (v1).
#[derive(Debug, Clone)]
pub struct CurseForgeApiClient {
    client: reqwest::Client,
    api_key: String,
}

impl CurseForgeApiClient {
    pub fn new(api_key: impl Into<String>) -> Self {
        let client = reqwest::Client::builder()
            .connect_timeout(std::time::Duration::from_secs(15))
            .redirect(reqwest::redirect::Policy::limited(10))
            .build()
            .expect("failed to build reqwest client");
        Self {
            client,
            api_key: api_key.into(),
        }
    }

    /// Batch-verifies MurmurHash3 fingerprints against CurseForge.
    ///
    /// Returns the exact matches as `CurseForgeFile` objects; an empty list
    /// means none of the fingerprints are known to CurseForge.
    pub async fn verify_fingerprints(
        &self,
        fingerprint_list: &[u64],
    ) -> Result<Vec<CurseForgeFile>, ApiError> {
        if fingerprint_list.is_empty() {
            return Ok(Vec::new());
        }
        let body = serde_json::json!({ "fingerprints": fingerprint_list });
        let text = self
            .post_json(&format!("{BASE_URL}/fingerprints"), &body)
            .await?;

        let root: serde_json::Value = serde_json::from_str(&text)?;
        let mut matches = Vec::new();
        if let Some(exact) = root
            .get("data")
            .and_then(|d| d.get("exactMatches"))
            .and_then(|e| e.as_array())
        {
            for element in exact {
                if let Some(file) = element.get("file") {
                    if let Ok(file) = serde_json::from_value::<CurseForgeFile>(file.clone()) {
                        matches.push(file);
                    }
                }
            }
        }
        Ok(matches)
    }

    /// Searches CurseForge mods for Minecraft.
    pub async fn search_mods(
        &self,
        query: &str,
        mc_version: Option<&str>,
    ) -> Result<Vec<CurseForgeMod>, ApiError> {
        let mut url = format!(
            "{BASE_URL}/mods/search?gameId={MINECRAFT_GAME_ID}&searchFilter={}&sortField=1&sortOrder=desc&pageSize=25",
            form_encode(query)
        );
        if let Some(v) = mc_version {
            url.push_str(&format!("&gameVersion={}", form_encode(v)));
        }

        let text = self.get(&url).await?;
        Ok(parse_data_data(&text))
    }

    /// Lists all files of a CurseForge mod, so the admin UI can pick which
    /// file to install for the target Minecraft version.
    pub async fn list_mod_files(&self, mod_id: i64) -> Result<Vec<CurseForgeFile>, ApiError> {
        let text = self
            .get(&format!("{BASE_URL}/mods/{mod_id}/files?pageSize=50"))
            .await?;
        Ok(parse_data_data(&text))
    }

    // ----------------------------------------------------------------------

    async fn get(&self, url: &str) -> Result<String, ApiError> {
        let response = self
            .client
            .get(url)
            .header("x-api-key", &self.api_key)
            .header("Accept", "application/json")
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
            .header("x-api-key", &self.api_key)
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

/// CurseForge wraps list payloads as `{"data": {"data": [...]}}`.
fn parse_data_data<T: serde::de::DeserializeOwned>(text: &str) -> Vec<T> {
    let root: serde_json::Value = match serde_json::from_str(text) {
        Ok(root) => root,
        Err(_) => return Vec::new(),
    };
    root.get("data")
        .and_then(|d| d.get("data"))
        .and_then(|d| d.as_array())
        .and_then(|arr| serde_json::from_value(serde_json::Value::Array(arr.clone())).ok())
        .unwrap_or_default()
}

// --------------------------------------------------------------------------
// Response DTOs
// --------------------------------------------------------------------------

/// A CurseForge mod (project) returned by the search endpoint.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CurseForgeMod {
    pub id: i64,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub slug: String,
    #[serde(default)]
    pub summary: String,
    #[serde(default)]
    pub download_count: u64,
    #[serde(default)]
    pub game_versions: Vec<String>,
    #[serde(default)]
    pub links: Option<CurseForgeLinks>,
    #[serde(default)]
    pub latest_files: Vec<CurseForgeFile>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CurseForgeLinks {
    #[serde(default)]
    pub website_url: String,
}

/// A CurseForge file (a concrete downloadable artifact of a mod).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CurseForgeFile {
    pub id: i64,
    #[serde(default)]
    pub display_name: String,
    #[serde(default)]
    pub file_name: String,
    #[serde(default)]
    pub download_url: String,
    #[serde(default)]
    pub file_fingerprint: u64,
    #[serde(default)]
    pub length: u64,
    #[serde(default)]
    pub hashes: Vec<CurseForgeFileHash>,
}

/// A hash entry in CurseForge's file metadata.
/// Algo enum in CurseForge v1: 1 = SHA-1, 2 = MD5.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CurseForgeFileHash {
    pub value: String,
    pub algo: i32,
}

impl CurseForgeFile {
    /// Returns the SHA-1 hash (algo == 1) if present in the metadata.
    pub fn sha1(&self) -> Option<&str> {
        self.hashes
            .iter()
            .find(|h| h.algo == 1)
            .map(|h| h.value.as_str())
    }

    /// Returns the MD5 hash (algo == 2) if present in the metadata.
    pub fn md5(&self) -> Option<&str> {
        self.hashes
            .iter()
            .find(|h| h.algo == 2)
            .map(|h| h.value.as_str())
    }
}

fn form_encode(value: &str) -> String {
    url::form_urlencoded::byte_serialize(value.as_bytes()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_nested_data_payload() {
        let json = r#"{
            "data": {
                "data": [
                    {"id": 1, "displayName": "One", "fileName": "one.jar", "fileFingerprint": 123, "length": 10},
                    {"id": 2, "displayName": "Two", "fileName": "two.jar", "fileFingerprint": 456, "length": 20}
                ]
            }
        }"#;
        let files: Vec<CurseForgeFile> = parse_data_data(json);
        assert_eq!(2, files.len());
        assert_eq!("One", files[0].display_name);
        assert_eq!(456, files[1].file_fingerprint);
    }

    #[test]
    fn parses_fingerprint_exact_matches() {
        let json = r#"{
            "data": {
                "exactMatches": [
                    {"id": 1, "file": {"id": 99, "displayName": "Matched", "fileName": "m.jar",
                                       "downloadUrl": "https://edge.forgecdn.net/f/m.jar",
                                       "fileFingerprint": 424242, "length": 512}}
                ]
            }
        }"#;
        let root: serde_json::Value = serde_json::from_str(json).unwrap();
        let file: CurseForgeFile = root
            .get("data")
            .and_then(|d| d.get("exactMatches"))
            .and_then(|e| e.as_array())
            .and_then(|a| a.first())
            .and_then(|m| m.get("file"))
            .and_then(|f| serde_json::from_value(f.clone()).ok())
            .unwrap();
        assert_eq!(99, file.id);
        assert_eq!(424242, file.file_fingerprint);
    }

    #[test]
    fn camel_case_field_mapping() {
        let json = r#"{"id": 1, "displayName": "X", "downloadUrl": "https://edge.forgecdn.net/x.jar",
                        "fileFingerprint": 1, "length": 2, "fileName": "x.jar"}"#;
        let file: CurseForgeFile = serde_json::from_str(json).unwrap();
        assert_eq!("X", file.display_name);
        assert_eq!("x.jar", file.file_name);
        assert_eq!("https://edge.forgecdn.net/x.jar", file.download_url);
    }

    #[test]
    fn parses_file_hashes_and_extracts_sha1() {
        let json = r#"{
            "id": 12345,
            "displayName": "JEI 15.2.0.27",
            "fileName": "jei-1.20.4-15.2.0.27.jar",
            "downloadUrl": "https://edge.forgecdn.net/files/123/456/jei.jar",
            "fileFingerprint": 2252075348,
            "length": 1024,
            "hashes": [
                { "value": "a1b2c3d4e5f60718293041526374859607182930", "algo": 1 },
                { "value": "0123456789abcdef0123456789abcdef", "algo": 2 }
            ]
        }"#;
        let file: CurseForgeFile = serde_json::from_str(json).unwrap();
        assert_eq!(
            Some("a1b2c3d4e5f60718293041526374859607182930"),
            file.sha1()
        );
        assert_eq!(Some("0123456789abcdef0123456789abcdef"), file.md5());
    }
}
