<template>
  <div
    v-if="open"
    class="fixed inset-0 z-50 flex items-center justify-center p-4 bg-black/80 backdrop-blur-xl transition-all duration-300"
    @click.self="handleClose"
  >
    <div
      class="w-full max-w-4xl bg-[#090e17]/95 border border-slate-700/60 rounded-3xl shadow-[0_24px_70px_rgba(0,0,0,0.85),0_0_40px_rgba(71,210,201,0.18)] overflow-hidden flex flex-col animate-in fade-in zoom-in-95 duration-200 max-h-[88vh] relative before:absolute before:inset-x-0 before:top-0 before:h-[2px] before:bg-gradient-to-r before:from-transparent before:via-cyan-400/80 before:to-transparent"
    >
      <!-- Header -->
      <div class="px-6 py-4 border-b border-slate-800/80 flex items-center justify-between bg-gradient-to-r from-slate-900/90 via-slate-900/50 to-cyan-950/20">
        <div class="flex items-center gap-3">
          <div class="w-10 h-10 rounded-xl bg-cyan-500/10 border border-cyan-500/30 flex items-center justify-center text-cyan-400 shadow-[0_0_15px_rgba(71,210,201,0.2)] shrink-0">
            <svg class="w-5 h-5" fill="none" viewBox="0 0 24 24" stroke="currentColor">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M8 7h12m0 0l-4-4m4 4l-4 4m0 6H4m0 0l4 4m-4-4l4-4" />
            </svg>
          </div>
          <div>
            <h2 class="text-base font-bold text-slate-100 flex items-center gap-2">
              {{ $t('migration.title') }}
              <span class="text-[10px] font-mono px-2 py-0.5 rounded-full bg-cyan-500/15 text-cyan-300 border border-cyan-500/30 shadow-[0_0_8px_rgba(71,210,201,0.2)]">
                {{ $t('migration.zero_friction') }}
              </span>
            </h2>
            <p class="text-xs text-slate-400">{{ $t('migration.subtitle') }}</p>
          </div>
        </div>

        <div class="flex items-center gap-2">
          <button
            class="px-2.5 py-1.5 rounded-xl border border-slate-800 bg-[#070b10]/80 hover:bg-[#111c2a] hover:border-cyan-500/40 text-slate-300 hover:text-cyan-300 text-xs font-semibold flex items-center gap-1.5 transition-all shadow-sm cursor-pointer"
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
            class="text-slate-500 hover:text-slate-300 p-1.5 rounded-lg hover:bg-slate-800/60 transition-colors cursor-pointer"
            :disabled="isImporting"
            @click="handleClose"
          >
            <svg class="w-5 h-5" fill="none" viewBox="0 0 24 24" stroke="currentColor">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 18L18 6M6 6l12 12" />
            </svg>
          </button>
        </div>
      </div>

      <!-- Split Master-Detail Body -->
      <div class="flex flex-1 min-h-0 overflow-hidden">
        <!-- Left Sidebar: Sources & Transfer Strategy -->
        <aside class="w-64 min-w-[240px] border-r border-slate-800/80 bg-[#070b11]/80 p-4 flex flex-col justify-between overflow-y-auto shrink-0 space-y-6">
          <!-- Top: Launchers Filter -->
          <div class="space-y-2">
            <div class="text-[10px] font-mono font-bold uppercase tracking-wider text-cyan-400/80 px-1">
              {{ $t('migration.all_launchers') }}
            </div>

            <div class="space-y-1">
              <button
                class="w-full px-3 py-2 rounded-xl text-xs font-bold transition-all flex items-center justify-between text-left cursor-pointer border"
                :class="activeFilter === 'all'
                  ? 'bg-cyan-500/15 border-cyan-500/40 text-cyan-200 shadow-[0_0_12px_rgba(71,210,201,0.15)] font-bold'
                  : 'border-transparent text-slate-400 hover:text-slate-200 hover:bg-slate-800/40'"
                @click="activeFilter = 'all'"
              >
                <div class="flex items-center gap-2">
                  <svg class="w-4 h-4 text-cyan-400" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                    <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M19 11H5m14 0a2 2 0 012 2v6a2 2 0 01-2 2H5a2 2 0 01-2-2v-6a2 2 0 012-2m14 0V9a2 2 0 00-2-2M5 11V9a2 2 0 012-2m0 0V5a2 2 0 012-2h6a2 2 0 012 2v2M7 7h10" />
                  </svg>
                  <span>{{ $t('migration.all_launchers') }}</span>
                </div>
                <span class="text-[10px] px-2 py-0.5 rounded-full font-mono bg-slate-800/80 text-slate-300">
                  {{ totalDiscoveredCount }}
                </span>
              </button>

              <button
                v-for="res in scanResults"
                :key="res.launcherType"
                class="w-full px-3 py-2 rounded-xl text-xs font-bold transition-all flex items-center justify-between text-left cursor-pointer border"
                :class="activeFilter === res.launcherType
                  ? 'bg-cyan-500/15 border-cyan-500/40 text-cyan-200 shadow-[0_0_12px_rgba(71,210,201,0.15)] font-bold'
                  : 'border-transparent text-slate-400 hover:text-slate-200 hover:bg-slate-800/40'"
                @click="activeFilter = res.launcherType"
              >
                <div class="flex items-center gap-2.5 min-w-0">
                  <img :src="getLauncherLogo(res.launcherType)" class="w-4 h-4 object-contain shrink-0" alt="" />
                  <span class="truncate">{{ getLauncherDisplayName(res.launcherType) }}</span>
                </div>
                <span
                  class="text-[10px] px-2 py-0.5 rounded-full font-mono font-bold shrink-0"
                  :class="res.instances.length > 0 ? 'bg-cyan-950 text-cyan-300 border border-cyan-800/80 shadow-sm' : 'bg-slate-800/60 text-slate-500'"
                >
                  {{ res.instances.length }}
                </span>
              </button>
            </div>
          </div>

          <!-- Bottom: Transfer Strategy Selector (Compact, no scroll) -->
          <div class="pt-3 border-t border-slate-800/80 mt-auto">
            <div class="text-[10px] font-mono font-bold uppercase tracking-wider text-slate-400 px-1 mb-2">
              {{ $t('migration.strategy_label') }}
            </div>

            <div class="space-y-1.5">
              <!-- Instant Link Option -->
              <div
                class="px-2.5 py-2 rounded-xl border cursor-pointer transition-all flex items-center justify-between gap-2"
                :class="strategy === 'hardlink_with_fallback'
                  ? 'bg-cyan-500/15 border-cyan-500/50 shadow-[0_0_10px_rgba(71,210,201,0.12)] text-cyan-200 font-bold'
                  : 'bg-[#070b10]/60 border-slate-800/80 text-slate-400 hover:text-slate-200 hover:border-slate-700'"
                @click="strategy = 'hardlink_with_fallback'"
                :title="$t('migration.instant_link_desc')"
              >
                <div class="flex items-center gap-2 min-w-0">
                  <div
                    class="w-3.5 h-3.5 rounded-full border flex items-center justify-center shrink-0 transition-all"
                    :class="strategy === 'hardlink_with_fallback' ? 'border-cyan-400 bg-cyan-950' : 'border-slate-700 bg-slate-900'"
                  >
                    <div v-if="strategy === 'hardlink_with_fallback'" class="w-1.5 h-1.5 rounded-full bg-cyan-400 shadow-[0_0_4px_rgba(71,210,201,0.8)]"></div>
                  </div>
                  <span class="text-xs truncate">{{ $t('migration.instant_link') }}</span>
                </div>
                <span class="text-[9px] px-1.5 py-0.2 rounded-full bg-cyan-500/20 text-cyan-300 border border-cyan-500/30 font-mono shrink-0">
                  0 GB
                </span>
              </div>

              <!-- Full Copy Option -->
              <div
                class="px-2.5 py-2 rounded-xl border cursor-pointer transition-all flex items-center justify-between gap-2"
                :class="strategy === 'full_copy'
                  ? 'bg-cyan-500/15 border-cyan-500/50 shadow-[0_0_10px_rgba(71,210,201,0.12)] text-cyan-200 font-bold'
                  : 'bg-[#070b10]/60 border-slate-800/80 text-slate-400 hover:text-slate-200 hover:border-slate-700'"
                @click="strategy = 'full_copy'"
                :title="$t('migration.full_copy_desc')"
              >
                <div class="flex items-center gap-2 min-w-0">
                  <div
                    class="w-3.5 h-3.5 rounded-full border flex items-center justify-center shrink-0 transition-all"
                    :class="strategy === 'full_copy' ? 'border-cyan-400 bg-cyan-950' : 'border-slate-700 bg-slate-900'"
                  >
                    <div v-if="strategy === 'full_copy'" class="w-1.5 h-1.5 rounded-full bg-cyan-400 shadow-[0_0_4px_rgba(71,210,201,0.8)]"></div>
                  </div>
                  <span class="text-xs truncate">{{ $t('migration.full_copy') }}</span>
                </div>
                <span class="text-[9px] px-1.5 py-0.2 rounded-full bg-slate-800 text-slate-400 border border-slate-700 font-mono shrink-0">
                  Clone
                </span>
              </div>
            </div>
            <p class="text-[10px] text-slate-500 px-1 pt-1.5 leading-tight">
              {{ strategy === 'hardlink_with_fallback' ? 'Hardlinks mod JARs with zero extra disk space.' : 'Creates detached file duplicates.' }}
            </p>
          </div>
        </aside>

        <!-- Right Main Stage: Toolbar & 2-Column Instance Grid -->
        <main class="flex-1 min-w-0 flex flex-col overflow-hidden bg-gradient-to-b from-[#0a121e]/50 via-[#070c14]/30 to-transparent">
          <!-- In-flight Import Progress Overlay -->
          <div v-if="isImporting" class="m-5 bg-cyan-950/30 border border-cyan-500/40 rounded-2xl p-5 space-y-4 shadow-[0_0_25px_rgba(71,210,201,0.15)]">
            <div class="flex items-center justify-between">
              <div class="flex items-center gap-2.5">
                <span class="inline-block w-4 h-4 border-2 border-cyan-400 border-t-transparent rounded-full animate-spin"></span>
                <span class="text-sm font-bold text-white tracking-wide">
                  {{ $t('migration.importing_prefix', { name: progressEvent?.currentInstanceName || $t('migration.preparing_files') }) }}
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
          <div v-else-if="scanning" class="py-24 flex flex-col items-center justify-center space-y-3">
            <div class="w-8 h-8 border-2 border-cyan-400 border-t-transparent rounded-full animate-spin shadow-[0_0_12px_rgba(71,210,201,0.3)]"></div>
            <p class="text-xs text-slate-400 font-mono">{{ $t('migration.scan_system_notice') }}</p>
          </div>

          <!-- Main Stage Content -->
          <div v-else class="flex-1 min-h-0 flex flex-col p-5">
            <!-- Toolbar: Search Filter & Batch Select -->
            <div class="flex items-center justify-between gap-3 pb-3 border-b border-slate-800/80 mb-3">
              <!-- Search Filter Input (Flex group prevents icon overlap) -->
              <div class="flex items-center gap-2 bg-[#070b10] border border-slate-800/90 rounded-xl px-3 py-1.5 focus-within:border-cyan-500/50 focus-within:ring-1 focus-within:ring-cyan-500/30 shadow-inner w-56 sm:w-64 transition-all">
                <svg class="w-3.5 h-3.5 text-slate-500 shrink-0" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                  <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M21 21l-6-6m2-5a7 7 0 11-14 0 7 7 0 0114 0z" />
                </svg>
                <input
                  v-model="searchQuery"
                  type="text"
                  placeholder="Filter instances..."
                  class="bg-transparent text-xs text-slate-200 placeholder-slate-500 focus:outline-none w-full"
                />
                <button
                  v-if="searchQuery"
                  type="button"
                  class="text-slate-500 hover:text-slate-300 text-xs shrink-0 cursor-pointer"
                  @click="searchQuery = ''"
                >
                  ✕
                </button>
              </div>

              <div class="flex items-center gap-3">
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

                <span class="font-mono text-[11px] text-cyan-400 bg-cyan-950/40 border border-cyan-800/50 px-2.5 py-0.5 rounded-full">
                  {{ $t('migration.selected_count', { count: selectedInstances.length }) }}
                </span>
              </div>
            </div>

            <!-- Empty State -->
            <div v-if="filteredInstances.length === 0" class="py-16 text-center space-y-3 flex-1 flex flex-col items-center justify-center">
              <div class="w-12 h-12 rounded-2xl bg-slate-900/80 border border-slate-800 flex items-center justify-center text-slate-500 shadow-inner">
                <svg class="w-6 h-6" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                  <path stroke-linecap="round" stroke-linejoin="round" stroke-width="1.5" d="M20 13V6a2 2 0 00-2-2H6a2 2 0 00-2 2v7m16 0v5a2 2 0 01-2 2H6a2 2 0 01-2-2v-5m16 0h-2.586a1 1 0 00-.707.293l-2.414 2.414a1 1 0 01-.707.293h-3.172a1 1 0 01-.707-.293l-2.414-2.414A1 1 0 006.586 13H4" />
                </svg>
              </div>
              <div class="text-sm font-bold text-slate-300">{{ $t('migration.no_instances') }}</div>
              <p class="text-xs text-slate-500 max-w-md leading-relaxed">
                {{ $t('migration.no_instances_desc') }}
              </p>
              <button
                v-if="searchQuery"
                class="px-3 py-1.5 rounded-xl border border-slate-700 bg-slate-900/60 hover:bg-slate-800 text-slate-300 text-xs font-semibold"
                @click="searchQuery = ''"
              >
                Clear Search
              </button>
            </div>

            <!-- 2-Column Responsive Card Grid (Compact auto-rows-max, no artificial stretch) -->
            <div v-else class="grid grid-cols-1 md:grid-cols-2 gap-3 auto-rows-max content-start overflow-y-auto flex-1 pr-1 pb-1">
              <div
                v-for="inst in filteredInstances"
                :key="inst.id"
                class="p-3.5 rounded-2xl border transition-all duration-200 cursor-pointer flex flex-col gap-2.5 relative overflow-hidden group hover:scale-[1.01]"
                :class="isInstanceSelected(inst.id)
                  ? 'bg-gradient-to-br from-[#0e1724]/95 to-[#0b121e] border-cyan-500/60 shadow-[0_0_18px_rgba(71,210,201,0.15)] ring-1 ring-cyan-500/30'
                  : 'bg-[#070b10]/70 border-slate-800/80 hover:border-slate-700 hover:bg-[#0c131d]'"
                @click="toggleInstance(inst)"
              >
                <!-- Card Header: Checkbox + Brand Icon + Name -->
                <div class="flex items-start gap-3 min-w-0">
                  <div
                    class="w-5 h-5 rounded-lg border flex items-center justify-center shrink-0 mt-0.5 transition-all"
                    :class="isInstanceSelected(inst.id)
                      ? 'bg-cyan-500 border-cyan-400 text-slate-950 shadow-[0_0_8px_rgba(71,210,201,0.5)]'
                      : 'border-slate-700 bg-slate-900/90 group-hover:border-slate-500'"
                    @click.stop="toggleInstance(inst)"
                  >
                    <svg v-if="isInstanceSelected(inst.id)" class="w-3.5 h-3.5 fill-current" viewBox="0 0 24 24">
                      <path d="M9 16.17L4.83 12l-1.42 1.41L9 19 21 7l-1.41-1.41z"/>
                    </svg>
                  </div>

                  <div class="w-8 h-8 rounded-xl bg-[#090f17] border border-slate-700/60 flex items-center justify-center shrink-0 p-1.5 shadow-inner">
                    <img :src="getLauncherLogo(inst.sourceLauncher)" class="w-full h-full object-contain" alt="" />
                  </div>

                  <div class="min-w-0 flex-1">
                    <div class="text-xs font-bold text-white tracking-wide truncate">
                      {{ getDisplayInstanceName(inst) }}
                    </div>
                    <div class="text-[10px] font-mono text-slate-400">
                      {{ getLauncherDisplayName(inst.sourceLauncher) }}
                    </div>
                  </div>
                </div>

                <!-- Metadata Badges -->
                <div class="flex items-center gap-1.5 flex-wrap text-[10px] font-mono">
                  <span class="bg-slate-900 px-2 py-0.5 rounded-lg text-slate-300 border border-slate-800">
                    MC {{ inst.minecraftVersion }}
                  </span>
                  <span class="bg-cyan-950/40 text-cyan-300 border border-cyan-800/60 px-2 py-0.5 rounded-lg capitalize">
                    {{ inst.loaderType }} {{ inst.loaderVersion || '' }}
                  </span>
                  <span class="text-slate-400 bg-slate-900/50 px-1.5 py-0.5 rounded border border-slate-800/60">
                    {{ $t('migration.mods_count', { count: inst.modCount }) }}
                  </span>
                  <span v-if="inst.worldCount > 0" class="text-slate-400 bg-slate-900/50 px-1.5 py-0.5 rounded border border-slate-800/60">
                    {{ $t('migration.worlds_count', { count: inst.worldCount }) }}
                  </span>
                </div>

                <!-- Card Footer: Last Played -->
                <div v-if="inst.lastPlayedMs" class="text-[10px] font-mono text-slate-500 pt-1 border-t border-slate-800/60">
                  {{ formatRelativeTime(inst.lastPlayedMs) }}
                </div>
              </div>
            </div>
          </div>
        </main>
      </div>

      <!-- Footer Actions -->
      <div class="px-6 py-4 border-t border-slate-800/80 bg-slate-950/70 flex items-center justify-between">
        <div class="flex items-center gap-2 text-xs text-slate-500">
          <svg class="w-4 h-4 text-emerald-400 shrink-0" fill="none" viewBox="0 0 24 24" stroke="currentColor">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9 12l2 2 4-4m5.618-4.016A11.955 11.955 0 0112 2.944a11.955 11.955 0 01-8.618 3.04A12.02 12.02 0 003 9c0 5.591 3.824 10.29 9 11.622 5.176-1.332 9-6.03 9-11.622 0-1.042-.133-2.052-.382-3.016z" />
          </svg>
          <span>{{ $t('migration.safe_notice') }}</span>
        </div>

        <div class="flex items-center gap-3">
          <button
            class="z-btn-ghost text-xs px-4 py-2 rounded-xl text-slate-300 hover:text-white cursor-pointer"
            :disabled="isImporting"
            @click="handleClose"
          >
            {{ $t('common.cancel') }}
          </button>

          <button
            class="z-btn-accent text-xs font-black px-6 py-2.5 rounded-xl shadow-[0_0_24px_rgba(71,210,201,0.35)] hover:shadow-[0_0_36px_rgba(71,210,201,0.6)] flex items-center gap-2 transition-transform active:scale-95 disabled:opacity-50 disabled:pointer-events-none hover:brightness-110 cursor-pointer"
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
  initialScanResults: {
    type: Array,
    default: null,
  },
});

const emit = defineEmits(['close', 'imported']);

// State
const scanning = ref(false);
const scanResults = ref([]);
const activeFilter = ref('all');
const searchQuery = ref('');
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

function getDisplayInstanceName(inst) {
  if (!inst.name || !inst.name.trim()) {
    if (inst.sourceLauncher === 'vanilla') {
      const ver = inst.minecraftVersion ? ` (${inst.minecraftVersion})` : '';
      return `Minecraft Profile${ver}`;
    }
    return 'Discovered Instance';
  }
  const clean = inst.name.trim();
  if (clean === 'latest-release') return 'Minecraft Latest Release';
  if (clean === 'latest-snapshot') return 'Minecraft Latest Snapshot';

  // Detect raw hex or alphanumeric profile hashes from launcher_profiles.json (e.g. 336a5236241dfb904693e1fa...)
  const isOpaqueHash = clean.length >= 16 && !clean.includes(' ') && /^[a-f0-9]+$/i.test(clean);
  if (isOpaqueHash) {
    if (inst.sourceLauncher === 'vanilla') {
      const ver = inst.minecraftVersion ? ` ${inst.minecraftVersion}` : '';
      return `Minecraft Profile${ver}`;
    }
    return `Instance (${clean.slice(0, 8)})`;
  }

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
  let list = [];
  if (activeFilter.value === 'all') {
    list = allDiscoveredInstances.value;
  } else {
    const group = scanResults.value.find((r) => r.launcherType === activeFilter.value);
    list = group ? group.instances : [];
  }

  const q = searchQuery.value.trim().toLowerCase();
  if (q) {
    list = list.filter((inst) => {
      const name = (inst.name || '').toLowerCase();
      const loader = (inst.loaderType || '').toLowerCase();
      const mc = (inst.minecraftVersion || '').toLowerCase();
      return name.includes(q) || loader.includes(q) || mc.includes(q);
    });
  }
  return list;
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
    const filteredIds = new Set(filteredInstances.value.map((i) => i.id));
    selectedInstances.value = selectedInstances.value.filter((i) => !filteredIds.has(i.id));
  } else {
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

function applyScanResults(results) {
  scanResults.value = results || [];
  const all = [];
  for (const res of scanResults.value) {
    if (res.instances) {
      all.push(...res.instances);
    }
  }
  selectedInstances.value = all;
}

async function triggerScan() {
  if (scanning.value || isImporting.value) return;
  scanning.value = true;
  try {
    const results = await api.scanInstalledLaunchers();
    applyScanResults(results);
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
      if (props.initialScanResults && props.initialScanResults.length > 0) {
        applyScanResults(props.initialScanResults);
      } else {
        triggerScan();
      }
    } else {
      progressEvent.value = null;
      isImporting.value = false;
    }
  }
);

onMounted(async () => {
  if (props.open) {
    if (props.initialScanResults && props.initialScanResults.length > 0) {
      applyScanResults(props.initialScanResults);
    } else {
      triggerScan();
    }
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
