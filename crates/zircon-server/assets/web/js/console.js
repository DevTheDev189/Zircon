// Zircon admin SPA - Live console (WebSocket) and commands.
window.Zircon = window.Zircon || {};
window.Zircon.console = {
        // ---- Console ----
    connectConsole() {
        if (this.consoleWs) return;
        const proto = location.protocol === 'https:' ? 'wss' : 'ws';
        // Browsers cannot set headers on WebSocket handshakes, and a token in
        // the URL would leak into access logs and history — so the JWT is sent
        // as the first message and re-validated server-side.
        this.consoleWs = new WebSocket(`${proto}://${location.host}/api/console`);
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
            this.$nextTick(() => {
                const box = this.$refs.consoleBox;
                if (box && this.autoScroll) box.scrollTop = box.scrollHeight;
            });
        };
        this.consoleWs.onclose = () => { this.consoleWs = null; };
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
