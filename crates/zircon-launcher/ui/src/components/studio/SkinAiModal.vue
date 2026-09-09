<template>
  <div class="fixed inset-0 z-50 flex items-center justify-center p-4 bg-black/80 backdrop-blur-md">
    <div class="w-full max-w-4xl bg-[#090e17] border border-slate-700/80 rounded-2xl shadow-2xl overflow-hidden flex flex-col max-h-[92vh]">
      
      <!-- HEADER -->
      <div class="px-6 py-4 bg-[#0d1522] border-b border-slate-800 flex items-center justify-between shrink-0">
        <div class="flex items-center gap-3">
          <div class="w-9 h-9 rounded-xl bg-cyan-500/15 border border-cyan-500/30 flex items-center justify-center text-cyan-400">
            <svg class="w-5 h-5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
              <path d="m12 3-1.9 5.8a2 2 0 0 1-1.3 1.3L3 12l5.8 1.9a2 2 0 0 1 1.3 1.3L12 21l1.9-5.8a2 2 0 0 1 1.3-1.3L21 12l-5.8-1.9a2 2 0 0 1-1.3-1.3L12 3z" />
            </svg>
          </div>
          <div>
            <div class="flex items-center gap-2">
              <h2 class="text-base font-bold text-white tracking-wide">AI Concept & Draft Studio</h2>
              <span class="px-2 py-0.5 rounded-full text-[10px] font-bold tracking-wider uppercase bg-emerald-500/15 border border-emerald-500/30 text-emerald-300 flex items-center gap-1">
                <svg class="w-3 h-3" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5">
                  <rect x="3" y="11" width="18" height="11" rx="2" ry="2"/>
                  <path d="M7 11V7a5 5 0 0 1 10 0v4"/>
                </svg>
                100% Local & Private
              </span>
            </div>
            <p class="text-xs text-slate-400 mt-0.5">
              Rapid concepting & rough drafting on your device. Zero cloud telemetry. Drop straight into Paint Studio to finish.
            </p>
          </div>
        </div>

        <button
          type="button"
          class="p-2 rounded-xl text-slate-400 hover:text-white hover:bg-slate-800 transition-colors"
          @click="$emit('close')"
        >
          <svg class="w-5 h-5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
            <line x1="18" y1="6" x2="6" y2="18" />
            <line x1="6" y1="6" x2="18" y2="18" />
          </svg>
        </button>
      </div>

      <!-- BODY CONTENT -->
      <div class="flex-1 overflow-y-auto p-6 space-y-6">

        <!-- DOWNLOAD BANNER IF MODEL NOT DOWNLOADED -->
        <div
          v-if="!modelStatus.is_downloaded"
          class="p-5 rounded-2xl bg-gradient-to-r from-cyan-950/40 via-[#0e1a2b] to-slate-900 border border-cyan-500/30 flex flex-col md:flex-row items-center justify-between gap-4"
        >
          <div class="flex items-center gap-4">
            <div class="w-12 h-12 rounded-xl bg-cyan-500/20 border border-cyan-400/40 flex items-center justify-center text-cyan-300 shrink-0">
              <svg class="w-6 h-6" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                <path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4" />
                <polyline points="7 10 12 15 17 10" />
                <line x1="12" y1="15" x2="12" y2="3" />
              </svg>
            </div>
            <div>
              <h3 class="text-sm font-bold text-white">Local Pixel-DiT Model Required (~65 MB)</h3>
              <p class="text-xs text-slate-300 mt-0.5 max-w-xl">
                To keep Zircon Launcher lightweight, the AI skin generation weights are an optional on-demand download. Once downloaded, all generation runs 100% on your local CPU/GPU with zero cloud reliance.
              </p>
            </div>
          </div>

          <div class="shrink-0 flex items-center gap-3">
            <div v-if="modelStatus.is_downloading" class="flex items-center gap-3">
              <div class="flex flex-col items-end text-xs">
                <span class="font-bold text-cyan-400">{{ downloadProgress.percentage.toFixed(1) }}%</span>
                <span class="text-slate-400 text-[11px]">{{ (downloadProgress.speed_bytes_per_sec / 1048576).toFixed(1) }} MB/s</span>
              </div>
              <button
                type="button"
                class="px-3 py-2 rounded-xl bg-red-500/20 hover:bg-red-500/30 text-red-300 text-xs font-semibold"
                @click="cancelDownload"
              >
                Cancel
              </button>
            </div>
            <button
              v-else
              type="button"
              class="px-4 py-2 rounded-xl bg-cyan-500 hover:bg-cyan-400 text-slate-950 font-bold text-xs shadow-lg shadow-cyan-500/20 flex items-center gap-2 transition-transform active:scale-95"
              @click="startDownload"
            >
              <svg class="w-4 h-4" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                <path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4" />
                <polyline points="7 10 12 15 17 10" />
                <line x1="12" y1="15" x2="12" y2="3" />
              </svg>
              Download Model (~65 MB)
            </button>
          </div>
        </div>

        <!-- CONTROLS: SMART OMNIBAR & TAG CHIPS -->
        <div class="space-y-3">
          <div class="flex items-center justify-between">
            <label class="text-xs font-bold uppercase tracking-wider text-slate-300 flex items-center gap-1.5">
              <span>Concept Prompt & Tags</span>
              <span class="text-slate-500 font-normal lowercase">(type words or select tags below)</span>
            </label>
            <div class="flex items-center gap-2">
              <span class="text-xs text-slate-400 font-semibold">Model Rig:</span>
              <div class="flex rounded-lg bg-[#121c2a] p-0.5 border border-slate-700">
                <button
                  type="button"
                  class="px-2.5 py-1 text-xs font-semibold rounded-md transition-colors"
                  :class="selectedVariant === 'classic' ? 'bg-cyan-500 text-slate-950 font-bold' : 'text-slate-300 hover:text-white'"
                  @click="selectedVariant = 'classic'"
                >
                  Classic (4px)
                </button>
                <button
                  type="button"
                  class="px-2.5 py-1 text-xs font-semibold rounded-md transition-colors"
                  :class="selectedVariant === 'slim' ? 'bg-cyan-500 text-slate-950 font-bold' : 'text-slate-300 hover:text-white'"
                  @click="selectedVariant = 'slim'"
                >
                  Slim (3px)
                </button>
              </div>
            </div>
          </div>

          <!-- SMART OMNIBAR INPUT WITH TAG AUTOCOMPLETE -->
          <div class="relative">
            <div class="min-h-[44px] px-3 py-1.5 bg-[#101826] border border-slate-700 hover:border-slate-600 focus-within:border-cyan-400 rounded-xl flex flex-wrap items-center gap-1.5 transition-colors">
              <!-- SELECTED TAG CHIPS -->
              <span
                v-for="(tag, idx) in selectedTags"
                :key="tag"
                class="inline-flex items-center gap-1.5 px-2.5 py-1 rounded-lg bg-cyan-500/15 border border-cyan-400/30 text-cyan-300 text-xs font-semibold shadow-sm"
              >
                <span>{{ tag }}</span>
                <button
                  type="button"
                  class="text-cyan-400 hover:text-cyan-100"
                  @click="removeTag(idx)"
                >
                  &times;
                </button>
              </span>

              <!-- TEXT INPUT -->
              <input
                v-model="tagInput"
                type="text"
                class="flex-1 min-w-[140px] bg-transparent text-xs text-white placeholder-slate-500 focus:outline-none py-1"
                placeholder="Type tags like cyberpunk, hoodie, knight, neon..."
                @keydown.enter.prevent="addTypedTag"
                @keydown.backspace="onBackspace"
                @focus="showSuggestions = true"
              />
            </div>

            <!-- AUTOCOMPLETE DROPDOWN SUGGESTIONS -->
            <div
              v-if="showSuggestions && filteredSuggestions.length > 0"
              class="absolute left-0 right-0 top-full mt-1 bg-[#0f1724] border border-slate-700 rounded-xl shadow-2xl p-1.5 z-40 max-h-48 overflow-y-auto flex flex-wrap gap-1"
            >
              <button
                v-for="sugg in filteredSuggestions"
                :key="sugg.name"
                type="button"
                class="px-2.5 py-1 text-xs rounded-lg bg-slate-800/80 hover:bg-cyan-500/20 text-slate-200 hover:text-cyan-300 font-medium transition-colors"
                @click="selectSuggestion(sugg.name)"
              >
                + {{ sugg.name }}
              </button>
            </div>
          </div>
        </div>

        <!-- PALETTE PICKER (PRIMARY, SECONDARY, ACCENT) -->
        <div class="space-y-2">
          <label class="text-xs font-bold uppercase tracking-wider text-slate-300">
            Color Palette Guidance (Primary, Secondary, Accent)
          </label>
          <div class="flex items-center gap-4 flex-wrap">
            <div class="flex items-center gap-2 bg-[#101826] p-2 rounded-xl border border-slate-700/80">
              <span class="text-xs font-semibold text-slate-400">Primary:</span>
              <input v-model="palette[0]" type="color" class="w-7 h-7 rounded cursor-pointer bg-transparent border-0" />
              <span class="text-xs font-mono text-slate-200 uppercase">{{ palette[0] }}</span>
            </div>
            <div class="flex items-center gap-2 bg-[#101826] p-2 rounded-xl border border-slate-700/80">
              <span class="text-xs font-semibold text-slate-400">Secondary:</span>
              <input v-model="palette[1]" type="color" class="w-7 h-7 rounded cursor-pointer bg-transparent border-0" />
              <span class="text-xs font-mono text-slate-200 uppercase">{{ palette[1] }}</span>
            </div>
            <div class="flex items-center gap-2 bg-[#101826] p-2 rounded-xl border border-slate-700/80">
              <span class="text-xs font-semibold text-slate-400">Accent:</span>
              <input v-model="palette[2]" type="color" class="w-7 h-7 rounded cursor-pointer bg-transparent border-0" />
              <span class="text-xs font-mono text-slate-200 uppercase">{{ palette[2] }}</span>
            </div>

            <!-- PRESET PALETTES -->
            <div class="flex items-center gap-1.5 ml-auto">
              <span class="text-[11px] text-slate-400">Presets:</span>
              <button
                type="button"
                class="px-2 py-1 text-[11px] rounded-lg bg-slate-800 hover:bg-slate-700 text-cyan-300 font-semibold"
                @click="setPalette(['#0f172a', '#0284c7', '#38bdf8'])"
              >
                Cyberpunk
              </button>
              <button
                type="button"
                class="px-2 py-1 text-[11px] rounded-lg bg-slate-800 hover:bg-slate-700 text-amber-300 font-semibold"
                @click="setPalette(['#292524', '#78350f', '#fbbf24'])"
              >
                Medieval
              </button>
              <button
                type="button"
                class="px-2 py-1 text-[11px] rounded-lg bg-slate-800 hover:bg-slate-700 text-emerald-300 font-semibold"
                @click="setPalette(['#064e3b', '#047857', '#34d399'])"
              >
                Druid
              </button>
            </div>
          </div>
        </div>

        <!-- GENERATE ACTIONS -->
        <div class="flex items-center justify-between pt-2">
          <div class="text-xs text-slate-400">
            <span v-if="generationTime">Generated in {{ generationTime }}ms</span>
          </div>

          <div class="flex items-center gap-3">
            <button
              v-if="candidates.length > 0"
              type="button"
              class="px-3.5 py-2 rounded-xl bg-slate-800 hover:bg-slate-700 text-slate-200 font-bold text-xs flex items-center gap-1.5 transition-colors"
              :disabled="isGenerating"
              @click="reRollSeeds"
            >
              <svg class="w-3.5 h-3.5 text-cyan-400" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                <path d="M21.5 2v6h-6M21.34 15.57a10 10 0 1 1-.57-8.38l5.67-5.67" />
              </svg>
              Re-roll Seeds
            </button>

            <button
              type="button"
              class="px-5 py-2.5 rounded-xl bg-gradient-to-r from-cyan-500 to-blue-500 hover:from-cyan-400 hover:to-blue-400 text-slate-950 font-extrabold text-xs shadow-lg shadow-cyan-500/25 flex items-center gap-2 transition-transform active:scale-95 disabled:opacity-50"
              :disabled="isGenerating || selectedTags.length === 0"
              @click="generateBatch"
            >
              <svg v-if="isGenerating" class="w-4 h-4 animate-spin" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                <circle cx="12" cy="12" r="10" stroke-opacity="0.25" stroke="currentColor" />
                <path d="M12 2a10 10 0 0 1 10 10" />
              </svg>
              <svg v-else class="w-4 h-4" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5">
                <polygon points="5 3 19 12 5 21 5 3" />
              </svg>
              <span>{{ isGenerating ? 'Generating 4 Variants...' : 'Generate 4 Concept Drafts' }}</span>
            </button>
          </div>
        </div>

        <!-- 4-CANDIDATE GRID (THE CORE WORKFLOW) -->
        <div v-if="candidates.length > 0" class="space-y-3 pt-2">
          <div class="flex items-center justify-between">
            <h3 class="text-xs font-bold uppercase tracking-wider text-slate-300">
              Select a Concept Draft to Edit:
            </h3>
            <span class="text-[11px] text-cyan-400 font-medium">Click "Edit in Paint Studio" to jump straight into editing!</span>
          </div>

          <div class="grid grid-cols-2 md:grid-cols-4 gap-4">
            <div
              v-for="cand in candidates"
              :key="cand.seed"
              class="bg-[#0f1725] border border-slate-700/80 hover:border-cyan-400/80 rounded-2xl p-3 flex flex-col items-center gap-3 transition-all duration-200 group hover:shadow-xl hover:shadow-cyan-500/10"
            >
              <!-- 3D LIVE ISOMETRIC PREVIEW -->
              <div class="w-full h-44 bg-[#080d14] rounded-xl flex items-center justify-center overflow-hidden relative">
                <img
                  v-if="cand.render3d"
                  :src="cand.render3d"
                  alt="3D Preview"
                  class="h-40 object-contain drop-shadow-[0_8px_16px_rgba(0,0,0,0.7)] group-hover:scale-105 transition-transform"
                />
                <img
                  v-else
                  :src="cand.data_url"
                  alt="Flat 2D Texture"
                  class="w-24 h-24 image-pixelated object-contain"
                />

                <span class="absolute top-2 left-2 px-1.5 py-0.5 rounded bg-slate-900/80 text-[10px] font-mono text-slate-400">
                  #{{ cand.index + 1 }}
                </span>
              </div>

              <!-- ACTIONS: DROP STRAIGHT INTO EDITING -->
              <div class="w-full space-y-1.5">
                <button
                  type="button"
                  class="w-full py-2 px-2.5 rounded-xl bg-cyan-500 hover:bg-cyan-400 text-slate-950 font-bold text-xs flex items-center justify-center gap-1.5 shadow-md shadow-cyan-500/20 transition-colors"
                  title="Open directly in the 2D & 3D Paint Studio"
                  @click="editInStudio(cand)"
                >
                  <svg class="w-3.5 h-3.5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5">
                    <path d="M12 20h9" />
                    <path d="M16.5 3.5a2.121 2.121 0 0 1 3 3L7 19l-4 1 1-4L16.5 3.5z" />
                  </svg>
                  <span>Edit in Studio</span>
                </button>

                <button
                  type="button"
                  class="w-full py-1.5 px-2 rounded-xl bg-slate-800/80 hover:bg-slate-700 text-slate-300 hover:text-white font-semibold text-[11px] transition-colors"
                  @click="quickSave(cand)"
                >
                  Save to Gallery
                </button>
              </div>
            </div>
          </div>
        </div>

      </div>
    </div>
  </div>
</template>

<script setup>
import { ref, computed, onMounted, onUnmounted } from 'vue';
import { api, renderSkinIsometric3D, onSkinAiDownloadProgress, onSkinAiDownloadComplete } from '../../lib/api.js';

const emit = defineEmits(['close', 'edit-in-studio', 'skin-saved']);

// State
const modelStatus = ref({ is_downloaded: true, is_downloading: false });
const downloadProgress = ref({ percentage: 0, speed_bytes_per_sec: 0 });
const selectedVariant = ref('classic');
const selectedTags = ref(['cyberpunk', 'hoodie', 'cyan_neon']);
const tagInput = ref('');
const showSuggestions = ref(false);
const palette = ref(['#0f172a', '#38bdf8', '#f1f5f9']);
const candidates = ref([]);
const isGenerating = ref(false);
const generationTime = ref(0);

// Curated vocabulary for Omnibar autocomplete with synonym mapping
const CURATED_TAGS = [
  { name: 'cyberpunk', category: 'theme' },
  { name: 'medieval_fantasy', category: 'theme' },
  { name: 'knight', category: 'archetype' },
  { name: 'hoodie', category: 'outfit' },
  { name: 'trench_coat', category: 'outfit' },
  { name: 'plate_armor', category: 'outfit' },
  { name: 'headphones', category: 'headwear' },
  { name: 'crown', category: 'headwear' },
  { name: 'horns', category: 'headwear' },
  { name: 'ninja_assassin', category: 'archetype' },
  { name: 'robot_cyborg', category: 'archetype' },
  { name: 'neon_glow', category: 'accent' },
  { name: 'glowing_eyes', category: 'accent' },
  { name: 'steampunk', category: 'theme' },
  { name: 'casual_modern', category: 'theme' },
];

const filteredSuggestions = computed(() => {
  const query = tagInput.value.trim().toLowerCase();
  if (!query) return CURATED_TAGS.slice(0, 10);
  return CURATED_TAGS.filter(t => t.name.includes(query) && !selectedTags.value.includes(t.name));
});

function addTypedTag() {
  const clean = tagInput.value.trim().toLowerCase().replace(/\s+/g, '_');
  if (clean && !selectedTags.value.includes(clean)) {
    selectedTags.value.push(clean);
  }
  tagInput.value = '';
  showSuggestions.value = false;
}

function selectSuggestion(name) {
  if (!selectedTags.value.includes(name)) {
    selectedTags.value.push(name);
  }
  tagInput.value = '';
  showSuggestions.value = false;
}

function removeTag(idx) {
  selectedTags.value.splice(idx, 1);
}

function onBackspace() {
  if (!tagInput.value && selectedTags.value.length > 0) {
    selectedTags.value.pop();
  }
}

function setPalette(colors) {
  palette.value = [...colors];
}

async function checkStatus() {
  try {
    const status = await api.getSkinAiStatus();
    modelStatus.value = status;
  } catch (err) {
    console.warn('Failed to check Skin AI status:', err);
  }
}

async function startDownload() {
  try {
    modelStatus.value.is_downloading = true;
    await api.startSkinAiDownload();
  } catch (err) {
    console.error('Failed to start model download:', err);
    modelStatus.value.is_downloading = false;
  }
}

async function cancelDownload() {
  try {
    await api.cancelSkinAiDownload();
    modelStatus.value.is_downloading = false;
  } catch (err) {
    console.error('Failed to cancel download:', err);
  }
}

// Generation
async function generateBatch() {
  if (selectedTags.value.length === 0) return;
  isGenerating.value = true;
  try {
    const res = await api.generateSkinBatch({
      tags: selectedTags.value,
      palette: palette.value,
      variant: selectedVariant.value,
      steps: 15,
      guidance_scale: 2.0,
      seed: Date.now(),
    });

    generationTime.value = res.generation_time_ms;

    // Enrich variants with 3D isometric turnaround renders
    const enriched = await Promise.all(
      res.variants.map(async (v) => {
        const render = await renderSkinIsometric3D(v.data_url, v.variant || selectedVariant.value);
        return {
          ...v,
          render3d: render,
        };
      })
    );

    candidates.value = enriched;
  } catch (err) {
    console.error('Skin AI generation error:', err);
  } finally {
    isGenerating.value = false;
  }
}

async function reRollSeeds() {
  await generateBatch();
}

// Direct-to-Studio handoff: drops user straight into editing
function editInStudio(cand) {
  emit('edit-in-studio', {
    dataUrl: cand.data_url,
    name: `concept_${selectedTags.value[0] || 'draft'}.png`,
    variant: cand.variant || selectedVariant.value,
  });
  emit('close');
}

async function quickSave(cand) {
  try {
    const base64 = cand.data_url.split(',')[1];
    const binary = atob(base64);
    const bytes = new Uint8Array(binary.length);
    for (let i = 0; i < binary.length; i++) {
      bytes[i] = binary.charCodeAt(i);
    }
    const name = `ai_concept_${Date.now()}.png`;
    await api.saveSkinBytes(name, Array.from(bytes), cand.variant || selectedVariant.value);
    emit('skin-saved');
  } catch (err) {
    console.error('Failed to save AI skin:', err);
  }
}

let unlistenProgress = null;
let unlistenComplete = null;

onMounted(async () => {
  await checkStatus();
  try {
    unlistenProgress = await onSkinAiDownloadProgress((payload) => {
      downloadProgress.value = payload;
      modelStatus.value.is_downloading = true;
    });
    unlistenComplete = await onSkinAiDownloadComplete(() => {
      modelStatus.value.is_downloaded = true;
      modelStatus.value.is_downloading = false;
    });
  } catch (err) {
    console.warn('Download listeners unavailable:', err);
  }

  // Pre-generate initial 4 candidates on open
  generateBatch();
});

onUnmounted(() => {
  if (unlistenProgress) unlistenProgress();
  if (unlistenComplete) unlistenComplete();
});
</script>

<style scoped>
.image-pixelated {
  image-rendering: pixelated;
  image-rendering: -moz-crisp-edges;
  image-rendering: crisp-edges;
}
</style>
