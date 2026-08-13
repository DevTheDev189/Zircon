package com.mcmanager.client.render;

import org.junit.jupiter.api.Test;
import org.lwjgl.opengl.GL;
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
import static org.lwjgl.opengl.GL11.GL_COLOR_BUFFER_BIT;
import static org.lwjgl.opengl.GL11.GL_DEPTH_BUFFER_BIT;
import static org.lwjgl.opengl.GL11.GL_DEPTH_COMPONENT;
import static org.lwjgl.opengl.GL11.GL_NEAREST;
import static org.lwjgl.opengl.GL11.GL_RGBA;
import static org.lwjgl.opengl.GL11.GL_RGBA8;
import static org.lwjgl.opengl.GL11.GL_TEXTURE_2D;
import static org.lwjgl.opengl.GL11.GL_TEXTURE_MAG_FILTER;
import static org.lwjgl.opengl.GL11.GL_TEXTURE_MIN_FILTER;
import static org.lwjgl.opengl.GL11.GL_UNSIGNED_BYTE;
import static org.lwjgl.opengl.GL11.glBindTexture;
import static org.lwjgl.opengl.GL11.glClear;
import static org.lwjgl.opengl.GL11.glClearColor;
import static org.lwjgl.opengl.GL11.glDeleteTextures;
import static org.lwjgl.opengl.GL11.glGenTextures;
import static org.lwjgl.opengl.GL11.glReadPixels;
import static org.lwjgl.opengl.GL11.glTexImage2D;
import static org.lwjgl.opengl.GL11.glTexParameteri;
import static org.lwjgl.opengl.GL11.glViewport;
import static org.lwjgl.opengl.GL30.GL_COLOR_ATTACHMENT0;
import static org.lwjgl.opengl.GL30.GL_DEPTH_ATTACHMENT;
import static org.lwjgl.opengl.GL30.GL_FRAMEBUFFER;
import static org.lwjgl.opengl.GL30.GL_FRAMEBUFFER_COMPLETE;
import static org.lwjgl.opengl.GL30.GL_RENDERBUFFER;
import static org.lwjgl.opengl.GL30.glBindFramebuffer;
import static org.lwjgl.opengl.GL30.glBindRenderbuffer;
import static org.lwjgl.opengl.GL30.glCheckFramebufferStatus;
import static org.lwjgl.opengl.GL30.glDeleteFramebuffers;
import static org.lwjgl.opengl.GL30.glDeleteRenderbuffers;
import static org.lwjgl.opengl.GL30.glFramebufferRenderbuffer;
import static org.lwjgl.opengl.GL30.glFramebufferTexture2D;
import static org.lwjgl.opengl.GL30.glGenFramebuffers;
import static org.lwjgl.opengl.GL30.glGenRenderbuffers;
import static org.lwjgl.opengl.GL30.glRenderbufferStorage;

/**
 * M0 smoke test: proves LWJGL natives load, a hidden GLFW window can create an
 * OpenGL context, and we can render offscreen into an FBO and read pixels back.
 */
class GlContextSmokeTest {

    private static final int WIDTH = 64;
    private static final int HEIGHT = 64;

    @Test
    void createsHiddenContextAndClearsDefaultFramebuffer() {
        assertTrue(glfwInit(), "GLFW failed to initialize");
        long window = createWindow();
        try {
            glfwMakeContextCurrent(window);
            assertNotNull(GL.createCapabilities(), "Failed to create OpenGL capabilities");

            glClearColor(1.0f, 0.0f, 0.0f, 1.0f);
            glClear(GL_COLOR_BUFFER_BIT);

            ByteBuffer pixel = readPixel(32, 32);
            assertRgba(pixel, 255, 0, 0, 255);
        } finally {
            glfwDestroyWindow(window);
            glfwTerminate();
        }
    }

    @Test
    void rendersOffscreenToFramebufferObject() {
        assertTrue(glfwInit(), "GLFW failed to initialize");
        long window = createWindow();
        try {
            glfwMakeContextCurrent(window);
            assertNotNull(GL.createCapabilities(), "Failed to create OpenGL capabilities");

            int fbo = glGenFramebuffers();
            int colorTex = glGenTextures();
            int depthRbo = glGenRenderbuffers();
            try {
                glBindFramebuffer(GL_FRAMEBUFFER, fbo);

                glBindTexture(GL_TEXTURE_2D, colorTex);
                glTexImage2D(GL_TEXTURE_2D, 0, GL_RGBA8, WIDTH, HEIGHT, 0, GL_RGBA, GL_UNSIGNED_BYTE, (ByteBuffer) null);
                glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_MIN_FILTER, GL_NEAREST);
                glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_MAG_FILTER, GL_NEAREST);
                glFramebufferTexture2D(GL_FRAMEBUFFER, GL_COLOR_ATTACHMENT0, GL_TEXTURE_2D, colorTex, 0);

                glBindRenderbuffer(GL_RENDERBUFFER, depthRbo);
                glRenderbufferStorage(GL_RENDERBUFFER, GL_DEPTH_COMPONENT, WIDTH, HEIGHT);
                glFramebufferRenderbuffer(GL_FRAMEBUFFER, GL_DEPTH_ATTACHMENT, GL_RENDERBUFFER, depthRbo);

                assertEquals(GL_FRAMEBUFFER_COMPLETE, glCheckFramebufferStatus(GL_FRAMEBUFFER),
                        "Framebuffer is not complete");

                glViewport(0, 0, WIDTH, HEIGHT);
                glClearColor(0.0f, 1.0f, 0.0f, 1.0f);
                glClear(GL_COLOR_BUFFER_BIT | GL_DEPTH_BUFFER_BIT);

                ByteBuffer pixel = readPixel(32, 32);
                assertRgba(pixel, 0, 255, 0, 255);
            } finally {
                glDeleteFramebuffers(fbo);
                glDeleteTextures(colorTex);
                glDeleteRenderbuffers(depthRbo);
            }
        } finally {
            glfwDestroyWindow(window);
            glfwTerminate();
        }
    }

    private static long createWindow() {
        glfwDefaultWindowHints();
        glfwWindowHint(GLFW_VISIBLE, GLFW_FALSE);
        glfwWindowHint(GLFW_CONTEXT_VERSION_MAJOR, 3);
        glfwWindowHint(GLFW_CONTEXT_VERSION_MINOR, 3);
        glfwWindowHint(GLFW_OPENGL_PROFILE, GLFW_OPENGL_CORE_PROFILE);

        long window = glfwCreateWindow(WIDTH, HEIGHT, "gl-smoke", MemoryUtil.NULL, MemoryUtil.NULL);
        assertNotEquals(MemoryUtil.NULL, window, "GLFW window creation failed (no GL context available?)");
        return window;
    }

    private static ByteBuffer readPixel(int x, int y) {
        try (MemoryStack stack = MemoryStack.stackPush()) {
            ByteBuffer pixel = stack.malloc(4);
            glReadPixels(x, y, 1, 1, GL_RGBA, GL_UNSIGNED_BYTE, pixel);
            return pixel;
        }
    }

    private static void assertRgba(ByteBuffer pixel, int r, int g, int b, int a) {
        assertEquals((byte) r, pixel.get(0), "red channel");
        assertEquals((byte) g, pixel.get(1), "green channel");
        assertEquals((byte) b, pixel.get(2), "blue channel");
        assertEquals((byte) a, pixel.get(3), "alpha channel");
    }
}
