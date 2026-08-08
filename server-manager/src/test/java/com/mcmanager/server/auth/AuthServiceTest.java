package com.mcmanager.server.auth;

import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.io.TempDir;

import java.nio.file.Files;
import java.nio.file.Path;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertTrue;

class AuthServiceTest {

    @TempDir
    Path tempDir;

    @Test
    void createsUsersJsonWithHashedPasswordOnFirstRun() throws Exception {
        AuthService.initializeAuth(tempDir);

        Path usersFile = tempDir.resolve("users.json");
        assertTrue(Files.exists(usersFile));
        String content = Files.readString(usersFile);
        // Never store the password in plain text.
        assertFalse(content.contains("Password"));
        assertFalse(content.contains("\"admin\": \"admin\""));
        assertTrue(content.contains("\"admin\""));

        // The generated password is random, so we cannot know it — but
        // authenticate() must round-trip with BCrypt regardless.
        assertFalse(AuthService.authenticate("admin", "wrong-password"));
        assertFalse(AuthService.authenticate("nobody", "whatever"));
    }

    @Test
    void changePasswordRequiresCurrentPassword() throws Exception {
        AuthService.initializeAuth(tempDir);
        // Set a known password to test the flow deterministically.
        AuthService.setPassword("admin", "KnownPass123");

        assertTrue(AuthService.authenticate("admin", "KnownPass123"));

        assertFalse(AuthService.changePassword("admin", "WrongPass", "NewPass123"));
        assertTrue(AuthService.authenticate("admin", "KnownPass123"));

        assertTrue(AuthService.changePassword("admin", "KnownPass123", "NewPass456"));
        assertFalse(AuthService.authenticate("admin", "KnownPass123"));
        assertTrue(AuthService.authenticate("admin", "NewPass456"));
    }

    @Test
    void initializesWithoutOverwritingExistingUsers() throws Exception {
        AuthService.initializeAuth(tempDir);
        AuthService.setPassword("admin", "FirstPass1");
        String before = Files.readString(tempDir.resolve("users.json"));
        // Simulate a restart: re-initialize on the same dir.
        AuthService.initializeAuth(tempDir);

        assertTrue(AuthService.authenticate("admin", "FirstPass1"));
        // The file must not have been regenerated (which would reset the password).
        assertEquals(before, Files.readString(tempDir.resolve("users.json")));
    }
}
