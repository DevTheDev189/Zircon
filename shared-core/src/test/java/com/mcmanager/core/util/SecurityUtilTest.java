package com.mcmanager.core.util;

import org.junit.jupiter.api.Test;

import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertTrue;

class SecurityUtilTest {

    @Test
    void acceptsAllowedCdnDomains() {
        assertTrue(SecurityUtil.isSafeCdnUrl("https://cdn.modrinth.com/data/abc/1.0.jar"));
        assertTrue(SecurityUtil.isSafeCdnUrl("https://edge.forgecdn.net/files/1234/5678/mod.jar"));
        assertTrue(SecurityUtil.isSafeCdnUrl("https://maven.neoforged.net/releases/net/neoforged/neoforge/1.0/neoforge-1.0-installer.jar"));
        assertTrue(SecurityUtil.isSafeCdnUrl("https://piston-meta.mojang.com/mc/game/version_manifest_v2.json"));
    }

    @Test
    void acceptsStrictSubdomainsOfAllowedDomains() {
        assertTrue(SecurityUtil.isSafeCdnUrl("https://files.cdn.modrinth.com/x/y.jar"));
    }

    @Test
    void rejectsCloudMetadataAndLoopbackHosts() {
        // The classic SSRF target: AWS/GCP cloud metadata.
        assertFalse(SecurityUtil.isSafeCdnUrl("http://169.254.169.254/latest/meta-data/"));
        assertFalse(SecurityUtil.isSafeCdnUrl("http://metadata.google.internal/computeMetadata/v1/"));
        assertFalse(SecurityUtil.isSafeCdnUrl("http://127.0.0.1:25564/api/config"));
        assertFalse(SecurityUtil.isSafeCdnUrl("http://localhost:8080/"));
    }

    @Test
    void rejectsArbitraryHostsAndLookalikes() {
        assertFalse(SecurityUtil.isSafeCdnUrl("https://evil.example.com/cdn.modrinth.com/x.jar"));
        assertFalse(SecurityUtil.isSafeCdnUrl("https://modrinth.com.evil.com/x.jar"));
        assertFalse(SecurityUtil.isSafeCdnUrl("https://notmodrinth.com/x.jar"));
    }

    @Test
    void rejectsMalformedUrls() {
        assertFalse(SecurityUtil.isSafeCdnUrl("not a url"));
        assertFalse(SecurityUtil.isSafeCdnUrl("file:///etc/passwd"));
        assertFalse(SecurityUtil.isSafeCdnUrl(""));
        assertFalse(SecurityUtil.isSafeCdnUrl(null));
    }
}
