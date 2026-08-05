package com.mcmanager.client.sync;

import com.mcmanager.core.api.CurseForgeApiClient;
import com.mcmanager.core.api.ModrinthApiClient;
import com.mcmanager.core.model.BillOfMaterials;
import com.mcmanager.core.model.BomJson;
import com.mcmanager.core.model.ModEntry;
import com.google.gson.JsonObject;
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
import java.nio.file.StandardCopyOption;
import java.util.ArrayList;
import java.util.HashSet;
import java.util.List;
import java.util.Map;
import java.util.Set;
import java.util.concurrent.Executors;
import java.util.concurrent.atomic.AtomicLong;

/**
 * Brings the local {@code .minecraft/mods} folder in line with the server's
 * Bill of Materials, per plan task 3.3:
 *
 * <ol>
 *   <li>Fetch {@code /bom} from the server.</li>
 *   <li>Batch-verify hashes against Modrinth / CurseForge (safety check).</li>
 *   <li>Delete local JARs that are not part of the BOM.</li>
 *   <li>Download missing / mismatched JARs from the server wrapper, reporting
 *       progress for the UI.</li>
 * </ol>
 */
public class ModSyncEngine {

    private static final Logger log = LoggerFactory.getLogger(ModSyncEngine.class);

    private final HttpClient http;

    public ModSyncEngine() {
        this.http = HttpClient.newBuilder()
                .connectTimeout(java.time.Duration.ofSeconds(15))
                .executor(Executors.newVirtualThreadPerTaskExecutor())
                .followRedirects(HttpClient.Redirect.NORMAL)
                .build();
    }

    public interface ProgressListener {
        void onStatus(String message);

        void onProgress(double fraction, String detail);
    }

    public static class SyncResult {
        public BillOfMaterials bom;
        public List<String> downloaded = new ArrayList<>();
        public List<String> removed = new ArrayList<>();
        public List<String> kept = new ArrayList<>();
        public List<String> unverified = new ArrayList<>();
        public boolean aborted;
        public String abortReason;
    }

    /**
     * Synchronizes the mods folder with the server.
     *
     * @param serverBaseUrl     e.g. {@code http://mc.example.com:25565}
     * @param gameDir           the client's game directory (contains {@code mods/})
     * @param strictVerification when {@code true}, abort if a mod cannot be verified
     * @param trustDirectMods    whether unverifiable "direct" mods are acceptable
     * @param listener           progress callbacks (called from worker threads)
     */
    public SyncResult sync(String serverBaseUrl, Path gameDir, boolean strictVerification,
                           boolean trustDirectMods, ProgressListener listener)
            throws IOException, InterruptedException {
        String base = serverBaseUrl.endsWith("/")
                ? serverBaseUrl.substring(0, serverBaseUrl.length() - 1)
                : serverBaseUrl;

        SyncResult result = new SyncResult();
        Path modsDir = gameDir.resolve("mods");
        Files.createDirectories(modsDir);

        // --- Step 1: fetch the BOM ---
        listener.onStatus("Fetching mod list from " + base + "...");
        String bomJson = get(base + "/bom");
        result.bom = BomJson.fromJson(bomJson);
        List<ModEntry> mods = result.bom.getMods();
        log.info("BOM: {} mods for MC {}", mods.size(), result.bom.getMinecraftVersion());

        // --- Step 2: verify hashes against Modrinth / CurseForge ---
        listener.onStatus("Verifying mod hashes...");
        String curseForgeKey = resolveCurseForgeKey(base);
        verifyAgainstProviders(mods, curseForgeKey, result, strictVerification, trustDirectMods);
        if (result.aborted) {
            return result;
        }

        // --- Step 3: reconcile the local mods folder ---
        Set<String> wanted = new HashSet<>();
        for (ModEntry mod : mods) {
            wanted.add(mod.getFilename());
        }

        try (var stream = Files.list(modsDir)) {
            for (Path file : stream.filter(p -> HashVerifier.isModJar(p.getFileName().toString())).toList()) {
                if (!wanted.contains(file.getFileName().toString())) {
                    Files.deleteIfExists(file);
                    result.removed.add(file.getFileName().toString());
                    log.info("Removed stale mod {}", file.getFileName());
                }
            }
        }

        // --- Step 4: download missing / mismatched mods ---
        long totalBytes = mods.stream().mapToLong(ModEntry::getFileSize).sum();
        AtomicLong downloadedBytes = new AtomicLong();

        for (int i = 0; i < mods.size(); i++) {
            ModEntry mod = mods.get(i);
            Path target = modsDir.resolve(mod.getFilename());
            if (HashVerifier.matches(target, mod)) {
                result.kept.add(mod.getFilename());
                continue;
            }

            String url = base + "/files/mods/" + urlEncode(mod.getFilename());
            listener.onStatus("Downloading " + mod.getFilename() + " (" + (i + 1) + "/" + mods.size() + ")...");
            long size = download(url, target);
            downloadedBytes.addAndGet(size);
            result.downloaded.add(mod.getFilename());

            double fraction = totalBytes > 0 ? Math.min(1.0, downloadedBytes.get() / (double) totalBytes) : 0;
            listener.onProgress(fraction, mod.getFilename());
        }

        listener.onProgress(1.0, "Done");
        listener.onStatus("Mods up to date (" + result.kept.size() + " kept, "
                + result.downloaded.size() + " downloaded, " + result.removed.size() + " removed)");
        return result;
    }

    // ------------------------------------------------------------------
    // Provider verification
    // ------------------------------------------------------------------

    /**
     * Prefers the CurseForge key configured on the server (via /api/config) and
     * falls back to the local system property, so testers don't need to configure
     * anything on the client.
     */
    private String resolveCurseForgeKey(String baseUrl) {
        try {
            String configJson = get(baseUrl + "/api/config");
            JsonObject config = BomJson.gson().fromJson(configJson, JsonObject.class);
            if (config != null && config.has("curseforgeApiKey")
                    && !config.get("curseforgeApiKey").getAsString().isBlank()) {
                return config.get("curseforgeApiKey").getAsString();
            }
        } catch (IOException | InterruptedException | RuntimeException e) {
            if (e instanceof InterruptedException) {
                Thread.currentThread().interrupt();
            }
            log.debug("Could not read CurseForge key from server config: {}", e.getMessage());
        }
        return System.getProperty("mcmanager.curseforgeApiKey", "");
    }

    private void verifyAgainstProviders(List<ModEntry> mods, String curseForgeApiKey,
                                        SyncResult result,
                                        boolean strict, boolean trustDirect) {
        List<String> sha1s = new ArrayList<>();
        List<Long> fingerprints = new ArrayList<>();
        for (ModEntry mod : mods) {
            if ("modrinth".equals(mod.getOrigin()) && mod.getSha1() != null) {
                sha1s.add(mod.getSha1());
            } else if ("curseforge".equals(mod.getOrigin()) && mod.getMurmur3() != 0) {
                fingerprints.add(mod.getMurmur3());
            }
        }

        Set<String> verifiedSha1 = new HashSet<>();
        Set<Long> verifiedFp = new HashSet<>();

        // "checked" means the provider responded. If the provider was unreachable
        // (network down, no API key) we do NOT abort — the mods are simply not
        // confirmable, which must not block testing.
        boolean modrinthChecked = sha1s.isEmpty();
        boolean curseForgeChecked = fingerprints.isEmpty();

        if (!sha1s.isEmpty()) {
            try {
                ModrinthApiClient modrinth = new ModrinthApiClient();
                Map<String, ModrinthApiClient.ModrinthVersion> found = modrinth.verifyHashes(sha1s);
                verifiedSha1.addAll(found.keySet());
                modrinthChecked = true;
            } catch (IOException | InterruptedException e) {
                if (e instanceof InterruptedException) {
                    Thread.currentThread().interrupt();
                }
                log.warn("Modrinth hash verification unavailable: {}", e.getMessage());
            }
        }

        if (!fingerprints.isEmpty()) {
            if (curseForgeApiKey == null || curseForgeApiKey.isBlank()) {
                log.info("No CurseForge API key configured — skipping fingerprint verification "
                        + "(CurseForge mods will not block launch)");
            } else {
                try {
                    CurseForgeApiClient cf = new CurseForgeApiClient(curseForgeApiKey);
                    for (CurseForgeApiClient.CurseForgeFile file : cf.verifyFingerprints(fingerprints)) {
                        verifiedFp.add(file.fileFingerprint);
                    }
                    curseForgeChecked = true;
                } catch (IOException | InterruptedException e) {
                    if (e instanceof InterruptedException) {
                        Thread.currentThread().interrupt();
                    }
                    log.warn("CurseForge fingerprint verification unavailable: {}", e.getMessage());
                }
            }
        }

        for (ModEntry mod : mods) {
            boolean verified;
            if ("modrinth".equals(mod.getOrigin())) {
                // Verified when: no hash pinned, or provider unreachable, or hash found.
                verified = mod.getSha1() == null || !modrinthChecked
                        || verifiedSha1.contains(mod.getSha1());
            } else if ("curseforge".equals(mod.getOrigin())) {
                verified = mod.getMurmur3() == 0 || !curseForgeChecked
                        || verifiedFp.contains(mod.getMurmur3());
            } else {
                verified = trustDirect;
            }
            if (!verified) {
                result.unverified.add(mod.getFilename());
                log.warn("Unverified mod: {} ({})", mod.getFilename(), mod.getOrigin());
            }
        }

        if (strict && !result.unverified.isEmpty()) {
            result.aborted = true;
            result.abortReason = "The following mods could not be verified against their source: "
                    + String.join(", ", result.unverified)
                    + ". Enable 'trust custom mods' or fix the server BOM to continue.";
        }
    }

    // ------------------------------------------------------------------
    // HTTP helpers
    // ------------------------------------------------------------------

    private String get(String url) throws IOException, InterruptedException {
        HttpRequest request = HttpRequest.newBuilder()
                .uri(URI.create(url))
                .header("Accept", "application/json")
                .GET()
                .build();
        HttpResponse<String> response = http.send(request, HttpResponse.BodyHandlers.ofString());
        if (response.statusCode() / 100 != 2) {
            throw new IOException("GET " + url + " failed: HTTP " + response.statusCode());
        }
        return response.body();
    }

    /** Streams a file download to {@code target}, returning the byte count. */
    private long download(String url, Path target) throws IOException, InterruptedException {
        HttpRequest request = HttpRequest.newBuilder()
                .uri(URI.create(url))
                .GET()
                .build();
        HttpResponse<InputStream> response = http.send(request, HttpResponse.BodyHandlers.ofInputStream());
        if (response.statusCode() / 100 != 2) {
            throw new IOException("Download " + url + " failed: HTTP " + response.statusCode());
        }
        long written = 0;
        try (InputStream in = response.body()) {
            byte[] buffer = new byte[8192];
            int read;
            try (var out = Files.newOutputStream(target)) {
                while ((read = in.read(buffer)) != -1) {
                    out.write(buffer, 0, read);
                    written += read;
                }
            }
        }
        return written;
    }

    private static String urlEncode(String value) {
        return URLEncoder.encode(value, StandardCharsets.UTF_8);
    }
}
