<template>
  <Transition name="overlay-fade">
    <div
      v-if="visible"
      class="fixed inset-0 z-50 flex items-center justify-center bg-[#03070d]/85 backdrop-blur-2xl px-4 py-6 overflow-y-auto select-none"
      role="dialog"
      aria-modal="true"
      aria-labelledby="launch-dialog-title"
    >
      <!-- Server Background Banner (Cinematic blurred backdrop) -->
      <div
        v-if="bannerSource"
        class="absolute inset-0 overflow-hidden pointer-events-none select-none z-0"
      >
        <img
          :src="bannerSource"
          alt=""
          class="w-full h-full object-cover scale-110 blur-3xl saturate-[1.65] brightness-[0.3] transition-all duration-700 ease-out"
          @error="onBannerLoadError"
        />
        <!-- Vignette & Deep Obsidian Gradient Overlays -->
        <div class="absolute inset-0 bg-gradient-to-t from-[#03070d] via-[#03070d]/70 to-[#03070d]/80" />
        <div class="absolute inset-0 bg-[radial-gradient(circle_at_center,transparent_10%,rgba(3,7,13,0.85)_80%)]" />
      </div>

      <!-- Ambient Background Glow Orbs -->
      <div
        class="absolute -top-32 -left-32 w-96 h-96 rounded-full blur-3xl pointer-events-none animate-pulse z-0"
        :class="bannerSource ? 'bg-teal-500/10' : 'bg-teal-500/20'"
      />
      <div
        class="absolute -bottom-32 -right-32 w-96 h-96 rounded-full blur-3xl pointer-events-none animate-pulse z-0"
        :class="bannerSource ? 'bg-emerald-500/10' : 'bg-emerald-500/15'"
      />

      <!-- Main Launch Card with Dynamic Dual-Pane Width -->
      <div
        class="relative z-10 w-full overflow-hidden rounded-3xl border border-teal-500/30 bg-[#09111c]/95 p-6 sm:p-7 shadow-[0_25px_80px_rgba(0,0,0,0.85),0_0_50px_rgba(71,210,201,0.12)] backdrop-blur-2xl transition-all duration-500 ease-[cubic-bezier(0.16,1,0.3,1)]"
        :class="show2048 ? 'max-w-[940px]' : 'max-w-[580px]'"
      >
        <!-- Top Animated Holographic Accent Line -->
        <div class="absolute inset-x-0 top-0 h-1 bg-gradient-to-r from-teal-500 via-cyan-300 to-emerald-400 animate-shimmer" />

        <!-- Card Header Banner Specular Ambient Reflection -->
        <div
          v-if="bannerSource"
          class="absolute inset-x-0 top-0 h-28 overflow-hidden pointer-events-none opacity-25 z-0"
        >
          <img
            :src="bannerSource"
            alt=""
            class="w-full h-full object-cover blur-sm"
          />
          <div class="absolute inset-0 bg-gradient-to-b from-transparent via-[#09111c]/60 to-[#09111c]" />
        </div>

        <!-- Header: Branding, Title, Minigame Toggle, Stop/Close Actions -->
        <div class="relative z-10 flex items-start justify-between gap-4 mb-6 pb-4 border-b border-white/5">
          <div class="flex items-center gap-3.5 min-w-0">
            <!-- Server or Launcher Icon -->
            <div class="relative shrink-0 flex h-11 w-11 items-center justify-center rounded-2xl bg-gradient-to-br from-[#0e2130] to-[#08121c] border border-teal-500/35 shadow-inner">
              <img
                v-if="server?.icon || server?.iconUrl"
                :src="server?.icon || server?.iconUrl"
                class="h-7 w-7 object-contain rounded-lg"
                alt="Server Icon"
              />
              <svg
                v-else
                class="h-6 w-6 text-teal-300"
                fill="none"
                stroke="currentColor"
                viewBox="0 0 24 24"
              >
                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="1.8" d="M14 10l-2 1m0 0l-2-1m2 1v2.5M20 7l-2 1m2-1l-2-1m2 1v2.5M14 4l-2-1-2 1M4 7l2-1M4 7l2 1M4 7v2.5M12 21l-2-1m2 1l2-1m-2 1v-2.5M6 18l-2-1v-2.5M18 18l2-1v-2.5" />
              </svg>
              <span
                class="absolute -bottom-1 -right-1 h-3.5 w-3.5 rounded-full border-2 border-[#09111c]"
                :class="running ? 'bg-emerald-400 shadow-[0_0_8px_#34d399]' : 'bg-teal-400 animate-pulse shadow-[0_0_8px_#2dd4bf]'"
              />
            </div>

            <div class="min-w-0">
              <div class="flex items-center gap-2">
                <img
                  :src="zirconTitle"
                  alt="Zircon"
                  class="h-6 w-auto max-w-[130px] select-none object-contain drop-shadow-[0_0_8px_rgba(71,210,201,0.3)]"
                  draggable="false"
                />
                <span
                  class="inline-flex items-center gap-1 rounded-full border px-2 py-0.5 text-[9px] font-bold uppercase tracking-wider"
                  :class="running
                    ? 'border-emerald-500/40 bg-emerald-500/15 text-emerald-300 shadow-[0_0_10px_rgba(52,211,153,0.2)]'
                    : 'border-teal-500/40 bg-teal-500/15 text-teal-300 shadow-[0_0_10px_rgba(45,212,191,0.2)]'"
                >
                  <span
                    class="h-1.5 w-1.5 rounded-full"
                    :class="running ? 'bg-emerald-400' : 'bg-teal-400 animate-ping'"
                  />
                  {{ running ? 'Online' : 'Booting' }}
                </span>
              </div>
              <h2 id="launch-dialog-title" class="mt-1 text-lg font-extrabold tracking-tight text-white truncate">
                {{ server?.name || (running ? 'Game Running' : 'Starting Minecraft') }}
              </h2>
              <p v-if="server?.address" class="text-[11px] font-mono text-teal-400/85 truncate">
                {{ server.address }}
              </p>
            </div>
          </div>

          <!-- Header Actions -->
          <div class="flex items-center gap-2 shrink-0">
            <!-- 2048 Minigame Toggle Button -->
            <button
              type="button"
              class="relative inline-flex items-center gap-1.5 rounded-xl border px-3 py-1.5 text-xs font-bold transition-all duration-200"
              :class="show2048
                ? 'border-teal-400/60 bg-teal-500/20 text-teal-200 shadow-[0_0_14px_rgba(71,210,201,0.35)]'
                : 'border-white/10 bg-white/5 text-slate-300 hover:border-teal-500/40 hover:text-teal-200 hover:bg-teal-500/10'"
              :title="show2048 ? 'Hide 2048 Minigame' : 'Play 2048 while Minecraft loads'"
              @click="toggle2048"
            >
              <svg class="h-3.5 w-3.5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                <rect x="2" y="6" width="20" height="12" rx="4" />
                <path d="M6 12h4m-2-2v4m7-2h.01m3-2h.01" stroke-linecap="round" />
              </svg>
              <span>2048</span>
              <span
                v-if="!show2048"
                class="hidden sm:inline-block rounded-full bg-teal-400/20 px-1.5 py-0.2 text-[9px] font-black text-teal-300 border border-teal-400/30 uppercase"
              >
                Play
              </span>
            </button>

            <!-- Dismiss Error Button -->
            <button
              v-if="error"
              type="button"
              class="inline-flex items-center gap-1 rounded-xl border border-white/10 bg-white/5 px-3 py-1.5 text-xs font-semibold text-slate-300 transition hover:bg-white/10 hover:text-white"
              @click="$emit('close')"
            >
              Dismiss
            </button>

            <!-- Stop/Cancel Launch Button -->
            <button
              type="button"
              class="group inline-flex items-center gap-1.5 rounded-xl border border-rose-500/30 bg-rose-500/10 px-3 py-1.5 text-xs font-bold text-rose-300 transition-all hover:border-rose-500/60 hover:bg-rose-500/20 active:scale-95 shadow-sm"
              title="Cancel launch and stop process"
              :disabled="stopping"
              @click="cancelLaunch"
            >
              <svg
                v-if="!stopping"
                class="h-3.5 w-3.5 transition-transform group-hover:scale-110"
                fill="none"
                stroke="currentColor"
                viewBox="0 0 24 24"
              >
                <rect x="6" y="6" width="12" height="12" rx="2" stroke-width="2.2" />
              </svg>
              <svg
                v-else
                class="h-3.5 w-3.5 animate-spin text-rose-300"
                viewBox="0 0 24 24"
                fill="none"
              >
                <circle class="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" stroke-width="3" />
                <path class="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8v8H4z" />
              </svg>
              <span>{{ stopping ? 'Stopping...' : 'Stop' }}</span>
            </button>
          </div>
        </div>

        <!-- Card Body: Single column or Dual-Pane Grid with 2048 -->
        <div
          class="relative z-10 transition-all duration-500 ease-[cubic-bezier(0.16,1,0.3,1)]"
          :class="show2048 ? 'grid grid-cols-1 md:grid-cols-12 gap-6 items-start' : 'flex flex-col'"
        >
          <!-- Left Column: Launch Sequence & Status -->
          <div :class="show2048 ? 'md:col-span-6 flex flex-col justify-between' : 'flex flex-col'">
            <!-- Launch Error State Banner -->
            <div
              v-if="error"
              class="mb-6 rounded-2xl border border-rose-500/40 bg-rose-950/40 p-4 text-rose-200 shadow-lg shadow-rose-950/40"
            >
              <div class="flex items-center gap-2 text-xs font-extrabold text-rose-400">
                <svg class="h-4 w-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                  <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 8v4m0 4h.01M21 12a9 9 0 11-18 0 9 9 0 0118 0z" />
                </svg>
                <span>Launch Failed</span>
              </div>
              <p class="mt-2 text-xs leading-relaxed text-rose-200/90 break-words font-mono bg-rose-950/60 p-2.5 rounded-xl border border-rose-900/50">
                {{ error }}
              </p>
            </div>

            <!-- High-Tech Shader Engine Opt-In Card (Clean 2026 aesthetics without emojis) -->
            <div
              v-else-if="shaderPrompt"
              class="mb-6 rounded-2xl border border-amber-500/35 bg-gradient-to-b from-amber-950/40 to-[#120e06]/80 p-5 text-amber-200 shadow-xl shadow-black/40 backdrop-blur-md"
            >
              <div class="flex items-center justify-between gap-2 mb-2">
                <div class="flex items-center gap-2 text-xs font-black uppercase tracking-wider text-amber-300">
                  <span class="flex h-6 w-6 items-center justify-center rounded-lg bg-amber-500/20 text-amber-300 border border-amber-500/40">
                    <svg class="h-3.5 w-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                      <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M13 10V3L4 14h7v7l9-11h-7z" />
                    </svg>
                  </span>
                  <span>Shader Engine Recommended</span>
                </div>
                <span class="rounded-full bg-amber-500/15 border border-amber-500/30 px-2 py-0.5 text-[9px] font-extrabold text-amber-300 uppercase">
                  Iris / Oculus
                </span>
              </div>

              <div class="my-3 rounded-xl bg-black/40 border border-amber-500/20 p-3">
                <div class="flex items-center justify-between">
                  <span class="text-xs font-bold text-white">
                    {{ shaderPrompt.shaderName || 'Iris Recommended Shaders' }}
                  </span>
                  <span v-if="shaderPrompt.shaderAuthor" class="text-[10px] text-amber-300/80 font-medium">
                    by {{ shaderPrompt.shaderAuthor }}
                  </span>
                </div>
                <p class="mt-1.5 text-[11px] text-amber-100/80 leading-relaxed">
                  This server provides high-performance shader enhancements for realistic lighting, water refractions, and atmospheric skyboxes.
                </p>
              </div>

              <!-- Remember Choice Toggle -->
              <label class="flex items-center gap-2.5 text-xs text-amber-200/90 cursor-pointer my-3 select-none">
                <input
                  type="checkbox"
                  v-model="rememberShaders"
                  class="rounded border-amber-500/40 bg-amber-950/50 text-amber-400 focus:ring-0 focus:ring-offset-0 cursor-pointer h-4 w-4"
                />
                <span class="font-medium text-[11px]">Remember my preference for this server</span>
              </label>

              <!-- Actions: Skip or Enable (Clean SVG icons, no emojis) -->
              <div class="flex items-center justify-end gap-2.5 mt-4 pt-3 border-t border-amber-500/15">
                <button
                  type="button"
                  class="rounded-xl border border-white/15 bg-white/5 px-3.5 py-1.5 text-xs font-semibold text-slate-300 transition hover:bg-white/10 hover:text-white"
                  @click="submitShaderChoice(false)"
                >
                  Play Without Shaders
                </button>
                <button
                  type="button"
                  class="rounded-xl bg-gradient-to-r from-amber-500 via-amber-400 to-yellow-500 px-4 py-1.5 text-xs font-black text-slate-950 shadow-md shadow-amber-500/30 hover:brightness-110 active:scale-95 transition-all flex items-center gap-1.5"
                  @click="submitShaderChoice(true)"
                >
                  <svg class="h-3.5 w-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                    <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2.2" d="M13 10V3L4 14h7v7l9-11h-7z" />
                  </svg>
                  <span>Enable Shaders</span>
                </button>
              </div>
            </div>

            <!-- Standard Launch Lifecycle View -->
            <template v-else>
              <!-- 2026 Stage Timeline Pipeline with Isolated Interconnects (no line through checkboxes) -->
              <div class="relative mb-6 rounded-2xl bg-[#070c14]/80 border border-teal-500/15 p-4 shadow-inner">
                <div class="grid grid-cols-5 gap-1 items-start">
                  <div
                    v-for="(st, idx) in stages"
                    :key="idx"
                    class="relative flex flex-col items-center text-center transition-all duration-300"
                    :class="idx <= currentStageIndex ? 'opacity-100' : 'opacity-40'"
                  >
                    <!-- Connector line to next node (stops before entering circle boundaries) -->
                    <div
                      v-if="idx < stages.length - 1"
                      class="absolute top-4 left-[calc(50%+18px)] right-[calc(-50%+18px)] h-0.5 pointer-events-none z-0"
                    >
                      <!-- Inactive track -->
                      <div class="w-full h-full bg-slate-800/80 rounded-full overflow-hidden">
                        <!-- Active filled track -->
                        <div
                          class="h-full bg-gradient-to-r from-teal-400 to-emerald-400 transition-all duration-500"
                          :style="{ width: idx < currentStageIndex ? '100%' : '0%' }"
                        />
                      </div>
                    </div>

                    <!-- Stage Node Badge (Opaque background so nothing can ever show through the checkmark) -->
                    <div
                      class="relative z-10 flex h-8 w-8 items-center justify-center rounded-full text-xs font-extrabold transition-all duration-300 shadow-md"
                      :class="stageBadgeClass(idx)"
                    >
                      <!-- Completed stage -->
                      <svg v-if="idx < currentStageIndex" class="h-4 w-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="3" d="M5 13l4 4L19 7" />
                      </svg>
                      <!-- Active stage -->
                      <span v-else-if="idx === currentStageIndex" class="relative flex h-2.5 w-2.5">
                        <span class="animate-ping absolute inline-flex h-full w-full rounded-full bg-teal-400 opacity-75" />
                        <span class="relative inline-flex rounded-full h-2.5 w-2.5 bg-teal-300" />
                      </span>
                      <!-- Future stage -->
                      <span v-else class="text-slate-500 font-mono text-[11px]">{{ idx + 1 }}</span>
                    </div>

                    <span
                      class="mt-2 text-[11px] font-bold leading-tight"
                      :class="idx === currentStageIndex ? 'text-teal-300' : (idx < currentStageIndex ? 'text-slate-200' : 'text-slate-400')"
                    >
                      {{ st.name }}
                    </span>
                    <span class="text-[9px] text-slate-500 font-mono mt-0.5">
                      {{ st.subtitle }}
                    </span>
                  </div>
                </div>
              </div>

              <!-- Rotating Tip / Insights Card -->
              <div class="mb-5 min-h-[78px] rounded-2xl border border-teal-500/15 bg-[#060b12]/80 p-3.5 shadow-inner">
                <Transition name="tip-fade" mode="out-in">
                  <div :key="activeTipIndex">
                    <div class="flex items-center justify-between">
                      <div class="flex items-center gap-1.5 text-[10px] font-black uppercase tracking-wider text-teal-300">
                        <svg class="h-3.5 w-3.5 text-teal-400" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M13 16h-1v-4h-1m1-4h.01M21 12a9 9 0 11-18 0 9 9 0 0118 0z" />
                        </svg>
                        <span>{{ tips[activeTipIndex].headline }}</span>
                      </div>
                      <!-- Pips -->
                      <div class="flex items-center gap-1">
                        <span
                          v-for="(_, tIdx) in tips"
                          :key="tIdx"
                          class="h-1 rounded-full transition-all duration-300"
                          :class="tIdx === activeTipIndex ? 'w-3.5 bg-teal-400' : 'w-1 bg-white/20'"
                        />
                      </div>
                    </div>
                    <p class="mt-1.5 text-[11px] leading-relaxed text-[#b4c4cb]">
                      {{ tips[activeTipIndex].message }}
                    </p>
                  </div>
                </Transition>
              </div>

              <!-- Progress Bar & Status Readout -->
              <div class="space-y-2">
                <div class="flex items-center justify-between text-xs font-semibold">
                  <span class="truncate text-slate-200 pr-2 flex items-center gap-2">
                    <span class="h-2 w-2 rounded-full bg-teal-400" :class="{ 'animate-ping': !running }" />
                    <span class="truncate">{{ status || (running ? 'Minecraft is active' : 'Preparing launch environment...') }}</span>
                  </span>
                  <span v-if="progress !== null" class="shrink-0 font-mono font-black text-teal-300 text-sm">
                    {{ Math.round(progress * 100) }}%
                  </span>
                  <span v-else class="shrink-0 text-teal-400/80 animate-pulse font-mono text-[11px] font-bold">
                    {{ running ? 'ACTIVE' : 'SYNCING' }}
                  </span>
                </div>

                <!-- Luminous Shimmer Progress Bar -->
                <div class="h-2.5 w-full overflow-hidden rounded-full bg-[#0a121c] ring-1 ring-inset ring-teal-500/20 p-0.5">
                  <div
                    class="h-full rounded-full bg-gradient-to-r from-teal-400 via-cyan-300 to-emerald-400 transition-all duration-300 ease-out shadow-[0_0_12px_rgba(71,210,201,0.5)]"
                    :class="{ 'indeterminate-bar': progress === null && !running }"
                    :style="{ width: running ? '100%' : (progress === null ? '35%' : `${Math.min(100, Math.max(5, Math.round(progress * 100)))}%`) }"
                  />
                </div>
              </div>
            </template>

            <!-- Minigame Teaser Banner (shown only when 2048 is not expanded) -->
            <div
              v-if="!show2048"
              class="mt-5 rounded-2xl border border-teal-500/20 bg-gradient-to-r from-teal-950/30 via-[#0a1624]/60 to-[#071018]/80 p-3 flex items-center justify-between gap-3 shadow-md"
            >
              <div class="flex items-center gap-2.5">
                <div class="flex h-8 w-8 items-center justify-center rounded-xl bg-teal-500/15 border border-teal-400/30 text-teal-300">
                  <svg class="h-4 w-4" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                    <rect x="2" y="6" width="20" height="12" rx="4" />
                    <path d="M6 12h4m-2-2v4m7-2h.01m3-2h.01" stroke-linecap="round" />
                  </svg>
                </div>
                <div>
                  <div class="text-xs font-bold text-white">Play 2048 While Waiting</div>
                  <div class="text-[10px] text-slate-400">Pass the time with tiles while Minecraft launches</div>
                </div>
              </div>
              <button
                type="button"
                class="shrink-0 z-btn-accent text-[11px] font-black px-3 py-1.5 rounded-xl shadow-md flex items-center gap-1.5 active:scale-95 transition-all"
                @click="toggle2048"
              >
                <span>Play Now</span>
                <svg class="w-3 h-3" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                  <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2.5" d="M14 5l7 7m0 0l-7 7m7-7H3" />
                </svg>
              </button>
            </div>
          </div>

          <!-- Right Column: Embedded 2048 Minigame -->
          <div
            v-if="show2048"
            class="md:col-span-6 flex flex-col h-full border-t md:border-t-0 md:border-l border-white/5 pt-4 md:pt-0 md:pl-6 animate-fade-in"
          >
            <Game2048
              :embedded="true"
              @close="toggle2048"
            />
          </div>
        </div>
      </div>
    </div>
  </Transition>
</template>

<script setup>
import { computed, onBeforeUnmount, onMounted, ref, watch } from 'vue';
import { api } from '../lib/api';
import zirconTitle from '../assets/zircon-title.svg';
import Game2048 from './Game2048.vue';

const props = defineProps({
  visible: {
    type: Boolean,
    default: false,
  },
  status: {
    type: String,
    default: '',
  },
  progress: {
    type: Number,
    default: null,
  },
  running: {
    type: Boolean,
    default: false,
  },
  gameLabel: {
    type: String,
    default: '',
  },
  error: {
    type: String,
    default: '',
  },
  server: {
    type: Object,
    default: null,
  },
  shaderPrompt: {
    type: Object,
    default: null,
  },
});

const emit = defineEmits(['close', 'shader-choice']);

const stopping = ref(false);
const activeTipIndex = ref(0);
const rememberShaders = ref(false);
const show2048 = ref(false);
const bannerFailed = ref(false);

let tipInterval = null;
let expandTimer = null;
let stageStepTimer = null;
const currentStageIndex = ref(0);

// Server background banner computation
const bannerSource = computed(() => {
  if (bannerFailed.value) return null;
  return props.server?.bannerUrl || props.server?.banner || null;
});

function onBannerLoadError() {
  bannerFailed.value = true;
}

watch(
  () => props.server?.bannerUrl || props.server?.banner,
  () => {
    bannerFailed.value = false;
  }
);

const stages = [
  { name: 'Auth', subtitle: 'Profile', key: 'auth' },
  { name: 'Runtime', subtitle: 'Java VM', key: 'java' },
  { name: 'Packs & Mods', subtitle: 'Sync & Hash', key: 'sync' },
  { name: 'Server Boot', subtitle: 'Wakeup', key: 'server' },
  { name: 'Spawn', subtitle: 'Launching', key: 'game' },
];

const tips = [
  {
    headline: 'High-Speed Mod Verification',
    message: 'Zircon verifies mod SHA-1 hashes directly against Modrinth & CurseForge so you never crash from server desyncs.',
  },
  {
    headline: 'Isolated Server Environments',
    message: 'Each server has its own dedicated directory. Configs, shaders, and mods never conflict with your other worlds.',
  },
  {
    headline: 'Instant Idle Wakeup',
    message: 'Sleeping servers automatically spin up the moment you click Play. Zircon waits until the world finishes loading.',
  },
  {
    headline: 'Iris Shader Cache',
    message: 'Compiled shaders are cached locally on NVMe/SSD, significantly reducing first-time stutter and chunk loading delays.',
  },
];

function computeTargetStage() {
  const s = (props.status || '').toLowerCase();
  if (
    props.running ||
    s.includes('starting minecraft') ||
    s.includes('minecraft process') ||
    s.includes('starting offline') ||
    s.includes('game running') ||
    s.includes('minecraft is active') ||
    s.includes('playing')
  ) {
    return 4;
  }
  if (
    s.includes('waking up') ||
    s.includes('finish booting') ||
    s.includes('server is waking') ||
    s.includes('pre-join intent') ||
    (s.includes('server') && (s.includes('waiting') || s.includes('booting') || s.includes('wake') || s.includes('ping')))
  ) {
    return 3;
  }
  if (
    s.includes('mod') ||
    s.includes('pack') ||
    s.includes('hash') ||
    s.includes('sync') ||
    s.includes('staging') ||
    s.includes('bom') ||
    s.includes('downloading') ||
    s.includes('shader') ||
    s.includes('resourcepack')
  ) {
    return 2;
  }
  if (
    s.includes('java') ||
    s.includes('runtime') ||
    s.includes('classpath') ||
    s.includes('librar') ||
    s.includes('jvm')
  ) {
    return 1;
  }
  return 0;
}

function advanceStagesSequentially() {
  clearTimeout(stageStepTimer);
  const target = computeTargetStage();
  if (target > currentStageIndex.value) {
    currentStageIndex.value++;
    if (currentStageIndex.value < target) {
      stageStepTimer = setTimeout(advanceStagesSequentially, 180);
    }
  }
}

// When launch overlay becomes visible (user hits Play):
// Start compact for the first frame, and within the first second (at ~200ms),
// automatically unfold into the dual-pane 2048 minigame with smooth animation!
watch(
  () => props.visible,
  (isOpen) => {
    clearTimeout(expandTimer);
    clearTimeout(stageStepTimer);
    if (isOpen) {
      currentStageIndex.value = 0;
      bannerFailed.value = false;
      show2048.value = false;
      expandTimer = setTimeout(() => {
        if (props.visible) {
          show2048.value = true;
        }
      }, 200);
      advanceStagesSequentially();
    } else {
      currentStageIndex.value = 0;
      show2048.value = false;
    }
  },
  { immediate: true }
);

watch(
  [() => props.status, () => props.running],
  () => {
    if (props.visible) {
      advanceStagesSequentially();
    }
  }
);

function stageBadgeClass(idx) {
  if (idx < currentStageIndex.value) {
    return 'bg-[#091f28] text-teal-300 border border-teal-500/70 shadow-[0_0_10px_rgba(71,210,201,0.25)]';
  }
  if (idx === currentStageIndex.value) {
    return 'bg-[#0c2937] text-teal-200 border border-teal-400 ring-4 ring-teal-400/20 shadow-[0_0_15px_rgba(71,210,201,0.45)]';
  }
  return 'bg-[#08111c] text-slate-500 border border-slate-800';
}

function toggle2048() {
  show2048.value = !show2048.value;
}

function submitShaderChoice(enabled) {
  emit('shader-choice', {
    enabled: Boolean(enabled),
    remember: Boolean(rememberShaders.value),
  });
}

async function cancelLaunch() {
  if (stopping.value) return;
  stopping.value = true;
  try {
    await api.stopGame();
  } catch (err) {
    console.error('Failed to cancel launch:', err);
  } finally {
    stopping.value = false;
  }
}

onMounted(() => {
  tipInterval = setInterval(() => {
    activeTipIndex.value = (activeTipIndex.value + 1) % tips.length;
  }, 4800);
});

onBeforeUnmount(() => {
  if (tipInterval) {
    clearInterval(tipInterval);
  }
  clearTimeout(expandTimer);
  clearTimeout(stageStepTimer);
});
</script>

<style scoped>
.overlay-fade-enter-active,
.overlay-fade-leave-active {
  transition: opacity 220ms cubic-bezier(0.16, 1, 0.3, 1), transform 220ms cubic-bezier(0.16, 1, 0.3, 1);
}

.overlay-fade-enter-from,
.overlay-fade-leave-to {
  opacity: 0;
  transform: scale(0.97);
}

.tip-fade-enter-active,
.tip-fade-leave-active {
  transition: opacity 220ms ease, transform 220ms ease;
}

.tip-fade-enter-from {
  opacity: 0;
  transform: translateY(4px);
}

.tip-fade-leave-to {
  opacity: 0;
  transform: translateY(-4px);
}

.indeterminate-bar {
  animation: indeterminate 1.5s infinite cubic-bezier(0.65, 0.815, 0.735, 0.395);
}

@keyframes indeterminate {
  0% {
    transform: translateX(-100%);
  }
  50% {
    transform: translateX(120%);
  }
  100% {
    transform: translateX(300%);
  }
}

@keyframes shimmer {
  0% {
    background-position: 0% 50%;
  }
  50% {
    background-position: 100% 50%;
  }
  100% {
    background-position: 0% 50%;
  }
}

.animate-shimmer {
  background-size: 200% 200%;
  animation: shimmer 4s ease infinite;
}

@keyframes fadeIn {
  from {
    opacity: 0;
    transform: translateY(6px);
  }
  to {
    opacity: 1;
    transform: translateY(0);
  }
}

.animate-fade-in {
  animation: fadeIn 0.3s cubic-bezier(0.16, 1, 0.3, 1) forwards;
}
</style>