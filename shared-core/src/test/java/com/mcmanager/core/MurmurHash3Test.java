package com.mcmanager.core;

import com.mcmanager.core.crypto.MurmurHash3;
import org.junit.jupiter.api.Test;

import java.nio.charset.StandardCharsets;

import static org.junit.jupiter.api.Assertions.assertEquals;

class MurmurHash3Test {

    /** Official MurmurHash3 x86-32 test vectors (seed 0). */
    @Test
    void knownVectorsSeedZero() {
        assertHash("", 0, 0x00000000L);
        assertHash("hello", 0, 0x248bfa47L);
        assertHash("hello, world", 0, 0x149bbb7fL);
        assertHash("The quick brown fox jumps over the lazy dog", 0, 0x2e4ff723L);
    }

    private static void assertHash(String input, int seed, long expected) {
        byte[] data = input.getBytes(StandardCharsets.UTF_8);
        assertEquals(expected, MurmurHash3.murmur3_x86_32(data, seed),
                "murmur3(\"" + input + "\", seed " + seed + ")");
    }

    @Test
    void curseForgeFingerprintStripsWhitespace() {
        String body = "PK\u0003\u0004 some zip content \r\nwith\twhitespace and  spaces ";
        byte[] raw = body.getBytes(StandardCharsets.UTF_8);

        long withWhitespace = MurmurHash3.curseForgeFingerprint(raw);
        long withoutWhitespace = MurmurHash3.murmur3_x86_32(
                body.replace("\r", "").replace("\n", "").replace("\t", "").replace(" ", "")
                        .getBytes(StandardCharsets.UTF_8),
                MurmurHash3.CURSEFORGE_SEED);

        assertEquals(withoutWhitespace, withWhitespace,
                "fingerprint must ignore 0x09/0x0A/0x0D/0x20 bytes");
    }

    @Test
    void curseForgeFingerprintOfWhitespaceOnlyEqualsEmpty() {
        byte[] whitespace = {' ', '\t', '\r', '\n', ' ', '\t'};
        assertEquals(MurmurHash3.murmur3_x86_32(new byte[0], MurmurHash3.CURSEFORGE_SEED),
                MurmurHash3.curseForgeFingerprint(whitespace));
    }

    @Test
    void fingerprintIsUnsigned() {
        // A value that would be negative as a signed int must come back positive.
        long fp = MurmurHash3.curseForgeFingerprint(
                "arbitrary content that should produce a high bit".getBytes(StandardCharsets.UTF_8));
        assertEquals(fp, fp & 0xFFFFFFFFL);
        org.junit.jupiter.api.Assertions.assertTrue(fp >= 0);
    }
}
