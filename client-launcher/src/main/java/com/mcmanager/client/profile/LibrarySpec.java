package com.mcmanager.client.profile;

import com.google.gson.JsonArray;
import com.google.gson.JsonObject;

/**
 * One entry in a {@link VersionProfile}'s {@code libraries} array.
 *
 * <p>The {@code name} is a Maven coordinate with an optional classifier and
 * explicit extension:
 *
 * <pre>
 * group:artifact:version@jar                  -> net.neoforged:neoforge:20.4.250@jar
 * group:artifact:version:classifier@jar      -> net.neoforged:mergetool:2.0.0:api@jar
 * </pre>
 *
 * Most loader libraries carry a {@code downloads.artifact} object whose
 * {@code path} is the authoritative relative location under the libraries dir.
 */
public class LibrarySpec {

    private String name;

    /** Optional {@code rules} array restricting the library to certain OSes. */
    private JsonArray rules;

    /** Optional {@code downloads} object ({@code artifact.path/url} and {@code classifiers}). */
    private JsonObject downloads;

    public String getName() {
        return name;
    }

    public JsonArray getRules() {
        return rules;
    }

    public JsonObject getDownloads() {
        return downloads;
    }

    // ------------------------------------------------------------------
    // Artifact coordinate parsing
    // ------------------------------------------------------------------

    public record ArtifactCoordinates(String group, String artifact, String version,
                                      String classifier, String extension) {
    }

    /** Parses a Maven coordinate of the form {@code g:a:v[:c]@ext}. */
    public static ArtifactCoordinates parseCoordinates(String name) {
        if (name == null) {
            return null;
        }
        int at = name.indexOf('@');
        String extension = at >= 0 ? name.substring(at + 1) : "jar";
        String coords = at >= 0 ? name.substring(0, at) : name;

        String[] parts = coords.split(":");
        if (parts.length < 3) {
            return null;
        }
        String group = parts[0];
        String artifact = parts[1];
        String version = parts[2];
        String classifier = parts.length > 3 ? parts[3] : null;
        return new ArtifactCoordinates(group, artifact, version, classifier, extension);
    }

    /**
     * The relative path of this library under the libraries directory, e.g.
     * {@code net/neoforged/neoforge/20.4.250/neoforge-20.4.250-client.jar}.
     * Prefers the installer-provided {@code downloads.artifact.path} and falls
     * back to deriving the path from the Maven coordinate.
     */
    public String getArtifactPath() {
        if (downloads != null && downloads.has("artifact")
                && downloads.getAsJsonObject("artifact").has("path")) {
            return downloads.getAsJsonObject("artifact").get("path").getAsString();
        }
        ArtifactCoordinates coords = parseCoordinates(name);
        if (coords == null) {
            return null;
        }
        String groupPath = coords.group().replace('.', '/');
        String file = coords.artifact() + "-" + coords.version()
                + (coords.classifier() != null ? "-" + coords.classifier() : "")
                + "." + coords.extension();
        return groupPath + "/" + coords.artifact() + "/" + coords.version() + "/" + file;
    }

    /** The download URL for this library, or {@code null} when the profile has none. */
    public String getDownloadUrl() {
        if (downloads != null && downloads.has("artifact")
                && downloads.getAsJsonObject("artifact").has("url")) {
            return downloads.getAsJsonObject("artifact").get("url").getAsString();
        }
        return null;
    }

    @Override
    public String toString() {
        return "LibrarySpec{" + name + "}";
    }
}
