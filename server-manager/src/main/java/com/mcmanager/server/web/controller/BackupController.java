package com.mcmanager.server.web.controller;

import com.mcmanager.core.model.BackupEntry;
import com.mcmanager.server.service.BackupService;
import io.javalin.http.Context;

import java.io.FileNotFoundException;
import java.io.IOException;
import java.util.List;
import java.util.Map;

/**
 * REST endpoints for instance backups: list the audit trail, trigger a manual
 * backup, and restore an archive over the instance directory.
 */
public class BackupController {

    private final BackupService backupService;

    public BackupController(BackupService backupService) {
        this.backupService = backupService;
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
}
