package com.mcmanager.client.render;

import org.junit.jupiter.api.Test;
import org.lwjgl.BufferUtils;
import org.lwjgl.opengl.GL;
import org.lwjgl.opengl.GL11;
import org.lwjgl.opengl.GL12;
import org.lwjgl.opengl.GL30;
import org.lwjgl.system.MemoryStack;
import org.lwjgl.system.MemoryUtil;

import java.nio.ByteBuffer;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertNotEquals;
import static org.junit.jupiter.api.Assertions.assertNotNull;
import static org.junit.jupiter.api.Assertions.assertTrue;
import static org.lwjgl.glfw.GLFW.GLFW_CONTEXT_VERSION_MAJOR;
import static org.lwjgl.glfw.GLFW.GLFW_CONTEXT_VERSION_MINOR;
import static org.lwjgl.glfw.GLFW.GLFW_FALSE;
import static org.lwjgl.glfw.GLFW.GLFW_OPENGL_CORE_PROFILE;
import static org.lwjgl.glfw.GLFW.GLFW_OPENGL_PROFILE;
import static org.lwjgl.glfw.GLFW.GLFW_VISIBLE;
import static org.lwjgl.glfw.GLFW.glfwCreateWindow;
import static org.lwjgl.glfw.GLFW.glfwDefaultWindowHints;
import static org.lwjgl.glfw.GLFW.glfwDestroyWindow;
import static org.lwjgl.glfw.GLFW.glfwInit;
import static org.lwjgl.glfw.GLFW.glfwMakeContextCurrent;
import static org.lwjgl.glfw.GLFW.glfwTerminate;
import static org.lwjgl.glfw.GLFW.glfwWindowHint;

/**
 * Renders the player model with a solid-red skin offscreen and verifies the
 * framebuffer contains only red model pixels plus transparent background. This
 * proves the shader/mesh/texture pipeline end-to-end without a visible window.
 */
class PlayerRendererTest {

    private static final int SIZE = 64;

    @Test
    void rendersSolidRedPlayerOntoTransparentBackground() {
        ByteBuffer pixels = renderFramebuffer(renderer -> renderer.setSkin(solidRedSkin(), SIZE, SIZE));
        assertRenderedOutput(pixels);
    }

    @Test
    void rendersWithPitchAndRotationApplied() {
        ByteBuffer pixels = renderFramebuffer(renderer -> {
            renderer.setSkin(solidRedSkin(), SIZE, SIZE);
            renderer.setPitch((float) Math.toRadians(35));
            renderer.setRotation((float) Math.toRadians(90));
        });
        assertRenderedOutput(pixels);
    }

    /** Creates a hidden GL context + FBO, renders the configured player, reads back the color. */
    private static ByteBuffer renderFramebuffer(java.util.function.Consumer<PlayerRenderer> configure) {
        assertTrue(glfwInit(), "GLFW failed to initialize");
        long window = createWindow();
        try {
            glfwMakeContextCurrent(window);
            assertNotNull(GL.createCapabilities());

            int fbo = GL30.glGenFramebuffers();
            int colorTex = GL11.glGenTextures();
            int depthRbo = GL30.glGenRenderbuffers();
            try {
                setupFbo(fbo, colorTex, depthRbo);

                PlayerRenderer renderer = new PlayerRenderer();
                configure.accept(renderer);

                GL30.glBindFramebuffer(GL30.GL_FRAMEBUFFER, fbo);
                GL11.glViewport(0, 0, SIZE, SIZE);
                GL11.glEnable(GL11.GL_DEPTH_TEST);
                GL11.glClearColor(0.0f, 0.0f, 0.0f, 0.0f);
                GL11.glClear(GL11.GL_COLOR_BUFFER_BIT | GL11.GL_DEPTH_BUFFER_BIT);
                renderer.render(SIZE, SIZE);

                ByteBuffer pixels = BufferUtils.createByteBuffer(SIZE * SIZE * 4);
                GL11.glReadPixels(0, 0, SIZE, SIZE, GL12.GL_BGRA, GL11.GL_UNSIGNED_BYTE, pixels);

                renderer.dispose();
                return pixels;
            } finally {
                GL30.glDeleteFramebuffers(fbo);
                GL11.glDeleteTextures(colorTex);
                GL30.glDeleteRenderbuffers(depthRbo);
            }
        } finally {
            glfwDestroyWindow(window);
            glfwTerminate();
        }
    }

    private static void setupFbo(int fbo, int colorTex, int depthRbo) {
        GL30.glBindFramebuffer(GL30.GL_FRAMEBUFFER, fbo);

        GL11.glBindTexture(GL11.GL_TEXTURE_2D, colorTex);
        GL11.glTexImage2D(GL11.GL_TEXTURE_2D, 0, GL11.GL_RGBA8, SIZE, SIZE, 0,
                GL11.GL_RGBA, GL11.GL_UNSIGNED_BYTE, (ByteBuffer) null);
        GL11.glTexParameteri(GL11.GL_TEXTURE_2D, GL11.GL_TEXTURE_MIN_FILTER, GL11.GL_NEAREST);
        GL11.glTexParameteri(GL11.GL_TEXTURE_2D, GL11.GL_TEXTURE_MAG_FILTER, GL11.GL_NEAREST);
        GL30.glFramebufferTexture2D(GL30.GL_FRAMEBUFFER, GL30.GL_COLOR_ATTACHMENT0,
                GL11.GL_TEXTURE_2D, colorTex, 0);

        GL30.glBindRenderbuffer(GL30.GL_RENDERBUFFER, depthRbo);
        GL30.glRenderbufferStorage(GL30.GL_RENDERBUFFER, GL11.GL_DEPTH_COMPONENT, SIZE, SIZE);
        GL30.glFramebufferRenderbuffer(GL30.GL_FRAMEBUFFER, GL30.GL_DEPTH_ATTACHMENT,
                GL30.GL_RENDERBUFFER, depthRbo);

        assertEquals(GL30.GL_FRAMEBUFFER_COMPLETE, GL30.glCheckFramebufferStatus(GL30.GL_FRAMEBUFFER));
        GL30.glBindFramebuffer(GL30.GL_FRAMEBUFFER, 0);
    }

    private static ByteBuffer solidRedSkin() {
        ByteBuffer skin = BufferUtils.createByteBuffer(SIZE * SIZE * 4);
        for (int i = 0; i < SIZE * SIZE; i++) {
            skin.put((byte) 0);      // B
            skin.put((byte) 0);      // G
            skin.put((byte) 255);    // R
            skin.put((byte) 255);    // A
        }
        return skin.flip();
    }

    private static void assertRenderedOutput(ByteBuffer pixels) {
        boolean foundOpaque = false;
        boolean foundTransparent = false;

        for (int i = 0; i < pixels.capacity(); i += 4) {
            int b = pixels.get(i) & 0xFF;
            int g = pixels.get(i + 1) & 0xFF;
            int r = pixels.get(i + 2) & 0xFF;
            int a = pixels.get(i + 3) & 0xFF;

            if (a == 0) {
                foundTransparent = true;
            } else {
                foundOpaque = true;
                assertEquals(255, a, "model pixels must be opaque");
                assertTrue(r > 0, "model pixels must have red light");
                assertEquals(0, g, "green channel must stay zero for a red skin");
                assertEquals(0, b, "blue channel must stay zero for a red skin");
            }
        }

        assertTrue(foundOpaque, "expected some model pixels to be drawn");
        assertTrue(foundTransparent, "expected some background pixels to remain transparent");
    }

    private static long createWindow() {
        glfwDefaultWindowHints();
        glfwWindowHint(GLFW_VISIBLE, GLFW_FALSE);
        glfwWindowHint(GLFW_CONTEXT_VERSION_MAJOR, 3);
        glfwWindowHint(GLFW_CONTEXT_VERSION_MINOR, 3);
        glfwWindowHint(GLFW_OPENGL_PROFILE, GLFW_OPENGL_CORE_PROFILE);

        long window = glfwCreateWindow(SIZE, SIZE, "player-test", MemoryUtil.NULL, MemoryUtil.NULL);
        assertNotEquals(MemoryUtil.NULL, window, "GLFW window creation failed");
        return window;
    }
}
