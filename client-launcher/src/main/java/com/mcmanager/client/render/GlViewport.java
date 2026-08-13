package com.mcmanager.client.render;

import javafx.application.Platform;
import javafx.scene.image.ImageView;
import javafx.scene.image.PixelBuffer;
import javafx.scene.image.PixelFormat;
import javafx.scene.image.WritableImage;
import javafx.scene.layout.StackPane;
import org.lwjgl.BufferUtils;
import org.lwjgl.opengl.GL11;
import org.lwjgl.opengl.GL12;
import org.lwjgl.opengl.GL30;

import java.nio.ByteBuffer;
import java.util.concurrent.atomic.AtomicBoolean;

/**
 * One offscreen render target: an FBO rendered by the GL thread, whose color is
 * read back into a {@link PixelBuffer} and shown in an {@link ImageView}.
 *
 * <p>The {@code pixels} buffer backs the {@link PixelBuffer}; the GL thread fills
 * it via {@code glReadPixels}, then marshals the dirty notification to the JavaFX
 * application thread via {@link Platform#runLater}. We intentionally avoid
 * {@code ByteBuffer.put(ByteBuffer)} on LWJGL-allocated buffers because it routes
 * through {@code Unsafe.copyMemory} and crashes on newer JDKs.
 *
 * <p>Dimensions are dynamic: {@link #resize(int, int)} reallocates the pixel
 * buffer / {@code PixelBuffer} / {@code ImageView} on the FX thread and marks the
 * framebuffer for re-creation on the GL thread at the start of the next render.
 * The {@link #getNode() node} is a {@link StackPane} whose only child is the
 * {@code ImageView}, so a resize can swap the image in place without callers
 * re-attaching a node.
 */
public final class GlViewport {

    private final Drawable drawable;
    private final StackPane node;

    /** Consistent (pixels, width, height) triple read by the GL thread each frame. */
    private volatile Frame frame;
    private volatile PixelBuffer<ByteBuffer> pixelBuffer;

    private final AtomicBoolean dirty = new AtomicBoolean(true);
    private final AtomicBoolean updatePending = new AtomicBoolean(false);
    private final AtomicBoolean glResizePending = new AtomicBoolean(false);

    private int fbo;
    private int colorTexture;
    private int depthRbo;

    /** Must be constructed on the JavaFX application thread. */
    public GlViewport(int width, int height, Drawable drawable) {
        this.drawable = drawable;

        this.frame = new Frame(BufferUtils.createByteBuffer(width * height * 4), width, height);
        this.pixelBuffer = new PixelBuffer<>(width, height, frame.pixels(), PixelFormat.getByteBgraPreInstance());
        this.node = new StackPane(createImageView(pixelBuffer));
    }

    /** @return the JavaFX node displaying this viewport (a pickable StackPane). */
    public StackPane getNode() {
        return node;
    }

    /**
     * Re-allocates the viewport at a new pixel resolution. Safe to call from the
     * FX thread while the GL thread is rendering: the pixel buffer and JavaFX
     * image are swapped here, and the framebuffer is re-created on the GL thread
     * at the start of the next render. No-op when the size is unchanged.
     */
    public void resize(int newWidth, int newHeight) {
        int w = Math.max(1, newWidth);
        int h = Math.max(1, newHeight);
        Frame current = frame;
        if (w == current.width() && h == current.height()) {
            return;
        }

        ByteBuffer newPixels = BufferUtils.createByteBuffer(w * h * 4);
        PixelBuffer<ByteBuffer> newPixelBuffer = new PixelBuffer<>(w, h, newPixels,
                PixelFormat.getByteBgraPreInstance());

        this.frame = new Frame(newPixels, w, h);
        this.pixelBuffer = newPixelBuffer;
        node.getChildren().clear();
        node.getChildren().add(createImageView(newPixelBuffer));

        glResizePending.set(true);
        requestRender();
    }

    public void requestRender() {
        dirty.set(true);
        GlContext.instance().requestRender();
    }

    boolean consumeDirty() {
        return dirty.getAndSet(false);
    }

    /** Renders one frame. Called on the GL thread with the context current. */
    void render() {
        ensureFbo();

        Frame f = frame;
        int w = f.width();
        int h = f.height();
        ByteBuffer target = f.pixels();

        GL30.glBindFramebuffer(GL30.GL_FRAMEBUFFER, fbo);
        GL11.glViewport(0, 0, w, h);
        GL11.glEnable(GL11.GL_DEPTH_TEST);
        GL11.glClearColor(0.0f, 0.0f, 0.0f, 0.0f);
        GL11.glClear(GL11.GL_COLOR_BUFFER_BIT | GL11.GL_DEPTH_BUFFER_BIT);

        drawable.render(w, h);

        GL11.glPixelStorei(GL11.GL_PACK_ALIGNMENT, 1);
        GL11.glReadPixels(0, 0, w, h, GL12.GL_BGRA, GL11.GL_UNSIGNED_BYTE, target);
        GL30.glBindFramebuffer(GL30.GL_FRAMEBUFFER, 0);

        // Marshal the dirty notification to the FX thread, coalescing updates.
        // The buffer is captured so a resize between schedule and execution cannot
        // refresh a mismatched image.
        PixelBuffer<ByteBuffer> refreshTarget = pixelBuffer;
        if (updatePending.compareAndSet(false, true)) {
            Platform.runLater(() -> {
                try {
                    refreshTarget.updateBuffer(pb -> null);
                } finally {
                    updatePending.set(false);
                }
            });
        }
    }

    /** Frees GL resources. Called on the GL thread with the context current. */
    void disposeGl() {
        if (colorTexture != 0) {
            GL11.glDeleteTextures(colorTexture);
        }
        if (depthRbo != 0) {
            GL30.glDeleteRenderbuffers(depthRbo);
        }
        if (fbo != 0) {
            GL30.glDeleteFramebuffers(fbo);
        }
        drawable.dispose();
        fbo = 0;
        colorTexture = 0;
        depthRbo = 0;
    }

    private void ensureFbo() {
        // A resize happened after the previous frame: tear down the old FBO so the
        // next block re-creates it at the new (already swapped) dimensions.
        if (glResizePending.getAndSet(false) && fbo != 0) {
            GL30.glDeleteFramebuffers(fbo);
            GL11.glDeleteTextures(colorTexture);
            GL30.glDeleteRenderbuffers(depthRbo);
            fbo = 0;
            colorTexture = 0;
            depthRbo = 0;
        }
        if (fbo != 0) {
            return;
        }

        Frame f = frame;
        int w = f.width();
        int h = f.height();

        fbo = GL30.glGenFramebuffers();
        GL30.glBindFramebuffer(GL30.GL_FRAMEBUFFER, fbo);

        colorTexture = GL11.glGenTextures();
        GL11.glBindTexture(GL11.GL_TEXTURE_2D, colorTexture);
        GL11.glTexImage2D(GL11.GL_TEXTURE_2D, 0, GL11.GL_RGBA8, w, h, 0,
                GL11.GL_RGBA, GL11.GL_UNSIGNED_BYTE, (ByteBuffer) null);
        GL11.glTexParameteri(GL11.GL_TEXTURE_2D, GL11.GL_TEXTURE_MIN_FILTER, GL11.GL_NEAREST);
        GL11.glTexParameteri(GL11.GL_TEXTURE_2D, GL11.GL_TEXTURE_MAG_FILTER, GL11.GL_NEAREST);
        GL30.glFramebufferTexture2D(GL30.GL_FRAMEBUFFER, GL30.GL_COLOR_ATTACHMENT0,
                GL11.GL_TEXTURE_2D, colorTexture, 0);

        depthRbo = GL30.glGenRenderbuffers();
        GL30.glBindRenderbuffer(GL30.GL_RENDERBUFFER, depthRbo);
        GL30.glRenderbufferStorage(GL30.GL_RENDERBUFFER, GL11.GL_DEPTH_COMPONENT, w, h);
        GL30.glFramebufferRenderbuffer(GL30.GL_FRAMEBUFFER, GL30.GL_DEPTH_ATTACHMENT,
                GL30.GL_RENDERBUFFER, depthRbo);

        GL30.glBindFramebuffer(GL30.GL_FRAMEBUFFER, 0);
    }

    private static ImageView createImageView(PixelBuffer<ByteBuffer> buffer) {
        ImageView view = new ImageView(new WritableImage(buffer));
        view.setSmooth(false);
        // OpenGL readback is bottom-up; JavaFX images are top-down. Flip here so
        // the GL coordinate system, winding, and lighting stay untouched.
        view.setScaleY(-1.0);
        return view;
    }

    private record Frame(ByteBuffer pixels, int width, int height) {
    }
}
