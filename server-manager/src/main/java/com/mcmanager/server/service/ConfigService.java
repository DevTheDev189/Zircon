package com.mcmanager.server.service;

import com.google.gson.Gson;
import com.google.gson.GsonBuilder;
import com.mcmanager.core.model.ModLoaderInfo;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

import java.io.IOException;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;

/**
 * Owns the on-disk layout of the server wrapper and the {@code config.json} /
 * {@code server.properties} files.
 *
 * <pre>
 * dataDir/
 *   config.json          - wrapper settings (ports, paths, loader, title)
 *   bom.json             - the published Bill of Materials
 *   mods/                - mod JARs hosted for clients
 *   server/              - the actual Minecraft server (server.jar, server.properties, ...)
 * </pre>
 */
public class ConfigService {

    private static final Logger log = LoggerFactory.getLogger(ConfigService.class);
    private static final Gson GSON = new GsonBuilder().setPrettyPrinting().create();

    public static final int DEFAULT_PUBLIC_PORT = 25565;
    public static final int DEFAULT_WEB_PORT = 25564;
    public static final int DEFAULT_MC_PORT = 25566;

    private final Path dataDir;
    private final Path configFile;
    private final Path modsDir;
    private final Path serverDir;
    private final Path bomFile;
    private final Path serverJar;
    private final Path serverPropertiesFile;

    private ServerConfig config;

    public ConfigService() throws IOException {
        String override = System.getProperty("mcmanager.dataDir");
        this.dataDir = (override != null
                ? Path.of(override).toAbsolutePath()
                : Path.of(".").toAbsolutePath().resolve("server-data"))
                .normalize();
        this.configFile = dataDir.resolve("config.json");
        this.modsDir = dataDir.resolve("mods");
        this.serverDir = dataDir.resolve("server");
        this.bomFile = dataDir.resolve("bom.json");
        this.serverJar = serverDir.resolve("server.jar");
        this.serverPropertiesFile = serverDir.resolve("server.properties");

        Files.createDirectories(modsDir);
        Files.createDirectories(serverDir);

        this.config = loadConfig();
    }

    // ------------------------------------------------------------------
    // config.json
    // ------------------------------------------------------------------

    private ServerConfig loadConfig() throws IOException {
        if (Files.exists(configFile)) {
            try {
                ServerConfig loaded = GSON.fromJson(Files.readString(configFile), ServerConfig.class);
                if (loaded != null) {
                    loaded.applyDefaults();
                    return loaded;
                }
            } catch (IOException | RuntimeException e) {
                log.warn("Could not parse {}, falling back to defaults", configFile, e);
            }
        }
        ServerConfig fresh = new ServerConfig();
        saveConfig(fresh);
        return fresh;
    }

    public synchronized void saveConfig() throws IOException {
        saveConfig(config);
    }

    private void saveConfig(ServerConfig cfg) throws IOException {
        Files.writeString(configFile, GSON.toJson(cfg), StandardCharsets.UTF_8);
    }

    public ServerConfig getConfig() {
        return config;
    }

    // ------------------------------------------------------------------
    // Paths
    // ------------------------------------------------------------------

    public Path getDataDir() {
        return dataDir;
    }

    public Path getModsDir() {
        return modsDir;
    }

    public Path getServerDir() {
        return serverDir;
    }

    public Path getBomFile() {
        return bomFile;
    }

    public Path getServerJar() {
        return serverJar;
    }

    public Path getServerPropertiesFile() {
        return serverPropertiesFile;
    }

    // ------------------------------------------------------------------
    // server.properties
    // ------------------------------------------------------------------

    /** Loads {@code server.properties}, creating the file with defaults if absent. */
    public ServerProperties loadServerProperties() throws IOException {
        if (!Files.exists(serverPropertiesFile)) {
            ServerProperties fresh = new ServerProperties();
            fresh.set("server-port", String.valueOf(config.mcPort));
            fresh.set("motd", config.serverTitle);
            fresh.save(serverPropertiesFile);
            return fresh;
        }
        return ServerProperties.load(serverPropertiesFile);
    }

    public synchronized void saveServerProperties(ServerProperties props) throws IOException {
        props.save(serverPropertiesFile);
    }

    // ------------------------------------------------------------------
    // DTO
    // ------------------------------------------------------------------

    /** Serializable wrapper settings. */
    public static class ServerConfig {
        public int webPort = DEFAULT_WEB_PORT;
        public int mcPort = DEFAULT_MC_PORT;
        public int publicPort = DEFAULT_PUBLIC_PORT;

        public String serverTitle = "My Minecraft Server";
        public String minecraftVersion = "1.21.4";
        public ModLoaderInfo modLoader = new ModLoaderInfo("fabric", "", "");

        public String javaArgs = "-Xms2G -Xmx4G";
        public boolean autoStartServer = false;

        public String curseforgeApiKey = "";

        private void applyDefaults() {
            if (serverTitle == null) serverTitle = "My Minecraft Server";
            if (minecraftVersion == null) minecraftVersion = "1.21.4";
            if (modLoader == null) modLoader = new ModLoaderInfo("fabric", "", "");
            if (modLoader.getType() == null) modLoader.setType("fabric");
            if (javaArgs == null) javaArgs = "-Xms2G -Xmx4G";
            if (curseforgeApiKey == null) curseforgeApiKey = "";
        }
    }

    // ------------------------------------------------------------------
    // server.properties helper
    // ------------------------------------------------------------------

    /**
     * Line-preserving editor for {@code server.properties}: comments and unknown
     * keys survive a round-trip; known keys get their values updated in place.
     */
    public static class ServerProperties {

        private final List<String> lines = new ArrayList<>();
        private final Map<String, Integer> keyToLine = new LinkedHashMap<>();
        private final Map<String, String> values = new LinkedHashMap<>();

        public static ServerProperties load(Path file) throws IOException {
            ServerProperties props = new ServerProperties();
            for (String raw : Files.readAllLines(file, StandardCharsets.UTF_8)) {
                String trimmed = raw.trim();
                if (!trimmed.isEmpty() && !trimmed.startsWith("#")) {
                    int eq = trimmed.indexOf('=');
                    if (eq > 0) {
                        String key = trimmed.substring(0, eq).trim();
                        String value = trimmed.substring(eq + 1).trim();
                        props.lines.add(raw);
                        props.keyToLine.put(key, props.lines.size() - 1);
                        props.values.put(key, value);
                        continue;
                    }
                }
                props.lines.add(raw);
            }
            return props;
        }

        public String get(String key, String defaultValue) {
            return values.getOrDefault(key, defaultValue);
        }

        public void set(String key, String value) {
            Integer lineIndex = keyToLine.get(key);
            if (lineIndex == null) {
                keyToLine.put(key, lines.size());
                values.put(key, value);
                lines.add(key + "=" + value);
            } else {
                values.put(key, value);
                lines.set(lineIndex, key + "=" + value);
            }
        }

        public Map<String, String> asMap() {
            return new LinkedHashMap<>(values);
        }

        public void save(Path file) throws IOException {
            Files.write(file, lines, StandardCharsets.UTF_8);
        }
    }
}
