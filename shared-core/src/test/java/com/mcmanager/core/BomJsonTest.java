package com.mcmanager.core;

import com.mcmanager.core.model.BillOfMaterials;
import com.mcmanager.core.model.BomJson;
import com.mcmanager.core.model.ModEntry;
import com.mcmanager.core.model.ModLoaderInfo;
import org.junit.jupiter.api.Test;

import java.util.List;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertNotNull;
import static org.junit.jupiter.api.Assertions.assertNull;
import static org.junit.jupiter.api.Assertions.assertTrue;

class BomJsonTest {

    @Test
    void roundTripPreservesAllFields() {
        BillOfMaterials bom = new BillOfMaterials("1.20.4",
                new ModLoaderInfo("fabric", "0.15.11", "https://meta.fabricmc.net/v2/versions/loader/1.20.4"),
                "My Cool Server");
        bom.addMod(new ModEntry("sodium", "sodium-0.5.8.jar",
                "abc123def456", 0L, "modrinth",
                "https://cdn.modrinth.com/data/sodium.jar", 512000L));
        bom.addMod(new ModEntry("some-other", "custom-mod.jar",
                null, 987654321L, "curseforge",
                "https://server/files/mods/custom-mod.jar", 1024L));

        String json = BomJson.toJson(bom);
        BillOfMaterials parsed = BomJson.fromJson(json);

        assertEquals(1, parsed.getSchemaVersion());
        assertEquals("1.20.4", parsed.getMinecraftVersion());
        assertEquals("My Cool Server", parsed.getServerTitle());
        assertNotNull(parsed.getModLoader());
        assertEquals("fabric", parsed.getModLoader().getType());
        assertEquals("0.15.11", parsed.getModLoader().getVersion());

        assertEquals(2, parsed.getMods().size());
        ModEntry sodium = parsed.getModByFilename("sodium-0.5.8.jar");
        assertNotNull(sodium);
        assertEquals("sodium", sodium.getId());
        assertEquals("modrinth", sodium.getOrigin());
        assertEquals("abc123def456", sodium.getSha1());
        assertEquals(512000L, sodium.getFileSize());

        ModEntry curse = parsed.getModByFilename("custom-mod.jar");
        assertNotNull(curse);
        assertEquals(987654321L, curse.getMurmur3());
        assertEquals("curseforge", curse.getOrigin());
    }

    @Test
    void helperQueries() {
        BillOfMaterials bom = new BillOfMaterials("1.20.4", null, "t");
        bom.addMod(new ModEntry("a", "a.jar", null, 0, "modrinth", null, 1));
        bom.addMod(new ModEntry("b", "b.jar", null, 0, "curseforge", null, 2));

        assertEquals(3L, bom.totalSizeBytes());
        assertEquals(1, bom.getModsByOrigin("modrinth").size());
        assertTrue(bom.removeMod("a.jar"));
        assertNull(bom.getModByFilename("a.jar"));
        assertEquals(List.of("b.jar"), bom.getMods().stream().map(ModEntry::getFilename).toList());
    }
}
