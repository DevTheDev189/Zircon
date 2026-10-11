<template>
  <div
    v-if="open"
    class="fixed inset-0 z-50 flex items-center justify-center p-4 bg-black/80 backdrop-blur-md transition-all duration-300"
    @click.self="handleClose"
  >
    <div
      class="w-full max-w-2xl bg-[#090e17]/95 border border-slate-700/60 rounded-2xl shadow-[0_0_60px_rgba(0,0,0,0.8),0_0_30px_rgba(71,210,201,0.12)] overflow-hidden flex flex-col animate-in fade-in zoom-in-95 duration-200 max-h-[90vh] relative before:absolute before:inset-x-0 before:top-0 before:h-[2px] before:bg-gradient-to-r before:from-transparent before:via-cyan-400/60 before:to-transparent"
    >
      <!-- Header -->
      <div class="px-6 py-5 border-b border-slate-800/80 flex items-center justify-between bg-gradient-to-r from-slate-900/90 via-slate-900/50 to-cyan-950/20">
        <div class="flex items-center gap-3">
          <div class="w-10 h-10 rounded-xl bg-cyan-500/10 border border-cyan-500/30 flex items-center justify-center text-cyan-400 shadow-[0_0_15px_rgba(71,210,201,0.2)]">
            <svg class="w-5 h-5" fill="none" viewBox="0 0 24 24" stroke="currentColor">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M4 16v1a3 3 0 003 3h10a3 3 0 003-3v-1m-4-8l-4-4m0 0L8 8m4-4v12" />
            </svg>
          </div>
          <div>
            <h2 class="text-base font-bold text-slate-100 flex items-center gap-2">
              {{ $t('import_setup.title') }}
              <span class="text-[10px] font-mono px-2 py-0.5 rounded-full bg-cyan-500/15 text-cyan-300 border border-cyan-500/30 shadow-[0_0_8px_rgba(71,210,201,0.2)]">
                {{ $t('import_setup.badge') }}
              </span>
            </h2>
            <p class="text-xs text-slate-400">{{ $t('import_setup.subtitle') }}</p>
          </div>
        </div>
        <button
          class="text-slate-500 hover:text-slate-300 p-1.5 rounded-lg hover:bg-slate-800/60 transition-colors"
          :disabled="applying"
          @click="handleClose"
        >
          <svg class="w-5 h-5" fill="none" viewBox="0 0 24 24" stroke="currentColor">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 18L18 6M6 6l12 12" />
          </svg>
        </button>
      </div>

      <!-- Body Content -->
      <div class="p-6 overflow-y-auto space-y-5 flex-1">
        <!-- Progress Overlay if actively importing -->
        <div v-if="applying" class="bg-cyan-950/30 border border-cyan-500/40 rounded-xl p-5 space-y-4 shadow-[0_0_25px_rgba(71,210,201,0.15)]">
          <div class="flex items-center justify-between">
            <div class="flex items-center gap-2.5">
              <span class="inline-block w-4 h-4 border-2 border-cyan-400 border-t-transparent rounded-full animate-spin"></span>
              <span class="text-sm font-bold text-white tracking-wide">
                {{ progress.phase || $t('import_setup.importing_phase') }}
              </span>
            </div>
            <span class="text-xs font-mono font-bold text-cyan-300">
              {{ Math.round((progress.fraction || 0) * 100) }}%
            </span>
          </div>

          <!-- Progress Bar -->
          <div class="w-full bg-slate-900 rounded-full h-2.5 overflow-hidden border border-slate-800/80">
            <div
              class="bg-gradient-to-r from-cyan-500 via-teal-400 to-cyan-300 h-full transition-all duration-300 shadow-[0_0_14px_rgba(71,210,201,0.6)]"
              :style="{ width: `${Math.round((progress.fraction || 0) * 100)}%` }"
            ></div>
          </div>

          <div class="flex items-center justify-between text-[11px] text-slate-400 font-mono">
            <span class="truncate max-w-[380px]">{{ progress.detail || progress.message || $t('import_setup.downloading_deps') }}</span>
            <span v-if="progress.total > 0">{{ $t('import_setup.files_progress', { current: progress.current, total: progress.total }) }}</span>
          </div>
        </div>

        <!-- Normal Content when not applying -->
        <template v-else>
          <!-- 1. DRAG & DROP ZONE -->
          <div
            class="relative rounded-2xl border-2 border-dashed transition-all cursor-pointer overflow-hidden p-6 text-center select-none"
            :class="[
              isDragOver
                ? 'border-cyan-400 bg-cyan-950/40 shadow-[0_0_30px_rgba(71,210,201,0.25)] ring-2 ring-cyan-400/40'
                : mrpackData
                  ? 'border-cyan-500/50 bg-[#0e1724]/90 shadow-[0_0_20px_rgba(71,210,201,0.1)]'
                  : 'border-slate-700/80 bg-[#070b10]/80 hover:border-cyan-500/50 hover:bg-[#0c131e]'
            ]"
            @dragover.prevent="onDragOver"
            @dragleave.prevent="onDragLeave"
            @drop.prevent="onDrop"
            @click="browseFile"
          >
            <!-- Case A: Modpack is Selected / Inspected -->
            <div v-if="mrpackData" class="space-y-4 text-left" @click.stop>
              <div class="flex items-start justify-between gap-4">
                <div class="flex items-center gap-3.5">
                  <div class="w-12 h-12 rounded-xl bg-[#090e17] border border-cyan-500/40 flex items-center justify-center p-2.5 shrink-0 shadow-[0_0_15px_rgba(71,210,201,0.15)]">
                    <img
                      v-if="mrpackData.archiveType === 'curseforge'"
                      src="../assets/curseforge.svg"
                      class="w-7 h-7 object-contain"
                      alt="CurseForge"
                    />
                    <img
                      v-else
                      src="../assets/modrinth.svg"
                      class="w-7 h-7 object-contain"
                      alt="Modrinth"
                    />
                  </div>

                  <div>
                    <div class="flex items-center gap-2">
                      <h3 class="text-sm font-bold text-white tracking-wide">
                        {{ mrpackData.name }}
                      </h3>
                      <span
                        class="text-[10px] font-mono px-2 py-0.5 rounded-full uppercase"
                        :class="mrpackData.archiveType === 'curseforge' ? 'bg-amber-950/60 text-amber-300 border border-amber-800/60' : 'bg-emerald-950/60 text-emerald-300 border border-emerald-800/60'"
                      >
                        {{ mrpackData.archiveType === 'curseforge' ? $t('import_setup.curseforge_zip') : $t('import_setup.modrinth_mrpack') }}
                      </span>
                    </div>

                    <div class="flex items-center gap-2 text-[11px] font-mono text-slate-400 mt-1 flex-wrap">
                      <span class="bg-slate-900 px-2 py-0.5 rounded text-slate-200 border border-slate-800">
                        MC {{ mrpackData.mcVersion }}
                      </span>
                      <span class="bg-cyan-950/40 text-cyan-300 border border-cyan-800/60 px-2 py-0.5 rounded capitalize">
                        {{ mrpackData.loaderType }} {{ mrpackData.loaderVersion || '' }}
                      </span>
                      <span class="text-slate-300 font-semibold">{{ $t('import_setup.mods_files_count', { count: mrpackData.fileCount }) }}</span>
                    </div>
                  </div>
                </div>

                <button
                  class="text-xs text-slate-400 hover:text-cyan-300 px-2.5 py-1.5 rounded-lg border border-slate-800 hover:border-slate-700 bg-slate-900/60 transition-colors"
                  @click.stop="clearMrpack"
                  :title="$t('import_setup.choose_diff_file')"
                >
                  {{ $t('import_setup.change_file') }}
                </button>
              </div>

              <!-- Summary / description -->
              <p v-if="mrpackData.summary" class="text-xs text-slate-400 line-clamp-2 leading-relaxed bg-[#070b10] p-2.5 rounded-lg border border-slate-800/60">
                {{ mrpackData.summary }}
              </p>

              <!-- Custom Instance Name Input -->
              <div class="pt-2 border-t border-slate-800/80">
                <label class="block text-xs font-semibold text-slate-300 mb-1.5">{{ $t('import_setup.instance_name_label') }}</label>
                <input
                  v-model="mrpackCustomName"
                  type="text"
                  :placeholder="$t('import_setup.instance_name_placeholder')"
                  class="w-full bg-[#070b10] border border-slate-700/80 rounded-xl px-3.5 py-2 text-xs font-semibold text-white focus:outline-none focus:border-cyan-400"
                />
              </div>

              <div class="text-[11px] text-slate-500 font-mono truncate">
                {{ $t('import_setup.source_prefix', { path: mrpackFilePath }) }}
              </div>
            </div>

            <!-- Case B: Empty Drop Target State -->
            <div v-else class="space-y-3">
              <div class="w-12 h-12 mx-auto rounded-2xl bg-cyan-500/10 border border-cyan-500/30 flex items-center justify-center text-cyan-400 shadow-[0_0_20px_rgba(71,210,201,0.15)] group-hover:scale-105 transition-transform">
                <svg class="w-6 h-6" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                  <path stroke-linecap="round" stroke-linejoin="round" stroke-width="1.8" d="M7 16a4 4 0 01-.88-7.903A5 5 0 1115.9 6L16 6a5 5 0 011 9.9M15 13l-3-3m0 0l-3 3m3-3v12" />
                </svg>
              </div>

              <div>
                <div class="text-sm font-bold text-slate-200">
                  {{ $t('import_setup.drop_mrpack_prompt') }}
                </div>
                <div class="text-xs text-slate-400 mt-1">
                  {{ $t('import_setup.drop_recipe_subtext') }}
                </div>
              </div>

              <div class="flex items-center justify-center gap-2 pt-1">
                <span class="text-[10px] font-mono px-2 py-0.5 rounded-full bg-slate-900 border border-slate-800 text-slate-400">
                  {{ $t('import_setup.modrinth_mrpack') }}
                </span>
                <span class="text-[10px] font-mono px-2 py-0.5 rounded-full bg-slate-900 border border-slate-800 text-slate-400">
                  {{ $t('import_setup.curseforge_zip') }}
                </span>
                <span class="text-[10px] font-mono px-2 py-0.5 rounded-full bg-slate-900 border border-slate-800 text-slate-400">
                  {{ $t('import_setup.zircon_json') }}
                </span>
              </div>
            </div>
          </div>

          <!-- Divider with Toggle -->
          <div class="relative flex items-center justify-center">
            <div class="border-t border-slate-800/80 w-full"></div>
            <span class="bg-[#090e17] px-3 text-[11px] font-mono text-slate-500 uppercase tracking-wider shrink-0">
              {{ $t('import_setup.or_paste_share_code') }}
            </span>
          </div>

          <!-- 2. PASTE SHARE CODE OR JSON -->
          <div>
            <div class="flex items-center justify-between mb-1.5">
              <label class="block text-xs font-semibold text-slate-300">{{ $t('import_setup.paste_input_label') }}</label>
              <button
                v-if="rawInput"
                class="text-[11px] text-slate-500 hover:text-slate-300 font-mono"
                @click="clearRawInput"
              >
                {{ $t('common.clear') }}
              </button>
            </div>
            <textarea
              v-model="rawInput"
              rows="3"
              :placeholder="$t('import_setup.paste_input_placeholder')"
              class="w-full bg-[#070b10] border border-slate-700/80 rounded-xl p-3 text-xs font-mono text-cyan-300 placeholder:text-slate-600 resize-none focus:outline-none focus:border-cyan-500 transition-colors"
              @input="handleInput"
            ></textarea>
          </div>

          <!-- Recipe Preview Card (When rawInput is parsed) -->
          <div v-if="preview" class="bg-slate-900/60 border border-slate-800 rounded-xl p-4 space-y-3 animate-in fade-in duration-200">
            <div class="flex items-center justify-between border-b border-slate-800/80 pb-3">
              <div>
                <h3 class="text-sm font-bold text-white flex items-center gap-2">
                  {{ preview.incomingBom?.serverTitle || $t('import_setup.default_setup_title') }}
                  <span class="text-[11px] font-mono px-2 py-0.5 rounded-full bg-slate-800 text-slate-300">
                    MC {{ preview.incomingBom?.minecraftVersion }}
                  </span>
                  <span v-if="preview.incomingBom?.modLoader" class="text-[11px] font-mono px-2 py-0.5 rounded-full bg-cyan-950 text-cyan-300 border border-cyan-800">
                    {{ preview.incomingBom?.modLoader?.type }} {{ preview.incomingBom?.modLoader?.version }}
                  </span>
                </h3>
                <p class="text-xs text-slate-400 mt-1">{{ $t('import_setup.contains_mods_count', { count: preview.incomingBom?.mods?.length || 0 }) }}</p>
              </div>
            </div>

            <!-- Diff Breakdown if target instance exists -->
            <div v-if="preview.diff" class="grid grid-cols-4 gap-2 pt-1 text-center text-xs">
              <div class="bg-emerald-950/30 border border-emerald-800/40 rounded-lg p-2">
                <div class="text-emerald-400 font-bold text-sm">+{{ preview.diff.toAdd?.length || 0 }}</div>
                <div class="text-[10px] text-slate-400">{{ $t('import_setup.diff_to_add') }}</div>
              </div>
              <div class="bg-amber-950/30 border border-amber-800/40 rounded-lg p-2">
                <div class="text-amber-400 font-bold text-sm">~{{ preview.diff.toUpdate?.length || 0 }}</div>
                <div class="text-[10px] text-slate-400">{{ $t('import_setup.diff_to_update') }}</div>
              </div>
              <div class="bg-slate-800/30 border border-slate-700/40 rounded-lg p-2">
                <div class="text-slate-300 font-bold text-sm">{{ preview.diff.unchanged?.length || 0 }}</div>
                <div class="text-[10px] text-slate-400">{{ $t('import_setup.diff_unchanged') }}</div>
              </div>
              <div class="bg-rose-950/30 border border-rose-800/40 rounded-lg p-2">
                <div class="text-rose-400 font-bold text-sm">-{{ preview.diff.toRemove?.length || 0 }}</div>
                <div class="text-[10px] text-slate-400">{{ $t('import_setup.diff_removed') }}</div>
              </div>
            </div>

            <!-- Strategy Selection -->
            <div class="space-y-2 pt-2">
              <label class="block text-xs font-semibold text-slate-300">{{ $t('import_setup.strategy_label') }}</label>
              <div class="space-y-2">
                <label class="flex items-start gap-3 p-3 rounded-xl border cursor-pointer transition-all"
                  :class="strategy === 'new_instance' ? 'bg-cyan-950/30 border-cyan-500/50' : 'bg-slate-950/40 border-slate-800 hover:border-slate-700'">
                  <input type="radio" v-model="strategy" value="new_instance" class="mt-0.5 zircon-radio" />
                  <div>
                    <div class="text-xs font-bold text-slate-200">{{ $t('import_setup.strategy_new_instance') }}</div>
                    <div class="text-[11px] text-slate-400">{{ $t('import_setup.strategy_new_instance_desc') }}</div>
                  </div>
                </label>

                <label v-if="effectiveTargetInstance" class="flex items-start gap-3 p-3 rounded-xl border cursor-pointer transition-all"
                  :class="strategy === 'merge' ? 'bg-cyan-950/30 border-cyan-500/50' : 'bg-slate-950/40 border-slate-800 hover:border-slate-700'">
                  <input type="radio" v-model="strategy" value="merge" class="mt-0.5 zircon-radio" />
                  <div>
                    <div class="text-xs font-bold text-slate-200">{{ $t('import_setup.strategy_merge', { name: effectiveTargetInstance.name }) }}</div>
                    <div class="text-[11px] text-slate-400">{{ $t('import_setup.strategy_merge_desc') }}</div>
                  </div>
                </label>

                <label v-if="effectiveTargetInstance" class="flex items-start gap-3 p-3 rounded-xl border cursor-pointer transition-all"
                  :class="strategy === 'replace' ? 'bg-cyan-950/30 border-cyan-500/50' : 'bg-slate-950/40 border-slate-800 hover:border-slate-700'">
                  <input type="radio" v-model="strategy" value="replace" class="mt-0.5 zircon-radio" />
                  <div>
                    <div class="text-xs font-bold text-slate-200">{{ $t('import_setup.strategy_replace', { name: effectiveTargetInstance.name }) }}</div>
                    <div class="text-[11px] text-slate-400">{{ $t('import_setup.strategy_replace_desc') }}</div>
                  </div>
                </label>
              </div>
            </div>
          </div>

          <div v-if="parseError" class="p-3.5 rounded-xl bg-rose-950/40 border border-rose-800/60 text-xs text-rose-300 font-mono">
            {{ parseError }}
          </div>
        </template>
      </div>

      <!-- Footer -->
      <div class="px-6 py-4 border-t border-slate-800/80 bg-slate-950/60 flex items-center justify-between">
        <button
          class="z-btn-ghost text-xs px-4 py-2 rounded-xl font-semibold border border-slate-800 hover:border-slate-700 text-slate-400 hover:text-white"
          :disabled="applying"
          @click="handleClose"
        >
          {{ $t('common.cancel') }}
        </button>

        <div class="flex items-center gap-3">
          <!-- Install Button for .mrpack -->
          <button
            v-if="mrpackData"
            class="z-btn-accent text-xs px-5 py-2.5 rounded-xl font-bold flex items-center gap-2 shadow-[0_0_15px_rgba(71,210,201,0.25)] hover:shadow-[0_0_20px_rgba(71,210,201,0.4)]"
            :disabled="applying || inspecting"
            @click="handleApplyMrpack"
          >
            <span v-if="applying" class="w-3.5 h-3.5 border-2 border-slate-950 border-t-transparent rounded-full animate-spin"></span>
            <svg v-else class="w-3.5 h-3.5 fill-current" viewBox="0 0 24 24">
              <path d="M19 9h-4V3H9v6H5l7 7 7-7zM5 18v2h14v-2H5z"/>
            </svg>
            <span>{{ applying ? $t('import_setup.importing_phase') : $t('import_setup.install_mrpack_btn', { name: mrpackData.name }) }}</span>
          </button>

          <!-- Apply Button for Recipe Manifest -->
          <button
            v-else
            class="z-btn-primary text-xs px-5 py-2.5 rounded-xl font-bold flex items-center gap-2"
            :disabled="!preview || applying"
            @click="handleApplyRecipe"
          >
            <span v-if="applying" class="w-3 h-3 border-2 border-white border-t-transparent rounded-full animate-spin"></span>
            <span>{{ applying ? $t('import_setup.downloading_and_installing') : $t('import_setup.apply_and_download') }}</span>
          </button>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup>
import { computed, onBeforeUnmount, onMounted, ref, watch } from 'vue';
import { api, MODPACK_FILTER, pickFile, onModpackProgress } from '../lib/api.js';

const props = defineProps({
  open: { type: Boolean, default: false },
  targetInstance: { type: Object, default: null },
  currentInstance: { type: Object, default: null },
  initialFilePath: { type: String, default: '' },
});

const emit = defineEmits(['close', 'imported', 'applied']);

const effectiveTargetInstance = computed(() => props.targetInstance || props.currentInstance || null);

const isDragOver = ref(false);
const mrpackFilePath = ref('');
const mrpackCustomName = ref('');
const mrpackData = ref(null);
const inspecting = ref(false);

const rawInput = ref('');
const preview = ref(null);
const parseError = ref('');
const strategy = ref('new_instance');
const applying = ref(false);

const progress = ref({
  phase: '',
  fraction: 0,
  current: 0,
  total: 0,
  detail: '',
  message: '',
});

let unlistenProgress = null;

watch(
  () => props.open,
  async (isOpen) => {
    if (isOpen) {
      clearAll();
      strategy.value = effectiveTargetInstance.value ? 'merge' : 'new_instance';
      if (props.initialFilePath) {
        await loadFilePath(props.initialFilePath);
      }
    }
  }
);

watch(
  () => props.initialFilePath,
  async (newPath) => {
    if (newPath && props.open) {
      await loadFilePath(newPath);
    }
  }
);

onMounted(async () => {
  try {
    unlistenProgress = await onModpackProgress((p) => {
      progress.value = {
        phase: p.phase || 'Importing...',
        fraction: typeof p.fraction === 'number' ? p.fraction : 0,
        current: p.current || 0,
        total: p.total || 0,
        detail: p.message || p.detail || '',
        message: p.message || '',
      };
    });
  } catch (err) {
    console.warn('Modpack progress listener registration error:', err);
  }

  window.addEventListener('zircon-mrpack-dropped', onGlobalMrpackDrop);
});

onBeforeUnmount(() => {
  if (unlistenProgress) unlistenProgress();
  window.removeEventListener('zircon-mrpack-dropped', onGlobalMrpackDrop);
});

function onGlobalMrpackDrop(e) {
  if (props.open && e.detail?.filePath) {
    loadFilePath(e.detail.filePath);
  }
}

function clearAll() {
  mrpackFilePath.value = '';
  mrpackCustomName.value = '';
  mrpackData.value = null;
  inspecting.value = false;
  rawInput.value = '';
  preview.value = null;
  parseError.value = '';
  applying.value = false;
  progress.value = { phase: '', fraction: 0, current: 0, total: 0, detail: '', message: '' };
}

function clearMrpack() {
  mrpackFilePath.value = '';
  mrpackCustomName.value = '';
  mrpackData.value = null;
}

function clearRawInput() {
  rawInput.value = '';
  preview.value = null;
  parseError.value = '';
}

function onDragOver(e) {
  isDragOver.value = true;
}

function onDragLeave(e) {
  isDragOver.value = false;
}

async function onDrop(e) {
  isDragOver.value = false;
  const files = e.dataTransfer?.files;
  if (!files || !files.length) return;

  const file = files[0];
  const name = file.name.toLowerCase();

  // If dropped file is .mrpack or .zip
  if (name.endsWith('.mrpack') || name.endsWith('.zip')) {
    if (file.path) {
      await loadFilePath(file.path);
    } else {
      // In web fallback when file.path is stripped
      parseError.value = 'File path not accessible from browser drag-drop. Please click to browse.';
    }
    return;
  }

  // If dropped file is JSON manifest or text
  if (name.endsWith('.json') || name.endsWith('.txt')) {
    try {
      const text = await file.text();
      rawInput.value = text;
      runPreview();
    } catch (err) {
      parseError.value = `Failed to read file: ${err}`;
    }
  }
}

async function browseFile() {
  if (mrpackData.value) return;
  try {
    const picked = await pickFile(MODPACK_FILTER);
    if (picked) {
      await loadFilePath(picked);
    }
  } catch (err) {
    console.warn('Pick file error:', err);
  }
}

async function loadFilePath(filePath) {
  parseError.value = '';
  const lower = filePath.toLowerCase();

  if (lower.endsWith('.mrpack') || lower.endsWith('.zip')) {
    inspecting.value = true;
    mrpackFilePath.value = filePath;
    try {
      const info = await api.inspectLocalModpack(filePath);
      mrpackData.value = info;
      mrpackCustomName.value = info.name || filePath.split(/[/\\]/).pop().replace(/\.(mrpack|zip)$/i, '');
      rawInput.value = '';
      preview.value = null;
    } catch (err) {
      // Fallback if inspect fails, still allow direct install
      const fallbackName = filePath.split(/[/\\]/).pop().replace(/\.(mrpack|zip)$/i, '');
      mrpackData.value = {
        name: fallbackName,
        summary: 'Archive ready for import',
        mcVersion: 'Auto-detect',
        loaderType: 'Auto-detect',
        loaderVersion: '',
        file_count: 0,
        archiveType: lower.endsWith('.mrpack') ? 'modrinth' : 'curseforge',
      };
      mrpackCustomName.value = fallbackName;
    } finally {
      inspecting.value = false;
    }
  }
}

let debounceTimer = null;
function handleInput() {
  clearTimeout(debounceTimer);
  debounceTimer = setTimeout(() => {
    runPreview();
  }, 300);
}

async function runPreview() {
  const code = rawInput.value.trim();
  if (!code) {
    preview.value = null;
    parseError.value = '';
    return;
  }

  try {
    parseError.value = '';
    const res = await api.previewImportSetup(code, effectiveTargetInstance.value?.id);
    preview.value = res;
    // Clear mrpack if recipe is actively loaded
    mrpackData.value = null;
    mrpackFilePath.value = '';
  } catch (e) {
    parseError.value = `Failed to parse setup: ${e}`;
    preview.value = null;
  }
}

async function handleApplyMrpack() {
  if (!mrpackFilePath.value) return;
  applying.value = true;
  progress.value = {
    phase: 'Initializing modpack installation...',
    fraction: 0.05,
    current: 0,
    total: mrpackData.value?.fileCount || 1,
    detail: mrpackFilePath.value,
    message: 'Extracting manifest and downloading files...',
  };

  try {
    const inst = await api.importLocalMrpack(
      mrpackFilePath.value,
      mrpackCustomName.value.trim() || null
    );
    window.dispatchEvent(
      new CustomEvent('zircon-status', {
        detail: `Modpack "${inst.name || mrpackCustomName.value}" imported successfully!`,
      })
    );
    emit('imported', inst);
    emit('applied', inst);
    handleClose();
  } catch (e) {
    parseError.value = `Modpack import failed: ${e}`;
    window.dispatchEvent(new CustomEvent('zircon-status', { detail: `Import error: ${e}` }));
  } finally {
    applying.value = false;
  }
}

async function handleApplyRecipe() {
  if (!preview.value?.incomingBom) return;
  applying.value = true;
  try {
    const res = await api.applyImportSetup(
      preview.value.incomingBom,
      strategy.value,
      effectiveTargetInstance.value?.id,
      null
    );
    window.dispatchEvent(
      new CustomEvent('zircon-status', { detail: `Setup applied successfully!` })
    );
    emit('imported', res);
    emit('applied', res);
    handleClose();
  } catch (e) {
    parseError.value = `Import failed: ${e}`;
    window.dispatchEvent(new CustomEvent('zircon-status', { detail: `Import failed: ${e}` }));
  } finally {
    applying.value = false;
  }
}

function handleClose() {
  emit('close');
}
</script>
