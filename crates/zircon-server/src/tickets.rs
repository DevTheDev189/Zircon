//! Issues short-lived, one-time "join tickets" so the Zircon launcher can prove
//! to the server's connection gate that a player is joining through the official
//! client.
//!
//! The launcher registers a ticket (username and/or UUID) immediately before
//! starting the game; the TCP multiplexer consumes it when the player's login
//! handshake arrives. Tickets expire after `TICKET_TTL_MS` and can be consumed
//! once — a second connection attempt with the same identity is rejected, as is
//! any attempt from a vanilla launcher.
//!
//! Port of `com.mcmanager.server.auth.JoinTicketManager`.

use dashmap::DashMap;
use std::time::{Duration, Instant};

/// 5 minutes — generous enough that a heavily modded pack on an older device
/// can finish booting and connect before the ticket expires (the launcher
/// registers the ticket right before spawning the game process).
pub const TICKET_TTL_MS: u64 = 300_000;

/// TTL in whole seconds, exposed to clients via the join-intent endpoint.
pub const TICKET_TTL_SECONDS: u64 = TICKET_TTL_MS / 1000;

/// How long a join-intent hold keeps a server awake without the launcher
/// refreshing it. Aligned with the ticket TTL: the launcher's last intent is
/// registered right before the game spawns, and a player must connect before
/// both expire (a heavily modded pack on an older device can take minutes to
/// boot).
pub fn join_intent_ttl() -> Duration {
    Duration::from_millis(TICKET_TTL_MS)
}

/// Upper bound on live tickets. `/api/join-intent` is a public endpoint, so a
/// remote attacker could otherwise grow the store without limit; past the cap,
/// registrations are dropped until the housekeeping task purges expired ones.
pub const MAX_TICKETS: usize = 5000;

/// In-memory join ticket store. Keys are lower-cased identifiers
/// (username or UUID); values are expiry instants.
#[derive(Default)]
pub struct JoinTicketManager {
    active_tickets: DashMap<String, Instant>,
}

impl JoinTicketManager {
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers a join intent for a username or UUID (case-insensitive).
    pub fn register_ticket(&self, identifier: &str) {
        self.register_ticket_ttl(identifier, Duration::from_millis(TICKET_TTL_MS));
    }

    /// Registers a ticket with a custom TTL (tests use short TTLs). Drops the
    /// registration when the store is full so a public endpoint cannot grow
    /// memory without bound.
    pub fn register_ticket_ttl(&self, identifier: &str, ttl: Duration) {
        let trimmed = identifier.trim();
        if trimmed.is_empty() {
            return;
        }
        if self.active_tickets.len() >= MAX_TICKETS {
            self.purge_expired();
            if self.active_tickets.len() >= MAX_TICKETS {
                tracing::warn!(
                    "Join ticket store is full ({MAX_TICKETS}); dropping registration for '{trimmed}'"
                );
                return;
            }
        }
        self.active_tickets
            .insert(trimmed.to_lowercase(), Instant::now() + ttl);
    }

    /// Checks and consumes (one-time use) the ticket for an identifier.
    ///
    /// Returns `true` when a fresh ticket existed and was consumed.
    pub fn consume_ticket(&self, identifier: &str) -> bool {
        let trimmed = identifier.trim();
        if trimmed.is_empty() {
            return false;
        }
        let key = trimmed.to_lowercase();
        match self.active_tickets.remove(&key) {
            Some((_, expiry)) => expiry > Instant::now(),
            None => false,
        }
    }

    /// Removes expired tickets. Called periodically by the scheduler.
    pub fn purge_expired(&self) {
        let now = Instant::now();
        self.active_tickets.retain(|_, expiry| *expiry > now);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ticket_is_consumed_once() {
        let manager = JoinTicketManager::new();
        manager.register_ticket("Steve");
        assert!(manager.consume_ticket("steve")); // case-insensitive
        assert!(!manager.consume_ticket("Steve")); // one-time use
    }

    #[test]
    fn expired_ticket_is_rejected() {
        let manager = JoinTicketManager::new();
        manager.register_ticket_ttl("Alex", Duration::from_millis(1));
        std::thread::sleep(Duration::from_millis(10));
        assert!(!manager.consume_ticket("Alex"));
    }

    #[test]
    fn unknown_and_blank_identifiers_are_rejected() {
        let manager = JoinTicketManager::new();
        assert!(!manager.consume_ticket("nobody"));
        assert!(!manager.consume_ticket("  "));
        assert!(!manager.consume_ticket(""));
    }

    #[test]
    fn purge_removes_only_expired() {
        let manager = JoinTicketManager::new();
        manager.register_ticket_ttl("Old", Duration::from_millis(1));
        manager.register_ticket("New");
        std::thread::sleep(Duration::from_millis(10));
        manager.purge_expired();
        assert!(!manager.consume_ticket("Old"));
        assert!(manager.consume_ticket("New"));
    }

    #[test]
    fn ticket_store_is_bounded() {
        let manager = JoinTicketManager::new();
        for i in 0..MAX_TICKETS {
            manager.register_ticket(&format!("player-{i}"));
        }
        assert_eq!(MAX_TICKETS, manager.active_tickets.len());

        // Past the cap, new registrations are dropped (not grown unboundedly).
        manager.register_ticket("overflow");
        assert_eq!(MAX_TICKETS, manager.active_tickets.len());

        // Consuming a ticket frees its slot for a new registration.
        assert!(manager.consume_ticket("player-0"));
        manager.register_ticket("new-player");
        assert_eq!(MAX_TICKETS, manager.active_tickets.len());
        assert!(manager.consume_ticket("new-player"));
    }
}
