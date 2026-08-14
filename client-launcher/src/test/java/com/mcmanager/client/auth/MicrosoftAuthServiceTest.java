package com.mcmanager.client.auth;

import org.junit.jupiter.api.Test;

import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertNotEquals;

class MicrosoftAuthServiceTest {

    @Test
    void embeddedClientIdIsConfigured() {
        // The shipped binary embeds a real public OAuth client id so login works
        // out of the box — it must not be the "not configured" placeholder.
        String id = MicrosoftAuthService.EMBEDDED_CLIENT_ID;
        assertFalse(id == null || id.isBlank(), "embedded client id must be present");
        assertNotEquals(MicrosoftAuthService.DEFAULT_CLIENT_ID, id,
                "embedded client id must not be the placeholder");
    }
}
