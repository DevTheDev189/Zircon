<template>
  <Transition name="overlay-fade">
    <div
      v-if="visible"
      class="fixed inset-0 z-50 flex items-center justify-center bg-[#03070d]/85 backdrop-blur-2xl px-4 py-6 overflow-y-auto select-none"
      role="dialog"
      aria-modal="true"
      aria-labelledby="launch-dialog-title"
    >
      <!-- Server Background Banner (Cinematic blurred backdrop) -->
      <div
        v-if="bannerSource"
        class="absolute inset-0 overflow-hidden pointer-events-none select-none z-0"
      >
        <img
          :src="bannerSource"
          alt=""
          class="w-full h-full object-cover scale-110 blur-3xl saturate-[1.65] brightness-[0.3] transition-all duration-700 ease-out"
          @error="onBannerLoadError"
        />
        <!-- Vignette & Deep Obsidian Gradient Overlays -->
        <div class="absolute inset-0 bg-gradient-to-t from-[#03070d] via-[#03070d]/70 to-[#03070d]/80" />
        <div class="absolute inset-0 bg-[radial-gradient(circle_at_center,transparent_10%,rgba(3,7,13,0.85)_80%)]" />
      </div>

      <!-- Ambient Background Glow Orbs -->
      <div
        class="absolute -top-32 -left-32 w-96 h-96 rounded-full blur-3xl pointer-events-none animate-pulse z-0"
        :class="bannerSource ? 'bg-teal-500/10' : 'bg-teal-500/20'"
      />
      <div
        class="absolute -bottom-32 -right-32 w-96 h-96 rounded-full blur-3xl pointer-events-none animate-pulse z-0"
        :class="bannerSource ? 'bg-emerald-500/10' : 'bg-emerald-500/15'"
      />

      <!-- Main Launch Card with Dynamic Dual-Pane Width -->
      <div
        class="relative z-10 w-full overflow-hidden rounded-3xl border border-teal-500/30 bg-[#09111c]/95 p-6 sm:p-7 shadow-[0_25px_80px_rgba(0,0,0,0.85),0_0_50px_rgba(71,210,201,0.12)] backdrop-blur-2xl transition-all duration-500 ease-[cubic-bezier(0.16,1,0.3,1)]"
        :class="show2048 ? 'max-w-[940px]' : 'max-w-[580px]'"
      >
        <!-- Top Animated Holographic Accent Line -->
        <div class="absolute inset-x-0 top-0 h-1 bg-gradient-to-r from-teal-500 via-cyan-300 to-emerald-400 animate-shimmer" />

        <!-- PRE-LAUNCH PROMPTS FULL-CARD OVERLAY (covers entire card & 2048 with backdrop-blur-xl bg-black/85) -->
        <Transition name="overlay-fade">
          <div
            v-if="promptStep"
            class="absolute inset-0 z-30 flex items-center justify-center p-6 bg-black/85 backdrop-blur-xl rounded-3xl animate-fade-in"
          >
            <!-- Step 1: Server Resource Packs Prompt -->
            <div
              v-if="promptStep === 'packs'"
              class="w-full max-w-lg bg-[#0b0f19]/95 border border-cyan-500/35 rounded-2xl p-6 shadow-2xl flex flex-col gap-4 ring-1 ring-cyan-500/20"
            >
              <div class="flex items-center justify-between border-b border-slate-800 pb-3">
                <div class="flex items-center gap-3">
                  <div class="w-9 h-9 rounded-xl bg-cyan-500/15 border border-cyan-500/40 flex items-center justify-center text-cyan-400">
                    <svg class="w-5 h-5" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                      <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M4 16v1a3 3 0 003 3h10a3 3 0 003-3v-1m-4-8l-4-4m0 0L8 8m4-4v12" />
                    </svg>
                  </div>
                  <div>
                    <div class="flex items-center gap-2">
                      <h3 class="text-sm font-bold text-white">{{ $t('server_packs_modal.title') }}</h3>
                      <span v-if="hasShaderPrompt" class="text-[9px] uppercase font-black px-2 py-0.5 rounded-full bg-cyan-500/20 text-cyan-300 border border-cyan-500/30">Step 1 of 2</span>
                    </div>
                    <p class="text-[11px] text-slate-400">{{ $t('server_packs_modal.subtitle') }}</p>
                  </div>
                </div>
              </div>

              <!-- Pack list -->
              <div class="max-h-[200px] overflow-y-auto flex flex-col gap-2 pr-1">
                <div
                  v-for="pack in (serverPacksPrompt?.packs || [])"
                  :key="pack.filename"
                  class="p-3 rounded-xl bg-slate-900/80 border border-slate-800 flex items-center justify-between gap-3"
                >
                  <div class="min-w-0">
                    <div class="text-xs font-bold text-slate-100 truncate">{{ pack.title || pack.filename }}</div>
                    <div class="text-[10px] text-slate-400 truncate">{{ pack.description || pack.filename }}</div>
                  </div>
                  <div class="flex items-center gap-2 shrink-0">
                    <span class="text-[10px] font-mono font-bold uppercase" :class="packDecisions[pack.filename] ? 'text-cyan-400' : 'text-slate-500'">
                      {{ packDecisions[pack.filename] ? $t('server_packs_modal.accept') : $t('server_packs_modal.decline') }}
                    </span>
                    <button
                      type="button"
                      class="w-10 h-5 rounded-full transition-colors relative focus:outline-none"
                      :class="packDecisions[pack.filename] ? 'bg-cyan-500' : 'bg-slate-800 border border-slate-700'"
                      @click="packDecisions[pack.filename] = !packDecisions[pack.filename]"
                    >
                      <span
                        class="block w-3.5 h-3.5 rounded-full bg-white transition-transform transform shadow-sm"
                        :class="packDecisions[pack.filename] ? 'translate-x-5' : 'translate-x-1'"
                      ></span>
                    </button>
                  </div>
                </div>
              </div>

              <!-- Remember Choice -->
              <label class="flex items-center gap-2 text-xs text-slate-300 cursor-pointer select-none">
                <input type="checkbox" v-model="rememberPacks" class="zircon-check" />
                <span class="text-[11px]">Remember my choice for this server</span>
              </label>

              <!-- Actions -->
              <div class="flex items-center justify-between pt-2 border-t border-slate-800">
                <button
                  type="button"
                  class="text-xs text-slate-400 hover:text-slate-200 transition"
                  @click="declineAllPacks"
                >
                  {{ $t('server_packs_modal.decline_all') }}
                </button>
                <button
                  type="button"
                  class="z-btn-primary text-xs px-5 py-2 rounded-xl font-bold flex items-center gap-1.5"
                  @click="confirmPacksStep"
                >
                  <span>{{ hasShaderPrompt ? 'Next: Shaders →' : $t('server_packs_modal.accept_selection') }}</span>
                </button>
              </div>
            </div>

            <!-- Step 2 (or standalone): Shaders Prompt -->
            <div
              v-else-if="promptStep === 'shaders'"
              class="w-full max-w-lg rounded-2xl border border-amber-500/40 bg-gradient-to-b from-[#160f06]/95 to-[#0b0c10]/95 p-6 text-amber-200 shadow-2xl backdrop-blur-xl ring-1 ring-amber-500/20"
            >
              <div class="flex items-center justify-between gap-2 mb-3 border-b border-amber-500/20 pb-3">
                <div class="flex items-center gap-3">
                  <div class="flex h-9 w-9 items-center justify-center rounded-xl bg-amber-500/20 text-amber-300 border border-amber-500/40">
                    <svg class="h-5 w-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                      <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M13 10V3L4 14h7v7l9-11h-7z" />
                    </svg>
                  </div>
                  <div>
                    <div class="flex items-center gap-2">
                      <h3 class="text-sm font-bold text-white">{{ $t('overlay.shader_recommended') }}</h3>
                      <span v-if="hasServerPacksPrompt" class="text-[9px] uppercase font-black px-2 py-0.5 rounded-full bg-amber-500/20 text-amber-300 border border-amber-500/30">Step 2 of 2</span>
                    </div>
                    <p class="text-[11px] text-amber-300/80">{{ $t('overlay.iris_oculus') }}</p>
                  </div>
                </div>
              </div>

              <div class="my-3 rounded-xl bg-black/50 border border-amber-500/20 p-3.5">
                <div class="flex items-center justify-between">
                  <span class="text-xs font-bold text-white">
                    {{ shaderPrompt?.shaderName || $t('overlay.iris_recommended_shaders') }}
                  </span>
                  <span v-if="shaderPrompt?.shaderAuthor" class="text-[10px] text-amber-300/80 font-medium">
                    {{ $t('mods.by_author', { author: shaderPrompt.shaderAuthor }) }}
                  </span>
                </div>
                <p class="mt-1.5 text-[11px] text-amber-100/80 leading-relaxed">
                  {{ $t('overlay.shader_desc') }}
                </p>
              </div>

              <!-- Remember Choice Toggle -->
              <label class="flex items-center gap-2.5 text-xs text-amber-200/90 cursor-pointer my-3 select-none">
                <input
                  type="checkbox"
                  v-model="rememberShaders"
                  class="rounded border-amber-500/40 bg-amber-950/50 text-amber-400 focus:ring-0 focus:ring-offset-0 cursor-pointer h-4 w-4"
                />
                <span class="font-medium text-[11px]">{{ $t('overlay.remember_shaders') }}</span>
              </label>

              <!-- Actions -->
              <div class="flex items-center justify-end gap-2.5 mt-4 pt-3 border-t border-amber-500/20">
                <button
                  type="button"
                  class="rounded-xl border border-white/15 bg-white/5 px-4 py-2 text-xs font-semibold text-slate-300 transition hover:bg-white/10 hover:text-white"
                  @click="confirmShaderStep(false)"
                >
                  {{ $t('overlay.play_without_shaders') }}
                </button>
                <button
                  type="button"
                  class="rounded-xl bg-gradient-to-r from-amber-500 via-amber-400 to-yellow-500 px-5 py-2 text-xs font-black text-slate-950 shadow-md shadow-amber-500/30 hover:brightness-110 active:scale-95 transition-all flex items-center gap-1.5"
                  @click="confirmShaderStep(true)"
                >
                  <svg class="h-3.5 w-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                    <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2.2" d="M13 10V3L4 14h7v7l9-11h-7z" />
                  </svg>
                  <span>{{ $t('overlay.enable_shaders') }}</span>
                </button>
              </div>
            </div>
          </div>
        </Transition>

        <!-- Card Header Banner Specular Ambient Reflection -->
        <div
          v-if="bannerSource"
          class="absolute inset-x-0 top-0 h-28 overflow-hidden pointer-events-none opacity-25 z-0"
        >
          <img
            :src="bannerSource"
            alt=""
            class="w-full h-full object-cover blur-sm"
          />
          <div class="absolute inset-0 bg-gradient-to-b from-transparent via-[#09111c]/60 to-[#09111c]" />
        </div>

        <!-- Header: Branding, Title, Minigame Toggle, Stop/Close Actions -->
        <div class="relative z-10 flex items-start justify-between gap-4 mb-6 pb-4 border-b border-white/5">
          <div class="flex items-center gap-3.5 min-w-0">
            <!-- Server or Launcher Icon -->
            <div class="relative shrink-0 flex h-11 w-11 items-center justify-center rounded-2xl bg-gradient-to-br from-[#0e2130] to-[#08121c] border border-teal-500/35 shadow-inner">
              <img
                v-if="server?.icon || server?.iconUrl"
                :src="server?.icon || server?.iconUrl"
                class="h-7 w-7 object-contain rounded-lg"
                alt="Server Icon"
              />
              <svg
                v-else
                class="h-6 w-6 text-teal-300"
                fill="none"
                stroke="currentColor"
                viewBox="0 0 24 24"
              >
                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="1.8" d="M14 10l-2 1m0 0l-2-1m2 1v2.5M20 7l-2 1m2-1l-2-1m2 1v2.5M14 4l-2-1-2 1M4 7l2-1M4 7l2 1M4 7v2.5M12 21l-2-1m2 1l2-1m-2 1v-2.5M6 18l-2-1v-2.5M18 18l2-1v-2.5" />
              </svg>
              <span
                class="absolute -bottom-1 -right-1 h-3.5 w-3.5 rounded-full border-2 border-[#09111c]"
                :class="running ? 'bg-emerald-400 shadow-[0_0_8px_#34d399]' : 'bg-teal-400 animate-pulse shadow-[0_0_8px_#2dd4bf]'"
              />
            </div>

            <div class="min-w-0">
              <div class="flex items-center gap-2">
                <img
                  :src="zirconTitle"
                  alt="Zircon"
                  class="h-6 w-auto max-w-[130px] select-none object-contain drop-shadow-[0_0_8px_rgba(71,210,201,0.3)]"
                  draggable="false"
                />
                <span
                  class="inline-flex items-center gap-1 rounded-full border px-2 py-0.5 text-[9px] font-bold uppercase tracking-wider"
                  :class="running
                    ? 'border-emerald-500/40 bg-emerald-500/15 text-emerald-300 shadow-[0_0_10px_rgba(52,211,153,0.2)]'
                    : 'border-teal-500/40 bg-teal-500/15 text-teal-300 shadow-[0_0_10px_rgba(45,212,191,0.2)]'"
                >
                  <span
                    class="h-1.5 w-1.5 rounded-full"
                    :class="running ? 'bg-emerald-400' : 'bg-teal-400 animate-ping'"
                  />
                  {{ running ? $t('overlay.online') : $t('overlay.booting') }}
                </span>
              </div>
              <h2 id="launch-dialog-title" class="mt-1 text-lg font-extrabold tracking-tight text-white truncate">
                {{ server?.name || (running ? $t('overlay.game_running') : $t('overlay.starting_mc')) }}
              </h2>
              <p v-if="server?.address" class="text-[11px] font-mono text-teal-400/85 truncate">
                {{ server.address }}
              </p>
            </div>
          </div>

          <!-- Header Actions -->
          <div class="flex items-center gap-2 shrink-0">
            <!-- 2048 Minigame Toggle Button -->
            <button
              type="button"
              class="relative inline-flex items-center gap-1.5 rounded-xl border px-3 py-1.5 text-xs font-bold transition-all duration-200"
              :class="show2048
                ? 'border-teal-400/60 bg-teal-500/20 text-teal-200 shadow-[0_0_14px_rgba(71,210,201,0.35)]'
                : 'border-white/10 bg-white/5 text-slate-300 hover:border-teal-500/40 hover:text-teal-200 hover:bg-teal-500/10'"
              :title="show2048 ? $t('overlay.minigame_title_show') : $t('overlay.minigame_title_play')"
              @click="toggle2048"
            >
              <svg class="h-3.5 w-3.5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                <rect x="2" y="6" width="20" height="12" rx="4" />
                <path d="M6 12h4m-2-2v4m7-2h.01m3-2h.01" stroke-linecap="round" />
              </svg>
              <span>{{ $t('overlay.minigame_2048') }}</span>
              <span
                v-if="!show2048"
                class="hidden sm:inline-block rounded-full bg-teal-400/20 px-1.5 py-0.2 text-[9px] font-black text-teal-300 border border-teal-400/30 uppercase"
              >
                {{ $t('overlay.play_badge') }}
              </span>
            </button>

            <!-- Live Console Drawer Toggle Button -->
            <button
              type="button"
              class="relative inline-flex items-center gap-1.5 rounded-xl border px-3 py-1.5 text-xs font-bold transition-all duration-200"
              :class="showConsole
                ? 'border-cyan-400/60 bg-cyan-500/20 text-cyan-200 shadow-[0_0_14px_rgba(71,210,201,0.35)]'
                : 'border-white/10 bg-white/5 text-slate-300 hover:border-cyan-500/40 hover:text-cyan-200 hover:bg-cyan-500/10'"
              title="Toggle Live Console Output"
              @click="showConsole = !showConsole"
            >
              <svg class="h-3.5 w-3.5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                <polyline points="4 17 10 11 4 5" />
                <line x1="12" y1="19" x2="20" y2="19" />
              </svg>
              <span>Console</span>
              <span
                v-if="consoleLines.length > 0"
                class="rounded-full bg-cyan-400/20 px-1.5 py-0.2 text-[9px] font-mono font-bold text-cyan-300 border border-cyan-400/30"
              >
                {{ consoleLines.length }}
              </span>
            </button>

            <!-- Character Forge Quick Access Button -->
            <button
              type="button"
              class="relative inline-flex items-center gap-1.5 rounded-xl border border-white/10 bg-white/5 px-3 py-1.5 text-xs font-bold text-slate-300 transition-all duration-200 hover:border-cyan-400/50 hover:text-cyan-200 hover:bg-cyan-500/10"
              :title="$t('overlay.character_forge_tooltip')"
              @click="$emit('open-skins')"
            >
              <svg class="h-3.5 w-3.5 text-cyan-400" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                <path d="M20 21v-2a4 4 0 0 0-4-4H8a4 4 0 0 0-4 4v2" />
                <circle cx="12" cy="7" r="4" />
              </svg>
              <span>{{ $t('overlay.character_forge') }}</span>
            </button>

            <!-- Dismiss Error Button -->
            <button
              v-if="error"
              type="button"
              class="inline-flex items-center gap-1 rounded-xl border border-white/10 bg-white/5 px-3 py-1.5 text-xs font-semibold text-slate-300 transition hover:bg-white/10 hover:text-white"
              @click="$emit('close')"
            >
              {{ $t('overlay.dismiss') }}
            </button>

            <!-- Stop/Cancel Launch Button -->
            <button
              type="button"
              class="group inline-flex items-center gap-1.5 rounded-xl border border-rose-500/30 bg-rose-500/10 px-3 py-1.5 text-xs font-bold text-rose-300 transition-all hover:border-rose-500/60 hover:bg-rose-500/20 active:scale-95 shadow-sm"
              :title="$t('overlay.cancel_tooltip')"
              :disabled="stopping"
              @click="cancelLaunch"
            >
              <svg
                v-if="!stopping"
                class="h-3.5 w-3.5 transition-transform group-hover:scale-110"
                fill="none"
                stroke="currentColor"
                viewBox="0 0 24 24"
              >
                <rect x="6" y="6" width="12" height="12" rx="2" stroke-width="2.2" />
              </svg>
              <svg
                v-else
                class="h-3.5 w-3.5 animate-spin text-rose-300"
                viewBox="0 0 24 24"
                fill="none"
              >
                <circle class="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" stroke-width="3" />
                <path class="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8v8H4z" />
              </svg>
              <span>{{ stopping ? $t('overlay.stopping') : $t('overlay.stop') }}</span>
            </button>
          </div>
        </div>

        <!-- Card Body: Single column or Dual-Pane Grid with 2048 -->
        <div
          class="relative z-10 transition-all duration-500 ease-[cubic-bezier(0.16,1,0.3,1)]"
          :class="show2048 ? 'grid grid-cols-1 md:grid-cols-12 gap-6 items-start' : 'flex flex-col'"
        >
          <!-- Left Column: Launch Sequence & Status -->
          <div :class="show2048 ? 'md:col-span-6 flex flex-col justify-between' : 'flex flex-col'">
            <!-- Launch Error State Banner -->
            <div
              v-if="error"
              class="mb-6 rounded-2xl border border-rose-500/40 bg-rose-950/40 p-4 text-rose-200 shadow-lg shadow-rose-950/40"
            >
              <div class="flex items-center gap-2 text-xs font-extrabold text-rose-400">
                <svg class="h-4 w-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                  <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 8v4m0 4h.01M21 12a9 9 0 11-18 0 9 9 0 0118 0z" />
                </svg>
                <span>{{ $t('overlay.launch_failed') }}</span>
              </div>
              <p class="mt-2 text-xs leading-relaxed text-rose-200/90 break-words font-mono bg-rose-950/60 p-2.5 rounded-xl border border-rose-900/50">
                {{ error }}
              </p>
            </div>

            <!-- Standard Launch Lifecycle View -->
            <template v-if="!error">
              <!-- 2026 Stage Timeline Pipeline with Isolated Interconnects (no line through checkboxes) -->
              <div class="relative mb-6 rounded-2xl bg-[#070c14]/80 border border-teal-500/15 p-4 shadow-inner">
                <div class="grid grid-cols-5 gap-1 items-start">
                  <div
                    v-for="(st, idx) in stages"
                    :key="idx"
                    class="relative flex flex-col items-center text-center transition-all duration-300"
                    :class="idx <= currentStageIndex ? 'opacity-100' : 'opacity-40'"
                  >
                    <!-- Connector line to next node (stops before entering circle boundaries) -->
                    <div
                      v-if="idx < stages.length - 1"
                      class="absolute top-4 left-[calc(50%+18px)] right-[calc(-50%+18px)] h-0.5 pointer-events-none z-0"
                    >
                      <!-- Inactive track -->
                      <div class="w-full h-full bg-slate-800/80 rounded-full overflow-hidden">
                        <!-- Active filled track -->
                        <div
                          class="h-full bg-gradient-to-r from-teal-400 to-emerald-400 transition-all duration-500"
                          :style="{ width: idx < currentStageIndex ? '100%' : '0%' }"
                        />
                      </div>
                    </div>

                    <!-- Stage Node Badge (Opaque background so nothing can ever show through the checkmark) -->
                    <div
                      class="relative z-10 flex h-8 w-8 items-center justify-center rounded-full text-xs font-extrabold transition-all duration-300 shadow-md"
                      :class="stageBadgeClass(idx)"
                    >
                      <!-- Completed stage -->
                      <svg v-if="idx < currentStageIndex" class="h-4 w-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="3" d="M5 13l4 4L19 7" />
                      </svg>
                      <!-- Active stage -->
                      <span v-else-if="idx === currentStageIndex" class="relative flex h-2.5 w-2.5">
                        <span class="animate-ping absolute inline-flex h-full w-full rounded-full bg-teal-400 opacity-75" />
                        <span class="relative inline-flex rounded-full h-2.5 w-2.5 bg-teal-300" />
                      </span>
                      <!-- Future stage -->
                      <span v-else class="text-slate-500 font-mono text-[11px]">{{ idx + 1 }}</span>
                    </div>

                    <span
                      class="mt-2 text-[11px] font-bold leading-tight"
                      :class="idx === currentStageIndex ? 'text-teal-300' : (idx < currentStageIndex ? 'text-slate-200' : 'text-slate-400')"
                    >
                      {{ $t('overlay.stage_' + st.key) }}
                    </span>
                    <span class="text-[9px] text-slate-500 font-mono mt-0.5">
                      {{ $t('overlay.stage_' + st.key + '_sub') }}
                    </span>
                  </div>
                </div>
              </div>

              <!-- Rotating Tip / Insights Card -->
              <div class="mb-5 min-h-[78px] rounded-2xl border border-teal-500/15 bg-[#060b12]/80 p-3.5 shadow-inner">
                <Transition name="tip-fade" mode="out-in">
                  <div :key="activeTipIndex">
                    <div class="flex items-center justify-between">
                      <div class="flex items-center gap-1.5 text-[10px] font-black uppercase tracking-wider text-teal-300">
                        <svg class="h-3.5 w-3.5 text-teal-400" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M13 16h-1v-4h-1m1-4h.01M21 12a9 9 0 11-18 0 9 9 0 0118 0z" />
                        </svg>
                        <span>{{ $t(tips[activeTipIndex].headlineKey) }}</span>
                      </div>
                      <!-- Pips -->
                      <div class="flex items-center gap-1">
                        <span
                          v-for="(_, tIdx) in tips"
                          :key="tIdx"
                          class="h-1 rounded-full transition-all duration-300"
                          :class="tIdx === activeTipIndex ? 'w-3.5 bg-teal-400' : 'w-1 bg-white/20'"
                        />
                      </div>
                    </div>
                    <p class="mt-1.5 text-[11px] leading-relaxed text-[#b4c4cb]">
                      {{ $t(tips[activeTipIndex].messageKey) }}
                    </p>
                  </div>
                </Transition>
              </div>

              <!-- Progress Bar & Status Readout -->
              <div class="space-y-2">
                <div class="flex items-center justify-between text-xs font-semibold">
                  <span class="truncate text-slate-200 pr-2 flex items-center gap-2">
                    <span class="h-2 w-2 rounded-full bg-teal-400" :class="{ 'animate-ping': !running }" />
                    <span class="truncate">{{ status || (running ? $t('overlay.mc_active') : $t('overlay.preparing_env')) }}</span>
                  </span>
                  <span v-if="progress !== null" class="shrink-0 font-mono font-black text-teal-300 text-sm">
                    {{ Math.round(progress * 100) }}%
                  </span>
                  <span v-else class="shrink-0 text-teal-400/80 animate-pulse font-mono text-[11px] font-bold">
                    {{ running ? $t('overlay.status_active') : $t('overlay.status_syncing') }}
                  </span>
                </div>

                <!-- Luminous Shimmer Progress Bar -->
                <div class="h-2.5 w-full overflow-hidden rounded-full bg-[#0a121c] ring-1 ring-inset ring-teal-500/20 p-0.5">
                  <div
                    class="h-full rounded-full bg-gradient-to-r from-teal-400 via-cyan-300 to-emerald-400 transition-all duration-300 ease-out shadow-[0_0_12px_rgba(71,210,201,0.5)]"
                    :class="{ 'indeterminate-bar': progress === null && !running }"
                    :style="{ width: running ? '100%' : (progress === null ? '35%' : `${Math.min(100, Math.max(5, Math.round(progress * 100)))}%`) }"
                  />
                </div>
              </div>

              <!-- Live Minecraft Console Drawer -->
              <Transition name="drawer-expand">
                <div
                  v-if="showConsole"
                  class="mt-4 rounded-2xl border border-cyan-500/30 bg-[#060a12]/95 p-3.5 shadow-2xl flex flex-col gap-2 ring-1 ring-cyan-500/20"
                >
                  <div class="flex items-center justify-between border-b border-slate-800 pb-2 px-1">
                    <div class="flex items-center gap-2">
                      <div class="w-2 h-2 rounded-full bg-emerald-400 animate-pulse"></div>
                      <span class="text-xs font-bold text-slate-200 font-mono">Live Minecraft Console</span>
                      <span class="text-[10px] text-slate-500 font-mono">({{ consoleLines.length }} lines)</span>
                    </div>
                    <div class="flex items-center gap-2">
                      <label class="flex items-center gap-1.5 text-[10px] font-mono text-slate-400 cursor-pointer select-none">
                        <input type="checkbox" v-model="consoleAutoScroll" class="zircon-check" /> Auto-scroll
                      </label>
                      <button
                        type="button"
                        class="text-[10px] font-mono px-2 py-0.5 rounded bg-slate-800 hover:bg-slate-700 text-slate-300 transition-colors cursor-pointer"
                        @click="copyConsoleLogs"
                      >
                        {{ copiedConsole ? 'Copied!' : 'Copy' }}
                      </button>
                      <button
                        type="button"
                        class="text-[10px] font-mono px-2 py-0.5 rounded bg-slate-800 hover:bg-slate-700 text-slate-400 hover:text-white transition-colors cursor-pointer"
                        @click="consoleLines = []"
                      >
                        Clear
                      </button>
                    </div>
                  </div>
                  <div
                    ref="consoleBoxRef"
                    class="bg-black/60 rounded-xl p-3 font-mono text-[10px] leading-relaxed text-slate-300 max-h-[170px] overflow-y-auto whitespace-pre-wrap space-y-0.5 select-text shadow-inner"
                  >
                    <template v-if="consoleLines.length > 0">
                      <div
                        v-for="(line, idx) in consoleLines"
                        :key="idx"
                        :class="getConsoleLineClass(line)"
                      >{{ line }}</div>
                    </template>
                    <div v-else class="text-slate-500 italic text-[11px] py-4 text-center">
                      Awaiting child process stdout/stderr stream...
                    </div>
                  </div>
                </div>
              </Transition>
            </template>

            <!-- Minigame Teaser Banner (shown only when 2048 is not expanded) -->
            <div
              v-if="!show2048"
              class="mt-5 rounded-2xl border border-teal-500/20 bg-gradient-to-r from-teal-950/30 via-[#0a1624]/60 to-[#071018]/80 p-3 flex items-center justify-between gap-3 shadow-md"
            >
              <div class="flex items-center gap-2.5">
                <div class="flex h-8 w-8 items-center justify-center rounded-xl bg-teal-500/15 border border-teal-400/30 text-teal-300">
                  <svg class="h-4 w-4" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                    <rect x="2" y="6" width="20" height="12" rx="4" />
                    <path d="M6 12h4m-2-2v4m7-2h.01m3-2h.01" stroke-linecap="round" />
                  </svg>
                </div>
                <div>
                  <div class="text-xs font-bold text-white">{{ $t('overlay.play_2048_teaser') }}</div>
                  <div class="text-[10px] text-slate-400">{{ $t('overlay.play_2048_desc') }}</div>
                </div>
              </div>
              <button
                type="button"
                class="shrink-0 z-btn-accent text-[11px] font-black px-3 py-1.5 rounded-xl shadow-md flex items-center gap-1.5 active:scale-95 transition-all"
                @click="toggle2048"
              >
                <span>{{ $t('overlay.play_now') }}</span>
                <svg class="w-3 h-3" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                  <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2.5" d="M14 5l7 7m0 0l-7 7m7-7H3" />
                </svg>
              </button>
            </div>

            <!-- Character Forge Quick Access Teaser (shown when 2048 is not expanded) -->
            <div
              v-if="!show2048"
              class="mt-3 rounded-2xl border border-cyan-500/20 bg-gradient-to-r from-cyan-950/30 via-[#0a1624]/60 to-[#071018]/80 p-3 flex items-center justify-between gap-3 shadow-md"
            >
              <div class="flex items-center gap-2.5">
                <div class="flex h-8 w-8 items-center justify-center rounded-xl bg-cyan-500/15 border border-cyan-400/30 text-cyan-300">
                  <svg class="h-4 w-4" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                    <path d="M20 21v-2a4 4 0 0 0-4-4H8a4 4 0 0 0-4 4v2" />
                    <circle cx="12" cy="7" r="4" />
                  </svg>
                </div>
                <div>
                  <div class="text-xs font-bold text-white">{{ $t('overlay.forge_teaser') }}</div>
                  <div class="text-[10px] text-slate-400">{{ $t('overlay.forge_desc') }}</div>
                </div>
              </div>
              <button
                type="button"
                class="shrink-0 z-btn-ghost text-[11px] font-bold px-3 py-1.5 rounded-xl border border-cyan-400/30 text-cyan-300 hover:bg-cyan-500/20 shadow-md flex items-center gap-1.5 active:scale-95 transition-all"
                @click="$emit('open-skins')"
              >
                <span>{{ $t('overlay.open_forge') }}</span>
                <svg class="w-3 h-3" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                  <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2.5" d="M14 5l7 7m0 0l-7 7m7-7H3" />
                </svg>
              </button>
            </div>
          </div>

          <!-- Right Column: Embedded 2048 Minigame -->
          <div
            v-if="show2048"
            class="md:col-span-6 flex flex-col h-full border-t md:border-t-0 md:border-l border-white/5 pt-4 md:pt-0 md:pl-6 animate-fade-in"
          >
            <Game2048
              :embedded="true"
              @close="toggle2048"
            />
          </div>
        </div>
      </div>
    </div>
  </Transition>
</template>

<script setup>
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from 'vue';
import { api, onGameOutput } from '../lib/api';
import zirconTitle from '../assets/zircon-title.svg';
import Game2048 from './Game2048.vue';

const props = defineProps({
  visible: {
    type: Boolean,
    default: false,
  },
  status: {
    type: String,
    default: '',
  },
  progress: {
    type: Number,
    default: null,
  },
  running: {
    type: Boolean,
    default: false,
  },
  gameLabel: {
    type: String,
    default: '',
  },
  error: {
    type: String,
    default: '',
  },
  server: {
    type: Object,
    default: null,
  },
  shaderPrompt: {
    type: Object,
    default: null,
  },
  serverPacksPrompt: {
    type: Object,
    default: null,
  },
});

const emit = defineEmits(['close', 'shader-choice', 'server-packs-choice', 'open-skins']);

const promptStep = ref(null); // 'packs' | 'shaders' | null
const rememberPacks = ref(false);
const packDecisions = ref({});

const hasServerPacksPrompt = computed(() => Boolean(props.serverPacksPrompt?.packs?.length));
const hasShaderPrompt = computed(() => Boolean(props.shaderPrompt));

// Live Console Drawer State & Output Streaming
const showConsole = ref(false);
const consoleLines = ref([]);
const consoleAutoScroll = ref(true);
const copiedConsole = ref(false);
const consoleBoxRef = ref(null);
let unlistenOutput = null;

function getConsoleLineClass(line) {
  if (!line) return 'text-slate-400';
  const lower = line.toLowerCase();
  if (lower.includes('error') || lower.includes('fatal') || lower.includes('exception') || lower.includes('crash')) {
    return 'text-red-400 font-semibold';
  }
  if (lower.includes('warn')) {
    return 'text-yellow-400';
  }
  if (lower.includes('info') || lower.includes('mixin') || lower.includes('loading')) {
    return 'text-teal-300';
  }
  return 'text-slate-300';
}

async function copyConsoleLogs() {
  if (!consoleLines.value.length) return;
  try {
    await navigator.clipboard.writeText(consoleLines.value.join('\n'));
    copiedConsole.value = true;
    setTimeout(() => { copiedConsole.value = false; }, 2000);
  } catch (err) {
    console.error('Failed to copy console logs:', err);
  }
}

async function setupGameOutputListener() {
  if (unlistenOutput) return;
  try {
    unlistenOutput = await onGameOutput((line) => {
      consoleLines.value.push(line);
      if (consoleLines.value.length > 300) {
        consoleLines.value.shift();
      }
      if (consoleAutoScroll.value) {
        nextTick(() => {
          if (consoleBoxRef.value) {
            consoleBoxRef.value.scrollTop = consoleBoxRef.value.scrollHeight;
          }
        });
      }
    });
  } catch (err) {
    console.warn('Failed to attach game-output listener:', err);
  }
}

function cleanupGameOutputListener() {
  if (typeof unlistenOutput === 'function') {
    unlistenOutput();
    unlistenOutput = null;
  }
}

watch(
  () => props.visible,
  (isVisible) => {
    if (isVisible) {
      consoleLines.value = [];
      setupGameOutputListener();
    } else {
      cleanupGameOutputListener();
    }
  },
  { immediate: true }
);

onBeforeUnmount(() => {
  cleanupGameOutputListener();
});

watch(
  [() => props.serverPacksPrompt, () => props.shaderPrompt],
  ([packs, shaders]) => {
    if (packs && packs.packs && packs.packs.length > 0) {
      promptStep.value = 'packs';
      const map = {};
      packs.packs.forEach((p) => {
        map[p.filename] = true;
      });
      packDecisions.value = map;
    } else if (shaders) {
      promptStep.value = 'shaders';
    } else {
      promptStep.value = null;
    }
  },
  { immediate: true }
);

function declineAllPacks() {
  const map = {};
  (props.serverPacksPrompt?.packs || []).forEach((p) => {
    map[p.filename] = false;
  });
  packDecisions.value = map;
}

function confirmPacksStep() {
  const accepted = [];
  const rejected = [];
  Object.keys(packDecisions.value).forEach((filename) => {
    if (packDecisions.value[filename]) {
      accepted.push(filename);
    } else {
      rejected.push(filename);
    }
  });
  emit('server-packs-choice', {
    accepted,
    rejected,
    remember: Boolean(rememberPacks.value),
  });

  if (hasShaderPrompt.value) {
    promptStep.value = 'shaders';
  } else {
    promptStep.value = null;
  }
}

function confirmShaderStep(enabled) {
  emit('shader-choice', {
    enabled: Boolean(enabled),
    remember: Boolean(rememberShaders.value),
  });
  promptStep.value = null;
}

const stopping = ref(false);
const activeTipIndex = ref(0);
const rememberShaders = ref(false);
const show2048 = ref(false);
const bannerFailed = ref(false);

let tipInterval = null;
let expandTimer = null;
let stageStepTimer = null;
const currentStageIndex = ref(0);

// Server background banner computation
const bannerSource = computed(() => {
  if (bannerFailed.value) return null;
  return props.server?.bannerUrl || props.server?.banner || null;
});

function onBannerLoadError() {
  bannerFailed.value = true;
}

watch(
  () => props.server?.bannerUrl || props.server?.banner,
  () => {
    bannerFailed.value = false;
  }
);

const stages = [
  { name: 'Auth', subtitle: 'Profile', key: 'auth' },
  { name: 'Runtime', subtitle: 'Java VM', key: 'java' },
  { name: 'Packs & Mods', subtitle: 'Sync & Hash', key: 'sync' },
  { name: 'Server Boot', subtitle: 'Wakeup', key: 'server' },
  { name: 'Spawn', subtitle: 'Launching', key: 'game' },
];

const tips = [
  {
    headlineKey: 'overlay.tip1_head',
    messageKey: 'overlay.tip1_body',
  },
  {
    headlineKey: 'overlay.tip2_head',
    messageKey: 'overlay.tip2_body',
  },
  {
    headlineKey: 'overlay.tip3_head',
    messageKey: 'overlay.tip3_body',
  },
  {
    headlineKey: 'overlay.tip4_head',
    messageKey: 'overlay.tip4_body',
  },
];

function computeTargetStage() {
  const s = (props.status || '').toLowerCase();
  if (
    props.running ||
    s.includes('starting minecraft') ||
    s.includes('minecraft process') ||
    s.includes('starting offline') ||
    s.includes('game running') ||
    s.includes('minecraft is active') ||
    s.includes('playing')
  ) {
    return 4;
  }
  if (
    s.includes('waking up') ||
    s.includes('finish booting') ||
    s.includes('server is waking') ||
    s.includes('pre-join intent') ||
    (s.includes('server') && (s.includes('waiting') || s.includes('booting') || s.includes('wake') || s.includes('ping')))
  ) {
    return 3;
  }
  if (
    s.includes('mod') ||
    s.includes('pack') ||
    s.includes('hash') ||
    s.includes('sync') ||
    s.includes('staging') ||
    s.includes('bom') ||
    s.includes('downloading') ||
    s.includes('shader') ||
    s.includes('resourcepack')
  ) {
    return 2;
  }
  if (
    s.includes('java') ||
    s.includes('runtime') ||
    s.includes('classpath') ||
    s.includes('librar') ||
    s.includes('jvm')
  ) {
    return 1;
  }
  return 0;
}

function advanceStagesSequentially() {
  clearTimeout(stageStepTimer);
  const target = computeTargetStage();
  if (target > currentStageIndex.value) {
    currentStageIndex.value++;
    if (currentStageIndex.value < target) {
      stageStepTimer = setTimeout(advanceStagesSequentially, 180);
    }
  }
}

// When launch overlay becomes visible (user hits Play):
// Start compact for the first frame, and within the first second (at ~200ms),
// automatically unfold into the dual-pane 2048 minigame with smooth animation!
watch(
  () => props.visible,
  (isOpen) => {
    clearTimeout(expandTimer);
    clearTimeout(stageStepTimer);
    if (isOpen) {
      currentStageIndex.value = 0;
      bannerFailed.value = false;
      show2048.value = false;
      expandTimer = setTimeout(() => {
        if (props.visible) {
          show2048.value = true;
        }
      }, 200);
      advanceStagesSequentially();
    } else {
      currentStageIndex.value = 0;
      show2048.value = false;
    }
  },
  { immediate: true }
);

watch(
  [() => props.status, () => props.running],
  () => {
    if (props.visible) {
      advanceStagesSequentially();
    }
  }
);

function stageBadgeClass(idx) {
  if (idx < currentStageIndex.value) {
    return 'bg-[#091f28] text-teal-300 border border-teal-500/70 shadow-[0_0_10px_rgba(71,210,201,0.25)]';
  }
  if (idx === currentStageIndex.value) {
    return 'bg-[#0c2937] text-teal-200 border border-teal-400 ring-4 ring-teal-400/20 shadow-[0_0_15px_rgba(71,210,201,0.45)]';
  }
  return 'bg-[#08111c] text-slate-500 border border-slate-800';
}

function toggle2048() {
  show2048.value = !show2048.value;
}

function submitShaderChoice(enabled) {
  emit('shader-choice', {
    enabled: Boolean(enabled),
    remember: Boolean(rememberShaders.value),
  });
}

async function cancelLaunch() {
  if (stopping.value) return;
  stopping.value = true;
  try {
    await api.stopGame();
  } catch (err) {
    console.error('Failed to cancel launch:', err);
  } finally {
    stopping.value = false;
  }
}

onMounted(() => {
  tipInterval = setInterval(() => {
    activeTipIndex.value = (activeTipIndex.value + 1) % tips.length;
  }, 4800);
});

onBeforeUnmount(() => {
  if (tipInterval) {
    clearInterval(tipInterval);
  }
  clearTimeout(expandTimer);
  clearTimeout(stageStepTimer);
});
</script>

<style scoped>
.overlay-fade-enter-active,
.overlay-fade-leave-active {
  transition: opacity 220ms cubic-bezier(0.16, 1, 0.3, 1), transform 220ms cubic-bezier(0.16, 1, 0.3, 1);
}

.overlay-fade-enter-from,
.overlay-fade-leave-to {
  opacity: 0;
  transform: scale(0.97);
}

.tip-fade-enter-active,
.tip-fade-leave-active {
  transition: opacity 220ms ease, transform 220ms ease;
}

.tip-fade-enter-from {
  opacity: 0;
  transform: translateY(4px);
}

.tip-fade-leave-to {
  opacity: 0;
  transform: translateY(-4px);
}

.indeterminate-bar {
  animation: indeterminate 1.5s infinite cubic-bezier(0.65, 0.815, 0.735, 0.395);
}

@keyframes indeterminate {
  0% {
    transform: translateX(-100%);
  }
  50% {
    transform: translateX(120%);
  }
  100% {
    transform: translateX(300%);
  }
}

@keyframes shimmer {
  0% {
    background-position: 0% 50%;
  }
  50% {
    background-position: 100% 50%;
  }
  100% {
    background-position: 0% 50%;
  }
}

.animate-shimmer {
  background-size: 200% 200%;
  animation: shimmer 4s ease infinite;
}

@keyframes fadeIn {
  from {
    opacity: 0;
    transform: translateY(6px);
  }
  to {
    opacity: 1;
    transform: translateY(0);
  }
}

.animate-fade-in {
  animation: fadeIn 0.3s cubic-bezier(0.16, 1, 0.3, 1) forwards;
}

.drawer-expand-enter-active,
.drawer-expand-leave-active {
  transition: all 0.25s cubic-bezier(0.16, 1, 0.3, 1);
}
.drawer-expand-enter-from,
.drawer-expand-leave-to {
  opacity: 0;
  transform: translateY(-6px);
}
</style>