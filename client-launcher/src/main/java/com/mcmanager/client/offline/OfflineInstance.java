package com.mcmanager.client.offline;

import com.mcmanager.core.model.ModLoaderInfo;

/**
 * A locally-managed Minecraft world/instance that can be launched without
 * connecting to a Zircon server. Each instance owns its own {@code mods/}
 * folder and persists its configuration to {@code instance.json}.
 *
 * <p>Instances live under {@code ~/.mcmanager/offline_instances/<id>/}.
 */
public class OfflineInstance {

    private String id;
    private String name;
    private String minecraftVersion = "1.20.4";
    private ModLoaderInfo modLoader = new ModLoaderInfo("fabric", "0.15.11", "");
    private String gameMode = "survival";
    private boolean allowCheats = false;
    private String javaArgs = "-Xms2G -Xmx4G";
    private long lastPlayed = System.currentTimeMillis();

    public OfflineInstance() {
    }

    public String getId() {
        return id;
    }

    public void setId(String id) {
        this.id = id;
    }

    public String getName() {
        return name;
    }

    public void setName(String name) {
        this.name = name;
    }

    public String getMinecraftVersion() {
        return minecraftVersion;
    }

    public void setMinecraftVersion(String minecraftVersion) {
        this.minecraftVersion = minecraftVersion;
    }

    /** @return the mod loader descriptor; never {@code null}. */
    public ModLoaderInfo getModLoader() {
        if (modLoader == null) {
            modLoader = new ModLoaderInfo("fabric", "0.15.11", "");
        }
        return modLoader;
    }

    public void setModLoader(ModLoaderInfo modLoader) {
        this.modLoader = modLoader;
    }

    public String getGameMode() {
        return gameMode;
    }

    public void setGameMode(String gameMode) {
        this.gameMode = gameMode;
    }

    public boolean isAllowCheats() {
        return allowCheats;
    }

    public void setAllowCheats(boolean allowCheats) {
        this.allowCheats = allowCheats;
    }

    public String getJavaArgs() {
        return javaArgs;
    }

    public void setJavaArgs(String javaArgs) {
        this.javaArgs = javaArgs;
    }

    public long getLastPlayed() {
        return lastPlayed;
    }

    public void setLastPlayed(long lastPlayed) {
        this.lastPlayed = lastPlayed;
    }

    @Override
    public String toString() {
        return "OfflineInstance{" + name + " (" + minecraftVersion + ", "
                + getModLoader().getType() + ")}";
    }
}
