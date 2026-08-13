package com.mcmanager.client.render;

import org.lwjgl.opengl.GL11;
import org.lwjgl.opengl.GL15;
import org.lwjgl.opengl.GL20;
import org.lwjgl.opengl.GL30;
import org.lwjgl.system.MemoryStack;

import java.nio.FloatBuffer;

/**
 * A simple interleaved triangle mesh: a VAO + VBO with a fixed layout of
 * {@code position(3), normal(3), uv(2)}. Attribute locations are 0, 1, and 2
 * to match the shader.
 */
public final class GlMesh {

    private static final int FLOATS_PER_VERTEX = 8;
    private static final int STRIDE_BYTES = FLOATS_PER_VERTEX * Float.BYTES;

    private final int vao;
    private final int vbo;
    private final int vertexCount;

    private GlMesh(int vao, int vbo, int vertexCount) {
        this.vao = vao;
        this.vbo = vbo;
        this.vertexCount = vertexCount;
    }

    public static GlMesh create(float[] interleaved) {
        int vao = GL30.glGenVertexArrays();
        GL30.glBindVertexArray(vao);

        int vbo = GL15.glGenBuffers();
        GL15.glBindBuffer(GL15.GL_ARRAY_BUFFER, vbo);
        try (MemoryStack stack = MemoryStack.stackPush()) {
            FloatBuffer buffer = stack.mallocFloat(interleaved.length);
            buffer.put(interleaved).flip();
            GL15.glBufferData(GL15.GL_ARRAY_BUFFER, buffer, GL15.GL_STATIC_DRAW);
        }

        // position
        GL20.glVertexAttribPointer(0, 3, GL11.GL_FLOAT, false, STRIDE_BYTES, 0);
        GL20.glEnableVertexAttribArray(0);
        // normal
        GL20.glVertexAttribPointer(1, 3, GL11.GL_FLOAT, false, STRIDE_BYTES, 3L * Float.BYTES);
        GL20.glEnableVertexAttribArray(1);
        // uv
        GL20.glVertexAttribPointer(2, 2, GL11.GL_FLOAT, false, STRIDE_BYTES, 6L * Float.BYTES);
        GL20.glEnableVertexAttribArray(2);

        GL30.glBindVertexArray(0);
        return new GlMesh(vao, vbo, interleaved.length / FLOATS_PER_VERTEX);
    }

    public void draw() {
        GL30.glBindVertexArray(vao);
        GL11.glDrawArrays(GL11.GL_TRIANGLES, 0, vertexCount);
        GL30.glBindVertexArray(0);
    }

    public void dispose() {
        GL30.glDeleteVertexArrays(vao);
        GL15.glDeleteBuffers(vbo);
    }
}
