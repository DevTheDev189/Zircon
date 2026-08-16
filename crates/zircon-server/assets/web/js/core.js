// Zircon admin SPA - Shared constants, API helper, formatting, and polling.

const CURATED_PROPERTY_KEYS = [
    'motd', 'difficulty', 'gamemode', 'pvp', 'max-players',
    'view-distance', 'online-mode', 'enable-command-block', 'hardcore', 'spawn-protection'
];
// Vanilla defaults used when an instance has no server.properties yet.
const DEFAULT_SERVER_PROPERTIES = {
    'motd': 'A Minecraft Server',
    'difficulty': 'easy',
    'gamemode': 'survival',
    'pvp': 'true',
    'max-players': '20',
    'view-distance': '10',
    'online-mode': 'true',
    'enable-command-block': 'false',
    'hardcore': 'false',
    'spawn-protection': '16'
};

window.Zircon = window.Zircon || {};
window.Zircon.core = {
    async api(path, opts = {}) {
        opts.headers = { ...opts.headers, 'Authorization': 'Bearer ' + this.jwtToken, 'Content-Type': 'application/json' };
        const res = await fetch(path, opts);
        if (res.status === 401) {
            // Session expired or revoked (JWT TTL is 12h) — drop it and show login.
            this.logout();
            throw new Error('Session expired. Please log in again.');
        }
        if (!res.ok) throw new Error(await res.text());
        // 204 No Content (e.g. DELETE endpoints) has no body to parse.
        if (res.status === 204) return {};
        return res.json();
    },
    async loadStats() {
        try {
            this.systemStats = await this.api('/api/stats');
        } catch (e) { /* stats are best-effort */ }
    },
    startPolling() {
        if (this.pollTimer) clearInterval(this.pollTimer);
        this.pollTimer = setInterval(() => {
            if (!this.authenticated) return;
            this.loadInstances();
            if (this.activeTab === 'stats') this.loadStats();
            if (this.activeTab === 'players') this.loadPlayers();
            if (this.activeTab === 'backups') this.loadBackups();
        }, 5000);
    },
    formatBytes(bytes) {
        if (!bytes) return '0 B';
        const k = 1024, sizes = ['B', 'KB', 'MB', 'GB', 'TB'];
        const i = Math.floor(Math.log(bytes) / Math.log(k));
        return parseFloat((bytes / Math.pow(k, i)).toFixed(1)) + ' ' + sizes[i];
    },
    formatDate(ts) {
        if (!ts) return '—';
        return new Date(ts).toLocaleString();
    },
};
