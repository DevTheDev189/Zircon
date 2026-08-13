package com.mcmanager.client.skin;

import javafx.scene.image.Image;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

import java.io.IOException;
import java.io.InputStream;
import java.net.JarURLConnection;
import java.net.URL;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.Comparator;
import java.util.Enumeration;
import java.util.List;
import java.util.Optional;
import java.util.stream.Stream;

/**
 * Loads the launcher's bundled default skins from the classpath {@code skins/}
 * folder (packaged as PNG resources inside the fat jar). Adding a new default
 * skin is just dropping a {@code .png} into {@code client-launcher/src/main/resources/skins/}
 * before building — the gallery and fallback previews pick it up automatically,
 * and every player gets it.
 *
 * <p>Works both in development (exploded resources on disk) and from the shaded
 * jar (entries inside the archive).
 */
public final class BundledSkins {

    private static final Logger log = LoggerFactory.getLogger(BundledSkins.class);

    /** Classpath directory holding the bundled skin PNGs. */
    public static final String DIR = "skins";

    /** Prefix for UI selection keys of bundled skins, e.g. {@code bundled:steve.png}. */
    public static final String KEY_PREFIX = "bundled:";

    private static final List<Skin> CACHE = loadAll();

    private BundledSkins() {
    }

    /** One bundled default skin. */
    public record Skin(String fileName, String label, Image image) {
    }

    /** Selection key used by the UI for a bundled skin (stable across restarts). */
    public static String key(String fileName) {
        return KEY_PREFIX + fileName;
    }

    /** @return true when {@code key} refers to a bundled skin. */
    public static boolean isBundled(String key) {
        return key != null && key.startsWith(KEY_PREFIX);
    }

    /** Every bundled skin, sorted by file name. */
    public static List<Skin> all() {
        return CACHE;
    }

    /** The first bundled skin, used as the fallback preview before a custom skin exists. */
    public static Optional<Skin> fallback() {
        return CACHE.isEmpty() ? Optional.empty() : Optional.of(CACHE.get(0));
    }

    /** Looks up a bundled skin by its UI selection key. */
    public static Optional<Skin> byKey(String key) {
        if (!isBundled(key)) {
            return Optional.empty();
        }
        String fileName = key.substring(KEY_PREFIX.length());
        return CACHE.stream().filter(s -> s.fileName().equals(fileName)).findFirst();
    }

    /** Opens the raw PNG bytes of a bundled skin (for copying to the active skin file). */
    public static InputStream open(Skin skin) {
        return BundledSkins.class.getResourceAsStream("/" + DIR + "/" + skin.fileName());
    }

    private static List<Skin> loadAll() {
        List<String> fileNames = new ArrayList<>();
        try {
            Enumeration<URL> roots = BundledSkins.class.getClassLoader().getResources(DIR);
            while (roots.hasMoreElements()) {
                URL url = roots.nextElement();
                if ("file".equals(url.getProtocol())) {
                    try (Stream<Path> stream = Files.list(Path.of(url.toURI()))) {
                        stream.filter(p -> p.getFileName().toString().toLowerCase().endsWith(".png"))
                                .map(p -> p.getFileName().toString())
                                .forEach(fileNames::add);
                    }
                } else if ("jar".equals(url.getProtocol())) {
                    try (var jar = ((JarURLConnection) url.openConnection()).getJarFile()) {
                        jar.stream()
                                .filter(e -> !e.isDirectory())
                                .filter(e -> e.getName().startsWith(DIR + "/"))
                                .filter(e -> e.getName().toLowerCase().endsWith(".png"))
                                .map(e -> e.getName().substring(DIR.length() + 1))
                                .forEach(fileNames::add);
                    }
                }
            }
        } catch (Exception e) {
            log.warn("Could not enumerate bundled skins: {}", e.getMessage());
        }

        fileNames.sort(Comparator.naturalOrder());
        List<Skin> skins = new ArrayList<>();
        for (String name : fileNames) {
            try (InputStream in = BundledSkins.class.getResourceAsStream("/" + DIR + "/" + name)) {
                if (in == null) {
                    log.warn("Bundled skin {} disappeared while loading, skipping", name);
                    continue;
                }
                Image image = new Image(in);
                if (image.isError() || image.getWidth() < 16 || image.getHeight() < 16) {
                    log.warn("Skipping bundled skin {}: not a usable 16x16+ PNG", name);
                    continue;
                }
                String label = name.substring(0, name.length() - 4); // strip ".png"
                skins.add(new Skin(name, label, image));
            } catch (IOException e) {
                log.warn("Could not read bundled skin {}: {}", name, e.getMessage());
            }
        }
        return List.copyOf(skins);
    }
}
