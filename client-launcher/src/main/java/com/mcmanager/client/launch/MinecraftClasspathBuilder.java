package com.mcmanager.client.launch;

import com.google.gson.Gson;
import com.google.gson.JsonArray;
import com.google.gson.JsonElement;
import com.google.gson.JsonObject;
import com.mcmanager.core.model.ModLoaderInfo;
import com.mcmanager.core.model.ModLoaderType;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

import java.io.IOException;
import java.io.InputStream;
import java.net.URI;
import java.net.http.HttpClient;
import java.net.http.HttpRequest;
import java.net.http.HttpResponse;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.StandardCopyOption;
import java.util.ArrayList;
import java.util.HashSet;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Locale;
import java.util.Map;
import java.util.Set;
import java.util.concurrent.ConcurrentHashMap;
import java.util.concurrent.CountDownLatch;
import java.util.concurrent.Executors;
import java.util.concurrent.atomic.AtomicInteger;
import java.util.zip.ZipEntry;
import java.util.zip.ZipInputStream;

/**
 * Resolves the launch classpath for a Minecraft version + mod loader:
 * downloads the client JAR, all libraries, natives, the asset index + objects,
 * and the loader's libraries (Fabric / Quilt meta profiles; NeoForge / Forge are
 * best-effort via the loader JAR URL from the BOM).
 *
 * <p>Everything is cached under {@code ~/.mcmanager/launcher} so subsequent
 * launches are offline after the first sync.
 */
public class MinecraftClasspathBuilder {

    private static final Logger log = LoggerFactory.getLogger(MinecraftClasspathBuilder.class);

    public static final String VERSION_MANIFEST_URL = "https://piston-meta.mojang.com/mc/game/version_manifest_v2.json";
    public static final String FABRIC_META_URL = "https://meta.fabricmc.net/v2";
    public static final String QUILT_META_URL = "https://meta.quiltmc.org/v3";

    private final Path cacheDir;
    private final HttpClient http;
    private final Gson gson = new Gson();

    public MinecraftClasspathBuilder() {
        this(Path.of(System.getProperty("user.home"), ".mcmanager", "launcher"));
    }

    public MinecraftClasspathBuilder(Path cacheDir) {
        this.cacheDir = cacheDir;
        this.http = HttpClient.newBuilder()
                .connectTimeout(java.time.Duration.ofSeconds(15))
                .followRedirects(HttpClient.Redirect.NORMAL)
                .build();
    }

    public Path getCacheDir() {
        return cacheDir;
    }

    /** Everything needed to launch the game process. */
    public record LaunchData(
            String mainClass,
            String classpath,
            String assetIndexId,
            String versionName,
            Path assetsDir,
            Path nativesDir,
            Path javaHome,
            List<String> jvmArgs,
            List<String> gameArgs) {
    }

    /**
     * Resolves the full launch environment for a BOM's Minecraft version + loader.
     * May download hundreds of megabytes on first run (libraries + assets).
     */
    public LaunchData resolve(String mcVersion, ModLoaderInfo loader, int requiredJavaMajor)
            throws IOException, InterruptedException {
        Files.createDirectories(cacheDir);

        // --- locate the version ---
        JsonObject versionJson = resolveVersionJson(mcVersion);
        String versionId = versionJson.get("id").getAsString();

        Path librariesDir = cacheDir.resolve("libraries");
        Path nativesDir = cacheDir.resolve("natives").resolve(sanitize(versionId + "-" + loaderName(loader)));
        Files.createDirectories(nativesDir);

        List<Path> classpath = new ArrayList<>();

        // --- vanilla client jar ---
        String clientUrl = versionJson.getAsJsonObject("downloads")
                .getAsJsonObject("client").get("url").getAsString();
        Path clientJar = cacheDir.resolve("versions").resolve(versionId).resolve(versionId + ".jar");
        downloadIfMissing(clientUrl, clientJar);
        classpath.add(clientJar);

        // --- vanilla libraries + natives ---
        // Keyed by "group:artifact" so a loader-provided version of a shared
        // dependency (e.g. ASM, pulled in by Fabric Loader) replaces the vanilla
        // one instead of both landing on the classpath — Fabric's Knot loader
        // refuses to start if it sees duplicate ASM classes.
        Map<String, Path> libraryByArtifact = new LinkedHashMap<>();
        JsonArray libraries = versionJson.getAsJsonArray("libraries");
        for (JsonElement element : libraries) {
            JsonObject lib = element.getAsJsonObject();
            if (!rulesAllow(lib)) {
                continue;
            }
            JsonObject downloads = lib.getAsJsonObject("downloads");
            if (downloads == null) {
                continue;
            }
            String name = lib.get("name").getAsString();
            if (downloads.has("artifact")) {
                JsonObject artifact = downloads.getAsJsonObject("artifact");
                Path jar = librariesDir.resolve(artifactPath(artifact, name, null));
                downloadIfMissing(artifact.get("url").getAsString(), jar);
                libraryByArtifact.put(groupAndArtifact(name), jar);
            }
            if (downloads.has("classifiers")) {
                JsonObject classifiers = downloads.getAsJsonObject("classifiers");
                String classifier = pickNativesClassifier(classifiers);
                if (classifier != null) {
                    JsonObject natives = classifiers.getAsJsonObject(classifier);
                    Path jar = librariesDir.resolve(artifactPath(natives, name, classifier));
                    downloadIfMissing(natives.get("url").getAsString(), jar);
                    extractNatives(jar, nativesDir);
                }
            }
        }

        // --- loader ---
        String mainClass = versionJson.get("mainClass").getAsString();
        String loaderType = loader != null && loader.getType() != null ? loader.getType() : "";
        List<String> jvmArgs = new ArrayList<>();
        List<String> gameArgs = new ArrayList<>();
        switch (loaderType.toLowerCase(Locale.ROOT)) {
            case "fabric" -> {
                mainClass = resolveLoaderProfile(mcVersion, loader,
                        FABRIC_META_URL + "/versions/loader/%s/%s/profile/json", libraryByArtifact, librariesDir);
                classpath.addAll(libraryByArtifact.values());
            }
            case "quilt" -> {
                mainClass = resolveLoaderProfile(mcVersion, loader,
                        QUILT_META_URL + "/versions/loader/%s/%s/profile/json", libraryByArtifact, librariesDir);
                classpath.addAll(libraryByArtifact.values());
            }
            case "neoforge", "forge" -> {
                if (loader.getVersion() == null || loader.getVersion().isBlank()) {
                    throw new IOException("Loader version is required for " + loaderType
                            + " (set 'modLoader.version' in the server BOM)");
                }
                classpath.addAll(libraryByArtifact.values());
                // Install the loader headlessly, parse the generated version
                // profile and merge its libraries/arguments into the launch.
                Path vanillaJson = cacheDir.resolve("versions")
                        .resolve(sanitize(mcVersion)).resolve(mcVersion + ".json");
                ForgeLaunchResolver.ForgeLaunchData forge = new ForgeLaunchResolver().resolve(
                        cacheDir, mcVersion, ModLoaderType.fromString(loaderType),
                        loader.getVersion(), vanillaJson, clientJar, nativesDir, classpath);
                mainClass = forge.mainClass();
                jvmArgs.addAll(forge.jvmArgs());
                gameArgs.addAll(forge.gameArgs());
            }
            default -> {
                classpath.addAll(libraryByArtifact.values());
                log.info("No loader configured — launching vanilla");
            }
        }

        // --- asset index + objects ---
        JsonObject assetIndex = versionJson.getAsJsonObject("assetIndex");
        String assetIndexId = assetIndex.get("id").getAsString();
        Path assetsDir = cacheDir.resolve("assets");
        Path indexFile = assetsDir.resolve("indexes").resolve(assetIndexId + ".json");
        downloadIfMissing(assetIndex.get("url").getAsString(), indexFile);
        downloadAssets(indexFile, assetsDir);

        String classpathStr = String.join(System.getProperty("path.separator"),
                classpath.stream().map(Path::toString).toList());

        int javaMajor = versionJson.getAsJsonObject("javaVersion") != null
                ? versionJson.getAsJsonObject("javaVersion").get("majorVersion").getAsInt()
                : requiredJavaMajor;

        Path javaHome = new JavaRuntimeResolver(cacheDir, http).resolve(javaMajor);
        log.info("Launch data ready: version={}, loader={}, classpath entries={}, java={}",
                versionId, loaderType, classpath.size(), javaHome);

        return new LaunchData(mainClass, classpathStr, assetIndexId, versionId,
                assetsDir, nativesDir, javaHome, jvmArgs, gameArgs);
    }

    // ------------------------------------------------------------------
    // Version resolution
    // ------------------------------------------------------------------

    private JsonObject resolveVersionJson(String mcVersion) throws IOException, InterruptedException {
        Path manifestFile = cacheDir.resolve("version_manifest_v2.json");
        downloadIfMissing(VERSION_MANIFEST_URL, manifestFile);
        JsonObject manifest = gson.fromJson(Files.readString(manifestFile), JsonObject.class);

        String url = null;
        for (JsonElement v : manifest.getAsJsonArray("versions")) {
            if (mcVersion.equals(v.getAsJsonObject().get("id").getAsString())) {
                url = v.getAsJsonObject().get("url").getAsString();
                break;
            }
        }
        if (url == null) {
            throw new IOException("Minecraft version not found in manifest: " + mcVersion);
        }

        Path versionFile = cacheDir.resolve("versions").resolve(sanitize(mcVersion)).resolve(mcVersion + ".json");
        downloadIfMissing(url, versionFile);
        JsonObject versionJson = gson.fromJson(Files.readString(versionFile), JsonObject.class);
        if (versionJson.get("id") == null) {
            throw new IOException("Invalid version JSON at " + versionFile);
        }
        return versionJson;
    }

    private String resolveLoaderProfile(String mcVersion, ModLoaderInfo loader, String urlTemplate,
                                        Map<String, Path> libraryByArtifact, Path librariesDir)
            throws IOException, InterruptedException {
        String loaderVersion = loader.getVersion();
        if (loaderVersion == null || loaderVersion.isBlank()) {
            // Be forgiving: auto-resolve the newest stable loader for this MC version
            // when the BOM left the version empty.
            loaderVersion = resolveLatestLoaderVersion(mcVersion, loader.getType());
            if (loaderVersion == null) {
                throw new IOException("Loader version is empty in the BOM for " + loader.getType()
                        + " and could not be auto-resolved from " + ("quilt".equalsIgnoreCase(loader.getType())
                        ? QUILT_META_URL : FABRIC_META_URL));
            }
            log.info("Auto-resolved {} loader version {} for MC {}", loader.getType(), loaderVersion, mcVersion);
        }
        String url = urlTemplate.formatted(mcVersion, loaderVersion);
        String profileJson = get(url);
        JsonObject profile = gson.fromJson(profileJson, JsonObject.class);

        for (JsonElement element : profile.getAsJsonArray("libraries")) {
            JsonObject lib = element.getAsJsonObject();
            if (!rulesAllow(lib)) {
                continue;
            }
            String name = lib.get("name").getAsString();
            String repo = lib.has("url") ? lib.get("url").getAsString() : "https://maven.fabricmc.net/";
            Path jar = librariesDir.resolve(mavenPath(name));
            downloadIfMissing(repo + mavenPath(name), jar);
            // Loader-provided libraries take precedence over vanilla ones with
            // the same group:artifact (e.g. a newer ASM required by Fabric Loader).
            libraryByArtifact.put(groupAndArtifact(name), jar);
        }
        String mainClass = profile.get("mainClass").getAsString();
        log.info("{} loader profile resolved: mainClass={}, {} libraries",
                loader.getType(), mainClass, libraryByArtifact.size());
        return mainClass;
    }

    /**
     * Queries the loader meta API for the newest stable loader version of a
     * Minecraft version (responses are ordered newest-first).
     *
     * @return the loader version string, or {@code null} if none exists.
     */
    private String resolveLatestLoaderVersion(String mcVersion, String loaderType)
            throws IOException, InterruptedException {
        String base = "quilt".equalsIgnoreCase(loaderType) ? QUILT_META_URL : FABRIC_META_URL;
        String body = get(base + "/versions/loader/" + mcVersion);
        JsonArray versions = gson.fromJson(body, JsonArray.class);
        if (versions == null || versions.isEmpty()) {
            return null;
        }
        JsonObject first = versions.get(0).getAsJsonObject();
        if (first.has("loader") && first.getAsJsonObject("loader").has("version")) {
            return first.getAsJsonObject("loader").get("version").getAsString();
        }
        return null;
    }

    // ------------------------------------------------------------------
    // Libraries & natives helpers
    // ------------------------------------------------------------------

    /**
     * Maven coordinate "group:artifact:version[:classifier]" -> "group:artifact[:classifier]"
     * (the version is dropped so a loader-provided version of a dependency can replace
     * the vanilla one, but the classifier is kept so per-OS native jars — which modern
     * version manifests list as their own "group:artifact:version:natives-xxx" library
     * entries with a real "downloads.artifact" — don't collide with the main artifact
     * or with each other).
     */
    private static String groupAndArtifact(String mavenCoordinate) {
        String[] parts = mavenCoordinate.split(":");
        if (parts.length >= 4) {
            return parts[0] + ":" + parts[1] + ":" + parts[3];
        }
        return parts.length >= 2 ? parts[0] + ":" + parts[1] : mavenCoordinate;
    }

    private boolean rulesAllow(JsonObject lib) {
        if (!lib.has("rules")) {
            return true;
        }
        boolean allow = false;
        for (JsonElement ruleEl : lib.getAsJsonArray("rules")) {
            JsonObject rule = ruleEl.getAsJsonObject();
            boolean applies = osMatches(rule.has("os") ? rule.getAsJsonObject("os") : null);
            if (applies) {
                allow = "allow".equals(rule.get("action").getAsString());
            }
        }
        return allow;
    }

    private boolean osMatches(JsonObject os) {
        if (os == null) {
            return true;
        }
        String osName = System.getProperty("os.name").toLowerCase(Locale.ROOT);
        String osTarget = os.has("name") ? os.get("name").getAsString() : null;
        if (osTarget != null) {
            boolean match = switch (osTarget) {
                case "windows" -> osName.contains("win");
                case "linux" -> osName.contains("linux");
                case "osx" -> osName.contains("mac");
                default -> false;
            };
            if (!match) {
                return false;
            }
        }
        if (os.has("arch")) {
            String arch = os.get("arch").getAsString();
            String actual = System.getProperty("os.arch").toLowerCase(Locale.ROOT);
            boolean match = arch.equals("x86") ? actual.contains("86") && !actual.contains("64")
                    : arch.equals("x86_64") ? actual.contains("64")
                    : arch.equals("arm64") ? actual.contains("aarch64") || actual.contains("arm64")
                    : false;
            if (!match) {
                return false;
            }
        }
        return true;
    }

    private String pickNativesClassifier(JsonObject classifiers) {
        String osName = System.getProperty("os.name").toLowerCase(Locale.ROOT);
        String arch = System.getProperty("os.arch").toLowerCase(Locale.ROOT);
        String osKey = osName.contains("win") ? "natives-windows"
                : osName.contains("linux") ? "natives-linux"
                : "natives-macos";
        if (classifiers.has(osKey)) {
            return osKey;
        }
        String osArchKey = osKey + (arch.contains("64") ? "-64" : "-32");
        if (classifiers.has(osArchKey)) {
            return osArchKey;
        }
        return null;
    }

    private String artifactPath(JsonObject artifact, String name, String classifier) {
        if (artifact.has("path")) {
            return artifact.get("path").getAsString();
        }
        return mavenPath(name, classifier);
    }

    private String mavenPath(String name) {
        return mavenPath(name, null);
    }

    private String mavenPath(String name, String classifier) {
        String[] parts = name.split(":");
        String group = parts[0].replace('.', '/');
        String artifact = parts[1];
        String version = parts.length > 2 ? parts[2] : "unknown";
        String file = artifact + "-" + version + (classifier != null ? "-" + classifier : "") + ".jar";
        return group + "/" + artifact + "/" + version + "/" + file;
    }

    private void extractNatives(Path jar, Path nativesDir) throws IOException {
        Set<String> existing = new HashSet<>();
        try (var stream = Files.list(nativesDir)) {
            stream.forEach(p -> existing.add(p.getFileName().toString()));
        }
        try (ZipInputStream zip = new ZipInputStream(Files.newInputStream(jar))) {
            ZipEntry entry;
            while ((entry = zip.getNextEntry()) != null) {
                if (entry.isDirectory()) {
                    continue;
                }
                String name = entry.getName();
                if (!name.contains("/") && !existing.contains(name)
                        && (name.endsWith(".dll") || name.endsWith(".so") || name.endsWith(".dylib"))) {
                    Files.copy(zip, nativesDir.resolve(name), StandardCopyOption.REPLACE_EXISTING);
                }
            }
        }
    }

    // ------------------------------------------------------------------
    // Assets
    // ------------------------------------------------------------------

    private void downloadAssets(Path indexFile, Path assetsDir) throws IOException, InterruptedException {
        JsonObject index = gson.fromJson(Files.readString(indexFile), JsonObject.class);
        if (!index.has("objects")) {
            return;
        }
        JsonObject objects = index.getAsJsonObject("objects");
        Path objectsDir = assetsDir.resolve("objects");

        // Compute which assets are missing or size-mismatched on disk.
        Set<String> missing = ConcurrentHashMap.newKeySet();
        for (Map.Entry<String, JsonElement> entry : objects.entrySet()) {
            JsonObject obj = entry.getValue().getAsJsonObject();
            String hash = obj.get("hash").getAsString();
            long size = obj.get("size").getAsLong();
            Path target = objectsDir.resolve(hash.substring(0, 2)).resolve(hash);
            if (!Files.isRegularFile(target) || Files.size(target) != size) {
                missing.add(hash);
            }
        }
        log.info("Assets: {} total, {} to download", objects.size(), missing.size());

        // Download with bounded concurrency and retries until stable. The game
        // cannot render without its resources, so we fail loudly instead of
        // leaving gaps that cause a black screen.
        int pass = 0;
        while (!missing.isEmpty() && pass < 4) {
            pass++;
            CountDownLatch latch = new CountDownLatch(missing.size());
            AtomicInteger failures = new AtomicInteger();
            try (var executor = Executors.newFixedThreadPool(8)) {
                for (String hash : List.copyOf(missing)) {
                    executor.submit(() -> {
                        try {
                            downloadAsset(objectsDir, hash);
                            missing.remove(hash);
                        } catch (Exception e) {
                            failures.incrementAndGet();
                        } finally {
                            latch.countDown();
                        }
                    });
                }
                latch.await();
            }
            log.info("Asset download pass {} complete ({} failed)", pass, failures.get());
            if (failures.get() == 0) {
                break;
            }
            Thread.sleep(1000L * pass); // back off before retrying
        }

        if (!missing.isEmpty()) {
            throw new IOException("Could not download " + missing.size() + " of " + objects.size()
                    + " asset files (e.g. " + missing.iterator().next() + "). Minecraft cannot render "
                    + "without its resources — check the network and retry the launch.");
        }
    }

    private void downloadAsset(Path objectsDir, String hash) throws IOException {
        String url = "https://resources.download.minecraft.net/"
                + hash.substring(0, 2) + "/" + hash;
        Path target = objectsDir.resolve(hash.substring(0, 2)).resolve(hash);
        HttpRequest request = HttpRequest.newBuilder()
                .uri(URI.create(url))
                .timeout(java.time.Duration.ofSeconds(60))
                .GET()
                .build();
        HttpResponse<InputStream> response;
        try {
            response = http.send(request, HttpResponse.BodyHandlers.ofInputStream());
        } catch (InterruptedException e) {
            Thread.currentThread().interrupt();
            throw new IOException("Interrupted downloading asset " + hash, e);
        }
        if (response.statusCode() / 100 != 2) {
            throw new IOException("Asset download failed: HTTP " + response.statusCode()
                    + " for " + hash);
        }
        Files.createDirectories(target.getParent());
        try (InputStream in = response.body()) {
            Files.copy(in, target, StandardCopyOption.REPLACE_EXISTING);
        }
    }

    // ------------------------------------------------------------------
    // HTTP helpers
    // ------------------------------------------------------------------

    private String get(String url) throws IOException, InterruptedException {
        HttpRequest request = HttpRequest.newBuilder().uri(URI.create(url)).GET().build();
        HttpResponse<String> response = http.send(request, HttpResponse.BodyHandlers.ofString());
        if (response.statusCode() / 100 != 2) {
            throw new IOException("GET " + url + " failed: HTTP " + response.statusCode());
        }
        return response.body();
    }

    private void downloadIfMissing(String url, Path target) throws IOException {
        if (Files.isRegularFile(target) && Files.size(target) > 0) {
            return;
        }
        Files.createDirectories(target.getParent());
        HttpRequest request = HttpRequest.newBuilder().uri(URI.create(url)).GET().build();
        try {
            HttpResponse<InputStream> response = http.send(request, HttpResponse.BodyHandlers.ofInputStream());
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

    private String loaderName(ModLoaderInfo loader) {
        return loader != null && loader.getType() != null ? loader.getType() : "vanilla";
    }

    private String sanitize(String s) {
        return s.replaceAll("[^A-Za-z0-9._-]", "_");
    }
}
