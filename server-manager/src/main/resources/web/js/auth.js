// Zircon admin SPA - Authentication and profile management.
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
            await this.loadAuthMe();
            this.loadInstances();
            this.startPolling();
        } else {
            alert('Invalid credentials');
        }
    },
    async loadAuthMe() {
        try {
            const data = await this.api('/api/auth/me');
            this.currentUser = { username: data.username, icon: data.icon };
            this.profileForm = { username: data.username, currentPassword: '', newPassword: '' };
        } catch (e) { /* fall back to login-provided name */ }
    },
    logout() {
        this.authenticated = false;
        this.jwtToken = '';
        this.selectedInstance = null;
        this.showProfileModal = false;
        if (this.pollTimer) clearInterval(this.pollTimer);
        if (this.consoleWs) this.consoleWs.close();
    },
    async saveProfile() {
        try {
            await this.api('/api/auth/profile', {
                method: 'POST',
                body: JSON.stringify({
                    currentUsername: this.currentUser.username,
                    newUsername: this.profileForm.username,
                    currentPassword: this.profileForm.currentPassword,
                    newPassword: this.profileForm.newPassword
                })
            });
            this.currentUser.username = this.profileForm.username;
            this.profileForm.currentPassword = '';
            this.profileForm.newPassword = '';
            this.showProfileModal = false;
            alert('Profile updated successfully!');
        } catch (e) { alert('Update failed: ' + e.message); }
    },
};
