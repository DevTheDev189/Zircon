package com.mcmanager.client.pack;

import java.io.File;
import java.io.IOException;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.StandardCopyOption;
import java.util.UUID;

/**
 * Local file management for shaderpacks/resourcepacks a player drags/picks in
 * directly, independent of anything the server offers. Files added this way are
 * recorded in {@link PackSelection}'s locally-added sets so {@code PackSyncEngine}
 * never prunes them just because the server doesn't list them.
 */
public class ClientPackManager {

    private ClientPackManager() {
    }

    public static Path addLocalShaderpack(Path gameDir, File source, PackSelection selection) throws IOException {
        String filename = copyIn(gameDir.resolve("shaderpacks"), source);
        selection.getLocallyAddedShaderpacks().add(filename);
        selection.save(gameDir);
        return gameDir.resolve("shaderpacks").resolve(filename);
    }

    public static Path addLocalResourcepack(Path gameDir, File source, PackSelection selection) throws IOException {
        String filename = copyIn(gameDir.resolve("resourcepacks"), source);
        selection.getLocallyAddedResourcepacks().add(filename);
        selection.save(gameDir);
        return gameDir.resolve("resourcepacks").resolve(filename);
    }

    public static void removeShaderpack(Path gameDir, String filename, PackSelection selection) throws IOException {
        Files.deleteIfExists(gameDir.resolve("shaderpacks").resolve(sanitize(filename)));
        selection.getLocallyAddedShaderpacks().remove(filename);
        if (filename.equals(selection.getActiveShaderpack())) {
            selection.setActiveShaderpack(null);
        }
        selection.save(gameDir);
    }

    public static void removeResourcepack(Path gameDir, String filename, PackSelection selection) throws IOException {
        Files.deleteIfExists(gameDir.resolve("resourcepacks").resolve(sanitize(filename)));
        selection.getLocallyAddedResourcepacks().remove(filename);
        selection.getActiveResourcepacks().remove(filename);
        selection.save(gameDir);
    }

    private static String copyIn(Path dir, File source) throws IOException {
        Files.createDirectories(dir);
        String filename = sanitize(source.getName());
        Path target = dir.resolve(filename);
        Files.copy(source.toPath(), target, StandardCopyOption.REPLACE_EXISTING);
        return filename;
    }

    private static String sanitize(String filename) {
        if (filename == null) {
            throw new IllegalArgumentException("filename is required");
        }
        String base = filename.replace('\\', '/');
        int slash = base.lastIndexOf('/');
        if (slash >= 0) {
            base = base.substring(slash + 1);
        }
        base = base.replaceAll("[^A-Za-z0-9._\\-]", "_");
        if (base.isBlank()) {
            base = "pack-" + UUID.randomUUID().toString().substring(0, 8) + ".zip";
        }
        if (!base.toLowerCase().endsWith(".zip")) {
            base = base + ".zip";
        }
        return base;
    }
}
