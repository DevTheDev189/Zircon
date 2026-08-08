package com.mcmanager.core.model;

import com.google.gson.annotations.SerializedName;

import java.util.UUID;

/**
 * Persistent metadata for one isolated Zircon server instance.
 *
 * <p>The {@link ModLoaderInfo} is <b>locked at creation time</b>: there is no
 * setter for it (and no API route that mutates it), so a server's mod loader
 * type can never be switched out from under the mods that were installed for
 * it. Only {@code name}, {@code javaArgs}, {@code autoStart},
 * {@code minecraftVersion} and the loader <em>version</em> (via
 * {@link #setLoaderVersion}) are mutable after creation.
 */
public class InstanceConfig {

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

    @SerializedName("javaArgs")
    private String javaArgs = "-Xms2G -Xmx4G";

    @SerializedName("autoStart")
    private boolean autoStart = false;

    /** Gson deserialization. */
    public InstanceConfig() {
    }

    /**
     * Creates a new instance configuration. {@code loaderType} is one of
     * "vanilla", "fabric", "quilt", "forge", "neoforge"; the loader is frozen
     * in place from this moment on.
     */
    public InstanceConfig(String name, String minecraftVersion, String loaderType,
                          String loaderVersion, int internalMcPort) {
        this.name = name;
        this.minecraftVersion = minecraftVersion;
        this.modLoader = new ModLoaderInfo(loaderType, loaderVersion, "");
        this.internalMcPort = internalMcPort;
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

    public String getJavaArgs() {
        return javaArgs;
    }

    public boolean isAutoStart() {
        return autoStart;
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
