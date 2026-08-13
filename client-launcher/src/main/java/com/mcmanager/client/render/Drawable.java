package com.mcmanager.client.render;

/**
 * Something that can be rendered by the GL thread into a bound framebuffer.
 * Implementations are responsible for uploading their own GPU resources on the
 * GL thread (the context is current when these methods are called).
 */
public interface Drawable {

    /** Uploads GPU resources once. Called on the GL thread with the context current. */
    void init();

    /**
     * Draws into the currently bound framebuffer. The caller has already set the
     * viewport and cleared color/depth.
     *
     * @param width  framebuffer width in pixels
     * @param height framebuffer height in pixels
     */
    void render(int width, int height);

    /** Frees GPU resources. Called on the GL thread with the context current. */
    void dispose();
}
