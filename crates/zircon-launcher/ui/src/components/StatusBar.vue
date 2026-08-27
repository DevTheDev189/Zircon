<template>
  <div class="flex items-center gap-3 px-4 py-2 border-t border-slate-800/80 bg-[#0a0f14]">
    <span
      class="w-2 h-2 rounded-full shrink-0 shadow-sm"
      :class="dotClass"
      :title="status || 'Ready.'"
    ></span>
    <div class="flex-1 truncate text-xs font-medium" :class="textClass">
      {{ status || 'Ready.' }}
    </div>
    <div v-if="busy || progress !== null" class="w-44 h-2 bg-slate-950 rounded-full overflow-hidden border border-slate-800 p-0.5">
      <div
        v-if="busy && progress === null"
        class="h-full w-1/3 bg-gradient-to-r from-[#5adfd5] via-accent to-[#20b2aa] rounded-full animate-indeterminate shadow-[0_0_8px_rgba(71,210,201,0.4)]"
      ></div>
      <div
        v-else
        class="h-full bg-gradient-to-r from-[#5adfd5] via-accent to-[#20b2aa] rounded-full transition-all shadow-[0_0_8px_rgba(71,210,201,0.4)]"
        :style="{ width: `${Math.round((progress ?? 0) * 100)}%` }"
      ></div>
    </div>
  </div>
</template>

<style scoped>
@keyframes indeterminate {
  0% {
    transform: translateX(-100%);
  }
  100% {
    transform: translateX(400%);
  }
}
.animate-indeterminate {
  animation: indeterminate 1.2s infinite linear;
}
</style>

<script setup>
import { computed } from 'vue';

const props = defineProps({
  status: { type: String, default: '' },
  progress: { type: Number, default: null },
  busy: { type: Boolean, default: false },
});

const isError = computed(() => props.status.toLowerCase().includes('error'));

const dotClass = computed(() => {
  if (isError.value) return 'bg-[#f87171] shadow-[0_0_8px_#f87171]';
  if (props.busy) return 'bg-accent animate-pulse shadow-[0_0_8px_#47d2c9]';
  return 'bg-[#4ade80] shadow-[0_0_6px_#4ade80]';
});

const textClass = computed(() => (isError.value ? 'text-[#f87171]' : (props.busy ? 'text-cyan-300' : 'text-slate-400')));
</script>
