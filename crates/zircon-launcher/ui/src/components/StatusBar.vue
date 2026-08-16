<template>
  <div class="flex items-center gap-3 px-4 py-2 border-t border-edge bg-card">
    <span
      class="w-2 h-2 rounded-full shrink-0"
      :class="dotClass"
      :title="status || 'Ready.'"
    ></span>
    <div class="flex-1 truncate text-xs" :class="textClass">
      {{ status || 'Ready.' }}
    </div>
    <div v-if="busy || progress !== null" class="w-40 h-1.5 bg-bg rounded-full overflow-hidden ring-1 ring-inset ring-edge">
      <div
        v-if="busy && progress === null"
        class="h-full w-1/3 bg-gradient-to-r from-accent to-[#2ba89e] rounded-full animate-indeterminate"
      ></div>
      <div
        v-else
        class="h-full bg-gradient-to-r from-accent to-[#2ba89e] rounded-full transition-all"
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
  if (isError.value) return 'bg-[#f85149]';
  if (props.busy) return 'bg-accent animate-pulse';
  return 'bg-[#3fb950]';
});

const textClass = computed(() => (isError.value ? 'text-[#f85149]' : 'text-muted'));
</script>
