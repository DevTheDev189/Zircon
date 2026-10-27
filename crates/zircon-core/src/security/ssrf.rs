//! SSRF (Server-Side Request Forgery) protection for outbound mod downloads.
//!
//! The wrapper only ever fetches files from well-known mod CDNs / metadata
//! hosts. Any URL whose host is not one of these — including loopback, link
//! local (`169.254.169.254` cloud metadata), or arbitrary user-supplied
//! hosts — is rejected before an HTTP request is made.
//!
//! Only strict `https://` URLs on default ports are accepted: plaintext
//! `http://` (MITM tampering) and explicit custom ports (intranet port
//! scanning) are rejected outright.
//!
//! Port of `com.mcmanager.core.util.SecurityUtil`.

use std::net::{IpAddr, Ipv4Addr, SocketAddr, ToSocketAddrs};

/// Hosts the wrapper is allowed to fetch from. A URL is safe when its host
/// equals one of these or is a strict subdomain of one.
pub const ALLOWED_CDN_DOMAINS: &[&str] = &[
    "zirconmc.net",
    "cdn.modrinth.com",
    "edge.forgecdn.net",
    "media.forgecdn.net",
    "maven.neoforged.net",
    "maven.minecraftforge.net",
    "files.minecraftforge.net",
    "meta.fabricmc.net",
    "maven.fabricmc.net",
    "meta.quiltmc.org",
    "maven.quiltmc.org",
    "piston-meta.mojang.com",
    "piston-data.mojang.com",
    "launcher.mojang.com",
    "launchermeta.mojang.com",
    // Adoptium JDK metadata API (Phase 2: supply-chain-verified Java toolchain).
    "api.adoptium.net",
    "github.com",
    "objects.githubusercontent.com",
    "release-assets.githubusercontent.com",
];

/// Returns `true` if the URL is strictly HTTPS, has no custom port, and its
/// host is an allowed CDN domain or a strict subdomain of one.
pub fn is_safe_cdn_url(url: &str) -> bool {
    let parsed = match url::Url::parse(url) {
        Ok(parsed) => parsed,
        Err(_) => return false,
    };

    // 1. Enforce HTTPS only (no plaintext HTTP or file/ftp schemes).
    if parsed.scheme() != "https" {
        return false;
    }

    // 2. Disallow explicit custom ports and embedded userinfo credentials.
    if parsed.port().is_some() || !parsed.username().is_empty() || parsed.password().is_some() {
        return false;
    }

    // 3. Reject raw IPv4/IPv6 literals (including decimal, octal, and IPv4-mapped IPv6).
    match parsed.host() {
        Some(url::Host::Domain(_)) => {}
        _ => return false,
    }

    let host = match parsed.host_str() {
        Some(host) if !host.is_empty() => host,
        _ => return false,
    };
    let host_lower = host.trim_end_matches('.').to_ascii_lowercase();
    if host_lower.parse::<IpAddr>().is_ok() {
        return false;
    }

    ALLOWED_CDN_DOMAINS
        .iter()
        .any(|allowed| host_lower == *allowed || host_lower.ends_with(&format!(".{allowed}")))
}

/// Returns `true` if `ip` is a globally routable public unicast IP address.
/// Strictly blocks loopback, RFC1918 private, link-local (`169.254.0.0/16`, `fe80::/10`),
/// CGNAT (`100.64.0.0/10`), AWS/cloud metadata (`169.254.169.254`, `fd00:ec2::254`),
/// unique local (`fc00::/7`), multicast, broadcast, unspecified, and IPv4-mapped/compatible IPv6.
pub fn is_public_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => is_public_ipv4(v4),
        IpAddr::V6(v6) => {
            if let Some(mapped_v4) = v6.to_ipv4_mapped().or_else(|| v6.to_ipv4()) {
                return is_public_ipv4(mapped_v4);
            }
            let segs = v6.segments();
            !v6.is_loopback()
                && !v6.is_unspecified()
                && !v6.is_multicast()
                // Unique Local Addresses (fc00::/7, includes fd00:ec2::254 AWS IPv6 metadata)
                && (segs[0] & 0xfe00) != 0xfc00
                // Link-local unicast (fe80::/10)
                && (segs[0] & 0xffc0) != 0xfe80
                // Documentation (2001:db8::/32)
                && !(segs[0] == 0x2001 && segs[1] == 0x0db8)
        }
    }
}

fn is_public_ipv4(v4: Ipv4Addr) -> bool {
    let octets = v4.octets();
    !v4.is_private()
        && !v4.is_loopback()
        && !v4.is_link_local()
        && !v4.is_broadcast()
        && !v4.is_documentation()
        && !v4.is_unspecified()
        && !v4.is_multicast()
        // 0.0.0.0/8 ("this" network)
        && octets[0] != 0
        // 100.64.0.0/10 (Carrier-Grade NAT / RFC 6598)
        && !(octets[0] == 100 && (octets[1] & 0b1100_0000) == 64)
        // 192.0.0.0/24 (IETF Protocol Assignments)
        && !(octets[0] == 192 && octets[1] == 0 && octets[2] == 0)
        // 198.18.0.0/15 (Benchmarking)
        && !(octets[0] == 198 && (octets[1] & 0xfe) == 18)
        // 240.0.0.0/4 (Reserved)
        && octets[0] >= 1 && octets[0] < 240
}

/// Resolves the CDN URL's hostname once, verifies every resolved IP is a public
/// unicast address, and returns `(host, validated_socket_addrs)` so the caller can
/// pin them on `reqwest::ClientBuilder::resolve_to_addrs` to eliminate DNS rebinding TOCTOU.
pub fn resolve_and_validate_cdn_url(url: &str) -> Result<(String, Vec<SocketAddr>), String> {
    if !is_safe_cdn_url(url) {
        return Err(format!("URL failed CDN allowlist check: {url}"));
    }
    let parsed = url::Url::parse(url).map_err(|e| format!("Invalid URL {url}: {e}"))?;
    let host = parsed
        .host_str()
        .ok_or_else(|| format!("Missing host in URL: {url}"))?
        .trim_end_matches('.')
        .to_ascii_lowercase();
    let port = parsed.port_or_known_default().unwrap_or(443);

    let addrs: Vec<SocketAddr> = (host.as_str(), port)
        .to_socket_addrs()
        .map_err(|e| format!("DNS resolution failed for {host}: {e}"))?
        .collect();
    if addrs.is_empty() {
        return Err(format!("DNS resolution returned no addresses for {host}"));
    }
    for addr in &addrs {
        if !is_public_ip(addr.ip()) {
            return Err(format!(
                "SSRF protection blocked non-public IP {} resolved for host {host}",
                addr.ip()
            ));
        }
    }
    Ok((host, addrs))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_allowed_cdn_domains() {
        assert!(is_safe_cdn_url(
            "https://zirconmc.net/updates/server/latest.json"
        ));
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
        // Mojang's data CDN serves the actual server jars listed in manifests.
        assert!(is_safe_cdn_url(
            "https://piston-data.mojang.com/v1/objects/0123456789abcdef/server.jar"
        ));
        assert!(is_safe_cdn_url(
            "https://launcher.mojang.com/v1/objects/0123456789abcdef/server.jar"
        ));
        // Adoptium's JDK metadata API (checksum + download link resolution).
        assert!(is_safe_cdn_url(
            "https://api.adoptium.net/v3/assets/latest/21/hotspot?architecture=x64&image_type=jdk&os=windows&vendor=eclipse"
        ));
    }

    #[test]
    fn accepts_strict_subdomains_of_allowed_domains() {
        assert!(is_safe_cdn_url("https://files.cdn.modrinth.com/x/y.jar"));
        assert!(is_safe_cdn_url("https://updates.zirconmc.net/latest.json"));
    }

    #[test]
    fn accepts_allowed_https_domains() {
        assert!(is_safe_cdn_url(
            "https://zirconmc.net/updates/server/latest.json"
        ));
        assert!(is_safe_cdn_url("https://cdn.modrinth.com/data/abc/1.0.jar"));
        assert!(is_safe_cdn_url(
            "https://edge.forgecdn.net/files/123/456/mod.jar"
        ));
    }

    #[test]
    fn rejects_insecure_schemes_and_ports() {
        assert!(!is_safe_cdn_url("http://cdn.modrinth.com/data/abc/1.0.jar")); // No HTTP
        assert!(!is_safe_cdn_url(
            "https://cdn.modrinth.com:8443/data/abc/1.0.jar"
        )); // No custom ports
        assert!(!is_safe_cdn_url("file:///etc/passwd"));
        assert!(!is_safe_cdn_url("http://169.254.169.254/latest/meta-data/"));
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
        assert!(!is_safe_cdn_url("https://user:pass@cdn.modrinth.com/x.jar"));
        assert!(!is_safe_cdn_url("https://127.0.0.1/"));
    }

    #[test]
    fn rejects_malformed_urls() {
        assert!(!is_safe_cdn_url("not a url"));
        assert!(!is_safe_cdn_url("file:///etc/passwd"));
        assert!(!is_safe_cdn_url(""));
    }

    #[test]
    fn test_is_public_ip() {
        use std::str::FromStr;
        assert!(!is_public_ip(IpAddr::from_str("127.0.0.1").unwrap()));
        assert!(!is_public_ip(IpAddr::from_str("10.0.0.1").unwrap()));
        assert!(!is_public_ip(IpAddr::from_str("172.16.0.1").unwrap()));
        assert!(!is_public_ip(IpAddr::from_str("192.168.1.1").unwrap()));
        assert!(!is_public_ip(IpAddr::from_str("169.254.169.254").unwrap()));
        assert!(!is_public_ip(IpAddr::from_str("::1").unwrap()));
        assert!(!is_public_ip(IpAddr::from_str("fc00::1").unwrap()));
        assert!(!is_public_ip(IpAddr::from_str("fe80::1").unwrap()));
        assert!(!is_public_ip(IpAddr::from_str("::ffff:127.0.0.1").unwrap()));
        assert!(is_public_ip(IpAddr::from_str("8.8.8.8").unwrap()));
        assert!(is_public_ip(IpAddr::from_str("1.1.1.1").unwrap()));
    }
}
