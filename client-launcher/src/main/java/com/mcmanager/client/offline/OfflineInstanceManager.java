package com.mcmanager.client.offline;

import com.google.gson.Gson;
import com.google.gson.GsonBuilder;
import com.mcmanager.core.model.ModLoaderInfo;

import java.io.IOException;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.Comparator;
import java.util.List;
import java.util.UUID;
import java.util.stream.Stream;

/**
 * Storage manager for offline instances under
 * {@code ~/.mcmanager/offline_instances/}. Each instance directory contains an
 * {@code instance.json} and a {@code mods/} folder.
 */
public final class OfflineInstanceManager {

    public static final Path DEFAULT_ROOT = Path.of(
            System.getProperty("user.home"), ".mcmanager", "offline_instances");

    private static final Gson GSON = new GsonBuilder().setPrettyPrinting().create();

    /** Test override so unit tests never touch the real {@code ~/.mcmanager}. */
    private static Path rootOverride;

    private OfflineInstanceManager() {
    }

    private static Path root() {
        return rootOverride != null ? rootOverride : DEFAULT_ROOT;
    }

    /** Sets the storage root for the current JVM (used by unit tests). */
    static void setRootForTesting(Path root) {
        rootOverride = root;
    }

    /** @return all saved instances ordered by {@code lastPlayed} descending. */
    public static List<OfflineInstance> loadAll() {
        List<OfflineInstance> result = new ArrayList<>();
        Path root = root();
        if (!Files.isDirectory(root)) {
            return result;
        }
        try (Stream<Path> dirs = Files.list(root)) {
            for (Path dir : dirs.filter(Files::isDirectory).toList()) {
                Path json = dir.resolve("instance.json");
                if (!Files.isRegularFile(json)) {
                    continue;
                }
                try {
                    OfflineInstance instance = GSON.fromJson(
                            Files.readString(json, StandardCharsets.UTF_8), OfflineInstance.class);
                    if (instance != null && instance.getId() != null && !instance.getId().isBlank()) {
                        result.add(instance);
                    }
                } catch (IOException ignored) {
                    // Skip unreadable / corrupted instance.json entries.
                }
            }
        } catch (IOException ignored) {
            // Fall through and return whatever we could read.
        }
        result.sort(Comparator.comparingLong(OfflineInstance::getLastPlayed).reversed());
        return result;
    }

    /**
     * Creates and persists a new offline instance with a fresh UUID and the
     * supplied Minecraft version / mod loader configuration.
     */
    public static OfflineInstance createInstance(String name, String mcVersion,
                                                 String loaderType, String loaderVersion) throws IOException {
        OfflineInstance instance = new OfflineInstance();
        instance.setId(UUID.randomUUID().toString());
        instance.setName(name == null || name.isBlank() ? "New Instance" : name.trim());
        instance.setMinecraftVersion(mcVersion == null || mcVersion.isBlank() ? "1.20.4" : mcVersion.trim());
        instance.setModLoader(new ModLoaderInfo(
                loaderType == null || loaderType.isBlank() ? "fabric" : loaderType.trim(),
                loaderVersion == null || loaderVersion.isBlank() ? "" : loaderVersion.trim(),
                ""));
        instance.setLastPlayed(System.currentTimeMillis());
        save(instance);
        return instance;
    }

    /** Writes the instance's {@code instance.json} and creates its {@code mods/} folder. */
    public static void save(OfflineInstance instance) throws IOException {
        if (instance == null || instance.getId() == null || instance.getId().isBlank()) {
            throw new IOException("Cannot save an offline instance without an id");
        }
        Path dir = instanceDir(instance.getId());
        Files.createDirectories(dir.resolve("mods"));
        Files.writeString(dir.resolve("instance.json"), GSON.toJson(instance), StandardCharsets.UTF_8);
    }

    /** Recursively deletes the instance directory and all of its contents. */
    public static void delete(OfflineInstance instance) {
        if (instance == null || instance.getId() == null) {
            return;
        }
        Path dir = instanceDir(instance.getId());
        if (!Files.isDirectory(dir)) {
            return;
        }
        try (Stream<Path> walk = Files.walk(dir)) {
            walk.sorted(Comparator.reverseOrder()).forEach(path -> {
                try {
                    Files.deleteIfExists(path);
                } catch (IOException ignored) {
                    // Best-effort delete.
                }
            });
        } catch (IOException ignored) {
        }
    }

    /** @return the instance's root directory (does not create it). */
    public static Path instanceDir(String id) {
        String safe = id == null ? "instance" : id.replaceAll("[^A-Za-z0-9._-]", "_");
        return root().resolve(safe);
    }

    /** @return the instance's {@code mods/} directory (does not create it). */
    public static Path modsDir(OfflineInstance instance) {
        return instanceDir(instance.getId()).resolve("mods");
    }

    /** Deletes a single mod jar from the instance's {@code mods/} folder. */
    public static void deleteMod(OfflineInstance instance, String filename) throws IOException {
        if (instance == null || instance.getId() == null || filename == null || filename.isBlank()) {
            return;
        }
        Files.deleteIfExists(modsDir(instance).resolve(filename));
    }

    /** @return sorted list of {@code .jar} files in the instance's mods folder. */
    public static List<Path> listMods(OfflineInstance instance) {
        Path mods = modsDir(instance);
        if (!Files.isDirectory(mods)) {
            return List.of();
        }
        try (Stream<Path> files = Files.list(mods)) {
            return files.filter(Files::isRegularFile)
                    .filter(path -> path.getFileName().toString().toLowerCase().endsWith(".jar"))
                    .sorted()
                    .toList();
        } catch (IOException ignored) {
            return List.of();
        }
    }
}
