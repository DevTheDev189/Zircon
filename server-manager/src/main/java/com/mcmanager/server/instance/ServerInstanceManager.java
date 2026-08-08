package com.mcmanager.server.instance;

import com.google.gson.Gson;
import com.google.gson.GsonBuilder;
import com.mcmanager.core.model.BillOfMaterials;
import com.mcmanager.core.model.InstanceConfig;
import com.mcmanager.server.process.ConsoleStreamHandler;
import com.mcmanager.server.process.MinecraftProcessManager;
import com.mcmanager.server.process.PlayerTracker;
import com.mcmanager.server.service.BomService;
import com.mcmanager.server.service.ModManagementService;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

import java.io.IOException;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.Comparator;
import java.util.List;
import java.util.Locale;
import java.util.Map;
import java.util.Set;
import java.util.concurrent.ConcurrentHashMap;
import java.util.stream.Stream;

/**
 * Lifecycle manager for multiple isolated Zircon server instances.
 *
 * <p>Each instance lives in {@code <data>/instances/<id>/} and owns its own
 * {@code instance.json} (metadata, loader LOCKED here), {@code bom.json},
 * {@code mods/} and {@code server/} directory — cross-loader file pollution is
 * impossible by construction.
 *
 * <p>The mod loader is frozen at creation: {@link #updateInstanceConfig} only
 * ever mutates name/javaArgs/autoStart and there is no API to change the loader
 * of an existing instance.
 */
public class ServerInstanceManager {

    private static final Logger log = LoggerFactory.getLogger(ServerInstanceManager.class);
    private static final Gson GSON = new GsonBuilder().setPrettyPrinting().create();

    /** First automatically assigned internal MC port (incremented per instance). */
    public static final int MC_PORT_BASE = 25566;

    /** Standard Mojang eula.txt content; the server refuses to boot without it. */
    private static final String EULA_TEXT = """
            #By changing the settings below to TRUE you are indicating your agreement to our EULA (https://aka.ms/MinecraftEULA).
            eula=true
            """;

    private final Path dataDir;
    private final Path instancesDir;
    private final Path installerCacheDir;
    private final ConsoleStreamHandler console;

    private final Map<String, InstanceConfig> instanceConfigs = new ConcurrentHashMap<>();
    private final Map<String, MinecraftProcessManager> activeProcesses = new ConcurrentHashMap<>();
    /** Per-instance player sets, fed by each instance's own console stream. */
    private final Map<String, PlayerTracker> playerTrackers = new ConcurrentHashMap<>();

    /**
     * The instance whose data the client-facing legacy endpoints ({@code /bom},
     * {@code /files/mods/*}) serve. Falls back to the first-created instance on
     * startup; starting an instance makes it the active one.
     */
    private volatile String activeInstanceId;

    public ServerInstanceManager(Path dataDir, ConsoleStreamHandler console) throws IOException {
        this.dataDir = dataDir;
        this.instancesDir = dataDir.resolve("instances");
        this.installerCacheDir = dataDir.resolve(".cache").resolve("installers");
        this.console = console;
        Files.createDirectories(instancesDir);
        Files.createDirectories(installerCacheDir);
        loadFromDisk();
    }

    // ------------------------------------------------------------------
    // Instance lifecycle
    // ------------------------------------------------------------------

    /**
     * Creates a new instance and persists it. The mod loader choice is frozen
     * in {@link InstanceConfig} from this moment on.
     */
    public synchronized InstanceConfig createInstance(String name, String mcVersion,
                                                      String loaderType, String loaderVersion) {
        InstanceConfig config = new InstanceConfig(name, mcVersion, loaderType, loaderVersion,
                allocateNextPort());
        try {
            Files.createDirectories(instanceDir(config.getId()));
            Files.createDirectories(instanceDir(config.getId()).resolve("mods"));
            Files.createDirectories(instanceDir(config.getId()).resolve("server"));
            saveInstanceToDisk(config);
        } catch (IOException e) {
            throw new IllegalStateException("Could not persist new instance " + config.getId(), e);
        }
        instanceConfigs.put(config.getId(), config);
        if (activeInstanceId == null) {
            activeInstanceId = config.getId();
            log.info("Instance '{}' is now the active instance (first created)", name);
        }
        log.info("Created instance '{}' ({} {} / loader {}, internal port {})",
                name, mcVersion, loaderType, loaderVersion, config.getInternalMcPort());
        return config;
    }

    public synchronized void startInstance(String instanceId) throws IOException {
        InstanceConfig config = getInstance(instanceId);
        if (!isEulaAccepted(instanceId)) {
            // Fail fast with a clear error instead of letting the MC server
            // boot and immediately exit with the EULA prompt.
            throw new IllegalStateException("The Minecraft EULA has not been accepted for instance '"
                    + config.getName() + "'. Accept it in the admin UI (Settings tab) first.");
        }
        MinecraftProcessManager pm = activeProcesses.get(instanceId);
        if (pm == null) {
            // Each instance gets its own console so its player activity is
            // tracked separately; every line is forwarded to the shared console
            // so the WebSocket console and legacy views keep working.
            ConsoleStreamHandler instConsole = new ConsoleStreamHandler();
            instConsole.addListener(console::accept);
            playerTrackers.put(instanceId, instConsole.getPlayerTracker());
            pm = new MinecraftProcessManager(config, instanceDir(instanceId).resolve("server"),
                    installerCacheDir, instConsole);
            activeProcesses.put(instanceId, pm);
        }
        pm.start();
        if (!instanceId.equals(activeInstanceId)) {
            activeInstanceId = instanceId;
            log.info("Instance '{}' is now the active instance (started)", config.getName());
        }
        log.info("Instance '{}' started on internal port {}", config.getName(), config.getInternalMcPort());
    }

    public synchronized void stopInstance(String instanceId) {
        MinecraftProcessManager pm = activeProcesses.remove(instanceId);
        if (pm != null) {
            pm.stop();
        }
        playerTrackers.remove(instanceId);
    }

    /**
     * Renames / re-arms an instance. Note that the {@code modLoader} can never
     * be changed — passing one here is intentionally impossible.
     */
    public synchronized void updateInstanceConfig(String instanceId, String newName, String newJavaArgs) {
        InstanceConfig config = getInstance(instanceId);
        if (newName != null && !newName.isBlank()) {
            config.setName(newName);
        }
        if (newJavaArgs != null) {
            config.setJavaArgs(sanitizeJavaArgs(newJavaArgs));
        }
        saveInstanceToDisk(config);
    }

    public synchronized void updateAutoStart(String instanceId, boolean autoStart) {
        InstanceConfig config = getInstance(instanceId);
        config.setAutoStart(autoStart);
        saveInstanceToDisk(config);
    }

    /**
     * Applies a Minecraft / loader version change (and optionally a rename) to an
     * instance, then re-syncs every installed mod against the new versions. The mod
     * loader <em>type</em> stays locked — only its version string may change.
     *
     * @return the mod-sync summary from {@link ModManagementService#syncModsForVersionChange}.
     */
    public synchronized Map<String, Object> updateInstanceVersions(String instanceId,
                                                                   String newMcVersion,
                                                                   String newLoaderVersion,
                                                                   String newName) throws IOException {
        InstanceConfig config = getInstance(instanceId);
        if (newName != null && !newName.isBlank()) config.setName(newName);
        if (newMcVersion != null && !newMcVersion.isBlank()) config.setMinecraftVersion(newMcVersion);
        if (newLoaderVersion != null) config.setLoaderVersion(newLoaderVersion);
        saveInstanceToDisk(config);

        Path instanceDir = instanceDir(instanceId);
        BomService bom = new BomService(instanceDir.resolve("bom.json"),
                new BillOfMaterials(config.getMinecraftVersion(), config.getModLoader(), config.getName()));
        ModManagementService mods = new ModManagementService(bom, instanceDir.resolve("mods"), "");

        String loaderType = config.getModLoader() == null ? "vanilla" : config.getModLoader().getType();
        String loaderVersion = config.getModLoader() == null ? "" : config.getModLoader().getVersion();
        return mods.syncModsForVersionChange(config.getMinecraftVersion(), loaderType, loaderVersion);
    }

    /** Stops (if running), removes the process manager and deletes the instance dir. */
    public synchronized boolean deleteInstance(String instanceId) throws IOException {
        InstanceConfig config = instanceConfigs.get(instanceId);
        if (config == null) {
            return false;
        }
        stopInstance(instanceId);
        instanceConfigs.remove(instanceId);
        if (instanceId.equals(activeInstanceId)) {
            activeInstanceId = pickDefaultActiveInstance();
            log.info("Active instance is now {}", activeInstanceId == null ? "none (legacy mode)" : activeInstanceId);
        }
        Path dir = instanceDir(instanceId);
        if (Files.isDirectory(dir)) {
            try (Stream<Path> walk = Files.walk(dir)) {
                for (Path p : walk.sorted(Comparator.reverseOrder()).toList()) {
                    Files.deleteIfExists(p);
                }
            }
        }
        log.info("Deleted instance '{}'", config.getName());
        return true;
    }

    /** @return {@code true} if the instance's {@code server/eula.txt} contains {@code eula=true}. */
    public boolean isEulaAccepted(String instanceId) {
        getInstance(instanceId); // 404 for unknown ids
        Path eula = instanceDir(instanceId).resolve("server").resolve("eula.txt");
        if (!Files.isRegularFile(eula)) {
            return false;
        }
        try {
            return Files.readAllLines(eula, StandardCharsets.UTF_8).stream()
                    .map(String::trim)
                    .filter(line -> line.startsWith("eula="))
                    .anyMatch(line -> line.substring("eula=".length()).trim().equalsIgnoreCase("true"));
        } catch (IOException e) {
            log.warn("Could not read {}: {}", eula, e.getMessage());
            return false;
        }
    }

    /** Writes {@code server/eula.txt} with {@code eula=true} (records the operator's consent). */
    public synchronized void acceptEula(String instanceId) throws IOException {
        getInstance(instanceId); // 404 for unknown ids
        Path serverDir = instanceDir(instanceId).resolve("server");
        Files.createDirectories(serverDir);
        Files.writeString(serverDir.resolve("eula.txt"), EULA_TEXT, StandardCharsets.UTF_8);
        log.info("EULA accepted for instance {}", instanceId);
    }

    // ------------------------------------------------------------------
    // Queries
    // ------------------------------------------------------------------

    public InstanceConfig getInstance(String instanceId) {
        InstanceConfig config = instanceConfigs.get(instanceId);
        if (config == null) {
            throw new IllegalArgumentException("Instance not found: " + instanceId);
        }
        return config;
    }

    public List<InstanceConfig> listInstances() {
        return new ArrayList<>(instanceConfigs.values());
    }

    public boolean isRunning(String instanceId) {
        MinecraftProcessManager pm = activeProcesses.get(instanceId);
        return pm != null && pm.isRunning();
    }

    /** @return the instance's currently online players (empty when not running). */
    public Set<String> getOnlinePlayers(String instanceId) {
        if (!isRunning(instanceId)) {
            return Set.of();
        }
        PlayerTracker tracker = playerTrackers.get(instanceId);
        return tracker == null ? Set.of() : tracker.getOnlinePlayers();
    }

    public int getOnlinePlayerCount(String instanceId) {
        return getOnlinePlayers(instanceId).size();
    }

    public MinecraftProcessManager getProcessManager(String instanceId) {
        return activeProcesses.get(instanceId);
    }

    /** Resolves the instance whose id or (normalized) name matches a handshake hostname. */
    public InstanceConfig findByHostname(String hostname) {
        if (hostname == null || hostname.isBlank()) {
            return null;
        }
        String h = hostname.trim().toLowerCase(Locale.ROOT);
        for (InstanceConfig cfg : instanceConfigs.values()) {
            if (h.equals(cfg.getId().toLowerCase(Locale.ROOT))) {
                return cfg;
            }
            if (normalizeName(cfg.getName()).equals(h)) {
                return cfg;
            }
        }
        return null;
    }

    public Path getInstanceDir(String instanceId) {
        return instanceDir(instanceId);
    }

    public Path getInstancesDir() {
        return instancesDir;
    }

    // ------------------------------------------------------------------
    // persistence
    // ------------------------------------------------------------------

    private void loadFromDisk() throws IOException {
        try (Stream<Path> dirs = Files.list(instancesDir)) {
            for (Path dir : dirs.filter(Files::isDirectory).toList()) {
                Path cfgFile = dir.resolve("instance.json");
                if (!Files.isRegularFile(cfgFile)) {
                    continue;
                }
                try {
                    InstanceConfig config = GSON.fromJson(Files.readString(cfgFile, StandardCharsets.UTF_8),
                            InstanceConfig.class);
                    if (config != null && config.getId() != null) {
                        instanceConfigs.put(config.getId(), config);
                        log.info("Loaded instance '{}' ({} {}, internal port {})",
                                config.getName(), config.getMinecraftVersion(),
                                config.getModLoader() == null ? "?" : config.getModLoader().getType(),
                                config.getInternalMcPort());
                    }
                } catch (IOException | RuntimeException e) {
                    log.warn("Could not parse {}, skipping", cfgFile, e);
                }
            }
        }
        log.info("Loaded {} instance(s) from {}", instanceConfigs.size(), instancesDir);
        if (activeInstanceId == null) {
            activeInstanceId = pickDefaultActiveInstance();
            if (activeInstanceId != null) {
                log.info("Active instance for client sync: {}", activeInstanceId);
            }
        }
    }

    /**
     * @return the instance whose data the client-facing legacy endpoints serve,
     *         or {@code null} when the wrapper runs in pure legacy mode (no
     *         instances exist).
     */
    public InstanceConfig getActiveInstance() {
        if (activeInstanceId == null) {
            return null;
        }
        InstanceConfig cfg = instanceConfigs.get(activeInstanceId);
        return cfg == null ? null : cfg;
    }

    /** Deterministic fallback: the first remaining instance by id, or {@code null}. */
    private String pickDefaultActiveInstance() {
        return instanceConfigs.keySet().stream().sorted().findFirst().orElse(null);
    }

    private void saveInstanceToDisk(InstanceConfig config) {
        try {
            Path cfgFile = instanceDir(config.getId()).resolve("instance.json");
            Files.writeString(cfgFile, GSON.toJson(config), StandardCharsets.UTF_8);
        } catch (IOException e) {
            throw new IllegalStateException("Could not persist instance " + config.getId(), e);
        }
    }

    // ------------------------------------------------------------------
    // helpers
    // ------------------------------------------------------------------

    private Path instanceDir(String instanceId) {
        return instancesDir.resolve(instanceId);
    }

    /** Picks the next free internal port above {@link #MC_PORT_BASE}. */
    private int allocateNextPort() {
        int max = MC_PORT_BASE - 1;
        for (InstanceConfig cfg : instanceConfigs.values()) {
            if (cfg.getInternalMcPort() > max) {
                max = cfg.getInternalMcPort();
            }
        }
        return max + 1;
    }

    private static String normalizeName(String name) {
        return name == null ? "" : name.trim().toLowerCase(Locale.ROOT).replaceAll("[^a-z0-9]", "-");
    }

    /** Allows only safe JVM flag characters; everything else is stripped. */
    private String sanitizeJavaArgs(String javaArgs) {
        if (javaArgs == null || javaArgs.isBlank()) {
            return "-Xms2G -Xmx4G";
        }
        return javaArgs.replaceAll("[^\\w.\\-+ ]", "").trim();
    }
}
