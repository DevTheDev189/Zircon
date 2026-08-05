package com.mcmanager.server;

import com.mcmanager.server.multiplexer.TcpMultiplexer;
import com.mcmanager.server.process.ConsoleStreamHandler;
import com.mcmanager.server.process.MinecraftProcessManager;
import com.mcmanager.server.service.BomService;
import com.mcmanager.server.service.ConfigService;
import com.mcmanager.server.service.ModManagementService;
import com.mcmanager.server.web.JavalinApp;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

import java.io.IOException;

/**
 * Entry point of the server manager: wires up configuration, the mod/BOM
 * services, the Minecraft subprocess manager, the Javalin admin API and the
 * Netty protocol multiplexer on the public port.
 */
public class Main {

    private static final Logger log = LoggerFactory.getLogger(Main.class);

    public static void main(String[] args) throws Exception {
        ConfigService configService = new ConfigService();
        BomService bomService = new BomService(configService);
        ModManagementService modService = new ModManagementService(bomService, configService);
        ConsoleStreamHandler console = new ConsoleStreamHandler();
        MinecraftProcessManager processManager = new MinecraftProcessManager(configService, console);

        JavalinApp webApp = new JavalinApp(configService, bomService, modService, processManager, console);
        webApp.start();

        TcpMultiplexer multiplexer = new TcpMultiplexer(configService);
        multiplexer.start();

        Runtime.getRuntime().addShutdownHook(new Thread(() -> {
            log.info("Shutting down...");
            processManager.stop();
            multiplexer.stop();
            webApp.stop();
        }));

        if (configService.getConfig().autoStartServer) {
            try {
                processManager.start();
            } catch (IOException | IllegalStateException e) {
                log.warn("Auto-start failed: {}", e.getMessage());
            }
        }

        log.info("Server manager ready. Public port: {}, data dir: {}",
                configService.getConfig().publicPort, configService.getDataDir());

        // Keep the JVM alive until killed; the multiplexer + web server run on
        // their own threads but the boss/worker loops are non-daemon.
        Thread.currentThread().join();
    }
}
