package com.mcmanager.client.launch;

import com.mcmanager.client.auth.SessionData;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

import java.io.BufferedReader;
import java.io.IOException;
import java.io.InputStreamReader;
import java.nio.charset.StandardCharsets;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.List;
import java.util.function.Consumer;

/**
 * Builds and runs the Minecraft client process with the resolved classpath,
 * injecting the session and auto-connecting the player straight to the server
 * via {@code --server}/{@code --port} (plan task 3.4).
 */
public class MinecraftRunner {

    private static final Logger log = LoggerFactory.getLogger(MinecraftRunner.class);

    /**
     * Launches the game.
     *
     * @param data       resolved launch environment
     * @param session    authenticated Minecraft session
     * @param gameDir    the game directory (contains mods/, config/, ...)
     * @param serverIp   server address to auto-connect to (e.g. "mc.example.com")
     * @param serverPort public server port
     * @param output     receives game stdout lines (may be {@code null})
     */
    public Process launch(MinecraftClasspathBuilder.LaunchData data, SessionData session,
                          Path gameDir, String serverIp, int serverPort, Consumer<String> output)
            throws IOException {
        Path java = data.javaHome().resolve("bin/java"
                + (System.getProperty("os.name").toLowerCase().contains("win") ? ".exe" : ""));

        List<String> command = new ArrayList<>();
        command.add(java.toString());
        command.add("-Xmx4G");
        command.add("-Djava.library.path=" + data.nativesDir());
        command.add("-cp");
        command.add(data.classpath());
        command.add(data.mainClass());
        addArg(command, "--username", session.getUsername());
        addArg(command, "--version", data.versionName());
        addArg(command, "--gameDir", gameDir.toString());
        addArg(command, "--assetsDir", data.assetsDir().toString());
        addArg(command, "--assetIndex", data.assetIndexId());
        addArg(command, "--uuid", session.getUuid());
        addArg(command, "--accessToken", session.getAccessToken());
        addArg(command, "--userType", "msa");
        addArg(command, "--versionType", "release");
        addArg(command, "--server", serverIp);
        addArg(command, "--port", String.valueOf(serverPort));

        log.info("Launching Minecraft: {}", String.join(" ", command.subList(0, 6)) + " ...");

        ProcessBuilder pb = new ProcessBuilder(command);
        pb.directory(gameDir.toFile());
        pb.redirectErrorStream(true);
        Process process = pb.start();

        Thread.ofVirtual().name("mc-client-output").start(() -> pump(process, output));
        return process;
    }

    private static void addArg(List<String> command, String key, String value) {
        command.add(key);
        command.add(value);
    }

    private void pump(Process process, Consumer<String> output) {
        try (BufferedReader reader = new BufferedReader(
                new InputStreamReader(process.getInputStream(), StandardCharsets.UTF_8))) {
            String line;
            while ((line = reader.readLine()) != null) {
                if (output != null) {
                    try {
                        output.accept(line);
                    } catch (RuntimeException ignored) {
                    }
                } else {
                    System.out.println(line);
                }
            }
        } catch (IOException e) {
            if (process.isAlive()) {
                log.warn("Game output stream ended unexpectedly", e);
            }
        }
    }
}
