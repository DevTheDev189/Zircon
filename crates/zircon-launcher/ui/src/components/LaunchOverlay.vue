<template>
  <div
    v-if="visible"
    class="absolute inset-0 z-40 flex items-center justify-center bg-[#070b0f]/85 px-5 backdrop-blur-md"
    role="dialog"
    aria-modal="true"
    aria-labelledby="launch-overlay-title"
  >
    <div class="launch-shell relative w-full max-w-[560px] overflow-hidden rounded-2xl border border-[#33414a] bg-[#101820] shadow-2xl shadow-black/50">
      <div class="launch-grid absolute inset-0 pointer-events-none"></div>
      <div class="relative p-6 sm:p-8">
        <div class="mb-8 flex items-start justify-between gap-4">
          <div>
            <img
              :src="zirconTitle"
              alt="Zircon"
              class="h-10 w-auto max-w-[190px] select-none object-contain object-left"
              draggable="false"
            />
            <p class="mt-2 text-[10px] font-bold uppercase tracking-[0.24em] text-accent/80">Launcher</p>
            <h2 id="launch-overlay-title" class="mt-1 text-lg font-bold text-white">Preparing Minecraft</h2>
          </div>
          <button
            class="z-btn-ghost shrink-0 text-xs"
            type="button"
            title="Stop launching Minecraft"
            @click="cancelLaunch"
          >
            Stop
          </button>
        </div>

        <div class="min-h-[142px] border-l-2 border-accent/50 pl-5">
          <transition name="slide" mode="out-in">
            <div :key="activeSlide" class="slide-copy">
              <p class="mb-2 text-[10px] font-bold uppercase tracking-[0.2em] text-[#8babb0]">{{ slides[activeSlide].eyebrow }}</p>
              <h3 class="max-w-[420px] text-2xl font-bold leading-tight text-white">{{ slides[activeSlide].title }}</h3>
              <p class="mt-3 max-w-[450px] text-sm leading-relaxed text-[#9eafb7]">{{ slides[activeSlide].body }}</p>
            </div>
          </transition>
        </div>

        <div class="mt-8">
          <div class="mb-2 flex items-center justify-between gap-4 text-xs">
            <span class="min-w-0 truncate font-semibold text-[#d6e2e4]">{{ status || 'Starting Minecraft...' }}</span>
            <span v-if="progress !== null" class="shrink-0 font-mono text-accent">{{ Math.round(progress * 100) }}%</span>
            <span v-else class="launch-pulse shrink-0 text-[#789399]">Working</span>
          </div>
          <div class="h-2 overflow-hidden rounded-full bg-[#202d34] ring-1 ring-inset ring-white/5">
            <div
              class="h-full rounded-full bg-gradient-to-r from-[#5adfd5] via-accent to-[#2ba89e] transition-[width] duration-500"
              :class="{ 'launch-indeterminate': progress === null }"
              :style="{ width: progress === null ? '35%' : `${Math.max(3, Math.round(progress * 100))}%` }"
            ></div>
          </div>
          <div class="mt-3 flex gap-1.5" aria-hidden="true">
            <span v-for="(_, index) in slides" :key="index" class="h-1 rounded-full transition-all" :class="index === activeSlide ? 'w-6 bg-accent' : 'w-1.5 bg-[#53666c]'" />
          </div>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup>
import { onBeforeUnmount, onMounted, ref } from 'vue';
import { api } from '../lib/api';
import zirconTitle from '../assets/zircon-title.svg';

defineProps({
  visible: { type: Boolean, default: false },
  status: { type: String, default: '' },
  progress: { type: Number, default: null },
});

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
    eyebrow: 'While you wait',
    title: 'The first boot can take a little longer.',
    body: 'Minecraft may be indexing libraries or compiling shaders. Later launches are usually quicker.',
  },
];

const activeSlide = ref(0);
let slideTimer;

onMounted(() => {
  slideTimer = window.setInterval(() => {
    activeSlide.value = (activeSlide.value + 1) % slides.length;
  }, 5200);
});

onBeforeUnmount(() => window.clearInterval(slideTimer));

async function cancelLaunch() {
  try {
    await api.stopGame();
  } catch (error) {
    console.warn('Unable to stop Minecraft:', error);
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