// Zircon admin SPA - Server instance lifecycle (create, start, stop, restart, delete, settings).
window.Zircon = window.Zircon || {};
window.Zircon.instances = {
    async loadInstances() {
        const data = await this.api('/api/instances');
        this.instances = data.instances || [];
        if (this.instances.length > 0 && !this.selectedInstance) this.selectInstance(this.instances[0]);
        if (this.selectedInstance) {
            const fresh = this.instances.find(i => i.id === this.selectedInstance.id);
            if (fresh) Object.assign(this.selectedInstance, fresh);
        }
    },
    selectInstance(inst) {
        this.selectedInstance = inst;
        this.settingsForm = {
            name: inst.name,
            mcVersion: inst.minecraftVersion,
            loaderVersion: inst.modLoader ? inst.modLoader.version : '',
            javaArgs: inst.javaArgs || '',
            externalPort: inst.externalPort || null
        };
        this.loadMods();
        this.loadServerProperties();
        this.playersLoaded = false; // first load of the new instance shows the spinner
        this.loadPlayers();
        this.loadBackups();
        this.backupForm = {
            frequency: inst.backupFrequency || 'off',
            time: inst.backupTime || '02:00',
            retention: inst.backupRetention || 10
        };
        // The Stats view hides the top bar, so picking a server there must
        // also navigate back into the instance pages (mods is the landing tab).
        if (this.activeTab === 'stats') this.activeTab = 'mods';
    },
    async refreshSelectedInstance() {
        if (!this.selectedInstance) return;
        try {
            const data = await this.api(`/api/instances/${this.selectedInstance.id}`);
            Object.assign(this.selectedInstance, data);
        } catch (e) { /* instance may have been deleted */ }
    },
    async createNewServer() {
        try {
            await this.api('/api/instances', {
                method: 'POST',
                body: JSON.stringify(this.newServerForm)
            });
            this.showAddServerModal = false;
            this.newServerForm = { name: '', mcVersion: '1.21.4', loaderType: 'fabric', loaderVersion: '' };
            await this.loadInstances();
        } catch (e) {
            alert('Create failed: ' + e.message);
        }
    },
    async startInstance(inst) {
        inst = inst || this.selectedInstance;
        if (!inst) return;
        try {
            await this.api(`/api/instances/${inst.id}/start`, { method: 'POST' });
            await this.loadInstances();
        } catch (e) {
            if ((e.message || '').toLowerCase().includes('eula')) {
                this.eulaInstance = inst;
                this.showEulaModal = true;
            } else {
                alert('Start failed: ' + e.message);
            }
        }
    },
    async stopInstance(inst) {
        inst = inst || this.selectedInstance;
        if (!inst) return;
        await this.api(`/api/instances/${inst.id}/stop`, { method: 'POST' });
        await this.loadInstances();
    },
    async restartInstance(inst) {
        inst = inst || this.selectedInstance;
        if (!inst) return;
        try {
            await this.api(`/api/instances/${inst.id}/restart`, { method: 'POST' });
            alert(`Server "${inst.name}" is restarting...`);
            setTimeout(() => this.loadInstances(), 2000);
        } catch (e) {
            alert('Restart failed: ' + e.message);
        }
    },
    async acceptAndStart() {
        if (!this.eulaInstance) return;
        try {
            await this.api(`/api/instances/${this.eulaInstance.id}/eula`, {
                method: 'POST',
                body: JSON.stringify({ accepted: true })
            });
            this.showEulaModal = false;
            const id = this.eulaInstance.id;
            this.eulaInstance = null;
            await this.api(`/api/instances/${id}/start`, { method: 'POST' });
            await this.refreshSelectedInstance();
        } catch (e) {
            alert('Could not accept EULA / start: ' + e.message);
        }
    },
    async confirmDeleteInstance() {
        const inst = this.selectedInstance;
        if (!inst) return;
        try {
            await this.api(`/api/instances/${inst.id}`, { method: 'DELETE' });
            this.showDeleteModal = false;
            this.instances = this.instances.filter(i => i.id !== inst.id);
            if (this.selectedInstance && this.selectedInstance.id === inst.id) {
                this.selectedInstance = null;
            }
            if (this.instances.length > 0) {
                this.selectInstance(this.instances[0]);
            } else {
                this.installedMods = [];
                this.whitelistPlayers = [];
                this.opPlayers = [];
                this.bannedPlayers = [];
                this.playerHistory = [];
                this.serverProps = {};
                this.whitelistEnabled = false;
            }
            alert(`Deleted "${inst.name}".`);
        } catch (e) {
            alert('Delete failed: ' + e.message);
        }
    },
    async saveInstanceSettings() {
        try {
            const res = await this.api(`/api/instances/${this.selectedInstance.id}`, {
                method: 'PATCH',
                body: JSON.stringify({
                    name: this.settingsForm.name,
                    mcVersion: this.settingsForm.mcVersion,
                    loaderVersion: this.settingsForm.loaderVersion,
                    javaArgs: this.settingsForm.javaArgs,
                    // 0 / blank leaves the player-facing port unchanged.
                    externalPort: Number(this.settingsForm.externalPort) || 0
                })
            });
            alert(`Instance updated! ${res.updatedCount || 0} mods auto-updated, ${res.incompatibleCount || 0} flagged incompatible.`);
            await this.loadInstances();
            await this.loadMods();
        } catch (e) { alert('Update failed: ' + e.message); }
    },
};
