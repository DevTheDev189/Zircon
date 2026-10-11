<template>
  <div
    v-if="open"
    class="fixed inset-0 z-50 flex items-center justify-center p-4 bg-black/80 backdrop-blur-xl transition-all duration-300"
    @click.self="handleClose"
  >
    <div
      class="w-full max-w-3xl bg-[#090e17]/95 border border-slate-700/60 rounded-2xl shadow-[0_0_60px_rgba(0,0,0,0.8),0_0_30px_rgba(71,210,201,0.12)] overflow-hidden flex flex-col animate-in fade-in zoom-in-95 duration-200 max-h-[90vh] relative before:absolute before:inset-x-0 before:top-0 before:h-[2px] before:bg-gradient-to-r before:from-transparent before:via-cyan-400/60 before:to-transparent"
    >
      <!-- Header -->
      <div class="px-6 py-5 border-b border-slate-800/80 flex items-center justify-between bg-gradient-to-r from-slate-900/90 via-slate-900/50 to-cyan-950/20">
        <div class="flex items-center gap-3">
          <div class="w-10 h-10 rounded-xl bg-cyan-500/10 border border-cyan-500/30 flex items-center justify-center text-cyan-400 shadow-[0_0_15px_rgba(71,210,201,0.2)]">
            <svg class="w-5 h-5" fill="none" viewBox="0 0 24 24" stroke="currentColor">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M8 7h12m0 0l-4-4m4 4l-4 4m0 6H4m0 0l4 4m-4-4l4-4" />
            </svg>
          </div>
          <div>
            <h2 class="text-base font-bold text-slate-100 flex items-center gap-2">
              {{ $t('migration.title') }}
              <span class="text-[10px] font-mono px-2 py-0.5 rounded-full bg-cyan-500/15 text-cyan-300 border border-cyan-500/30 shadow-[0_0_8px_rgba(71,210,201,0.2)]">{{ $t('migration.zero_friction') }}</span>
            </h2>
            <p class="text-xs text-slate-400">{{ $t('migration.subtitle') }}</p>
          </div>
        </div>

        <div class="flex items-center gap-2">
          <button
            class="px-2.5 py-1.5 rounded-lg border border-slate-800 bg-[#070b10]/80 hover:bg-[#111c2a] hover:border-cyan-500/40 text-slate-300 hover:text-cyan-300 text-xs font-semibold flex items-center gap-1.5 transition-all shadow-sm"
            :disabled="scanning || isImporting"
            @click="triggerScan"
            :title="$t('migration.rescan_tooltip')"
          >
            <svg class="w-3.5 h-3.5" :class="{ 'animate-spin': scanning }" fill="none" viewBox="0 0 24 24" stroke="currentColor">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M4 4v5h.582m15.356 2A8.001 8.001 0 004.582 9m0 0H9m11 11v-5h-.581m0 0a8.003 8.003 0 01-15.357-2m15.357 2H15" />
            </svg>
            <span>{{ scanning ? $t('migration.scanning') : $t('migration.rescan') }}</span>
          </button>

          <button
            class="text-slate-500 hover:text-slate-300 p-1.5 rounded-lg hover:bg-slate-800/60 transition-colors"
            :disabled="isImporting"
            @click="handleClose"
          >
            <svg class="w-5 h-5" fill="none" viewBox="0 0 24 24" stroke="currentColor">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 18L18 6M6 6l12 12" />
            </svg>
          </button>
        </div>
      </div>

      <!-- Main Body -->
      <div class="p-6 overflow-y-auto space-y-5 flex-1">
        <!-- In-flight Import Progress Overlay -->
        <div v-if="isImporting" class="bg-cyan-950/30 border border-cyan-500/40 rounded-xl p-5 space-y-4 shadow-[0_0_25px_rgba(71,210,201,0.15)]">
          <div class="flex items-center justify-between">
            <div class="flex items-center gap-2.5">
              <span class="inline-block w-4 h-4 border-2 border-cyan-400 border-t-transparent rounded-full animate-spin"></span>
              <span class="text-sm font-bold text-white tracking-wide">
                {{ $t('migration.importing_label', { name: progressEvent?.currentInstanceName || $t('migration.preparing_files') }) }}
              </span>
            </div>
            <span class="text-xs font-mono font-bold text-cyan-300">
              {{ Math.round((progressEvent?.fraction || 0) * 100) }}%
            </span>
          </div>

          <!-- Progress Bar -->
          <div class="w-full bg-slate-900 rounded-full h-2.5 overflow-hidden border border-slate-800/80">
            <div
              class="bg-gradient-to-r from-cyan-500 via-teal-400 to-cyan-300 h-full transition-all duration-300 shadow-[0_0_14px_rgba(71,210,201,0.6)]"
              :style="{ width: `${Math.round((progressEvent?.fraction || 0) * 100)}%` }"
            ></div>
          </div>

          <div class="flex items-center justify-between text-[11px] text-slate-400 font-mono">
            <span>{{ progressEvent?.currentFile || $t('migration.transferring') }}</span>
            <span>{{ $t('migration.instance_progress', { current: progressEvent?.completedInstances || 0, total: progressEvent?.totalInstances || selectedInstances.length }) }}</span>
          </div>
        </div>

        <!-- Scanning Indicator -->
        <div v-else-if="scanning" class="py-14 flex flex-col items-center justify-center space-y-3">
          <div class="w-8 h-8 border-2 border-cyan-400 border-t-transparent rounded-full animate-spin shadow-[0_0_12px_rgba(71,210,201,0.3)]"></div>
          <p class="text-xs text-slate-400 font-mono">{{ $t('migration.scanning_system') }}</p>
        </div>

        <!-- Discovered Instance List -->
        <div v-else>
          <!-- Launcher Tabs / Filters -->
          <div class="flex items-center gap-1.5 flex-wrap pb-3 border-b border-slate-800/80 mb-4">
            <button
              class="px-3 py-1.5 rounded-lg text-xs font-bold transition-all flex items-center gap-1.5 shrink-0"
              :class="activeFilter === 'all' ? 'bg-cyan-500/20 text-cyan-300 border border-cyan-500/50 shadow-[0_0_10px_rgba(71,210,201,0.2)]' : 'bg-[#070b10]/70 text-slate-400 border border-slate-800 hover:text-slate-200 hover:border-slate-700'"
              @click="activeFilter = 'all'"
            >
              <span>{{ $t('migration.all_launchers') }}</span>
              <span class="text-[10px] px-1.5 py-0.5 rounded-full bg-slate-800/90 font-mono text-slate-300">{{ totalDiscoveredCount }}</span>
            </button>

            <button
              v-for="res in scanResults"
              :key="res.launcherType"
              class="px-3 py-1.5 rounded-lg text-xs font-bold transition-all flex items-center gap-1.5 shrink-0"
              :class="activeFilter === res.launcherType ? 'bg-cyan-500/20 text-cyan-300 border border-cyan-500/50 shadow-[0_0_10px_rgba(71,210,201,0.2)]' : 'bg-[#070b10]/70 text-slate-400 border border-slate-800 hover:text-slate-200 hover:border-slate-700'"
              @click="activeFilter = res.launcherType"
            >
              <img :src="getLauncherLogo(res.launcherType)" class="w-3.5 h-3.5 object-contain shrink-0" alt="" />
              <span>{{ getLauncherShortName(res.launcherType) }}</span>
              <span
                class="text-[10px] px-1.5 py-0.5 rounded-full font-mono"
                :class="res.instances.length > 0 ? 'bg-cyan-950 text-cyan-400 border border-cyan-800/80' : 'bg-slate-800/60 text-slate-500'"
              >
                {{ res.instances.length }}
              </span>
            </button>
          </div>

          <!-- Empty State -->
          <div v-if="filteredInstances.length === 0" class="py-12 text-center space-y-3">
            <div class="w-12 h-12 mx-auto rounded-xl bg-slate-900/80 border border-slate-800 flex items-center justify-center text-slate-500 shadow-inner">
              <svg class="w-6 h-6" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="1.5" d="M20 13V6a2 2 0 00-2-2H6a2 2 0 00-2 2v7m16 0v5a2 2 0 01-2 2H6a2 2 0 01-2-2v-5m16 0h-2.586a1 1 0 00-.707.293l-2.414 2.414a1 1 0 01-.707.293h-3.172a1 1 0 01-.707-.293l-2.414-2.414A1 1 0 006.586 13H4" />
              </svg>
            </div>
            <div class="text-sm font-bold text-slate-300">{{ $t('migration.empty_title') }}</div>
            <p class="text-xs text-slate-500 max-w-md mx-auto leading-relaxed">
              {{ $t('migration.empty_desc') }}
            </p>
          </div>

          <!-- Items Selection Toolbar -->
          <div v-else class="space-y-3">
            <div class="flex items-center justify-between text-xs text-slate-400 px-1">
              <button
                class="flex items-center gap-2 group cursor-pointer text-slate-300 hover:text-cyan-300 transition-colors"
                @click="toggleSelectAllFiltered"
              >
                <div
                  class="w-4 h-4 rounded border flex items-center justify-center transition-all"
                  :class="areAllFilteredSelected ? 'bg-cyan-500 border-cyan-400 text-slate-950 shadow-[0_0_8px_rgba(71,210,201,0.5)]' : 'border-slate-700 bg-slate-900 group-hover:border-cyan-500/60'"
                >
                  <svg v-if="areAllFilteredSelected" class="w-3 h-3 fill-current" viewBox="0 0 24 24">
                    <path d="M9 16.17L4.83 12l-1.42 1.41L9 19 21 7l-1.41-1.41z"/>
                  </svg>
                </div>
                <span class="font-medium text-xs">{{ $t('migration.select_all', { count: filteredInstances.length }) }}</span>
              </button>

              <span class="font-mono text-[11px] text-cyan-400 bg-cyan-950/40 border border-cyan-800/50 px-2 py-0.5 rounded-full">
                {{ $t('migration.selected_count', { count: selectedInstances.length }) }}
              </span>
            </div>

            <!-- List of discovered instances -->
            <div class="space-y-2 max-h-[300px] overflow-y-auto pr-1">
              <div
                v-for="inst in filteredInstances"
                :key="inst.id"
                class="flex items-center justify-between p-3.5 rounded-xl border transition-all cursor-pointer"
                :class="
                  isInstanceSelected(inst.id)
                    ? 'bg-[#0f1724]/90 border-cyan-500/60 shadow-[0_0_16px_rgba(71,210,201,0.12)] ring-1 ring-cyan-500/30'
                    : 'bg-[#070b10]/70 border-slate-800/80 hover:border-slate-700 hover:bg-[#0c131d]'
                "
                @click="toggleInstance(inst)"
              >
                <!-- Left: Checkbox + Icon + Info -->
                <div class="flex items-center gap-3.5 min-w-0 flex-1">
                  <!-- Custom Checkbox -->
                  <div
                    class="w-5 h-5 rounded-lg border flex items-center justify-center shrink-0 transition-all"
                    :class="
                      isInstanceSelected(inst.id)
                        ? 'bg-cyan-500 border-cyan-400 text-slate-950 shadow-[0_0_8px_rgba(71,210,201,0.5)]'
                        : 'border-slate-700 bg-slate-900/90 hover:border-slate-500'
                    "
                    @click.stop="toggleInstance(inst)"
                  >
                    <svg v-if="isInstanceSelected(inst.id)" class="w-3.5 h-3.5 fill-current" viewBox="0 0 24 24">
                      <path d="M9 16.17L4.83 12l-1.42 1.41L9 19 21 7l-1.41-1.41z"/>
                    </svg>
                  </div>

                  <!-- Launcher SVG Icon badge -->
                  <div class="w-10 h-10 rounded-xl bg-[#0e1622] border border-slate-800 flex items-center justify-center shrink-0 shadow-inner p-2">
                    <img :src="getLauncherLogo(inst.sourceLauncher)" class="w-6 h-6 object-contain" alt="" />
                  </div>

                  <div class="min-w-0 flex-1">
                    <div class="flex items-center gap-2 mb-1">
                      <span class="text-sm font-bold text-white tracking-wide truncate">{{ getDisplayInstanceName(inst) }}</span>
                      <span class="text-[10px] font-mono px-2 py-0.5 rounded-full bg-slate-800/80 text-slate-300 border border-slate-700/60 shrink-0">
                        {{ getLauncherDisplayName(inst.sourceLauncher) }}
                      </span>
                    </div>

                    <div class="flex items-center gap-2 text-[11px] font-mono text-slate-400 flex-wrap">
                      <span class="bg-slate-900 px-2 py-0.5 rounded text-slate-200 border border-slate-800">
                        MC {{ inst.minecraftVersion }}
                      </span>
                      <span class="bg-cyan-950/40 text-cyan-300 border border-cyan-800/60 px-2 py-0.5 rounded capitalize">
                        {{ inst.loaderType }} {{ inst.loaderVersion || '' }}
                      </span>
                      <span>{{ $t('migration.mods_count', { count: inst.modCount }) }}</span>
                      <span v-if="inst.worldCount > 0">{{ $t('migration.worlds_count', { count: inst.worldCount }) }}</span>
                    </div>
                  </div>
                </div>

                <!-- Right: Last Played -->
                <div v-if="inst.lastPlayedMs" class="text-right text-[11px] font-mono text-slate-500 pl-3 shrink-0">
                  {{ formatRelativeTime(inst.lastPlayedMs) }}
                </div>
              </div>
            </div>

            <!-- Strategy Selection -->
            <div class="pt-4 border-t border-slate-800/80 space-y-2">
              <label class="block text-xs font-semibold text-slate-300">{{ $t('migration.strategy_label') }}</label>
              <div class="grid grid-cols-2 gap-3">
                <!-- Instant Link Strategy Card -->
                <div
                  class="p-3.5 rounded-xl border cursor-pointer transition-all flex items-start gap-3"
                  :class="strategy === 'hardlink_with_fallback' ? 'bg-gradient-to-br from-cyan-950/30 to-[#0e1724] border-cyan-500/60 shadow-[0_0_15px_rgba(71,210,201,0.12)] ring-1 ring-cyan-500/30' : 'bg-[#070b10]/60 border-slate-800/80 hover:border-slate-700 hover:bg-[#0c131d]'"
                  @click="strategy = 'hardlink_with_fallback'"
                >
                  <div
                    class="w-4 h-4 rounded-full border flex items-center justify-center shrink-0 mt-0.5 transition-all"
                    :class="strategy === 'hardlink_with_fallback' ? 'border-cyan-400 bg-cyan-950' : 'border-slate-700 bg-slate-900'"
                  >
                    <div v-if="strategy === 'hardlink_with_fallback'" class="w-2 h-2 rounded-full bg-cyan-400 shadow-[0_0_6px_rgba(71,210,201,0.8)]"></div>
                  </div>
                  <div>
                    <div class="text-xs font-bold text-slate-100 flex items-center gap-1.5">
                      <span>{{ $t('migration.instant_link') }}</span>
                      <span class="text-[9px] px-1.5 py-0.2 rounded-full bg-cyan-500/20 text-cyan-300 border border-cyan-500/40 font-mono">{{ $t('migration.recommended') }}</span>
                    </div>
                    <div class="text-[11px] text-slate-400 mt-1 leading-relaxed">
                      {{ $t('migration.instant_link_desc') }}
                    </div>
                  </div>
                </div>

                <!-- Full Copy Strategy Card -->
                <div
                  class="p-3.5 rounded-xl border cursor-pointer transition-all flex items-start gap-3"
                  :class="strategy === 'full_copy' ? 'bg-gradient-to-br from-cyan-950/30 to-[#0e1724] border-cyan-500/60 shadow-[0_0_15px_rgba(71,210,201,0.12)] ring-1 ring-cyan-500/30' : 'bg-[#070b10]/60 border-slate-800/80 hover:border-slate-700 hover:bg-[#0c131d]'"
                  @click="strategy = 'full_copy'"
                >
                  <div
                    class="w-4 h-4 rounded-full border flex items-center justify-center shrink-0 mt-0.5 transition-all"
                    :class="strategy === 'full_copy' ? 'border-cyan-400 bg-cyan-950' : 'border-slate-700 bg-slate-900'"
                  >
                    <div v-if="strategy === 'full_copy'" class="w-2 h-2 rounded-full bg-cyan-400 shadow-[0_0_6px_rgba(71,210,201,0.8)]"></div>
                  </div>
                  <div>
                    <div class="text-xs font-bold text-slate-100 flex items-center gap-1.5">
                      <span>{{ $t('migration.full_copy') }}</span>
                      <span class="text-[9px] px-1.5 py-0.2 rounded-full bg-slate-800 text-slate-400 border border-slate-700 font-mono">{{ $t('migration.standard') }}</span>
                    </div>
                    <div class="text-[11px] text-slate-400 mt-1 leading-relaxed">
                      {{ $t('migration.full_copy_desc') }}
                    </div>
                  </div>
                </div>
              </div>
            </div>
          </div>
        </div>
      </div>

      <!-- Footer Actions -->
      <div class="px-6 py-4 border-t border-slate-800/80 bg-slate-950/70 flex items-center justify-between">
        <div class="text-xs text-slate-500">
          {{ $t('migration.safe_notice') }}
        </div>

        <div class="flex items-center gap-3">
          <button
            class="z-btn-ghost text-xs px-4 py-2 rounded-xl text-slate-300 hover:text-white"
            :disabled="isImporting"
            @click="handleClose"
          >
            {{ $t('common.cancel') }}
          </button>

          <button
            class="z-btn-accent text-xs font-bold px-6 py-2.5 rounded-xl shadow-[0_0_20px_rgba(71,210,201,0.25)] hover:shadow-[0_0_28px_rgba(71,210,201,0.45)] flex items-center gap-2 transition-transform active:scale-95 disabled:opacity-50 disabled:pointer-events-none"
            :disabled="selectedInstances.length === 0 || isImporting"
            @click="executeImport"
          >
            <span v-if="isImporting" class="inline-block w-3.5 h-3.5 border-2 border-slate-900 border-t-transparent rounded-full animate-spin"></span>
            <svg v-else class="w-4 h-4 fill-none stroke-current" viewBox="0 0 24 24">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M4 16v1a3 3 0 003 3h10a3 3 0 003-3v-1m-4-8l-4-4m0 0L8 8m4-4v12" />
            </svg>
            <span>{{ $t('migration.import_selected', { count: selectedInstances.length }) }}</span>
          </button>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup>
import { ref, computed, onMounted, onUnmounted, watch } from 'vue';
import { api } from '../lib/api.js';

import prismLogo from '../assets/prism.svg';
import modrinthLogo from '../assets/modrinth.svg';
import curseforgeLogo from '../assets/curseforge.svg';
import minecraftLogo from '../assets/minecraft.svg';
import atlauncherLogo from '../assets/atlauncher.svg';

const props = defineProps({
  open: {
    type: Boolean,
    default: false,
  },
});

const emit = defineEmits(['close', 'imported']);

// State
const scanning = ref(false);
const scanResults = ref([]);
const activeFilter = ref('all');
const selectedInstances = ref([]);
const strategy = ref('hardlink_with_fallback');
const isImporting = ref(false);
const progressEvent = ref(null);
let unlistenProgress = null;

function getLauncherLogo(type) {
  switch (type) {
    case 'prism': return prismLogo;
    case 'modrinth': return modrinthLogo;
    case 'curse_forge': return curseforgeLogo;
    case 'vanilla': return minecraftLogo;
    case 'at_launcher': return atlauncherLogo;
    default: return minecraftLogo;
  }
}

function getLauncherDisplayName(type) {
  switch (type) {
    case 'prism': return 'Prism';
    case 'modrinth': return 'Modrinth';
    case 'curse_forge': return 'CurseForge';
    case 'vanilla': return 'Minecraft';
    case 'at_launcher': return 'ATLauncher';
    default: return type;
  }
}

function getLauncherShortName(type) {
  switch (type) {
    case 'prism': return 'Prism';
    case 'modrinth': return 'Modrinth';
    case 'curse_forge': return 'CurseForge';
    case 'vanilla': return 'Minecraft';
    case 'at_launcher': return 'ATLauncher';
    default: return type;
  }
}

function getDisplayInstanceName(inst) {
  if (!inst.name || !inst.name.trim()) {
    if (inst.sourceLauncher === 'vanilla') {
      return 'Minecraft Profile';
    }
    return 'Discovered Instance';
  }
  const clean = inst.name.trim();
  if (clean === 'latest-release') return 'Minecraft Latest Release';
  if (clean === 'latest-snapshot') return 'Minecraft Latest Snapshot';
  return clean;
}

const allDiscoveredInstances = computed(() => {
  const list = [];
  for (const res of scanResults.value) {
    if (res.instances && res.instances.length > 0) {
      list.push(...res.instances);
    }
  }
  return list;
});

const totalDiscoveredCount = computed(() => allDiscoveredInstances.value.length);

const filteredInstances = computed(() => {
  if (activeFilter.value === 'all') {
    return allDiscoveredInstances.value;
  }
  const group = scanResults.value.find((r) => r.launcherType === activeFilter.value);
  return group ? group.instances : [];
});

const areAllFilteredSelected = computed(() => {
  if (filteredInstances.value.length === 0) return false;
  return filteredInstances.value.every((inst) => isInstanceSelected(inst.id));
});

function isInstanceSelected(id) {
  return selectedInstances.value.some((inst) => inst.id === id);
}

function toggleInstance(inst) {
  const idx = selectedInstances.value.findIndex((i) => i.id === inst.id);
  if (idx >= 0) {
    selectedInstances.value.splice(idx, 1);
  } else {
    selectedInstances.value.push(inst);
  }
}

function toggleSelectAllFiltered() {
  if (areAllFilteredSelected.value) {
    // Deselect filtered
    const filteredIds = new Set(filteredInstances.value.map((i) => i.id));
    selectedInstances.value = selectedInstances.value.filter((i) => !filteredIds.has(i.id));
  } else {
    // Select all filtered
    for (const inst of filteredInstances.value) {
      if (!isInstanceSelected(inst.id)) {
        selectedInstances.value.push(inst);
      }
    }
  }
}

function formatRelativeTime(ms) {
  if (!ms) return '';
  const now = Date.now();
  const diffSec = Math.max(0, Math.floor((now - ms) / 1000));
  if (diffSec < 60) return 'Played just now';
  const diffMin = Math.floor(diffSec / 60);
  if (diffMin < 60) return `Played ${diffMin}m ago`;
  const diffHours = Math.floor(diffMin / 60);
  if (diffHours < 24) return `Played ${diffHours}h ago`;
  const diffDays = Math.floor(diffHours / 24);
  if (diffDays < 30) return `Played ${diffDays}d ago`;
  return new Date(ms).toLocaleDateString();
}

async function triggerScan() {
  if (scanning.value || isImporting.value) return;
  scanning.value = true;
  try {
    const results = await api.scanInstalledLaunchers();
    scanResults.value = results || [];

    // Pre-select all instances by default for immediate 1-click migration
    const all = [];
    for (const res of scanResults.value) {
      if (res.instances) {
        all.push(...res.instances);
      }
    }
    selectedInstances.value = all;
  } catch (err) {
    console.error('Failed to scan installed launchers:', err);
  } finally {
    scanning.value = false;
  }
}

async function executeImport() {
  if (selectedInstances.value.length === 0 || isImporting.value) return;
  isImporting.value = true;
  progressEvent.value = {
    currentInstanceName: getDisplayInstanceName(selectedInstances.value[0]),
    currentFile: 'Starting import...',
    completedInstances: 0,
    totalInstances: selectedInstances.value.length,
    fraction: 0.05,
  };

  try {
    const imported = await api.importDiscoveredInstances(
      selectedInstances.value,
      strategy.value
    );
    emit('imported', imported);
    handleClose();
  } catch (err) {
    console.error('Failed to import instances:', err);
    alert(`Migration error: ${err}`);
  } finally {
    isImporting.value = false;
  }
}

function handleClose() {
  if (isImporting.value) return;
  emit('close');
}

watch(
  () => props.open,
  (isOpen) => {
    if (isOpen) {
      triggerScan();
    } else {
      progressEvent.value = null;
      isImporting.value = false;
    }
  }
);

onMounted(async () => {
  if (props.open) {
    triggerScan();
  }
  unlistenProgress = await api.onMigrationProgress((evt) => {
    progressEvent.value = evt;
  });
});

onUnmounted(() => {
  if (unlistenProgress) {
    unlistenProgress();
  }
});
</script>
