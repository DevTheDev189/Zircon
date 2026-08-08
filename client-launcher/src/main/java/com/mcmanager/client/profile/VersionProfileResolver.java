package com.mcmanager.client.profile;

import com.google.gson.Gson;
import com.google.gson.JsonArray;
import com.google.gson.JsonElement;
import com.google.gson.JsonObject;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

import java.io.File;
import java.io.IOException;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.util.ArrayList;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Locale;
import java.util.Map;
import java.util.function.Function;

/**
 * Parses version profile JSON files (the ones the Forge/NeoForge installers
 * write to {@code versions/<id>/<id>.json}) and resolves the launch inputs they
 * describe: the inherited-profile chain, the merged library set, the JVM
 * arguments and the game arguments — with {@code ${token}} placeholders
 * substituted.
 */
public class VersionProfileResolver {

    private static final Logger log = LoggerFactory.getLogger(VersionProfileResolver.class);

    private final Gson gson = new Gson();

    public VersionProfile parseProfile(File versionJsonFile) throws IOException {
        return gson.fromJson(Files.readString(versionJsonFile.toPath(), StandardCharsets.UTF_8),
                VersionProfile.class);
    }

    // ------------------------------------------------------------------
    // Inheritance chain
    // ------------------------------------------------------------------

    /**
     * Walks the {@code inheritsFrom} chain from {@code root} towards vanilla,
     * loading parents through {@code parentLoader} (which typically parses the
     * vanilla version profile already fetched from Mojang's manifest).
     *
     * @return the chain ordered child-first, ending at the profile with no parent.
     */
    public List<VersionProfile> resolveChain(VersionProfile root,
                                             Function<String, VersionProfile> parentLoader) {
        List<VersionProfile> chain = new ArrayList<>();
        VersionProfile current = root;
        java.util.Set<String> seen = new java.util.HashSet<>();
        while (current != null && seen.add(current.getId())) {
            chain.add(current);
            String parent = current.getInheritsFrom();
            if (parent == null || parent.isBlank()) {
                break;
            }
            try {
                current = parentLoader.apply(parent);
            } catch (RuntimeException e) {
                log.warn("Could not resolve inherited profile '{}' of '{}': {}",
                        parent, current.getId(), e.getMessage());
                break;
            }
        }
        return chain;
    }

    /**
     * Merges the libraries of every profile in the chain (child first), skipping
     * libraries whose OS {@code rules} disallow them and de-duplicating by Maven
     * coordinate name.
     */
    public List<LibrarySpec> mergedLibraries(List<VersionProfile> chain) {
        Map<String, LibrarySpec> merged = new LinkedHashMap<>();
        for (VersionProfile profile : chain) {
            if (profile.getLibraries() == null) {
                continue;
            }
            for (LibrarySpec lib : profile.getLibraries()) {
                if (!libraryAllowed(lib)) {
                    continue;
                }
                merged.putIfAbsent(lib.getName(), lib);
            }
        }
        return new ArrayList<>(merged.values());
    }

    // ------------------------------------------------------------------
    // Arguments
    // ------------------------------------------------------------------

    /**
     * Resolves the JVM arguments of the profile chain (child first) with
     * {@code tokens} substituted. Argument entries may be plain strings or rule
     * objects ({@code {"rules": [...], "value": "..."}}), mirroring the Mojang
     * version JSON schema.
     */
    public List<String> resolveJvmArguments(List<VersionProfile> chain, Map<String, String> tokens) {
        List<String> args = new ArrayList<>();
        for (VersionProfile profile : chain) {
            if (profile.getArguments() == null) {
                continue;
            }
            collectArguments(profile.getArguments().get("jvm"), tokens, args, java.util.Set.of());
        }
        return args;
    }

    /**
     * Same as {@link #resolveJvmArguments} but for the game arguments.
     *
     * @param enabledFeatures launcher features for {@code rules.features} gating,
     *                        e.g. {@code "is_quick_play_multiplayer"} when the
     *                        launcher auto-connects the player to a server
     */
    public List<String> resolveGameArguments(List<VersionProfile> chain, Map<String, String> tokens,
                                             java.util.Set<String> enabledFeatures) {
        List<String> args = new ArrayList<>();
        for (VersionProfile profile : chain) {
            if (profile.getArguments() != null) {
                collectArguments(profile.getArguments().get("game"), tokens, args, enabledFeatures);
            }
            // Pre-1.13 profiles carry game args as a single space-separated string.
            if (profile.getMinecraftArguments() != null && !profile.getMinecraftArguments().isBlank()) {
                args.add(substitute(profile.getMinecraftArguments(), tokens));
            }
        }
        return args;
    }

    private void collectArguments(JsonElement section, Map<String, String> tokens, List<String> out,
                                  java.util.Set<String> enabledFeatures) {
        if (section == null || !section.isJsonArray()) {
            return;
        }
        for (JsonElement element : section.getAsJsonArray()) {
            if (element.isJsonPrimitive() && element.getAsJsonPrimitive().isString()) {
                out.add(substitute(element.getAsString(), tokens));
            } else if (element.isJsonObject()) {
                JsonObject obj = element.getAsJsonObject();
                if (!argumentAllowed(obj, enabledFeatures)) {
                    continue;
                }
                JsonElement value = obj.get("value");
                if (value == null) {
                    continue;
                }
                if (value.isJsonPrimitive() && value.getAsJsonPrimitive().isString()) {
                    out.add(substitute(value.getAsString(), tokens));
                } else if (value.isJsonArray()) {
                    // Rule objects may carry an array of values (e.g. --width/--height).
                    for (JsonElement item : value.getAsJsonArray()) {
                        if (item.isJsonPrimitive() && item.getAsJsonPrimitive().isString()) {
                            out.add(substitute(item.getAsString(), tokens));
                        }
                    }
                }
            }
        }
    }

    private boolean argumentAllowed(JsonObject obj, java.util.Set<String> enabledFeatures) {
        if (!obj.has("rules")) {
            return true;
        }
        return rulesAllow(obj.getAsJsonArray("rules"), enabledFeatures);
    }

    // ------------------------------------------------------------------
    // Rules / OS matching
    // ------------------------------------------------------------------

    private boolean libraryAllowed(LibrarySpec lib) {
        if (lib.getRules() == null) {
            return true;
        }
        return rulesAllow(lib.getRules(), java.util.Set.of());
    }

    private boolean rulesAllow(JsonArray rules, java.util.Set<String> enabledFeatures) {
        if (rules == null || rules.isEmpty()) {
            return true;
        }
        boolean allow = false;
        for (JsonElement ruleEl : rules) {
            JsonObject rule = ruleEl.getAsJsonObject();
            if (!featuresMatch(rule, enabledFeatures)) {
                continue;
            }
            JsonElement os = rule.get("os");
            boolean applies = os == null || osMatches(os.getAsJsonObject());
            if (applies) {
                allow = "allow".equals(rule.get("action").getAsString());
            }
        }
        return allow;
    }

    /**
     * A {@code features} rule applies only when every declared feature matches
     * the launcher's enabled feature set (e.g. {@code is_quick_play_multiplayer}).
     */
    private boolean featuresMatch(JsonObject rule, java.util.Set<String> enabledFeatures) {
        JsonElement features = rule.get("features");
        if (features == null || !features.isJsonObject()) {
            return true;
        }
        for (Map.Entry<String, JsonElement> entry : features.getAsJsonObject().entrySet()) {
            boolean expected = entry.getValue().getAsBoolean();
            if (enabledFeatures.contains(entry.getKey()) != expected) {
                return false;
            }
        }
        return true;
    }

    private boolean osMatches(JsonObject os) {
        if (os == null) {
            return true;
        }
        String osName = System.getProperty("os.name").toLowerCase(Locale.ROOT);
        if (os.has("name")) {
            String osTarget = os.get("name").getAsString();
            boolean match = switch (osTarget) {
                case "windows" -> osName.contains("win");
                case "linux" -> osName.contains("linux");
                case "osx" -> osName.contains("mac");
                default -> false;
            };
            if (!match) {
                return false;
            }
        }
        if (os.has("arch")) {
            String arch = os.get("arch").getAsString();
            String actual = System.getProperty("os.arch").toLowerCase(Locale.ROOT);
            boolean match = switch (arch) {
                case "x86" -> actual.contains("86") && !actual.contains("64");
                case "x86_64" -> actual.contains("64");
                case "arm64" -> actual.contains("aarch64") || actual.contains("arm64");
                default -> false;
            };
            if (!match) {
                return false;
            }
        }
        return true;
    }

    // ------------------------------------------------------------------
    // Token substitution
    // ------------------------------------------------------------------

    /**
     * Replaces {@code ${key}} placeholders using {@code tokens}. Unknown
     * placeholders are left untouched (they may belong to a newer profile
     * format; substituting garbage would be worse).
     */
    public static String substitute(String template, Map<String, String> tokens) {
        String result = template;
        for (Map.Entry<String, String> entry : tokens.entrySet()) {
            String value = entry.getValue() == null ? "" : entry.getValue();
            result = result.replace("${" + entry.getKey() + "}", value);
        }
        return result;
    }
}
