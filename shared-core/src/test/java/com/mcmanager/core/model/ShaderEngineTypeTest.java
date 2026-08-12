package com.mcmanager.core.model;

import org.junit.jupiter.api.Test;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertTrue;

class ShaderEngineTypeTest {

    @Test
    void fabricAndQuiltUseIris() {
        assertEquals(ShaderEngineType.IRIS, ShaderEngineType.forLoader("fabric"));
        assertEquals(ShaderEngineType.IRIS, ShaderEngineType.forLoader("quilt"));
    }

    @Test
    void forgeAndNeoforgeUseOculus() {
        assertEquals(ShaderEngineType.OCULUS, ShaderEngineType.forLoader("forge"));
        assertEquals(ShaderEngineType.OCULUS, ShaderEngineType.forLoader("neoforge"));
    }

    @Test
    void caseInsensitiveAndNullDefaultsToIris() {
        assertEquals(ShaderEngineType.OCULUS, ShaderEngineType.forLoader("NeoForge"));
        assertEquals(ShaderEngineType.IRIS, ShaderEngineType.forLoader(null));
        assertEquals(ShaderEngineType.IRIS, ShaderEngineType.forLoader("vanilla"));
    }

    @Test
    void shaderModProjectIdsCoversBothEngines() {
        assertTrue(ShaderEngineType.SHADER_MOD_PROJECT_IDS.contains(ShaderEngineType.IRIS.getPrimaryProjectId()));
        assertTrue(ShaderEngineType.SHADER_MOD_PROJECT_IDS.contains(ShaderEngineType.IRIS.getDependencyProjectId()));
        assertTrue(ShaderEngineType.SHADER_MOD_PROJECT_IDS.contains(ShaderEngineType.OCULUS.getPrimaryProjectId()));
        assertTrue(ShaderEngineType.SHADER_MOD_PROJECT_IDS.contains(ShaderEngineType.OCULUS.getDependencyProjectId()));
    }
}
