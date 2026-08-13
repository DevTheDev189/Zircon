package com.mcmanager.client.render;

import org.joml.Matrix4f;
import org.joml.Vector3f;
import org.lwjgl.BufferUtils;
import org.lwjgl.opengl.GL11;

import java.nio.ByteBuffer;
import java.util.concurrent.atomic.AtomicReference;

/**
 * Renders the textured Minecraft player model with a single directional light +
 * ambient. This is a pure GL implementation (no JavaFX) so it can be unit-tested
 * against a hidden context and reused by whatever UI shell drives it.
 */
public final class PlayerRenderer implements Drawable {

    private static final float FOV_Y = (float) Math.toRadians(40.0);
    private static final float NEAR = 0.1f;
    private static final float FAR = 1000f;

    private static final Vector3f LIGHT_DIR = new Vector3f(0.4f, 0.8f, 0.5f).normalize();
    private static final Vector3f LIGHT_COLOR = new Vector3f(0.7f, 0.7f, 0.7f);
    private static final Vector3f AMBIENT = new Vector3f(0.32f, 0.32f, 0.32f);

    private static final String VERTEX_SRC = """
            #version 330 core
            layout(location = 0) in vec3 aPos;
            layout(location = 1) in vec3 aNormal;
            layout(location = 2) in vec2 aUV;

            uniform mat4 uModel;
            uniform mat4 uView;
            uniform mat4 uProj;

            out vec3 vNormal;
            out vec2 vUV;

            void main() {
                vNormal = mat3(uModel) * aNormal;
                vUV = aUV;
                gl_Position = uProj * uView * uModel * vec4(aPos, 1.0);
            }
            """;

    private static final String FRAGMENT_SRC = """
            #version 330 core
            in vec3 vNormal;
            in vec2 vUV;

            uniform sampler2D uTexture;
            uniform vec3 uLightDir;
            uniform vec3 uLightColor;
            uniform vec3 uAmbient;

            out vec4 fragColor;

            void main() {
                vec4 texColor = texture(uTexture, vUV);
                if (texColor.a < 0.1) {
                    discard; // Fully transparent overlay pixels (hat/jacket/etc.)
                }
                vec3 n = normalize(vNormal);
                float diffuse = max(dot(n, uLightDir), 0.0);
                // texColor is premultiplied; keep the premultiplied alpha so the
                // GL_ONE / GL_ONE_MINUS_SRC_ALPHA blend composites correctly.
                fragColor = vec4(texColor.rgb * (uAmbient + uLightColor * diffuse), texColor.a);
            }
            """;

    private final AtomicReference<SkinData> pendingSkin = new AtomicReference<>();

    private final Matrix4f model = new Matrix4f();
    private final Matrix4f view = new Matrix4f();
    private final Matrix4f proj = new Matrix4f();

    private GlShader shader;
    private GlMesh mesh;
    private GlTexture texture;
    private boolean initialized;

    private int locModel;
    private int locView;
    private int locProj;
    private int locTexture;
    private int locLightDir;
    private int locLightColor;
    private int locAmbient;

    private volatile float rotationRadians;
    private volatile float pitchRadians;
    private float lastAspect;

    @Override
    public void init() {
        shader = new GlShader(VERTEX_SRC, FRAGMENT_SRC);
        mesh = GlMesh.create(PlayerModel.buildInterleaved());
        texture = GlTexture.create(1, 1, whitePixel());

        // Frame the model (feet at Y=0, top at Y~64) at ~80% of the viewport
        // height: camera pulled in to 100 units from the previous 120.
        view.setLookAt(new Vector3f(0f, 30f, 100f), new Vector3f(0f, 32f, 0f), new Vector3f(0f, 1f, 0f));

        // Dual-layer skins blend the inflated outer overlay over the base layer.
        // Texture bytes are premultiplied BGRA (JavaFX PixelFormat), so the blend
        // factors are GL_ONE / GL_ONE_MINUS_SRC_ALPHA, not straight-alpha.
        GL11.glEnable(GL11.GL_BLEND);
        GL11.glBlendFunc(GL11.GL_ONE, GL11.GL_ONE_MINUS_SRC_ALPHA);

        locModel = shader.uniform("uModel");
        locView = shader.uniform("uView");
        locProj = shader.uniform("uProj");
        locTexture = shader.uniform("uTexture");
        locLightDir = shader.uniform("uLightDir");
        locLightColor = shader.uniform("uLightColor");
        locAmbient = shader.uniform("uAmbient");

        initialized = true;
    }

    @Override
    public void render(int width, int height) {
        if (!initialized) {
            init();
        }
        uploadPendingSkin();

        float aspect = (float) width / (float) height;
        if (aspect != lastAspect) {
            proj.setPerspective(FOV_Y, aspect, NEAR, FAR);
            lastAspect = aspect;
        }
        model.identity()
                .rotateX(pitchRadians)
                .rotateY(rotationRadians);

        shader.use();
        shader.setMat4(locModel, model);
        shader.setMat4(locView, view);
        shader.setMat4(locProj, proj);
        shader.setInt(locTexture, 0);
        shader.setVec3(locLightDir, LIGHT_DIR.x, LIGHT_DIR.y, LIGHT_DIR.z);
        shader.setVec3(locLightColor, LIGHT_COLOR.x, LIGHT_COLOR.y, LIGHT_COLOR.z);
        shader.setVec3(locAmbient, AMBIENT.x, AMBIENT.y, AMBIENT.z);

        texture.bind(0);
        mesh.draw();
    }

    @Override
    public void dispose() {
        if (mesh != null) {
            mesh.dispose();
        }
        if (texture != null) {
            texture.dispose();
        }
        if (shader != null) {
            shader.dispose();
        }
    }

    /**
     * Replaces the skin texture. The buffer is consumed on the next render (on the
     * GL thread). Must be premultiplied BGRA, one byte per channel, position 0.
     */
    public void setSkin(ByteBuffer bgraPixels, int width, int height) {
        pendingSkin.set(new SkinData(bgraPixels, width, height));
    }

    /** Sets the rotation around the Y axis in radians. */
    public void setRotation(float radians) {
        this.rotationRadians = radians;
    }

    /** Sets the pitch (tilt) around the X axis in radians. */
    public void setPitch(float radians) {
        this.pitchRadians = radians;
    }

    private void uploadPendingSkin() {
        SkinData skin = pendingSkin.getAndSet(null);
        if (skin != null) {
            texture.upload(skin.width(), skin.height(), skin.pixels());
        }
    }

    private static ByteBuffer whitePixel() {
        ByteBuffer buffer = BufferUtils.createByteBuffer(4);
        buffer.put(new byte[]{(byte) 255, (byte) 255, (byte) 255, (byte) 255}).flip();
        return buffer;
    }

    private record SkinData(ByteBuffer pixels, int width, int height) {
    }
}
