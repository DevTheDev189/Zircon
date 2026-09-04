<template>
  <div
    v-if="open"
    class="fixed inset-0 z-50 bg-[#070b0f]/85 backdrop-blur-md flex items-center justify-center p-4 select-none"
    @click.self="emit('close')"
  >
    <div
      class="z-card w-full max-w-lg flex flex-col p-6 bg-[#0e1622] border border-slate-700/80 rounded-2xl shadow-2xl overflow-hidden animate-in fade-in zoom-in-95 duration-150"
    >
      <!-- Modal Header -->
      <div class="flex items-center justify-between pb-3 border-b border-slate-800/80 mb-4">
        <div>
          <h3 class="text-white font-extrabold text-base flex items-center gap-2">
            <span>Additional Mods Required</span>
          </h3>
          <div class="text-xs text-slate-400 mt-0.5">
            <span class="text-cyan-300 font-semibold">{{ dependencyData?.targetProjectTitle || 'This mod' }}</span>
            requires extra dependencies to run properly without crashing.
          </div>
        </div>
        <button
          class="text-slate-400 hover:text-white p-1.5 rounded-lg transition-colors hover:bg-slate-800/80"
          title="Cancel"
          @click="emit('close')"
        >
          ✕
        </button>
      </div>

      <!-- Incompatibility Warning Callout -->
      <div
        v-if="dependencyData?.incompatibleInstalled && dependencyData.incompatibleInstalled.length > 0"
        class="mb-4 p-3 rounded-xl bg-red-950/40 border border-red-500/40 text-xs text-red-200 flex items-start gap-2.5"
      >
        <svg class="w-4 h-4 text-red-400 shrink-0 mt-0.5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
          <path d="M10.29 3.86L1.82 18a2 2 0 0 0 1.71 3h20.94a2 2 0 0 0 1.71-3L13.71 3.86a2 2 0 0 0-3.42 0z" />
          <line x1="12" y1="9" x2="12" y2="13" />
          <line x1="12" y1="17" x2="12.01" y2="17" />
        </svg>
        <div>
          <span class="font-bold text-red-300">Conflict Detected:</span>
          The following incompatible mods are already in your instance:
          <span class="font-semibold text-white">{{ dependencyData.incompatibleInstalled.join(', ') }}</span>.
          Launching with both may cause game crashes.
        </div>
      </div>

      <!-- Dependencies List -->
      <div class="flex-1 min-h-0 overflow-y-auto pr-1 flex flex-col gap-3 my-1">
        <!-- Required Dependencies -->
        <div v-if="dependencyData?.requiredMissing && dependencyData.requiredMissing.length > 0">
          <div class="text-[11px] font-bold uppercase tracking-wider text-amber-400 mb-2 flex items-center gap-1.5">
            <span>Required Dependencies ({{ dependencyData.requiredMissing.length }})</span>
          </div>
          <div class="flex flex-col gap-1.5">
            <label
              v-for="dep in dependencyData.requiredMissing"
              :key="dep.projectId"
              class="flex items-center gap-3 p-2.5 rounded-xl bg-slate-900/70 border border-slate-800 hover:border-slate-700 cursor-pointer transition"
            >
              <input
                type="checkbox"
                v-model="selectedMap[dep.projectId]"
                class="rounded border-slate-700 bg-slate-800 text-cyan-500 focus:ring-0 focus:ring-offset-0 cursor-pointer"
              />
              <img
                v-if="dep.projectIcon"
                :src="dep.projectIcon"
                class="w-7 h-7 rounded-lg object-cover bg-slate-950 shrink-0"
              />
              <div v-else class="w-7 h-7 rounded-lg bg-cyan-500/20 text-cyan-300 font-bold flex items-center justify-center text-xs shrink-0">
                {{ dep.projectTitle.charAt(0) }}
              </div>
              <div class="flex-1 min-w-0">
                <div class="text-xs font-semibold text-white truncate">{{ dep.projectTitle }}</div>
                <div class="text-[10px] text-slate-400 font-mono truncate">{{ dep.filename || 'Compatible version' }}</div>
              </div>
              <span class="px-2 py-0.5 rounded text-[10px] font-bold bg-amber-500/20 text-amber-300 border border-amber-500/30">
                Required
              </span>
            </label>
          </div>
        </div>

        <!-- Optional / Recommended Dependencies -->
        <div v-if="dependencyData?.optionalMissing && dependencyData.optionalMissing.length > 0">
          <div class="text-[11px] font-bold uppercase tracking-wider text-slate-400 mb-2 flex items-center gap-1.5">
            <span>Recommended / Optional ({{ dependencyData.optionalMissing.length }})</span>
          </div>
          <div class="flex flex-col gap-1.5">
            <label
              v-for="dep in dependencyData.optionalMissing"
              :key="dep.projectId"
              class="flex items-center gap-3 p-2.5 rounded-xl bg-slate-900/70 border border-slate-800 hover:border-slate-700 cursor-pointer transition"
            >
              <input
                type="checkbox"
                v-model="selectedMap[dep.projectId]"
                class="rounded border-slate-700 bg-slate-800 text-cyan-500 focus:ring-0 focus:ring-offset-0 cursor-pointer"
              />
              <img
                v-if="dep.projectIcon"
                :src="dep.projectIcon"
                class="w-7 h-7 rounded-lg object-cover bg-slate-950 shrink-0"
              />
              <div v-else class="w-7 h-7 rounded-lg bg-slate-800 text-slate-300 font-bold flex items-center justify-center text-xs shrink-0">
                {{ dep.projectTitle.charAt(0) }}
              </div>
              <div class="flex-1 min-w-0">
                <div class="text-xs font-semibold text-white truncate">{{ dep.projectTitle }}</div>
                <div class="text-[10px] text-slate-400 font-mono truncate">{{ dep.filename || 'Compatible version' }}</div>
              </div>
              <span class="px-2 py-0.5 rounded text-[10px] font-semibold bg-slate-800 text-slate-400 border border-slate-700">
                Optional
              </span>
            </label>
          </div>
        </div>

        <!-- Already Installed Summary -->
        <div
          v-if="dependencyData?.alreadyInstalled && dependencyData.alreadyInstalled.length > 0"
          class="text-[11px] text-slate-400 px-1 pt-1"
        >
          <span class="text-emerald-400 font-semibold">Already installed:</span>
          {{ dependencyData.alreadyInstalled.join(', ') }}
        </div>
      </div>

      <!-- Actions -->
      <div class="flex items-center justify-between pt-4 border-t border-slate-800/80 mt-3">
        <button
          type="button"
          class="z-btn-ghost text-xs px-3 py-1.5 rounded-xl text-slate-400 hover:text-white"
          @click="emit('skip')"
        >
          Skip Dependencies
        </button>

        <div class="flex items-center gap-2">
          <button
            type="button"
            class="z-btn-ghost text-xs px-3 py-1.5 rounded-xl border border-slate-700 hover:border-slate-600 hover:text-white"
            @click="emit('close')"
          >
            Cancel
          </button>
          <button
            type="button"
            class="z-btn text-xs px-4 py-1.5 rounded-xl font-bold bg-cyan-600 hover:bg-cyan-500 text-white shadow-lg shadow-cyan-900/30"
            @click="onConfirm"
          >
            Install Selected &amp; Continue
          </button>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup>
import { ref, watch } from 'vue';

const props = defineProps({
  open: { type: Boolean, default: false },
  dependencyData: { type: Object, default: null },
});

const emit = defineEmits(['confirm', 'skip', 'close']);

const selectedMap = ref({});

watch(
  () => props.dependencyData,
  (data) => {
    selectedMap.value = {};
    if (data) {
      // Default: check all required dependencies
      if (data.requiredMissing) {
        for (const dep of data.requiredMissing) {
          selectedMap.value[dep.projectId] = true;
        }
      }
      // Default: check optional dependencies too for smoother out-of-box experience
      if (data.optionalMissing) {
        for (const dep of data.optionalMissing) {
          selectedMap.value[dep.projectId] = true;
        }
      }
    }
  },
  { immediate: true }
);

function onConfirm() {
  if (!props.dependencyData) {
    emit('skip');
    return;
  }
  const items = [];
  // Main mod item
  items.push({
    projectId: props.dependencyData.targetProjectId,
    versionId: props.dependencyData.targetVersionId || null,
  });

  // Selected required dependencies
  if (props.dependencyData.requiredMissing) {
    for (const dep of props.dependencyData.requiredMissing) {
      if (selectedMap.value[dep.projectId]) {
        items.push({
          projectId: dep.projectId,
          versionId: dep.versionId || null,
        });
      }
    }
  }

  // Selected optional dependencies
  if (props.dependencyData.optionalMissing) {
    for (const dep of props.dependencyData.optionalMissing) {
      if (selectedMap.value[dep.projectId]) {
        items.push({
          projectId: dep.projectId,
          versionId: dep.versionId || null,
        });
      }
    }
  }

  emit('confirm', items);
}
</script>
