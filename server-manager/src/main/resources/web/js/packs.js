// Zircon admin SPA - Shaderpack and texture pack management.
window.Zircon = window.Zircon || {};
window.Zircon.packs = {
        // ---- Shaders & texture packs ----
    packEndpoint(type) {
        return type === 'shaderpack' ? 'shaderpacks' : 'resourcepacks';
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
        const modrinthType = type === 'shaderpack' ? 'shader' : 'resourcepack';
        // Modrinth doesn't tag shaderpacks/resourcepacks by mod loader (they aren't
        // loader-specific like mods are) — a loader facet here returns zero hits.
        const q = new URLSearchParams({
            query: type === 'shaderpack' ? this.shaderSearchQuery : this.texturePackSearchQuery,
            mcVersion: this.selectedInstance.minecraftVersion,
            origin: 'modrinth',
            type: modrinthType
        });
        this.packSearching = true;
        try {
            const data = await this.api(`/api/instances/${this.selectedInstance.id}/mods/search?${q}`);
            this.packSearchResults = data.hits || [];
        } catch (e) {
            alert('Search failed: ' + e.message);
        } finally {
            this.packSearching = false;
        }
    },
    async installPack(hit, type) {
        const id = hit.projectId;
        this.installingPacks[id] = true;
        try {
            // No loader filter here either — pack versions aren't tagged per mod loader.
            const q = new URLSearchParams({ projectId: id, mcVersion: this.selectedInstance.minecraftVersion });
            const versions = await this.api(`/api/instances/${this.selectedInstance.id}/mods/modrinth/versions?${q}`);
            const chosen = (versions.versions || [])[0];
            const file = chosen && chosen.file;
            if (!file) { alert('No installable version found for ' + hit.title); return; }
            await this.api(`/api/instances/${this.selectedInstance.id}/${this.packEndpoint(type)}/install`, {
                method: 'POST',
                body: JSON.stringify({ downloadUrl: file.url, filename: file.filename, origin: 'modrinth' })
            });
            this.packSearchResults = this.packSearchResults.filter(r => r.projectId !== id);
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
