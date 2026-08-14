// Zircon admin SPA - Mod search, install, and management.
window.Zircon = window.Zircon || {};
window.Zircon.mods = {
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
            mcVersion: this.selectedInstance.minecraftVersion,
            loader,
            type: this.searchType
        });
        const mySeq = ++this.searchSeq;
        this.searching = true;
        try {
            const data = await this.api(`/api/instances/${this.selectedInstance.id}/mods/search?${q}`);
            if (mySeq !== this.searchSeq) return; // superseded by a newer search
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
    /** Fetches the compatible Modrinth versions for every search hit and pre-selects the best one. */
    async attachVersionOptions(hits) {
        const loader = this.selectedInstance.modLoader.type === 'vanilla' ? '' : this.selectedInstance.modLoader.type;
        const base = `/api/instances/${this.selectedInstance.id}/mods/modrinth/versions?`;
        await Promise.all(hits.map(async (hit) => {
            hit.versionOptions = [];
            hit.selectedVersionId = '';
            try {
                const q = new URLSearchParams({ projectId: hit.projectId, mcVersion: this.selectedInstance.minecraftVersion, loader });
                const data = await this.api(base + q);
                hit.versionOptions = data.versions || [];
                hit.selectedVersionId = hit.versionOptions[0] ? hit.versionOptions[0].id : '';
            } catch (e) {
                hit.versionsFailed = true;
            }
        }));
    },
    async installMod(hit) {
        const id = hit.projectId;
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
            this.searchResults = this.searchResults.filter(r => r.projectId !== id);
            await this.loadMods();
        } catch (e) {
            alert('Install failed: ' + e.message);
        } finally {
            delete this.installingMods[id];
        }
    },
    async installModpack(hit) {
        const id = hit.projectId;
        this.installingMods[id] = true;
        try {
            const res = await this.api(`/api/instances/${this.selectedInstance.id}/modpacks/install`, {
                method: 'POST',
                body: JSON.stringify({ projectId: id })
            });
            alert(res.message || 'Modpack installed successfully!');
            this.searchResults = this.searchResults.filter(r => r.projectId !== id);
            await this.loadMods();
        } catch (e) {
            alert('Modpack install failed: ' + e.message);
        } finally {
            delete this.installingMods[id];
        }
    },
    isModInstalled(rec) {
        // Mods installed via the generic search flow are keyed by Modrinth's
        // internal project id, not the slug used in the curated list, so fall
        // back to a case-insensitive title match.
        return this.installedMods.some(m => m.id === rec.projectId
            || (m.title && m.title.toLowerCase() === rec.title.toLowerCase()));
    },
    async loadMods() {
        if (!this.selectedInstance) return;
        try {
            const data = await this.api(`/api/instances/${this.selectedInstance.id}/mods`);
            this.installedMods = data.mods || [];
        } catch (e) {
            this.installedMods = [];
        }
    },
    async deleteMod(filename) {
        await this.api(`/api/instances/${this.selectedInstance.id}/mods/${encodeURIComponent(filename)}`, { method: 'DELETE' });
        this.loadMods();
    },
};
