package com.mcmanager.server.service;

import com.mcmanager.core.api.CurseForgeApiClient;
import com.mcmanager.core.api.ModrinthApiClient;
import com.mcmanager.core.crypto.HashUtil;
import com.mcmanager.core.crypto.MurmurHash3;
import com.mcmanager.core.model.BillOfMaterials;
import com.mcmanager.core.model.ModEntry;
import com.mcmanager.core.util.SecurityUtil;
import com.google.gson.JsonArray;
import com.google.gson.JsonElement;
import com.google.gson.JsonObject;
import com.google.gson.JsonParser;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

import java.io.IOException;
import java.io.InputStream;
import java.io.InputStreamReader;
import java.net.URI;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.StandardCopyOption;
import java.util.ArrayList;
import java.util.HashMap;
import java.util.List;
import java.util.Locale;
import java.util.Map;
import java.util.UUID;
import java.util.zip.ZipEntry;
import java.util.zip.ZipFile;

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
    private final Path modsDir;
    private final String curseForgeApiKey;
    private final ModrinthApiClient modrinth;
    private final CurseForgeApiClient curseForge;

    public ModManagementService(BomService bomService, ConfigService configService) {
        this(bomService, configService.getModsDir(), configService.getConfig().curseforgeApiKey);
    }

    /** Instance-scoped variant: mods live in {@code <instance>/mods}. */
    public ModManagementService(BomService bomService, Path modsDir, String curseForgeApiKey) {
        this.bomService = bomService;
        this.modsDir = modsDir;
        this.curseForgeApiKey = curseForgeApiKey == null ? "" : curseForgeApiKey;
        this.modrinth = new ModrinthApiClient();
        this.curseForge = new CurseForgeApiClient(this.curseForgeApiKey);
    }

    public ModrinthApiClient modrinth() {
        return modrinth;
    }

    public CurseForgeApiClient curseForge() {
        return curseForge;
    }

    /** @return {@code true} if a CurseForge API key is configured (required for CF search/verify). */
    public boolean hasCurseForgeKey() {
        return curseForgeApiKey != null && !curseForgeApiKey.isBlank();
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
        if (!SecurityUtil.isSafeCdnUrl(url)) {
            throw new IOException("Rejected download URL (host is not an allowed CDN): " + url);
        }
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

    /**
     * Installs a specific Modrinth version into the mods folder and enriches the
     * resulting entry with the project's rich metadata (title, description, icon,
     * author).
     *
     * @param projectId Modrinth project id
     * @param versionId the exact version id to install (or {@code null} for the
     *                  first version matching {@code mcVersion}/{@code loaderType})
     */
    public synchronized ModEntry installModrinthVersion(String projectId, String versionId,
                                                        String mcVersion, String loaderType)
            throws IOException, InterruptedException {
        List<ModrinthApiClient.ModrinthVersion> versions =
                modrinth.listProjectVersions(projectId, mcVersion, loaderType);
        ModrinthApiClient.ModrinthVersion chosen = versions.stream()
                .filter(v -> versionId == null || versionId.equals(v.id))
                .findFirst()
                .orElse(null);
        if (chosen == null || chosen.primaryFile() == null) {
            throw new IOException("No installable Modrinth version found for project " + projectId);
        }

        ModrinthApiClient.ModrinthFile file = chosen.primaryFile();
        ModEntry entry = installFromUrl(file.url, file.filename, ORIGIN_MODRINTH);
        entry.setId(projectId);
        enrichMetadata(entry);
        bomService.save();
        return entry;
    }

    /**
     * Downloads a Modrinth modpack ({@code .mrpack}) and installs every mod listed
     * under {@code files} in its {@code modrinth.index.json} into this instance's
     * mods folder. Overrides (config/resource files bundled in the pack) are not
     * applied — only the {@code mods/} entries.
     *
     * @param projectId Modrinth modpack project id
     * @param versionId the exact version id to install (or {@code null} for the
     *                  first published version)
     */
    public synchronized Map<String, Object> installModrinthModpack(String projectId, String versionId)
            throws IOException, InterruptedException {
        List<ModrinthApiClient.ModrinthVersion> versions = modrinth.listProjectVersions(projectId, null, null);
        ModrinthApiClient.ModrinthVersion version = versions.stream()
                .filter(v -> versionId == null || versionId.equals(v.id))
                .findFirst()
                .orElseThrow(() -> new IOException("Modpack version not found"));

        ModrinthApiClient.ModrinthFile primaryFile = version.primaryFile();
        if (primaryFile == null || !primaryFile.filename.toLowerCase(Locale.ROOT).endsWith(".mrpack")) {
            throw new IOException("Selected version does not contain a valid .mrpack file");
        }
        if (!SecurityUtil.isSafeCdnUrl(primaryFile.url)) {
            throw new IOException("Rejected modpack download URL (host is not an allowed CDN): " + primaryFile.url);
        }

        Path tempMrpack = Files.createTempFile("modpack-", ".mrpack");
        int installedCount = 0;
        List<String> failedMods = new ArrayList<>();
        try {
            try (InputStream in = URI.create(primaryFile.url).toURL().openStream()) {
                Files.copy(in, tempMrpack, StandardCopyOption.REPLACE_EXISTING);
            }

            try (ZipFile zip = new ZipFile(tempMrpack.toFile())) {
                ZipEntry indexEntry = zip.getEntry("modrinth.index.json");
                if (indexEntry == null) {
                    throw new IOException("Invalid .mrpack: missing modrinth.index.json");
                }

                JsonObject indexJson;
                try (InputStreamReader reader =
                             new InputStreamReader(zip.getInputStream(indexEntry), StandardCharsets.UTF_8)) {
                    indexJson = JsonParser.parseReader(reader).getAsJsonObject();
                }

                JsonArray files = indexJson.getAsJsonArray("files");
                if (files != null) {
                    for (JsonElement element : files) {
                        JsonObject fileObj = element.getAsJsonObject();
                        String path = fileObj.has("path") ? fileObj.get("path").getAsString() : null;
                        if (path == null || !path.startsWith("mods/")) {
                            continue;
                        }
                        String filename = path.substring("mods/".length());
                        JsonArray downloads = fileObj.getAsJsonArray("downloads");
                        if (downloads == null || downloads.size() == 0) {
                            continue;
                        }
                        String downloadUrl = downloads.get(0).getAsString();
                        try {
                            installFromUrl(downloadUrl, filename, ORIGIN_MODRINTH);
                            installedCount++;
                        } catch (IOException e) {
                            log.warn("Modpack file install failed for {}: {}", filename, e.getMessage());
                            failedMods.add(filename);
                        }
                    }
                }
            }
        } finally {
            Files.deleteIfExists(tempMrpack);
        }

        Map<String, Object> result = new HashMap<>();
        result.put("installedCount", installedCount);
        result.put("failedMods", failedMods);
        result.put("message", "Installed modpack (" + installedCount + " mods)"
                + (failedMods.isEmpty() ? "" : ", " + failedMods.size() + " failed"));
        log.info("Installed Modrinth modpack {} ({} mods, {} failed)", projectId, installedCount, failedMods.size());
        return result;
    }

    /**
     * Called after an instance's Minecraft and/or loader version changes: pins the
     * new versions into the BOM and re-resolves every installed mod. Modrinth mods
     * with a compatible version are re-downloaded in place; anything without a
     * verified match is flagged {@code compatible=false} with a warning message.
     *
     * @return a summary map ({@code updatedCount}, {@code incompatibleCount},
     *         {@code updatedMods}, {@code incompatibleMods}) for the admin UI.
     */
    public synchronized Map<String, Object> syncModsForVersionChange(String newMcVersion,
                                                                     String loaderType,
                                                                     String newLoaderVersion)
            throws IOException {
        Files.createDirectories(modsDir);
        BillOfMaterials bom = bomService.getBom();
        bom.setMinecraftVersion(newMcVersion);
        if (bom.getModLoader() != null) {
            bom.getModLoader().setVersion(newLoaderVersion);
        }

        int updatedCount = 0;
        int incompatibleCount = 0;
        List<String> updatedMods = new ArrayList<>();
        List<String> incompatibleMods = new ArrayList<>();

        for (ModEntry mod : new ArrayList<>(bom.getMods())) {
            String origin = mod.getOrigin();
            boolean foundCompat = false;

            if (ORIGIN_MODRINTH.equalsIgnoreCase(origin) && mod.getId() != null) {
                try {
                    List<ModrinthApiClient.ModrinthVersion> versions =
                            modrinth.listProjectVersions(mod.getId(), newMcVersion, loaderType);
                    if (!versions.isEmpty()) {
                        ModrinthApiClient.ModrinthVersion chosen = versions.get(0);
                        ModrinthApiClient.ModrinthFile primary = chosen.primaryFile();
                        if (primary != null) {
                            Path oldFile = modsDir.resolve(mod.getFilename());
                            Files.deleteIfExists(oldFile);

                            ModEntry newEntry = installFromUrl(primary.url, primary.filename, ORIGIN_MODRINTH);
                            newEntry.setId(mod.getId());
                            // Carry the previously known metadata over, then refresh it
                            // from the provider on a best-effort basis.
                            newEntry.setTitle(mod.getTitle());
                            newEntry.setIconUrl(mod.getIconUrl());
                            newEntry.setAuthor(mod.getAuthor());
                            newEntry.setDescription(mod.getDescription());
                            newEntry.setCompatible(true);
                            enrichMetadata(newEntry);

                            bom.removeMod(mod.getFilename());
                            bom.addMod(newEntry);

                            foundCompat = true;
                            updatedCount++;
                            updatedMods.add(newEntry.getFilename());
                        }
                    }
                } catch (IOException | InterruptedException e) {
                    if (e instanceof InterruptedException) {
                        Thread.currentThread().interrupt();
                    }
                    log.warn("Auto-update failed for Modrinth mod {}: {}",
                            mod.getFilename(), e.getMessage());
                }
            }

            if (!foundCompat) {
                mod.setCompatible(false);
                mod.setWarningMessage("Unverified for MC " + newMcVersion
                        + " (" + (loaderType == null ? "?" : loaderType) + ")");
                incompatibleCount++;
                incompatibleMods.add(mod.getFilename());
            }
        }

        bomService.save();
        log.info("Version sync for MC {} / {}: {} updated, {} incompatible",
                newMcVersion, loaderType, updatedCount, incompatibleCount);

        Map<String, Object> summary = new java.util.HashMap<>();
        summary.put("updatedCount", updatedCount);
        summary.put("incompatibleCount", incompatibleCount);
        summary.put("updatedMods", updatedMods);
        summary.put("incompatibleMods", incompatibleMods);
        return summary;
    }

    /** Lists every mod currently present in the BOM. */
    public List<ModEntry> listMods() {
        return bomService.getBom().getMods();
    }

    /**
     * Lists mods excluding shader-engine mods ({@code iris}/{@code sodium}/{@code oculus}/
     * {@code embeddium}/{@code rubidium}) so they don't clutter the generic Installed Mods
     * panel — those are managed from the dedicated Shaders tab instead.
     */
    public List<ModEntry> listModsFiltered() {
        return listMods().stream()
                .filter(m -> !com.mcmanager.core.model.ShaderEngineType.SHADER_MOD_PROJECT_IDS.contains(
                        m.getId() == null ? "" : m.getId().toLowerCase(Locale.ROOT)))
                .toList();
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

    /**
     * Best-effort metadata enrichment: fetches the provider project page for the
     * entry's id and fills in title/description/icon/author. Never throws — on
     * network failure the entry keeps whatever metadata it already has.
     */
    private void enrichMetadata(ModEntry entry) {
        if (entry == null || entry.getId() == null || !ORIGIN_MODRINTH.equalsIgnoreCase(entry.getOrigin())) {
            return;
        }
        try {
            ModrinthApiClient.ModrinthProject project = modrinth.getProject(entry.getId());
            if (project != null) {
                if (project.title != null && !project.title.isBlank()) entry.setTitle(project.title);
                if (project.description != null && !project.description.isBlank()) entry.setDescription(project.description);
                if (project.iconUrl != null && !project.iconUrl.isBlank()) entry.setIconUrl(project.iconUrl);
                if (project.author != null && !project.author.isBlank()) entry.setAuthor(project.author);
            }
        } catch (IOException | InterruptedException e) {
            if (e instanceof InterruptedException) {
                Thread.currentThread().interrupt();
            }
            log.warn("Could not enrich metadata for mod {}: {}",
                    entry.getFilename(), e.getMessage());
        }
    }
}
