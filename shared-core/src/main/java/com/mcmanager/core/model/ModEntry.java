package com.mcmanager.core.model;

import com.google.gson.annotations.SerializedName;

/**
 * A single mod entry inside a {@link BillOfMaterials}.
 * Every client downloads the mod from {@link #getDownloadUrl()} and verifies it
 * against either {@link #getSha1()} (Modrinth / direct) or {@link #getMurmur3()}
 * (CurseForge fingerprint) before adding it to the local mods folder.
 */
public class ModEntry {

    /** Modrinth project id, CurseForge file id, or a client-generated id for direct uploads. */
    @SerializedName("id")
    private String id;

    /** File name as it must appear in the client's mods folder, e.g. "sodium-0.5.8.jar". */
    @SerializedName("filename")
    private String filename;

    /** Lower-case hex SHA-1 of the file. */
    @SerializedName("sha1")
    private String sha1;

    /** CurseForge MurmurHash3 fingerprint (only meaningful for CurseForge origin mods). */
    @SerializedName("murmur3")
    private long murmur3;

    /** One of: "modrinth", "curseforge", "direct". */
    @SerializedName("origin")
    private String origin;

    /** Absolute URL the client downloads the JAR from. */
    @SerializedName("downloadUrl")
    private String downloadUrl;

    /** File size in bytes (used for download progress reporting). */
    @SerializedName("fileSize")
    private long fileSize;

    public ModEntry() {
    }

    public ModEntry(String id, String filename, String sha1, long murmur3, String origin,
                    String downloadUrl, long fileSize) {
        this.id = id;
        this.filename = filename;
        this.sha1 = sha1;
        this.murmur3 = murmur3;
        this.origin = origin;
        this.downloadUrl = downloadUrl;
        this.fileSize = fileSize;
    }

    public String getId() {
        return id;
    }

    public void setId(String id) {
        this.id = id;
    }

    public String getFilename() {
        return filename;
    }

    public void setFilename(String filename) {
        this.filename = filename;
    }

    public String getSha1() {
        return sha1;
    }

    public void setSha1(String sha1) {
        this.sha1 = sha1;
    }

    public long getMurmur3() {
        return murmur3;
    }

    public void setMurmur3(long murmur3) {
        this.murmur3 = murmur3;
    }

    public String getOrigin() {
        return origin;
    }

    public void setOrigin(String origin) {
        this.origin = origin;
    }

    public String getDownloadUrl() {
        return downloadUrl;
    }

    public void setDownloadUrl(String downloadUrl) {
        this.downloadUrl = downloadUrl;
    }

    public long getFileSize() {
        return fileSize;
    }

    public void setFileSize(long fileSize) {
        this.fileSize = fileSize;
    }

    /** Plain map view, convenient for serializing to the admin web UI. */
    public java.util.Map<String, Object> toMap() {
        java.util.Map<String, Object> map = new java.util.HashMap<>();
        map.put("id", id);
        map.put("filename", filename);
        map.put("sha1", sha1);
        map.put("murmur3", murmur3);
        map.put("origin", origin);
        map.put("downloadUrl", downloadUrl);
        map.put("fileSize", fileSize);
        return map;
    }

    @Override
    public String toString() {
        return "ModEntry{" + filename + " (" + origin + ")}";
    }
}
