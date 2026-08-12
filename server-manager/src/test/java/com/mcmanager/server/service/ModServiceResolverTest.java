package com.mcmanager.server.service;

import com.mcmanager.core.model.BillOfMaterials;
import com.mcmanager.core.model.ModLoaderInfo;
import com.mcmanager.server.instance.ServerInstanceManager;
import com.mcmanager.server.process.ConsoleStreamHandler;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.io.TempDir;

import java.io.ByteArrayInputStream;
import java.nio.charset.StandardCharsets;
import java.nio.file.Path;
import java.util.List;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertNotNull;
import static org.junit.jupiter.api.Assertions.assertNull;
import static org.junit.jupiter.api.Assertions.assertSame;

class ModServiceResolverTest {

    @TempDir
    Path tempDir;

    private BomService legacyBom() {
        return new BomService(tempDir.resolve("legacy").resolve("bom.json"),
                new BillOfMaterials("1.21.4", new ModLoaderInfo("fabric", "", ""), "Legacy"));
    }

    private ModServiceResolver resolver(ServerInstanceManager manager, BomService bom,
                                        ModManagementService mods) {
        PackManagementService packs = new PackManagementService(bom,
                tempDir.resolve("legacy").resolve("shaderpacks"), tempDir.resolve("legacy").resolve("resourcepacks"));
        return new ModServiceResolver(manager, bom, mods, packs, "");
    }

    @Test
    void fallsBackToLegacyServicesWithoutInstances() throws Exception {
        ServerInstanceManager manager = new ServerInstanceManager(tempDir.resolve("data"),
                new ConsoleStreamHandler());
        BomService bom = legacyBom();
        ModManagementService mods = new ModManagementService(bom, tempDir.resolve("legacy").resolve("mods"), "");

        ModServiceResolver resolver = resolver(manager, bom, mods);

        assertNull(resolver.activeInstance());
        assertSame(bom, resolver.bom());
        assertSame(mods, resolver.mods());
    }

    @Test
    void servesActiveInstanceDataFreshFromDisk() throws Exception {
        ServerInstanceManager manager = new ServerInstanceManager(tempDir.resolve("data"),
                new ConsoleStreamHandler());
        BomService bom = legacyBom();
        ModManagementService mods = new ModManagementService(bom, tempDir.resolve("legacy").resolve("mods"), "");
        ModServiceResolver resolver = resolver(manager, bom, mods);

        var created = manager.createInstance("MyServer", "26.2", "neoforge", "26.2.0.48-beta");
        assertNotNull(resolver.activeInstance());
        assertEquals(created.getId(), resolver.activeInstance().getId());

        // The instance BOM starts empty — the resolver must reflect it, not the legacy BOM.
        assertEquals(0, resolver.mods().listMods().size());

        // Ingest a mod through the same per-request path the admin UI uses.
        ModManagementService adminMods = new ModManagementService(
                new BomService(manager.getInstanceDir(created.getId()).resolve("bom.json"),
                        new BillOfMaterials("26.2", created.getModLoader(), created.getName())),
                manager.getInstanceDir(created.getId()).resolve("mods"), "");
        adminMods.addMod(new ByteArrayInputStream("fake jar bytes".getBytes(StandardCharsets.UTF_8)),
                "sodium-test.jar", "modrinth");

        // The resolver sees the change immediately (fresh disk read, no stale cache).
        List<String> names = resolver.mods().listMods().stream()
                .map(e -> e.getFilename()).toList();
        assertEquals(List.of("sodium-test.jar"), names);
    }

    @Test
    void deletingActiveInstanceRepicksAnother() throws Exception {
        ServerInstanceManager manager = new ServerInstanceManager(tempDir.resolve("data"),
                new ConsoleStreamHandler());
        BomService bom = legacyBom();
        ModManagementService mods = new ModManagementService(bom, tempDir.resolve("legacy").resolve("mods"), "");
        ModServiceResolver resolver = resolver(manager, bom, mods);

        var first = manager.createInstance("First", "1.21.4", "fabric", "");
        var second = manager.createInstance("Second", "1.21.4", "fabric", "");
        assertEquals(first.getId(), resolver.activeInstance().getId());

        manager.deleteInstance(first.getId());
        assertEquals(second.getId(), resolver.activeInstance().getId());

        manager.deleteInstance(second.getId());
        assertNull(resolver.activeInstance());
        assertSame(bom, resolver.bom());
    }

    @Test
    void activeInstanceSurvivesRestart() throws Exception {
        Path dataDir = tempDir.resolve("data");
        ServerInstanceManager first = new ServerInstanceManager(dataDir, new ConsoleStreamHandler());
        var created = first.createInstance("Persistent", "1.21.4", "fabric", "");
        assertEquals(created.getId(), first.getActiveInstance().getId());

        ServerInstanceManager second = new ServerInstanceManager(dataDir, new ConsoleStreamHandler());
        assertEquals(created.getId(), second.getActiveInstance().getId());
    }
}
