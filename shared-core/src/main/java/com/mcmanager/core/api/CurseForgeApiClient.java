package com.mcmanager.core.api;

import com.google.gson.Gson;
import com.google.gson.JsonArray;
import com.google.gson.JsonElement;
import com.google.gson.JsonObject;
import com.google.gson.annotations.SerializedName;

import java.io.IOException;
import java.net.URI;
import java.net.URLEncoder;
import java.net.http.HttpClient;
import java.net.http.HttpRequest;
import java.net.http.HttpResponse;
import java.nio.charset.StandardCharsets;
import java.time.Duration;
import java.util.ArrayList;
import java.util.List;
import java.util.concurrent.Executors;

/**
 * Client for the CurseForge API (v1).
 *
 * <p>All requests require the {@code x-api-key} header. CurseForge identifies
 * files by MurmurHash3 "fingerprints" (see {@code MurmurHash3} in shared-core).
 */
public class CurseForgeApiClient {

    public static final String BASE_URL = "https://api.curseforge.com/v1";
    public static final int MINECRAFT_GAME_ID = 432;

    private final HttpClient httpClient;
    private final Gson gson = new Gson();
    private final String apiKey;

    public CurseForgeApiClient(String apiKey) {
        this.apiKey = apiKey;
        this.httpClient = HttpClient.newBuilder()
                .connectTimeout(Duration.ofSeconds(15))
                .executor(Executors.newVirtualThreadPerTaskExecutor())
                .followRedirects(HttpClient.Redirect.NORMAL)
                .build();
    }

    /**
     * Batch-verifies MurmurHash3 fingerprints against CurseForge.
     *
     * @param fingerprintList list of unsigned fingerprint values
     * @return the exact matches as {@link CurseForgeFile} objects; an empty list
     *         means none of the fingerprints are known to CurseForge.
     */
    public List<CurseForgeFile> verifyFingerprints(List<Long> fingerprintList)
            throws IOException, InterruptedException {
        if (fingerprintList == null || fingerprintList.isEmpty()) {
            return List.of();
        }

        JsonObject body = new JsonObject();
        body.add("fingerprints", gson.toJsonTree(fingerprintList));

        HttpRequest request = HttpRequest.newBuilder()
                .uri(URI.create(BASE_URL + "/fingerprints"))
                .header("x-api-key", apiKey)
                .header("Content-Type", "application/json")
                .POST(HttpRequest.BodyPublishers.ofString(body.toString()))
                .build();

        HttpResponse<String> response = httpClient.send(request,
                HttpResponse.BodyHandlers.ofString());
        if (response.statusCode() / 100 != 2) {
            throw new IOException("CurseForge verifyFingerprints failed: HTTP "
                    + response.statusCode() + " " + response.body());
        }

        List<CurseForgeFile> matches = new ArrayList<>();
        JsonObject root = gson.fromJson(response.body(), JsonObject.class);
        JsonObject data = root.getAsJsonObject("data");
        if (data != null && data.has("exactMatches")) {
            JsonArray exactMatches = data.getAsJsonArray("exactMatches");
            for (JsonElement element : exactMatches) {
                JsonObject match = element.getAsJsonObject();
                if (match.has("file")) {
                    matches.add(gson.fromJson(match.get("file"), CurseForgeFile.class));
                }
            }
        }
        return matches;
    }

    /**
     * Searches CurseForge mods for Minecraft.
     *
     * @param query     search text
     * @param mcVersion e.g. "1.20.4" (may be {@code null} to ignore)
     */
    public List<CurseForgeMod> searchMods(String query, String mcVersion)
            throws IOException, InterruptedException {
        StringBuilder url = new StringBuilder(BASE_URL).append("/mods/search?gameId=")
                .append(MINECRAFT_GAME_ID)
                .append("&searchFilter=").append(urlEncode(query == null ? "" : query))
                .append("&sortField=1&sortOrder=desc&pageSize=25");
        if (mcVersion != null) {
            url.append("&gameVersion=").append(urlEncode(mcVersion));
        }

        HttpRequest request = HttpRequest.newBuilder()
                .uri(URI.create(url.toString()))
                .header("x-api-key", apiKey)
                .header("Accept", "application/json")
                .GET()
                .build();

        HttpResponse<String> response = httpClient.send(request,
                HttpResponse.BodyHandlers.ofString());
        if (response.statusCode() / 100 != 2) {
            throw new IOException("CurseForge search failed: HTTP " + response.statusCode()
                    + " " + response.body());
        }

        List<CurseForgeMod> mods = new ArrayList<>();
        JsonObject root = gson.fromJson(response.body(), JsonObject.class);
        JsonObject data = root.getAsJsonObject("data");
        if (data != null && data.has("data")) {
            data.getAsJsonArray("data")
                    .forEach(e -> mods.add(gson.fromJson(e, CurseForgeMod.class)));
        }
        return mods;
    }

    /**
     * Lists all files of a CurseForge mod, so the admin UI can pick which file to
     * install for the target Minecraft version.
     */
    public List<CurseForgeFile> listModFiles(long modId) throws IOException, InterruptedException {
        HttpRequest request = HttpRequest.newBuilder()
                .uri(URI.create(BASE_URL + "/mods/" + modId + "/files?pageSize=50"))
                .header("x-api-key", apiKey)
                .header("Accept", "application/json")
                .GET()
                .build();

        HttpResponse<String> response = httpClient.send(request,
                HttpResponse.BodyHandlers.ofString());
        if (response.statusCode() / 100 != 2) {
            throw new IOException("CurseForge listModFiles failed: HTTP " + response.statusCode()
                    + " " + response.body());
        }

        List<CurseForgeFile> files = new ArrayList<>();
        JsonObject root = gson.fromJson(response.body(), JsonObject.class);
        JsonObject data = root.getAsJsonObject("data");
        if (data != null && data.has("data")) {
            data.getAsJsonArray("data")
                    .forEach(e -> files.add(gson.fromJson(e, CurseForgeFile.class)));
        }
        return files;
    }

    // ------------------------------------------------------------------
    // Response DTOs
    // ------------------------------------------------------------------

    /** A CurseForge mod (project) returned by the search endpoint. */
    public static class CurseForgeMod {
        @SerializedName("id")
        public long id;
        @SerializedName("name")
        public String name;
        @SerializedName("slug")
        public String slug;
        @SerializedName("summary")
        public String summary;
        @SerializedName("downloadCount")
        public long downloadCount;
        @SerializedName("gameVersions")
        public List<String> gameVersions;
        @SerializedName("links")
        public CurseForgeLinks links;
        @SerializedName("latestFiles")
        public List<CurseForgeFile> latestFiles;

        public java.util.Map<String, Object> toMap() {
            java.util.Map<String, Object> map = new java.util.HashMap<>();
            map.put("id", id);
            map.put("name", name);
            map.put("slug", slug);
            map.put("summary", summary);
            map.put("downloadCount", downloadCount);
            map.put("gameVersions", gameVersions);
            map.put("websiteUrl", links == null ? null : links.websiteUrl);
            return map;
        }
    }

    public static class CurseForgeLinks {
        @SerializedName("websiteUrl")
        public String websiteUrl;
    }

    /** A CurseForge file (a concrete downloadable artifact of a mod). */
    public static class CurseForgeFile {
        @SerializedName("id")
        public long id;
        @SerializedName("displayName")
        public String displayName;
        @SerializedName("fileName")
        public String fileName;
        @SerializedName("downloadUrl")
        public String downloadUrl;
        @SerializedName("fileFingerprint")
        public long fileFingerprint;
        @SerializedName("length")
        public long length;

        public java.util.Map<String, Object> toMap() {
            java.util.Map<String, Object> map = new java.util.HashMap<>();
            map.put("id", id);
            map.put("displayName", displayName);
            map.put("fileName", fileName);
            map.put("downloadUrl", downloadUrl);
            map.put("fileFingerprint", fileFingerprint);
            map.put("length", length);
            return map;
        }
    }

    private static String urlEncode(String value) {
        return URLEncoder.encode(value, StandardCharsets.UTF_8);
    }
}
