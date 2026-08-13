package com.mcmanager.client.sync;

import com.mcmanager.core.model.BillOfMaterials;
import com.mcmanager.core.model.PackEntry;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

import java.io.IOException;
import java.io.InputStream;
import java.net.URI;
import java.net.URLEncoder;
import java.net.http.HttpClient;
import java.net.http.HttpRequest;
import java.net.http.HttpResponse;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.HashSet;
import java.util.List;
import java.util.Set;
import java.util.concurrent.Executors;

/**
 * Downloads every shaderpack/resourcepack advertised in the server's BOM into
 * {@code gameDir/shaderpacks} and {@code gameDir/resourcepacks}, mirroring
 * {@link ModSyncEngine}'s fetch-and-reconcile shape but deliberately simpler:
 *
 * <ul>
 *   <li>No strict/trust-direct abort gating — packs are inert data files, not
 *       executable code, so a verification failure is only logged.</li>
 *   <li>No staging directory — presence in {@code shaderpacks}/{@code resourcepacks}
 *       never activates anything in Minecraft, unlike {@code mods/}.</li>
 *   <li>Reconciliation never deletes a file the caller marks as "keep" (a player's
 *       locally added pack), even if the server no longer lists it.</li>
 * </ul>
 *
 * <p>Activation is never touched here — that's a purely local, per-player choice
 * (see {@code PackSelection}) applied at launch time.
 */
public class PackSyncEngine {

    private static final Logger log = LoggerFactory.getLogger(PackSyncEngine.class);

    private final HttpClient http;

    public PackSyncEngine() {
        this.http = HttpClient.newBuilder()
                .connectTimeout(java.time.Duration.ofSeconds(15))
                .executor(Executors.newVirtualThreadPerTaskExecutor())
                .followRedirects(HttpClient.Redirect.NORMAL)
                .build();
    }

    public interface ProgressListener {
        void onStatus(String message);
    }

    public static class SyncResult {
        public List<String> downloadedShaderpacks = new ArrayList<>();
        public List<String> downloadedResourcepacks = new ArrayList<>();
        public List<String> removedShaderpacks = new ArrayList<>();
        public List<String> removedResourcepacks = new ArrayList<>();
    }

    /**
     * @param bom               already-fetched server BOM (see {@code MainController.fetchBom})
     * @param keepShaderpacks   local shaderpack filenames to never prune even if absent from the BOM
     * @param keepResourcepacks local resourcepack filenames to never prune even if absent from the BOM
     */
    public SyncResult sync(BillOfMaterials bom, String serverBaseUrl, Path gameDir,
                           Set<String> keepShaderpacks, Set<String> keepResourcepacks,
                           ProgressListener listener) {
        String base = serverBaseUrl.endsWith("/")
                ? serverBaseUrl.substring(0, serverBaseUrl.length() - 1)
                : serverBaseUrl;

        SyncResult result = new SyncResult();
        syncBucket(base, gameDir.resolve("shaderpacks"), bom.getShaderpacks(), "/files/shaderpacks/",
                keepShaderpacks, result.downloadedShaderpacks, result.removedShaderpacks, listener);
        syncBucket(base, gameDir.resolve("resourcepacks"), bom.getResourcepacks(), "/files/resourcepacks/",
                keepResourcepacks, result.downloadedResourcepacks, result.removedResourcepacks, listener);
        return result;
    }

    private void syncBucket(String base, Path dir, List<PackEntry> packs, String urlPrefix,
                            Set<String> keep, List<String> downloaded, List<String> removed,
                            ProgressListener listener) {
        try {
            Files.createDirectories(dir);
        } catch (IOException e) {
            log.warn("Could not create pack directory {}: {}", dir, e.getMessage());
            return;
        }

        Set<String> wanted = new HashSet<>();
        for (PackEntry pack : packs) {
            wanted.add(pack.getFilename());
        }

        for (PackEntry pack : packs) {
            Path target = dir.resolve(pack.getFilename());
            try {
                if (HashVerifier.matches(target, pack)) {
                    continue;
                }
                listener.onStatus("Downloading " + pack.getFilename() + "...");
                download(base + urlPrefix + urlEncode(pack.getFilename()), target);
                downloaded.add(pack.getFilename());
            } catch (IOException | InterruptedException e) {
                if (e instanceof InterruptedException) {
                    Thread.currentThread().interrupt();
                }
                log.warn("Pack sync failed for {}: {}", pack.getFilename(), e.getMessage());
            }
        }

        try (var stream = Files.list(dir)) {
            for (Path file : stream.filter(p -> HashVerifier.isZip(p.getFileName().toString())).toList()) {
                String name = file.getFileName().toString();
                if (!wanted.contains(name) && !keep.contains(name)) {
                    Files.deleteIfExists(file);
                    removed.add(name);
                    log.info("Pruned pack no longer offered by server: {}", name);
                }
            }
        } catch (IOException e) {
            log.warn("Could not reconcile pack directory {}: {}", dir, e.getMessage());
        }
    }

    private void download(String url, Path target) throws IOException, InterruptedException {
        HttpRequest request = HttpRequest.newBuilder().uri(URI.create(url)).GET().build();
        HttpResponse<InputStream> response = http.send(request, HttpResponse.BodyHandlers.ofInputStream());
        if (response.statusCode() / 100 != 2) {
            throw new IOException("Download " + url + " failed: HTTP " + response.statusCode());
        }
        try (InputStream in = response.body(); var out = Files.newOutputStream(target)) {
            in.transferTo(out);
        }
    }

    private static String urlEncode(String value) {
        return URLEncoder.encode(value, StandardCharsets.UTF_8);
    }
}
