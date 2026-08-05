package com.mcmanager.core.model;

import com.google.gson.annotations.SerializedName;

/**
 * Describes the mod loader (Fabric, NeoForge, Forge, Quilt) used by the server,
 * including the exact loader version and where the loader installer JAR can be fetched.
 */
public class ModLoaderInfo {

    /** One of: "fabric", "neoforge", "forge", "quilt". */
    @SerializedName("type")
    private String type;

    /** Loader version string, e.g. "0.15.11" for Fabric. */
    @SerializedName("version")
    private String version;

    /** URL of the loader installer JAR. */
    @SerializedName("loaderJarUrl")
    private String loaderJarUrl;

    public ModLoaderInfo() {
    }

    public ModLoaderInfo(String type, String version, String loaderJarUrl) {
        this.type = type;
        this.version = version;
        this.loaderJarUrl = loaderJarUrl;
    }

    public String getType() {
        return type;
    }

    public void setType(String type) {
        this.type = type;
    }

    public String getVersion() {
        return version;
    }

    public void setVersion(String version) {
        this.version = version;
    }

    public String getLoaderJarUrl() {
        return loaderJarUrl;
    }

    public void setLoaderJarUrl(String loaderJarUrl) {
        this.loaderJarUrl = loaderJarUrl;
    }

    @Override
    public String toString() {
        return "ModLoaderInfo{" + type + " " + version + "}";
    }
}
