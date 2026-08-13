package com.mcmanager.client.auth;

import org.junit.jupiter.api.Test;

import java.nio.charset.StandardCharsets;

import static org.junit.jupiter.api.Assertions.assertArrayEquals;
import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertNull;

class MicrosoftAuthServiceTest {

    @Test
    void embeddedClientIdDecodesToConfiguredPlaceholder() {
        // Guards that EMBEDDED_CLIENT_ID_BYTES stays in sync with DEFAULT_CLIENT_ID.
        // When a real client id is embedded, update both the byte array and this assertion.
        assertEquals(MicrosoftAuthService.DEFAULT_CLIENT_ID,
                MicrosoftAuthService.decodeClientId(MicrosoftAuthService.EMBEDDED_CLIENT_ID_BYTES));
    }

    @Test
    void encodeDecodeRoundTrips() {
        String sample = "01234567-89ab-cdef-0123-456789abcdef";
        byte[] encoded = MicrosoftAuthService.encodeClientId(sample);
        assertArrayEquals(sample.getBytes(StandardCharsets.UTF_8),
                MicrosoftAuthService.decodeClientId(encoded).getBytes(StandardCharsets.UTF_8));
    }

    @Test
    void decodeHandlesNull() {
        assertNull(MicrosoftAuthService.decodeClientId(null));
    }
}
