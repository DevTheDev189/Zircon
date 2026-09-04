<template>
  <div class="bg-[#0b121c] border border-slate-800/90 rounded-xl p-3 flex flex-col gap-3">
    <!-- Header -->
    <div class="flex items-center justify-between">
      <span class="text-[11px] font-bold uppercase tracking-wider text-slate-300 flex items-center gap-1.5">
        <svg class="w-3.5 h-3.5 text-cyan-400" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
          <path d="M20 21v-2a4 4 0 0 0-4-4H8a4 4 0 0 0-4 4v2" />
          <circle cx="12" cy="7" r="4" />
        </svg>
        Part Isolation
      </span>
      <button
        class="text-[10px] text-cyan-400 hover:text-cyan-300 font-semibold transition-colors"
        @click="showAllParts"
      >
        Show All
      </button>
    </div>

    <!-- Interactive 2D Mannequin Silhouette -->
    <div class="flex justify-center items-center py-2 bg-[#070b10] rounded-lg border border-slate-800/80">
      <div class="flex flex-col items-center gap-1 select-none">
        <!-- Head -->
        <button
          type="button"
          class="w-10 h-10 rounded transition-all duration-150 flex items-center justify-center border font-mono text-[9px] font-bold relative"
          :class="
            studio.state.visibleParts.head
              ? 'bg-cyan-500/25 border-cyan-400/80 text-cyan-200 shadow-[0_0_8px_rgba(56,189,248,0.2)]'
              : 'bg-slate-900/60 border-slate-700/60 text-slate-500 opacity-40 line-through'
          "
          title="Toggle Head Visibility"
          @click="togglePart('head')"
        >
          HEAD
        </button>

        <!-- Torso + Arms Row -->
        <div class="flex items-center gap-1">
          <!-- Right Arm -->
          <button
            type="button"
            class="h-14 rounded transition-all duration-150 flex items-center justify-center border font-mono text-[9px] font-bold"
            :class="[
              isSlim ? 'w-4' : 'w-5',
              studio.state.visibleParts.rightArm
                ? 'bg-amber-500/25 border-amber-400/80 text-amber-200 shadow-[0_0_8px_rgba(251,191,36,0.2)]'
                : 'bg-slate-900/60 border-slate-700/60 text-slate-500 opacity-40 line-through'
            ]"
            title="Toggle Right Arm Visibility"
            @click="togglePart('rightArm')"
          >
            R
          </button>

          <!-- Torso -->
          <button
            type="button"
            class="w-10 h-14 rounded transition-all duration-150 flex items-center justify-center border font-mono text-[9px] font-bold"
            :class="
              studio.state.visibleParts.body
                ? 'bg-emerald-500/25 border-emerald-400/80 text-emerald-200 shadow-[0_0_8px_rgba(52,211,153,0.2)]'
                : 'bg-slate-900/60 border-slate-700/60 text-slate-500 opacity-40 line-through'
            "
            title="Toggle Torso Visibility"
            @click="togglePart('body')"
          >
            BODY
          </button>

          <!-- Left Arm -->
          <button
            type="button"
            class="h-14 rounded transition-all duration-150 flex items-center justify-center border font-mono text-[9px] font-bold"
            :class="[
              isSlim ? 'w-4' : 'w-5',
              studio.state.visibleParts.leftArm
                ? 'bg-amber-500/25 border-amber-400/80 text-amber-200 shadow-[0_0_8px_rgba(251,191,36,0.2)]'
                : 'bg-slate-900/60 border-slate-700/60 text-slate-500 opacity-40 line-through'
            ]"
            title="Toggle Left Arm Visibility"
            @click="togglePart('leftArm')"
          >
            L
          </button>
        </div>

        <!-- Legs Row -->
        <div class="flex items-center gap-1">
          <!-- Right Leg -->
          <button
            type="button"
            class="w-5 h-14 rounded transition-all duration-150 flex items-center justify-center border font-mono text-[9px] font-bold"
            :class="
              studio.state.visibleParts.rightLeg
                ? 'bg-purple-500/25 border-purple-400/80 text-purple-200 shadow-[0_0_8px_rgba(168,85,247,0.2)]'
                : 'bg-slate-900/60 border-slate-700/60 text-slate-500 opacity-40 line-through'
            "
            title="Toggle Right Leg Visibility"
            @click="togglePart('rightLeg')"
          >
            R
          </button>

          <!-- Left Leg -->
          <button
            type="button"
            class="w-5 h-14 rounded transition-all duration-150 flex items-center justify-center border font-mono text-[9px] font-bold"
            :class="
              studio.state.visibleParts.leftLeg
                ? 'bg-pink-500/25 border-pink-400/80 text-pink-200 shadow-[0_0_8px_rgba(244,114,182,0.2)]'
                : 'bg-slate-900/60 border-slate-700/60 text-slate-500 opacity-40 line-through'
            "
            title="Toggle Left Leg Visibility"
            @click="togglePart('leftLeg')"
          >
            L
          </button>
        </div>
      </div>
    </div>

    <!-- Layer Visibility Toggles -->
    <div class="flex flex-col gap-1.5 pt-1">
      <span class="text-[10px] font-bold uppercase tracking-wider text-slate-400">Layer Visibility</span>
      <div class="grid grid-cols-2 gap-1.5">
        <button
          type="button"
          class="py-1 px-2 rounded-lg text-xs font-semibold border flex items-center justify-center gap-1.5 transition-all"
          :class="
            studio.state.layers.base
              ? 'bg-cyan-500/20 border-cyan-500/60 text-cyan-200'
              : 'bg-slate-900/50 border-slate-800 text-slate-500 hover:border-slate-700'
          "
          @click="studio.state.layers.base = !studio.state.layers.base"
        >
          <svg class="w-3 h-3" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
            <rect x="3" y="3" width="18" height="18" rx="2" />
          </svg>
          Inner (Base)
        </button>

        <button
          type="button"
          class="py-1 px-2 rounded-lg text-xs font-semibold border flex items-center justify-center gap-1.5 transition-all"
          :class="
            studio.state.layers.overlay
              ? 'bg-cyan-500/20 border-cyan-500/60 text-cyan-200'
              : 'bg-slate-900/50 border-slate-800 text-slate-500 hover:border-slate-700'
          "
          @click="studio.state.layers.overlay = !studio.state.layers.overlay"
        >
          <svg class="w-3 h-3" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
            <polygon points="12 2 2 7 12 12 22 7 12 2" />
            <polyline points="2 17 12 22 22 17" />
            <polyline points="2 12 12 17 22 12" />
          </svg>
          Outer (Overlay)
        </button>
      </div>
    </div>

    <!-- Active Paint Target Filter -->
    <div class="flex flex-col gap-1.5 pt-1">
      <span class="text-[10px] font-bold uppercase tracking-wider text-slate-400">Paint Target</span>
      <div class="grid grid-cols-3 gap-1 bg-[#070b10] p-1 rounded-lg border border-slate-800/80">
        <button
          type="button"
          class="py-1 text-[11px] font-semibold rounded transition-all text-center"
          :class="
            studio.state.activeLayer === 'all'
              ? 'bg-cyan-500/30 text-cyan-200 border border-cyan-400/50 font-bold'
              : 'text-slate-400 hover:text-slate-200'
          "
          @click="studio.state.activeLayer = 'all'"
        >
          All
        </button>
        <button
          type="button"
          class="py-1 text-[11px] font-semibold rounded transition-all text-center"
          :class="
            studio.state.activeLayer === 'base'
              ? 'bg-cyan-500/30 text-cyan-200 border border-cyan-400/50 font-bold'
              : 'text-slate-400 hover:text-slate-200'
          "
          @click="studio.state.activeLayer = 'base'"
        >
          Inner
        </button>
        <button
          type="button"
          class="py-1 text-[11px] font-semibold rounded transition-all text-center"
          :class="
            studio.state.activeLayer === 'overlay'
              ? 'bg-cyan-500/30 text-cyan-200 border border-cyan-400/50 font-bold'
              : 'text-slate-400 hover:text-slate-200'
          "
          @click="studio.state.activeLayer = 'overlay'"
        >
          Outer
        </button>
      </div>
    </div>
  </div>
</template>

<script setup>
import { computed } from 'vue';

const props = defineProps({
  studio: { type: Object, required: true },
});

const isSlim = computed(() => props.studio.state.variant === 'slim');

function togglePart(part) {
  props.studio.state.visibleParts[part] = !props.studio.state.visibleParts[part];
}

function showAllParts() {
  for (const k in props.studio.state.visibleParts) {
    props.studio.state.visibleParts[k] = true;
  }
}
</script>
