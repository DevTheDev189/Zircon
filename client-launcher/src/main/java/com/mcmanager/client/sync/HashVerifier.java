package com.mcmanager.client.sync;

import com.mcmanager.core.crypto.HashUtil;
import com.mcmanager.core.crypto.MurmurHash3;
import com.mcmanager.core.model.ModEntry;
import com.mcmanager.core.model.PackEntry;

import java.io.IOException;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.Locale;

/**
 * Verifies a local file against pinned SHA-1 / CurseForge-fingerprint hashes,
 * shared by {@link ModEntry} (mods) and {@link PackEntry} (shaderpacks/resourcepacks).
 */
public class HashVerifier {

    private HashVerifier() {
    }

    /**
     * Checks that {@code file} matches the hashes of {@code entry}.
     *
     * @return {@code true} if the file is present and matches, {@code false} if it
     *         is missing or fails verification.
     */
    public static boolean matches(Path file, ModEntry entry) throws IOException {
        return matches(file, entry.getSha1(), entry.getMurmur3());
    }

    /** Same check as {@link #matches(Path, ModEntry)}, for a {@link PackEntry}. */
    public static boolean matches(Path file, PackEntry entry) throws IOException {
        return matches(file, entry.getSha1(), entry.getMurmur3());
    }

    private static boolean matches(Path file, String sha1, long murmur3) throws IOException {
        if (!Files.isRegularFile(file)) {
            return false;
        }
        if (sha1 != null && !sha1.isBlank()) {
            return sha1.equalsIgnoreCase(HashUtil.getSha1(file));
        }
        if (murmur3 != 0) {
            return murmur3 == MurmurHash3.curseForgeFingerprint(file);
        }
        // No hash pinned: treat as "unknown", caller decides (strict mode aborts).
        return false;
    }

    public static boolean isModJar(String filename) {
        return filename != null
                && filename.toLowerCase(Locale.ROOT).endsWith(".jar")
                && !filename.toLowerCase(Locale.ROOT).startsWith(".") // .DS_Store etc.
                ;
    }

    public static boolean isZip(String filename) {
        return filename != null
                && filename.toLowerCase(Locale.ROOT).endsWith(".zip")
                && !filename.toLowerCase(Locale.ROOT).startsWith(".");
    }
}
