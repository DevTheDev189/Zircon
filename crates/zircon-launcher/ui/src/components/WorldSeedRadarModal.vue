<template>
  <div
    v-if="open"
    class="fixed inset-0 z-50 flex items-center justify-center p-4 bg-black/80 backdrop-blur-md transition-all duration-300"
    @click.self="handleClose"
  >
    <div
      class="w-full max-w-4xl bg-[#080d14]/95 border border-slate-800/90 rounded-2xl shadow-2xl shadow-cyan-950/40 overflow-hidden flex flex-col animate-in fade-in zoom-in-95 duration-200 max-h-[92vh]"
    >
      <!-- Header -->
      <div class="px-6 py-4 border-b border-slate-800/80 flex items-center justify-between bg-gradient-to-r from-slate-900/90 via-slate-900/50 to-cyan-950/30">
        <div class="flex items-center gap-3">
          <div class="w-10 h-10 rounded-xl bg-cyan-500/10 border border-cyan-500/30 flex items-center justify-center text-cyan-400 shadow-inner">
            <svg class="w-5 h-5 animate-spin-slow" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
              <circle cx="12" cy="12" r="9" stroke-dasharray="4 2" />
              <path d="M12 3v18M3 12h18" stroke-opacity="0.4" />
              <circle cx="12" cy="12" r="3" fill="currentColor" fill-opacity="0.2" />
            </svg>
          </div>
          <div>
            <h2 class="text-base font-bold text-slate-100 flex items-center gap-2">
              {{ $t('radar.title') }}
              <span class="text-[10px] font-mono px-2 py-0.5 rounded-full bg-cyan-500/10 text-cyan-400 border border-cyan-500/20">{{ $t('radar.anvil_hud') }}</span>
            </h2>
            <p class="text-xs text-slate-400">{{ $t('radar.subtitle') }}</p>
          </div>
        </div>
        <button
          class="text-slate-500 hover:text-slate-300 p-1.5 rounded-lg hover:bg-slate-800/60 transition-colors"
          :title="$t('common.close')"
          @click="handleClose"
        >
          <svg class="w-5 h-5" fill="none" viewBox="0 0 24 24" stroke="currentColor">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 18L18 6M6 6l12 12" />
          </svg>
        </button>
      </div>

      <!-- Main Body -->
      <div class="p-6 overflow-y-auto flex flex-col md:flex-row gap-6 flex-1 items-start">
        <!-- Radar Viewport (Left / Center) -->
        <div class="flex-1 flex flex-col items-center justify-center w-full">
          <!-- Circular Radar Frame -->
          <div class="relative w-[340px] h-[340px] sm:w-[380px] sm:h-[380px] rounded-full border-2 border-cyan-500/40 bg-slate-950 p-2 shadow-[0_0_50px_rgba(6,182,212,0.15)] flex items-center justify-center overflow-hidden">
            <!-- Background Map Image -->
            <img
              v-if="radarResult?.imageDataUrl"
              :src="radarResult.imageDataUrl"
              alt="World Map Radar"
              class="w-full h-full object-cover rounded-full select-none pointer-events-none transition-opacity duration-300"
              :class="loading ? 'opacity-40' : 'opacity-100'"
            />
            <div
              v-else
              class="w-full h-full rounded-full bg-slate-900/60 flex items-center justify-center text-slate-500 text-xs text-center p-6 font-mono"
            >
              <div v-if="loading" class="flex flex-col items-center gap-3">
                <span class="w-8 h-8 border-2 border-cyan-400 border-t-transparent rounded-full animate-spin"></span>
                <span class="text-cyan-400 animate-pulse text-xs">{{ $t('radar.scanning_chunks') }}</span>
              </div>
              <span v-else>{{ $t('radar.click_to_scan') }}</span>
            </div>

            <!-- Concentric Grid Overlay (HUD Optics) -->
            <div class="absolute inset-0 rounded-full border border-cyan-500/20 pointer-events-none m-8"></div>
            <div class="absolute inset-0 rounded-full border border-cyan-500/15 pointer-events-none m-20"></div>
            <div class="absolute inset-0 rounded-full border border-cyan-500/10 pointer-events-none m-32"></div>

            <!-- Coordinate Crosshairs -->
            <div class="absolute top-0 bottom-0 left-1/2 w-px bg-cyan-400/30 pointer-events-none"></div>
            <div class="absolute left-0 right-1/2 top-1/2 h-px w-full bg-cyan-400/30 pointer-events-none"></div>

            <!-- Loading Spinner Ring -->
            <div
              v-if="loading"
              class="absolute inset-0 rounded-full border-2 border-transparent border-t-cyan-400 animate-spin pointer-events-none"
            ></div>

            <!-- North Indicator -->
            <div class="absolute top-3 font-mono font-bold text-[10px] text-cyan-400 bg-slate-950/80 px-1.5 py-0.5 rounded border border-cyan-500/30 pointer-events-none shadow">
              N
            </div>
            <!-- Center Reticle Marker -->
            <div class="absolute w-3 h-3 rounded-full border border-cyan-300 pointer-events-none flex items-center justify-center">
              <span class="w-1 h-1 rounded-full bg-cyan-300"></span>
            </div>
          </div>

          <!-- Compass Pan & Zoom Controls Bar -->
          <div class="mt-4 flex items-center gap-3 bg-slate-900/70 border border-slate-800 rounded-xl px-4 py-2">
            <!-- Pan Buttons -->
            <div class="flex items-center gap-1">
              <button
                class="w-7 h-7 rounded-lg bg-slate-800 hover:bg-slate-700 text-slate-300 hover:text-white flex items-center justify-center text-xs transition"
                :title="$t('radar.pan_west')"
                :disabled="loading"
                @click="pan(-128, 0)"
              >
                ◀
              </button>
              <div class="flex flex-col gap-1">
                <button
                  class="w-7 h-7 rounded-lg bg-slate-800 hover:bg-slate-700 text-slate-300 hover:text-white flex items-center justify-center text-xs transition"
                  :title="$t('radar.pan_north')"
                  :disabled="loading"
                  @click="pan(0, -128)"
                >
                  ▲
                </button>
                <button
                  class="w-7 h-7 rounded-lg bg-slate-800 hover:bg-slate-700 text-slate-300 hover:text-white flex items-center justify-center text-xs transition"
                  :title="$t('radar.pan_south')"
                  :disabled="loading"
                  @click="pan(0, 128)"
                >
                  ▼
                </button>
              </div>
              <button
                class="w-7 h-7 rounded-lg bg-slate-800 hover:bg-slate-700 text-slate-300 hover:text-white flex items-center justify-center text-xs transition"
                :title="$t('radar.pan_east')"
                :disabled="loading"
                @click="pan(128, 0)"
              >
                ▶
              </button>
            </div>

            <div class="h-6 w-px bg-slate-700 mx-1"></div>

            <!-- Reset to 0,0 -->
            <button
              class="px-2.5 py-1 rounded-lg bg-slate-800 hover:bg-slate-700 text-[11px] font-mono text-slate-300 hover:text-cyan-300 transition"
              :title="$t('radar.reset_spawn')"
              :disabled="loading"
              @click="resetCoords"
            >
              {{ $t('radar.spawn_button') }}
            </button>

            <div class="h-6 w-px bg-slate-700 mx-1"></div>

            <!-- Zoom Controls -->
            <div class="flex items-center gap-1">
              <button
                class="w-7 h-7 rounded-lg bg-slate-800 hover:bg-slate-700 text-slate-300 hover:text-white flex items-center justify-center text-sm font-bold transition"
                :title="$t('radar.zoom_out')"
                :disabled="loading || zoom <= 0.5"
                @click="adjustZoom(-0.5)"
              >
                −
              </button>
              <span class="text-xs font-mono text-cyan-400 w-9 text-center">{{ zoom }}x</span>
              <button
                class="w-7 h-7 rounded-lg bg-slate-800 hover:bg-slate-700 text-slate-300 hover:text-white flex items-center justify-center text-sm font-bold transition"
                :title="$t('radar.zoom_in')"
                :disabled="loading || zoom >= 4.0"
                @click="adjustZoom(0.5)"
              >
                +
              </button>
            </div>
          </div>
        </div>

        <!-- Telemetry & Controls Sidebar (Right) -->
        <div class="w-full md:w-80 flex flex-col gap-4">
          <!-- Seed & Target Source Box -->
          <div class="bg-slate-900/70 border border-slate-800 rounded-xl p-4 space-y-3">
            <div class="flex items-center justify-between">
              <label class="text-xs font-bold text-slate-300">{{ $t('radar.target_title') }}</label>
              <button
                v-if="activeSeed"
                class="text-[10px] text-cyan-400 hover:underline flex items-center gap-1"
                @click="copySeed"
              >
                <svg v-if="copied" class="w-3 h-3 text-cyan-400" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5">
                  <polyline points="20 6 9 17 4 12" />
                </svg>
                <svg v-else class="w-3 h-3" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                  <path d="M8 5H6a2 2 0 00-2 2v12a2 2 0 002 2h10a2 2 0 002-2v-1M8 5a2 2 0 002 2h2a2 2 0 002-2M8 5a2 2 0 012-2h2a2 2 0 012 2m0 0h2a2 2 0 012 2v3m2 4H10m0 0l3-3m-3 3l3 3" />
                </svg>
                <span>{{ copied ? $t('radar.copied') : $t('radar.copy_seed') }}</span>
              </button>
            </div>

            <!-- If instance has saves, allow picking a local world -->
            <div v-if="worldsList.length > 0" class="space-y-1">
              <label class="text-[10px] text-slate-400">{{ $t('radar.select_save') }}</label>
              <select
                v-model="selectedWorldFolder"
                class="z-input text-xs w-full py-1.5"
                @change="onWorldSelected"
              >
                <option value="">{{ $t('radar.procedural_seed_option') }}</option>
                <option v-for="w in worldsList" :key="w.folderName" :value="w.folderName">
                  {{ w.levelName }} ({{ w.folderName }})
                </option>
              </select>
            </div>

            <!-- Seed Input Field -->
            <div class="space-y-1">
              <label class="text-[10px] text-slate-400">{{ $t('radar.seed_input_label') }}</label>
              <div class="flex gap-2">
                <input
                  v-model="activeSeed"
                  type="text"
                  :placeholder="$t('radar.seed_placeholder')"
                  class="z-input flex-1 text-xs px-2.5 py-1.5 font-mono"
                  @keyup.enter="refreshRadar"
                />
                <button
                  class="px-2.5 py-1.5 rounded-lg bg-slate-800 hover:bg-slate-700 text-xs text-slate-300 hover:text-white transition flex items-center justify-center"
                  :title="$t('radar.random_seed_tooltip')"
                  @click="randomizeSeed"
                >
                  <svg class="w-3.5 h-3.5 text-slate-300" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                    <rect x="3" y="3" width="18" height="18" rx="3" />
                    <circle cx="8" cy="8" r="1.2" fill="currentColor" />
                    <circle cx="16" cy="16" r="1.2" fill="currentColor" />
                    <circle cx="12" cy="12" r="1.2" fill="currentColor" />
                  </svg>
                </button>
              </div>
            </div>

            <!-- Scan Button -->
            <button
              class="z-btn-primary w-full text-xs py-2 rounded-xl font-bold flex items-center justify-center gap-2"
              :disabled="loading"
              @click="refreshRadar"
            >
              <span v-if="loading" class="w-3.5 h-3.5 border-2 border-white border-t-transparent rounded-full animate-spin"></span>
              <svg v-else class="w-3.5 h-3.5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                <circle cx="12" cy="12" r="9" stroke-dasharray="3 2" />
                <circle cx="12" cy="12" r="3" fill="currentColor" fill-opacity="0.3" />
              </svg>
              <span>{{ loading ? $t('radar.rendering_radar') : $t('radar.scan_button') }}</span>
            </button>
          </div>

          <!-- Position Telemetry Box -->
          <div class="bg-slate-900/70 border border-slate-800 rounded-xl p-4 space-y-2.5 font-mono text-xs">
            <div class="text-[11px] font-bold text-slate-300 font-sans uppercase tracking-wider flex items-center gap-1.5">
              <span class="w-2 h-2 rounded-full bg-cyan-400 animate-pulse"></span>
              {{ $t('radar.telemetry_title') }}
            </div>

            <div class="grid grid-cols-2 gap-2 text-slate-400">
              <div class="bg-slate-950/60 p-2 rounded-lg border border-slate-800/80">
                <div class="text-[10px] text-slate-500">{{ $t('radar.center_x') }}</div>
                <div class="text-cyan-300 font-bold mt-0.5">{{ centerX }}</div>
              </div>
              <div class="bg-slate-950/60 p-2 rounded-lg border border-slate-800/80">
                <div class="text-[10px] text-slate-500">{{ $t('radar.center_z') }}</div>
                <div class="text-cyan-300 font-bold mt-0.5">{{ centerZ }}</div>
              </div>
              <div class="bg-slate-950/60 p-2 rounded-lg border border-slate-800/80">
                <div class="text-[10px] text-slate-500">{{ $t('radar.scan_radius') }}</div>
                <div class="text-slate-200 mt-0.5">{{ $t('radar.blocks_unit', { count: radarResult?.radiusBlocks || 384 }) }}</div>
              </div>
              <div class="bg-slate-950/60 p-2 rounded-lg border border-slate-800/80">
                <div class="text-[10px] text-slate-500">{{ $t('radar.elevation_range') }}</div>
                <div class="text-slate-200 mt-0.5">{{ $t('radar.elevation_format', { min: radarResult?.minElevation ?? '—', max: radarResult?.maxElevation ?? '—' }) }}</div>
              </div>
            </div>

            <div v-if="radarResult?.renderTimeMs" class="text-[10px] text-slate-500 text-right">
              {{ $t('radar.rendered_in', { ms: radarResult.renderTimeMs }) }}
            </div>
          </div>

          <!-- Discovered Biomes Box -->
          <div class="bg-slate-900/70 border border-slate-800 rounded-xl p-4 space-y-2 flex-1">
            <div class="text-[11px] font-bold text-slate-300 uppercase tracking-wider flex items-center justify-between">
              <span>{{ $t('radar.discovered_biomes') }}</span>
              <span class="text-[10px] text-cyan-400 font-mono">({{ radarResult?.biomesFound?.length || 0 }})</span>
            </div>

            <div v-if="radarResult?.biomesFound?.length" class="flex flex-wrap gap-1.5 max-h-36 overflow-y-auto pr-1">
              <span
                v-for="b in radarResult.biomesFound"
                :key="b"
                class="px-2 py-0.5 rounded-full text-[10px] font-mono border bg-slate-950/80 border-slate-800 text-slate-300 flex items-center gap-1.5 shadow-sm"
              >
                <span class="w-1.5 h-1.5 rounded-full" :class="getBiomeDotColor(b)"></span>
                {{ formatBiomeName(b) }}
              </span>
            </div>
            <div v-else class="text-xs text-slate-500 italic py-2">
              {{ $t('radar.no_biomes') }}
            </div>
          </div>
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

const loading = ref(false);
const radarResult = ref(null);
const activeSeed = ref('1337424269101');
const selectedWorldFolder = ref('');
const centerX = ref(0);
const centerZ = ref(0);
const zoom = ref(1.0);
const copied = ref(false);
const worldsList = ref([]);

function handleClose() {
  emit('close');
}

async function loadInstanceWorlds() {
  if (!props.instance?.id) return;
  try {
    const list = await api.listInstanceWorlds(props.instance.id);
    worldsList.value = list || [];
    if (worldsList.value.length > 0 && !selectedWorldFolder.value) {
      // Pick latest played world
      selectedWorldFolder.value = worldsList.value[0].folderName;
      if (worldsList.value[0].seed) {
        activeSeed.value = String(worldsList.value[0].seed);
      }
    }
  } catch (e) {
    console.warn('Could not load instance worlds for radar:', e);
  }
}

function onWorldSelected() {
  if (!selectedWorldFolder.value) return;
  const match = worldsList.value.find((w) => w.folderName === selectedWorldFolder.value);
  if (match && match.seed) {
    activeSeed.value = String(match.seed);
  }
  refreshRadar();
}

async function refreshRadar() {
  if (!props.instance?.id) return;
  loading.value = true;
  try {
    const res = await api.getWorldSeedRadarPreview(
      props.instance.id,
      selectedWorldFolder.value || null,
      activeSeed.value || null,
      centerX.value,
      centerZ.value,
      384,
      zoom.value
    );
    radarResult.value = res;
    if (res.seed && !activeSeed.value) {
      activeSeed.value = res.seed;
    }
  } catch (err) {
    window.dispatchEvent(
      new CustomEvent('zircon-status', { detail: `Radar scan error: ${err}` })
    );
  } finally {
    loading.value = false;
  }
}

function pan(dx, dz) {
  centerX.value += dx;
  centerZ.value += dz;
  refreshRadar();
}

function resetCoords() {
  centerX.value = 0;
  centerZ.value = 0;
  refreshRadar();
}

function adjustZoom(delta) {
  zoom.value = Math.max(0.5, Math.min(4.0, Number((zoom.value + delta).toFixed(1))));
  refreshRadar();
}

function randomizeSeed() {
  selectedWorldFolder.value = '';
  activeSeed.value = String(Math.floor(Math.random() * 9000000000000000) + 1000000000000000);
  refreshRadar();
}

function copySeed() {
  if (!activeSeed.value) return;
  navigator.clipboard.writeText(activeSeed.value);
  copied.value = true;
  setTimeout(() => {
    copied.value = false;
  }, 2000);
}

function formatBiomeName(biome) {
  return biome.replace(/^minecraft:/, '').replace(/^terralith:/, '').replace(/_/g, ' ');
}

function getBiomeDotColor(biome) {
  const b = biome.toLowerCase();
  if (b.includes('ocean') || b.includes('river') || b.includes('water')) return 'bg-cyan-400';
  if (b.includes('peak') || b.includes('snow') || b.includes('ice') || b.includes('frozen')) return 'bg-sky-200';
  if (b.includes('desert') || b.includes('badlands') || b.includes('savanna')) return 'bg-amber-400';
  if (b.includes('swamp') || b.includes('mangrove')) return 'bg-emerald-600';
  if (b.includes('jungle')) return 'bg-lime-400';
  if (b.includes('forest') || b.includes('taiga')) return 'bg-emerald-400';
  return 'bg-green-400';
}

watch(
  () => props.open,
  (isOpen) => {
    if (isOpen) {
      loadInstanceWorlds().then(() => refreshRadar());
    }
  }
);

onMounted(() => {
  if (props.open) {
    loadInstanceWorlds().then(() => refreshRadar());
  }
});
</script>

<style scoped>
@keyframes spinSlow {
  from {
    transform: rotate(0deg);
  }
  to {
    transform: rotate(360deg);
  }
}
.animate-spin-slow {
  animation: spinSlow 20s linear infinite;
}
</style>
