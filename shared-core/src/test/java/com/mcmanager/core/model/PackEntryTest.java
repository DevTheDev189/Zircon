package com.mcmanager.core.model;

import org.junit.jupiter.api.Test;

import java.util.Map;

import static org.junit.jupiter.api.Assertions.assertEquals;

class PackEntryTest {

    @Test
    void toMapIncludesFallbacks() {
        PackEntry entry = new PackEntry("complementary", "ComplementaryShaders.zip", "deadbeef",
                0L, "modrinth", "https://cdn.modrinth.com/complementary.zip", 4096L);

        Map<String, Object> map = entry.toMap();

        assertEquals("complementary", map.get("id"));
        assertEquals("ComplementaryShaders.zip", map.get("filename"));
        // Title falls back to the filename when unset.
        assertEquals("ComplementaryShaders.zip", map.get("title"));
        assertEquals("", map.get("iconUrl"));
    }

    @Test
    void richMetadataRoundTripsThroughSetters() {
        PackEntry entry = new PackEntry();
        entry.setId("faithful");
        entry.setFilename("Faithful.zip");
        entry.setTitle("Faithful 32x");
        entry.setIconUrl("https://cdn.modrinth.com/icon.png");

        Map<String, Object> map = entry.toMap();
        assertEquals("Faithful 32x", map.get("title"));
        assertEquals("https://cdn.modrinth.com/icon.png", map.get("iconUrl"));
    }
}
