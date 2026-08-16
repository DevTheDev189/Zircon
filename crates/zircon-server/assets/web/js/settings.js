// Zircon admin SPA - Server properties editor.
window.Zircon = window.Zircon || {};
window.Zircon.settings = {
    async loadServerProperties() {
        if (!this.selectedInstance) return;
        try {
            const data = await this.api(`/api/instances/${this.selectedInstance.id}/server-properties`);
            const props = data.properties || {};
            // A fresh instance has no server.properties yet — seed the curated
            // fields with vanilla defaults so the editor is usable immediately.
            for (const [key, value] of Object.entries(DEFAULT_SERVER_PROPERTIES)) {
                if (props[key] === undefined || props[key] === '') props[key] = value;
            }
            this.serverProps = props;
            this.whitelistEnabled = this.serverProps['white-list'] === 'true';
        } catch (e) {
            this.serverProps = {};
        }
    },
    async saveServerProperties() {
        if (!this.selectedInstance) return;
        try {
            await this.api(`/api/instances/${this.selectedInstance.id}/server-properties`, {
                method: 'POST',
                body: JSON.stringify({ properties: this.serverProps })
            });
            alert('server.properties saved — restart the server to apply changes.');
            this.loadServerProperties();
        } catch (e) {
            alert('Could not save server.properties: ' + e.message);
        }
    },
};
