package com.mcmanager.client.offline;

import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.io.TempDir;

import java.io.IOException;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.List;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertTrue;

class OfflineInstanceManagerTest {

    @TempDir
    Path tempDir;

    @BeforeEach
    void setUp() {
        OfflineInstanceManager.setRootForTesting(tempDir.resolve("offline_instances"));
    }

    @AfterEach
    void tearDown() {
        OfflineInstanceManager.setRootForTesting(null);
    }

    @Test
    void savesAndLoadsInstance() throws IOException {
        OfflineInstance created = OfflineInstanceManager.createInstance(
                "Test Instance", "1.20.4", "fabric", "0.15.11");

        assertTrue(Files.isRegularFile(
                OfflineInstanceManager.instanceDir(created.getId()).resolve("instance.json")));
        assertTrue(Files.isDirectory(
                OfflineInstanceManager.instanceDir(created.getId()).resolve("mods")));

        List<OfflineInstance> loaded = OfflineInstanceManager.loadAll();
        assertEquals(1, loaded.size());
        assertEquals("Test Instance", loaded.get(0).getName());
        assertEquals("1.20.4", loaded.get(0).getMinecraftVersion());
        assertEquals("fabric", loaded.get(0).getModLoader().getType());
        assertEquals("0.15.11", loaded.get(0).getModLoader().getVersion());
    }

    @Test
    void listsJarModsSortedAndIgnoresNonJars() throws IOException {
        OfflineInstance instance = OfflineInstanceManager.createInstance(
                "Modded", "1.20.4", "fabric", "0.15.11");
        Path mods = OfflineInstanceManager.modsDir(instance);
        Files.createDirectories(mods);
        Files.writeString(mods.resolve("b-mod.jar"), "b");
        Files.writeString(mods.resolve("a-mod.jar"), "a");
        Files.writeString(mods.resolve("readme.txt"), "not a mod");

        List<Path> modsList = OfflineInstanceManager.listMods(instance);
        assertEquals(2, modsList.size());
        assertEquals("a-mod.jar", modsList.get(0).getFileName().toString());
        assertEquals("b-mod.jar", modsList.get(1).getFileName().toString());
    }

    @Test
    void deletesSingleModJar() throws IOException {
        OfflineInstance instance = OfflineInstanceManager.createInstance(
                "Modded", "1.20.4", "fabric", "0.15.11");
        Path mods = OfflineInstanceManager.modsDir(instance);
        Files.createDirectories(mods);
        Files.writeString(mods.resolve("a-mod.jar"), "a");
        Files.writeString(mods.resolve("b-mod.jar"), "b");

        OfflineInstanceManager.deleteMod(instance, "a-mod.jar");

        assertEquals(List.of("b-mod.jar"),
                OfflineInstanceManager.listMods(instance).stream()
                        .map(p -> p.getFileName().toString()).toList());
    }

    @Test
    void deleteMissingModIsNoop() throws IOException {
        OfflineInstance instance = OfflineInstanceManager.createInstance(
                "Modded", "1.20.4", "fabric", "0.15.11");

        OfflineInstanceManager.deleteMod(instance, "does-not-exist.jar");

        assertTrue(OfflineInstanceManager.listMods(instance).isEmpty());
    }

    @Test
    void deletesInstanceDirectory() throws IOException {
        OfflineInstance instance = OfflineInstanceManager.createInstance(
                "Doomed", "1.20.4", "fabric", "0.15.11");
        OfflineInstanceManager.delete(instance);

        assertFalse(Files.isDirectory(OfflineInstanceManager.instanceDir(instance.getId())));
        assertTrue(OfflineInstanceManager.loadAll().isEmpty());
    }

    @Test
    void sortsByLastPlayedDescending() throws IOException {
        OfflineInstance first = OfflineInstanceManager.createInstance(
                "First", "1.20.4", "fabric", "0.15.11");
        first.setLastPlayed(1_000L);
        OfflineInstanceManager.save(first);

        OfflineInstance second = OfflineInstanceManager.createInstance(
                "Second", "1.20.4", "fabric", "0.15.11");
        second.setLastPlayed(2_000L);
        OfflineInstanceManager.save(second);

        List<OfflineInstance> loaded = OfflineInstanceManager.loadAll();
        assertEquals(2, loaded.size());
        assertEquals("Second", loaded.get(0).getName());
        assertEquals("First", loaded.get(1).getName());
    }
}
