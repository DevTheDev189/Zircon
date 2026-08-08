package com.mcmanager.client.profile;

import com.google.gson.JsonObject;
import com.google.gson.annotations.SerializedName;

import java.util.List;

/**
 * A Minecraft version profile JSON (as written by the Forge/NeoForge installers
 * into {@code versions/<id>/<id>.json}). Models only the fields the launcher
 * needs to build a launch command:
 *
 * <pre>
 * {
 *   "id": "neoforge-20.4.250",
 *   "mainClass": "cpw.mods.bootstraplauncher.BootstrapLauncher",
 *   "inheritsFrom": "1.20.4",
 *   "arguments": { "game": [...], "jvm": [...] },
 *   "libraries": [ { "name": "group:artifact:version@jar", "downloads": {...} } ]
 * }
 * </pre>
 *
 * Unknown fields are ignored, so vanilla and loader profiles parse with the
 * same model.
 */
public class VersionProfile {

    @SerializedName("id")
    private String id;

    @SerializedName("mainClass")
    private String mainClass;

    @SerializedName("inheritsFrom")
    private String inheritsFrom;

    @SerializedName("libraries")
    private List<LibrarySpec> libraries;

    @SerializedName("arguments")
    private JsonObject arguments;

    /** Legacy {@code minecraftArguments} (string) profiles, pre-1.13. Kept for robustness. */
    @SerializedName("minecraftArguments")
    private String minecraftArguments;

    public String getId() {
        return id;
    }

    public String getMainClass() {
        return mainClass;
    }

    /** The id of the parent profile (usually the vanilla Minecraft version), or {@code null}. */
    public String getInheritsFrom() {
        return inheritsFrom;
    }

    public List<LibrarySpec> getLibraries() {
        return libraries;
    }

    public JsonObject getArguments() {
        return arguments;
    }

    public String getMinecraftArguments() {
        return minecraftArguments;
    }

    @Override
    public String toString() {
        return "VersionProfile{id='" + id + "', inheritsFrom='" + inheritsFrom + "'}";
    }
}
