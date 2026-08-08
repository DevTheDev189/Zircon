package com.mcmanager.server.web.controller;

import com.google.gson.JsonArray;
import com.google.gson.JsonObject;
import com.google.gson.JsonParser;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.io.TempDir;

import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.UUID;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertNotNull;
import static org.junit.jupiter.api.Assertions.assertTrue;

class VanillaPlayerFilesTest {

    @TempDir
    Path tempDir;

    private Path bansFile() {
        return tempDir.resolve("server").resolve("banned-players.json");
    }

    @Test
    void banWritesVanillaFormatEntryAndCreatesDirectories() throws Exception {
        Path file = bansFile(); // parent dir does not exist yet
        VanillaPlayerFiles.ban(file, "Steve", "Griefing", "1234-5678");

        assertTrue(Files.isRegularFile(file));
        JsonArray arr = JsonParser.parseString(Files.readString(file)).getAsJsonArray();
        assertEquals(1, arr.size());
        JsonObject entry = arr.get(0).getAsJsonObject();
        assertEquals("Steve", entry.get("name").getAsString());
        assertEquals("1234-5678", entry.get("uuid").getAsString());
        assertEquals("Griefing", entry.get("reason").getAsString());
        assertEquals("forever", entry.get("expires").getAsString());
        assertNotNull(entry.get("created"));
        assertNotNull(entry.get("source"));
    }

    @Test
    void banWithoutReasonUsesDefaultMessage() throws Exception {
        Path file = bansFile();
        VanillaPlayerFiles.ban(file, "Alex", null, "abc");

        JsonArray arr = JsonParser.parseString(Files.readString(file)).getAsJsonArray();
        assertEquals("Banned by an operator.", arr.get(0).getAsJsonObject().get("reason").getAsString());
    }

    @Test
    void banningSameNameReplacesExistingEntry() throws Exception {
        Path file = bansFile();
        VanillaPlayerFiles.ban(file, "Steve", "first reason", "uuid-1");
        VanillaPlayerFiles.ban(file, "steve", "second reason", "uuid-2"); // case-insensitive

        JsonArray arr = JsonParser.parseString(Files.readString(file)).getAsJsonArray();
        assertEquals(1, arr.size());
        assertEquals("second reason", arr.get(0).getAsJsonObject().get("reason").getAsString());
        assertEquals("uuid-2", arr.get(0).getAsJsonObject().get("uuid").getAsString());
    }

    @Test
    void pardonRemovesByNameCaseInsensitively() throws Exception {
        Path file = bansFile();
        VanillaPlayerFiles.ban(file, "Steve", "grief", "uuid-1");
        VanillaPlayerFiles.ban(file, "Alex", "spam", "uuid-2");

        assertTrue(VanillaPlayerFiles.pardon(file, "STEVE"));

        JsonArray arr = JsonParser.parseString(Files.readString(file)).getAsJsonArray();
        assertEquals(1, arr.size());
        assertEquals("Alex", arr.get(0).getAsJsonObject().get("name").getAsString());
    }

    @Test
    void pardonReturnsFalseForMissingFileOrUnknownName() throws Exception {
        Path file = bansFile();
        assertFalse(VanillaPlayerFiles.pardon(file, "Nobody")); // file does not exist

        VanillaPlayerFiles.ban(file, "Steve", "grief", "uuid-1");
        assertFalse(VanillaPlayerFiles.pardon(file, "Unknown"));
    }

    @Test
    void resolveUuidUsesUsercacheWhenAvailable() throws Exception {
        Path serverDir = tempDir.resolve("server");
        Files.createDirectories(serverDir);
        Files.writeString(serverDir.resolve("usercache.json"),
                "[{\"name\":\"Steve\",\"uuid\":\"real-uuid-123\"}]");

        assertEquals("real-uuid-123", VanillaPlayerFiles.resolveUuid(
                serverDir.resolve("usercache.json"), "steve"));
    }

    @Test
    void resolveUuidFallsBackToOfflineModeUuid() throws Exception {
        Path missing = tempDir.resolve("server").resolve("usercache.json");
        String uuid = VanillaPlayerFiles.resolveUuid(missing, "Steve");

        // Minecraft offline-mode UUID: UUID.nameUUIDFromBytes("OfflinePlayer:" + name)
        UUID expected = UUID.nameUUIDFromBytes("OfflinePlayer:Steve".getBytes(StandardCharsets.UTF_8));
        assertEquals(expected.toString(), uuid);
    }

    @Test
    void readArrayToleratesMissingAndMalformedFiles() throws Exception {
        assertEquals(0, VanillaPlayerFiles.readArray(bansFile()).size()); // missing

        Path bad = tempDir.resolve("bad.json");
        Files.writeString(bad, "this is not json");
        assertEquals(0, VanillaPlayerFiles.readArray(bad).size()); // malformed
    }
}
