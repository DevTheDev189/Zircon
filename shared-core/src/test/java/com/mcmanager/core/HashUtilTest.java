package com.mcmanager.core;

import com.mcmanager.core.crypto.HashUtil;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.io.TempDir;

import java.nio.file.Files;
import java.nio.file.Path;
import java.security.MessageDigest;

import static org.junit.jupiter.api.Assertions.assertEquals;

class HashUtilTest {

    @TempDir
    Path tempDir;

    @Test
    void sha1MatchesMessageDigest() throws Exception {
        Path file = tempDir.resolve("data.bin");
        byte[] content = new byte[20000]; // larger than the 8192 buffer
        for (int i = 0; i < content.length; i++) {
            content[i] = (byte) (i * 31);
        }
        Files.write(file, content);

        String expected = HashUtil.toHex(MessageDigest.getInstance("SHA-1").digest(content));
        assertEquals(expected, HashUtil.getSha1(file));
    }

    @Test
    void sha256MatchesMessageDigest() throws Exception {
        Path file = tempDir.resolve("data2.bin");
        Files.write(file, "hello world".getBytes());

        String expected = HashUtil.toHex(MessageDigest.getInstance("SHA-256").digest("hello world".getBytes()));
        assertEquals(expected, HashUtil.getSha256(file));
    }

    @Test
    void hashesEmptyFile() throws Exception {
        Path file = tempDir.resolve("empty.bin");
        Files.write(file, new byte[0]);

        assertEquals(HashUtil.toHex(MessageDigest.getInstance("SHA-1").digest(new byte[0])),
                HashUtil.getSha1(file));
    }
}
