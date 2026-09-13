<template>
  <div class="h-full flex flex-col md:flex-row gap-4 overflow-hidden select-none">
    
    <!-- LEFT PANEL: CATEGORIES, PRESETS & CONTROLS (GLASSMORPHIC) -->
    <div
      class="w-full md:w-[380px] lg:w-[420px] p-4 flex flex-col gap-3.5 shrink-0 overflow-y-auto rounded-2xl bg-[#0e1622]/85 backdrop-blur-xl border border-slate-800/80 shadow-[inset_0_1px_0_0_rgba(255,255,255,0.04),0_8px_32px_rgba(0,0,0,0.45)] custom-scrollbar"
    >
      <!-- Top Title Bar -->
      <div class="flex items-center justify-between border-b border-slate-800/80 pb-3 shrink-0">
        <div class="flex items-center gap-2.5">
          <div class="w-8 h-8 rounded-xl bg-[var(--color-accent)]/15 border border-[var(--color-accent)]/40 flex items-center justify-center text-[var(--color-accent)] shadow-[0_0_12px_var(--color-accent-glow)]">
            <svg class="w-4 h-4" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
              <path d="m12 3-1.9 5.8a2 2 0 0 1-1.3 1.3L3 12l5.8 1.9a2 2 0 0 1 1.3 1.3L12 21l1.9-5.8a2 2 0 0 1 1.3-1.3L21 12l-5.8-1.9a2 2 0 0 1-1.3-1.3L12 3z" />
            </svg>
          </div>
          <div>
            <h2 class="text-sm font-extrabold text-white tracking-wide">AI Skin Studio</h2>
            <div class="flex items-center gap-1.5 text-[10px] text-slate-400">
              <template v-if="isGenerating && progressStage === 'loading_model'">
                <span class="w-1.5 h-1.5 rounded-full bg-amber-400 animate-ping"></span>
                <span class="text-amber-300 font-bold">Loading Model to VRAM...</span>
              </template>
              <template v-else-if="isGenerating && progressStage === 'generating'">
                <span class="w-1.5 h-1.5 rounded-full bg-[var(--color-accent)] animate-pulse"></span>
                <span class="text-[var(--color-accent)] font-semibold">Euler Step {{ progressStep }}/{{ progressTotalSteps }} ({{ Math.round(lerpedProgress) }}%)</span>
              </template>
              <template v-else-if="isGenerating && progressStage === 'postprocessing'">
                <span class="w-1.5 h-1.5 rounded-full bg-cyan-400 animate-pulse"></span>
                <span class="text-cyan-300 font-semibold">Post-processing...</span>
              </template>
              <template v-else>
                <span class="w-1.5 h-1.5 rounded-full bg-[var(--color-accent)] animate-pulse"></span>
                <span>Local ONNX Model</span>
              </template>
            </div>
          </div>
        </div>

        <!-- Model Rig Toggle (Classic vs Slim) -->
        <div class="flex rounded-xl bg-slate-900/60 backdrop-blur-md p-0.5 border border-slate-700/60 shadow-inner">
          <button
            type="button"
            class="px-2.5 py-1 text-xs font-semibold rounded-lg transition-all"
            :class="selectedVariant === 'classic' ? 'bg-[var(--color-accent)] text-[var(--color-accent-ink)] font-bold shadow-sm' : 'text-slate-400 hover:text-white'"
            @click="selectedVariant = 'classic'"
          >
            Classic (4px)
          </button>
          <button
            type="button"
            class="px-2.5 py-1 text-xs font-semibold rounded-lg transition-all"
            :class="selectedVariant === 'slim' ? 'bg-[var(--color-accent)] text-[var(--color-accent-ink)] font-bold shadow-sm' : 'text-slate-400 hover:text-white'"
            @click="selectedVariant = 'slim'"
          >
            Slim (3px)
          </button>
        </div>
      </div>

      <!-- Quick Preset Selector (Collapsible Group matching Settings Page) -->
      <div class="rounded-xl border border-slate-800/80 bg-slate-900/50 backdrop-blur-md overflow-hidden transition-all shrink-0">
        <div
          class="flex items-center justify-between p-2.5 hover:bg-slate-800/40 cursor-pointer select-none transition-colors"
          @click="showPresets = !showPresets"
        >
          <div class="flex items-center gap-2 min-w-0">
            <span class="text-xs font-bold text-slate-200">Preset Style</span>
            <span class="text-[10px] text-[var(--color-accent)] font-semibold px-2 py-0.5 rounded-full bg-[var(--color-accent)]/15 border border-[var(--color-accent)]/30 truncate">
              {{ activePresetName }}
            </span>
          </div>

          <div class="flex items-center gap-1.5 shrink-0">
            <button
              type="button"
              class="p-1 rounded text-slate-400 hover:text-[var(--color-accent)] hover:bg-slate-800 transition-colors"
              title="Shuffle all categories"
              @click.stop="randomizeAllSlots"
            >
              <svg class="w-3.5 h-3.5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                <path d="M21.5 2v6h-6M21.34 15.57a10 10 0 1 1-.57-8.38l5.67-5.67" />
              </svg>
            </button>
            <svg
              class="w-4 h-4 text-slate-400 transition-transform duration-200"
              :class="{ 'rotate-180': showPresets }"
              viewBox="0 0 24 24"
              fill="none"
              stroke="currentColor"
              stroke-width="2"
            >
              <polyline points="6 9 12 15 18 9" />
            </svg>
          </div>
        </div>

        <!-- Preset Cards Grid (matching SettingsView Curated Themes) -->
        <div v-show="showPresets" class="p-2.5 border-t border-slate-800/80 grid grid-cols-2 gap-2 bg-[#080d14]/70">
          <button
            v-for="preset in PRESETS"
            :key="preset.id"
            type="button"
            class="flex items-center gap-2.5 p-2 rounded-xl border text-left transition-all group relative cursor-pointer"
            :class="
              activePresetKey === preset.id
                ? 'border-[var(--color-accent)] bg-[var(--color-accent)]/15 shadow-[0_0_12px_var(--color-accent-glow)] ring-1 ring-[var(--color-accent)]/30'
                : 'border-slate-800/80 bg-slate-900/60 hover:border-slate-700 hover:bg-slate-900'
            "
            @click="applyPreset(preset)"
          >
            <!-- Preset Palette Swatch Dot -->
            <div class="w-4 h-4 rounded-full shrink-0 flex overflow-hidden border border-black/40 shadow-sm">
              <div class="w-1/3 h-full" :style="{ backgroundColor: preset.palette[0] }"></div>
              <div class="w-1/3 h-full" :style="{ backgroundColor: preset.palette[1] }"></div>
              <div class="w-1/3 h-full" :style="{ backgroundColor: preset.palette[2] }"></div>
            </div>

            <div class="min-w-0 flex-1">
              <div
                class="text-xs font-bold truncate transition-colors"
                :class="activePresetKey === preset.id ? 'text-white' : 'text-slate-300 group-hover:text-white'"
              >
                {{ preset.name }}
              </div>
              <div class="text-[9px] text-slate-400 truncate">
                {{ preset.badge }}
              </div>
            </div>

            <svg
              v-if="activePresetKey === preset.id"
              class="w-3.5 h-3.5 text-[var(--color-accent)] shrink-0"
              fill="none"
              viewBox="0 0 24 24"
              stroke="currentColor"
              stroke-width="3"
            >
              <polyline points="20 6 9 17 4 12" />
            </svg>
          </button>
        </div>
      </div>

      <!-- 8 COLLAPSIBLE CATEGORY ACCORDIONS (SETTINGS-STYLE WITH RADIO BUTTONS) -->
      <div class="space-y-2 shrink-0">
        <div class="flex items-center justify-between px-1">
          <span class="text-[10px] font-bold uppercase tracking-wider text-slate-400">
            Attribute Groups ({{ activeSlotCount }} / 8 Active)
          </span>
          <button
            type="button"
            class="text-[10px] text-slate-400 hover:text-white transition-colors"
            @click="clearAllSlots"
          >
            Reset All
          </button>
        </div>

        <div
          v-for="cat in CATEGORIES"
          :key="cat.id"
          class="rounded-xl border border-slate-800/80 bg-slate-900/50 backdrop-blur-md overflow-hidden transition-all shadow-sm"
        >
          <!-- Category Header (Click to Expand / Collapse) -->
          <div
            class="flex items-center justify-between p-2.5 hover:bg-slate-800/40 cursor-pointer select-none transition-colors"
            @click="toggleGroup(cat.id)"
          >
            <div class="flex items-center gap-2 min-w-0">
              <span class="text-xs font-bold text-slate-200">{{ cat.name }}</span>
              <span
                v-if="selectedSlots[cat.id]"
                class="text-[10px] text-[var(--color-accent)] font-semibold truncate max-w-[150px] px-2 py-0.5 rounded-full bg-[var(--color-accent)]/15 border border-[var(--color-accent)]/30"
              >
                {{ getOptionLabel(cat, selectedSlots[cat.id]) }}
              </span>
              <span v-else class="text-[10px] text-slate-500 italic">Default</span>
            </div>

            <div class="flex items-center gap-1.5 shrink-0">
              <!-- Inline Randomize Slot Button -->
              <button
                type="button"
                class="p-1 rounded text-slate-400 hover:text-[var(--color-accent)] hover:bg-slate-800 transition-colors"
                title="Randomize this slot"
                @click.stop="randomizeSlot(cat)"
              >
                <svg class="w-3.5 h-3.5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                  <path d="M21.5 2v6h-6M21.34 15.57a10 10 0 1 1-.57-8.38l5.67-5.67" />
                </svg>
              </button>

              <!-- Chevron -->
              <svg
                class="w-4 h-4 text-slate-400 transition-transform duration-200"
                :class="{ 'rotate-180': openGroups[cat.id] }"
                viewBox="0 0 24 24"
                fill="none"
                stroke="currentColor"
                stroke-width="2"
              >
                <polyline points="6 9 12 15 18 9" />
              </svg>
            </div>
          </div>

          <!-- Collapsible Body (Radio-style buttons like Settings Page) -->
          <div
            v-show="openGroups[cat.id]"
            class="p-2 border-t border-slate-800/80 grid grid-cols-2 gap-1.5 bg-[#080d14]/70"
          >
            <!-- None / Default Option -->
            <button
              type="button"
              class="flex items-center gap-2 p-2 rounded-xl border text-left transition-all group relative cursor-pointer"
              :class="
                !selectedSlots[cat.id]
                  ? 'border-[var(--color-accent)] bg-[var(--color-accent)]/15 shadow-[0_0_12px_var(--color-accent-glow)] ring-1 ring-[var(--color-accent)]/30'
                  : 'border-slate-800/80 bg-slate-900/60 hover:border-slate-700 hover:bg-slate-900'
              "
              @click="clearSlot(cat.id)"
            >
              <span
                class="w-2.5 h-2.5 rounded-full shrink-0 transition-transform"
                :class="!selectedSlots[cat.id] ? 'bg-[var(--color-accent)] shadow-[0_0_6px_var(--color-accent)]' : 'border border-slate-600'"
              ></span>
              <span
                class="text-xs font-semibold truncate transition-colors min-w-0 flex-1"
                :class="!selectedSlots[cat.id] ? 'text-white font-bold' : 'text-slate-400 group-hover:text-white'"
              >
                Default / Unset
              </span>
              <svg
                v-if="!selectedSlots[cat.id]"
                class="w-3.5 h-3.5 text-[var(--color-accent)] shrink-0"
                fill="none"
                viewBox="0 0 24 24"
                stroke="currentColor"
                stroke-width="2.5"
              >
                <polyline points="20 6 9 17 4 12" />
              </svg>
            </button>

            <!-- Categorized Options -->
            <button
              v-for="opt in cat.options"
              :key="opt.id"
              type="button"
              class="flex items-center gap-2 p-2 rounded-xl border text-left transition-all group relative cursor-pointer"
              :class="
                selectedSlots[cat.id] === opt.id
                  ? 'border-[var(--color-accent)] bg-[var(--color-accent)]/15 shadow-[0_0_12px_var(--color-accent-glow)] ring-1 ring-[var(--color-accent)]/30'
                  : 'border-slate-800/80 bg-slate-900/60 hover:border-slate-700 hover:bg-slate-900'
              "
              @click="selectOption(cat.id, opt.id)"
            >
              <!-- Color swatch if available, or radio dot -->
              <span
                v-if="opt.color"
                class="w-3.5 h-3.5 rounded-full shrink-0 shadow-sm border border-black/30"
                :style="{ backgroundColor: opt.color, boxShadow: `0 0 6px ${opt.color}80` }"
              ></span>
              <span
                v-else
                class="w-2.5 h-2.5 rounded-full shrink-0 transition-transform"
                :class="selectedSlots[cat.id] === opt.id ? 'bg-[var(--color-accent)] shadow-[0_0_6px_var(--color-accent)]' : 'border border-slate-600'"
              ></span>

              <span
                class="text-xs font-semibold truncate transition-colors min-w-0 flex-1"
                :class="selectedSlots[cat.id] === opt.id ? 'text-white font-bold' : 'text-slate-300 group-hover:text-white'"
              >
                {{ opt.label }}
              </span>

              <svg
                v-if="selectedSlots[cat.id] === opt.id"
                class="w-3.5 h-3.5 text-[var(--color-accent)] shrink-0"
                fill="none"
                viewBox="0 0 24 24"
                stroke="currentColor"
                stroke-width="2.5"
              >
                <polyline points="20 6 9 17 4 12" />
              </svg>
            </button>
          </div>
        </div>
      </div>

      <!-- COLOR PALETTE GUIDANCE -->
      <div class="space-y-2 bg-slate-900/50 backdrop-blur-md p-3 rounded-2xl border border-slate-800/80 shadow-inner shrink-0">
        <div class="flex items-center justify-between">
          <label class="text-[10px] font-bold uppercase tracking-wider text-slate-400">
            Color Palette
          </label>
          <span class="text-[10px] text-slate-500">Primary • Secondary • Accent</span>
        </div>

        <div class="grid grid-cols-3 gap-2">
          <!-- Primary -->
          <div class="flex items-center gap-1.5 bg-slate-900/70 px-2 py-1.5 rounded-xl border border-slate-700/50">
            <input v-model="palette[0]" type="color" class="w-5 h-5 rounded cursor-pointer bg-transparent border-0" />
            <span class="text-[10px] font-mono text-slate-300 uppercase truncate">{{ palette[0] }}</span>
          </div>
          <!-- Secondary -->
          <div class="flex items-center gap-1.5 bg-slate-900/70 px-2 py-1.5 rounded-xl border border-slate-700/50">
            <input v-model="palette[1]" type="color" class="w-5 h-5 rounded cursor-pointer bg-transparent border-0" />
            <span class="text-[10px] font-mono text-slate-300 uppercase truncate">{{ palette[1] }}</span>
          </div>
          <!-- Accent -->
          <div class="flex items-center gap-1.5 bg-slate-900/70 px-2 py-1.5 rounded-xl border border-slate-700/50">
            <input v-model="palette[2]" type="color" class="w-5 h-5 rounded cursor-pointer bg-transparent border-0" />
            <span class="text-[10px] font-mono text-slate-300 uppercase truncate">{{ palette[2] }}</span>
          </div>
        </div>

        <!-- Live Gradient Preview Bar -->
        <div
          class="h-2 w-full rounded-md shadow-inner border border-white/5"
          :style="{
            background: `linear-gradient(to right, ${palette[0]}, ${palette[1]}, ${palette[2]})`
          }"
        ></div>
      </div>

      <!-- SEED & RANDOMIZATION CONTROLS -->
      <div class="space-y-2 bg-slate-900/50 backdrop-blur-md p-3 rounded-2xl border border-slate-800/80 shadow-inner shrink-0">
        <div class="flex items-center justify-between">
          <label class="text-[10px] font-bold uppercase tracking-wider text-slate-400">Randomization</label>
          <label class="flex items-center gap-1.5 text-xs text-slate-300 cursor-pointer">
            <input
              v-model="isRandomSeed"
              type="checkbox"
              class="rounded border-slate-700 bg-slate-900 text-[var(--color-accent)] focus:ring-0"
            />
            <span>New Random Seed Each Run</span>
          </label>
        </div>

        <div v-if="!isRandomSeed" class="flex items-center gap-2 pt-1">
          <span class="text-xs text-slate-400">Seed:</span>
          <input
            v-model.number="manualSeed"
            type="number"
            class="flex-1 text-xs py-1.5 px-2.5 bg-slate-900/70 border border-slate-700/60 rounded-xl text-slate-200 focus:outline-none focus:border-[var(--color-accent)]"
            placeholder="Locked seed value"
          />
        </div>
      </div>

      <!-- GENERATE ACTION BUTTON -->
      <div class="mt-auto pt-2 space-y-2 shrink-0">
        <button
          type="button"
          class="z-btn-accent relative overflow-hidden w-full py-3 px-4 rounded-xl text-sm font-extrabold flex items-center justify-center gap-2 shadow-lg hover:shadow-[var(--color-accent-glow)] transition-all active:scale-[0.99] disabled:opacity-50 cursor-pointer"
          :disabled="isGenerating"
          @click="generateBatch"
        >
          <!-- Underlay smooth fill matching lerp progress -->
          <div
            v-if="isGenerating"
            class="absolute left-0 top-0 bottom-0 bg-white/15 pointer-events-none transition-[width] duration-75"
            :style="{ width: `${lerpedProgress}%` }"
          ></div>

          <svg v-if="!isGenerating" class="w-4 h-4 z-10" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5">
            <polygon points="5 3 19 12 5 21 5 3" />
          </svg>
          <svg v-else class="w-4 h-4 animate-spin z-10" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5">
            <circle cx="12" cy="12" r="10" stroke-opacity="0.25" stroke="currentColor" />
            <path d="M12 2a10 10 0 0 1 10 10" />
          </svg>
          <span class="z-10">
            {{
              !isGenerating
                ? 'Generate 4 Skins'
                : progressStage === 'loading_model'
                  ? 'Loading Model to VRAM...'
                  : progressStage === 'postprocessing'
                    ? 'Finalizing Textures...'
                    : `Synthesizing (${Math.round(lerpedProgress)}%)...`
            }}
          </span>
        </button>

        <div v-if="generationTime" class="text-center text-[11px] text-slate-400">
          Generated 4 skins in {{ (generationTime / 1000).toFixed(2) }}s
        </div>
      </div>

    </div>

    <!-- RIGHT PANEL: 2x2 ROTATABLE SKINS GRID WITH 2026 ROTATING LED UNDERGLOW -->
    <div class="flex-1 min-h-0 flex flex-col gap-3 overflow-hidden">
      
      <!-- Top Status Bar for Candidate Panel -->
      <div class="flex items-center justify-between px-1 shrink-0">
        <div class="flex items-center gap-2">
          <span class="text-xs font-bold uppercase tracking-wider text-slate-400">Generated Candidates</span>
          <span class="text-[11px] text-slate-500">Interactive 3D preview • Click and drag to rotate</span>
        </div>

        <div v-if="candidates.length > 0" class="flex items-center gap-2">
          <button
            type="button"
            class="z-btn-ghost text-xs px-3 py-1.5 rounded-xl font-semibold flex items-center gap-1.5 text-slate-300 hover:text-white"
            :disabled="isGenerating"
            @click="generateBatch"
          >
            <svg class="w-3.5 h-3.5 text-[var(--color-accent)]" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
              <path d="M21.5 2v6h-6M21.34 15.57a10 10 0 1 1-.57-8.38l5.67-5.67" />
            </svg>
            <span>Re-roll Batch</span>
          </button>
        </div>
      </div>

      <!-- MODEL LOADING INTO VRAM CALLOUT BANNER (VISUALLY APPARENT COLD START STATE) -->
      <div
        v-if="isGenerating && progressStage === 'loading_model'"
        class="px-4 py-3 rounded-2xl bg-amber-500/10 border border-amber-500/30 backdrop-blur-xl flex items-center justify-between shadow-[0_0_24px_rgba(245,158,11,0.15)] animate-in fade-in slide-in-from-top-1 duration-300 shrink-0"
      >
        <div class="flex items-center gap-3">
          <div class="w-9 h-9 rounded-xl bg-amber-500/20 border border-amber-500/40 flex items-center justify-center text-amber-300 shrink-0 shadow-[0_0_12px_rgba(245,158,11,0.3)]">
            <svg class="w-5 h-5 animate-pulse" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
              <rect x="4" y="4" width="16" height="16" rx="2" />
              <rect x="9" y="9" width="6" height="6" />
              <path d="M9 1v3M15 1v3M9 20v3M15 20v3M20 9h3M20 14h3M1 9h3M1 14h3" />
            </svg>
          </div>
          <div>
            <div class="text-xs font-extrabold text-amber-200 flex items-center gap-2">
              <span>Loading AI Model into GPU VRAM</span>
              <span class="text-[10px] px-2 py-0.5 rounded-full bg-amber-500/20 text-amber-300 font-semibold border border-amber-500/30">
                DirectML Initialization
              </span>
            </div>
            <div class="text-[11px] text-amber-300/80 font-medium">
              Compiling DiT Flow Matching graph and allocating FP16 buffers on Discrete GPU. Subsequent runs will be instant!
            </div>
          </div>
        </div>
        <div class="flex items-center gap-2 px-3 py-1.5 rounded-xl bg-amber-950/40 border border-amber-500/30 text-xs font-mono font-bold text-amber-300">
          <svg class="w-4 h-4 animate-spin text-amber-400" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5">
            <circle cx="12" cy="12" r="10" stroke-opacity="0.25" stroke="currentColor" />
            <path d="M12 2a10 10 0 0 1 10 10" />
          </svg>
          <span>Allocating VRAM</span>
        </div>
      </div>

      <!-- SMOOTH LERPED PROGRESS BAR CARD (ACTIVE DURING ALL STAGES OF GENERATION) -->
      <div
        v-if="isGenerating"
        class="px-3.5 py-2.5 rounded-2xl bg-[#0d1624]/90 border border-slate-800/90 shadow-[0_4px_24px_rgba(0,0,0,0.4)] backdrop-blur-xl flex flex-col gap-2 transition-all duration-300 shrink-0"
      >
        <div class="flex items-center justify-between text-xs">
          <!-- Left: Stage & Status Text -->
          <div class="flex items-center gap-2.5 min-w-0">
            <span class="relative flex h-2.5 w-2.5 shrink-0">
              <span
                class="animate-ping absolute inline-flex h-full w-full rounded-full opacity-75"
                :class="progressStage === 'loading_model' ? 'bg-amber-400' : 'bg-[var(--color-accent)]'"
              ></span>
              <span
                class="relative inline-flex rounded-full h-2.5 w-2.5"
                :class="progressStage === 'loading_model' ? 'bg-amber-500' : 'bg-[var(--color-accent)]'"
              ></span>
            </span>

            <span class="font-extrabold text-slate-100 truncate">
              {{
                progressStage === 'loading_model'
                  ? 'Initializing DirectML Engine & Loading VRAM...'
                  : progressStage === 'postprocessing'
                    ? 'Post-Processing Dual-Layer Minecraft Skins...'
                    : 'Solving Flow Matching Euler ODE'
              }}
            </span>

            <span
              v-if="progressStage === 'generating'"
              class="text-[10px] font-mono font-bold px-2 py-0.5 rounded-full bg-[var(--color-accent)]/15 text-[var(--color-accent)] border border-[var(--color-accent)]/30 shrink-0"
            >
              Step {{ progressStep }} / {{ progressTotalSteps }}
            </span>
          </div>

          <!-- Right: Live Smooth Percentage -->
          <div class="flex items-center gap-2 shrink-0">
            <span class="text-[11px] text-slate-400 font-mono hidden md:inline">
              {{ progressMessage }}
            </span>
            <span
              class="text-xs font-mono font-black px-2.5 py-0.5 rounded-lg border shadow-inner"
              :class="
                progressStage === 'loading_model'
                  ? 'bg-amber-500/15 border-amber-500/40 text-amber-300'
                  : 'bg-slate-900/90 border-slate-700/70 text-[var(--color-accent)]'
              "
            >
              {{ Math.round(lerpedProgress) }}%
            </span>
          </div>
        </div>

        <!-- Progress Track with Smooth Lerped Bar & High-Speed Shimmer -->
        <div class="h-2 w-full bg-[#060a10] rounded-full overflow-hidden border border-slate-800 relative shadow-inner p-[1px]">
          <div
            class="h-full rounded-full relative overflow-hidden transition-[width] duration-75"
            :class="
              progressStage === 'loading_model'
                ? 'bg-gradient-to-r from-amber-500 via-orange-400 to-amber-300 shadow-[0_0_12px_rgba(245,158,11,0.6)]'
                : 'bg-gradient-to-r from-[var(--color-accent)] via-cyan-400 to-[var(--color-accent-bright)] shadow-[0_0_12px_var(--color-accent-glow)]'
            "
            :style="{ width: `${Math.min(100, Math.max(3, lerpedProgress))}%` }"
          >
            <!-- Animated Glowing Shimmer Wave Traversing the Bar -->
            <div class="absolute inset-0 bg-gradient-to-r from-transparent via-white/40 to-transparent animate-shimmer-bar pointer-events-none"></div>
          </div>
        </div>
      </div>

      <!-- 2x2 GRID (SOFTER BROWSE/LIBRARY CARD EFFECT + ROTATING CONIC LED WAVE UNDERGLOW) -->
      <div class="flex-1 min-h-0 grid grid-cols-2 grid-rows-2 gap-4">
        <div
          v-for="slotIndex in 4"
          :key="slotIndex"
          class="relative p-[1.5px] rounded-2xl overflow-hidden transition-all duration-300 group"
          :class="isGenerating ? 'shadow-[0_0_24px_var(--color-accent-glow)]' : 'shadow-md'"
        >
          <!-- ROTATING CONIC LED WAVE (ACTIVE DURING GENERATION) -->
          <div
            v-if="isGenerating"
            class="absolute -inset-[150%] animate-led-spin pointer-events-none"
            style="background: conic-gradient(from 0deg at 50% 50%, transparent 0deg, transparent 200deg, var(--color-accent) 290deg, var(--color-accent-bright) 340deg, transparent 360deg);"
          ></div>
          <div
            v-if="isGenerating"
            class="absolute -inset-[150%] animate-led-spin blur-xl opacity-75 pointer-events-none"
            style="background: conic-gradient(from 0deg at 50% 50%, transparent 0deg, transparent 200deg, var(--color-accent) 290deg, var(--color-accent-bright) 340deg, transparent 360deg);"
          ></div>

          <!-- INNER CARD CONTAINER (MATCHING SAVED SKINS / BROWSE LIBRARY SOFT CARD EFFECT) -->
          <div
            class="relative z-10 w-full h-full p-3.5 flex flex-col gap-2 rounded-[14.5px] transition-all duration-200"
            :class="
              isGenerating
                ? 'bg-[#0d1624] border border-[var(--color-accent)]/30'
                : 'bg-[#0e1722]/85 hover:bg-[#121d2b]/95 border border-slate-800/80 hover:border-slate-700/90'
            "
          >
            <!-- Candidate Slot Header -->
            <div class="flex items-center justify-between text-xs shrink-0 z-10">
              <span class="font-bold text-slate-300">Option {{ slotIndex }}</span>
              <span v-if="candidates[slotIndex - 1]" class="font-mono text-[10px] text-slate-400">
                seed: {{ String(candidates[slotIndex - 1].seed).slice(-5) }}
              </span>
            </div>

            <!-- Main Viewport Window (3D Rotatable or Loading Gradient Wave / Idle) -->
            <div class="flex-1 min-h-0 rounded-xl overflow-hidden bg-[#070b10]/90 border border-slate-800/70 relative flex items-center justify-center">
              
              <!-- STATE 1: LOADING STATE WITH TEXT GRADIENT WAVE & DYNAMIC STEP TRACK -->
              <div
                v-if="isGenerating"
                class="absolute inset-0 flex flex-col items-center justify-center gap-3 p-4 pointer-events-none z-20"
              >
                <!-- Soft background pulse -->
                <div class="absolute inset-0 bg-gradient-to-b from-transparent via-[var(--color-accent)]/5 to-transparent animate-pulse"></div>

                <!-- STATE 1A: MODEL LOADING / COMPILING INTO GPU VRAM -->
                <template v-if="progressStage === 'loading_model'">
                  <div class="w-10 h-10 rounded-2xl bg-amber-500/15 border border-amber-500/40 flex items-center justify-center text-amber-300 shadow-[0_0_20px_rgba(245,158,11,0.3)] animate-pulse">
                    <svg class="w-5 h-5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                      <rect x="4" y="4" width="16" height="16" rx="2" />
                      <rect x="9" y="9" width="6" height="6" />
                      <path d="M9 1v3M15 1v3M9 20v3M15 20v3M20 9h3M20 14h3M1 9h3M1 14h3" />
                    </svg>
                  </div>
                  <div class="flex flex-col items-center gap-0.5">
                    <span class="text-xs font-black uppercase tracking-wider text-amber-300 drop-shadow-[0_0_10px_rgba(245,158,11,0.6)]">
                      Loading Model
                    </span>
                    <span class="text-[10px] text-amber-200/80 font-medium">
                      Compiling DirectML VRAM
                    </span>
                  </div>
                </template>

                <!-- STATE 1B: FLOW MATCHING ODE STEPS -->
                <template v-else>
                  <div class="flex flex-col items-center gap-1">
                    <span class="text-xs font-black uppercase tracking-[0.2em] text-transparent bg-clip-text bg-gradient-to-r from-slate-400 via-[var(--color-accent-bright)] via-white to-slate-400 animate-shimmer-text drop-shadow-[0_0_12px_var(--color-accent-glow)]">
                      Synthesizing
                    </span>
                    <span class="text-[10px] font-mono text-[var(--color-accent)] font-bold">
                      {{ progressStage === 'postprocessing' ? 'Assembling 3D UVs' : `Step ${progressStep} / ${progressTotalSteps}` }}
                    </span>
                  </div>

                  <!-- Slot Mini Lerp Bar -->
                  <div class="w-28 h-1.5 bg-slate-900/90 rounded-full overflow-hidden border border-slate-700/60 shadow-inner">
                    <div
                      class="h-full bg-gradient-to-r from-[var(--color-accent)] to-[var(--color-accent-bright)] rounded-full transition-[width] duration-75"
                      :style="{ width: `${Math.min(100, Math.max(3, lerpedProgress))}%` }"
                    ></div>
                  </div>
                </template>
              </div>

              <!-- STATE 2: GENERATED 3D ROTATABLE SKIN -->
              <div
                v-else-if="candidates[slotIndex - 1]"
                class="w-full h-full relative"
              >
                <Player3DPreview
                  :image-uri="candidates[slotIndex - 1].data_url"
                  :variant="candidates[slotIndex - 1].variant || selectedVariant"
                />
              </div>

              <!-- STATE 3: INITIAL IDLE PLACEHOLDER (NO SKIN GENERATED YET) -->
              <div
                v-else
                class="flex flex-col items-center justify-center gap-2 text-slate-500 p-4 text-center"
              >
                <div class="w-10 h-10 rounded-xl bg-slate-900/60 border border-slate-800 flex items-center justify-center text-slate-600">
                  <svg class="w-5 h-5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5">
                    <path d="M20 21v-2a4 4 0 0 0-4-4H8a4 4 0 0 0-4 4v2" />
                    <circle cx="12" cy="7" r="4" />
                  </svg>
                </div>
                <span class="text-xs font-medium text-slate-400">Ready for generation</span>
              </div>

            </div>

            <!-- Bottom Action Buttons (Only when candidate exists) -->
            <div
              v-if="candidates[slotIndex - 1] && !isGenerating"
              class="flex items-center gap-2 shrink-0 pt-1"
            >
              <button
                type="button"
                class="z-btn-accent flex-1 py-1.5 px-2.5 rounded-xl text-xs font-bold flex items-center justify-center gap-1.5 shadow-md hover:shadow-[var(--color-accent-glow)] transition-all cursor-pointer"
                title="Open in Paint Studio for 2D/3D editing"
                @click="editInStudio(candidates[slotIndex - 1])"
              >
                <svg class="w-3.5 h-3.5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5">
                  <path d="M12 20h9" />
                  <path d="M16.5 3.5a2.121 2.121 0 0 1 3 3L7 19l-4 1 1-4L16.5 3.5z" />
                </svg>
                <span>Edit in Studio</span>
              </button>

              <button
                type="button"
                class="z-btn-ghost py-1.5 px-3 rounded-xl text-xs font-semibold text-slate-300 hover:text-white transition-colors cursor-pointer"
                title="Save to My Skins Gallery"
                @click="quickSave(candidates[slotIndex - 1])"
              >
                Save
              </button>

              <button
                type="button"
                class="py-1.5 px-3 rounded-xl text-xs font-semibold bg-[var(--color-accent)]/15 border border-[var(--color-accent)]/40 text-[var(--color-accent)] hover:bg-[var(--color-accent)]/25 transition-colors cursor-pointer"
                title="Wear this skin right now"
                @click="wearSkinNow(candidates[slotIndex - 1])"
              >
                Wear
              </button>
            </div>

            <!-- Placeholder footer when empty -->
            <div
              v-else
              class="h-8 flex items-center justify-center text-[11px] text-slate-600 shrink-0"
            >
              Option {{ slotIndex }}
            </div>

          </div>
        </div>
      </div>

    </div>

  </div>
</template>

<script setup>
import { ref, computed, onMounted, onUnmounted } from 'vue';
import Player3DPreview from '../Player3DPreview.vue';
import { api } from '../../lib/api.js';

const emit = defineEmits(['edit-in-studio', 'skin-saved']);

// 8 Discrete Categories
const CATEGORIES = [
  {
    id: 'theme',
    name: 'Theme',
    options: [
      { id: 'theme:cyberpunk_scifi', label: 'Cyberpunk Sci-Fi' },
      { id: 'theme:anime', label: 'Anime' },
      { id: 'theme:medieval_fantasy', label: 'Medieval Fantasy' },
      { id: 'theme:cute_pastel', label: 'Cute Pastel' },
      { id: 'theme:gothic_grunge', label: 'Gothic Grunge' },
      { id: 'theme:modern_streetwear', label: 'Modern Streetwear' },
      { id: 'theme:military_tactical', label: 'Military Tactical' },
      { id: 'theme:nether_ender', label: 'Nether / Ender' },
      { id: 'theme:mob_hybrid', label: 'Mob Hybrid' },
      { id: 'theme:holiday_seasonal', label: 'Holiday Seasonal' },
    ],
  },
  {
    id: 'archetype',
    name: 'Archetype',
    options: [
      { id: 'archetype:knight_warrior', label: 'Knight Warrior' },
      { id: 'archetype:robot_cyborg', label: 'Robot Cyborg' },
      { id: 'archetype:ninja_assassin', label: 'Ninja Assassin' },
      { id: 'archetype:mage_wizard', label: 'Mage Wizard' },
      { id: 'archetype:rogue_adventurer', label: 'Rogue Adventurer' },
      { id: 'archetype:demon_undead', label: 'Demon / Undead' },
      { id: 'archetype:angel_celestial', label: 'Angel Celestial' },
      { id: 'archetype:beast_hybrid', label: 'Beast Hybrid' },
      { id: 'archetype:royalty', label: 'Royalty' },
      { id: 'archetype:maid_uniform', label: 'Maid Uniform' },
      { id: 'archetype:teen_casual', label: 'Casual Teen' },
    ],
  },
  {
    id: 'outfit',
    name: 'Outfit',
    options: [
      { id: 'outfit:hoodie', label: 'Hoodie' },
      { id: 'outfit:plate_armor', label: 'Plate Armor' },
      { id: 'outfit:trench_coat', label: 'Trench Coat' },
      { id: 'outfit:cloak_cape', label: 'Cloak / Cape' },
      { id: 'outfit:tactical_vest', label: 'Tactical Vest' },
      { id: 'outfit:tshirt_pants', label: 'T-Shirt & Pants' },
      { id: 'outfit:sweater', label: 'Cozy Sweater' },
      { id: 'outfit:suit_tie', label: 'Suit & Tie' },
      { id: 'outfit:kimono_haori', label: 'Kimono / Haori' },
      { id: 'outfit:dress', label: 'Dress' },
      { id: 'outfit:tunic', label: 'Tunic' },
    ],
  },
  {
    id: 'headwear',
    name: 'Headwear',
    options: [
      { id: 'headwear:mask_visor', label: 'Visor / Mask' },
      { id: 'headwear:hood_up', label: 'Hood (Up)' },
      { id: 'headwear:headphones', label: 'Headphones' },
      { id: 'headwear:helmet', label: 'Armored Helmet' },
      { id: 'headwear:crown', label: 'Royal Crown' },
      { id: 'headwear:animal_ears', label: 'Animal / Bunny Ears' },
      { id: 'headwear:horns', label: 'Demon / Dragon Horns' },
      { id: 'headwear:halo', label: 'Glowing Halo' },
      { id: 'headwear:goggles_glasses', label: 'Goggles / Glasses' },
      { id: 'headwear:hat_cap', label: 'Cap / Hat' },
      { id: 'headwear:flower_trim', label: 'Flower Trim' },
      { id: 'headwear:none', label: 'None' },
    ],
  },
  {
    id: 'hair',
    name: 'Hair Color',
    options: [
      { id: 'hair_color:silver_white', label: 'Silver / White', color: '#e2e8f0' },
      { id: 'hair_color:black', label: 'Black', color: '#18181b' },
      { id: 'hair_color:dark_brown', label: 'Dark Brown', color: '#451a03' },
      { id: 'hair_color:brown', label: 'Chestnut Brown', color: '#78350f' },
      { id: 'hair_color:blonde', label: 'Golden Blonde', color: '#facc15' },
      { id: 'hair_color:blue', label: 'Ocean Blue', color: '#38bdf8' },
      { id: 'hair_color:pink', label: 'Sakura Pink', color: '#f472b6' },
      { id: 'hair_color:purple', label: 'Violet Purple', color: '#a855f7' },
      { id: 'hair_color:red', label: 'Crimson Red', color: '#ef4444' },
      { id: 'hair_color:green', label: 'Emerald Green', color: '#22c55e' },
      { id: 'hair_color:ginger_orange', label: 'Ginger Orange', color: '#f97316' },
      { id: 'hair_color:two_tone', label: 'Two-Tone Split', color: '#6366f1' },
      { id: 'hair_color:none_covered', label: 'Covered / None', color: '#64748b' },
    ],
  },
  {
    id: 'eyes',
    name: 'Eye Color',
    options: [
      { id: 'eye_color:blue', label: 'Sky Blue', color: '#38bdf8' },
      { id: 'eye_color:red', label: 'Ruby Red', color: '#ef4444' },
      { id: 'eye_color:green', label: 'Jade Green', color: '#22c55e' },
      { id: 'eye_color:purple', label: 'Amethyst Purple', color: '#a855f7' },
      { id: 'eye_color:gold_amber', label: 'Gold Amber', color: '#fbbf24' },
      { id: 'eye_color:glowing_white', label: 'Glowing White', color: '#ffffff' },
      { id: 'eye_color:pink', label: 'Pink Blush', color: '#f472b6' },
      { id: 'eye_color:brown', label: 'Earth Brown', color: '#78350f' },
      { id: 'eye_color:dark_black', label: 'Obsidian Black', color: '#09090b' },
      { id: 'eye_color:heterochromia', label: 'Heterochromia', color: '#06b6d4' },
    ],
  },
  {
    id: 'shading',
    name: 'Shading Style',
    options: [
      { id: 'shading_style:textured_detailed', label: 'Textured Detailed' },
      { id: 'shading_style:soft_aesthetic', label: 'Soft Aesthetic' },
      { id: 'shading_style:high_contrast_dithered', label: 'High Contrast' },
      { id: 'shading_style:flat_retro', label: 'Flat Retro' },
    ],
  },
  {
    id: 'gender',
    name: 'Silhouette',
    options: [
      { id: 'gender_presentation:masculine', label: 'Masculine' },
      { id: 'gender_presentation:feminine', label: 'Feminine' },
      { id: 'gender_presentation:androgynous_neutral', label: 'Neutral' },
    ],
  },
];

// Presets
const PRESETS = [
  {
    id: 'cyberpunk',
    name: 'Cyberpunk Cyborg',
    badge: 'Sci-Fi',
    variant: 'classic',
    palette: ['#0b132b', '#00f0ff', '#ff0055'],
    slots: {
      theme: 'theme:cyberpunk_scifi',
      archetype: 'archetype:robot_cyborg',
      outfit: 'outfit:tactical_vest',
      headwear: 'headwear:mask_visor',
      hair: 'hair_color:silver_white',
      eyes: 'eye_color:blue',
      shading: 'shading_style:textured_detailed',
      gender: 'gender_presentation:masculine',
    },
  },
  {
    id: 'bunny_hoodie',
    name: 'Pastel Bunny Hoodie',
    badge: 'Pastel',
    variant: 'slim',
    palette: ['#fce7f3', '#f472b6', '#a855f7'],
    slots: {
      theme: 'theme:cute_pastel',
      archetype: 'archetype:teen_casual',
      outfit: 'outfit:hoodie',
      headwear: 'headwear:animal_ears',
      hair: 'hair_color:blonde',
      eyes: 'eye_color:pink',
      shading: 'shading_style:soft_aesthetic',
      gender: 'gender_presentation:feminine',
    },
  },
  {
    id: 'paladin',
    name: 'Paladin Knight',
    badge: 'Fantasy',
    variant: 'classic',
    palette: ['#1e293b', '#64748b', '#fbbf24'],
    slots: {
      theme: 'theme:medieval_fantasy',
      archetype: 'archetype:knight_warrior',
      outfit: 'outfit:plate_armor',
      headwear: 'headwear:helmet',
      hair: 'hair_color:dark_brown',
      eyes: 'eye_color:gold_amber',
      shading: 'shading_style:textured_detailed',
      gender: 'gender_presentation:masculine',
    },
  },
  {
    id: 'anime_cloak',
    name: 'Anime Cloak Rogue',
    badge: 'Anime',
    variant: 'classic',
    palette: ['#0f172a', '#1e40af', '#38bdf8'],
    slots: {
      theme: 'theme:anime',
      archetype: 'archetype:rogue_adventurer',
      outfit: 'outfit:cloak_cape',
      headwear: 'headwear:hood_up',
      hair: 'hair_color:blue',
      eyes: 'eye_color:dark_black',
      shading: 'shading_style:textured_detailed',
      gender: 'gender_presentation:masculine',
    },
  },
  {
    id: 'druid',
    name: 'Druid of the Forest',
    badge: 'Nature',
    variant: 'slim',
    palette: ['#064e3b', '#059669', '#34d399'],
    slots: {
      theme: 'theme:medieval_fantasy',
      archetype: 'archetype:mage_wizard',
      outfit: 'outfit:tunic',
      headwear: 'headwear:horns',
      hair: 'hair_color:green',
      eyes: 'eye_color:green',
      shading: 'shading_style:soft_aesthetic',
      gender: 'gender_presentation:androgynous_neutral',
    },
  },
  {
    id: 'nether',
    name: 'Nether Assassin',
    badge: 'Nether',
    variant: 'classic',
    palette: ['#18181b', '#991b1b', '#f97316'],
    slots: {
      theme: 'theme:nether_ender',
      archetype: 'archetype:ninja_assassin',
      outfit: 'outfit:hoodie',
      headwear: 'headwear:mask_visor',
      hair: 'hair_color:black',
      eyes: 'eye_color:red',
      shading: 'shading_style:high_contrast_dithered',
      gender: 'gender_presentation:masculine',
    },
  },
];

// State
const selectedVariant = ref('classic');
const activePresetKey = ref('cyberpunk');
const selectedSlots = ref({ ...PRESETS[0].slots });
const palette = ref([...PRESETS[0].palette]);

const isRandomSeed = ref(true);
const manualSeed = ref(0);

// UI Accordion State (only theme open by default, users can toggle groups)
const showPresets = ref(false);
const openGroups = ref({
  theme: true,
  archetype: false,
  outfit: false,
  headwear: false,
  hair: false,
  eyes: false,
  shading: false,
  gender: false,
});

// Candidates are empty on first load (user must trigger generation)
const candidates = ref([]);
const isGenerating = ref(false);
const generationTime = ref(0);
const progressStage = ref('idle'); // 'idle' | 'loading_model' | 'generating' | 'postprocessing' | 'completed'
const progressStep = ref(0);
const progressTotalSteps = ref(10);
const progressMessage = ref('');
const targetProgress = ref(0);
const lerpedProgress = ref(0);
let lerpAnimId = null;

function startLerpLoop() {
  if (lerpAnimId) return;
  const tick = () => {
    const diff = targetProgress.value - lerpedProgress.value;
    if (Math.abs(diff) > 0.05) {
      lerpedProgress.value += diff * 0.12;
    } else {
      lerpedProgress.value = targetProgress.value;
    }

    if (isGenerating.value || Math.abs(diff) > 0.05) {
      lerpAnimId = requestAnimationFrame(tick);
    } else {
      lerpAnimId = null;
    }
  };
  lerpAnimId = requestAnimationFrame(tick);
}

function setTargetProgress(val) {
  targetProgress.value = Math.min(100, Math.max(0, val));
  startLerpLoop();
}

let unlistenProgress = null;

onMounted(async () => {
  try {
    unlistenProgress = await api.onSkinAiProgress((payload) => {
      progressStage.value = payload.stage;
      progressStep.value = payload.step;
      progressTotalSteps.value = payload.total_steps || 10;
      progressMessage.value = payload.message || '';
      setTargetProgress(payload.percentage);
    });
  } catch (err) {
    console.warn('Could not register skin AI progress listener:', err);
  }
});

onUnmounted(() => {
  if (unlistenProgress) unlistenProgress();
  if (lerpAnimId) cancelAnimationFrame(lerpAnimId);
});

const activeSlotCount = computed(() => {
  return Object.values(selectedSlots.value).filter(Boolean).length;
});

const activePresetName = computed(() => {
  if (activePresetKey.value === 'custom') return 'Custom';
  const p = PRESETS.find((pr) => pr.id === activePresetKey.value);
  return p ? p.name : 'Custom';
});

function toggleGroup(catId) {
  openGroups.value[catId] = !openGroups.value[catId];
}

function getOptionLabel(cat, optionId) {
  const opt = cat.options.find((o) => o.id === optionId);
  return opt ? opt.label : optionId;
}

function selectOption(catId, optionId) {
  if (selectedSlots.value[catId] === optionId) {
    delete selectedSlots.value[catId];
  } else {
    selectedSlots.value[catId] = optionId;
  }
  activePresetKey.value = 'custom';
}

function clearSlot(catId) {
  delete selectedSlots.value[catId];
  activePresetKey.value = 'custom';
}

function applyPreset(preset) {
  selectedVariant.value = preset.variant;
  selectedSlots.value = { ...preset.slots };
  palette.value = [...preset.palette];
  activePresetKey.value = preset.id;
  showPresets.value = false;
}

function randomizeSlot(cat) {
  const opts = cat.options;
  const pick = opts[Math.floor(Math.random() * opts.length)];
  selectedSlots.value[cat.id] = pick.id;
  activePresetKey.value = 'custom';
}

function randomizeAllSlots() {
  const newSlots = {};
  for (const cat of CATEGORIES) {
    const opts = cat.options;
    const pick = opts[Math.floor(Math.random() * opts.length)];
    newSlots[cat.id] = pick.id;
  }
  selectedSlots.value = newSlots;
  activePresetKey.value = 'custom';
}

function clearAllSlots() {
  selectedSlots.value = {};
  activePresetKey.value = 'custom';
}

function generateHighEntropySeed() {
  const buf = new Uint32Array(2);
  window.crypto.getRandomValues(buf);
  return Number(((BigInt(buf[0]) << 32n) | BigInt(buf[1])) & 0x7fffffffffffffffn);
}

// Generate Batch with Authentic ONNX Execution
async function generateBatch() {
  if (isGenerating.value) return;

  const runSeed = isRandomSeed.value ? generateHighEntropySeed() : (manualSeed.value || generateHighEntropySeed());
  isGenerating.value = true;
  progressStage.value = 'preparing';
  progressStep.value = 0;
  progressTotalSteps.value = 10;
  progressMessage.value = 'Preparing synthesis request...';
  targetProgress.value = 3;
  lerpedProgress.value = 0;
  startLerpLoop();

  try {
    const tagsList = Object.values(selectedSlots.value).filter(Boolean);

    const res = await api.generateSkinBatch({
      tags: tagsList,
      palette: palette.value,
      variant: selectedVariant.value,
      steps: 10,
      guidance_scale: 3.5,
      seed: runSeed,
    });

    generationTime.value = res.generation_time_ms;
    candidates.value = res.variants;
    targetProgress.value = 100;
    progressStage.value = 'completed';
    progressMessage.value = 'Generation complete!';
  } catch (err) {
    console.error('Skin AI generation error:', err);
  } finally {
    setTimeout(() => {
      isGenerating.value = false;
    }, 450);
  }
}

function editInStudio(cand) {
  emit('edit-in-studio', {
    dataUrl: cand.data_url,
    name: `ai_skin_${selectedVariant.value}_${Date.now()}.png`,
    variant: cand.variant || selectedVariant.value,
  });
}

async function quickSave(cand) {
  try {
    const base64 = cand.data_url.split(',')[1];
    const binary = atob(base64);
    const bytes = new Uint8Array(binary.length);
    for (let i = 0; i < binary.length; i++) {
      bytes[i] = binary.charCodeAt(i);
    }
    const name = `ai_skin_${Date.now()}.png`;
    await api.saveSkinBytes(name, Array.from(bytes), cand.variant || selectedVariant.value);
    emit('skin-saved');
  } catch (err) {
    console.error('Failed to save AI skin:', err);
  }
}

async function wearSkinNow(cand) {
  try {
    const base64 = cand.data_url.split(',')[1];
    const binary = atob(base64);
    const bytes = new Uint8Array(binary.length);
    for (let i = 0; i < binary.length; i++) {
      bytes[i] = binary.charCodeAt(i);
    }
    const name = `active_ai_${Date.now()}.png`;
    await api.saveSkinBytes(name, Array.from(bytes), cand.variant || selectedVariant.value);
    emit('skin-saved');
  } catch (err) {
    console.error('Failed to wear AI skin:', err);
  }
}
</script>

<style scoped>
@keyframes led-spin {
  0% {
    transform: rotate(0deg);
  }
  100% {
    transform: rotate(360deg);
  }
}

.animate-led-spin {
  animation: led-spin 3.2s linear infinite;
}

@keyframes shimmer-text {
  0% {
    background-position: -200% 0;
  }
  100% {
    background-position: 200% 0;
  }
}

.animate-shimmer-text {
  background-size: 200% auto;
  animation: shimmer-text 2.2s linear infinite;
}

@keyframes shimmer-bar {
  0% {
    transform: translateX(-100%);
  }
  100% {
    transform: translateX(100%);
  }
}

.animate-shimmer-bar {
  animation: shimmer-bar 1.6s ease-in-out infinite;
}

.custom-scrollbar::-webkit-scrollbar {
  width: 5px;
}
.custom-scrollbar::-webkit-scrollbar-track {
  background: transparent;
}
.custom-scrollbar::-webkit-scrollbar-thumb {
  background: rgba(255, 255, 255, 0.1);
  border-radius: 9999px;
}
.custom-scrollbar::-webkit-scrollbar-thumb:hover {
  background: var(--color-accent-border);
}
</style>
