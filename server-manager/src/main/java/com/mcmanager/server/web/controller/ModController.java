package com.mcmanager.server.web.controller;

import com.mcmanager.core.api.CurseForgeApiClient;
import com.mcmanager.core.api.ModrinthApiClient;
import com.mcmanager.core.model.ModEntry;
import com.mcmanager.server.service.ModManagementService;
import io.javalin.http.Context;
import io.javalin.http.UploadedFile;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

import java.io.IOException;
import java.io.InputStream;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.HashMap;
import java.util.List;
import java.util.Map;

/**
 * REST endpoints for the mod manager tab of the admin UI: file listing, uploads,
 * downloads, provider search and remote installs.
 */
public class ModController {

    private static final Logger log = LoggerFactory.getLogger(ModController.class);

    private final ModManagementService mods;

    public ModController(ModManagementService mods) {
        this.mods = mods;
    }

    /** GET /api/mods — list of installed mods from the BOM. */
    public void listMods(Context ctx) {
        List<Map<String, Object>> result = mods.listMods().stream().map(ModEntry::toMap).toList();
        ctx.json(Map.of("mods", result));
    }

    /** GET /files/mods/{filename} — download a hosted mod JAR. */
    public void downloadMod(Context ctx) {
        String filename = ctx.pathParam("filename");
        Path file = mods.getModFile(filename);
        if (file == null) {
            ctx.status(404).result("Mod not found: " + filename);
            return;
        }
        try {
            ctx.contentType("application/java-archive");
            ctx.header("Content-Disposition", "attachment; filename=\"" + file.getFileName() + "\"");
            ctx.header("Content-Length", String.valueOf(Files.size(file)));
            ctx.result(Files.newInputStream(file));
        } catch (IOException e) {
            ctx.status(500).result("Could not stream file: " + e.getMessage());
        }
    }

    /** POST /api/mods/upload (multipart, field "file") — add a local JAR to the server. */
    public void uploadMod(Context ctx) {
        List<UploadedFile> files = ctx.uploadedFiles("file");
        if (files == null || files.isEmpty()) {
            ctx.status(400).result("No file uploaded (form field 'file')");
            return;
        }
        UploadedFile uploaded = files.get(0);
        String origin = ctx.queryParam("origin");

        try (InputStream in = uploaded.content()) {
            ModEntry entry = mods.addMod(in, uploaded.filename(), origin);
            ctx.status(201).json(entry.toMap());
        } catch (IOException e) {
            log.warn("Upload failed", e);
            ctx.status(500).result("Upload failed: " + e.getMessage());
        }
    }

    /** DELETE /api/mods/{filename} — remove a mod. */
    public void removeMod(Context ctx) {
        try {
            boolean removed = mods.removeMod(ctx.pathParam("filename"));
            if (!removed) {
                ctx.status(404).result("Mod not found");
                return;
            }
            ctx.status(204);
        } catch (IOException e) {
            ctx.status(500).result("Delete failed: " + e.getMessage());
        }
    }

    /** GET /api/mods/search?query=&mcVersion=&loader=&origin= — search Modrinth/CurseForge. */
    public void searchMods(Context ctx) {
        String query = ctx.queryParam("query");
        String mcVersion = ctx.queryParam("mcVersion");
        String loader = ctx.queryParam("loader");
        String origin = ctx.queryParam("origin");
        if (query == null) query = "";
        if (origin == null) origin = "modrinth";

        Map<String, Object> result = new HashMap<>();
        try {
            if ("curseforge".equalsIgnoreCase(origin)) {
                if (!mods.hasCurseForgeKey()) {
                    result.put("origin", "curseforge");
                    result.put("hits", List.of());
                    result.put("notice", "CurseForge API key not configured on the server. "
                            + "Add one in the Settings tab (or start the wrapper with "
                            + "-Dmcmanager.curseforgeApiKey=<KEY>) to search CurseForge.");
                    ctx.json(result);
                    return;
                }
                List<Map<String, Object>> hits = mods.curseForge()
                        .searchMods(query, mcVersion)
                        .stream().map(CurseForgeApiClient.CurseForgeMod::toMap).toList();
                result.put("origin", "curseforge");
                result.put("hits", hits);
            } else {
                List<Map<String, Object>> hits = mods.modrinth()
                        .searchMods(query, mcVersion, loader)
                        .stream().map(ModrinthApiClient.ModrinthSearchHit::toMap).toList();
                result.put("origin", "modrinth");
                result.put("hits", hits);
            }
            ctx.json(result);
        } catch (IOException | InterruptedException e) {
            if (e instanceof InterruptedException) {
                Thread.currentThread().interrupt();
            }
            ctx.status(502).result("Provider search failed: " + e.getMessage());
        }
    }

    /** GET /api/mods/modrinth/versions?projectId=&mcVersion=&loader= — version picker. */
    public void modrinthVersions(Context ctx) {
        String projectId = ctx.queryParam("projectId");
        if (projectId == null) {
            ctx.status(400).result("projectId is required");
            return;
        }
        try {
            List<Map<String, Object>> versions = mods.modrinth()
                    .listProjectVersions(projectId, ctx.queryParam("mcVersion"), ctx.queryParam("loader"))
                    .stream().map(ModrinthApiClient.ModrinthVersion::toMap).toList();
            ctx.json(Map.of("versions", versions));
        } catch (IOException | InterruptedException e) {
            if (e instanceof InterruptedException) {
                Thread.currentThread().interrupt();
            }
            ctx.status(502).result("Could not list versions: " + e.getMessage());
        }
    }

    /** GET /api/mods/curseforge/files?modId= — file picker for a CurseForge mod. */
    public void curseForgeFiles(Context ctx) {
        String modId = ctx.queryParam("modId");
        if (modId == null) {
            ctx.status(400).result("modId is required");
            return;
        }
        if (!mods.hasCurseForgeKey()) {
            ctx.status(400).result("CurseForge API key not configured on the server");
            return;
        }
        try {
            List<Map<String, Object>> files = mods.curseForge()
                    .listModFiles(Long.parseLong(modId))
                    .stream().map(CurseForgeApiClient.CurseForgeFile::toMap).toList();
            ctx.json(Map.of("files", files));
        } catch (IOException | InterruptedException e) {
            if (e instanceof InterruptedException) {
                Thread.currentThread().interrupt();
            }
            ctx.status(502).result("Could not list files: " + e.getMessage());
        } catch (NumberFormatException e) {
            ctx.status(400).result("modId must be a number");
        }
    }

    /**
     * POST /api/mods/install
     * body: {"origin":"modrinth","projectId":"...","versionId":"..."}
     *   or: {"origin":"curseforge","downloadUrl":"...","filename":"...","fileId":"..."}
     * Downloads the file from the provider CDN and hosts it locally.
     */
    public void installMod(Context ctx) {
        InstallRequest body;
        try {
            body = ctx.bodyAsClass(InstallRequest.class);
        } catch (RuntimeException e) {
            ctx.status(400).result("Invalid JSON body");
            return;
        }
        if (body == null || body.origin == null) {
            ctx.status(400).result("origin is required");
            return;
        }

        try {
            switch (body.origin.toLowerCase()) {
                case "modrinth" -> {
                    if (body.projectId == null || body.versionId == null) {
                        ctx.status(400).result("projectId and versionId are required for modrinth");
                        return;
                    }
                    List<ModrinthApiClient.ModrinthVersion> versions = mods.modrinth()
                            .listProjectVersions(body.projectId, null, null);
                    ModrinthApiClient.ModrinthVersion chosen = versions.stream()
                            .filter(v -> body.versionId.equals(v.id))
                            .findFirst()
                            .orElse(null);
                    if (chosen == null || chosen.primaryFile() == null) {
                        ctx.status(404).result("Version not found or has no downloadable file");
                        return;
                    }
                    ModrinthApiClient.ModrinthFile file = chosen.primaryFile();
                    ModEntry entry = mods.installFromUrl(file.url, file.filename,
                            ModManagementService.ORIGIN_MODRINTH);
                    entry.setId(chosen.projectId);
                    mods.modrinth().verifyHashes(List.of(entry.getSha1())); // sanity check (best effort)
                    ctx.status(201).json(entry.toMap());
                }
                case "curseforge" -> {
                    if (body.downloadUrl == null || body.filename == null) {
                        ctx.status(400).result("downloadUrl and filename are required for curseforge");
                        return;
                    }
                    ModEntry entry = mods.installFromUrl(body.downloadUrl, body.filename,
                            ModManagementService.ORIGIN_CURSEFORGE);
                    if (body.fileId != null) {
                        entry.setId(String.valueOf(body.fileId));
                    }
                    ctx.status(201).json(entry.toMap());
                }
                default -> ctx.status(400).result("origin must be 'modrinth' or 'curseforge'");
            }
        } catch (IOException | InterruptedException e) {
            if (e instanceof InterruptedException) {
                Thread.currentThread().interrupt();
            }
            ctx.status(502).result("Install failed: " + e.getMessage());
        }
    }

    public static class InstallRequest {
        public String origin;
        public String projectId;
        public String versionId;
        public String downloadUrl;
        public String filename;
        public String fileId;
    }
}
