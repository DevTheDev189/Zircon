<template>
  <div class="relative h-full flex flex-col bg-bg text-text">
    <!-- Microsoft login overlay (z-above everything) -->
    <LoginOverlay :visible="!session" @logged-in="onLoggedIn" />

    <div class="flex flex-1 min-h-0">
      <!-- Sidebar -->
      <aside
        class="w-[230px] min-w-[230px] flex flex-col border-r border-edge bg-[#0a0f14] py-0"
      >
        <div class="px-4 pt-5 pb-5">
          <img
            :src="zirconTitle"
            alt="Zircon"
            class="h-9 w-auto select-none"
            draggable="false"
          />
          <div class="text-muted text-[9px] tracking-[0.25em] mt-1.5">LAUNCHER</div>
        </div>

        <nav class="px-3 flex flex-col gap-1">
          <button
            v-for="item in navItems"
            :key="item.key"
            class="relative flex items-center gap-2.5 text-left px-3.5 py-2.5 rounded-lg text-[13px] font-semibold transition-colors"
            :class="
              view === item.key
                ? 'bg-[#1c2530] text-white'
                : 'text-muted hover:text-text hover:bg-[#161b22]'
            "
            @click="view = item.key"
          >
            <span
              v-if="view === item.key"
              class="absolute left-0 top-1/2 -translate-y-1/2 w-[3px] h-5 rounded-full bg-accent"
            ></span>
            <svg
              class="w-[17px] h-[17px] shrink-0"
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
            class="flex items-center gap-3 bg-card border border-edge rounded-xl p-3 transition-colors hover:border-[#3d444d]"
          >
            <img
              v-if="avatarUrl"
              :src="avatarUrl"
              class="w-8 h-8 rounded-md image-render-pixel border border-edge"
              alt="avatar"
            />
            <div v-else class="w-8 h-8 rounded-md bg-[#21262d] border border-edge"></div>
            <div class="flex-1 min-w-0">
              <div class="text-xs font-bold text-white truncate">
                {{ session?.username || 'Not signed in' }}
              </div>
            </div>
            <button
              v-if="session"
              class="text-[10px] text-muted hover:text-[#f85149] transition-colors"
              @click="onLogout"
            >
              Logout
            </button>
          </div>
        </div>
      </aside>

      <!-- Main view -->
      <main
        class="flex-1 min-w-0 flex flex-col bg-gradient-to-br from-[#0e151d] via-bg to-[#0b1117]"
      >
        <div class="flex-1 min-h-0 overflow-hidden">
          <ServersView
            v-if="view === 'servers'"
            :session="session"
            :game-status="gameStatus"
            @launching="onLaunching"
            @stopped="onStopped"
          />
          <OfflineView
            v-else-if="view === 'offline'"
            :session="session"
            @launching="onLaunching"
            @stopped="onStopped"
          />
          <SkinsView v-else-if="view === 'skins'" :session="session" />
          <SettingsView v-else />
        </div>
        <StatusBar
          :status="statusText"
          :progress="progress"
          :busy="busy"
        />
      </main>
    </div>

    <!-- Shader opt-in dialog (server offers shaders, choice not remembered yet) -->
    <div
      v-if="shaderPrompt"
      class="absolute inset-0 z-40 bg-black/60 backdrop-blur-sm flex items-center justify-center"
      @click.self="respondShaders(false)"
    >
      <div class="z-card w-[420px] pt-0 overflow-hidden">
        <div
          class="h-[3px] bg-gradient-to-r from-accent to-[#1f8f87] -mx-4 -mt-4 mb-4"
        ></div>
        <h3 class="text-white font-bold mb-1">Enable shaders?</h3>
        <p class="text-muted text-sm mb-1">
          {{ shaderPrompt.server }} offers shaders
          <span v-if="shaderPrompt.shaderName" class="text-muted">
            ({{ shaderPrompt.shaderName }}<span v-if="shaderPrompt.shaderAuthor"> by {{ shaderPrompt.shaderAuthor }}</span>)
          </span>
          .
        </p>
        <p class="text-muted text-xs mb-4">
          Shaders look great but use a lot of GPU — disable them if your PC
          struggles to keep up.
        </p>
        <label class="flex items-center gap-2 text-sm text-text cursor-pointer mb-4 select-none">
          <input v-model="shaderRemember" type="checkbox" class="accent-[#47d2c9]" />
          Remember my choice for this server
        </label>
        <div class="flex justify-end gap-2">
          <button class="z-btn-ghost" @click="respondShaders(false)">No, thanks</button>
          <button class="z-btn-accent" @click="respondShaders(true)">Enable Shaders</button>
        </div>
      </div>
    </div>

    <!-- Host-key rotation dialog (TOFU): the server presents a different
         Ed25519 key than the one pinned on first contact. -->
    <div
      v-if="keyPrompt"
      class="absolute inset-0 z-40 bg-black/60 backdrop-blur-sm flex items-center justify-center"
      @click.self="respondKeyPrompt(false)"
    >
      <div class="z-card w-[460px] pt-0 overflow-hidden">
        <div class="h-[3px] bg-gradient-to-r from-[#f85149] to-[#b62324] -mx-4 -mt-4 mb-4"></div>
        <h3 class="text-white font-bold mb-1">Server identity changed!</h3>
        <p class="text-muted text-sm mb-3">
          <span class="text-[#f85149] font-semibold">{{ keyPrompt.serverAddress }}</span>
          is presenting a <span class="text-white font-semibold">new security key</span>.
          This happens after a server reinstall — or when the server was
          replaced or is being intercepted by an attacker.
        </p>
        <div class="bg-bg border border-edge rounded-lg p-3 mb-4 font-mono text-[11px] leading-relaxed break-all">
          <div class="text-muted mb-0.5">Previous key:</div>
          <div class="text-text">{{ keyPrompt.oldFingerprint }}</div>
          <div class="text-muted mt-2 mb-0.5">New key:</div>
          <div class="text-[#f85149]">{{ keyPrompt.newFingerprint }}</div>
        </div>
        <p class="text-muted text-xs mb-4">
          Only trust the new key if you know the server was legitimately
          reinstalled. Rejecting cancels the launch.
        </p>
        <div class="flex justify-end gap-2">
          <button class="z-btn-ghost" @click="respondKeyPrompt(false)">Reject</button>
          <button class="z-btn-danger" @click="respondKeyPrompt(true)">Trust New Key</button>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup>
import { onBeforeUnmount, onMounted, ref } from 'vue';
import LoginOverlay from './components/LoginOverlay.vue';
import StatusBar from './components/StatusBar.vue';
import ServersView from './views/ServersView.vue';
import OfflineView from './views/OfflineView.vue';
import SkinsView from './views/SkinsView.vue';
import SettingsView from './views/SettingsView.vue';
import { api, onGameOutput, onGameStatus, onLaunchProgress, onLaunchStatus, onServerKeyMismatch, onShaderRequest, onSkinUpdated, skinFaceDataUrl } from './lib/api';
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
const avatarUrl = ref('');
const statusText = ref('');
const progress = ref(null);
const busy = ref(false);
const gameStatus = ref(null);
const gameOutputBuffer = ref([]);
const shaderPrompt = ref(null);
const shaderRemember = ref(false);
const keyPrompt = ref(null);

let unlisten = [];

onMounted(async () => {
  // Auth restore (silent refresh when expired).
  try {
    session.value = await api.getCachedSession();
  } catch {
    session.value = null;
  }
  refreshAvatar();
  refreshMojangSkin();
  try {
    gameStatus.value = await api.getGameStatus();
    if (gameStatus.value?.running) {
      busy.value = true;
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
      } else {
        busy.value = false;
        progress.value = null;
      }
    }),
    onSkinUpdated(() => {
      refreshAvatar();
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
//
// Tauri's `downloadAndInstall` reports *delta* progress: `Started` carries the
// total content length and each `Progress` event carries `chunkLength`, the
// bytes received for that chunk. Progress is accumulated here rather than read
// from a cumulative field (which does not exist and shows NaN / stuck at
// "Downloading...").
async function checkLauncherUpdate() {
  try {
    const update = await checkUpdate();
    if (update?.available) {
      let totalBytes = 0;
      let downloadedBytes = 0;
      statusText.value = `Downloading launcher update ${update.version}...`;
      progress.value = 0;
      await update.downloadAndInstall((event) => {
        if (event.event === 'Started') {
          totalBytes = event.data.contentLength || 0;
          statusText.value = `Downloading launcher update ${update.version}...`;
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
        }
      });
      await relaunch();
    }
  } catch (err) {
    statusText.value = '';
    progress.value = null;
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
  // No custom skin yet — show the first preset's face as a placeholder.
  try {
    const bundled = await api.getBundledSkins();
    const first = bundled[0];
    if (first) {
      avatarUrl.value = (await skinFaceDataUrl(first.dataUrl)) || first.dataUrl;
    }
  } catch {
    avatarUrl.value = '';
  }
}

// Boot / sign-in refresh: pull the player's Minecraft skin so the launcher's
// active skin (and sidebar avatar) mirror it. Best-effort.
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

// Sends the player's shader answer back to the pending launch flow.
async function respondShaders(enabled) {
  const prompt = shaderPrompt.value;
  if (!prompt) return;
  shaderPrompt.value = null;
  try {
    await api.respondShaderChoice(prompt.requestId, enabled, shaderRemember.value);
  } catch {
    // The launch flow falls back to "no shaders" if it never hears back.
  }
}

// Sends the player's host-key decision back to the pending launch flow.
// Accepting re-pins the new key; rejecting (or a closed window) aborts the
// launch — the Rust side never auto-accepts a key change.
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

function onLaunching() {
  busy.value = true;
  progress.value = null;
}

function onStopped() {
  busy.value = false;
  progress.value = null;
}
</script>

<style scoped>
.image-render-pixel {
  image-rendering: pixelated;
}
</style>
