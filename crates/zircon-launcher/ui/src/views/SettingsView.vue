<template>
  <div class="h-full p-5 overflow-y-auto flex flex-col xl:flex-row gap-6 items-start">
    <!-- Left Column: Launcher Settings & Debug Logs -->
    <div class="w-full xl:w-[480px] z-card flex flex-col p-6 bg-[#0e1622]/90 border border-slate-800/80 shrink-0">
      <h3 class="text-white font-bold text-base mb-5">Settings</h3>

      <!-- RAM slider -->
      <div class="mb-6">
        <div class="flex items-center justify-between mb-2">
          <span class="z-label font-semibold text-slate-300">Max Memory Allocation (RAM)</span>
          <span class="text-sm font-bold text-cyan-300 font-mono">{{ settings.memoryGb }} GB</span>
        </div>
        <input
          v-model.number="settings.memoryGb"
          type="range"
          min="2"
          max="16"
          step="1"
          class="w-full accent-cyan-400 cursor-pointer"
        />
        <div class="flex justify-between text-[10px] text-slate-500 font-mono mt-1">
          <span>2 GB</span><span>16 GB</span>
        </div>
        <p class="z-label mt-2 text-slate-400">
          Applied to offline instance launches (replaces the instance's -Xmx). Server launches use the standard 4 GB default.
        </p>
      </div>

      <button class="z-btn-accent rounded-xl font-bold px-5 py-2 shadow-md hover:shadow-cyan-500/25" :disabled="saving" @click="save">
        {{ saving ? 'Saving…' : 'Save Settings' }}
      </button>
      <p class="z-label mt-2.5 text-cyan-300/90 font-medium">{{ savedAt }}</p>

      <!-- About & Updates -->
      <div class="mt-8 pt-6 border-t border-slate-800/80">
        <div class="flex items-center justify-between mb-2">
          <div>
            <h3 class="text-white font-bold text-sm">Zircon Launcher</h3>
            <p class="text-xs text-slate-400 mt-0.5">
              Current Version:
              <span class="inline-block bg-cyan-500/15 text-cyan-300 border border-cyan-500/30 px-2 py-0.5 rounded text-[11px] font-mono font-bold ml-1">
                v{{ launcherVersion || '0.3.7' }}
              </span>
            </p>
          </div>
          <button
            class="z-btn-ghost text-[11px] px-3 py-1.5 rounded-lg border border-slate-700/80 hover:border-cyan-400/50 hover:text-cyan-300 flex items-center gap-1.5"
            :disabled="checkingUpdate || updating"
            @click="checkForUpdates"
          >
            <svg v-if="checkingUpdate" class="w-3.5 h-3.5 animate-spin" fill="none" viewBox="0 0 24 24">
              <circle class="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" stroke-width="4"></circle>
              <path class="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4zm2 5.291A7.962 7.962 0 014 12H0c0 3.042 1.135 5.824 3 7.938l3-2.647z"></path>
            </svg>
            <span>{{ checkingUpdate ? 'Checking…' : 'Check for Updates' }}</span>
          </button>
        </div>

        <!-- Update Status Message Box -->
        <div v-if="updateStatusMessage" class="mt-3 p-3 rounded-xl border text-xs" :class="updateStatusClass">
          <div class="font-semibold">{{ updateStatusMessage }}</div>
          <div v-if="updateInfo?.notes" class="text-[11px] text-slate-400 mt-1">{{ updateInfo.notes }}</div>

          <!-- Download / Install progress bar -->
          <div v-if="updating" class="mt-2.5">
            <div class="w-full bg-[#070b10] h-2 rounded-full overflow-hidden border border-slate-800">
              <div class="bg-cyan-400 h-full rounded-full transition-all duration-200" :style="{ width: Math.round(updateProgress * 100) + '%' }"></div>
            </div>
            <p class="text-[10px] text-cyan-300 font-mono mt-1">{{ Math.round(updateProgress * 100) }}% downloaded</p>
          </div>

          <div v-if="updateInfo && !updating" class="mt-3 flex justify-end">
            <button
              class="z-btn-accent text-xs px-4 py-1.5 rounded-lg font-bold shadow-md hover:shadow-cyan-500/25"
              @click="installUpdate"
            >
              Update &amp; Restart
            </button>
          </div>
        </div>
      </div>

      <!-- Debug logs -->
      <div class="mt-8 pt-6 border-t border-slate-800/80">
        <div class="flex items-center justify-between mb-2">
          <h3 class="text-white font-bold text-sm">Debug Logs</h3>
          <div class="flex gap-2">
            <button class="z-btn-ghost text-[11px] px-2.5 py-1 rounded-lg" @click="copyLogs">Copy</button>
            <button class="z-btn-ghost text-[11px] px-2.5 py-1 rounded-lg hover:text-red-400" @click="clearLogs">Clear</button>
          </div>
        </div>
        <p class="z-label mb-3 text-slate-400 text-xs">
          Recent launcher events (in-memory only — cleared on exit). Useful when reporting issues.
        </p>
        <button class="z-btn-ghost text-[11px] mb-3 px-3 py-1 rounded-lg" @click="refreshLogs">Refresh</button>
        <pre class="bg-[#070b10] border border-slate-800/90 rounded-xl p-3.5 text-[10px] font-mono leading-relaxed text-slate-400 h-48 overflow-y-auto whitespace-pre-wrap select-text shadow-inner">{{ logText || 'No log lines captured yet.' }}</pre>
        <p v-if="copiedAt" class="z-label mt-2 text-cyan-300 font-medium">{{ copiedAt }}</p>
      </div>
    </div>

    <!-- Right Column: Last Played Minecraft Instance Log -->
    <div class="w-full xl:flex-1 z-card flex flex-col p-6 bg-[#0e1622]/90 border border-slate-800/80 min-w-[360px]">
      <div class="flex items-center justify-between mb-3">
        <div>
          <h3 class="text-white font-bold text-base">Last Played Minecraft Log</h3>
          <p class="text-xs text-slate-400 mt-0.5">
            <span v-if="mcLog" class="inline-flex items-center gap-1.5">
              <span class="text-cyan-300 font-bold">{{ mcLog.instanceName }}</span>
              <span class="bg-slate-900 text-[10px] px-2 py-0.5 rounded-md text-slate-300 border border-slate-800 uppercase font-mono font-bold">{{ mcLog.instanceType }}</span>
            </span>
            <span v-else>No log file found from a recently played instance.</span>
          </p>
        </div>
        <div class="flex gap-2">
          <button class="z-btn-ghost text-[11px] px-2.5 py-1 rounded-lg" @click="refreshMcLog">Refresh</button>
          <button class="z-btn-ghost text-[11px] px-2.5 py-1 rounded-lg" @click="copyMcLogs">Copy</button>
          <button class="z-btn-ghost text-[11px] px-2.5 py-1 rounded-lg hover:text-red-400" @click="clearMcLogs">Clear</button>
        </div>
      </div>

      <!-- Filters (Info, Warnings, Errors + Search) -->
      <div class="flex flex-wrap items-center gap-3 mt-1 pt-3 border-t border-slate-800/80">
        <input
          v-model="mcSearchQuery"
          type="text"
          placeholder="Filter log output..."
          class="bg-[#070b10] border border-slate-700/80 rounded-xl px-3.5 py-1.5 text-xs text-white placeholder-slate-500 focus:outline-none focus:border-cyan-400 flex-1 min-w-[160px]"
        />
        <div class="flex items-center gap-3 text-xs">
          <label class="flex items-center gap-1.5 cursor-pointer text-slate-300 hover:text-white select-none">
            <input type="checkbox" v-model="mcFilters.info" class="zircon-check" /> Info
          </label>
          <label class="flex items-center gap-1.5 cursor-pointer text-yellow-400 hover:text-yellow-300 select-none">
            <input type="checkbox" v-model="mcFilters.warnings" class="zircon-check" /> Warnings
          </label>
          <label class="flex items-center gap-1.5 cursor-pointer text-red-400 hover:text-red-300 select-none">
            <input type="checkbox" v-model="mcFilters.errors" class="zircon-check" /> Errors
          </label>
          <label class="flex items-center gap-1.5 cursor-pointer text-slate-400 hover:text-slate-200 select-none ml-auto">
            <input type="checkbox" v-model="autoScroll" class="zircon-check" /> Auto-scroll
          </label>
        </div>
      </div>

      <!-- Log Terminal Box -->
      <div
        ref="mcLogBox"
        class="bg-[#070b10] border border-slate-800/90 rounded-xl p-4 text-[11px] font-mono leading-relaxed min-h-[300px] max-h-[440px] overflow-y-auto whitespace-pre-wrap space-y-1 mt-3.5 select-text shadow-inner"
      >
        <template v-if="filteredMcLogLines.length > 0">
          <div
            v-for="(line, idx) in filteredMcLogLines"
            :key="idx"
            :class="mcLogColor(line)"
          >{{ line }}</div>
        </template>
        <p v-else-if="mcLogLines.length > 0" class="text-slate-500 text-xs italic">
          No log lines match current filters.
        </p>
        <p v-else class="text-slate-500 text-xs italic">
          No log output captured yet. Launch a server or offline instance to view logs.
        </p>
      </div>

      <div class="flex items-center justify-between text-[10px] text-slate-500 font-mono mt-2 px-1">
        <span>Showing {{ filteredMcLogLines.length }} of {{ mcLogLines.length }} lines</span>
        <span v-if="copiedMcAt" class="text-cyan-400 font-sans font-medium">{{ copiedMcAt }}</span>
      </div>
    </div>
  </div>
</template>

<script setup>
import { computed, nextTick, onMounted, ref } from 'vue';
import { api, onGameOutput } from '../lib/api';
import { check as checkUpdate } from '@tauri-apps/plugin-updater';
import { relaunch } from '@tauri-apps/plugin-process';

const settings = ref({ memoryGb: 4 });
const saving = ref(false);
const savedAt = ref('');
const logText = ref('');
const copiedAt = ref('');

// Launcher Version & Updates
const launcherVersion = ref('');
const checkingUpdate = ref(false);
const updating = ref(false);
const updateProgress = ref(0);
const updateInfo = ref(null);
const updateStatusMessage = ref('');
const updateStatusClass = ref('');

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
  try {
    launcherVersion.value = await api.getLauncherVersion();
  } catch {
    launcherVersion.value = '0.3.7';
  }
  refreshLogs();
  refreshMcLog();

  onGameOutput((line) => {
    mcLogLines.value.push(line);
    if (autoScroll.value) {
      scrollToBottom();
    }
  });
});

async function checkForUpdates() {
  checkingUpdate.value = true;
  updateStatusMessage.value = '';
  updateInfo.value = null;
  updateProgress.value = 0;
  api.logDebug('Manual launcher update check started...');
  try {
    const update = await checkUpdate();
    if (update?.available) {
      updateInfo.value = update;
      updateStatusMessage.value = `Update available: v${update.version} (current: v${update.currentVersion || launcherVersion.value})`;
      updateStatusClass.value = 'bg-cyan-500/10 border-cyan-500/30 text-cyan-300';
      api.logDebug(`Manual check: update available -> v${update.version}`);
    } else {
      updateStatusMessage.value = `You are on the latest version (v${launcherVersion.value || '0.3.7'}).`;
      updateStatusClass.value = 'bg-emerald-500/10 border-emerald-500/30 text-emerald-300';
      api.logDebug(`Manual check: up to date (v${launcherVersion.value})`);
    }
  } catch (err) {
    updateStatusMessage.value = `Update check error: ${err?.message || err}`;
    updateStatusClass.value = 'bg-red-500/10 border-red-500/30 text-red-300';
    api.logDebug(`Manual check error: ${err?.message || err}`);
  } finally {
    checkingUpdate.value = false;
    refreshLogs();
  }
}

async function installUpdate() {
  if (!updateInfo.value) return;
  updating.value = true;
  updateProgress.value = 0;
  updateStatusMessage.value = `Downloading update v${updateInfo.value.version}...`;
  api.logDebug(`Starting download of v${updateInfo.value.version}...`);
  try {
    let totalBytes = 0;
    let downloadedBytes = 0;
    await updateInfo.value.downloadAndInstall((event) => {
      if (event.event === 'Started') {
        totalBytes = event.data.contentLength || 0;
      } else if (event.event === 'Progress') {
        downloadedBytes += event.data.chunkLength || 0;
        const percent = totalBytes > 0 ? Math.min(100, Math.round((downloadedBytes / totalBytes) * 100)) : 0;
        updateProgress.value = percent / 100;
        updateStatusMessage.value = `Downloading update v${updateInfo.value.version}... ${percent}%`;
      } else if (event.event === 'Finished') {
        updateProgress.value = 1;
        updateStatusMessage.value = 'Update downloaded. Restarting application...';
        api.logDebug('Launcher update downloaded. Restarting application...');
      }
    });
    refreshLogs();
    await relaunch();
  } catch (err) {
    updating.value = false;
    updateStatusMessage.value = `Failed to install update: ${err?.message || err}`;
    updateStatusClass.value = 'bg-red-500/10 border-red-500/30 text-red-300';
    api.logDebug(`Update install error: ${err?.message || err}`);
    refreshLogs();
  }
}

async function save() {
  saving.value = true;
  try {
    await api.saveSettings(settings.value);
    savedAt.value = 'Settings saved.';
    setTimeout(() => {
      savedAt.value = '';
    }, 2500);
  } catch (e) {
    savedAt.value = `Save error: ${e}`;
  } finally {
    saving.value = false;
  }
}

async function refreshLogs() {
  try {
    const logs = await api.getDebugLogs();
    logText.value = logs.join('\n');
  } catch (e) {
    logText.value = `Unable to fetch debug logs: ${e}`;
  }
}

async function copyLogs() {
  if (!logText.value) return;
  try {
    await navigator.clipboard.writeText(logText.value);
    copiedAt.value = 'Copied to clipboard!';
    setTimeout(() => {
      copiedAt.value = '';
    }, 2000);
  } catch {
    copiedAt.value = 'Failed to copy.';
  }
}

async function clearLogs() {
  try {
    await api.clearDebugLogs();
    logText.value = '';
  } catch (e) {
    console.warn('Failed to clear logs:', e);
  }
}

// -------------------------------------------------------------
// Minecraft Instance Log logic
// -------------------------------------------------------------

async function refreshMcLog() {
  try {
    const logInfo = await api.getLastInstanceLog();
    if (logInfo) {
      mcLog.value = logInfo;
      mcLogLines.value = logInfo.lines || [];
      if (autoScroll.value) {
        scrollToBottom();
      }
    } else {
      mcLog.value = null;
      mcLogLines.value = [];
    }
  } catch (err) {
    console.warn('Failed to load last instance log:', err);
  }
}

function scrollToBottom() {
  nextTick(() => {
    if (mcLogBox.value) {
      mcLogBox.value.scrollTop = mcLogBox.value.scrollHeight;
    }
  });
}

const filteredMcLogLines = computed(() => {
  const q = mcSearchQuery.value.toLowerCase().trim();
  return mcLogLines.value.filter((line) => {
    const lower = line.toLowerCase();
    const isErr = lower.includes('error') || lower.includes('exception') || lower.includes('fatal');
    const isWarn = !isErr && (lower.includes('warn') || lower.includes('warning'));
    const isInfo = !isErr && !isWarn;

    if (isErr && !mcFilters.value.errors) return false;
    if (isWarn && !mcFilters.value.warnings) return false;
    if (isInfo && !mcFilters.value.info) return false;

    if (q && !lower.includes(q)) return false;

    return true;
  });
});

function mcLogColor(line) {
  const lower = line.toLowerCase();
  if (lower.includes('error') || lower.includes('exception') || lower.includes('fatal')) {
    return 'text-[#f87171] font-semibold';
  }
  if (lower.includes('warn') || lower.includes('warning')) {
    return 'text-[#fbbf24]';
  }
  if (lower.includes('info')) {
    return 'text-slate-300';
  }
  if (lower.includes('debug')) {
    return 'text-slate-500';
  }
  return 'text-slate-400';
}

async function copyMcLogs() {
  if (!mcLogLines.value.length) return;
  try {
    await navigator.clipboard.writeText(filteredMcLogLines.value.join('\n'));
    copiedMcAt.value = 'Copied to clipboard!';
    setTimeout(() => {
      copiedMcAt.value = '';
    }, 2000);
  } catch {
    copiedMcAt.value = 'Failed to copy.';
  }
}

function clearMcLogs() {
  mcLogLines.value = [];
}
</script>
