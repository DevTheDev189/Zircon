package com.mcmanager.core.model;

import com.google.gson.annotations.SerializedName;

import java.util.ArrayList;
import java.util.List;

/**
 * Metadata record for one server instance backup: the {@code .tar.lz4} archive
 * plus an audit trail of what happened while it was created. Persisted as
 * {@code <backupId>.json} next to the archive under
 * {@code <data>/backups/<instanceId>/}.
 */
public class BackupEntry {

    public static final String TRIGGER_MANUAL = "manual";
    public static final String TRIGGER_SCHEDULED = "scheduled";

    public static final String STATUS_IN_PROGRESS = "in_progress";
    public static final String STATUS_COMPLETED = "completed";
    public static final String STATUS_FAILED = "failed";

    @SerializedName("id")
    private String id;

    @SerializedName("instanceId")
    private String instanceId;

    /** Archive file name inside the instance's backups folder. */
    @SerializedName("filename")
    private String filename;

    /** Epoch millis when the backup was created. */
    @SerializedName("timestamp")
    private long timestamp;

    /** Archive size in bytes (0 while in progress or after a failure). */
    @SerializedName("sizeBytes")
    private long sizeBytes;

    /** One of {@link #TRIGGER_MANUAL} or {@link #TRIGGER_SCHEDULED}. */
    @SerializedName("triggerType")
    private String triggerType;

    /** One of {@link #STATUS_IN_PROGRESS}, {@link #STATUS_COMPLETED}, {@link #STATUS_FAILED}. */
    @SerializedName("status")
    private String status;

    /** Human-readable audit trail (flush commands, file counts, errors). */
    @SerializedName("logs")
    private List<String> logs = new ArrayList<>();

    /** Gson deserialization. */
    public BackupEntry() {
    }

    public String getId() {
        return id;
    }

    public void setId(String id) {
        this.id = id;
    }

    public String getInstanceId() {
        return instanceId;
    }

    public void setInstanceId(String instanceId) {
        this.instanceId = instanceId;
    }

    public String getFilename() {
        return filename;
    }

    public void setFilename(String filename) {
        this.filename = filename;
    }

    public long getTimestamp() {
        return timestamp;
    }

    public void setTimestamp(long timestamp) {
        this.timestamp = timestamp;
    }

    public long getSizeBytes() {
        return sizeBytes;
    }

    public void setSizeBytes(long sizeBytes) {
        this.sizeBytes = sizeBytes;
    }

    public String getTriggerType() {
        return triggerType;
    }

    public void setTriggerType(String triggerType) {
        this.triggerType = triggerType;
    }

    public String getStatus() {
        return status;
    }

    public void setStatus(String status) {
        this.status = status;
    }

    public List<String> getLogs() {
        return logs;
    }

    public void setLogs(List<String> logs) {
        this.logs = logs != null ? logs : new ArrayList<>();
    }

    @Override
    public String toString() {
        return "BackupEntry{id=" + id + ", instance=" + instanceId
                + ", status=" + status + ", trigger=" + triggerType + "}";
    }
}
