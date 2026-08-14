package com.mcmanager.client.auth;

import com.google.gson.Gson;
import com.google.gson.JsonObject;
import com.google.gson.annotations.SerializedName;
import com.sun.net.httpserver.HttpServer;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

import java.io.IOException;
import java.io.OutputStream;
import java.net.InetSocketAddress;
import java.net.URI;
import java.net.URLEncoder;
import java.net.http.HttpClient;
import java.net.http.HttpRequest;
import java.net.http.HttpResponse;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.security.MessageDigest;
import java.security.SecureRandom;
import java.util.Base64;
import java.util.concurrent.CompletableFuture;
import java.util.concurrent.Executors;
import java.util.concurrent.TimeUnit;

/**
 * Microsoft Account (MSA) → Xbox Live → Xbox Security Token Service (XSTS) →
 * Minecraft authentication flow, per plan task 3.2.
 *
 * <p>The flow opens the system browser against {@code login.live.com} with a
 * local callback server on a dynamically selected {@code http://localhost:<port>/callback}
 * (PKCE S256; the Azure app must allow localhost redirect URIs). The resulting
 * session (including the Microsoft refresh token for silent renewal) is cached in
 * {@code ~/.mcmanager/auth_cache.json}.
 *
 * <p>You must register an Azure application with a localhost redirect URI and pass
 * its client id via the {@code mcmanager.clientId} system property, the
 * {@code --clientId=...} launcher argument, the {@code ~/.mcmanager/client_id.txt}
 * file, or the embedded default ({@link #EMBEDDED_CLIENT_ID}). The client id is a
 * public OAuth identifier for a public (PKCE) client — not a secret — so shipping
 * it in the binary is fine.
 */
public class MicrosoftAuthService {

    private static final Logger log = LoggerFactory.getLogger(MicrosoftAuthService.class);

    /**
     * Sentinel value meaning "no real client id configured yet". {@code login()}
     * refuses to start the OAuth flow while the resolved id equals this value.
     */
    public static final String DEFAULT_CLIENT_ID = "REPLACE_WITH_AZURE_CLIENT_ID";

    /**
     * The Azure client id embedded in the binary so login works out of the box.
     * OAuth client ids for public clients are not secrets, so this is a plain
     * constant; the {@code --clientId=...} argument and the
     * {@code ~/.mcmanager/client_id.txt} file still override it.
     */
    static final String EMBEDDED_CLIENT_ID = "37f881f0-0083-45af-b2c4-52a658fec513";

    private static final String REDIRECT_URI = "http://localhost:8080/callback";

    private static final String AUTH_URL = "https://login.live.com/oauth20_authorize.srf";
    private static final String TOKEN_URL = "https://login.live.com/oauth20_token.srf";
    private static final String XBL_URL = "https://user.auth.xboxlive.com/user/authenticate";
    private static final String XSTS_URL = "https://xsts.auth.xboxlive.com/xsts/authorize";
    private static final String MC_LOGIN_URL = "https://api.minecraftservices.com/authentication/login_with_xbox";
    private static final String MC_ENTITLEMENTS_URL = "https://api.minecraftservices.com/entitlements/mcstore";
    private static final String MC_PROFILE_URL = "https://api.minecraftservices.com/minecraft/profile";

    private static final Path CACHE_FILE = Path.of(
            System.getProperty("user.home"), ".mcmanager", "auth_cache.json");

    /** Optional one-line file containing the Azure client id (no -D flag needed). */
    private static final Path CLIENT_ID_FILE = Path.of(
            System.getProperty("user.home"), ".mcmanager", "client_id.txt");

    private final String clientId;
    private final HttpClient http;
    private final Gson gson = new Gson();

    public MicrosoftAuthService() {
        this(resolveClientId());
    }

    public MicrosoftAuthService(String clientId) {
        this.clientId = clientId;
        this.http = HttpClient.newBuilder()
                .connectTimeout(java.time.Duration.ofSeconds(20))
                .executor(Executors.newVirtualThreadPerTaskExecutor())
                .followRedirects(HttpClient.Redirect.NORMAL)
                .build();
    }

    /**
     * Resolution order: {@code -Dmcmanager.clientId}, the {@code --clientId=...}
     * launcher argument (converted to a system property by {@code Main}), then the
     * {@code ~/.mcmanager/client_id.txt} file, then the embedded default.
     */
    private static String resolveClientId() {
        String fromProp = System.getProperty("mcmanager.clientId");
        if (fromProp != null && !fromProp.isBlank()) {
            return fromProp;
        }
        try {
            if (Files.isRegularFile(CLIENT_ID_FILE)) {
                String fromFile = Files.readString(CLIENT_ID_FILE).trim();
                if (!fromFile.isBlank()) {
                    return fromFile;
                }
            }
        } catch (IOException e) {
            // fall through to the embedded default
        }
        return EMBEDDED_CLIENT_ID;
    }

    // ------------------------------------------------------------------
    // Interactive login
    // ------------------------------------------------------------------

    /**
     * Runs the full interactive browser flow and returns the authenticated session.
     * Uses PKCE (S256) with a dynamically selected localhost port so concurrent
     * launchers never fight over a fixed callback port.
     *
     * @throws IOException if the browser/HTTP steps fail
     */
    public SessionData login() throws IOException, InterruptedException {
        if (DEFAULT_CLIENT_ID.equals(clientId)) {
            throw new IllegalStateException(
                    "Microsoft client id not configured. Run the launcher with "
                    + "--clientId=<AZURE_CLIENT_ID> (e.g. java -jar client-launcher-1.0.0-all.jar "
                    + "--clientId=abc123) or create " + CLIENT_ID_FILE + " containing the id. "
                    + "The Azure app must allow localhost redirect URIs (http://localhost:<port>/callback).");
        }

        // PKCE: the code verifier is a one-time secret; only its S256 challenge
        // is sent in the authorize URL, and the verifier is sent at token exchange.
        String codeVerifier = generateCodeVerifier();
        String codeChallenge = generateCodeChallenge(codeVerifier);

        try (CallbackServer server = new CallbackServer()) {
            server.start();
            String redirectUri = "http://localhost:" + server.getPort() + "/callback";

            String authorizeUrl = AUTH_URL
                    + "?client_id=" + urlEncode(clientId)
                    + "&response_type=code"
                    + "&redirect_uri=" + urlEncode(redirectUri)
                    + "&scope=" + urlEncode("XboxLive.signin offline_access")
                    + "&code_challenge=" + urlEncode(codeChallenge)
                    + "&code_challenge_method=S256"
                    + "&prompt=login";

            log.info("Opening browser for Microsoft login (client_id={}, redirect_uri={})",
                    clientId, redirectUri);
            log.debug("Authorize URL: {}", authorizeUrl);
            if (!java.awt.Desktop.isDesktopSupported()
                    || !java.awt.Desktop.getDesktop().isSupported(java.awt.Desktop.Action.BROWSE)) {
                throw new IOException("Desktop browser not available; open this URL manually:\n" + authorizeUrl);
            }
            java.awt.Desktop.getDesktop().browse(URI.create(authorizeUrl));

            String code = server.awaitCode(5, TimeUnit.MINUTES);
            if (code == null) {
                throw new IOException("Login timed out waiting for the browser redirect");
            }
            return completeLogin(code, codeVerifier, redirectUri);
        }
    }

    /**
     * Continues the flow after the browser callback: MS token → XBL → XSTS →
     * Minecraft token → profile. Persists the session to disk.
     */
    public SessionData completeLogin(String authCode) throws IOException, InterruptedException {
        return completeLogin(authCode, null, REDIRECT_URI);
    }

    private SessionData completeLogin(String authCode, String codeVerifier, String redirectUri)
            throws IOException, InterruptedException {
        log.debug("Step 1/5: exchanging auth code for Microsoft token...");
        MsTokenResponse ms = exchangeCodeForMsToken(authCode, codeVerifier, redirectUri);

        log.debug("Step 2/5: XBL authenticate...");
        String xblToken = xblAuthenticate(ms.accessToken);

        log.debug("Step 3/5: XSTS authorize...");
        XstsResponse xsts = xstsAuthorize(xblToken);

        log.debug("Step 4/5: Minecraft login...");
        String identityToken = "XBL3.0 x=" + xsts.uhs + ";" + xsts.token;
        McLoginResponse mc = minecraftLogin(identityToken);

        log.debug("Step 5/5: fetching Minecraft profile...");
        JsonObject profile = fetchProfile(mc.accessToken);

        SessionData session = new SessionData(
                mc.accessToken,
                ms.refreshToken,
                profile.get("name").getAsString(),
                profile.get("id").getAsString(),
                System.currentTimeMillis() + (mc.expiresIn * 1000L));
        save(session);
        log.info("Signed in as {}", session.getUsername());
        return session;
    }

    // ------------------------------------------------------------------
    // Silent renewal / cache
    // ------------------------------------------------------------------

    /** Attempts to renew an expired session using its Microsoft refresh token. */
    public SessionData refresh(SessionData session) throws IOException, InterruptedException {
        if (session == null || session.getRefreshToken() == null) {
            throw new IOException("No refresh token available");
        }
        String body = form("client_id", clientId,
                "redirect_uri", REDIRECT_URI,
                "grant_type", "refresh_token",
                "refresh_token", session.getRefreshToken(),
                "scope", "XboxLive.signin offline_access");

        JsonObject json = postJson(TOKEN_URL, body, "application/x-www-form-urlencoded");
        MsTokenResponse ms = gson.fromJson(json, MsTokenResponse.class);
        if (ms.accessToken == null) {
            throw new IOException("Token refresh failed: response missing access_token");
        }
        return completeLoginWithMsToken(ms);
    }

    private SessionData completeLoginWithMsToken(MsTokenResponse ms)
            throws IOException, InterruptedException {
        String xblToken = xblAuthenticate(ms.accessToken);
        XstsResponse xsts = xstsAuthorize(xblToken);
        McLoginResponse mc = minecraftLogin("XBL3.0 x=" + xsts.uhs + ";" + xsts.token);
        JsonObject profile = fetchProfile(mc.accessToken);
        SessionData session = new SessionData(
                mc.accessToken, ms.refreshToken,
                profile.get("name").getAsString(), profile.get("id").getAsString(),
                System.currentTimeMillis() + (mc.expiresIn * 1000L));
        save(session);
        return session;
    }

    public SessionData loadCached() {
        if (!Files.isRegularFile(CACHE_FILE)) {
            return null;
        }
        try {
            SessionData data = gson.fromJson(Files.readString(CACHE_FILE), SessionData.class);
            if (!isValidSession(data)) {
                log.warn("Ignoring invalid auth cache (missing/dummy token or non-msa session)");
                Files.deleteIfExists(CACHE_FILE);
                return null;
            }
            return data;
        } catch (IOException | RuntimeException e) {
            log.warn("Could not read auth cache", e);
            return null;
        }
    }

    /**
     * A usable session must have come from Microsoft auth: a real access token
     * and {@code userType=msa}. Rejects hand-crafted caches (dummy tokens,
     * legacy sessions) so the launcher can never launch without signing in.
     */
    private static boolean isValidSession(SessionData data) {
        if (data == null || data.getUsername() == null || data.getUsername().isBlank()) {
            return false;
        }
        String token = data.getAccessToken();
        if (token == null || token.isBlank() || "0".equals(token)) {
            return false;
        }
        String userType = data.getUserType() == null ? "msa" : data.getUserType();
        return "msa".equals(userType);
    }

    public void save(SessionData session) throws IOException {
        Files.createDirectories(CACHE_FILE.getParent());
        Files.writeString(CACHE_FILE, gson.toJson(session));
    }

    public void clearCache() throws IOException {
        Files.deleteIfExists(CACHE_FILE);
    }

    // ------------------------------------------------------------------
    // Token exchange steps
    // ------------------------------------------------------------------

    private MsTokenResponse exchangeCodeForMsToken(String code, String codeVerifier, String redirectUri)
            throws IOException, InterruptedException {
        String body = form("client_id", clientId,
                "redirect_uri", redirectUri,
                "grant_type", "authorization_code",
                "code", code,
                "scope", "XboxLive.signin offline_access");
        if (codeVerifier != null && !codeVerifier.isBlank()) {
            body += "&code_verifier=" + urlEncode(codeVerifier);
        }
        JsonObject json = postJson(TOKEN_URL, body, "application/x-www-form-urlencoded");
        MsTokenResponse ms = gson.fromJson(json, MsTokenResponse.class);
        if (ms.accessToken == null) {
            throw new IOException("OAuth token exchange failed: response missing access_token: " + json);
        }
        log.debug("OAuth token exchange OK (refresh_token present: {})", ms.refreshToken != null);
        return ms;
    }

    private String xblAuthenticate(String msAccessToken) throws IOException, InterruptedException {
        JsonObject body = new JsonObject();
        body.addProperty("RelyingParty", "http://auth.xboxlive.com");
        body.addProperty("TokenType", "JWT");
        JsonObject properties = new JsonObject();
        properties.addProperty("AuthMethod", "RPS");
        properties.addProperty("SiteName", "user.auth.xboxlive.com");
        properties.addProperty("RpsTicket", "d=" + msAccessToken);
        body.add("Properties", properties);

        JsonObject json = postJson(XBL_URL, body.toString(), "application/json");
        String token = require(json, "Token", "XBL authenticate");
        log.debug("XBL authenticate OK");
        return token;
    }

    private XstsResponse xstsAuthorize(String xblToken) throws IOException, InterruptedException {
        JsonObject body = new JsonObject();
        body.addProperty("RelyingParty", "rp://api.minecraftservices.com/");
        body.addProperty("TokenType", "JWT");
        JsonObject properties = new JsonObject();
        properties.addProperty("SandboxId", "RETAIL");
        properties.add("UserTokens", gson.toJsonTree(new String[]{xblToken}));
        body.add("Properties", properties);

        JsonObject json = postJson(XSTS_URL, body.toString(), "application/json");
        XstsResponse xsts = new XstsResponse();
        xsts.token = require(json, "Token", "XSTS authorize");
        JsonObject displayClaims = json.getAsJsonObject("DisplayClaims");
        if (displayClaims != null && displayClaims.has("xui")) {
            var xui = displayClaims.getAsJsonArray("xui");
            if (!xui.isEmpty() && xui.get(0).isJsonObject()) {
                JsonObject first = xui.get(0).getAsJsonObject();
                if (first.has("uhs")) {
                    xsts.uhs = first.get("uhs").getAsString();
                }
            }
        }
        if (xsts.uhs == null) {
            throw new IOException("XSTS response missing user hash (uhs): " + json);
        }
        log.debug("XSTS authorize OK (uhs={})", xsts.uhs);
        return xsts;
    }

    private McLoginResponse minecraftLogin(String identityToken) throws IOException, InterruptedException {
        JsonObject body = new JsonObject();
        body.addProperty("identityToken", identityToken);
        JsonObject json;
        try {
            json = postJson(MC_LOGIN_URL, body.toString(), "application/json");
        } catch (IOException e) {
            String message = e.getMessage();
            if (message != null && message.contains("Invalid app registration")) {
                throw new IOException("Minecraft rejected the login with 'Invalid app registration'. "
                        + "Two things must be true: (1) the Microsoft account owns Minecraft Java Edition, "
                        + "and (2) the Azure client ID is approved by Minecraft for authentication. "
                        + "If the account is correct, submit your client ID for review at "
                        + "https://aka.ms/mce-reviewappid — once approved (you receive an email), "
                        + "login works with no code change.", e);
            }
            throw e;
        }
        McLoginResponse mc = new McLoginResponse();
        mc.accessToken = require(json, "access_token", "Minecraft login");
        mc.expiresIn = json.has("expires_in") ? json.get("expires_in").getAsLong() : 86_400;
        log.debug("Minecraft login OK (expires_in={}s)", mc.expiresIn);
        return mc;
    }

    private JsonObject fetchProfile(String mcAccessToken) throws IOException, InterruptedException {
        HttpRequest request = HttpRequest.newBuilder()
                .uri(URI.create(MC_PROFILE_URL))
                .header("Authorization", "Bearer " + mcAccessToken)
                .GET()
                .build();
        HttpResponse<String> response = http.send(request, HttpResponse.BodyHandlers.ofString());
        log.debug("GET {} -> HTTP {}", MC_PROFILE_URL, response.statusCode());
        if (response.statusCode() != 200) {
            throw new IOException("Minecraft profile fetch failed: HTTP " + response.statusCode()
                    + " " + truncate(response.body()));
        }
        JsonObject profile = gson.fromJson(response.body(), JsonObject.class);
        if (profile == null || !profile.has("id") || !profile.has("name")) {
            throw new IOException("Minecraft profile response missing 'id'/'name' "
                    + "(does the account own Minecraft?): " + truncate(response.body()));
        }
        log.debug("Minecraft profile OK (id={}, name={})",
                profile.get("id").getAsString(), profile.get("name").getAsString());
        return profile;
    }

    /** Returns true if the account owns Minecraft (best effort — never aborts login). */
    public boolean checkEntitlements(String mcAccessToken) {
        try {
            HttpRequest request = HttpRequest.newBuilder()
                    .uri(URI.create(MC_ENTITLEMENTS_URL))
                    .header("Authorization", "Bearer " + mcAccessToken)
                    .GET()
                    .build();
            HttpResponse<String> response = http.send(request, HttpResponse.BodyHandlers.ofString());
            JsonObject json = gson.fromJson(response.body(), JsonObject.class);
            return json != null && json.has("items") && json.getAsJsonArray("items").size() > 0;
        } catch (Exception e) {
            log.warn("Entitlements check failed: {}", e.getMessage());
            return true; // don't block login on transient API issues
        }
    }

    // ------------------------------------------------------------------
    // HTTP helpers
    // ------------------------------------------------------------------

    private JsonObject postJson(String url, String body, String contentType)
            throws IOException, InterruptedException {
        HttpRequest request = HttpRequest.newBuilder()
                .uri(URI.create(url))
                .header("Content-Type", contentType)
                .header("Accept", "application/json")
                .POST(HttpRequest.BodyPublishers.ofString(body))
                .build();
        HttpResponse<String> response = http.send(request, HttpResponse.BodyHandlers.ofString());
        log.debug("POST {} -> HTTP {}: {}", url, response.statusCode(), truncate(response.body()));
        if (response.statusCode() / 100 != 2) {
            throw new IOException("POST " + url + " failed: HTTP " + response.statusCode()
                    + " " + truncate(response.body()));
        }
        if (response.body() == null || response.body().isBlank()) {
            throw new IOException("POST " + url + " returned an empty response body");
        }
        return gson.fromJson(response.body(), JsonObject.class);
    }

    private String require(JsonObject json, String field, String step) throws IOException {
        if (json == null) {
            throw new IOException(step + " failed: response was empty or not valid JSON");
        }
        if (!json.has(field)) {
            throw new IOException(step + " failed: response missing '" + field + "': " + json);
        }
        return json.get(field).getAsString();
    }

    /** Truncates long response bodies so error messages and debug logs stay readable. */
    private static String truncate(String s) {
        if (s == null) {
            return "null";
        }
        return s.length() > 500 ? s.substring(0, 500) + "…" : s;
    }

    private static String form(String... kv) {
        StringBuilder sb = new StringBuilder();
        for (int i = 0; i < kv.length; i += 2) {
            if (i > 0) {
                sb.append('&');
            }
            sb.append(urlEncode(kv[i])).append('=').append(urlEncode(kv[i + 1]));
        }
        return sb.toString();
    }

    private static String urlEncode(String value) {
        return URLEncoder.encode(value, StandardCharsets.UTF_8);
    }

    // ------------------------------------------------------------------
    // PKCE helpers
    // ------------------------------------------------------------------

    /** 64 random chars from the RFC 7636 unreserved alphabet. */
    private static String generateCodeVerifier() {
        String chars = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-._~";
        SecureRandom random = new SecureRandom();
        StringBuilder sb = new StringBuilder(64);
        for (int i = 0; i < 64; i++) {
            sb.append(chars.charAt(random.nextInt(chars.length())));
        }
        return sb.toString();
    }

    /** S256 challenge = base64url(sha256(verifier)), unpadded. */
    private static String generateCodeChallenge(String codeVerifier) throws IOException {
        try {
            byte[] digest = MessageDigest.getInstance("SHA-256")
                    .digest(codeVerifier.getBytes(StandardCharsets.US_ASCII));
            return Base64.getUrlEncoder().withoutPadding().encodeToString(digest);
        } catch (java.security.NoSuchAlgorithmException e) {
            throw new IOException("SHA-256 unavailable for PKCE", e);
        }
    }

    /**
     * Renders the local OAuth callback page in Zircon's dark theme (matching the
     * launcher UI: {@code #0d1117} background, {@code #161b22} cards, teal
     * {@code #47d2c9} accents). The page confirms a successful sign-in or surfaces
     * the Azure error returned in the redirect query string.
     */
    private static String callbackPage(boolean success, String error, String errorDescription) {
        String title = success ? "Authentication Successful!" : "Authentication Failed";
        String message = success
                ? "You may now close this browser window and return to the launcher."
                : "Something went wrong — close this window and return to the launcher.";
        String errorHtml = error != null
                ? "<p class='error'>" + escapeHtml(error)
                + (errorDescription != null ? " — " + escapeHtml(errorDescription) : "") + "</p>"
                : "";
        return """
                <!DOCTYPE html>
                <html>
                <head>
                    <meta charset="utf-8">
                    <style>
                        body { background-color: #0d1117; color: #c9d1d9; font-family: 'Segoe UI', sans-serif; text-align: center; padding-top: 100px; margin: 0; }
                        .card { background: #161b22; border: 1px solid #30363d; border-radius: 12px; display: inline-block; padding: 40px; box-shadow: 0 10px 25px rgba(0,0,0,0.5); }
                        .logo { background: #47d2c9; color: #022c29; border-radius: 8px; font-weight: bold; padding: 6px 12px; font-size: 20px; display: inline-block; margin-bottom: 16px; }
                        h2 { margin: 0 0 12px 0; color: #ffffff; }
                        p { color: #8b949e; font-size: 14px; margin: 0; }
                        .error { color: #f85149; margin-top: 12px; }
                    </style>
                </head>
                <body>
                    <div class="card">
                        <div class="logo">⚡ Zircon</div>
                        <h2>%s</h2>
                        <p>%s</p>%s
                    </div>
                </body>
                </html>
                """.formatted(title, message, errorHtml);
    }

    /** Minimal HTML escaping so Azure error text can't break out of the page markup. */
    private static String escapeHtml(String value) {
        if (value == null) {
            return "";
        }
        return value.replace("&", "&amp;")
                .replace("<", "&lt;")
                .replace(">", "&gt;")
                .replace("\"", "&quot;");
    }

    // ------------------------------------------------------------------
    // Local callback server
    // ------------------------------------------------------------------

    private static final class CallbackServer implements AutoCloseable {
        private final HttpServer server;
        private final CompletableFuture<String> codeFuture = new CompletableFuture<>();

        CallbackServer() throws IOException {
            // Port 0 → the OS assigns a free port; no more 8080 collisions.
            this.server = HttpServer.create(new InetSocketAddress("127.0.0.1", 0), 0);
        }

        int getPort() {
            return server.getAddress().getPort();
        }

        void start() {
            server.createContext("/callback", exchange -> {
                String query = exchange.getRequestURI().getQuery();
                String code = null;
                String error = null;
                String errorDescription = null;
                if (query != null) {
                    for (String pair : query.split("&")) {
                        String[] kv = pair.split("=", 2);
                        if (kv.length != 2) {
                            continue;
                        }
                        String key = kv[0];
                        String value = java.net.URLDecoder.decode(kv[1], StandardCharsets.UTF_8);
                        switch (key) {
                            case "code" -> code = value;
                            case "error" -> error = value;
                            case "error_description" -> errorDescription = value;
                            default -> {
                            }
                        }
                    }
                }
                byte[] response = callbackPage(code != null, error, errorDescription)
                        .getBytes(StandardCharsets.UTF_8);
                exchange.getResponseHeaders().set("Content-Type", "text/html; charset=utf-8");
                exchange.sendResponseHeaders(200, response.length);
                try (OutputStream out = exchange.getResponseBody()) {
                    out.write(response);
                }
                if (code != null) {
                    codeFuture.complete(code);
                } else if (error != null) {
                    codeFuture.completeExceptionally(new IOException("Microsoft login failed: " + error
                            + (errorDescription != null ? " — " + errorDescription : "")));
                } else {
                    codeFuture.completeExceptionally(new IOException("OAuth callback missing code"));
                }
            });
            server.start();
        }

        /** @return the auth code, {@code null} on timeout, or throws the Azure error. */
        String awaitCode(long timeout, TimeUnit unit) throws InterruptedException, IOException {
            try {
                return codeFuture.get(timeout, unit);
            } catch (java.util.concurrent.TimeoutException e) {
                return null;
            } catch (java.util.concurrent.ExecutionException e) {
                Throwable cause = e.getCause();
                if (cause instanceof IOException io) {
                    throw io;
                }
                throw new IOException("OAuth callback failed", cause);
            }
        }

        @Override
        public void close() {
            server.stop(0);
        }
    }

    // ------------------------------------------------------------------
    // Token response DTOs
    // ------------------------------------------------------------------

    private static class MsTokenResponse {
        @SerializedName("access_token")
        String accessToken;
        @SerializedName("refresh_token")
        String refreshToken;
    }

    private static class XstsResponse {
        String token;
        String uhs;
    }

    private static class McLoginResponse {
        String accessToken;
        long expiresIn;
    }
}
