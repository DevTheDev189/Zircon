package com.mcmanager.server.instance;

import com.mcmanager.core.model.InstanceConfig;
import com.mcmanager.server.process.ConsoleStreamHandler;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.io.TempDir;

import java.nio.file.Files;
import java.nio.file.Path;
import java.util.Map;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertNotNull;
import static org.junit.jupiter.api.Assertions.assertNull;
import static org.junit.jupiter.api.Assertions.assertThrows;
import static org.junit.jupiter.api.Assertions.assertTrue;

class ServerInstanceManagerTest {

    @TempDir
    Path tempDir;

    private ServerInstanceManager newManager() throws Exception {
        return new ServerInstanceManager(tempDir.resolve("server-data"), new ConsoleStreamHandler());
    }

    @Test
    void createsIsolatedInstanceDirectories() throws Exception {
        ServerInstanceManager manager = newManager();

        InstanceConfig a = manager.createInstance("Fabric World", "1.21.4", "fabric", "0.15.11");
        InstanceConfig b = manager.createInstance("NeoForge World", "1.20.4", "neoforge", "20.4.250");

        // Distinct ports, distinct dirs, distinct loaders locked in.
        assertTrue(a.getInternalMcPort() != b.getInternalMcPort());
        Path dirA = manager.getInstanceDir(a.getId());
        Path dirB = manager.getInstanceDir(b.getId());
        assertTrue(dirA.toString().contains(a.getId()));
        assertTrue(dirB.toString().contains(b.getId()));
        assertTrue(Files.isDirectory(dirA.resolve("mods")));
        assertTrue(Files.isDirectory(dirA.resolve("server")));
        assertTrue(Files.isRegularFile(dirA.resolve("instance.json")));
        assertEquals("fabric", a.getModLoader().getType());
        assertEquals("neoforge", b.getModLoader().getType());
    }

    @Test
    void persistsInstancesAcrossRestart() throws Exception {
        Path dataDir = tempDir.resolve("server-data");
        ServerInstanceManager first = new ServerInstanceManager(dataDir, new ConsoleStreamHandler());
        InstanceConfig created = first.createInstance("Persistent", "1.20.4", "quilt", "0.24.0");

        // Simulate a restart.
        ServerInstanceManager second = new ServerInstanceManager(dataDir, new ConsoleStreamHandler());
        InstanceConfig reloaded = second.getInstance(created.getId());

        assertEquals("Persistent", reloaded.getName());
        assertEquals("quilt", reloaded.getModLoader().getType());
        assertEquals(created.getInternalMcPort(), reloaded.getInternalMcPort());
    }

    @Test
    void updateCannotChangeLoader() throws Exception {
        ServerInstanceManager manager = newManager();
        InstanceConfig created = manager.createInstance("Original", "1.20.4", "forge", "47.2.0");

        manager.updateInstanceConfig(created.getId(), "Renamed", "-Xmx6G");

        InstanceConfig updated = manager.getInstance(created.getId());
        assertEquals("Renamed", updated.getName());
        assertEquals("-Xmx6G", updated.getJavaArgs());
        // Loader untouched — no API surface to change it.
        assertEquals("forge", updated.getModLoader().getType());
        assertEquals("47.2.0", updated.getModLoader().getVersion());
    }

    @Test
    void updateInstanceVersionsChangesVersionsAndSyncsBom() throws Exception {
        ServerInstanceManager manager = newManager();
        InstanceConfig created = manager.createInstance("Fabric World", "1.21.4", "fabric", "0.15.11");

        // Fresh instance has an empty BOM, so the mod sync performs no network calls.
        Map<String, Object> summary = manager.updateInstanceVersions(
                created.getId(), "1.21.1", "0.16.5", "Renamed");

        InstanceConfig updated = manager.getInstance(created.getId());
        assertEquals("Renamed", updated.getName());
        assertEquals("1.21.1", updated.getMinecraftVersion());
        // Loader type stays locked; only its version moved.
        assertEquals("fabric", updated.getModLoader().getType());
        assertEquals("0.16.5", updated.getModLoader().getVersion());
        assertEquals(0, summary.get("updatedCount"));
        assertEquals(0, summary.get("incompatibleCount"));

        // Changes are persisted to instance.json and the BOM is pinned to the new version.
        Path cfgFile = manager.getInstanceDir(created.getId()).resolve("instance.json");
        assertTrue(Files.readString(cfgFile).contains("1.21.1"));
        Path bomFile = manager.getInstanceDir(created.getId()).resolve("bom.json");
        assertTrue(Files.isRegularFile(bomFile));
        assertTrue(Files.readString(bomFile).contains("1.21.1"));
    }

    @Test
    void resolvesInstancesByHostname() throws Exception {
        ServerInstanceManager manager = newManager();
        InstanceConfig created = manager.createInstance("My Fabric Server", "1.21.4", "fabric", "");

        assertEquals(created.getId(), manager.findByHostname("my-fabric-server").getId());
        assertEquals(created.getId(), manager.findByHostname("MY-FABRIC-SERVER").getId());
        assertEquals(created.getId(), manager.findByHostname(created.getId()).getId());
        assertNull(manager.findByHostname("unknown-host"));
        assertNull(manager.findByHostname(""));
    }

    @Test
    void rejectsUnknownInstanceIds() throws Exception {
        ServerInstanceManager manager = newManager();
        assertThrows(IllegalArgumentException.class, () -> manager.getInstance("does-not-exist"));
        assertThrows(IllegalArgumentException.class, () -> manager.updateInstanceConfig("nope", "x", "-Xmx2G"));
        assertNotNull(manager.listInstances());
        assertEquals(0, manager.listInstances().size());
    }

    @Test
    void eulaStartsUnacceptedAndCanBeAccepted() throws Exception {
        ServerInstanceManager manager = newManager();
        InstanceConfig created = manager.createInstance("Eula Server", "1.21.4", "fabric", "");

        assertFalse(manager.isEulaAccepted(created.getId()));
        // Starting without an accepted EULA must fail fast with a clear message.
        IllegalStateException e = assertThrows(IllegalStateException.class,
                () -> manager.startInstance(created.getId()));
        assertTrue(e.getMessage().toLowerCase().contains("eula"));

        manager.acceptEula(created.getId());
        assertTrue(manager.isEulaAccepted(created.getId()));
        Path eula = manager.getInstanceDir(created.getId()).resolve("server").resolve("eula.txt");
        assertTrue(Files.isRegularFile(eula));
        assertTrue(Files.readString(eula).contains("eula=true"));
    }

    @Test
    void eulaPersistsAcrossRestart() throws Exception {
        Path dataDir = tempDir.resolve("server-data");
        ServerInstanceManager first = new ServerInstanceManager(dataDir, new ConsoleStreamHandler());
        InstanceConfig created = first.createInstance("Persistent Eula", "1.21.4", "vanilla", "");
        first.acceptEula(created.getId());

        ServerInstanceManager second = new ServerInstanceManager(dataDir, new ConsoleStreamHandler());
        assertTrue(second.isEulaAccepted(created.getId()));
    }
}
