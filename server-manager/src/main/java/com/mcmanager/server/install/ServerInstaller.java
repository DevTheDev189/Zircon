package com.mcmanager.server.install;

import com.google.gson.Gson;
import com.google.gson.JsonArray;
import com.google.gson.JsonElement;
import com.google.gson.JsonObject;
import com.mcmanager.core.model.ModLoaderInfo;
import com.mcmanager.core.model.ModLoaderType;
import com.mcmanager.core.util.ProcessExecutionHelper;
import com.mcmanager.server.service.ConfigService;
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
import java.util.List;
import java.util.stream.Stream;

/**
 * Ensures the Minecraft server matching the configured mod loader is present in
 * {@code <data>/server}. The wrapper no longer expects the operator to drop a
 * server JAR in manually; instead the correct server is installed on demand:
 *
 * <ul>
 *   <li><b>vanilla</b> — Mojang's server JAR from the version manifest</li>
 *   <li><b>fabric / quilt</b> — the official server launcher JAR from the meta API</li>
 *   <li><b>forge / neoforge</b> — the official installer JAR run headlessly with
 *       {@code --installServer}, which lays out {@code libraries/} and the
 *       {@code win_args.txt} / {@code unix_args.txt} launch file</li>
 * </ul>
 */
public final class ServerInstaller {

    private static final Logger log = LoggerFactory.getLogger(ServerInstaller.class);

    private static final String VERSION_MANIFEST_URL =
            "https://piston-meta.mojang.com/mc/game/version_manifest_v2.json";
    private static final String FABRIC_META_URL = "https://meta.fabricmc.net/v2";
    private static final String QUILT_META_URL = "https://meta.quiltmc.org/v3";
    private static final String FORGE_MAVEN_BASE = "https://maven.minecraftforge.net/net/minecraftforge/forge/";
    private static final String NEOFORGE_MAVEN_BASE = "https://maven.neoforged.net/releases/net/neoforged/neoforge/";
    private static final Duration INSTALL_TIMEOUT = Duration.ofMinutes(15);

    private static final HttpClient HTTP = HttpClient.newBuilder()
            .connectTimeout(Duration.ofSeconds(15))
            .followRedirects(HttpClient.Redirect.NORMAL)
            .build();
    private static final Gson GSON = new Gson();

    private ServerInstaller() {
    }

    // ------------------------------------------------------------------
    // Entry points
    // ------------------------------------------------------------------

    /**
     * @return {@code true} when the server matching the configured loader is
     *         already installed — a {@code server.jar} for vanilla/fabric/quilt,
     *         or a launch args file for the <em>configured</em> loader version
     *         for forge/neoforge (an install for a different loader version does
     *         not count).
     */
    public static boolean isInstalled(ConfigService config) {
        ModLoaderType loader = loaderOf(config);
        if (loader != null && loader.isForgeLike()) {
            return findServerArgsFile(config.getServerDir(), config.getConfig().modLoader.getVersion()) != null;
        }
        return Files.isRegularFile(config.getServerJar());
    }

    /** Instance variant of {@link #isInstalled(ConfigService)}. */
    public static boolean isInstalled(Path serverDir, Path serverJar, ModLoaderInfo loader) {
        ModLoaderType type = ModLoaderType.fromString(loader.getType(), null);
        if (type != null && type.isForgeLike()) {
            return findServerArgsFile(serverDir, loader.getVersion()) != null;
        }
        return Files.isRegularFile(serverJar);
    }

    /**
     * Installs the server for the configured loader if it is not already
     * installed. Safe to call on every start; it is a no-op once installed.
     *
     * @throws IOException when installation fails or required configuration is missing
     */
    public static void ensureServerInstalled(ConfigService config) throws IOException {
        ModLoaderType loader = loaderOf(config);
        if (isInstalled(config)) {
            log.info("Server for {} is already installed", loader == null ? "vanilla" : loader.getId());
        } else {
            log.info("No server installed for loader {} — installing...",
                    loader == null ? "vanilla" : loader.getId());
            switch (loader) {
                case FABRIC -> installFabricLike(config, false);
                case QUILT -> installFabricLike(config, true);
                case FORGE -> installForgeLike(config, false);
                case NEOFORGE -> installForgeLike(config, true);
                default -> installVanilla(config);
            }
            if (!isInstalled(config)) {
                throw new IOException("Server installation finished but the server is still missing");
            }
            log.info("Server installed successfully");
        }

        validateLoaderMatchesConfig(config);
    }

    /**
     * Instance variant of {@link #ensureServerInstalled(ConfigService)}.
     *
     * @param serverDir  the instance's {@code server/} directory (install target)
     * @param serverJar  the instance's {@code server/server.jar}
     * @param cacheDir   shared installer cache dir (under the data dir)
     * @param mcVersion  Minecraft version of the instance
     * @param loader     the (locked) {@link ModLoaderInfo} of the instance
     */
    public static void ensureServerInstalled(Path serverDir, Path serverJar, Path cacheDir,
                                             String mcVersion, ModLoaderInfo loader) throws IOException {
        ModLoaderType type = ModLoaderType.fromString(loader.getType(), null);
        if (isInstalled(serverDir, serverJar, loader)) {
            log.info("Server for {} is already installed", type == null ? "vanilla" : type.getId());
        } else {
            log.info("No server installed for loader {} — installing...",
                    type == null ? "vanilla" : type.getId());
            switch (type) {
                case FABRIC -> installFabricLike(serverJar, mcVersion, loader, false);
                case QUILT -> installFabricLike(serverJar, mcVersion, loader, true);
                case FORGE -> installForgeLike(serverDir, cacheDir, mcVersion, loader, false);
                case NEOFORGE -> installForgeLike(serverDir, cacheDir, mcVersion, loader, true);
                default -> installVanilla(serverJar, mcVersion);
            }
            if (!isInstalled(serverDir, serverJar, loader)) {
                throw new IOException("Server installation finished but the server is still missing");
            }
            log.info("Server installed successfully");
        }

        validateLoaderMatchesConfig(serverDir, mcVersion, loader);
    }

    /**
     * Forge/NeoForge loader versions encode their Minecraft version (e.g.
     * NeoForge 20.4.250 is MC 1.20.4). Refuse to start when
     * {@code config.minecraftVersion} disagrees with the installed server's
     * real Minecraft version — otherwise the BOM served to clients would
     * describe an impossible combination and every client launch would fail.
     */
    private static void validateLoaderMatchesConfig(ConfigService config) throws IOException {
        validateLoaderMatchesConfig(config.getServerDir(), config.getConfig().minecraftVersion,
                config.getConfig().modLoader);
    }

    /**
     * Forge/NeoForge loader versions encode their Minecraft version (e.g.
     * NeoForge 20.4.250 is MC 1.20.4). Refuse to start when the configured
     * Minecraft version disagrees with the installed server's real Minecraft
     * version — otherwise the BOM served to clients would describe an
     * impossible combination and every client launch would fail.
     */
    private static void validateLoaderMatchesConfig(Path serverDir, String mcVersion,
                                                    ModLoaderInfo loader) throws IOException {
        ModLoaderType type = ModLoaderType.fromString(loader.getType(), null);
        if (type == null || !type.isForgeLike()) {
            return;
        }
        Path argsFile = findServerArgsFile(serverDir, loader.getVersion());
        if (argsFile == null) {
            return;
        }
        String installedMcVersion = readFmlMcVersion(argsFile);
        if (installedMcVersion != null && !installedMcVersion.isBlank()
                && mcVersion != null && !mcVersion.isBlank()
                && !installedMcVersion.equals(mcVersion)) {
            throw new IOException("Installed " + type.getId() + " server targets Minecraft "
                    + installedMcVersion + " but config.minecraftVersion is " + mcVersion
                    + ". Set 'minecraftVersion' to " + installedMcVersion
                    + " (or pick a 'modLoader.version' that matches " + mcVersion + ").");
        }
    }

    /** Extracts the {@code --fml.mcVersion <x>} value from a loader args file. */
    private static String readFmlMcVersion(Path argsFile) {
        try {
            List<String> lines = Files.readAllLines(argsFile, StandardCharsets.UTF_8);
            for (int i = 0; i < lines.size(); i++) {
                String line = lines.get(i).trim();
                if (line.equals("--fml.mcVersion")) {
                    if (i + 1 < lines.size()) {
                        return lines.get(i + 1).trim();
                    }
                } else if (line.startsWith("--fml.mcVersion=")) {
                    return line.substring("--fml.mcVersion=".length()).trim();
                }
            }
        } catch (IOException e) {
            log.warn("Could not read loader args file {}: {}", argsFile, e.getMessage());
        }
        return null;
    }

    /**
     * Locates the {@code win_args.txt} / {@code unix_args.txt} launch file the
     * Forge/NeoForge server installer produced for the given loader version
     * (stale installs for other versions are skipped), or {@code null} when
     * absent.
     */
    public static Path findServerArgsFile(Path serverDir, String loaderVersion) {
        String argsFileName = isWindows() ? "win_args.txt" : "unix_args.txt";
        Path librariesDir = serverDir.resolve("libraries");
        if (!Files.isDirectory(librariesDir)) {
            return null;
        }
        try (Stream<Path> files = Files.walk(librariesDir)) {
            return files.filter(Files::isRegularFile)
                    .filter(p -> p.getFileName().toString().equals(argsFileName))
                    .filter(p -> loaderVersion == null || loaderVersion.isBlank()
                            || p.toString().contains(loaderVersion))
                    .findFirst()
                    .orElse(null);
        } catch (IOException e) {
            log.warn("Could not scan {} for server args file", librariesDir, e);
            return null;
        }
    }

    // ------------------------------------------------------------------
    // Vanilla
    // ------------------------------------------------------------------

    private static void installVanilla(ConfigService config) throws IOException {
        installVanilla(config.getServerJar(), config.getConfig().minecraftVersion);
    }

    private static void installVanilla(Path serverJar, String mcVersion) throws IOException {
        JsonObject manifest = GSON.fromJson(get(VERSION_MANIFEST_URL), JsonObject.class);

        String versionUrl = null;
        for (JsonElement v : manifest.getAsJsonArray("versions")) {
            if (mcVersion.equals(v.getAsJsonObject().get("id").getAsString())) {
                versionUrl = v.getAsJsonObject().get("url").getAsString();
                break;
            }
        }
        if (versionUrl == null) {
            throw new IOException("Minecraft version not found in Mojang manifest: " + mcVersion);
        }
        JsonObject versionJson = GSON.fromJson(get(versionUrl), JsonObject.class);
        JsonObject serverDownload = versionJson.getAsJsonObject("downloads").getAsJsonObject("server");
        if (serverDownload == null || !serverDownload.has("url")) {
            throw new IOException("Version " + mcVersion + " has no downloadable server jar");
        }

        log.info("Downloading vanilla server jar for MC {}...", mcVersion);
        download(serverDownload.get("url").getAsString(), serverJar);
    }

    // ------------------------------------------------------------------
    // Fabric / Quilt
    // ------------------------------------------------------------------

    private static void installFabricLike(ConfigService config, boolean quilt) throws IOException {
        installFabricLike(config.getServerJar(), config.getConfig().minecraftVersion,
                config.getConfig().modLoader, quilt);
    }

    private static void installFabricLike(Path serverJar, String mcVersion, ModLoaderInfo loader,
                                          boolean quilt) throws IOException {
        String metaUrl = quilt ? QUILT_META_URL : FABRIC_META_URL;

        String loaderVersion = loader.getVersion();
        if (loaderVersion == null || loaderVersion.isBlank()) {
            loaderVersion = resolveLatestLoaderVersion(mcVersion, metaUrl);
        }

        // The meta API's combined server JAR needs the installer version too.
        JsonArray installers = GSON.fromJson(get(metaUrl + "/versions/installer"), JsonArray.class);
        String installerVersion = installers.get(0).getAsJsonObject().get("version").getAsString();

        String url = metaUrl + "/versions/loader/" + mcVersion + "/" + loaderVersion
                + "/" + installerVersion + "/server/jar";
        log.info("Downloading {} server launcher (MC {}, loader {})...",
                quilt ? "Quilt" : "Fabric", mcVersion, loaderVersion);
        download(url, serverJar);
    }

    private static String resolveLatestLoaderVersion(String mcVersion, String metaUrl) throws IOException {
        JsonArray versions = GSON.fromJson(get(metaUrl + "/versions/loader/" + mcVersion), JsonArray.class);
        if (versions == null || versions.isEmpty()) {
            throw new IOException("No loader versions found for MC " + mcVersion + " at " + metaUrl);
        }
        String version = versions.get(0).getAsJsonObject()
                .getAsJsonObject("loader").get("version").getAsString();
        log.info("Auto-resolved loader version {} for MC {}", version, mcVersion);
        return version;
    }

    // ------------------------------------------------------------------
    // Forge / NeoForge
    // ------------------------------------------------------------------

    private static void installForgeLike(ConfigService config, boolean neoForge) throws IOException {
        installForgeLike(config.getServerDir(), config.getDataDir().resolve(".cache").resolve("installers"),
                config.getConfig().minecraftVersion, config.getConfig().modLoader, neoForge);
    }

    private static void installForgeLike(Path serverDir, Path cacheDir, String mcVersion,
                                         ModLoaderInfo loader, boolean neoForge) throws IOException {
        String loaderVersion = loader.getVersion();
        if (loaderVersion == null || loaderVersion.isBlank()) {
            throw new IOException("Loader version is required to install a "
                    + (neoForge ? "NeoForge" : "Forge") + " server (set 'modLoader.version' in config.json)");
        }

        String fullVersion = neoForge ? loaderVersion : mcVersion + "-" + loaderVersion;
        String mavenBase = neoForge ? NEOFORGE_MAVEN_BASE : FORGE_MAVEN_BASE;
        String artifact = neoForge ? "neoforge" : "forge";
        String downloadUrl = mavenBase + fullVersion + "/" + artifact + "-" + fullVersion + "-installer.jar";

        Files.createDirectories(cacheDir);
        Path installerJar = cacheDir.resolve(artifact + "-" + fullVersion + "-installer.jar");
        if (!Files.isRegularFile(installerJar) || Files.size(installerJar) == 0) {
            log.info("Downloading {} server installer from {}", artifact, downloadUrl);
            download(downloadUrl, installerJar);
        }

        log.info("Running {} server installer headlessly into {}...", artifact, serverDir);
        List<String> command = List.of(
                javaBin(),
                "-jar",
                installerJar.toString(),
                neoForge ? "--install-server" : "--installServer",
                serverDir.toString()
        );
        int exitCode;
        try {
            exitCode = ProcessExecutionHelper.runProcess(command, serverDir.toFile(), INSTALL_TIMEOUT);
        } catch (InterruptedException e) {
            Thread.currentThread().interrupt();
            throw new IOException(artifact + " server installer was interrupted", e);
        }
        if (exitCode != 0) {
            throw new IOException(artifact + " server installer failed with exit code " + exitCode);
        }
    }

    // ------------------------------------------------------------------
    // helpers
    // ------------------------------------------------------------------

    private static ModLoaderType loaderOf(ConfigService config) {
        String type = config.getConfig().modLoader.getType();
        return ModLoaderType.fromString(type, null);
    }

    private static String get(String url) throws IOException {
        HttpRequest request = HttpRequest.newBuilder().uri(URI.create(url)).GET().build();
        HttpResponse<String> response;
        try {
            response = HTTP.send(request, HttpResponse.BodyHandlers.ofString());
        } catch (InterruptedException e) {
            Thread.currentThread().interrupt();
            throw new IOException("Interrupted GET " + url, e);
        }
        if (response.statusCode() / 100 != 2) {
            throw new IOException("GET " + url + " failed: HTTP " + response.statusCode());
        }
        return response.body();
    }

    private static void download(String url, Path target) throws IOException {
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
            Files.createDirectories(target.getParent());
            try (InputStream in = response.body()) {
                Files.copy(in, target, StandardCopyOption.REPLACE_EXISTING);
            }
        } catch (InterruptedException e) {
            Thread.currentThread().interrupt();
            throw new IOException("Interrupted downloading " + url, e);
        }
    }

    private static String javaBin() {
        String exe = isWindows() ? "java.exe" : "java";
        return Path.of(System.getProperty("java.home"), "bin", exe).toString();
    }

    private static boolean isWindows() {
        return System.getProperty("os.name").toLowerCase().contains("win");
    }
}
