package com.mcmanager.server.web.controller;

import com.google.gson.JsonArray;
import com.google.gson.JsonElement;
import com.google.gson.JsonObject;
import com.google.gson.JsonParser;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

import java.io.IOException;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.text.SimpleDateFormat;
import java.util.Date;
import java.util.UUID;

/**
 * Read/write helpers for the vanilla player JSON files ({@code whitelist.json},
 * {@code ops.json}, {@code banned-players.json}, {@code usercache.json}) that
 * live in an instance's {@code server/} directory.
 *
 * <p>Banning normally goes through the {@code ban} server command, but that
 * requires the server process to be running. These helpers let the admin UI
 * manage bans (and by extension, records) while the server is offline: entries
 * are written straight into {@code banned-players.json} and take effect on the
 * next server start — so a player can be banned before anyone has ever joined.
 */
final class VanillaPlayerFiles {

    private static final Logger log = LoggerFactory.getLogger(VanillaPlayerFiles.class);

    private VanillaPlayerFiles() {
    }

    /** Parses a JSON array file, tolerating a missing file or malformed content. */
    static JsonArray readArray(Path file) throws IOException {
        if (!Files.isRegularFile(file)) {
            return new JsonArray();
        }
        try {
            JsonElement parsed = JsonParser.parseString(Files.readString(file));
            return parsed.isJsonArray() ? parsed.getAsJsonArray() : new JsonArray();
        } catch (RuntimeException e) {
            log.warn("Could not parse {} — treating as empty", file, e);
            return new JsonArray();
        }
    }

    /**
     * Adds (or replaces) a permanent ban entry in {@code banned-players.json}.
     * The file format matches what the vanilla server writes for {@code /ban}.
     */
    static void ban(Path file, String name, String reason, String uuid) throws IOException {
        Files.createDirectories(file.getParent());
        JsonArray arr = readArray(file);
        // Replace any existing ban for the same name.
        JsonArray filtered = new JsonArray();
        for (JsonElement el : arr) {
            if (el.isJsonObject() && sameName(el.getAsJsonObject(), name)) {
                continue;
            }
            filtered.add(el);
        }

        JsonObject entry = new JsonObject();
        entry.addProperty("uuid", uuid);
        entry.addProperty("name", name);
        entry.addProperty("created", new SimpleDateFormat("yyyy-MM-dd HH:mm:ss Z").format(new Date()));
        entry.addProperty("source", "Server");
        entry.addProperty("expires", "forever");
        entry.addProperty("reason", reason == null || reason.isBlank() ? "Banned by an operator." : reason.trim());
        filtered.add(entry);
        Files.writeString(file, filtered.toString(), StandardCharsets.UTF_8);
    }

    /** Removes a ban entry by name (case-insensitive). @return {@code true} if an entry was removed. */
    static boolean pardon(Path file, String name) throws IOException {
        if (!Files.isRegularFile(file)) {
            return false;
        }
        JsonArray arr = readArray(file);
        JsonArray filtered = new JsonArray();
        boolean removed = false;
        for (JsonElement el : arr) {
            if (el.isJsonObject() && sameName(el.getAsJsonObject(), name)) {
                removed = true;
                continue;
            }
            filtered.add(el);
        }
        if (removed) {
            Files.writeString(file, filtered.toString(), StandardCharsets.UTF_8);
        }
        return removed;
    }

    private static boolean sameName(JsonObject obj, String name) {
        return name.equalsIgnoreCase(obj.has("name") ? obj.get("name").getAsString() : "");
    }

    /**
     * Resolves the best-known UUID for a player name: the real UUID from
     * {@code usercache.json} when they have joined before, otherwise the
     * deterministic offline-mode UUID (valid for offline servers).
     */
    static String resolveUuid(Path userCache, String name) {
        if (Files.isRegularFile(userCache)) {
            try {
                JsonArray arr = JsonParser.parseString(Files.readString(userCache)).getAsJsonArray();
                for (JsonElement el : arr) {
                    if (el.isJsonObject()) {
                        JsonObject obj = el.getAsJsonObject();
                        if (name.equalsIgnoreCase(obj.has("name") ? obj.get("name").getAsString() : "")
                                && obj.has("uuid")) {
                            return obj.get("uuid").getAsString();
                        }
                    }
                }
            } catch (IOException | RuntimeException e) {
                log.warn("Could not read usercache {}", userCache, e);
            }
        }
        return UUID.nameUUIDFromBytes(
                ("OfflinePlayer:" + name).getBytes(StandardCharsets.UTF_8)).toString();
    }
}
