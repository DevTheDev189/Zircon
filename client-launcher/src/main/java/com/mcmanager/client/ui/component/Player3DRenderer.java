package com.mcmanager.client.ui.component;

import com.mcmanager.client.render.GlContext;
import com.mcmanager.client.render.GlViewport;
import com.mcmanager.client.render.PlayerRenderer;
import com.mcmanager.client.skin.DefaultSkinFactory;
import javafx.scene.Node;
import javafx.scene.image.Image;
import javafx.scene.image.PixelFormat;
import javafx.scene.layout.StackPane;
import org.lwjgl.BufferUtils;

import java.nio.ByteBuffer;

/**
 * UI façade over the LWJGL/OpenGL player renderer. Exposes a JavaFX {@link Node}
 * (a pickable {@link StackPane} wrapping the {@code ImageView} fed by a
 * {@link javafx.scene.image.PixelBuffer}) plus the familiar
 * {@link #updateSkin(Image)} and drag-to-rotate / drag-to-pitch contract.
 *
 * <p>The offscreen framebuffer tracks the node's layout bounds: whenever the
 * container JavaFX lays the preview out at changes size, {@link GlViewport#resize}
 * re-allocates it so the 3D preview stays crisp instead of scaling a fixed
 * resolution image.
 */
public class Player3DRenderer {

    /** Lower bound for the offscreen framebuffer; avoids degenerate sizes during layout. */
    private static final int MIN_SIZE = 64;

    private final PlayerRenderer player = new PlayerRenderer();
    private final GlViewport viewport;
    private final StackPane node;

    private double mouseAnchorX;
    private double mouseAnchorY;
    private volatile double rotationDegrees;
    private volatile double pitchDegrees;

    public Player3DRenderer(double width, double height) {
        this.viewport = new GlViewport((int) width, (int) height, player);
        GlContext.instance().register(viewport);

        this.node = viewport.getNode();
        this.node.setPickOnBounds(true);

        this.node.setOnMousePressed(e -> {
            mouseAnchorX = e.getSceneX();
            mouseAnchorY = e.getSceneY();
        });
        this.node.setOnMouseDragged(e -> {
            double deltaX = e.getSceneX() - mouseAnchorX;
            double deltaY = e.getSceneY() - mouseAnchorY;

            rotationDegrees += deltaX * 0.5;
            // Vertical drag tilts the model up/down, clamped to ±45°.
            pitchDegrees = Math.max(-45.0, Math.min(45.0, pitchDegrees - deltaY * 0.5));

            player.setRotation((float) Math.toRadians(rotationDegrees));
            player.setPitch((float) Math.toRadians(pitchDegrees));

            mouseAnchorX = e.getSceneX();
            mouseAnchorY = e.getSceneY();
            viewport.requestRender();
        });

        // Keep the framebuffer resolution in lockstep with the space the layout
        // actually gives the preview (see class javadoc).
        node.widthProperty().addListener((obs, oldVal, newVal) ->
                viewport.resize(roundSize(newVal), roundSize(node.getHeight())));
        node.heightProperty().addListener((obs, oldVal, newVal) ->
                viewport.resize(roundSize(node.getWidth()), roundSize(newVal)));

        player.setSkin(toBgra(DefaultSkinFactory.steve()), 64, 64);
        viewport.requestRender();
    }

    /** @return the JavaFX node to place in the UI tree. */
    public Node getNode() {
        return node;
    }

    /** @return the current rotation angle around the Y axis (degrees). */
    public double getRotationAngle() {
        return rotationDegrees;
    }

    /** @return the current pitch angle around the X axis (degrees, clamped to ±45). */
    public double getPitchAngle() {
        return pitchDegrees;
    }

    /**
     * Swaps the player skin. Must be called on the JavaFX application thread; the
     * actual texture upload happens asynchronously on the GL render thread.
     */
    public void updateSkin(Image skinImage) {
        Image image = skinImage != null ? skinImage : DefaultSkinFactory.steve();
        player.setSkin(toBgra(image), (int) image.getWidth(), (int) image.getHeight());
        viewport.requestRender();
    }

    private static int roundSize(Number value) {
        return Math.max(MIN_SIZE, (int) Math.round(value.doubleValue()));
    }

    private static ByteBuffer toBgra(Image image) {
        int width = (int) image.getWidth();
        int height = (int) image.getHeight();
        byte[] data = new byte[width * height * 4];
        image.getPixelReader().getPixels(0, 0, width, height,
                PixelFormat.getByteBgraPreInstance(), data, 0, width * 4);

        ByteBuffer buffer = BufferUtils.createByteBuffer(data.length);
        buffer.put(data).flip();
        return buffer;
    }
}
