// Zircon admin SPA - Server instance lifecycle (create, start, stop, restart, delete, settings).
window.Zircon = window.Zircon || {};
window.Zircon.instances = {
    async loadInstances() {
        try {
            const data = await this.api('/api/instances');
            this.instances = data.instances || [];
            if (this.instances.length > 0 && !this.selectedInstance) this.selectInstance(this.instances[0]);
            if (this.selectedInstance) {
                const fresh = this.instances.find(i => i.id === this.selectedInstance.id);
                if (fresh) Object.assign(this.selectedInstance, fresh);
            }
        } catch (e) {
            // Best-effort during polling or temporary connection drops
        }
    },
    selectInstance(inst) {
        this.selectedInstance = inst;
        const parsed = this.parseJavaArgs(inst.javaArgs);
        this.settingsForm = {
            name: inst.name,
            mcVersion: inst.minecraftVersion,
            loaderVersion: inst.modLoader ? inst.modLoader.version : '',
            javaArgs: inst.javaArgs || '',
            externalPort: inst.externalPort || null,
            ramAuto: parsed.auto,
            ramGB: parsed.gb,
            extraJvmArgs: parsed.extra,
            idleShutdownEnabled: !!inst.idleShutdownEnabled,
            idleShutdownMinutes: inst.idleShutdownMinutes || 5,
            autoStart: !!inst.autoStart
        };
        this.loadSettingsLoaderVersions();
        this.loadMods();
        this.loadServerProperties();
        this.playersLoaded = false; // first load of the new instance shows the spinner
        this.loadPlayers();
        this.loadBackups();
        this.fetchFiles();
        this.loadBranding();
        this.backupForm = {
            frequency: inst.backupFrequency || 'off',
            time: inst.backupTime || '02:00',
            retention: inst.backupRetention || 10
        };
        if (this.activeTab === 'console') {
            this.connectConsole();
        }
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
    async openAddServerModal() {
        this.showAddServerModal = true;
        await this.loadMinecraftVersions();
        await this.onNewServerVersionOrLoaderChange();
    },
    async createNewServer() {
        try {
            await this.api('/api/instances', {
                method: 'POST',
                body: JSON.stringify({
                    name: this.newServerForm.name,
                    mcVersion: this.newServerForm.mcVersion,
                    loaderType: this.newServerForm.loaderType,
                    loaderVersion: this.newServerForm.loaderVersion,
                    javaArgs: this.buildJavaArgs(this.newServerForm),
                    autoStart: !!this.newServerForm.autoStart
                })
            });
            this.showAddServerModal = false;
            this.newServerForm = { name: '', mcVersion: '1.21.4', loaderType: 'fabric', loaderVersion: '', ramAuto: true, ramGB: 4, autoStart: false };
            await this.loadInstances();
        } catch (e) {
            alert('Create failed: ' + e.message);
        }
    },
    openImportModal() {
        this.showImportServerModal = true;
        this.importStep = 1;
        this.importUploading = false;
        this.importUploadProgress = 0;
        this.importUploadLoadedText = '';
        this.importUploadTotalText = '';
        this.importUploadSpeed = '';
        this.importStatusMessage = '';
        this.importLogs = [];
        this.importError = '';
        this.importReport = null;
        this.importForm = {
            name: '',
            mcVersion: '1.21.4',
            loaderType: 'fabric',
            loaderVersion: '',
            ramAuto: true,
            ramGB: 4,
            convertDimensions: true,
            externalPort: null
        };
        this.addImportLog('Ready. Select or drop a server .zip archive to begin.');
    },
    addImportLog(message) {
        const now = new Date();
        const timeStr = now.toTimeString().split(' ')[0];
        this.importLogs.push(`[${timeStr}] ${message}`);
        this.$nextTick(() => {
            const el = document.getElementById('import-log-container');
            if (el) el.scrollTop = el.scrollHeight;
        });
    },
    formatFileSize(bytes) {
        if (!bytes || bytes === 0) return '0 B';
        const k = 1024;
        const sizes = ['B', 'KB', 'MB', 'GB', 'TB'];
        const i = Math.floor(Math.log(bytes) / Math.log(k));
        return parseFloat((bytes / Math.pow(k, i)).toFixed(2)) + ' ' + sizes[i];
    },
    handleImportDrop(event) {
        const files = (event.dataTransfer && event.dataTransfer.files) || (event.target && event.target.files);
        if (files && files.length > 0) {
            this.handleZipFileSelect(files[0]);
        }
        if (event.target && event.target.value !== undefined) {
            event.target.value = '';
        }
    },
    async handleZipFileSelect(file) {
        if (!file) return;
        if (!file.name.toLowerCase().endsWith('.zip')) {
            this.importError = 'Please select a valid Minecraft server .zip archive';
            this.addImportLog('ERROR: Selected file is not a .zip archive');
            return;
        }
        this.importError = '';
        this.importUploading = true;
        this.importUploadProgress = 0;
        this.importUploadLoadedText = '0 MB';
        this.importUploadTotalText = this.formatFileSize(file.size);
        this.importUploadSpeed = '0 MB/s';
        this.importStatusMessage = 'Uploading archive to server staging...';

        this.addImportLog(`Selected archive: "${file.name}" (${this.formatFileSize(file.size)})`);
        this.addImportLog(`Initiating streaming upload to /api/instances/import/analyze...`);

        const formData = new FormData();
        formData.append('file', file);

        try {
            const xhr = new XMLHttpRequest();
            const startTime = Date.now();
            let lastLoaded = 0;
            let lastTime = startTime;

            const promise = new Promise((resolve, reject) => {
                xhr.upload.addEventListener('progress', (e) => {
                    if (e.lengthComputable) {
                        const now = Date.now();
                        const percent = Math.round((e.loaded / e.total) * 100);
                        this.importUploadProgress = percent;
                        this.importUploadLoadedText = this.formatFileSize(e.loaded);
                        this.importUploadTotalText = this.formatFileSize(e.total);

                        // Calculate upload speed
                        const timeDiff = (now - lastTime) / 1000;
                        if (timeDiff >= 0.5) {
                            const bytesDiff = e.loaded - lastLoaded;
                            const speed = bytesDiff / timeDiff;
                            this.importUploadSpeed = this.formatFileSize(speed) + '/s';
                            lastLoaded = e.loaded;
                            lastTime = now;
                        }

                        if (percent >= 100) {
                            this.importStatusMessage = 'Upload complete! Decompressing & inspecting world NBT on server...';
                            this.addImportLog('100% Uploaded. Server is unpacking archive and analyzing level.dat & mods...');
                        }
                    }
                });
                xhr.addEventListener('load', () => {
                    if (xhr.status >= 200 && xhr.status < 300) {
                        try {
                            resolve(JSON.parse(xhr.responseText));
                        } catch (err) {
                            reject(new Error('Invalid response from server'));
                        }
                    } else {
                        try {
                            const errObj = JSON.parse(xhr.responseText);
                            reject(new Error(errObj.error || `Upload failed with status ${xhr.status}`));
                        } catch {
                            reject(new Error(xhr.responseText || `Upload failed with status ${xhr.status}`));
                        }
                    }
                });
                xhr.addEventListener('error', () => reject(new Error('Network error during upload')));
                xhr.addEventListener('abort', () => reject(new Error('Upload aborted')));
            });

            const token = localStorage.getItem('zircon_jwt') || this.jwtToken;
            xhr.open('POST', '/api/instances/import/analyze');
            if (token) {
                xhr.setRequestHeader('Authorization', 'Bearer ' + token);
            }
            xhr.send(formData);

            const report = await promise;
            this.importReport = report;
            this.importForm.name = report.suggestedName || 'Imported Server';
            if (report.minecraftVersion) {
                this.importForm.mcVersion = report.minecraftVersion;
            }
            if (report.detectedLoader && report.detectedLoader !== 'vanilla') {
                this.importForm.loaderType = report.detectedLoader;
            } else if (report.detectedLoader === 'vanilla') {
                this.importForm.loaderType = 'fabric';
            }
            if (report.detectedLoaderVersion) {
                this.importForm.loaderVersion = report.detectedLoaderVersion;
            }
            this.importForm.convertDimensions = !!report.bukkitDimensionsDetected;

            const worldInfo = report.world
                ? `World "${report.world.levelDat?.levelName || report.world.folderName}" (${report.world.totalChunks} chunks, ${report.world.playerCount} players)`
                : 'No existing world found';
            const modCount = report.mods ? report.mods.length : 0;
            this.addImportLog(`Inspection complete: ${worldInfo}, ${modCount} mods indexed.`);
            if (report.bukkitDimensionsDetected) {
                this.addImportLog('Bukkit/Paper multi-folder dimension layout detected (world_nether, world_the_end).');
            }
            if (report.downgradeWarning) {
                this.addImportLog(`WARNING: ${report.downgradeWarning}`);
            } else {
                this.addImportLog(`Target version compatibility verified (${report.minecraftVersion || '1.21.4'}).`);
            }

            this.importStep = 2;
        } catch (e) {
            this.importError = e.message || 'Import analysis failed';
            this.addImportLog(`ERROR: ${this.importError}`);
        } finally {
            this.importUploading = false;
        }
    },
    async commitServerImport() {
        if (!this.importReport) return;
        this.importUploading = true;
        this.importError = '';
        this.addImportLog(`Assembling server instance "${this.importForm.name}" (${this.importForm.loaderType} ${this.importForm.mcVersion})...`);
        try {
            const created = await this.api('/api/instances/import/commit', {
                method: 'POST',
                body: JSON.stringify({
                    importId: this.importReport.importId,
                    name: this.importForm.name,
                    mcVersion: this.importForm.mcVersion,
                    loaderType: this.importForm.loaderType,
                    loaderVersion: this.importForm.loaderVersion,
                    javaArgs: this.buildJavaArgs(this.importForm),
                    externalPort: this.importForm.externalPort ? parseInt(this.importForm.externalPort, 10) : null,
                    convertDimensions: this.importForm.convertDimensions
                })
            });
            this.addImportLog(`Instance created successfully (ID: ${created.id}).`);
            this.showImportServerModal = false;
            this.importReport = null;
            await this.loadInstances();
            const found = this.instances.find(i => i.id === created.id);
            if (found) this.selectInstance(found);
        } catch (e) {
            this.importError = 'Import commit failed: ' + e.message;
            this.addImportLog(`ERROR: ${this.importError}`);
        } finally {
            this.importUploading = false;
        }
    },
    async cancelServerImport() {
        if (this.importReport && this.importReport.importId) {
            try {
                await this.api(`/api/instances/import/${this.importReport.importId}`, { method: 'DELETE' });
            } catch { /* ignore */ }
        }
        this.showImportServerModal = false;
        this.importReport = null;
        this.importStep = 1;
        this.importError = '';
        this.importLogs = [];
    },
    async startInstance(inst) {
        inst = inst || this.selectedInstance;
        if (!inst) return;
        if (this.actionLoading[inst.id]) return;
        this.actionLoading[inst.id] = true;
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
        } finally {
            delete this.actionLoading[inst.id];
        }
    },
    async stopInstance(inst) {
        inst = inst || this.selectedInstance;
        if (!inst) return;
        this.actionLoading[inst.id] = 'manual';
        try {
            await this.api(`/api/instances/${inst.id}/stop`, { method: 'POST' });
        } catch (e) {
            alert('Stop failed: ' + e.message);
        } finally {
            delete this.actionLoading[inst.id];
            await this.loadInstances();
        }
    },
    async restartInstance(inst) {
        inst = inst || this.selectedInstance;
        if (!inst) return;
        this.modsRestartNeeded = false;
        this.actionLoading[inst.id] = 'manual';
        try {
            await this.api(`/api/instances/${inst.id}/restart`, { method: 'POST' });
            setTimeout(() => this.loadInstances(), 2000);
        } catch (e) {
            alert('Restart failed: ' + e.message);
        } finally {
            delete this.actionLoading[inst.id];
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
                    javaArgs: this.buildJavaArgs(this.settingsForm),
                    // 0 / blank leaves the player-facing port unchanged.
                    externalPort: Number(this.settingsForm.externalPort) || 0,
                    idleShutdownEnabled: !!this.settingsForm.idleShutdownEnabled,
                    idleShutdownMinutes: Number(this.settingsForm.idleShutdownMinutes) || 5,
                    autoStart: !!this.settingsForm.autoStart
                })
            });
            alert(`Instance updated! ${res.updatedCount || 0} mods auto-updated, ${res.incompatibleCount || 0} flagged incompatible.`);
            await this.loadInstances();
            await this.loadMods();
        } catch (e) { alert('Update failed: ' + e.message); }
    },

    async loadMinecraftVersions() {
        if (this.minecraftVersions && this.minecraftVersions.length > 0) return;
        this.minecraftVersionsLoading = true;
        try {
            const data = await this.api('/api/versions/minecraft');
            this.minecraftVersions = data.versions || [];
        } catch (e) {
            this.minecraftVersions = [
                { id: '1.21.4' }, { id: '1.21.3' }, { id: '1.21.1' }, { id: '1.21' },
                { id: '1.20.6' }, { id: '1.20.4' }, { id: '1.20.2' }, { id: '1.20.1' },
                { id: '1.19.4' }, { id: '1.19.2' }, { id: '1.18.2' }, { id: '1.16.5' }, { id: '1.12.2' }
            ];
        } finally {
            this.minecraftVersionsLoading = false;
        }
    },

    async onNewServerVersionOrLoaderChange() {
        const mc = this.newServerForm.mcVersion;
        const loader = this.newServerForm.loaderType;
        if (!mc || !loader) return;
        if (loader === 'vanilla') {
            this.newServerLoaderVersions = [];
            this.newServerForm.loaderVersion = '';
            return;
        }
        this.newServerLoaderLoading = true;
        try {
            const data = await this.api(`/api/versions/loaders?loader=${encodeURIComponent(loader)}&mcVersion=${encodeURIComponent(mc)}`);
            this.newServerLoaderVersions = data.versions || [];
            if (data.recommended) {
                this.newServerForm.loaderVersion = data.recommended;
            } else if (this.newServerLoaderVersions.length > 0) {
                this.newServerForm.loaderVersion = this.newServerLoaderVersions[0];
            } else {
                this.newServerForm.loaderVersion = '';
            }
        } catch (e) {
            this.newServerLoaderVersions = [];
        } finally {
            this.newServerLoaderLoading = false;
        }
    },

    async loadSettingsLoaderVersions() {
        if (!this.selectedInstance) return;
        const mc = this.settingsForm.mcVersion;
        const loader = this.selectedInstance.modLoader?.type || 'fabric';
        if (!mc || loader === 'vanilla') {
            this.settingsLoaderVersions = [];
            return;
        }
        this.settingsLoaderLoading = true;
        try {
            const data = await this.api(`/api/versions/loaders?loader=${encodeURIComponent(loader)}&mcVersion=${encodeURIComponent(mc)}`);
            this.settingsLoaderVersions = data.versions || [];
            if (!this.settingsForm.loaderVersion && data.recommended) {
                this.settingsForm.loaderVersion = data.recommended;
            }
        } catch (e) {
            this.settingsLoaderVersions = [];
        } finally {
            this.settingsLoaderLoading = false;
        }
    },

    async onSettingsMinecraftVersionChange() {
        if (!this.selectedInstance) return;
        const mc = this.settingsForm.mcVersion;
        const loader = this.selectedInstance.modLoader?.type || 'fabric';
        if (!mc || loader === 'vanilla') {
            this.settingsLoaderVersions = [];
            this.settingsForm.loaderVersion = '';
            return;
        }
        this.settingsLoaderLoading = true;
        try {
            const data = await this.api(`/api/versions/loaders?loader=${encodeURIComponent(loader)}&mcVersion=${encodeURIComponent(mc)}`);
            this.settingsLoaderVersions = data.versions || [];
            if (data.recommended) {
                this.settingsForm.loaderVersion = data.recommended;
            } else if (this.settingsLoaderVersions.length > 0) {
                this.settingsForm.loaderVersion = this.settingsLoaderVersions[0];
            }
        } catch (e) {
            this.settingsLoaderVersions = [];
        } finally {
            this.settingsLoaderLoading = false;
        }
    },

    async loadAutostartStatus() {
        try {
            const data = await this.api('/api/system/autostart');
            this.windowsAutostartEnabled = !!data.enabled;
            this.windowsAutostartSupported = data.supported !== false;
        } catch (e) {
            // ignore
        }
    },

    async toggleWindowsAutostart() {
        if (this.windowsAutostartLoading) return;
        this.windowsAutostartLoading = true;
        try {
            const next = !this.windowsAutostartEnabled;
            const data = await this.api('/api/system/autostart', {
                method: 'POST',
                body: JSON.stringify({ enabled: next })
            });
            this.windowsAutostartEnabled = !!data.enabled;
        } catch (e) {
            alert('Failed to update Windows startup settings: ' + e.message);
        } finally {
            this.windowsAutostartLoading = false;
        }
    },

    // Splits a JVM args string into {auto, gb, extra} so the RAM slider can
    // represent it. -Xmx/-Xms map to the slider value; MaxRAMPercentage flags
    // mean "auto"; every other flag is preserved for the advanced field.
    parseJavaArgs(args) {
        let auto = false;
        let gb = 4;
        const extra = [];
        for (const token of (args || '').split(/\s+/)) {
            if (!token) continue;
            const lower = token.toLowerCase();
            if (lower.startsWith('-xmx')) {
                const value = parseFloat(token.slice(4));
                // Guard against MB-unit values (e.g. -Xmx2048M) — cap at the
                // slider ceiling so the round-trip stays sane.
                if (Number.isFinite(value) && value > 0) gb = Math.max(1, Math.min(64, Math.round(value)));
            } else if (lower.startsWith('-xms')) {
                // Derived from the slider; ignored when parsing.
            } else if (lower.includes('maxrampercentage') || lower.includes('initialrampercentage')) {
                auto = true;
            } else {
                extra.push(token);
            }
        }
        return { auto, gb, extra: extra.join(' ') };
    },

    // Renders slider state back into a JVM args string: a fixed heap when a
    // GB value is picked, or percentage flags so the JVM sizes itself when
    // "auto" is chosen. Extra flags are always preserved.
    buildJavaArgs({ auto, gb, extra }) {
        // Coerce/clamp the slider value so a missing or non-numeric value can
        // never emit something like -XmsNaNG (which the JVM rejects at launch).
        const gbNum = Number(gb);
        const gbSafe = Number.isFinite(gbNum) ? Math.max(1, Math.min(64, Math.round(gbNum))) : 4;
        const heap = auto
            ? '-XX:InitialRAMPercentage=25.0 -XX:MaxRAMPercentage=75.0'
            : `-Xms${Math.min(gbSafe, 2)}G -Xmx${gbSafe}G`;
        const extraTrimmed = (extra || '').trim();
        return extraTrimmed ? `${heap} ${extraTrimmed}` : heap;
    },
};
