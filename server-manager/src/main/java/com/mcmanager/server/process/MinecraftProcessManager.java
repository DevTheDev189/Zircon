package com.mcmanager.server.process;

import com.mcmanager.core.model.InstanceConfig;
import com.mcmanager.core.model.ModLoaderInfo;
import com.mcmanager.core.model.ModLoaderType;
import com.mcmanager.server.install.ServerInstaller;
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
 * Launches and supervises a Minecraft server subprocess.
 *
 * <p>Supports two wiring styles:
 * <ul>
 *   <li>the legacy single-server layout, derived from {@link ConfigService}
 *       ({@code <data>/server}, global {@code mcPort});</li>
 *   <li>isolated Zircon instances, derived from an {@link InstanceConfig} whose
 *       server lives in {@code <data>/instances/<id>/server} and binds its own
 *       internal port.</li>
 * </ul>
 *
 * <p>The server is told to bind the internal port ({@code --port <mcPort>}) so
 * the Netty multiplexer on the public port can proxy to it. stdout/stderr are
 * piped to a {@link ConsoleStreamHandler} via virtual threads and commands can
 * be written back to the process stdin.
 */
public class MinecraftProcessManager {

    private static final Logger log = LoggerFactory.getLogger(MinecraftProcessManager.class);

    /** Immutable launch description captured at construction time. */
    private static final class LaunchContext {
        final Path serverDir;
        final Path serverJar;
        final Path installerCacheDir;
        final String minecraftVersion;
        final ModLoaderInfo loaderInfo;
        final String javaArgs;
        final int mcPort;
        final int publicPort;
        final ModLoaderType loader;
        final String loaderVersion;

        LaunchContext(Path serverDir, Path serverJar, Path installerCacheDir, String minecraftVersion,
                      ModLoaderInfo loaderInfo, String javaArgs, int mcPort, int publicPort) {
            this.serverDir = serverDir;
            this.serverJar = serverJar;
            this.installerCacheDir = installerCacheDir;
            this.minecraftVersion = minecraftVersion;
            this.loaderInfo = loaderInfo;
            this.javaArgs = javaArgs;
            this.mcPort = mcPort;
            this.publicPort = publicPort;
            this.loader = ModLoaderType.fromString(loaderInfo.getType(), null);
            this.loaderVersion = loaderInfo.getVersion();
        }
    }

    private final LaunchContext context;
    private final ConsoleStreamHandler console;

    private final AtomicBoolean running = new AtomicBoolean(false);
    private final AtomicInteger exitCode = new AtomicInteger(-1);
    private final Object lock = new Object();

    private Process process;
    private volatile boolean stopRequested = false;

    /** Legacy single-server wiring (existing tests and controllers keep working). */
    public MinecraftProcessManager(ConfigService configService, ConsoleStreamHandler console) {
        ConfigService.ServerConfig cfg = configService.getConfig();
        this.context = new LaunchContext(
                configService.getServerDir(),
                configService.getServerJar(),
                configService.getDataDir().resolve(".cache").resolve("installers"),
                cfg.minecraftVersion,
                cfg.modLoader,
                cfg.javaArgs,
                cfg.mcPort,
                cfg.publicPort);
        this.console = console;
    }

    /** Multi-instance wiring: the process manager is bound to one isolated instance. */
    public MinecraftProcessManager(InstanceConfig config, Path serverDir, Path installerCacheDir,
                                   ConsoleStreamHandler console) {
        this.context = new LaunchContext(
                serverDir,
                serverDir.resolve("server.jar"),
                installerCacheDir,
                config.getMinecraftVersion(),
                config.getModLoader(),
                config.getJavaArgs(),
                config.getInternalMcPort(),
                config.getExternalMcPort()); // dedicated player-facing port proxied by the multiplexer
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
     * The server matching the configured mod loader is installed on demand.
     *
     * @throws IllegalStateException if the server is already running
     * @throws IOException           if the server cannot be installed or launched
     */
    public void start() throws IOException {
        synchronized (lock) {
            if (isRunning()) {
                throw new IllegalStateException("Server is already running");
            }

            // Install the server matching the configured mod loader (vanilla /
            // fabric / quilt / forge / neoforge) before launching it.
            ServerInstaller.ensureServerInstalled(context.serverDir, context.serverJar,
                    context.installerCacheDir, context.minecraftVersion, context.loaderInfo);

            // Pin the server to its internal port on loopback only: players reach
            // it exclusively through the multiplexer, and internal ports never
            // bind publicly or collide with the player-facing range.
            Path propsFile = context.serverDir.resolve("server.properties");
            ConfigService.ServerProperties props = Files.isRegularFile(propsFile)
                    ? ConfigService.ServerProperties.load(propsFile)
                    : new ConfigService.ServerProperties();
            props.set("server-port", String.valueOf(context.mcPort));
            props.set("server-ip", "127.0.0.1");
            props.save(propsFile);

            List<String> command = new ArrayList<>();
            command.add(javaBin());
            command.addAll(List.of(context.javaArgs.split("\\s+")));

            if (context.loader != null && context.loader.isForgeLike()) {
                // Forge/NeoForge servers launch through the installer-generated
                // @args file (module path + JVM args + main class). Paths inside
                // the file are relative to the server dir, which is the CWD.
                Path argsFile = ServerInstaller.findServerArgsFile(context.serverDir, context.loaderVersion);
                if (argsFile == null) {
                    throw new IOException("Forge/NeoForge server args file not found after installation");
                }
                command.add("@" + context.serverDir.relativize(argsFile));
            } else {
                if (!Files.isRegularFile(context.serverJar)) {
                    throw new IllegalStateException("No server.jar found at " + context.serverJar
                            + ". Drop the vanilla/fabric server JAR into " + context.serverDir);
                }
                command.add("-jar");
                command.add(context.serverJar.toString());
            }
            command.add("nogui");
            command.add("--port");
            command.add(String.valueOf(context.mcPort));

            log.info("Launching: {}", String.join(" ", command));
            ProcessBuilder pb = new ProcessBuilder(command);
            pb.directory(context.serverDir.toFile());
            pb.redirectErrorStream(true);

            stopRequested = false;
            Process launched = pb.start();
            this.process = launched;
            running.set(true);
            exitCode.set(-1);

            String publicPortText = context.publicPort > 0
                    ? " (public port " + context.publicPort + ")" : "";
            console.accept("[wrapper] Starting Minecraft server on internal port " + context.mcPort
                    + publicPortText);

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
