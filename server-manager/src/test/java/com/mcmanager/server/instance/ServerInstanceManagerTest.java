package com.mcmanager.server.instance;

import com.mcmanager.core.model.BillOfMaterials;
import com.mcmanager.core.model.BomJson;
import com.mcmanager.core.model.InstanceConfig;
import com.mcmanager.server.process.ConsoleStreamHandler;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.io.TempDir;

import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.List;
import java.util.Map;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertNotEquals;
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

        InstanceConfig a = manager.createInstance("Fabric Instance", "1.21.4", "fabric", "0.15.11");
        InstanceConfig b = manager.createInstance("NeoForge Instance", "1.20.4", "neoforge", "20.4.250");

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
    void assignsDistinctPlayerFacingPorts() throws Exception {
        ServerInstanceManager manager = newManager();

        InstanceConfig a = manager.createInstance("A", "1.21.4", "fabric", "");
        InstanceConfig b = manager.createInstance("B", "1.20.4", "neoforge", "");

        // Player-facing ports are distinct, in the 25565-25665 range, and disjoint
        // from the internal range.
        assertTrue(a.getExternalMcPort() > 0);
        assertTrue(a.getExternalMcPort() != b.getExternalMcPort());
        assertTrue(a.getExternalMcPort() != a.getInternalMcPort());
        assertTrue(a.getExternalMcPort() >= ServerInstanceManager.EXTERNAL_PORT_BASE);
        assertTrue(a.getExternalMcPort() <= ServerInstanceManager.EXTERNAL_PORT_MAX);
    }

    @Test
    void mainInstanceOwnsTheDefaultMinecraftPort() throws Exception {
        ServerInstanceManager manager = newManager();

        InstanceConfig main = manager.createInstance("Main", "1.21.4", "fabric", "");

        // The first (main) instance gets the classic Minecraft port 25565, which is
        // also the port the web app is served on via the multiplexer.
        assertEquals(25565, main.getExternalMcPort());
        assertEquals(ServerInstanceManager.EXTERNAL_PORT_BASE, main.getExternalMcPort());
    }

    @Test
    void canSetExternalPortManually() throws Exception {
        ServerInstanceManager manager = newManager();
        InstanceConfig a = manager.createInstance("A", "1.21.4", "fabric", "");
        InstanceConfig b = manager.createInstance("B", "1.20.4", "neoforge", "");
        assertEquals(25565, a.getExternalMcPort());
        assertEquals(25566, b.getExternalMcPort());

        // Manual override for reverse proxies.
        manager.updateExternalPort(b.getId(), 30000);
        assertEquals(30000, manager.getInstance(b.getId()).getExternalMcPort());

        // Duplicates and invalid values are rejected.
        IllegalArgumentException dup = assertThrows(IllegalArgumentException.class,
                () -> manager.updateExternalPort(b.getId(), 25565));
        assertTrue(dup.getMessage().contains("already used"));
        assertThrows(IllegalArgumentException.class, () -> manager.updateExternalPort(b.getId(), 0));
        assertThrows(IllegalArgumentException.class, () -> manager.updateExternalPort(b.getId(), 70000));
    }

    @Test
    void newInstanceDoesNotReclaimTheMainPortAfterMainDeleted() throws Exception {
        ServerInstanceManager manager = newManager();
        InstanceConfig main = manager.createInstance("Main", "1.21.4", "fabric", "");
        InstanceConfig other = manager.createInstance("Other", "1.20.4", "neoforge", "");
        assertEquals(25565, main.getExternalMcPort());
        assertEquals(25566, other.getExternalMcPort());

        // Deleting the main instance frees 25565, but a new instance must not
        // reclaim it — 25565 stays the shared web/main port.
        manager.deleteInstance(main.getId());
        InstanceConfig fresh = manager.createInstance("Fresh", "1.21.1", "fabric", "");
        assertNotEquals(ServerInstanceManager.EXTERNAL_PORT_BASE, fresh.getExternalMcPort(),
                "a new instance must not reclaim the shared main port 25565");
        assertTrue(fresh.getExternalMcPort() > ServerInstanceManager.EXTERNAL_PORT_BASE);
    }

    @Test
    void notifiesPortBindingListenerOnLifecycleChanges() throws Exception {
        ServerInstanceManager manager = newManager();
        List<String> events = new ArrayList<>();
        manager.setPortBindingListener(new ServerInstanceManager.PortBindingListener() {
            @Override
            public void onInstanceAdded(InstanceConfig config) {
                events.add("added:" + config.getName());
            }

            @Override
            public void onInstanceUpdated(InstanceConfig config) {
                events.add("updated:" + config.getName());
            }

            @Override
            public void onInstanceRemoved(String instanceId) {
                events.add("removed:" + instanceId);
            }
        });

        InstanceConfig a = manager.createInstance("A", "1.21.4", "fabric", "");
        manager.updateExternalPort(a.getId(), 29999);
        manager.deleteInstance(a.getId());

        assertEquals(List.of("added:A", "updated:A", "removed:" + a.getId()), events);
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
        assertEquals(created.getExternalMcPort(), reloaded.getExternalMcPort());
    }

    @Test
    void relocatesLegacyInternalPortsOutOfThePlayerFacingRange() throws Exception {
        Path dataDir = tempDir.resolve("server-data");
        ServerInstanceManager first = new ServerInstanceManager(dataDir, new ConsoleStreamHandler());
        InstanceConfig created = first.createInstance("Legacy", "26.2", "neoforge", "");

        // Simulate a pre-migration instance whose internal port sat inside the
        // player-facing range (the old MC_PORT_BASE was 25566).
        Path cfgFile = first.getInstanceDir(created.getId()).resolve("instance.json");
        String json = Files.readString(cfgFile)
                .replaceFirst("\"internalMcPort\": \\d+", "\"internalMcPort\": 25566");
        Files.writeString(cfgFile, json);

        ServerInstanceManager second = new ServerInstanceManager(dataDir, new ConsoleStreamHandler());
        InstanceConfig reloaded = second.getInstance(created.getId());
        assertTrue(reloaded.getInternalMcPort() >= ServerInstanceManager.MC_PORT_BASE,
                "legacy internal port must be relocated out of the player-facing range");
    }

    @Test
    void assignsPlayerFacingPortToLegacyInstancesOnLoad() throws Exception {
        Path dataDir = tempDir.resolve("server-data");
        ServerInstanceManager first = new ServerInstanceManager(dataDir, new ConsoleStreamHandler());
        InstanceConfig created = first.createInstance("Legacy", "1.20.4", "fabric", "");

        // Strip the externalMcPort field to simulate a pre-feature instance.json.
        Path cfgFile = first.getInstanceDir(created.getId()).resolve("instance.json");
        String json = Files.readString(cfgFile).replaceAll("(?m)^\\s*\"externalMcPort\": \\d+,\\n", "");
        assertFalse(json.contains("externalMcPort"), "test setup should have stripped the field");
        Files.writeString(cfgFile, json);

        ServerInstanceManager second = new ServerInstanceManager(dataDir, new ConsoleStreamHandler());
        InstanceConfig reloaded = second.getInstance(created.getId());
        assertTrue(reloaded.getExternalMcPort() > 0,
                "legacy instance must be assigned a player-facing port on load");
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
    void renameKeepsBomServerTitleInSync() throws Exception {
        ServerInstanceManager manager = newManager();
        InstanceConfig created = manager.createInstance("Original", "1.21.4", "fabric", "0.15.11");
        Path bomFile = manager.getInstanceDir(created.getId()).resolve("bom.json");

        // The BOM is written lazily on the first client connect. Renaming before
        // it exists must not create one — the lazily-created default BOM picks
        // up the current name automatically.
        manager.updateInstanceConfig(created.getId(), "Renamed", null);
        assertFalse(Files.isRegularFile(bomFile));

        // Once the BOM exists (as after a client connect), renaming propagates.
        Files.writeString(bomFile,
                BomJson.toJson(new BillOfMaterials("1.21.4", created.getModLoader(), "Original")));
        manager.updateInstanceConfig(created.getId(), "Renamed Again", null);
        assertTrue(Files.readString(bomFile).contains("\"serverTitle\": \"Renamed Again\""));

        // Version updates carry the rename through as well.
        manager.updateInstanceVersions(created.getId(), "1.21.1", "0.16.5", "Version Renamed");
        assertTrue(Files.readString(bomFile).contains("\"serverTitle\": \"Version Renamed\""));
    }

    @Test
    void updateInstanceVersionsChangesVersionsAndSyncsBom() throws Exception {
        ServerInstanceManager manager = newManager();
        InstanceConfig created = manager.createInstance("Fabric Instance", "1.21.4", "fabric", "0.15.11");

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
