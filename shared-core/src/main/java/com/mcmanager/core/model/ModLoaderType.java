package com.mcmanager.core.model;

/**
 * The supported mod loaders. Values correspond to the {@code type} field used in
 * {@link ModLoaderInfo} / the published BOM (e.g. {@code "fabric"},
 * {@code "neoforge"}).
 */
public enum ModLoaderType {

    FABRIC("fabric"),
    QUILT("quilt"),
    FORGE("forge"),
    NEOFORGE("neoforge");

    private final String id;

    ModLoaderType(String id) {
        this.id = id;
    }

    public String getId() {
        return id;
    }

    /** @return {@code true} for loaders that are launched through the Forge/NeoForge version-profile pipeline. */
    public boolean isForgeLike() {
        return this == FORGE || this == NEOFORGE;
    }

    /**
     * Case-insensitive lookup by the BOM id.
     *
     * @throws IllegalArgumentException when the id is not a known loader.
     */
    public static ModLoaderType fromString(String text) {
        if (text == null || text.isBlank()) {
            throw new IllegalArgumentException("Mod loader type is null or blank");
        }
        for (ModLoaderType type : values()) {
            if (type.id.equalsIgnoreCase(text.trim())) {
                return type;
            }
        }
        throw new IllegalArgumentException("Unknown mod loader type: " + text);
    }

    /**
     * Lenient lookup: returns {@code fallback} (usually {@code null}) instead of
     * throwing, so callers that accept unconfigured / vanilla installs don't have
     * to wrap every lookup in a try/catch.
     */
    public static ModLoaderType fromString(String text, ModLoaderType fallback) {
        try {
            return fromString(text);
        } catch (IllegalArgumentException e) {
            return fallback;
        }
    }
}
