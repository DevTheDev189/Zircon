package com.mcmanager.client.sync;

import com.mcmanager.core.crypto.HashUtil;
import com.mcmanager.core.crypto.MurmurHash3;
import com.mcmanager.core.model.ModEntry;

import java.io.IOException;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.Locale;

/**
 * Verifies a local mod file against the hashes pinned in a {@link ModEntry}:
 * SHA-1 for Modrinth / direct mods, CurseForge fingerprint for CurseForge mods.
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
        if (!Files.isRegularFile(file)) {
            return false;
        }
        if (entry.getSha1() != null && !entry.getSha1().isBlank()) {
            return entry.getSha1().equalsIgnoreCase(HashUtil.getSha1(file));
        }
        if (entry.getMurmur3() != 0) {
            return entry.getMurmur3() == MurmurHash3.curseForgeFingerprint(file);
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
}
