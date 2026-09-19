// Zircon admin SPA - Shared constants, API helper, formatting, and polling.

// Built-in stylized alert system fallback
(function () {
    if (window.__zirconAlertInitialized) return;
    window.__zirconAlertInitialized = true;
    const queue = [];
    let isShowing = false;
    function ensureStyles() {
        if (document.getElementById('z-alert-injected-styles')) return;
        const style = document.createElement('style');
        style.id = 'z-alert-injected-styles';
        style.textContent = `
            .z-alert-backdrop {
                position: fixed; inset: 0; z-index: 999999;
                background: rgba(4, 7, 13, 0.8);
                backdrop-filter: blur(8px); -webkit-backdrop-filter: blur(8px);
                display: flex; align-items: center; justify-content: center;
                padding: 1.25rem;
                animation: z-alert-fade-in 0.2s cubic-bezier(0.16, 1, 0.3, 1) forwards;
            }
            .z-alert-backdrop.closing { animation: z-alert-fade-out 0.18s cubic-bezier(0.16, 1, 0.3, 1) forwards; }
            .z-alert-card {
                position: relative; width: 100%; max-width: 420px;
                background: radial-gradient(135% 135% at 50% 0%, #111b2b 0%, #080d16 100%);
                border: 1px solid rgba(71, 210, 201, 0.3); border-radius: 1.125rem;
                box-shadow: 0 25px 60px -10px rgba(0, 0, 0, 0.9), 0 0 40px rgba(71, 210, 201, 0.18), inset 0 1px 0 rgba(255, 255, 255, 0.1);
                padding: 1.75rem 1.5rem 1.5rem; display: flex; flex-direction: column; align-items: center; text-align: center; gap: 1.125rem; overflow: hidden;
                animation: z-alert-pop-in 0.25s cubic-bezier(0.16, 1, 0.3, 1) forwards;
            }
            .z-alert-backdrop.closing .z-alert-card { animation: z-alert-pop-out 0.18s cubic-bezier(0.16, 1, 0.3, 1) forwards; }
            .z-alert-glow {
                position: absolute; top: 0; left: 0; right: 0; height: 120px;
                background: radial-gradient(ellipse 85% 100% at 50% 0%, rgba(71, 210, 201, 0.25) 0%, transparent 75%);
                pointer-events: none;
            }
            .z-alert-card.state-error {
                border-color: rgba(244, 63, 94, 0.45);
                box-shadow: 0 25px 60px -10px rgba(0, 0, 0, 0.9), 0 0 45px rgba(244, 63, 94, 0.22), inset 0 1px 0 rgba(244, 63, 94, 0.25);
            }
            .z-alert-card.state-error .z-alert-glow {
                background: radial-gradient(ellipse 85% 100% at 50% 0%, rgba(244, 63, 94, 0.3) 0%, transparent 75%);
            }
            .z-alert-card.state-success {
                border-color: rgba(16, 185, 129, 0.45);
                box-shadow: 0 25px 60px -10px rgba(0, 0, 0, 0.9), 0 0 45px rgba(16, 185, 129, 0.22), inset 0 1px 0 rgba(16, 185, 129, 0.25);
            }
            .z-alert-card.state-success .z-alert-glow {
                background: radial-gradient(ellipse 85% 100% at 50% 0%, rgba(16, 185, 129, 0.3) 0%, transparent 75%);
            }
            .z-alert-header {
                display: flex; flex-direction: column; align-items: center; text-align: center; gap: 0.75rem; width: 100%; position: relative; z-index: 1;
            }
            .z-alert-icon-wrap {
                width: 3.25rem; height: 3.25rem; border-radius: 1rem;
                display: flex; align-items: center; justify-content: center; flex-shrink: 0;
                background: rgba(71, 210, 201, 0.12); border: 1px solid rgba(71, 210, 201, 0.35); color: #47d2c9;
                box-shadow: 0 0 20px rgba(71, 210, 201, 0.15);
            }
            .z-alert-card.state-error .z-alert-icon-wrap {
                background: rgba(244, 63, 94, 0.14); border-color: rgba(244, 63, 94, 0.4); color: #f43f5e;
                box-shadow: 0 0 20px rgba(244, 63, 94, 0.2);
            }
            .z-alert-card.state-success .z-alert-icon-wrap {
                background: rgba(16, 185, 129, 0.14); border-color: rgba(16, 185, 129, 0.4); color: #34d399;
                box-shadow: 0 0 20px rgba(16, 185, 129, 0.2);
            }
            .z-alert-title-wrap { display: flex; flex-direction: column; align-items: center; gap: 0.25rem; }
            .z-alert-subtitle {
                font-size: 0.6875rem; font-weight: 700; letter-spacing: 0.08em;
                text-transform: uppercase; color: #94a3b8; font-family: ui-sans-serif, system-ui, sans-serif;
            }
            .z-alert-title {
                font-size: 1.125rem; font-weight: 800; color: #ffffff; letter-spacing: -0.01em; margin: 0;
                font-family: ui-sans-serif, system-ui, sans-serif;
            }
            .z-alert-body { width: 100%; padding: 0.125rem 0.5rem; position: relative; z-index: 1; }
            .z-alert-message {
                color: #cbd5e1; font-size: 0.9375rem; line-height: 1.6; margin: 0;
                white-space: pre-wrap; word-break: break-word; font-family: ui-sans-serif, system-ui, sans-serif; text-align: center;
            }
            .z-alert-footer { width: 100%; margin-top: 0.5rem; position: relative; z-index: 1; }
            .z-alert-btn {
                width: 100%; display: block; text-align: center;
                background: linear-gradient(135deg, var(--color-accent-bright, #5adfd5) 0%, var(--color-accent, #47d2c9) 50%, var(--color-accent-deep, #20b2aa) 100%);
                border: 1px solid rgba(255, 255, 255, 0.25); color: var(--color-accent-ink, #022623); font-weight: 800;
                font-size: 0.875rem; letter-spacing: 0.02em; padding: 0.75rem 1.25rem;
                border-radius: 0.75rem; cursor: pointer; box-shadow: 0 4px 18px var(--color-accent-glow, rgba(71, 210, 201, 0.35));
                transition: all 0.2s cubic-bezier(0.4, 0, 0.2, 1); outline: none; font-family: ui-sans-serif, system-ui, sans-serif;
            }
            .z-alert-btn:hover {
                background: linear-gradient(135deg, var(--color-accent-bright, #5adfd5) 0%, var(--color-accent, #47d2c9) 50%, var(--color-accent-deep, #20b2aa) 100%);
                filter: brightness(1.1);
                box-shadow: 0 6px 24px var(--color-accent-glow-strong, rgba(71, 210, 201, 0.55));
                transform: translateY(-1px);
            }
            .z-alert-btn:active {
                transform: translateY(1px) scale(0.98);
                box-shadow: 0 2px 10px var(--color-accent-glow, rgba(71, 210, 201, 0.35));
            }
            @keyframes z-alert-fade-in { from { opacity: 0; } to { opacity: 1; } }
            @keyframes z-alert-fade-out { from { opacity: 1; } to { opacity: 0; } }
            @keyframes z-alert-pop-in { from { opacity: 0; transform: scale(0.94) translateY(12px); } to { opacity: 1; transform: scale(1) translateY(0); } }
            @keyframes z-alert-pop-out { from { opacity: 1; transform: scale(1) translateY(0); } to { opacity: 0; transform: scale(0.96) translateY(6px); } }
        `;
        document.head.appendChild(style);
    }
    function sanitize(str) {
        if (str === null || str === undefined) return '';
        const temp = document.createElement('div');
        temp.textContent = String(str);
        return temp.innerHTML;
    }
    function detectCategory(msg) {
        const text = String(msg || '');
        if (/failed|error|cannot|could not|rejected|invalid|denied|exception/i.test(text)) return { type: 'error', title: 'Action Failed', subtitle: 'Zircon Error Alert' };
        if (/please enter|required|missing|must be/i.test(text)) return { type: 'error', title: 'Action Required', subtitle: 'Validation Notice' };
        if (/success|saved|deployed|restored|updated|completed|enabled|applied/i.test(text)) return { type: 'success', title: 'Action Completed', subtitle: 'Zircon Success' };
        return { type: 'info', title: 'System Notice', subtitle: 'Zircon System Alert' };
    }
    function getIconSvg(type) {
        if (type === 'error') return `<svg class="w-6 h-6" style="width: 1.5rem; height: 1.5rem;" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="10"></circle><line x1="12" y1="8" x2="12" y2="12"></line><line x1="12" y1="16" x2="12.01" y2="16"></line></svg>`;
        if (type === 'success') return `<svg class="w-6 h-6" style="width: 1.5rem; height: 1.5rem;" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round" stroke-linejoin="round"><path d="M22 11.08V12a10 10 0 1 1-5.93-9.14"></path><polyline points="22 4 12 14.01 9 11.01"></polyline></svg>`;
        return `<svg class="w-6 h-6" style="width: 1.5rem; height: 1.5rem;" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="10"></circle><line x1="12" y1="8" x2="12" y2="12"></line><line x1="12" y1="8" x2="12.01" y2="8"></line></svg>`;
    }
    function showZirconAlert(message, options) {
        ensureStyles();
        return new Promise((resolve) => {
            const opts = typeof options === 'string' ? { title: options } : (options || {});
            const detected = detectCategory(message);
            const type = opts.type || detected.type;
            const title = opts.title || detected.title;
            const subtitle = opts.subtitle || detected.subtitle;
            const buttonText = opts.buttonText || 'Understood';
            queue.push({ message: String(message !== undefined && message !== null ? message : ''), type, title, subtitle, buttonText, resolve });
            processQueue();
        });
    }
    function processQueue() {
        if (isShowing || queue.length === 0) return;
        isShowing = true;
        const current = queue.shift();
        const backdrop = document.createElement('div');
        backdrop.className = 'z-alert-backdrop';
        backdrop.setAttribute('role', 'dialog');
        backdrop.setAttribute('aria-modal', 'true');
        const card = document.createElement('div');
        card.className = `z-alert-card state-${current.type}`;
        card.innerHTML = `
            <div class="z-alert-glow"></div>
            <div class="z-alert-header">
                <div class="z-alert-icon-wrap">${getIconSvg(current.type)}</div>
                <div class="z-alert-title-wrap">
                    <span class="z-alert-subtitle">${sanitize(current.subtitle)}</span>
                    <h4 class="z-alert-title">${sanitize(current.title)}</h4>
                </div>
            </div>
            <div class="z-alert-body"><p class="z-alert-message">${sanitize(current.message)}</p></div>
            <div class="z-alert-footer"><button type="button" class="z-alert-btn">${sanitize(current.buttonText)}</button></div>
        `;
        backdrop.appendChild(card);
        document.body.appendChild(backdrop);
        const btn = card.querySelector('.z-alert-btn');
        if (btn) setTimeout(() => btn.focus(), 30);
        let dismissed = false;
        function dismiss() {
            if (dismissed) return;
            dismissed = true;
            document.removeEventListener('keydown', handleKeydown);
            backdrop.classList.add('closing');
            setTimeout(() => {
                if (backdrop.parentNode) backdrop.parentNode.removeChild(backdrop);
                isShowing = false;
                current.resolve(true);
                processQueue();
            }, 180);
        }
        function handleKeydown(e) {
            if (e.key === 'Enter' || e.key === 'Escape' || e.key === ' ') { e.preventDefault(); dismiss(); }
        }
        document.addEventListener('keydown', handleKeydown);
        btn.addEventListener('click', dismiss);
        backdrop.addEventListener('click', (e) => { if (e.target === backdrop) dismiss(); });
    }
    window.showZirconAlert = showZirconAlert;
    window.zirconAlert = showZirconAlert;
    window.nativeAlert = window.alert;
    window.alert = function (msg) { return showZirconAlert(msg); };
})();

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
