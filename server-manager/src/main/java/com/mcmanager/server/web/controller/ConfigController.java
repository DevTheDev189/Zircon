package com.mcmanager.server.web.controller;

import com.mcmanager.core.model.BillOfMaterials;
import com.mcmanager.server.process.MinecraftProcessManager;
import com.mcmanager.server.service.BomService;
import com.mcmanager.server.service.ConfigService;
import io.javalin.http.Context;

import java.io.IOException;
import java.util.HashMap;
import java.util.Map;

/**
 * Server settings endpoint: reads/writes {@code server.properties} plus wrapper
 * settings (ports, title, loader) from the BOM / config.
 */
public class ConfigController {

    private final ConfigService configService;
    private final BomService bomService;
    private final MinecraftProcessManager processManager;
    private final com.mcmanager.server.process.ConsoleStreamHandler console;

    public ConfigController(ConfigService configService, BomService bomService,
                            MinecraftProcessManager processManager,
                            com.mcmanager.server.process.ConsoleStreamHandler console) {
        this.configService = configService;
        this.bomService = bomService;
        this.processManager = processManager;
        this.console = console;
    }

    /** GET /api/config — wrapper config + server.properties + server status. */
    public void getConfig(Context ctx) {
        Map<String, Object> result = new HashMap<>();
        ConfigService.ServerConfig cfg = configService.getConfig();

        result.put("serverTitle", cfg.serverTitle);
        result.put("minecraftVersion", cfg.minecraftVersion);
        result.put("modLoader", cfg.modLoader);
        result.put("javaArgs", cfg.javaArgs);
        result.put("publicPort", cfg.publicPort);
        result.put("mcPort", cfg.mcPort);
        result.put("autoStartServer", cfg.autoStartServer);
        result.put("curseforgeApiKey", cfg.curseforgeApiKey);

        result.put("serverProperties", safeServerProperties());
        ctx.json(result);
    }

    private java.util.Map<String, String> safeServerProperties() {
        try {
            return configService.loadServerProperties().asMap();
        } catch (IOException e) {
            return Map.of();
        }
    }

    /**
     * POST /api/config — accepts partial updates:
     * {"serverTitle":"...", "minecraftVersion":"...", "modLoader":{...},
     *  "javaArgs":"...", "autoStartServer":true, "serverProperties":{"motd":"..."}}
     */
    public void updateConfig(Context ctx) {
        ConfigUpdate body;
        try {
            body = ctx.bodyAsClass(ConfigUpdate.class);
        } catch (RuntimeException e) {
            ctx.status(400).result("Invalid JSON body");
            return;
        }
        if (body == null) {
            ctx.status(400).result("Empty body");
            return;
        }

        ConfigService.ServerConfig cfg = configService.getConfig();
        BillOfMaterials bom = bomService.getBom();

        if (body.serverTitle != null) {
            cfg.serverTitle = body.serverTitle;
            bom.setServerTitle(body.serverTitle);
        }
        if (body.minecraftVersion != null) {
            cfg.minecraftVersion = body.minecraftVersion;
            bom.setMinecraftVersion(body.minecraftVersion);
        }
        if (body.modLoader != null) {
            cfg.modLoader = body.modLoader;
            bom.setModLoader(body.modLoader);
        }
        if (body.javaArgs != null) {
            cfg.javaArgs = body.javaArgs;
        }
        if (body.curseforgeApiKey != null) {
            cfg.curseforgeApiKey = body.curseforgeApiKey;
        }
        if (body.autoStartServer != null) {
            cfg.autoStartServer = body.autoStartServer;
        }

        try {
            configService.saveConfig();
            bomService.save();
            if (body.serverProperties != null && !body.serverProperties.isEmpty()) {
                ConfigService.ServerProperties props = configService.loadServerProperties();
                body.serverProperties.forEach(props::set);
                configService.saveServerProperties(props);
            }
            ctx.json(Map.of("ok", true));
        } catch (IOException e) {
            ctx.status(500).result("Failed to persist config: " + e.getMessage());
        }
    }

    /** POST /api/server/start — launch the Minecraft subprocess. */
    public void startServer(Context ctx) {
        try {
            processManager.start();
            ctx.json(Map.of("ok", true, "running", true));
        } catch (IOException | IllegalStateException e) {
            ctx.status(409).result(e.getMessage());
        }
    }

    /** POST /api/server/stop — stop the Minecraft subprocess. */
    public void stopServer(Context ctx) {
        processManager.stop();
        ctx.json(Map.of("ok", true, "running", false));
    }

    /** GET /api/status — process status, online players, port wiring. */
    public void getStatus(Context ctx) {
        ConfigService.ServerConfig cfg = configService.getConfig();
        Map<String, Object> result = new HashMap<>();
        result.put("running", processManager.isRunning());
        result.put("exitCode", processManager.getExitCode());
        result.put("onlinePlayers", new java.util.ArrayList<>(console.getPlayerTracker().getOnlinePlayers()));
        result.put("publicPort", cfg.publicPort);
        result.put("mcPort", cfg.mcPort);
        result.put("webPort", cfg.webPort);
        ctx.json(result);
    }

    public static class ConfigUpdate {
        public String serverTitle;
        public String minecraftVersion;
        public com.mcmanager.core.model.ModLoaderInfo modLoader;
        public String javaArgs;
        public String curseforgeApiKey;
        public Boolean autoStartServer;
        public Map<String, String> serverProperties;
    }
}
