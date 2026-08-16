<template>
  <div class="flex items-center gap-3 px-4 py-2 border-t border-edge bg-card">
    <div class="flex-1 truncate text-xs text-muted" :title="status">
      {{ status || 'Ready.' }}
    </div>
    <div v-if="busy || progress !== null" class="w-40 h-1.5 bg-bg rounded-full overflow-hidden">
      <div
        v-if="busy && progress === null"
        class="h-full w-1/3 bg-accent rounded-full animate-indeterminate"
      ></div>
      <div
        v-else
        class="h-full bg-accent rounded-full transition-all"
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
defineProps({
  status: { type: String, default: '' },
  progress: { type: Number, default: null },
  busy: { type: Boolean, default: false },
});
</script>
