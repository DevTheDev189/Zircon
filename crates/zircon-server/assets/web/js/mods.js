// Zircon admin SPA - Mod search, install, and management.
window.Zircon = window.Zircon || {};
window.Zircon.mods = {
    setSearchProvider(p) {
        this.searchProvider = p;
        if (!this.searchQuery.trim()) {
            this.searchResults = [];
        } else {
            this.searchMods();
        }
    },
    setSearchType(t) {
        this.searchType = t;
        // Toggling must re-run the search with the new project type; when the
        // query is empty, drop stale results instead of showing mods under a
        // modpack toggle (and vice versa).
        if (!this.searchQuery.trim()) {
            this.searchResults = [];
        } else {
            this.searchMods();
        }
    },
    async searchMods() {
        if (!this.selectedInstance || !this.searchQuery.trim()) return;
        const loader = this.selectedInstance.modLoader.type === 'vanilla' ? '' : this.selectedInstance.modLoader.type;
        const q = new URLSearchParams({
            query: this.searchQuery,
            mcVersion: this.searchAllVersions ? '' : (this.selectedInstance.minecraftVersion || ''),
            loader,
            type: this.searchType,
            origin: this.searchProvider
        });
        const mySeq = ++this.searchSeq;
        this.searching = true;
        try {
            const data = await this.api(`/api/instances/${this.selectedInstance.id}/mods/search?${q}`);
            if (mySeq !== this.searchSeq) return; // superseded by a newer search
            if (data.notice && (!data.hits || !data.hits.length)) {
                alert(data.notice);
            }
            this.searchResults = data.hits || [];
            // Auto-detect the best version for each mod hit, but keep the
            // full option list so the admin can override the pick manually.
            if (this.searchType === 'mod') this.attachVersionOptions(data.hits || []);
        } catch (e) {
            if (mySeq === this.searchSeq) alert('Search failed: ' + e.message);
        } finally {
            if (mySeq === this.searchSeq) this.searching = false;
        }
    },
    /** Fetches compatible versions for every search hit and pre-selects the best one. */
    async attachVersionOptions(hits) {
        const loader = this.selectedInstance.modLoader.type === 'vanilla' ? '' : this.selectedInstance.modLoader.type;
        const isCurseForge = this.searchProvider === 'curseforge';
        await Promise.all(hits.map(async (hit) => {
            hit.versionOptions = [];
            hit.selectedVersionId = '';
            try {
                if (isCurseForge || hit.origin === 'curseforge') {
                    const q = new URLSearchParams({ modId: hit.projectId || hit.id });
                    const data = await this.api(`/api/instances/${this.selectedInstance.id}/mods/curseforge/files?${q}`);
                    hit.versionOptions = data.files || [];
                    hit.selectedVersionId = hit.versionOptions[0] ? hit.versionOptions[0].id : '';
                } else {
                    const q = new URLSearchParams({
                        projectId: hit.projectId || hit.id,
                        mcVersion: this.searchAllVersions ? '' : (this.selectedInstance.minecraftVersion || ''),
                        loader
                    });
                    const data = await this.api(`/api/instances/${this.selectedInstance.id}/mods/modrinth/versions?${q}`);
                    hit.versionOptions = data.versions || [];
                    hit.selectedVersionId = hit.versionOptions[0] ? hit.versionOptions[0].id : '';
                }
            } catch (e) {
                hit.versionsFailed = true;
            }
        }));
    },
    async installMod(hit) {
        const id = hit.projectId || hit.id;
        const isCurseForge = this.searchProvider === 'curseforge' || hit.origin === 'curseforge';

        if (isCurseForge) {
            // Compliant browser hand-off: open the CurseForge mod/modpack/file page in a new tab
            const fileOpt = (hit.versionOptions || []).find(v => v.id === hit.selectedVersionId || v.fileId === hit.selectedVersionId);
            const rawBase = (hit.websiteUrl || hit.projectUrl || '').replace(/\/$/, '');
            let targetUrl = rawBase;

            if (rawBase && hit.selectedVersionId) {
                const fileId = fileOpt ? (fileOpt.fileId || fileOpt.id) : hit.selectedVersionId;
                targetUrl = `${rawBase}/files/${fileId}`;
            } else if (!targetUrl) {
                const categoryPath = (this.searchType === 'modpack' || hit.projectType === 'modpack') ? 'modpacks' :
                                     (this.searchType === 'shaderpack' ? 'shaders' :
                                     (this.searchType === 'resourcepack' ? 'texture-packs' : 'mc-mods'));
                if (hit.slug && hit.selectedVersionId) {
                    const fileId = fileOpt ? (fileOpt.fileId || fileOpt.id) : hit.selectedVersionId;
                    targetUrl = `https://www.curseforge.com/minecraft/${categoryPath}/${hit.slug}/files/${fileId}`;
                } else if (hit.slug) {
                    targetUrl = `https://www.curseforge.com/minecraft/${categoryPath}/${hit.slug}`;
                } else {
                    targetUrl = `https://www.curseforge.com/projects/${id}`;
                }
            }

            // Start 3-second countdown modal before opening CurseForge
            if (this.curseforgeDropModal.countdownInterval) {
                clearInterval(this.curseforgeDropModal.countdownInterval);
            }
            if (this.curseforgeDropModal.timer) {
                clearTimeout(this.curseforgeDropModal.timer);
            }

            this.curseforgeDropModal = {
                open: true,
                modId: hit.projectId || hit.id || '',
                modTitle: hit.title || hit.name || (this.searchType === 'modpack' ? 'CurseForge Modpack' : 'CurseForge Mod'),
                modSlug: hit.slug || '',
                modFileId: hit.selectedVersionId || null,
                targetFileName: fileOpt ? (fileOpt.fileName || fileOpt.displayName) : '',
                projectUrl: targetUrl,
                iconUrl: hit.iconUrl || '',
                summary: hit.description || hit.summary || '',
                author: hit.author || '',
                countdown: 3,
                redirectTriggered: false,
                uploading: false,
                uploadSuccess: false,
                successTitle: '',
                error: '',
                timer: null,
                countdownInterval: null
            };

            this.curseforgeDropModal.countdownInterval = setInterval(() => {
                if (this.curseforgeDropModal.countdown > 1) {
                    this.curseforgeDropModal.countdown--;
                } else {
                    this.triggerCurseforgeDownload();
                }
            }, 1000);
            return;
        }

        this.installingMods[id] = true;
        try {
            const loader = this.selectedInstance.modLoader.type === 'vanilla' ? '' : this.selectedInstance.modLoader.type;
            // Use the version picked in the dropdown when available;
            // otherwise (e.g. Recommended Mods cards) auto-detect the best one.
            let versionId = hit.selectedVersionId;
            if (!versionId) {
                const q = new URLSearchParams({ projectId: id, mcVersion: this.selectedInstance.minecraftVersion, loader });
                const versions = await this.api(`/api/instances/${this.selectedInstance.id}/mods/modrinth/versions?${q}`);
                const chosen = (versions.versions || [])[0];
                if (!chosen) { alert('No compatible version found for ' + this.selectedInstance.minecraftVersion); return; }
                versionId = chosen.id;
            }
            await this.api(`/api/instances/${this.selectedInstance.id}/mods/install`, {
                method: 'POST',
                body: JSON.stringify({ origin: 'modrinth', projectId: id, versionId })
            });
            this.searchResults = this.searchResults.filter(r => (r.projectId || r.id) !== id);
            await this.loadMods();
        } catch (e) {
            alert('Install failed: ' + e.message);
        } finally {
            delete this.installingMods[id];
        }
    },
    triggerCurseforgeDownload() {
        if (this.curseforgeDropModal.countdownInterval) {
            clearInterval(this.curseforgeDropModal.countdownInterval);
            this.curseforgeDropModal.countdownInterval = null;
        }
        this.curseforgeDropModal.countdown = 0;
        this.curseforgeDropModal.redirectTriggered = true;
        if (this.curseforgeDropModal.projectUrl) {
            window.open(this.curseforgeDropModal.projectUrl, '_blank', 'noopener,noreferrer');
        }
    },
    async installModpack(hit) {
        const isCurseForge = this.searchProvider === 'curseforge' || hit.origin === 'curseforge';
        if (isCurseForge) {
            return this.installMod(hit);
        }
        const id = hit.projectId || hit.id;
        this.installingMods[id] = true;
        try {
            const res = await this.api(`/api/instances/${this.selectedInstance.id}/modpacks/install`, {
                method: 'POST',
                body: JSON.stringify({ projectId: id })
            });
            alert(res.message || 'Modpack installed successfully!');
            this.searchResults = this.searchResults.filter(r => (r.projectId || r.id) !== id);
            await this.loadMods();
        } catch (e) {
            alert('Modpack install failed: ' + e.message);
        } finally {
            delete this.installingMods[id];
        }
    },
    async handleModFileDrop(event) {
        event.preventDefault();
        this.isDraggingMod = false;
        const files = event.dataTransfer ? event.dataTransfer.files : event.target.files;
        if (!files || !files.length) return;
        for (const file of files) {
            await this.uploadModFile(file);
        }
    },
    async uploadModFile(file) {
        if (!this.selectedInstance) return;
        if (!file.name.endsWith('.jar') && !file.name.endsWith('.zip')) {
            this.curseforgeDropModal.error = 'Please upload a .jar or .zip file';
            return;
        }
        this.curseforgeDropModal.uploading = true;
        this.curseforgeDropModal.uploadSuccess = false;
        this.curseforgeDropModal.error = '';
        const formData = new FormData();
        formData.append('file', file);
        try {
            const q = new URLSearchParams({
                origin: 'curseforge',
                expectedModId: String(this.curseforgeDropModal.modId || ''),
                expectedFileId: String(this.curseforgeDropModal.modFileId || ''),
                iconUrl: this.curseforgeDropModal.iconUrl || '',
                title: this.curseforgeDropModal.modTitle || '',
                projectUrl: this.curseforgeDropModal.projectUrl || ''
            });
            const endpoint = this.curseforgeDropModal.packType
                ? this.packEndpoint(this.curseforgeDropModal.packType)
                : 'mods';
            const res = await fetch(`/api/instances/${this.selectedInstance.id}/${endpoint}/upload?${q.toString()}`, {
                method: 'POST',
                headers: {
                    'Authorization': `Bearer ${this.jwtToken}`
                },
                body: formData
            });
            if (!res.ok) {
                const errJson = await res.json().catch(() => ({}));
                throw new Error(errJson.error || errJson.message || `Verification/Upload failed (status ${res.status})`);
            }
            const uploadedEntry = await res.json().catch(() => ({}));
            if (this.curseforgeDropModal.packType) {
                await this.loadShaders();
            } else {
                await this.loadMods();
            }
            this.curseforgeDropModal.uploading = false;
            this.curseforgeDropModal.uploadSuccess = true;
            this.curseforgeDropModal.successTitle = uploadedEntry.title || this.curseforgeDropModal.modTitle;
            
            // Auto close after 3 seconds
            if (this.curseforgeDropModal.timer) clearTimeout(this.curseforgeDropModal.timer);
            this.curseforgeDropModal.timer = setTimeout(() => {
                this.closeCurseforgeDropModal();
            }, 3000);
        } catch (e) {
            this.curseforgeDropModal.uploading = false;
            this.curseforgeDropModal.error = e.message;
        }
    },
    closeCurseforgeDropModal() {
        if (this.curseforgeDropModal.timer) {
            clearTimeout(this.curseforgeDropModal.timer);
            this.curseforgeDropModal.timer = null;
        }
        if (this.curseforgeDropModal.countdownInterval) {
            clearInterval(this.curseforgeDropModal.countdownInterval);
            this.curseforgeDropModal.countdownInterval = null;
        }
        this.curseforgeDropModal.open = false;
        this.curseforgeDropModal.uploading = false;
        this.curseforgeDropModal.uploadSuccess = false;
        this.curseforgeDropModal.error = '';
    },
    triggerServerModUpload() {
        const input = document.getElementById('server-mod-upload-input');
        if (input) input.click();
    },
    async handleServerModSelect(event) {
        const files = event.target.files;
        if (!files || !files.length) return;
        for (const file of files) {
            await this.uploadCustomServerMod(file);
        }
        event.target.value = '';
    },
    async handleServerModDrop(event) {
        event.preventDefault();
        this.isDraggingServerMod = false;
        const files = event.dataTransfer ? event.dataTransfer.files : event.target.files;
        if (!files || !files.length) return;
        for (const file of files) {
            await this.uploadCustomServerMod(file);
        }
    },
    async uploadCustomServerMod(file) {
        if (!this.selectedInstance) return;
        if (!file.name.toLowerCase().endsWith('.jar')) {
            this.serverModUpload.error = 'Only .jar mod files are supported for server-side mods';
            return;
        }
        this.serverModUpload.uploading = true;
        this.serverModUpload.error = '';
        this.serverModUpload.successMessage = '';
        const formData = new FormData();
        formData.append('file', file);
        try {
            const res = await fetch(`/api/instances/${this.selectedInstance.id}/mods/upload-server`, {
                method: 'POST',
                headers: {
                    'Authorization': `Bearer ${this.jwtToken}`
                },
                body: formData
            });
            if (!res.ok) {
                const errJson = await res.json().catch(() => ({}));
                throw new Error(errJson.error || errJson.message || `Upload failed with status ${res.status}`);
            }
            const uploadedEntry = await res.json().catch(() => ({}));
            this.serverModUpload.uploading = false;
            this.serverModUpload.successMessage = `Installed server-side mod: ${uploadedEntry.title || file.name} (${uploadedEntry.version || 'v1.0'})`;
            this.modsRestartNeeded = true;
            await this.loadMods();
        } catch (e) {
            this.serverModUpload.uploading = false;
            this.serverModUpload.error = e.message;
        }
    },
    isModInstalled(rec) {
        // Mods installed via generic search or upload are matched by id or title
        const id = rec.projectId || rec.id;
        return this.installedMods.some(m => (id && m.id === id)
            || (m.title && rec.title && m.title.toLowerCase() === rec.title.toLowerCase()));
    },
    async loadMods() {
        if (!this.selectedInstance) return;
        this.isLoadingMods = true;
        try {
            const data = await this.api(`/api/instances/${this.selectedInstance.id}/mods`);
            this.installedMods = data.mods || [];
        } catch (e) {
            this.installedMods = [];
        } finally {
            this.isLoadingMods = false;
        }
        this.selectedMods = {};
    },
    async deleteMod(filename) {
        await this.api(`/api/instances/${this.selectedInstance.id}/mods/${encodeURIComponent(filename)}`, { method: 'DELETE' });
        this.loadMods();
    },
    // --- Mod Selection & Batch Lifecycle Management ---
    toggleModSelected(filename) {
        if (!filename) return;
        const current = { ...this.selectedMods };
        if (current[filename]) {
            delete current[filename];
        } else {
            current[filename] = true;
        }
        this.selectedMods = current;
    },

    toggleSelectAllMods() {
        if (this.allModsSelected) {
            this.selectedMods = {};
            return;
        }
        const selections = {};
        for (const mod of (this.installedMods || [])) {
            if (mod && mod.filename) {
                selections[mod.filename] = true;
            }
        }
        this.selectedMods = selections;
    },

    async toggleModEnabled(mod) {
        if (!mod || !mod.filename || !this.selectedInstance) return;
        const endpoint = mod.enabled ? 'disable' : 'enable';
        try {
            await this.api(`/api/instances/${this.selectedInstance.id}/mods/${endpoint}`, {
                method: 'POST',
                body: JSON.stringify({ filenames: [mod.filename] })
            });
            this.modsRestartNeeded = true;
            await this.loadMods();
        } catch (err) {
            console.error('Failed to change mod activation state:', err);
            alert('Failed to update mod: ' + (err.message || 'Unknown error'));
        }
    },

    async setSelectedModsEnabled(enabled) {
        if (!this.selectedInstance) return;
        const filenames = Object.keys(this.selectedMods || {});
        if (filenames.length === 0) return;

        const action = enabled ? 'enable' : 'disable';
        try {
            await this.api(`/api/instances/${this.selectedInstance.id}/mods/${action}`, {
                method: 'POST',
                body: JSON.stringify({ filenames })
            });
            this.modsRestartNeeded = true;
            await this.loadMods();
        } catch (err) {
            console.error(`Failed to ${action} selected mods:`, err);
            alert(`Failed to ${action} mods: ` + (err.message || 'Unknown error'));
        }
    },

    bulkEnableMods() {
        return this.setSelectedModsEnabled(true);
    },

    bulkDisableMods() {
        return this.setSelectedModsEnabled(false);
    },

    async bulkDeleteMods() {
        if (!this.selectedInstance) return;
        const filenames = Object.keys(this.selectedMods || {});
        if (filenames.length === 0) return;

        const count = filenames.length;
        const noun = count === 1 ? 'mod' : 'mods';
        if (!window.confirm(`Permanently remove ${count} ${noun}? This action cannot be reverted.`)) {
            return;
        }

        try {
            await this.api(`/api/instances/${this.selectedInstance.id}/mods/bulk-delete`, {
                method: 'POST',
                body: JSON.stringify({ filenames })
            });
            this.selectedMods = {};
            await this.loadMods();
        } catch (err) {
            console.error('Failed to delete selected mods:', err);
            alert('Failed to delete mods: ' + (err.message || 'Unknown error'));
        }
    },

    dismissModsRestartBanner() {
        this.modsRestartNeeded = false;
    },
    async setModSide(filename, side) {
        if (!this.selectedInstance) return;
        const prevMods = [...this.installedMods];
        const mod = this.installedMods.find(m => m.filename === filename);
        if (mod) {
            mod.side = side;
            this.installedMods = [...this.installedMods];
        }
        try {
            await this.api(`/api/instances/${this.selectedInstance.id}/mods/${encodeURIComponent(filename)}/side`, {
                method: 'PATCH',
                body: JSON.stringify({ side })
            });
        } catch (e) {
            this.installedMods = prevMods;
            alert('Failed to update mod environment: ' + e.message);
            this.loadMods();
        }
    },
};
