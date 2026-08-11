package com.mcmanager.server.process;

import java.nio.file.Path;
import java.util.List;
import java.util.concurrent.CopyOnWriteArrayList;
import java.util.function.Consumer;

/**
 * Fan-out point for Minecraft server console output. Every line printed by the
 * server process is broadcast to registered consumers (WebSocket sessions) and
 * kept in a small ring buffer so late-joining clients see recent history.
 */
public class ConsoleStreamHandler {

    private static final int HISTORY_SIZE = 1000;

    private final CopyOnWriteArrayList<Consumer<String>> listeners = new CopyOnWriteArrayList<>();
    private final String[] history = new String[HISTORY_SIZE];
    private int historyIndex = 0;
    private int historyCount = 0;

    private final PlayerTracker playerTracker;

    /** No persistence: legacy single-server wiring. */
    public ConsoleStreamHandler() {
        this(null);
    }

    /** @param playersFile optional path for the ever-joined player log (see {@link PlayerTracker}). */
    public ConsoleStreamHandler(Path playersFile) {
        this.playerTracker = new PlayerTracker(playersFile);
    }

    public void addListener(Consumer<String> listener) {
        listeners.add(listener);
    }

    public void removeListener(Consumer<String> listener) {
        listeners.remove(listener);
    }

    /** Feeds a raw console line into the tracker and fans it out. */
    public void accept(String line) {
        playerTracker.onLine(line);

        synchronized (history) {
            history[historyIndex] = line;
            historyIndex = (historyIndex + 1) % HISTORY_SIZE;
            if (historyCount < HISTORY_SIZE) {
                historyCount++;
            }
        }
        for (Consumer<String> listener : listeners) {
            try {
                listener.accept(line);
            } catch (RuntimeException e) {
                // Never let a broken listener kill the console pipeline.
                listeners.remove(listener);
            }
        }
    }

    /** @return the most recent lines, oldest first. */
    public List<String> recentHistory(int maxLines) {
        int n = Math.min(maxLines, historyCount);
        List<String> out = new java.util.ArrayList<>(n);
        synchronized (history) {
            int start = (historyIndex - n + HISTORY_SIZE) % HISTORY_SIZE;
            for (int i = 0; i < n; i++) {
                out.add(history[(start + i) % HISTORY_SIZE]);
            }
        }
        return out;
    }

    public PlayerTracker getPlayerTracker() {
        return playerTracker;
    }
}
