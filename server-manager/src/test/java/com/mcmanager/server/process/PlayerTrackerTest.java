package com.mcmanager.server.process;

import org.junit.jupiter.api.Test;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertTrue;

class PlayerTrackerTest {

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
}
