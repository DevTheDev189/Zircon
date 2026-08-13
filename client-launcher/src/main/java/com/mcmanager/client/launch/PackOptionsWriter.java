package com.mcmanager.client.launch;

import com.mcmanager.client.pack.PackSelection;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

import java.io.IOException;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.List;

/**
 * Applies the player's local {@link PackSelection} to the game directory right
 * before launch — never the server's full synced set, only what the player
 * explicitly opted into. Self-contained: takes just {@code gameDir} and loads
 * the selection itself, so {@link MinecraftRunner#launch} needs no new parameters.
 */
final class PackOptionsWriter {

    private static final Logger log = LoggerFactory.getLogger(PackOptionsWriter.class);

    private PackOptionsWriter() {
    }

    static void apply(Path gameDir) throws IOException {
        PackSelection selection = PackSelection.load(gameDir);
        applyShaderpack(gameDir, selection);
        applyResourcepacks(gameDir, selection);
    }

    /** Writes {@code optionsiris.txt}: only the pack the player selected, or disabled entirely. */
    private static void applyShaderpack(Path gameDir, PackSelection selection) throws IOException {
        boolean enabled = selection.isShadersEnabled() && selection.getActiveShaderpack() != null;
        Path irisOptions = gameDir.resolve("optionsiris.txt");
        OptionsFileUtil.upsertLine(irisOptions, "enableShaders=", String.valueOf(enabled));
        OptionsFileUtil.upsertLine(irisOptions, "shaderPack=", enabled ? selection.getActiveShaderpack() : "");
        log.info("Shaders: enabled={}, pack={}", enabled, enabled ? selection.getActiveShaderpack() : "(none)");
    }

    /** Writes {@code options.txt}'s {@code resourcePacks} entry from the player's checked packs, "vanilla" first. */
    private static void applyResourcepacks(Path gameDir, PackSelection selection) throws IOException {
        Path options = gameDir.resolve("options.txt");
        List<String> entries = new ArrayList<>();
        entries.add("\"vanilla\"");
        for (String filename : selection.getActiveResourcepacks()) {
            entries.add("\"file/" + filename + "\"");
        }
        OptionsFileUtil.upsertLine(options, "resourcePacks:", "[" + String.join(",", entries) + "]");
        log.info("Texture packs active: {}", selection.getActiveResourcepacks());
    }
}
