package com.mcmanager.server.service;

import com.mcmanager.core.api.CurseForgeApiClient;
import com.mcmanager.core.api.ModrinthApiClient;
import com.mcmanager.core.crypto.HashUtil;
import com.mcmanager.core.crypto.MurmurHash3;
import com.mcmanager.core.model.BillOfMaterials;
import com.mcmanager.core.model.ModEntry;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

import java.io.IOException;
import java.io.InputStream;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.StandardCopyOption;
import java.util.List;
import java.util.Locale;
import java.util.UUID;

/**
 * Handles the physical mod files in {@code <data>/mods} and keeps the BOM in
 * sync. Files are hashed (SHA-1 + CurseForge fingerprint) on ingest so clients
 * can verify integrity after download.
 */
public class ModManagementService {

    private static final Logger log = LoggerFactory.getLogger(ModManagementService.class);

    public static final String ORIGIN_MODRINTH = "modrinth";
    public static final String ORIGIN_CURSEFORGE = "curseforge";
    public static final String ORIGIN_DIRECT = "direct";

    private final BomService bomService;
    private final ConfigService configService;
    private final Path modsDir;
    private final ModrinthApiClient modrinth;
    private final CurseForgeApiClient curseForge;

    public ModManagementService(BomService bomService, ConfigService configService) {
        this.bomService = bomService;
        this.configService = configService;
        this.modsDir = configService.getModsDir();
        this.modrinth = new ModrinthApiClient();
        this.curseForge = new CurseForgeApiClient(configService.getConfig().curseforgeApiKey);
    }

    public ModrinthApiClient modrinth() {
        return modrinth;
    }

    public CurseForgeApiClient curseForge() {
        return curseForge;
    }

    /** @return {@code true} if a CurseForge API key is configured (required for CF search/verify). */
    public boolean hasCurseForgeKey() {
        String key = configService.getConfig().curseforgeApiKey;
        return key != null && !key.isBlank();
    }

    /** Resolves a BOM file name to the on-disk file, or {@code null} if absent. */
    public Path getModFile(String filename) {
        Path file = safeResolve(filename);
        return file != null && Files.isRegularFile(file) ? file : null;
    }

    /**
     * Ingests an uploaded JAR (from the admin UI) into the mods folder and adds it
     * to the BOM. Replaces any existing mod with the same file name.
     *
     * @return the newly created {@link ModEntry}.
     */
    public synchronized ModEntry addMod(InputStream content, String filename, String origin)
            throws IOException {
        String safeName = sanitizeFilename(filename);
        Path target = modsDir.resolve(safeName);

        try (InputStream in = content) {
            Files.copy(in, target, StandardCopyOption.REPLACE_EXISTING);
        }

        long size = Files.size(target);
        String sha1 = HashUtil.getSha1(target);
        long murmur3 = MurmurHash3.curseForgeFingerprint(target);

        String normalizedOrigin = normalizeOrigin(origin);
        String id = switch (normalizedOrigin) {
            case ORIGIN_MODRINTH, ORIGIN_CURSEFORGE -> safeName;
            default -> UUID.randomUUID().toString();
        };

        ModEntry entry = new ModEntry(id, safeName, sha1, murmur3, normalizedOrigin, null, size);
        BillOfMaterials bom = bomService.getBom();
        ModEntry existing = bom.getModByFilename(safeName);
        if (existing != null) {
            bom.getMods().remove(existing);
        }
        bom.addMod(entry);
        bomService.save();
        log.info("Added mod {} ({} bytes, {})", safeName, size, normalizedOrigin);
        return entry;
    }

    /** Downloads a file from a URL directly into the mods folder (mod CDN installs). */
    public synchronized ModEntry installFromUrl(String url, String filename, String origin)
            throws IOException {
        java.net.URI uri = java.net.URI.create(url);
        try (InputStream in = uri.toURL().openStream()) {
            return addMod(in, filename, origin);
        }
    }

    /** Removes a mod file and its BOM entry. */
    public synchronized boolean removeMod(String filename) throws IOException {
        String safeName = sanitizeFilename(filename);
        Path file = modsDir.resolve(safeName);
        boolean deleted = Files.deleteIfExists(file);

        BillOfMaterials bom = bomService.getBom();
        boolean removedFromBom = bom.removeMod(safeName);
        if (removedFromBom) {
            bomService.save();
        }
        return deleted || removedFromBom;
    }

    /** Lists every mod currently present in the BOM. */
    public List<ModEntry> listMods() {
        return bomService.getBom().getMods();
    }

    /** Lists the files physically present in the mods folder. */
    public List<Path> listModFiles() throws IOException {
        try (var stream = Files.list(modsDir)) {
            return stream.filter(Files::isRegularFile).toList();
        }
    }

    // ------------------------------------------------------------------
    // helpers
    // ------------------------------------------------------------------

    private Path safeResolve(String filename) {
        String safeName = sanitizeFilename(filename);
        Path resolved = modsDir.resolve(safeName).normalize();
        return resolved.startsWith(modsDir) ? resolved : null;
    }

    /** Strips path separators and control characters so uploads cannot escape the mods dir. */
    private String sanitizeFilename(String filename) {
        if (filename == null) {
            throw new IllegalArgumentException("filename is required");
        }
        String base = filename.replace('\\', '/');
        int slash = base.lastIndexOf('/');
        if (slash >= 0) {
            base = base.substring(slash + 1);
        }
        base = base.replaceAll("[^A-Za-z0-9._\\-]", "_");
        if (base.isBlank()) {
            base = "mod-" + UUID.randomUUID().toString().substring(0, 8) + ".jar";
        }
        if (!base.toLowerCase(Locale.ROOT).endsWith(".jar")) {
            base = base + ".jar";
        }
        return base;
    }

    private String normalizeOrigin(String origin) {
        if (origin == null) {
            return ORIGIN_DIRECT;
        }
        return switch (origin.toLowerCase(Locale.ROOT)) {
            case "modrinth" -> ORIGIN_MODRINTH;
            case "curseforge" -> ORIGIN_CURSEFORGE;
            default -> ORIGIN_DIRECT;
        };
    }
}
