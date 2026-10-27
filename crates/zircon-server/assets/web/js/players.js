// Zircon admin SPA - Player management (whitelist, ops, bans).
window.Zircon = window.Zircon || {};
window.Zircon.players = {
        // ---- Player management ----
    async loadPlayers() {
        if (!this.selectedInstance) return;
        this.playersLoading = true;
        try {
            const [wl, ops, bans, history, online] = await Promise.all([
                this.api(`/api/instances/${this.selectedInstance.id}/players/whitelist`),
                this.api(`/api/instances/${this.selectedInstance.id}/players/ops`),
                this.api(`/api/instances/${this.selectedInstance.id}/players/bans`),
                this.api(`/api/instances/${this.selectedInstance.id}/players/history`),
                this.api(`/api/instances/${this.selectedInstance.id}/players/online`).catch(() => ({ players: [] }))
            ]);
            this.whitelistPlayers = wl.players || [];
            this.opPlayers = ops.players || [];
            this.bannedPlayers = bans.players || [];
            this.playerHistory = history.players || [];
            this.onlinePlayers = (online && Array.isArray(online.players)) ? online.players : ((this.selectedInstance && Array.isArray(this.selectedInstance.onlinePlayers)) ? this.selectedInstance.onlinePlayers : []);
            this.playersLoaded = true;
        } catch (e) { /* instance may be offline / deleted */ } finally {
            this.playersLoading = false;
        }
    },
    async toggleWhitelist() {
        if (!this.selectedInstance) return;
        try {
            const props = { ...this.serverProps };
            props['white-list'] = this.whitelistEnabled ? 'false' : 'true';
            await this.api(`/api/instances/${this.selectedInstance.id}/server-properties`, {
                method: 'POST',
                body: JSON.stringify({ properties: props })
            });
            this.whitelistEnabled = !this.whitelistEnabled;
            alert('Whitelist ' + (this.whitelistEnabled ? 'enabled' : 'disabled') + '. Restart the server to apply.');
        } catch (e) { alert('Failed to toggle whitelist: ' + e.message); }
    },
    async addWhitelist() {
        const name = this.playerForms.whitelist.trim();
        if (!name) return;
        try {
            const res = await this.api(`/api/instances/${this.selectedInstance.id}/players/whitelist`, {
                method: 'POST',
                body: JSON.stringify({ name })
            });
            if (res.sent === false) alert(res.error || 'Could not add player');
            this.playerForms.whitelist = '';
            this.loadPlayers();
        } catch (e) { alert('Failed: ' + e.message); }
    },
    async removeWhitelist(name) {
        try {
            const res = await this.api(`/api/instances/${this.selectedInstance.id}/players/whitelist/${encodeURIComponent(name)}`, { method: 'DELETE' });
            if (res.sent === false) alert(res.error || 'Could not remove player');
            this.loadPlayers();
        } catch (e) { alert('Failed: ' + e.message); }
    },
    async addOp() {
        const name = this.playerForms.op.trim();
        if (!name) return;
        try {
            const res = await this.api(`/api/instances/${this.selectedInstance.id}/players/ops`, {
                method: 'POST',
                body: JSON.stringify({ name })
            });
            if (res.sent === false) alert(res.error || 'Could not op player');
            this.playerForms.op = '';
            this.loadPlayers();
        } catch (e) { alert('Failed: ' + e.message); }
    },
    async removeOp(name) {
        try {
            const res = await this.api(`/api/instances/${this.selectedInstance.id}/players/ops/${encodeURIComponent(name)}`, { method: 'DELETE' });
            if (res.sent === false) alert(res.error || 'Could not deop player');
            this.loadPlayers();
        } catch (e) { alert('Failed: ' + e.message); }
    },
    async addBan() {
        const name = this.banForm.name.trim();
        if (!name) return;
        try {
            const res = await this.api(`/api/instances/${this.selectedInstance.id}/players/bans`, {
                method: 'POST',
                body: JSON.stringify({ name, reason: this.banForm.reason.trim() })
            });
            if (res.sent === false) alert(res.error || 'Could not ban player');
            this.banForm.name = '';
            this.banForm.reason = '';
            this.loadPlayers();
        } catch (e) { alert('Ban failed: ' + e.message); }
    },
    async removeBan(name) {
        try {
            const res = await this.api(`/api/instances/${this.selectedInstance.id}/players/bans/${encodeURIComponent(name)}`, { method: 'DELETE' });
            if (res.sent === false) alert(res.error || 'Could not unban player');
            this.loadPlayers();
        } catch (e) { alert('Unban failed: ' + e.message); }
    },
    async kickPlayer(name) {
        if (!name || !this.selectedInstance) return;
        const playerName = typeof name === 'string' ? name : (name.name || '');
        const reason = prompt(`Kick reason for "${playerName}" (optional):`, 'Kicked by administrator');
        if (reason === null) return;
        try {
            const res = await this.api(`/api/instances/${this.selectedInstance.id}/players/kick`, {
                method: 'POST',
                body: JSON.stringify({ name: playerName, reason: reason.trim() })
            });
            if (res.sent === false) alert(res.error || 'Could not kick player');
            this.loadPlayers();
        } catch (e) { alert('Kick failed: ' + e.message); }
    },
    async banOnlinePlayer(name) {
        if (!name || !this.selectedInstance) return;
        const playerName = typeof name === 'string' ? name : (name.name || '');
        const reason = prompt(`Ban reason for "${playerName}" (optional):`, 'Banned by administrator');
        if (reason === null) return;
        try {
            const res = await this.api(`/api/instances/${this.selectedInstance.id}/players/bans`, {
                method: 'POST',
                body: JSON.stringify({ name: playerName, reason: reason.trim() })
            });
            if (res.sent === false) alert(res.error || 'Could not ban player');
            this.loadPlayers();
        } catch (e) { alert('Ban failed: ' + e.message); }
    },
    async opPlayerDirect(name) {
        if (!name || !this.selectedInstance) return;
        const playerName = typeof name === 'string' ? name : (name.name || '');
        try {
            const res = await this.api(`/api/instances/${this.selectedInstance.id}/players/ops`, {
                method: 'POST',
                body: JSON.stringify({ name: playerName })
            });
            if (res.sent === false) alert(res.error || 'Could not op player');
            this.loadPlayers();
        } catch (e) { alert('Failed to op player: ' + e.message); }
    },
    isPlayerOp(name) {
        if (!name || !this.opPlayers) return false;
        const target = (typeof name === 'string' ? name : (name.name || '')).toLowerCase();
        return this.opPlayers.some(p => {
            const pName = (typeof p === 'string' ? p : (p.name || '')).toLowerCase();
            return pName === target;
        });
    },
};
