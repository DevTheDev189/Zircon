<template>
  <div
    v-if="open"
    class="fixed inset-0 z-50 flex items-center justify-center p-4 bg-black/80 backdrop-blur-md transition-all duration-300"
    @click.self="handleClose"
  >
    <div
      class="w-full max-w-xl bg-[#0b0f17]/95 border border-slate-800/90 rounded-2xl shadow-2xl shadow-cyan-950/30 overflow-hidden flex flex-col animate-in fade-in zoom-in-95 duration-200 max-h-[90vh]"
    >
      <!-- Header -->
      <div class="px-6 py-5 border-b border-slate-800/80 flex items-center justify-between bg-gradient-to-r from-slate-900/90 via-slate-900/50 to-cyan-950/20">
        <div class="flex items-center gap-3">
          <div class="w-10 h-10 rounded-xl bg-cyan-500/10 border border-cyan-500/30 flex items-center justify-center text-cyan-400 shadow-inner">
            <svg class="w-5 h-5" fill="none" viewBox="0 0 24 24" stroke="currentColor">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M11 16l-4-4m0 0l4-4m-4 4h14m-5 4v1a3 3 0 01-3 3H6a3 3 0 01-3-3V7a3 3 0 013-3h7a3 3 0 013 3v1" />
            </svg>
          </div>
          <div>
            <h2 class="text-base font-bold text-slate-100 flex items-center gap-2">
              Join via Code
              <span class="text-[10px] font-mono px-2 py-0.5 rounded-full bg-cyan-500/10 text-cyan-400 border border-cyan-500/20">P2P Sync</span>
            </h2>
            <p class="text-xs text-slate-400">Connect to a friend's hosted world with automatic delta sync</p>
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
      <div class="p-6 overflow-y-auto space-y-5 flex-1">
        <!-- Input & Instance Selection Step -->
        <div class="space-y-4">
          <div>
            <label class="block text-xs font-semibold text-slate-300 mb-1.5">
              Enter 6-Character Join Code or Direct Address
            </label>
            <div class="flex gap-2">
              <input
                v-model="joinInput"
                type="text"
                class="z-input flex-1 font-mono uppercase text-sm tracking-widest px-3 py-2 placeholder:normal-case placeholder:tracking-normal placeholder:text-slate-600"
                placeholder="e.g. ZK-7492 or 192.168.1.100:25565"
                :disabled="resolving || syncing"
                @keyup.enter="handleResolve"
              />
              <button
                type="button"
                class="z-btn-primary text-xs px-4 py-2 rounded-xl font-bold flex items-center gap-1.5"
                :disabled="resolving || syncing || !joinInput.trim()"
                @click="handleResolve"
              >
                <span v-if="resolving" class="w-3.5 h-3.5 rounded-full border-2 border-white border-t-transparent animate-spin"></span>
                <span>{{ resolving ? 'Checking...' : 'Check World' }}</span>
              </button>
            </div>
          </div>

          <div>
            <label class="block text-xs font-semibold text-slate-300 mb-1.5">
              Use Instance
            </label>
            <select
              v-model="selectedInstanceId"
              class="z-input w-full text-xs"
              :disabled="syncing"
            >
              <option v-for="inst in instances" :key="inst.id" :value="inst.id">
                {{ inst.name }} (MC {{ inst.minecraftVersion }} - {{ inst.modLoader?.type || 'fabric' }})
              </option>
            </select>
          </div>
        </div>

        <!-- Preflight Results -->
        <div v-if="preflight" class="space-y-4 animate-in fade-in duration-200">
          <!-- Session summary card -->
          <div class="bg-slate-900/60 border border-slate-800/80 rounded-xl p-4 space-y-2.5">
            <div class="flex items-center justify-between text-xs">
              <span class="text-slate-400">Host World:</span>
              <span class="font-bold text-slate-200">{{ preflight.manifest.instanceName }}</span>
            </div>
            <div class="flex items-center justify-between text-xs">
              <span class="text-slate-400">Environment:</span>
              <span class="font-mono text-cyan-300">MC {{ preflight.manifest.mcVersion }} ({{ preflight.manifest.loaderType }})</span>
            </div>
            <div class="flex items-center justify-between text-xs pt-2 border-t border-slate-800/60">
              <span class="text-slate-400">Total Mods on Host:</span>
              <span class="font-semibold text-slate-300">{{ preflight.manifest.mods.length }} mods</span>
            </div>
            <div class="flex items-center justify-between text-xs">
              <span class="text-slate-400">Missing Delta to Stream:</span>
              <span class="font-bold text-cyan-400">
                {{ preflight.missingMods.length }} mods ({{ formatBytes(preflight.totalDownloadBytes) }})
              </span>
            </div>
          </div>

          <!-- Security Warnings for Custom Mods -->
          <div v-if="preflight.customMods.length > 0" class="space-y-3">
            <!-- Mode 1: Blocked by Settings (Default Safe Mode) -->
            <div
              v-if="!preflight.unverifiedAllowedBySettings"
              class="bg-amber-500/10 border border-amber-500/30 rounded-xl p-4 space-y-2 text-xs"
            >
              <div class="flex items-center gap-2 text-amber-300 font-bold">
                <svg class="w-4 h-4 text-amber-400 shrink-0" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                  <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 9v2m0 4h.01m-6.938 4h13.856c1.54 0 2.502-1.667 1.732-3L13.732 4c-.77-1.333-2.694-1.333-3.464 0L3.34 16c-.77 1.333.192 3 1.732 3z" />
                </svg>
                Security Gate: Unverified Custom Mods
              </div>
              <p class="text-slate-300 leading-relaxed">
                The host includes <span class="text-amber-300 font-semibold">{{ preflight.customMods.length }} custom mod(s)</span>
                not cataloged on Modrinth or CurseForge. For your protection against malware, automatic network streaming of unverified JARs is disabled by default.
              </p>
              <ul class="list-disc list-inside text-slate-400 text-[11px] space-y-0.5 pl-1">
                <li v-for="m in preflight.customMods" :key="m.sha1" class="font-mono">
                  {{ m.filename }} ({{ formatBytes(m.fileSize) }})
                </li>
              </ul>
              <p class="text-slate-400 text-[11px] pt-1">
                You can still stream all verified catalog mods, or enable Developer Mode in Settings if you explicitly trust this friend.
              </p>
            </div>

            <!-- Mode 2: Developer Approval Mode (Enabled in Settings) -->
            <div
              v-else
              class="bg-amber-950/30 border border-amber-500/40 rounded-xl p-4 space-y-3 text-xs"
            >
              <div class="flex items-center gap-2 text-amber-300 font-bold">
                <svg class="w-4 h-4 text-amber-400 shrink-0" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                  <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 9v2m0 4h.01m-6.938 4h13.856c1.54 0 2.502-1.667 1.732-3L13.732 4c-.77-1.333-2.694-1.333-3.464 0L3.34 16c-.77 1.333.192 3 1.732 3z" />
                </svg>
                Developer Mode: Custom Mod Approval
              </div>
              <p class="text-slate-300 leading-relaxed">
                The host has included executable mod JARs not verified by Modrinth/CurseForge. Inspect the hashes below:
              </p>
              <div class="space-y-1.5 max-h-32 overflow-y-auto pr-1">
                <div
                  v-for="m in preflight.customMods"
                  :key="m.sha1"
                  class="bg-slate-900/80 p-2 rounded-lg border border-slate-800 text-[11px] space-y-0.5 font-mono"
                >
                  <div class="text-slate-200 font-bold">{{ m.filename }} ({{ formatBytes(m.fileSize) }})</div>
                  <div class="text-slate-500 text-[10px] break-all">SHA-1: {{ m.sha1 }}</div>
                </div>
              </div>
              <label class="flex items-start gap-2 cursor-pointer pt-1">
                <input
                  v-model="approveCustomMods"
                  type="checkbox"
                  class="zircon-check mt-0.5"
                />
                <span class="text-slate-200 font-medium">
                  I trust this friend and approve downloading these {{ preflight.customMods.length }} custom mod(s).
                </span>
              </label>
            </div>
          </div>

          <!-- Progress / Sync Status -->
          <div v-if="syncing" class="bg-slate-900/70 border border-slate-800 rounded-xl p-4 space-y-2">
            <div class="flex items-center justify-between text-xs">
              <span class="text-slate-300 font-medium">{{ syncStatus || 'Streaming mods from friend...' }}</span>
              <span class="text-cyan-400 font-mono font-bold">{{ syncPercent }}%</span>
            </div>
            <div class="w-full bg-slate-800 rounded-full h-2 overflow-hidden">
              <div
                class="bg-gradient-to-r from-cyan-500 to-sky-400 h-full transition-all duration-200 rounded-full"
                :style="{ width: `${syncPercent}%` }"
              ></div>
            </div>
          </div>
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
          :disabled="syncing"
          @click="handleClose"
        >
          Cancel
        </button>

        <div class="flex items-center gap-2">
          <button
            v-if="preflight"
            type="button"
            class="z-btn-primary text-xs px-5 py-2 rounded-xl font-bold flex items-center gap-2 shadow-lg shadow-cyan-500/20 hover:shadow-cyan-500/30 transition-all"
            :disabled="syncing"
            @click="handleSyncAndPlay"
          >
            <span v-if="syncing" class="w-3.5 h-3.5 rounded-full border-2 border-white border-t-transparent animate-spin"></span>
            <span>{{ syncing ? 'Streaming Delta...' : 'Stream Delta & Play' }}</span>
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
  instances: { type: Array, default: () => [] },
  defaultInstanceId: { type: String, default: '' },
});

const emit = defineEmits(['close', 'joined']);

const joinInput = ref('');
const selectedInstanceId = ref('');
const resolving = ref(false);
const syncing = ref(false);
const preflight = ref(null);
const approveCustomMods = ref(false);
const syncStatus = ref('');
const syncPercent = ref(0);
const error = ref('');

watch(
  () => props.open,
  (isOpen) => {
    if (isOpen) {
      error.value = '';
      preflight.value = null;
      syncing.value = false;
      syncPercent.value = 0;
      approveCustomMods.value = false;
      if (props.defaultInstanceId) {
        selectedInstanceId.value = props.defaultInstanceId;
      } else if (props.instances.length > 0) {
        selectedInstanceId.value = props.instances[0].id;
      }
    }
  }
);

function formatBytes(bytes) {
  if (!bytes || bytes <= 0) return '0 B';
  const k = 1024;
  const sizes = ['B', 'KB', 'MB', 'GB'];
  const i = Math.floor(Math.log(bytes) / Math.log(k));
  return `${parseFloat((bytes / Math.pow(k, i)).toFixed(1))} ${sizes[i]}`;
}

async function handleResolve() {
  if (!joinInput.value.trim()) return;
  resolving.value = true;
  error.value = '';
  preflight.value = null;
  try {
    const session = await api.resolveCoopCode(joinInput.value.trim());
    const result = await api.coopPreflight(
      session.host,
      session.p2pPort,
      session.gamePort,
      selectedInstanceId.value || null
    );
    preflight.value = result;
  } catch (err) {
    error.value = `Failed to resolve host: ${err}`;
  } finally {
    resolving.value = false;
  }
}

async function handleSyncAndPlay() {
  if (!preflight.value) return;
  syncing.value = true;
  error.value = '';
  syncPercent.value = 10;
  syncStatus.value = 'Preparing download staging...';

  try {
    const approvedHashes = approveCustomMods.value
      ? preflight.value.customMods.map((m) => m.sha1)
      : [];

    syncStatus.value = 'Streaming missing mods from host...';
    syncPercent.value = 40;

    await api.coopSyncMods(
      preflight.value.hostAddress,
      preflight.value.p2pPort,
      preflight.value.missingMods,
      approvedHashes,
      selectedInstanceId.value || null
    );

    syncPercent.value = 90;
    syncStatus.value = 'Launching Minecraft instance...';

    if (selectedInstanceId.value) {
      await api.launchOfflineInstance(selectedInstanceId.value);
    }

    syncPercent.value = 100;
    emit('joined');
    handleClose();
  } catch (err) {
    error.value = `P2P sync failed: ${err}`;
  } finally {
    syncing.value = false;
  }
}

function handleClose() {
  emit('close');
}
</script>
