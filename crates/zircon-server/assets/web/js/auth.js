// Zircon admin SPA - Authentication and profile management.

// localStorage key for the persisted admin JWT, so a reload (or browser
// restart) stays logged in for the token lifetime (12h TTL server-side).
// Module-scope const: Vue 3 only exposes *functions* from the `methods`
// option on the component proxy, so a `this.TOKEN_STORAGE_KEY` property there
// would always be undefined and the session could never be restored.
const TOKEN_STORAGE_KEY = 'zircon.adminToken';

window.Zircon = window.Zircon || {};
window.Zircon.auth = {
    async login() {
        const res = await fetch('/api/auth/login', {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify(this.loginForm)
        });
        if (res.ok) {
            const data = await res.json();
            this.jwtToken = data.token;
            this.currentUser.username = data.username;
            this.authenticated = true;
            this.sessionRestoring = false;
            try { localStorage.setItem(TOKEN_STORAGE_KEY, this.jwtToken); } catch (e) { /* storage unavailable */ }
            await this.loadAuthMe();
            this.loadInstances();
            this.startPolling();
        } else {
            alert('Invalid credentials');
        }
    },
    // Called on app boot: restores a previously persisted session. The stored
    // JWT is validated against /api/auth/me before the dashboard is shown, so
    // expired or revoked tokens fall back to the login screen cleanly.
    async restoreSession() {
        let token = null;
        try { token = localStorage.getItem(TOKEN_STORAGE_KEY); } catch (e) { /* storage unavailable */ }
        if (!token) { this.sessionRestoring = false; return; }

        this.jwtToken = token;
        try {
            // loadAuthMe() swallows errors, so validate directly here. On a 401
            // api() calls logout(), which also clears the stored token.
            const data = await this.api('/api/auth/me');
            this.currentUser = { username: data.username, icon: data.icon };
            this.profileForm = { username: data.username, currentPassword: '', newPassword: '' };
            this.authenticated = true;
        } catch (e) {
            // Invalid/expired token or a transient error — show the login screen.
            this.authenticated = false;
        } finally {
            this.sessionRestoring = false;
        }
        if (this.authenticated) {
            this.loadInstances();
            this.startPolling();
        }
    },
    async loadAuthMe() {
        try {
            const data = await this.api('/api/auth/me');
            this.currentUser = { username: data.username, icon: data.icon };
            this.profileForm = { username: data.username, currentPassword: '', newPassword: '' };
        } catch (e) { /* fall back to login-provided name */ }
    },
    async logout() {
        const token = this.jwtToken;
        // Clear local state immediately (synchronous up to the first await) so
        // the UI never waits on the network to sign out.
        this.authenticated = false;
        this.jwtToken = '';
        this.selectedInstance = null;
        this.showProfileModal = false;
        try { localStorage.removeItem(TOKEN_STORAGE_KEY); } catch (e) { /* storage unavailable */ }
        if (this.pollTimer) clearInterval(this.pollTimer);
        if (this.consoleWs) this.consoleWs.close();
        // Best-effort server-side revocation so the token dies immediately, not
        // just at its 12h expiry. Raw fetch (not this.api) avoids recursion on 401.
        if (token) {
            try {
                await fetch('/api/auth/logout', {
                    method: 'POST',
                    headers: { 'Authorization': 'Bearer ' + token }
                });
            } catch (e) { /* server unreachable — the token still expires on its own */ }
        }
    },
    async saveProfile() {
        try {
            const data = await this.api('/api/auth/profile', {
                method: 'POST',
                body: JSON.stringify({
                    currentUsername: this.currentUser.username,
                    newUsername: this.profileForm.username,
                    currentPassword: this.profileForm.currentPassword,
                    newPassword: this.profileForm.newPassword
                })
            });
            this.currentUser.username = this.profileForm.username;
            if (data.token) {
                // Password changed server-side: other sessions were revoked and
                // this one was re-issued — adopt and persist the fresh token.
                this.jwtToken = data.token;
                try { localStorage.setItem(TOKEN_STORAGE_KEY, data.token); } catch (e) { /* storage unavailable */ }
            }
            this.profileForm.currentPassword = '';
            this.profileForm.newPassword = '';
            this.showProfileModal = false;
            alert('Profile updated successfully!');
        } catch (e) { alert('Update failed: ' + e.message); }
    },
};
