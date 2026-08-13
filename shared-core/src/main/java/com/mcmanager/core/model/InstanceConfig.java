package com.mcmanager.core.model;

import com.google.gson.annotations.SerializedName;

import java.util.Set;
import java.util.UUID;

/**
 * Persistent metadata for one isolated Zircon server instance.
 *
 * <p>The {@link ModLoaderInfo} is <b>locked at creation time</b>: there is no
 * setter for it (and no API route that mutates it), so a server's mod loader
 * type can never be switched out from under the mods that were installed for
 * it. Only {@code name}, {@code javaArgs}, {@code autoStart},
 * {@code minecraftVersion}, the loader <em>version</em> (via
 * {@link #setLoaderVersion}) and the backup settings
 * ({@link #setBackupFrequency}/{@link #setBackupTime}/{@link #setBackupRetention})
 * are mutable after creation.
 */
public class InstanceConfig {

    /** Manual backups only — the scheduler never auto-backs up. */
    public static final String BACKUP_OFF = "off";
    public static final String BACKUP_DAILY = "daily";
    public static final String BACKUP_WEEKLY = "weekly";
    public static final String BACKUP_MONTHLY = "monthly";

    /** All frequency values accepted by the backup scheduler. */
    public static final Set<String> VALID_BACKUP_FREQUENCIES =
            Set.of(BACKUP_OFF, BACKUP_DAILY, BACKUP_WEEKLY, BACKUP_MONTHLY);

    /** Default number of backups kept per instance before old ones are pruned. */
    public static final int DEFAULT_BACKUP_RETENTION = 10;

    /** Allowed bounds for the per-instance retention setting. */
    public static final int MIN_BACKUP_RETENTION = 1;
    public static final int MAX_BACKUP_RETENTION = 100;

    @SerializedName("id")
    private String id = UUID.randomUUID().toString().substring(0, 8);

    @SerializedName("name")
    private String name = "New Zircon Server";

    @SerializedName("minecraftVersion")
    private String minecraftVersion;

    // IMMUTABLE after creation — no setter exposed to the API!
    @SerializedName("modLoader")
    private ModLoaderInfo modLoader;

    @SerializedName("internalMcPort")
    private int internalMcPort; // Automatically assigned, e.g. 25566, 25567

    /** Player-facing port where the multiplexer accepts connections for this instance (0 = unassigned). */
    @SerializedName("externalMcPort")
    private int externalMcPort;

    @SerializedName("javaArgs")
    private String javaArgs = "-Xms2G -Xmx4G";

    @SerializedName("autoStart")
    private boolean autoStart = false;

    /** Backup cadence: one of {@link #BACKUP_OFF}, {@link #BACKUP_DAILY}, {@link #BACKUP_WEEKLY}, {@link #BACKUP_MONTHLY}. */
    @SerializedName("backupFrequency")
    private String backupFrequency = BACKUP_OFF;

    /** Local time of day (24-hour "HH:MM") at which scheduled backups run. */
    @SerializedName("backupTime")
    private String backupTime = "02:00";

    /** How many backups to keep; older ones are pruned. */
    @SerializedName("backupRetention")
    private int backupRetention = DEFAULT_BACKUP_RETENTION;

    /** Gson deserialization. */
    public InstanceConfig() {
    }

    /**
     * Creates a new instance configuration. {@code loaderType} is one of
     * "vanilla", "fabric", "quilt", "forge", "neoforge"; the loader is frozen
     * in place from this moment on. The external (player-facing) port is left
     * unassigned (0) and allocated by the instance manager.
     */
    public InstanceConfig(String name, String minecraftVersion, String loaderType,
                          String loaderVersion, int internalMcPort) {
        this(name, minecraftVersion, loaderType, loaderVersion, internalMcPort, 0);
    }

    public InstanceConfig(String name, String minecraftVersion, String loaderType,
                          String loaderVersion, int internalMcPort, int externalMcPort) {
        this.name = name;
        this.minecraftVersion = minecraftVersion;
        this.modLoader = new ModLoaderInfo(loaderType, loaderVersion, "");
        this.internalMcPort = internalMcPort;
        this.externalMcPort = externalMcPort;
    }

    public String getId() {
        return id;
    }

    public String getName() {
        return name;
    }

    public String getMinecraftVersion() {
        return minecraftVersion;
    }

    public ModLoaderInfo getModLoader() {
        return modLoader;
    }

    public int getInternalMcPort() {
        return internalMcPort;
    }

    /** Only used by the instance manager to relocate legacy internal ports out of the player-facing range. */
    public void setInternalMcPort(int internalMcPort) {
        this.internalMcPort = internalMcPort;
    }

    public int getExternalMcPort() {
        return externalMcPort;
    }

    public void setExternalMcPort(int externalMcPort) {
        this.externalMcPort = externalMcPort;
    }

    public String getJavaArgs() {
        return javaArgs;
    }

    public boolean isAutoStart() {
        return autoStart;
    }

    public String getBackupFrequency() {
        return backupFrequency;
    }

    public String getBackupTime() {
        return backupTime;
    }

    public int getBackupRetention() {
        return backupRetention;
    }

    // ------------------------------------------------------------------
    // The only mutable fields. Note: NO setModLoader()!
    // ------------------------------------------------------------------

    public void setName(String name) {
        this.name = name;
    }

    public void setJavaArgs(String javaArgs) {
        this.javaArgs = javaArgs;
    }

    public void setAutoStart(boolean autoStart) {
        this.autoStart = autoStart;
    }

    public void setBackupFrequency(String backupFrequency) {
        this.backupFrequency = backupFrequency;
    }

    public void setBackupTime(String backupTime) {
        this.backupTime = backupTime;
    }

    public void setBackupRetention(int backupRetention) {
        this.backupRetention = backupRetention;
    }

    public void setMinecraftVersion(String minecraftVersion) {
        this.minecraftVersion = minecraftVersion;
    }

    /**
     * Updates the mod loader <em>version</em> string (e.g. Fabric {@code 0.15.11}).
     * The loader <em>type</em> stays locked — this only ever touches the version
     * inside the existing {@link ModLoaderInfo}.
     */
    public void setLoaderVersion(String loaderVersion) {
        if (this.modLoader == null) {
            this.modLoader = new ModLoaderInfo("vanilla", loaderVersion, "");
        } else {
            this.modLoader.setVersion(loaderVersion);
        }
    }

    @Override
    public String toString() {
        return "InstanceConfig{id=" + id + ", name=" + name
                + ", mc=" + minecraftVersion + ", loader=" + modLoader
                + ", port=" + internalMcPort + "}";
    }
}
