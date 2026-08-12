package com.mcmanager.client.pack;

import com.google.gson.Gson;
import com.google.gson.GsonBuilder;

import java.io.IOException;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.LinkedHashSet;
import java.util.List;
import java.util.Set;

/**
 * A player's local, per-instance choice of which synced shaderpack/resourcepacks
 * to actually use — never server-driven. Packs the server offers are auto-downloaded
 * by {@code PackSyncEngine}, but nothing in this file gets applied to the game
 * until the player opts in via the "Shaders & Packs" view.
 *
 * <p>Persisted at {@code <gameDir>/pack-selection.json}, following the same
 * silent-catch Gson load/save pattern as {@code SavedServer}, but scoped to a
 * single instance's game directory instead of one global file.
 */
public class PackSelection {

    private static final String FILE_NAME = "pack-selection.json";
    private static final Gson GSON = new GsonBuilder().setPrettyPrinting().create();

    private boolean shadersEnabled;
    private String activeShaderpack;
    private List<String> activeResourcepacks = new ArrayList<>();
    private Set<String> locallyAddedShaderpacks = new LinkedHashSet<>();
    private Set<String> locallyAddedResourcepacks = new LinkedHashSet<>();

    public boolean isShadersEnabled() {
        return shadersEnabled;
    }

    public void setShadersEnabled(boolean shadersEnabled) {
        this.shadersEnabled = shadersEnabled;
    }

    public String getActiveShaderpack() {
        return activeShaderpack;
    }

    public void setActiveShaderpack(String activeShaderpack) {
        this.activeShaderpack = activeShaderpack;
    }

    public List<String> getActiveResourcepacks() {
        return activeResourcepacks;
    }

    public void setActiveResourcepacks(List<String> activeResourcepacks) {
        this.activeResourcepacks = activeResourcepacks != null ? activeResourcepacks : new ArrayList<>();
    }

    public Set<String> getLocallyAddedShaderpacks() {
        return locallyAddedShaderpacks;
    }

    public Set<String> getLocallyAddedResourcepacks() {
        return locallyAddedResourcepacks;
    }

    public static Path fileFor(Path gameDir) {
        return gameDir.resolve(FILE_NAME);
    }

    public static PackSelection load(Path gameDir) {
        Path file = fileFor(gameDir);
        if (!Files.isRegularFile(file)) {
            return new PackSelection();
        }
        try {
            String json = Files.readString(file, StandardCharsets.UTF_8);
            PackSelection loaded = GSON.fromJson(json, PackSelection.class);
            if (loaded != null) {
                if (loaded.activeResourcepacks == null) loaded.activeResourcepacks = new ArrayList<>();
                if (loaded.locallyAddedShaderpacks == null) loaded.locallyAddedShaderpacks = new LinkedHashSet<>();
                if (loaded.locallyAddedResourcepacks == null) loaded.locallyAddedResourcepacks = new LinkedHashSet<>();
                return loaded;
            }
        } catch (Exception ignored) {
        }
        return new PackSelection();
    }

    public void save(Path gameDir) {
        try {
            Files.createDirectories(gameDir);
            Files.writeString(fileFor(gameDir), GSON.toJson(this), StandardCharsets.UTF_8);
        } catch (IOException ignored) {
        }
    }
}
