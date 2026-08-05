package com.mcmanager.core.crypto;

import java.io.IOException;
import java.nio.file.Files;
import java.nio.file.Path;

/**
 * 32-bit MurmurHash3 implementation matching CurseForge's file fingerprint
 * algorithm.
 *
 * <p>CurseForge computes fingerprints by first stripping four ASCII whitespace
 * bytes (Tab 0x09, LF 0x0A, CR 0x0D, Space 0x20) from the file bytes, then
 * running standard MurmurHash3 (x86, 32-bit) with seed {@code 1}. The result is
 * returned as an unsigned value (masked with {@code 0xFFFFFFFFL}).
 */
public final class MurmurHash3 {

    /** Seed used by CurseForge for file fingerprints. */
    public static final int CURSEFORGE_SEED = 1;

    /** Whitespace byte values ignored by CurseForge before hashing. */
    private static final int TAB = 0x09;
    private static final int LF = 0x0A;
    private static final int CR = 0x0D;
    private static final int SPACE = 0x20;

    private MurmurHash3() {
    }

    /**
     * CurseForge fingerprint of a file: strips the four whitespace byte values,
     * then runs 32-bit MurmurHash3 with seed 1.
     *
     * @return unsigned hash value in the range {@code [0, 0xFFFFFFFF]}.
     */
    public static long curseForgeFingerprint(byte[] data) {
        byte[] stripped = stripWhitespace(data);
        return murmur3_x86_32(stripped, CURSEFORGE_SEED);
    }

    /**
     * Convenience overload that reads a file from disk and computes its
     * CurseForge fingerprint.
     */
    public static long curseForgeFingerprint(Path filePath) throws IOException {
        return curseForgeFingerprint(Files.readAllBytes(filePath));
    }

    /**
     * Standard 32-bit MurmurHash3 (x86 variant) over the given data.
     *
     * @return unsigned hash value in the range {@code [0, 0xFFFFFFFF]}.
     */
    public static long murmur3_x86_32(byte[] data, int seed) {
        final int c1 = 0xcc9e2d51;
        final int c2 = 0x1b873593;
        final int length = data.length;

        int h1 = seed;
        int roundedEnd = (length & 0xfffffffc); // round down to 4 byte block

        for (int i = 0; i < roundedEnd; i += 4) {
            int k1 = (data[i] & 0xff)
                    | ((data[i + 1] & 0xff) << 8)
                    | ((data[i + 2] & 0xff) << 16)
                    | (data[i + 3] << 24);

            k1 *= c1;
            k1 = Integer.rotateLeft(k1, 15);
            k1 *= c2;

            h1 ^= k1;
            h1 = Integer.rotateLeft(h1, 13);
            h1 = h1 * 5 + 0xe6546b64;
        }

        int k1 = 0;
        switch (length & 0x03) {
            case 3:
                k1 ^= (data[roundedEnd + 2] & 0xff) << 16;
                // fall through
            case 2:
                k1 ^= (data[roundedEnd + 1] & 0xff) << 8;
                // fall through
            case 1:
                k1 ^= (data[roundedEnd] & 0xff);
                k1 *= c1;
                k1 = Integer.rotateLeft(k1, 15);
                k1 *= c2;
                h1 ^= k1;
                break;
            default:
                break;
        }

        h1 ^= length;
        h1 = fmix32(h1);

        return h1 & 0xFFFFFFFFL;
    }

    private static int fmix32(int h) {
        h ^= h >>> 16;
        h *= 0x85ebca6b;
        h ^= h >>> 13;
        h *= 0xc2b2ae35;
        h ^= h >>> 16;
        return h;
    }

    private static byte[] stripWhitespace(byte[] data) {
        int count = 0;
        for (byte b : data) {
            int v = b & 0xFF;
            if (v != TAB && v != LF && v != CR && v != SPACE) {
                count++;
            }
        }
        if (count == data.length) {
            return data;
        }
        byte[] out = new byte[count];
        int idx = 0;
        for (byte b : data) {
            int v = b & 0xFF;
            if (v != TAB && v != LF && v != CR && v != SPACE) {
                out[idx++] = b;
            }
        }
        return out;
    }
}
