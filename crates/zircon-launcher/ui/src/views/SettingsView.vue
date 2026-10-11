<template>
  <div class="h-full p-5 overflow-y-auto flex flex-col xl:flex-row gap-6 items-start">
    <!-- Left Column: Launcher Settings & Diagnostics -->
    <div class="w-full xl:w-[500px] 2xl:w-[540px] z-card flex flex-col p-6 bg-[#0e1622]/90 border border-slate-800/80 shrink-0 min-w-0">
      
      <!-- Top Header: Title, Version Tag, Inline Update Check & Save CTA -->
      <div class="flex items-start justify-between gap-4 mb-4 pb-4 border-b border-slate-800/80">
        <div>
          <div class="flex items-center gap-2.5">
            <h3 class="text-white font-extrabold text-lg tracking-tight">{{ $t('settings.title') }}</h3>
            <!-- Launcher Version right at the top beside title -->
            <span class="inline-flex items-center gap-1.5 px-2.5 py-0.5 rounded-full text-xs font-mono font-bold bg-cyan-500/10 text-cyan-300 border border-cyan-500/30">
              <span class="w-1.5 h-1.5 rounded-full bg-cyan-400"></span>
              v{{ launcherVersion || '0.4.3' }}
            </span>
          </div>
          <!-- Subtitle right under Settings title with inline update checker -->
          <div class="text-xs text-slate-400 mt-1 flex items-center gap-2 flex-wrap">
            <span>{{ $t('settings.subtitle') }}</span>
            <span class="text-slate-600">•</span>
            <button
              type="button"
              class="text-xs text-cyan-400 hover:text-cyan-300 font-medium inline-flex items-center gap-1 transition-colors disabled:opacity-50 cursor-pointer"
              :disabled="checkingUpdate || updating"
              @click="checkForUpdates"
            >
              <svg v-if="checkingUpdate" class="w-3 h-3 animate-spin" fill="none" viewBox="0 0 24 24">
                <circle class="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" stroke-width="4"></circle>
                <path class="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4zm2 5.291A7.962 7.962 0 014 12H0c0 3.042 1.135 5.824 3 7.938l3-2.647z"></path>
              </svg>
              <svg v-else class="w-3 h-3" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
                <path stroke-linecap="round" stroke-linejoin="round" d="M4 4v5h.582m15.356 2A8.001 8.001 0 004.582 9m0 0H9m11 11v-5h-.581m0 0a8.003 8.003 0 01-15.357-2m15.357 2H15" />
              </svg>
              <span>{{ checkingUpdate ? $t('settings.checking_updates') : $t('settings.check_updates') }}</span>
            </button>
          </div>
        </div>

        <div class="flex flex-col items-end gap-1 shrink-0">
          <button
            class="z-btn-accent text-xs rounded-xl font-bold px-4 py-2 shadow-md hover:shadow-cyan-500/25 flex items-center gap-1.5 cursor-pointer"
            :disabled="saving"
            @click="save"
          >
            <svg v-if="saving" class="w-3.5 h-3.5 animate-spin" fill="none" viewBox="0 0 24 24">
              <circle class="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" stroke-width="4"></circle>
              <path class="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4z"></path>
            </svg>
            <span>{{ saving ? $t('common.saving') : $t('common.save') }}</span>
          </button>
          <p v-if="savedAt" class="text-cyan-300 font-medium text-[11px] animate-fade-in">{{ savedAt }}</p>
        </div>
      </div>

      <!-- Update Status Notification Box (at the very top if active) -->
      <div v-if="updateStatusMessage" class="mb-4 p-3 rounded-xl border text-xs transition-all" :class="updateStatusClass">
        <div class="flex items-center justify-between">
          <div class="font-semibold">{{ updateStatusMessage }}</div>
          <button v-if="!updating && !updateInfo" class="text-slate-400 hover:text-white text-xs ml-2 cursor-pointer" @click="updateStatusMessage = ''">✕</button>
        </div>
        <div v-if="updateInfo?.notes" class="text-[11px] text-slate-400 mt-1">{{ updateInfo.notes }}</div>

        <!-- Download / Install progress bar -->
        <div v-if="updating" class="mt-2.5">
          <div class="w-full bg-[#070b10] h-2 rounded-full overflow-hidden border border-slate-800">
            <div class="bg-cyan-400 h-full rounded-full transition-all duration-200" :style="{ width: Math.round(updateProgress * 100) + '%' }"></div>
          </div>
          <p class="text-[10px] text-cyan-300 font-mono mt-1">{{ Math.round(updateProgress * 100) }}% downloaded</p>
        </div>

        <div v-if="updateInfo && !updating" class="mt-3 flex justify-end">
          <button
            class="z-btn-accent text-xs px-4 py-1.5 rounded-lg font-bold shadow-md hover:shadow-cyan-500/25"
            @click="installUpdate"
          >
            Update &amp; Restart
          </button>
        </div>
      </div>

      <!-- Category Filter Navigation Bar -->
      <div class="flex items-center gap-1.5 pb-2 mb-4 overflow-x-auto no-scrollbar">
        <button
          v-for="cat in categories"
          :key="cat.id"
          type="button"
          class="px-3 py-1.5 rounded-xl text-xs font-semibold whitespace-nowrap transition-all flex items-center gap-1.5 cursor-pointer shrink-0"
          :class="activeCategory === cat.id
            ? 'bg-accent/20 text-accent-bright border border-accent/40 shadow-[0_0_10px_var(--color-accent-glow)]'
            : 'bg-well/70 text-slate-400 border border-slate-800/80 hover:text-white hover:border-slate-700'"
          @click="activeCategory = cat.id"
        >
          <svg class="w-3.5 h-3.5 shrink-0" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
            <path stroke-linecap="round" stroke-linejoin="round" :d="cat.icon" />
          </svg>
          <span>{{ $t('settings.categories.' + cat.id) }}</span>
        </button>
      </div>

      <!-- ================= SECTION 1: PERFORMANCE & HARDWARE ================= -->
      <div v-show="activeCategory === 'all' || activeCategory === 'performance'" class="mb-4 p-4 rounded-2xl bg-well/40 border border-slate-800/80 space-y-4">
        <div class="flex items-center justify-between pb-2 border-b border-slate-800/60">
          <div class="flex items-center gap-2">
            <div class="w-6 h-6 rounded-lg bg-accent/15 border border-accent/40 flex items-center justify-center text-accent-bright shadow-[0_0_12px_var(--color-accent-glow)]">
              <svg class="w-3.5 h-3.5" viewBox="0 0 24 24" fill="currentColor">
                <path d="M13 10V3L4 14h7v7l9-11h-7z" />
              </svg>
            </div>
            <div>
              <span class="text-xs font-bold text-white block">{{ $t('settings.ram.title') }}</span>
              <span class="text-[10px] text-slate-400">{{ $t('settings.ram.subtitle') }}</span>
            </div>
          </div>
          <span class="text-[9px] bg-accent/15 text-accent-bright border border-accent/30 px-2 py-0.5 rounded-full font-bold uppercase tracking-wider font-mono">
            Engine
          </span>
        </div>

        <!-- RAM slider -->
        <div>
          <div class="flex items-center justify-between mb-2">
            <div class="flex items-center gap-2">
              <span class="z-label font-semibold text-slate-300 text-xs">{{ $t('settings.ram.label') }}</span>
              <span v-if="systemRamInfo.totalRamGb" class="text-[10px] text-slate-400 font-mono bg-slate-900 border border-slate-800 px-2 py-0.5 rounded">
                {{ systemRamInfo.totalRamGb }} GB detected
              </span>
            </div>
            <div class="flex items-center gap-2">
              <span
                v-if="settings.memoryGb === systemRamInfo.recommendedRamGb"
                class="text-[10px] font-bold text-emerald-400 font-mono bg-emerald-500/10 border border-emerald-500/30 px-1.5 py-0.5 rounded"
              >
                Recommended
              </span>
              <span class="text-xs font-bold text-cyan-300 font-mono bg-cyan-500/10 border border-cyan-500/30 px-2 py-0.5 rounded-md">
                {{ settings.memoryGb }} GB
              </span>
            </div>
          </div>
          <input
            v-model.number="settings.memoryGb"
            type="range"
            min="2"
            :max="sliderMax"
            step="1"
            class="w-full accent-cyan-400 cursor-pointer"
          />
          <div class="flex justify-between text-[10px] text-slate-500 font-mono mt-1">
            <span>2 GB (Minimal)</span>
            <span
              class="text-cyan-400/80 cursor-pointer hover:underline"
              :title="'Click to apply recommended memory (' + systemRamInfo.recommendedRamGb + ' GB)'"
              @click="settings.memoryGb = systemRamInfo.recommendedRamGb"
            >
              Optimal: {{ systemRamInfo.recommendedRamGb }} GB
            </span>
            <span>{{ sliderMax }} GB (Heavy Modpacks)</span>
          </div>
          <div v-if="isHighRam" class="mt-2 p-2 rounded-lg bg-amber-500/10 border border-amber-500/30 text-amber-300 text-xs flex items-center gap-2">
            <svg class="w-4 h-4 shrink-0 text-amber-400" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
              <path d="m21.73 18-8-14a2 2 0 0 0-3.48 0l-8 14A2 2 0 0 0 4 21h16a2 2 0 0 0 1.73-3Z" />
              <line x1="12" y1="9" x2="12" y2="13" />
              <line x1="12" y1="17" x2="12.01" y2="17" />
            </svg>
            <span>{{ $t('settings.ram.high_ram_warning') }}</span>
          </div>
          <p class="z-label mt-1.5 text-slate-400 text-[11px]">
            {{ $t('settings.ram.java_flag_hint') }}
          </p>
        </div>

        <!-- Graphics Processor & GPU Preference -->
        <div class="pt-3 border-t border-slate-800/60">
          <div class="flex items-center justify-between mb-2">
            <span class="z-label font-semibold text-slate-300 text-xs">{{ $t('settings.gpu.label') }}</span>
            <span v-if="activeGpuLabel" class="text-[10px] text-slate-400 font-mono truncate max-w-[200px]" :title="activeGpuLabel">
              {{ activeGpuLabel }}
            </span>
          </div>
          <p class="z-label text-slate-400 text-[11px] mb-2.5">
            {{ $t('settings.gpu.desc') }}
          </p>

          <div class="space-y-2">
            <!-- Dedicated GPU Option -->
            <button
              type="button"
              class="w-full flex items-start gap-3 p-3 rounded-xl border text-left transition-all cursor-pointer group"
              :class="settings.gpuPreference === 'dedicated'
                ? 'border-accent bg-accent/10 shadow-[0_0_12px_var(--color-accent-glow)] ring-1 ring-accent/30'
                : 'border-slate-800/80 bg-well/70 hover:border-accent/40 hover:bg-well'"
              @click="settings.gpuPreference = 'dedicated'"
            >
              <div class="mt-0.5 w-4 h-4 rounded-full border flex items-center justify-center shrink-0 transition-colors"
                :class="settings.gpuPreference === 'dedicated' ? 'border-accent bg-accent/20' : 'border-slate-600'"
              >
                <div v-if="settings.gpuPreference === 'dedicated'" class="w-2 h-2 rounded-full bg-accent shadow-[0_0_6px_var(--color-accent-glow)]"></div>
              </div>
              <div class="flex-1 min-w-0">
                <div class="flex items-center gap-2">
                  <span class="text-xs font-bold transition-colors" :class="settings.gpuPreference === 'dedicated' ? 'text-white' : 'text-slate-300 group-hover:text-white'">
                    {{ $t('settings.gpu.dedicated_title') }}
                  </span>
                  <span class="text-[9px] font-bold text-emerald-400 font-mono bg-emerald-500/10 border border-emerald-500/30 px-1.5 py-0.2 rounded">
                    {{ $t('settings.gpu.recommended_badge') }}
                  </span>
                </div>
                <div class="text-[11px] text-slate-400 mt-0.5">
                  {{ dedicatedGpu ? dedicatedGpu.name + (dedicatedGpu.vramMb ? ` (${dedicatedGpu.vramMb} MB VRAM)` : '') : 'Primary high-performance graphics card' }}
                </div>
                <div class="text-[10px] text-accent-bright/80 mt-1">
                  {{ $t('settings.gpu.dedicated_desc') }}
                </div>
              </div>
            </button>

            <!-- Integrated GPU Option -->
            <button
              type="button"
              class="w-full flex items-start gap-3 p-3 rounded-xl border text-left transition-all cursor-pointer group"
              :class="settings.gpuPreference === 'integrated'
                ? 'border-accent bg-accent/10 shadow-[0_0_12px_var(--color-accent-glow)] ring-1 ring-accent/30'
                : 'border-slate-800/80 bg-well/70 hover:border-accent/40 hover:bg-well'"
              @click="settings.gpuPreference = 'integrated'"
            >
              <div class="mt-0.5 w-4 h-4 rounded-full border flex items-center justify-center shrink-0 transition-colors"
                :class="settings.gpuPreference === 'integrated' ? 'border-accent bg-accent/20' : 'border-slate-600'"
              >
                <div v-if="settings.gpuPreference === 'integrated'" class="w-2 h-2 rounded-full bg-accent shadow-[0_0_6px_var(--color-accent-glow)]"></div>
              </div>
              <div class="flex-1 min-w-0">
                <span class="text-xs font-bold transition-colors" :class="settings.gpuPreference === 'integrated' ? 'text-white' : 'text-slate-300 group-hover:text-white'">
                  {{ $t('settings.gpu.integrated_title') }}
                </span>
                <div class="text-[11px] text-slate-400 mt-0.5">
                  {{ integratedGpu ? integratedGpu.name : $t('settings.gpu.integrated_desc') }}
                </div>
              </div>
            </button>

            <!-- System Default Option -->
            <button
              type="button"
              class="w-full flex items-start gap-3 p-3 rounded-xl border text-left transition-all cursor-pointer group"
              :class="settings.gpuPreference === 'system'
                ? 'border-accent bg-accent/10 shadow-[0_0_12px_var(--color-accent-glow)] ring-1 ring-accent/30'
                : 'border-slate-800/80 bg-well/70 hover:border-accent/40 hover:bg-well'"
              @click="settings.gpuPreference = 'system'"
            >
              <div class="mt-0.5 w-4 h-4 rounded-full border flex items-center justify-center shrink-0 transition-colors"
                :class="settings.gpuPreference === 'system' ? 'border-accent bg-accent/20' : 'border-slate-600'"
              >
                <div v-if="settings.gpuPreference === 'system'" class="w-2 h-2 rounded-full bg-accent shadow-[0_0_6px_var(--color-accent-glow)]"></div>
              </div>
              <div class="flex-1 min-w-0">
                <span class="text-xs font-bold transition-colors" :class="settings.gpuPreference === 'system' ? 'text-white' : 'text-slate-300 group-hover:text-white'">
                  {{ $t('settings.gpu.system_title') }}
                </span>
                <div class="text-[11px] text-slate-400 mt-0.5">
                  {{ $t('settings.gpu.system_desc') }}
                </div>
              </div>
            </button>
          </div>
        </div>
      </div>

      <!-- ================= SECTION 2: GAME WINDOW & DISPLAY ================= -->
      <div v-show="activeCategory === 'all' || activeCategory === 'display'" class="mb-4 p-4 rounded-2xl bg-well/40 border border-slate-800/80 space-y-4">
        <div class="flex items-center justify-between pb-2 border-b border-slate-800/60">
          <div class="flex items-center gap-2">
            <div class="w-6 h-6 rounded-lg bg-accent/15 border border-accent/40 flex items-center justify-center text-accent-bright shadow-[0_0_12px_var(--color-accent-glow)]">
              <svg class="w-3.5 h-3.5" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
                <rect x="2" y="3" width="20" height="14" rx="2" />
                <line x1="8" y1="21" x2="16" y2="21" />
                <line x1="12" y1="17" x2="12" y2="21" />
              </svg>
            </div>
            <div>
              <span class="text-xs font-bold text-white block">{{ $t('settings.window.title') }}</span>
              <span class="text-[10px] text-slate-400">{{ $t('settings.window.subtitle') }}</span>
            </div>
          </div>
          <span class="text-[9px] bg-accent/15 text-accent-bright border border-accent/30 px-2 py-0.5 rounded-full font-bold uppercase tracking-wider font-mono">
            {{ $t('settings.categories.display') }}
          </span>
        </div>

        <label class="flex items-center gap-3 cursor-pointer select-none group p-2 rounded-xl bg-well/60 border border-slate-800/80 hover:border-slate-700 transition">
          <input
            v-model="settings.startFullscreen"
            type="checkbox"
            class="zircon-check"
          />
          <div class="flex-1">
            <span class="z-label font-semibold text-slate-200 group-hover:text-white transition-colors text-xs block">
              {{ $t('settings.window.fullscreen_label') }}
            </span>
            <span class="text-[10px] text-slate-400">{{ $t('settings.window.fullscreen_desc') }}</span>
          </div>
        </label>

        <div v-if="!settings.startFullscreen" class="space-y-2 pt-1">
          <div class="flex items-center gap-3">
            <div class="flex-1">
              <span class="text-[10px] text-slate-400 font-mono block mb-1">{{ $t('settings.window.width_label') }}</span>
              <input
                v-model.number="settings.windowWidth"
                type="number"
                min="0"
                step="10"
                class="z-input w-full font-mono text-xs"
                placeholder="1920 (Auto)"
              />
            </div>
            <span class="text-slate-500 font-bold mt-4">×</span>
            <div class="flex-1">
              <span class="text-[10px] text-slate-400 font-mono block mb-1">{{ $t('settings.window.height_label') }}</span>
              <input
                v-model.number="settings.windowHeight"
                type="number"
                min="0"
                step="10"
                class="z-input w-full font-mono text-xs"
                placeholder="1080 (Auto)"
              />
            </div>
          </div>
          <div class="flex flex-wrap gap-1.5 pt-0.5">
            <button
              type="button"
              class="z-btn-ghost text-[10px] px-2.5 py-1 rounded-lg border border-slate-800 hover:border-slate-600 hover:text-cyan-300 cursor-pointer"
              @click="setResolution(1280, 720)"
            >
              {{ $t('settings.window.res_720') }}
            </button>
            <button
              type="button"
              class="z-btn-ghost text-[10px] px-2.5 py-1 rounded-lg border border-slate-800 hover:border-slate-600 hover:text-cyan-300 cursor-pointer"
              @click="setResolution(1920, 1080)"
            >
              {{ $t('settings.window.res_1080') }}
            </button>
            <button
              type="button"
              class="z-btn-ghost text-[10px] px-2.5 py-1 rounded-lg border border-slate-800 hover:border-slate-600 hover:text-cyan-300 cursor-pointer"
              @click="setResolution(2560, 1440)"
            >
              {{ $t('settings.window.res_1440') }}
            </button>
            <button
              type="button"
              class="z-btn-ghost text-[10px] px-2.5 py-1 rounded-lg border border-slate-800 hover:border-slate-600 hover:text-slate-300 cursor-pointer"
              @click="setResolution(0, 0)"
            >
              {{ $t('settings.window.res_auto') }}
            </button>
          </div>
        </div>
      </div>

      <!-- ================= SECTION 3: THEMES & PERSONALIZATION ================= -->
      <div v-show="activeCategory === 'all' || activeCategory === 'appearance'" class="mb-4 p-4 rounded-2xl bg-well/40 border border-slate-800/80 space-y-4">
        <div class="flex items-center justify-between pb-2 border-b border-slate-800/60">
          <div class="flex items-center gap-2">
            <div class="w-6 h-6 rounded-lg bg-accent/15 border border-accent/40 flex items-center justify-center text-accent-bright shadow-[0_0_12px_var(--color-accent-glow)]">
              <svg class="w-3.5 h-3.5" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
                <path stroke-linecap="round" stroke-linejoin="round" d="M7 21a4 4 0 01-4-4V5a2 2 0 012-2h4a2 2 0 012 2v12a4 4 0 01-4 4zm0 0h12a2 2 0 002-2v-4a2 2 0 00-2-2h-2.343M11 7.343l1.657-1.657a2 2 0 012.828 0l2.829 2.829a2 2 0 010 2.828l-8.486 8.485M7 17h.01" />
              </svg>
            </div>
            <div>
              <span class="text-xs font-bold text-white block">{{ $t('settings.theme.curated_title') }}</span>
              <span class="text-[10px] text-slate-400">{{ $t('settings.theme.curated_subtitle') }}</span>
            </div>
          </div>
          <span class="text-[10px] text-accent font-mono uppercase tracking-wider font-bold">
            {{ activeCuratedThemeName }}
          </span>
        </div>

        <!-- Curated Themes Selector (Clean, 1-Click Cards) -->
        <div class="grid grid-cols-2 sm:grid-cols-3 gap-2.5">
          <button
            v-for="curated in CURATED_THEMES"
            :key="curated.id"
            type="button"
            class="flex items-center gap-2.5 p-2.5 rounded-xl border text-left transition-all group relative overflow-hidden cursor-pointer"
            :class="
              activeCuratedId === curated.id
                ? 'border-accent bg-accent/10 shadow-[0_0_14px_var(--color-accent-glow)] ring-1 ring-accent/30'
                : 'border-edge bg-well/70 hover:border-accent/40 hover:shadow-[0_0_14px_var(--color-accent-glow)] hover:bg-well'
            "
            @click="selectCuratedTheme(curated)"
          >
            <!-- Visual Swatch Token -->
            <div
              class="w-8 h-8 rounded-lg shrink-0 border border-slate-700/80 p-1 flex items-center justify-center relative shadow-sm"
              :style="{ backgroundColor: curated.bg }"
            >
              <div
                class="w-5 h-5 rounded-md flex items-center justify-center"
                :style="{ backgroundColor: curated.card }"
              >
                <span
                  class="w-2.5 h-2.5 rounded-full"
                  :style="{ backgroundColor: curated.accent, boxShadow: `0 0 6px ${curated.accent}` }"
                ></span>
              </div>
            </div>

            <div class="min-w-0 flex-1">
              <div
                class="text-xs font-bold truncate transition-colors"
                :class="activeCuratedId === curated.id ? 'text-white' : 'text-slate-200 group-hover:text-white'"
              >
                {{ curated.name }}
              </div>
              <div class="text-[10px] text-slate-400 truncate mt-0.5">
                {{ curated.subtitle }}
              </div>
            </div>

            <svg
              v-if="activeCuratedId === curated.id"
              class="w-4 h-4 text-accent shrink-0"
              fill="none"
              viewBox="0 0 24 24"
              stroke="currentColor"
              stroke-width="3"
            >
              <polyline points="20 6 9 17 4 12" />
            </svg>
          </button>
        </div>

        <!-- Collapsible Dropdown for Advanced UI Customization -->
        <div class="pt-1">
          <button
            type="button"
            class="w-full flex items-center justify-between p-2.5 rounded-xl bg-well/80 border border-edge hover:border-slate-700 text-slate-300 hover:text-white transition-all select-none cursor-pointer group"
            @click="showAdvancedTheme = !showAdvancedTheme"
          >
            <div class="flex items-center gap-2.5">
              <svg class="w-4 h-4 text-accent shrink-0" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                <circle cx="12" cy="12" r="3" />
                <path d="M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 0 1 0 2.83 2 2 0 0 1-2.83 0l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 0 1-2 2 2 2 0 0 1-2-2v-.09A1.65 1.65 0 0 0 9 19.4a1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 0 1-2.83 0 2 2 0 0 1 0-2.83l.06-.06a1.65 1.65 0 0 0 .33-1.82 1.65 1.65 0 0 0-1.51-1H3a2 2 0 0 1-2-2 2 2 0 0 1 2-2h.09A1.65 1.65 0 0 0 4.6 9a1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 0 1 0-2.83 2 2 0 0 1 2.83 0l.06.06a1.65 1.65 0 0 0 1.82.33H9a1.65 1.65 0 0 0 1-1.51V3a2 2 0 0 1 2-2 2 2 0 0 1 2 2v.09a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 0 1 2.83 0 2 2 0 0 1 0 2.83l-.06.06a1.65 1.65 0 0 0-.33 1.82V9a1.65 1.65 0 0 0 1.51 1H21a2 2 0 0 1 2 2 2 2 0 0 1-2 2h-.09a1.65 1.65 0 0 0-1.51 1z" />
              </svg>
              <span class="text-xs font-bold text-slate-200 group-hover:text-white">
                {{ $t('settings.theme.advanced_studio') }}
              </span>
              <span class="text-[10px] bg-slate-800/90 text-slate-400 border border-slate-700 px-2 py-0.5 rounded-md font-mono">
                Granular Hex &amp; Glass
              </span>
            </div>
            <svg
              class="w-4 h-4 text-slate-400 group-hover:text-accent transition-transform duration-200 shrink-0"
              :class="{ 'rotate-180': showAdvancedTheme }"
              fill="none"
              viewBox="0 0 24 24"
              stroke="currentColor"
            >
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M19 9l-7 7-7-7" />
            </svg>
          </button>

          <!-- Collapsible Body -->
          <div v-show="showAdvancedTheme" class="mt-3.5 space-y-4 p-3.5 rounded-xl bg-well/40 border border-edge/80">
            <!-- Granular 1: Accent Color -->
            <div>
              <span class="text-xs font-semibold text-slate-300 block mb-2">Accent Highlight Color</span>
              <div class="grid grid-cols-2 sm:grid-cols-4 gap-2 mb-2.5">
                <button
                  v-for="preset in THEME_PRESETS"
                  :key="preset.id"
                  type="button"
                  class="flex items-center gap-2 p-2 rounded-xl border text-left transition-all group relative overflow-hidden cursor-pointer"
                  :class="
                    settings.theme === preset.id
                      ? 'border-accent bg-accent/15 shadow-[0_0_12px_var(--color-accent-glow)]'
                      : 'border-edge bg-well/60 hover:border-accent/40 hover:shadow-[0_0_12px_var(--color-accent-glow)] hover:bg-well'
                  "
                  @click="selectTheme(preset.id)"
                >
                  <span
                    class="w-3.5 h-3.5 rounded-full shrink-0 shadow-sm transition-transform group-hover:scale-110"
                    :style="{ backgroundColor: preset.hex, boxShadow: `0 0 8px ${preset.hex}80` }"
                  ></span>
                  <div class="min-w-0 flex-1">
                    <div
                      class="text-xs font-semibold truncate transition-colors"
                      :class="settings.theme === preset.id ? 'text-white' : 'text-slate-300 group-hover:text-white'"
                    >
                      {{ preset.name.split(' ')[1] || preset.name }}
                    </div>
                  </div>
                  <svg
                    v-if="settings.theme === preset.id"
                    class="w-3.5 h-3.5 text-accent shrink-0"
                    fill="none"
                    viewBox="0 0 24 24"
                    stroke="currentColor"
                    stroke-width="3"
                  >
                    <polyline points="20 6 9 17 4 12" />
                  </svg>
                </button>

                <!-- Custom Accent Card -->
                <button
                  type="button"
                  class="flex items-center gap-2 p-2 rounded-xl border text-left transition-all group relative overflow-hidden cursor-pointer"
                  :class="
                    settings.theme === 'custom'
                      ? 'border-accent bg-accent/15 shadow-[0_0_12px_var(--color-accent-glow)]'
                      : 'border-edge bg-well/60 hover:border-accent/40 hover:shadow-[0_0_12px_var(--color-accent-glow)] hover:bg-well'
                  "
                  @click="selectCustomTheme"
                >
                  <span
                    class="w-3.5 h-3.5 rounded-full shrink-0 shadow-sm transition-transform group-hover:scale-110"
                    :style="{ backgroundColor: settings.customAccent || '#47d2c9', boxShadow: `0 0 8px ${settings.customAccent || '#47d2c9'}80` }"
                  ></span>
                  <div class="min-w-0 flex-1">
                    <div
                      class="text-xs font-semibold truncate transition-colors"
                      :class="settings.theme === 'custom' ? 'text-white' : 'text-slate-300 group-hover:text-white'"
                    >
                      Custom
                    </div>
                  </div>
                  <svg
                    v-if="settings.theme === 'custom'"
                    class="w-3.5 h-3.5 text-accent shrink-0"
                    fill="none"
                    viewBox="0 0 24 24"
                    stroke="currentColor"
                    stroke-width="3"
                  >
                    <polyline points="20 6 9 17 4 12" />
                  </svg>
                </button>
              </div>

              <!-- Custom Accent Hex Input -->
              <div v-if="settings.theme === 'custom'" class="flex items-center gap-3 p-2.5 rounded-xl bg-well border border-edge mb-3">
                <input
                  type="color"
                  :value="settings.customAccent || '#47d2c9'"
                  class="w-8 h-8 rounded-lg cursor-pointer bg-transparent border-0 p-0"
                  @input="onCustomColorInput"
                />
                <div class="flex-1">
                  <span class="text-[10px] text-slate-400 font-mono block">Custom Accent Hex</span>
                  <input
                    v-model="settings.customAccent"
                    type="text"
                    maxlength="7"
                    placeholder="#47d2c9"
                    class="z-input font-mono text-xs py-1 px-2 mt-0.5"
                    @input="onCustomColorInput"
                  />
                </div>
              </div>
            </div>

            <!-- Granular 2: Background Canvas & Surface Theme -->
            <div class="pt-3 border-t border-edge/60">
              <span class="text-xs font-semibold text-slate-300 block mb-2">Background &amp; Surface Tone</span>
              <div class="grid grid-cols-2 sm:grid-cols-3 gap-2 mb-2.5">
                <button
                  v-for="bgPreset in BG_THEME_PRESETS"
                  :key="bgPreset.id"
                  type="button"
                  class="flex items-center gap-2.5 p-2 rounded-xl border text-left transition-all group relative overflow-hidden cursor-pointer"
                  :class="
                    settings.bgTheme === bgPreset.id
                      ? 'border-accent bg-accent/10 shadow-[0_0_12px_var(--color-accent-glow)]'
                      : 'border-edge bg-well/60 hover:border-accent/40 hover:shadow-[0_0_12px_var(--color-accent-glow)] hover:bg-well'
                  "
                  @click="selectBgTheme(bgPreset.id)"
                >
                  <div
                    class="w-4 h-4 rounded-md shrink-0 border border-slate-700/80 p-0.5 flex items-center justify-center shadow-sm"
                    :style="{ backgroundColor: bgPreset.bg }"
                  >
                    <div class="w-2 h-2 rounded-sm" :style="{ backgroundColor: bgPreset.card }"></div>
                  </div>
                  <div class="min-w-0 flex-1">
                    <div
                      class="text-xs font-semibold truncate transition-colors"
                      :class="settings.bgTheme === bgPreset.id ? 'text-white' : 'text-slate-300 group-hover:text-white'"
                    >
                      {{ bgPreset.name }}
                    </div>
                  </div>
                  <svg
                    v-if="settings.bgTheme === bgPreset.id"
                    class="w-3.5 h-3.5 text-accent shrink-0"
                    fill="none"
                    viewBox="0 0 24 24"
                    stroke="currentColor"
                    stroke-width="3"
                  >
                    <polyline points="20 6 9 17 4 12" />
                  </svg>
                </button>

                <!-- Custom Background Card -->
                <button
                  type="button"
                  class="flex items-center gap-2.5 p-2 rounded-xl border text-left transition-all group relative overflow-hidden cursor-pointer"
                  :class="
                    settings.bgTheme === 'custom'
                      ? 'border-accent bg-accent/10 shadow-[0_0_12px_var(--color-accent-glow)]'
                      : 'border-edge bg-well/60 hover:border-accent/40 hover:shadow-[0_0_12px_var(--color-accent-glow)] hover:bg-well'
                  "
                  @click="selectCustomBgTheme"
                >
                  <div
                    class="w-4 h-4 rounded-md shrink-0 border border-slate-700/80 p-0.5 flex items-center justify-center shadow-sm"
                    :style="{ backgroundColor: settings.customBg || '#070b0f' }"
                  >
                    <div class="w-2 h-2 rounded-sm" :style="{ backgroundColor: settings.customCardBg || '#0e1622' }"></div>
                  </div>
                  <div class="min-w-0 flex-1">
                    <div
                      class="text-xs font-semibold truncate transition-colors"
                      :class="settings.bgTheme === 'custom' ? 'text-white' : 'text-slate-300 group-hover:text-white'"
                    >
                      Custom Canvas
                    </div>
                  </div>
                  <svg
                    v-if="settings.bgTheme === 'custom'"
                    class="w-3.5 h-3.5 text-accent shrink-0"
                    fill="none"
                    viewBox="0 0 24 24"
                    stroke="currentColor"
                    stroke-width="3"
                  >
                    <polyline points="20 6 9 17 4 12" />
                  </svg>
                </button>
              </div>

              <!-- Custom Background Picker Expandable Row -->
              <div v-if="settings.bgTheme === 'custom'" class="flex items-center gap-3 p-2.5 rounded-xl bg-well border border-edge mb-3">
                <div class="flex items-center gap-2">
                  <input
                    type="color"
                    :value="settings.customBg || '#070b0f'"
                    class="w-7 h-7 rounded-lg cursor-pointer bg-transparent border-0 p-0"
                    @input="onCustomBgInput"
                  />
                  <span class="text-[10px] text-slate-400 font-mono">Base</span>
                </div>
                <div class="flex items-center gap-2">
                  <input
                    type="color"
                    :value="settings.customCardBg || '#0e1622'"
                    class="w-7 h-7 rounded-lg cursor-pointer bg-transparent border-0 p-0"
                    @input="onCustomCardBgInput"
                  />
                  <span class="text-[10px] text-slate-400 font-mono">Cards</span>
                </div>
                <div class="flex-1">
                  <input
                    v-model="settings.customBg"
                    type="text"
                    maxlength="7"
                    placeholder="#070b0f"
                    class="z-input font-mono text-xs py-1 px-2"
                    @input="onCustomBgInput"
                  />
                </div>
              </div>
            </div>

            <!-- Granular 3: Component Geometry & Glassmorphism Options -->
            <div class="grid grid-cols-1 sm:grid-cols-2 gap-3 pt-3 border-t border-edge/60">
              <!-- Button Corner Radius -->
              <div>
                <span class="text-xs font-semibold text-slate-300 block mb-1.5">Button &amp; Component Shape</span>
                <div class="flex p-1 rounded-xl bg-well border border-edge gap-1">
                  <button
                    v-for="bStyle in BUTTON_STYLES"
                    :key="bStyle.id"
                    type="button"
                    class="flex-1 py-1.5 text-xs font-semibold text-center transition-all cursor-pointer select-none"
                    :class="
                      settings.buttonStyle === bStyle.id
                        ? 'bg-accent text-accent-ink font-bold shadow-[0_0_10px_var(--color-accent-glow)]'
                        : 'text-slate-400 hover:text-white'
                    "
                    :style="{ borderRadius: bStyle.btnRadius }"
                    @click="selectButtonStyle(bStyle.id)"
                  >
                    {{ bStyle.name }}
                  </button>
                </div>
              </div>

              <!-- Glassmorphism Effect -->
              <div>
                <span class="text-xs font-semibold text-slate-300 block mb-1.5">Glassmorphism &amp; Blur</span>
                <div class="flex p-1 rounded-xl bg-well border border-edge gap-1">
                  <button
                    v-for="gEffect in GLASS_EFFECTS"
                    :key="gEffect.id"
                    type="button"
                    class="flex-1 py-1.5 text-xs font-semibold text-center transition-all cursor-pointer select-none"
                    :class="
                      settings.glassEffect === gEffect.id
                        ? 'bg-accent text-accent-ink font-bold shadow-[0_0_10px_var(--color-accent-glow)]'
                        : 'text-slate-400 hover:text-white'
                    "
                    :style="{ borderRadius: 'var(--border-radius-btn)' }"
                    @click="selectGlassEffect(gEffect.id)"
                  >
                    {{ gEffect.name.split(' ')[0] }}
                  </button>
                </div>
              </div>
            </div>

            <!-- Interactive Live Theme Showcase -->
            <div class="pt-3 border-t border-edge/60 space-y-3">
              <div class="flex items-center justify-between">
                <span class="text-xs font-bold text-slate-200">Live Preview Showcase</span>
                <span class="text-[10px] text-slate-500 font-mono">Dynamic CSS Variables</span>
              </div>

              <div class="flex flex-wrap items-center gap-2">
                <button type="button" class="z-btn z-btn-accent text-xs px-3.5 py-1.5">Primary CTA</button>
                <button type="button" class="z-btn z-btn-secondary text-xs px-3 py-1.5">Secondary</button>
                <button type="button" class="z-btn z-btn-danger text-xs px-3 py-1.5">Danger</button>
                <label class="inline-flex items-center gap-2 cursor-pointer ml-1">
                  <input type="checkbox" checked class="zircon-check" />
                  <span class="text-xs text-slate-300">Checked</span>
                </label>
                <span class="z-toggle z-toggle-on !h-5 !w-9"><span class="z-toggle-thumb !h-3.5 !w-3.5 !translate-x-4"></span></span>
              </div>

              <!-- Mini Progress Bar -->
              <div class="w-full h-1.5 bg-well rounded-full overflow-hidden border border-edge">
                <div class="h-full w-2/3 zircon-progress-bar rounded-full"></div>
              </div>
            </div>
          </div>
        </div>
      </div>

      <!-- ================= SECTION: ACCESSIBILITY & ERGONOMICS ================= -->
      <div v-show="activeCategory === 'all' || activeCategory === 'accessibility'" class="mb-4 p-4 rounded-2xl bg-well/40 border border-slate-800/80 space-y-4">
        <div class="flex items-center justify-between pb-2 border-b border-slate-800/60">
          <div class="flex items-center gap-2">
            <div class="w-6 h-6 rounded-lg bg-accent/15 border border-accent/40 flex items-center justify-center text-accent-bright shadow-[0_0_12px_var(--color-accent-glow)]">
              <svg class="w-3.5 h-3.5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                <path stroke-linecap="round" stroke-linejoin="round" d="M15 12a3 3 0 11-6 0 3 3 0 016 0z" />
                <path stroke-linecap="round" stroke-linejoin="round" d="M2.458 12C3.732 7.943 7.523 5 12 5c4.478 0 8.268 2.943 9.542 7-1.274 4.057-5.064 7-9.542 7-4.477 0-8.268-2.943-9.542-7z" />
              </svg>
            </div>
            <div>
              <span class="text-xs font-bold text-white block">{{ $t('accessibility.title') }}</span>
              <span class="text-[10px] text-slate-400">{{ $t('accessibility.subtitle') }}</span>
            </div>
          </div>
          <span class="text-[9px] bg-accent/15 text-accent-bright border border-accent/30 px-2 py-0.5 rounded-full font-bold uppercase tracking-wider font-mono">
            A11Y
          </span>
        </div>

        <!-- Language Selector -->
        <div>
          <div class="flex items-center justify-between mb-1.5">
            <span class="z-label font-semibold text-slate-200 text-xs">{{ $t('settings.language_label') }}</span>
            <span class="text-[10px] font-mono text-cyan-300">{{ currentLocale }}</span>
          </div>
          <select
            :value="currentLocale"
            class="z-input w-full text-xs py-2 px-3 cursor-pointer"
            @change="onLocaleChange($event.target.value)"
          >
            <option v-for="loc in AVAILABLE_LOCALES" :key="loc.code" :value="loc.code">
              {{ loc.flag }} {{ loc.native }} ({{ loc.name }})
            </option>
          </select>
          <p class="z-label mt-1 text-slate-400 text-[11px]">
            {{ $t('settings.language_desc') }}
          </p>
        </div>

        <!-- Typography / Font Preference -->
        <div class="pt-3 border-t border-slate-800/60">
          <div class="flex items-center justify-between mb-2">
            <span class="z-label font-semibold text-slate-200 text-xs">{{ $t('accessibility.font_family_label') }}</span>
          </div>
          <div class="grid grid-cols-1 sm:grid-cols-3 gap-2 mb-2">
            <button
              type="button"
              class="p-2.5 rounded-xl border text-left transition-all cursor-pointer flex flex-col justify-between"
              :class="settings.fontFamily === 'default'
                ? 'border-accent bg-accent/15 text-white shadow-[0_0_12px_var(--color-accent-glow)] font-bold'
                : 'border-edge bg-well/60 text-slate-400 hover:text-white hover:border-accent/40'"
              @click="setFontFamily('default')"
            >
              <span class="text-xs font-semibold block">{{ $t('accessibility.font_default') }}</span>
              <span class="text-[10px] text-slate-400 mt-1">{{ $t('accessibility.font_default_sub') }}</span>
            </button>
            <button
              type="button"
              class="p-2.5 rounded-xl border text-left transition-all cursor-pointer flex flex-col justify-between font-dyslexic"
              :class="settings.fontFamily === 'open-dyslexic'
                ? 'border-accent bg-accent/15 text-white shadow-[0_0_12px_var(--color-accent-glow)] font-bold'
                : 'border-edge bg-well/60 text-slate-400 hover:text-white hover:border-accent/40'"
              @click="setFontFamily('open-dyslexic')"
            >
              <span class="text-xs font-semibold block">{{ $t('accessibility.font_dyslexic') }}</span>
              <span class="text-[10px] text-slate-400 mt-1">{{ $t('accessibility.font_dyslexic_sub') }}</span>
            </button>
            <button
              type="button"
              class="p-2.5 rounded-xl border text-left transition-all cursor-pointer flex flex-col justify-between font-lexend"
              :class="settings.fontFamily === 'lexend'
                ? 'border-accent bg-accent/15 text-white shadow-[0_0_12px_var(--color-accent-glow)] font-bold'
                : 'border-edge bg-well/60 text-slate-400 hover:text-white hover:border-accent/40'"
              @click="setFontFamily('lexend')"
            >
              <span class="text-xs font-semibold block">{{ $t('accessibility.font_lexend') }}</span>
              <span class="text-[10px] text-slate-400 mt-1">{{ $t('accessibility.font_lexend_sub') }}</span>
            </button>
          </div>

          <!-- Typography Live Preview Box -->
          <div class="p-2.5 rounded-xl bg-well/80 border border-edge text-slate-300 text-xs">
            <span class="text-[10px] text-cyan-400 font-mono block mb-1">{{ $t('accessibility.font_preview_label') }}</span>
            <p :class="{ 'font-dyslexic': settings.fontFamily === 'open-dyslexic', 'font-lexend': settings.fontFamily === 'lexend' }">
              {{ $t('accessibility.font_preview') }}
            </p>
          </div>
          <p class="z-label mt-1.5 text-slate-400 text-[11px] leading-relaxed">
            {{ $t('accessibility.font_hint') }}
          </p>
        </div>

        <!-- UI Scaling -->
        <div class="pt-3 border-t border-slate-800/60">
          <div class="flex items-center justify-between mb-1.5">
            <span class="z-label font-semibold text-slate-200 text-xs">{{ $t('accessibility.ui_scale_label') }}</span>
            <span class="text-[10px] font-mono text-cyan-300">{{ Math.round((settings.uiScale || 1.0) * 100) }}%</span>
          </div>
          <div class="grid grid-cols-4 gap-2">
            <button
              v-for="scale in [1.0, 1.15, 1.25, 1.4]"
              :key="scale"
              type="button"
              class="py-1.5 px-2 rounded-xl text-xs font-semibold border text-center transition-all cursor-pointer"
              :class="(settings.uiScale || 1.0) === scale
                ? 'border-accent bg-accent text-accent-ink font-bold shadow-[0_0_10px_var(--color-accent-glow)]'
                : 'border-edge bg-well/60 text-slate-400 hover:text-white hover:border-accent/40'"
              @click="setUiScale(scale)"
            >
              {{ Math.round(scale * 100) }}%
            </button>
          </div>
          <p class="z-label mt-1 text-slate-400 text-[11px]">
            {{ $t('accessibility.ui_scale_desc') }}
          </p>
        </div>

        <!-- High Contrast Mode Toggle -->
        <div class="pt-3 border-t border-slate-800/60">
          <label class="flex items-start gap-3 cursor-pointer select-none group">
            <input
              v-model="settings.highContrast"
              type="checkbox"
              class="zircon-check mt-0.5"
              @change="applyAccessibilitySettings"
            />
            <div>
              <span class="z-label font-semibold text-slate-200 group-hover:text-cyan-300 transition-colors text-xs flex items-center gap-1.5">
                {{ $t('accessibility.high_contrast_label') }}
              </span>
              <p class="z-label mt-1 text-slate-400 text-[11px] leading-relaxed">
                {{ $t('accessibility.high_contrast_desc') }}
              </p>
            </div>
          </label>
        </div>

        <!-- Reduced Motion Toggle -->
        <div class="pt-3 border-t border-slate-800/60">
          <label class="flex items-start gap-3 cursor-pointer select-none group">
            <input
              v-model="settings.reduceMotion"
              type="checkbox"
              class="zircon-check mt-0.5"
              @change="applyAccessibilitySettings"
            />
            <div>
              <span class="z-label font-semibold text-slate-200 group-hover:text-cyan-300 transition-colors text-xs flex items-center gap-1.5">
                {{ $t('accessibility.reduce_motion_label') }}
              </span>
              <p class="z-label mt-1 text-slate-400 text-[11px] leading-relaxed">
                {{ $t('accessibility.reduce_motion_desc') }}
              </p>
            </div>
          </label>
        </div>

        <!-- Colorblind Status Icons Toggle -->
        <div class="pt-3 border-t border-slate-800/60">
          <label class="flex items-start gap-3 cursor-pointer select-none group">
            <input
              v-model="settings.colorblindBadges"
              type="checkbox"
              class="zircon-check mt-0.5"
              @change="applyAccessibilitySettings"
            />
            <div>
              <span class="z-label font-semibold text-slate-200 group-hover:text-cyan-300 transition-colors text-xs flex items-center gap-1.5">
                {{ $t('accessibility.colorblind_badges_label') }}
              </span>
              <p class="z-label mt-1 text-slate-400 text-[11px] leading-relaxed">
                {{ $t('accessibility.colorblind_badges_desc') }}
              </p>
            </div>
          </label>
        </div>
      </div>

      <!-- ================= SECTION 4: CLOUD & INTEGRATIONS ================= -->
      <div v-show="activeCategory === 'all' || activeCategory === 'integrations'" class="mb-4 p-4 rounded-2xl bg-well/40 border border-slate-800/80 space-y-4">
        <div class="flex items-center justify-between pb-2 border-b border-slate-800/60">
          <div class="flex items-center gap-2">
            <div class="w-6 h-6 rounded-lg bg-accent/15 border border-accent/40 flex items-center justify-center text-accent-bright shadow-[0_0_12px_var(--color-accent-glow)]">
              <svg class="w-3.5 h-3.5" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
                <path stroke-linecap="round" stroke-linejoin="round" d="M3 15a4 4 0 004 4h9a5 5 0 10-.1-9.999 5.002 5.002 0 00-9.78 2.096A4.001 4.001 0 003 15z" />
              </svg>
            </div>
            <div>
              <span class="text-xs font-bold text-white block">Cloud &amp; Integrations</span>
              <span class="text-[10px] text-slate-400">OneDrive synchronization and Discord RPC presence</span>
            </div>
          </div>
          <span class="text-[9px] bg-accent/15 text-accent-bright border border-accent/30 px-2 py-0.5 rounded-full font-bold uppercase tracking-wider font-mono">
            Connected
          </span>
        </div>

        <!-- OneDrive Cloud Sync & Universal Backup -->
        <div class="space-y-3">
          <div class="flex items-center justify-between">
            <div class="flex items-center gap-2">
              <span class="z-label font-semibold text-slate-200 text-xs">{{ $t('settings.onedrive.title') }}</span>
              <span
                class="text-[9px] font-mono px-2 py-0.5 rounded-full font-bold uppercase tracking-wider"
                :class="cloudSyncStatus?.available ? 'bg-accent/15 text-accent-bright border border-accent/30 shadow-[0_0_8px_var(--color-accent-glow)]' : 'bg-slate-800 text-slate-400 border border-slate-700'"
              >
                {{ cloudSyncStatus?.available ? $t('settings.onedrive.connected') : $t('settings.onedrive.not_detected') }}
              </span>
            </div>

            <button
              v-if="cloudSyncStatus?.available"
              type="button"
              class="text-[11px] font-semibold text-slate-400 hover:text-white flex items-center gap-1.5 transition-colors cursor-pointer"
              @click="fetchCloudSyncStatus"
              :title="$t('settings.onedrive.refresh')"
            >
              <svg class="w-3.5 h-3.5 text-slate-400" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M4 4v5h.582m15.356 2A8.001 8.001 0 004.582 9m0 0H9m11 11v-5h-.581m0 0a8.003 8.003 0 01-15.357-2m15.357 2H15" />
              </svg>
              <span>{{ $t('settings.onedrive.refresh') }}</span>
            </button>
          </div>

          <p class="z-label text-slate-400 text-[11px] leading-relaxed">
            {{ $t('settings.onedrive.desc') }}
          </p>

          <!-- Primary Backup Action Button -->
          <div class="flex items-center gap-3 pt-0.5">
            <button
              type="button"
              class="z-btn-accent text-xs px-4 py-2.5 rounded-xl font-bold flex items-center gap-2 shadow-md hover:shadow-[0_0_16px_var(--color-accent-glow)] transition-all cursor-pointer active:scale-95 disabled:opacity-50 disabled:pointer-events-none"
              :disabled="syncingCloud || !cloudSyncStatus?.available"
              @click="triggerCloudBackup"
            >
              <span v-if="syncingCloud" class="w-3.5 h-3.5 border-2 border-accent-ink border-t-transparent rounded-full animate-spin"></span>
              <svg v-else class="w-4 h-4 fill-none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2.2">
                <path stroke-linecap="round" stroke-linejoin="round" d="M3 15a4 4 0 004 4h9a5 5 0 10-.1-9.999 5.002 5.002 0 00-9.78 2.096A4.001 4.001 0 003 15z" />
                <path stroke-linecap="round" stroke-linejoin="round" d="M12 12v6m0-6l-2.5 2.5m2.5-2.5l2.5 2.5" />
              </svg>
              <span>{{ syncingCloud ? $t('settings.onedrive.backing_up') : $t('settings.onedrive.backup_now') }}</span>
            </button>

            <span v-if="cloudSyncStatus?.lastSyncTimestamp" class="text-[11px] font-mono text-slate-400">
              {{ $t('settings.onedrive.last_backup', { time: formatRelativeTime(cloudSyncStatus.lastSyncTimestamp) }) }}
            </span>
          </div>

          <!-- Status & Sync Folder Readout -->
          <div v-if="cloudSyncStatus?.available" class="bg-well/70 border border-slate-800 rounded-xl p-3 mb-2 text-xs space-y-2">
            <div class="flex items-center justify-between text-[11px] text-slate-400">
              <span class="flex items-center gap-1.5">
                <span class="w-2 h-2 rounded-full bg-emerald-400 animate-pulse"></span>
                {{ $t('settings.onedrive.local_dir') }}
                <strong class="text-slate-200 font-mono select-all">{{ cloudSyncStatus.syncDir }}</strong>
              </span>
              <span v-if="cloudSyncStatus.lastSyncTimestamp" class="font-mono text-slate-500">
                {{ $t('settings.onedrive.last_backup', { time: formatRelativeTime(cloudSyncStatus.lastSyncTimestamp) }) }}
              </span>
            </div>

            <!-- Cloud Backups List -->
            <div v-if="cloudBackups.length > 0" class="pt-2 border-t border-slate-800/80 space-y-1.5">
              <div class="text-[10px] uppercase font-bold text-slate-400 font-mono tracking-wider flex items-center justify-between">
                <span>{{ $t('settings.onedrive.history_title', { count: cloudBackups.length }) }}</span>
                <span>{{ fmtBytes(cloudSyncStatus.totalCloudBytes) }} Total</span>
              </div>
              <div class="max-h-36 overflow-y-auto space-y-1 pr-1 custom-scrollbar">
                <div
                  v-for="b in cloudBackups"
                  :key="b.filename"
                  class="bg-slate-950/70 border border-slate-800/80 rounded-lg px-2.5 py-1.5 flex items-center justify-between text-xs hover:border-slate-700 transition"
                >
                  <div class="flex items-center gap-2 min-w-0">
                    <svg class="w-3.5 h-3.5 text-accent-bright shrink-0" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
                      <path stroke-linecap="round" stroke-linejoin="round" d="M3 7v10a2 2 0 002 2h14a2 2 0 002-2V9a2 2 0 00-2-2h-6l-2-2H5a2 2 0 00-2 2z" />
                    </svg>
                    <span class="font-mono text-[11px] text-slate-200 truncate">{{ b.filename }}</span>
                    <span class="text-[10px] text-slate-500 font-mono shrink-0">{{ fmtBytes(b.sizeBytes) }}</span>
                  </div>
                  <button
                    class="text-[10px] font-bold text-accent-bright hover:text-white bg-accent/15 hover:bg-accent/25 border border-accent/30 px-2.5 py-1 rounded-lg transition shrink-0 cursor-pointer shadow-sm active:scale-95"
                    :disabled="restoringCloud"
                    @click="restoreCloudBackup(b.filename)"
                  >
                    {{ restoringCloud ? $t('settings.onedrive.restoring') : $t('settings.onedrive.restore_btn') }}
                  </button>
                </div>
              </div>
            </div>
            <div v-else class="text-[11px] text-slate-500 italic pt-1">
              {{ $t('settings.onedrive.no_backups') }}
            </div>
          </div>

          <div v-else class="bg-amber-950/20 border border-amber-500/30 rounded-xl p-3 text-xs text-amber-300 flex items-start gap-2">
            <svg class="w-4 h-4 text-amber-400 shrink-0 mt-0.5" fill="none" viewBox="0 0 24 24" stroke="currentColor">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 9v2m0 4h.01m-6.938 4h13.856c1.54 0 2.502-1.667 1.732-3L13.732 4c-.77-1.333-2.694-1.333-3.464 0L3.34 16c-.77 1.333.192 3 1.732 3z" />
            </svg>
            <div>
              <strong>{{ $t('settings.onedrive.not_detected_title') }}</strong>
              {{ $t('settings.onedrive.not_detected_desc') }}
            </div>
          </div>
        </div>

        <!-- Discord Rich Presence & Privacy Modes -->
        <div class="pt-3 border-t border-slate-800/60">
          <div class="flex items-center justify-between mb-2">
            <div class="flex items-center gap-2">
              <div class="w-6 h-6 rounded-lg bg-accent/15 border border-accent/40 flex items-center justify-center text-accent-bright shadow-[0_0_12px_var(--color-accent-glow)]">
                <svg class="w-3.5 h-3.5 fill-current" viewBox="0 0 24 24">
                  <path d="M20.317 4.37a19.791 19.791 0 0 0-4.885-1.515.074.074 0 0 0-.079.037c-.21.375-.444.864-.608 1.25a18.27 18.27 0 0 0-5.487 0 12.64 12.64 0 0 0-.617-1.25.077.077 0 0 0-.079-.037A19.736 19.736 0 0 0 3.677 4.37a.07.07 0 0 0-.032.027C.533 9.046-.32 13.58.099 18.057a.082.082 0 0 0 .031.057 19.9 19.9 0 0 0 5.993 3.03.078.078 0 0 0 .084-.028c.462-.63.874-1.295 1.226-1.994.021-.041.001-.09-.041-.106a13.107 13.107 0 0 1-1.872-.892.077.077 0 0 1-.008-.128 10.2 10.2 0 0 0 .372-.292.074.074 0 0 1 .077-.01c3.929 1.793 8.18 1.793 12.061 0a.074.074 0 0 1 .078.01c.12.098.246.198.373.292a.077.077 0 0 1-.006.127 12.299 12.299 0 0 1-1.873.894.077.077 0 0 0-.041.107c.36.698.772 1.362 1.225 1.993a.076.076 0 0 0 .084.028 19.839 19.839 0 0 0 6.002-3.03.077.077 0 0 0 .032-.054c.5-5.177-.838-9.674-3.549-13.66a.061.061 0 0 0-.031-.028zM8.02 15.33c-1.183 0-2.157-1.085-2.157-2.419 0-1.333.956-2.419 2.157-2.419 1.21 0 2.176 1.096 2.157 2.42 0 1.333-.956 2.418-2.157 2.418zm7.975 0c-1.183 0-2.157-1.085-2.157-2.419 0-1.333.955-2.419 2.157-2.419 1.21 0 2.176 1.096 2.157 2.42 0 1.333-.946 2.418-2.157 2.418z" />
                </svg>
              </div>
              <span class="z-label font-semibold text-slate-200 text-xs">{{ $t('settings.discord.enable_label') }}</span>
            </div>
            <label class="flex items-center gap-2 cursor-pointer select-none">
              <span class="text-xs text-slate-400 font-medium">{{ settings.discordRpc ? $t('settings.discord.status_enabled') : $t('settings.discord.status_disabled') }}</span>
              <input
                v-model="settings.discordRpc"
                type="checkbox"
                class="zircon-check"
              />
            </label>
          </div>
          <p class="z-label text-slate-400 text-[11px] mb-2.5">
            {{ $t('settings.discord.desc') }}
          </p>

          <!-- Privacy Modes (when enabled) -->
          <div v-if="settings.discordRpc" class="space-y-2">
            <!-- Public Mode -->
            <button
              type="button"
              class="w-full flex items-start gap-3 p-3 rounded-xl border text-left transition-all cursor-pointer group"
              :class="settings.discordRpcMode === 'public'
                ? 'border-accent bg-accent/10 shadow-[0_0_12px_var(--color-accent-glow)] ring-1 ring-accent/30'
                : 'border-slate-800/80 bg-well/70 hover:border-accent/40 hover:bg-well'"
              @click="settings.discordRpcMode = 'public'"
            >
              <div
                class="mt-0.5 w-4 h-4 rounded-full border flex items-center justify-center shrink-0 transition-colors"
                :class="settings.discordRpcMode === 'public' ? 'border-accent bg-accent/20' : 'border-slate-600'"
              >
                <div v-if="settings.discordRpcMode === 'public'" class="w-2 h-2 rounded-full bg-accent shadow-[0_0_6px_var(--color-accent-glow)]"></div>
              </div>
              <div class="flex-1 min-w-0">
                <div class="flex items-center gap-2">
                  <span class="text-xs font-bold transition-colors" :class="settings.discordRpcMode === 'public' ? 'text-white' : 'text-slate-300 group-hover:text-white'">
                    {{ $t('settings.discord.mode_public_title') }}
                  </span>
                  <span class="text-[9px] font-bold text-cyan-300 font-mono bg-cyan-500/15 border border-cyan-500/30 px-1.5 py-0.2 rounded">
                    {{ $t('settings.discord.mode_public_badge') }}
                  </span>
                </div>
                <div class="text-[11px] text-slate-400 mt-0.5">
                  {{ $t('settings.discord.mode_public_desc') }}
                </div>
              </div>
            </button>

            <!-- Presence Only Mode -->
            <button
              type="button"
              class="w-full flex items-start gap-3 p-3 rounded-xl border text-left transition-all cursor-pointer group"
              :class="settings.discordRpcMode === 'presence_only'
                ? 'border-accent bg-accent/10 shadow-[0_0_12px_var(--color-accent-glow)] ring-1 ring-accent/30'
                : 'border-slate-800/80 bg-well/70 hover:border-accent/40 hover:bg-well'"
              @click="settings.discordRpcMode = 'presence_only'"
            >
              <div
                class="mt-0.5 w-4 h-4 rounded-full border flex items-center justify-center shrink-0 transition-colors"
                :class="settings.discordRpcMode === 'presence_only' ? 'border-accent bg-accent/20' : 'border-slate-600'"
              >
                <div v-if="settings.discordRpcMode === 'presence_only'" class="w-2 h-2 rounded-full bg-accent shadow-[0_0_6px_var(--color-accent-glow)]"></div>
              </div>
              <div class="flex-1 min-w-0">
                <span class="text-xs font-bold transition-colors" :class="settings.discordRpcMode === 'presence_only' ? 'text-white' : 'text-slate-300 group-hover:text-white'">
                  {{ $t('settings.discord.mode_presence_title') }}
                </span>
                <div class="text-[11px] text-slate-400 mt-0.5">
                  {{ $t('settings.discord.mode_presence_desc') }}
                </div>
              </div>
            </button>

            <!-- Incognito Mode -->
            <button
              type="button"
              class="w-full flex items-start gap-3 p-3 rounded-xl border text-left transition-all cursor-pointer group"
              :class="settings.discordRpcMode === 'incognito'
                ? 'border-accent bg-accent/10 shadow-[0_0_12px_var(--color-accent-glow)] ring-1 ring-accent/30'
                : 'border-slate-800/80 bg-well/70 hover:border-accent/40 hover:bg-well'"
              @click="settings.discordRpcMode = 'incognito'"
            >
              <div
                class="mt-0.5 w-4 h-4 rounded-full border flex items-center justify-center shrink-0 transition-colors"
                :class="settings.discordRpcMode === 'incognito' ? 'border-accent bg-accent/20' : 'border-slate-600'"
              >
                <div v-if="settings.discordRpcMode === 'incognito'" class="w-2 h-2 rounded-full bg-accent shadow-[0_0_6px_var(--color-accent-glow)]"></div>
              </div>
              <div class="flex-1 min-w-0">
                <span class="text-xs font-bold transition-colors" :class="settings.discordRpcMode === 'incognito' ? 'text-white' : 'text-slate-300 group-hover:text-white'">
                  {{ $t('settings.discord.mode_incognito_title') }}
                </span>
                <div class="text-[11px] text-slate-400 mt-0.5">
                  {{ $t('settings.discord.mode_incognito_desc') }}
                </div>
              </div>
            </button>
          </div>
        </div>
      </div>

      <!-- ================= SECTION 5: ADVANCED JAVA & ENGINE ================= -->
      <div v-show="activeCategory === 'all' || activeCategory === 'java'" class="mb-4 p-4 rounded-2xl bg-well/40 border border-slate-800/80 space-y-4">
        <div class="flex items-center justify-between pb-2 border-b border-slate-800/60">
          <div class="flex items-center gap-2">
            <div class="w-6 h-6 rounded-lg bg-accent/15 border border-accent/40 flex items-center justify-center text-accent-bright shadow-[0_0_12px_var(--color-accent-glow)]">
              <svg class="w-3.5 h-3.5" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
                <path stroke-linecap="round" stroke-linejoin="round" d="M10 20l4-16m4 4l4 4-4 4M6 16l-4-4 4-4" />
              </svg>
            </div>
            <div>
              <span class="text-xs font-bold text-white block">{{ $t('settings.java.title') }}</span>
              <span class="text-[10px] text-slate-400">{{ $t('settings.java.subtitle') }}</span>
            </div>
          </div>
          <span class="text-[9px] bg-accent/15 text-accent-bright border border-accent/30 px-2 py-0.5 rounded-full font-bold uppercase tracking-wider font-mono">
            JVM / GC
          </span>
        </div>

        <!-- Java Path Override -->
        <div>
          <div class="flex items-center justify-between mb-1.5">
            <span class="z-label font-semibold text-slate-300 text-xs">{{ $t('settings.java.custom_path_label') }}</span>
            <button
              v-if="settings.javaPathOverride"
              class="text-[10px] text-cyan-400 hover:text-cyan-300 transition-colors cursor-pointer"
              @click="settings.javaPathOverride = ''"
            >
              {{ $t('settings.java.reset_auto') }}
            </button>
          </div>
          <div class="flex gap-2">
            <input
              v-model="settings.javaPathOverride"
              class="z-input flex-1 font-mono text-xs"
              :placeholder="$t('settings.java.auto_placeholder')"
            />
            <button
              type="button"
              class="z-btn-ghost text-xs px-3 py-1.5 rounded-xl border border-slate-700 hover:border-cyan-400 hover:text-cyan-300 shrink-0 cursor-pointer"
              @click="browseJavaPath"
            >
              {{ $t('settings.java.browse_btn') }}
            </button>
          </div>
          <p class="z-label mt-1 text-slate-400 text-[11px]">
            {{ $t('settings.java.custom_path_desc') }}
          </p>
        </div>

        <!-- JVM Arguments & GC Presets -->
        <div class="pt-3 border-t border-slate-800/60">
          <div class="flex items-center justify-between mb-1.5">
            <span class="z-label font-semibold text-slate-300 text-xs">{{ $t('settings.java.custom_args_label') }}</span>
            <button
              class="text-[10px] text-cyan-400 hover:text-cyan-300 font-mono transition-colors cursor-pointer"
              @click="applyJvmPreset('clear')"
            >
              {{ $t('settings.java.clear_custom_args') }}
            </button>
          </div>
          <div class="flex flex-wrap gap-1.5 mb-2">
            <button
              type="button"
              class="px-2.5 py-1 rounded-lg text-[10px] font-bold bg-accent/20 text-accent-bright border border-accent/40 hover:bg-accent/30 shadow-[0_0_10px_var(--color-accent-glow)] flex items-center gap-1.5 transition-all cursor-pointer"
              :title="$t('settings.java.autotune_tooltip')"
              @click="applyJvmPreset('autotune')"
            >
              <svg class="w-3 h-3 fill-current" viewBox="0 0 24 24">
                <path d="M13 10V3L4 14h7v7l9-11h-7z" />
              </svg>
              <span>{{ $t('settings.java.autotune_btn') }}</span>
            </button>
            <button
              type="button"
              class="z-btn-ghost text-[10px] px-2 py-0.5 rounded-lg border border-slate-700/80 hover:border-cyan-400/50 hover:text-cyan-300 cursor-pointer"
              @click="applyJvmPreset('zgc')"
            >
              {{ $t('settings.java.zgc_btn') }}
            </button>
            <button
              type="button"
              class="z-btn-ghost text-[10px] px-2 py-0.5 rounded-lg border border-slate-700/80 hover:border-cyan-400/50 hover:text-cyan-300 cursor-pointer"
              @click="applyJvmPreset('aikar')"
            >
              {{ $t('settings.java.aikar_btn') }}
            </button>
            <button
              type="button"
              class="z-btn-ghost text-[10px] px-2 py-0.5 rounded-lg border border-slate-700/80 hover:border-emerald-400/50 hover:text-emerald-300 flex items-center gap-1 cursor-pointer"
              @click="applyJvmPreset('battery')"
            >
              <svg class="w-3 h-3 text-emerald-400" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
                <rect x="2" y="7" width="16" height="10" rx="2" />
                <path d="M22 11v2" stroke-linecap="round" />
              </svg>
              <span>{{ $t('settings.java.battery_btn') }}</span>
            </button>
            <button
              type="button"
              class="z-btn-ghost text-[10px] px-2 py-0.5 rounded-lg border border-slate-700/80 hover:border-cyan-400/50 hover:text-cyan-300 cursor-pointer"
              @click="applyJvmPreset('default')"
            >
              {{ $t('settings.java.default_btn') }}
            </button>
          </div>
          <textarea
            v-model="settings.customJvmArgs"
            rows="2"
            class="z-input w-full font-mono text-xs p-2 leading-relaxed resize-y"
            :placeholder="$t('settings.java.custom_args_placeholder')"
          ></textarea>
          <p class="z-label mt-1 text-slate-400 text-[11px]">
            {{ $t('settings.java.custom_args_hint') }}
          </p>
        </div>

        <!-- P2P Join-by-Code Custom Mods Developer Mode Toggle -->
        <div class="pt-3 border-t border-slate-800/60">
          <label class="flex items-start gap-3 cursor-pointer select-none group">
            <input
              v-model="settings.allowUnverifiedP2pMods"
              type="checkbox"
              class="zircon-check mt-0.5"
            />
            <div>
              <span class="z-label font-semibold text-slate-200 group-hover:text-amber-300 transition-colors text-xs flex items-center gap-1.5">
                {{ $t('settings.java.developer_mode') }}
                <span class="text-[9px] font-mono bg-amber-500/10 text-amber-400 border border-amber-500/30 px-1 py-0.2 rounded font-semibold">{{ $t('settings.java.developer_mode_badge') }}</span>
              </span>
              <p class="z-label mt-1 text-slate-400 text-[11px] leading-relaxed">
                {{ $t('settings.java.developer_mode_desc') }}
              </p>
            </div>
          </label>
        </div>
      </div>

      <!-- ================= SECTION 6: LAUNCHER DIAGNOSTICS & LOGS ================= -->
      <div v-show="activeCategory === 'all' || activeCategory === 'diagnostics'" class="mb-4 p-4 rounded-2xl bg-well/40 border border-slate-800/80 space-y-3">
        <div class="flex items-center justify-between pb-2 border-b border-slate-800/60">
          <div class="flex items-center gap-2">
            <div class="w-6 h-6 rounded-lg bg-accent/15 border border-accent/40 flex items-center justify-center text-accent-bright shadow-[0_0_12px_var(--color-accent-glow)]">
              <svg class="w-3.5 h-3.5" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
                <path stroke-linecap="round" stroke-linejoin="round" d="M9 12h6m-6 4h6m2 5H7a2 2 0 01-2-2V5a2 2 0 012-2h5.586a1 1 0 01.707.293l5.414 5.414a1 1 0 01.293.707V19a2 2 0 01-2 2z" />
              </svg>
            </div>
            <div>
              <span class="text-xs font-bold text-white block">{{ $t('settings.diagnostics.title') }}</span>
              <span class="text-[10px] text-slate-400">{{ $t('settings.diagnostics.subtitle') }}</span>
            </div>
          </div>
          <div class="flex gap-1.5">
            <button class="z-btn-ghost text-[10px] px-2 py-0.5 rounded cursor-pointer" @click="refreshLogs">{{ $t('settings.diagnostics.refresh') }}</button>
            <button class="z-btn-ghost text-[10px] px-2 py-0.5 rounded cursor-pointer" @click="copyLogs">{{ $t('common.copy') }}</button>
            <button class="z-btn-ghost text-[10px] px-2 py-0.5 rounded hover:text-red-400 cursor-pointer" @click="clearLogs">{{ $t('common.clear') }}</button>
          </div>
        </div>

        <pre class="bg-black/40 backdrop-blur-md border border-slate-800/80 rounded-xl p-3 text-[10px] font-mono leading-relaxed text-slate-300 h-40 overflow-y-auto whitespace-pre-wrap select-text shadow-inner">{{ logText || $t('settings.diagnostics.empty') }}</pre>
        <p v-if="copiedAt" class="z-label text-cyan-300 font-medium text-xs">{{ copiedAt }}</p>
      </div>

    </div>

    <!-- Right Column: Last Played Minecraft Instance Log -->
    <div class="w-full xl:flex-1 z-card flex flex-col p-6 bg-[#0e1622]/90 border border-slate-800/80 min-w-[360px]">
      <div class="flex items-center justify-between mb-3">
        <div>
          <h3 class="text-white font-bold text-base">{{ $t('settings.mc_log.title') }}</h3>
          <p class="text-xs text-slate-400 mt-0.5">
            <span v-if="mcLog" class="inline-flex items-center gap-1.5">
              <span class="text-cyan-300 font-bold">{{ mcLog.instanceName }}</span>
              <span class="bg-slate-900 text-[10px] px-2 py-0.5 rounded-md text-slate-300 border border-slate-800 uppercase font-mono font-bold">{{ mcLog.instanceType }}</span>
            </span>
            <span v-else>{{ $t('settings.mc_log.no_log') }}</span>
          </p>
        </div>
        <div class="flex gap-2">
          <button class="z-btn-ghost text-[11px] px-2.5 py-1 rounded-lg" @click="refreshMcLog">{{ $t('common.refresh') }}</button>
          <button class="z-btn-ghost text-[11px] px-2.5 py-1 rounded-lg" @click="copyMcLogs">{{ $t('common.copy') }}</button>
          <button class="z-btn-ghost text-[11px] px-2.5 py-1 rounded-lg hover:text-red-400" @click="clearMcLogs">{{ $t('common.clear') }}</button>
        </div>
      </div>

      <!-- Filters (Info, Warnings, Errors + Search) -->
      <div class="flex flex-wrap items-center gap-3 mt-1 pt-3 border-t border-slate-800/80">
        <input
          v-model="mcSearchQuery"
          type="text"
          :placeholder="$t('settings.mc_log.filter_placeholder')"
          class="bg-[#070b10] border border-slate-700/80 rounded-xl px-3.5 py-1.5 text-xs text-white placeholder-slate-500 focus:outline-none focus:border-cyan-400 flex-1 min-w-[160px]"
        />
        <div class="flex items-center gap-3 text-xs">
          <label class="flex items-center gap-1.5 cursor-pointer text-slate-300 hover:text-white select-none">
            <input type="checkbox" v-model="mcFilters.info" class="zircon-check" /> {{ $t('settings.mc_log.filter_info') }}
          </label>
          <label class="flex items-center gap-1.5 cursor-pointer text-yellow-400 hover:text-yellow-300 select-none">
            <input type="checkbox" v-model="mcFilters.warnings" class="zircon-check" /> {{ $t('settings.mc_log.filter_warnings') }}
          </label>
          <label class="flex items-center gap-1.5 cursor-pointer text-red-400 hover:text-red-300 select-none">
            <input type="checkbox" v-model="mcFilters.errors" class="zircon-check" /> {{ $t('settings.mc_log.filter_errors') }}
          </label>
          <label class="flex items-center gap-1.5 cursor-pointer text-slate-400 hover:text-slate-200 select-none ml-auto">
            <input type="checkbox" v-model="autoScroll" class="zircon-check" /> {{ $t('settings.mc_log.auto_scroll') }}
          </label>
        </div>
      </div>

      <!-- Log Terminal Box -->
      <div
        ref="mcLogBox"
        class="bg-black/40 backdrop-blur-md border border-slate-800/80 rounded-xl p-4 text-[11px] font-mono leading-relaxed min-h-[300px] max-h-[440px] overflow-y-auto whitespace-pre-wrap space-y-1 mt-3.5 select-text shadow-inner"
      >
        <template v-if="filteredMcLogLines.length > 0">
          <div
            v-for="(line, idx) in filteredMcLogLines"
            :key="idx"
            :class="mcLogColor(line)"
          >{{ line }}</div>
        </template>
        <p v-else-if="mcLogLines.length > 0" class="text-slate-500 text-xs italic">
          No log lines match current filters.
        </p>
        <p v-else class="text-slate-500 text-xs italic">
          No log output captured yet. Launch a server or offline instance to view logs.
        </p>
      </div>

      <div class="flex items-center justify-between text-[10px] text-slate-500 font-mono mt-2 px-1">
        <span>Showing {{ filteredMcLogLines.length }} of {{ mcLogLines.length }} lines</span>
        <span v-if="copiedMcAt" class="text-cyan-400 font-sans font-medium">{{ copiedMcAt }}</span>
      </div>
    </div>
  </div>
</template>

<script setup>
import { computed, nextTick, onMounted, ref } from 'vue';
import { api, onGameOutput, pickFile, EXE_FILTER, fmtBytes } from '../lib/api';
import { check as checkUpdate } from '@tauri-apps/plugin-updater';
import { relaunch } from '@tauri-apps/plugin-process';
import { THEME_PRESETS, BG_THEME_PRESETS, BUTTON_STYLES, GLASS_EFFECTS, CURATED_THEMES, detectCuratedTheme, applyTheme } from '../lib/theme';
import { t, currentLocale, setLocale, AVAILABLE_LOCALES } from '../lib/i18n';

// OneDrive Cloud Sync state & helpers
const cloudSyncStatus = ref(null);
const cloudBackups = ref([]);
const syncingCloud = ref(false);
const restoringCloud = ref(false);

function formatRelativeTime(timestamp) {
  if (!timestamp) return 'Never';
  const diff = Math.max(0, Date.now() - Number(timestamp));
  const mins = Math.floor(diff / 60000);
  if (mins < 1) return 'Just now';
  if (mins < 60) return `${mins}m ago`;
  const hours = Math.floor(mins / 60);
  if (hours < 24) return `${hours}h ago`;
  const days = Math.floor(hours / 24);
  return `${days}d ago`;
}

async function fetchCloudSyncStatus() {
  try {
    const status = await api.getCloudSyncStatus();
    cloudSyncStatus.value = status;
    if (status?.available) {
      const backups = await api.listCloudBackups();
      cloudBackups.value = backups || [];
    }
  } catch (err) {
    console.warn('Could not load cloud sync status:', err);
  }
}

async function triggerCloudBackup() {
  syncingCloud.value = true;
  try {
    const entry = await api.triggerCloudBackup();
    window.dispatchEvent(
      new CustomEvent('zircon-status', {
        detail: `OneDrive Cloud Backup created: ${entry.filename} (${fmtBytes(entry.sizeBytes)})`,
      })
    );
    await fetchCloudSyncStatus();
  } catch (err) {
    window.dispatchEvent(
      new CustomEvent('zircon-status', { detail: `Cloud backup failed: ${err}` })
    );
  } finally {
    syncingCloud.value = false;
  }
}

async function restoreCloudBackup(filename) {
  if (!window.confirm(`Restore settings and configuration from '${filename}'? This will restore your saved launcher options and servers.`)) {
    return;
  }
  restoringCloud.value = true;
  try {
    await api.restoreFromCloudBackup(filename);
    window.dispatchEvent(
      new CustomEvent('zircon-status', { detail: `Settings successfully restored from ${filename}!` })
    );
    const loaded = await api.getSettings();
    if (loaded) settings.value = { ...settings.value, ...loaded };
    await fetchCloudSyncStatus();
  } catch (err) {
    window.dispatchEvent(
      new CustomEvent('zircon-status', { detail: `Restore from cloud failed: ${err}` })
    );
  } finally {
    restoringCloud.value = false;
  }
}

const settings = ref({
  memoryGb: 4,
  discordRpc: true,
  discordRpcMode: 'public',
  gpuPreference: 'dedicated',
  customJvmArgs: '',
  javaPathOverride: '',
  windowWidth: 0,
  windowHeight: 0,
  startFullscreen: false,
  allowUnverifiedP2pMods: false,
  theme: 'zircon-cyan',
  customAccent: '#47d2c9',
  bgTheme: 'deep-void',
  customBg: '#070b0f',
  customCardBg: '#0e1622',
  buttonStyle: 'rounded',
  glassEffect: 'standard',
  // Accessibility
  fontFamily: (typeof localStorage !== 'undefined' && localStorage.getItem('zircon_font')) || 'default',
  uiScale: (typeof localStorage !== 'undefined' && Number(localStorage.getItem('zircon_ui_scale'))) || 1.0,
  reduceMotion: typeof localStorage !== 'undefined' && localStorage.getItem('zircon_reduce_motion') === 'true',
  highContrast: typeof localStorage !== 'undefined' && localStorage.getItem('zircon_high_contrast') === 'true',
  colorblindBadges: typeof localStorage !== 'undefined' && localStorage.getItem('zircon_colorblind') === 'true',
  soundCues: typeof localStorage !== 'undefined' && localStorage.getItem('zircon_sound_cues') !== 'false',
});
const saving = ref(false);
const savedAt = ref('');
const logText = ref('');
const copiedAt = ref('');

const systemRamInfo = ref({ totalRamGb: 16, recommendedRamGb: 6 });
const sliderMax = computed(() => Math.min(32, Math.max(8, systemRamInfo.value.totalRamGb || 16)));
const isHighRam = computed(() => {
  const total = systemRamInfo.value.totalRamGb || 16;
  return settings.value.memoryGb > (total * 0.8);
});

const gpuInfo = ref({ supported: false, adapters: [], preferred: 'dedicated', activeGpuName: null });
const dedicatedGpu = computed(() => gpuInfo.value.adapters?.find(a => a.isDedicated));
const integratedGpu = computed(() => gpuInfo.value.adapters?.find(a => !a.isDedicated));
const activeGpuLabel = computed(() => {
  if (settings.value.gpuPreference === 'dedicated') {
    return dedicatedGpu.value ? dedicatedGpu.value.name : 'Dedicated GPU';
  }
  if (settings.value.gpuPreference === 'integrated') {
    return integratedGpu.value ? integratedGpu.value.name : 'Integrated GPU';
  }
  return 'System Default';
});

const activeCategory = ref('all');
const categories = [
  { id: 'all', label: 'All Settings', icon: 'M4 6h16M4 12h16M4 18h16' },
  { id: 'performance', label: 'Performance', icon: 'M13 10V3L4 14h7v7l9-11h-7z' },
  { id: 'display', label: 'Display', icon: 'M9.75 17L9 20l-1 1h8l-1-1-.75-3M3 13h18M5 17h14a2 2 0 002-2V5a2 2 0 00-2-2H5a2 2 0 00-2 2v10a2 2 0 002 2z' },
  { id: 'appearance', label: 'Appearance', icon: 'M7 21a4 4 0 01-4-4V5a2 2 0 012-2h4a2 2 0 012 2v12a4 4 0 01-4 4zm0 0h12a2 2 0 002-2v-4a2 2 0 00-2-2h-2.343M11 7.343l1.657-1.657a2 2 0 012.828 0l2.829 2.829a2 2 0 010 2.828l-8.486 8.485M7 17h.01' },
  { id: 'accessibility', label: 'Accessibility', icon: 'M15 12a3 3 0 11-6 0 3 3 0 016 0z M2.458 12C3.732 7.943 7.523 5 12 5c4.478 0 8.268 2.943 9.542 7-1.274 4.057-5.064 7-9.542 7-4.477 0-8.268-2.943-9.542-7z' },
  { id: 'integrations', label: 'Cloud & Discord', icon: 'M3 15a4 4 0 004 4h9a5 5 0 10-.1-9.999 5.002 5.002 0 00-9.78 2.096A4.001 4.001 0 003 15z' },
  { id: 'java', label: 'Java & Engine', icon: 'M10 20l4-16m4 4l4 4-4 4M6 16l-4-4 4-4' },
  { id: 'diagnostics', label: 'Diagnostics', icon: 'M9 12h6m-6 4h6m2 5H7a2 2 0 01-2-2V5a2 2 0 012-2h5.586a1 1 0 01.707.293l5.414 5.414a1 1 0 01.293.707V19a2 2 0 01-2 2z' },
];

const showAdvancedTheme = ref(false);

const activeCuratedId = computed(() => {
  return detectCuratedTheme(settings.value.theme, settings.value.bgTheme);
});

const activeCuratedThemeName = computed(() => {
  const match = CURATED_THEMES.find(c => c.id === activeCuratedId.value);
  return match ? match.name : 'Custom Studio';
});

function selectCuratedTheme(curated) {
  settings.value.theme = curated.theme;
  settings.value.bgTheme = curated.bgTheme;
  settings.value.buttonStyle = curated.buttonStyle;
  settings.value.glassEffect = curated.glassEffect;
  applyActiveTheme();
}

function applyActiveTheme() {
  applyTheme({
    theme: settings.value.theme,
    customAccent: settings.value.customAccent,
    bgTheme: settings.value.bgTheme,
    customBg: settings.value.customBg,
    customCardBg: settings.value.customCardBg,
    buttonStyle: settings.value.buttonStyle,
    glassEffect: settings.value.glassEffect,
  });
}

function onLocaleChange(code) {
  setLocale(code);
}

function setFontFamily(font) {
  settings.value.fontFamily = font;
  applyAccessibilitySettings();
}

function setUiScale(scale) {
  settings.value.uiScale = scale;
  applyAccessibilitySettings();
}

function applyAccessibilitySettings() {
  if (typeof document === 'undefined') return;
  const root = document.documentElement;

  // Font
  root.classList.remove('font-dyslexic', 'font-lexend');
  if (settings.value.fontFamily === 'open-dyslexic') {
    root.classList.add('font-dyslexic');
  } else if (settings.value.fontFamily === 'lexend') {
    root.classList.add('font-lexend');
  }
  localStorage.setItem('zircon_font', settings.value.fontFamily);

  // UI Scale
  if (settings.value.uiScale && settings.value.uiScale !== 1.0) {
    root.setAttribute('data-ui-scale', String(settings.value.uiScale));
  } else {
    root.removeAttribute('data-ui-scale');
  }
  localStorage.setItem('zircon_ui_scale', String(settings.value.uiScale));

  // Reduce Motion
  if (settings.value.reduceMotion) {
    root.classList.add('reduce-motion');
  } else {
    root.classList.remove('reduce-motion');
  }
  localStorage.setItem('zircon_reduce_motion', String(settings.value.reduceMotion));

  // High Contrast
  if (settings.value.highContrast) {
    root.classList.add('high-contrast');
  } else {
    root.classList.remove('high-contrast');
  }
  localStorage.setItem('zircon_high_contrast', String(settings.value.highContrast));

  // Colorblind & Sound Cues
  localStorage.setItem('zircon_colorblind', String(settings.value.colorblindBadges));
  localStorage.setItem('zircon_sound_cues', String(settings.value.soundCues));
}

function selectTheme(themeId) {
  settings.value.theme = themeId;
  applyActiveTheme();
}

function onCustomColorInput(e) {
  settings.value.customAccent = e.target.value;
  if (settings.value.theme === 'custom') {
    applyActiveTheme();
  }
}

function selectCustomTheme() {
  settings.value.theme = 'custom';
  if (!settings.value.customAccent) {
    settings.value.customAccent = '#47d2c9';
  }
  applyActiveTheme();
}

function selectBgTheme(bgId) {
  settings.value.bgTheme = bgId;
  applyActiveTheme();
}

function onCustomBgInput(e) {
  settings.value.customBg = e.target.value;
  if (settings.value.bgTheme === 'custom') {
    applyActiveTheme();
  }
}

function onCustomCardBgInput(e) {
  settings.value.customCardBg = e.target.value;
  if (settings.value.bgTheme === 'custom') {
    applyActiveTheme();
  }
}

function selectCustomBgTheme() {
  settings.value.bgTheme = 'custom';
  if (!settings.value.customBg) {
    settings.value.customBg = '#070b0f';
  }
  if (!settings.value.customCardBg) {
    settings.value.customCardBg = '#0e1622';
  }
  applyActiveTheme();
}

function selectButtonStyle(styleId) {
  settings.value.buttonStyle = styleId;
  applyActiveTheme();
}

function selectGlassEffect(effectId) {
  settings.value.glassEffect = effectId;
  applyActiveTheme();
}

function applyJvmPreset(preset) {
  if (preset === 'default') {
    settings.value.customJvmArgs = '-XX:+UseG1GC';
  } else if (preset === 'aikar') {
    settings.value.customJvmArgs =
      '-XX:+UseG1GC -XX:+ParallelRefProcEnabled -XX:MaxGCPauseMillis=200 -XX:+UnlockExperimentalVMOptions -XX:+DisableExplicitGC -XX:+AlwaysPreTouch -XX:G1NewSizePercent=30 -XX:G1MaxNewSizePercent=40 -XX:G1ReservePercent=20 -XX:G1HeapWastePercent=5 -XX:G1MixedGCCountTarget=4 -XX:InitiatingHeapOccupancyPercent=15 -XX:G1MixedGCLiveThresholdPercent=90 -XX:G1RSetUpdatingPauseTimePercent=5 -XX:SurvivorRatio=32 -XX:+PerfDisableSharedMem -XX:MaxTenuringThreshold=1';
  } else if (preset === 'zgc') {
    settings.value.customJvmArgs = '-XX:+UseZGC -XX:+ZGenerational';
  } else if (preset === 'battery') {
    settings.value.customJvmArgs = '-XX:+UseG1GC -XX:MaxGCPauseMillis=400 -XX:ConcGCThreads=1 -XX:ParallelGCThreads=2';
  } else if (preset === 'autotune') {
    const totalRam = systemRamInfo.value.totalRamGb || 16;
    if (totalRam >= 16) {
      settings.value.memoryGb = 8;
    } else if (totalRam >= 8) {
      settings.value.memoryGb = 6;
    } else {
      settings.value.memoryGb = 4;
    }
    settings.value.customJvmArgs = '-XX:+UseZGC -XX:+ZGenerational';
    window.dispatchEvent(
      new CustomEvent('zircon-status', {
        detail: `Auto-Tuned: Allocated ${settings.value.memoryGb} GB RAM & Generational ZGC for optimal performance.`,
      })
    );
  } else if (preset === 'clear') {
    settings.value.customJvmArgs = '';
  }
}

function setResolution(w, h) {
  settings.value.windowWidth = w;
  settings.value.windowHeight = h;
}

async function browseJavaPath() {
  try {
    const picked = await pickFile(EXE_FILTER);
    if (picked) {
      settings.value.javaPathOverride = picked;
    }
  } catch (err) {
    console.warn('File picker error:', err);
  }
}


// Launcher Version & Updates
const launcherVersion = ref('');
const checkingUpdate = ref(false);
const updating = ref(false);
const updateProgress = ref(0);
const updateInfo = ref(null);
const updateStatusMessage = ref('');
const updateStatusClass = ref('');
let rawUpdateInstance = null;

// Minecraft Instance Log state
const mcLog = ref(null);
const mcLogLines = ref([]);
const mcSearchQuery = ref('');
const mcFilters = ref({ info: true, warnings: true, errors: true });
const autoScroll = ref(true);
const mcLogBox = ref(null);
const copiedMcAt = ref('');

onMounted(async () => {
  try {
    const loaded = await api.getSettings();
    if (loaded) {
      settings.value = {
        ...settings.value,
        ...loaded,
        theme: loaded.theme || 'zircon-cyan',
        bgTheme: loaded.bgTheme || 'deep-void',
        buttonStyle: loaded.buttonStyle || 'rounded',
        glassEffect: loaded.glassEffect || 'standard',
      };
    }
    applyActiveTheme();
    applyAccessibilitySettings();
  } catch {
    applyActiveTheme();
    applyAccessibilitySettings();
  }
  try {
    launcherVersion.value = await api.getLauncherVersion();
  } catch {
    launcherVersion.value = '0.4.2';
  }
  try {
    const ram = await api.getSystemRamInfo();
    if (ram) systemRamInfo.value = ram;
  } catch (e) {
    console.warn('Could not load system RAM info:', e);
  }
  try {
    const gpu = await api.getGpuInfo();
    if (gpu) {
      gpuInfo.value = gpu;
      if (!settings.value.gpuPreference) {
        settings.value.gpuPreference = gpu.preferred || 'dedicated';
      }
    }
  } catch (e) {
    console.warn('Could not load GPU info:', e);
  }
  fetchCloudSyncStatus();
  refreshLogs();
  refreshMcLog();

  onGameOutput((line) => {
    mcLogLines.value.push(line);
    if (autoScroll.value) {
      scrollToBottom();
    }
  });
});

async function checkForUpdates() {
  checkingUpdate.value = true;
  updateStatusMessage.value = '';
  updateInfo.value = null;
  rawUpdateInstance = null;
  updateProgress.value = 0;
  api.logDebug('Manual launcher update check started...');
  try {
    const update = await checkUpdate();
    if (update?.available) {
      rawUpdateInstance = update;
      updateInfo.value = {
        version: update.version,
        currentVersion: update.currentVersion || launcherVersion.value,
        notes: update.body || '',
      };
      updateStatusMessage.value = `Update available: v${update.version} (current: v${update.currentVersion || launcherVersion.value})`;
      updateStatusClass.value = 'bg-cyan-500/10 border-cyan-500/30 text-cyan-300';
      api.logDebug(`Manual check: update available -> v${update.version}`);
    } else {
      updateStatusMessage.value = `You are on the latest version (v${launcherVersion.value || '0.4.2'}).`;
      updateStatusClass.value = 'bg-emerald-500/10 border-emerald-500/30 text-emerald-300';
      api.logDebug(`Manual check: up to date (v${launcherVersion.value})`);
    }
  } catch (err) {
    updateStatusMessage.value = `Update check error: ${err?.message || err}`;
    updateStatusClass.value = 'bg-red-500/10 border-red-500/30 text-red-300';
    api.logDebug(`Manual check error: ${err?.message || err}`);
  } finally {
    checkingUpdate.value = false;
    refreshLogs();
  }
}

async function installUpdate() {
  if (!rawUpdateInstance) return;
  updating.value = true;
  updateProgress.value = 0;
  const targetVer = rawUpdateInstance.version;
  updateStatusMessage.value = `Downloading update v${targetVer}...`;
  api.logDebug(`Starting download of v${targetVer}...`);
  try {
    let totalBytes = 0;
    let downloadedBytes = 0;
    await rawUpdateInstance.downloadAndInstall((event) => {
      if (event.event === 'Started') {
        totalBytes = event.data.contentLength || 0;
      } else if (event.event === 'Progress') {
        downloadedBytes += event.data.chunkLength || 0;
        const percent = totalBytes > 0 ? Math.min(100, Math.round((downloadedBytes / totalBytes) * 100)) : 0;
        updateProgress.value = percent / 100;
        updateStatusMessage.value = `Downloading update v${targetVer}... ${percent}%`;
      } else if (event.event === 'Finished') {
        updateProgress.value = 1;
        updateStatusMessage.value = 'Update downloaded. Restarting application...';
        api.logDebug('Launcher update downloaded. Restarting application...');
      }
    });
    refreshLogs();
    await relaunch();
  } catch (err) {
    updating.value = false;
    updateStatusMessage.value = `Failed to install update: ${err?.message || err}`;
    updateStatusClass.value = 'bg-red-500/10 border-red-500/30 text-red-300';
    api.logDebug(`Update install error: ${err?.message || err}`);
    refreshLogs();
  }
}

async function save() {
  saving.value = true;
  try {
    await api.saveSettings(settings.value);
    savedAt.value = 'Settings saved.';
    setTimeout(() => {
      savedAt.value = '';
    }, 2500);
  } catch (e) {
    savedAt.value = `Save error: ${e}`;
  } finally {
    saving.value = false;
  }
}

async function refreshLogs() {
  try {
    const logs = await api.getDebugLogs();
    logText.value = logs.join('\n');
  } catch (e) {
    logText.value = `Unable to fetch debug logs: ${e}`;
  }
}

async function copyLogs() {
  if (!logText.value) return;
  try {
    await navigator.clipboard.writeText(logText.value);
    copiedAt.value = 'Copied to clipboard!';
    setTimeout(() => {
      copiedAt.value = '';
    }, 2000);
  } catch {
    copiedAt.value = 'Failed to copy.';
  }
}

async function clearLogs() {
  try {
    await api.clearDebugLogs();
    logText.value = '';
  } catch (e) {
    console.warn('Failed to clear logs:', e);
  }
}

// -------------------------------------------------------------
// Minecraft Instance Log logic
// -------------------------------------------------------------

async function refreshMcLog() {
  try {
    const logInfo = await api.getLastInstanceLog();
    if (logInfo) {
      mcLog.value = logInfo;
      mcLogLines.value = logInfo.lines || [];
      if (autoScroll.value) {
        scrollToBottom();
      }
    } else {
      mcLog.value = null;
      mcLogLines.value = [];
    }
  } catch (err) {
    console.warn('Failed to load last instance log:', err);
  }
}

function scrollToBottom() {
  nextTick(() => {
    if (mcLogBox.value) {
      mcLogBox.value.scrollTop = mcLogBox.value.scrollHeight;
    }
  });
}

const filteredMcLogLines = computed(() => {
  const q = mcSearchQuery.value.toLowerCase().trim();
  return mcLogLines.value.filter((line) => {
    const lower = line.toLowerCase();
    const isErr = lower.includes('error') || lower.includes('exception') || lower.includes('fatal');
    const isWarn = !isErr && (lower.includes('warn') || lower.includes('warning'));
    const isInfo = !isErr && !isWarn;

    if (isErr && !mcFilters.value.errors) return false;
    if (isWarn && !mcFilters.value.warnings) return false;
    if (isInfo && !mcFilters.value.info) return false;

    if (q && !lower.includes(q)) return false;

    return true;
  });
});

function mcLogColor(line) {
  const lower = line.toLowerCase();
  if (lower.includes('error') || lower.includes('exception') || lower.includes('fatal')) {
    return 'text-[#f87171] font-semibold';
  }
  if (lower.includes('warn') || lower.includes('warning')) {
    return 'text-[#fbbf24]';
  }
  if (lower.includes('info')) {
    return 'text-slate-300';
  }
  if (lower.includes('debug')) {
    return 'text-slate-500';
  }
  return 'text-slate-400';
}

async function copyMcLogs() {
  if (!mcLogLines.value.length) return;
  try {
    await navigator.clipboard.writeText(filteredMcLogLines.value.join('\n'));
    copiedMcAt.value = 'Copied to clipboard!';
    setTimeout(() => {
      copiedMcAt.value = '';
    }, 2000);
  } catch {
    copiedMcAt.value = 'Failed to copy.';
  }
}

function clearMcLogs() {
  mcLogLines.value = [];
}
</script>
