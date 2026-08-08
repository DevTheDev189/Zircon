package com.mcmanager.client.launch;

import com.mcmanager.core.model.ModLoaderType;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.condition.EnabledIfEnvironmentVariable;
import org.junit.jupiter.api.io.TempDir;

import java.io.IOException;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.List;
import java.util.stream.Stream;

import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertTrue;

/**
 * End-to-end smoke test against a real loader installation. Enabled only when
 * the {@code MC_REAL_INSTALL} environment variable points at a directory
 * produced by an official installer run (a {@code --install-client} target
 * containing {@code versions/<id>/<id>.json} and {@code libraries/}).
 *
 * <p>The install directory name must follow the launcher's convention,
 * {@code <loader>-<mc>-<loaderVersion>} (e.g. {@code neoforge-1.20.4-20.4.250}
 * or {@code neoforge-26.2-26.2.0.48-beta}). Run with:
 * <pre>
 * MC_REAL_INSTALL=C:/path/to/install ./gradlew :client-launcher:test
 * </pre>
 */
@EnabledIfEnvironmentVariable(named = "MC_REAL_INSTALL", matches = ".+")
class RealForgeInstallSmokeTest {

    @TempDir
    Path tempDir;

    @Test
    void resolvesRealInstallation() throws Exception {
        String installSrc = System.getenv("MC_REAL_INSTALL");
        Path sourceInstall = Path.of(installSrc);

        // Parse the launcher's install-dir name: <loader>-<mc>-<loaderVersion>.
        String[] parts = sourceInstall.getFileName().toString().split("-", 3);
        assertTrue(parts.length == 3, "install dir must be <loader>-<mc>-<loaderVersion>: "
                + sourceInstall.getFileName());
        String loader = parts[0];
        String mcVersion = parts[1];
        String loaderVersion = parts[2];
        assertTrue("forge".equals(loader) || "neoforge".equals(loader), "unsupported loader: " + loader);

        Path cacheDir = tempDir.resolve("cache");
        Path installDir = cacheDir.resolve("install").resolve(sourceInstall.getFileName().toString());
        copyRecursive(sourceInstall, installDir);

        Path vanillaJson = installDir.resolve("versions").resolve(mcVersion).resolve(mcVersion + ".json");
        Path vanillaJar = installDir.resolve("versions").resolve(mcVersion).resolve(mcVersion + ".jar");
        assertTrue(Files.isRegularFile(vanillaJson), "fixture missing vanilla version json");
        assertTrue(Files.isRegularFile(vanillaJar), "fixture missing vanilla client jar");

        List<Path> classpath = new ArrayList<>();
        ForgeLaunchResolver.ForgeLaunchData data = new ForgeLaunchResolver().resolve(
                cacheDir, mcVersion,
                "neoforge".equals(loader) ? ModLoaderType.NEOFORGE : ModLoaderType.FORGE,
                loaderVersion, vanillaJson, vanillaJar, tempDir.resolve("natives"), classpath);

        // Main class from the real profile (BootstrapLauncher for < 1.26,
        // net.neoforged.fml.startup.Client for the 26+ FancyModLoader era).
        assertTrue(data.mainClass().contains("Launcher") || data.mainClass().contains("fml.startup."),
                "unexpected mainClass: " + data.mainClass());

        // JVM args: library directory substituted; the vanilla "-cp ${classpath}"
        // template stripped (present on the 1.20.x-era profiles).
        assertTrue(data.jvmArgs().stream()
                        .anyMatch(a -> a.startsWith("-DlibraryDirectory=" + cacheDir.resolve("libraries"))),
                "libraryDirectory not resolved: " + data.jvmArgs());
        assertFalse(data.jvmArgs().contains("-cp"), "duplicate -cp not stripped: " + data.jvmArgs());
        assertFalse(data.jvmArgs().contains("${classpath}"), "unresolved classpath token: " + data.jvmArgs());

        // Game args: loader args + vanilla player args.
        assertTrue(data.gameArgs().contains("--username"), "player args missing: " + data.gameArgs());

        // Classpath: the loader's own jars (universal + patched game jar) must
        // NOT be on -cp — FML would mistake that for an IDE dev run and demand a
        // Minecraft-Dists manifest attribute. Instead they are staged into the
        // unified libraries dir for GameLocator.locateProductionMinecraft, which
        // resolves them via -DlibraryDirectory.
        assertTrue(classpath.stream().noneMatch(p -> p.getFileName().toString().endsWith("-universal.jar")
                        || p.getFileName().toString().startsWith("minecraft-client-patched-")
                        || p.getFileName().toString().endsWith("-client.jar")),
                "loader jars must not be on the classpath: " + classpath);

        Path librariesDir = cacheDir.resolve("libraries");
        assertTrue(Files.isRegularFile(librariesDir.resolve("net/neoforged/neoforge/")
                        .resolve(loaderVersion).resolve("neoforge-" + loaderVersion + "-universal.jar")),
                "universal jar not staged into libraries dir");

        // The patched game jar: either the 26+ minecraft-client-patched artifact
        // or the classic neoforge-<ver>-client.jar.
        boolean gameJarStaged = Files.isRegularFile(librariesDir.resolve("net/neoforged/minecraft-client-patched")
                .resolve(loaderVersion).resolve("minecraft-client-patched-" + loaderVersion + ".jar"))
                || Files.isRegularFile(librariesDir.resolve("net/neoforged/neoforge")
                        .resolve(loaderVersion).resolve("neoforge-" + loaderVersion + "-client.jar"));
        assertTrue(gameJarStaged, "patched game jar not staged into libraries dir");

        // Loader profile libraries still land on the classpath.
        assertTrue(classpath.stream().anyMatch(p -> p.toString().contains("fancymodloader")
                        || p.toString().contains("modlauncher")
                        || p.toString().contains("securejarhandler")),
                "loader library missing from classpath");
    }

    private static void copyRecursive(Path source, Path target) throws IOException {
        try (Stream<Path> stream = Files.walk(source)) {
            for (Path path : stream.toList()) {
                Path dest = target.resolve(source.relativize(path).toString());
                if (Files.isDirectory(path)) {
                    Files.createDirectories(dest);
                } else {
                    Files.createDirectories(dest.getParent());
                    Files.copy(path, dest);
                }
            }
        }
    }
}
