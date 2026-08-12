package com.mcmanager.client.launch;

import java.io.IOException;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.List;

/**
 * Upserts a single "prefix + value" line in a Minecraft-style options file
 * ({@code options.txt} uses {@code key:value}, {@code optionsiris.txt} uses
 * {@code key=value} — the separator is just part of {@code prefix}), preserving
 * every other line. Shared by {@link MinecraftRunner} and {@code PackOptionsWriter}.
 */
final class OptionsFileUtil {

    private OptionsFileUtil() {
    }

    static void upsertLine(Path file, String prefix, String value) throws IOException {
        List<String> lines = Files.isRegularFile(file)
                ? new ArrayList<>(Files.readAllLines(file, StandardCharsets.UTF_8))
                : new ArrayList<>();
        String newLine = prefix + value;
        boolean found = false;
        for (int i = 0; i < lines.size(); i++) {
            if (lines.get(i).startsWith(prefix)) {
                lines.set(i, newLine);
                found = true;
            }
        }
        if (!found) {
            lines.add(newLine);
        }
        Files.write(file, lines, StandardCharsets.UTF_8);
    }
}
