<template>
  <div
    v-if="modal.open"
    class="absolute inset-0 z-50 bg-[#070b0f]/85 backdrop-blur-md flex items-center justify-center p-4"
    @click.self="$emit('close')"
  >
    <div class="z-card w-full max-w-[500px] p-6 overflow-hidden shadow-2xl relative border border-slate-700/70 rounded-2xl bg-[#0e1622]">
      <div class="flex items-start justify-between gap-3 mb-4">
        <div class="flex items-center gap-3">
          <img v-if="modal.iconUrl" :src="modal.iconUrl" class="w-11 h-11 rounded-xl object-cover bg-slate-950 shrink-0" />
          <div v-else class="w-11 h-11 rounded-xl bg-[#f16436]/20 border border-[#f16436]/40 flex items-center justify-center shrink-0">
            <img src="../../assets/curseforge.svg" class="w-6 h-6" />
          </div>
          <div>
            <div class="text-white font-bold text-base truncate">{{ modal.title }}</div>
            <div class="text-[11px] text-slate-400">CurseForge {{ modal.packType === 'shaderpack' ? $t('pack_sync.shaderpack') : (modal.packType === 'resourcepack' ? $t('pack_sync.texture_pack') : $t('pack_sync.mod')) }}</div>
          </div>
        </div>
        <button class="text-slate-400 hover:text-white text-lg leading-none" :title="$t('common.close')" @click="$emit('close')">✕</button>
      </div>

      <!-- Countdown / Download link state -->
      <div v-if="!modal.success" class="mb-4 bg-slate-950/80 border border-slate-800 rounded-xl p-3.5 text-xs text-slate-300 flex flex-col gap-2">
        <div v-if="modal.countdown > 0" class="flex items-center justify-between">
          <span>{{ $t('pack_sync.opening_in') }} <strong class="text-cyan-400 font-mono">{{ modal.countdown }}s</strong>…</span>
          <button class="text-cyan-400 underline font-semibold hover:text-cyan-300" @click="$emit('triggerDownload')">{{ $t('pack_sync.open_now') }}</button>
        </div>
        <div v-else class="flex items-center justify-between">
          <span class="text-emerald-400 font-medium">{{ $t('pack_sync.page_opened') }}</span>
          <button class="text-cyan-400 underline font-semibold hover:text-cyan-300" @click="$emit('triggerDownload')">{{ $t('pack_sync.reopen_link') }}</button>
        </div>
        <div v-if="modal.targetFileName" class="text-[11px] text-slate-400 font-mono">
          {{ $t('pack_sync.expected_file') }} <span class="text-slate-200">{{ modal.targetFileName }}</span>
        </div>
      </div>

      <!-- Success feedback state -->
      <div v-if="modal.success" class="mb-4 bg-emerald-950/50 border border-emerald-800/80 rounded-xl p-3.5 text-xs text-emerald-300 flex items-center gap-2.5">
        <span class="text-emerald-400 text-base font-bold">✓</span>
        <span><strong>{{ modal.successTitle || modal.title }}</strong> {{ $t('pack_sync.installed_success') }}</span>
      </div>

      <!-- Drop zone inside modal -->
      <div
        class="zircon-drop-zone p-6 text-center text-xs text-slate-400 cursor-pointer rounded-xl border border-dashed transition mb-4"
        :class="modal.success ? 'border-emerald-500/50 bg-emerald-950/20' : 'border-slate-700 hover:border-cyan-500/60 bg-slate-950/40'"
        @dragover.prevent
        @drop.prevent="$emit('drop', $event)"
      >
        <div class="text-slate-200 font-semibold mb-1">
          {{ modal.success ? $t('pack_sync.file_received') : $t('pack_sync.drop_file_here') }}
        </div>
        <div class="text-[11px] text-slate-400">
          {{ $t('pack_sync.accepts_format') }} <code class="text-cyan-300 font-mono">{{ modal.packType === 'mod' ? '.jar' : '.zip' }}</code> ({{ $t('pack_sync.or_browse') }} <button class="text-cyan-400 underline font-semibold hover:text-cyan-300" @click="$emit('browse')">{{ $t('pack_sync.browse_file') }}</button>)
        </div>
      </div>

      <div class="flex justify-end gap-2.5 pt-3 border-t border-slate-800/80">
        <button class="z-btn-ghost text-xs px-5 py-2 rounded-xl font-semibold" @click="$emit('close')">
          {{ modal.success ? $t('snapshots.done') : $t('common.close') }}
        </button>
      </div>
    </div>
  </div>
</template>

<script setup>
defineProps({
  modal: {
    type: Object,
    required: true,
  },
});

defineEmits(['close', 'triggerDownload', 'drop', 'browse']);
</script>
