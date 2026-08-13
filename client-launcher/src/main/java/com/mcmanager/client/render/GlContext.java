package com.mcmanager.client.render;

import org.lwjgl.glfw.GLFW;
import org.lwjgl.opengl.GL;
import org.lwjgl.system.MemoryUtil;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

import java.util.List;
import java.util.concurrent.CopyOnWriteArrayList;

/**
 * Owns the single hidden GLFW window + OpenGL context and a render thread. All
 * {@link GlViewport}s are drawn by this one thread, which keeps context creation
 * and lifecycle in a single place.
 */
public final class GlContext {

    private static final Logger log = LoggerFactory.getLogger(GlContext.class);
    private static final GlContext INSTANCE = new GlContext();

    private final List<GlViewport> viewports = new CopyOnWriteArrayList<>();
    private final Object lock = new Object();

    private Thread thread;
    private volatile boolean running;
    private long window;

    private GlContext() {
    }

    public static GlContext instance() {
        return INSTANCE;
    }

    public void register(GlViewport viewport) {
        viewports.add(viewport);
        ensureStarted();
        viewport.requestRender();
    }

    public void requestRender() {
        synchronized (lock) {
            lock.notifyAll();
        }
    }

    public void dispose() {
        running = false;
        requestRender();
        Thread t = thread;
        if (t != null) {
            try {
                t.join(2000);
            } catch (InterruptedException e) {
                Thread.currentThread().interrupt();
            }
        }
    }

    private void ensureStarted() {
        synchronized (lock) {
            if (thread == null) {
                running = true;
                thread = new Thread(this::run, "gl-renderer");
                thread.setDaemon(true);
                thread.start();
            }
        }
    }

    private void run() {
        try {
            if (!GLFW.glfwInit()) {
                throw new IllegalStateException("GLFW failed to initialize");
            }

            GLFW.glfwDefaultWindowHints();
            GLFW.glfwWindowHint(GLFW.GLFW_VISIBLE, GLFW.GLFW_FALSE);
            GLFW.glfwWindowHint(GLFW.GLFW_CONTEXT_VERSION_MAJOR, 3);
            GLFW.glfwWindowHint(GLFW.GLFW_CONTEXT_VERSION_MINOR, 3);
            GLFW.glfwWindowHint(GLFW.GLFW_OPENGL_PROFILE, GLFW.GLFW_OPENGL_CORE_PROFILE);
            GLFW.glfwWindowHint(GLFW.GLFW_OPENGL_FORWARD_COMPAT, GLFW.GLFW_TRUE); // macOS core profile
            GLFW.glfwWindowHint(GLFW.GLFW_COCOA_MENUBAR, GLFW.GLFW_FALSE);         // ignored off macOS

            window = GLFW.glfwCreateWindow(16, 16, "zircon-gl", MemoryUtil.NULL, MemoryUtil.NULL);
            if (window == MemoryUtil.NULL) {
                throw new IllegalStateException("Failed to create hidden GLFW window");
            }

            GLFW.glfwMakeContextCurrent(window);
            if (GL.createCapabilities() == null) {
                throw new IllegalStateException("Failed to create OpenGL capabilities");
            }

            while (running) {
                boolean rendered = false;
                for (GlViewport viewport : viewports) {
                    if (viewport.consumeDirty()) {
                        try {
                            viewport.render();
                            rendered = true;
                        } catch (Throwable t) {
                            log.error("Failed to render GL viewport", t);
                        }
                    }
                }
                if (!rendered) {
                    synchronized (lock) {
                        try {
                            lock.wait(200);
                        } catch (InterruptedException e) {
                            Thread.currentThread().interrupt();
                            break;
                        }
                    }
                }
            }
        } catch (Throwable t) {
            log.error("OpenGL renderer failed", t);
        } finally {
            for (GlViewport viewport : viewports) {
                try {
                    viewport.disposeGl();
                } catch (Throwable t) {
                    log.warn("Failed to dispose GL viewport", t);
                }
            }
            if (window != MemoryUtil.NULL) {
                GLFW.glfwDestroyWindow(window);
            }
            GLFW.glfwTerminate();
        }
    }
}
