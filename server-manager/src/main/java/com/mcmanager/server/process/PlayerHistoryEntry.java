package com.mcmanager.server.process;

import com.google.gson.annotations.SerializedName;

import java.util.HashMap;
import java.util.Map;

/**
 * One entry of the persistent "players who have ever joined" log. Serialized to
 * {@code <instance>/players.json} by {@link PlayerTracker} and served to the
 * admin UI; keyed by player name (case-insensitive) with activity stats.
 */
public class PlayerHistoryEntry {

    @SerializedName("name")
    private String name;

    /** Epoch millis of the first join ever observed. */
    @SerializedName("firstJoined")
    private long firstJoined;

    /** Epoch millis of the most recent join. */
    @SerializedName("lastJoined")
    private long lastJoined;

    /** Number of times the player has joined. */
    @SerializedName("joinCount")
    private int joinCount;

    /** Gson deserialization. */
    public PlayerHistoryEntry() {
    }

    public String getName() {
        return name;
    }

    public void setName(String name) {
        this.name = name;
    }

    public long getFirstJoined() {
        return firstJoined;
    }

    public void setFirstJoined(long firstJoined) {
        this.firstJoined = firstJoined;
    }

    public long getLastJoined() {
        return lastJoined;
    }

    public void setLastJoined(long lastJoined) {
        this.lastJoined = lastJoined;
    }

    public int getJoinCount() {
        return joinCount;
    }

    public void setJoinCount(int joinCount) {
        this.joinCount = joinCount;
    }

    /** Plain map view for the admin web UI. */
    public Map<String, Object> toMap() {
        Map<String, Object> map = new HashMap<>();
        map.put("name", name);
        map.put("firstJoined", firstJoined);
        map.put("lastJoined", lastJoined);
        map.put("joinCount", joinCount);
        return map;
    }

    @Override
    public String toString() {
        return "PlayerHistoryEntry{name='" + name + "', joins=" + joinCount
                + ", last=" + lastJoined + "}";
    }
}
