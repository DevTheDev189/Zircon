package com.mcmanager.server.web.controller;

import com.google.gson.JsonArray;
import com.google.gson.JsonElement;
import com.google.gson.JsonParser;
import com.mcmanager.server.process.ConsoleStreamHandler;
import com.mcmanager.server.process.MinecraftProcessManager;
import com.mcmanager.server.service.ConfigService;
import io.javalin.http.Context;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

import java.io.IOException;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.HashMap;
import java.util.List;
import java.util.Map;

/**
 * Player management endpoints. Lists come from the vanilla JSON files
 * (whitelist.json, banned-players.json, ops.json) and mutations are performed by
 * sending the corresponding server commands through the console.
 */
public class PlayerController {

    private static final Logger log = LoggerFactory.getLogger(PlayerController.class);

    private final ConfigService configService;
    private final MinecraftProcessManager processManager;
    private final ConsoleStreamHandler console;

    public PlayerController(ConfigService configService, MinecraftProcessManager processManager,
                            ConsoleStreamHandler console) {
        this.configService = configService;
        this.processManager = processManager;
        this.console = console;
    }

    /** GET /api/players/online — names of players currently connected. */
    public void online(Context ctx) {
        ctx.json(Map.of("players", new ArrayList<>(console.getPlayerTracker().getOnlinePlayers())));
    }

    /** GET /api/players/whitelist — contents of whitelist.json. */
    public void getWhitelist(Context ctx) {
        ctx.json(Map.of("players", readJsonList("whitelist.json")));
    }

    /** POST /api/players/whitelist {"name":"Steve"} — whitelist add. */
    public void addWhitelist(Context ctx) {
        PlayerAction body = ctx.bodyAsClass(PlayerAction.class);
        if (body == null || blank(body.name)) {
            ctx.status(400).result("name is required");
            return;
        }
        ctx.json(runCommand("whitelist add " + body.name));
    }

    /** DELETE /api/players/whitelist/{name} — whitelist remove. */
    public void removeWhitelist(Context ctx) {
        ctx.json(runCommand("whitelist remove " + ctx.pathParam("name")));
    }

    /** GET /api/players/bans — contents of banned-players.json. */
    public void getBans(Context ctx) {
        ctx.json(Map.of("players", readJsonList("banned-players.json")));
    }

    /** POST /api/players/bans {"name":"X","reason":"..."} — ban. */
    public void addBan(Context ctx) {
        PlayerAction body = ctx.bodyAsClass(PlayerAction.class);
        if (body == null || blank(body.name)) {
            ctx.status(400).result("name is required");
            return;
        }
        String reason = blank(body.reason) ? "" : " " + body.reason;
        ctx.json(runCommand("ban " + body.name + reason));
    }

    /** DELETE /api/players/bans/{name} — pardon. */
    public void removeBan(Context ctx) {
        ctx.json(runCommand("pardon " + ctx.pathParam("name")));
    }

    /** GET /api/players/ops — contents of ops.json. */
    public void getOps(Context ctx) {
        ctx.json(Map.of("players", readJsonList("ops.json")));
    }

    /** POST /api/players/ops {"name":"Steve"} — op. */
    public void addOp(Context ctx) {
        PlayerAction body = ctx.bodyAsClass(PlayerAction.class);
        if (body == null || blank(body.name)) {
            ctx.status(400).result("name is required");
            return;
        }
        ctx.json(runCommand("op " + body.name));
    }

    /** DELETE /api/players/ops/{name} — deop. */
    public void removeOp(Context ctx) {
        ctx.json(runCommand("deop " + ctx.pathParam("name")));
    }

    /** POST /api/players/kick {"name":"X","reason":"..."} — kick. */
    public void kick(Context ctx) {
        PlayerAction body = ctx.bodyAsClass(PlayerAction.class);
        if (body == null || blank(body.name)) {
            ctx.status(400).result("name is required");
            return;
        }
        String reason = blank(body.reason) ? "" : " " + body.reason;
        ctx.json(runCommand("kick " + body.name + reason));
    }

    /** POST /api/players/command {"command":"say hi"} — arbitrary server command. */
    public void runCommand(Context ctx) {
        PlayerAction body = ctx.bodyAsClass(PlayerAction.class);
        if (body == null || blank(body.command)) {
            ctx.status(400).result("command is required");
            return;
        }
        ctx.json(runCommand(body.command.trim()));
    }

    // ------------------------------------------------------------------

    private Map<String, Object> runCommand(String command) {
        Map<String, Object> result = new HashMap<>();
        result.put("command", command);
        try {
            processManager.sendCommand(command);
            result.put("sent", true);
        } catch (IllegalStateException e) {
            result.put("sent", false);
            result.put("error", e.getMessage());
        }
        return result;
    }

    private List<Map<String, Object>> readJsonList(String fileName) {
        Path file = configService.getServerDir().resolve(fileName);
        List<Map<String, Object>> out = new ArrayList<>();
        if (!Files.isRegularFile(file)) {
            return out;
        }
        try {
            JsonArray arr = JsonParser.parseString(Files.readString(file)).getAsJsonArray();
            for (JsonElement element : arr) {
                Map<String, Object> entry = new HashMap<>();
                if (element.isJsonObject()) {
                    var obj = element.getAsJsonObject();
                    entry.put("uuid", obj.has("uuid") ? obj.get("uuid").getAsString() : "");
                    entry.put("name", obj.has("name") ? obj.get("name").getAsString() : "");
                    if (obj.has("reason")) {
                        entry.put("reason", obj.get("reason").getAsString());
                    }
                    if (obj.has("created")) {
                        entry.put("created", obj.get("created").getAsString());
                    }
                }
                out.add(entry);
            }
        } catch (IOException | RuntimeException e) {
            log.warn("Could not read {}", fileName, e);
        }
        return out;
    }

    private boolean blank(String s) {
        return s == null || s.isBlank();
    }

    public static class PlayerAction {
        public String name;
        public String reason;
        public String command;
    }
}
