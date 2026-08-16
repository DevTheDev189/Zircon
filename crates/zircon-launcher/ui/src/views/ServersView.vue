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
          class="flex items-center gap-3 bg-card border border-edge rounded-lg p-3 mb-2.5 transition-colors hover:border-[#3d444d] hover:bg-[#1a2129]"
        >
          <div
            class="w-[30px] h-[30px] rounded-full bg-gradient-to-br from-accent to-[#1f8f87] text-[#032b28] font-bold flex items-center justify-center text-sm shrink-0"
          >
            {{ (server.name || '?').charAt(0).toUpperCase() }}
          </div>
          <div class="flex-1 min-w-0">
            <div class="flex items-center gap-1.5">
              <span
                v-if="isThisServerRunning(server.address)"
                class="w-2 h-2 rounded-full bg-[#3fb950] animate-pulse shrink-0"
                title="Running"
              ></span>
              <div class="text-sm font-bold text-white truncate">{{ server.name }}</div>
            </div>
            <div class="text-[11px] text-muted truncate flex items-center gap-1.5">
              <span class="truncate">{{ server.address }}</span>
              <span
                v-if="statusView(server.address).state === 'checking'"
                class="shrink-0 opacity-70"
                >checking…</span
              >
              <template v-else-if="statusView(server.address).state === 'online'">
                <span class="shrink-0 font-semibold text-[#3fb950]">
                  {{ statusView(server.address).online }}/{{ statusView(server.address).max }}
                </span>
                <span class="shrink-0">{{ statusView(server.address).pingMs }}ms</span>
              </template>
              <span v-else class="shrink-0 text-[#8b949e]">offline</span>
            </div>
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
          <button
            class="text-muted hover:text-[#f85149] transition-colors p-1.5 rounded-md hover:bg-[#f85149]/10 shrink-0"
            :disabled="isLaunching(server.address)"
            :title="isThisServerRunning(server.address)
              ? 'Stop the game before removing this server'
              : 'Remove server and delete its instance folder'"
            @click="removeServer(server)"
          >
            <svg
              class="w-4 h-4"
              viewBox="0 0 24 24"
              fill="none"
              stroke="currentColor"
              stroke-width="1.8"
              stroke-linecap="round"
              stroke-linejoin="round"
            >
              <path d="M3 6h18" />
              <path d="M19 6v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6" />
              <path d="M8 6V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2" />
            </svg>
          </button>
        </div>
      </div>

      <div class="z-section mt-4 mb-3">Recommended Servers</div>
      <div class="shrink-0 overflow-y-auto max-h-[220px] pr-1">
        <div
          v-for="rec in recommended"
          :key="rec.address"
          class="flex items-center gap-3 bg-[#12181f] border border-[#21262d] rounded-lg p-2.5 mb-2.5 transition-colors hover:border-[#3d444d]"
        >
          <div
            class="w-[30px] h-[30px] rounded-full bg-gradient-to-br from-accent to-[#1f8f87] text-[#032b28] font-bold flex items-center justify-center text-sm shrink-0"
          >
            {{ rec.name.charAt(0) }}
          </div>
          <div class="flex-1 min-w-0">
            <div class="text-[13px] font-bold text-white truncate">{{ rec.name }}</div>
            <div class="text-[11px] text-muted truncate flex items-center gap-1.5">
              <span class="truncate">{{ rec.desc }} ({{ rec.address }})</span>
              <span
                v-if="statusView(rec.address).state === 'checking'"
                class="shrink-0 opacity-70"
                >checking…</span
              >
              <template v-else-if="statusView(rec.address).state === 'online'">
                <span class="shrink-0 font-semibold text-[#3fb950]">
                  {{ statusView(rec.address).online }}/{{ statusView(rec.address).max }}
                </span>
                <span class="shrink-0">{{ statusView(rec.address).pingMs }}ms</span>
              </template>
              <span v-else class="shrink-0 text-[#8b949e]">offline</span>
            </div>
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
      class="absolute inset-0 z-40 bg-black/60 backdrop-blur-sm flex items-center justify-center"
      @click.self="showAddDialog = false"
    >
      <div class="z-card w-[400px] pt-0 overflow-hidden">
        <div
          class="h-[3px] bg-gradient-to-r from-accent to-[#1f8f87] -mx-4 -mt-4 mb-4"
        ></div>
        <h3 class="text-white font-bold mb-4">Add Server</h3>
        <label class="z-label">Server name</label>
        <input v-model="newServerName" class="z-input mb-3" placeholder="My Server" />
        <label class="z-label">Address (host:port)</label>
        <input
          v-model="newServerAddress"
          class="z-input mb-4"
          placeholder="mc.example.com:25565"
        />
        <label class="flex items-center gap-2 mb-4 cursor-pointer select-none">
          <input v-model="newServerUseHttps" type="checkbox" class="accent-[#47d2c9]" />
          <span class="text-xs text-muted">
            Use HTTPS for downloads &amp; login (port 443, e.g. behind Caddy/Nginx)
          </span>
        </label>
        <div class="flex justify-end gap-2">
          <button class="z-btn-ghost" @click="showAddDialog = false">Cancel</button>
          <button class="z-btn-accent" @click="addServer">Add Server</button>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup>
import { computed, onBeforeUnmount, onMounted, ref, watch } from 'vue';
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
const newServerUseHttps = ref(false);

// Live status per address: undefined (not checked yet) | 'checking' | null
// (offline) | { online, max, pingMs, version, running }.
const statuses = ref({});
let statusTimer = null;

const recommended = [
  { name: 'Hypixel Network', address: 'mc.hypixel.net', desc: 'Popular Minigames & SkyBlock' },
  { name: 'Wynncraft', address: 'play.wynncraft.net', desc: 'The Minecraft MMORPG' },
  { name: 'Zircon Official', address: 'mc.zircon.example.com:25565', desc: 'Official Mod-Synced Server' },
];

const gameRunning = computed(() => !!props.gameStatus?.running);

// Pings every listed server (saved + recommended) every 30s. Player counts
// come from the wrapper's public status endpoint; latency from a Minecraft
// status ping. Saved servers ping over HTTPS when configured for it.
async function refreshStatuses() {
  const httpsByAddress = new Map(
    servers.value.map((s) => [s.address, !!s.useHttps])
  );
  const addresses = [
    ...servers.value.map((s) => s.address),
    ...recommended.map((r) => r.address),
  ];
  const unique = [...new Set(addresses)];
  for (const addr of unique) {
    if (statuses.value[addr] === undefined) statuses.value[addr] = 'checking';
  }
  await Promise.all(
    unique.map(async (addr) => {
      try {
        statuses.value[addr] = await api.serverStatus(addr, httpsByAddress.get(addr) ?? false);
      } catch {
        statuses.value[addr] = null;
      }
    })
  );
}

// Normalized view of a server's status for the row template.
function statusView(address) {
  const s = statuses.value[address];
  if (s === 'checking') return { state: 'checking' };
  if (!s || s.running === false) return { state: 'offline' };
  return { state: 'online', online: s.online, max: s.max, pingMs: s.pingMs };
}

function isThisServerRunning(address) {
  return props.gameStatus?.running === true && props.gameStatus?.label === address;
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
  await launch(server.name, server.address, server.useHttps);
}

async function playRecommended(rec) {
  const list = await api.loadSavedServers();
  if (!list.some((s) => s.address === rec.address)) {
    list.unshift({
      name: rec.name,
      address: rec.address,
      lastPlayed: Date.now(),
      useHttps: false,
    });
    await api.saveServerList(list);
  }
  await refresh();
  await launch(rec.name, rec.address, false);
}

async function launch(name, address, useHttps) {
  launchingAddress.value = address;
  emit('launching');
  try {
    await api.launchServer(address, name, false, useHttps);
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

async function removeServer(server) {
  const ok = window.confirm(
    `Remove '${server.name}' from your list?\n\n` +
      `This also deletes its local instance folder (mods, configs, packs) ` +
      `for ${server.address}.`
  );
  if (!ok) return;
  try {
    await api.deleteServer(server.address);
    delete statuses.value[server.address];
    await refresh();
  } catch (e) {
    window.dispatchEvent(
      new CustomEvent('zircon-status', { detail: `Remove failed: ${e}` })
    );
  }
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
    existing.useHttps = newServerUseHttps.value;
  } else {
    list.unshift({
      name,
      address,
      lastPlayed: Date.now(),
      useHttps: newServerUseHttps.value,
    });
  }
  await api.saveServerList(list);
  showAddDialog.value = false;
  newServerName.value = '';
  newServerAddress.value = '';
  newServerUseHttps.value = false;
  await refresh();
  refreshStatuses();
}

onMounted(async () => {
  await refresh();
  await refreshPreview();
  await refreshStatuses();
  statusTimer = setInterval(refreshStatuses, 30000);
});

onBeforeUnmount(() => {
  if (statusTimer) clearInterval(statusTimer);
});

// Re-pick the preview skin when the signed-in player changes (e.g. the login
// overlay completes, or the user logs out and back in).
watch(
  () => props.session?.uuid,
  () => refreshPreview()
);

// Preview priority: the player's custom active skin, then their live Mojang
// skin (read-only — never saved), then the bundled default.
async function refreshPreview() {
  try {
    const active = await api.getActiveSkin();
    if (active) {
      previewSkin.value = active.dataUrl;
      return;
    }
  } catch {
    /* fall through to Mojang / bundled */
  }
  if (props.session?.uuid) {
    try {
      const mojang = await api.fetchMojangSkinPreview(props.session.uuid);
      if (mojang) {
        previewSkin.value = mojang.dataUrl;
        return;
      }
    } catch {
      /* no custom Mojang skin — use bundled */
    }
  }
  try {
    const bundled = await api.getBundledSkins();
    const steve = bundled.find((s) => s.name === 'steve.png') || bundled[0];
    if (steve) previewSkin.value = steve.dataUrl;
  } catch {
    previewSkin.value = null;
  }
}
</script>
