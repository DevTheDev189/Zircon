package com.mcmanager.client.skin;

import javafx.scene.image.Image;
import javafx.scene.image.PixelReader;
import javafx.scene.image.PixelWriter;
import javafx.scene.image.WritableImage;

import java.io.File;
import java.io.FileInputStream;
import java.io.IOException;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.StandardCopyOption;
import java.util.Comparator;
import java.util.List;
import java.util.stream.Stream;

/**
 * Stores and loads custom player PNG skin files.
 *
 * <p>The active skin lives at {@code ~/.mcmanager/skins/active_skin.png}. Every
 * saved skin is also archived under {@code ~/.mcmanager/skins/history/} so the UI
 * can offer a "recently uploaded skins" gallery.
 */
public class SkinManager {

    private static final Path SKIN_DIR = Path.of(System.getProperty("user.home"), ".mcmanager", "skins");
    private static final Path ACTIVE_SKIN = SKIN_DIR.resolve("active_skin.png");
    private static final Path HISTORY_DIR = SKIN_DIR.resolve("history");

    /** Maximum number of skin files retained in history; the oldest are pruned. */
    private static final int HISTORY_LIMIT = 25;

    public static void saveSkin(File sourcePng) throws IOException {
        Files.createDirectories(SKIN_DIR);
        Files.copy(sourcePng.toPath(), ACTIVE_SKIN, StandardCopyOption.REPLACE_EXISTING);
        saveToHistory(sourcePng);
    }

    /**
     * Archives a skin PNG into the history folder under a timestamped name so
     * repeated uploads never overwrite each other.
     */
    public static void saveToHistory(File sourcePng) throws IOException {
        Files.createDirectories(HISTORY_DIR);
        String safeName = sourcePng.getName().replaceAll("[^A-Za-z0-9._-]", "_");
        if (!safeName.toLowerCase().endsWith(".png")) {
            safeName = safeName + ".png";
        }
        Path target = HISTORY_DIR.resolve(System.currentTimeMillis() + "-" + safeName);
        Files.copy(sourcePng.toPath(), target, StandardCopyOption.REPLACE_EXISTING);
        pruneHistory();
    }

    /**
     * @return history skin files ordered by modification time, newest first
     *         (empty list when the history folder does not exist yet).
     */
    public static List<Path> getSkinHistory() {
        if (!Files.isDirectory(HISTORY_DIR)) {
            return List.of();
        }
        try (Stream<Path> s = Files.list(HISTORY_DIR)) {
            return s.filter(p -> p.toString().toLowerCase().endsWith(".png"))
                    .sorted(Comparator.comparingLong((Path p) -> p.toFile().lastModified()).reversed())
                    .toList();
        } catch (IOException e) {
            return List.of();
        }
    }

    /** Drops the oldest history files beyond {@link #HISTORY_LIMIT}. */
    private static void pruneHistory() {
        List<Path> history = getSkinHistory();
        for (int i = HISTORY_LIMIT; i < history.size(); i++) {
            try {
                Files.deleteIfExists(history.get(i));
            } catch (IOException ignored) {
                // best-effort pruning
            }
        }
    }

    /**
     * Crops the 8x8 face area (pixels {@code (8,8)-(16,16)}) of a 64x64 skin as a
     * player head icon for the sidebar user card.
     *
     * @return the 8x8 face crop, or {@code null} when the skin is too small
     */
    public static Image extractHeadIcon(Image skin) {
        if (skin == null || skin.getWidth() < 16 || skin.getHeight() < 16) {
            return null;
        }
        PixelReader reader = skin.getPixelReader();
        WritableImage head = new WritableImage(8, 8);
        head.getPixelWriter().setPixels(0, 0, 8, 8, reader, 8, 8);
        return head;
    }

    /**
     * Returns the 8x8 face crop upscaled by {@code scale} using nearest-neighbor
     * sampling. The result can be displayed 1:1 without JavaFX interpolation, so
     * the head icon stays pixel-perfect instead of blurry.
     *
     * @return an {@code (8 * scale)} by {@code (8 * scale)} image, or {@code null}
     *         when the skin is too small
     */
    public static Image extractHeadIconScaled(Image skin, int scale) {
        if (scale <= 1) {
            return extractHeadIcon(skin);
        }
        Image base = extractHeadIcon(skin);
        if (base == null) {
            return null;
        }
        int size = 8 * scale;
        WritableImage out = new WritableImage(size, size);
        PixelReader reader = base.getPixelReader();
        PixelWriter writer = out.getPixelWriter();
        for (int y = 0; y < size; y++) {
            for (int x = 0; x < size; x++) {
                writer.setArgb(x, y, reader.getArgb(x / scale, y / scale));
            }
        }
        return out;
    }

    /** Loads any PNG skin file as an image, or {@code null} when unreadable. */
    public static Image loadImage(Path path) {
        if (path == null || !Files.isRegularFile(path)) {
            return null;
        }
        try (FileInputStream fis = new FileInputStream(path.toFile())) {
            return new Image(fis);
        } catch (IOException ignored) {
            return null;
        }
    }

    public static boolean hasCustomSkin() {
        return Files.isRegularFile(ACTIVE_SKIN);
    }

    public static Path getActiveSkinPath() {
        return ACTIVE_SKIN;
    }

    public static Image loadActiveSkinImage() {
        return loadImage(ACTIVE_SKIN);
    }

    public static void resetSkin() {
        try {
            Files.deleteIfExists(ACTIVE_SKIN);
        } catch (IOException ignored) {
        }
    }
}
