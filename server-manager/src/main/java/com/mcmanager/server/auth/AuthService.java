package com.mcmanager.server.auth;

import com.google.gson.Gson;
import com.google.gson.GsonBuilder;
import com.google.gson.reflect.TypeToken;
import org.mindrot.jbcrypt.BCrypt;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

import java.io.IOException;
import java.lang.reflect.Type;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.security.SecureRandom;
import java.util.LinkedHashMap;
import java.util.Map;

/**
 * Admin credential store: {@code users.json} (username → {@link UserProfile}) in
 * the data dir. A profile holds the BCrypt password hash plus display metadata
 * ({@code icon}) so the admin UI can personalize the header.
 *
 * <p>On first run a random 16-character admin password is generated, stored as
 * a BCrypt hash and printed to stdout — the operator copies it into the admin
 * web UI, then should change it. Passwords are never stored in plain text.
 *
 * <p>Files written by older versions (plain {@code "user": "hash"} maps) are
 * migrated to the profile schema transparently on load.
 */
public class AuthService {

    private static final Logger log = LoggerFactory.getLogger(AuthService.class);
    private static final Gson GSON = new GsonBuilder().setPrettyPrinting().create();
    private static final Type USERS_TYPE = new TypeToken<Map<String, UserProfile>>() {
    }.getType();
    private static final Type LEGACY_USERS_TYPE = new TypeToken<Map<String, String>>() {
    }.getType();

    private static final String ALPHABET =
            "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789!@#$%^&*";

    private static volatile Path usersFile;
    private static volatile Map<String, UserProfile> users = new LinkedHashMap<>();

    /** Serializable admin profile stored in {@code users.json}. */
    public static class UserProfile {
        public String username;
        public String passwordHash;
        public String icon = "emerald";

        public UserProfile() {
        }

        public UserProfile(String username, String passwordHash, String icon) {
            this.username = username;
            this.passwordHash = passwordHash;
            this.icon = icon != null && !icon.isBlank() ? icon : "emerald";
        }
    }

    private AuthService() {
    }

    /**
     * Ensures {@code users.json} exists, creating the initial {@code admin}
     * account with a random password (printed to stdout) when it does not.
     */
    public static void initializeAuth(Path dataDir) throws IOException {
        Files.createDirectories(dataDir);
        usersFile = dataDir.resolve("users.json");
        if (Files.exists(usersFile)) {
            users = load(usersFile);
            return;
        }

        String initialPassword = generateRandomPassword(16);
        String hashedPassword = BCrypt.hashpw(initialPassword, BCrypt.gensalt(12));
        users = new LinkedHashMap<>();
        users.put("admin", new UserProfile("admin", hashedPassword, "emerald"));
        save();

        System.out.println("=================================================");
        System.out.println("  ZIRCON SERVER CREATED INITIAL ADMIN USER");
        System.out.println("  Username: admin");
        System.out.println("  Password: " + initialPassword);
        System.out.println("  Please log in and change your password!");
        System.out.println("=================================================");
        log.info("Created initial admin user; password printed to stdout");
    }

    /** Verifies a username/password pair against the stored BCrypt hashes. */
    public static synchronized boolean authenticate(String username, String password) {
        UserProfile user = users.get(username);
        return user != null && password != null && BCrypt.checkpw(password, user.passwordHash);
    }

    /** @return the stored profile for a username, or {@code null} if unknown. */
    public static synchronized UserProfile getUser(String username) {
        return users.get(username);
    }

    /**
     * Atomically updates a profile: optionally renames the account, changes the
     * password and/or updates the display icon. The current password is always
     * required as proof of identity.
     *
     * @return {@code true} on success, {@code false} if credentials were wrong.
     * @throws IOException if the new username is taken or the new password is too short.
     */
    public static synchronized boolean updateProfile(String currentUsername, String newUsername,
                                                     String currentPassword, String newPassword,
                                                     String newIcon) throws IOException {
        if (!authenticate(currentUsername, currentPassword)) {
            return false;
        }
        UserProfile profile = users.get(currentUsername);
        if (profile == null) {
            return false;
        }

        String targetUser = (newUsername != null && !newUsername.isBlank())
                ? newUsername.trim() : currentUsername;

        if (!targetUser.equalsIgnoreCase(currentUsername) && users.containsKey(targetUser)) {
            throw new IOException("Username '" + targetUser + "' is already taken");
        }

        if (newPassword != null && !newPassword.isBlank()) {
            if (newPassword.length() < 8) {
                throw new IOException("New password must be at least 8 characters");
            }
            profile.passwordHash = BCrypt.hashpw(newPassword, BCrypt.gensalt(12));
        }

        if (newIcon != null && !newIcon.isBlank()) {
            profile.icon = newIcon.trim();
        }

        if (!targetUser.equals(currentUsername)) {
            users.remove(currentUsername);
            profile.username = targetUser;
            users.put(targetUser, profile);
        }

        save();
        log.info("Profile updated for user {}", targetUser);
        return true;
    }

    /**
     * Changes a password after verifying the current one.
     *
     * @return {@code true} on success, {@code false} if credentials were wrong.
     */
    public static synchronized boolean changePassword(String username, String currentPassword,
                                                      String newPassword) throws IOException {
        return updateProfile(username, username, currentPassword, newPassword, null);
    }

    /** Sets a password without verification (used by tests / initial setup). */
    public static synchronized void setPassword(String username, String newPassword) throws IOException {
        UserProfile profile = users.get(username);
        if (profile == null) {
            profile = new UserProfile(username, "", "emerald");
            users.put(username, profile);
        }
        profile.passwordHash = BCrypt.hashpw(newPassword, BCrypt.gensalt(12));
        save();
    }

    // ------------------------------------------------------------------
    // persistence
    // ------------------------------------------------------------------

    private static Map<String, UserProfile> load(Path file) {
        // Preferred schema: {"user": {"username":..., "passwordHash":..., "icon":...}}.
        try {
            String content = Files.readString(file, StandardCharsets.UTF_8);
            Map<String, UserProfile> parsed = GSON.fromJson(content, USERS_TYPE);
            if (parsed != null && !parsed.isEmpty()) {
                return new LinkedHashMap<>(parsed);
            }
        } catch (IOException | RuntimeException e) {
            log.warn("Could not parse {} as profile map, trying legacy schema", file);
        }
        // Legacy schema: {"user": "<bcrypt hash>"} — migrate in place.
        try {
            String content = Files.readString(file, StandardCharsets.UTF_8);
            Map<String, String> legacy = GSON.fromJson(content, LEGACY_USERS_TYPE);
            if (legacy != null && !legacy.isEmpty()) {
                Map<String, UserProfile> migrated = new LinkedHashMap<>();
                legacy.forEach((name, hash) -> {
                    if (hash != null && !hash.isBlank()) {
                        migrated.put(name, new UserProfile(name, hash, "emerald"));
                    }
                });
                if (!migrated.isEmpty()) {
                    log.info("Migrated {} legacy user(s) to the profile schema", migrated.size());
                    return migrated;
                }
            }
        } catch (IOException | RuntimeException e) {
            log.warn("Could not parse {} at all, starting with no users", file, e);
        }
        return new LinkedHashMap<>();
    }

    private static void save() throws IOException {
        if (usersFile == null) {
            throw new IllegalStateException("initializeAuth(Path) must be called first");
        }
        Files.writeString(usersFile, GSON.toJson(users), StandardCharsets.UTF_8);
    }

    private static String generateRandomPassword(int length) {
        SecureRandom random = new SecureRandom();
        StringBuilder sb = new StringBuilder(length);
        for (int i = 0; i < length; i++) {
            sb.append(ALPHABET.charAt(random.nextInt(ALPHABET.length())));
        }
        return sb.toString();
    }
}
