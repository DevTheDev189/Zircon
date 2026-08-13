package com.mcmanager.client.render;

/**
 * The static Minecraft player model geometry: twelve axis-aligned boxes — the
 * six inner body parts (head, torso, arms, legs) plus the six outer overlay
 * parts (hat, jacket, sleeves, pants) of a modern 64x64 dual-layer skin — with
 * the standard 64x64 skin UV rectangles. Vertices are emitted in an interleaved
 * {@code pos(3), normal(3), uv(2)} layout, with counter-clockwise (outward)
 * winding and explicit per-face normals.
 *
 * <p>Outer overlay boxes are inflated by {@value #OVERLAY_INFLATE} units per
 * side so they sit just outside the base layer and avoid Z-fighting; transparent
 * overlay pixels are discarded by the fragment shader. The base layer is emitted
 * first so blending with depth testing composites the overlay on top.
 *
 * <p>The model uses a conventional OpenGL coordinate system: +Y up, +Z toward
 * the camera, feet at Y = 0. All dimensions are in block units scaled by 2.
 */
public final class PlayerModel {

    private static final float SKIN = 64f;
    private static final int FLOATS_PER_VERTEX = 8;
    private static final int VERTICES_PER_BOX = 36; // 6 faces * 2 triangles * 3 verts
    private static final int PARTS = 12; // 6 inner body parts + 6 outer overlay parts

    /** Half-extent growth per axis for the outer overlay boxes. */
    private static final float OVERLAY_INFLATE = 0.25f;

    private PlayerModel() {
    }

    /**
     * @return interleaved vertex data for the base layer (six body parts) followed
     *         by the inflated outer overlay layer (six parts). Vertex order is
     *         deliberate: the renderer draws base-then-overlay so depth-tested
     *         alpha blending composites the overlay over the base correctly.
     */
    public static float[] buildInterleaved() {
        float[] out = new float[PARTS * VERTICES_PER_BOX * FLOATS_PER_VERTEX];
        int[] cursor = {0};

        // half extents, center, uv rects
        addBox(out, cursor, 8, 8, 8, 0, 56, 0, HEAD_UVS);
        addBox(out, cursor, 8, 12, 4, 0, 36, 0, TORSO_UVS);
        addBox(out, cursor, 4, 12, 4, 12, 36, 0, LEFT_ARM_UVS);
        addBox(out, cursor, 4, 12, 4, -12, 36, 0, RIGHT_ARM_UVS);
        addBox(out, cursor, 4, 12, 4, 4, 12, 0, LEFT_LEG_UVS);
        addBox(out, cursor, 4, 12, 4, -4, 12, 0, RIGHT_LEG_UVS);

        // Outer overlay layer: same centers, half-extents inflated per axis.
        addBox(out, cursor, 8 + OVERLAY_INFLATE, 8 + OVERLAY_INFLATE, 8 + OVERLAY_INFLATE,
                0, 56, 0, HAT_UVS);
        addBox(out, cursor, 8 + OVERLAY_INFLATE, 12 + OVERLAY_INFLATE, 4 + OVERLAY_INFLATE,
                0, 36, 0, JACKET_UVS);
        addBox(out, cursor, 4 + OVERLAY_INFLATE, 12 + OVERLAY_INFLATE, 4 + OVERLAY_INFLATE,
                12, 36, 0, LEFT_SLEEVE_UVS);
        addBox(out, cursor, 4 + OVERLAY_INFLATE, 12 + OVERLAY_INFLATE, 4 + OVERLAY_INFLATE,
                -12, 36, 0, RIGHT_SLEEVE_UVS);
        addBox(out, cursor, 4 + OVERLAY_INFLATE, 12 + OVERLAY_INFLATE, 4 + OVERLAY_INFLATE,
                4, 12, 0, LEFT_PANTS_UVS);
        addBox(out, cursor, 4 + OVERLAY_INFLATE, 12 + OVERLAY_INFLATE, 4 + OVERLAY_INFLATE,
                -4, 12, 0, RIGHT_PANTS_UVS);

        return out;
    }

    private static void addBox(float[] out, int[] cursor, float hx, float hy, float hz,
                               float cx, float cy, float cz, float[][] uvs) {
        float[][] corners = {
                {-hx, -hy, -hz}, // 0 back-bottom-left
                {hx, -hy, -hz},  // 1 back-bottom-right
                {hx, hy, -hz},   // 2 back-top-right
                {-hx, hy, -hz},  // 3 back-top-left
                {-hx, -hy, hz},  // 4 front-bottom-left
                {hx, -hy, hz},   // 5 front-bottom-right
                {hx, hy, hz},    // 6 front-top-right
                {-hx, hy, hz}    // 7 front-top-left
        };

        for (int face = 0; face < 6; face++) {
            float[] r = uvs[face];
            float u0 = r[0] / SKIN;
            float v0 = r[1] / SKIN;
            float u1 = r[2] / SKIN;
            float v1 = r[3] / SKIN;
            // selector -> uv: 0 TL, 1 TR, 2 BR, 3 BL
            float[][] uv = {{u0, v0}, {u1, v0}, {u1, v1}, {u0, v1}};

            int[] cornerIdx = FACE_CORNERS[face];
            int[] selectors = FACE_UV_SELECTORS[face];
            float[] normal = FACE_NORMALS[face];

            // Two triangles from the four corners: (0,1,2) and (0,2,3).
            for (int t : new int[]{0, 1, 2, 0, 2, 3}) {
                float[] pos = corners[cornerIdx[t]];
                float[] tex = uv[selectors[t]];
                emit(out, cursor, pos[0] + cx, pos[1] + cy, pos[2] + cz,
                        normal[0], normal[1], normal[2], tex[0], tex[1]);
            }
        }
    }

    private static void emit(float[] out, int[] cursor, float px, float py, float pz,
                             float nx, float ny, float nz, float u, float v) {
        int i = cursor[0];
        out[i] = px;
        out[i + 1] = py;
        out[i + 2] = pz;
        out[i + 3] = nx;
        out[i + 4] = ny;
        out[i + 5] = nz;
        out[i + 6] = u;
        out[i + 7] = v;
        cursor[0] = i + FLOATS_PER_VERTEX;
    }

    // Face order: front, back, top, bottom, left, right.
    private static final int[][] FACE_CORNERS = {
            {7, 4, 5, 6}, // front
            {0, 3, 2, 1}, // back
            {3, 7, 6, 2}, // top
            {0, 1, 5, 4}, // bottom
            {0, 4, 7, 3}, // left
            {1, 2, 6, 5}  // right
    };

    private static final float[][] FACE_NORMALS = {
            {0, 0, 1},  // front
            {0, 0, -1}, // back
            {0, 1, 0},  // top
            {0, -1, 0}, // bottom
            {-1, 0, 0}, // left
            {1, 0, 0}   // right
    };

    private static final int[][] FACE_UV_SELECTORS = {
            {0, 3, 2, 1}, // front  TL,BL,BR,TR
            {2, 1, 0, 3}, // back   BR,TR,TL,BL
            {0, 3, 2, 1}, // top    TL,BL,BR,TR
            {0, 1, 2, 3}, // bottom TL,TR,BR,BL
            {2, 3, 0, 1}, // left   BR,BL,TL,TR
            {3, 0, 1, 2}  // right  BL,TL,TR,BR
    };

    private static float[] r(float x, float y, float w, float h) {
        return new float[]{x, y, x + w, y + h};
    }

    // UV rectangles in order: front, back, top, bottom, left, right.
    private static final float[][] HEAD_UVS = {
            r(8, 8, 8, 8), r(24, 8, 8, 8), r(8, 0, 8, 8), r(16, 0, 8, 8), r(16, 8, 8, 8), r(0, 8, 8, 8)
    };

    private static final float[][] TORSO_UVS = {
            r(20, 20, 8, 12), r(32, 20, 8, 12), r(20, 16, 8, 4), r(28, 16, 8, 4), r(28, 20, 4, 12), r(16, 20, 4, 12)
    };

    private static final float[][] LEFT_ARM_UVS = {
            r(36, 52, 4, 12), r(44, 52, 4, 12), r(36, 48, 4, 4), r(40, 48, 4, 4), r(40, 52, 4, 12), r(32, 52, 4, 12)
    };

    private static final float[][] RIGHT_ARM_UVS = {
            r(44, 20, 4, 12), r(52, 20, 4, 12), r(44, 16, 4, 4), r(48, 16, 4, 4), r(48, 20, 4, 12), r(40, 20, 4, 12)
    };

    private static final float[][] LEFT_LEG_UVS = {
            r(20, 52, 4, 12), r(28, 52, 4, 12), r(20, 48, 4, 4), r(24, 48, 4, 4), r(24, 52, 4, 12), r(16, 52, 4, 12)
    };

    private static final float[][] RIGHT_LEG_UVS = {
            r(4, 20, 4, 12), r(12, 20, 4, 12), r(4, 16, 4, 4), r(8, 16, 4, 4), r(8, 20, 4, 12), r(0, 20, 4, 12)
    };

    // Outer overlay UV rectangles (64x64 layout, same face order as above).
    // Each overlay region sits exactly 16px below/right of its base counterpart.
    private static final float[][] HAT_UVS = {
            r(40, 8, 8, 8), r(56, 8, 8, 8), r(40, 0, 8, 8), r(48, 0, 8, 8), r(48, 8, 8, 8), r(32, 8, 8, 8)
    };

    private static final float[][] JACKET_UVS = {
            r(20, 36, 8, 12), r(32, 36, 8, 12), r(20, 32, 8, 4), r(28, 32, 8, 4), r(28, 36, 4, 12), r(16, 36, 4, 12)
    };

    private static final float[][] RIGHT_SLEEVE_UVS = {
            r(44, 36, 4, 12), r(52, 36, 4, 12), r(44, 32, 4, 4), r(48, 32, 4, 4), r(48, 36, 4, 12), r(40, 36, 4, 12)
    };

    private static final float[][] LEFT_SLEEVE_UVS = {
            r(52, 52, 4, 12), r(60, 52, 4, 12), r(52, 48, 4, 4), r(56, 48, 4, 4), r(56, 52, 4, 12), r(48, 52, 4, 12)
    };

    private static final float[][] RIGHT_PANTS_UVS = {
            r(4, 36, 4, 12), r(12, 36, 4, 12), r(4, 32, 4, 4), r(8, 32, 4, 4), r(8, 36, 4, 12), r(0, 36, 4, 12)
    };

    private static final float[][] LEFT_PANTS_UVS = {
            r(4, 52, 4, 12), r(12, 52, 4, 12), r(4, 48, 4, 4), r(8, 48, 4, 4), r(8, 52, 4, 12), r(0, 52, 4, 12)
    };
}
