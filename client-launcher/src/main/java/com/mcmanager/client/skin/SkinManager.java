package com.mcmanager.client.skin;

import javafx.scene.image.Image;

import java.io.File;
import java.io.FileInputStream;
import java.io.IOException;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.StandardCopyOption;

/**
 * Stores and loads custom player PNG skin files.
 */
public class SkinManager {

    private static final Path SKIN_DIR = Path.of(System.getProperty("user.home"), ".mcmanager", "skins");
    private static final Path ACTIVE_SKIN = SKIN_DIR.resolve("active_skin.png");

    public static void saveSkin(File sourcePng) throws IOException {
        Files.createDirectories(SKIN_DIR);
        Files.copy(sourcePng.toPath(), ACTIVE_SKIN, StandardCopyOption.REPLACE_EXISTING);
    }

    public static boolean hasCustomSkin() {
        return Files.isRegularFile(ACTIVE_SKIN);
    }

    public static Path getActiveSkinPath() {
        return ACTIVE_SKIN;
    }

    public static Image loadActiveSkinImage() {
        if (hasCustomSkin()) {
            try (FileInputStream fis = new FileInputStream(ACTIVE_SKIN.toFile())) {
                return new Image(fis);
            } catch (IOException ignored) {
            }
        }
        return null;
    }

    public static void resetSkin() {
        try {
            Files.deleteIfExists(ACTIVE_SKIN);
        } catch (IOException ignored) {
        }
    }
}
