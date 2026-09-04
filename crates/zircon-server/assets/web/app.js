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
            newServerForm: { name: '', mcVersion: '1.21.4', loaderType: 'fabric', loaderVersion: '', ramAuto: true, ramGB: 4, autoStart: false },
            minecraftVersions: [],
            minecraftVersionsLoading: false,
            newServerLoaderVersions: [],
            newServerLoaderLoading: false,
            settingsLoaderVersions: [],
            settingsLoaderLoading: false,
            windowsAutostartEnabled: false,
            windowsAutostartSupported: true,
            windowsAutostartLoading: false,
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
            statsSelectedInstanceId: null,
            showStatsDropdown: false,
            chartTooltip: { visible: false, x: 0, y: 0, chartType: '', time: '', systemCpu: 0, processCpu: 0, usedMemory: '', maxMemory: '', tps: '', mspt: '', ping: '' },
            statsPollingActive: true,
            serverCurrentVersion: '0.4.2',
            serverUpdateChecking: false,
            serverUpdateApplying: false,
            serverUpdateAvailable: false,
            serverUpdateManifest: null,
            serverUpdateStatus: '',
            serverUpdateError: '',
            // Crash Analyzer State
            crashAnalysis: null,
            crashAnalysisLoading: false,
            crashFixApplying: false,
            crashFixSuccess: '',
            crashFixError: '',
            showCrashModal: false,
            showSanitizedLog: false,
            copiedSanitizedLog: false,
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
            isDraggingServerMod: false,
            serverModUpload: {
                uploading: false,
                successMessage: '',
                error: ''
            },
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
            serverResourcePack: null,
            serverPackLoading: false,
            serverPackUploading: false,
            whitelistEnabled: false,
            whitelistPlayers: [],
            opPlayers: [],
            bannedPlayers: [],
            playerHistory: [],
            playersLoading: false,
            playersLoaded: false,
            playerForms: { whitelist: '', op: '' },
            banForm: { name: '', reason: '' },
            settingsForm: { name: '', mcVersion: '', loaderVersion: '', javaArgs: '', externalPort: null, ramAuto: false, ramGB: 4, extraJvmArgs: '', idleShutdownEnabled: false, idleShutdownMinutes: 5, autoStart: false },
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
            currentConsoleInstanceId: null,
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
            if (!this.selectedMods) return 0;
            return Object.values(this.selectedMods).filter(Boolean).length;
        },
        allModsSelected() {
            const list = this.installedMods;
            if (!Array.isArray(list) || list.length === 0) return false;
            return list.every(item => Boolean(this.selectedMods?.[item.filename]));
        },
        isStatsInstanceRunning() {
            const inst = this.statsSelectedInstance;
            return inst ? !!inst.running : (this.runningInstancesCount > 0);
        },
        currentTps() {
            if (!this.isStatsInstanceRunning) return 0;
            return this.systemStats?.current?.tps ?? this.systemStats?.tps ?? 0;
        },
        currentMspt() {
            if (!this.isStatsInstanceRunning) return null;
            return this.systemStats?.current?.mspt ?? this.systemStats?.mspt ?? null;
        },
        currentPing() {
            if (!this.isStatsInstanceRunning) return null;
            return this.systemStats?.current?.pingLatencyMs ?? this.systemStats?.pingLatencyMs ?? null;
        },
        tpsHealth() {
            return this.getTpsInfo(this.currentTps, !this.isStatsInstanceRunning);
        },
        msptHealth() {
            return this.getMsptInfo(this.currentMspt);
        },
        cpuHistoryPoints() {
            return (this.systemStats?.history || []).map(p => p.systemCpuLoad || 0);
        },
        processCpuHistoryPoints() {
            return (this.systemStats?.history || []).map(p => p.processCpuLoad || 0);
        },
        ramHistoryPoints() {
            return (this.systemStats?.history || []).map(p => {
                const used = p.usedMemoryBytes || 0;
                const max = p.maxMemoryBytes || 1;
                return Math.min(100, Math.round((used / max) * 100));
            });
        },
        tpsHistoryPoints() {
            if (!this.isStatsInstanceRunning) {
                return new Array(60).fill(0);
            }
            const history = this.systemStats?.tpsHistory;
            if (!history || !history.length) {
                return new Array(60).fill(this.currentTps != null ? this.currentTps : 0);
            }
            return history;
        },
        msptHistoryPoints() {
            if (!this.isStatsInstanceRunning) {
                return new Array(60).fill(0);
            }
            const history = this.systemStats?.msptHistory;
            if (!history || !history.length) {
                return new Array(60).fill(this.currentMspt != null ? this.currentMspt : 0);
            }
            return history;
        },
        statsActiveInstanceName() {
            const id = this.statsSelectedInstanceId || this.systemStats?.activeInstanceId || this.selectedInstance?.id;
            const found = (this.instances || []).find(i => i.id === id);
            return found ? found.name : (this.instances?.[0]?.name || 'Server Instance');
        },
        statsSelectedInstance() {
            const id = this.statsSelectedInstanceId || this.systemStats?.activeInstanceId || this.selectedInstance?.id;
            return (this.instances || []).find(i => i.id === id) || this.instances?.[0] || null;
        },
        runningInstancesCount() {
            return (this.instances || []).filter(i => i.running).length;
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
            if (tab === 'stats') {
                if (this.selectedInstance?.id && !this.statsSelectedInstanceId) {
                    this.statsSelectedInstanceId = this.selectedInstance.id;
                }
                this.loadStats();
                this.loadAutostartStatus();
            }
            if (tab === 'players') this.loadPlayers();
            if (tab === 'backups') this.loadBackups();
            if (tab === 'settings') {
                this.loadMinecraftVersions();
                this.loadServerProperties();
                // RAM slider needs the host's total memory for its ceiling.
                this.loadStats();
                this.checkServerUpdate();
            }
            if (tab === 'shaders') this.loadShaders();
        }
    }
}).mount('#app');
