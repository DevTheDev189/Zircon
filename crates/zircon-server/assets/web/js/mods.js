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
    // --- Bulk selection & enable/disable ---

    toggleModSelected(filename) {
        const next = { ...this.selectedMods };
        if (next[filename]) {
            delete next[filename];
        } else {
            next[filename] = true;
        }
        this.selectedMods = next;
    },
    toggleSelectAllMods() {
        if (this.allModsSelected) {
            this.selectedMods = {};
            return;
        }
        const next = {};
        for (const m of this.installedMods) next[m.filename] = true;
        this.selectedMods = next;
    },
    async toggleModEnabled(mod) {
        const route = mod.enabled ? 'disable' : 'enable';
        await this.api(`/api/instances/${this.selectedInstance.id}/mods/${route}`, {
            method: 'POST',
            body: JSON.stringify({ filenames: [mod.filename] })
        });
        this.modsRestartNeeded = true;
        await this.loadMods();
    },
    async setSelectedModsEnabled(enabled) {
        const filenames = Object.keys(this.selectedMods);
        if (!filenames.length) return;
        const route = enabled ? 'enable' : 'disable';
        await this.api(`/api/instances/${this.selectedInstance.id}/mods/${route}`, {
            method: 'POST',
            body: JSON.stringify({ filenames })
        });
        this.modsRestartNeeded = true;
        await this.loadMods();
    },
    bulkEnableMods() {
        return this.setSelectedModsEnabled(true);
    },
    bulkDisableMods() {
        return this.setSelectedModsEnabled(false);
    },
    async bulkDeleteMods() {
        const filenames = Object.keys(this.selectedMods);
        if (!filenames.length) return;
        const label = filenames.length === 1 ? 'mod' : 'mods';
        if (!confirm(`Delete ${filenames.length} selected ${label}? This cannot be undone.`)) return;
        await this.api(`/api/instances/${this.selectedInstance.id}/mods/bulk-delete`, {
            method: 'POST',
            body: JSON.stringify({ filenames })
        });
        await this.loadMods();
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
