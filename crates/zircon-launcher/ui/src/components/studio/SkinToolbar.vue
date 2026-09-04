<template>
  <div class="bg-[#0b121c] border border-slate-800/90 rounded-xl p-3 flex flex-col gap-3.5 select-none">
    <!-- Tools Grid -->
    <div>
      <div class="text-[10px] font-bold uppercase tracking-wider text-slate-400 mb-2">Tools</div>
      <div class="grid grid-cols-4 gap-1.5">
        <!-- Pencil -->
        <button
          type="button"
          class="h-9 rounded-lg border flex flex-col items-center justify-center gap-0.5 transition-all text-xs font-semibold relative group"
          :class="
            studio.state.activeTool === 'pencil'
              ? 'bg-cyan-500/25 border-cyan-400 text-cyan-200 shadow-[0_0_10px_rgba(56,189,248,0.25)]'
              : 'bg-slate-900/60 border-slate-800 text-slate-400 hover:text-white hover:border-slate-700'
          "
          title="Pencil (B) - Shift+Click for straight lines"
          @click="selectTool('pencil')"
        >
          <svg class="w-4 h-4" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
            <path d="M17 3a2.828 2.828 0 1 1 4 4L7.5 20.5 2 22l1.5-5.5L17 3z" />
          </svg>
          <span class="text-[8px] font-mono text-slate-500 group-hover:text-slate-400">B</span>
        </button>

        <!-- Eraser -->
        <button
          type="button"
          class="h-9 rounded-lg border flex flex-col items-center justify-center gap-0.5 transition-all text-xs font-semibold relative group"
          :class="
            studio.state.activeTool === 'eraser'
              ? 'bg-cyan-500/25 border-cyan-400 text-cyan-200 shadow-[0_0_10px_rgba(56,189,248,0.25)]'
              : 'bg-slate-900/60 border-slate-800 text-slate-400 hover:text-white hover:border-slate-700'
          "
          title="Eraser (E) - Clears pixels to transparent"
          @click="selectTool('eraser')"
        >
          <svg class="w-4 h-4" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
            <path d="m7 21-4.3-4.3c-1-1-1-2.5 0-3.4l9.6-9.6c1-1 2.5-1 3.4 0l5.6 5.6c1 1 1 2.5 0 3.4L13 21" />
            <path d="M22 21H7" />
            <path d="m5 11 9 9" />
          </svg>
          <span class="text-[8px] font-mono text-slate-500 group-hover:text-slate-400">E</span>
        </button>

        <!-- Eyedropper -->
        <button
          type="button"
          class="h-9 rounded-lg border flex flex-col items-center justify-center gap-0.5 transition-all text-xs font-semibold relative group"
          :class="
            studio.state.activeTool === 'picker'
              ? 'bg-cyan-500/25 border-cyan-400 text-cyan-200 shadow-[0_0_10px_rgba(56,189,248,0.25)]'
              : 'bg-slate-900/60 border-slate-800 text-slate-400 hover:text-white hover:border-slate-700'
          "
          title="Eyedropper (I) - Sample color from 2D or 3D"
          @click="selectTool('picker')"
        >
          <svg class="w-4 h-4" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
            <path d="m19 11-8-8-8.6 8.6a2 2 0 0 0 0 2.8l5.2 5.2c.8.8 2 .8 2.8 0L19 11Z" />
            <path d="m5 2 5 5" />
            <path d="m2 5 5 5" />
            <circle cx="2" cy="22" r="2" />
          </svg>
          <span class="text-[8px] font-mono text-slate-500 group-hover:text-slate-400">I</span>
        </button>

        <!-- Flood Fill (Bucket) -->
        <button
          type="button"
          class="h-9 rounded-lg border flex flex-col items-center justify-center gap-0.5 transition-all text-xs font-semibold relative group"
          :class="
            studio.state.activeTool === 'bucket'
              ? 'bg-cyan-500/25 border-cyan-400 text-cyan-200 shadow-[0_0_10px_rgba(56,189,248,0.25)]'
              : 'bg-slate-900/60 border-slate-800 text-slate-400 hover:text-white hover:border-slate-700'
          "
          title="Paint Bucket (G) - Region-bounded flood fill"
          @click="selectTool('bucket')"
        >
          <svg class="w-4 h-4" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
            <path d="m19 11-8-8-8.6 8.6a2 2 0 0 0 0 2.8l5.2 5.2c.8.8 2 .8 2.8 0L19 11Z" />
            <path d="m5 2 5 5" />
            <path d="M22 20a2 2 0 1 1-4 0c0-1.6 1.7-2.4 2-4 .3 1.6 2 2.4 2 4Z" />
          </svg>
          <span class="text-[8px] font-mono text-slate-500 group-hover:text-slate-400">G</span>
        </button>

        <!-- Noise Shading Brush -->
        <button
          type="button"
          class="h-9 rounded-lg border flex flex-col items-center justify-center gap-0.5 transition-all text-xs font-semibold relative group"
          :class="
            studio.state.activeTool === 'noise'
              ? 'bg-cyan-500/25 border-cyan-400 text-cyan-200 shadow-[0_0_10px_rgba(56,189,248,0.25)]'
              : 'bg-slate-900/60 border-slate-800 text-slate-400 hover:text-white hover:border-slate-700'
          "
          title="Noise Shading Brush (N) - Natural texture variations"
          @click="selectTool('noise')"
        >
          <svg class="w-4 h-4" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
            <path d="m12 3-1.9 5.8a2 2 0 0 1-1.3 1.3L3 12l5.8 1.9a2 2 0 0 1 1.3 1.3L12 21l1.9-5.8a2 2 0 0 1 1.3-1.3L21 12l-5.8-1.9a2 2 0 0 1-1.3-1.3L12 3z" />
          </svg>
          <span class="text-[8px] font-mono text-slate-500 group-hover:text-slate-400">N</span>
        </button>

        <!-- Lighten (Dodge) -->
        <button
          type="button"
          class="h-9 rounded-lg border flex flex-col items-center justify-center gap-0.5 transition-all text-xs font-semibold relative group"
          :class="
            studio.state.activeTool === 'lighten'
              ? 'bg-cyan-500/25 border-cyan-400 text-cyan-200 shadow-[0_0_10px_rgba(56,189,248,0.25)]'
              : 'bg-slate-900/60 border-slate-800 text-slate-400 hover:text-white hover:border-slate-700'
          "
          title="Lighten / Dodge - Brightens pixels by 8%"
          @click="selectTool('lighten')"
        >
          <svg class="w-4 h-4" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
            <circle cx="12" cy="12" r="4" />
            <path d="M12 2v2" />
            <path d="M12 20v2" />
            <path d="m4.93 4.93 1.41 1.41" />
            <path d="m17.66 17.66 1.41 1.41" />
            <path d="M2 12h2" />
            <path d="M20 12h2" />
            <path d="m6.34 17.66-1.41 1.41" />
            <path d="m19.07 4.93-1.41 1.41" />
          </svg>
          <span class="text-[8px] font-mono text-slate-500 group-hover:text-slate-400">+</span>
        </button>

        <!-- Darken (Burn) -->
        <button
          type="button"
          class="h-9 rounded-lg border flex flex-col items-center justify-center gap-0.5 transition-all text-xs font-semibold relative group"
          :class="
            studio.state.activeTool === 'darken'
              ? 'bg-cyan-500/25 border-cyan-400 text-cyan-200 shadow-[0_0_10px_rgba(56,189,248,0.25)]'
              : 'bg-slate-900/60 border-slate-800 text-slate-400 hover:text-white hover:border-slate-700'
          "
          title="Darken / Burn - Darkens pixels by 8%"
          @click="selectTool('darken')"
        >
          <svg class="w-4 h-4" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
            <path d="M12 3a6 6 0 0 0 9 9 9 9 0 1 1-9-9Z" />
          </svg>
          <span class="text-[8px] font-mono text-slate-500 group-hover:text-slate-400">-</span>
        </button>

        <!-- Color Swatch Indicator Button -->
        <div class="h-9 rounded-lg border border-slate-700/80 bg-[#070b10] flex items-center justify-center p-1 relative">
          <input
            v-model="studio.state.activeColor"
            type="color"
            class="w-full h-full cursor-pointer rounded border-0 bg-transparent p-0"
            @change="onNativeColorChange"
          />
        </div>
      </div>
    </div>

    <!-- Active Color & Controls -->
    <div class="bg-[#070b10] border border-slate-800/80 rounded-lg p-2.5 flex flex-col gap-2">
      <div class="flex items-center justify-between gap-2">
        <div class="flex items-center gap-2">
          <div
            class="w-6 h-6 rounded-md border border-slate-700 shadow-inner"
            :style="{ backgroundColor: studio.state.activeColor }"
          />
          <input
            v-model="hexInput"
            type="text"
            maxlength="7"
            class="w-20 bg-[#111c2a] border border-slate-700 rounded px-1.5 py-0.5 text-xs font-mono font-bold text-slate-200 uppercase focus:border-cyan-400 focus:outline-none"
            @change="onHexInputChange"
            @keydown.enter="onHexInputChange"
          />
        </div>

        <button
          type="button"
          class="p-1 rounded text-slate-400 hover:text-cyan-300 hover:bg-slate-800 transition-colors"
          title="Save active color to custom palette"
          @click="addCustomColor"
        >
          <svg class="w-4 h-4" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5">
            <line x1="12" y1="5" x2="12" y2="19" />
            <line x1="5" y1="12" x2="19" y2="12" />
          </svg>
        </button>
      </div>

      <!-- Opacity Slider -->
      <div class="flex items-center gap-2 pt-0.5">
        <span class="text-[10px] font-semibold text-slate-400 w-10">Alpha:</span>
        <input
          v-model.number="studio.state.activeAlpha"
          type="range"
          min="0"
          max="255"
          step="5"
          class="flex-1 h-1.5 bg-slate-800 rounded-lg appearance-none cursor-pointer accent-cyan-400"
        />
        <span class="text-[10px] font-mono text-slate-400 w-8 text-right">
          {{ Math.round((studio.state.activeAlpha / 255) * 100) }}%
        </span>
      </div>
    </div>

    <!-- Palette Presets -->
    <div class="flex flex-col gap-2">
      <div class="flex items-center justify-between">
        <span class="text-[10px] font-bold uppercase tracking-wider text-slate-400">Palettes</span>
        <div class="flex items-center gap-1">
          <button
            v-for="(label, key) in paletteTabs"
            :key="key"
            type="button"
            class="text-[9px] font-semibold px-1.5 py-0.5 rounded transition-colors"
            :class="
              studio.state.selectedPalette === key
                ? 'bg-cyan-500/20 text-cyan-300 font-bold'
                : 'text-slate-500 hover:text-slate-300'
            "
            @click="studio.state.selectedPalette = key"
          >
            {{ label }}
          </button>
        </div>
      </div>

      <!-- Palette Swatches Grid -->
      <div class="grid grid-cols-6 gap-1.5 p-1 bg-[#070b10] rounded-lg border border-slate-800/80">
        <button
          v-for="color in activePaletteColors"
          :key="color"
          type="button"
          class="w-full aspect-square rounded border border-black/40 hover:scale-110 transition-transform shadow-sm"
          :style="{ backgroundColor: color }"
          :title="color"
          @click="selectColor(color)"
        />
      </div>
    </div>

    <!-- Recent Colors Row -->
    <div v-if="studio.state.recentColors.length > 0" class="flex flex-col gap-1.5">
      <div class="text-[10px] font-bold uppercase tracking-wider text-slate-400">Recent Colors</div>
      <div class="flex items-center gap-1 flex-wrap p-1 bg-[#070b10] rounded-lg border border-slate-800/80">
        <button
          v-for="color in studio.state.recentColors"
          :key="color"
          type="button"
          class="w-5 h-5 rounded border border-black/40 hover:scale-110 transition-transform shadow-sm"
          :style="{ backgroundColor: color }"
          :title="color"
          @click="selectColor(color)"
        />
      </div>
    </div>

    <!-- Custom Saved Colors -->
    <div v-if="studio.state.customPalette.length > 0" class="flex flex-col gap-1.5">
      <div class="text-[10px] font-bold uppercase tracking-wider text-slate-400">Custom Colors</div>
      <div class="flex items-center gap-1 flex-wrap p-1 bg-[#070b10] rounded-lg border border-slate-800/80">
        <button
          v-for="(color, idx) in studio.state.customPalette"
          :key="idx"
          type="button"
          class="w-5 h-5 rounded border border-black/40 hover:scale-110 transition-transform shadow-sm"
          :style="{ backgroundColor: color }"
          :title="color"
          @click="selectColor(color)"
        />
      </div>
    </div>
  </div>
</template>

<script setup>
import { computed, ref, watch } from 'vue';
import { PRESET_PALETTES } from './skinStudioState';

const props = defineProps({
  studio: { type: Object, required: true },
});

const hexInput = ref(props.studio.state.activeColor);

watch(
  () => props.studio.state.activeColor,
  (val) => {
    hexInput.value = val;
  }
);

const paletteTabs = {
  skin: 'Skin',
  hair: 'Hair',
  clothes: 'Cloth',
  armor: 'Armor',
  dyes: 'Dyes',
};

const activePaletteColors = computed(() => {
  return PRESET_PALETTES[props.studio.state.selectedPalette] || PRESET_PALETTES.skin;
});

function selectTool(tool) {
  props.studio.state.activeTool = tool;
}

function selectColor(hex) {
  props.studio.state.activeColor = hex;
  props.studio.addColorToRecent(hex);
}

function onNativeColorChange() {
  props.studio.addColorToRecent(props.studio.state.activeColor);
}

function onHexInputChange() {
  let val = hexInput.value.trim();
  if (!val.startsWith('#')) val = '#' + val;
  if (/^#[0-9A-Fa-f]{6}$/.test(val)) {
    props.studio.state.activeColor = val.toLowerCase();
    props.studio.addColorToRecent(val.toLowerCase());
  } else {
    hexInput.value = props.studio.state.activeColor;
  }
}

function addCustomColor() {
  const current = props.studio.state.activeColor;
  if (!props.studio.state.customPalette.includes(current)) {
    props.studio.state.customPalette.push(current);
  }
}
</script>
