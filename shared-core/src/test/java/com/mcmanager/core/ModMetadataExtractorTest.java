package com.mcmanager.core;

import com.mcmanager.core.model.ModLoaderType;
import com.mcmanager.core.model.ModMetadata;
import com.mcmanager.core.mod.ModMetadataExtractor;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.io.TempDir;

import java.io.File;
import java.io.IOException;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.zip.ZipEntry;
import java.util.zip.ZipOutputStream;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertThrows;
import static org.junit.jupiter.api.Assertions.assertTrue;

/**
 * Verifies the {@link ModMetadataExtractor} against synthetic jars containing
 * each of the three supported metadata formats:
 * {@code fabric.mod.json}, {@code META-INF/mods.toml} and
 * {@code META-INF/neoforge.mods.toml}.
 */
class ModMetadataExtractorTest {

    @TempDir
    Path tempDir;

    private final ModMetadataExtractor extractor = new ModMetadataExtractor();

    @Test
    void extractsFabricMetadata() throws IOException {
        File jar = makeJar("fabric-mod.jar",
                entry("fabric.mod.json", """
                        {
                          "id": "sodium",
                          "name": "Sodium",
                          "version": "0.5.8",
                          "description": "Fast rendering",
                          "environment": "client"
                        }
                        """));

        ModMetadata meta = extractor.extract(jar);

        assertEquals("sodium", meta.id());
        assertEquals("Sodium", meta.name());
        assertEquals("0.5.8", meta.version());
        assertEquals("Fast rendering", meta.description());
        assertEquals(ModLoaderType.FABRIC, meta.loaderType());
        assertEquals("client", meta.normalizedEnvironment());
    }

    @Test
    void extractsFabricMetadataWithDefaults() throws IOException {
        File jar = makeJar("fabric-min.jar",
                entry("fabric.mod.json", """
                        {"id": "minimal"}
                        """));

        ModMetadata meta = extractor.extract(jar);

        assertEquals("minimal", meta.id());
        assertEquals("minimal", meta.name()); // name falls back to id
        assertEquals("0.0.0", meta.version());
        assertEquals(ModLoaderType.FABRIC, meta.loaderType());
        assertEquals("both", meta.normalizedEnvironment());
    }

    @Test
    void extractsForgeTomlMetadata() throws IOException {
        File jar = makeJar("forge-mod.jar",
                entry("META-INF/mods.toml", """
                        modLoader="javafml"
                        loaderVersion="[47,)"
                        license="MIT"

                        [[mods]]
                        modId="jei"
                        version="15.2.0.27"
                        displayName="Just Enough Items"
                        description="Show recipes in your inventory"
                        """));

        ModMetadata meta = extractor.extract(jar);

        assertEquals("jei", meta.id());
        assertEquals("Just Enough Items", meta.name());
        assertEquals("15.2.0.27", meta.version());
        assertEquals("Show recipes in your inventory", meta.description());
        assertEquals(ModLoaderType.FORGE, meta.loaderType());
        assertEquals("both", meta.normalizedEnvironment());
    }

    @Test
    void extractsNeoForgeTomlMetadata() throws IOException {
        File jar = makeJar("neoforge-mod.jar",
                entry("META-INF/neoforge.mods.toml", """
                        modLoader="javafml"
                        loaderVersion="[2,)"
                        license="MIT"

                        [[mods]]
                        modId="example"
                        version="1.0.0"
                        displayName="Example Mod"
                        """));

        ModMetadata meta = extractor.extract(jar);

        assertEquals("example", meta.id());
        assertEquals("Example Mod", meta.name());
        assertEquals("1.0.0", meta.version());
        assertEquals(ModLoaderType.NEOFORGE, meta.loaderType());
    }

    @Test
    void neoForgeMetadataWinsOverForgeWhenBothPresent() throws IOException {
        File jar = makeJar("dual-toml.jar",
                entry("META-INF/mods.toml", """
                        [[mods]]
                        modId="forge-only"
                        version="1.0.0"
                        """),
                entry("META-INF/neoforge.mods.toml", """
                        [[mods]]
                        modId="neoforge-only"
                        version="2.0.0"
                        """));

        ModMetadata meta = extractor.extract(jar);

        assertEquals("neoforge-only", meta.id());
        assertEquals(ModLoaderType.NEOFORGE, meta.loaderType());
    }

    @Test
    void rejectsJarWithoutMetadata() throws IOException {
        File jar = makeJar("empty.jar", entry("META-INF/MANIFEST.MF", "Manifest-Version: 1.0\n"));

        IllegalArgumentException ex = assertThrows(IllegalArgumentException.class, () -> extractor.extract(jar));
        assertTrue(ex.getMessage().contains("Unknown or unparseable mod jar"));
    }

    // ------------------------------------------------------------------

    private File makeJar(String name, ZipEntryData... entries) throws IOException {
        Path file = tempDir.resolve(name);
        try (ZipOutputStream zip = new ZipOutputStream(Files.newOutputStream(file))) {
            for (ZipEntryData entry : entries) {
                zip.putNextEntry(new ZipEntry(entry.name()));
                zip.write(entry.content().getBytes(StandardCharsets.UTF_8));
                zip.closeEntry();
            }
        }
        return file.toFile();
    }

    private record ZipEntryData(String name, String content) {
    }

    private ZipEntryData entry(String name, String content) {
        return new ZipEntryData(name, content);
    }
}
