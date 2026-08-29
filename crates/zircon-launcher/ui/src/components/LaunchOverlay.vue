<template>
  <div
    v-if="visible"
    class="absolute inset-0 z-40 flex items-center justify-center bg-[#070b0f]/85 px-5 backdrop-blur-md"
    role="dialog"
    aria-modal="true"
    aria-labelledby="launch-overlay-title"
  >
    <div class="z-card launch-shell relative w-full max-w-[560px] overflow-hidden rounded-2xl border border-slate-700/60 shadow-2xl shadow-black/60 p-0 bg-[#0e1622]">
      <!-- 16:9 Hero Wallpaper Mode: Full Modal Backdrop with Vibrant Light Gradient & Soft Blur -->
      <div
        v-if="isHeroBanner && !bannerFailed"
        class="absolute inset-0 overflow-hidden pointer-events-none z-0"
      >
        <img
          :src="server.bannerUrl"
          class="w-full h-full object-cover opacity-60 transition-opacity duration-300 scale-105"
          :alt="server.name || 'Hero Backdrop'"
          @error="bannerFailed = true"
          @load="onBannerLoad"
        />
        <div class="absolute inset-0 bg-gradient-to-br from-[#070b10]/90 via-[#0e1622]/80 to-[#070b10]/85 backdrop-blur-[0.5px]"></div>
      </div>
      <div class="launch-grid absolute inset-0 pointer-events-none"></div>
      <div class="relative z-10 p-6 sm:p-8">
        <div class="mb-6 flex items-start justify-between gap-4">
          <div>
            <img
              :src="zirconTitle"
              alt="Zircon"
              class="h-10 w-auto max-w-[190px] select-none object-contain object-left drop-shadow-[0_0_12px_rgba(71,210,201,0.25)]"
              draggable="false"
            />
            <div v-if="error" class="mt-3">
              <div class="inline-flex items-center gap-2 px-3 py-1 rounded-full bg-rose-500/15 border border-rose-500/30 text-rose-300 text-[11px] font-bold tracking-wider uppercase shadow-[0_0_10px_rgba(244,63,94,0.2)]">
                <span class="relative flex h-2 w-2">
                  <span class="relative inline-flex rounded-full h-2 w-2 bg-rose-400"></span>
                </span>
                Launch Failed
              </div>
              <h2 id="launch-overlay-title" class="mt-1.5 text-2xl font-extrabold text-white tracking-tight">
                Unable to Start Minecraft
              </h2>
            </div>
            <div v-else-if="running" class="mt-3">
              <div class="inline-flex items-center gap-2 px-3 py-1 rounded-full bg-accent/15 border border-accent/30 text-accent text-[11px] font-bold tracking-wider uppercase shadow-[0_0_10px_rgba(71,210,201,0.2)]">
                <span class="relative flex h-2 w-2">
                  <span class="animate-ping absolute inline-flex h-full w-full rounded-full bg-accent opacity-75"></span>
                  <span class="relative inline-flex rounded-full h-2 w-2 bg-accent"></span>
                </span>
                Active Session
              </div>
              <h2 id="launch-overlay-title" class="mt-1.5 text-2xl font-extrabold text-white tracking-tight">
                Minecraft Booted
              </h2>
            </div>
            <div v-else>
              <p class="mt-2 text-[10px] font-bold uppercase tracking-[0.24em] text-accent/90">Launcher</p>
              <h2 id="launch-overlay-title" class="mt-1.5 text-xl font-extrabold text-white tracking-tight">
                Preparing Minecraft
              </h2>
            </div>
          </div>
          <div class="flex items-center gap-2">
            <button
              v-if="error"
              class="z-btn-ghost shrink-0 text-xs px-4 py-2 font-semibold border border-rose-500/40 text-rose-300 hover:bg-rose-500/10 rounded-xl"
              type="button"
              title="Close dialog"
              @click="close"
            >
              Close
            </button>
            <button
              v-else-if="running"
              class="z-btn-ghost shrink-0 text-xs text-accent hover:text-white hover:bg-accent/10 border border-accent/30 font-semibold px-4 py-2 rounded-xl"
              type="button"
              title="Stop Minecraft"
              @click="cancelLaunch"
            >
              Stop Game
            </button>
            <button
              v-else
              class="z-btn-ghost shrink-0 text-xs font-semibold px-4 py-2 rounded-xl border border-slate-700/80 hover:border-cyan-400 hover:text-cyan-300"
              type="button"
              title="Stop launching Minecraft"
              @click="cancelLaunch"
            >
              Stop
            </button>
          </div>
        </div>

        <!-- Error details banner when launch fails -->
        <div
          v-if="error"
          class="mb-6 rounded-xl border border-rose-500/30 bg-gradient-to-r from-rose-950/40 via-[#1a0a10]/80 to-slate-900/90 p-4 shadow-lg shadow-black/40 backdrop-blur-sm"
        >
          <div class="flex items-start gap-3.5">
            <div class="flex h-7 w-7 shrink-0 items-center justify-center rounded-full bg-rose-500/20 text-rose-400 border border-rose-500/40 shadow-[0_0_10px_rgba(244,63,94,0.3)] mt-0.5">
              <svg class="h-4 w-4" viewBox="0 0 20 20" fill="currentColor">
                <path fill-rule="evenodd" d="M10 18a8 8 0 100-16 8 8 0 000 16zM8.707 7.293a1 1 0 00-1.414 1.414L8.586 10l-1.293 1.293a1 1 0 101.414 1.414L10 11.414l1.293 1.293a1 1 0 001.414-1.414L11.414 10l1.293-1.293a1 1 0 00-1.414-1.414L10 8.586 8.707 7.293z" clip-rule="evenodd" />
              </svg>
            </div>
            <div class="flex-1 min-w-0">
              <p class="text-sm font-bold text-rose-300 mb-1">
                Launch Error Details
              </p>
              <p class="text-xs text-slate-300 font-mono break-words leading-relaxed max-h-36 overflow-y-auto pr-1">
                {{ error }}
              </p>
            </div>
          </div>
          <div class="mt-4 flex justify-end gap-2.5 pt-3 border-t border-slate-800/80">
            <button
              class="z-btn-accent text-xs font-bold px-5 py-2 rounded-xl"
              type="button"
              @click="close"
            >
              Back to Launcher
            </button>
          </div>
        </div>

        <template v-else>
          <!-- Prominent background startup disclaimer banner when Minecraft has booted -->
          <div
            v-if="running"
            class="mb-6 rounded-xl border border-accent/30 bg-gradient-to-r from-accent/15 via-[#0a1a20]/90 to-slate-900/90 p-4 shadow-lg shadow-black/40 backdrop-blur-sm"
          >
            <div class="flex items-center gap-3.5">
              <div class="flex h-7 w-7 shrink-0 items-center justify-center rounded-full bg-accent/20 text-accent border border-accent/40 shadow-[0_0_10px_rgba(71,210,201,0.3)]">
                <svg class="h-4 w-4" viewBox="0 0 20 20" fill="currentColor">
                  <path fill-rule="evenodd" d="M18 10a8 8 0 11-16 0 8 8 0 0116 0zm-7-4a1 1 0 11-2 0 1 1 0 012 0zM9 9a1 1 0 000 2v3a1 1 0 001 1h1a1 1 0 100-2v-3a1 1 0 00-1-1H9z" clip-rule="evenodd" />
                </svg>
              </div>
              <p class="text-base sm:text-lg font-bold text-accent tracking-tight leading-snug">
                It might take a few seconds for the window to open
              </p>
            </div>
          </div>

        <!-- Classic 468x60 Banner Mode: Padded, Framed Header with breathing room -->
        <div
          v-if="isClassicBanner && !bannerFailed"
          class="mb-6 w-full py-2.5 px-3 rounded-xl overflow-hidden bg-black/50 border border-slate-700/70 shadow-lg flex items-center justify-center relative z-10"
        >
          <img
            :src="server.bannerUrl"
            class="max-h-[60px] w-auto max-w-full rounded-md object-contain select-none pointer-events-none"
            :alt="server.name || 'Server Banner'"
            @error="bannerFailed = true"
            @load="onBannerLoad"
          />
        </div>

        <!-- Shaders Prompt Card (Integrated into Launch Card) -->
        <transition name="slide" mode="out-in">
          <div
            v-if="shaderPrompt"
            class="my-2 rounded-2xl border border-cyan-500/40 bg-[#0a1824]/95 p-5 shadow-[0_0_30px_rgba(71,210,201,0.15)] backdrop-blur-md relative overflow-hidden text-center"
          >
            <div class="mb-4 text-center">
              <h3 class="text-base font-extrabold text-white tracking-tight">
                This server offers shaders
              </h3>
              <p class="text-xs text-slate-300/80 font-normal leading-relaxed mt-1.5 max-w-[440px] mx-auto">
                Do you wish to install and use these shaders? If your computer isn't powerful enough, it can result in low fps.
              </p>
              <p v-if="shaderPrompt.shaderName" class="text-[11px] text-cyan-300/90 mt-2 font-medium">
                {{ shaderPrompt.shaderName }}<span v-if="shaderPrompt.shaderAuthor" class="text-slate-400 font-normal"> by {{ shaderPrompt.shaderAuthor }}</span>
              </p>
            </div>

            <!-- Choice Action Buttons -->
            <div class="grid grid-cols-2 gap-3 mb-3.5 max-w-[380px] mx-auto">
              <button
                type="button"
                class="z-btn-ghost py-2.5 px-4 font-bold text-xs rounded-xl border border-slate-700/80 text-slate-300 hover:text-white hover:border-slate-500 hover:bg-slate-800/60 transition-all text-center"
                @click="onShaderSelect(false)"
              >
                Reject Shaders
              </button>

              <button
                type="button"
                class="z-btn-accent py-2.5 px-4 font-bold text-xs rounded-xl text-center shadow-md hover:shadow-accent/30 transition-all"
                @click="onShaderSelect(true)"
              >
                Enable Shaders
              </button>
            </div>

            <!-- Remember choice checkbox -->
            <label class="flex items-center justify-center gap-2 text-xs text-slate-400 hover:text-slate-200 cursor-pointer select-none transition-colors">
              <input v-model="rememberShaderChoice" type="checkbox" class="zircon-check" />
              <span>Remember my choice for this server</span>
            </label>
          </div>

          <!-- Normal Slide Display when not prompting for shaders -->
          <div v-else class="min-h-[135px] border-l-2 border-accent/50 pl-5">
            <transition name="slide" mode="out-in">
              <div :key="activeSlide" class="slide-copy">
                <p class="mb-2 text-[10px] font-bold uppercase tracking-[0.2em] text-accent/80">{{ slides[activeSlide].eyebrow }}</p>
                <h3 class="max-w-[420px] text-2xl font-bold leading-tight text-white">{{ slides[activeSlide].title }}</h3>
                <p class="mt-3 max-w-[450px] text-sm leading-relaxed text-[#9eafb7]">{{ slides[activeSlide].body }}</p>
              </div>
            </transition>
          </div>
        </transition>

          <div class="mt-8">
            <div class="mb-2 flex items-center justify-between gap-4 text-xs">
              <span v-if="running" class="min-w-0 truncate font-semibold text-accent/90">
                {{ gameLabel ? `Minecraft running: ${gameLabel}` : 'Minecraft is active' }}
              </span>
              <span v-else-if="shaderPrompt" class="min-w-0 truncate font-semibold text-cyan-300 animate-pulse">
                Awaiting your shader preference...
              </span>
              <span v-else class="min-w-0 truncate font-semibold text-[#d6e2e4]">
                {{ status || 'Starting Minecraft...' }}
              </span>

              <span v-if="running" class="shrink-0 flex items-center gap-1.5 font-mono text-accent font-semibold">
                <span class="inline-block h-1.5 w-1.5 rounded-full bg-accent shadow-[0_0_6px_#47d2c9]"></span>
                Running
              </span>
              <span v-else-if="shaderPrompt" class="shrink-0 font-mono text-cyan-300 font-semibold text-[11px] px-2 py-0.5 rounded-full bg-cyan-500/20 border border-cyan-500/40">
                Action Required
              </span>
              <span v-else-if="progress !== null" class="shrink-0 font-mono text-accent font-bold">
                {{ Math.round(progress * 100) }}%
              </span>
              <span v-else class="launch-pulse shrink-0 text-cyan-300/80 font-mono font-medium">Working</span>
            </div>
            <div class="h-2.5 overflow-hidden rounded-full bg-slate-950 p-0.5 border border-slate-800">
              <div
                v-if="running"
                class="h-full w-full rounded-full bg-gradient-to-r from-[#5adfd5] via-accent to-[#20b2aa] shadow-[0_0_10px_rgba(71,210,201,0.4)]"
              ></div>
              <div
                v-else
                class="h-full rounded-full bg-gradient-to-r from-[#5adfd5] via-accent to-[#20b2aa] transition-[width] duration-500 shadow-[0_0_10px_rgba(71,210,201,0.4)]"
                :class="{ 'launch-indeterminate': progress === null }"
                :style="{ width: progress === null ? '35%' : `${Math.max(3, Math.round(progress * 100))}%` }"
              ></div>
            </div>
            <div class="mt-3.5 flex gap-1.5" aria-hidden="true">
              <span v-for="(_, index) in slides" :key="index" class="h-1 rounded-full transition-all duration-300" :class="index === activeSlide ? 'w-6 bg-accent shadow-[0_0_8px_#47d2c9]' : 'w-1.5 bg-slate-700'" />
            </div>
          </div>
        </template>
      </div>
    </div>
  </div>
</template>

<script setup>
import { computed, onBeforeUnmount, onMounted, ref, watch } from 'vue';
import { api } from '../lib/api';
import zirconTitle from '../assets/zircon-title.svg';

const props = defineProps({
  visible: { type: Boolean, default: false },
  status: { type: String, default: '' },
  progress: { type: Number, default: null },
  running: { type: Boolean, default: false },
  gameLabel: { type: String, default: '' },
  error: { type: String, default: '' },
  server: { type: Object, default: null },
  shaderPrompt: { type: Object, default: null },
});

const bannerFailed = ref(false);
const bannerType = ref('classic');
const rememberShaderChoice = ref(false);

function onBannerLoad(event) {
  const img = event?.target;
  if (img && img.naturalWidth && img.naturalHeight) {
    const ratio = img.naturalWidth / img.naturalHeight;
    bannerType.value = ratio > 2.8 ? 'classic' : 'hero';
  }
}

const isHeroBanner = computed(() => {
  return !!props.server?.bannerUrl && bannerType.value === 'hero';
});

const isClassicBanner = computed(() => {
  return !!props.server?.bannerUrl && bannerType.value !== 'hero';
});

watch(
  [() => props.server, () => props.server?.bannerUrl],
  () => {
    bannerFailed.value = false;
    bannerType.value = 'classic';
  }
);

watch(
  () => props.shaderPrompt,
  (prompt) => {
    if (prompt) {
      rememberShaderChoice.value = false;
    }
  }
);

const emit = defineEmits(['close', 'shader-choice']);

function onShaderSelect(enabled) {
  emit('shader-choice', {
    enabled,
    remember: rememberShaderChoice.value,
  });
}

const slides = [
  {
    eyebrow: 'A quieter way to play',
    title: 'Your world is almost ready.',
    body: 'Zircon is checking the instance, preparing its packs, and handing the final details to Minecraft.',
  },
  {
    eyebrow: 'Built for your server',
    title: 'Getting the details right.',
    body: 'Server identity, mods, and local choices are checked before the game opens so your session starts cleanly.',
  },
  {
    eyebrow: 'High-Performance Startup',
    title: 'Maximizing system resources.',
    body: 'Minecraft is prioritized with high CPU scheduling and preallocated JVM memory for smooth framerates and fast load times.',
  },
  {
    eyebrow: 'While you wait',
    title: 'Initializing graphics & mods.',
    body: 'Minecraft is loading textures, mods, and shaders in the background. The window will open directly in fullscreen.',
  },
];

const activeSlide = ref(0);
let slideTimer;

function handleKeydown(e) {
  if (e.key === 'Escape' && props.visible) {
    close();
  }
}

onMounted(() => {
  slideTimer = window.setInterval(() => {
    activeSlide.value = (activeSlide.value + 1) % slides.length;
  }, 5200);
  window.addEventListener('keydown', handleKeydown);
});

onBeforeUnmount(() => {
  window.clearInterval(slideTimer);
  window.removeEventListener('keydown', handleKeydown);
});

async function cancelLaunch() {
  try {
    await api.stopGame();
  } catch (error) {
    console.warn('Unable to stop Minecraft:', error);
  } finally {
    emit('close');
  }
}

function close() {
  if (!props.error) {
    cancelLaunch();
  } else {
    emit('close');
  }
}
</script>

<style scoped>
.launch-grid {
  opacity: 0.22;
  background-image: linear-gradient(rgba(71, 210, 201, 0.08) 1px, transparent 1px), linear-gradient(90deg, rgba(71, 210, 201, 0.08) 1px, transparent 1px);
  background-size: 28px 28px;
  mask-image: linear-gradient(to bottom right, black, transparent 70%);
}
.launch-indeterminate {
  animation: launch-progress 1.4s infinite ease-in-out;
}
.launch-pulse {
  animation: launch-pulse 1.4s infinite ease-in-out;
}
.slide-enter-active,
.slide-leave-active {
  transition: opacity 250ms ease, transform 250ms ease;
}
.slide-enter-from {
  opacity: 0;
  transform: translateY(8px);
}
.slide-leave-to {
  opacity: 0;
  transform: translateY(-8px);
}
@keyframes launch-progress {
  0% { transform: translateX(-120%); }
  100% { transform: translateX(320%); }
}
@keyframes launch-pulse {
  0%, 100% { opacity: 0.45; }
  50% { opacity: 1; }
}
</style>