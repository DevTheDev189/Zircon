package com.mcmanager.core.model;

import java.util.Objects;

/**
 * Unified metadata extracted from a mod JAR's metadata file. Supports the three
 * formats the launcher must read:
 *
 * <ul>
 *   <li>{@code fabric.mod.json} (Fabric / Quilt)</li>
 *   <li>{@code META-INF/mods.toml} (Forge)</li>
 *   <li>{@code META-INF/neoforge.mods.toml} (NeoForge)</li>
 * </ul>
 *
 * @param id          stable mod id, e.g. {@code "sodium"}
 * @param name        human-readable display name
 * @param version     mod version string
 * @param description short description from the metadata file
 * @param loaderType  which loader's metadata format produced this entry
 * @param environment {@code "client"}, {@code "server"}, {@code "both"} or {@code "*"}
 */
public record ModMetadata(
        String id,
        String name,
        String version,
        String description,
        ModLoaderType loaderType,
        String environment) {

    public ModMetadata {
        Objects.requireNonNull(id, "id");
        Objects.requireNonNull(loaderType, "loaderType");
    }

    /** Normalizes an environment token ("*" or "both" → "both"). */
    public String normalizedEnvironment() {
        if (environment == null || environment.isBlank() || "*".equals(environment.trim())) {
            return "both";
        }
        return environment.trim();
    }
}
