<template>
  <div
    v-if="open"
    class="fixed inset-0 z-50 flex items-center justify-center p-4 bg-black/80 backdrop-blur-md transition-all duration-300"
    @click.self="handleClose"
  >
    <div
      class="w-full max-w-lg bg-[#0b0f17]/95 border border-slate-800/90 rounded-2xl shadow-2xl shadow-cyan-950/30 overflow-hidden flex flex-col animate-in fade-in zoom-in-95 duration-200"
    >
      <!-- Header -->
      <div class="px-6 py-5 border-b border-slate-800/80 flex items-center justify-between bg-gradient-to-r from-slate-900/90 via-slate-900/50 to-cyan-950/20">
        <div class="flex items-center gap-3">
          <div class="w-10 h-10 rounded-xl bg-cyan-500/10 border border-cyan-500/30 flex items-center justify-center text-cyan-400 shadow-inner">
            <svg class="w-5 h-5" fill="none" viewBox="0 0 24 24" stroke="currentColor">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M17 20h5v-2a3 3 0 00-5.356-1.857M17 20H7m10 0v-2c0-.656-.126-1.283-.356-1.857M7 20H2v-2a3 3 0 015.356-1.857M7 20v-2c0-.656.126-1.283.356-1.857m0 0a5.002 5.002 0 019.288 0M15 7a3 3 0 11-6 0 3 3 0 016 0zm6 3a2 2 0 11-4 0 2 2 0 014 0zM7 10a2 2 0 11-4 0 2 2 0 014 0z" />
            </svg>
          </div>
          <div>
            <h2 class="text-base font-bold text-slate-100 flex items-center gap-2">
              Host for Friends
              <span class="text-[10px] font-mono px-2 py-0.5 rounded-full bg-cyan-500/10 text-cyan-400 border border-cyan-500/20">P2P Co-Op</span>
            </h2>
            <p class="text-xs text-slate-400">Share your world with friends using a lightweight Join Code</p>
          </div>
        </div>
        <button
          class="text-slate-500 hover:text-slate-300 p-1.5 rounded-lg hover:bg-slate-800/60 transition-colors"
          @click="handleClose"
        >
          <svg class="w-5 h-5" fill="none" viewBox="0 0 24 24" stroke="currentColor">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 18L18 6M6 6l12 12" />
          </svg>
        </button>
      </div>

      <!-- Body -->
      <div class="p-6 space-y-5">
        <!-- Instance & World Info -->
        <div class="bg-slate-900/60 border border-slate-800/80 rounded-xl p-4 space-y-3">
          <div class="flex items-center justify-between text-xs">
            <span class="text-slate-400">Target Instance:</span>
            <span class="font-semibold text-slate-200">{{ instance?.name }}</span>
          </div>

          <div v-if="!activeSession">
            <label class="block text-xs font-semibold text-slate-300 mb-1.5">Select World to Host</label>
            <div v-if="loadingWorlds" class="text-xs text-slate-500 py-2 flex items-center gap-2">
              <span class="w-3 h-3 rounded-full border-2 border-cyan-400 border-t-transparent animate-spin"></span>
              Loading saves...
            </div>
            <select
              v-else-if="worlds.length > 0"
              v-model="selectedWorld"
              class="z-input w-full text-xs"
            >
              <option v-for="w in worlds" :key="w.folderName" :value="w.folderName">
                {{ w.levelName }} ({{ w.folderName }})
              </option>
            </select>
            <p v-else class="text-xs text-amber-400/90 py-1">
              No single-player worlds found in this instance. Launch the instance first to create a world!
            </p>
          </div>

          <div v-else class="flex items-center justify-between text-xs">
            <span class="text-slate-400">Hosted World:</span>
            <span class="font-semibold text-cyan-300">{{ activeSession.worldName }}</span>
          </div>
        </div>

        <!-- Active Session Card -->
        <div v-if="activeSession" class="bg-gradient-to-br from-cyan-950/40 via-slate-900/80 to-slate-900/90 border border-cyan-500/30 rounded-2xl p-5 text-center space-y-4 shadow-lg shadow-cyan-950/20">
          <div class="inline-flex items-center gap-2 px-3 py-1 rounded-full bg-cyan-500/10 border border-cyan-500/30 text-cyan-300 text-xs font-medium">
            <span class="w-2 h-2 rounded-full bg-cyan-400 animate-pulse"></span>
            Hosting Live &amp; P2P Ready
          </div>

          <div>
            <div class="text-[11px] text-slate-400 uppercase tracking-wider font-semibold mb-1">Your 6-Character Join Code</div>
            <div class="flex items-center justify-center gap-3">
              <span class="font-mono text-3xl font-black tracking-widest text-transparent bg-clip-text bg-gradient-to-r from-cyan-400 to-sky-300 select-all">
                {{ activeSession.joinCode }}
              </span>
              <button
                type="button"
                class="p-2 rounded-xl bg-cyan-500/20 hover:bg-cyan-500/30 text-cyan-300 border border-cyan-500/40 transition-all hover:scale-105 active:scale-95"
                title="Copy Join Code"
                @click="copyCode"
              >
                <svg v-if="!copied" class="w-5 h-5" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                  <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M8 16H6a2 2 0 01-2-2V6a2 2 0 012-2h8a2 2 0 012 2v2m-6 12h8a2 2 0 002-2v-8a2 2 0 00-2-2h-8a2 2 0 00-2 2v8a2 2 0 002 2z" />
                </svg>
                <svg v-else class="w-5 h-5 text-emerald-400" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                  <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M5 13l4 4L19 7" />
                </svg>
              </button>
            </div>
            <p v-if="copied" class="text-[11px] text-emerald-400 font-medium mt-1 animate-in fade-in">
              Copied to clipboard! Send this code to your friend.
            </p>
          </div>

          <div class="grid grid-cols-2 gap-2 pt-2 border-t border-slate-800/80 text-[11px]">
            <div class="bg-slate-900/70 p-2 rounded-lg border border-slate-800/60">
              <span class="text-slate-500 block">Game Port</span>
              <span class="font-mono font-bold text-slate-300">{{ activeSession.gamePort }}</span>
            </div>
            <div class="bg-slate-900/70 p-2 rounded-lg border border-slate-800/60">
              <span class="text-slate-500 block">P2P Mod Sync Port</span>
              <span class="font-mono font-bold text-cyan-400">{{ activeSession.p2pPort }}</span>
            </div>
          </div>

          <!-- UPnP Network Status Indicator Card -->
          <div
            v-if="activeSession.upnp"
            class="rounded-xl p-3.5 text-left text-xs transition-all border"
            :class="activeSession.upnp.available && activeSession.upnp.gamePortMapped && activeSession.upnp.p2pPortMapped
              ? 'bg-emerald-950/30 border-emerald-500/40 text-emerald-300'
              : 'bg-amber-950/30 border-amber-500/40 text-amber-300'"
          >
            <div class="flex items-center justify-between mb-1.5">
              <div class="flex items-center gap-2 font-bold text-xs">
                <span
                  class="w-2.5 h-2.5 rounded-full shrink-0"
                  :class="activeSession.upnp.available && activeSession.upnp.gamePortMapped && activeSession.upnp.p2pPortMapped
                    ? 'bg-emerald-400 shadow-[0_0_8px_rgba(52,211,153,0.8)]'
                    : 'bg-amber-400 shadow-[0_0_8px_rgba(251,191,36,0.8)]'"
                ></span>
                <span v-if="activeSession.upnp.available && activeSession.upnp.gamePortMapped && activeSession.upnp.p2pPortMapped">
                  UPnP Active · Zero-Config Ready
                </span>
                <span v-else>
                  UPnP Not Available
                </span>
              </div>
              <span
                class="text-[10px] font-mono px-2 py-0.5 rounded-full border"
                :class="activeSession.upnp.available && activeSession.upnp.gamePortMapped && activeSession.upnp.p2pPortMapped
                  ? 'bg-emerald-500/10 text-emerald-400 border-emerald-500/20'
                  : 'bg-amber-500/10 text-amber-400 border-amber-500/20'"
              >
                {{ (activeSession.upnp.available && activeSession.upnp.gamePortMapped && activeSession.upnp.p2pPortMapped) ? 'Auto-Forwarded' : 'LAN / Manual' }}
              </span>
            </div>

            <p class="text-[11px] leading-relaxed text-slate-300">
              <span v-if="activeSession.upnp.available && activeSession.upnp.gamePortMapped && activeSession.upnp.p2pPortMapped">
                Ports {{ activeSession.gamePort }} &amp; {{ activeSession.p2pPort }} opened automatically on your router. Friends can join seamlessly!
              </span>
              <span v-else>
                UPnP is disabled on this router. Local friends can join via LAN, but internet friends may require manual router port forwarding.
              </span>
            </p>

            <div v-if="activeSession.upnp.externalIp" class="mt-2.5 pt-2 border-t border-slate-800/80 flex items-center justify-between text-[11px]">
              <span class="text-slate-400">Detected External IP:</span>
              <span class="font-mono font-bold text-cyan-300 select-all">{{ activeSession.upnp.externalIp }}</span>
            </div>
          </div>
        </div>

        <!-- Hosting Notice -->
        <div v-else class="text-xs text-slate-400 bg-slate-900/40 border border-slate-800/60 rounded-xl p-3.5 leading-relaxed space-y-1">
          <p class="text-slate-300 font-semibold flex items-center gap-1.5">
            <svg class="w-4 h-4 text-cyan-400" fill="none" viewBox="0 0 24 24" stroke="currentColor">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M13 16h-1v-4h-1m1-4h.01M21 12a9 9 0 11-18 0 9 9 0 0118 0z" />
            </svg>
            How it Works
          </p>
          <p>
            When you click Start Hosting, Zircon creates a temporary session with a direct P2P HTTP server. Friends can enter your Join Code to automatically stream any missing mods and connect directly.
          </p>
        </div>

        <div v-if="error" class="text-xs text-rose-400 bg-rose-500/10 border border-rose-500/30 rounded-xl p-3">
          {{ error }}
        </div>
      </div>

      <!-- Footer Actions -->
      <div class="px-6 py-4 border-t border-slate-800/80 bg-slate-900/40 flex items-center justify-between">
        <button
          type="button"
          class="z-btn-ghost text-xs px-4 py-2 rounded-xl"
          @click="handleClose"
        >
          Close
        </button>

        <div class="flex items-center gap-2">
          <button
            v-if="activeSession"
            type="button"
            class="px-4 py-2 rounded-xl text-xs font-bold text-rose-300 bg-rose-500/10 hover:bg-rose-500/20 border border-rose-500/30 transition-all"
            :disabled="busy"
            @click="stopHosting"
          >
            <span v-if="busy">Stopping...</span>
            <span v-else>Stop Hosting</span>
          </button>

          <button
            v-else
            type="button"
            class="z-btn-primary text-xs px-5 py-2 rounded-xl font-bold flex items-center gap-2 shadow-lg shadow-cyan-500/20 hover:shadow-cyan-500/30 hover:scale-[1.02] active:scale-95 transition-all"
            :disabled="busy || !selectedWorld"
            @click="startHosting"
          >
            <span v-if="busy" class="w-3.5 h-3.5 rounded-full border-2 border-white border-t-transparent animate-spin"></span>
            <span>{{ busy ? 'Starting P2P Server...' : 'Start Hosting' }}</span>
          </button>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup>
import { ref, watch, onMounted } from 'vue';
import { api } from '../lib/api';

const props = defineProps({
  open: { type: Boolean, default: false },
  instance: { type: Object, default: null },
});

const emit = defineEmits(['close']);

const worlds = ref([]);
const selectedWorld = ref('');
const loadingWorlds = ref(false);
const activeSession = ref(null);
const busy = ref(false);
const copied = ref(false);
const error = ref('');

async function loadWorlds() {
  if (!props.instance?.id) return;
  loadingWorlds.value = true;
  error.value = '';
  try {
    const list = await api.listInstanceWorlds(props.instance.id);
    worlds.value = list || [];
    if (worlds.value.length > 0) {
      selectedWorld.value = worlds.value[0].folderName;
    }
  } catch (err) {
    error.value = `Failed to load worlds: ${err}`;
  } finally {
    loadingWorlds.value = false;
  }
}

async function checkActiveSession() {
  try {
    const status = await api.getCoopSessionStatus();
    if (status && status.active && status.instanceId === props.instance?.id) {
      activeSession.value = status;
    } else {
      activeSession.value = null;
    }
  } catch (err) {
    console.warn('Failed to check co-op session:', err);
  }
}

watch(
  () => props.open,
  async (isOpen) => {
    if (isOpen && props.instance) {
      error.value = '';
      await checkActiveSession();
      if (!activeSession.value) {
        await loadWorlds();
      }
    }
  }
);

async function startHosting() {
  if (!props.instance?.id || !selectedWorld.value) return;
  busy.value = true;
  error.value = '';
  try {
    const session = await api.startCoopSession(props.instance.id, selectedWorld.value);
    activeSession.value = session;
  } catch (err) {
    error.value = `Hosting failed: ${err}`;
  } finally {
    busy.value = false;
  }
}

async function stopHosting() {
  busy.value = true;
  error.value = '';
  try {
    await api.stopCoopSession();
    activeSession.value = null;
  } catch (err) {
    error.value = `Failed to stop hosting: ${err}`;
  } finally {
    busy.value = false;
  }
}

async function copyCode() {
  if (!activeSession.value?.joinCode) return;
  try {
    await navigator.clipboard.writeText(activeSession.value.joinCode);
    copied.value = true;
    setTimeout(() => {
      copied.value = false;
    }, 3000);
  } catch (err) {
    console.warn('Clipboard write failed:', err);
  }
}

function handleClose() {
  emit('close');
}
</script>
