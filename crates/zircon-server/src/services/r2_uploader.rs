//! Cloudflare R2 Content-Addressed Storage (CAS) client and backup engine.
//!
//! Provides zero-egress mod JAR deduplication and cold world backup archival
//! to Cloudflare R2 object storage using native AWS Signature Version 4 (SigV4)
//! authentication without external heavy SDK dependencies.

use std::io;
use std::path::Path;
use std::time::SystemTime;

use chrono::{DateTime, Utc};
use reqwest::{Client, Method, StatusCode};
use sha2::{Digest, Sha256};
use tracing::{info, warn};

/// Configuration for Cloudflare R2 object storage.
#[derive(Debug, Clone)]
pub struct R2CasConfig {
    pub endpoint: String,
    pub access_key_id: String,
    pub secret_access_key: String,
    pub region: String,
    pub mod_cache_bucket: String,
    pub backups_bucket: String,
    pub public_cdn_url: String,
}

impl Default for R2CasConfig {
    fn default() -> Self {
        Self {
            endpoint: std::env::var("R2_ENDPOINT").unwrap_or_default(),
            access_key_id: std::env::var("R2_ACCESS_KEY_ID").unwrap_or_default(),
            secret_access_key: std::env::var("R2_SECRET_ACCESS_KEY").unwrap_or_default(),
            region: std::env::var("R2_REGION").unwrap_or_else(|_| "auto".to_string()),
            mod_cache_bucket: std::env::var("R2_MOD_CACHE_BUCKET")
                .unwrap_or_else(|_| "zircon-mod-cache".to_string()),
            backups_bucket: std::env::var("R2_BACKUPS_BUCKET")
                .unwrap_or_else(|_| "zircon-backups".to_string()),
            public_cdn_url: std::env::var("R2_PUBLIC_CDN_DOMAIN")
                .map(|d| format!("https://{}", d.trim_start_matches("https://").trim_end_matches('/')))
                .unwrap_or_else(|_| "https://cdn.zirconmc.net".to_string()),
        }
    }
}

/// Client for Cloudflare R2 CAS mod deduplication and cold backups.
#[derive(Clone)]
pub struct R2CasClient {
    config: R2CasConfig,
    http_client: Client,
}

impl R2CasClient {
    pub fn new(config: R2CasConfig) -> Self {
        Self {
            config,
            http_client: Client::builder().build().unwrap_or_default(),
        }
    }

    pub fn from_env() -> Self {
        Self::new(R2CasConfig::default())
    }

    /// Whether live R2 credentials are fully configured.
    pub fn is_configured(&self) -> bool {
        !self.config.access_key_id.is_empty()
            && !self.config.secret_access_key.is_empty()
            && !self.config.endpoint.is_empty()
    }

    pub fn public_cdn_url(&self) -> &str {
        &self.config.public_cdn_url
    }

    pub fn mod_cache_bucket(&self) -> &str {
        &self.config.mod_cache_bucket
    }

    pub fn backups_bucket(&self) -> &str {
        &self.config.backups_bucket
    }

    /// Computes the SHA-256 hash and uploads the mod JAR to CAS if not already present.
    /// Returns `(sha256_hash, download_url)`.
    pub async fn upload_mod_jar(&self, file_path: &Path) -> io::Result<(String, String)> {
        let file_bytes = tokio::fs::read(file_path).await?;
        self.upload_mod_bytes(&file_bytes).await
    }

    /// Computes the SHA-256 hash and uploads mod bytes to CAS if not already present.
    /// Returns `(sha256_hash, download_url)`.
    pub async fn upload_mod_bytes(&self, file_bytes: &[u8]) -> io::Result<(String, String)> {
        let mut hasher = Sha256::new();
        hasher.update(file_bytes);
        let sha256_hash = hex::encode(hasher.finalize());

        let object_key = format!("objects/{sha256_hash}.jar");
        let download_url = format!("{}/{}", self.config.public_cdn_url, object_key);

        if !self.is_configured() {
            warn!(
                "[R2 CAS] R2 credentials not configured. Mocking CAS upload for {sha256_hash} -> {download_url}"
            );
            return Ok((sha256_hash, download_url));
        }

        // Check if object already exists in R2 CAS (HEAD request)
        if self
            .head_object(&self.config.mod_cache_bucket, &object_key)
            .await?
        {
            info!("[R2 CAS] Mod with hash {sha256_hash} already exists in R2 CAS. Skipping upload.");
            return Ok((sha256_hash, download_url));
        }

        // Object does not exist: upload payload to R2 CAS
        info!("[R2 CAS] Uploading new mod {sha256_hash} to R2 CAS: {object_key}");
        self.put_object(
            &self.config.mod_cache_bucket,
            &object_key,
            file_bytes.to_vec(),
            "application/java-archive",
        )
        .await?;

        Ok((sha256_hash, download_url))
    }

    /// Archives an instance folder to an LZ4 TAR archive and exports it to the cold backups bucket in R2.
    /// Returns the storage key `backups/{instance_id}/{archive_filename}`.
    pub async fn export_instance_to_r2(
        &self,
        instance_id: &str,
        instance_dir: &Path,
        backup_id: &str,
    ) -> io::Result<String> {
        let archive_filename = format!("{instance_id}_{backup_id}.tar.lz4");
        let temp_archive_path = std::env::temp_dir().join(&archive_filename);

        // Compress instance folder using zircon_core native LZ4 TAR
        let instance_dir_owned = instance_dir.to_path_buf();
        let temp_path_owned = temp_archive_path.clone();

        tokio::task::spawn_blocking(move || {
            let mut audit = Vec::new();
            zircon_core::archive::lz4_tar::compress_directory(
                &instance_dir_owned,
                &temp_path_owned,
                None,
                &mut audit,
            )
        })
        .await
        .map_err(|e| io::Error::new(io::ErrorKind::Other, e.to_string()))??;

        let file_bytes = tokio::fs::read(&temp_archive_path).await?;
        let r2_key = format!("backups/{instance_id}/{archive_filename}");

        if self.is_configured() {
            info!("[R2 Backup] Streaming {archive_filename} ({} bytes) to R2: s3://{}/{r2_key}", file_bytes.len(), self.config.backups_bucket);
            self.put_object(
                &self.config.backups_bucket,
                &r2_key,
                file_bytes,
                "application/x-lz4",
            )
            .await?;
        } else {
            warn!("[R2 Backup] R2 credentials not configured. Mocking cold backup export to s3://{}/{r2_key}", self.config.backups_bucket);
        }

        // Clean up temporary local archive
        let _ = tokio::fs::remove_file(&temp_archive_path).await;

        Ok(r2_key)
    }

    /// Checks whether an object exists in the given bucket (HEAD request).
    pub async fn head_object(&self, bucket: &str, key: &str) -> io::Result<bool> {
        if !self.is_configured() {
            return Ok(false);
        }

        let now: DateTime<Utc> = SystemTime::now().into();
        let url = format!("{}/{}/{}", self.clean_endpoint(), bucket, key);
        let host = extract_host(&self.config.endpoint);

        let payload_hash = hex::encode(Sha256::digest(b""));
        let headers = build_sigv4_headers(
            &Method::HEAD,
            &url,
            &host,
            &now,
            &self.config.region,
            &self.config.access_key_id,
            &self.config.secret_access_key,
            &payload_hash,
            None,
        );

        let mut req = self.http_client.head(&url);
        for (k, v) in headers {
            req = req.header(k, v);
        }

        let resp = req
            .send()
            .await
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e.to_string()))?;

        if resp.status().is_success() {
            Ok(true)
        } else if resp.status() == StatusCode::NOT_FOUND {
            Ok(false)
        } else {
            Err(io::Error::new(
                io::ErrorKind::Other,
                format!("R2 HEAD request returned status {}", resp.status()),
            ))
        }
    }

    /// Uploads an object to the given bucket (PUT request).
    pub async fn put_object(
        &self,
        bucket: &str,
        key: &str,
        body: Vec<u8>,
        content_type: &str,
    ) -> io::Result<()> {
        if !self.is_configured() {
            warn!("[R2] Mock PutObject: {bucket}/{key} ({} bytes)", body.len());
            return Ok(());
        }

        let now: DateTime<Utc> = SystemTime::now().into();
        let url = format!("{}/{}/{}", self.clean_endpoint(), bucket, key);
        let host = extract_host(&self.config.endpoint);

        let payload_hash = hex::encode(Sha256::digest(&body));
        let headers = build_sigv4_headers(
            &Method::PUT,
            &url,
            &host,
            &now,
            &self.config.region,
            &self.config.access_key_id,
            &self.config.secret_access_key,
            &payload_hash,
            Some(content_type),
        );

        let mut req = self.http_client.put(&url).body(body);
        for (k, v) in headers {
            req = req.header(k, v);
        }

        let resp = req
            .send()
            .await
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e.to_string()))?;

        if resp.status().is_success() {
            Ok(())
        } else {
            let status = resp.status();
            let err_text = resp.text().await.unwrap_or_default();
            Err(io::Error::new(
                io::ErrorKind::Other,
                format!("R2 PUT failed with status {status}: {err_text}"),
            ))
        }
    }

    /// Downloads an object from the given bucket (GET request).
    pub async fn get_object(&self, bucket: &str, key: &str) -> io::Result<Vec<u8>> {
        if !self.is_configured() {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                format!("R2 not configured: cannot download {bucket}/{key}"),
            ));
        }

        let now: DateTime<Utc> = SystemTime::now().into();
        let url = format!("{}/{}/{}", self.clean_endpoint(), bucket, key);
        let host = extract_host(&self.config.endpoint);

        let payload_hash = hex::encode(Sha256::digest(b""));
        let headers = build_sigv4_headers(
            &Method::GET,
            &url,
            &host,
            &now,
            &self.config.region,
            &self.config.access_key_id,
            &self.config.secret_access_key,
            &payload_hash,
            None,
        );

        let mut req = self.http_client.get(&url);
        for (k, v) in headers {
            req = req.header(k, v);
        }

        let resp = req
            .send()
            .await
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e.to_string()))?;

        if resp.status().is_success() {
            let bytes = resp
                .bytes()
                .await
                .map_err(|e| io::Error::new(io::ErrorKind::Other, e.to_string()))?;
            Ok(bytes.to_vec())
        } else {
            let status = resp.status();
            Err(io::Error::new(
                io::ErrorKind::Other,
                format!("R2 GET failed with status {status}"),
            ))
        }
    }

    fn clean_endpoint(&self) -> String {
        self.config.endpoint.trim_end_matches('/').to_string()
    }
}

// ---------------------------------------------------------------------------
// AWS Signature Version 4 (SigV4) Implementation
// ---------------------------------------------------------------------------

fn extract_host(endpoint: &str) -> String {
    let without_proto = endpoint
        .strip_prefix("https://")
        .or_else(|| endpoint.strip_prefix("http://"))
        .unwrap_or(endpoint);
    without_proto.split('/').next().unwrap_or(without_proto).to_string()
}

fn hmac_sha256(key: &[u8], data: &[u8]) -> [u8; 32] {
    let mut k = [0u8; 64];
    if key.len() > 64 {
        let hash = Sha256::digest(key);
        k[..32].copy_from_slice(&hash);
    } else {
        k[..key.len()].copy_from_slice(key);
    }

    let mut ipad = [0x36u8; 64];
    let mut opad = [0x5cu8; 64];
    for i in 0..64 {
        ipad[i] ^= k[i];
        opad[i] ^= k[i];
    }

    let mut inner_hasher = Sha256::new();
    inner_hasher.update(&ipad);
    inner_hasher.update(data);
    let inner = inner_hasher.finalize();

    let mut outer_hasher = Sha256::new();
    outer_hasher.update(&opad);
    outer_hasher.update(&inner);
    outer_hasher.finalize().into()
}

#[allow(clippy::too_many_arguments)]
fn build_sigv4_headers(
    method: &Method,
    url_str: &str,
    host: &str,
    now: &DateTime<Utc>,
    region: &str,
    access_key: &str,
    secret_key: &str,
    payload_hash: &str,
    content_type: Option<&str>,
) -> Vec<(String, String)> {
    let amz_date = now.format("%Y%m%dT%H%M%SZ").to_string();
    let date_stamp = now.format("%Y%m%d").to_string();

    let parsed_url = url::Url::parse(url_str).unwrap_or_else(|_| url::Url::parse("http://localhost").unwrap());
    let canonical_uri = parsed_url.path();

    let mut canonical_headers = format!("host:{host}\nx-amz-content-sha256:{payload_hash}\nx-amz-date:{amz_date}\n");
    let mut signed_headers = "host;x-amz-content-sha256;x-amz-date".to_string();

    if let Some(ct) = content_type {
        canonical_headers = format!("content-type:{ct}\n{canonical_headers}");
        signed_headers = format!("content-type;{signed_headers}");
    }

    let canonical_request = format!(
        "{}\n{}\n\n{}\n{}\n{}",
        method.as_str(),
        canonical_uri,
        canonical_headers,
        signed_headers,
        payload_hash
    );

    let canonical_req_hash = hex::encode(Sha256::digest(canonical_request.as_bytes()));
    let credential_scope = format!("{date_stamp}/{region}/s3/aws4_request");
    let string_to_sign = format!(
        "AWS4-HMAC-SHA256\n{}\n{}\n{}",
        amz_date,
        credential_scope,
        canonical_req_hash
    );

    // Derived signing key
    let k_secret = format!("AWS4{secret_key}");
    let k_date = hmac_sha256(k_secret.as_bytes(), date_stamp.as_bytes());
    let k_region = hmac_sha256(&k_date, region.as_bytes());
    let k_service = hmac_sha256(&k_region, b"s3");
    let k_signing = hmac_sha256(&k_service, b"aws4_request");

    let signature = hex::encode(hmac_sha256(&k_signing, string_to_sign.as_bytes()));

    let auth_header = format!(
        "AWS4-HMAC-SHA256 Credential={access_key}/{credential_scope}, SignedHeaders={signed_headers}, Signature={signature}"
    );

    let mut headers = vec![
        ("host".to_string(), host.to_string()),
        ("x-amz-date".to_string(), amz_date),
        ("x-amz-content-sha256".to_string(), payload_hash.to_string()),
        ("authorization".to_string(), auth_header),
    ];

    if let Some(ct) = content_type {
        headers.push(("content-type".to_string(), ct.to_string()));
    }

    headers
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hmac_sha256_matches_known_vector() {
        // RFC 4231 test case 1
        let key = [0x0bu8; 20];
        let data = b"Hi There";
        let out = hmac_sha256(&key, data);
        assert_eq!(
            hex::encode(out),
            "b0344c61d8db38535ca8afceaf0bf12b881dc200c9833da726e9376c2e32cff7"
        );
    }

    #[tokio::test]
    async fn mock_cas_upload_produces_consistent_sha256_and_cdn_url() {
        let client = R2CasClient::new(R2CasConfig {
            endpoint: String::new(),
            access_key_id: String::new(),
            secret_access_key: String::new(),
            region: "auto".to_string(),
            mod_cache_bucket: "zircon-mod-cache".to_string(),
            backups_bucket: "zircon-backups".to_string(),
            public_cdn_url: "https://cdn.zirconmc.net".to_string(),
        });

        let test_payload = b"Fake JAR content for JEI 1.20.1";
        let (sha256, cdn_url) = client.upload_mod_bytes(test_payload).await.unwrap();

        let expected_sha256 = hex::encode(Sha256::digest(test_payload));
        assert_eq!(sha256, expected_sha256);
        assert_eq!(
            cdn_url,
            format!("https://cdn.zirconmc.net/objects/{expected_sha256}.jar")
        );
    }
}
