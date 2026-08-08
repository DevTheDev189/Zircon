package com.mcmanager.client.launch;

import com.mcmanager.client.installer.ForgeInstaller;
import com.mcmanager.client.installer.LoaderInstallSupport;
import com.mcmanager.client.installer.ModLoaderInstaller;
import com.mcmanager.client.installer.NeoForgeInstaller;
import com.mcmanager.client.profile.LibrarySpec;
import com.mcmanager.client.profile.VersionProfile;
import com.mcmanager.client.profile.VersionProfileResolver;
import com.mcmanager.core.model.ModLoaderType;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

import java.io.File;
import java.io.IOException;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.StandardCopyOption;
import java.util.ArrayList;
import java.util.HashMap;
import java.util.List;
import java.util.Map;

/**
 * End-to-end launch preparation for Forge and NeoForge:
 *
 * <ol>
 *   <li>Runs the official headless installer into a dedicated install dir
 *       (downloads the installer JAR from Maven when needed).</li>
 *   <li>Parses the generated {@code versions/&lt;id&gt;/&lt;id&gt;.json} profile.</li>
 *   <li>Resolves the {@code inheritsFrom} chain against the vanilla profile the
 *       launcher already fetched.</li>
 *   <li>Copies the loader's libraries into the unified launcher libraries dir
 *       and adds them to the classpath, replacing the vanilla client JAR with
 *       the patched {@code -client.jar} the installer produced.</li>
 *   <li>Resolves the profile's JVM and game arguments (with {@code ${token}}
 *       substitution).</li>
 * </ol>
 */
public class ForgeLaunchResolver {

    private static final Logger log = LoggerFactory.getLogger(ForgeLaunchResolver.class);

    private final VersionProfileResolver profileResolver;

    public ForgeLaunchResolver() {
        this(new VersionProfileResolver());
    }

    public ForgeLaunchResolver(VersionProfileResolver profileResolver) {
        this.profileResolver = profileResolver;
    }

    /** Everything the launcher needs beyond the vanilla resolution. */
    public record ForgeLaunchData(
            String mainClass,
            List<String> jvmArgs,
            List<String> gameArgs) {
    }

    /**
     * @param cacheDir           the launcher cache (holds {@code versions/}, {@code libraries/}, {@code install/})
     * @param mcVersion          Minecraft version, e.g. {@code "1.20.4"}
     * @param loaderType         FORGE or NEOFORGE
     * @param loaderVersion      loader version, e.g. {@code "47.2.0"} or {@code "20.4.250"}
     * @param vanillaVersionJson path to the vanilla version profile JSON (already downloaded)
     * @param vanillaClientJar   path to the vanilla client JAR (already downloaded)
     * @param nativesDir         extracted natives dir (used only for token substitution)
     * @param classpath          the in-progress classpath; the patched client JAR
     *                           replaces the vanilla client JAR and the loader
     *                           libraries are appended
     * @throws IOException when installation or profile resolution fails
     */
    public ForgeLaunchData resolve(Path cacheDir, String mcVersion, ModLoaderType loaderType,
                                   String loaderVersion, Path vanillaVersionJson,
                                   Path vanillaClientJar, Path nativesDir, List<Path> classpath)
            throws IOException {
        Path installDir = cacheDir.resolve("install")
                .resolve(loaderType.getId() + "-" + mcVersion + "-" + loaderVersion);

        // --- 1. headless installation ---
        LoaderInstallSupport.prepareInstallDir(installDir, mcVersion, vanillaVersionJson, vanillaClientJar);
        ModLoaderInstaller installer = loaderType == ModLoaderType.NEOFORGE
                ? new NeoForgeInstaller(cacheDir.resolve(".installers"))
                : new ForgeInstaller(cacheDir.resolve(".installers"));
        installer.install(mcVersion, loaderVersion, installDir.toFile(),
                JavaRuntimeSelector.javaExecutable(Path.of(System.getProperty("java.home"))))
                .join();

        // --- 2. parse the generated version profile ---
        Path profileJson = LoaderInstallSupport.findVersionProfileJson(installDir, mcVersion)
                .orElseThrow(() -> new IOException("Loader profile JSON not found after installation in "
                        + installDir));
        VersionProfile root = profileResolver.parseProfile(profileJson.toFile());
        log.info("Parsed loader profile '{}' (mainClass={}, inheritsFrom={})",
                root.getId(), root.getMainClass(), root.getInheritsFrom());

        // The loader version encodes its Minecraft version (e.g. NeoForge
        // 20.4.250 = MC 1.20.4). Refuse to launch when the server manifest
        // disagrees — a mismatched game jar + vanilla libraries crashes with
        // confusing NoSuchMethodError/classloader errors.
        String profileMcVersion = root.getInheritsFrom();
        if (profileMcVersion != null && !profileMcVersion.isBlank()
                && !profileMcVersion.equals(mcVersion)) {
            throw new IOException("Loader version " + loaderVersion + " targets Minecraft "
                    + profileMcVersion + " but the server manifest declares " + mcVersion
                    + ". Fix 'minecraftVersion' and 'modLoader.version' in the server config"
                    + " so they describe the same Minecraft version.");
        }

        // --- 3. resolve the inheritance chain (loader -> vanilla) ---
        List<VersionProfile> chain = profileResolver.resolveChain(root, parentId -> {
            Path parentJson = parentId.equals(mcVersion) && Files.isRegularFile(vanillaVersionJson)
                    ? vanillaVersionJson
                    : cacheDir.resolve("versions").resolve(parentId).resolve(parentId + ".json");
            try {
                return profileResolver.parseProfile(parentJson.toFile());
            } catch (IOException e) {
                throw new RuntimeException("Could not parse inherited profile " + parentJson, e);
            }
        });

        // --- 4. libraries ---
        Path librariesDir = cacheDir.resolve("libraries");
        for (LibrarySpec lib : profileResolver.mergedLibraries(chain)) {
            String artifactPath = lib.getArtifactPath();
            if (artifactPath == null) {
                log.warn("Skipping library with unparseable coordinate: {}", lib.getName());
                continue;
            }
            Path target = librariesDir.resolve(artifactPath).toAbsolutePath().normalize();
            if (Files.isRegularFile(target) && Files.size(target) > 0) {
                // Present on disk: add it to the classpath unless the vanilla
                // library loop already did (avoids duplicates while keeping
                // loader-only libs on the classpath across repeated launches).
                boolean alreadyOnClasspath = classpath.stream()
                        .anyMatch(p -> p.toAbsolutePath().normalize().equals(target));
                if (!alreadyOnClasspath) {
                    classpath.add(target);
                }
                continue;
            }
            stageLibrary(installDir, librariesDir, lib, target, classpath);
        }

        // --- 5. loader artifacts ---
        // The loader's own jars must NOT go on the classpath. FML's
        // RequiredSystemFiles treats game + loader classes found on -cp as a
        // merged IDE/dev environment and then runs NeoForgeDevDistCleaner, which
        // demands a Minecraft-Dists manifest attribute that production jars
        // don't carry (that attribute is generated by NeoGradle only) — aborting
        // with "NeoForge dev environment Minecraft jar does not have a
        // Minecraft-Dists attribute".
        //
        // Instead the artifacts are staged into the unified libraries dir under
        // their maven-relative paths so GameLocator.locateProductionMinecraft
        // can find them via -DlibraryDirectory (that path does not run the dist
        // cleaner). This covers both eras:
        //   *-universal.jar              -> the loader mod container
        //   *-client.jar                 -> patched game jar (Forge/NeoForge < 1.26)
        //   minecraft-client-patched.jar -> patched game jar (NeoForge 26+)
        //   client-<mcp>-srg/-extra.jar  -> split client partials (1.20.x-era)
        Path loaderArtifactDir = LoaderInstallSupport.loaderArtifactDir(installDir, loaderType,
                mcVersion, loaderVersion);
        for (Path universalJar : LoaderInstallSupport.findJars(loaderArtifactDir, "-universal.jar")) {
            stageLoaderArtifact(librariesDir, installDir, universalJar);
        }

        Path patchedGameJar = LoaderInstallSupport.findJars(loaderArtifactDir, "-client.jar").stream()
                .findFirst()
                .orElseGet(() -> minecraftClientPatched(installDir, loaderVersion)
                        .orElseGet(() -> LoaderInstallSupport.findPatchedClientJar(installDir, root.getId())
                                .orElse(null)));
        if (patchedGameJar != null) {
            stageLoaderArtifact(librariesDir, installDir, patchedGameJar);
            log.info("Staged patched game jar {} into libraries", patchedGameJar.getFileName());
        } else {
            log.warn("No patched game jar found — production locator will use the vanilla client jar");
        }

        // 1.20.x-era: locateProductionMinecraft assembles the game from the
        // split srg + extra client partials in the libraries dir.
        stageMinecraftClientPartials(installDir, librariesDir);

        // --- 6. arguments ---
        Map<String, String> tokens = new HashMap<>();
        tokens.put("library_directory", librariesDir.toString());
        tokens.put("classpath_separator", File.pathSeparator);
        tokens.put("version_name", root.getId());
        tokens.put("launcher_name", "mcmanager");
        tokens.put("launcher_version", "1.0.0");
        tokens.put("natives_directory", nativesDir != null ? nativesDir.toString() : "");

        List<String> jvmArgs = profileResolver.resolveJvmArguments(chain, tokens);
        // The vanilla profile contributes "-cp ${classpath}"; the launcher
        // injects the classpath itself (the runner adds -cp uniformly for every
        // loader), so drop the template pair to avoid a duplicate -cp.
        for (int i = 0; i + 1 < jvmArgs.size(); i++) {
            if ("-cp".equals(jvmArgs.get(i)) && "${classpath}".equals(jvmArgs.get(i + 1))) {
                jvmArgs.remove(i + 1);
                jvmArgs.remove(i);
                break;
            }
        }

        // This launcher always auto-connects the player to a server, so the
        // "is_quick_play_multiplayer" feature is enabled (the game arg template
        // ${quickPlayMultiplayer} is filled in at launch time with host:port).
        List<String> gameArgs = profileResolver.resolveGameArguments(chain, tokens,
                java.util.Set.of("is_quick_play_multiplayer"));

        String mainClass = root.getMainClass();
        if (mainClass == null || mainClass.isBlank()) {
            throw new IOException("Profile " + root.getId() + " declares no mainClass");
        }

        log.info("Forge/NeoForge launch prepared: mainClass={}, jvmArgs={}, gameArgs={}",
                mainClass, jvmArgs.size(), gameArgs.size());
        return new ForgeLaunchData(mainClass, jvmArgs, gameArgs);
    }

    /**
     * Stages the 1.20.x-era split client partials ({@code client-&lt;mcp&gt;-srg.jar}
     * and {@code client-&lt;mcp&gt;-extra.jar}) from the installer output into the
     * unified libraries dir. FML's {@code locateProductionMinecraft} assembles
     * the game from these when no {@code minecraft-client-patched} artifact
     * exists for the loader version.
     */
    private void stageMinecraftClientPartials(Path installDir, Path librariesDir) {
        Path clientDir = installDir.resolve("libraries/net/minecraft/client");
        if (!Files.isDirectory(clientDir)) {
            return;
        }
        try (var mcpDirs = Files.list(clientDir)) {
            for (Path mcpDir : mcpDirs.toList()) {
                for (String suffix : List.of("-srg.jar", "-extra.jar")) {
                    for (Path partial : LoaderInstallSupport.findJars(mcpDir, suffix)) {
                        stageLoaderArtifact(librariesDir, installDir, partial);
                    }
                }
            }
        } catch (IOException e) {
            log.warn("Could not scan client partials in {}: {}", clientDir, e.getMessage());
        }
    }

    /**
     * NeoForge 26+ publishes the patched game jar as a dedicated artifact:
     * {@code libraries/net/neoforged/minecraft-client-patched/<ver>/minecraft-client-patched-<ver>.jar}.
     */
    private static java.util.Optional<Path> minecraftClientPatched(Path installDir, String loaderVersion) {
        Path patched = installDir.resolve("libraries/net/neoforged/minecraft-client-patched")
                .resolve(loaderVersion)
                .resolve("minecraft-client-patched-" + loaderVersion + ".jar");
        return Files.isRegularFile(patched) ? java.util.Optional.of(patched) : java.util.Optional.empty();
    }

    /**
     * Copies a loader artifact (e.g. the {@code -universal.jar}) from the
     * installer output into the unified libraries dir under its maven-relative
     * path, returning the staged copy. Locators that scan the library
     * directory (and FML's classpath scanner) can both find it there.
     */
    private Path stageLoaderArtifact(Path librariesDir, Path installDir, Path artifact) {
        try {
            Path relative = installDir.resolve("libraries").relativize(artifact);
            Path target = librariesDir.resolve(relative).toAbsolutePath().normalize();
            if (!Files.isRegularFile(target) || Files.size(target) == 0) {
                Files.createDirectories(target.getParent());
                Files.copy(artifact, target, StandardCopyOption.REPLACE_EXISTING);
            }
            return target;
        } catch (IOException e) {
            log.warn("Could not stage loader artifact {}: {}", artifact, e.getMessage());
            return null;
        }
    }

    /**
     * Ensures a loader library is present in the unified libraries dir — copied
     * from the installer output when available, otherwise downloaded from the
     * profile's artifact URL — and appends it to the classpath.
     */
    private void stageLibrary(Path installDir, Path librariesDir, LibrarySpec lib,
                              Path target, List<Path> classpath) {
        try {
            Path installed = installDir.resolve("libraries").resolve(lib.getArtifactPath());
            if (Files.isRegularFile(installed)) {
                Files.createDirectories(target.getParent());
                Files.copy(installed, target, StandardCopyOption.REPLACE_EXISTING);
            } else if (lib.getDownloadUrl() != null) {
                LoaderInstallSupport.downloadIfMissing(lib.getDownloadUrl(), target);
            } else {
                log.warn("Library {} has no download URL and was not installed — skipping", lib.getName());
                return;
            }
            classpath.add(target);
        } catch (IOException | InterruptedException e) {
            if (e instanceof InterruptedException) {
                Thread.currentThread().interrupt();
            }
            log.warn("Could not stage library {}: {}", lib.getName(), e.getMessage());
        }
    }
}
