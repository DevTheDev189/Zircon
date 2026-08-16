const { createApp } = Vue;

// Zircon admin SPA entry. State lives in data(); the methods are merged
// in from the feature modules in js/ so every method keeps sharing the
// same `this` as before the split.
createApp({
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
            newServerForm: { name: '', mcVersion: '1.21.4', loaderType: 'fabric', loaderVersion: '' },
            showProfileModal: false,
            profileForm: { username: 'admin', currentPassword: '', newPassword: '' },
            systemStats: {},
            searchQuery: '',
            searchType: 'mod', // 'mod' or 'modpack'
            searching: false,
            searchSeq: 0, // bumped per search so stale responses never clobber newer ones
            searchResults: [],
            installingMods: {}, // { [projectId]: true } while an install is in flight
            recommendedMods: [
                { projectId: 'sodium', title: 'Sodium', description: 'Modern rendering engine that greatly improves frame rates.', loader: 'fabric' },
                { projectId: 'lithium', title: 'Lithium', description: 'General-purpose optimization for physics, chunk loading and entity ticking.', loader: 'fabric' },
                { projectId: 'ferritecore', title: 'FerriteCore', description: 'Memory usage optimizations for Minecraft.', loader: 'both' },
                { projectId: 'cloth-config', title: 'Cloth Config', description: 'Configuration screen library used by many mods.', loader: 'both' },
                { projectId: 'appleskin', title: 'AppleSkin', description: 'Adds food value information to tooltips and HUD.', loader: 'both' }
            ],
            installedMods: [],
            shaderpacks: [],
            resourcepacks: [],
            shaderSearchQuery: '',
            texturePackSearchQuery: '',
            packSearchType: 'shaderpack', // 'shaderpack' or 'resourcepack' — which panel's results are shown
            packSearching: false,
            packSearchResults: [],
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
            settingsForm: { name: '', mcVersion: '', loaderVersion: '', javaArgs: '', externalPort: null },
            serverProps: {},
            backupForm: { frequency: 'off', time: '02:00', retention: 10 },
            backupsList: [],
            creatingBackup: false,
            selectedLogBackup: null,
            showEulaModal: false,
            eulaInstance: null,
            showDeleteModal: false,
            consoleLines: [],
            command: '',
            consoleWs: null,
            autoScroll: true,
            pollTimer: null,
            consoleFilters: { info: true, warnings: true, errors: true }
        };
    },
    methods: Object.assign({},
        Zircon.core, Zircon.auth, Zircon.instances, Zircon.settings,
        Zircon.mods, Zircon.packs, Zircon.players, Zircon.backups,
        Zircon.console),
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
        // Recommended mods filtered to the selected instance's mod loader.
        filteredRecommendedMods() {
            if (!this.selectedInstance) return [];
            const loader = this.selectedInstance.modLoader.type;
            return this.recommendedMods.filter(r => r.loader === 'both' || r.loader === loader);
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
            if (tab === 'settings') this.loadServerProperties();
            if (tab === 'shaders') this.loadShaders();
        }
    }
}).mount('#app');
