<template>
  <div class="relative h-full flex flex-col bg-[#070b0f] text-text">
    <!-- Microsoft login overlay (z-above everything) only visible if auth check is complete and no session exists -->
    <LoginOverlay :visible="!authChecking && !session" @logged-in="onLoggedIn" />
    <LaunchOverlay
      :visible="launchOverlayVisible"
      :status="statusText"
      :progress="progress"
      :running="!!gameStatus?.running"
      :game-label="gameStatus?.label || ''"
      :error="launchError"
      :server="launchingServer"
      :shader-prompt="shaderPrompt"
      @shader-choice="onShaderChoice"
      @close="onLaunchOverlayClose"
    />

    <div class="flex flex-1 min-h-0">
      <!-- Sidebar -->
      <aside
        class="w-[230px] min-w-[230px] flex flex-col border-r border-slate-800/80 bg-[#0a0f14] py-0"
      >
        <div class="px-4 pt-5 pb-5">
          <img
            :src="zirconTitle"
            alt="Zircon"
            class="h-9 w-auto select-none drop-shadow-[0_0_12px_rgba(71,210,201,0.25)]"
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
                ? 'bg-cyan-500/15 text-cyan-300 border border-cyan-500/30 shadow-[0_0_12px_rgba(71,210,201,0.15)] font-bold'
                : 'text-slate-400 hover:text-white hover:bg-slate-800/50 border border-transparent'
            "
            @click="view = item.key"
          >
            <span
              v-if="view === item.key"
              class="absolute left-0 top-1/2 -translate-y-1/2 w-[3px] h-5 rounded-full bg-accent shadow-[0_0_8px_#47d2c9]"
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
            {{ item.label }}
          </button>
        </nav>

        <div class="flex-1"></div>

        <!-- User card -->
        <div class="px-3 pb-4">
          <div
            class="z-card flex items-center gap-3 p-3 transition-all hover:border-accent/40"
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
            </div>
            <button
              v-if="session"
              class="text-[10px] font-semibold text-slate-400 hover:text-[#f87171] transition-colors"
              @click="onLogout"
            >
              Logout
            </button>
          </div>
        </div>
      </aside>

      <!-- Main view -->
      <main
        class="flex-1 min-w-0 flex flex-col bg-gradient-to-br from-[#0e1620] via-[#070b0f] to-[#0a1218]"
      >
        <div class="flex-1 min-h-0 overflow-hidden">
          <KeepAlive>
            <ServersView
              v-if="view === 'servers'"
              :session="session"
              :game-status="gameStatus"
              @launching="onLaunching"
              @stopped="onStopped"
              @error="onLaunchError"
            />
            <OfflineView
              v-else-if="view === 'offline'"
              :session="session"
              @launching="onLaunching"
              @stopped="onStopped"
              @error="onLaunchError"
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
import { onBeforeUnmount, onMounted, ref } from 'vue';
import { getCurrentWindow } from '@tauri-apps/api/window';
import LoginOverlay from './components/LoginOverlay.vue';
import LaunchOverlay from './components/LaunchOverlay.vue';
import StatusBar from './components/StatusBar.vue';
import ServersView from './views/ServersView.vue';
import OfflineView from './views/OfflineView.vue';
import SkinsView from './views/SkinsView.vue';
import SettingsView from './views/SettingsView.vue';
import { api, createDefaultSteveDataUrl, onGameOutput, onGameStatus, onGameWindowReady, onLaunchProgress, onLaunchStatus, onServerKeyMismatch, onShaderRequest, onSkinUpdated, skinFaceDataUrl } from './lib/api';
import { check as checkUpdate } from '@tauri-apps/plugin-updater';
import { relaunch } from '@tauri-apps/plugin-process';
import zirconTitle from './assets/zircon-title.svg';

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
    label: 'Play Offline',
    icon: [
      'M6 12h4',
      'M8 10v4',
      'M15 11h.01',
      'M18 13h.01',
      'M17.32 5H6.68a4 4 0 0 0-3.978 3.59c-.006.052-.01.101-.017.152C2.604 9.416 2 14.456 2 16a3 3 0 0 0 3 3c1 0 1.5-.5 2-1l1.414-1.414A2 2 0 0 1 9.828 16h4.344a2 2 0 0 1 1.414.586L17 18c.5.5 1 1 2 1a3 3 0 0 0 3-3c0-1.545-.604-6.584-.685-7.258-.007-.05-.011-.1-.017-.151A4 4 0 0 0 17.32 5z',
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

const view = ref('servers');
const session = ref(null);
const authChecking = ref(true);
const avatarUrl = ref('');
const statusText = ref('');
const progress = ref(null);
const busy = ref(false);
const launchOverlayVisible = ref(false);
const launchError = ref('');
const gameStatus = ref(null);
const gameOutputBuffer = ref([]);
const shaderPrompt = ref(null);
const shaderRemember = ref(false);
const keyPrompt = ref(null);
const launchingServer = ref(null);

let unlisten = [];

onMounted(async () => {
  // Auth restore (silent refresh when expired) before showing login overlay or opening window.
  try {
    session.value = await api.getCachedSession();
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
  try {
    gameStatus.value = await api.getGameStatus();
    if (gameStatus.value?.running) {
      busy.value = true;
      launchOverlayVisible.value = true;
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
        launchOverlayVisible.value = true;
      } else {
        launchOverlayVisible.value = false;
        busy.value = false;
        progress.value = null;
      }
    }),
    onSkinUpdated(() => {
      refreshAvatar();
    }),
    onGameWindowReady(() => {
      // Game window is open and active — dismiss launch overlay
      launchOverlayVisible.value = false;
      busy.value = false;
    }),
    onShaderRequest((payload) => {
      shaderPrompt.value = payload;
      shaderRemember.value = false;
    }),
    onServerKeyMismatch((payload) => {
      keyPrompt.value = payload;
    })
  );

  // Views surface command errors through this lightweight channel.
  const onStatusEvent = (e) => {
    statusText.value = e.detail;
  };
  window.addEventListener('zircon-status', onStatusEvent);
  unlisten.push(() => window.removeEventListener('zircon-status', onStatusEvent));

  checkLauncherUpdate();
});

// Best-effort launcher self-update: silently checks Cloudflare R2 for a newer
// signed build and relaunches once it's downloaded and installed.
async function checkLauncherUpdate() {
  try {
    api.logDebug('Checking for launcher updates from update feed...');
    const update = await checkUpdate();
    if (update?.available) {
      api.logDebug(`Launcher update available: v${update.version} (current: v${update.currentVersion})`);
      let totalBytes = 0;
      let downloadedBytes = 0;
      statusText.value = `Downloading launcher update ${update.version}...`;
      progress.value = 0;
      await update.downloadAndInstall((event) => {
        if (event.event === 'Started') {
          totalBytes = event.data.contentLength || 0;
          statusText.value = `Downloading launcher update ${update.version}...`;
          api.logDebug(`Launcher update download started (${totalBytes} bytes)`);
        } else if (event.event === 'Progress') {
          downloadedBytes += event.data.chunkLength || 0;
          const percent =
            totalBytes > 0
              ? Math.min(100, Math.round((downloadedBytes / totalBytes) * 100))
              : 0;
          progress.value = percent / 100;
          statusText.value = `Downloading launcher update ${update.version}... ${percent}%`;
        } else if (event.event === 'Finished') {
          progress.value = 1;
          statusText.value = 'Update downloaded. Restarting...';
          api.logDebug('Launcher update downloaded. Restarting application...');
        }
      });
      await relaunch();
    } else {
      api.logDebug('Launcher is on the latest version.');
    }
  } catch (err) {
    statusText.value = '';
    progress.value = null;
    api.logDebug(`Launcher update check failed: ${err}`);
    console.warn('Launcher update check failed:', err);
  }
}

onBeforeUnmount(() => {
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

async function refreshMojangSkin() {
  if (!session.value?.uuid) return;
  try {
    await api.fetchMojangSkinActive(session.value.uuid);
    await refreshAvatar();
  } catch {
    // No custom Mojang skin / network hiccup — keep whatever is active.
  }
}

function onLoggedIn(loggedInSession) {
  session.value = loggedInSession;
  statusText.value = 'Signed in.';
  refreshAvatar();
  refreshMojangSkin();
}

async function onLogout() {
  await api.logout();
  session.value = null;
  avatarUrl.value = '';
  statusText.value = 'Signed out.';
}

async function onShaderChoice({ enabled, remember }) {
  const prompt = shaderPrompt.value;
  if (!prompt) return;
  shaderPrompt.value = null;
  try {
    await api.respondShaderChoice(prompt.requestId, enabled, !!remember);
  } catch {
    // The launch flow falls back to "no shaders" if it never hears back.
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
  launchOverlayVisible.value = false;
  busy.value = false;
  launchError.value = '';
  progress.value = null;
  launchingServer.value = null;
}

function onLaunching(server = null) {
  launchingServer.value = server;
  busy.value = true;
  launchError.value = '';
  launchOverlayVisible.value = true;
  progress.value = null;
}

function onStopped() {
  busy.value = false;
  launchOverlayVisible.value = false;
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
