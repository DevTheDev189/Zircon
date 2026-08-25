<template>
  <div class="h-full p-5 overflow-y-auto flex flex-col xl:flex-row gap-6 items-start">
    <!-- Left Column: Launcher Settings & Debug Logs -->
    <div class="w-full xl:w-[500px] 2xl:w-[560px] z-card shrink-0">
      <h3 class="text-white font-bold mb-5">Settings</h3>

      <!-- RAM slider -->
      <div class="mb-6">
        <div class="flex items-center justify-between mb-2">
          <span class="z-label">Max Memory Allocation (RAM)</span>
          <span class="text-sm font-bold text-accent">{{ settings.memoryGb }} GB</span>
        </div>
        <input
          v-model.number="settings.memoryGb"
          type="range"
          min="2"
          max="16"
          step="1"
          class="w-full accent-[#47d2c9]"
        />
        <div class="flex justify-between text-[10px] text-muted">
          <span>2 GB</span><span>16 GB</span>
        </div>
        <p class="z-label mt-1">
          Applied to offline instance launches (replaces the instance's -Xmx). Server launches use the standard 4 GB default.
        </p>
      </div>

      <button class="z-btn-accent" :disabled="saving" @click="save">
        {{ saving ? 'Saving…' : 'Save Settings' }}
      </button>
      <p class="z-label mt-3">{{ savedAt }}</p>

      <!-- Debug logs -->
      <div class="mt-8 pt-6 border-t border-edge">
        <div class="flex items-center justify-between mb-2">
          <h3 class="text-white font-bold">Debug Logs</h3>
          <div class="flex gap-2">
            <button class="z-btn-ghost text-[11px]" @click="copyLogs">Copy to Clipboard</button>
            <button class="z-btn-ghost text-[11px]" @click="clearLogs">Clear Logs</button>
          </div>
        </div>
        <p class="z-label mb-2">
          Recent launcher events (in-memory only — cleared on exit). Useful when reporting issues.
        </p>
        <button class="z-btn-ghost text-[11px] mb-2" @click="refreshLogs">Refresh</button>
        <pre class="bg-black/40 border border-edge rounded-lg p-3 text-[10px] leading-relaxed text-muted h-64 overflow-y-auto whitespace-pre-wrap">{{ logText || 'No log lines captured yet.' }}</pre>
        <p v-if="copiedAt" class="z-label mt-2">{{ copiedAt }}</p>
      </div>
    </div>

    <!-- Right Column: Last Played Minecraft Instance Log -->
    <div class="w-full xl:flex-1 z-card flex flex-col min-w-[360px]">
      <div class="flex items-center justify-between mb-2">
        <div>
          <h3 class="text-white font-bold">Last Played Minecraft Log</h3>
          <p class="text-xs text-muted mt-0.5">
            <span v-if="mcLog" class="inline-flex items-center gap-1.5">
              <span class="text-accent font-semibold">{{ mcLog.instanceName }}</span>
              <span class="bg-slate-800 text-[10px] px-2 py-0.5 rounded text-slate-300 border border-edge uppercase">{{ mcLog.instanceType }}</span>
            </span>
            <span v-else>No log file found from a recently played instance.</span>
          </p>
        </div>
        <div class="flex gap-2">
          <button class="z-btn-ghost text-[11px]" @click="refreshMcLog">Refresh</button>
          <button class="z-btn-ghost text-[11px]" @click="copyMcLogs">Copy to Clipboard</button>
          <button class="z-btn-ghost text-[11px]" @click="clearMcLogs">Clear Log</button>
        </div>
      </div>

      <!-- Filters (Info, Warnings, Errors + Search) -->
      <div class="flex flex-wrap items-center gap-3 mt-3 pt-3 border-t border-edge">
        <input
          v-model="mcSearchQuery"
          type="text"
          placeholder="Filter log output..."
          class="bg-black/40 border border-edge rounded px-3 py-1.5 text-xs text-white placeholder-muted focus:outline-none focus:border-accent flex-1 min-w-[160px]"
        />
        <div class="flex items-center gap-3 text-xs">
          <label class="flex items-center gap-1.5 cursor-pointer text-slate-300 hover:text-white select-none">
            <input type="checkbox" v-model="mcFilters.info" class="accent-[#47d2c9]" /> Info
          </label>
          <label class="flex items-center gap-1.5 cursor-pointer text-yellow-400 hover:text-yellow-300 select-none">
            <input type="checkbox" v-model="mcFilters.warnings" class="accent-yellow-400" /> Warnings
          </label>
          <label class="flex items-center gap-1.5 cursor-pointer text-red-400 hover:text-red-300 select-none">
            <input type="checkbox" v-model="mcFilters.errors" class="accent-red-400" /> Errors
          </label>
          <label class="flex items-center gap-1.5 cursor-pointer text-slate-400 hover:text-slate-200 select-none ml-auto">
            <input type="checkbox" v-model="autoScroll" class="accent-[#47d2c9]" /> Auto-scroll
          </label>
        </div>
      </div>

      <!-- Log Terminal Box -->
      <div
        ref="mcLogBox"
        class="bg-black/40 border border-edge rounded-lg p-3 text-[11px] font-mono leading-relaxed h-[420px] overflow-y-auto whitespace-pre-wrap space-y-0.5 mt-3"
      >
        <template v-if="filteredMcLogLines.length > 0">
          <div
            v-for="(line, idx) in filteredMcLogLines"
            :key="idx"
            :class="mcLogColor(line)"
          >{{ line }}</div>
        </template>
        <p v-else-if="mcLogLines.length > 0" class="text-muted text-xs italic">
          No log lines match current filters.
        </p>
        <p v-else class="text-muted text-xs italic">
          No log output captured yet. Launch a server or offline instance to view logs.
        </p>
      </div>

      <div class="flex items-center justify-between text-[10px] text-muted mt-2 px-1">
        <span>Showing {{ filteredMcLogLines.length }} of {{ mcLogLines.length }} lines</span>
        <span v-if="copiedMcAt" class="text-accent">{{ copiedMcAt }}</span>
      </div>
    </div>
  </div>
</template>

<script setup>
import { computed, nextTick, onMounted, ref } from 'vue';
import { api, onGameOutput } from '../lib/api';

const settings = ref({ memoryGb: 4 });
const saving = ref(false);
const savedAt = ref('');
const logText = ref('');
const copiedAt = ref('');

// Minecraft Instance Log state
const mcLog = ref(null);
const mcLogLines = ref([]);
const mcSearchQuery = ref('');
const mcFilters = ref({ info: true, warnings: true, errors: true });
const autoScroll = ref(true);
const mcLogBox = ref(null);
const copiedMcAt = ref('');

onMounted(async () => {
  try {
    settings.value = await api.getSettings();
  } catch {
    // keep defaults
  }
  refreshLogs();
  refreshMcLog();

  // Listen for live game output when an instance is running
  onGameOutput((line) => {
    mcLogLines.value.push(line);
    if (mcLogLines.value.length > 2000) mcLogLines.value.shift();
    if (autoScroll.value) scrollToBottom();
  });
});

async function refreshLogs() {
  try {
    const lines = await api.getLauncherLogs();
    logText.value = lines.join('\n');
  } catch {
    logText.value = 'Failed to read launcher logs.';
  }
}

async function copyLogs() {
  try {
    await navigator.clipboard.writeText(logText.value || '');
    copiedAt.value = `Copied at ${new Date().toLocaleTimeString()}.`;
  } catch {
    copiedAt.value = 'Copy failed — select and copy manually.';
  }
}

async function clearLogs() {
  try {
    await api.clearLauncherLogs();
    logText.value = '';
    copiedAt.value = '';
  } catch {
    copiedAt.value = 'Failed to clear logs.';
  }
}

async function save() {
  saving.value = true;
  try {
    await api.saveSettings(settings.value);
    savedAt.value = `Saved at ${new Date().toLocaleTimeString()}.`;
  } catch (e) {
    savedAt.value = `Save failed: ${e}`;
  } finally {
    saving.value = false;
  }
}

// Minecraft Instance Log functions
async function refreshMcLog() {
  try {
    const data = await api.getLastInstanceLog();
    if (data) {
      mcLog.value = data;
      mcLogLines.value = data.lines || [];
    } else {
      mcLog.value = null;
      mcLogLines.value = [];
    }
  } catch {
    mcLog.value = null;
    mcLogLines.value = [];
  }
  if (autoScroll.value) scrollToBottom();
}

function scrollToBottom() {
  nextTick(() => {
    if (mcLogBox.value) {
      mcLogBox.value.scrollTop = mcLogBox.value.scrollHeight;
    }
  });
}

const filteredMcLogLines = computed(() => {
  const query = mcSearchQuery.value.trim().toLowerCase();
  return mcLogLines.value.filter((line) => {
    const upper = line.toUpperCase();
    const isError = upper.includes('ERROR') || upper.includes('EXCEPTION') || upper.includes('FATAL') || upper.includes('AT JAVA.');
    const isWarn = !isError && (upper.includes('WARN') || upper.includes('WARNING'));
    const isInfo = !isError && !isWarn;

    if (isError && !mcFilters.value.errors) return false;
    if (isWarn && !mcFilters.value.warnings) return false;
    if (isInfo && !mcFilters.value.info) return false;

    if (query && !line.toLowerCase().includes(query)) return false;

    return true;
  });
});

function mcLogColor(line) {
  const upper = line.toUpperCase();
  if (upper.includes('ERROR') || upper.includes('EXCEPTION') || upper.includes('FATAL')) return 'text-red-400 font-medium';
  if (upper.includes('WARN') || upper.includes('WARNING')) return 'text-yellow-400';
  if (upper.includes('DEBUG') || upper.includes('TRACE')) return 'text-slate-500';
  if (upper.includes('[WRAPPER]') || upper.includes('SUCCESS') || upper.includes('JOINED THE GAME')) return 'text-emerald-400';
  return 'text-slate-300';
}

async function copyMcLogs() {
  try {
    const text = filteredMcLogLines.value.join('\n');
    await navigator.clipboard.writeText(text);
    copiedMcAt.value = `Copied at ${new Date().toLocaleTimeString()}.`;
  } catch {
    copiedMcAt.value = 'Copy failed — select manually.';
  }
}

async function clearMcLogs() {
  try {
    await api.clearLastInstanceLog();
    mcLogLines.value = [];
    copiedMcAt.value = '';
  } catch {
    copiedMcAt.value = 'Failed to clear instance log.';
  }
}
</script>
