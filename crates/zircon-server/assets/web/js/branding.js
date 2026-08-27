// Zircon admin SPA - Server Branding (Icon & Animated Banner Management)
window.Zircon = window.Zircon || {};
window.Zircon.branding = {
    async loadBranding() {
        if (!this.selectedInstance) return;
        try {
            const data = await this.api(`/api/instances/${this.selectedInstance.id}/branding`);
            this.branding = {
                hasIcon: data.hasIcon,
                hasBanner: data.hasBanner,
                bannerIsAnimated: data.bannerIsAnimated,
                iconSha1: data.iconSha1,
                bannerSha1: data.bannerSha1,
                iconUrl: data.hasIcon ? `/api/instances/${this.selectedInstance.id}/branding/icon?t=${Date.now()}` : null,
                bannerUrl: data.hasBanner ? `/api/instances/${this.selectedInstance.id}/branding/banner?t=${Date.now()}` : null,
                loading: false
            };
        } catch (e) {
            console.error('Failed to load branding:', e);
        }
    },

    triggerIconUpload() {
        const el = document.getElementById('server-icon-upload-input');
        if (el) el.click();
    },

    triggerBannerUpload() {
        const el = document.getElementById('server-banner-upload-input');
        if (el) el.click();
    },

    async uploadIcon(event) {
        const files = event.target.files;
        if (!files || !files.length || !this.selectedInstance) return;
        const file = files[0];

        if (file.size > 2 * 1024 * 1024) {
            alert('Server icon must be under 2 MiB.');
            event.target.value = '';
            return;
        }

        const formData = new FormData();
        formData.append('file', file);
        this.branding.loading = true;

        try {
            const res = await fetch(`/api/instances/${this.selectedInstance.id}/branding/icon`, {
                method: 'POST',
                headers: { 'Authorization': `Bearer ${this.jwtToken}` },
                body: formData
            });
            if (!res.ok) {
                const text = await res.text();
                throw new Error(text || 'Failed to upload icon');
            }
            await this.loadBranding();
        } catch (e) {
            alert(`Error uploading icon: ${e.message || e}`);
        } finally {
            this.branding.loading = false;
            event.target.value = '';
        }
    },

    async uploadBanner(event) {
        const files = event.target.files;
        if (!files || !files.length || !this.selectedInstance) return;
        const file = files[0];

        if (file.size > 10 * 1024 * 1024) {
            alert('Server banner must be under 10 MiB.');
            event.target.value = '';
            return;
        }

        const formData = new FormData();
        formData.append('file', file);
        this.branding.loading = true;

        try {
            const res = await fetch(`/api/instances/${this.selectedInstance.id}/branding/banner`, {
                method: 'POST',
                headers: { 'Authorization': `Bearer ${this.jwtToken}` },
                body: formData
            });
            if (!res.ok) {
                const text = await res.text();
                throw new Error(text || 'Failed to upload banner');
            }
            await this.loadBranding();
        } catch (e) {
            alert(`Error uploading banner: ${e.message || e}`);
        } finally {
            this.branding.loading = false;
            event.target.value = '';
        }
    },

    async removeIcon() {
        if (!confirm('Are you sure you want to remove the custom server icon?')) return;
        this.branding.loading = true;
        try {
            await this.api(`/api/instances/${this.selectedInstance.id}/branding/icon`, {
                method: 'DELETE'
            });
            await this.loadBranding();
        } catch (e) {
            alert(`Error removing icon: ${e.message || e}`);
        } finally {
            this.branding.loading = false;
        }
    },

    async removeBanner() {
        if (!confirm('Are you sure you want to remove the custom server banner?')) return;
        this.branding.loading = true;
        try {
            await this.api(`/api/instances/${this.selectedInstance.id}/branding/banner`, {
                method: 'DELETE'
            });
            await this.loadBranding();
        } catch (e) {
            alert(`Error removing banner: ${e.message || e}`);
        } finally {
            this.branding.loading = false;
        }
    }
};
