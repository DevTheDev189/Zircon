package com.mcmanager.server.web;

import com.mcmanager.server.process.ConsoleStreamHandler;
import com.mcmanager.server.process.MinecraftProcessManager;
import com.mcmanager.server.service.BomService;
import com.mcmanager.server.service.ConfigService;
import com.mcmanager.server.service.ModManagementService;
import com.mcmanager.server.web.controller.BomController;
import com.mcmanager.server.web.controller.ConfigController;
import com.mcmanager.server.web.controller.ConsoleController;
import com.mcmanager.server.web.controller.ModController;
import com.mcmanager.server.web.controller.PlayerController;
import io.javalin.Javalin;
import io.javalin.http.staticfiles.Location;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

/**
 * Javalin application hosting the admin REST API and the bundled SPA on the
 * internal web port. Public access goes through the TCP multiplexer, which
 * proxies HTTP here and Minecraft traffic to the server process.
 */
public class JavalinApp {

    private static final Logger log = LoggerFactory.getLogger(JavalinApp.class);

    private final ConfigService configService;
    private final BomService bomService;
    private final ModManagementService modService;
    private final MinecraftProcessManager processManager;
    private final ConsoleStreamHandler console;

    private Javalin app;

    public JavalinApp(ConfigService configService, BomService bomService,
                      ModManagementService modService, MinecraftProcessManager processManager,
                      ConsoleStreamHandler console) {
        this.configService = configService;
        this.bomService = bomService;
        this.modService = modService;
        this.processManager = processManager;
        this.console = console;
    }

    public void start() {
        BomController bomController = new BomController(bomService);
        ModController modController = new ModController(modService);
        PlayerController playerController = new PlayerController(configService, processManager, console);
        ConfigController configController = new ConfigController(configService, bomService,
                processManager, console);
        ConsoleController consoleController = new ConsoleController(console, processManager);
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

        // BOM / sync
        app.get("/bom", bomController::getBom);

        // Mod management
        app.get("/api/mods", modController::listMods);
        app.get("/files/mods/{filename}", modController::downloadMod);
        app.post("/api/mods/upload", modController::uploadMod);
        app.delete("/api/mods/{filename}", modController::removeMod);
        app.get("/api/mods/search", modController::searchMods);
        app.get("/api/mods/modrinth/versions", modController::modrinthVersions);
        app.get("/api/mods/curseforge/files", modController::curseForgeFiles);
        app.post("/api/mods/install", modController::installMod);

        // Players
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

        // Config & status
        app.get("/api/config", configController::getConfig);
        app.post("/api/config", configController::updateConfig);
        app.get("/api/status", configController::getStatus);
        app.post("/api/server/start", configController::startServer);
        app.post("/api/server/stop", configController::stopServer);

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
}
