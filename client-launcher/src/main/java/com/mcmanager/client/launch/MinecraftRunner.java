package com.mcmanager.client.launch;

import com.mcmanager.client.auth.SessionData;
import com.mcmanager.client.profile.VersionProfileResolver;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

import java.io.BufferedReader;
import java.io.IOException;
import java.io.InputStreamReader;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.HashMap;
import java.util.List;
import java.util.Map;
import java.util.function.Consumer;

/**
 * Builds and runs the Minecraft client process with the resolved classpath,
 * injecting the session and auto-connecting the player straight to the server
 * (plan task 3.4).
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
        // Forge/NeoForge contribute JVM args from the version profile chain:
        // -p module path, --add-modules/--add-opens/--add-exports, -D system
        // properties such as -DlibraryDirectory and -DignoreList.
        command.addAll(data.jvmArgs());
        command.add("-Xmx4G");
        command.add("-Djava.library.path=" + data.nativesDir());
        command.add("-cp");
        command.add(data.classpath());
        command.add(data.mainClass());

        // Sessions are always produced by Microsoft auth (userType=msa) with a real
        // access token; the fallbacks are purely defensive.
        String userType = session.getUserType() == null ? "msa" : session.getUserType();
        String accessToken = session.getAccessToken() == null || session.getAccessToken().isBlank()
                ? "0" : session.getAccessToken();

        if (!data.gameArgs().isEmpty()) {
            // Forge/NeoForge: the version profile (including the inherited vanilla
            // profile) already supplies the complete standard game arguments
            // (--username, --gameDir, --accessToken, ...). Resolve their
            // placeholders instead of re-adding them below.
            Map<String, String> tokens = launchTokens(data, session, gameDir, serverIp, serverPort,
                    accessToken, userType);
            List<String> profileGameArgs = new ArrayList<>();
            for (String arg : data.gameArgs()) {
                profileGameArgs.add(VersionProfileResolver.substitute(arg, tokens));
            }
            // The profile may contribute --quickPlayMultiplayer; drop it so the
            // canonical auto-connect args below win (no duplicate keys).
            for (int i = 0; i < profileGameArgs.size(); i++) {
                if ("--quickPlayMultiplayer".equals(profileGameArgs.get(i)) && i + 1 < profileGameArgs.size()) {
                    profileGameArgs.remove(i + 1);
                    profileGameArgs.remove(i);
                    i--;
                }
            }
            command.addAll(profileGameArgs);
        } else {
            addArg(command, "--username", session.getUsername());
            addArg(command, "--version", data.versionName());
            addArg(command, "--gameDir", gameDir.toString());
            addArg(command, "--assetsDir", data.assetsDir().toString());
            addArg(command, "--assetIndex", data.assetIndexId());
            addArg(command, "--uuid", session.getUuid());
            addArg(command, "--accessToken", accessToken);
            addArg(command, "--userType", userType);
            addArg(command, "--versionType", "release");
        }

        // Auto-connect: modern Minecraft (1.20.2+) replaced --server/--port with
        // --quickPlayMultiplayer <host:port> and ignores the old args. Passing
        // both keeps compatibility with older versions (they ignore the unknown one).
        addArg(command, "--server", serverIp);
        addArg(command, "--port", String.valueOf(serverPort));
        addArg(command, "--quickPlayMultiplayer", serverIp + ":" + serverPort);

        // Start the game in fullscreen mode.
        command.add("--fullscreen");

        // Pre-accept the "multiplayer is third-party" disclaimer so the game
        // auto-joins instead of stopping at the warning screen.
        acceptMultiplayerWarning(gameDir);

        // Set the video-setting the game actually honors on boot, so the window
        // opens fullscreen even when the profile args differ between loaders.
        enableFullscreen(gameDir);

        log.info("Launching Minecraft: {} --quickPlayMultiplayer {}:{} ...",
                String.join(" ", command.subList(0, 6)), serverIp, serverPort);

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

    /**
     * Token map for resolving the version profile's game-argument placeholders
     * (only used by Forge/NeoForge launches). Mirrors the tokens the official
     * launcher fills in from the authenticated session and the resolved paths.
     */
    private static Map<String, String> launchTokens(MinecraftClasspathBuilder.LaunchData data,
                                                    SessionData session, Path gameDir,
                                                    String serverIp, int serverPort,
                                                    String accessToken, String userType) {
        Map<String, String> tokens = new HashMap<>();
        tokens.put("auth_player_name", session.getUsername());
        tokens.put("auth_uuid", session.getUuid());
        tokens.put("auth_access_token", accessToken);
        tokens.put("auth_xuid", "");
        tokens.put("clientid", "");
        tokens.put("user_type", userType);
        tokens.put("version_type", "release");
        tokens.put("version_name", data.versionName());
        tokens.put("game_directory", gameDir.toString());
        tokens.put("assets_root", data.assetsDir().toString());
        tokens.put("assets_index_name", data.assetIndexId());
        tokens.put("quickPlayMultiplayer", serverIp + ":" + serverPort);
        tokens.put("launcher_name", "mcmanager");
        tokens.put("launcher_version", "1.0.0");
        return tokens;
    }

    /**
     * Ensures {@code options.txt} has {@code skipMultiplayerWarning:true} so the
     * game skips the third-party disclaimer screen and quick-play auto-join works.
     * Preserves every other option; safe to run repeatedly.
     */
    private static void acceptMultiplayerWarning(Path gameDir) throws IOException {
        setOptionsEntry(gameDir, "skipMultiplayerWarning", "true");
    }

    /**
     * Ensures {@code options.txt} has {@code fullscreen:true} so the game opens
     * in fullscreen mode on startup. The {@code --fullscreen} launch flag alone is
     * not honored by every loader, but the video setting is.
     */
    private static void enableFullscreen(Path gameDir) throws IOException {
        setOptionsEntry(gameDir, "fullscreen", "true");
    }

    /** Upserts a {@code key:value} entry in the instance's {@code options.txt}. */
    private static void setOptionsEntry(Path gameDir, String key, String value) throws IOException {
        Path options = gameDir.resolve("options.txt");
        List<String> lines = Files.isRegularFile(options)
                ? Files.readAllLines(options, StandardCharsets.UTF_8)
                : new ArrayList<>();
        String prefix = key + ":";
        boolean found = false;
        for (int i = 0; i < lines.size(); i++) {
            if (lines.get(i).startsWith(prefix)) {
                lines.set(i, prefix + value);
                found = true;
            }
        }
        if (!found) {
            lines.add(prefix + value);
        }
        Files.write(options, lines, StandardCharsets.UTF_8);
        log.info("Set {} in {}", prefix + value, options);
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
