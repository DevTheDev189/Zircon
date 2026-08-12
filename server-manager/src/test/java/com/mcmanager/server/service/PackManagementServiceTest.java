package com.mcmanager.server.service;

import com.mcmanager.core.model.BillOfMaterials;
import com.mcmanager.core.model.PackEntry;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.io.TempDir;

import java.io.ByteArrayInputStream;
import java.io.IOException;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.List;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertNotNull;
import static org.junit.jupiter.api.Assertions.assertNull;
import static org.junit.jupiter.api.Assertions.assertTrue;

class PackManagementServiceTest {

    @TempDir
    Path tempDir;

    private PackManagementService newService() {
        BomService bom = new BomService(tempDir.resolve("bom.json"),
                new BillOfMaterials("1.21.4", null, "Test Instance"));
        return new PackManagementService(bom, tempDir.resolve("shaderpacks"), tempDir.resolve("resourcepacks"));
    }

    @Test
    void addShaderpackWritesFileAndBomEntry() throws IOException {
        PackManagementService packs = newService();

        PackEntry entry = packs.addShaderpack(
                new ByteArrayInputStream("fake-zip-bytes".getBytes(StandardCharsets.UTF_8)),
                "ComplementaryShaders.zip", "direct");

        assertEquals("ComplementaryShaders.zip", entry.getFilename());
        assertNotNull(entry.getSha1());
        assertTrue(Files.isRegularFile(tempDir.resolve("shaderpacks").resolve("ComplementaryShaders.zip")));

        List<PackEntry> listed = packs.listShaderpacks();
        assertEquals(1, listed.size());
        assertEquals("ComplementaryShaders.zip", listed.get(0).getFilename());
    }

    @Test
    void addResourcepackIsIndependentOfShaderpacks() throws IOException {
        PackManagementService packs = newService();

        packs.addResourcepack(new ByteArrayInputStream("tex".getBytes(StandardCharsets.UTF_8)),
                "Faithful.zip", "modrinth");

        assertEquals(1, packs.listResourcepacks().size());
        assertTrue(packs.listShaderpacks().isEmpty());
        assertTrue(Files.isRegularFile(tempDir.resolve("resourcepacks").resolve("Faithful.zip")));
    }

    @Test
    void removeShaderpackDeletesFileAndBomEntry() throws IOException {
        PackManagementService packs = newService();
        packs.addShaderpack(new ByteArrayInputStream("x".getBytes(StandardCharsets.UTF_8)),
                "Sildurs.zip", "direct");

        boolean removed = packs.removeShaderpack("Sildurs.zip");

        assertTrue(removed);
        assertTrue(packs.listShaderpacks().isEmpty());
        assertFalse(Files.exists(tempDir.resolve("shaderpacks").resolve("Sildurs.zip")));
    }

    @Test
    void removeMissingPackReturnsFalse() throws IOException {
        PackManagementService packs = newService();
        assertFalse(packs.removeResourcepack("does-not-exist.zip"));
    }

    @Test
    void filenamesAreSanitizedAndForcedToZipExtension() throws IOException {
        PackManagementService packs = newService();

        PackEntry entry = packs.addShaderpack(
                new ByteArrayInputStream("x".getBytes(StandardCharsets.UTF_8)),
                "../../evil pack", "direct");

        assertFalse(entry.getFilename().contains(".."));
        assertFalse(entry.getFilename().contains("/"));
        assertTrue(entry.getFilename().endsWith(".zip"));
    }

    @Test
    void getShaderpackFileRefusesPathEscape() throws IOException {
        PackManagementService packs = newService();
        packs.addShaderpack(new ByteArrayInputStream("x".getBytes(StandardCharsets.UTF_8)),
                "Sildurs.zip", "direct");

        assertNotNull(packs.getShaderpackFile("Sildurs.zip"));
        assertNull(packs.getShaderpackFile("../../outside.zip"));
    }
}
