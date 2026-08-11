package com.mcmanager.server.process;

import com.google.gson.Gson;
import com.google.gson.GsonBuilder;
import com.google.gson.reflect.TypeToken;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

import java.io.IOException;
import java.lang.reflect.Type;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.Comparator;
import java.util.List;
import java.util.Locale;
import java.util.Map;
import java.util.Set;
import java.util.concurrent.ConcurrentHashMap;

/**
 * Derives the set of online players by parsing the vanilla server's console
 * messages ("X joined the game", "X left the game", "X lost connection: ...").
 * This is intentionally tolerant of log format changes: unmatched lines are
 * simply ignored.
 *
 * <p>When constructed with a {@code players.json} path, it also maintains the
 * persistent "players who have ever joined" log: the file is loaded at startup,
 * each join appends/updates an entry (name, first/last seen, join count), and
 * the file is rewritten on every change so the log survives restarts and is
 * visible to the admin UI even while the server is offline.
 */
public class PlayerTracker {

    private static final Logger log = LoggerFactory.getLogger(PlayerTracker.class);
    private static final Gson GSON = new GsonBuilder().setPrettyPrinting().create();
    private static final Type HISTORY_TYPE = new TypeToken<List<PlayerHistoryEntry>>() {
    }.getType();

    private static final String JOINED = " joined the game";
    private static final String LEFT = " left the game";
    private static final String LOST = " lost connection:";

    private final Set<String> online = ConcurrentHashMap.newKeySet();
    private final Path playersFile; // nullable → no persistence (legacy single-server)
    private final Map<String, PlayerHistoryEntry> history = new ConcurrentHashMap<>();

    public PlayerTracker() {
        this(null);
    }

    /** @param playersFile where the ever-joined log is persisted ({@code null} to disable). */
    public PlayerTracker(Path playersFile) {
        this.playersFile = playersFile;
        if (playersFile != null) {
            for (PlayerHistoryEntry entry : loadHistory(playersFile)) {
                if (entry.getName() != null && !entry.getName().isBlank()) {
                    history.put(entry.getName().toLowerCase(Locale.ROOT), entry);
                }
            }
        }
    }

    public void onLine(String line) {
        if (line == null) {
            return;
        }
        String name = null;
        boolean remove = false;

        if (line.contains(JOINED)) {
            name = line.substring(0, line.indexOf(JOINED));
        } else if (line.contains(LEFT)) {
            name = line.substring(0, line.indexOf(LEFT));
            remove = true;
        } else if (line.contains(LOST)) {
            name = line.substring(0, line.indexOf(LOST));
            remove = true;
        }
        // The vanilla server logs a prefix like "[Server thread/INFO]: " before
        // player names. Strip anything up to and including "] ".
        if (name != null) {
            int bracket = name.lastIndexOf("]: ");
            if (bracket >= 0) {
                name = name.substring(bracket + 3);
            }
            name = name.trim();
            if (!name.isEmpty()) {
                if (remove) {
                    online.remove(name);
                } else {
                    online.add(name);
                    recordJoin(name);
                }
            }
        }
    }

    public Set<String> getOnlinePlayers() {
        return Set.copyOf(online);
    }

    /** @return the ever-joined log, most recently active players first. */
    public List<PlayerHistoryEntry> getHistory() {
        return history.values().stream()
                .sorted(Comparator.comparingLong(PlayerHistoryEntry::getLastJoined).reversed())
                .toList();
    }

    /**
     * Loads a persisted ever-joined log, tolerating a missing or corrupt file.
     * Used by the admin API so history is readable even when the instance is
     * not running (and no live {@link PlayerTracker} exists).
     */
    public static List<PlayerHistoryEntry> loadHistory(Path playersFile) {
        if (playersFile == null || !Files.isRegularFile(playersFile)) {
            return List.of();
        }
        try {
            String json = Files.readString(playersFile, StandardCharsets.UTF_8);
            List<PlayerHistoryEntry> parsed = GSON.fromJson(json, HISTORY_TYPE);
            return parsed == null ? List.of() : parsed;
        } catch (IOException | RuntimeException e) {
            log.warn("Could not read player history {}, starting empty", playersFile, e);
            return List.of();
        }
    }

    // ------------------------------------------------------------------
    // internals
    // ------------------------------------------------------------------

    /** Upserts the ever-joined entry for a player and persists the log. */
    private void recordJoin(String name) {
        if (playersFile == null) {
            return; // no persistence configured → don't accumulate history
        }
        long now = System.currentTimeMillis();
        history.compute(name.toLowerCase(Locale.ROOT), (key, existing) -> {
            PlayerHistoryEntry entry = existing != null ? existing : new PlayerHistoryEntry();
            entry.setName(name);
            if (existing == null) {
                entry.setFirstJoined(now);
            }
            entry.setLastJoined(now);
            entry.setJoinCount(existing == null ? 1 : existing.getJoinCount() + 1);
            return entry;
        });
        saveHistory();
    }

    private synchronized void saveHistory() {
        if (playersFile == null) {
            return;
        }
        try {
            List<PlayerHistoryEntry> sorted = history.values().stream()
                    .sorted(Comparator.comparingLong(PlayerHistoryEntry::getLastJoined).reversed())
                    .toList();
            Files.writeString(playersFile, GSON.toJson(sorted), StandardCharsets.UTF_8);
        } catch (IOException e) {
            log.warn("Could not persist player history to {}", playersFile, e);
        }
    }
}
