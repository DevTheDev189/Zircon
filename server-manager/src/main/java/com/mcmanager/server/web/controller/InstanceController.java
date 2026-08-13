package com.mcmanager.server.web.controller;

import com.mcmanager.core.api.CurseForgeApiClient;
import com.mcmanager.core.api.ModrinthApiClient;
import com.mcmanager.core.model.BillOfMaterials;
import com.mcmanager.core.model.InstanceConfig;
import com.mcmanager.core.model.ModEntry;
import com.mcmanager.core.model.PackEntry;
import com.mcmanager.server.auth.JoinTicketManager;
import com.mcmanager.server.instance.ServerInstanceManager;
import com.mcmanager.server.process.MinecraftProcessManager;
import com.mcmanager.server.process.PlayerTracker;
import com.mcmanager.server.service.BomService;
import com.mcmanager.server.service.ConfigService;
import com.mcmanager.server.service.ModManagementService;
import com.mcmanager.server.service.PackManagementService;
import com.google.gson.JsonArray;
import com.google.gson.JsonElement;
import com.google.gson.JsonParser;
import io.javalin.http.Context;
import io.javalin.http.UploadedFile;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

import java.io.IOException;
import java.io.InputStream;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.HashMap;
import java.util.List;
import java.util.Map;
import java.util.concurrent.CompletableFuture;

/**
 * REST endpoints for multi-instance management (Phase 2/3 of the Zircon plan):
 * instance CRUD, start/stop, per-instance BOM and per-instance mod management.
 * The mod loader is locked at creation and no endpoint mutates it.
 */
public class InstanceController {

    private static final Logger log = LoggerFactory.getLogger(InstanceController.class);

    private final ServerInstanceManager instanceManager;
    private final String curseForgeApiKey;

    public InstanceController(ServerInstanceManager instanceManager, String curseForgeApiKey) {
        this.instanceManager = instanceManager;
        this.curseForgeApiKey = curseForgeApiKey;
    }

    // ------------------------------------------------------------------
    // Instance CRUD
    // ------------------------------------------------------------------

    /** GET /api/instances */
    public void listInstances(Context ctx) {
        List<Map<String, Object>> result = instanceManager.listInstances().stream()
                .map(this::toMap)
                .toList();
        ctx.json(Map.of("instances", result));
    }

    /** POST /api/instances — body: {name, mcVersion, loaderType, loaderVersion} */
    public void createInstance(Context ctx) {
        CreateRequest body = ctx.bodyAsClass(CreateRequest.class);
        if (body == null || body.name == null || body.name.isBlank()
                || body.mcVersion == null || body.mcVersion.isBlank()
                || body.loaderType == null || body.loaderType.isBlank()) {
            ctx.status(400).result("name, mcVersion and loaderType are required");
            return;
        }
        InstanceConfig created = instanceManager.createInstance(
                body.name.trim(), body.mcVersion.trim(),
                body.loaderType.trim().toLowerCase(),
                body.loaderVersion == null ? "" : body.loaderVersion.trim());
        ctx.status(201).json(toMap(created));
    }

    /** GET /api/instances/{id} */
    public void getInstance(Context ctx) {
        try {
            ctx.json(toMap(instanceManager.getInstance(ctx.pathParam("id"))));
        } catch (IllegalArgumentException e) {
            ctx.status(404).result(e.getMessage());
        }
    }

    /**
     * PATCH /api/instances/{id} — body: {name?, javaArgs?, mcVersion?, loaderVersion?}
     * Rename/javaArgs updates are applied directly; a Minecraft or loader version
     * change additionally re-syncs every installed mod for compatibility.
     */
    public void updateInstance(Context ctx) {
        UpdateRequest body;
        try {
            body = ctx.bodyAsClass(UpdateRequest.class);
        } catch (RuntimeException e) {
            ctx.status(400).result("Invalid JSON body");
            return;
        }
        try {
            String id = ctx.pathParam("id");
            // Manual player-facing port override (reverse proxies etc.) — rebinds
            // the multiplexer listener via the instance manager.
            if (body != null && body.externalPort > 0) {
                try {
                    instanceManager.updateExternalPort(id, body.externalPort);
                } catch (IllegalArgumentException e) {
                    ctx.status(400).result(e.getMessage());
                    return;
                }
            }
            // Backup schedule changes are independent of version re-sync.
            if (body != null && (body.backupFrequency != null || body.backupTime != null)) {
                if (!validSchedule(body.backupFrequency, body.backupTime)) {
                    ctx.status(400).result("backupFrequency must be one of off, daily, weekly, monthly "
                            + "and backupTime must be in HH:MM 24-hour format");
                    return;
                }
                instanceManager.updateBackupSchedule(id, body.backupFrequency, body.backupTime);
            }
            boolean versionChange = false;
            if (body != null) {
                InstanceConfig current = instanceManager.getInstance(id);
                String curMc = current.getMinecraftVersion();
                String curLoader = current.getModLoader() == null ? "" : current.getModLoader().getVersion();
                boolean mcChanged = body.mcVersion != null && !body.mcVersion.isBlank()
                        && !body.mcVersion.equals(curMc);
                boolean loaderChanged = body.loaderVersion != null && !body.loaderVersion.isBlank()
                        && !body.loaderVersion.equals(curLoader);
                versionChange = mcChanged || loaderChanged;
            }
            if (versionChange) {
                // Keep javaArgs changes from getting lost in the version-sync path.
                if (body.javaArgs != null) {
                    instanceManager.updateInstanceConfig(id, null, body.javaArgs);
                }
                Map<String, Object> syncResult = instanceManager.updateInstanceVersions(id,
                        body.mcVersion, body.loaderVersion, body.name);
                Map<String, Object> response = new HashMap<>(syncResult);
                response.put("instance", toMap(instanceManager.getInstance(id)));
                ctx.json(response);
            } else {
                instanceManager.updateInstanceConfig(id,
                        body == null ? null : body.name,
                        body == null ? null : body.javaArgs);
                ctx.json(toMap(instanceManager.getInstance(id)));
            }
        } catch (IllegalArgumentException e) {
            ctx.status(404).result(e.getMessage());
        } catch (IOException e) {
            ctx.status(500).result("Version update failed: " + e.getMessage());
        }
    }

    /** DELETE /api/instances/{id} */
    public void deleteInstance(Context ctx) {
        try {
            boolean deleted = instanceManager.deleteInstance(ctx.pathParam("id"));
            if (!deleted) {
                ctx.status(404).result("Instance not found");
                return;
            }
            ctx.status(204);
        } catch (IOException e) {
            ctx.status(500).result("Delete failed: " + e.getMessage());
        }
    }

    /** POST /api/instances/{id}/start */
    public void startInstance(Context ctx) {
        try {
            instanceManager.startInstance(ctx.pathParam("id"));
            ctx.json(Map.of("ok", true));
        } catch (IllegalArgumentException e) {
            ctx.status(404).result(e.getMessage());
        } catch (IllegalStateException e) {
            ctx.status(409).result(e.getMessage());
        } catch (IOException e) {
            ctx.status(500).result("Start failed: " + e.getMessage());
        }
    }

    /** POST /api/instances/{id}/stop */
    public void stopInstance(Context ctx) {
        instanceManager.stopInstance(ctx.pathParam("id"));
        ctx.json(Map.of("ok", true));
    }

    /** POST /api/instances/{id}/restart — stops the instance, then starts it again shortly after. */
    public void restartInstance(Context ctx) {
        String id = ctx.pathParam("id");
        try {
            instanceManager.getInstance(id); // 404 for unknown ids
        } catch (IllegalArgumentException e) {
            ctx.status(404).result(e.getMessage());
            return;
        }

        instanceManager.stopInstance(id);
        CompletableFuture.runAsync(() -> {
            try {
                Thread.sleep(1500); // allow OS process cleanup before restart
                instanceManager.startInstance(id);
            } catch (Exception e) {
                log.error("Failed to restart instance {}", id, e);
            }
        });
        ctx.json(Map.of("ok", true, "message", "Server is restarting..."));
    }

    /** GET /api/instances/{id}/eula — EULA acceptance status. */
    public void getEula(Context ctx) {
        try {
            String id = ctx.pathParam("id");
            instanceManager.getInstance(id); // 404 for unknown ids
            ctx.json(Map.of("accepted", instanceManager.isEulaAccepted(id),
                    "eulaUrl", "https://aka.ms/MinecraftEULA"));
        } catch (IllegalArgumentException e) {
            ctx.status(404).result(e.getMessage());
        }
    }

    /** POST /api/instances/{id}/eula — body: {"accepted":true} records EULA consent. */
    public void acceptEula(Context ctx) {
        EulaRequest body;
        try {
            body = ctx.bodyAsClass(EulaRequest.class);
        } catch (RuntimeException e) {
            ctx.status(400).result("Invalid JSON body");
            return;
        }
        if (body == null || !body.accepted) {
            ctx.status(400).result("{\"accepted\":true} is required to accept the EULA");
            return;
        }
        try {
            String id = ctx.pathParam("id");
            instanceManager.acceptEula(id);
            ctx.json(Map.of("accepted", true));
        } catch (IllegalArgumentException e) {
            ctx.status(404).result(e.getMessage());
        } catch (IOException e) {
            ctx.status(500).result("Could not write eula.txt: " + e.getMessage());
        }
    }

    // ------------------------------------------------------------------
    // Per-instance server.properties
    // ------------------------------------------------------------------

    /** GET /api/instances/{id}/server-properties — the instance's server.properties as a map. */
    public void getServerProperties(Context ctx) {
        try {
            String id = ctx.pathParam("id");
            instanceManager.getInstance(id); // 404 for unknown ids
            Path propsFile = instanceManager.getInstanceDir(id).resolve("server").resolve("server.properties");
            ConfigService.ServerProperties props = Files.isRegularFile(propsFile)
                    ? ConfigService.ServerProperties.load(propsFile)
                    : new ConfigService.ServerProperties();
            ctx.json(Map.of("properties", props.asMap()));
        } catch (IllegalArgumentException e) {
            ctx.status(404).result(e.getMessage());
        } catch (IOException e) {
            ctx.status(500).result("Could not read server.properties: " + e.getMessage());
        }
    }

    /**
     * POST /api/instances/{id}/server-properties — body: {"properties":{"motd":"...",...}}
     * Applies a partial update, preserving comments and untouched keys.
     */
    public void saveServerProperties(Context ctx) {
        ServerPropertiesRequest body;
        try {
            body = ctx.bodyAsClass(ServerPropertiesRequest.class);
        } catch (RuntimeException e) {
            ctx.status(400).result("Invalid JSON body");
            return;
        }
        if (body == null || body.properties == null || body.properties.isEmpty()) {
            ctx.status(400).result("properties map is required");
            return;
        }
        try {
            String id = ctx.pathParam("id");
            instanceManager.getInstance(id); // 404 for unknown ids
            Path serverDir = instanceManager.getInstanceDir(id).resolve("server");
            Files.createDirectories(serverDir);
            Path propsFile = serverDir.resolve("server.properties");
            ConfigService.ServerProperties props = Files.isRegularFile(propsFile)
                    ? ConfigService.ServerProperties.load(propsFile)
                    : new ConfigService.ServerProperties();
            body.properties.forEach(props::set);
            props.save(propsFile);
            ctx.json(Map.of("ok", true));
        } catch (IllegalArgumentException e) {
            ctx.status(404).result(e.getMessage());
        } catch (IOException e) {
            ctx.status(500).result("Could not save server.properties: " + e.getMessage());
        }
    }

    // ------------------------------------------------------------------
    // Per-instance player management
    // ------------------------------------------------------------------

    /** GET /api/instances/{id}/players/online — names of players currently connected. */
    public void onlinePlayers(Context ctx) {
        try {
            String id = ctx.pathParam("id");
            instanceManager.getInstance(id); // 404 for unknown ids
            ctx.json(Map.of("players", List.copyOf(instanceManager.getOnlinePlayers(id))));
        } catch (IllegalArgumentException e) {
            ctx.status(404).result(e.getMessage());
        }
    }

    /** GET /api/instances/{id}/players/history — every player that has ever joined (persisted). */
    public void playerHistory(Context ctx) {
        try {
            String id = ctx.pathParam("id");
            instanceManager.getInstance(id); // 404 for unknown ids
            Path historyFile = instanceManager.getInstanceDir(id).resolve("players.json");
            List<Map<String, Object>> players = PlayerTracker.loadHistory(historyFile).stream()
                    .map(com.mcmanager.server.process.PlayerHistoryEntry::toMap).toList();
            ctx.json(Map.of("players", players));
        } catch (IllegalArgumentException e) {
            ctx.status(404).result(e.getMessage());
        }
    }

    /** GET /api/instances/{id}/players/whitelist — contents of the instance's whitelist.json. */
    public void getWhitelist(Context ctx) {
        try {
            String id = ctx.pathParam("id");
            instanceManager.getInstance(id);
            ctx.json(Map.of("players", readPlayerJson(id, "whitelist.json")));
        } catch (IllegalArgumentException e) {
            ctx.status(404).result(e.getMessage());
        }
    }

    /** POST /api/instances/{id}/players/whitelist — body: {"name":"Steve"}. */
    public void addWhitelist(Context ctx) {
        PlayerActionRequest body = ctx.bodyAsClass(PlayerActionRequest.class);
        if (body == null || blank(body.name)) {
            ctx.status(400).result("name is required");
            return;
        }
        try {
            instanceManager.getInstance(ctx.pathParam("id"));
            ctx.json(sendInstanceCommand(ctx.pathParam("id"), "whitelist add " + body.name));
        } catch (IllegalArgumentException e) {
            ctx.status(404).result(e.getMessage());
        }
    }

    /** DELETE /api/instances/{id}/players/whitelist/{name} */
    public void removeWhitelist(Context ctx) {
        try {
            instanceManager.getInstance(ctx.pathParam("id"));
            ctx.json(sendInstanceCommand(ctx.pathParam("id"), "whitelist remove " + ctx.pathParam("name")));
        } catch (IllegalArgumentException e) {
            ctx.status(404).result(e.getMessage());
        }
    }

    /** GET /api/instances/{id}/players/ops — contents of the instance's ops.json. */
    public void getOps(Context ctx) {
        try {
            String id = ctx.pathParam("id");
            instanceManager.getInstance(id);
            ctx.json(Map.of("players", readPlayerJson(id, "ops.json")));
        } catch (IllegalArgumentException e) {
            ctx.status(404).result(e.getMessage());
        }
    }

    /** POST /api/instances/{id}/players/ops — body: {"name":"Steve"}. */
    public void addOp(Context ctx) {
        PlayerActionRequest body = ctx.bodyAsClass(PlayerActionRequest.class);
        if (body == null || blank(body.name)) {
            ctx.status(400).result("name is required");
            return;
        }
        try {
            instanceManager.getInstance(ctx.pathParam("id"));
            ctx.json(sendInstanceCommand(ctx.pathParam("id"), "op " + body.name));
        } catch (IllegalArgumentException e) {
            ctx.status(404).result(e.getMessage());
        }
    }

    /** DELETE /api/instances/{id}/players/ops/{name} */
    public void removeOp(Context ctx) {
        try {
            instanceManager.getInstance(ctx.pathParam("id"));
            ctx.json(sendInstanceCommand(ctx.pathParam("id"), "deop " + ctx.pathParam("name")));
        } catch (IllegalArgumentException e) {
            ctx.status(404).result(e.getMessage());
        }
    }

    /** GET /api/instances/{id}/players/bans — contents of the instance's banned-players.json. */
    public void getBans(Context ctx) {
        try {
            String id = ctx.pathParam("id");
            instanceManager.getInstance(id);
            ctx.json(Map.of("players", readPlayerJson(id, "banned-players.json")));
        } catch (IllegalArgumentException e) {
            ctx.status(404).result(e.getMessage());
        }
    }

    /**
     * POST /api/instances/{id}/players/bans — body: {"name":"X","reason":"..."}
     * Applies immediately via the {@code ban} command when the server is running;
     * otherwise writes the ban straight into banned-players.json so it takes effect
     * on next start (banning before anyone has ever joined works fine).
     */
    public void addBan(Context ctx) {
        PlayerActionRequest body = ctx.bodyAsClass(PlayerActionRequest.class);
        if (body == null || blank(body.name)) {
            ctx.status(400).result("name is required");
            return;
        }
        try {
            String id = ctx.pathParam("id");
            instanceManager.getInstance(id);
            MinecraftProcessManager pm = instanceManager.getProcessManager(id);
            if (pm != null && pm.isRunning()) {
                String reason = blank(body.reason) ? "" : " " + body.reason.trim();
                ctx.json(sendInstanceCommand(id, "ban " + body.name.trim() + reason));
            } else {
                addBanOffline(id, body.name.trim(), body.reason);
                ctx.json(Map.of("ok", true, "offline", true));
            }
        } catch (IllegalArgumentException e) {
            ctx.status(404).result(e.getMessage());
        } catch (IOException e) {
            ctx.status(500).result("Could not ban player: " + e.getMessage());
        }
    }

    /** DELETE /api/instances/{id}/players/bans/{name} — pardon, online or offline. */
    public void removeBan(Context ctx) {
        try {
            String id = ctx.pathParam("id");
            instanceManager.getInstance(id);
            MinecraftProcessManager pm = instanceManager.getProcessManager(id);
            if (pm != null && pm.isRunning()) {
                ctx.json(sendInstanceCommand(id, "pardon " + ctx.pathParam("name")));
            } else {
                boolean removed = removeBanOffline(id, ctx.pathParam("name"));
                ctx.json(Map.of("ok", true, "offline", true, "removed", removed));
            }
        } catch (IllegalArgumentException e) {
            ctx.status(404).result(e.getMessage());
        } catch (IOException e) {
            ctx.status(500).result("Could not unban player: " + e.getMessage());
        }
    }

    /** GET /api/instances/{id}/bom */
    public void getInstanceBom(Context ctx) {
        try {
            ctx.json(modsFor(ctx.pathParam("id")).listMods()
                    .stream().map(ModEntry::toMap).toList());
        } catch (IllegalArgumentException e) {
            ctx.status(404).result(e.getMessage());
        }
    }

    /**
     * POST /api/join-intent and /api/instances/{id}/join-intent — registers a
     * short-lived join ticket for the launcher's session so the player's
     * connection passes the Zircon join gate (AGENT_PLAN_7). Intentionally
     * unauthenticated: the launcher has no admin token. The {@code id} path
     * parameter (when present) is accepted for route compatibility; tickets are
     * global by username/UUID.
     */
    public void registerJoinIntent(Context ctx) {
        JoinIntentRequest body;
        try {
            body = ctx.bodyAsClass(JoinIntentRequest.class);
        } catch (RuntimeException e) {
            ctx.status(400).result("Invalid JSON body");
            return;
        }
        if (body == null || (body.username == null && body.uuid == null)) {
            ctx.status(400).result("username or uuid is required");
            return;
        }
        if (body.username != null) {
            JoinTicketManager.registerTicket(body.username);
        }
        if (body.uuid != null) {
            JoinTicketManager.registerTicket(body.uuid);
        }
        ctx.json(Map.of("ok", true, "expiresInSeconds", JoinTicketManager.TICKET_TTL_SECONDS));
    }

    // ------------------------------------------------------------------
    // Per-instance mods
    // ------------------------------------------------------------------

    /** GET /api/instances/{id}/mods */
    public void listMods(Context ctx) {
        try {
            ctx.json(Map.of("mods", modsFor(ctx.pathParam("id")).listMods()
                    .stream().map(ModEntry::toMap).toList()));
        } catch (IllegalArgumentException e) {
            ctx.status(404).result(e.getMessage());
        }
    }

    /** POST /api/instances/{id}/mods/upload (multipart, field "file") */
    public void uploadMod(Context ctx) {
        List<UploadedFile> files = ctx.uploadedFiles("file");
        if (files == null || files.isEmpty()) {
            ctx.status(400).result("No file uploaded (form field 'file')");
            return;
        }
        UploadedFile uploaded = files.get(0);
        String origin = ctx.queryParam("origin");
        try (InputStream in = uploaded.content()) {
            ModEntry entry = modsFor(ctx.pathParam("id")).addMod(in, uploaded.filename(), origin);
            ctx.status(201).json(entry.toMap());
        } catch (IllegalArgumentException e) {
            ctx.status(404).result(e.getMessage());
        } catch (IOException e) {
            log.warn("Upload failed", e);
            ctx.status(500).result("Upload failed: " + e.getMessage());
        }
    }

    /** DELETE /api/instances/{id}/mods/{filename} */
    public void removeMod(Context ctx) {
        try {
            boolean removed = modsFor(ctx.pathParam("id")).removeMod(ctx.pathParam("filename"));
            if (!removed) {
                ctx.status(404).result("Mod not found");
                return;
            }
            ctx.status(204);
        } catch (IOException e) {
            ctx.status(500).result("Delete failed: " + e.getMessage());
        }
    }

    /** GET /api/instances/{id}/mods/search?query=&mcVersion=&loader=&origin=&type= */
    public void searchMods(Context ctx) {
        String query = ctx.queryParam("query");
        String mcVersion = ctx.queryParam("mcVersion");
        String loader = ctx.queryParam("loader");
        String origin = ctx.queryParam("origin");
        String type = ctx.queryParam("type"); // "mod" or "modpack" (modrinth only); null = mods
        if (query == null) query = "";
        if (origin == null) origin = "modrinth";

        ModManagementService mods;
        try {
            mods = modsFor(ctx.pathParam("id"));
        } catch (IllegalArgumentException e) {
            ctx.status(404).result(e.getMessage());
            return;
        }

        Map<String, Object> result = new HashMap<>();
        try {
            if ("curseforge".equalsIgnoreCase(origin)) {
                if (!mods.hasCurseForgeKey()) {
                    result.put("origin", "curseforge");
                    result.put("hits", List.of());
                    result.put("notice", "CurseForge API key not configured on the server.");
                    ctx.json(result);
                    return;
                }
                result.put("origin", "curseforge");
                result.put("hits", mods.curseForge().searchMods(query, mcVersion)
                        .stream().map(CurseForgeApiClient.CurseForgeMod::toMap).toList());
            } else {
                result.put("origin", "modrinth");
                result.put("hits", mods.modrinth().searchMods(query, mcVersion, loader, type)
                        .stream().map(ModrinthApiClient.ModrinthSearchHit::toMap).toList());
            }
            ctx.json(result);
        } catch (IOException | InterruptedException e) {
            if (e instanceof InterruptedException) {
                Thread.currentThread().interrupt();
            }
            ctx.status(502).result("Provider search failed: " + e.getMessage());
        }
    }

    /** GET /api/instances/{id}/mods/modrinth/versions?projectId=&mcVersion=&loader= */
    public void modrinthVersions(Context ctx) {
        String projectId = ctx.queryParam("projectId");
        if (projectId == null) {
            ctx.status(400).result("projectId is required");
            return;
        }
        try {
            ModManagementService mods = modsFor(ctx.pathParam("id"));
            ctx.json(Map.of("versions", mods.modrinth()
                    .listProjectVersions(projectId, ctx.queryParam("mcVersion"), ctx.queryParam("loader"))
                    .stream().map(ModrinthApiClient.ModrinthVersion::toMap).toList()));
        } catch (IllegalArgumentException e) {
            ctx.status(404).result(e.getMessage());
        } catch (IOException | InterruptedException e) {
            if (e instanceof InterruptedException) {
                Thread.currentThread().interrupt();
            }
            ctx.status(502).result("Could not list versions: " + e.getMessage());
        }
    }

    /** GET /api/instances/{id}/mods/curseforge/files?modId= */
    public void curseForgeFiles(Context ctx) {
        String modId = ctx.queryParam("modId");
        if (modId == null) {
            ctx.status(400).result("modId is required");
            return;
        }
        try {
            ModManagementService mods = modsFor(ctx.pathParam("id"));
            if (!mods.hasCurseForgeKey()) {
                ctx.status(400).result("CurseForge API key not configured on the server");
                return;
            }
            ctx.json(Map.of("files", mods.curseForge().listModFiles(Long.parseLong(modId))
                    .stream().map(CurseForgeApiClient.CurseForgeFile::toMap).toList()));
        } catch (NumberFormatException e) {
            ctx.status(400).result("modId must be a number");
        } catch (IllegalArgumentException e) {
            ctx.status(404).result(e.getMessage());
        } catch (IOException | InterruptedException e) {
            if (e instanceof InterruptedException) {
                Thread.currentThread().interrupt();
            }
            ctx.status(502).result("Could not list files: " + e.getMessage());
        }
    }

    /**
     * POST /api/instances/{id}/mods/install
     * body: {"origin":"modrinth","projectId":"...","versionId":"..."}
     *   or: {"origin":"curseforge","downloadUrl":"...","filename":"...","fileId":"..."}
     * Downloads the file from the provider CDN into this instance's mods folder.
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

        ModManagementService mods;
        try {
            mods = modsFor(ctx.pathParam("id"));
        } catch (IllegalArgumentException e) {
            ctx.status(404).result(e.getMessage());
            return;
        }

        try {
            switch (body.origin.toLowerCase()) {
                case "modrinth" -> {
                    if (body.projectId == null || body.versionId == null) {
                        ctx.status(400).result("projectId and versionId are required for modrinth");
                        return;
                    }
                    ModEntry entry = mods.installModrinthVersion(body.projectId, body.versionId, null, null);
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

    /**
     * POST /api/instances/{id}/modpacks/install
     * body: {"projectId":"...","versionId":"..."}
     * Downloads a Modrinth modpack (.mrpack) and installs every mod it lists into
     * this instance's mods folder.
     */
    public void installModpack(Context ctx) {
        InstallRequest body;
        try {
            body = ctx.bodyAsClass(InstallRequest.class);
        } catch (RuntimeException e) {
            ctx.status(400).result("Invalid JSON body");
            return;
        }
        if (body == null || body.projectId == null) {
            ctx.status(400).result("projectId is required");
            return;
        }

        ModManagementService mods;
        try {
            mods = modsFor(ctx.pathParam("id"));
        } catch (IllegalArgumentException e) {
            ctx.status(404).result(e.getMessage());
            return;
        }

        try {
            Map<String, Object> result = mods.installModrinthModpack(body.projectId, body.versionId);
            ctx.status(201).json(result);
        } catch (IOException | InterruptedException e) {
            if (e instanceof InterruptedException) {
                Thread.currentThread().interrupt();
            }
            ctx.status(502).result("Modpack installation failed: " + e.getMessage());
        }
    }

    // ------------------------------------------------------------------
    // Per-instance shaders & texture packs
    // ------------------------------------------------------------------

    /** GET /api/instances/{id}/shaderpacks */
    public void listShaderpacks(Context ctx) {
        try {
            ctx.json(Map.of("shaderpacks", packsFor(ctx.pathParam("id")).listShaderpacks()
                    .stream().map(PackEntry::toMap).toList()));
        } catch (IllegalArgumentException e) {
            ctx.status(404).result(e.getMessage());
        }
    }

    /** POST /api/instances/{id}/shaderpacks/upload (multipart, field "file") */
    public void uploadShaderpack(Context ctx) {
        uploadPack(ctx, true);
    }

    /** POST /api/instances/{id}/shaderpacks/install body: {"downloadUrl":"...","filename":"..."} */
    public void installShaderpack(Context ctx) {
        installPack(ctx, true);
    }

    /** DELETE /api/instances/{id}/shaderpacks/{filename} */
    public void removeShaderpack(Context ctx) {
        removePack(ctx, true);
    }

    /** GET /api/instances/{id}/resourcepacks */
    public void listResourcepacks(Context ctx) {
        try {
            ctx.json(Map.of("resourcepacks", packsFor(ctx.pathParam("id")).listResourcepacks()
                    .stream().map(PackEntry::toMap).toList()));
        } catch (IllegalArgumentException e) {
            ctx.status(404).result(e.getMessage());
        }
    }

    /** POST /api/instances/{id}/resourcepacks/upload (multipart, field "file") */
    public void uploadResourcepack(Context ctx) {
        uploadPack(ctx, false);
    }

    /** POST /api/instances/{id}/resourcepacks/install body: {"downloadUrl":"...","filename":"..."} */
    public void installResourcepack(Context ctx) {
        installPack(ctx, false);
    }

    /** DELETE /api/instances/{id}/resourcepacks/{filename} */
    public void removeResourcepack(Context ctx) {
        removePack(ctx, false);
    }

    private void uploadPack(Context ctx, boolean shader) {
        List<UploadedFile> files = ctx.uploadedFiles("file");
        if (files == null || files.isEmpty()) {
            ctx.status(400).result("No file uploaded (form field 'file')");
            return;
        }
        UploadedFile uploaded = files.get(0);
        String origin = ctx.queryParam("origin");
        try {
            PackManagementService packs = packsFor(ctx.pathParam("id"));
            try (InputStream in = uploaded.content()) {
                PackEntry entry = shader
                        ? packs.addShaderpack(in, uploaded.filename(), origin)
                        : packs.addResourcepack(in, uploaded.filename(), origin);
                ctx.status(201).json(entry.toMap());
            }
        } catch (IllegalArgumentException e) {
            ctx.status(404).result(e.getMessage());
        } catch (IOException e) {
            log.warn("Pack upload failed", e);
            ctx.status(500).result("Upload failed: " + e.getMessage());
        }
    }

    private void installPack(Context ctx, boolean shader) {
        InstallRequest body;
        try {
            body = ctx.bodyAsClass(InstallRequest.class);
        } catch (RuntimeException e) {
            ctx.status(400).result("Invalid JSON body");
            return;
        }
        if (body == null || body.downloadUrl == null || body.filename == null) {
            ctx.status(400).result("downloadUrl and filename are required");
            return;
        }
        try {
            PackManagementService packs = packsFor(ctx.pathParam("id"));
            PackEntry entry = shader
                    ? packs.installShaderpackFromUrl(body.downloadUrl, body.filename,
                            body.origin == null ? "modrinth" : body.origin)
                    : packs.installResourcepackFromUrl(body.downloadUrl, body.filename,
                            body.origin == null ? "modrinth" : body.origin);
            ctx.status(201).json(entry.toMap());
        } catch (IllegalArgumentException e) {
            ctx.status(404).result(e.getMessage());
        } catch (IOException e) {
            ctx.status(502).result("Pack install failed: " + e.getMessage());
        }
    }

    private void removePack(Context ctx, boolean shader) {
        try {
            PackManagementService packs = packsFor(ctx.pathParam("id"));
            boolean removed = shader
                    ? packs.removeShaderpack(ctx.pathParam("filename"))
                    : packs.removeResourcepack(ctx.pathParam("filename"));
            if (!removed) {
                ctx.status(404).result("Pack not found");
                return;
            }
            ctx.status(204);
        } catch (IOException e) {
            ctx.status(500).result("Delete failed: " + e.getMessage());
        }
    }

    // ------------------------------------------------------------------
    // helpers
    // ------------------------------------------------------------------

    private ModManagementService modsFor(String instanceId) {
        InstanceConfig cfg = instanceManager.getInstance(instanceId);
        Path instanceDir = instanceManager.getInstanceDir(instanceId);
        BomService bom = new BomService(instanceDir.resolve("bom.json"),
                new BillOfMaterials(cfg.getMinecraftVersion(), cfg.getModLoader(), cfg.getName()));
        return new ModManagementService(bom, instanceDir.resolve("mods"), curseForgeApiKey);
    }

    private PackManagementService packsFor(String instanceId) {
        InstanceConfig cfg = instanceManager.getInstance(instanceId);
        Path instanceDir = instanceManager.getInstanceDir(instanceId);
        BomService bom = new BomService(instanceDir.resolve("bom.json"),
                new BillOfMaterials(cfg.getMinecraftVersion(), cfg.getModLoader(), cfg.getName()));
        return new PackManagementService(bom, instanceDir.resolve("shaderpacks"),
                instanceDir.resolve("resourcepacks"));
    }

    /** Sends a command to the instance's own server process (no-op when offline). */
    private Map<String, Object> sendInstanceCommand(String instanceId, String command) {
        Map<String, Object> result = new HashMap<>();
        result.put("command", command);
        MinecraftProcessManager pm = instanceManager.getProcessManager(instanceId);
        if (pm == null || !pm.isRunning()) {
            result.put("sent", false);
            result.put("error", "Server is not running — start it before managing players");
            return result;
        }
        try {
            pm.sendCommand(command);
            result.put("sent", true);
        } catch (IllegalStateException e) {
            result.put("sent", false);
            result.put("error", e.getMessage());
        }
        return result;
    }

    /** Reads a vanilla JSON player list ({@code whitelist.json}, {@code ops.json},
     *  {@code banned-players.json}) from the instance dir. */
    private List<Map<String, Object>> readPlayerJson(String instanceId, String fileName) {
        Path file = instanceManager.getInstanceDir(instanceId).resolve("server").resolve(fileName);
        List<Map<String, Object>> out = new ArrayList<>();
        if (!Files.isRegularFile(file)) {
            return out;
        }
        try {
            JsonArray arr = JsonParser.parseString(Files.readString(file)).getAsJsonArray();
            for (JsonElement element : arr) {
                if (element.isJsonObject()) {
                    var obj = element.getAsJsonObject();
                    Map<String, Object> entry = new HashMap<>();
                    entry.put("uuid", obj.has("uuid") ? obj.get("uuid").getAsString() : "");
                    entry.put("name", obj.has("name") ? obj.get("name").getAsString() : "");
                    if (obj.has("reason")) entry.put("reason", obj.get("reason").getAsString());
                    if (obj.has("source")) entry.put("source", obj.get("source").getAsString());
                    if (obj.has("created")) entry.put("created", obj.get("created").getAsString());
                    if (obj.has("expires")) entry.put("expires", obj.get("expires").getAsString());
                    out.add(entry);
                }
            }
        } catch (IOException | RuntimeException e) {
            log.warn("Could not read {} for instance {}", fileName, instanceId, e);
        }
        return out;
    }

    /** Writes a ban entry directly into banned-players.json (server offline). */
    private void addBanOffline(String instanceId, String name, String reason) throws IOException {
        Path file = instanceManager.getInstanceDir(instanceId).resolve("server").resolve("banned-players.json");
        VanillaPlayerFiles.ban(file, name, reason, resolveUuid(instanceId, name));
        log.info("Banned {} (offline, instance {})", name, instanceId);
    }

    /** Removes a ban entry from banned-players.json (server offline). */
    private boolean removeBanOffline(String instanceId, String name) throws IOException {
        Path file = instanceManager.getInstanceDir(instanceId).resolve("server").resolve("banned-players.json");
        boolean removed = VanillaPlayerFiles.pardon(file, name);
        if (removed) {
            log.info("Unbanned {} (offline, instance {})", name, instanceId);
        }
        return removed;
    }

    /**
     * Resolves the best-known UUID for a player name: the real UUID from
     * usercache.json when they have joined before, otherwise the deterministic
     * offline-mode UUID (valid for offline servers).
     */
    private String resolveUuid(String instanceId, String name) {
        Path userCache = instanceManager.getInstanceDir(instanceId).resolve("server").resolve("usercache.json");
        return VanillaPlayerFiles.resolveUuid(userCache, name);
    }

    private boolean blank(String s) {
        return s == null || s.isBlank();
    }

    private boolean validSchedule(String frequency, String time) {
        if (frequency != null && !InstanceConfig.VALID_BACKUP_FREQUENCIES.contains(frequency)) {
            return false;
        }
        if (time != null && !time.matches("^\\d{2}:\\d{2}$")) {
            return false;
        }
        return true;
    }

    private Map<String, Object> toMap(InstanceConfig cfg) {
        Map<String, Object> map = new HashMap<>();
        map.put("id", cfg.getId());
        map.put("name", cfg.getName());
        map.put("minecraftVersion", cfg.getMinecraftVersion());
        map.put("modLoader", Map.of(
                "type", cfg.getModLoader() == null ? "vanilla" : cfg.getModLoader().getType(),
                "version", cfg.getModLoader() == null ? "" : cfg.getModLoader().getVersion()));
        map.put("internalMcPort", cfg.getInternalMcPort());
        map.put("externalPort", cfg.getExternalMcPort());
        map.put("javaArgs", cfg.getJavaArgs());
        map.put("autoStart", cfg.isAutoStart());
        map.put("backupFrequency", cfg.getBackupFrequency());
        map.put("backupTime", cfg.getBackupTime());
        map.put("backupRetention", cfg.getBackupRetention());
        map.put("running", instanceManager.isRunning(cfg.getId()));
        map.put("playerCount", instanceManager.getOnlinePlayerCount(cfg.getId()));
        map.put("onlinePlayers", List.copyOf(instanceManager.getOnlinePlayers(cfg.getId())));
        return map;
    }

    public static class CreateRequest {
        public String name;
        public String mcVersion;
        public String loaderType;
        public String loaderVersion;
    }

    public static class UpdateRequest {
        public String name;
        public String mcVersion;
        public String loaderVersion;
        public String javaArgs;
        public String backupFrequency;
        public String backupTime;
        /** Player-facing port; 0 / absent leaves it unchanged. */
        public int externalPort;
    }

    public static class PlayerActionRequest {
        public String name;
        public String reason;
    }

    public static class InstallRequest {
        public String origin;
        public String projectId;
        public String versionId;
        public String downloadUrl;
        public String filename;
        public String fileId;
    }

    public static class JoinIntentRequest {
        public String username;
        public String uuid;
    }

    public static class EulaRequest {
        public boolean accepted;
    }

    public static class ServerPropertiesRequest {
        public Map<String, String> properties;
    }
}
