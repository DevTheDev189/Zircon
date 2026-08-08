package com.mcmanager.client.model;

import com.google.gson.Gson;
import com.google.gson.reflect.TypeToken;

import java.io.IOException;
import java.lang.reflect.Type;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.Comparator;
import java.util.List;

/**
 * Represents a saved/played-on server entry in the client launcher.
 * Persisted in {@code ~/.mcmanager/servers.json}.
 */
public class SavedServer {

    private String name;
    private String address;
    private long lastPlayed;

    public SavedServer() {
    }

    public SavedServer(String name, String address, long lastPlayed) {
        this.name = name;
        this.address = address;
        this.lastPlayed = lastPlayed;
    }

    public String getName() {
        return name;
    }

    public void setName(String name) {
        this.name = name;
    }

    public String getAddress() {
        return address;
    }

    public void setAddress(String address) {
        this.address = address;
    }

    public long getLastPlayed() {
        return lastPlayed;
    }

    public void setLastPlayed(long lastPlayed) {
        this.lastPlayed = lastPlayed;
    }

    private static final Path SERVERS_FILE = Path.of(System.getProperty("user.home"), ".mcmanager", "servers.json");
    private static final Gson GSON = new Gson();

    public static List<SavedServer> load() {
        if (!Files.isRegularFile(SERVERS_FILE)) {
            return new ArrayList<>();
        }
        try {
            String json = Files.readString(SERVERS_FILE, StandardCharsets.UTF_8);
            Type type = new TypeToken<List<SavedServer>>() {}.getType();
            List<SavedServer> list = GSON.fromJson(json, type);
            if (list != null) {
                list.sort(Comparator.comparingLong(SavedServer::getLastPlayed).reversed());
                return list;
            }
        } catch (Exception ignored) {
        }
        return new ArrayList<>();
    }

    public static void save(List<SavedServer> servers) {
        try {
            Files.createDirectories(SERVERS_FILE.getParent());
            servers.sort(Comparator.comparingLong(SavedServer::getLastPlayed).reversed());
            Files.writeString(SERVERS_FILE, GSON.toJson(servers), StandardCharsets.UTF_8);
        } catch (IOException ignored) {
        }
    }

    public static void recordPlayed(String name, String address) {
        List<SavedServer> servers = load();
        SavedServer existing = null;
        for (SavedServer s : servers) {
            if (s.getAddress().equalsIgnoreCase(address.trim())) {
                existing = s;
                break;
            }
        }
        if (existing != null) {
            if (name != null && !name.isBlank()) {
                existing.setName(name.trim());
            }
            existing.setLastPlayed(System.currentTimeMillis());
        } else {
            String serverName = (name != null && !name.isBlank()) ? name.trim() : address.trim();
            servers.add(new SavedServer(serverName, address.trim(), System.currentTimeMillis()));
        }
        save(servers);
    }
}
