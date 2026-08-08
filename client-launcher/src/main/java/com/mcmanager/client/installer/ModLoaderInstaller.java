package com.mcmanager.client.installer;

import java.io.File;
import java.util.concurrent.CompletableFuture;

/**
 * Strategy for installing a mod loader into a game directory by downloading the
 * official installer JAR from Maven and executing it headlessly.
 *
 * <p>After {@link #install} completes, the target directory contains a
 * {@code versions/<profile-id>/<profile-id>.json} file that the launcher parses
 * to build the JVM arguments, classpath and main class.
 */
public interface ModLoaderInstaller {

    /**
     * Checks whether the loader's version profile JSON already exists for this
     * combination of Minecraft version and loader version.
     *
     * @param mcVersion     Minecraft version, e.g. {@code "1.20.1"}
     * @param loaderVersion loader version, e.g. {@code "47.2.0"} (Forge) or {@code "20.4.250"} (NeoForge)
     * @param installDir    the {@code .minecraft}-style directory the loader installs into
     */
    boolean isInstalled(String mcVersion, String loaderVersion, File installDir);

    /**
     * Downloads the loader installer JAR (if not cached) and runs it headlessly
     * against {@code installDir}. The returned future completes when the
     * installer process exits; it completes exceptionally on download/execution
     * failure or a non-zero exit code.
     *
     * @param javaExecutablePath absolute path to a {@code java} executable used to run the installer
     */
    CompletableFuture<Void> install(String mcVersion, String loaderVersion, File installDir,
                                    String javaExecutablePath);
}
