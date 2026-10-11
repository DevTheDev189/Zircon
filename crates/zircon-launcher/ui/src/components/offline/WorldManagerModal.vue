<template>
  <div
    v-if="open"
    class="absolute inset-0 z-40 bg-[#070b0f]/85 backdrop-blur-md flex items-center justify-center p-4"
    @click.self="$emit('close')"
  >
    <div class="z-card w-full max-w-lg max-h-[80vh] flex flex-col p-6 shadow-2xl relative border border-slate-700/60 rounded-2xl bg-[#0e1622]">
      <div class="flex items-center justify-between pb-3 border-b border-slate-800/80 mb-3">
        <h3 class="text-white font-bold text-base">
          {{ filterWorld ? $t('world_manager.backups_for', { world: filterWorld }) : $t('world_manager.all_backups') }}
        </h3>
        <button class="text-slate-400 hover:text-white text-lg leading-none" :title="$t('common.close')" @click="$emit('close')">✕</button>
      </div>

      <div class="flex-1 overflow-y-auto flex flex-col gap-2 min-h-0 pr-1">
        <div v-if="loading" class="py-6 text-center text-xs text-slate-400">{{ $t('world_manager.loading_backups') }}</div>
        <div v-else-if="backups.length === 0" class="py-8 text-center text-xs text-slate-500">
          {{ $t('world_manager.no_backups') }}
        </div>
        <div
          v-for="b in backups"
          :key="b.filename"
          class="bg-slate-900/60 border border-slate-800 rounded-xl p-3 flex items-center justify-between gap-3 hover:border-slate-700 transition"
        >
          <div class="flex-1 min-w-0">
            <div class="text-xs font-bold text-slate-200 truncate">{{ b.worldName }}</div>
            <div class="text-[11px] text-slate-400 font-mono mt-0.5 truncate">{{ b.filename }}</div>
            <div class="text-[10px] text-slate-500 mt-0.5 flex items-center gap-2">
              <span>{{ fmtSize(b.sizeBytes) }}</span>
              <span>·</span>
              <span>{{ formatRelativeTime(b.createdTimestamp) }}</span>
            </div>
          </div>
          <div class="flex items-center gap-2 shrink-0">
            <button
              class="text-xs px-2.5 py-1 rounded-lg font-semibold bg-emerald-500/20 text-emerald-300 border border-emerald-500/30 hover:bg-emerald-500/30 transition"
              :title="$t('world_manager.restore_tooltip')"
              @click="$emit('restore', b.filename)"
            >
              {{ $t('world_manager.restore') }}
            </button>
            <button
              class="text-xs px-2 py-1 rounded-lg font-semibold bg-red-500/20 text-red-300 border border-red-500/30 hover:bg-red-500/30 transition"
              :title="$t('world_manager.delete_tooltip')"
              @click="$emit('delete', b.filename)"
            >
              {{ $t('world_manager.delete') }}
            </button>
          </div>
        </div>
      </div>

      <div class="flex justify-end pt-3 border-t border-slate-800/80 mt-3">
        <button class="z-btn-ghost text-xs px-5 py-2 rounded-xl font-semibold" @click="$emit('close')">{{ $t('common.close') }}</button>
      </div>
    </div>
  </div>
</template>

<script setup>
defineProps({
  open: {
    type: Boolean,
    default: false,
  },
  filterWorld: {
    type: String,
    default: '',
  },
  loading: {
    type: Boolean,
    default: false,
  },
  backups: {
    type: Array,
    default: () => [],
  },
  fmtSize: {
    type: Function,
    default: (bytes) => `${bytes} B`,
  },
  formatRelativeTime: {
    type: Function,
    default: (ts) => ts,
  },
});

defineEmits(['close', 'restore', 'delete']);
</script>
