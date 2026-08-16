<template>
  <div class="relative h-full flex flex-col bg-bg text-text">
    <!-- Microsoft login overlay (z-above everything) -->
    <LoginOverlay :visible="!session" @logged-in="onLoggedIn" />

    <div class="flex flex-1 min-h-0">
      <!-- Sidebar -->
      <aside
        class="w-[230px] min-w-[230px] flex flex-col border-r border-[#21262d] py-0"
      >
        <div class="px-4 pt-4 pb-5">
          <span class="text-accent font-bold text-lg tracking-wide">⚡ Zircon</span>
          <span class="text-muted text-xs ml-2">LAUNCHER</span>
        </div>

        <nav class="px-3 flex flex-col gap-1.5">
          <button
            v-for="item in navItems"
            :key="item.key"
            class="text-left px-3.5 py-2.5 rounded-lg text-sm transition-colors"
            :class="
              view === item.key
                ? 'bg-[#21262d] text-white font-bold'
                : 'text-text hover:bg-[#161b22]'
            "
            @click="view = item.key"
          >
            {{ item.label }}
          </button>
        </nav>

        <div class="flex-1"></div>

        <!-- User card -->
        <div class="px-3 pb-4">
          <div class="flex items-center gap-3 bg-card rounded-xl p-3">
            <img
              v-if="avatarUrl"
              :src="avatarUrl"
              class="w-8 h-8 rounded-md image-render-pixel"
              alt="avatar"
            />
            <div v-else class="w-8 h-8 rounded-md bg-[#21262d]"></div>
            <div class="flex-1 min-w-0">
              <div class="text-xs font-bold text-white truncate">
                {{ session?.username || 'Not signed in' }}
              </div>
            </div>
            <button
              v-if="session"
              class="text-[10px] text-muted hover:text-[#f85149]"
              @click="onLogout"
            >
              Logout
            </button>
          </div>
        </div>
      </aside>

      <!-- Main view -->
      <main class="flex-1 min-w-0 flex flex-col">
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
import { api, onGameOutput, onGameStatus, onLaunchProgress, onLaunchStatus } from './lib/api';

const navItems = [
  { key: 'servers', label: '⚡  Servers' },
  { key: 'offline', label: '🎮  Play Offline' },
  { key: 'skins', label: '👕  Skins' },
  { key: 'settings', label: '⚙️  Settings' },
];

const view = ref('servers');
const session = ref(null);
const avatarUrl = ref('');
const statusText = ref('');
const progress = ref(null);
const busy = ref(false);
const gameStatus = ref(null);
const gameOutputBuffer = ref([]);

let unlisten = [];

onMounted(async () => {
  // Auth restore (silent refresh when expired).
  try {
    session.value = await api.getCachedSession();
  } catch {
    session.value = null;
  }
  refreshAvatar();
  try {
    gameStatus.value = await api.getGameStatus();
    if (gameStatus.value?.running) {
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
    })
  );

  // Views surface command errors through this lightweight channel.
  const onStatusEvent = (e) => {
    statusText.value = e.detail;
  };
  window.addEventListener('zircon-status', onStatusEvent);
  unlisten.push(() => window.removeEventListener('zircon-status', onStatusEvent));
});

onBeforeUnmount(() => {
  for (const off of unlisten) off();
});

async function refreshAvatar() {
  try {
    avatarUrl.value = (await api.getSkinHeadIcon()) || '';
  } catch {
    avatarUrl.value = '';
  }
}

function onLoggedIn() {
  refreshAvatar();
  statusText.value = 'Signed in.';
}

async function onLogout() {
  await api.logout();
  session.value = null;
  avatarUrl.value = '';
  statusText.value = 'Signed out.';
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
