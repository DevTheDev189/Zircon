package com.mcmanager.server.process;

import java.util.Set;
import java.util.concurrent.ConcurrentHashMap;

/**
 * Derives the set of online players by parsing the vanilla server's console
 * messages ("X joined the game", "X left the game", "X lost connection: ...").
 * This is intentionally tolerant of log format changes: unmatched lines are
 * simply ignored.
 */
public class PlayerTracker {

    private static final String JOINED = " joined the game";
    private static final String LEFT = " left the game";
    private static final String LOST = " lost connection:";

    private final Set<String> online = ConcurrentHashMap.newKeySet();

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
                }
            }
        }
    }

    public Set<String> getOnlinePlayers() {
        return Set.copyOf(online);
    }
}
