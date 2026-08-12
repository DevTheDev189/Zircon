package com.mcmanager.server.web.controller;

import com.mcmanager.server.process.ConsoleStreamHandler;
import com.mcmanager.server.process.MinecraftProcessManager;
import io.javalin.websocket.WsConfig;
import org.eclipse.jetty.websocket.api.Session;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

import java.util.Set;
import java.util.concurrent.ConcurrentHashMap;

/**
 * WebSocket endpoint {@code /api/console}. Server console lines are streamed to
 * every connected session; messages sent by a client are written to the server
 * stdin as commands (this is how the admin UI sends "whitelist add X" etc.).
 */
public class ConsoleController {

    private static final Logger log = LoggerFactory.getLogger(ConsoleController.class);

    private final ConsoleStreamHandler console;
    private final MinecraftProcessManager processManager;
    private final Set<Session> sessions = ConcurrentHashMap.newKeySet();

    public ConsoleController(ConsoleStreamHandler console, MinecraftProcessManager processManager) {
        this.console = console;
        this.processManager = processManager;
    }

    public void register(WsConfig ws) {
        ws.onConnect(ctx -> {
            sessions.add(ctx.session);
            log.info("Console client connected: {}", ctx.session.getRemoteAddress());
            // Replay recent history so the UI is not blank on connect.
            for (String line : console.recentHistory(500)) {
                ctx.session.getRemote().sendString(line);
            }
        });

        ws.onMessage(ctx -> {
            String command = ctx.message();
            if (command == null || command.isBlank()) {
                return;
            }
            if (command.trim().equals("__CLEAR__")) {
                console.clearHistory();
                broadcast("__CLEAR__");
                return;
            }
            try {
                processManager.sendCommand(command.trim());
            } catch (IllegalStateException e) {
                ctx.send("[wrapper] " + e.getMessage());
            }
        });
        ws.onClose(ctx -> sessions.remove(ctx.session));
        ws.onError(ctx -> sessions.remove(ctx.session));
    }

    /** Broadcasts one console line to all connected sessions. */
    public void broadcast(String line) {
        for (Session session : sessions) {
            try {
                if (session.isOpen()) {
                    session.getRemote().sendString(line);
                }
            } catch (Exception e) {
                sessions.remove(session);
            }
        }
    }
}
