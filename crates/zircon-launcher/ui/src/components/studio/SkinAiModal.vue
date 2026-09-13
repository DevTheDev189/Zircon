<template>
  <div class="fixed inset-0 z-50 flex items-center justify-center p-3 sm:p-5 bg-black/80 backdrop-blur-md">
    <div
      class="w-full max-w-5xl bg-[#080d14] border border-slate-700/80 rounded-2xl shadow-[0_16px_50px_rgba(0,0,0,0.8)] overflow-hidden flex flex-col max-h-[94vh] transition-all"
    >
      <!-- HEADER -->
      <div class="px-5 py-3.5 bg-[#0c141f] border-b border-slate-800 flex items-center justify-between shrink-0">
        <div class="flex items-center gap-3">
          <div class="w-9 h-9 rounded-xl bg-cyan-500/15 border border-cyan-400/40 flex items-center justify-center text-cyan-300 shadow-[0_0_12px_rgba(71,210,201,0.25)]">
            <svg class="w-5 h-5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
              <path d="m12 3-1.9 5.8a2 2 0 0 1-1.3 1.3L3 12l5.8 1.9a2 2 0 0 1 1.3 1.3L12 21l1.9-5.8a2 2 0 0 1 1.3-1.3L21 12l-5.8-1.9a2 2 0 0 1-1.3-1.3L12 3z" />
            </svg>
          </div>
          <div>
            <div class="flex items-center gap-2">
              <h2 class="text-sm sm:text-base font-extrabold text-white tracking-wide">
                AI Skin Studio
              </h2>
              <span class="px-2 py-0.5 rounded-full text-[10px] font-extrabold tracking-wider uppercase bg-cyan-500/15 border border-cyan-400/40 text-cyan-300 flex items-center gap-1 shadow-[0_0_8px_rgba(71,210,201,0.2)]">
                <span class="w-1.5 h-1.5 rounded-full bg-cyan-400 animate-pulse"></span>
                Local ONNX Flow Matching
              </span>
            </div>
            <p class="text-xs text-slate-400 mt-0.5">
              80M Geometry-Grounded Mesh-DiT • 100% offline edge inference on your hardware
            </p>
          </div>
        </div>

        <div class="flex items-center gap-2">
          <!-- Arm Variant Toggle -->
          <div class="flex rounded-lg bg-[#070b10] p-0.5 border border-slate-700/80">
            <button
              type="button"
              class="px-2.5 py-1 text-xs font-semibold rounded-md transition-all"
              :class="selectedVariant === 'classic' ? 'bg-cyan-500 text-slate-950 font-bold shadow-sm' : 'text-slate-400 hover:text-white'"
              @click="selectedVariant = 'classic'"
            >
              Classic (4px)
            </button>
            <button
              type="button"
              class="px-2.5 py-1 text-xs font-semibold rounded-md transition-all"
              :class="selectedVariant === 'slim' ? 'bg-cyan-500 text-slate-950 font-bold shadow-sm' : 'text-slate-400 hover:text-white'"
              @click="selectedVariant = 'slim'"
            >
              Slim (3px)
            </button>
          </div>

          <!-- Close Modal -->
          <button
            type="button"
            class="p-2 rounded-xl text-slate-400 hover:text-white hover:bg-slate-800 transition-colors"
            @click="$emit('close')"
          >
            <svg class="w-5 h-5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
              <line x1="18" y1="6" x2="6" y2="18" />
              <line x1="6" y1="6" x2="18" y2="18" />
            </svg>
          </button>
        </div>
      </div>

      <!-- BODY CONTENT -->
      <div class="flex-1 overflow-y-auto p-4 sm:p-5 space-y-5">

        <!-- DOWNLOAD BANNER IF MODEL NOT FOUND -->
        <div
          v-if="!modelStatus.is_downloaded"
          class="p-4 rounded-xl bg-gradient-to-r from-cyan-950/40 via-[#0e1a2b] to-slate-900 border border-cyan-500/30 flex flex-col md:flex-row items-center justify-between gap-4 shadow-lg"
        >
          <div class="flex items-center gap-3.5">
            <div class="w-10 h-10 rounded-xl bg-cyan-500/20 border border-cyan-400/40 flex items-center justify-center text-cyan-300 shrink-0">
              <svg class="w-5 h-5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                <path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4" />
                <polyline points="7 10 12 15 17 10" />
                <line x1="12" y1="15" x2="12" y2="3" />
              </svg>
            </div>
            <div>
              <h3 class="text-xs font-bold text-white uppercase tracking-wider">Local Pixel-DiT Model Setup (~65 MB)</h3>
              <p class="text-xs text-slate-300 mt-0.5">
                Download the trained ONNX model weights to unlock instant offline generation with zero cloud reliance.
              </p>
            </div>
          </div>

          <div class="shrink-0 flex items-center gap-2.5">
            <div v-if="modelStatus.is_downloading" class="flex items-center gap-3">
              <div class="flex flex-col items-end text-xs">
                <span class="font-bold text-cyan-400">{{ downloadProgress.percentage.toFixed(1) }}%</span>
                <span class="text-slate-400 text-[10px]">{{ (downloadProgress.speed_bytes_per_sec / 1048576).toFixed(1) }} MB/s</span>
              </div>
              <button
                type="button"
                class="px-3 py-1.5 rounded-lg bg-red-500/20 hover:bg-red-500/30 text-red-300 text-xs font-semibold"
                @click="cancelDownload"
              >
                Cancel
              </button>
            </div>
            <button
              v-else
              type="button"
              class="z-btn-accent text-xs px-4 py-2 rounded-xl font-bold flex items-center gap-2 shadow-md hover:shadow-cyan-500/25"
              @click="startDownload"
            >
              Download Model (~65 MB)
            </button>
          </div>
        </div>

        <!-- 1-CLICK INSPIRATION PRESETS -->
        <div class="space-y-2">
          <div class="flex items-center justify-between">
            <span class="text-[11px] font-bold uppercase tracking-wider text-slate-400 flex items-center gap-1.5">
              <span>Quick Presets</span>
              <span class="text-slate-500 font-normal lowercase">(instant 1-click combinations)</span>
            </span>
            <button
              type="button"
              class="text-[11px] text-cyan-400 hover:text-cyan-300 font-semibold flex items-center gap-1"
              @click="randomizeAllCategories"
              title="Randomize every category slot"
            >
              <span>🎲 Shuffle All Categories</span>
            </button>
          </div>

          <div class="grid grid-cols-2 sm:grid-cols-3 md:grid-cols-6 gap-2">
            <button
              v-for="preset in PRESETS"
              :key="preset.name"
              type="button"
              class="p-2.5 rounded-xl border transition-all text-left flex flex-col justify-between group relative overflow-hidden"
              :class="
                isPresetActive(preset)
                  ? 'bg-cyan-500/15 border-cyan-400/80 shadow-[0_0_12px_rgba(71,210,201,0.2)]'
                  : 'bg-[#0d1522] border-slate-800 hover:border-slate-700 hover:bg-[#111c2a]'
              "
              @click="applyPreset(preset)"
            >
              <div class="flex items-center justify-between w-full mb-1">
                <span class="text-base">{{ preset.icon }}</span>
                <span class="text-[9px] font-bold uppercase px-1.5 py-0.5 rounded bg-slate-800/80 text-slate-300">
                  {{ preset.badge }}
                </span>
              </div>
              <span class="text-xs font-bold text-white truncate group-hover:text-cyan-300 transition-colors">
                {{ preset.name }}
              </span>
              <!-- Mini palette preview bar -->
              <div class="h-1.5 w-full rounded-full mt-2 flex overflow-hidden">
                <div class="h-full flex-1" :style="{ backgroundColor: preset.palette[0] }"></div>
                <div class="h-full flex-1" :style="{ backgroundColor: preset.palette[1] }"></div>
                <div class="h-full flex-1" :style="{ backgroundColor: preset.palette[2] }"></div>
              </div>
            </button>
          </div>
        </div>

        <!-- CATEGORIZED SLOT SELECTOR (THE CORE NEW FEATURE) -->
        <div class="space-y-3 bg-[#0c141f] border border-slate-800/90 rounded-2xl p-4 shadow-inner">
          <div class="flex items-center justify-between border-b border-slate-800 pb-2.5">
            <div>
              <h3 class="text-xs font-bold text-white uppercase tracking-wider flex items-center gap-2">
                <span>Prompt Attributes by Category</span>
                <span class="text-[10px] font-normal lowercase text-slate-400">
                  ({{ activeSlotCount }} of 8 active)
                </span>
              </h3>
              <p class="text-[11px] text-slate-400 mt-0.5">
                Select tags under each category to condition the Flow Matching vector field.
              </p>
            </div>

            <div class="flex items-center gap-2">
              <button
                type="button"
                class="px-2.5 py-1 text-[11px] font-semibold text-slate-400 hover:text-white rounded-lg bg-slate-800/70 hover:bg-slate-700 transition-colors"
                @click="clearAllSlots"
              >
                Clear All
              </button>
            </div>
          </div>

          <!-- Category Sections Grid -->
          <div class="space-y-3.5 pt-1">
            <div
              v-for="cat in CATEGORIES"
              :key="cat.id"
              class="flex flex-col sm:flex-row sm:items-center gap-2 pb-3 border-b border-slate-800/60 last:border-b-0 last:pb-0"
            >
              <!-- Category Header / Title Column -->
              <div class="sm:w-36 shrink-0 flex items-center justify-between sm:justify-start gap-2">
                <span class="text-xs font-bold text-slate-300">
                  {{ cat.name }}
                </span>
                <div class="flex items-center gap-1">
                  <!-- Randomize this slot -->
                  <button
                    type="button"
                    class="p-1 rounded text-slate-400 hover:text-cyan-300 hover:bg-slate-800 transition-colors"
                    title="Randomize this slot"
                    @click="randomizeCategory(cat)"
                  >
                    🎲
                  </button>
                  <!-- Clear this slot if selected -->
                  <button
                    v-if="selectedSlots[cat.id]"
                    type="button"
                    class="p-1 rounded text-slate-500 hover:text-red-400 transition-colors"
                    title="Clear slot"
                    @click="clearSlot(cat.id)"
                  >
                    &times;
                  </button>
                </div>
              </div>

              <!-- Options Chip Row -->
              <div class="flex-1 flex flex-wrap items-center gap-1.5">
                <button
                  v-for="opt in cat.options"
                  :key="opt.id"
                  type="button"
                  class="px-2.5 py-1 rounded-lg text-xs font-medium transition-all flex items-center gap-1.5 border"
                  :class="
                    selectedSlots[cat.id] === opt.id
                      ? 'bg-cyan-500/20 border-cyan-400 text-cyan-200 font-bold shadow-[0_0_8px_rgba(71,210,201,0.25)]'
                      : 'bg-[#070c14] border-slate-800/90 text-slate-300 hover:text-white hover:border-slate-700 hover:bg-[#101928]'
                  "
                  @click="toggleSlot(cat.id, opt.id)"
                >
                  <!-- Color Dot if Option Has Swatch -->
                  <span
                    v-if="opt.color"
                    class="w-2.5 h-2.5 rounded-full shrink-0 border border-black/40"
                    :style="{ backgroundColor: opt.color }"
                  ></span>
                  <span>{{ opt.label }}</span>
                </button>
              </div>
            </div>
          </div>
        </div>

        <!-- PALETTE & ADVANCED INFERENCE GUIDANCE -->
        <div class="grid grid-cols-1 md:grid-cols-2 gap-4">
          <!-- Palette Swatches & Live Ribbon -->
          <div class="bg-[#0c141f] border border-slate-800 rounded-2xl p-4 space-y-3">
            <div class="flex items-center justify-between">
              <label class="text-xs font-bold uppercase tracking-wider text-slate-300">
                Color Palette Guidance
              </label>
              <span class="text-[11px] text-slate-400">Primary • Secondary • Accent</span>
            </div>

            <div class="flex items-center gap-3">
              <!-- Primary -->
              <div class="flex-1 flex items-center gap-2 bg-[#070c14] px-2.5 py-1.5 rounded-xl border border-slate-800">
                <input v-model="palette[0]" type="color" class="w-6 h-6 rounded cursor-pointer bg-transparent border-0" />
                <span class="text-[11px] font-mono text-slate-200 uppercase">{{ palette[0] }}</span>
              </div>
              <!-- Secondary -->
              <div class="flex-1 flex items-center gap-2 bg-[#070c14] px-2.5 py-1.5 rounded-xl border border-slate-800">
                <input v-model="palette[1]" type="color" class="w-6 h-6 rounded cursor-pointer bg-transparent border-0" />
                <span class="text-[11px] font-mono text-slate-200 uppercase">{{ palette[1] }}</span>
              </div>
              <!-- Accent -->
              <div class="flex-1 flex items-center gap-2 bg-[#070c14] px-2.5 py-1.5 rounded-xl border border-slate-800">
                <input v-model="palette[2]" type="color" class="w-6 h-6 rounded cursor-pointer bg-transparent border-0" />
                <span class="text-[11px] font-mono text-slate-200 uppercase">{{ palette[2] }}</span>
              </div>
            </div>

            <!-- Live Ribbon Gradient Bar -->
            <div
              class="h-3 w-full rounded-lg shadow-inner border border-slate-700/50"
              :style="{
                background: `linear-gradient(to right, ${palette[0]}, ${palette[1]}, ${palette[2]})`
              }"
            ></div>
          </div>

          <!-- Seed Controls & Generation Tuning -->
          <div class="bg-[#0c141f] border border-slate-800 rounded-2xl p-4 space-y-3">
            <div class="flex items-center justify-between">
              <label class="text-xs font-bold uppercase tracking-wider text-slate-300">
                Inference & Seed Control
              </label>
              <div class="flex items-center gap-1.5">
                <button
                  type="button"
                  class="px-2 py-0.5 rounded text-[11px] font-semibold transition-colors"
                  :class="isRandomSeed ? 'bg-cyan-500/20 text-cyan-300 font-bold' : 'text-slate-400 hover:text-white'"
                  @click="isRandomSeed = true"
                >
                  🎲 Random Seed
                </button>
                <button
                  type="button"
                  class="px-2 py-0.5 rounded text-[11px] font-semibold transition-colors"
                  :class="!isRandomSeed ? 'bg-cyan-500/20 text-cyan-300 font-bold' : 'text-slate-400 hover:text-white'"
                  @click="lockCurrentSeed"
                >
                  🔒 Locked
                </button>
              </div>
            </div>

            <div class="flex items-center gap-3">
              <div class="flex-1 bg-[#070c14] px-3 py-1.5 rounded-xl border border-slate-800 flex items-center justify-between">
                <span class="text-xs text-slate-400 font-semibold">Base Seed:</span>
                <span class="text-xs font-mono text-cyan-300 font-bold">{{ activeSeedDisplay }}</span>
                <button
                  type="button"
                  class="p-1 text-slate-400 hover:text-white"
                  title="Generate new random seed"
                  @click="shuffleBaseSeed"
                >
                  🎲
                </button>
              </div>

              <div class="flex items-center gap-2 bg-[#070c14] px-3 py-1.5 rounded-xl border border-slate-800">
                <span class="text-xs text-slate-400 font-semibold">Steps:</span>
                <input
                  v-model.number="odeSteps"
                  type="number"
                  min="4"
                  max="20"
                  class="w-10 bg-transparent text-xs font-bold text-white text-center focus:outline-none"
                />
              </div>

              <div class="flex items-center gap-2 bg-[#070c14] px-3 py-1.5 rounded-xl border border-slate-800">
                <span class="text-xs text-slate-400 font-semibold">CFG:</span>
                <span class="text-xs font-bold text-white">{{ cfgScale.toFixed(1) }}</span>
              </div>
            </div>

            <div class="text-[11px] text-slate-500">
              Each of the 4 candidates receives a mathematically scrambled seed for diverse, unique creative variations.
            </div>
          </div>
        </div>

        <!-- GENERATE ACTIONS & TELEMETRY BAR -->
        <div class="flex flex-col sm:flex-row items-center justify-between gap-3 pt-2">
          <div class="text-xs text-slate-400 flex items-center gap-2">
            <span v-if="generationTime" class="text-cyan-300 font-semibold">
              ⚡ Generated 4 authentic skins in {{ (generationTime / 1000).toFixed(2) }}s ({{ (generationTime / 4000).toFixed(2) }}s / skin)
            </span>
            <span v-else class="text-slate-500">
              Ready to generate 4 distinct candidates using ONNX Flow Matching
            </span>
          </div>

          <div class="flex items-center gap-3 w-full sm:w-auto">
            <button
              v-if="candidates.length > 0"
              type="button"
              class="z-btn-ghost flex-1 sm:flex-none px-4 py-2.5 rounded-xl text-slate-200 font-bold text-xs flex items-center justify-center gap-2 transition-colors border border-slate-700 hover:border-slate-600"
              :disabled="isGenerating"
              @click="reRollBatch"
            >
              <svg class="w-4 h-4 text-cyan-400" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                <path d="M21.5 2v6h-6M21.34 15.57a10 10 0 1 1-.57-8.38l5.67-5.67" />
              </svg>
              <span>Re-roll Seeds</span>
            </button>

            <button
              type="button"
              class="z-btn-accent relative overflow-hidden flex-1 sm:flex-none px-6 py-2.5 rounded-xl text-slate-950 font-extrabold text-xs shadow-lg shadow-cyan-500/25 flex items-center justify-center gap-2 transition-transform active:scale-95 disabled:opacity-50 cursor-pointer"
              :disabled="isGenerating"
              @click="generateBatch"
            >
              <!-- Underlay fill matching lerp progress -->
              <div
                v-if="isGenerating"
                class="absolute left-0 top-0 bottom-0 bg-white/20 pointer-events-none transition-[width] duration-75"
                :style="{ width: `${lerpedProgress}%` }"
              ></div>

              <svg v-if="isGenerating" class="w-4 h-4 animate-spin z-10" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5">
                <circle cx="12" cy="12" r="10" stroke-opacity="0.25" stroke="currentColor" />
                <path d="M12 2a10 10 0 0 1 10 10" />
              </svg>
              <svg v-else class="w-4 h-4" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5">
                <polygon points="5 3 19 12 5 21 5 3" />
              </svg>
              <span class="z-10">
                {{
                  !isGenerating
                    ? 'Generate 4 Authentic Candidates'
                    : progressStage === 'loading_model'
                      ? 'Loading Model to GPU VRAM...'
                      : progressStage === 'postprocessing'
                        ? 'Finalizing Textures...'
                        : `Solving ODE (${Math.round(lerpedProgress)}%)...`
                }}
              </span>
            </button>
          </div>
        </div>

        <!-- MODEL LOADING INTO VRAM CALLOUT BANNER -->
        <div
          v-if="isGenerating && progressStage === 'loading_model'"
          class="px-4 py-3 rounded-2xl bg-amber-500/10 border border-amber-500/30 backdrop-blur-xl flex items-center justify-between shadow-[0_0_24px_rgba(245,158,11,0.15)] animate-in fade-in slide-in-from-top-1 duration-300"
        >
          <div class="flex items-center gap-3">
            <div class="w-8 h-8 rounded-xl bg-amber-500/20 border border-amber-500/40 flex items-center justify-center text-amber-300 shrink-0 shadow-[0_0_12px_rgba(245,158,11,0.3)]">
              <svg class="w-4 h-4 animate-pulse" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
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

        <!-- GENERATION PROGRESS INDICATOR WITH SMOOTH LERP ANIMATION -->
        <div v-if="isGenerating" class="space-y-2 p-3 rounded-2xl bg-[#080e18]/80 border border-slate-800 shadow-inner animate-in fade-in duration-200">
          <div class="flex items-center justify-between text-xs">
            <div class="flex items-center gap-2 font-mono">
              <span class="relative flex h-2 w-2">
                <span
                  class="animate-ping absolute inline-flex h-full w-full rounded-full opacity-75"
                  :class="progressStage === 'loading_model' ? 'bg-amber-400' : 'bg-cyan-400'"
                ></span>
                <span
                  class="relative inline-flex rounded-full h-2 w-2"
                  :class="progressStage === 'loading_model' ? 'bg-amber-500' : 'bg-cyan-500'"
                ></span>
              </span>
              <span class="font-bold text-slate-200">
                {{
                  progressStage === 'loading_model'
                    ? 'Initializing DirectML Session in GPU VRAM...'
                    : progressStage === 'postprocessing'
                      ? 'Post-Processing Dual-Layer Minecraft Skins...'
                      : `Euler Flow Matching: Step ${progressStep} / ${progressTotalSteps}`
                }}
              </span>
            </div>
            <div class="flex items-center gap-2">
              <span class="text-[11px] text-slate-400 font-mono hidden sm:inline">{{ progressMessage }}</span>
              <span
                class="text-xs font-mono font-black px-2 py-0.5 rounded-md border"
                :class="
                  progressStage === 'loading_model'
                    ? 'bg-amber-500/15 border-amber-500/40 text-amber-300'
                    : 'bg-slate-900 border-cyan-500/40 text-cyan-300'
                "
              >
                {{ Math.round(lerpedProgress) }}%
              </span>
            </div>
          </div>

          <!-- Lerped Progress Track -->
          <div class="h-2 w-full bg-[#070b10] rounded-full overflow-hidden border border-slate-800 relative p-[1px]">
            <div
              class="h-full rounded-full relative overflow-hidden transition-[width] duration-75"
              :class="
                progressStage === 'loading_model'
                  ? 'bg-gradient-to-r from-amber-500 via-orange-400 to-amber-300 shadow-[0_0_12px_rgba(245,158,11,0.5)]'
                  : 'bg-gradient-to-r from-cyan-400 via-blue-500 to-indigo-500 shadow-[0_0_12px_rgba(6,182,212,0.5)]'
              "
              :style="{ width: `${Math.min(100, Math.max(3, lerpedProgress))}%` }"
            >
              <div class="absolute inset-0 bg-gradient-to-r from-transparent via-white/40 to-transparent animate-shimmer-bar pointer-events-none"></div>
            </div>
          </div>
        </div>

        <!-- 4-CANDIDATE GRID (AUTHENTIC HIGH-FIDELITY RESULTS) -->
        <div v-if="candidates.length > 0" class="space-y-3 pt-2">
          <div class="flex items-center justify-between">
            <h3 class="text-xs font-bold uppercase tracking-wider text-slate-300">
              Generated Candidates (Authentic ONNX Output):
            </h3>
            <span class="text-[11px] text-cyan-400 font-medium">Click "Edit in Studio" or "Wear Now" to use your skin!</span>
          </div>

          <div class="grid grid-cols-2 md:grid-cols-4 gap-4">
            <div
              v-for="cand in candidates"
              :key="cand.seed"
              class="bg-[#0b121c] border border-slate-700/80 hover:border-cyan-400/80 rounded-2xl p-3 flex flex-col items-center gap-3 transition-all duration-200 group hover:shadow-xl hover:shadow-cyan-500/15"
            >
              <!-- 3D LIVE ISOMETRIC PREVIEW -->
              <div class="w-full h-44 bg-[#060a0f] rounded-xl flex items-center justify-center overflow-hidden relative shadow-inner">
                <img
                  v-if="cand.render3d"
                  :src="cand.render3d"
                  alt="3D Preview"
                  class="h-40 object-contain drop-shadow-[0_10px_20px_rgba(0,0,0,0.8)] group-hover:scale-105 transition-transform"
                />
                <img
                  v-else
                  :src="cand.data_url"
                  alt="Flat 2D Texture"
                  class="w-24 h-24 image-pixelated object-contain"
                />

                <span class="absolute top-2 left-2 px-1.5 py-0.5 rounded bg-slate-900/90 text-[10px] font-mono text-cyan-300 border border-slate-800">
                  #{{ cand.index + 1 }}
                </span>

                <span class="absolute bottom-2 right-2 px-1.5 py-0.5 rounded bg-slate-900/90 text-[9px] font-mono text-slate-400 border border-slate-800 truncate max-w-[110px]" :title="`Seed: ${cand.seed}`">
                  seed: {{ String(cand.seed).slice(-5) }}
                </span>
              </div>

              <!-- ACTIONS: DROP STRAIGHT INTO EDITING OR WEAR -->
              <div class="w-full space-y-1.5">
                <button
                  type="button"
                  class="z-btn-accent w-full py-2 px-2.5 rounded-xl font-bold text-xs flex items-center justify-center gap-1.5 shadow-md hover:shadow-cyan-500/25 transition-all"
                  title="Open directly in the Paint Studio"
                  @click="editInStudio(cand)"
                >
                  <svg class="w-3.5 h-3.5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5">
                    <path d="M12 20h9" />
                    <path d="M16.5 3.5a2.121 2.121 0 0 1 3 3L7 19l-4 1 1-4L16.5 3.5z" />
                  </svg>
                  <span>Edit in Studio</span>
                </button>

                <div class="grid grid-cols-2 gap-1.5">
                  <button
                    type="button"
                    class="py-1.5 px-2 rounded-xl bg-slate-800 hover:bg-slate-700 text-slate-300 hover:text-white font-semibold text-[11px] transition-colors flex items-center justify-center gap-1"
                    title="Save to My Skins Gallery"
                    @click="quickSave(cand)"
                  >
                    <span>💾 Save</span>
                  </button>

                  <button
                    type="button"
                    class="py-1.5 px-2 rounded-xl bg-cyan-950/60 hover:bg-cyan-900/80 border border-cyan-500/40 text-cyan-300 hover:text-white font-semibold text-[11px] transition-colors flex items-center justify-center gap-1"
                    title="Wear this skin right now"
                    @click="wearSkinNow(cand)"
                  >
                    <span>👕 Wear</span>
                  </button>
                </div>
              </div>
            </div>
          </div>
        </div>

      </div>
    </div>
  </div>
</template>

<script setup>
import { ref, computed, onMounted, onUnmounted } from 'vue';
import { api, renderSkinIsometric3D, onSkinAiDownloadProgress, onSkinAiDownloadComplete, onSkinAiProgress } from '../../lib/api.js';

const emit = defineEmits(['close', 'edit-in-studio', 'skin-saved']);

// Categories specification matching the 8 discrete slots of Mesh-DiT model
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
    name: 'Gender / Build',
    options: [
      { id: 'gender_presentation:masculine', label: 'Masculine' },
      { id: 'gender_presentation:feminine', label: 'Feminine' },
      { id: 'gender_presentation:androgynous_neutral', label: 'Neutral' },
    ],
  },
];

// Curated 1-click presets matching Checkpoint 400 benchmarks
const PRESETS = [
  {
    name: 'Cyberpunk Cyborg',
    badge: 'Sci-Fi',
    icon: '⚡',
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
    name: 'Pastel Bunny Hoodie',
    badge: 'Cute',
    icon: '🐰',
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
    name: 'Paladin Knight',
    badge: 'Fantasy',
    icon: '⚔️',
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
    name: 'Anime Cloak Rogue',
    badge: 'Anime',
    icon: '🌌',
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
    name: 'Druid of the Forest',
    badge: 'Nature',
    icon: '🌿',
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
    name: 'Nether Assassin',
    badge: 'Nether',
    icon: '🔥',
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
const modelStatus = ref({ is_downloaded: true, is_downloading: false });
const downloadProgress = ref({ percentage: 0, speed_bytes_per_sec: 0 });
const selectedVariant = ref('classic');
const selectedSlots = ref({ ...PRESETS[0].slots });
const palette = ref([...PRESETS[0].palette]);

// Inference Tuning & Random Seed State
const isRandomSeed = ref(true);
const currentBaseSeed = ref(generateHighEntropySeed());
const odeSteps = ref(10);
const cfgScale = ref(3.5);

const candidates = ref([]);
const isGenerating = ref(false);
const generationTime = ref(0);
const progressStage = ref('idle');
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

function generateHighEntropySeed() {
  const buf = new Uint32Array(2);
  window.crypto.getRandomValues(buf);
  // Return positive 64-bit safe representation
  return (BigInt(buf[0]) << 32n) | BigInt(buf[1]);
}

const activeSeedDisplay = computed(() => {
  if (isRandomSeed.value) {
    return '🎲 [Auto-Random]';
  }
  return String(currentBaseSeed.value).slice(0, 12) + '…';
});

const activeSlotCount = computed(() => {
  return Object.values(selectedSlots.value).filter(Boolean).length;
});

function toggleSlot(categoryId, optionId) {
  if (selectedSlots.value[categoryId] === optionId) {
    delete selectedSlots.value[categoryId];
  } else {
    selectedSlots.value[categoryId] = optionId;
  }
}

function clearSlot(categoryId) {
  delete selectedSlots.value[categoryId];
}

function clearAllSlots() {
  selectedSlots.value = {};
}

function randomizeCategory(cat) {
  const opts = cat.options;
  const pick = opts[Math.floor(Math.random() * opts.length)];
  selectedSlots.value[cat.id] = pick.id;
}

function randomizeAllCategories() {
  const newSlots = {};
  for (const cat of CATEGORIES) {
    const opts = cat.options;
    const pick = opts[Math.floor(Math.random() * opts.length)];
    newSlots[cat.id] = pick.id;
  }
  selectedSlots.value = newSlots;
}

function applyPreset(preset) {
  selectedVariant.value = preset.variant;
  selectedSlots.value = { ...preset.slots };
  palette.value = [...preset.palette];
}

function isPresetActive(preset) {
  return Object.entries(preset.slots).every(([k, v]) => selectedSlots.value[k] === v);
}

function shuffleBaseSeed() {
  currentBaseSeed.value = generateHighEntropySeed();
  isRandomSeed.value = false;
}

function lockCurrentSeed() {
  isRandomSeed.value = false;
  if (!currentBaseSeed.value) {
    currentBaseSeed.value = generateHighEntropySeed();
  }
}

async function checkStatus() {
  try {
    const status = await api.getSkinAiStatus();
    modelStatus.value = status;
  } catch (err) {
    console.warn('Failed to check Skin AI status:', err);
  }
}

async function startDownload() {
  try {
    modelStatus.value.is_downloading = true;
    await api.startSkinAiDownload();
  } catch (err) {
    console.error('Failed to start model download:', err);
    modelStatus.value.is_downloading = false;
  }
}

async function cancelDownload() {
  try {
    await api.cancelSkinAiDownload();
    modelStatus.value.is_downloading = false;
  } catch (err) {
    console.error('Failed to cancel download:', err);
  }
}

// Generate Batch with Authentic ONNX Execution
async function generateBatch() {
  if (isGenerating.value) return;

  // Derive high-entropy random base seed if random mode enabled
  let runSeed = 0;
  if (isRandomSeed.value) {
    currentBaseSeed.value = generateHighEntropySeed();
    runSeed = Number(currentBaseSeed.value & 0x7fffffffffffffffn);
  } else {
    runSeed = Number(currentBaseSeed.value & 0x7fffffffffffffffn);
  }

  isGenerating.value = true;
  progressStage.value = 'preparing';
  progressStep.value = 0;
  progressTotalSteps.value = odeSteps.value;
  progressMessage.value = 'Preparing synthesis request...';
  targetProgress.value = 3;
  lerpedProgress.value = 0;
  startLerpLoop();

  try {
    // Collect active tags from the 8 slots
    const tagsList = Object.values(selectedSlots.value).filter(Boolean);

    const res = await api.generateSkinBatch({
      tags: tagsList,
      palette: palette.value,
      variant: selectedVariant.value,
      steps: odeSteps.value,
      guidance_scale: cfgScale.value,
      seed: runSeed,
    });

    generationTime.value = res.generation_time_ms;
    targetProgress.value = 100;
    progressStage.value = 'completed';
    progressMessage.value = 'Generation complete!';

    // Enrich variants with 3D isometric turnaround renders
    const enriched = await Promise.all(
      res.variants.map(async (v) => {
        const render = await renderSkinIsometric3D(v.data_url, v.variant || selectedVariant.value);
        return {
          ...v,
          render3d: render,
        };
      })
    );

    candidates.value = enriched;
  } catch (err) {
    console.error('Skin AI ONNX generation error:', err);
  } finally {
    setTimeout(() => {
      isGenerating.value = false;
    }, 450);
  }
}

async function reRollBatch() {
  if (isRandomSeed.value) {
    currentBaseSeed.value = generateHighEntropySeed();
  }
  await generateBatch();
}

function editInStudio(cand) {
  emit('edit-in-studio', {
    dataUrl: cand.data_url,
    name: `ai_skin_${selectedVariant.value}_${Date.now()}.png`,
    variant: cand.variant || selectedVariant.value,
  });
  emit('close');
}

async function quickSave(cand) {
  try {
    const base64 = cand.data_url.split(',')[1];
    const binary = atob(base64);
    const bytes = new Uint8Array(binary.length);
    for (let i = 0; i < binary.length; i++) {
      bytes[i] = binary.charCodeAt(i);
    }
    const name = `ai_${selectedSlots.value.theme ? selectedSlots.value.theme.split(':')[1] : 'skin'}_${Date.now()}.png`;
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
    emit('close');
  } catch (err) {
    console.error('Failed to wear AI skin:', err);
  }
}

let unlistenProgress = null;
let unlistenComplete = null;
let unlistenGenProgress = null;

onMounted(async () => {
  await checkStatus();
  try {
    unlistenProgress = await onSkinAiDownloadProgress((payload) => {
      downloadProgress.value = payload;
      modelStatus.value.is_downloading = true;
    });
    unlistenComplete = await onSkinAiDownloadComplete(() => {
      modelStatus.value.is_downloaded = true;
      modelStatus.value.is_downloading = false;
    });
    unlistenGenProgress = await onSkinAiProgress((payload) => {
      progressStage.value = payload.stage;
      progressStep.value = payload.step;
      progressTotalSteps.value = payload.total_steps || odeSteps.value;
      progressMessage.value = payload.message || '';
      setTargetProgress(payload.percentage);
    });
  } catch (err) {
    console.warn('Download listeners unavailable:', err);
  }

  // Pre-generate initial 4 candidates on open
  generateBatch();
});

onUnmounted(() => {
  if (unlistenProgress) unlistenProgress();
  if (unlistenComplete) unlistenComplete();
  if (unlistenGenProgress) unlistenGenProgress();
  if (lerpAnimId) cancelAnimationFrame(lerpAnimId);
});
</script>

<style scoped>
.image-pixelated {
  image-rendering: pixelated;
  image-rendering: -moz-crisp-edges;
  image-rendering: crisp-edges;
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
</style>
