package com.mcmanager.client.profile;

import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.io.TempDir;

import java.io.File;
import java.io.IOException;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.List;
import java.util.Map;
import java.util.Set;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertNull;
import static org.junit.jupiter.api.Assertions.assertTrue;

/**
 * Exercises {@link VersionProfileResolver} against a realistic NeoForge
 * profile — the JSON shape below mirrors the {@code version.json} embedded in
 * the real {@code neoforge-20.4.250} installer (id, inheritsFrom, mainClass,
 * {@code @jar}-suffixed library coordinates, module-path JVM args).
 */
class VersionProfileResolverTest {

    @TempDir
    Path tempDir;

    private final VersionProfileResolver resolver = new VersionProfileResolver();

    private static final String NEOFORGE_PROFILE = """
            {
              "id": "neoforge-20.4.250",
              "time": "2025-08-11T18:14:20Z",
              "type": "release",
              "mainClass": "cpw.mods.bootstraplauncher.BootstrapLauncher",
              "inheritsFrom": "1.20.4",
              "arguments": {
                "game": [
                  "--fml.neoForgeVersion", "20.4.250",
                  "--fml.fmlVersion", "2.0.17",
                  "--launchTarget", "forgeclient"
                ],
                "jvm": [
                  "-DignoreList=securejarhandler-2.1.24.jar,client-extra,neoforge-,${version_name}.jar",
                  "-DlibraryDirectory=${library_directory}",
                  "-p",
                  "${library_directory}/cpw/mods/securejarhandler/2.1.24/securejarhandler-2.1.24.jar${classpath_separator}${library_directory}/org/ow2/asm/asm/9.8/asm-9.8.jar",
                  "--add-modules", "ALL-MODULE-PATH",
                  "--add-opens", "java.base/java.lang.invoke=cpw.mods.securejarhandler"
                ]
              },
              "libraries": [
                {
                  "name": "cpw.mods:securejarhandler:2.1.24@jar",
                  "downloads": {
                    "artifact": {
                      "url": "https://maven.neoforged.net/releases/cpw/mods/securejarhandler/2.1.24/securejarhandler-2.1.24.jar",
                      "path": "cpw/mods/securejarhandler/2.1.24/securejarhandler-2.1.24.jar"
                    }
                  }
                },
                {
                  "name": "net.neoforged:mergetool:2.0.0:api@jar",
                  "downloads": {
                    "artifact": {
                      "url": "https://maven.neoforged.net/releases/net/neoforged/mergetool/2.0.0/mergetool-2.0.0-api.jar",
                      "path": "net/neoforged/mergetool/2.0.0/mergetool-2.0.0-api.jar"
                    }
                  }
                },
                {
                  "name": "com.google.guava:guava:31.1-jre@jar",
                  "downloads": {
                    "artifact": {
                      "url": "https://libraries.minecraft.net/com/google/guava/guava/31.1-jre/guava-31.1-jre.jar",
                      "path": "com/google/guava/guava/31.1-jre/guava-31.1-jre.jar"
                    }
                  }
                }
              ]
            }
            """;

    private static final String VANILLA_PROFILE = """
            {
              "id": "1.20.4",
              "type": "release",
              "mainClass": "net.minecraft.client.main.Main",
              "arguments": {
                "game": [
                  "--username", "${auth_player_name}",
                  "--version", "${version_name}",
                  "--gameDir", "${game_directory}",
                  "--assetsDir", "${assets_root}",
                  "--assetIndex", "${assets_index_name}",
                  "--uuid", "${auth_uuid}",
                  "--accessToken", "${auth_access_token}",
                  "--userType", "${user_type}",
                  "--versionType", "${version_type}",
                  {"rules": [{"action": "allow", "features": {"is_quick_play_multiplayer": true}}],
                   "value": ["--quickPlayMultiplayer", "${quickPlayMultiplayer}"]},
                  {"rules": [{"action": "allow", "features": {"is_demo_user": true}}],
                   "value": "--demo"}
                ],
                "jvm": [
                  "-Djava.library.path=${natives_directory}",
                  "-cp", "${classpath}"
                ]
              },
              "libraries": [
                {
                  "name": "com.google.guava:guava:31.1-jre@jar",
                  "downloads": {"artifact": {"path": "com/google/guava/guava/31.1-jre/guava-31.1-jre.jar",
                                            "url": "https://libraries.minecraft.net/com/google/guava/guava/31.1-jre/guava-31.1-jre.jar"}}
                },
                {
                  "name": "org.lwjgl:lwjgl:3.3.3@jar",
                  "downloads": {"artifact": {"path": "org/lwjgl/lwjgl/3.3.3/lwjgl-3.3.3.jar",
                                            "url": "https://libraries.minecraft.net/org/lwjgl/lwjgl/3.3.3/lwjgl-3.3.3.jar"}}
                }
              ]
            }
            """;

    @Test
    void parsesRealisticNeoForgeProfile() throws IOException {
        VersionProfile profile = parse(NEOFORGE_PROFILE);

        assertEquals("neoforge-20.4.250", profile.getId());
        assertEquals("cpw.mods.bootstraplauncher.BootstrapLauncher", profile.getMainClass());
        assertEquals("1.20.4", profile.getInheritsFrom());
        assertEquals(3, profile.getLibraries().size());
    }

    @Test
    void resolvesArtifactPathsIncludingClassifiers() {
        LibrarySpec secureJar = spec("cpw.mods:securejarhandler:2.1.24@jar",
                "cpw/mods/securejarhandler/2.1.24/securejarhandler-2.1.24.jar");
        assertEquals("cpw/mods/securejarhandler/2.1.24/securejarhandler-2.1.24.jar",
                secureJar.getArtifactPath());

        // classifier "api": artifact-version-api.jar
        LibrarySpec mergetool = spec("net.neoforged:mergetool:2.0.0:api@jar",
                "net/neoforged/mergetool/2.0.0/mergetool-2.0.0-api.jar");
        assertEquals("net/neoforged/mergetool/2.0.0/mergetool-2.0.0-api.jar", mergetool.getArtifactPath());
    }

    @Test
    void derivesMavenPathWhenDownloadsMissing() {
        LibrarySpec lib = new LibrarySpec();
        // No downloads section → path derived from the coordinate.
        try {
            var field = LibrarySpec.class.getDeclaredField("name");
            field.setAccessible(true);
            field.set(lib, "org.ow2.asm:asm:9.8@jar");
        } catch (ReflectiveOperationException e) {
            throw new IllegalStateException(e);
        }

        assertEquals("org/ow2/asm/asm/9.8/asm-9.8.jar", lib.getArtifactPath());
    }

    @Test
    void resolvesInheritanceChainChildFirst() throws IOException {
        VersionProfile root = parse(NEOFORGE_PROFILE);

        List<VersionProfile> chain = resolver.resolveChain(root, id -> {
            assertEquals("1.20.4", id);
            return parseUnchecked(VANILLA_PROFILE);
        });

        assertEquals(2, chain.size());
        assertEquals("neoforge-20.4.250", chain.get(0).getId());
        assertEquals("1.20.4", chain.get(1).getId());
    }

    @Test
    void mergesLibrariesAndDeDuplicatesByCoordinate() throws IOException {
        VersionProfile root = parse(NEOFORGE_PROFILE);

        List<VersionProfile> chain = resolver.resolveChain(root, id -> parseUnchecked(VANILLA_PROFILE));
        List<LibrarySpec> merged = resolver.mergedLibraries(chain);

        // guava appears in both profiles but must be merged to a single entry.
        long guavaCount = merged.stream().filter(l -> l.getName().contains("guava")).count();
        assertEquals(1, guavaCount);
        assertTrue(merged.stream().anyMatch(l -> l.getName().equals("org.lwjgl:lwjgl:3.3.3@jar")));
        assertTrue(merged.stream().anyMatch(l -> l.getName().equals("cpw.mods:securejarhandler:2.1.24@jar")));
    }

    @Test
    void resolvesJvmArgumentsWithTokenSubstitution() throws IOException {
        VersionProfile root = parse(NEOFORGE_PROFILE);
        List<VersionProfile> chain = resolver.resolveChain(root, id -> parseUnchecked(VANILLA_PROFILE));

        Map<String, String> tokens = Map.of(
                "library_directory", "C:/libs",
                "classpath_separator", ";",
                "version_name", "neoforge-20.4.250",
                "natives_directory", "C:/natives");

        List<String> jvmArgs = resolver.resolveJvmArguments(chain, tokens);

        assertTrue(jvmArgs.contains("-DlibraryDirectory=C:/libs"));
        assertTrue(jvmArgs.contains("-p"));
        assertTrue(jvmArgs.contains("C:/libs/cpw/mods/securejarhandler/2.1.24/securejarhandler-2.1.24.jar;C:/libs/org/ow2/asm/asm/9.8/asm-9.8.jar"));
        // The vanilla profile contributes natives dir + -cp ${classpath} template.
        assertTrue(jvmArgs.contains("-Djava.library.path=C:/natives"));
        assertTrue(jvmArgs.contains("-cp"));
        assertTrue(jvmArgs.contains("${classpath}"));
    }

    @Test
    void resolvesGameArgumentsAndHonorsFeatureRules() throws IOException {
        VersionProfile root = parse(NEOFORGE_PROFILE);
        List<VersionProfile> chain = resolver.resolveChain(root, id -> parseUnchecked(VANILLA_PROFILE));

        Map<String, String> tokens = Map.of(
                "auth_player_name", "Steve",
                "quickPlayMultiplayer", "mc.example.com:25565");

        // Quick-play feature enabled → the --quickPlayMultiplayer pair is included.
        List<String> enabled = resolver.resolveGameArguments(chain, tokens, Set.of("is_quick_play_multiplayer"));
        assertTrue(enabled.contains("--quickPlayMultiplayer"));
        assertTrue(enabled.contains("mc.example.com:25565"));
        assertTrue(enabled.contains("--username"));
        assertTrue(enabled.contains("Steve"));
        // The fml args from the loader profile come first.
        assertEquals("--fml.neoForgeVersion", enabled.get(0));
        // --demo is gated behind is_demo_user, which we never enable.
        assertTrue(!enabled.contains("--demo"));

        // Without the feature, quick-play args must not appear.
        List<String> disabled = resolver.resolveGameArguments(chain, tokens, Set.of());
        assertTrue(!disabled.contains("--quickPlayMultiplayer"));
    }

    @Test
    void parseFailsOnMissingFile() {
        File missing = tempDir.resolve("nope.json").toFile();
        org.junit.jupiter.api.Assertions.assertThrows(IOException.class,
                () -> resolver.parseProfile(missing));
    }

    @Test
    void libraryWithUnparseableCoordinateYieldsNullPath() throws ReflectiveOperationException {
        LibrarySpec lib = new LibrarySpec();
        var field = LibrarySpec.class.getDeclaredField("name");
        field.setAccessible(true);
        field.set(lib, "not-a-coordinate");
        assertNull(lib.getArtifactPath());
    }

    // ------------------------------------------------------------------

    private VersionProfile parse(String json) throws IOException {
        Path file = tempDir.resolve("profile-" + System.nanoTime() + ".json");
        Files.writeString(file, json, StandardCharsets.UTF_8);
        return resolver.parseProfile(file.toFile());
    }

    /** {@link #parse} variant for use inside {@code Function} lambdas. */
    private VersionProfile parseUnchecked(String json) {
        try {
            return parse(json);
        } catch (IOException e) {
            throw new RuntimeException(e);
        }
    }

    private LibrarySpec spec(String name, String path) {
        LibrarySpec lib = new LibrarySpec();
        try {
            var nameField = LibrarySpec.class.getDeclaredField("name");
            nameField.setAccessible(true);
            nameField.set(lib, name);
            var downloadsField = LibrarySpec.class.getDeclaredField("downloads");
            downloadsField.setAccessible(true);
            com.google.gson.JsonObject downloads = new com.google.gson.JsonObject();
            com.google.gson.JsonObject artifact = new com.google.gson.JsonObject();
            artifact.addProperty("path", path);
            downloads.add("artifact", artifact);
            downloadsField.set(lib, downloads);
        } catch (ReflectiveOperationException e) {
            throw new IllegalStateException(e);
        }
        return lib;
    }
}
