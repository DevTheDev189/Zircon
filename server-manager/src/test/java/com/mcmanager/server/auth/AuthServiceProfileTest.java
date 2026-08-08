package com.mcmanager.server.auth;

import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.io.TempDir;

import java.nio.file.Files;
import java.nio.file.Path;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertNotNull;
import static org.junit.jupiter.api.Assertions.assertNull;
import static org.junit.jupiter.api.Assertions.assertThrows;
import static org.junit.jupiter.api.Assertions.assertTrue;

class AuthServiceProfileTest {

    @TempDir
    Path tempDir;

    private void initWithKnownPassword() throws Exception {
        AuthService.initializeAuth(tempDir);
        AuthService.setPassword("admin", "KnownPass123");
    }

    @Test
    void getUserReturnsProfileWithIconDefault() throws Exception {
        initWithKnownPassword();
        AuthService.UserProfile profile = AuthService.getUser("admin");

        assertNotNull(profile);
        assertEquals("admin", profile.username);
        assertEquals("emerald", profile.icon);
        assertTrue(profile.passwordHash.startsWith("$2"));
    }

    @Test
    void getUserReturnsNullForUnknownUser() throws Exception {
        initWithKnownPassword();
        assertNull(AuthService.getUser("nobody"));
    }

    @Test
    void updateProfileChangesPassword() throws Exception {
        initWithKnownPassword();

        assertTrue(AuthService.updateProfile("admin", "admin", "KnownPass123", "BrandNewPass1", null));
        assertFalse(AuthService.authenticate("admin", "KnownPass123"));
        assertTrue(AuthService.authenticate("admin", "BrandNewPass1"));
    }

    @Test
    void updateProfileRejectsWrongCurrentPassword() throws Exception {
        initWithKnownPassword();

        assertFalse(AuthService.updateProfile("admin", "admin", "wrong-pass", "BrandNewPass1", null));
        assertTrue(AuthService.authenticate("admin", "KnownPass123")); // unchanged
    }

    @Test
    void updateProfileRenamesUserAndMovesCredentials() throws Exception {
        initWithKnownPassword();

        assertTrue(AuthService.updateProfile("admin", "root-admin", "KnownPass123", null, null));

        assertNull(AuthService.getUser("admin"));
        AuthService.UserProfile renamed = AuthService.getUser("root-admin");
        assertNotNull(renamed);
        assertEquals("root-admin", renamed.username);
        assertTrue(AuthService.authenticate("root-admin", "KnownPass123"));
    }

    @Test
    void updateProfileRejectsTakenUsername() throws Exception {
        initWithKnownPassword();
        AuthService.setPassword("other", "OtherPass123");

        assertThrows(Exception.class,
                () -> AuthService.updateProfile("admin", "other", "KnownPass123", null, null));
    }

    @Test
    void updateProfileRejectsShortPassword() throws Exception {
        initWithKnownPassword();

        assertThrows(Exception.class,
                () -> AuthService.updateProfile("admin", "admin", "KnownPass123", "short", null));
        assertTrue(AuthService.authenticate("admin", "KnownPass123"));
    }

    @Test
    void updateProfileSetsIcon() throws Exception {
        initWithKnownPassword();

        assertTrue(AuthService.updateProfile("admin", "admin", "KnownPass123", null, "diamond"));
        assertEquals("diamond", AuthService.getUser("admin").icon);
    }

    @Test
    void migratesLegacyPlainHashUsersJson() throws Exception {
        // Old schema: {"admin": "<bcrypt hash>"} written by earlier versions.
        Path usersFile = tempDir.resolve("users.json");
        Files.writeString(usersFile,
                "{\"admin\": \"" + org.mindrot.jbcrypt.BCrypt.hashpw("LegacyPass1",
                        org.mindrot.jbcrypt.BCrypt.gensalt()) + "\"}");

        AuthService.initializeAuth(tempDir);

        assertTrue(AuthService.authenticate("admin", "LegacyPass1"));
        AuthService.UserProfile profile = AuthService.getUser("admin");
        assertNotNull(profile);
        assertEquals("emerald", profile.icon);
    }
}
