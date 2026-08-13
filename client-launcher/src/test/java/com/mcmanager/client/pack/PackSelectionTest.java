package com.mcmanager.client.pack;

import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.io.TempDir;

import java.nio.file.Path;
import java.util.List;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertNull;
import static org.junit.jupiter.api.Assertions.assertTrue;

class PackSelectionTest {

    @TempDir
    Path gameDir;

    @Test
    void loadWithNoFileReturnsDisabledDefaults() {
        PackSelection selection = PackSelection.load(gameDir);

        assertFalse(selection.isShadersEnabled());
        assertNull(selection.getActiveShaderpack());
        assertTrue(selection.getActiveResourcepacks().isEmpty());
        assertTrue(selection.getLocallyAddedShaderpacks().isEmpty());
    }

    @Test
    void saveAndLoadRoundTrips() {
        PackSelection selection = new PackSelection();
        selection.setShadersEnabled(true);
        selection.setActiveShaderpack("ComplementaryShaders.zip");
        selection.setActiveResourcepacks(List.of("Faithful.zip", "Default32x.zip"));
        selection.getLocallyAddedShaderpacks().add("MyOwnShader.zip");

        selection.save(gameDir);
        PackSelection reloaded = PackSelection.load(gameDir);

        assertTrue(reloaded.isShadersEnabled());
        assertEquals("ComplementaryShaders.zip", reloaded.getActiveShaderpack());
        assertEquals(List.of("Faithful.zip", "Default32x.zip"), reloaded.getActiveResourcepacks());
        assertTrue(reloaded.getLocallyAddedShaderpacks().contains("MyOwnShader.zip"));
    }

    @Test
    void disablingClearsActiveShaderpack() {
        PackSelection selection = new PackSelection();
        selection.setShadersEnabled(true);
        selection.setActiveShaderpack("Sildurs.zip");
        selection.setShadersEnabled(false);
        selection.setActiveShaderpack(null);
        selection.save(gameDir);

        PackSelection reloaded = PackSelection.load(gameDir);
        assertFalse(reloaded.isShadersEnabled());
        assertNull(reloaded.getActiveShaderpack());
    }
}
