package com.mcmanager.client.skin;

import javafx.scene.image.Image;
import javafx.scene.image.PixelWriter;
import javafx.scene.image.WritableImage;
import javafx.scene.paint.Color;

/**
 * Generates the two built-in "Steve" and "Alex" 64x64 skin placeholders used by
 * the skins gallery and as the default 3D preview when the player has not
 * uploaded a custom skin.
 *
 * <p>The artwork is intentionally simple (flat colored regions plus a small face),
 * but the colors are written to the exact regions of the standard 64x64 skin
 * layout so {@link SkinManager#extractHeadIcon(Image)} and the
 * {@link com.mcmanager.client.ui.component.Player3DRenderer} UV mapping both
 * read the correct pixels.
 */
public final class DefaultSkinFactory {

    private DefaultSkinFactory() {
    }

    public static Image steve() {
        return create(
                Color.web("#c68e5f"), // skin
                Color.web("#4a2f1b"), // hair
                Color.web("#00a2a2"), // shirt
                Color.web("#3b3f8a"), // pants
                Color.web("#5a83d6"), // eye
                Color.web("#2b2b2b")); // boots
    }

    public static Image alex() {
        return create(
                Color.web("#e0b48a"), // skin
                Color.web("#c4652e"), // hair
                Color.web("#65b33f"), // shirt
                Color.web("#8c5a3b"), // pants
                Color.web("#4f9d5b"), // eye
                Color.web("#2b2b2b")); // boots
    }

    private static Image create(Color skin, Color hair, Color shirt, Color pants, Color eye, Color boots) {
        WritableImage image = new WritableImage(64, 64);
        PixelWriter writer = image.getPixelWriter();

        // Base skin tone across the whole canvas.
        rect(writer, 0, 0, 64, 64, skin);

        // Head — hair on every face except the front (which keeps the skin tone
        // so the face features below stay visible).
        rect(writer, 8, 0, 16, 8, hair);   // top
        rect(writer, 16, 0, 24, 8, hair);  // bottom
        rect(writer, 24, 8, 32, 16, hair); // back
        rect(writer, 0, 8, 8, 16, hair);   // right side
        rect(writer, 16, 8, 24, 16, hair); // left side

        // Face (head front) — hair fringe, eyes, and mouth.
        rect(writer, 8, 8, 16, 10, hair);
        writer.setColor(12, 12, eye);
        writer.setColor(14, 12, eye);
        rect(writer, 12, 15, 16, 16, hair);

        // Torso (8 wide x 12 tall x 4 deep), spanning x16-40 y16-32.
        rect(writer, 16, 16, 40, 32, shirt);

        // Right arm (x40-56 y16-32) and left arm (x32-48 y48-64).
        rect(writer, 40, 16, 56, 32, shirt);
        rect(writer, 32, 48, 48, 64, shirt);

        // Right leg (x0-16 y16-32) and left leg (x16-32 y48-64).
        rect(writer, 0, 16, 16, 32, pants);
        rect(writer, 16, 48, 32, 64, pants);

        // Boots — the bottom four rows of each leg.
        rect(writer, 0, 28, 16, 32, boots);
        rect(writer, 16, 60, 32, 64, boots);

        return image;
    }

    private static void rect(PixelWriter writer, int x0, int y0, int x1, int y1, Color color) {
        for (int y = y0; y < y1; y++) {
            for (int x = x0; x < x1; x++) {
                writer.setColor(x, y, color);
            }
        }
    }
}
