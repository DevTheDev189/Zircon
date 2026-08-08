package com.mcmanager.core.model;

import org.junit.jupiter.api.Test;

import java.util.Map;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertTrue;

class ModEntryTest {

    @Test
    void toMapIncludesRichMetadataWithFallbacks() {
        ModEntry entry = new ModEntry("abc123", "sodium-0.5.8.jar", "deadbeef",
                12345L, "modrinth", "https://cdn.modrinth.com/sodium.jar", 4096L);

        Map<String, Object> map = entry.toMap();

        assertEquals("abc123", map.get("id"));
        assertEquals("sodium-0.5.8.jar", map.get("filename"));
        // Title falls back to the filename when unset.
        assertEquals("sodium-0.5.8.jar", map.get("title"));
        assertEquals("", map.get("description"));
        assertEquals("", map.get("iconUrl"));
        assertEquals("", map.get("author"));
        assertEquals(true, map.get("compatible"));
        assertEquals("", map.get("warningMessage"));
    }

    @Test
    void defaultsToCompatible() {
        ModEntry entry = new ModEntry();
        assertTrue(entry.isCompatible());
    }

    @Test
    void richMetadataRoundTripsThroughSetters() {
        ModEntry entry = new ModEntry();
        entry.setId("proj-1");
        entry.setTitle("Sodium");
        entry.setDescription("Fast rendering engine");
        entry.setIconUrl("https://cdn.modrinth.com/icon.png");
        entry.setAuthor("jellysquid3");
        entry.setCompatible(false);
        entry.setWarningMessage("Unverified for MC 1.21.1");

        Map<String, Object> map = entry.toMap();
        assertEquals("Sodium", map.get("title"));
        assertEquals("Fast rendering engine", map.get("description"));
        assertEquals("https://cdn.modrinth.com/icon.png", map.get("iconUrl"));
        assertEquals("jellysquid3", map.get("author"));
        assertEquals(false, map.get("compatible"));
        assertEquals("Unverified for MC 1.21.1", map.get("warningMessage"));
    }
}
