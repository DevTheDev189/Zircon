<template>
  <div
    class="group relative rounded-2xl border transition-all duration-300 p-4 flex flex-col justify-between min-h-[185px] cursor-pointer"
    :class="[
      activeDropdown ? 'z-30 overflow-visible' : 'overflow-hidden',
      selected
        ? 'border-accent/80 bg-[#101b2a] shadow-[0_0_24px_-4px_var(--color-accent-glow)]'
        : 'border-slate-800/80 bg-gradient-to-br from-card/90 via-bg/85 to-well/90 hover:border-accent/40 hover:shadow-[0_12px_28px_-6px_rgba(0,0,0,0.6),0_0_20px_-6px_var(--color-accent-glow)] hover:-translate-y-0.5'
    ]"
    @click="$emit('select', instance)"
  >
    <!-- Fullscreen Static Background Wallpaper Layer (Matches ServersView) -->
    <div
      v-if="bannerUrl"
      class="absolute inset-0 overflow-hidden rounded-2xl pointer-events-none z-0"
    >
      <img
        :src="bannerUrl"
        class="w-full h-full object-cover opacity-60 group-hover:opacity-85 transition-all duration-700 ease-out scale-100 group-hover:scale-105"
        :alt="instance.name || 'Instance Banner'"
      />
      <div class="absolute inset-0 bg-gradient-to-r from-bg/95 via-bg/80 to-bg/85 backdrop-blur-[0.5px]"></div>
    </div>

    <!-- Active Selection Indicator Bar -->
    <div
      v-if="selected"
      class="absolute top-0 inset-x-0 h-1 bg-gradient-to-r from-accent via-accent-bright to-accent shadow-[0_0_12px_var(--color-accent-glow)] rounded-t-2xl z-30"
    ></div>

    <!-- Ambient glows & decorative flares -->
    <div class="absolute top-0 inset-x-0 h-[1px] bg-gradient-to-r from-transparent via-accent/50 to-transparent pointer-events-none z-[2]"></div>
    <div class="absolute -bottom-12 -left-12 w-48 h-48 bg-accent/10 rounded-full blur-3xl pointer-events-none z-[1]"></div>
    <div class="absolute -top-12 -right-12 w-48 h-48 bg-accent-deep/10 rounded-full blur-3xl pointer-events-none z-[1]"></div>

    <!-- Top Row inside Card: Loader Badge + 3-dots Dropdown -->
    <div
      class="relative flex items-center justify-between gap-2 mb-3"
      :class="activeDropdown ? 'z-40' : 'z-20'"
    >
      <span
        class="px-2 py-0.5 rounded-lg text-[10px] font-bold uppercase tracking-wider font-mono border backdrop-blur-md shadow-sm"
        :class="
          instance.modLoader?.type === 'fabric'
            ? 'bg-sky-500/25 text-sky-200 border-sky-400/40'
            : instance.modLoader?.type === 'neoforge'
            ? 'bg-amber-500/25 text-amber-200 border-amber-400/40'
            : instance.modLoader?.type === 'forge'
            ? 'bg-orange-500/25 text-orange-200 border-orange-400/40'
            : 'bg-emerald-500/25 text-emerald-200 border-emerald-400/40'
        "
      >
        {{ instance.modLoader?.type || 'vanilla' }}
      </span>

      <!-- 3-dots Menu -->
      <div
        class="relative"
        :class="activeDropdown ? 'z-50' : ''"
        data-dropdown="card-menu"
        @click.stop
      >
        <button
          class="p-1 rounded-lg bg-black/40 hover:bg-black/75 backdrop-blur-md border border-white/10 text-slate-300 hover:text-white transition-colors cursor-pointer shadow-sm"
          :title="$t('instance_card.options_tooltip')"
          @click="$emit('toggleDropdown', instance.id, $event)"
        >
          <svg class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 5v.01M12 12v.01M12 19v.01M12 6a1 1 0 110-2 1 1 0 010 2zm0 7a1 1 0 110-2 1 1 0 010 2zm0 7a1 1 0 110-2 1 1 0 010 2z" />
          </svg>
        </button>

        <transition name="dropdown-pop">
          <div
            v-if="activeDropdown"
            class="absolute right-0 top-full mt-1.5 w-52 bg-[#0c1322] border border-white/15 ring-1 ring-cyan-500/30 rounded-2xl shadow-[0_25px_60px_rgba(0,0,0,0.95),0_0_25px_rgba(71,210,201,0.2)] p-1.5 z-50 text-xs text-slate-200"
          >
            <!-- Ambient Top Accent Glow -->
            <div class="absolute top-0 inset-x-3 h-px bg-gradient-to-r from-transparent via-cyan-400/50 to-transparent pointer-events-none"></div>

            <button
              class="w-full text-left px-2.5 py-1.5 rounded-xl hover:bg-cyan-500/15 border border-transparent hover:border-cyan-500/30 flex items-center gap-2.5 cursor-pointer transition-all duration-150 group"
              @click="$emit('clone', instance)"
            >
              <div class="w-6 h-6 rounded-lg bg-cyan-500/10 border border-cyan-500/25 flex items-center justify-center text-cyan-400 group-hover:text-cyan-300 group-hover:bg-cyan-500/20 group-hover:scale-105 transition-all shrink-0">
                <svg class="w-3.5 h-3.5" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                  <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M8 16H6a2 2 0 01-2-2V6a2 2 0 012-2h8a2 2 0 012 2v2m-6 12h8a2 2 0 002-2v-8a2 2 0 00-2-2h-8a2 2 0 00-2 2v8a2 2 0 002 2z" />
                </svg>
              </div>
              <span class="font-semibold text-white group-hover:text-cyan-200 transition-colors">{{ $t('instance_card.clone') }}</span>
            </button>

            <button
              class="w-full text-left px-2.5 py-1.5 rounded-xl hover:bg-cyan-500/15 border border-transparent hover:border-cyan-500/30 flex items-center gap-2.5 cursor-pointer transition-all duration-150 group"
              @click="$emit('openFolder', instance)"
            >
              <div class="w-6 h-6 rounded-lg bg-cyan-500/10 border border-cyan-500/25 flex items-center justify-center text-cyan-400 group-hover:text-cyan-300 group-hover:bg-cyan-500/20 group-hover:scale-105 transition-all shrink-0">
                <svg class="w-3.5 h-3.5 text-cyan-400" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                  <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M3 7v10a2 2 0 002 2h14a2 2 0 002-2V9a2 2 0 00-2-2h-6l-2-2H5a2 2 0 00-2 2z" />
                </svg>
              </div>
              <span class="font-semibold text-white group-hover:text-cyan-200 transition-colors">{{ $t('instance_card.open_folder') }}</span>
            </button>

            <button
              class="w-full text-left px-2.5 py-1.5 rounded-xl hover:bg-cyan-500/15 border border-transparent hover:border-cyan-500/30 flex items-center gap-2.5 cursor-pointer transition-all duration-150 group"
              @click="$emit('exportMrpack', instance)"
            >
              <div class="w-6 h-6 rounded-lg bg-cyan-500/10 border border-cyan-500/25 flex items-center justify-center text-cyan-400 group-hover:text-cyan-300 group-hover:bg-cyan-500/20 group-hover:scale-105 transition-all shrink-0">
                <svg class="w-3.5 h-3.5" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                  <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M4 16v1a3 3 0 003 3h10a3 3 0 003-3v-1m-4-4l-4 4m0 0l-4-4m4 4V4" />
                </svg>
              </div>
              <span class="font-semibold text-white group-hover:text-cyan-200 transition-colors">{{ $t('instance_card.export_mrpack') }}</span>
            </button>

            <button
              class="w-full text-left px-2.5 py-1.5 rounded-xl hover:bg-amber-500/15 border border-transparent hover:border-amber-500/30 flex items-center gap-2.5 cursor-pointer transition-all duration-150 group"
              :title="$t('instance_card.cinematic_tooltip')"
              @click="$emit('installCinematic', instance)"
            >
              <div class="w-6 h-6 rounded-lg bg-amber-500/10 border border-amber-500/25 flex items-center justify-center text-amber-400 group-hover:text-amber-300 group-hover:bg-amber-500/20 group-hover:scale-105 transition-all shrink-0">
                <svg class="w-3.5 h-3.5" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
                  <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M5 3v4M3 5h4M6 17v4m-2-2h4m5-16l2.286 6.857L21 12l-5.714 2.143L13 21l-2.286-6.857L5 12l5.714-2.143L13 3z" />
                </svg>
              </div>
              <span class="font-semibold text-white group-hover:text-amber-200 transition-colors">{{ $t('instance_card.cinematic_shaders') }}</span>
            </button>

            <div class="h-px bg-gradient-to-r from-transparent via-white/10 to-transparent my-1 mx-2"></div>

            <button
              class="w-full text-left px-2.5 py-1.5 rounded-xl hover:bg-red-500/15 border border-transparent hover:border-red-500/30 text-red-400 hover:text-red-300 flex items-center gap-2.5 cursor-pointer transition-all duration-150 group"
              @click="$emit('delete', instance)"
            >
              <div class="w-6 h-6 rounded-lg bg-red-500/10 border border-red-500/25 flex items-center justify-center text-red-400 group-hover:text-red-300 group-hover:bg-red-500/20 group-hover:scale-105 transition-all shrink-0">
                <svg class="w-3.5 h-3.5" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                  <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M19 7l-.867 12.142A2 2 0 0116.138 21H7.862a2 2 0 01-1.995-1.858L5 7m5 4v6m4-6v6m1-10V4a1 1 0 00-1-1h-4a1 1 0 00-1 1v3M4 7h16" />
                </svg>
              </div>
              <span class="font-semibold text-red-400 group-hover:text-red-200 transition-colors">{{ $t('instance_card.delete') }}</span>
            </button>
          </div>
        </transition>
      </div>
    </div>

    <!-- Middle Identity Row: Avatar + Title & Meta -->
    <div class="relative z-10 flex items-center gap-3 my-auto">
      <!-- Avatar (world icon or stylish initial) -->
      <div
        v-if="iconUrl"
        class="w-12 h-12 rounded-xl border border-slate-700/80 shadow-md overflow-hidden bg-slate-900 shrink-0 group-hover:scale-105 transition-transform"
      >
        <img :src="iconUrl" class="w-full h-full object-cover" alt="Icon" />
      </div>
      <div
        v-else
        class="w-12 h-12 rounded-xl bg-gradient-to-br from-accent-bright via-accent to-accent-deep text-accent-ink font-black flex items-center justify-center text-lg shrink-0 shadow-[0_0_12px_var(--color-accent-glow)] group-hover:scale-105 transition-transform"
      >
        {{ (instance.name || 'M').charAt(0).toUpperCase() }}
      </div>

      <!-- Title & Meta -->
      <div class="min-w-0 flex-1">
        <h3 class="text-sm font-bold text-white tracking-wide truncate group-hover:text-accent-bright transition-colors" :title="instance.name">
          {{ instance.name }}
        </h3>
        <div class="flex items-center gap-2 mt-1 text-[11px] font-mono text-slate-400">
          <span class="bg-[#070b10] px-2 py-0.5 rounded border border-slate-800 text-slate-300">
            MC {{ instance.minecraftVersion }}
          </span>
          <span v-if="instance.lastPlayed" class="text-slate-500 truncate">
            {{ formatRelativeTime(instance.lastPlayed) }}
          </span>
          <span v-else class="text-slate-600">
            {{ $t('instance_card.never_played') }}
          </span>
        </div>
      </div>
    </div>

    <!-- Bottom Actions: Manage + Play -->
    <div class="relative z-10 pt-3 mt-3 border-t border-slate-800/80 flex items-center gap-2">
      <button
        class="flex-1 py-1.5 px-2.5 rounded-xl border border-slate-700/80 hover:border-slate-600 bg-slate-900/60 hover:bg-slate-800/80 text-slate-300 hover:text-white text-xs font-semibold flex items-center justify-center gap-1.5 transition-all cursor-pointer"
        :title="$t('instance_card.manage_tooltip')"
        @click.stop="$emit('manage', instance)"
      >
        <svg class="w-3.5 h-3.5 text-cyan-400" fill="none" viewBox="0 0 24 24" stroke="currentColor">
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M10.325 4.317c.426-1.756 2.924-1.756 3.35 0a1.724 1.724 0 002.573 1.066c1.543-.94 3.31.826 2.37 2.37a1.724 1.724 0 001.065 2.572c1.756.426 1.756 2.924 0 3.35a1.724 1.724 0 00-1.066 2.573c.94 1.543-.826 3.31-2.37 2.37a1.724 1.724 0 00-2.572 1.065c-.426 1.756-2.924 1.756-3.35 0a1.724 1.724 0 00-2.573-1.066c-1.543.94-3.31-.826-2.37-2.37a1.724 1.724 0 00-1.065-2.572c-1.756-.426-1.756-2.924 0-3.35a1.724 1.724 0 001.066-2.573c-.94-1.543.826-3.31 2.37-2.37.996.608 2.296.07 2.572-1.065z" />
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M15 12a3 3 0 11-6 0 3 3 0 016 0z" />
        </svg>
        <span>{{ $t('instance_card.manage') }}</span>
      </button>

      <button
        class="z-btn-accent py-1.5 px-3.5 rounded-xl text-xs font-bold shadow-md hover:shadow-cyan-500/25 flex items-center justify-center gap-1.5 transition-transform active:scale-95 shrink-0 cursor-pointer"
        :disabled="launching && selected"
        @click.stop="$emit('play', instance)"
      >
        <svg class="w-3.5 h-3.5 fill-current" viewBox="0 0 24 24"><path d="M8 5v14l11-7z" /></svg>
        <span>{{ $t('instance_card.play') }}</span>
      </button>
    </div>
  </div>
</template>

<script setup>
defineProps({
  instance: {
    type: Object,
    required: true,
  },
  selected: {
    type: Boolean,
    default: false,
  },
  bannerUrl: {
    type: String,
    default: '',
  },
  iconUrl: {
    type: String,
    default: '',
  },
  activeDropdown: {
    type: Boolean,
    default: false,
  },
  launching: {
    type: Boolean,
    default: false,
  },
  formatRelativeTime: {
    type: Function,
    default: (ts) => ts,
  },
});

defineEmits([
  'select',
  'play',
  'manage',
  'toggleDropdown',
  'clone',
  'openFolder',
  'exportMrpack',
  'installCinematic',
  'delete',
]);
</script>
