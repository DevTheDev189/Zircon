// Zircon admin SPA - File Manager and In-Browser Editor
window.Zircon = window.Zircon || {};
window.Zircon.files = {
    async fetchFiles(dirPath) {
        if (!this.selectedInstance) return;
        const target = dirPath !== undefined ? dirPath : (this.fileManager.currentPath || '');
        this.fileManager.loading = true;
        this.fileManager.error = '';
        try {
            const res = await this.api(`/api/instances/${this.selectedInstance.id}/files?path=${encodeURIComponent(target)}`);
            this.fileManager.files = res.files || [];
            this.fileManager.currentPath = res.currentPath || '';
            this.updateBreadcrumbs();
        } catch (e) {
            this.fileManager.error = e.message || 'Failed to load files';
        } finally {
            this.fileManager.loading = false;
        }
    },

    updateBreadcrumbs() {
        const parts = this.fileManager.currentPath ? this.fileManager.currentPath.split('/').filter(Boolean) : [];
        const crumbs = [{ name: 'server', path: '' }];
        let cum = '';
        for (const p of parts) {
            cum = cum ? `${cum}/${p}` : p;
            crumbs.push({ name: p, path: cum });
        }
        this.fileManager.breadcrumbs = crumbs;
    },

    navigateBreadcrumb(crumb) {
        this.fetchFiles(crumb.path);
    },

    navigateToFolder(folder) {
        this.fetchFiles(folder.path);
    },

    navigateUp() {
        if (!this.fileManager.currentPath) return;
        const parts = this.fileManager.currentPath.split('/').filter(Boolean);
        parts.pop();
        this.fetchFiles(parts.join('/'));
    },

    async openFile(file) {
        if (file.is_dir || file.isDir) {
            return this.navigateToFolder(file);
        }

        // Open text editor for editable files
        this.editorModal.open = true;
        this.editorModal.path = file.path;
        this.editorModal.name = file.name;
        this.editorModal.canSyncConfig = !!(file.can_sync_config || file.canSyncConfig);
        this.editorModal.isSyncedConfig = !!(file.is_synced_config || file.isSyncedConfig);
        this.editorModal.loading = true;
        this.editorModal.error = '';
        this.editorModal.saving = false;
        this.editorModal.saveSuccess = false;

        try {
            const res = await this.api(`/api/instances/${this.selectedInstance.id}/files/content?path=${encodeURIComponent(file.path)}`);
            this.editorModal.content = res.content || '';
            this.editorModal.originalContent = res.content || '';
            this.editorModal.size = res.size || 0;
        } catch (e) {
            this.editorModal.error = e.message || 'Failed to read file content (may be binary or too large)';
        } finally {
            this.editorModal.loading = false;
        }
    },

    async toggleEditorBomSync() {
        if (!this.selectedInstance || !this.editorModal.path) return;
        try {
            const res = await this.api(`/api/instances/${this.selectedInstance.id}/files/sync-toggle`, {
                method: 'POST',
                body: { path: this.editorModal.path }
            });
            this.editorModal.isSyncedConfig = res.synced;
            const item = this.fileManager.files.find(f => f.path === this.editorModal.path);
            if (item) {
                item.isSyncedConfig = res.synced;
                item.is_synced_config = res.synced;
            }
        } catch (e) {
            alert(`Cannot toggle BOM sync: ${e.message || e}`);
        }
    },

    async saveFile() {
        if (this.editorModal.saving) return;
        this.editorModal.saving = true;
        this.editorModal.error = '';
        this.editorModal.saveSuccess = false;
        try {
            await this.api(`/api/instances/${this.selectedInstance.id}/files/content`, {
                method: 'PUT',
                body: {
                    path: this.editorModal.path,
                    content: this.editorModal.content
                }
            });
            this.editorModal.originalContent = this.editorModal.content;
            this.editorModal.saveSuccess = true;
            setTimeout(() => { this.editorModal.saveSuccess = false; }, 2500);
            this.fetchFiles();
        } catch (e) {
            this.editorModal.error = e.message || 'Failed to save file';
        } finally {
            this.editorModal.saving = false;
        }
    },

    closeEditor() {
        if (this.editorModal.content !== this.editorModal.originalContent) {
            if (!confirm('You have unsaved changes. Are you sure you want to close the editor?')) {
                return;
            }
        }
        this.editorModal.open = false;
        this.editorModal.content = '';
        this.editorModal.originalContent = '';
    },

    openCreateModal(type) {
        this.createFileModal = {
            open: true,
            isDir: type === 'dir',
            name: '',
            error: '',
            loading: false
        };
    },

    async createItem() {
        if (!this.createFileModal.name.trim()) return;
        this.createFileModal.loading = true;
        this.createFileModal.error = '';
        const name = this.createFileModal.name.trim();
        const current = this.fileManager.currentPath;
        const targetPath = current ? `${current}/${name}` : name;

        try {
            await this.api(`/api/instances/${this.selectedInstance.id}/files/create`, {
                method: 'POST',
                body: {
                    path: targetPath,
                    is_dir: this.createFileModal.isDir,
                    content: ''
                }
            });
            this.createFileModal.open = false;
            this.fetchFiles();
        } catch (e) {
            this.createFileModal.error = e.message || 'Failed to create item';
        } finally {
            this.createFileModal.loading = false;
        }
    },

    async deleteFile(file) {
        const itemType = file.is_dir || file.isDir ? 'folder' : 'file';
        if (!confirm(`Are you sure you want to delete the ${itemType} '${file.name}'?`)) {
            return;
        }
        try {
            await this.api(`/api/instances/${this.selectedInstance.id}/files/delete`, {
                method: 'POST',
                body: { path: file.path }
            });
            this.fetchFiles();
        } catch (e) {
            alert(`Error deleting file: ${e.message || e}`);
        }
    },

    copyFile(file) {
        this.fileClipboard = {
            path: file.path,
            name: file.name,
            isCut: false
        };
    },

    cutFile(file) {
        this.fileClipboard = {
            path: file.path,
            name: file.name,
            isCut: true
        };
    },

    async pasteFile() {
        if (!this.fileClipboard) return;
        const current = this.fileManager.currentPath;
        const targetPath = current ? `${current}/${this.fileClipboard.name}` : this.fileClipboard.name;
        const endpoint = this.fileClipboard.isCut ? 'move' : 'copy';

        try {
            await this.api(`/api/instances/${this.selectedInstance.id}/files/${endpoint}`, {
                method: 'POST',
                body: {
                    from: this.fileClipboard.path,
                    to: targetPath
                }
            });
            if (this.fileClipboard.isCut) {
                this.fileClipboard = null;
            }
            this.fetchFiles();
        } catch (e) {
            alert(`Failed to paste: ${e.message || e}`);
        }
    },

    triggerUpload() {
        const el = document.getElementById('file-upload-input');
        if (el) el.click();
    },

    async uploadSelectedFiles(event) {
        const files = event.target.files;
        if (!files || !files.length || !this.selectedInstance) return;

        const formData = new FormData();
        for (let i = 0; i < files.length; i++) {
            formData.append('file', files[i]);
        }

        const current = this.fileManager.currentPath || '';
        this.fileManager.loading = true;

        try {
            const token = this.jwtToken;
            const res = await fetch(`/api/instances/${this.selectedInstance.id}/files/upload?path=${encodeURIComponent(current)}`, {
                method: 'POST',
                headers: {
                    'Authorization': `Bearer ${token}`
                },
                body: formData
            });
            if (!res.ok) {
                const text = await res.text();
                throw new Error(text || `Upload failed with status ${res.status}`);
            }
            this.fetchFiles();
        } catch (e) {
            alert(`Upload failed: ${e.message || e}`);
        } finally {
            this.fileManager.loading = false;
            event.target.value = '';
        }
    },

    async toggleBomSync(file) {
        if (!this.selectedInstance) return;
        try {
            const res = await this.api(`/api/instances/${this.selectedInstance.id}/files/sync-toggle`, {
                method: 'POST',
                body: { path: file.path }
            });
            file.isSyncedConfig = res.synced;
            file.is_synced_config = res.synced;
        } catch (e) {
            alert(`Cannot toggle config sync: ${e.message || e}`);
        }
    },

    formatFileDate(secs) {
        if (!secs) return '—';
        const d = new Date(secs * 1000);
        return d.toLocaleDateString() + ' ' + d.toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' });
    },

    openContextMenu(event, file) {
        if (event) {
            event.preventDefault();
            event.stopPropagation();
        }
        const x = Math.min(event.clientX, window.innerWidth - 220);
        const y = Math.min(event.clientY, window.innerHeight - 260);
        this.fileContextMenu = {
            open: true,
            x: Math.max(10, x),
            y: Math.max(10, y),
            file: file
        };
    },

    closeContextMenu() {
        if (this.fileContextMenu && this.fileContextMenu.open) {
            this.fileContextMenu.open = false;
        }
    }
};
