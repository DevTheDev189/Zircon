package com.mcmanager.client.render;

import org.lwjgl.opengl.GL20;
import org.lwjgl.system.MemoryStack;
import org.joml.Matrix4f;

/**
 * Thin wrapper around a compiled/linked GLSL program. Provides typed uniform
 * setters so callers never touch raw locations.
 */
public final class GlShader {

    private final int program;

    public GlShader(String vertexSrc, String fragmentSrc) {
        int vs = compile(GL20.GL_VERTEX_SHADER, vertexSrc);
        int fs = compile(GL20.GL_FRAGMENT_SHADER, fragmentSrc);

        program = GL20.glCreateProgram();
        GL20.glAttachShader(program, vs);
        GL20.glAttachShader(program, fs);
        GL20.glLinkProgram(program);

        if (GL20.glGetProgrami(program, GL20.GL_LINK_STATUS) == GL20.GL_FALSE) {
            String log = GL20.glGetProgramInfoLog(program);
            throw new IllegalStateException("Shader link failed: " + log);
        }

        // Shaders can be deleted after linking.
        GL20.glDeleteShader(vs);
        GL20.glDeleteShader(fs);
    }

    public void use() {
        GL20.glUseProgram(program);
    }

    public int uniform(String name) {
        return GL20.glGetUniformLocation(program, name);
    }

    public void setInt(int location, int value) {
        GL20.glUniform1i(location, value);
    }

    public void setVec3(int location, float x, float y, float z) {
        GL20.glUniform3f(location, x, y, z);
    }

    public void setMat4(int location, Matrix4f matrix) {
        try (MemoryStack stack = MemoryStack.stackPush()) {
            GL20.glUniformMatrix4fv(location, false, matrix.get(stack.mallocFloat(16)));
        }
    }

    public void dispose() {
        GL20.glDeleteProgram(program);
    }

    private static int compile(int type, String source) {
        int shader = GL20.glCreateShader(type);
        GL20.glShaderSource(shader, source);
        GL20.glCompileShader(shader);
        if (GL20.glGetShaderi(shader, GL20.GL_COMPILE_STATUS) == GL20.GL_FALSE) {
            String log = GL20.glGetShaderInfoLog(shader);
            throw new IllegalStateException("Shader compile failed: " + log);
        }
        return shader;
    }
}
