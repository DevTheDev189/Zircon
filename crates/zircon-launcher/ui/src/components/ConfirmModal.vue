<template>
  <Teleport to="body">
    <Transition name="z-modal-fade">
      <div
        v-if="confirmState.isOpen"
        class="fixed inset-0 z-[9999] flex items-center justify-center p-4 bg-black/75 backdrop-blur-md select-none"
        tabindex="-1"
      >
        <div
          class="relative w-full max-w-md rounded-2xl bg-[#0c121e]/95 border p-6 shadow-2xl flex flex-col gap-4 text-left transition-all duration-200"
          :class="confirmState.isDanger ? 'border-red-500/40 shadow-[0_0_30px_rgba(239,68,68,0.15)]' : 'border-cyan-500/40 shadow-[0_0_30px_rgba(71,210,201,0.15)]'"
          @click.stop
        >
          <!-- Header -->
          <div class="flex items-center gap-3">
            <div
              class="w-10 h-10 rounded-xl flex items-center justify-center shrink-0"
              :class="confirmState.isDanger ? 'bg-red-500/20 text-red-400 border border-red-500/30' : 'bg-cyan-500/20 text-cyan-300 border border-cyan-500/30'"
            >
              <svg v-if="confirmState.isDanger" class="w-5 h-5" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 9v2m0 4h.01m-6.938 4h13.856c1.54 0 2.502-1.667 1.732-3L13.732 4c-.77-1.333-2.694-1.333-3.464 0L3.34 16c-.77 1.333.192 3 1.732 3z" />
              </svg>
              <svg v-else class="w-5 h-5" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M13 16h-1v-4h-1m1-4h.01M21 12a9 9 0 11-18 0 9 9 0 0118 0z" />
              </svg>
            </div>
            <div>
              <h3 class="text-white font-bold text-base leading-tight">{{ confirmState.title }}</h3>
              <p class="text-xs text-slate-400 mt-0.5">Please review before continuing</p>
            </div>
          </div>

          <!-- Body message -->
          <p class="text-sm text-slate-200 leading-relaxed break-words bg-[#070b10]/60 p-3.5 rounded-xl border border-slate-800/80">
            {{ confirmState.message }}
          </p>

          <!-- Buttons -->
          <div class="flex justify-end items-center gap-2.5 pt-2">
            <button
              type="button"
              class="px-4 py-2 text-xs font-semibold rounded-xl text-slate-300 hover:text-white bg-slate-800/80 hover:bg-slate-700/80 border border-slate-700/60 transition-all cursor-pointer"
              @click="handleConfirmResponse(false)"
            >
              {{ confirmState.cancelText }}
            </button>
            <button
              type="button"
              class="px-5 py-2 text-xs font-bold rounded-xl transition-all shadow-md cursor-pointer"
              :class="confirmState.isDanger
                ? 'bg-red-500 hover:bg-red-600 text-white shadow-red-500/20 hover:shadow-red-500/40'
                : 'bg-cyan-500 hover:bg-cyan-400 text-slate-950 shadow-cyan-500/20 hover:shadow-cyan-500/40'"
              @click="handleConfirmResponse(true)"
            >
              {{ confirmState.confirmText }}
            </button>
          </div>
        </div>
      </div>
    </Transition>
  </Teleport>
</template>

<script setup>
import { onMounted, onBeforeUnmount } from 'vue';
import { confirmState, handleConfirmResponse } from '../lib/confirm';

function handleKeyDown(e) {
  if (!confirmState.value.isOpen) return;
  if (e.key === 'Escape') {
    e.preventDefault();
    handleConfirmResponse(false);
  } else if (e.key === 'Enter') {
    e.preventDefault();
    handleConfirmResponse(true);
  }
}

onMounted(() => {
  window.addEventListener('keydown', handleKeyDown);
});

onBeforeUnmount(() => {
  window.removeEventListener('keydown', handleKeyDown);
});
</script>
