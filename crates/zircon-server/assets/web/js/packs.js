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
    },
    async searchPacks(type) {
        this.packSearchType = type;
        const provider = type === 'shaderpack' ? (this.shaderSearchProvider || 'modrinth') : (this.texturePackSearchProvider || 'modrinth');
        const query = type === 'shaderpack' ? this.shaderSearchQuery : this.texturePackSearchQuery;
        
        let reqType = '';
        if (provider === 'curseforge') {
            reqType = type === 'shaderpack' ? 'shaderpack' : 'resourcepack';
        } else {
            reqType = type === 'shaderpack' ? 'shader' : 'resourcepack';
        }

        const q = new URLSearchParams({
            query: query || '',
            mcVersion: this.selectedInstance.minecraftVersion || '',
            origin: provider,
            type: reqType
        });

        this.packSearching = true;
        try {
            const data = await this.api(`/api/instances/${this.selectedInstance.id}/mods/search?${q.toString()}`);
            this.packSearchResults = data.hits || [];
        } catch (e) {
            alert('Search failed: ' + e.message);
        } finally {
            this.packSearching = false;
        }
    },
    async installPack(hit, type) {
        const id = hit.projectId || hit.id;
        const provider = type === 'shaderpack' ? (this.shaderSearchProvider || 'modrinth') : (this.texturePackSearchProvider || 'modrinth');
        const isCurseForge = provider === 'curseforge' || hit.origin === 'curseforge';

        if (isCurseForge) {
            const rawBase = (hit.websiteUrl || hit.projectUrl || '').replace(/\/$/, '');
            let targetUrl = rawBase;
            if (!targetUrl) {
                const categoryPath = type === 'shaderpack' ? 'shaders' : 'texture-packs';
                if (hit.slug) {
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
                targetFileName: '',
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
                body: JSON.stringify({ origin: 'modrinth', projectId: id })
            });
            this.packSearchResults = this.packSearchResults.filter(r => (r.projectId || r.id) !== id);
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
                await fetch(`/api/instances/${this.selectedInstance.id}/${this.packEndpoint(type)}/upload`, {
                    method: 'POST',
                    headers: { 'Authorization': 'Bearer ' + this.jwtToken },
                    body: form
                });
            } catch (e) {
                alert('Upload failed for ' + file.name + ': ' + e.message);
            }
        }
        this.loadShaders();
    },
};
