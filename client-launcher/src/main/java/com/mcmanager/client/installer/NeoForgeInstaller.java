package com.mcmanager.client.installer;

import com.mcmanager.core.util.ProcessExecutionHelper;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

import java.io.File;
import java.nio.file.Path;
import java.time.Duration;
import java.util.List;
import java.util.concurrent.CompletableFuture;

/**
 * Headless installer for NeoForge. Downloads the official installer JAR from
 * the NeoForge Maven and runs:
 *
 * <pre>java -jar neoforge-&lt;ver&gt;-installer.jar --install-client &lt;installDir&gt;</pre>
 *
 * Installer URL: {@code https://maven.neoforged.net/releases/net/neoforged/neoforge/}
 * {@code <ver>/neoforge-<ver>-installer.jar}
 */
public class NeoForgeInstaller implements ModLoaderInstaller {

    private static final Logger logger = LoggerFactory.getLogger(NeoForgeInstaller.class);
    private static final String NEOFORGE_MAVEN_BASE = "https://maven.neoforged.net/releases/net/neoforged/neoforge/";
    private static final Duration INSTALL_TIMEOUT = Duration.ofMinutes(15);

    private final Path installerCacheDir;

    public NeoForgeInstaller() {
        this(Path.of(System.getProperty("user.home"), ".mcmanager", "launcher", ".installers"));
    }

    public NeoForgeInstaller(Path installerCacheDir) {
        this.installerCacheDir = installerCacheDir;
    }

    @Override
    public boolean isInstalled(String mcVersion, String loaderVersion, File installDir) {
        return LoaderInstallSupport.findVersionProfileJson(installDir.toPath(), mcVersion).isPresent();
    }

    @Override
    public CompletableFuture<Void> install(String mcVersion, String loaderVersion, File installDir,
                                           String javaExecutablePath) {
        return CompletableFuture.runAsync(() -> {
            try {
                if (isInstalled(mcVersion, loaderVersion, installDir)) {
                    logger.info("NeoForge {} for MC {} is already installed.", loaderVersion, mcVersion);
                    return;
                }

                String downloadUrl = NEOFORGE_MAVEN_BASE + loaderVersion
                        + "/neoforge-" + loaderVersion + "-installer.jar";

                File installerJar = installerCacheDir.resolve("neoforge-" + loaderVersion + "-installer.jar").toFile();
                if (!installerJar.exists()) {
                    logger.info("Downloading NeoForge installer from {}", downloadUrl);
                    LoaderInstallSupport.downloadIfMissing(downloadUrl, installerJar.toPath());
                }

                logger.info("Running NeoForge installer headlessly into {}...", installDir);
                List<String> command = List.of(
                        javaExecutablePath,
                        "-jar",
                        installerJar.getAbsolutePath(),
                        "--install-client",
                        installDir.getAbsolutePath()
                );

                int exitCode = ProcessExecutionHelper.runProcess(command, installerJar.getParentFile(),
                        INSTALL_TIMEOUT);
                if (exitCode != 0) {
                    throw new IllegalStateException("NeoForge installer failed with exit code: " + exitCode);
                }
                if (!isInstalled(mcVersion, loaderVersion, installDir)) {
                    throw new IllegalStateException("NeoForge installer reported success but produced no version profile");
                }
                logger.info("NeoForge {} installed successfully.", loaderVersion);
            } catch (Exception e) {
                throw new IllegalStateException("NeoForge installation failed", e);
            }
        });
    }
}
