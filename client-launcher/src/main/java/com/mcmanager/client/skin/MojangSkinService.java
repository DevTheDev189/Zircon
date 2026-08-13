package com.mcmanager.client.skin;

import com.google.gson.Gson;
import com.google.gson.JsonArray;
import com.google.gson.JsonObject;

import java.io.ByteArrayOutputStream;
import java.io.IOException;
import java.net.URI;
import java.net.http.HttpClient;
import java.net.http.HttpRequest;
import java.net.http.HttpResponse;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.time.Duration;
import java.util.Base64;

/**
 * Mojang Minecraft skin integration: downloads a player's current skin (by
 * UUID) and uploads a new skin (using the Minecraft access token).
 *
 * <p>Skin download is unauthenticated via the session server; upload requires
 * the Minecraft bearer token produced by Microsoft auth.
 */
public final class MojangSkinService {

    private static final String PROFILE_URL = "https://sessionserver.mojang.com/session/minecraft/profile/";
    private static final String UPLOAD_URL = "https://api.minecraftservices.com/minecraft/profile/skins";

    private static final HttpClient HTTP = HttpClient.newBuilder()
            .connectTimeout(Duration.ofSeconds(15))
            .followRedirects(HttpClient.Redirect.NORMAL)
            .build();
    private static final Gson GSON = new Gson();

    private MojangSkinService() {
    }

    /** A downloaded skin plus its model variant ({@code classic} or {@code slim}). */
    public record DownloadedSkin(byte[] png, String variant) {
    }

    /**
     * Downloads the current Mojang skin for the given profile UUID.
     *
     * @throws IOException if the profile has no custom skin or the request fails
     */
    public static DownloadedSkin download(String uuid) throws IOException, InterruptedException {
        JsonObject profile = fetchProfile(uuid);
        JsonObject textures = texturesObject(profile);
        if (textures == null) {
            throw new IOException("This account has no custom Mojang skin (using the default skin).");
        }

        JsonObject skin = textures.getAsJsonObject("SKIN");
        if (skin == null || !skin.has("url")) {
            throw new IOException("This account has no custom Mojang skin (using the default skin).");
        }

        String skinUrl = skin.get("url").getAsString().replaceFirst("^http://", "https://");
        String variant = "classic";
        if (skin.has("metadata")) {
            JsonObject metadata = skin.getAsJsonObject("metadata");
            if (metadata != null && metadata.has("model")) {
                variant = metadata.get("model").getAsString();
            }
        }

        HttpResponse<byte[]> response = HTTP.send(
                HttpRequest.newBuilder().uri(URI.create(skinUrl)).GET().build(),
                HttpResponse.BodyHandlers.ofByteArray());
        if (response.statusCode() != 200) {
            throw new IOException("Skin download failed: HTTP " + response.statusCode());
        }
        return new DownloadedSkin(response.body(), variant);
    }

    /**
     * Uploads a local PNG as the player's new Minecraft skin.
     *
     * @param mcAccessToken Minecraft access token (not the Microsoft OAuth token)
     * @param pngFile       64x64 (or 64x32) skin PNG
     * @param variant       {@code classic} or {@code slim}
     */
    public static void upload(String mcAccessToken, Path pngFile, String variant)
            throws IOException, InterruptedException {
        byte[] fileBytes = Files.readAllBytes(pngFile);
        String boundary = "----ZirconSkin" + Long.toHexString(System.nanoTime());
        byte[] body = buildMultipart(boundary, pngFile.getFileName().toString(), variant, fileBytes);

        HttpRequest request = HttpRequest.newBuilder()
                .uri(URI.create(UPLOAD_URL))
                .header("Authorization", "Bearer " + mcAccessToken)
                .header("Content-Type", "multipart/form-data; boundary=" + boundary)
                .POST(HttpRequest.BodyPublishers.ofByteArray(body))
                .build();

        HttpResponse<String> response = HTTP.send(request, HttpResponse.BodyHandlers.ofString());
        if (response.statusCode() / 100 != 2) {
            throw new IOException("Mojang skin upload failed: HTTP " + response.statusCode()
                    + " " + truncate(response.body()));
        }
    }

    private static JsonObject fetchProfile(String uuid) throws IOException, InterruptedException {
        HttpRequest request = HttpRequest.newBuilder()
                .uri(URI.create(PROFILE_URL + sanitizeUuid(uuid)))
                .GET()
                .build();
        HttpResponse<String> response = HTTP.send(request, HttpResponse.BodyHandlers.ofString());
        if (response.statusCode() == 204 || response.statusCode() == 404) {
            throw new IOException("No Mojang profile found for this UUID.");
        }
        if (response.statusCode() != 200) {
            throw new IOException("Mojang profile fetch failed: HTTP " + response.statusCode());
        }
        return GSON.fromJson(response.body(), JsonObject.class);
    }

    /** Decodes the Base64 {@code textures} property from a session-server profile. */
    private static JsonObject texturesObject(JsonObject profile) {
        if (profile == null || !profile.has("properties")) {
            return null;
        }
        JsonArray properties = profile.getAsJsonArray("properties");
        for (var element : properties) {
            JsonObject property = element.getAsJsonObject();
            if (!"textures".equals(property.has("name") ? property.get("name").getAsString() : null)) {
                continue;
            }
            String value = property.get("value").getAsString();
            String decoded = new String(Base64.getDecoder().decode(value), StandardCharsets.UTF_8);
            JsonObject root = GSON.fromJson(decoded, JsonObject.class);
            if (root != null && root.has("textures")) {
                return root.getAsJsonObject("textures");
            }
        }
        return null;
    }

    private static byte[] buildMultipart(String boundary, String filename, String variant, byte[] fileBytes)
            throws IOException {
        ByteArrayOutputStream out = new ByteArrayOutputStream();
        write(out, "--" + boundary + "\r\n");
        write(out, "Content-Disposition: form-data; name=\"variant\"\r\n\r\n");
        write(out, variant + "\r\n");
        write(out, "--" + boundary + "\r\n");
        write(out, "Content-Disposition: form-data; name=\"file\"; filename=\""
                + sanitizeFilename(filename) + "\"\r\n");
        write(out, "Content-Type: image/png\r\n\r\n");
        out.write(fileBytes);
        write(out, "\r\n");
        write(out, "--" + boundary + "--\r\n");
        return out.toByteArray();
    }

    private static void write(ByteArrayOutputStream out, String text) throws IOException {
        out.write(text.getBytes(StandardCharsets.UTF_8));
    }

    private static String sanitizeFilename(String name) {
        if (name == null || name.isBlank()) {
            return "skin.png";
        }
        return name.replaceAll("[^A-Za-z0-9._-]", "_");
    }

    private static String sanitizeUuid(String uuid) {
        return uuid == null ? "" : uuid.replaceAll("[^0-9a-fA-F-]", "");
    }

    private static String truncate(String text) {
        if (text == null) {
            return "";
        }
        return text.length() > 200 ? text.substring(0, 200) : text;
    }
}
