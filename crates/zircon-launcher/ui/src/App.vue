<template>
  <div class="relative h-full flex flex-col bg-bg text-text">
    <!-- Microsoft login overlay (visible only when signed out) -->
    <LoginOverlay :visible="!authChecking && !session" @logged-in="onLoggedIn" />
    <!-- First-Login Onboarding Instance Migration Welcome Dialog -->
    <OnboardingWelcomeModal
      :open="showOnboardingWelcomeModal"
      :session="session"
      :avatar-url="avatarUrl"
      :scan-results="onboardingScanResults"
      @review="onReviewOnboardingMigration"
      @skip="showOnboardingWelcomeModal = false"
    />
    <!-- Top-Level Launcher Migration Modal -->
    <LauncherMigrationModal
      :open="showAppMigrationModal"
      :initial-scan-results="onboardingScanResults"
      @close="showAppMigrationModal = false"
      @imported="onAppInstancesMigrated"
    />
    <!-- Active Minecraft launch lifecycle modal -->
    <LaunchOverlay
      :visible="launchModalActive"
      :status="statusText || ''"
      :progress="progress ?? null"
      :running="Boolean(gameStatus?.running)"
      :game-label="gameStatus?.label || ''"
      :error="launchError"
      :server="launchingServer"
      :shader-prompt="shaderPrompt"
      :server-packs-prompt="serverPacksPrompt"
      @shader-choice="onShaderChoice"
      @server-packs-choice="onServerPacksChoice"
      @close="onLaunchOverlayClose"
      @open-skins="onOpenSkinsFromLaunch"
    ></LaunchOverlay>
    <!-- Automatic Crash Diagnostics Modal -->
    <CrashReportModal
      :is-open="showCrashReportModal"
      :crash-data="crashReportData"
      @close="showCrashReportModal = false"
    />
    <!-- First-Launch Zircon End User License Agreement (EULA) Modal -->
    <EulaModal
      :is-open="showEulaModal"
      :can-dismiss="eulaCanDismiss"
      @accept="onEulaAccepted"
      @decline="onEulaDeclined"
      @close="showEulaModal = false"
    />
    <!-- Global Glassmorphic Confirmation Modal -->
    <ConfirmModal />

    <div class="flex flex-1 min-h-0">
      <!-- Sidebar -->
      <aside
        class="w-[230px] min-w-[230px] flex flex-col border-r border-edge bg-sidebar py-0"
      >
        <div class="px-4 pt-5 pb-5">
          <img
            :src="zirconTitle"
            alt="Zircon"
            class="h-9 w-auto select-none drop-shadow-[0_0_12px_var(--color-accent-glow)]"
            draggable="false"
          />
          <div class="text-slate-500 text-[9px] tracking-[0.25em] font-bold mt-1.5 uppercase">LAUNCHER</div>
        </div>

        <nav class="px-3 flex flex-col gap-1.5">
          <button
            v-for="item in navItems"
            :key="item.key"
            class="relative flex items-center gap-2.5 text-left px-3.5 py-2.5 rounded-xl text-[13px] font-semibold transition-all"
            :class="
              view === item.key
                ? 'bg-accent/15 text-accent-bright border border-accent/30 shadow-[0_0_12px_var(--color-accent-glow)] font-bold'
                : 'text-slate-400 hover:text-white hover:bg-slate-800/50 border border-transparent'
            "
            @click="view = item.key"
          >
            <span
              v-if="view === item.key"
              class="absolute left-0 top-1/2 -translate-y-1/2 w-[3px] h-5 rounded-full bg-accent shadow-[0_0_8px_var(--color-accent)]"
            ></span>
            <svg
              class="w-[17px] h-[17px] shrink-0"
              :class="view === item.key ? 'text-accent' : 'text-slate-400'"
              viewBox="0 0 24 24"
              fill="none"
              stroke="currentColor"
              stroke-width="1.8"
              stroke-linecap="round"
              stroke-linejoin="round"
            >
              <path v-for="(d, i) in item.icon" :key="i" :d="d" />
            </svg>
            <span class="flex-1 truncate">{{ $t('nav.' + (item.key === 'offline' ? 'instances' : item.key)) || item.label }}</span>
            <span
              v-if="item.key === 'mission_control' && gameStatus?.running"
              class="px-1.5 py-0.5 rounded-md text-[9px] font-bold bg-emerald-500/20 text-emerald-300 border border-emerald-500/40 animate-pulse font-mono shrink-0 shadow-[0_0_6px_rgba(16,185,129,0.3)]"
            >
              LIVE
            </span>
          </button>
        </nav>

        <div class="flex-1"></div>

        <!-- User card & Account Switcher Popover -->
        <div class="px-3 pb-4 relative user-account-switcher-container">
          <div
            class="z-card flex items-center gap-2.5 p-2.5 transition-all hover:border-accent/40 cursor-pointer"
            @click="session ? (showAccountSwitcher = !showAccountSwitcher) : addAccount()"
            :title="session ? 'Account Switcher' : 'Sign in / Add Account'"
          >
            <img
              v-if="avatarUrl"
              :src="avatarUrl"
              class="w-8 h-8 rounded-lg image-render-pixel border border-slate-700/80 ring-1 ring-accent/30"
              alt="avatar"
            />
            <div v-else class="w-8 h-8 rounded-lg bg-[#1a232f] border border-slate-700"></div>
            <div class="flex-1 min-w-0">
              <div class="text-xs font-bold text-white truncate">
                {{ session?.username || 'Not signed in' }}
              </div>
              <div class="text-[10px] text-accent font-mono flex items-center gap-1">
                <span>Microsoft</span>
                <span v-if="accounts.length > 1" class="text-slate-400">({{ accounts.length }})</span>
              </div>
            </div>
            <svg
              v-if="session"
              class="w-3.5 h-3.5 text-slate-400 transition-transform"
              :class="{ 'rotate-180': showAccountSwitcher }"
              viewBox="0 0 24 24"
              fill="none"
              stroke="currentColor"
              stroke-width="2"
            >
              <polyline points="6 9 12 15 18 9"></polyline>
            </svg>
          </div>

          <!-- Account Switcher Popover (anchored flyout with high z-index) -->
          <div
            v-if="showAccountSwitcher && session"
            class="absolute bottom-2 left-[calc(100%+0.5rem)] w-64 bg-[#0c141f]/95 backdrop-blur-2xl border border-slate-700/80 rounded-2xl shadow-[0_20px_50px_rgba(0,0,0,0.85),0_0_25px_rgba(71,210,201,0.15)] p-2.5 z-[999] origin-bottom-left animate-in fade-in zoom-in-95 duration-150"
          >
            <div class="text-[10px] font-extrabold uppercase tracking-wider text-slate-400 px-2 py-1 flex items-center justify-between">
              <span>Saved Accounts</span>
              <button
                class="text-cyan-400 hover:text-cyan-300 font-bold flex items-center gap-1"
                @click="addAccount"
                title="Sign in with another Microsoft account"
              >
                <span>+ Add</span>
              </button>
            </div>

            <div class="flex flex-col gap-1 max-h-48 overflow-y-auto pr-0.5 mt-1">
              <div
                v-for="acc in accounts"
                :key="acc.uuid"
                class="flex items-center gap-2 p-1.5 rounded-xl hover:bg-slate-800/80 cursor-pointer transition group"
                :class="{ 'bg-slate-800/60 border border-slate-700/80': acc.uuid === session?.uuid }"
                @click="switchAccount(acc.uuid)"
              >
                <div class="w-6 h-6 rounded-md bg-slate-950 border border-slate-800 flex items-center justify-center font-bold text-[10px] text-cyan-300 overflow-hidden shrink-0">
                  {{ acc.username.charAt(0).toUpperCase() }}
                </div>
                <div class="flex-1 min-w-0">
                  <div class="text-xs font-semibold text-white truncate">{{ acc.username }}</div>
                </div>
                <span v-if="acc.uuid === session?.uuid" class="text-cyan-400 text-xs font-bold shrink-0">✓</span>
                <button
                  v-if="accounts.length > 1"
                  class="opacity-0 group-hover:opacity-100 text-slate-400 hover:text-red-400 p-1 text-xs transition"
                  title="Remove account"
                  @click.stop="removeAccount(acc.uuid)"
                >
                  ✕
                </button>
              </div>
            </div>

            <div class="mt-2 pt-2 border-t border-slate-800 flex items-center justify-between px-1">
              <button
                class="text-[11px] font-semibold text-slate-400 hover:text-red-400 transition-colors"
                @click="onLogout"
              >
                Sign out
              </button>
            </div>
          </div>
        </div>
      </aside>


      <!-- Main view with theme-aware gradient and consistent dynamic ambient radial glow -->
      <main
        class="relative flex-1 min-w-0 flex flex-col bg-gradient-to-br from-card/85 via-bg to-well overflow-hidden"
      >
        <!-- Dynamic ambient radial glow layers (top center beam + corner aura, gradually animated) -->
        <div class="z-ambient-glow-top"></div>
        <div class="z-ambient-glow-corner"></div>

        <div class="relative z-10 flex-1 min-h-0 overflow-hidden">
          <KeepAlive>
            <ServersView
              v-if="view === 'servers'"
              :session="session"
              :game-status="gameStatus"
              :deep-link-address="deepLinkAddress"
              :deep-link-server="deepLinkServer"
              @launching="onLaunching"
              @stopped="onStopped"
              @error="onLaunchError"
            />
            <OfflineView
              v-else-if="view === 'offline'"
              :session="session"
              :deep-link-code="deepLinkCode"
              @launching="onLaunching"
              @stopped="onStopped"
              @error="onLaunchError"
            />
            <MissionControlView
              v-else-if="view === 'mission_control'"
              :game-status="gameStatus"
              :session="session"
              @stopped="onStopped"
            />
            <SkinsView v-else-if="view === 'skins'" :session="session" />
            <SettingsView v-else-if="view === 'settings'" />
          </KeepAlive>
        </div>
        <StatusBar
          :status="statusText"
          :progress="progress"
          :busy="busy"
        />
      </main>
    </div>

    <!-- Host-key rotation dialog (TOFU): the server presents a different
         Ed25519 key than the one pinned on first contact. -->
    <div
      v-if="keyPrompt"
      class="absolute inset-0 z-40 bg-[#070b0f]/85 backdrop-blur-md flex items-center justify-center p-4"
      @click.self="respondKeyPrompt(false)"
    >
      <div class="z-card w-full max-w-[480px] p-6 overflow-hidden shadow-2xl relative border border-red-500/40 rounded-2xl bg-[#0e1622]">
        <h3 class="text-white font-bold text-base mb-1 text-red-400">Server identity changed!</h3>
        <p class="text-slate-300 text-sm mb-3">
          <span class="text-red-400 font-semibold">{{ keyPrompt.serverAddress }}</span>
          is presenting a <span class="text-white font-semibold">new security key</span>.
          This happens after a server reinstall — or when the server was
          replaced or is being intercepted.
        </p>
        <div class="bg-[#070b10] border border-slate-800/90 rounded-xl p-3.5 mb-4 font-mono text-[11px] leading-relaxed break-all shadow-inner">
          <div class="text-slate-400 mb-0.5 font-sans font-semibold text-xs">Previous key:</div>
          <div class="text-slate-300">{{ keyPrompt.oldFingerprint }}</div>
          <div class="text-slate-400 mt-2.5 mb-0.5 font-sans font-semibold text-xs">New key:</div>
          <div class="text-red-400 font-bold">{{ keyPrompt.newFingerprint }}</div>
        </div>
        <p class="text-slate-400 text-xs mb-4">
          Only trust the new key if you know the server was legitimately
          reinstalled. Rejecting cancels the launch.
        </p>
        <div class="flex justify-end gap-2.5 pt-4 border-t border-slate-800/80">
          <button class="z-btn-ghost text-xs px-4 py-2 rounded-xl font-semibold border border-slate-700/80 hover:border-slate-600 hover:text-white" @click="respondKeyPrompt(false)">Reject</button>
          <button class="z-btn-danger text-xs font-bold px-5 py-2 rounded-xl shadow-md hover:shadow-red-500/25" @click="respondKeyPrompt(true)">Trust New Key</button>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup>
import { defineAsyncComponent, onBeforeUnmount, onMounted, ref } from 'vue';
import { getCurrentWindow } from '@tauri-apps/api/window';
import LoginOverlay from './components/LoginOverlay.vue';
import LaunchOverlay from './components/LaunchOverlay.vue'; // Modern launch progress overlay
import StatusBar from './components/StatusBar.vue';
import ConfirmModal from './components/ConfirmModal.vue';
import ServersView from './views/ServersView.vue';

// Async lazy-loaded views and modals to keep cold-boot bundle small
const OfflineView = defineAsyncComponent(() => import('./views/OfflineView.vue'));
const SkinsView = defineAsyncComponent(() => import('./views/SkinsView.vue'));
const MissionControlView = defineAsyncComponent(() => import('./views/MissionControlView.vue'));
const SettingsView = defineAsyncComponent(() => import('./views/SettingsView.vue'));
const OnboardingWelcomeModal = defineAsyncComponent(() => import('./components/OnboardingWelcomeModal.vue'));
const LauncherMigrationModal = defineAsyncComponent(() => import('./components/LauncherMigrationModal.vue'));
const CrashReportModal = defineAsyncComponent(() => import('./components/CrashReportModal.vue'));
const EulaModal = defineAsyncComponent(() => import('./components/EulaModal.vue'));

import { api, createDefaultSteveDataUrl, onDeepLinkAddServer, onDeepLinkJoin, onDeepLinkJoinRequest, onGameCrashed, onGameOutput, onGameStatus, onGameWindowReady, onLaunchProgress, onLaunchStatus, onServerKeyMismatch, onShaderRequest, onServerPacksRequest, onSkinUpdated, skinFaceDataUrl } from './lib/api';
import { check as checkUpdate } from '@tauri-apps/plugin-updater';
import { relaunch } from '@tauri-apps/plugin-process';
import { applyTheme } from './lib/theme';
import zirconTitle from './assets/zircon-title.svg';

function openDiscordLink() {
  if (api && typeof api.openBrowserUrl === 'function') {
    api.openBrowserUrl('https://discord.gg/zircon');
  } else {
    window.open('https://discord.gg/zircon', '_blank');
  }
}

function openMatrixLink() {
  if (api && typeof api.openBrowserUrl === 'function') {
    api.openBrowserUrl('https://matrix.to/#/#zircon:chat.winslow.plus');
  } else {
    window.open('https://matrix.to/#/#zircon:chat.winslow.plus', '_blank');
  }
}

const navItems = [
  {
    key: 'servers',
    label: 'Servers',
    icon: [
      'M4 4h16v4H4z',
      'M4 10h16v4H4z',
      'M4 16h16v4H4z',
      'M7 6h.01',
      'M7 12h.01',
      'M7 18h.01',
    ],
  },
  {
    key: 'offline',
    label: 'Instances',
    icon: [
      'M6 12h4',
      'M8 10v4',
      'M15 11h.01',
      'M18 13h.01',
      'M17.32 5H6.68a4 4 0 0 0-3.978 3.59c-.006.052-.01.101-.017.152C2.604 9.416 2 14.456 2 16a3 3 0 0 0 3 3c1 0 1.5-.5 2-1l1.414-1.414A2 2 0 0 1 9.828 16h4.344a2 2 0 0 1 1.414.586L17 18c.5.5 1 1 2 1a3 3 0 0 0 3-3c0-1.545-.604-6.584-.685-7.258-.007-.05-.011-.1-.017-.151A4 4 0 0 0 17.32 5z',
    ],
  },
  {
    key: 'mission_control',
    label: 'Mission Control',
    icon: [
      'M12 2a10 10 0 1 0 10 10A10 10 0 0 0 12 2z',
      'M12 6a6 6 0 1 0 6 6',
      'M12 12l3-3',
    ],
  },
  {
    key: 'skins',
    label: 'Skins',
    icon: [
      'M20.38 3.46 16 2a4 4 0 0 1-8 0L3.62 3.46a2 2 0 0 0-1.34 2.23l.58 3.47a1 1 0 0 0 .99.84H6v10a2 2 0 0 0 2 2h8a2 2 0 0 0 2-2V10h2.15a1 1 0 0 0 .99-.84l.58-3.47a2 2 0 0 0-1.34-2.23z',
    ],
  },
  {
    key: 'settings',
    label: 'Settings',
    icon: [
      'M4 21v-7',
      'M4 10V3',
      'M12 21v-9',
      'M12 8V3',
      'M20 21v-5',
      'M20 12V3',
      'M1 14h6',
      'M9 8h6',
      'M17 16h6',
    ],
  },
];

const queryView = typeof window !== 'undefined' && typeof URLSearchParams !== 'undefined'
  ? new URLSearchParams(window.location.search).get('view')
  : null;
const view = ref(queryView || 'servers');
const session = ref(null);
const authChecking = ref(true);
const avatarUrl = ref('');
const statusText = ref('');
const progress = ref(null);
const busy = ref(false);
const launchModalActive = ref(false);
const launchError = ref('');
const gameStatus = ref(null);
const gameOutputBuffer = ref([]);
const shaderPrompt = ref(null);
const shaderRemember = ref(false);
const serverPacksPrompt = ref(null);
const keyPrompt = ref(null);
const launchingServer = ref(null);
const deepLinkAddress = ref('');
const deepLinkServer = ref(null);
const deepLinkCode = ref('');
const crashReportData = ref(null);
const showCrashReportModal = ref(false);
const showOnboardingWelcomeModal = ref(false);
const showAppMigrationModal = ref(false);
const onboardingScanResults = ref([]);
const showEulaModal = ref(false);
const eulaCanDismiss = ref(false);

let unlisten = [];

onMounted(async () => {
  // Automatically restore active session from keychain/cache on launch
  try {
    const cached = await api.getCachedSession();
    if (cached) {
      session.value = cached;
    }
  } catch (err) {
    console.warn('Failed to restore cached session:', err);
    session.value = null;
  } finally {
    authChecking.value = false;
  }

  // Reveal window once auth check has settled so login overlay doesn't flash preemptively
  try {
    const win = getCurrentWindow();
    await win.show();
    await win.setFocus();
  } catch (err) {
    console.warn('Native window.show() failed, falling back to backend command:', err);
    await api.showMainWindow().catch(() => {});
  }

  api.getActiveSkin().catch(() => {});
  refreshAvatar();
  refreshMojangSkin();
  loadAccounts();
  checkOnboardingMigration();
  
  // First-run EULA check
  const acceptedEula = localStorage.getItem('zircon_eula_accepted');
  if (!acceptedEula) {
    showEulaModal.value = true;
    eulaCanDismiss.value = false;
  }
  window.addEventListener('zircon-open-eula', () => {
    eulaCanDismiss.value = true;
    showEulaModal.value = true;
  });

  window.addEventListener('click', onWindowClick);
  try {

    gameStatus.value = await api.getGameStatus();
    if (gameStatus.value?.running) {
      busy.value = true;
      launchModalActive.value = true;
      statusText.value = `Game running: ${gameStatus.value.label}`;
    }
  } catch {
    gameStatus.value = null;
  }

  // Launch-flow events from Rust.
  unlisten.push(
    onLaunchStatus((msg) => {
      statusText.value = msg;
      progress.value = null;
    }),
    onLaunchProgress(({ fraction, detail }) => {
      progress.value = fraction;
      statusText.value = detail || statusText.value;
    }),
    onGameOutput((line) => {
      gameOutputBuffer.value.push(line);
      if (gameOutputBuffer.value.length > 200) gameOutputBuffer.value.shift();
    }),
    onGameStatus((status) => {
      gameStatus.value = status;
      if (status.running) {
        busy.value = true;
        launchModalActive.value = true;
      } else {
        launchModalActive.value = false;
        busy.value = false;
        progress.value = null;
      }
    }),
    onGameCrashed((payload) => {
      crashReportData.value = payload;
      showCrashReportModal.value = true;
      launchModalActive.value = false;
      busy.value = false;
    }),
    onSkinUpdated(() => {
      refreshAvatar();
    }),
    onGameWindowReady(() => {
      // Game window is open and active — dismiss launch overlay and switch to Mission Control companion
      launchModalActive.value = false;
      busy.value = false;
      view.value = 'mission_control';
    }),
    onShaderRequest((payload) => {
      shaderPrompt.value = payload;
      shaderRemember.value = false;
    }),
    onServerPacksRequest((payload) => {
      serverPacksPrompt.value = payload;
    }),
    onServerKeyMismatch((payload) => {
      keyPrompt.value = payload;
    }),
    onDeepLinkJoin((addr) => {
      view.value = 'servers';
      deepLinkAddress.value = addr;
    }),
    onDeepLinkAddServer((payload) => {
      view.value = 'servers';
      deepLinkServer.value = payload;
      if (payload && payload.address) {
        deepLinkAddress.value = payload.address;
      }
    }),
    onDeepLinkJoinRequest((req) => {
      if (req && req.joinCode) {
        view.value = 'offline';
        deepLinkCode.value = req.joinCode;
      } else if (req && req.serverAddress) {
        view.value = 'servers';
        deepLinkAddress.value = req.serverAddress;
        if (req.serverName) {
          deepLinkServer.value = { name: req.serverName, address: req.serverAddress };
        }
      }
    })
  );

  // Views surface command errors through this lightweight channel.
  const onStatusEvent = (e) => {
    statusText.value = e.detail;
  };
  window.addEventListener('zircon-status', onStatusEvent);
  unlisten.push(() => window.removeEventListener('zircon-status', onStatusEvent));

  const onAccountsUpdated = () => {
    loadAccounts();
  };
  window.addEventListener('accounts-updated', onAccountsUpdated);
  unlisten.push(() => window.removeEventListener('accounts-updated', onAccountsUpdated));

  api.getSettings().then((s) => {
    if (s) {
      applyTheme({
        theme: s.theme,
        customAccent: s.customAccent,
        bgTheme: s.bgTheme,
        customBg: s.customBg,
        customCardBg: s.customCardBg,
        buttonStyle: s.buttonStyle,
        glassEffect: s.glassEffect,
      });
    }
  }).catch(() => {});

  checkLauncherUpdate();
});

// Auto-update handler: checks configured update feed for signed releases
async function checkLauncherUpdate() {
  try {
    api.logDebug('Querying launcher update channel...');
    const releaseUpdate = await checkUpdate();
    if (!releaseUpdate || !releaseUpdate.available) {
      api.logDebug('Launcher is up to date.');
      return;
    }

    const targetVer = releaseUpdate.version;
    api.logDebug(`New launcher version detected: v${targetVer} (active: v${releaseUpdate.currentVersion})`);
    statusText.value = `Downloading update v${targetVer}...`;
    progress.value = 0;

    let totalLength = 0;
    let receivedBytes = 0;

    await releaseUpdate.downloadAndInstall((downloadEvent) => {
      switch (downloadEvent.event) {
        case 'Started': {
          totalLength = downloadEvent.data.contentLength || 0;
          api.logDebug(`Starting payload download (${totalLength} bytes)`);
          break;
        }
        case 'Progress': {
          receivedBytes += downloadEvent.data.chunkLength || 0;
          const ratio = totalLength > 0 ? receivedBytes / totalLength : 0;
          progress.value = Math.min(1, Math.max(0, ratio));
          const pct = Math.round(progress.value * 100);
          statusText.value = `Downloading update v${targetVer}... ${pct}%`;
          break;
        }
        case 'Finished': {
          progress.value = 1;
          statusText.value = 'Update installed. Relaunching...';
          api.logDebug('Launcher update installation complete. Triggering relaunch...');
          break;
        }
      }
    });

    await relaunch();
  } catch (err) {
    statusText.value = '';
    progress.value = null;
    api.logDebug(`Update check or installation error: ${err}`);
    console.error('Update service error:', err);
  }
}

onBeforeUnmount(() => {
  window.removeEventListener('click', onWindowClick);
  for (const off of unlisten) off();
});


async function refreshAvatar() {
  try {
    avatarUrl.value = (await api.getSkinHeadIcon()) || '';
    if (avatarUrl.value) return;
  } catch {
    avatarUrl.value = '';
  }
  try {
    const active = await api.getActiveSkin();
    if (active?.dataUrl) {
      avatarUrl.value = (await skinFaceDataUrl(active.dataUrl)) || '';
      if (avatarUrl.value) return;
    }
  } catch {
    // fallback
  }
  try {
    const def = createDefaultSteveDataUrl();
    avatarUrl.value = (await skinFaceDataUrl(def)) || '';
  } catch {
    avatarUrl.value = '';
  }
}

const showAccountSwitcher = ref(false);
const accounts = ref([]);

async function loadAccounts() {
  try {
    accounts.value = (await api.listAccounts()) || [];
  } catch (err) {
    console.warn('Could not load accounts:', err);
    accounts.value = [];
  }
}

async function switchAccount(uuid) {
  if (uuid === session.value?.uuid) {
    showAccountSwitcher.value = false;
    return;
  }
  try {
    const newSession = await api.switchAccount(uuid);
    session.value = newSession;
    showAccountSwitcher.value = false;
    await refreshAvatar();
    await refreshMojangSkin();
    await loadAccounts();
    statusText.value = `Switched to ${newSession.username}.`;
  } catch (err) {
    console.error('Failed to switch account:', err);
  }
}

async function addAccount() {
  showAccountSwitcher.value = false;
  try {
    const newSession = await api.loginMicrosoft();
    session.value = newSession;
    await refreshAvatar();
    await refreshMojangSkin();
    await loadAccounts();
    statusText.value = `Added ${newSession.username}.`;
  } catch (err) {
    console.error('Login error:', err);
    const msg = String(err).toLowerCase();
    if (msg.includes('does not own minecraft') || (msg.includes('404') && (msg.includes('profile') || msg.includes('minecraft')))) {
      statusText.value = 'Account does not own Minecraft Java Edition. Purchase required.';
      api.openBrowserUrl('https://www.minecraft.net/store/minecraft-java-bedrock-edition-pc').catch(() => {});
    } else {
      statusText.value = `Login failed: ${err}`;
    }
  }
}

async function removeAccount(uuid) {
  try {
    const nextSession = await api.removeAccount(uuid);
    session.value = nextSession;
    await loadAccounts();
    if (nextSession) {
      await refreshAvatar();
      await refreshMojangSkin();
      statusText.value = `Active account is now ${nextSession.username}.`;
    } else {
      avatarUrl.value = '';
      statusText.value = 'Signed out.';
    }
  } catch (err) {
    console.error('Failed to remove account:', err);
  }
}

function onWindowClick(e) {
  if (showAccountSwitcher.value && !e.target.closest('.user-account-switcher-container')) {
    showAccountSwitcher.value = false;
  }
}

async function refreshMojangSkin() {
  if (!session.value?.uuid) return;
  try {
    await api.fetchMojangSkinActive(session.value.uuid);
    await refreshAvatar();
  } catch {
    // No custom Mojang skin / network hiccup — keep whatever is active.
  }
}

async function checkOnboardingMigration() {
  if (!session.value) return;
  if (localStorage.getItem('zircon_migration_onboarding_shown')) return;
  localStorage.setItem('zircon_migration_onboarding_shown', 'true');
  try {
    const results = await api.scanInstalledLaunchers();
    const total = (results || []).reduce(
      (acc, r) => acc + (r.instances?.length || 0),
      0
    );
    if (total > 0) {
      onboardingScanResults.value = results;
      showOnboardingWelcomeModal.value = true;
    }
  } catch (err) {
    console.warn('Background launcher scan error:', err);
  }
}

function onReviewOnboardingMigration() {
  showOnboardingWelcomeModal.value = false;
  showAppMigrationModal.value = true;
}

function onAppInstancesMigrated() {
  showAppMigrationModal.value = false;
  view.value = 'offline';
}

function onLoggedIn(loggedInSession) {
  session.value = loggedInSession;
  statusText.value = 'Signed in.';
  refreshAvatar();
  refreshMojangSkin();
  loadAccounts();
  checkOnboardingMigration();
}

async function onLogout() {
  showAccountSwitcher.value = false;
  await api.logout();
  await loadAccounts();
  if (accounts.value.length > 0) {
    await switchAccount(accounts.value[0].uuid);
  } else {
    session.value = null;
    avatarUrl.value = '';
    statusText.value = 'Signed out.';
  }
}


async function onShaderChoice(payload) {
  const prompt = shaderPrompt.value;
  if (!prompt) return;
  shaderPrompt.value = null;
  const enabled = typeof payload === 'boolean' ? payload : Boolean(payload?.enabled);
  const remember = typeof payload === 'object' && payload !== null ? Boolean(payload?.remember) : false;
  try {
    await api.respondShaderChoice(prompt.requestId, enabled, remember);
  } catch (err) {
    console.error('Failed to respond to shader choice:', err);
  }
}

async function onServerPacksChoice(payload) {
  const prompt = serverPacksPrompt.value;
  serverPacksPrompt.value = null;
  const gameDir = prompt?.gameDir || launchingServer.value?.gameDir;
  if (gameDir) {
    try {
      await api.setServerPackDecisions(gameDir, payload.accepted || [], payload.rejected || []);
    } catch (err) {
      console.error('Failed to set server pack decisions:', err);
    }
  }
}

async function respondKeyPrompt(accepted) {
  const prompt = keyPrompt.value;
  if (!prompt) return;
  keyPrompt.value = null;
  try {
    await api.respondKeyPrompt(prompt.requestId, accepted);
  } catch {
    // The launch flow times out and aborts if it never hears back.
  }
}

function onLaunchOverlayClose() {
  launchModalActive.value = false;
  busy.value = false;
  launchError.value = '';
  progress.value = null;
  launchingServer.value = null;
}

function onOpenSkinsFromLaunch() {
  launchModalActive.value = false;
  view.value = 'skins';
}

function onEulaAccepted() {
  localStorage.setItem('zircon_eula_accepted', 'v1.0.0');
  showEulaModal.value = false;
}

async function onEulaDeclined() {
  try {
    const win = getCurrentWindow();
    await win.close();
  } catch {
    window.close();
  }
}

function onLaunching(server = null) {
  launchingServer.value = server;
  busy.value = true;
  launchError.value = '';
  launchModalActive.value = true;
  progress.value = null;
}

function onStopped() {
  busy.value = false;
  launchModalActive.value = false;
  progress.value = null;
  launchingServer.value = null;
}

function onLaunchError(err) {
  busy.value = false;
  launchError.value = typeof err === 'string' ? err : (err?.message || JSON.stringify(err));
}
</script>

<style scoped>
.image-render-pixel {
  image-rendering: pixelated;
}
</style>
