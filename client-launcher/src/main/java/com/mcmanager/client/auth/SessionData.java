package com.mcmanager.client.auth;

import java.nio.charset.StandardCharsets;
import java.util.UUID;

/**
 * Plain Gson-serializable POJO holding a Minecraft/Microsoft authentication
 * session: the Minecraft access token, the Microsoft refresh token (for silent
 * renewal), and the Minecraft profile identity.
 */
public class SessionData {

    /** Tokens are considered expired {@value GRACE_MILLIS} ms before they actually expire. */
    private static final long GRACE_MILLIS = 60_000;

    private String accessToken;
    private String refreshToken;
    private String username;
    private String uuid;
    private long expiresAtMillis;

    /** Always {@code "msa"} in release builds; {@code "legacy"} only for DEV-ONLY offline sessions. */
    private String userType = "msa";

    public SessionData() {
    }

    public SessionData(String accessToken, String refreshToken, String username, String uuid, long expiresAtMillis) {
        this.accessToken = accessToken;
        this.refreshToken = refreshToken;
        this.username = username;
        this.uuid = uuid;
        this.expiresAtMillis = expiresAtMillis;
    }

    /**
     * DEV-ONLY (temporary testing aid): creates a fake session with a dummy token
     * and {@code userType=legacy}, so the client runs without Microsoft auth.
     * REMOVE BEFORE RELEASE.
     */
    public static SessionData offline(String username) {
        String name = username == null || username.isBlank() ? "DevPlayer" : username.trim();
        UUID offlineUuid = UUID.nameUUIDFromBytes(("OfflinePlayer:" + name).getBytes(StandardCharsets.UTF_8));
        SessionData session = new SessionData("0", null, name, offlineUuid.toString(), Long.MAX_VALUE);
        session.setUserType("legacy");
        return session;
    }

    public String getUserType() {
        return userType;
    }

    public void setUserType(String userType) {
        this.userType = userType;
    }

    public String getAccessToken() {
        return accessToken;
    }

    public void setAccessToken(String accessToken) {
        this.accessToken = accessToken;
    }

    public String getRefreshToken() {
        return refreshToken;
    }

    public void setRefreshToken(String refreshToken) {
        this.refreshToken = refreshToken;
    }

    public String getUsername() {
        return username;
    }

    public void setUsername(String username) {
        this.username = username;
    }

    public String getUuid() {
        return uuid;
    }

    public void setUuid(String uuid) {
        this.uuid = uuid;
    }

    public long getExpiresAtMillis() {
        return expiresAtMillis;
    }

    public void setExpiresAtMillis(long expiresAtMillis) {
        this.expiresAtMillis = expiresAtMillis;
    }

    /**
     * @return {@code true} if the access token is expired (or about to expire
     *         within the grace period), {@code false} if it is still valid.
     */
    public boolean isExpired() {
        return System.currentTimeMillis() > expiresAtMillis - GRACE_MILLIS;
    }

    @Override
    public String toString() {
        return "SessionData{username='" + username + "', uuid='" + uuid
                + "', expiresAtMillis=" + expiresAtMillis + '}';
    }
}
