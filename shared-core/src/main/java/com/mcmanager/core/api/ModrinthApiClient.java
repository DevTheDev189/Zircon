package com.mcmanager.core.api;

import com.google.gson.Gson;
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
import java.util.HashMap;
import java.util.List;
import java.util.Map;
import java.util.concurrent.Executors;

/**
 * Client for the Modrinth API (v2).
 *
 * <p>Modrinth requires a descriptive {@code User-Agent} header; requests are made
 * on virtual threads via {@link HttpClient}.
 */
public class ModrinthApiClient {

    public static final String BASE_URL = "https://api.modrinth.com/v2";
    public static final String DEFAULT_USER_AGENT = "McManager/1.0.0 (contact@example.com)";

    private final HttpClient httpClient;
    private final Gson gson = new Gson();
    private final String userAgent;

    public ModrinthApiClient() {
        this(DEFAULT_USER_AGENT);
    }

    public ModrinthApiClient(String userAgent) {
        this.userAgent = userAgent;
        this.httpClient = HttpClient.newBuilder()
                .connectTimeout(Duration.ofSeconds(15))
                .executor(Executors.newVirtualThreadPerTaskExecutor())
                .followRedirects(HttpClient.Redirect.NORMAL)
                .build();
    }

    /**
     * Batch-verifies SHA-1 hashes against Modrinth's file database.
     *
     * @param sha1List list of lower-case hex SHA-1 hashes
     * @return map of hash → {@link ModrinthVersion} for every hash Modrinth
     *         recognises. Hashes missing from the returned map are not known to
     *         Modrinth (and therefore not verified).
     */
    public Map<String, ModrinthVersion> verifyHashes(List<String> sha1List)
            throws IOException, InterruptedException {
        if (sha1List == null || sha1List.isEmpty()) {
            return Map.of();
        }

        JsonObject body = new JsonObject();
        body.add("hashes", gson.toJsonTree(sha1List));
        body.addProperty("algorithm", "sha1");

        HttpRequest request = HttpRequest.newBuilder()
                .uri(URI.create(BASE_URL + "/version_files"))
                .header("User-Agent", userAgent)
                .header("Content-Type", "application/json")
                .POST(HttpRequest.BodyPublishers.ofString(body.toString()))
                .build();

        HttpResponse<String> response = httpClient.send(request,
                HttpResponse.BodyHandlers.ofString());
        if (response.statusCode() / 100 != 2) {
            throw new IOException("Modrinth verifyHashes failed: HTTP " + response.statusCode()
                    + " " + response.body());
        }

        @SuppressWarnings("unchecked")
        Map<String, ModrinthVersion> result = gson.fromJson(response.body(),
                new com.google.gson.reflect.TypeToken<Map<String, ModrinthVersion>>() {
                }.getType());
        return result != null ? result : Map.of();
    }

    /**
     * Searches Modrinth for mods matching a query for the given game version and
     * loader category.
     *
     * @param query      search text
     * @param mcVersion  e.g. "1.20.4" (may be {@code null} to ignore)
     * @param loaderType e.g. "fabric" (may be {@code null} to ignore)
     */
    public List<ModrinthSearchHit> searchMods(String query, String mcVersion, String loaderType)
            throws IOException, InterruptedException {
        StringBuilder url = new StringBuilder(BASE_URL).append("/search?query=")
                .append(urlEncode(query == null ? "" : query));

        if (mcVersion != null && loaderType != null) {
            url.append("&facets=").append(urlEncode(
                    "[[\"versions:" + mcVersion + "\"],[\"categories:" + loaderType + "\"]]"));
        } else if (mcVersion != null) {
            url.append("&facets=").append(urlEncode("[[\"versions:" + mcVersion + "\"]]"));
        } else if (loaderType != null) {
            url.append("&facets=").append(urlEncode("[[\"categories:" + loaderType + "\"]]"));
        }
        url.append("&limit=25");

        HttpRequest request = HttpRequest.newBuilder()
                .uri(URI.create(url.toString()))
                .header("User-Agent", userAgent)
                .GET()
                .build();

        HttpResponse<String> response = httpClient.send(request,
                HttpResponse.BodyHandlers.ofString());
        if (response.statusCode() / 100 != 2) {
            throw new IOException("Modrinth search failed: HTTP " + response.statusCode()
                    + " " + response.body());
        }

        JsonObject root = gson.fromJson(response.body(), JsonObject.class);
        List<ModrinthSearchHit> hits = new ArrayList<>();
        if (root.has("hits")) {
            root.getAsJsonArray("hits").forEach(h -> hits.add(gson.fromJson(h, ModrinthSearchHit.class)));
        }
        return hits;
    }

    /**
     * Lists published versions of a Modrinth project, optionally filtered by game
     * version and loader. Used by the admin UI to pick a concrete version to install.
     */
    public List<ModrinthVersion> listProjectVersions(String projectId, String mcVersion, String loaderType)
            throws IOException, InterruptedException {
        StringBuilder url = new StringBuilder(BASE_URL)
                .append("/project/").append(urlEncode(projectId)).append("/version");

        List<String> filters = new ArrayList<>();
        if (mcVersion != null) {
            filters.add("game_versions=" + urlEncode("[\"" + mcVersion + "\"]"));
        }
        if (loaderType != null) {
            filters.add("loaders=" + urlEncode("[\"" + loaderType + "\"]"));
        }
        if (!filters.isEmpty()) {
            url.append("?").append(String.join("&", filters));
        }

        HttpRequest request = HttpRequest.newBuilder()
                .uri(URI.create(url.toString()))
                .header("User-Agent", userAgent)
                .GET()
                .build();

        HttpResponse<String> response = httpClient.send(request,
                HttpResponse.BodyHandlers.ofString());
        if (response.statusCode() / 100 != 2) {
            throw new IOException("Modrinth listProjectVersions failed: HTTP " + response.statusCode()
                    + " " + response.body());
        }

        List<ModrinthVersion> versions = new ArrayList<>();
        com.google.gson.JsonArray arr = gson.fromJson(response.body(), com.google.gson.JsonArray.class);
        if (arr != null) {
            arr.forEach(h -> versions.add(gson.fromJson(h, ModrinthVersion.class)));
        }
        return versions;
    }

    /**
     * Fetches the public metadata of a Modrinth project (title, description, icon,
     * author). Used to enrich installed mod entries for the admin UI.
     */
    public ModrinthProject getProject(String projectId) throws IOException, InterruptedException {
        HttpRequest request = HttpRequest.newBuilder()
                .uri(URI.create(BASE_URL + "/project/" + urlEncode(projectId)))
                .header("User-Agent", userAgent)
                .GET()
                .build();

        HttpResponse<String> response = httpClient.send(request,
                HttpResponse.BodyHandlers.ofString());
        if (response.statusCode() / 100 != 2) {
            throw new IOException("Modrinth getProject failed: HTTP " + response.statusCode()
                    + " " + response.body());
        }
        return gson.fromJson(response.body(), ModrinthProject.class);
    }

    // ------------------------------------------------------------------
    // Response DTOs
    // ------------------------------------------------------------------

    /** Public project metadata from {@code GET /project/{id}}. */
    public static class ModrinthProject {
        @SerializedName("id")
        public String id;
        @SerializedName("slug")
        public String slug;
        @SerializedName("title")
        public String title;
        @SerializedName("description")
        public String description;
        @SerializedName("icon_url")
        public String iconUrl;
        @SerializedName("author")
        public String author;
        @SerializedName("downloads")
        public long downloads;
    }

    /** A specific version/file of a Modrinth project. */
    public static class ModrinthVersion {
        @SerializedName("id")
        public String id;
        @SerializedName("project_id")
        public String projectId;
        @SerializedName("name")
        public String name;
        @SerializedName("version_number")
        public String versionNumber;
        @SerializedName("game_versions")
        public List<String> gameVersions;
        @SerializedName("loaders")
        public List<String> loaders;
        @SerializedName("files")
        public List<ModrinthFile> files;
        @SerializedName("url")
        public String url;

        public java.util.Map<String, Object> toMap() {
            java.util.Map<String, Object> map = new HashMap<>();
            map.put("id", id);
            map.put("projectId", projectId);
            map.put("name", name);
            map.put("versionNumber", versionNumber);
            map.put("gameVersions", gameVersions);
            map.put("loaders", loaders);
            map.put("url", url);
            if (primaryFile() != null) {
                map.put("file", primaryFile().toMap());
            }
            return map;
        }

        /** @return the primary (downloadable) file of this version, or the first file. */
        public ModrinthFile primaryFile() {
            if (files == null || files.isEmpty()) {
                return null;
            }
            for (ModrinthFile f : files) {
                if (f.primary) {
                    return f;
                }
            }
            return files.get(0);
        }
    }

    /** A downloadable file within a Modrinth version. */
    public static class ModrinthFile {
        @SerializedName("url")
        public String url;
        @SerializedName("filename")
        public String filename;
        @SerializedName("hashes")
        public Map<String, String> hashes;
        @SerializedName("size")
        public long size;
        @SerializedName("primary")
        public boolean primary;

        public String sha1() {
            return hashes == null ? null : hashes.get("sha1");
        }

        public java.util.Map<String, Object> toMap() {
            java.util.Map<String, Object> map = new HashMap<>();
            map.put("url", url);
            map.put("filename", filename);
            map.put("sha1", sha1());
            map.put("size", size);
            map.put("primary", primary);
            return map;
        }
    }

    /** One hit from the Modrinth search endpoint. */
    public static class ModrinthSearchHit {
        @SerializedName("project_id")
        public String projectId;
        @SerializedName("slug")
        public String slug;
        @SerializedName("title")
        public String title;
        @SerializedName("description")
        public String description;
        @SerializedName("author")
        public String author;
        @SerializedName("downloads")
        public long downloads;
        @SerializedName("icon_url")
        public String iconUrl;
        @SerializedName("versions")
        public List<String> versions;

        public Map<String, Object> toMap() {
            Map<String, Object> map = new HashMap<>();
            map.put("projectId", projectId);
            map.put("slug", slug);
            map.put("title", title);
            map.put("description", description);
            map.put("author", author);
            map.put("downloads", downloads);
            map.put("iconUrl", iconUrl);
            map.put("versions", versions);
            return map;
        }
    }

    private static String urlEncode(String value) {
        return URLEncoder.encode(value, StandardCharsets.UTF_8);
    }
}
