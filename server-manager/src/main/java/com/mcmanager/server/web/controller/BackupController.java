package com.mcmanager.server.web.controller;

import com.mcmanager.core.model.BackupEntry;
import com.mcmanager.core.model.InstanceConfig;
import com.mcmanager.server.instance.ServerInstanceManager;
import com.mcmanager.server.service.BackupService;
import io.javalin.http.Context;

import java.io.FileNotFoundException;
import java.io.IOException;
import java.util.List;
import java.util.Map;

/**
 * REST endpoints for instance backups: list the audit trail, trigger a manual
 * backup, restore an archive over the instance directory, and configure how
 * many backups are kept.
 */
public class BackupController {

    private final BackupService backupService;
    private final ServerInstanceManager instanceManager;

    public BackupController(BackupService backupService, ServerInstanceManager instanceManager) {
        this.backupService = backupService;
        this.instanceManager = instanceManager;
    }

    /** GET /api/instances/{id}/backups */
    public void listBackups(Context ctx) {
        String instanceId = ctx.pathParam("id");
        List<BackupEntry> backups = backupService.listBackups(instanceId);
        ctx.json(Map.of("backups", backups));
    }

    /** POST /api/instances/{id}/backups — creates a manual backup */
    public void createBackup(Context ctx) {
        String instanceId = ctx.pathParam("id");
        try {
            BackupEntry entry = backupService.createBackup(instanceId, BackupEntry.TRIGGER_MANUAL);
            ctx.status(201).json(entry);
        } catch (IllegalArgumentException e) {
            ctx.status(404).result(e.getMessage());
        } catch (IOException e) {
            ctx.status(500).result("Backup creation failed: " + e.getMessage());
        }
    }

    /** POST /api/instances/{id}/backups/{backupId}/restore */
    public void restoreBackup(Context ctx) {
        String instanceId = ctx.pathParam("id");
        String backupId = ctx.pathParam("backupId");
        try {
            backupService.restoreBackup(instanceId, backupId);
            ctx.json(Map.of("ok", true, "message", "Backup restored successfully."));
        } catch (FileNotFoundException | IllegalArgumentException e) {
            ctx.status(404).result(e.getMessage());
        } catch (IOException e) {
            ctx.status(500).result("Restore failed: " + e.getMessage());
        }
    }

    /** POST /api/instances/{id}/backups/retention — body: {retention: N} */
    public void setRetention(Context ctx) {
        String instanceId = ctx.pathParam("id");
        RetentionRequest body;
        try {
            body = ctx.bodyAsClass(RetentionRequest.class);
        } catch (RuntimeException e) {
            ctx.status(400).result("Invalid JSON body");
            return;
        }
        if (body == null || body.retention == null) {
            ctx.status(400).result("retention is required");
            return;
        }
        try {
            instanceManager.getInstance(instanceId); // 404 for unknown ids
            if (body.retention < InstanceConfig.MIN_BACKUP_RETENTION
                    || body.retention > InstanceConfig.MAX_BACKUP_RETENTION) {
                ctx.status(400).result("retention must be between "
                        + InstanceConfig.MIN_BACKUP_RETENTION + " and "
                        + InstanceConfig.MAX_BACKUP_RETENTION);
                return;
            }
            int deleted = backupService.setRetention(instanceId, body.retention);
            ctx.json(Map.of("retention", body.retention, "deletedBackups", deleted));
        } catch (IllegalArgumentException e) {
            ctx.status(404).result(e.getMessage());
        } catch (IOException e) {
            ctx.status(500).result("Failed to save retention: " + e.getMessage());
        }
    }

    public static class RetentionRequest {
        public Integer retention;
    }
}
