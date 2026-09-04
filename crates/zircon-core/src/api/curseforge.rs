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

pub const CLASS_BUKKIT_PLUGINS: i64 = 5;
pub const CLASS_MODS: i64 = 6;
pub const CLASS_RESOURCE_PACKS: i64 = 12;
pub const CLASS_WORLDS: i64 = 17;
pub const CLASS_MODPACKS: i64 = 4471;
pub const CLASS_SHADERS: i64 = 6552;

pub fn class_id_for_type(project_type: Option<&str>) -> i64 {
    match project_type.map(|t| t.to_ascii_lowercase()).as_deref() {
        Some("modpack") | Some("modpacks") => CLASS_MODPACKS,
        Some("shader") | Some("shaders") | Some("shaderpack") | Some("shaderpacks") => CLASS_SHADERS,
        Some("resourcepack") | Some("resourcepacks") | Some("texturepack") | Some("texturepacks") => CLASS_RESOURCE_PACKS,
        Some("plugin") | Some("plugins") | Some("bukkit") => CLASS_BUKKIT_PLUGINS,
        Some("world") | Some("worlds") => CLASS_WORLDS,
        _ => CLASS_MODS,
    }
}

impl CurseForgeApiClient {
    pub fn new(api_key: impl Into<String>) -> Self {
        let key_str = api_key.into();
        let cleaned_key = key_str.trim().trim_matches('"').trim_matches('\'').to_string();
        let client = reqwest::Client::builder()
            .user_agent("Zircon-Server/0.2.5 (https://github.com/DevTheDev189/Zircon)")
            .connect_timeout(std::time::Duration::from_secs(15))
            .redirect(reqwest::redirect::Policy::limited(10))
            .build()
            .expect("failed to build reqwest client");
        Self {
            client,
            api_key: cleaned_key,
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

    /// Searches CurseForge for Minecraft projects with specific classId (mods, modpacks, shaders, resourcepacks)
    /// and modLoaderType.
    pub async fn search_mods_with_type(
        &self,
        query: &str,
        mc_version: Option<&str>,
        loader: Option<&str>,
        project_type: Option<&str>,
    ) -> Result<Vec<CurseForgeMod>, ApiError> {
        let class_id = class_id_for_type(project_type);
        let mut url = format!(
            "{BASE_URL}/mods/search?gameId={MINECRAFT_GAME_ID}&classId={class_id}&searchFilter={}&sortField=1&sortOrder=desc&pageSize=25",
            form_encode(query)
        );
        if let Some(v) = mc_version {
            let v = v.trim();
            if !v.is_empty() {
                url.push_str(&format!("&gameVersion={}", form_encode(v)));
            }
        }
        if let Some(l) = loader {
            let mod_loader_type = match l.to_ascii_lowercase().as_str() {
                "forge" => Some(1),
                "cauldron" => Some(2),
                "liteloader" => Some(3),
                "fabric" => Some(4),
                "quilt" => Some(5),
                "neoforge" => Some(6),
                _ => None,
            };
            if let Some(lt) = mod_loader_type {
                url.push_str(&format!("&modLoaderType={lt}"));
            }
        }

        tracing::info!("Querying CurseForge API: {url}");
        let text = self.get(&url).await?;
        let parsed: Vec<CurseForgeMod> = parse_data_data(&text);
        tracing::info!(
            "CurseForge API responded ({} bytes) -> parsed {} hit(s) for classId {}",
            text.len(),
            parsed.len(),
            class_id
        );
        Ok(parsed)
    }

    /// Searches CurseForge mods for Minecraft (defaults to Mods, classId 6).
    pub async fn search_mods(
        &self,
        query: &str,
        mc_version: Option<&str>,
    ) -> Result<Vec<CurseForgeMod>, ApiError> {
        self.search_mods_with_type(query, mc_version, None, Some("mod")).await
    }

    /// Lists all files of a CurseForge mod, so the admin UI can pick which
    /// file to install for the target Minecraft version.
    pub async fn list_mod_files(&self, mod_id: i64) -> Result<Vec<CurseForgeFile>, ApiError> {
        let text = self
            .get(&format!("{BASE_URL}/mods/{mod_id}/files?pageSize=50"))
            .await?;
        Ok(parse_data_data(&text))
    }

    /// Fetches metadata for a specific file belonging to a CurseForge mod.
    pub async fn get_mod_file(&self, mod_id: i64, file_id: i64) -> Result<CurseForgeFile, ApiError> {
        let text = self
            .get(&format!("{BASE_URL}/mods/{mod_id}/files/{file_id}"))
            .await?;
        let root: serde_json::Value = serde_json::from_str(&text)?;
        if let Some(file_obj) = root.get("data") {
            let file: CurseForgeFile = serde_json::from_value(file_obj.clone())?;
            return Ok(file);
        }
        Err(ApiError::Status {
            status: 404,
            body: format!("CurseForge file {file_id} for mod {mod_id} not found"),
        })
    }

    /// Fetches full metadata for a single CurseForge mod by its project ID.
    pub async fn get_mod(&self, mod_id: i64) -> Result<CurseForgeMod, ApiError> {
        let text = self.get(&format!("{BASE_URL}/mods/{mod_id}")).await?;
        let root: serde_json::Value = serde_json::from_str(&text)?;
        if let Some(mod_obj) = root.get("data") {
            let m: CurseForgeMod = serde_json::from_value(mod_obj.clone())?;
            return Ok(m);
        }
        Err(ApiError::Status {
            status: 404,
            body: format!("CurseForge mod {mod_id} not found"),
        })
    }

    /// Batch fetches file metadata for multiple CurseForge file IDs.
    pub async fn get_files(&self, file_ids: &[i64]) -> Result<Vec<CurseForgeFile>, ApiError> {
        if file_ids.is_empty() {
            return Ok(Vec::new());
        }
        let body = serde_json::json!({ "fileIds": file_ids });
        let text = self.post_json(&format!("{BASE_URL}/mods/files"), &body).await?;
        let root: serde_json::Value = serde_json::from_str(&text)?;
        if let Some(arr) = root.get("data").and_then(|d| d.as_array()) {
            let files: Vec<CurseForgeFile> = serde_json::from_value(serde_json::Value::Array(arr.clone()))?;
            return Ok(files);
        }
        Ok(Vec::new())
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

/// CurseForge wraps list payloads as `{"data": [...]}` or `{"data": {"data": [...]}}`.
fn parse_data_data<T: serde::de::DeserializeOwned>(text: &str) -> Vec<T> {
    let root: serde_json::Value = match serde_json::from_str(text) {
        Ok(root) => root,
        Err(e) => {
            tracing::error!("Failed to parse CurseForge JSON response: {e}");
            return Vec::new();
        }
    };
    if let Some(arr) = root.get("data").and_then(|d| d.as_array()) {
        match serde_json::from_value::<Vec<T>>(serde_json::Value::Array(arr.clone())) {
            Ok(vec) => return vec,
            Err(e) => {
                tracing::error!("Failed to deserialize CurseForge data array: {e}");
            }
        }
    }
    if let Some(arr) = root
        .get("data")
        .and_then(|d| d.get("data"))
        .and_then(|d| d.as_array())
    {
        match serde_json::from_value::<Vec<T>>(serde_json::Value::Array(arr.clone())) {
            Ok(vec) => return vec,
            Err(e) => {
                tracing::error!("Failed to deserialize nested CurseForge data array: {e}");
            }
        }
    }
    Vec::new()
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
    pub links: Option<CurseForgeLinks>,
    #[serde(default)]
    pub logo: Option<CurseForgeLogo>,
    #[serde(default)]
    pub latest_files: Vec<CurseForgeFile>,
    #[serde(default)]
    pub authors: Vec<CurseForgeAuthor>,
}

impl CurseForgeMod {
    /// Comma-joined author names, e.g. "jellysquid3, grum".
    pub fn authors_string(&self) -> String {
        self.authors
            .iter()
            .map(|a| a.name.as_str())
            .filter(|n| !n.trim().is_empty())
            .collect::<Vec<_>>()
            .join(", ")
    }
}

/// A CurseForge mod author.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CurseForgeAuthor {
    pub id: Option<i64>,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub url: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CurseForgeLinks {
    #[serde(default)]
    pub website_url: Option<String>,
    #[serde(default)]
    pub wiki_url: Option<String>,
    #[serde(default)]
    pub issues_url: Option<String>,
    #[serde(default)]
    pub source_url: Option<String>,
}

/// A CurseForge mod logo asset.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CurseForgeLogo {
    pub id: Option<i64>,
    #[serde(default)]
    pub mod_id: Option<i64>,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub thumbnail_url: String,
    #[serde(default)]
    pub url: String,
}

/// A CurseForge file (a concrete downloadable artifact of a mod).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CurseForgeFile {
    pub id: i64,
    #[serde(default)]
    pub mod_id: i64,
    #[serde(default)]
    pub display_name: String,
    #[serde(default)]
    pub file_name: String,
    #[serde(default, deserialize_with = "deserialize_optional_string")]
    pub download_url: String,
    #[serde(default)]
    pub file_fingerprint: u64,
    #[serde(default, alias = "fileLength")]
    pub length: u64,
    #[serde(default)]
    pub hashes: Vec<CurseForgeFileHash>,
    #[serde(default)]
    pub game_versions: Vec<String>,
}

fn deserialize_optional_string<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let opt: Option<String> = serde::Deserialize::deserialize(deserializer)?;
    Ok(opt.unwrap_or_default())
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
