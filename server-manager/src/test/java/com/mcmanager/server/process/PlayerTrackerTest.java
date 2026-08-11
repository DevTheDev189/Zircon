package com.mcmanager.server.process;

import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.io.TempDir;

import java.io.IOException;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.List;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertTrue;

class PlayerTrackerTest {

    @TempDir
    Path tempDir;

    @Test
    void tracksJoinAndLeave() {
        PlayerTracker tracker = new PlayerTracker();

        tracker.onLine("[Server thread/INFO]: Steve joined the game");
        tracker.onLine("[Server thread/INFO]: Alex joined the game");
        assertEquals(2, tracker.getOnlinePlayers().size());

        tracker.onLine("[Server thread/INFO]: Steve left the game");
        assertEquals(1, tracker.getOnlinePlayers().size());
        assertTrue(tracker.getOnlinePlayers().contains("Alex"));
    }

    @Test
    void handlesLostConnectionAndGarbageLines() {
        PlayerTracker tracker = new PlayerTracker();

        tracker.onLine("[Server thread/INFO]: Alex joined the game");
        tracker.onLine("[Server thread/INFO]: Alex lost connection: Timed out");
        assertTrue(tracker.getOnlinePlayers().isEmpty());

        // Unrelated console noise must not crash or mutate the tracker.
        tracker.onLine("[Server thread/INFO]: Done (10.001s)! For help, type \"help\"");
        tracker.onLine("[Server thread/WARN]: Can't keep up! Is the server overloaded?");
        assertTrue(tracker.getOnlinePlayers().isEmpty());
    }

    @Test
    void recordsEveryPlayerThatHasEverJoined() {
        PlayerTracker tracker = new PlayerTracker(tempDir.resolve("players.json"));

        tracker.onLine("[Server thread/INFO]: Steve joined the game");
        tracker.onLine("[Server thread/INFO]: Steve left the game");
        tracker.onLine("[Server thread/INFO]: Alex joined the game");
        tracker.onLine("[Server thread/INFO]: Steve joined the game");

        List<PlayerHistoryEntry> history = tracker.getHistory();
        assertEquals(2, history.size());

        PlayerHistoryEntry steve = history.stream()
                .filter(e -> e.getName().equalsIgnoreCase("Steve")).findFirst().orElseThrow();
        assertEquals(2, steve.getJoinCount());
        assertTrue(steve.getFirstJoined() > 0);
        assertTrue(steve.getLastJoined() >= steve.getFirstJoined());
    }

    @Test
    void historyPersistsToFileAndReloads() throws IOException {
        Path file = tempDir.resolve("players.json");
        PlayerTracker first = new PlayerTracker(file);
        first.onLine("[Server thread/INFO]: Steve joined the game");
        first.onLine("[Server thread/INFO]: Steve left the game");
        first.onLine("[Server thread/INFO]: Alex joined the game");
        first.onLine("[Server thread/INFO]: Steve joined the game");

        assertTrue(Files.isRegularFile(file));

        // A fresh tracker (e.g. after a server restart) reloads the persisted log.
        PlayerTracker second = new PlayerTracker(file);
        List<PlayerHistoryEntry> history = second.getHistory();
        assertEquals(2, history.size());
        PlayerHistoryEntry steve = history.stream()
                .filter(e -> e.getName().equalsIgnoreCase("Steve")).findFirst().orElseThrow();
        assertEquals(2, steve.getJoinCount());

        // The static loader used by the admin API sees the same data while offline.
        assertEquals(2, PlayerTracker.loadHistory(file).size());
    }

    @Test
    void corruptHistoryFileStartsEmpty() throws IOException {
        Path file = tempDir.resolve("players.json");
        Files.writeString(file, "not json at all{{{ ", StandardCharsets.UTF_8);

        PlayerTracker tracker = new PlayerTracker(file);
        assertTrue(tracker.getHistory().isEmpty());
        assertTrue(PlayerTracker.loadHistory(file).isEmpty());
    }

    @Test
    void noPersistenceWhenDisabled() {
        Path file = tempDir.resolve("players.json");
        PlayerTracker tracker = new PlayerTracker(); // legacy mode: no file
        tracker.onLine("[Server thread/INFO]: Steve joined the game");

        assertFalse(Files.exists(file));
        assertTrue(tracker.getHistory().isEmpty());
    }
}
