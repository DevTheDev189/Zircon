package com.mcmanager.server.auth;

import io.jsonwebtoken.Claims;
import io.jsonwebtoken.Jwts;
import io.jsonwebtoken.security.Keys;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

import javax.crypto.SecretKey;
import java.io.IOException;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.security.SecureRandom;
import java.time.Instant;
import java.util.Base64;
import java.util.Date;

/**
 * Issues and validates admin JWTs.
 *
 * <p>The signing secret is generated once, persisted to {@code jwt-secret.key}
 * in the data dir, and reused across restarts so tokens stay valid. Tokens
 * expire after 12 hours.
 */
public final class JwtUtil {

    private static final Logger log = LoggerFactory.getLogger(JwtUtil.class);
    private static final long TTL_SECONDS = 12 * 60 * 60;

    private static volatile SecretKey key;

    private JwtUtil() {
    }

    /**
     * Loads (or creates) the persistent signing secret. Call once at startup.
     */
    public static synchronized void initialize(Path dataDir) throws IOException {
        if (key != null) {
            return;
        }
        Files.createDirectories(dataDir);
        Path secretFile = dataDir.resolve("jwt-secret.key");
        byte[] secretBytes;
        if (Files.isRegularFile(secretFile)) {
            secretBytes = Base64.getDecoder().decode(Files.readString(secretFile).trim());
        } else {
            secretBytes = new byte[32];
            new SecureRandom().nextBytes(secretBytes);
            Files.writeString(secretFile, Base64.getEncoder().encodeToString(secretBytes));
            log.info("Generated new JWT signing secret at {}", secretFile);
        }
        key = Keys.hmacShaKeyFor(secretBytes);
    }

    public static String generateToken(String username) {
        ensureReady();
        return Jwts.builder()
                .subject(username)
                .issuedAt(new Date())
                .expiration(Date.from(Instant.now().plusSeconds(TTL_SECONDS)))
                .signWith(key)
                .compact();
    }

    /** @return the token subject (username), or {@code null} if the token is invalid/expired. */
    public static String validateToken(String token) {
        ensureReady();
        try {
            Claims claims = Jwts.parser()
                    .verifyWith(key)
                    .build()
                    .parseSignedClaims(token)
                    .getPayload();
            return claims.getSubject();
        } catch (Exception e) {
            log.debug("JWT validation failed: {}", e.getMessage());
            return null;
        }
    }

    private static void ensureReady() {
        if (key == null) {
            throw new IllegalStateException("JwtUtil.initialize(Path) must be called before issuing tokens");
        }
    }
}
