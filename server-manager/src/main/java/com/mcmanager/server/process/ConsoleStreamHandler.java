package com.mcmanager.server.process;

import java.nio.file.Path;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.List;
import java.util.concurrent.CopyOnWriteArrayList;
import java.util.function.Consumer;
import java.util.function.Predicate;

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

    public enum LogLevel {
        WARN, ERROR
    }

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

    /**
     * @return the most recent lines matching the provided filter, oldest first.
     */
    public List<String> recentFilteredHistory(int maxLines, Predicate<String> filter) {
        List<String> out = new ArrayList<>();
        synchronized (history) {
            // Traverse from oldest available to newest
            int start = (historyIndex - historyCount + HISTORY_SIZE) % HISTORY_SIZE;
            for (int i = 0; i < historyCount; i++) {
                String line = history[(start + i) % HISTORY_SIZE];
                if (line != null && filter.test(line)) {
                    out.add(line);
                }
            }
        }

        // If the filtered result exceeds maxLines, trim it to the most recent ones
        if (out.size() > maxLines) {
            return out.subList(out.size() - maxLines, out.size());
        }
        return out;
    }

    /**
     * Retrieves the most recent console lines that match ANY of the specified log levels.
     * If no levels are provided, it defaults to returning all lines.
     *
     * @param maxLines The maximum number of lines to return.
     * @param levels   The log levels to filter by (e.g., LogLevel.WARN, LogLevel.ERROR).
     * @return A list of filtered console lines, oldest first.
     */
    public List<String> recentHistory(int maxLines, LogLevel... levels) {
        // If no specific levels are requested, return everything.
        if (levels == null || levels.length == 0) {
            return recentHistory(maxLines);
        }

        // Convert the varargs array to a List for easier checking
        List<LogLevel> activeFilters = Arrays.asList(levels);

        return recentFilteredHistory(maxLines, line -> {
            String upper = line.toUpperCase();

            // Check for Errors
            if (activeFilters.contains(LogLevel.ERROR)) {
                if (upper.contains("ERROR") || upper.contains("EXCEPTION") || upper.startsWith("\tAT ") || upper.startsWith("CAUSED BY: ")) {
                    return true;
                }
            }

            // Check for Warnings
            if (activeFilters.contains(LogLevel.WARN)) {
                // Catches standard Log4j [WARN] and raw standard out "WARNING:"
                if (upper.contains("WARN") || upper.contains("WARNING")) {
                    return true;
                }
            }

            return false;
        });
    }

    /**
     * Clearing the console history.
     */
    public void clearHistory() {
        synchronized (history) {
            Arrays.fill(history, null);
            historyIndex = 0;
            historyCount = 0;
        }
    }

    public PlayerTracker getPlayerTracker() {
        return playerTracker;
    }
}
