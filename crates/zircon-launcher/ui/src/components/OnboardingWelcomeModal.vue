<template>
  <Transition name="onboarding-fade">
    <div
      v-if="open"
      class="fixed inset-0 z-50 flex items-center justify-center p-4 bg-black/80 backdrop-blur-xl transition-all duration-300"
      @click.self="handleSkip"
    >
      <div
        class="w-full max-w-xl bg-[#090e17]/95 border border-slate-700/60 rounded-3xl shadow-[0_24px_70px_rgba(0,0,0,0.85),0_0_40px_rgba(71,210,201,0.18)] overflow-hidden relative p-7 before:absolute before:inset-x-0 before:top-0 before:h-[2px] before:bg-gradient-to-r before:from-transparent before:via-cyan-400/80 before:to-transparent animate-modal-pop"
      >
        <!-- Header: Avatar & Personalized Greeting -->
        <div class="flex items-center gap-4 mb-6">
          <div class="relative shrink-0">
            <div
              class="w-14 h-14 rounded-2xl bg-gradient-to-b from-[#101b2a] to-[#0a121c] border border-cyan-500/40 p-1 flex items-center justify-center shadow-[0_0_24px_rgba(71,210,201,0.35)]"
            >
              <img
                :src="avatarUrl || steveFallback"
                alt="Player"
                class="w-full h-full object-contain rounded-xl image-render-pixel"
              />
            </div>
            <!-- Online status pip -->
            <span class="absolute -bottom-1 -right-1 flex h-4 w-4">
              <span class="animate-ping absolute inline-flex h-full w-full rounded-full bg-cyan-400 opacity-60"></span>
              <span class="relative inline-flex rounded-full h-4 w-4 bg-cyan-400 border-2 border-[#090e17]"></span>
            </span>
          </div>

          <div class="min-w-0 flex-1">
            <div class="flex items-center gap-2">
              <h2 class="text-lg font-black text-white tracking-tight truncate">
                {{ $t('onboarding.welcome_title', { username: session?.username || 'Player' }) }}
              </h2>
            </div>
            <p class="text-xs text-slate-400 mt-0.5 leading-relaxed">
              {{ $t('onboarding.discovered_desc', { count: totalInstanceCount }) }}
            </p>
          </div>
        </div>

        <!-- Discovered Launchers Grid (Staggered Pop-In Badges) -->
        <div class="mb-5">
          <div class="text-[10px] font-mono font-bold uppercase tracking-wider text-cyan-400/80 mb-2.5">
            {{ $t('onboarding.discovered_title') }}
          </div>

          <div class="grid grid-cols-2 sm:grid-cols-3 gap-2.5">
            <div
              v-for="(launcher, idx) in activeLaunchers"
              :key="launcher.launcherType"
              class="launcher-chip p-3 rounded-2xl bg-[#0c1420]/80 border border-slate-800/90 hover:border-cyan-500/50 hover:bg-[#101c2d] transition-all flex items-center gap-3 shadow-inner"
              :style="{ animationDelay: `${idx * 70}ms` }"
            >
              <div
                class="w-9 h-9 rounded-xl bg-[#070b10] border border-slate-700/60 flex items-center justify-center p-2 shrink-0 shadow-sm"
              >
                <img
                  :src="getLauncherLogo(launcher.launcherType)"
                  class="w-full h-full object-contain"
                  :alt="launcher.launcherType"
                />
              </div>
              <div class="min-w-0 flex-1">
                <div class="text-xs font-bold text-slate-200 truncate">
                  {{ getLauncherDisplayName(launcher.launcherType) }}
                </div>
                <div class="text-[10px] font-mono text-cyan-400 font-semibold">
                  {{ launcher.instances.length }} {{ launcher.instances.length === 1 ? 'instance' : 'instances' }}
                </div>
              </div>
            </div>
          </div>
        </div>

        <!-- Safe Hardlink Feature Callout Banner -->
        <div
          class="mb-6 rounded-2xl border border-cyan-500/25 bg-gradient-to-r from-cyan-950/30 via-[#0a1625]/60 to-[#07111b]/80 p-3.5 flex items-center gap-3 shadow-inner"
        >
          <div
            class="flex h-8 w-8 items-center justify-center rounded-xl bg-cyan-500/15 border border-cyan-400/30 text-cyan-300 shrink-0"
          >
            <svg class="h-4 w-4" fill="none" viewBox="0 0 24 24" stroke="currentColor">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M13 10V3L4 14h7v7l9-11h-7z" />
            </svg>
          </div>
          <div class="min-w-0 flex-1 text-xs leading-relaxed text-slate-300">
            <span class="font-bold text-cyan-300">{{ $t('onboarding.hardlink_badge') }}:</span>
            {{ $t('onboarding.hardlink_notice') }}
          </div>
        </div>

        <!-- Footer Actions -->
        <div class="flex items-center justify-between pt-2">
          <button
            type="button"
            class="px-4 py-2 rounded-xl text-xs font-semibold text-slate-400 hover:text-slate-200 hover:bg-white/5 transition-colors"
            @click="handleSkip"
            :title="$t('onboarding.skip_tooltip')"
          >
            {{ $t('onboarding.start_fresh') }}
          </button>

          <button
            type="button"
            class="rounded-xl bg-gradient-to-r from-cyan-500 via-teal-400 to-cyan-300 px-6 py-2.5 text-xs font-black text-slate-950 shadow-[0_0_24px_rgba(71,210,201,0.35)] hover:shadow-[0_0_36px_rgba(71,210,201,0.6)] hover:brightness-110 active:scale-95 transition-all flex items-center gap-2 group cursor-pointer"
            @click="handleReview"
          >
            <span>{{ $t('onboarding.review_and_import', { count: totalInstanceCount }) }}</span>
            <svg
              class="w-3.5 h-3.5 stroke-current fill-none transition-transform group-hover:translate-x-1"
              viewBox="0 0 24 24"
            >
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2.5" d="M14 5l7 7m0 0l-7 7m7-7H3" />
            </svg>
          </button>
        </div>
      </div>
    </div>
  </Transition>
</template>

<script setup>
import { computed } from 'vue';
import prismLogo from '../assets/prism.svg';
import modrinthLogo from '../assets/modrinth.svg';
import curseforgeLogo from '../assets/curseforge.svg';
import minecraftLogo from '../assets/minecraft.svg';
import atlauncherLogo from '../assets/atlauncher.svg';
import { createDefaultSteveDataUrl } from '../lib/api';

const steveFallback = createDefaultSteveDataUrl();

const props = defineProps({
  open: {
    type: Boolean,
    default: false,
  },
  session: {
    type: Object,
    default: null,
  },
  avatarUrl: {
    type: String,
    default: '',
  },
  scanResults: {
    type: Array,
    default: () => [],
  },
});

const emit = defineEmits(['review', 'skip']);

const activeLaunchers = computed(() => {
  return (props.scanResults || []).filter(
    (res) => res.instances && res.instances.length > 0
  );
});

const totalInstanceCount = computed(() => {
  return activeLaunchers.value.reduce(
    (acc, res) => acc + (res.instances?.length || 0),
    0
  );
});

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

function handleReview() {
  emit('review');
}

function handleSkip() {
  emit('skip');
}
</script>

<style scoped>
.onboarding-fade-enter-active,
.onboarding-fade-leave-active {
  transition: opacity 240ms cubic-bezier(0.16, 1, 0.3, 1), transform 240ms cubic-bezier(0.16, 1, 0.3, 1);
}

.onboarding-fade-enter-from,
.onboarding-fade-leave-to {
  opacity: 0;
  transform: scale(0.96);
}

@keyframes modalPop {
  0% {
    opacity: 0;
    transform: scale(0.95) translateY(12px);
  }
  100% {
    opacity: 1;
    transform: scale(1) translateY(0);
  }
}

.animate-modal-pop {
  animation: modalPop 280ms cubic-bezier(0.16, 1, 0.3, 1) forwards;
}

@keyframes chipSlideIn {
  0% {
    opacity: 0;
    transform: translateY(8px) scale(0.96);
  }
  100% {
    opacity: 1;
    transform: translateY(0) scale(1);
  }
}

.launcher-chip {
  opacity: 0;
  animation: chipSlideIn 260ms cubic-bezier(0.16, 1, 0.3, 1) forwards;
}

.image-render-pixel {
  image-rendering: pixelated;
}
</style>
