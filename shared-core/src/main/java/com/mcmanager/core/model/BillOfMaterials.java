package com.mcmanager.core.model;

import com.google.gson.annotations.SerializedName;

import java.util.ArrayList;
import java.util.List;
import java.util.Objects;

/**
 * The "Bill of Materials" (BOM) that the server manager publishes and the client
 * launcher consumes. It pins the exact Minecraft version, mod loader and the
 * authoritative list of mods a client must install to join the server.
 */
public class BillOfMaterials {

    /** Current JSON schema version. Bump when breaking field changes are made. */
    public static final int CURRENT_SCHEMA_VERSION = 1;

    @SerializedName("schemaVersion")
    private int schemaVersion = CURRENT_SCHEMA_VERSION;

    @SerializedName("minecraftVersion")
    private String minecraftVersion;

    @SerializedName("modLoader")
    private ModLoaderInfo modLoader;

    @SerializedName("mods")
    private List<ModEntry> mods = new ArrayList<>();

    @SerializedName("serverTitle")
    private String serverTitle;

    public BillOfMaterials() {
    }

    public BillOfMaterials(String minecraftVersion, ModLoaderInfo modLoader, String serverTitle) {
        this.minecraftVersion = minecraftVersion;
        this.modLoader = modLoader;
        this.serverTitle = serverTitle;
    }

    public int getSchemaVersion() {
        return schemaVersion;
    }

    public void setSchemaVersion(int schemaVersion) {
        this.schemaVersion = schemaVersion;
    }

    public String getMinecraftVersion() {
        return minecraftVersion;
    }

    public void setMinecraftVersion(String minecraftVersion) {
        this.minecraftVersion = minecraftVersion;
    }

    public ModLoaderInfo getModLoader() {
        return modLoader;
    }

    public void setModLoader(ModLoaderInfo modLoader) {
        this.modLoader = modLoader;
    }

    public List<ModEntry> getMods() {
        return mods;
    }

    public void setMods(List<ModEntry> mods) {
        this.mods = mods != null ? mods : new ArrayList<>();
    }

    public String getServerTitle() {
        return serverTitle;
    }

    public void setServerTitle(String serverTitle) {
        this.serverTitle = serverTitle;
    }

    // ------------------------------------------------------------------
    // Convenience helpers
    // ------------------------------------------------------------------

    public void addMod(ModEntry entry) {
        if (mods == null) {
            mods = new ArrayList<>();
        }
        mods.add(entry);
    }

    public boolean removeMod(String filename) {
        return mods != null && mods.removeIf(m -> Objects.equals(m.getFilename(), filename));
    }

    /** @return the mod with the given file name, or {@code null}. */
    public ModEntry getModByFilename(String filename) {
        if (mods == null) {
            return null;
        }
        for (ModEntry mod : mods) {
            if (Objects.equals(mod.getFilename(), filename)) {
                return mod;
            }
        }
        return null;
    }

    /** @return the mod with the given id, or {@code null}. */
    public ModEntry getModById(String id) {
        if (mods == null) {
            return null;
        }
        for (ModEntry mod : mods) {
            if (Objects.equals(mod.getId(), id)) {
                return mod;
            }
        }
        return null;
    }

    public List<ModEntry> getModsByOrigin(String origin) {
        List<ModEntry> result = new ArrayList<>();
        if (mods != null) {
            for (ModEntry mod : mods) {
                if (Objects.equals(mod.getOrigin(), origin)) {
                    result.add(mod);
                }
            }
        }
        return result;
    }

    /** Total size of all mods in bytes. */
    public long totalSizeBytes() {
        long total = 0;
        if (mods != null) {
            for (ModEntry mod : mods) {
                total += mod.getFileSize();
            }
        }
        return total;
    }
}
