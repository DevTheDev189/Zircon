package com.mcmanager.server.service;

import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.io.TempDir;

import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertTrue;

class ServerPropertiesTest {

    @TempDir
    Path tempDir;

    @Test
    void loadSaveRoundTripPreservesCommentsAndUnknownKeys() throws Exception {
        Path file = tempDir.resolve("server.properties");
        Files.writeString(file, """
                #Minecraft server properties
                #Thu Aug 06 10:00:00 UTC 2026
                motd=A Zircon Server
                unknown-key=keep-me
                max-players=20
                """, StandardCharsets.UTF_8);

        ConfigService.ServerProperties props = ConfigService.ServerProperties.load(file);
        assertEquals("A Zircon Server", props.get("motd", ""));
        assertEquals("keep-me", props.get("unknown-key", ""));

        props.set("motd", "A New Motd");
        props.set("max-players", "50");
        props.save(file);

        String saved = Files.readString(file, StandardCharsets.UTF_8);
        // Comments and untouched keys survive the round trip.
        assertTrue(saved.contains("#Minecraft server properties"));
        assertTrue(saved.contains("unknown-key=keep-me"));
        assertTrue(saved.contains("motd=A New Motd"));
        assertTrue(saved.contains("max-players=50"));
        assertEquals(5, Files.readAllLines(file).size());
    }

    @Test
    void newPropertiesCanAddKeys() throws Exception {
        Path file = tempDir.resolve("server.properties");
        ConfigService.ServerProperties props = new ConfigService.ServerProperties();
        props.set("motd", "Hello");
        props.set("difficulty", "hard");
        props.save(file);

        ConfigService.ServerProperties reloaded = ConfigService.ServerProperties.load(file);
        assertEquals("Hello", reloaded.get("motd", ""));
        assertEquals("hard", reloaded.get("difficulty", ""));
        assertEquals(2, reloaded.asMap().size());
    }
}
