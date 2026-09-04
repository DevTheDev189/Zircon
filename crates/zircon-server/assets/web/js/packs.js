// Zircon admin SPA - Shaderpack and texture pack management.
window.Zircon = window.Zircon || {};
window.Zircon.packs = {
    // ---- Shaders & texture packs ----
    packEndpoint(type) {
        return type === 'shaderpack' ? 'shaderpacks' : 'resourcepacks';
    },
    setPackSearchProvider(type, provider) {
        if (type === 'shaderpack') {
            this.shaderSearchProvider = provider;
            if (this.shaderSearchQuery.trim()) {
                this.searchPacks('shaderpack');
            }
        } else {
            this.texturePackSearchProvider = provider;
            if (this.texturePackSearchQuery.trim()) {
                this.searchPacks('resourcepack');
            }
        }
    },
    async loadShaders() {
        if (!this.selectedInstance) return;
        try {
            const sp = await this.api(`/api/instances/${this.selectedInstance.id}/shaderpacks`);
            this.shaderpacks = sp.shaderpacks || [];
        } catch (e) {
            this.shaderpacks = [];
        }
        try {
            const rp = await this.api(`/api/instances/${this.selectedInstance.id}/resourcepacks`);
            this.resourcepacks = rp.resourcepacks || [];
        } catch (e) {
            this.resourcepacks = [];
        }
        try {
            const srp = await this.api(`/api/instances/${this.selectedInstance.id}/resourcepacks/server-pack`);
            this.serverResourcePack = srp.serverResourcePack || null;
        } catch (e) {
            this.serverResourcePack = null;
        }
    },
    async searchPacks(type) {
        const isShader = type === 'shaderpack';
        const provider = isShader ? (this.shaderSearchProvider || 'modrinth') : (this.texturePackSearchProvider || 'modrinth');
        const query = isShader ? this.shaderSearchQuery : this.texturePackSearchQuery;
        const searchAll = isShader ? this.shaderSearchAllVersions : this.texturePackSearchAllVersions;
        
        let reqType = '';
        if (provider === 'curseforge') {
            reqType = isShader ? 'shaderpack' : 'resourcepack';
        } else {
            reqType = isShader ? 'shader' : 'resourcepack';
        }

        const q = new URLSearchParams({
            query: query || '',
            mcVersion: searchAll ? '' : (this.selectedInstance?.minecraftVersion || ''),
            origin: provider,
            type: reqType
        });

        if (isShader) {
            this.shaderSearching = true;
        } else {
            this.texturePackSearching = true;
        }

        try {
            const data = await this.api(`/api/instances/${this.selectedInstance.id}/mods/search?${q.toString()}`);
            const hits = data.hits || [];
            if (isShader) {
                this.shaderSearchResults = hits;
            } else {
                this.texturePackSearchResults = hits;
            }
            this.attachPackVersionOptions(hits, type, provider);
        } catch (e) {
            alert('Search failed: ' + e.message);
        } finally {
            if (isShader) {
                this.shaderSearching = false;
            } else {
                this.texturePackSearching = false;
            }
        }
    },
    async attachPackVersionOptions(hits, type, provider) {
        const isCurseForge = provider === 'curseforge';
        const isShader = type === 'shaderpack';
        const searchAll = isShader ? this.shaderSearchAllVersions : this.texturePackSearchAllVersions;
        await Promise.all(hits.map(async (hit) => {
            hit.versionOptions = [];
            hit.selectedVersionId = '';
            try {
                if (isCurseForge || hit.origin === 'curseforge') {
                    const q = new URLSearchParams({ modId: hit.projectId || hit.id });
                    const data = await this.api(`/api/instances/${this.selectedInstance.id}/mods/curseforge/files?${q}`);
                    hit.versionOptions = data.files || [];
                    hit.selectedVersionId = hit.versionOptions[0] ? (hit.versionOptions[0].id || hit.versionOptions[0].fileId) : '';
                } else {
                    const q = new URLSearchParams({
                        projectId: hit.projectId || hit.id,
                        mcVersion: searchAll ? '' : (this.selectedInstance?.minecraftVersion || '')
                    });
                    const data = await this.api(`/api/instances/${this.selectedInstance.id}/mods/modrinth/versions?${q}`);
                    hit.versionOptions = data.versions || [];
                    hit.selectedVersionId = hit.versionOptions[0] ? (hit.versionOptions[0].id || hit.versionOptions[0].fileId) : '';
                }
            } catch (e) {
                hit.versionsFailed = true;
            }
        }));
    },
    async installPack(hit, type) {
        const id = hit.projectId || hit.id;
        const provider = type === 'shaderpack' ? (this.shaderSearchProvider || 'modrinth') : (this.texturePackSearchProvider || 'modrinth');
        const isCurseForge = provider === 'curseforge' || hit.origin === 'curseforge';

        if (isCurseForge) {
            const fileOpt = (hit.versionOptions || []).find(v => (v.id || v.fileId) === hit.selectedVersionId);
            const rawBase = (hit.websiteUrl || hit.projectUrl || '').replace(/\/$/, '');
            let targetUrl = rawBase;

            if (rawBase && hit.selectedVersionId) {
                const fileId = fileOpt ? (fileOpt.fileId || fileOpt.id) : hit.selectedVersionId;
                targetUrl = `${rawBase}/files/${fileId}`;
            } else if (!targetUrl) {
                const categoryPath = type === 'shaderpack' ? 'shaders' : 'texture-packs';
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
                modTitle: hit.title || hit.name || (type === 'shaderpack' ? 'CurseForge Shaderpack' : 'CurseForge Texture Pack'),
                modSlug: hit.slug || '',
                modFileId: hit.selectedVersionId || null,
                targetFileName: fileOpt ? (fileOpt.fileName || fileOpt.filename || '') : '',
                projectUrl: targetUrl,
                iconUrl: hit.iconUrl || '',
                summary: hit.description || hit.summary || '',
                author: hit.author || '',
                packType: type,
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

        this.installingPacks[id] = true;
        try {
            await this.api(`/api/instances/${this.selectedInstance.id}/${this.packEndpoint(type)}/install`, {
                method: 'POST',
                body: JSON.stringify({
                    origin: 'modrinth',
                    projectId: id,
                    versionId: hit.selectedVersionId || undefined
                })
            });
            if (type === 'shaderpack') {
                this.shaderSearchResults = this.shaderSearchResults.filter(r => (r.projectId || r.id) !== id);
            } else {
                this.texturePackSearchResults = this.texturePackSearchResults.filter(r => (r.projectId || r.id) !== id);
            }
            await this.loadShaders();
        } catch (e) {
            alert('Install failed: ' + e.message);
        } finally {
            delete this.installingPacks[id];
        }
    },
    async deletePack(filename, type) {
        await this.api(`/api/instances/${this.selectedInstance.id}/${this.packEndpoint(type)}/${encodeURIComponent(filename)}`, { method: 'DELETE' });
        this.loadShaders();
    },
    async handlePackDrop(event, type) {
        const files = Array.from(event.dataTransfer.files || []).filter(f => f.name.toLowerCase().endsWith('.zip'));
        for (const file of files) {
            const form = new FormData();
            form.append('file', file);
            try {
                const res = await fetch(`/api/instances/${this.selectedInstance.id}/${this.packEndpoint(type)}/upload`, {
                    method: 'POST',
                    headers: { 'Authorization': 'Bearer ' + this.jwtToken },
                    body: form
                });
                if (!res.ok) {
                    const err = await res.json().catch(() => ({}));
                    throw new Error(err.error || ('Upload failed with HTTP ' + res.status));
                }
            } catch (e) {
                alert('Upload failed for ' + file.name + ': ' + e.message);
            }
        }
        this.loadShaders();
    },
    async toggleServerResourcePack(pack) {
        if (!this.selectedInstance) return;
        const isCurrent = this.serverResourcePack && this.serverResourcePack.filename === pack.filename;
        const targetFilename = isCurrent ? null : pack.filename;
        this.serverPackLoading = true;
        try {
            await this.api(`/api/instances/${this.selectedInstance.id}/resourcepacks/server-pack`, {
                method: 'POST',
                body: JSON.stringify({
                    filename: targetFilename
                })
            });
            await this.loadShaders();
        } catch (e) {
            alert('Failed to update server resource pack: ' + e.message);
        } finally {
            this.serverPackLoading = false;
        }
    },
    async handleDirectServerPackUpload(event) {
        const files = Array.from(event.target?.files || event.dataTransfer?.files || []).filter(f => f.name.toLowerCase().endsWith('.zip'));
        if (!files.length) return;
        this.serverPackUploading = true;
        try {
            for (const file of files) {
                const form = new FormData();
                form.append('file', file);
                try {
                    const res = await fetch(`/api/instances/${this.selectedInstance.id}/resourcepacks/upload`, {
                        method: 'POST',
                        headers: { 'Authorization': 'Bearer ' + this.jwtToken },
                        body: form
                    });
                    if (!res.ok) {
                        const err = await res.json().catch(() => ({}));
                        throw new Error(err.error || ('Upload failed with HTTP ' + res.status));
                    }
                    const data = await res.json();
                    const packFilename = data.filename || data.pack?.filename;
                    if (packFilename) {
                        await this.api(`/api/instances/${this.selectedInstance.id}/resourcepacks/server-pack`, {
                            method: 'POST',
                            body: JSON.stringify({
                                filename: packFilename
                            })
                        });
                    }
                } catch (e) {
                    alert('Upload rejected for ' + file.name + ': ' + e.message);
                }
            }
            await this.loadShaders();
        } finally {
            this.serverPackUploading = false;
            if (event.target) event.target.value = '';
        }
    },
};
