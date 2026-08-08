package com.mcmanager.client.installer;

import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

import java.io.IOException;
import java.io.InputStream;
import java.net.URI;
import java.net.http.HttpClient;
import java.net.http.HttpRequest;
import java.net.http.HttpResponse;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.StandardCopyOption;
import java.time.Duration;
import java.util.Comparator;
import java.util.List;
import java.util.Optional;
import java.util.stream.Stream;

/**
 * Shared plumbing for the Forge / NeoForge headless installers:
 *
 * <ul>
 *   <li>downloading the installer JAR from Maven with caching</li>
 *   <li>preparing a {@code .minecraft}-style install directory (the vanilla
 *       version profile + client jar + {@code launcher_profiles.json} that the
 *       official installers require before they will run)</li>
 *   <li>locating the version profile JSON and the patched client JAR the
 *       installers produce</li>
 * </ul>
 */
public final class LoaderInstallSupport {

    private static final Logger log = LoggerFactory.getLogger(LoaderInstallSupport.class);

    private static final HttpClient HTTP = HttpClient.newBuilder()
            .connectTimeout(Duration.ofSeconds(15))
            .followRedirects(HttpClient.Redirect.NORMAL)
            .build();

    private LoaderInstallSupport() {
    }

    /**
     * Downloads {@code url} to {@code target}, skipping the download when the
     * file already exists and is non-empty.
     */
    public static void downloadIfMissing(String url, Path target) throws IOException, InterruptedException {
        if (Files.isRegularFile(target) && Files.size(target) > 0) {
            return;
        }
        Files.createDirectories(target.getParent());
        HttpRequest request = HttpRequest.newBuilder()
                .uri(URI.create(url))
                .timeout(Duration.ofMinutes(10))
                .GET()
                .build();
        try {
            HttpResponse<InputStream> response = HTTP.send(request, HttpResponse.BodyHandlers.ofInputStream());
            if (response.statusCode() / 100 != 2) {
                throw new IOException("Download " + url + " failed: HTTP " + response.statusCode());
            }
            try (InputStream in = response.body()) {
                Files.copy(in, target, StandardCopyOption.REPLACE_EXISTING);
            }
        } catch (InterruptedException e) {
            Thread.currentThread().interrupt();
            throw new IOException("Interrupted downloading " + url, e);
        }
    }

    // ------------------------------------------------------------------
    // Install directory preparation
    // ------------------------------------------------------------------

    /**
     * The official installers refuse to run against a directory that does not
     * look like an existing Minecraft installation. This creates the minimum
     * layout they accept:
     *
     * <pre>
     * installDir/
     *   launcher_profiles.json
     *   versions/&lt;mc&gt;/&lt;mc&gt;.json
     *   versions/&lt;mc&gt;/&lt;mc&gt;.jar
     * </pre>
     *
     * @param vanillaVersionJson the vanilla version profile JSON already fetched
     *                           from Mojang's version manifest
     * @param vanillaClientJar   the vanilla client JAR already downloaded
     */
    public static void prepareInstallDir(Path installDir, String mcVersion,
                                         Path vanillaVersionJson, Path vanillaClientJar) throws IOException {
        Path versionsDir = installDir.resolve("versions").resolve(mcVersion);
        Files.createDirectories(versionsDir);

        if (!Files.exists(installDir.resolve("launcher_profiles.json"))) {
            Files.writeString(installDir.resolve("launcher_profiles.json"),
                    "{\"profiles\":{},\"settings\":{},\"version\":3}", StandardCharsets.UTF_8);
        }
        if (!Files.exists(versionsDir.resolve(mcVersion + ".json")) && Files.isRegularFile(vanillaVersionJson)) {
            Files.copy(vanillaVersionJson, versionsDir.resolve(mcVersion + ".json"));
        }
        if (!Files.exists(versionsDir.resolve(mcVersion + ".jar")) && Files.isRegularFile(vanillaClientJar)) {
            Files.copy(vanillaClientJar, versionsDir.resolve(mcVersion + ".jar"));
        }
    }

    /**
     * The loader's artifact directory inside an install dir, e.g.
     * {@code libraries/net/neoforged/neoforge/<loaderVersion>} (NeoForge) or
     * {@code libraries/net/minecraftforge/forge/<mc>-<loaderVersion>} (Forge).
     */
    public static Path loaderArtifactDir(Path installDir, com.mcmanager.core.model.ModLoaderType loaderType,
                                         String mcVersion, String loaderVersion) {
        if (loaderType == com.mcmanager.core.model.ModLoaderType.NEOFORGE) {
            return installDir.resolve("libraries/net/neoforged/neoforge").resolve(loaderVersion);
        }
        return installDir.resolve("libraries/net/minecraftforge/forge").resolve(mcVersion + "-" + loaderVersion);
    }

    /**
     * Lists JARs directly inside {@code dir} whose file name ends with
     * {@code suffix} (e.g. {@code "-universal.jar"}, {@code "-client.jar"}).
     */
    public static List<Path> findJars(Path dir, String suffix) {
        if (!Files.isDirectory(dir)) {
            return List.of();
        }
        try (Stream<Path> files = Files.list(dir)) {
            return files.filter(Files::isRegularFile)
                    .filter(p -> p.getFileName().toString().endsWith(suffix))
                    .toList();
        } catch (IOException e) {
            log.warn("Could not scan {} for *{} jars", dir, suffix, e);
            return List.of();
        }
    }

    // ------------------------------------------------------------------
    // Locating installer output
    // ------------------------------------------------------------------

    /**
     * Locates the loader's version profile JSON in a prepared install dir,
     * e.g. {@code versions/neoforge-20.4.250/neoforge-20.4.250.json}.
     *
     * <p>The profile directory name is not predictable across loader generations
     * (NeoForge pre-1.20.2 used {@code <mc>-forge-<ver>}, later builds use
     * {@code neoforge-<ver>}), so any non-vanilla version directory with a
     * matching {@code <name>.json} is accepted.
     *
     * @return the profile JSON path, or {@link Optional#empty()} if not installed.
     */
    public static Optional<Path> findVersionProfileJson(Path installDir, String mcVersion) {
        Path versionsDir = installDir.resolve("versions");
        if (!Files.isDirectory(versionsDir)) {
            return Optional.empty();
        }
        try (Stream<Path> dirs = Files.list(versionsDir)) {
            return dirs.filter(Files::isDirectory)
                    .filter(dir -> !dir.getFileName().toString().equals(mcVersion))
                    .map(dir -> dir.resolve(dir.getFileName() + ".json"))
                    .filter(Files::isRegularFile)
                    .max(Comparator.comparingLong(p -> {
                        try {
                            return Files.size(p);
                        } catch (IOException e) {
                            return 0L;
                        }
                    }));
        } catch (IOException e) {
            log.warn("Could not scan version profiles in {}", versionsDir, e);
            return Optional.empty();
        }
    }

    /**
     * Locates the patched client JAR the installer produced, e.g.
     * {@code libraries/net/neoforged/neoforge/20.4.250/neoforge-20.4.250-client.jar}.
     *
     * <p>Modern Forge/NeoForge put the runnable game jar in the libraries
     * directory with a {@code -client} classifier; some older Forge builds write
     * {@code versions/<id>/<id>.jar} instead, which is checked first.
     *
     * @return the patched client JAR, or {@link Optional#empty()} when absent.
     */
    public static Optional<Path> findPatchedClientJar(Path installDir, String profileId) {
        Path legacy = installDir.resolve("versions").resolve(profileId).resolve(profileId + ".jar");
        if (Files.isRegularFile(legacy)) {
            return Optional.of(legacy);
        }
        Path librariesDir = installDir.resolve("libraries");
        if (!Files.isDirectory(librariesDir)) {
            return Optional.empty();
        }
        try (Stream<Path> jars = Files.walk(librariesDir)) {
            return jars.filter(Files::isRegularFile)
                    .filter(p -> p.getFileName().toString().endsWith("-client.jar"))
                    .findFirst();
        } catch (IOException e) {
            log.warn("Could not scan libraries in {}", librariesDir, e);
            return Optional.empty();
        }
    }
}
