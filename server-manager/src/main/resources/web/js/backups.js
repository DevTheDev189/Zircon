// Zircon admin SPA - Backup creation, scheduling, retention, and restore.
window.Zircon = window.Zircon || {};
window.Zircon.backups = {
        // ---- Backups ----
    async loadBackups() {
        if (!this.selectedInstance) return;
        try {
            const data = await this.api(`/api/instances/${this.selectedInstance.id}/backups`);
            this.backupsList = data.backups || [];
        } catch (e) {
            this.backupsList = [];
        }
    },
    async triggerBackup() {
        if (!this.selectedInstance) return;
        this.creatingBackup = true;
        try {
            await this.api(`/api/instances/${this.selectedInstance.id}/backups`, { method: 'POST' });
            await this.loadBackups();
            alert('Backup created successfully!');
        } catch (e) {
            alert('Backup failed: ' + e.message);
        } finally {
            this.creatingBackup = false;
        }
    },
    async saveBackupSchedule() {
        if (!this.selectedInstance) return;
        try {
            await this.api(`/api/instances/${this.selectedInstance.id}`, {
                method: 'PATCH',
                body: JSON.stringify({
                    backupFrequency: this.backupForm.frequency,
                    backupTime: this.backupForm.time
                })
            });
            alert('Backup schedule saved!');
            await this.loadInstances();
        } catch (e) {
            alert('Failed to save schedule: ' + e.message);
        }
    },
    async saveRetention() {
        if (!this.selectedInstance) return;
        const retention = Number(this.backupForm.retention);
        if (!Number.isInteger(retention) || retention < 1 || retention > 100) {
            alert('Keep backups must be a whole number between 1 and 100.');
            return;
        }
        const current = this.backupsList.length;
        if (current > retention) {
            const toDelete = current - retention;
            if (!confirm(`This will delete ${toDelete} of your ${current} backups, keeping only the ${retention} most recent. Continue?`)) return;
        }
        try {
            const res = await this.api(`/api/instances/${this.selectedInstance.id}/backups/retention`, {
                method: 'POST',
                body: JSON.stringify({ retention })
            });
            if (res.deletedBackups > 0) {
                alert(`Retention set to ${res.retention}. ${res.deletedBackups} old backup(s) deleted.`);
            } else {
                alert('Retention saved.');
            }
            await this.loadBackups();
            await this.loadInstances();
        } catch (e) {
            alert('Failed to save retention: ' + e.message);
        }
    },
    showLogsModal(backup) {
        this.selectedLogBackup = backup;
    },
    async confirmRestore(backup) {
        if (!this.selectedInstance) return;
        if (!confirm(`Are you sure you want to restore the backup from ${this.formatDate(backup.timestamp)}? The current server state will be overwritten!`)) return;
        try {
            await this.api(`/api/instances/${this.selectedInstance.id}/backups/${backup.id}/restore`, { method: 'POST' });
            alert('Backup restored successfully!');
            await this.loadInstances();
            await this.loadBackups();
        } catch (e) {
            alert('Restore failed: ' + e.message);
        }
    },
};
