package com.mcmanager.client.launch;

import java.nio.file.Path;

/**
 * Maps Minecraft versions to the Java major version they require, and locates a
 * {@code java} executable for running installers / the game.
 *
 * <p>Note: the authoritative value is the {@code javaVersion.majorVersion} field
 * of the vanilla version profile JSON; this selector is a fallback for cases
 * where that profile is not available (e.g. picking a JVM to run the loader
 * installer). Actual runtime provisioning is handled by
 * {@link JavaRuntimeResolver}.
 */
public final class JavaRuntimeSelector {

    private JavaRuntimeSelector() {
    }

    /**
     * Required Java major version per Minecraft version:
     *
     * <ul>
     *   <li>{@code < 1.17} — Java 8</li>
     *   <li>{@code 1.17} — Java 16</li>
     *   <li>{@code 1.18 .. 1.20.4} — Java 17</li>
     *   <li>{@code 1.20.5+} — Java 21</li>
     * </ul>
     */
    public static int getRequiredJavaMajorVersion(String minecraftVersion) {
        String[] parts = minecraftVersion == null ? new String[]{} : minecraftVersion.split("\\.");
        if (parts.length < 2) {
            return 17;
        }
        int minor = safeParse(parts[1], 0);
        int patch = parts.length > 2 ? safeParse(parts[2], 0) : 0;

        if (minor < 17) {
            return 8;                                     // MC < 1.17
        } else if (minor == 17) {
            return 16;                                    // MC 1.17
        } else if (minor < 20 || (minor == 20 && patch < 5)) {
            return 17;                                    // MC 1.18 - 1.20.4
        }
        return 21;                                        // MC 1.20.5+
    }

    /**
     * A {@code java} executable for the given major version. Prefers the JVM the
     * launcher itself runs on when it satisfies the requirement; otherwise falls
     * back to a runtime previously provisioned by {@link JavaRuntimeResolver}
     * under the launcher cache.
     */
    public static String getJavaExecutablePath(int majorVersion) {
        int current = Runtime.version().feature();
        if (current >= majorVersion) {
            return javaExecutable(Path.of(System.getProperty("java.home")));
        }
        Path cached = Path.of(System.getProperty("user.home"), ".mcmanager", "launcher", "jdk-" + majorVersion);
        String candidate = javaExecutable(cached);
        if (java.nio.file.Files.isExecutable(Path.of(candidate))) {
            return candidate;
        }
        // Best effort: the resolvers/provisioners will fix this up later.
        return javaExecutable(Path.of(System.getProperty("java.home")));
    }

    /** {@code <javaHome>/bin/java(.exe)} for the current platform. */
    public static String javaExecutable(Path javaHome) {
        String exe = System.getProperty("os.name").toLowerCase().contains("win") ? "java.exe" : "java";
        return javaHome.resolve("bin").resolve(exe).toString();
    }

    private static int safeParse(String value, int fallback) {
        try {
            return Integer.parseInt(value);
        } catch (NumberFormatException e) {
            return fallback;
        }
    }
}
