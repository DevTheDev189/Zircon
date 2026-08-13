package com.mcmanager.core.model;

import org.junit.jupiter.api.Test;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertNotNull;

class InstanceConfigTest {

    @Test
    void locksInLoaderAtCreation() {
        InstanceConfig config = new InstanceConfig("My Fabric Server", "1.21.4", "fabric", "0.15.11", 25566);

        assertNotNull(config.getId());
        assertEquals("My Fabric Server", config.getName());
        assertEquals("1.21.4", config.getMinecraftVersion());
        assertEquals("fabric", config.getModLoader().getType());
        assertEquals("0.15.11", config.getModLoader().getVersion());
        assertEquals(25566, config.getInternalMcPort());
        assertEquals("-Xms2G -Xmx4G", config.getJavaArgs());
        assertEquals(false, config.isAutoStart());
    }

    @Test
    void assignsUniqueShortIds() {
        InstanceConfig a = new InstanceConfig("A", "1.21.4", "fabric", "", 25566);
        InstanceConfig b = new InstanceConfig("B", "1.21.4", "neoforge", "", 25567);
        assertEquals(8, a.getId().length());
        assertEquals(8, b.getId().length());
    }

    @Test
    void allowsNameAndJavaArgsMutationsButNotLoader() {
        InstanceConfig config = new InstanceConfig("Original", "1.20.4", "neoforge", "20.4.250", 25566);

        config.setName("Renamed");
        config.setJavaArgs("-Xmx8G");
        config.setAutoStart(true);

        assertEquals("Renamed", config.getName());
        assertEquals("-Xmx8G", config.getJavaArgs());
        assertEquals(true, config.isAutoStart());
        // The loader is immutable after creation.
        assertEquals("neoforge", config.getModLoader().getType());
        assertEquals("20.4.250", config.getModLoader().getVersion());
    }

    @Test
    void updatesLoaderVersionButKeepsLoaderTypeLocked() {
        InstanceConfig config = new InstanceConfig("Fabric Instance", "1.21.4", "fabric", "0.15.11", 25566);

        config.setLoaderVersion("0.16.5");
        config.setMinecraftVersion("1.21.1");

        assertEquals("0.16.5", config.getModLoader().getVersion());
        assertEquals("fabric", config.getModLoader().getType()); // still locked
        assertEquals("1.21.1", config.getMinecraftVersion());
    }

    @Test
    void setLoaderVersionCreatesVanillaLoaderWhenAbsent() {
        InstanceConfig config = new InstanceConfig(); // Gson path: modLoader may be null
        config.setLoaderVersion("1.21.4");

        assertNotNull(config.getModLoader());
        assertEquals("vanilla", config.getModLoader().getType());
        assertEquals("1.21.4", config.getModLoader().getVersion());
    }
}
