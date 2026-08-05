package com.mcmanager.client.launch;

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
import java.util.zip.ZipEntry;
import java.util.zip.ZipInputStream;

/**
 * Ensures a Java runtime with the major version required by the Minecraft
 * version is available. Uses the current JVM when possible; otherwise downloads
 * a Temurin JDK from Adoptium (plan task 3.4).
 */
public class JavaRuntimeResolver {

    private static final Logger log = LoggerFactory.getLogger(JavaRuntimeResolver.class);

    private static final String ADOPTIUM_URL =
            "https://api.adoptium.net/v3/binary/latest/%d/ga/%s/%s/jdk/hotspot/normal/eclipse";

    private final Path cacheDir;
    private final HttpClient http;

    public JavaRuntimeResolver(Path cacheDir, HttpClient http) {
        this.cacheDir = cacheDir;
        this.http = http;
    }

    /** @return a {@code java.home} whose major version is >= {@code requiredMajor}. */
    public Path resolve(int requiredMajor) throws IOException {
        int current = Runtime.version().feature();
        if (current >= requiredMajor) {
            log.info("Using current JVM (Java {}) for required Java {}", current, requiredMajor);
            return Path.of(System.getProperty("java.home"));
        }

        Path jdkDir = cacheDir.resolve("jdk-" + requiredMajor);
        Path javaExecutable = osName().contains("win") ? jdkDir.resolve("bin/java.exe") : jdkDir.resolve("bin/java");
        if (Files.isExecutable(javaExecutable)) {
            log.info("Using cached Java runtime at {}", jdkDir);
            return jdkDir;
        }

        log.info("Downloading Java {} runtime from Adoptium (this can take a few minutes)...", requiredMajor);
        String url = ADOPTIUM_URL.formatted(requiredMajor, osName(), osArch());
        Path archive = cacheDir.resolve("jdk-" + requiredMajor + ".zip");
        download(url, archive);
        extract(archive, jdkDir);
        if (!Files.isExecutable(javaExecutable)) {
            // Adoptium archives contain a single top-level folder; look one level down.
            try (var stream = Files.list(jdkDir)) {
                Path nested = stream.filter(Files::isDirectory).findFirst().orElse(null);
                if (nested != null) {
                    return nested;
                }
            }
            throw new IOException("Java runtime downloaded but java executable not found under " + jdkDir);
        }
        return jdkDir;
    }

    private void download(String url, Path target) throws IOException {
        HttpRequest request = HttpRequest.newBuilder().uri(URI.create(url)).GET().build();
        try {
            HttpResponse<InputStream> response = http.send(request, HttpResponse.BodyHandlers.ofInputStream());
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

    private void extract(Path archive, Path targetDir) throws IOException {
        Files.createDirectories(targetDir);
        try (ZipInputStream zip = new ZipInputStream(Files.newInputStream(archive))) {
            ZipEntry entry;
            while ((entry = zip.getNextEntry()) != null) {
                Path out = targetDir.resolve(entry.getName());
                if (entry.isDirectory()) {
                    Files.createDirectories(out);
                } else {
                    Files.createDirectories(out.getParent());
                    Files.copy(zip, out, StandardCopyOption.REPLACE_EXISTING);
                }
            }
        }
        Files.deleteIfExists(archive);
    }

    private static String osName() {
        String os = System.getProperty("os.name").toLowerCase();
        if (os.contains("win")) return "windows";
        if (os.contains("mac")) return "mac";
        return "linux";
    }

    private static String osArch() {
        String arch = System.getProperty("os.arch").toLowerCase();
        if (arch.contains("aarch64") || arch.contains("arm64")) return "aarch64";
        return "x64";
    }
}
