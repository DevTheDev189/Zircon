package com.mcmanager.core.crypto;

import java.io.IOException;
import java.io.InputStream;
import java.nio.file.Files;
import java.nio.file.Path;
import java.security.MessageDigest;
import java.security.NoSuchAlgorithmException;

/**
 * Streaming SHA-1 / SHA-256 helpers. Files (potentially large mod JARs) are hashed
 * through an 8 KiB buffer so memory usage stays flat regardless of file size.
 */
public final class HashUtil {

    private static final int BUFFER_SIZE = 8192;

    private HashUtil() {
    }

    /** Computes the lower-case hex SHA-1 of a file. */
    public static String getSha1(Path filePath) throws IOException {
        return hashFile(filePath, "SHA-1");
    }

    /** Computes the lower-case hex SHA-256 of a file. */
    public static String getSha256(Path filePath) throws IOException {
        return hashFile(filePath, "SHA-256");
    }

    private static String hashFile(Path filePath, String algorithm) throws IOException {
        MessageDigest digest;
        try {
            digest = MessageDigest.getInstance(algorithm);
        } catch (NoSuchAlgorithmException e) {
            // Every Java SE runtime ships SHA-1 and SHA-256.
            throw new IllegalStateException(algorithm + " not available", e);
        }
        try (InputStream in = Files.newInputStream(filePath)) {
            byte[] buffer = new byte[BUFFER_SIZE];
            int read;
            while ((read = in.read(buffer)) != -1) {
                digest.update(buffer, 0, read);
            }
        }
        return toHex(digest.digest());
    }

    public static String toHex(byte[] bytes) {
        StringBuilder sb = new StringBuilder(bytes.length * 2);
        for (byte b : bytes) {
            sb.append(Character.forDigit((b >> 4) & 0xF, 16));
            sb.append(Character.forDigit(b & 0xF, 16));
        }
        return sb.toString();
    }
}
