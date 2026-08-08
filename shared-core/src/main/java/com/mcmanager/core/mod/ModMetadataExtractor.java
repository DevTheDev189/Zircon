package com.mcmanager.core.mod;

import com.google.gson.JsonElement;
import com.google.gson.JsonObject;
import com.google.gson.JsonParser;
import com.mcmanager.core.model.ModLoaderType;
import com.mcmanager.core.model.ModMetadata;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;
import org.tomlj.Toml;
import org.tomlj.TomlArray;
import org.tomlj.TomlParseResult;
import org.tomlj.TomlTable;

import java.io.File;
import java.io.IOException;
import java.io.InputStream;
import java.io.InputStreamReader;
import java.nio.charset.StandardCharsets;
import java.util.zip.ZipEntry;
import java.util.zip.ZipFile;

/**
 * Reads the authoritative metadata out of a mod JAR by inspecting its embedded
 * metadata file:
 *
 * <ol>
 *   <li>{@code fabric.mod.json} — Fabric / Quilt</li>
 *   <li>{@code META-INF/neoforge.mods.toml} — NeoForge</li>
 *   <li>{@code META-INF/mods.toml} — Forge</li>
 * </ol>
 *
 * The first matching file wins (a jar can ship multiple metadata files for
 * cross-loader compatibility; NeoForge takes precedence over Forge when both
 * TOML files exist).
 */
public class ModMetadataExtractor {

    private static final Logger log = LoggerFactory.getLogger(ModMetadataExtractor.class);

    public static final String FABRIC_ENTRY = "fabric.mod.json";
    public static final String NEOFORGE_ENTRY = "META-INF/neoforge.mods.toml";
    public static final String FORGE_ENTRY = "META-INF/mods.toml";

    /**
     * Extracts metadata from the given mod jar.
     *
     * @throws IllegalArgumentException when the jar carries no recognized metadata file.
     * @throws IOException              when the jar cannot be opened/read.
     */
    public ModMetadata extract(File jarFile) throws IOException {
        try (ZipFile zip = new ZipFile(jarFile)) {

            ZipEntry fabricEntry = zip.getEntry(FABRIC_ENTRY);
            if (fabricEntry != null) {
                try (InputStream is = zip.getInputStream(fabricEntry)) {
                    return parseFabricMetadata(is);
                }
            }

            ZipEntry neoForgeEntry = zip.getEntry(NEOFORGE_ENTRY);
            if (neoForgeEntry != null) {
                try (InputStream is = zip.getInputStream(neoForgeEntry)) {
                    return parseTomlMetadata(is, ModLoaderType.NEOFORGE);
                }
            }

            ZipEntry forgeEntry = zip.getEntry(FORGE_ENTRY);
            if (forgeEntry != null) {
                try (InputStream is = zip.getInputStream(forgeEntry)) {
                    return parseTomlMetadata(is, ModLoaderType.FORGE);
                }
            }
        }
        throw new IllegalArgumentException("Unknown or unparseable mod jar: " + jarFile.getName());
    }

    // ------------------------------------------------------------------
    // fabric.mod.json
    // ------------------------------------------------------------------

    private ModMetadata parseFabricMetadata(InputStream is) throws IOException {
        JsonObject root;
        try (InputStream in = is) {
            root = JsonParser.parseReader(new InputStreamReader(in, StandardCharsets.UTF_8))
                    .getAsJsonObject();
        } catch (IllegalStateException e) {
            throw new IllegalArgumentException("Invalid fabric.mod.json: " + e.getMessage(), e);
        }

        String id = text(root, "id");
        if (id == null || id.isBlank()) {
            throw new IllegalArgumentException("fabric.mod.json is missing required field 'id'");
        }
        String name = text(root, "name");
        String version = text(root, "version");
        String description = text(root, "description");

        return new ModMetadata(
                id,
                name != null && !name.isBlank() ? name : id,
                version != null ? version : "0.0.0",
                description != null ? description : "",
                ModLoaderType.FABRIC,
                environment(root)
        );
    }

    /**
     * {@code environment} is a string ("*", "client", "server") in the current
     * schema, but very old jars used an object like {@code {"client": "*"}}.
     */
    private String environment(JsonObject root) {
        JsonElement env = root.get("environment");
        if (env == null) {
            return "*";
        }
        if (env.isJsonObject()) {
            JsonObject envObj = env.getAsJsonObject();
            if (envObj.has("client") && envObj.has("server")) {
                return "both";
            }
            return envObj.has("client") ? "client" : envObj.has("server") ? "server" : "*";
        }
        return env.isJsonPrimitive() && env.getAsJsonPrimitive().isString()
                ? env.getAsString() : "*";
    }

    // ------------------------------------------------------------------
    // META-INF/mods.toml and META-INF/neoforge.mods.toml
    // ------------------------------------------------------------------

    private ModMetadata parseTomlMetadata(InputStream is, ModLoaderType loaderType) throws IOException {
        TomlParseResult result = Toml.parse(is);
        if (result.hasErrors()) {
            throw new IllegalArgumentException("Invalid TOML metadata: " + result.errors());
        }

        TomlArray mods = result.getArray("mods");
        if (mods == null || mods.isEmpty()) {
            throw new IllegalArgumentException("Missing [[mods]] section in " + tomlEntryName(loaderType));
        }

        TomlTable mod = mods.getTable(0);
        if (mod == null) {
            throw new IllegalArgumentException("Empty [[mods]] entry in " + tomlEntryName(loaderType));
        }

        String id = mod.getString("modId");
        if (id == null || id.isBlank()) {
            throw new IllegalArgumentException("Missing 'modId' in [[mods]] entry of " + tomlEntryName(loaderType));
        }
        String name = mod.getString("displayName");
        String version = tomlStringOrNumber(mod, "version");
        String description = mod.getString("description");

        return new ModMetadata(
                id,
                name != null && !name.isBlank() ? name : id,
                version != null ? version : "0.0.0",
                description != null ? description : "",
                loaderType,
                "both"
        );
    }

    private static String tomlEntryName(ModLoaderType loaderType) {
        return loaderType == ModLoaderType.NEOFORGE ? NEOFORGE_ENTRY : FORGE_ENTRY;
    }

    /** TOML is weakly typed: version is usually a string but occasionally a number. */
    private static String tomlStringOrNumber(TomlTable table, String key) {
        String s = table.getString(key);
        if (s != null) {
            return s;
        }
        Long l = table.getLong(key);
        return l != null ? l.toString() : null;
    }

    private static String text(JsonObject obj, String key) {
        JsonElement el = obj.get(key);
        if (el == null || !el.isJsonPrimitive() || !el.getAsJsonPrimitive().isString()) {
            return null;
        }
        return el.getAsString();
    }
}
