package com.mcmanager.client.launch;

import com.mcmanager.client.pack.PackSelection;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.io.TempDir;

import java.nio.file.Files;
import java.nio.file.Path;

import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertTrue;

/**
 * Verifies {@link PackOptionsWriter} pre-launch config generation. Iris and Oculus
 * both read their shader state from {@code config/iris.properties} in the game dir,
 * so that is the file the writer must produce for auto-enabling to actually work.
 */
class PackOptionsWriterTest {

    @TempDir
    Path gameDir;

    @Test
    void writesEnabledPackToIrisProperties() throws Exception {
        PackSelection selection = new PackSelection();
        selection.setShadersEnabled(true);
        selection.setActiveShaderpack("ComplementaryShaders.zip");
        selection.save(gameDir);

        PackOptionsWriter.apply(gameDir);

        Path irisProperties = gameDir.resolve("config").resolve("iris.properties");
        assertTrue(Files.isRegularFile(irisProperties), "config/iris.properties must be written");
        String content = Files.readString(irisProperties);
        assertTrue(content.contains("enableShaders=true"));
        assertTrue(content.contains("shaderPack=ComplementaryShaders.zip"));
    }

    @Test
    void writesDisabledStateWhenSelectionOff() throws Exception {
        PackSelection selection = new PackSelection();
        selection.setShadersEnabled(false);
        selection.setActiveShaderpack(null);
        selection.save(gameDir);

        PackOptionsWriter.apply(gameDir);

        String content = Files.readString(gameDir.resolve("config").resolve("iris.properties"));
        assertTrue(content.contains("enableShaders=false"));
        assertTrue(content.contains("shaderPack="));
        assertFalse(content.contains("shaderPack=ComplementaryShaders.zip"));
    }

    @Test
    void disabledSelectionStillWritesResourcePacksEntry() throws Exception {
        PackSelection selection = new PackSelection();
        selection.setShadersEnabled(false);
        selection.setActiveResourcepacks(java.util.List.of("Faithful.zip"));
        selection.save(gameDir);

        PackOptionsWriter.apply(gameDir);

        String content = Files.readString(gameDir.resolve("options.txt"));
        assertTrue(content.contains("\"file/Faithful.zip\""));
    }
}
