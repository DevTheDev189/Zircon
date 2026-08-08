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
 * Headless installer for Forge. Downloads the official installer JAR from the
 * Forge Maven and runs:
 *
 * <pre>java -jar forge-&lt;mc&gt;-&lt;ver&gt;-installer.jar --installClient &lt;installDir&gt;</pre>
 *
 * Installer URL: {@code https://maven.minecraftforge.net/net/minecraftforge/forge/}
 * {@code <mc>-<ver>/forge-<mc>-<ver>-installer.jar}
 */
public class ForgeInstaller implements ModLoaderInstaller {

    private static final Logger logger = LoggerFactory.getLogger(ForgeInstaller.class);
    private static final String FORGE_MAVEN_BASE = "https://maven.minecraftforge.net/net/minecraftforge/forge/";
    private static final Duration INSTALL_TIMEOUT = Duration.ofMinutes(15);

    private final Path installerCacheDir;

    public ForgeInstaller() {
        this(Path.of(System.getProperty("user.home"), ".mcmanager", "launcher", ".installers"));
    }

    public ForgeInstaller(Path installerCacheDir) {
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
                    logger.info("Forge {}-{} is already installed.", mcVersion, loaderVersion);
                    return;
                }

                String fullVersion = mcVersion + "-" + loaderVersion;
                String downloadUrl = FORGE_MAVEN_BASE + fullVersion
                        + "/forge-" + fullVersion + "-installer.jar";

                File installerJar = installerCacheDir.resolve("forge-" + fullVersion + "-installer.jar").toFile();
                if (!installerJar.exists()) {
                    logger.info("Downloading Forge installer from {}", downloadUrl);
                    LoaderInstallSupport.downloadIfMissing(downloadUrl, installerJar.toPath());
                }

                logger.info("Running Forge installer headlessly into {}...", installDir);
                List<String> command = List.of(
                        javaExecutablePath,
                        "-jar",
                        installerJar.getAbsolutePath(),
                        "--installClient",
                        installDir.getAbsolutePath()
                );

                int exitCode = ProcessExecutionHelper.runProcess(command, installerJar.getParentFile(),
                        INSTALL_TIMEOUT);
                if (exitCode != 0) {
                    throw new IllegalStateException("Forge installer failed with exit code: " + exitCode);
                }
                if (!isInstalled(mcVersion, loaderVersion, installDir)) {
                    throw new IllegalStateException("Forge installer reported success but produced no version profile");
                }
                logger.info("Forge {} installed successfully.", fullVersion);
            } catch (Exception e) {
                throw new IllegalStateException("Forge installation failed", e);
            }
        });
    }
}
