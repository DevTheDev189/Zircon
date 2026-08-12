package com.mcmanager.server.service;

import com.mcmanager.core.crypto.HashUtil;
import com.mcmanager.core.model.BillOfMaterials;
import com.mcmanager.core.model.PackEntry;
import com.mcmanager.core.util.SecurityUtil;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

import java.io.IOException;
import java.io.InputStream;
import java.net.URI;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.StandardCopyOption;
import java.util.List;
import java.util.Locale;
import java.util.UUID;

/**
 * Handles the physical shaderpack/resourcepack files in an instance's
 * {@code shaderpacks/} and {@code resourcepacks/} folders and keeps the BOM's
 * {@code shaderpacks}/{@code resourcepacks} lists in sync, mirroring
 * {@link ModManagementService} for mods.
 *
 * <p>Unlike mods, packs are never force-applied to a client — the BOM only
 * advertises what's available to download; activation is a local per-player
 * choice made in the client launcher.
 */
public class PackManagementService {

    private static final Logger log = LoggerFactory.getLogger(PackManagementService.class);

    public static final String ORIGIN_MODRINTH = "modrinth";
    public static final String ORIGIN_DIRECT = "direct";

    private final BomService bomService;
    private final Path shaderpacksDir;
    private final Path resourcepacksDir;

    public PackManagementService(BomService bomService, Path shaderpacksDir, Path resourcepacksDir) {
        this.bomService = bomService;
        this.shaderpacksDir = shaderpacksDir;
        this.resourcepacksDir = resourcepacksDir;
    }

    // ------------------------------------------------------------------
    // Shaderpacks
    // ------------------------------------------------------------------

    public synchronized PackEntry addShaderpack(InputStream content, String filename, String origin)
            throws IOException {
        return add(content, filename, origin, shaderpacksDir, true);
    }

    public synchronized PackEntry installShaderpackFromUrl(String url, String filename, String origin)
            throws IOException {
        return installFromUrl(url, filename, origin, true);
    }

    public synchronized boolean removeShaderpack(String filename) throws IOException {
        return remove(filename, shaderpacksDir, true);
    }

    public List<PackEntry> listShaderpacks() {
        return bomService.getBom().getShaderpacks();
    }

    public Path getShaderpackFile(String filename) {
        return safeResolve(filename, shaderpacksDir);
    }

    // ------------------------------------------------------------------
    // Resourcepacks
    // ------------------------------------------------------------------

    public synchronized PackEntry addResourcepack(InputStream content, String filename, String origin)
            throws IOException {
        return add(content, filename, origin, resourcepacksDir, false);
    }

    public synchronized PackEntry installResourcepackFromUrl(String url, String filename, String origin)
            throws IOException {
        return installFromUrl(url, filename, origin, false);
    }

    public synchronized boolean removeResourcepack(String filename) throws IOException {
        return remove(filename, resourcepacksDir, false);
    }

    public List<PackEntry> listResourcepacks() {
        return bomService.getBom().getResourcepacks();
    }

    public Path getResourcepackFile(String filename) {
        return safeResolve(filename, resourcepacksDir);
    }

    // ------------------------------------------------------------------
    // Shared implementation
    // ------------------------------------------------------------------

    private PackEntry add(InputStream content, String filename, String origin, Path dir, boolean shader)
            throws IOException {
        String safeName = sanitizeFilename(filename);
        Path target = dir.resolve(safeName);
        Files.createDirectories(dir);

        try (InputStream in = content) {
            Files.copy(in, target, StandardCopyOption.REPLACE_EXISTING);
        }

        long size = Files.size(target);
        String sha1 = HashUtil.getSha1(target);
        String normalizedOrigin = normalizeOrigin(origin);
        String id = ORIGIN_MODRINTH.equals(normalizedOrigin) ? safeName : UUID.randomUUID().toString();

        PackEntry entry = new PackEntry(id, safeName, sha1, 0L, normalizedOrigin, null, size);
        BillOfMaterials bom = bomService.getBom();
        if (shader) {
            bom.removeShaderpack(safeName);
            bom.addShaderpack(entry);
        } else {
            bom.removeResourcepack(safeName);
            bom.addResourcepack(entry);
        }
        bomService.save();
        log.info("Added {} {} ({} bytes, {})", shader ? "shaderpack" : "resourcepack", safeName, size, normalizedOrigin);
        return entry;
    }

    private PackEntry installFromUrl(String url, String filename, String origin, boolean shader)
            throws IOException {
        if (!SecurityUtil.isSafeCdnUrl(url)) {
            throw new IOException("Rejected download URL (host is not an allowed CDN): " + url);
        }
        try (InputStream in = URI.create(url).toURL().openStream()) {
            PackEntry entry = add(in, filename, origin, shader ? shaderpacksDir : resourcepacksDir, shader);
            entry.setDownloadUrl(url);
            bomService.save();
            return entry;
        }
    }

    private boolean remove(String filename, Path dir, boolean shader) throws IOException {
        String safeName = sanitizeFilename(filename);
        Path file = dir.resolve(safeName);
        boolean deleted = Files.deleteIfExists(file);

        BillOfMaterials bom = bomService.getBom();
        boolean removedFromBom = shader ? bom.removeShaderpack(safeName) : bom.removeResourcepack(safeName);
        if (removedFromBom) {
            bomService.save();
        }
        return deleted || removedFromBom;
    }

    private Path safeResolve(String filename, Path dir) {
        String safeName = sanitizeFilename(filename);
        Path resolved = dir.resolve(safeName).normalize();
        return resolved.startsWith(dir) && Files.isRegularFile(resolved) ? resolved : null;
    }

    /** Strips path separators and control characters so uploads cannot escape their pack dir. */
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
            base = "pack-" + UUID.randomUUID().toString().substring(0, 8) + ".zip";
        }
        if (!base.toLowerCase(Locale.ROOT).endsWith(".zip")) {
            base = base + ".zip";
        }
        return base;
    }

    private String normalizeOrigin(String origin) {
        return ORIGIN_MODRINTH.equalsIgnoreCase(origin) ? ORIGIN_MODRINTH : ORIGIN_DIRECT;
    }
}
