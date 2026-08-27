const { createApp } = Vue;

// Zircon admin SPA entry. State lives in data(); the methods are merged
// in from the feature modules in js/ so every method keeps sharing the
// same `this` as before the split. The view comes from `render`, a
// pre-compiled version of the old in-DOM #app template (see
// scripts/build-web-ui.js), so the page needs only Vue's runtime build —
// no template compilation at runtime, which keeps `unsafe-eval` out of
// the Content-Security-Policy.
createApp({
    render: ZirconRender,
    data() {
        return {
            authenticated: false,
            // True while a persisted session is being validated on boot; gates
            // the login overlay vs. dashboard so neither flashes prematurely.
            sessionRestoring: true,
            loginForm: { username: 'admin', password: '' },
            currentUser: { username: 'admin', icon: 'emerald' },
            jwtToken: '',
            instances: [],
            selectedInstance: null,
            activeTab: 'mods',
            showAddServerModal: false,
            newServerForm: { name: '', mcVersion: '1.21.4', loaderType: 'fabric', loaderVersion: '', ramAuto: true, ramGB: 4 },
            showImportServerModal: false,
            importStep: 1,
            importUploading: false,
            importUploadProgress: 0,
            importUploadLoadedText: '',
            importUploadTotalText: '',
            importUploadSpeed: '',
            importStatusMessage: '',
            importLogs: [],
            importError: '',
            importReport: null,
            importForm: { name: '', mcVersion: '1.21.4', loaderType: 'fabric', loaderVersion: '', ramAuto: true, ramGB: 4, convertDimensions: true, externalPort: null },
            showProfileModal: false,
            profileForm: { username: 'admin', currentPassword: '', newPassword: '' },
            systemStats: {},
            searchQuery: '',
            searchType: 'mod', // 'mod' or 'modpack'
            searchProvider: 'modrinth', // 'modrinth' or 'curseforge'
            searchAllVersions: false,
            searching: false,
            searchSeq: 0, // bumped per search so stale responses never clobber newer ones
            searchResults: [],
            installingMods: {}, // { [projectId]: true } while an install is in flight
            curseforgeDropModal: {
                open: false,
                modTitle: '',
                modSlug: '',
                modFileId: null,
                targetFileName: '',
                projectUrl: '',
                iconUrl: '',
                summary: '',
                countdown: 3,
                redirectTriggered: false,
                uploading: false,
                uploadSuccess: false,
                successTitle: '',
                error: '',
                timer: null,
                countdownInterval: null
            },
            isDraggingMod: false,
            installedMods: [],
            // True while the Installed Mods list is being fetched so the tab can
            // show a spinner instead of a jarring empty/offline flash.
            isLoadingMods: false,
            // { [filename]: true } for mods checked in the bulk-action toolbar.
            selectedMods: {},
            // True after an enable/disable change until the admin restarts the
            // server (mod loaders only rescan mods at JVM boot).
            modsRestartNeeded: false,
            shaderpacks: [],
            resourcepacks: [],
            shaderSearchQuery: '',
            texturePackSearchQuery: '',
            shaderSearchProvider: 'modrinth', // 'modrinth' or 'curseforge'
            texturePackSearchProvider: 'modrinth', // 'modrinth' or 'curseforge'
            shaderSearchAllVersions: false,
            texturePackSearchAllVersions: false,
            packSearchType: 'shaderpack', // 'shaderpack' or 'resourcepack'
            packSearching: false,
            shaderSearching: false,
            texturePackSearching: false,
            packSearchResults: [],
            shaderSearchResults: [],
            texturePackSearchResults: [],
            installingPacks: {}, // { [projectId]: true } while an install is in flight
            whitelistEnabled: false,
            whitelistPlayers: [],
            opPlayers: [],
            bannedPlayers: [],
            playerHistory: [],
            playersLoading: false,
            playersLoaded: false,
            playerForms: { whitelist: '', op: '' },
            banForm: { name: '', reason: '' },
            settingsForm: { name: '', mcVersion: '', loaderVersion: '', javaArgs: '', externalPort: null, ramAuto: false, ramGB: 4, extraJvmArgs: '', idleShutdownEnabled: false, idleShutdownMinutes: 5 },
            serverProps: {},
            backupForm: { frequency: 'off', time: '02:00', retention: 10 },
            backupsList: [],
            creatingBackup: false,
            selectedLogBackup: null,
            showEulaModal: false,
            eulaInstance: null,
            showDeleteModal: false,
            // { [instanceId]: true } while a Start action is in flight so the
            // start button can show a spinner and disable instead of appearing
            // unresponsive during a long first-boot install.
            actionLoading: {},
            consoleLines: [],
            command: '',
            consoleWs: null,
            autoScroll: true,
            pollTimer: null,
            idleTicker: null,
            consoleFilters: { info: true, warnings: true, errors: true },
            fileManager: {
                files: [],
                currentPath: '',
                breadcrumbs: [{ name: 'server', path: '' }],
                loading: false,
                error: '',
                searchQuery: ''
            },
            fileClipboard: null,
            createFileModal: {
                open: false,
                isDir: false,
                name: '',
                error: '',
                loading: false
            },
            editorModal: {
                open: false,
                path: '',
                name: '',
                content: '',
                originalContent: '',
                size: 0,
                loading: false,
                saving: false,
                saveSuccess: false,
                error: ''
            },
            fileContextMenu: {
                open: false,
                x: 0,
                y: 0,
                file: null
            },
            branding: {
                hasIcon: false,
                hasBanner: false,
                bannerIsAnimated: false,
                iconUrl: null,
                bannerUrl: null,
                loading: false
            }
        };
    },
    methods: Object.assign({},
        Zircon.core, Zircon.auth, Zircon.instances, Zircon.settings,
        Zircon.mods, Zircon.packs, Zircon.players, Zircon.backups,
        Zircon.files, Zircon.branding, Zircon.console),
    created() {
        // Restore a persisted session (js/auth.js) before the login overlay /
        // dashboard decision is made.
        this.restoreSession();
    },
    computed: {
        // NEW: Client-side filtering logic
        filteredConsoleLines() {
            return this.consoleLines.filter(line => {
                const upper = line.toUpperCase();

                // Categorize the line
                const isError = upper.includes('ERROR') || upper.includes('EXCEPTION') || upper.includes('AT JAVA.');
                const isWarn = !isError && (upper.includes('WARN') || upper.includes('WARNING:'));
                const isInfo = !isError && !isWarn;

                // Return true if its category's checkbox is checked
                if (isError && this.consoleFilters.errors) return true;
                if (isWarn && this.consoleFilters.warnings) return true;
                if (isInfo && this.consoleFilters.info) return true;

                return false;
            });
        },

        // Every property not covered by the curated fields above.
        advancedPropertyKeys() {
            return Object.keys(this.serverProps)
                .filter(k => !CURATED_PROPERTY_KEYS.includes(k))
                .sort();
        },
        scheduleLabel() {
            const freq = this.backupForm.frequency;
            if (!freq || freq === 'off') return 'Manual backups only';
            const labels = { daily: 'Every day', weekly: 'Every week', monthly: 'Every month' };
            return `${labels[freq] || freq} at ${this.backupForm.time || '--:--'}`;
        },
        // Total system RAM (GB) reported by /api/stats; null until first fetch.
        ramTotalGb() {
            const bytes = this.systemStats?.current?.maxMemoryBytes;
            if (!bytes) return null;
            return Math.max(1, Math.floor(bytes / (1024 ** 3)));
        },
        // Slider ceiling: a couple GB of headroom for the OS + Java off-heap,
        // with a sane fallback until the stats endpoint has answered.
        ramSliderMax() {
            const total = this.ramTotalGb;
            return Math.max(4, Math.min(total ? total - 2 : 16, 64));
        },
        selectedModCount() {
            return Object.keys(this.selectedMods).length;
        },
        allModsSelected() {
            return this.installedMods.length > 0
                && this.installedMods.every(m => this.selectedMods[m.filename]);
        }
    },
    watch: {
        activeTab(tab) {
            if (tab === 'console') {
                this.connectConsole();
                // Start at the bottom so the most recent activity is visible
                // immediately (the server replays up to 500 lines on connect).
                this.$nextTick(() => {
                    const box = this.$refs.consoleBox;
                    if (box) box.scrollTop = box.scrollHeight;
                });
            }
            if (tab === 'stats') this.loadStats();
            if (tab === 'players') this.loadPlayers();
            if (tab === 'backups') this.loadBackups();
            if (tab === 'settings') {
                this.loadServerProperties();
                // RAM slider needs the host's total memory for its ceiling.
                this.loadStats();
            }
            if (tab === 'shaders') this.loadShaders();
        }
    }
}).mount('#app');
