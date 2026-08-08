package com.mcmanager.core.util;

import java.net.URI;

/**
 * SSRF (Server-Side Request Forgery) protection for outbound mod downloads.
 *
 * <p>The wrapper only ever fetches files from well-known mod CDNs / metadata
 * hosts. Any URL whose host is not one of these — including loopback, link
 * local (169.254.169.254 cloud metadata), or arbitrary user-supplied hosts —
 * is rejected before an HTTP request is made.
 */
public final class SecurityUtil {

    private static final java.util.Set<String> ALLOWED_CDN_DOMAINS = java.util.Set.of(
            "cdn.modrinth.com",
            "edge.forgecdn.net",
            "media.forgecdn.net",
            "maven.neoforged.net",
            "maven.minecraftforge.net",
            "meta.fabricmc.net",
            "meta.quiltmc.org",
            "piston-meta.mojang.com",
            "launchermeta.mojang.com"
    );

    private SecurityUtil() {
    }

    /**
     * @return {@code true} if the URL parses and its host is an allowed CDN
     *         domain or a strict subdomain of one.
     */
    public static boolean isSafeCdnUrl(String url) {
        try {
            URI uri = URI.create(url);
            String host = uri.getHost();
            if (host == null || host.isBlank()) {
                return false;
            }
            return ALLOWED_CDN_DOMAINS.stream().anyMatch(allowed ->
                    host.equalsIgnoreCase(allowed) || host.endsWith("." + allowed));
        } catch (Exception e) {
            return false;
        }
    }
}
