<template>
  <div class="h-full flex gap-5 p-5 overflow-hidden">
    <!-- Left: server lists -->
    <div class="flex-1 flex flex-col min-w-0">
      <div class="flex items-center justify-between mb-3">
        <span class="z-section">Your Servers</span>
        <button class="z-btn-ghost text-xs" @click="showAddDialog = true">+ Add Server</button>
      </div>

      <div class="flex-1 min-h-0 overflow-y-auto pr-1">
        <div v-if="servers.length === 0" class="text-muted text-sm py-6 text-center">
          No saved servers yet — add one below.
        </div>
        <div
          v-for="server in servers"
          :key="server.address"
          class="flex items-center gap-3 bg-card border border-edge rounded-lg p-3 mb-2.5"
        >
          <div
            class="w-[30px] h-[30px] rounded-full bg-accent text-[#022c29] font-bold flex items-center justify-center text-sm"
          >
            {{ (server.name || '?').charAt(0).toUpperCase() }}
          </div>
          <div class="flex-1 min-w-0">
            <div class="text-sm font-bold text-white truncate">{{ server.name }}</div>
            <div class="text-[11px] text-muted truncate">{{ server.address }}</div>
          </div>
          <button
            v-if="isLaunching(server.address)"
            class="z-btn-accent flex items-center gap-2"
            disabled
          >
            <span class="inline-block w-3.5 h-3.5 border-2 border-[#022c29] border-t-transparent rounded-full animate-spin"></span>
            LAUNCHING
          </button>
          <button
            v-else-if="gameRunning"
            class="z-btn-ghost"
            :disabled="!isThisServerRunning(server.address)"
            @click="stopGame"
          >
            {{ isThisServerRunning(server.address) ? 'STOP' : 'PLAY' }}
          </button>
          <button v-else class="z-btn-accent" @click="playServer(server)">PLAY</button>
        </div>
      </div>

      <div class="z-section mt-4 mb-3">Recommended Servers</div>
      <div class="shrink-0 overflow-y-auto max-h-[220px] pr-1">
        <div
          v-for="rec in recommended"
          :key="rec.address"
          class="flex items-center gap-3 bg-bg border border-[#21262d] rounded-lg p-2.5 mb-2.5"
        >
          <div
            class="w-[30px] h-[30px] rounded-full bg-accent text-[#022c29] font-bold flex items-center justify-center text-sm"
          >
            {{ rec.name.charAt(0) }}
          </div>
          <div class="flex-1 min-w-0">
            <div class="text-[13px] font-bold text-white truncate">{{ rec.name }}</div>
            <div class="text-[11px] text-muted truncate">{{ rec.desc }} ({{ rec.address }})</div>
          </div>
          <button class="z-btn-ghost text-[11px]" @click="playRecommended(rec)">Add &amp; Play</button>
        </div>
      </div>
    </div>

    <!-- Right: 3D player preview -->
    <div class="w-[420px] min-w-[340px] z-card flex flex-col">
      <span class="z-label mb-2 text-center">3D Player Preview</span>
      <div class="flex-1 min-h-0">
        <Player3DPreview :image-uri="previewSkin" />
      </div>
    </div>

    <!-- Add Server dialog -->
    <div
      v-if="showAddDialog"
      class="absolute inset-0 z-40 bg-black/60 flex items-center justify-center"
      @click.self="showAddDialog = false"
    >
      <div class="z-card w-[400px]">
        <h3 class="text-white font-bold mb-4">Add Server</h3>
        <label class="z-label">Server name</label>
        <input v-model="newServerName" class="z-input mb-3" placeholder="My Server" />
        <label class="z-label">Address (host:port)</label>
        <input
          v-model="newServerAddress"
          class="z-input mb-4"
          placeholder="mc.example.com:25565"
        />
        <div class="flex justify-end gap-2">
          <button class="z-btn-ghost" @click="showAddDialog = false">Cancel</button>
          <button class="z-btn-accent" @click="addServer">Add Server</button>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup>
import { computed, onMounted, ref } from 'vue';
import Player3DPreview from '../components/Player3DPreview.vue';
import { api } from '../lib/api';

const emit = defineEmits(['launching', 'stopped']);

const props = defineProps({
  session: { type: Object, default: null },
  gameStatus: { type: Object, default: null },
});

const servers = ref([]);
const previewSkin = ref(null);
const launchingAddress = ref(null);
const showAddDialog = ref(false);
const newServerName = ref('');
const newServerAddress = ref('');

const recommended = [
  { name: 'Hypixel Network', address: 'mc.hypixel.net', desc: 'Popular Minigames & SkyBlock' },
  { name: 'Wynncraft', address: 'play.wynncraft.net', desc: 'The Minecraft MMORPG' },
  { name: 'Zircon Official', address: 'mc.zircon.example.com:25565', desc: 'Official Mod-Synced Server' },
];

const gameRunning = computed(() => !!props.gameStatus?.running);

function isThisServerRunning(address) {
  return props.gameStatus?.label === address;
}

function isLaunching(address) {
  return launchingAddress.value === address;
}

async function refresh() {
  servers.value = await api.loadSavedServers();
}

async function playServer(server) {
  if (gameRunning.value && isThisServerRunning(server.address)) {
    return stopGame();
  }
  await launch(server.name, server.address);
}

async function playRecommended(rec) {
  const list = await api.loadSavedServers();
  if (!list.some((s) => s.address === rec.address)) {
    list.unshift({ name: rec.name, address: rec.address, lastPlayed: Date.now() });
    await api.saveServerList(list);
  }
  await refresh();
  await launch(rec.name, rec.address);
}

async function launch(name, address) {
  launchingAddress.value = address;
  emit('launching');
  try {
    await api.launchServer(address, name, false);
  } catch (e) {
    console.error('launch failed', e);
    window.dispatchEvent(
      new CustomEvent('zircon-status', { detail: `Error: ${e}` })
    );
  } finally {
    launchingAddress.value = null;
    await refresh();
    emit('stopped');
  }
}

async function stopGame() {
  await api.stopGame();
  emit('stopped');
  await refresh();
}

async function addServer() {
  const address = newServerAddress.value.trim();
  if (!address) return;
  const name = newServerName.value.trim() || address;
  const list = await api.loadSavedServers();
  const existing = list.find((s) => s.address.toLowerCase() === address.toLowerCase());
  if (existing) {
    existing.name = name;
    existing.lastPlayed = Date.now();
  } else {
    list.unshift({ name, address, lastPlayed: Date.now() });
  }
  await api.saveServerList(list);
  showAddDialog.value = false;
  newServerName.value = '';
  newServerAddress.value = '';
  await refresh();
}

onMounted(async () => {
  await refresh();
  // Show the player's active skin (or the bundled default) in the preview.
  try {
    const active = await api.getActiveSkin();
    if (active) {
      previewSkin.value = active.data_url;
      return;
    }
  } catch {
    /* fall through to bundled skin */
  }
  try {
    const bundled = await api.getBundledSkins();
    const steve = bundled.find((s) => s.name === 'steve.png') || bundled[0];
    if (steve) previewSkin.value = steve.data_url;
  } catch {
    previewSkin.value = null;
  }
});
</script>
