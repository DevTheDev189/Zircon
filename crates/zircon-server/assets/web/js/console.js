// Zircon admin SPA - Live console (WebSocket) and commands.
window.Zircon = window.Zircon || {};
window.Zircon.console = {
        // ---- Console ----
    connectConsole() {
        const targetId = this.selectedInstance ? this.selectedInstance.id : null;

        // If already connected to this instance's console, do nothing
        if (this.consoleWs && this.currentConsoleInstanceId === targetId && (this.consoleWs.readyState === WebSocket.OPEN || this.consoleWs.readyState === WebSocket.CONNECTING)) {
            return;
        }

        // Close any existing console connection
        if (this.consoleWs) {
            this.consoleWs.onclose = null;
            this.consoleWs.close();
            this.consoleWs = null;
        }

        this.currentConsoleInstanceId = targetId;
        this.consoleLines = []; // Clear current lines on switch so old server's lines don't linger

        const proto = location.protocol === 'https:' ? 'wss' : 'ws';
        // Browsers cannot set headers on WebSocket handshakes, and a token in
        // the URL would leak into access logs and history — so the JWT is sent
        // as the first message and re-validated server-side.
        const url = targetId
            ? `${proto}://${location.host}/api/instances/${encodeURIComponent(targetId)}/console`
            : `${proto}://${location.host}/api/console`;

        this.consoleWs = new WebSocket(url);
        this.consoleWs.onopen = () => {
            if (this.jwtToken) this.consoleWs.send('AUTH ' + this.jwtToken);
        };
        this.consoleWs.onmessage = (ev) => {
            if (ev.data === '__CLEAR__') {
                this.consoleLines = [];
                return;
            }
            this.consoleLines.push(ev.data);
            if (this.consoleLines.length > 1000) this.consoleLines.shift();
            if (typeof ev.data === 'string' && (
                ev.data.includes('exited unexpectedly') ||
                ev.data.includes('---- Minecraft Crash Report ----') ||
                ev.data.includes('ModResolutionException') ||
                ev.data.includes('ModLoadingIssue') ||
                ev.data.includes('MissingMandatoryDependenciesException')
            )) {
                this.fetchCrashAnalysis();
            }
            this.$nextTick(() => {
                const box = this.$refs.consoleBox;
                if (box && this.autoScroll) box.scrollTop = box.scrollHeight;
            });
        };
        this.consoleWs.onclose = () => {
            if (this.currentConsoleInstanceId === targetId) {
                this.consoleWs = null;
            }
        };
    },
    clearConsole() {
        if (this.consoleWs && this.consoleWs.readyState === WebSocket.OPEN) {
            this.consoleWs.send('__CLEAR__');
        }
    },
    sendCommand() {
        if (this.consoleWs && this.consoleWs.readyState === WebSocket.OPEN && this.command.trim()) {
            this.consoleWs.send(this.command.trim());
            this.command = '';
        }
    },
    consoleColor(line) {
        if (line.includes('[ERROR]') || line.includes('ERROR')) return 'text-red-400';
        if (line.includes('[WARN]') || line.includes('WARN')) return 'text-yellow-400';
        if (line.includes('[wrapper]')) return 'text-emerald-400';
        return 'text-slate-300';
    }
};
