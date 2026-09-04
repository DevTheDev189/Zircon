<template>
  <div class="h-full p-5 overflow-y-auto flex flex-col xl:flex-row gap-6 items-start">
    <!-- Left Column: Launcher Settings & Debug Logs -->
    <div class="w-full xl:w-[480px] z-card flex flex-col p-6 bg-[#0e1622]/90 border border-slate-800/80 shrink-0">
      <div class="flex items-center justify-between mb-5">
        <h3 class="text-white font-bold text-base">Settings</h3>
        <button class="z-btn-accent text-xs rounded-xl font-bold px-4 py-1.5 shadow-md hover:shadow-cyan-500/25" :disabled="saving" @click="save">
          {{ saving ? 'Saving…' : 'Save Settings' }}
        </button>
      </div>
      <p v-if="savedAt" class="z-label -mt-3 mb-4 text-cyan-300/90 font-medium text-xs">{{ savedAt }}</p>

      <!-- ================= SECTION: THEMES & PERSONALIZATION ================= -->
      <div class="mb-6 pb-6 border-b border-edge">
        <div class="flex items-center justify-between mb-2">
          <div class="flex items-center gap-2">
            <span class="z-label font-bold text-sm text-slate-100">Theme &amp; Personalization</span>
            <span class="text-[9px] bg-accent/15 text-accent border border-accent/30 px-2 py-0.5 rounded-full font-bold uppercase tracking-wider">
              Style
            </span>
          </div>
          <span class="text-[10px] text-accent font-mono uppercase tracking-wider font-bold">
            {{ activeCuratedThemeName }}
          </span>
        </div>
        <p class="z-label text-slate-400 text-xs mb-3.5">
          Select a curated color theme, or expand Advanced Customization for granular control over individual colors, dark backgrounds, buttons, and glass effects.
        </p>

        <!-- Curated Themes Selector (Clean, 1-Click Cards) -->
        <div class="grid grid-cols-2 sm:grid-cols-3 gap-2.5 mb-3.5">
          <button
            v-for="curated in CURATED_THEMES"
            :key="curated.id"
            type="button"
            class="flex items-center gap-2.5 p-2.5 rounded-xl border text-left transition-all group relative overflow-hidden cursor-pointer"
            :class="
              activeCuratedId === curated.id
                ? 'border-accent bg-accent/10 shadow-[0_0_14px_var(--color-accent-glow)] ring-1 ring-accent/30'
                : 'border-edge bg-well/70 hover:border-slate-700 hover:bg-well'
            "
            @click="selectCuratedTheme(curated)"
          >
            <!-- Visual Swatch Token: Background outer box + card inner box + glowing accent dot -->
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
                Advanced UI Customization
              </span>
              <span class="text-[10px] bg-slate-800/90 text-slate-400 border border-slate-700 px-2 py-0.5 rounded-md font-mono">
                Individual Colors &amp; Glass
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
                      : 'border-edge bg-well/60 hover:border-slate-700 hover:bg-well'
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
                      : 'border-edge bg-well/60 hover:border-slate-700 hover:bg-well'
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
                      : 'border-edge bg-well/60 hover:border-slate-700 hover:bg-well'
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
                      : 'border-edge bg-well/60 hover:border-slate-700 hover:bg-well'
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
                <span class="text-xs font-bold text-slate-200">Interactive Live Theme Showcase</span>
                <span class="text-[10px] text-slate-500 font-mono">Real-time preview</span>
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

      <!-- ================= SECTION 1: CORE PREFERENCES ================= -->
      <!-- RAM slider -->
      <div class="mb-5">
        <div class="flex items-center justify-between mb-2">
          <span class="z-label font-semibold text-slate-300">Max Memory Allocation (RAM)</span>
          <span class="text-xs font-bold text-cyan-300 font-mono bg-cyan-500/10 border border-cyan-500/30 px-2 py-0.5 rounded-md">{{ settings.memoryGb }} GB</span>
        </div>
        <input
          v-model.number="settings.memoryGb"
          type="range"
          min="2"
          max="16"
          step="1"
          class="w-full accent-cyan-400 cursor-pointer"
        />
        <div class="flex justify-between text-[10px] text-slate-500 font-mono mt-1">
          <span>2 GB (Minimal)</span><span>16 GB (Heavy Modpacks)</span>
        </div>
        <p class="z-label mt-1.5 text-slate-400 text-xs">
          Applied to offline instance launches (replaces -Xmx). Server launches default to 4 GB.
        </p>
      </div>

      <!-- Game Window & Resolution -->
      <div class="mb-5 pt-4 border-t border-slate-800/80">
        <span class="z-label font-semibold text-slate-200 block mb-2">Game Window &amp; Display</span>
        <label class="flex items-center gap-3 cursor-pointer select-none group mb-2.5">
          <input
            v-model="settings.startFullscreen"
            type="checkbox"
            class="zircon-check"
          />
          <span class="z-label text-slate-300 group-hover:text-white transition-colors">
            Start game in fullscreen mode
          </span>
        </label>
        <div v-if="!settings.startFullscreen" class="space-y-2">
          <div class="flex items-center gap-3">
            <div class="flex-1">
              <span class="text-[10px] text-slate-400 font-mono block mb-1">Width (px)</span>
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
              <span class="text-[10px] text-slate-400 font-mono block mb-1">Height (px)</span>
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
              class="z-btn-ghost text-[10px] px-2 py-0.5 rounded border border-slate-800 hover:border-slate-600 hover:text-cyan-300"
              @click="setResolution(1280, 720)"
            >
              720p
            </button>
            <button
              type="button"
              class="z-btn-ghost text-[10px] px-2 py-0.5 rounded border border-slate-800 hover:border-slate-600 hover:text-cyan-300"
              @click="setResolution(1920, 1080)"
            >
              1080p
            </button>
            <button
              type="button"
              class="z-btn-ghost text-[10px] px-2 py-0.5 rounded border border-slate-800 hover:border-slate-600 hover:text-cyan-300"
              @click="setResolution(2560, 1440)"
            >
              1440p
            </button>
            <button
              type="button"
              class="z-btn-ghost text-[10px] px-2 py-0.5 rounded border border-slate-800 hover:border-slate-600 hover:text-slate-300"
              @click="setResolution(0, 0)"
            >
              Auto
            </button>
          </div>
        </div>
      </div>

      <!-- Discord Rich Presence toggle -->
      <div class="mb-5 pt-4 border-t border-slate-800/80">
        <label class="flex items-start gap-3 cursor-pointer select-none group">
          <input
            v-model="settings.discordRpc"
            type="checkbox"
            class="zircon-check mt-0.5"
          />
          <div>
            <span class="z-label font-semibold text-slate-200 group-hover:text-white transition-colors">
              Enable Discord Rich Presence (RPC)
            </span>
            <p class="z-label mt-1 text-slate-400 text-xs leading-relaxed">
              Show playing status, server name, and modpack details on Discord.
            </p>
          </div>
        </label>
      </div>

      <!-- ================= SECTION 2: ADVANCED JAVA & ENGINE (COLLAPSIBLE) ================= -->
      <div class="pt-4 border-t border-slate-800/80">
        <button
          type="button"
          class="w-full flex items-center justify-between text-left py-1 text-slate-200 hover:text-white group transition-colors select-none"
          @click="showAdvancedJava = !showAdvancedJava"
        >
          <div class="flex items-center gap-2">
            <span class="z-label font-bold text-xs text-slate-200 group-hover:text-cyan-300">Advanced Java &amp; Engine Options</span>
            <span class="text-[10px] bg-slate-800 border border-slate-700 px-1.5 py-0.2 rounded text-slate-400 font-mono">JVM / GC</span>
          </div>
          <svg class="w-4 h-4 text-slate-400 group-hover:text-cyan-400 transition-transform duration-200" :class="{ 'rotate-180': showAdvancedJava }" fill="none" viewBox="0 0 24 24" stroke="currentColor">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M19 9l-7 7-7-7" />
          </svg>
        </button>

        <div v-show="showAdvancedJava" class="mt-3.5 space-y-4 pt-2 border-t border-slate-800/40">
          <!-- Java Path Override -->
          <div>
            <div class="flex items-center justify-between mb-1.5">
              <span class="z-label font-semibold text-slate-300 text-xs">Java Runtime Override</span>
              <button
                v-if="settings.javaPathOverride"
                class="text-[10px] text-cyan-400 hover:text-cyan-300 transition-colors"
                @click="settings.javaPathOverride = ''"
              >
                Reset to Auto
              </button>
            </div>
            <div class="flex gap-2">
              <input
                v-model="settings.javaPathOverride"
                class="z-input flex-1 font-mono text-xs"
                placeholder="Auto (Managed Temurin JDK from Adoptium)"
              />
              <button
                type="button"
                class="z-btn-ghost text-xs px-3 py-1.5 rounded-xl border border-slate-700 hover:border-cyan-400 hover:text-cyan-300 shrink-0"
                @click="browseJavaPath"
              >
                Browse…
              </button>
            </div>
            <p class="z-label mt-1 text-slate-400 text-[11px]">
              Leave empty to let Zircon automatically download and run the verified Adoptium JDK.
            </p>
          </div>

          <!-- JVM Arguments & GC Presets -->
          <div>
            <div class="flex items-center justify-between mb-1.5">
              <span class="z-label font-semibold text-slate-300 text-xs">JVM Arguments &amp; GC Presets</span>
              <button
                class="text-[10px] text-cyan-400 hover:text-cyan-300 font-mono transition-colors"
                @click="applyJvmPreset('clear')"
              >
                Clear Custom Args
              </button>
            </div>
            <div class="flex flex-wrap gap-1.5 mb-2">
              <button
                type="button"
                class="z-btn-ghost text-[10px] px-2 py-0.5 rounded-lg border border-slate-700/80 hover:border-cyan-400/50 hover:text-cyan-300"
                @click="applyJvmPreset('default')"
              >
                Safe Default (G1GC)
              </button>
              <button
                type="button"
                class="z-btn-ghost text-[10px] px-2 py-0.5 rounded-lg border border-slate-700/80 hover:border-cyan-400/50 hover:text-cyan-300"
                @click="applyJvmPreset('aikar')"
              >
                Aikar's Flags
              </button>
              <button
                type="button"
                class="z-btn-ghost text-[10px] px-2 py-0.5 rounded-lg border border-slate-700/80 hover:border-cyan-400/50 hover:text-cyan-300"
                @click="applyJvmPreset('zgc')"
              >
                Modern ZGC (Java 21+)
              </button>
            </div>
            <textarea
              v-model="settings.customJvmArgs"
              rows="2"
              class="z-input w-full font-mono text-xs p-2 leading-relaxed resize-y"
              placeholder="e.g. -XX:+UseG1GC -XX:+ParallelRefProcEnabled"
            ></textarea>
            <p class="z-label mt-1 text-slate-400 text-[11px]">
              Appended to Minecraft launch command. GC flags override standard garbage collection.
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
                  Allow Unverified P2P Mod Sync (Developer Mode)
                  <span class="text-[9px] font-mono bg-amber-500/10 text-amber-400 border border-amber-500/30 px-1 py-0.2 rounded font-semibold">Join-by-Code Only</span>
                </span>
                <p class="z-label mt-1 text-slate-400 text-[11px] leading-relaxed">
                  Permits receiving unlisted or custom mod JARs from friends via Join Codes after an explicit hash-approval prompt. Dedicated servers permanently enforce strict catalog verification regardless of this setting.
                </p>
              </div>
            </label>
          </div>
        </div>
      </div>

      <!-- ================= SECTION 3: LAUNCHER DIAGNOSTICS & LOGS (COLLAPSIBLE) ================= -->
      <div class="pt-4 border-t border-slate-800/80">
        <button
          type="button"
          class="w-full flex items-center justify-between text-left py-1 text-slate-200 hover:text-white group transition-colors select-none"
          @click="showLauncherLogs = !showLauncherLogs"
        >
          <div class="flex items-center gap-2">
            <span class="z-label font-bold text-xs text-slate-200 group-hover:text-cyan-300">Launcher Diagnostics &amp; Logs</span>
            <span class="text-[10px] bg-slate-800 border border-slate-700 px-1.5 py-0.2 rounded text-slate-400 font-mono">Ring Buffer</span>
          </div>
          <svg class="w-4 h-4 text-slate-400 group-hover:text-cyan-400 transition-transform duration-200" :class="{ 'rotate-180': showLauncherLogs }" fill="none" viewBox="0 0 24 24" stroke="currentColor">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M19 9l-7 7-7-7" />
          </svg>
        </button>

        <div v-show="showLauncherLogs" class="mt-3.5 space-y-3 pt-2 border-t border-slate-800/40">
          <div class="flex items-center justify-between">
            <p class="z-label text-slate-400 text-[11px]">
              In-memory launcher debug ring buffer (cleared on exit).
            </p>
            <div class="flex gap-1.5">
              <button class="z-btn-ghost text-[10px] px-2 py-0.5 rounded" @click="refreshLogs">Refresh</button>
              <button class="z-btn-ghost text-[10px] px-2 py-0.5 rounded" @click="copyLogs">Copy</button>
              <button class="z-btn-ghost text-[10px] px-2 py-0.5 rounded hover:text-red-400" @click="clearLogs">Clear</button>
            </div>
          </div>
          <pre class="bg-[#070b10] border border-slate-800/90 rounded-xl p-3 text-[10px] font-mono leading-relaxed text-slate-400 h-40 overflow-y-auto whitespace-pre-wrap select-text shadow-inner">{{ logText || 'No log lines captured yet.' }}</pre>
          <p v-if="copiedAt" class="z-label text-cyan-300 font-medium text-xs">{{ copiedAt }}</p>
        </div>
      </div>

      <!-- ================= SECTION 4: ABOUT & UPDATES ================= -->
      <div class="mt-5 pt-4 border-t border-slate-800/80">
        <div class="flex items-center justify-between mb-2">
          <div>
            <h3 class="text-white font-bold text-sm">Zircon Launcher</h3>
            <p class="text-xs text-slate-400 mt-0.5">
              Version:
              <span class="inline-block bg-cyan-500/15 text-cyan-300 border border-cyan-500/30 px-2 py-0.5 rounded text-[11px] font-mono font-bold ml-1">
                v{{ launcherVersion || '0.4.3' }}
              </span>
            </p>
          </div>
          <button
            class="z-btn-ghost text-[11px] px-3 py-1.5 rounded-lg border border-slate-700/80 hover:border-cyan-400/50 hover:text-cyan-300 flex items-center gap-1.5"
            :disabled="checkingUpdate || updating"
            @click="checkForUpdates"
          >
            <svg v-if="checkingUpdate" class="w-3.5 h-3.5 animate-spin" fill="none" viewBox="0 0 24 24">
              <circle class="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" stroke-width="4"></circle>
              <path class="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4zm2 5.291A7.962 7.962 0 014 12H0c0 3.042 1.135 5.824 3 7.938l3-2.647z"></path>
            </svg>
            <span>{{ checkingUpdate ? 'Checking…' : 'Check for Updates' }}</span>
          </button>
        </div>

        <!-- Update Status Message Box -->
        <div v-if="updateStatusMessage" class="mt-3 p-3 rounded-xl border text-xs" :class="updateStatusClass">
          <div class="font-semibold">{{ updateStatusMessage }}</div>
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
      </div>
    </div>

    <!-- Right Column: Last Played Minecraft Instance Log -->
    <div class="w-full xl:flex-1 z-card flex flex-col p-6 bg-[#0e1622]/90 border border-slate-800/80 min-w-[360px]">
      <div class="flex items-center justify-between mb-3">
        <div>
          <h3 class="text-white font-bold text-base">Last Played Minecraft Log</h3>
          <p class="text-xs text-slate-400 mt-0.5">
            <span v-if="mcLog" class="inline-flex items-center gap-1.5">
              <span class="text-cyan-300 font-bold">{{ mcLog.instanceName }}</span>
              <span class="bg-slate-900 text-[10px] px-2 py-0.5 rounded-md text-slate-300 border border-slate-800 uppercase font-mono font-bold">{{ mcLog.instanceType }}</span>
            </span>
            <span v-else>No log file found from a recently played instance.</span>
          </p>
        </div>
        <div class="flex gap-2">
          <button class="z-btn-ghost text-[11px] px-2.5 py-1 rounded-lg" @click="refreshMcLog">Refresh</button>
          <button class="z-btn-ghost text-[11px] px-2.5 py-1 rounded-lg" @click="copyMcLogs">Copy</button>
          <button class="z-btn-ghost text-[11px] px-2.5 py-1 rounded-lg hover:text-red-400" @click="clearMcLogs">Clear</button>
        </div>
      </div>

      <!-- Filters (Info, Warnings, Errors + Search) -->
      <div class="flex flex-wrap items-center gap-3 mt-1 pt-3 border-t border-slate-800/80">
        <input
          v-model="mcSearchQuery"
          type="text"
          placeholder="Filter log output..."
          class="bg-[#070b10] border border-slate-700/80 rounded-xl px-3.5 py-1.5 text-xs text-white placeholder-slate-500 focus:outline-none focus:border-cyan-400 flex-1 min-w-[160px]"
        />
        <div class="flex items-center gap-3 text-xs">
          <label class="flex items-center gap-1.5 cursor-pointer text-slate-300 hover:text-white select-none">
            <input type="checkbox" v-model="mcFilters.info" class="zircon-check" /> Info
          </label>
          <label class="flex items-center gap-1.5 cursor-pointer text-yellow-400 hover:text-yellow-300 select-none">
            <input type="checkbox" v-model="mcFilters.warnings" class="zircon-check" /> Warnings
          </label>
          <label class="flex items-center gap-1.5 cursor-pointer text-red-400 hover:text-red-300 select-none">
            <input type="checkbox" v-model="mcFilters.errors" class="zircon-check" /> Errors
          </label>
          <label class="flex items-center gap-1.5 cursor-pointer text-slate-400 hover:text-slate-200 select-none ml-auto">
            <input type="checkbox" v-model="autoScroll" class="zircon-check" /> Auto-scroll
          </label>
        </div>
      </div>

      <!-- Log Terminal Box -->
      <div
        ref="mcLogBox"
        class="bg-[#070b10] border border-slate-800/90 rounded-xl p-4 text-[11px] font-mono leading-relaxed min-h-[300px] max-h-[440px] overflow-y-auto whitespace-pre-wrap space-y-1 mt-3.5 select-text shadow-inner"
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
import { api, onGameOutput, pickFile, EXE_FILTER } from '../lib/api';
import { check as checkUpdate } from '@tauri-apps/plugin-updater';
import { relaunch } from '@tauri-apps/plugin-process';
import { THEME_PRESETS, BG_THEME_PRESETS, BUTTON_STYLES, GLASS_EFFECTS, CURATED_THEMES, detectCuratedTheme, applyTheme } from '../lib/theme';

const settings = ref({
  memoryGb: 4,
  discordRpc: true,
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
});
const saving = ref(false);
const savedAt = ref('');
const logText = ref('');
const copiedAt = ref('');

const showAdvancedTheme = ref(false);
const showAdvancedJava = ref(false);
const showLauncherLogs = ref(false);

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
    settings.value.customJvmArgs = '-XX:+UseG1GC -XX:+ParallelRefProcEnabled -XX:MaxGCPauseMillis=200 -XX:+UnlockExperimentalVMOptions -XX:+DisableExplicitGC -XX:+AlwaysPreTouch';
  } else if (preset === 'zgc') {
    settings.value.customJvmArgs = '-XX:+UseZGC -XX:+ZGenerational';
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
  } catch {
    applyActiveTheme();
  }
  try {
    launcherVersion.value = await api.getLauncherVersion();
  } catch {
    launcherVersion.value = '0.4.2';
  }
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
  updateProgress.value = 0;
  api.logDebug('Manual launcher update check started...');
  try {
    const update = await checkUpdate();
    if (update?.available) {
      updateInfo.value = update;
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
  if (!updateInfo.value) return;
  updating.value = true;
  updateProgress.value = 0;
  updateStatusMessage.value = `Downloading update v${updateInfo.value.version}...`;
  api.logDebug(`Starting download of v${updateInfo.value.version}...`);
  try {
    let totalBytes = 0;
    let downloadedBytes = 0;
    await updateInfo.value.downloadAndInstall((event) => {
      if (event.event === 'Started') {
        totalBytes = event.data.contentLength || 0;
      } else if (event.event === 'Progress') {
        downloadedBytes += event.data.chunkLength || 0;
        const percent = totalBytes > 0 ? Math.min(100, Math.round((downloadedBytes / totalBytes) * 100)) : 0;
        updateProgress.value = percent / 100;
        updateStatusMessage.value = `Downloading update v${updateInfo.value.version}... ${percent}%`;
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
