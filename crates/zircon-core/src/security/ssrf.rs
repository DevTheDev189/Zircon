//! SSRF (Server-Side Request Forgery) protection for outbound mod downloads.
//!
//! The wrapper only ever fetches files from well-known mod CDNs / metadata
//! hosts. Any URL whose host is not one of these — including loopback, link
//! local (`169.254.169.254` cloud metadata), or arbitrary user-supplied
//! hosts — is rejected before an HTTP request is made.
//!
//! Port of `com.mcmanager.core.util.SecurityUtil`.

/// Hosts the wrapper is allowed to fetch from. A URL is safe when its host
/// equals one of these or is a strict subdomain of one.
pub const ALLOWED_CDN_DOMAINS: &[&str] = &[
    "cdn.modrinth.com",
    "edge.forgecdn.net",
    "media.forgecdn.net",
    "maven.neoforged.net",
    "maven.minecraftforge.net",
    "meta.fabricmc.net",
    "meta.quiltmc.org",
    "piston-meta.mojang.com",
    "launchermeta.mojang.com",
];

/// Returns `true` if the URL parses and its host is an allowed CDN domain or a
/// strict subdomain of one.
pub fn is_safe_cdn_url(url: &str) -> bool {
    let parsed = match url::Url::parse(url) {
        Ok(parsed) => parsed,
        Err(_) => return false,
    };
    let host = match parsed.host_str() {
        Some(host) if !host.is_empty() => host,
        _ => return false,
    };
    let host_lower = host.to_ascii_lowercase();
    ALLOWED_CDN_DOMAINS
        .iter()
        .any(|allowed| host_lower == *allowed || host_lower.ends_with(&format!(".{allowed}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_allowed_cdn_domains() {
        assert!(is_safe_cdn_url("https://cdn.modrinth.com/data/abc/1.0.jar"));
        assert!(is_safe_cdn_url(
            "https://edge.forgecdn.net/files/1234/5678/mod.jar"
        ));
        assert!(is_safe_cdn_url(
            "https://maven.neoforged.net/releases/net/neoforged/neoforge/1.0/neoforge-1.0-installer.jar"
        ));
        assert!(is_safe_cdn_url(
            "https://piston-meta.mojang.com/mc/game/version_manifest_v2.json"
        ));
    }

    #[test]
    fn accepts_strict_subdomains_of_allowed_domains() {
        assert!(is_safe_cdn_url("https://files.cdn.modrinth.com/x/y.jar"));
    }

    #[test]
    fn rejects_cloud_metadata_and_loopback_hosts() {
        // The classic SSRF target: AWS/GCP cloud metadata.
        assert!(!is_safe_cdn_url("http://169.254.169.254/latest/meta-data/"));
        assert!(!is_safe_cdn_url(
            "http://metadata.google.internal/computeMetadata/v1/"
        ));
        assert!(!is_safe_cdn_url("http://127.0.0.1:25564/api/config"));
        assert!(!is_safe_cdn_url("http://localhost:8080/"));
    }

    #[test]
    fn rejects_arbitrary_hosts_and_lookalikes() {
        assert!(!is_safe_cdn_url(
            "https://evil.example.com/cdn.modrinth.com/x.jar"
        ));
        assert!(!is_safe_cdn_url("https://modrinth.com.evil.com/x.jar"));
        assert!(!is_safe_cdn_url("https://notmodrinth.com/x.jar"));
    }

    #[test]
    fn rejects_malformed_urls() {
        assert!(!is_safe_cdn_url("not a url"));
        assert!(!is_safe_cdn_url("file:///etc/passwd"));
        assert!(!is_safe_cdn_url(""));
    }
}
