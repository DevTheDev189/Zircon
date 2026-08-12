package com.mcmanager.core.model;

import java.util.List;
import java.util.Set;

/**
 * Maps a mod loader type to the shader engine mod (and its rendering-engine
 * dependency) that must be installed for shaders to work under that loader.
 */
public enum ShaderEngineType {
    IRIS("Iris Shaders", "iris", "sodium", List.of("fabric", "quilt")),
    OCULUS("Oculus Shaders", "oculus", "embeddium", List.of("forge", "neoforge"));

    /** Every Modrinth project id involved in either shader engine, for filtering the generic mods list. */
    public static final Set<String> SHADER_MOD_PROJECT_IDS =
            Set.of("iris", "sodium", "oculus", "embeddium", "rubidium");

    private final String displayName;
    private final String primaryProjectId;
    private final String dependencyProjectId;
    private final List<String> supportedLoaders;

    ShaderEngineType(String displayName, String primaryProjectId, String dependencyProjectId,
                     List<String> supportedLoaders) {
        this.displayName = displayName;
        this.primaryProjectId = primaryProjectId;
        this.dependencyProjectId = dependencyProjectId;
        this.supportedLoaders = supportedLoaders;
    }

    /** @return the engine appropriate for {@code loaderType} (Forge/NeoForge -> Oculus, else Iris). */
    public static ShaderEngineType forLoader(String loaderType) {
        if (loaderType == null) {
            return IRIS;
        }
        String normalized = loaderType.toLowerCase().trim();
        return (normalized.equals("forge") || normalized.equals("neoforge")) ? OCULUS : IRIS;
    }

    public String getDisplayName() {
        return displayName;
    }

    public String getPrimaryProjectId() {
        return primaryProjectId;
    }

    public String getDependencyProjectId() {
        return dependencyProjectId;
    }

    public List<String> getSupportedLoaders() {
        return supportedLoaders;
    }
}
