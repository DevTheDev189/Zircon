package com.mcmanager.server.web;

import com.mcmanager.server.auth.AuthService;
import com.mcmanager.server.auth.JwtUtil;
import com.mcmanager.server.instance.ServerInstanceManager;
import com.mcmanager.server.process.ConsoleStreamHandler;
import com.mcmanager.server.process.MinecraftProcessManager;
import com.mcmanager.server.service.BackupService;
import com.mcmanager.server.service.BomService;
import com.mcmanager.server.service.ConfigService;
import com.mcmanager.server.service.ModManagementService;
import com.mcmanager.server.service.ModServiceResolver;
import com.mcmanager.server.service.PackManagementService;
import com.mcmanager.server.stats.SystemMetricsService;
import com.mcmanager.server.web.controller.BackupController;
import com.mcmanager.server.web.controller.BomController;
import com.mcmanager.server.web.controller.ConfigController;
import com.mcmanager.server.web.controller.ConsoleController;
import com.mcmanager.server.web.controller.InstanceController;
import com.mcmanager.server.web.controller.ModController;
import com.mcmanager.server.web.controller.PackFileController;
import com.mcmanager.server.web.controller.PlayerController;
import io.javalin.Javalin;
import io.javalin.http.UnauthorizedResponse;
import io.javalin.http.staticfiles.Location;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

import java.io.IOException;
import java.util.Map;

/**
 * Javalin application hosting the admin REST API and the bundled SPA on the
 * internal web port. Public access goes through the TCP multiplexer, which
 * proxies HTTP here and Minecraft traffic to the server process.
 *
 * <p>Every {@code /api/*} route except {@code POST /api/auth/login} requires a
 * valid {@code Authorization: Bearer <jwt>} header. All instance-management and
 * per-instance mod routes go through {@link InstanceController}.
 */
public class JavalinApp {

    private static final Logger log = LoggerFactory.getLogger(JavalinApp.class);

    private final ConfigService configService;
    private final BomService bomService;
    private final ModManagementService modService;
    private final MinecraftProcessManager processManager;
    private final ConsoleStreamHandler console;
    private final ServerInstanceManager instanceManager;
    private final BackupService backupService;

    private Javalin app;

    public JavalinApp(ConfigService configService, BomService bomService,
                      ModManagementService modService, MinecraftProcessManager processManager,
                      ConsoleStreamHandler console, ServerInstanceManager instanceManager,
                      BackupService backupService) {
        this.configService = configService;
        this.bomService = bomService;
        this.modService = modService;
        this.processManager = processManager;
        this.console = console;
        this.instanceManager = instanceManager;
        this.backupService = backupService;
    }

    public void start() {
        // Client-facing legacy endpoints (/bom, /files/mods/*, /api/mods/*) serve the
        // active instance's data when instances exist, so the client always syncs
        // against the same mods the admin UI manages (see ModServiceResolver).
        PackManagementService legacyPacks = new PackManagementService(bomService,
                configService.getDataDir().resolve("shaderpacks"),
                configService.getDataDir().resolve("resourcepacks"));
        ModServiceResolver serviceResolver = new ModServiceResolver(instanceManager, bomService,
                modService, legacyPacks, configService.getConfig().curseforgeApiKey);
        BomController bomController = new BomController(serviceResolver::bom);
        ModController modController = new ModController(serviceResolver::mods);
        PackFileController packFileController = new PackFileController(serviceResolver::packs);
        PlayerController playerController = new PlayerController(configService, processManager, console);
        ConfigController configController = new ConfigController(configService, bomService,
                processManager, console);
        ConsoleController consoleController = new ConsoleController(console, processManager);
        InstanceController instanceController = new InstanceController(instanceManager,
                configService.getConfig().curseforgeApiKey);
        BackupController backupController = new BackupController(backupService, instanceManager);
        console.addListener(consoleController::broadcast);

        app = Javalin.create(javalinConfig -> {
            javalinConfig.showJavalinBanner = false;
            javalinConfig.jsonMapper(new io.javalin.json.JavalinGson());
            javalinConfig.staticFiles.add(staticFiles -> {
                staticFiles.hostedPath = "/";
                staticFiles.directory = "/web";
                staticFiles.location = Location.CLASSPATH;
            });
            javalinConfig.router.mount(routes -> routes.addWsHandler(
                    io.javalin.websocket.WsHandlerType.WEBSOCKET,
                    "/api/console", consoleController::register));
        });

        // ------------------------------------------------------------------
        // Auth
        // ------------------------------------------------------------------
        app.before("/api/*", ctx -> {
            String path = ctx.path();
            // Public endpoints the launcher needs without an admin bearer token.
            boolean publicEndpoint = path.equals("/api/auth/login")
                    || path.equals("/api/auth/change-password")
                    || path.equals("/api/join-intent")
                    || (path.startsWith("/api/instances/") && path.endsWith("/join-intent"));
            if (publicEndpoint) {
                return; // change-password still validates credentials itself
            }
            String token = ctx.header("Authorization");
            if (token == null || !token.startsWith("Bearer ")
                    || JwtUtil.validateToken(token.substring(7)) == null) {
                throw new UnauthorizedResponse("Authentication required. Please log in.");
            }
        });

        app.post("/api/auth/login", ctx -> {
            LoginRequest req = ctx.bodyAsClass(LoginRequest.class);
            if (req == null || !AuthService.authenticate(req.username, req.password)) {
                ctx.status(401).result("Invalid username or password");
                return;
            }
            String token = JwtUtil.generateToken(req.username);
            ctx.json(Map.of("token", token, "username", req.username));
        });

        app.post("/api/auth/change-password", ctx -> {
            ChangePasswordRequest req = ctx.bodyAsClass(ChangePasswordRequest.class);
            if (req == null || req.username == null || req.currentPassword == null || req.newPassword == null) {
                ctx.status(400).result("username, currentPassword and newPassword are required");
                return;
            }
            try {
                if (AuthService.changePassword(req.username, req.currentPassword, req.newPassword)) {
                    ctx.json(Map.of("ok", true));
                } else {
                    ctx.status(401).result("Invalid username or current password");
                }
            } catch (IOException e) {
                ctx.status(400).result(e.getMessage());
            }
        });

        // Current user profile (username + icon for the admin header).
        app.get("/api/auth/me", ctx -> {
            String token = ctx.header("Authorization");
            String username = (token != null && token.startsWith("Bearer "))
                    ? JwtUtil.validateToken(token.substring(7)) : null;
            if (username == null) {
                ctx.status(401).result("Unauthorized");
                return;
            }
            AuthService.UserProfile user = AuthService.getUser(username);
            if (user == null) {
                ctx.status(404).result("User not found");
                return;
            }
            ctx.json(Map.of("username", user.username, "icon", user.icon));
        });

        // Atomic profile update (rename / change password / change icon).
        app.post("/api/auth/profile", ctx -> {
            ProfileUpdateRequest req = ctx.bodyAsClass(ProfileUpdateRequest.class);
            if (req == null || req.currentUsername == null || req.currentPassword == null) {
                ctx.status(400).result("currentUsername and currentPassword are required");
                return;
            }
            try {
                boolean ok = AuthService.updateProfile(req.currentUsername, req.newUsername,
                        req.currentPassword, req.newPassword, req.icon);
                if (ok) {
                    ctx.json(Map.of("ok", true));
                } else {
                    ctx.status(401).result("Invalid credentials");
                }
            } catch (IOException e) {
                ctx.status(400).result(e.getMessage());
            }
        });

        // ------------------------------------------------------------------
        // System metrics
        // ------------------------------------------------------------------
        app.get("/api/stats", ctx -> ctx.json(
                SystemMetricsService.getMetricsSnapshot(configService.getDataDir())));

        // ------------------------------------------------------------------
        // Legacy single-server API (BOM / mods / players / config / console)
        // ------------------------------------------------------------------
        // Launcher pre-join ticket registration (AGENT_PLAN_7): the launcher has
        // no admin JWT, so these routes are exempted from auth in the before filter.
        app.post("/api/join-intent", instanceController::registerJoinIntent);
        app.post("/api/instances/{id}/join-intent", instanceController::registerJoinIntent);

        app.get("/bom", bomController::getBom);

        app.get("/api/mods", modController::listMods);
        app.get("/files/mods/{filename}", modController::downloadMod);
        app.post("/api/mods/upload", modController::uploadMod);
        app.delete("/api/mods/{filename}", modController::removeMod);
        app.get("/api/mods/search", modController::searchMods);
        app.get("/api/mods/modrinth/versions", modController::modrinthVersions);
        app.get("/api/mods/curseforge/files", modController::curseForgeFiles);
        app.post("/api/mods/install", modController::installMod);

        app.get("/files/shaderpacks/{filename}", packFileController::downloadShaderpack);
        app.get("/files/resourcepacks/{filename}", packFileController::downloadResourcepack);

        app.get("/api/players/online", playerController::online);
        app.get("/api/players/whitelist", playerController::getWhitelist);
        app.post("/api/players/whitelist", playerController::addWhitelist);
        app.delete("/api/players/whitelist/{name}", playerController::removeWhitelist);
        app.get("/api/players/bans", playerController::getBans);
        app.post("/api/players/bans", playerController::addBan);
        app.delete("/api/players/bans/{name}", playerController::removeBan);
        app.get("/api/players/ops", playerController::getOps);
        app.post("/api/players/ops", playerController::addOp);
        app.delete("/api/players/ops/{name}", playerController::removeOp);
        app.post("/api/players/kick", playerController::kick);
        app.post("/api/players/command", playerController::runCommand);

        app.get("/api/config", configController::getConfig);
        app.post("/api/config", configController::updateConfig);
        app.get("/api/status", configController::getStatus);
        app.post("/api/server/start", configController::startServer);
        app.post("/api/server/stop", configController::stopServer);

        // ------------------------------------------------------------------
        // Multi-instance API
        // ------------------------------------------------------------------
        app.get("/api/instances", instanceController::listInstances);
        app.post("/api/instances", instanceController::createInstance); // Create instance + LOCK loader
        app.get("/api/instances/{id}", instanceController::getInstance);
        app.patch("/api/instances/{id}", instanceController::updateInstance);
        app.delete("/api/instances/{id}", instanceController::deleteInstance);
        app.post("/api/instances/{id}/start", instanceController::startInstance);
        app.post("/api/instances/{id}/stop", instanceController::stopInstance);
        app.post("/api/instances/{id}/restart", instanceController::restartInstance);
        app.get("/api/instances/{id}/eula", instanceController::getEula);
        app.post("/api/instances/{id}/eula", instanceController::acceptEula);
        app.get("/api/instances/{id}/server-properties", instanceController::getServerProperties);
        app.post("/api/instances/{id}/server-properties", instanceController::saveServerProperties);
        app.get("/api/instances/{id}/players/online", instanceController::onlinePlayers);
        app.get("/api/instances/{id}/players/history", instanceController::playerHistory);
        app.get("/api/instances/{id}/players/whitelist", instanceController::getWhitelist);
        app.post("/api/instances/{id}/players/whitelist", instanceController::addWhitelist);
        app.delete("/api/instances/{id}/players/whitelist/{name}", instanceController::removeWhitelist);
        app.get("/api/instances/{id}/players/ops", instanceController::getOps);
        app.post("/api/instances/{id}/players/ops", instanceController::addOp);
        app.delete("/api/instances/{id}/players/ops/{name}", instanceController::removeOp);
        app.get("/api/instances/{id}/players/bans", instanceController::getBans);
        app.post("/api/instances/{id}/players/bans", instanceController::addBan);
        app.delete("/api/instances/{id}/players/bans/{name}", instanceController::removeBan);
        app.get("/api/instances/{id}/bom", instanceController::getInstanceBom);
        app.get("/api/instances/{id}/mods", instanceController::listMods);
        app.post("/api/instances/{id}/mods/upload", instanceController::uploadMod);
        app.delete("/api/instances/{id}/mods/{filename}", instanceController::removeMod);
        app.get("/api/instances/{id}/mods/search", instanceController::searchMods);
        app.get("/api/instances/{id}/mods/modrinth/versions", instanceController::modrinthVersions);
        app.get("/api/instances/{id}/mods/curseforge/files", instanceController::curseForgeFiles);
        app.post("/api/instances/{id}/mods/install", instanceController::installMod);
        app.post("/api/instances/{id}/modpacks/install", instanceController::installModpack);

        // Shaders & texture packs REST endpoints
        app.get("/api/instances/{id}/shaders", instanceController::getShaderStatus);
        app.post("/api/instances/{id}/shaders/toggle", instanceController::toggleShaderEngine);
        app.get("/api/instances/{id}/shaderpacks", instanceController::listShaderpacks);
        app.post("/api/instances/{id}/shaderpacks/upload", instanceController::uploadShaderpack);
        app.post("/api/instances/{id}/shaderpacks/install", instanceController::installShaderpack);
        app.delete("/api/instances/{id}/shaderpacks/{filename}", instanceController::removeShaderpack);
        app.get("/api/instances/{id}/resourcepacks", instanceController::listResourcepacks);
        app.post("/api/instances/{id}/resourcepacks/upload", instanceController::uploadResourcepack);
        app.post("/api/instances/{id}/resourcepacks/install", instanceController::installResourcepack);
        app.delete("/api/instances/{id}/resourcepacks/{filename}", instanceController::removeResourcepack);

        // Backups REST endpoints
        app.get("/api/instances/{id}/backups", backupController::listBackups);
        app.post("/api/instances/{id}/backups", backupController::createBackup);
        app.post("/api/instances/{id}/backups/retention", backupController::setRetention);
        app.post("/api/instances/{id}/backups/{backupId}/restore", backupController::restoreBackup);

        // Static fallback: serve index.html for unknown GETs (SPA deep links).
        app.get("/", ctx -> ctx.redirect("/index.html"));
        app.exception(io.javalin.http.NotFoundResponse.class, (e, ctx) -> ctx.redirect("/index.html"));

        int webPort = configService.getConfig().webPort;
        app.start("127.0.0.1", webPort);
        log.info("Admin web server listening on 127.0.0.1:{}", webPort);
    }

    public void stop() {
        if (app != null) {
            app.stop();
        }
    }

    public static class LoginRequest {
        public String username;
        public String password;
    }

    public static class ChangePasswordRequest {
        public String username;
        public String currentPassword;
        public String newPassword;
    }

    public static class ProfileUpdateRequest {
        public String currentUsername;
        public String newUsername;
        public String currentPassword;
        public String newPassword;
        public String icon;
    }
}
