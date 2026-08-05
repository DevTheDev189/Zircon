package com.mcmanager.server.process;

import com.mcmanager.server.service.ConfigService;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

import java.io.BufferedReader;
import java.io.IOException;
import java.io.InputStreamReader;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.List;
import java.util.concurrent.TimeUnit;
import java.util.concurrent.atomic.AtomicBoolean;
import java.util.concurrent.atomic.AtomicInteger;

/**
 * Launches and supervises the Minecraft server subprocess.
 *
 * <p>The server is told to bind the internal port ({@code --port <mcPort>}) so
 * the Netty multiplexer on the public port can proxy to it. stdout/stderr are
 * piped to a {@link ConsoleStreamHandler} via virtual threads and commands can
 * be written back to the process stdin.
 */
public class MinecraftProcessManager {

    private static final Logger log = LoggerFactory.getLogger(MinecraftProcessManager.class);

    private final ConfigService configService;
    private final ConsoleStreamHandler console;

    private final AtomicBoolean running = new AtomicBoolean(false);
    private final AtomicInteger exitCode = new AtomicInteger(-1);
    private final Object lock = new Object();

    private Process process;
    private volatile boolean stopRequested = false;

    public MinecraftProcessManager(ConfigService configService, ConsoleStreamHandler console) {
        this.configService = configService;
        this.console = console;
    }

    /** @return {@code true} if a server process is currently alive. */
    public boolean isRunning() {
        return running.get() && process != null && process.isAlive();
    }

    public int getExitCode() {
        return exitCode.get();
    }

    /**
     * Starts the server. Returns immediately; the process is supervised in the
     * background and its console output is streamed to {@link ConsoleStreamHandler}.
     *
     * @throws IllegalStateException if the server JAR is missing or already running.
     */
    public void start() throws IOException {
        synchronized (lock) {
            if (isRunning()) {
                throw new IllegalStateException("Server is already running");
            }
            Path serverJar = configService.getServerJar();
            if (!Files.isRegularFile(serverJar)) {
                throw new IllegalStateException("No server.jar found at " + serverJar
                        + ". Drop the vanilla/fabric/neoforge server JAR into "
                        + configService.getServerDir());
            }

            ConfigService.ServerConfig cfg = configService.getConfig();
            List<String> command = new ArrayList<>();
            command.add(javaBin());
            command.addAll(List.of(cfg.javaArgs.split("\\s+")));
            command.add("-jar");
            command.add(serverJar.toString());
            command.add("nogui");
            command.add("--port");
            command.add(String.valueOf(cfg.mcPort));

            log.info("Launching: {}", String.join(" ", command));
            ProcessBuilder pb = new ProcessBuilder(command);
            pb.directory(configService.getServerDir().toFile());
            pb.redirectErrorStream(true);

            stopRequested = false;
            Process launched = pb.start();
            this.process = launched;
            running.set(true);
            exitCode.set(-1);

            console.accept("[wrapper] Starting Minecraft server on internal port " + cfg.mcPort
                    + " (public port " + cfg.publicPort + ")");

            Thread.ofVirtual().name("mc-stdout").start(() -> pumpOutput(launched));
            Thread.ofVirtual().name("mc-monitor").start(() -> monitor(launched));
        }
    }

    /** Writes a command to the server's stdin (e.g. "say hello"). */
    public void sendCommand(String command) {
        if (!isRunning()) {
            throw new IllegalStateException("Server is not running");
        }
        try {
            process.getOutputStream().write((command + "\n").getBytes(StandardCharsets.UTF_8));
            process.getOutputStream().flush();
            log.debug("Sent command: {}", command);
        } catch (IOException e) {
            log.warn("Failed to send command '{}'", command, e);
        }
    }

    /** Sends {@code stop}, waits for a graceful exit, then force-kills if needed. */
    public void stop() {
        synchronized (lock) {
            if (!isRunning()) {
                return;
            }
            stopRequested = true;
            console.accept("[wrapper] Stopping Minecraft server...");
            try {
                sendCommand("stop");
                if (process.waitFor(15, TimeUnit.SECONDS)) {
                    return;
                }
                log.warn("Server did not stop gracefully, force killing");
                process.destroy();
                if (process.waitFor(5, TimeUnit.SECONDS)) {
                    return;
                }
                process.destroyForcibly();
            } catch (InterruptedException e) {
                log.warn("Error while stopping server", e);
                Thread.currentThread().interrupt();
                process.destroyForcibly();
            } finally {
                running.set(false);
            }
        }
    }

    // ------------------------------------------------------------------
    // internals
    // ------------------------------------------------------------------

    private void pumpOutput(Process p) {
        try (BufferedReader reader = new BufferedReader(
                new InputStreamReader(p.getInputStream(), StandardCharsets.UTF_8))) {
            String line;
            while ((line = reader.readLine()) != null) {
                console.accept(line);
            }
        } catch (IOException e) {
            if (!stopRequested) {
                log.warn("Console stream ended unexpectedly", e);
            }
        }
    }

    private void monitor(Process p) {
        try {
            int code = p.waitFor();
            exitCode.set(code);
            if (!stopRequested) {
                console.accept("[wrapper] Minecraft server exited unexpectedly with code " + code);
                log.warn("Minecraft server exited with code {}", code);
            } else {
                console.accept("[wrapper] Minecraft server stopped (exit code " + code + ")");
            }
        } catch (InterruptedException e) {
            Thread.currentThread().interrupt();
        } finally {
            running.set(false);
        }
    }

    private String javaBin() {
        return Path.of(System.getProperty("java.home"), "bin", "java").toString();
    }
}
