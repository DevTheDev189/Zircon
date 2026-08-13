package com.mcmanager.client.render;

import org.lwjgl.opengl.GL11;
import org.lwjgl.opengl.GL12;
import org.lwjgl.opengl.GL13;

import java.nio.ByteBuffer;

/**
 * A 2D texture uploaded with nearest-neighbor filtering (pixel-art friendly).
 * Textures are uploaded from premultiplied BGRA bytes to keep parity with
 * JavaFX's {@code PixelFormat} and OpenGL's {@code GL_BGRA} readback.
 */
public final class GlTexture {

    private final int id;

    private GlTexture(int id) {
        this.id = id;
    }

    public static GlTexture create(int width, int height, ByteBuffer bgraPixels) {
        int id = GL11.glGenTextures();
        GlTexture texture = new GlTexture(id);
        texture.upload(width, height, bgraPixels);
        return texture;
    }

    public void upload(int width, int height, ByteBuffer bgraPixels) {
        GL11.glBindTexture(GL11.GL_TEXTURE_2D, id);
        GL11.glTexParameteri(GL11.GL_TEXTURE_2D, GL11.GL_TEXTURE_MIN_FILTER, GL11.GL_NEAREST);
        GL11.glTexParameteri(GL11.GL_TEXTURE_2D, GL11.GL_TEXTURE_MAG_FILTER, GL11.GL_NEAREST);
        GL11.glTexParameteri(GL11.GL_TEXTURE_2D, GL11.GL_TEXTURE_WRAP_S, GL12.GL_CLAMP_TO_EDGE);
        GL11.glTexParameteri(GL11.GL_TEXTURE_2D, GL11.GL_TEXTURE_WRAP_T, GL12.GL_CLAMP_TO_EDGE);
        GL11.glTexImage2D(GL11.GL_TEXTURE_2D, 0, GL11.GL_RGBA8, width, height, 0,
                GL12.GL_BGRA, GL11.GL_UNSIGNED_BYTE, bgraPixels);
    }

    public void bind(int unit) {
        GL13.glActiveTexture(GL13.GL_TEXTURE0 + unit);
        GL11.glBindTexture(GL11.GL_TEXTURE_2D, id);
    }

    public void dispose() {
        GL11.glDeleteTextures(id);
    }
}
