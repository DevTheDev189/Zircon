package com.mcmanager.server.auth;

import java.util.Map;
import java.util.concurrent.ConcurrentHashMap;
import java.util.concurrent.Executors;
import java.util.concurrent.ScheduledExecutorService;
import java.util.concurrent.TimeUnit;

/**
 * Issues short-lived, one-time "join tickets" so the Zircon launcher can prove
 * to the server's connection gate (see {@code ProtocolDetector}) that a player
 * is joining through the official client (AGENT_PLAN_7).
 *
 * <p>The launcher registers a ticket (username and/or UUID) immediately before
 * starting the game; the TCP multiplexer consumes it when the player's login
 * handshake arrives. Tickets expire after {@link #TICKET_TTL_MS} and can be
 * consumed once — a second connection attempt with the same identity is
 * rejected, as is any attempt from a vanilla launcher.
 */
public final class JoinTicketManager {

    /**
     * 5 minutes — generous enough that a heavily modded pack on an older device
     * can finish booting and connect before the ticket expires (the launcher
     * registers the ticket right before spawning the game process).
     */
    public static final long TICKET_TTL_MS = 300_000;

    /** TTL in whole seconds, exposed to clients via the join-intent endpoint. */
    public static final long TICKET_TTL_SECONDS = TICKET_TTL_MS / 1000;

    private static final Map<String, Long> activeTickets = new ConcurrentHashMap<>();

    static {
        // Background purge of expired tickets. Daemon so it never blocks shutdown.
        ScheduledExecutorService cleaner = Executors.newSingleThreadScheduledExecutor(r -> {
            Thread thread = new Thread(r, "join-ticket-cleaner");
            thread.setDaemon(true);
            return thread;
        });
        cleaner.scheduleAtFixedRate(() -> {
            long now = System.currentTimeMillis();
            activeTickets.entrySet().removeIf(entry -> entry.getValue() < now);
        }, 30, 30, TimeUnit.SECONDS);
    }

    private JoinTicketManager() {
    }

    /** Registers a join intent for a username or UUID (case-insensitive). */
    public static void registerTicket(String identifier) {
        registerTicket(identifier, TICKET_TTL_MS);
    }

    /** Package-private so tests can exercise expiry without waiting a minute. */
    static void registerTicket(String identifier, long ttlMs) {
        if (identifier != null && !identifier.isBlank()) {
            activeTickets.put(identifier.trim().toLowerCase(),
                    System.currentTimeMillis() + ttlMs);
        }
    }

    /**
     * Checks and consumes (one-time use) the ticket for an identifier.
     *
     * @return {@code true} when a fresh ticket existed and was consumed
     */
    public static boolean consumeTicket(String identifier) {
        if (identifier == null || identifier.isBlank()) {
            return false;
        }
        String key = identifier.trim().toLowerCase();
        Long expiry = activeTickets.remove(key);
        return expiry != null && expiry > System.currentTimeMillis();
    }
}
