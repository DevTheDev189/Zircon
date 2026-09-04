<template>
  <Transition name="overlay-fade">
    <div
      v-if="visible"
      class="fixed inset-0 z-50 flex items-center justify-center bg-[#05090e]/85 backdrop-blur-lg px-4"
      role="dialog"
      aria-modal="true"
      aria-labelledby="launch-dialog-title"
    >
      <div class="relative w-full max-w-[580px] overflow-hidden rounded-2xl border border-teal-500/25 bg-[#0e161f]/95 p-6 sm:p-8 shadow-2xl shadow-black/80">
        <!-- Top accent gradient stripe -->
        <div class="absolute inset-x-0 top-0 h-1 bg-gradient-to-r from-teal-500 via-teal-300 to-emerald-400" />

        <!-- Header -->
        <div class="flex items-start justify-between gap-4 mb-6">
          <div>
            <div class="flex items-center gap-2">
              <img
                :src="zirconTitle"
                alt="Zircon"
                class="h-8 w-auto max-w-[170px] select-none object-contain"
                draggable="false"
              />
              <span class="inline-flex items-center gap-1 rounded-full border border-teal-500/30 bg-teal-500/10 px-2.5 py-0.5 text-[10px] font-semibold uppercase tracking-wider text-teal-300">
                <span class="h-1.5 w-1.5 rounded-full bg-teal-400 animate-ping" />
                {{ running ? 'Running' : 'Booting' }}
              </span>
            </div>
            <h2 id="launch-dialog-title" class="mt-2 text-xl font-bold tracking-tight text-white">
              {{ server?.name || (running ? 'Game Running' : 'Starting Minecraft') }}
            </h2>
            <p v-if="server?.address" class="text-xs font-mono text-teal-400/80">
              {{ server.address }}
            </p>
          </div>

          <div class="flex items-center gap-2">
            <button
              v-if="error"
              type="button"
              class="inline-flex items-center gap-1 rounded-lg border border-white/10 bg-white/5 px-3 py-1.5 text-xs font-medium text-slate-300 transition hover:bg-white/10"
              @click="$emit('close')"
            >
              Dismiss
            </button>
            <button
              type="button"
              class="group inline-flex items-center gap-1.5 rounded-lg border border-red-500/25 bg-red-500/10 px-3 py-1.5 text-xs font-semibold text-red-300 transition-colors hover:border-red-500/50 hover:bg-red-500/20 active:scale-95"
              title="Cancel launch and stop process"
              :disabled="stopping"
              @click="cancelLaunch"
            >
              <svg
                class="h-3.5 w-3.5"
                fill="none"
                stroke="currentColor"
                viewBox="0 0 24 24"
              >
                <rect x="6" y="6" width="12" height="12" rx="2" stroke-width="2" />
              </svg>
              <span>{{ stopping ? 'Stopping...' : 'Stop' }}</span>
            </button>
          </div>
        </div>

        <!-- Error State Banner -->
        <div v-if="error" class="mb-6 rounded-xl border border-red-500/30 bg-red-950/40 p-4 text-red-200">
          <div class="flex items-center gap-2 text-xs font-bold text-red-400">
            <svg class="h-4 w-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 8v4m0 4h.01M21 12a9 9 0 11-18 0 9 9 0 0118 0z" />
            </svg>
            <span>Launch Failed</span>
          </div>
          <p class="mt-1.5 text-xs leading-relaxed text-red-200/90 break-words font-mono">
            {{ error }}
          </p>
        </div>

        <!-- Shader Prompt Card (if required) -->
        <div v-else-if="shaderPrompt" class="mb-6 rounded-xl border border-amber-500/30 bg-amber-950/40 p-4 text-amber-200">
          <div class="flex items-center gap-2 text-xs font-bold text-amber-300">
            <svg class="h-4 w-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M13 10V3L4 14h7v7l9-11h-7z" />
            </svg>
            <span>Shader Engine Recommendation</span>
          </div>
          <p class="mt-1.5 text-xs text-amber-200/90 leading-relaxed">
            The server recommends installing an Iris/Oculus shader engine. Would you like to enable it?
          </p>
          <div class="mt-3 flex items-center justify-end gap-2">
            <button
              type="button"
              class="rounded-lg border border-white/10 px-3 py-1.5 text-xs font-medium text-slate-300 hover:bg-white/5"
              @click="$emit('shader-choice', false)"
            >
              Skip
            </button>
            <button
              type="button"
              class="rounded-lg bg-amber-500/20 border border-amber-500/40 px-3 py-1.5 text-xs font-semibold text-amber-200 hover:bg-amber-500/30"
              @click="$emit('shader-choice', true)"
            >
              Enable Shaders
            </button>
          </div>
        </div>

        <template v-else>
          <!-- Stage Timeline Checklist -->
          <div class="mb-6 grid grid-cols-4 gap-2 border-y border-white/5 py-4">
            <div
              v-for="(st, idx) in stages"
              :key="idx"
              class="flex flex-col items-center text-center transition-all duration-300"
              :class="idx <= currentStageIndex ? 'opacity-100' : 'opacity-40'"
            >
              <div
                class="flex h-7 w-7 items-center justify-center rounded-full text-xs font-bold transition-all duration-300"
                :class="stageBadgeClass(idx)"
              >
                <svg v-if="idx < currentStageIndex" class="h-3.5 w-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                  <path stroke-linecap="round" stroke-linejoin="round" stroke-width="3" d="M5 13l4 4L19 7" />
                </svg>
                <span v-else-if="idx === currentStageIndex" class="relative flex h-2 w-2">
                  <span class="animate-ping absolute inline-flex h-full w-full rounded-full bg-teal-400 opacity-75" />
                  <span class="relative inline-flex rounded-full h-2 w-2 bg-teal-300" />
                </span>
                <span v-else>{{ idx + 1 }}</span>
              </div>
              <span class="mt-1.5 text-[11px] font-medium leading-tight text-[#a0b3bc]">
                {{ st.name }}
              </span>
            </div>
          </div>

          <!-- Rotating Community Tip Banner -->
          <div class="mb-6 min-h-[76px] rounded-xl border border-white/5 bg-[#090e14]/60 p-3.5">
            <Transition name="tip-fade" mode="out-in">
              <div :key="activeTipIndex">
                <div class="flex items-center gap-1.5 text-[10px] font-bold uppercase tracking-wider text-teal-400">
                  <svg class="h-3 w-3" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                    <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M13 16h-1v-4h-1m1-4h.01M21 12a9 9 0 11-18 0 9 9 0 0118 0z" />
                  </svg>
                  <span>{{ tips[activeTipIndex].headline }}</span>
                </div>
                <p class="mt-1 text-xs leading-relaxed text-[#b4c4cb]">
                  {{ tips[activeTipIndex].message }}
                </p>
              </div>
            </Transition>
          </div>

          <!-- Progress and Status Section -->
          <div class="space-y-2">
            <div class="flex items-center justify-between text-xs font-medium">
              <span class="truncate text-[#d5e2e6] pr-2">
                {{ status || (running ? 'Game is running' : 'Initializing launch sequence...') }}
              </span>
              <span v-if="progress !== null" class="shrink-0 font-mono font-bold text-teal-300">
                {{ Math.round(progress * 100) }}%
              </span>
              <span v-else class="shrink-0 text-teal-400/80 animate-pulse font-mono text-[11px]">
                {{ running ? 'Active' : 'Synchronizing' }}
              </span>
            </div>

            <!-- Progress Track -->
            <div class="h-2 w-full overflow-hidden rounded-full bg-[#18232c] ring-1 ring-inset ring-white/5">
              <div
                class="h-full rounded-full bg-gradient-to-r from-teal-400 via-teal-300 to-emerald-400 transition-all duration-300 ease-out"
                :class="{ 'indeterminate-bar': progress === null && !running }"
                :style="{ width: running ? '100%' : (progress === null ? '30%' : `${Math.min(100, Math.max(4, Math.round(progress * 100)))}%`) }"
              />
            </div>
          </div>
        </template>
      </div>
    </div>
  </Transition>
</template>

<script setup>
import { computed, onBeforeUnmount, onMounted, ref } from 'vue';
import { api } from '../lib/api';
import zirconTitle from '../assets/zircon-title.svg';

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

defineEmits(['close', 'shader-choice']);

const stopping = ref(false);
const activeTipIndex = ref(0);
let tipInterval = null;

const stages = [
  { name: 'Auth', key: 'auth' },
  { name: 'Runtime', key: 'java' },
  { name: 'Packs & Mods', key: 'sync' },
  { name: 'Spawn', key: 'game' },
];

const tips = [
  {
    headline: 'Seamless Mod Sync',
    message: 'Zircon verifies mod SHA-1 hashes directly against Modrinth & CurseForge so you never crash from desyncs.',
  },
  {
    headline: 'Per-Server Isolation',
    message: 'Each server has its own dedicated game directory. Configs, shaders, and mods never interfere with other servers.',
  },
  {
    headline: 'Idle Wakeup',
    message: 'Sleeping servers wake up automatically when you click Play. Zircon waits until the world finishes loading.',
  },
];

const currentStageIndex = computed(() => {
  const s = (props.status || '').toLowerCase();
  if (props.running || s.includes('starting minecraft') || s.includes('launching') || s.includes('starting offline')) {
    return 3;
  }
  if (s.includes('mod') || s.includes('bom') || s.includes('pack') || s.includes('hash') || s.includes('download')) {
    return 2;
  }
  if (s.includes('java') || s.includes('runtime') || s.includes('classpath') || s.includes('libraries')) {
    return 1;
  }
  return 0;
});

function stageBadgeClass(idx) {
  if (idx < currentStageIndex.value) {
    return 'bg-teal-500/20 text-teal-300 border border-teal-500/40';
  }
  if (idx === currentStageIndex.value) {
    return 'bg-teal-500/30 text-teal-200 border border-teal-400 ring-2 ring-teal-400/20';
  }
  return 'bg-white/5 text-white/30 border border-white/5';
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
});
</script>

<style scoped>
.overlay-fade-enter-active,
.overlay-fade-leave-active {
  transition: opacity 200ms ease, transform 200ms ease;
}

.overlay-fade-enter-from,
.overlay-fade-leave-to {
  opacity: 0;
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
</style>