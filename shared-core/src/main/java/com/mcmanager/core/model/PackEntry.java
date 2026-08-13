package com.mcmanager.core.model;

import com.google.gson.annotations.SerializedName;

/**
 * A single shaderpack or resourcepack entry inside a {@link BillOfMaterials}.
 * Clients download the file from {@link #getDownloadUrl()} and verify it against
 * {@link #getSha1()} (or {@link #getMurmur3()} for CurseForge origin) before
 * placing it in the local {@code shaderpacks}/{@code resourcepacks} folder.
 *
 * <p>Unlike {@link ModEntry}, packs are inert data files — presence in the BOM
 * only means the file is available to download, never that it is active in a
 * player's game. Activation is a purely local, per-player choice.
 */
public class PackEntry {

    /** Modrinth project id, CurseForge file id, or a client-generated id for direct uploads. */
    @SerializedName("id")
    private String id;

    /** File name as it must appear on disk, e.g. "ComplementaryShaders.zip". */
    @SerializedName("filename")
    private String filename;

    /** Lower-case hex SHA-1 of the file. */
    @SerializedName("sha1")
    private String sha1;

    /** CurseForge MurmurHash3 fingerprint (only meaningful for CurseForge origin). */
    @SerializedName("murmur3")
    private long murmur3;

    /** One of: "modrinth", "direct". */
    @SerializedName("origin")
    private String origin;

    /** Absolute URL the client downloads the file from. */
    @SerializedName("downloadUrl")
    private String downloadUrl;

    /** File size in bytes. */
    @SerializedName("fileSize")
    private long fileSize;

    /** Display title, falls back to the file name when unset. */
    @SerializedName("title")
    private String title;

    /** Icon URL for the admin UI (Modrinth CDN, etc.). */
    @SerializedName("iconUrl")
    private String iconUrl;

    public PackEntry() {
    }

    public PackEntry(String id, String filename, String sha1, long murmur3, String origin,
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

    public String getTitle() {
        return title;
    }

    public void setTitle(String title) {
        this.title = title;
    }

    public String getIconUrl() {
        return iconUrl;
    }

    public void setIconUrl(String iconUrl) {
        this.iconUrl = iconUrl;
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
        map.put("title", title != null ? title : filename);
        map.put("iconUrl", iconUrl != null ? iconUrl : "");
        return map;
    }

    @Override
    public String toString() {
        return "PackEntry{" + filename + " (" + origin + ")}";
    }
}
