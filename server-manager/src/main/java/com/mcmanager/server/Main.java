package com.mcmanager.server;

import com.mcmanager.server.auth.AuthService;
import com.mcmanager.server.auth.JwtUtil;
import com.mcmanager.server.instance.ServerInstanceManager;
import com.mcmanager.server.multiplexer.TcpMultiplexer;
import com.mcmanager.server.process.ConsoleStreamHandler;
import com.mcmanager.server.process.MinecraftProcessManager;
import com.mcmanager.server.service.BackupSchedulerService;
import com.mcmanager.server.service.BackupService;
import com.mcmanager.server.service.BomService;
import com.mcmanager.server.service.ConfigService;
import com.mcmanager.server.service.ModManagementService;
import com.mcmanager.server.web.JavalinApp;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

import java.io.IOException;

/**
 * Entry point of the server manager: wires up configuration, admin auth, the
 * multi-instance engine, the mod/BOM services, the Minecraft subprocess
 * manager, the Javalin admin API and the Netty protocol multiplexer on the
 * public port.
 */
public class Main {

    private static final Logger log = LoggerFactory.getLogger(Main.class);

    public static void main(String[] args) throws Exception {
        ConfigService configService = new ConfigService();

        // Admin auth: creates users.json + a random initial admin password on
        // first run (printed to stdout) and the JWT signing secret.
        AuthService.initializeAuth(configService.getDataDir());
        JwtUtil.initialize(configService.getDataDir());

        BomService bomService = new BomService(configService);
        ModManagementService modService = new ModManagementService(bomService, configService);
        ConsoleStreamHandler console = new ConsoleStreamHandler();
        MinecraftProcessManager processManager = new MinecraftProcessManager(configService, console);

        // Multi-instance engine (isolated <data>/instances/<id>/ dirs).
        ServerInstanceManager instanceManager = new ServerInstanceManager(configService.getDataDir(), console);

        // LZ4-compressed backups + the automatic scheduler (AGENT_PLAN_6).
        BackupService backupService = new BackupService(configService.getDataDir(), instanceManager);
        BackupSchedulerService backupScheduler = new BackupSchedulerService(instanceManager, backupService);
        backupScheduler.start();

        JavalinApp webApp = new JavalinApp(configService, bomService, modService, processManager, console,
                instanceManager, backupService);
        webApp.start();

        TcpMultiplexer multiplexer = new TcpMultiplexer(configService, instanceManager);
        multiplexer.start();

        Runtime.getRuntime().addShutdownHook(new Thread(() -> {
            log.info("Shutting down...");
            processManager.stop();
            instanceManager.listInstances().forEach(inst ->
                    instanceManager.stopInstance(inst.getId()));
            backupScheduler.stop();
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
