<template>
  <div
    v-if="open"
    class="fixed inset-0 z-50 flex items-center justify-center p-3 sm:p-5 bg-black/85 backdrop-blur-xl transition-all duration-300 select-none"
    @click.self="handleClose"
  >
    <!-- Modal Container -->
    <div
      class="w-full max-w-5xl bg-[#070b12]/95 border border-cyan-500/30 rounded-2xl shadow-2xl shadow-cyan-950/50 overflow-hidden flex flex-col animate-in fade-in zoom-in-95 duration-200 max-h-[92vh] ring-1 ring-white/10"
    >
      <!-- Header -->
      <div class="px-6 py-3.5 border-b border-slate-800/80 flex items-center justify-between bg-gradient-to-r from-slate-900/90 via-slate-900/60 to-cyan-950/40">
        <div class="flex items-center gap-3 min-w-0">
          <div class="w-9 h-9 rounded-xl bg-gradient-to-br from-cyan-500/20 to-cyan-950/40 border border-cyan-500/40 flex items-center justify-center text-cyan-400 shrink-0 shadow-[0_0_15px_rgba(6,182,212,0.25)]">
            <svg class="w-5 h-5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
              <circle cx="12" cy="12" r="9" stroke-dasharray="4 2" />
              <path d="M12 3v18M3 12h18" stroke-opacity="0.4" />
              <circle cx="12" cy="12" r="3" fill="currentColor" fill-opacity="0.3" />
            </svg>
          </div>
          <div class="min-w-0">
            <div class="flex items-center gap-2 flex-wrap">
              <h2 class="text-sm font-bold text-slate-100 tracking-wide truncate">
                {{ $t('radar.title') }}
              </h2>
              <span class="text-[10px] font-mono px-2 py-0.5 rounded-full bg-cyan-500/10 text-cyan-400 border border-cyan-500/25">
                {{ $t('radar.badge') || $t('radar.anvil_hud') || 'Anvil Slippy Map' }}
              </span>
              <span v-if="instance?.name" class="text-[10px] font-mono px-2 py-0.5 rounded-full bg-slate-800/80 text-slate-300 border border-slate-700">
                {{ instance.name }}
              </span>
            </div>
            <p class="text-[11px] text-slate-400 mt-0.5 truncate">{{ $t('radar.subtitle') }}</p>
          </div>
        </div>

        <button
          class="text-slate-400 hover:text-white p-1.5 rounded-lg hover:bg-slate-800/80 transition ml-2 shrink-0"
          :title="$t('common.close')"
          @click="handleClose"
        >
          <svg class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 18L18 6M6 6l12 12" />
          </svg>
        </button>
      </div>

      <!-- World Selection & Dimension Bar -->
      <div class="px-6 py-2.5 bg-slate-950/70 border-b border-slate-800/70 flex flex-wrap items-center justify-between gap-3 text-xs">
        <!-- Save Selector (Multi-World Architecture) -->
        <div class="flex items-center gap-2.5 flex-1 min-w-[240px]">
          <span class="text-slate-400 font-medium text-[11px] shrink-0 flex items-center gap-1.5">
            <svg class="w-3.5 h-3.5 text-cyan-400" fill="none" viewBox="0 0 24 24" stroke="currentColor">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M3.055 11H5a2 2 0 012 2v1a2 2 0 002 2 2 2 0 012 2v2.945M8 3.935V5.5A2.5 2.5 0 0010.5 8h.5a2 2 0 012 2 2 2 0 012 2v2.945M15 20.488V18a2 2 0 012-2h3.064M21 12a9 9 0 11-18 0 9 9 0 0118 0z" />
            </svg>
            World Save:
          </span>

          <select
            v-model="selectedWorldFolder"
            class="z-input text-xs py-1 px-2.5 bg-slate-900 border-slate-700 text-slate-200 flex-1 max-w-sm rounded-lg"
            @change="onWorldSelected"
          >
            <option value="">-- No World Save Selected --</option>
            <option v-for="w in worldsList" :key="w.folderName" :value="w.folderName">
              {{ w.levelName }} ({{ w.folderName }}) {{ w.gameType ? `• ${w.gameType}` : '' }}
            </option>
          </select>
        </div>

        <!-- Dimension Switcher (Overworld, Nether, End) -->
        <div class="flex items-center gap-1 bg-slate-900 border border-slate-800 rounded-lg p-0.5 shrink-0">
          <button
            type="button"
            class="px-2.5 py-1 rounded-md text-xs font-semibold flex items-center gap-1.5 transition"
            :class="selectedDimension === 'overworld' ? 'bg-emerald-500/20 text-emerald-300 border border-emerald-500/40 shadow-sm' : 'text-slate-400 hover:text-slate-200 border border-transparent'"
            @click="setDimension('overworld')"
          >
            <svg class="w-3.5 h-3.5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
              <circle cx="12" cy="12" r="10"></circle>
              <path d="M12 2a14.5 14.5 0 0 0 0 20 14.5 14.5 0 0 0 0-20"></path>
              <path d="M2 12h20"></path>
            </svg>
            <span>Overworld</span>
          </button>
          <button
            type="button"
            class="px-2.5 py-1 rounded-md text-xs font-semibold flex items-center gap-1.5 transition"
            :class="selectedDimension === 'the_nether' ? 'bg-red-500/20 text-red-300 border border-red-500/40 shadow-sm' : 'text-slate-400 hover:text-slate-200 border border-transparent'"
            @click="setDimension('the_nether')"
          >
            <svg class="w-3.5 h-3.5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
              <path d="M8.5 14.5A2.5 2.5 0 0 0 11 12c0-1.38-.5-2-1-3-1.072-2.143-.224-4.054 2-6 .5 2.5 2 4.9 4 6.5 2 1.6 3 3.5 3 5.5a7 7 0 1 1-14 0c0-1.153.433-2.294 1-3a2.5 2.5 0 0 0 2.5 2.5z"></path>
            </svg>
            <span>The Nether</span>
          </button>
          <button
            type="button"
            class="px-2.5 py-1 rounded-md text-xs font-semibold flex items-center gap-1.5 transition"
            :class="selectedDimension === 'the_end' ? 'bg-purple-500/20 text-purple-300 border border-purple-500/40 shadow-sm' : 'text-slate-400 hover:text-slate-200 border border-transparent'"
            @click="setDimension('the_end')"
          >
            <svg class="w-3.5 h-3.5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
              <polygon points="12 2 15.09 8.26 22 9.27 17 14.14 18.18 21.02 12 17.77 5.82 21.02 7 14.14 2 9.27 8.91 8.26 12 2"></polygon>
            </svg>
            <span>The End</span>
          </button>
        </div>

        <!-- Seed Display & Quick Copy -->
        <div class="flex items-center gap-2 shrink-0">
          <div class="flex items-center gap-1.5 bg-slate-900 border border-slate-800 rounded-lg px-2.5 py-1">
            <span class="text-[10px] text-slate-500 font-mono uppercase tracking-wider">Seed:</span>
            <span class="text-cyan-300 font-mono text-xs select-all">{{ activeSeed || 'Unknown' }}</span>
            <button
              v-if="activeSeed"
              class="text-slate-400 hover:text-cyan-300 p-0.5 rounded transition flex items-center ml-1"
              :title="copied ? 'Copied!' : 'Copy seed'"
              @click="copySeed"
            >
              <svg v-if="copied" class="w-3.5 h-3.5 text-emerald-400" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5">
                <polyline points="20 6 9 17 4 12" />
              </svg>
              <svg v-else class="w-3.5 h-3.5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                <path d="M8 5H6a2 2 0 00-2 2v12a2 2 0 002 2h10a2 2 0 002-2v-1M8 5a2 2 0 002 2h2a2 2 0 002-2M8 5a2 2 0 012-2h2a2 2 0 012 2m0 0h2a2 2 0 012 2v3m2 4H10m0 0l3-3m-3 3l3 3" />
              </svg>
            </button>
          </div>
        </div>
      </div>

      <!-- Main Body: Map Viewport (Left) + Telemetry / Biomes (Right) -->
      <div class="p-5 overflow-y-auto flex-1 grid grid-cols-1 lg:grid-cols-12 gap-5 items-start">
        <!-- Map Viewport Column (7 cols) -->
        <div class="lg:col-span-7 flex flex-col items-center gap-3">
          <!-- Slippy Canvas Viewport with 60 FPS Inertia Drag and Anchor Zoom -->
          <div
            ref="viewportRef"
            class="relative w-full aspect-square max-w-[480px] rounded-2xl border-2 overflow-hidden flex items-center justify-center select-none transition-colors duration-300"
            :class="[
              isDragging ? 'cursor-grabbing' : 'cursor-grab',
              selectedDimension === 'the_nether'
                ? 'border-red-500/40 bg-[#120608] shadow-[0_0_35px_rgba(239,68,68,0.2)]'
                : selectedDimension === 'the_end'
                ? 'border-purple-500/40 bg-[#080512] shadow-[0_0_35px_rgba(168,85,247,0.2)]'
                : 'border-cyan-500/40 bg-[#040810] shadow-[0_0_35px_rgba(6,182,212,0.15)]'
            ]"
            @mousedown="onMouseDown"
            @mousemove="onMouseMove"
            @mouseup="onMouseUp"
            @mouseleave="onMouseLeave"
            @wheel.prevent="onWheel"
            @dblclick="onDblClick"
          >
            <!-- 60 FPS Hardware-Accelerated Slippy Canvas -->
            <canvas
              ref="canvasRef"
              class="w-full h-full block select-none pointer-events-none"
            ></canvas>

            <!-- Coordinate Crosshairs (Subtle Optics) -->
            <div class="absolute inset-x-0 top-1/2 h-px bg-cyan-400/20 pointer-events-none"></div>
            <div class="absolute inset-y-0 left-1/2 w-px bg-cyan-400/20 pointer-events-none"></div>

            <!-- Optical Grid Ring Reticle -->
            <div class="absolute inset-8 rounded-full border border-cyan-500/10 pointer-events-none"></div>
            <div class="absolute inset-20 rounded-full border border-cyan-500/15 pointer-events-none"></div>

            <!-- Center Marker -->
            <div class="absolute w-3.5 h-3.5 rounded-full border border-cyan-300 pointer-events-none flex items-center justify-center shadow-[0_0_8px_rgba(6,182,212,0.8)]">
              <span class="w-1 h-1 rounded-full bg-cyan-300"></span>
            </div>

            <!-- North Compass Indicator -->
            <div class="absolute top-3 left-3 flex items-center gap-1.5 font-mono text-[10px] font-bold text-cyan-300 bg-slate-950/85 px-2 py-0.5 rounded-lg border border-cyan-500/30 pointer-events-none shadow">
              <span class="text-rose-400">▲</span> N
            </div>

            <!-- Center Coordinate & Hover Cursor Badge -->
            <div class="absolute top-3 right-3 flex flex-col items-end gap-1 pointer-events-none">
              <div class="font-mono text-[10px] text-slate-300 bg-slate-950/85 px-2 py-0.5 rounded-lg border border-slate-800 shadow">
                Center: X: <span class="text-cyan-300 font-bold">{{ Math.round(centerX) }}</span>, Z: <span class="text-cyan-300 font-bold">{{ Math.round(centerZ) }}</span>
              </div>
              <div v-if="hoverX !== null && hoverZ !== null" class="font-mono text-[10px] text-cyan-300 bg-slate-950/85 px-2 py-0.5 rounded-lg border border-cyan-500/30 shadow">
                Cursor: X: <span class="font-bold">{{ hoverX }}</span>, Z: <span class="font-bold">{{ hoverZ }}</span>
              </div>
            </div>

            <!-- Telemetry Footer Overlay -->
            <div class="absolute bottom-3 inset-x-3 flex items-center justify-between text-[10px] font-mono text-slate-400 bg-slate-950/80 backdrop-blur-md px-3 py-1 rounded-lg border border-slate-800/80 pointer-events-none">
              <span>View: <strong class="text-slate-200 capitalize">{{ selectedDimension.replace('the_', '') }}</strong></span>
              <span class="text-slate-500 hidden sm:inline">Drag to pan • Scroll to zoom</span>
              <span>Zoom: <strong class="text-cyan-400">{{ Number(zoom.toFixed(1)) }}x</strong></span>
            </div>
          </div>

          <!-- Clean Toolbar (Arrows, Player and Spawn buttons removed as requested) -->
          <div class="flex items-center gap-2 bg-slate-900/80 border border-slate-800 rounded-xl px-3 py-1.5 shadow-sm">
            <!-- Reset to Origin (0,0) -->
            <button
              class="px-2.5 py-1 rounded-lg bg-slate-800 hover:bg-slate-700 text-[11px] font-mono text-slate-300 hover:text-white transition flex items-center gap-1.5"
              title="Reset center to world origin (0, 0)"
              @click="resetToOrigin"
            >
              <span>(0, 0)</span>
            </button>

            <!-- Recenter on World Spawn / Exploration Center -->
            <button
              class="px-2.5 py-1 rounded-lg bg-slate-800 hover:bg-slate-700 text-[11px] font-mono text-amber-300 hover:text-white transition flex items-center gap-1.5"
              title="Recenter view on explored spawn area"
              @click="recenterOnActive"
            >
              <svg class="w-3 h-3" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                <circle cx="12" cy="12" r="10" />
                <line x1="12" y1="2" x2="12" y2="6" />
                <line x1="12" y1="18" x2="12" y2="22" />
                <line x1="2" y1="12" x2="6" y2="12" />
                <line x1="18" y1="12" x2="22" y2="12" />
              </svg>
              <span>Recenter</span>
            </button>

            <div class="h-5 w-px bg-slate-800 mx-1"></div>

            <!-- Zoom In / Out Controls -->
            <div class="flex items-center gap-1">
              <button
                class="w-7 h-7 rounded-lg bg-slate-800 hover:bg-slate-700 text-slate-300 hover:text-white flex items-center justify-center text-sm font-bold transition"
                title="Zoom Out"
                :disabled="zoom <= 0.25"
                @click="adjustZoom(-0.25)"
              >
                −
              </button>
              <span class="text-xs font-mono text-cyan-400 w-10 text-center">{{ Number(zoom.toFixed(1)) }}x</span>
              <button
                class="w-7 h-7 rounded-lg bg-slate-800 hover:bg-slate-700 text-slate-300 hover:text-white flex items-center justify-center text-sm font-bold transition"
                title="Zoom In"
                :disabled="zoom >= 5.0"
                @click="adjustZoom(0.25)"
              >
                +
              </button>
            </div>
          </div>
        </div>

        <!-- Telemetry & Discovered Biomes Column (5 cols) -->
        <div class="lg:col-span-5 flex flex-col gap-3.5 w-full">
          <!-- Active Target Card -->
          <div class="bg-slate-900/60 border border-slate-800/90 rounded-xl p-3.5 space-y-2.5">
            <div class="flex items-center justify-between">
              <span class="text-[11px] font-bold text-slate-300 uppercase tracking-wider flex items-center gap-1.5">
                <span class="w-2 h-2 rounded-full bg-cyan-400 animate-pulse"></span>
                Target Telemetry
              </span>
              <span class="text-[10px] font-mono px-2 py-0.5 rounded bg-cyan-500/10 text-cyan-400 border border-cyan-500/20">
                Live Anvil Tiles
              </span>
            </div>

            <!-- Active Save Details -->
            <div v-if="activeWorldInfo" class="bg-slate-950/70 p-2.5 rounded-lg border border-slate-800/80 flex items-center gap-3">
              <div class="w-10 h-10 rounded-lg bg-slate-900 border border-slate-800 shrink-0 overflow-hidden flex items-center justify-center">
                <img v-if="activeWorldInfo.iconDataUrl" :src="activeWorldInfo.iconDataUrl" class="w-full h-full object-cover" />
                <svg v-else class="w-5 h-5 text-slate-600" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                  <path stroke-linecap="round" stroke-linejoin="round" stroke-width="1.5" d="M3.055 11H5a2 2 0 012 2v1a2 2 0 002 2 2 2 0 012 2v2.945M8 3.935V5.5A2.5 2.5 0 0010.5 8h.5a2 2 0 012 2 2 2 0 012 2v2.945M15 20.488V18a2 2 0 012-2h3.064M21 12a9 9 0 11-18 0 9 9 0 0118 0z" />
                </svg>
              </div>
              <div class="min-w-0 flex-1">
                <div class="flex items-center gap-1.5 flex-wrap">
                  <span class="font-bold text-xs text-white truncate">{{ activeWorldInfo.levelName }}</span>
                  <span class="text-[9px] px-1.5 py-0.2 rounded font-bold bg-cyan-500/20 text-cyan-300 border border-cyan-500/30">
                    {{ activeWorldInfo.hardcore ? 'Hardcore' : activeWorldInfo.gameType }}
                  </span>
                  <span v-if="activeWorldInfo.minecraftVersion" class="text-[9px] px-1.5 py-0.2 rounded font-mono bg-slate-800 text-slate-300">
                    {{ activeWorldInfo.minecraftVersion }}
                  </span>
                </div>
                <div class="text-[10px] text-slate-400 font-mono truncate mt-0.5">saves/{{ activeWorldInfo.folderName }}</div>
              </div>
            </div>

            <!-- Coordinate & Elevation Stats Grid -->
            <div class="grid grid-cols-2 gap-2 text-xs font-mono">
              <div class="bg-slate-950/60 p-2 rounded-lg border border-slate-800/70">
                <div class="text-[10px] text-slate-500 font-sans">Center (X, Z)</div>
                <div class="text-cyan-300 font-bold mt-0.5">{{ Math.round(centerX) }}, {{ Math.round(centerZ) }}</div>
              </div>
              <div class="bg-slate-950/60 p-2 rounded-lg border border-slate-800/70">
                <div class="text-[10px] text-slate-500 font-sans">Elevation (Y)</div>
                <div class="text-slate-200 mt-0.5">
                  {{ (radarResult?.minElevation ?? radarResult?.min_elevation) != null ? `${radarResult?.minElevation ?? radarResult?.min_elevation} to ${radarResult?.maxElevation ?? radarResult?.max_elevation}` : '—' }}
                </div>
              </div>
            </div>

            <!-- Spawn & Player Coordinates Details -->
            <div v-if="activeWorldInfo && (activeWorldInfo.playerX != null || activeWorldInfo.spawnX != null)" class="grid grid-cols-2 gap-2 text-[10px] font-mono">
              <div v-if="activeWorldInfo.playerX != null" class="bg-slate-950/40 px-2 py-1.5 rounded-lg border border-slate-800/50">
                <span class="text-slate-500">Player: </span>
                <span class="text-cyan-300 font-bold">X: {{ Math.round(activeWorldInfo.playerX) }}, Z: {{ Math.round(activeWorldInfo.playerZ) }}</span>
              </div>
              <div v-if="activeWorldInfo.spawnX != null" class="bg-slate-950/40 px-2 py-1.5 rounded-lg border border-slate-800/50">
                <span class="text-slate-500">Spawn: </span>
                <span class="text-amber-300 font-bold">X: {{ activeWorldInfo.spawnX }}, Z: {{ activeWorldInfo.spawnZ }}</span>
              </div>
            </div>
          </div>

          <!-- Discovered Biomes Card -->
          <div class="bg-slate-900/60 border border-slate-800/90 rounded-xl p-3.5 space-y-2 flex-1 flex flex-col">
            <div class="flex items-center justify-between">
              <span class="text-[11px] font-bold text-slate-300 uppercase tracking-wider">Discovered Biomes</span>
              <span class="text-[10px] text-cyan-400 font-mono">({{ biomesList.length }})</span>
            </div>

            <div v-if="biomesList.length" class="flex flex-wrap gap-1.5 max-h-48 overflow-y-auto pr-1">
              <span
                v-for="b in biomesList"
                :key="b"
                class="px-2 py-0.5 rounded-full text-[10px] font-mono border bg-slate-950/80 border-slate-800 text-slate-300 flex items-center gap-1.5 shadow-sm"
              >
                <span class="w-1.5 h-1.5 rounded-full" :class="getBiomeDotColor(b)"></span>
                {{ formatBiomeName(b) }}
              </span>
            </div>
            <div v-else class="text-xs text-slate-500 italic py-4 text-center">
              {{ loading ? 'Analyzing biomes in region files…' : 'No biomes indexed for current view.' }}
            </div>
          </div>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup>
import { computed, ref, watch, onMounted, onUnmounted } from 'vue';
import { api } from '../lib/api';

const props = defineProps({
  open: { type: Boolean, default: false },
  instance: { type: Object, default: null },
  initialWorldFolder: { type: String, default: null },
});

const emit = defineEmits(['close']);

const loading = ref(false);
const radarResult = ref(null);
const activeSeed = ref('1337424269101');
const selectedWorldFolder = ref('');
const selectedDimension = ref('overworld');
const centerX = ref(0);
const centerZ = ref(0);
const zoom = ref(1.0);
const copied = ref(false);
const worldsList = ref([]);

// Dimension camera positions memory
const dimensionCenters = ref({
  overworld: null,
  the_nether: null,
  the_end: null,
});

// Slippy Canvas Tile Engine (Matching server web app)
const viewportRef = ref(null);
const canvasRef = ref(null);
const isDragging = ref(false);
const hoverX = ref(null);
const hoverZ = ref(null);

let lastClientX = 0;
let lastClientY = 0;
let mapVelocityX = 0;
let mapVelocityZ = 0;
let rafId = null;

const mapTileCache = new Map();
const mapPendingTiles = new Set();

function getInitialCoordinatesForDimension(match, dim) {
  if (!match) return { x: 0, z: 0 };
  const playerDim = match.playerDimension || '';

  if (dim === 'overworld') {
    if (match.spawnX != null && match.spawnZ != null) {
      return { x: match.spawnX, z: match.spawnZ };
    }
    if (playerDim.endsWith('overworld') && match.playerX != null && match.playerZ != null) {
      return { x: Math.round(match.playerX), z: Math.round(match.playerZ) };
    }
    return { x: 0, z: 0 };
  } else if (dim === 'the_nether') {
    if (playerDim.endsWith('the_nether') && match.playerX != null && match.playerZ != null) {
      return { x: Math.round(match.playerX), z: Math.round(match.playerZ) };
    }
    return { x: 0, z: 0 };
  } else if (dim === 'the_end') {
    if (playerDim.endsWith('the_end') && match.playerX != null && match.playerZ != null) {
      return { x: Math.round(match.playerX), z: Math.round(match.playerZ) };
    }
    return { x: 0, z: 0 };
  }
  return { x: 0, z: 0 };
}

function onMouseDown(e) {
  if (e.button === 0 || e.button === 1) {
    isDragging.value = true;
    lastClientX = e.clientX;
    lastClientY = e.clientY;
    mapVelocityX = 0;
    mapVelocityZ = 0;
  }
}

function onMouseMove(e) {
  if (!viewportRef.value) return;
  const rect = viewportRef.value.getBoundingClientRect();
  const mouseX = e.clientX - rect.left;
  const mouseY = e.clientY - rect.top;
  const curZoom = zoom.value || 1.0;

  hoverX.value = Math.round(centerX.value + (mouseX - rect.width / 2) / curZoom);
  hoverZ.value = Math.round(centerZ.value + (mouseY - rect.height / 2) / curZoom);

  if (isDragging.value) {
    const dx = e.clientX - lastClientX;
    const dy = e.clientY - lastClientY;

    centerX.value -= Math.round(dx / curZoom);
    centerZ.value -= Math.round(dy / curZoom);

    mapVelocityX = dx;
    mapVelocityZ = dy;

    lastClientX = e.clientX;
    lastClientY = e.clientY;
  }
}

function onMouseUp() {
  if (isDragging.value) {
    isDragging.value = false;
    scheduleTelemetryScan();
  }
}

function onMouseLeave() {
  hoverX.value = null;
  hoverZ.value = null;
  if (isDragging.value) {
    isDragging.value = false;
    scheduleTelemetryScan();
  }
}

function onWheel(e) {
  if (!viewportRef.value) return;
  const rect = viewportRef.value.getBoundingClientRect();
  const mouseX = e.clientX - rect.left;
  const mouseY = e.clientY - rect.top;

  const curZoom = zoom.value || 1.0;
  const cursorWorldX = centerX.value + (mouseX - rect.width / 2) / curZoom;
  const cursorWorldZ = centerZ.value + (mouseY - rect.height / 2) / curZoom;

  const factor = e.deltaY < 0 ? 1.25 : 0.8;
  const newZoom = Math.max(0.2, Math.min(6.0, Number((curZoom * factor).toFixed(2))));

  // Anchor zoom directly at mouse cursor coordinate
  centerX.value = Math.round(cursorWorldX - (mouseX - rect.width / 2) / newZoom);
  centerZ.value = Math.round(cursorWorldZ - (mouseY - rect.height / 2) / newZoom);
  zoom.value = newZoom;

  scheduleTelemetryScan();
}

function onDblClick(e) {
  if (!viewportRef.value) return;
  const rect = viewportRef.value.getBoundingClientRect();
  const mouseX = e.clientX - rect.left;
  const mouseY = e.clientY - rect.top;

  const curZoom = zoom.value || 1.0;
  const cursorWorldX = centerX.value + (mouseX - rect.width / 2) / curZoom;
  const cursorWorldZ = centerZ.value + (mouseY - rect.height / 2) / curZoom;

  const newZoom = Math.min(curZoom * 1.5, 6.0);
  centerX.value = Math.round(cursorWorldX);
  centerZ.value = Math.round(cursorWorldZ);
  zoom.value = parseFloat(newZoom.toFixed(2));

  scheduleTelemetryScan();
}

function resetToOrigin() {
  centerX.value = 0;
  centerZ.value = 0;
  mapVelocityX = 0;
  mapVelocityZ = 0;
  scheduleTelemetryScan();
}

function recenterOnActive() {
  const coords = getInitialCoordinatesForDimension(activeWorldInfo.value, selectedDimension.value);
  centerX.value = coords.x;
  centerZ.value = coords.z;
  mapVelocityX = 0;
  mapVelocityZ = 0;
  scheduleTelemetryScan();
}

function adjustZoom(delta) {
  const newZoom = Math.max(0.2, Math.min(6.0, Number((zoom.value + delta).toFixed(2))));
  zoom.value = newZoom;
  scheduleTelemetryScan();
}

const activeWorldInfo = computed(() => {
  if (!selectedWorldFolder.value) return null;
  return worldsList.value.find((w) => w.folderName === selectedWorldFolder.value) || null;
});

const biomesList = computed(() => {
  return radarResult.value?.biomesFound || radarResult.value?.biomes_found || [];
});

function handleClose() {
  emit('close');
}

function setDimension(dim) {
  if (selectedDimension.value === dim) return;

  // Save current dimension coordinates
  dimensionCenters.value[selectedDimension.value] = { x: centerX.value, z: centerZ.value };
  selectedDimension.value = dim;

  if (dimensionCenters.value[dim]) {
    centerX.value = dimensionCenters.value[dim].x;
    centerZ.value = dimensionCenters.value[dim].z;
  } else {
    const coords = getInitialCoordinatesForDimension(activeWorldInfo.value, dim);
    centerX.value = coords.x;
    centerZ.value = coords.z;
    dimensionCenters.value[dim] = { ...coords };
  }

  scheduleTelemetryScan();
}

async function loadInstanceWorlds() {
  if (!props.instance?.id) return;
  try {
    const list = await api.listInstanceWorlds(props.instance.id);
    worldsList.value = list || [];

    if (props.initialWorldFolder && worldsList.value.some((w) => w.folderName === props.initialWorldFolder)) {
      selectedWorldFolder.value = props.initialWorldFolder;
    } else if (worldsList.value.length > 0 && !selectedWorldFolder.value) {
      selectedWorldFolder.value = worldsList.value[0].folderName;
    }

    if (selectedWorldFolder.value) {
      const match = worldsList.value.find((w) => w.folderName === selectedWorldFolder.value);
      if (match) {
        if (match.seed) {
          activeSeed.value = String(match.seed);
        }
        const coords = getInitialCoordinatesForDimension(match, selectedDimension.value);
        centerX.value = coords.x;
        centerZ.value = coords.z;
        dimensionCenters.value[selectedDimension.value] = { ...coords };
      }
    }
  } catch (e) {
    console.warn('Could not load instance worlds for radar:', e);
  }
}

function onWorldSelected() {
  if (selectedWorldFolder.value) {
    const match = worldsList.value.find((w) => w.folderName === selectedWorldFolder.value);
    if (match) {
      if (match.seed) {
        activeSeed.value = String(match.seed);
      }
      const coords = getInitialCoordinatesForDimension(match, selectedDimension.value);
      centerX.value = coords.x;
      centerZ.value = coords.z;
      dimensionCenters.value[selectedDimension.value] = { ...coords };
    }
  } else {
    centerX.value = 0;
    centerZ.value = 0;
  }
  scheduleTelemetryScan();
}

let telemetryDebounceTimer = null;
function scheduleTelemetryScan() {
  if (telemetryDebounceTimer) clearTimeout(telemetryDebounceTimer);
  telemetryDebounceTimer = setTimeout(() => {
    refreshTelemetry();
  }, 350);
}

async function refreshTelemetry() {
  if (!props.instance?.id) return;
  loading.value = true;
  try {
    const res = await api.getWorldSeedRadarPreview(
      props.instance.id,
      selectedWorldFolder.value || null,
      activeSeed.value || null,
      Math.round(centerX.value),
      Math.round(centerZ.value),
      384,
      zoom.value,
      selectedDimension.value
    );
    radarResult.value = res;
    if (res.seed && !activeSeed.value) {
      activeSeed.value = res.seed;
    }
  } catch (err) {
    console.warn('Telemetry scan error:', err);
  } finally {
    loading.value = false;
  }
}

async function fetchRegionTile(tileKey, rx, rz) {
  if (!props.instance?.id) return;
  mapPendingTiles.add(tileKey);

  try {
    const dataUrl = await api.getInstanceWorldRegionTile(
      props.instance.id,
      selectedWorldFolder.value || null,
      selectedDimension.value,
      rx,
      rz
    );

    if (dataUrl) {
      const img = new Image();
      img.onload = () => {
        mapTileCache.set(tileKey, img);
        mapPendingTiles.delete(tileKey);
      };
      img.onerror = () => {
        mapTileCache.set(tileKey, 'none');
        mapPendingTiles.delete(tileKey);
      };
      img.src = dataUrl;
    } else {
      mapTileCache.set(tileKey, 'none');
      mapPendingTiles.delete(tileKey);
    }
  } catch (err) {
    mapTileCache.set(tileKey, 'none');
    mapPendingTiles.delete(tileKey);
  }
}

function renderWorldMapFrame() {
  if (!props.open) return;
  const canvas = canvasRef.value;
  if (!canvas) {
    rafId = requestAnimationFrame(renderWorldMapFrame);
    return;
  }

  const rect = canvas.getBoundingClientRect();
  if (rect.width === 0 || rect.height === 0) {
    rafId = requestAnimationFrame(renderWorldMapFrame);
    return;
  }

  const dpr = window.devicePixelRatio || 1;
  const targetW = Math.round(rect.width * dpr);
  const targetH = Math.round(rect.height * dpr);

  if (canvas.width !== targetW || canvas.height !== targetH) {
    canvas.width = targetW;
    canvas.height = targetH;
  }

  const ctx = canvas.getContext('2d');
  ctx.setTransform(dpr, 0, 0, dpr, 0, 0);

  const width = rect.width;
  const height = rect.height;

  // Apply smooth inertia momentum
  if (!isDragging.value && (Math.abs(mapVelocityX) > 0.05 || Math.abs(mapVelocityZ) > 0.05)) {
    const curZoom = zoom.value || 1.0;
    centerX.value = Math.round(centerX.value - mapVelocityX / curZoom);
    centerZ.value = Math.round(centerZ.value - mapVelocityZ / curZoom);
    mapVelocityX *= 0.91;
    mapVelocityZ *= 0.91;
  }

  const cx = centerX.value;
  const cz = centerZ.value;
  const curZoom = zoom.value || 1.0;

  // 1. Dark void canvas background
  const voidColor = selectedDimension.value === 'the_nether'
    ? '#120608'
    : selectedDimension.value === 'the_end'
    ? '#080512'
    : '#040810';
  ctx.fillStyle = voidColor;
  ctx.fillRect(0, 0, width, height);

  // 2. Visible world bounds
  const halfW = width / (2 * curZoom);
  const halfH = height / (2 * curZoom);
  const minWorldX = cx - halfW;
  const maxWorldX = cx + halfW;
  const minWorldZ = cz - halfH;
  const maxWorldZ = cz + halfH;

  const minRx = Math.floor(minWorldX / 512);
  const maxRx = Math.floor(maxWorldX / 512);
  const minRz = Math.floor(minWorldZ / 512);
  const maxRz = Math.floor(maxWorldZ / 512);

  // 3. Render region tiles
  const dim = selectedDimension.value;
  const folder = selectedWorldFolder.value || 'none';
  const instId = props.instance?.id || 'default';

  for (let rx = minRx; rx <= maxRx; rx++) {
    for (let rz = minRz; rz <= maxRz; rz++) {
      const tileKey = `${instId}:${folder}:${dim}:${rx},${rz}`;
      const tileWorldX = rx * 512;
      const tileWorldZ = rz * 512;

      const screenX = width / 2 + (tileWorldX - cx) * curZoom;
      const screenY = height / 2 + (tileWorldZ - cz) * curZoom;
      const tileSize = 512 * curZoom;

      const cached = mapTileCache.get(tileKey);

      if (cached && cached !== 'none') {
        ctx.imageSmoothingEnabled = curZoom < 1.0;
        ctx.drawImage(cached, screenX, screenY, tileSize, tileSize);
      } else {
        if (cached === undefined && !mapPendingTiles.has(tileKey)) {
          fetchRegionTile(tileKey, rx, rz);
        }

        // Subtly outline unexplored chunk space
        const gridColor = dim === 'the_nether'
          ? 'rgba(239, 68, 68, 0.08)'
          : dim === 'the_end'
          ? 'rgba(168, 85, 247, 0.08)'
          : 'rgba(6, 182, 212, 0.08)';
        ctx.strokeStyle = gridColor;
        ctx.lineWidth = 1;
        ctx.strokeRect(screenX, screenY, tileSize, tileSize);
      }
    }
  }

  // 4. World Origin (0,0) indicator crosshair
  const originScreenX = width / 2 + (0 - cx) * curZoom;
  const originScreenY = height / 2 + (0 - cz) * curZoom;
  if (originScreenX >= -20 && originScreenX <= width + 20 && originScreenY >= -20 && originScreenY <= height + 20) {
    ctx.strokeStyle = 'rgba(239, 68, 68, 0.7)';
    ctx.lineWidth = 1.5;
    ctx.beginPath();
    ctx.arc(originScreenX, originScreenY, 6, 0, Math.PI * 2);
    ctx.stroke();
    ctx.fillStyle = 'rgba(239, 68, 68, 0.9)';
    ctx.beginPath();
    ctx.arc(originScreenX, originScreenY, 2, 0, Math.PI * 2);
    ctx.fill();
  }

  // 5. World Spawn indicator (in Overworld)
  const info = activeWorldInfo.value;
  if (dim === 'overworld' && info?.spawnX != null && info?.spawnZ != null) {
    const spawnScreenX = width / 2 + (info.spawnX - cx) * curZoom;
    const spawnScreenY = height / 2 + (info.spawnZ - cz) * curZoom;
    if (spawnScreenX >= -20 && spawnScreenX <= width + 20 && spawnScreenY >= -20 && spawnScreenY <= height + 20) {
      ctx.fillStyle = '#f59e0b';
      ctx.strokeStyle = '#fef3c7';
      ctx.lineWidth = 1.5;
      ctx.beginPath();
      ctx.arc(spawnScreenX, spawnScreenY, 5, 0, Math.PI * 2);
      ctx.fill();
      ctx.stroke();
    }
  }

  // 6. Player position indicator
  const isPlayerInDim = info?.playerDimension
    ? info.playerDimension.endsWith(dim)
    : (dim === 'overworld');
  if (isPlayerInDim && info?.playerX != null && info?.playerZ != null) {
    const pxScreen = width / 2 + (info.playerX - cx) * curZoom;
    const pzScreen = height / 2 + (info.playerZ - cz) * curZoom;
    if (pxScreen >= -20 && pxScreen <= width + 20 && pzScreen >= -20 && pzScreen <= height + 20) {
      ctx.fillStyle = '#06b6d4';
      ctx.strokeStyle = '#cffafe';
      ctx.lineWidth = 2;
      ctx.beginPath();
      ctx.arc(pxScreen, pzScreen, 5, 0, Math.PI * 2);
      ctx.fill();
      ctx.stroke();

      const pulseTime = (Date.now() % 2000) / 2000;
      const pulseR = 5 + pulseTime * 12;
      const pulseAlpha = 1.0 - pulseTime;
      ctx.strokeStyle = `rgba(6, 182, 212, ${pulseAlpha * 0.7})`;
      ctx.lineWidth = 1.5;
      ctx.beginPath();
      ctx.arc(pxScreen, pzScreen, pulseR, 0, Math.PI * 2);
      ctx.stroke();
    }
  }

  rafId = requestAnimationFrame(renderWorldMapFrame);
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
      if (props.initialWorldFolder) {
        selectedWorldFolder.value = props.initialWorldFolder;
      }
      loadInstanceWorlds().then(() => scheduleTelemetryScan());
      if (rafId) cancelAnimationFrame(rafId);
      rafId = requestAnimationFrame(renderWorldMapFrame);
    } else {
      if (rafId) cancelAnimationFrame(rafId);
      rafId = null;
      if (telemetryDebounceTimer) clearTimeout(telemetryDebounceTimer);
    }
  }
);

onMounted(() => {
  if (props.open) {
    if (props.initialWorldFolder) {
      selectedWorldFolder.value = props.initialWorldFolder;
    }
    loadInstanceWorlds().then(() => scheduleTelemetryScan());
    if (rafId) cancelAnimationFrame(rafId);
    rafId = requestAnimationFrame(renderWorldMapFrame);
  }
});

onUnmounted(() => {
  if (rafId) cancelAnimationFrame(rafId);
  rafId = null;
  if (telemetryDebounceTimer) clearTimeout(telemetryDebounceTimer);
});
</script>

<style scoped>
</style>
