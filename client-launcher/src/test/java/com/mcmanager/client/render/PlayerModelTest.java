package com.mcmanager.client.render;

import org.junit.jupiter.api.Test;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertTrue;

/**
 * Verifies the dual-layer (inner base + outer overlay) player geometry: vertex
 * count for 12 boxes, overlay inflation beyond the base layer, and UVs within
 * the 64x64 skin bounds.
 */
class PlayerModelTest {

    private static final int FLOATS_PER_VERTEX = 8; // pos(3) + normal(3) + uv(2)
    private static final int VERTICES_PER_BOX = 36;  // 6 faces * 2 triangles * 3 verts

    @Test
    void emitsTwelveBoxesForDualLayerModel() {
        float[] data = PlayerModel.buildInterleaved();
        // 12 boxes * 36 vertices * 8 floats = 3456.
        assertEquals(12 * VERTICES_PER_BOX * FLOATS_PER_VERTEX, data.length);
    }

    @Test
    void overlayLayerIsInflatedBeyondBaseLayer() {
        float[] data = PlayerModel.buildInterleaved();
        // Base head top sits at y=64 (center 56 + half-extent 8). The hat overlay
        // is inflated by 0.25 per axis, so the tallest emitted vertex is 64.25.
        float maxY = Float.NEGATIVE_INFINITY;
        for (int i = 0; i < data.length; i += FLOATS_PER_VERTEX) {
            maxY = Math.max(maxY, data[i + 1]);
        }
        assertEquals(64.25f, maxY, 0.001f);
    }

    @Test
    void uvsStayWithinSkinBounds() {
        float[] data = PlayerModel.buildInterleaved();
        for (int i = 0; i < data.length; i += FLOATS_PER_VERTEX) {
            float u = data[i + 6];
            float v = data[i + 7];
            assertTrue(u >= 0f && u <= 1f, "u out of bounds: " + u);
            assertTrue(v >= 0f && v <= 1f, "v out of bounds: " + v);
        }
    }
}
