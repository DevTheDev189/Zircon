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
        if (opts.body && typeof opts.body === 'object' && !(opts.body instanceof FormData)) {
            opts.body = JSON.stringify(opts.body);
        }
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
    async loadStats(targetInstanceId) {
        try {
            const instId = targetInstanceId || this.statsSelectedInstanceId || this.selectedInstance?.id;
            const query = instId ? `?instanceId=${encodeURIComponent(instId)}` : '';
            this.systemStats = await this.api(`/api/stats${query}`);
            if (!this.statsSelectedInstanceId && this.systemStats?.activeInstanceId) {
                this.statsSelectedInstanceId = this.systemStats.activeInstanceId;
            }
        } catch (e) { /* stats are best-effort */ }
    },
    selectStatsInstance(instanceId) {
        if (this.statsSelectedInstanceId === instanceId) return;
        this.statsSelectedInstanceId = instanceId;
        // Instantly reset the TPS & MSPT graph arrays to prevent stale cross-server graph bleeding
        if (this.systemStats) {
            this.systemStats.tps = null;
            this.systemStats.mspt = null;
            this.systemStats.pingLatencyMs = null;
            this.systemStats.tpsHistory = [];
            this.systemStats.msptHistory = [];
        }
        this.loadStats(instanceId);
    },
    startPolling() {
        if (this.pollTimer) clearInterval(this.pollTimer);
        this.pollTimer = setInterval(async () => {
            if (!this.authenticated || (typeof document !== 'undefined' && document.hidden)) return;
            try {
                await this.loadInstances();
                if (this.activeTab === 'stats') await this.loadStats();
                if (this.activeTab === 'players') await this.loadPlayers();
                if (this.activeTab === 'backups') await this.loadBackups();
            } catch (e) {
                // Connection or server temporary network error
            }
        }, 5000);

        // Tire the idle-sleep countdown down locally so the badge ticks every
        // second between the 5s instance polls (the server re-syncs the true
        // value on each poll, so drift stays bounded).
        if (this.idleTicker) clearInterval(this.idleTicker);
        this.idleTicker = setInterval(() => {
            if (!this.authenticated || !this.instances) return;
            for (const inst of this.instances) {
                if (
                    inst.running &&
                    !inst.stopping &&
                    inst.playerCount === 0 &&
                    typeof inst.idleRemainingSeconds === 'number' &&
                    inst.idleRemainingSeconds > 0
                ) {
                    inst.idleRemainingSeconds = inst.idleRemainingSeconds - 1;
                }
            }
        }, 1000);
    },
    isStopping(inst) {
        if (!inst) return false;
        return Boolean(
            inst.stopping ||
            this.actionLoading[inst.id] === 'manual' ||
            this.actionLoading[inst.id] === 'stopping' ||
            (inst.running && inst.playerCount === 0 && typeof inst.idleRemainingSeconds === 'number' && inst.idleRemainingSeconds <= 0)
        );
    },
    isFallingAsleep(inst) {
        if (!inst) return false;
        if (inst.stoppingReason === 'idle') return true;
        if (inst.running && !inst.stopping && inst.playerCount === 0 && typeof inst.idleRemainingSeconds === 'number' && inst.idleRemainingSeconds <= 0) return true;
        return false;
    },
    isShuttingDown(inst) {
        if (!inst) return false;
        return this.isStopping(inst) && !this.isFallingAsleep(inst);
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
    // Formats a whole number of seconds as "{m}m {ss}s" for the idle-sleep
    // countdown badge (e.g. 222 -> "3m 42s").
    formatIdleTime(seconds) {
        const s = Math.max(0, Math.floor(seconds));
        const m = Math.floor(s / 60);
        const remain = s % 60;
        return `${m}m ${remain < 10 ? '0' : ''}${remain}s`;
    },
    async checkServerUpdate() {
        this.serverUpdateChecking = true;
        this.serverUpdateStatus = 'checking';
        this.serverUpdateError = '';
        try {
            const data = await this.api('/api/system/update/check');
            this.serverCurrentVersion = data.currentVersion || '0.4.2';
            if (data.updateAvailable && data.manifest) {
                this.serverUpdateAvailable = true;
                this.serverUpdateManifest = data.manifest;
                this.serverUpdateStatus = 'available';
            } else {
                this.serverUpdateAvailable = false;
                this.serverUpdateManifest = null;
                this.serverUpdateStatus = 'up-to-date';
            }
        } catch (e) {
            this.serverUpdateStatus = 'error';
            this.serverUpdateError = e.message || String(e);
        } finally {
            this.serverUpdateChecking = false;
        }
    },
    async applyServerUpdate() {
        if (!confirm('Applying this update will stop all running Minecraft server instances, update the Zircon Server executable, and restart the server daemon. Proceed?')) {
            return;
        }
        this.serverUpdateApplying = true;
        this.serverUpdateStatus = 'applying';
        this.serverUpdateError = '';
        try {
            const res = await this.api('/api/system/update/apply', { method: 'POST' });
            this.serverUpdateStatus = 'restarting';
            alert(res.message || 'Server updated. Restarting daemon...');
            setTimeout(() => {
                window.location.reload();
            }, 4000);
        } catch (e) {
            this.serverUpdateApplying = false;
            this.serverUpdateStatus = 'error';
            this.serverUpdateError = e.message || String(e);
            alert('Update failed: ' + (e.message || e));
        }
    },
    // Smooth cubic spline / Catmull-Rom interpolation for SVG area & line charts.
    buildSvgSpline(dataPoints, width = 600, height = 160, maxVal = 100, minVal = 0, isArea = false, padBottom = 8, padTop = 8) {
        if (!dataPoints || !dataPoints.length) {
            return isArea ? `M 0,${height} L ${width},${height} Z` : `M 0,${height} L ${width},${height}`;
        }
        const usableHeight = height - padTop - padBottom;
        const range = (maxVal - minVal) || 1;
        const pts = dataPoints.map((v, i) => {
            const val = typeof v === 'number' ? v : (v != null ? Number(v) : 0);
            const clamped = Math.max(minVal, Math.min(maxVal, val));
            const x = dataPoints.length > 1 ? (i / (dataPoints.length - 1)) * width : width;
            const y = padTop + usableHeight - ((clamped - minVal) / range) * usableHeight;
            return { x, y };
        });

        if (pts.length === 1) {
            return isArea ? `M 0,${pts[0].y.toFixed(1)} L ${width},${pts[0].y.toFixed(1)} V ${height} H 0 Z` : `M 0,${pts[0].y.toFixed(1)} L ${width},${pts[0].y.toFixed(1)}`;
        }

        let d = `M ${pts[0].x.toFixed(1)},${pts[0].y.toFixed(1)}`;
        for (let i = 0; i < pts.length - 1; i++) {
            const p0 = pts[i === 0 ? i : i - 1];
            const p1 = pts[i];
            const p2 = pts[i + 1];
            const p3 = pts[i + 2 < pts.length ? i + 2 : i + 1];

            const cp1x = p1.x + (p2.x - p0.x) / 6;
            const cp1y = p1.y + (p2.y - p0.y) / 6;
            const cp2x = p2.x - (p3.x - p1.x) / 6;
            const cp2y = p2.y - (p3.y - p1.y) / 6;

            d += ` C ${cp1x.toFixed(1)},${cp1y.toFixed(1)} ${cp2x.toFixed(1)},${cp2y.toFixed(1)} ${p2.x.toFixed(1)},${p2.y.toFixed(1)}`;
        }

        if (isArea) {
            d += ` V ${height} H ${pts[0].x.toFixed(1)} Z`;
        }
        return d;
    },
    getTpsInfo(tps, isOffline = false) {
        if (isOffline || tps == null || typeof tps !== 'number' || isNaN(tps) || tps <= 0) {
            return { status: 'offline', label: 'Offline', color: 'slate', stroke: '#64748b', fill: 'rgba(100,116,139,0.15)', text: 'text-slate-400', badgeBg: 'bg-slate-800/80', badgeBorder: 'border-slate-700/60', percent: 0, ringOffset: 251.2 };
        }
        const val = Math.max(0, Math.min(20, tps));
        // Circumference for r=40 is 2 * PI * 40 ~= 251.3
        const circ = 251.32;
        const ringOffset = circ - (val / 20) * circ;
        if (tps >= 19.5) {
            return { status: 'optimal', label: 'Optimal (20 TPS)', color: 'emerald', stroke: '#4ade80', fill: 'rgba(74,222,128,0.2)', text: 'text-[#4ade80]', badgeBg: 'bg-emerald-500/15', badgeBorder: 'border-emerald-500/30', percent: (tps / 20) * 100, ringOffset };
        }
        if (tps >= 15.0) {
            return { status: 'moderate', label: 'Moderate Load', color: 'amber', stroke: '#fbbf24', fill: 'rgba(251,191,36,0.2)', text: 'text-amber-400', badgeBg: 'bg-amber-500/15', badgeBorder: 'border-amber-500/30', percent: (tps / 20) * 100, ringOffset };
        }
        return { status: 'critical', label: 'Severe Tick Bottleneck', color: 'rose', stroke: '#f87171', fill: 'rgba(248,113,113,0.2)', text: 'text-rose-400', badgeBg: 'bg-rose-500/15', badgeBorder: 'border-rose-500/30', percent: (tps / 20) * 100, ringOffset };
    },
    getMsptInfo(mspt) {
        if (mspt == null || typeof mspt !== 'number' || isNaN(mspt)) {
            return { label: '—', text: 'text-slate-400', sub: 'No telemetry active' };
        }
        if (mspt <= 30.0) {
            return { label: `${mspt.toFixed(1)} ms`, text: 'text-[#4ade80]', sub: 'Optimal tick headroom (< 30ms)' };
        }
        if (mspt <= 50.0) {
            return { label: `${mspt.toFixed(1)} ms`, text: 'text-amber-400', sub: 'Acceptable tick headroom (30–50ms)' };
        }
        return { label: `${mspt.toFixed(1)} ms`, text: 'text-rose-400', sub: 'Tick stuttering / lag (> 50ms)' };
    },
    formatTps(tps) {
        if (tps == null || typeof tps !== 'number' || isNaN(tps) || tps <= 0) return '0.0';
        return tps.toFixed(1);
    },
    formatMspt(mspt) {
        if (mspt == null || typeof mspt !== 'number' || isNaN(mspt)) return '—';
        return `${mspt.toFixed(1)} ms`;
    },
    handleChartHover(event, chartType) {
        const svg = event.currentTarget;
        const rect = svg.getBoundingClientRect();
        const mouseX = Math.max(0, Math.min(rect.width, event.clientX - rect.left));
        const history = this.systemStats?.history || [];
        if (!history.length) return;

        const fraction = mouseX / rect.width;
        const idx = Math.min(history.length - 1, Math.max(0, Math.round(fraction * (history.length - 1))));
        const point = history[idx];
        if (!point) return;

        this.chartTooltip = {
            visible: true,
            x: event.clientX - rect.left,
            y: event.clientY - rect.top,
            time: point.timestamp ? new Date(point.timestamp).toLocaleTimeString() : `Sample ${idx + 1}`,
            chartType,
            systemCpu: point.systemCpuLoad || 0,
            processCpu: point.processCpuLoad || 0,
            usedMemory: this.formatBytes(point.usedMemoryBytes || 0),
            maxMemory: this.formatBytes(point.maxMemoryBytes || 0),
            tps: point.tps != null ? point.tps.toFixed(1) : (this.systemStats?.tpsHistory?.[idx] != null ? this.systemStats.tpsHistory[idx].toFixed(1) : '0.0'),
            mspt: point.mspt != null ? `${point.mspt.toFixed(1)} ms` : (this.systemStats?.msptHistory?.[idx] != null ? `${this.systemStats.msptHistory[idx].toFixed(1)} ms` : '—'),
            ping: point.pingLatencyMs != null ? `${point.pingLatencyMs} ms` : '—'
        };
    },
    hideChartTooltip() {
        this.chartTooltip = { visible: false };
    }
};
