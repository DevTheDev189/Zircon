package com.mcmanager.server.auth;

import org.junit.jupiter.api.Test;

import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertTrue;

class JoinTicketManagerTest {

    @Test
    void consumeWithoutTicketFails() {
        assertFalse(JoinTicketManager.consumeTicket("Steve"));
    }

    @Test
    void ticketIsConsumedOnce() {
        JoinTicketManager.registerTicket("Steve");
        assertTrue(JoinTicketManager.consumeTicket("Steve"));
        // One-time use: a second connection with the same identity is rejected.
        assertFalse(JoinTicketManager.consumeTicket("Steve"));
    }

    @Test
    void ticketMatchesCaseInsensitively() {
        JoinTicketManager.registerTicket("Steve");
        assertTrue(JoinTicketManager.consumeTicket("steve"));
        assertFalse(JoinTicketManager.consumeTicket("STEVE"));
    }

    @Test
    void uuidTicketsWorkLikeUsernameTickets() {
        JoinTicketManager.registerTicket("0f0f0f0f-aaaa-bbbb-cccc-0f0f0f0f0f0f");
        assertTrue(JoinTicketManager.consumeTicket("0F0F0F0F-AAAA-BBBB-CCCC-0F0F0F0F0F0F"));
    }

    @Test
    void blankOrNullIdentifiersAreIgnored() {
        JoinTicketManager.registerTicket("   ");
        JoinTicketManager.registerTicket(null);
        assertFalse(JoinTicketManager.consumeTicket(null));
        assertFalse(JoinTicketManager.consumeTicket(""));
        assertFalse(JoinTicketManager.consumeTicket("   "));
    }

    @Test
    void expiredTicketIsRejected() throws Exception {
        // Package-private overload: 30 ms TTL, well below the default minute.
        JoinTicketManager.registerTicket("Steve", 30);
        Thread.sleep(80);
        assertFalse(JoinTicketManager.consumeTicket("Steve"));
    }
}
