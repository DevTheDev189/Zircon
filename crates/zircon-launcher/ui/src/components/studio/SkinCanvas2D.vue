<template>
  <div
    ref="wrapperRef"
    class="relative w-full h-full flex flex-col bg-[#070b10] border border-slate-800/90 rounded-2xl overflow-hidden select-none"
  >
    <!-- Top Canvas Bar: Zoom and Overlay Controls -->
    <div class="h-9 px-3 bg-[#0a0f16] border-b border-slate-800/80 flex items-center justify-between z-10">
      <div class="flex items-center gap-2 min-w-0">
        <span
          class="text-[11px] font-bold uppercase tracking-wider text-slate-400 flex items-center gap-1.5 shrink-0"
          title="2D UV Editor"
        >
          <svg class="w-3.5 h-3.5 text-cyan-400 shrink-0" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
            <rect x="3" y="3" width="18" height="18" rx="2" />
            <path d="M3 9h18" />
            <path d="M9 21V9" />
          </svg>
          <span v-if="containerWidth >= 380">2D UV Editor</span>
          <span v-else-if="containerWidth >= 320">2D UV</span>
        </span>
        <span v-if="containerWidth >= 440" class="text-[10px] font-mono text-slate-500">64x64</span>
      </div>

      <!-- Controls: Grid & Guides Toggles, Zoom, and Fit (Styled identical to 3D side) -->
      <div class="flex items-center gap-1.5 shrink-0">
        <!-- Grid & Guides Toggle Segmented Pill -->
        <div class="flex items-center bg-[#070b10] p-0.5 rounded-lg border border-slate-800">
          <button
            type="button"
            class="px-2 py-0.5 rounded text-[10px] font-semibold transition-all flex items-center gap-1"
            :class="
              studio.state.showGrid
                ? 'bg-cyan-500/25 text-cyan-300 font-bold shadow-sm'
                : 'text-slate-400 hover:text-white'
            "
            title="Toggle Pixel Grid Lines"
            @click="studio.state.showGrid = !studio.state.showGrid"
          >
            <svg class="w-3 h-3 shrink-0" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
              <rect x="3" y="3" width="18" height="18" rx="2" />
              <line x1="3" y1="9" x2="21" y2="9" />
              <line x1="3" y1="15" x2="21" y2="15" />
              <line x1="9" y1="3" x2="9" y2="21" />
              <line x1="15" y1="3" x2="15" y2="21" />
            </svg>
            <span>Grid</span>
          </button>

          <button
            type="button"
            class="px-2 py-0.5 rounded text-[10px] font-semibold transition-all flex items-center gap-1"
            :class="
              studio.state.showGuides
                ? 'bg-cyan-500/25 text-cyan-300 font-bold shadow-sm'
                : 'text-slate-400 hover:text-white'
            "
            title="Toggle UV Body Region Guides"
            @click="studio.state.showGuides = !studio.state.showGuides"
          >
            <svg class="w-3 h-3 shrink-0" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
              <circle cx="12" cy="12" r="10" />
              <line x1="12" y1="8" x2="12" y2="12" />
              <line x1="12" y1="16" x2="12.01" y2="16" />
            </svg>
            <span>Guides</span>
          </button>
        </div>

        <div class="h-4 w-px bg-slate-800 mx-0.5" />

        <!-- Zoom Controls Segmented Pill -->
        <div class="flex items-center bg-[#070b10] p-0.5 rounded-lg border border-slate-800">
          <button
            type="button"
            class="w-5 h-5 rounded text-slate-400 hover:text-white hover:bg-slate-800 flex items-center justify-center font-bold text-xs transition-colors"
            title="Zoom Out (or Mouse Wheel)"
            @click="zoomOut"
          >
            -
          </button>
          <span class="text-[10px] font-mono text-slate-400 px-1 text-center min-w-[24px]">
            {{ Math.round(studio.state.zoom) }}x
          </span>
          <button
            type="button"
            class="w-5 h-5 rounded text-slate-400 hover:text-white hover:bg-slate-800 flex items-center justify-center font-bold text-xs transition-colors"
            title="Zoom In (or Mouse Wheel)"
            @click="zoomIn"
          >
            +
          </button>
        </div>

        <div class="h-3 w-px bg-slate-800 mx-0.5" />

        <!-- Fit View Button -->
        <button
          type="button"
          class="px-2 h-6 rounded border text-[10px] font-bold flex items-center gap-1 transition-all bg-slate-900 border-slate-800 text-slate-400 hover:text-white hover:border-slate-700"
          title="Center Canvas and Fit View"
          @click="resetView(true)"
        >
          Fit
        </button>
      </div>
    </div>

    <!-- Viewport Area (Pan & Zoom Container) -->
    <div
      ref="viewportRef"
      class="relative flex-1 min-h-0 overflow-hidden cursor-crosshair checkerboard-bg"
      :class="{ 'cursor-grab active:cursor-grabbing': isPanning }"
      :style="{
        backgroundSize: `${studio.state.zoom * 2}px ${studio.state.zoom * 2}px`,
        backgroundPosition: `${panX}px ${panY}px`,
      }"
      @wheel.prevent="onWheel"
      @pointerdown="onPointerDown"
      @pointermove="onPointerMove"
      @pointerup="onPointerUp"
      @pointerleave="onPointerLeave"
    >
      <!-- Transformed Content Plane with crisp pixel boundary shadow -->
      <div
        class="absolute will-change-transform origin-top-left shadow-[0_0_0_1px_rgba(56,189,248,0.45),0_12px_36px_rgba(0,0,0,0.85)]"
        :style="{
          transform: `translate(${panX}px, ${panY}px) scale(${studio.state.zoom})`,
          width: '64px',
          height: '64px',
        }"
      >
        <!-- Pixel Canvas -->
        <canvas
          ref="canvasRef"
          width="64"
          height="64"
          class="w-[64px] h-[64px] block image-render-pixel pointer-events-none"
        />

        <!-- Grid Lines Overlay (drawn natively on top of pixel canvas) -->
        <svg
          v-if="studio.state.showGrid && studio.state.zoom >= 5"
          width="64"
          height="64"
          viewBox="0 0 64 64"
          class="absolute inset-0 w-full h-full pointer-events-none opacity-35"
        >
          <defs>
            <pattern id="pixel-grid-pattern" width="1" height="1" patternUnits="userSpaceOnUse">
              <path d="M 1 0 L 0 0 0 1" fill="none" stroke="#ffffff" stroke-width="0.06" />
            </pattern>
          </defs>
          <rect width="64" height="64" fill="url(#pixel-grid-pattern)" />
        </svg>

        <!-- Body Region Guide Overlays -->
        <svg
          v-if="studio.state.showGuides"
          width="64"
          height="64"
          viewBox="0 0 64 64"
          class="absolute inset-0 w-full h-full pointer-events-none"
        >
          <g v-for="r in regions" :key="r.id">
            <rect
              :x="r.bounds[0]"
              :y="r.bounds[1]"
              :width="r.bounds[2]"
              :height="r.bounds[3]"
              fill="none"
              :stroke="r.layer === 'overlay' ? 'rgba(56, 189, 248, 0.75)' : 'rgba(74, 222, 128, 0.75)'"
              stroke-width="0.2"
              :stroke-dasharray="r.layer === 'overlay' ? '0.6, 0.4' : 'none'"
            />
          </g>
        </svg>

        <!-- Hovered Pixel Highlight Box -->
        <div
          v-if="hoverX >= 0 && hoverY >= 0"
          class="absolute w-[1px] h-[1px] pointer-events-none border border-cyan-400 bg-cyan-400/20 shadow-[0_0_2px_rgba(56,189,248,0.8)]"
          :style="{
            left: `${hoverX}px`,
            top: `${hoverY}px`,
          }"
        />
      </div>
    </div>

    <!-- Bottom Status Info Bar -->
    <div class="h-6 px-3 bg-[#0a0f16] border-t border-slate-800/80 flex items-center justify-between text-[10px] font-mono text-slate-400 z-10">
      <div class="flex items-center gap-3">
        <span v-if="hoverX >= 0 && hoverY >= 0">
          X: <strong class="text-cyan-300">{{ hoverX }}</strong> Y: <strong class="text-cyan-300">{{ hoverY }}</strong>
        </span>
        <span v-else class="text-slate-600">Canvas ready</span>

        <span v-if="hoverRegion" class="text-slate-300">
          {{ hoverRegion.label }}
        </span>
      </div>

      <div class="flex items-center gap-3 text-slate-500">
        <span>Middle-click or Space+Drag to pan</span>
      </div>
    </div>
  </div>
</template>

<script setup>
import { computed, nextTick, onMounted, onUnmounted, ref, watch } from 'vue';
import { findFaceAt, getLinePixels, getRegions, isPixelEditable } from './skinStudioState';

const props = defineProps({
  studio: { type: Object, required: true },
});

const wrapperRef = ref(null);
const viewportRef = ref(null);
const canvasRef = ref(null);
const containerWidth = ref(400);

const panX = ref(40);
const panY = ref(40);
const isPanning = ref(false);
let startPanMouseX = 0;
let startPanMouseY = 0;
let initialPanX = 0;
let initialPanY = 0;

const hoverX = ref(-1);
const hoverY = ref(-1);

let isPainting = false;
let lastPaintPixel = null;
let lastShiftPixel = null;

const regions = computed(() => getRegions(props.studio.state.variant));

const hoverRegion = computed(() => {
  if (hoverX.value < 0 || hoverY.value < 0) return null;
  const face = findFaceAt(hoverX.value, hoverY.value, props.studio.state.variant);
  if (!face) return null;
  const layerLabel = face.layer === 'overlay' ? 'Outer' : 'Base';
  return {
    label: `${face.part.toUpperCase()} (${layerLabel}) - ${face.face}`,
  };
});

function renderCanvas() {
  if (!canvasRef.value) return;
  const ctx = canvasRef.value.getContext('2d');
  const imgData = ctx.createImageData(64, 64);
  imgData.data.set(props.studio.pixelBuffer);
  ctx.putImageData(imgData, 0, 0);
}

// Watch version increments for instant re-draw
watch(
  () => props.studio.state.version,
  () => {
    renderCanvas();
  }
);

// Zoom and Pan mechanics with Pixel-Snapping
const ZOOM_LEVELS = [2, 3, 4, 5, 6, 7, 8, 10, 12, 14, 16, 20, 24, 28, 32];

function setZoomAndSnap(newZoom, clientX = null, clientY = null) {
  const oldZoom = Math.max(1, Math.round(props.studio.state.zoom));
  if (newZoom === oldZoom) return;

  if (viewportRef.value && clientX !== null && clientY !== null) {
    const rect = viewportRef.value.getBoundingClientRect();
    const mouseX = clientX - rect.left;
    const mouseY = clientY - rect.top;

    const rawPanX = mouseX - ((mouseX - panX.value) * newZoom) / oldZoom;
    const rawPanY = mouseY - ((mouseY - panY.value) * newZoom) / oldZoom;

    panX.value = Math.round(rawPanX / newZoom) * newZoom;
    panY.value = Math.round(rawPanY / newZoom) * newZoom;
  } else if (viewportRef.value) {
    const w = viewportRef.value.clientWidth || 300;
    const h = viewportRef.value.clientHeight || 300;
    const rawX = (w - 64 * newZoom) / 2;
    const rawY = (h - 64 * newZoom) / 2;
    panX.value = Math.round(rawX / newZoom) * newZoom;
    panY.value = Math.round(rawY / newZoom) * newZoom;
  }

  props.studio.state.zoom = newZoom;
}

function onWheel(e) {
  const cur = Math.round(props.studio.state.zoom);
  let newZoom;
  if (e.deltaY < 0) {
    newZoom = ZOOM_LEVELS.find((z) => z > cur) || 32;
  } else {
    newZoom = [...ZOOM_LEVELS].reverse().find((z) => z < cur) || 2;
  }
  setZoomAndSnap(newZoom, e.clientX, e.clientY);
}

function zoomIn() {
  const cur = Math.round(props.studio.state.zoom);
  const next = ZOOM_LEVELS.find((z) => z > cur) || 32;
  setZoomAndSnap(next);
}

function zoomOut() {
  const cur = Math.round(props.studio.state.zoom);
  const prev = [...ZOOM_LEVELS].reverse().find((z) => z < cur) || 2;
  setZoomAndSnap(prev);
}

function resetView(forceFit = true) {
  if (!viewportRef.value) return;
  const w = viewportRef.value.clientWidth || 300;
  const h = viewportRef.value.clientHeight || 300;
  if (forceFit) {
    const fitZoom = Math.floor(Math.min((w - 24) / 64, (h - 24) / 64));
    props.studio.state.zoom = Math.max(3, Math.min(24, fitZoom));
  }
  const step = Math.max(1, Math.round(props.studio.state.zoom));
  const rawX = (w - 64 * step) / 2;
  const rawY = (h - 64 * step) / 2;
  panX.value = Math.round(rawX / step) * step;
  panY.value = Math.round(rawY / step) * step;
}

// Coordinate conversions
function getCanvasPixelFromEvent(e) {
  if (!viewportRef.value) return { x: -1, y: -1 };
  const rect = viewportRef.value.getBoundingClientRect();
  const mouseX = e.clientX - rect.left;
  const mouseY = e.clientY - rect.top;

  const canvasX = Math.floor((mouseX - panX.value) / props.studio.state.zoom);
  const canvasY = Math.floor((mouseY - panY.value) / props.studio.state.zoom);

  if (canvasX >= 0 && canvasX < 64 && canvasY >= 0 && canvasY < 64) {
    return { x: canvasX, y: canvasY };
  }
  return { x: -1, y: -1 };
}

// Pointer Events for Painting & Panning
function onPointerDown(e) {
  // Middle click or Space key held down triggers pan
  if (e.button === 1 || e.button === 2 || e.spaceKey) {
    isPanning.value = true;
    startPanMouseX = e.clientX;
    startPanMouseY = e.clientY;
    initialPanX = panX.value;
    initialPanY = panY.value;
    return;
  }

  if (e.button !== 0) return; // Left click only for painting

  const { x, y } = getCanvasPixelFromEvent(e);
  if (x < 0 || y < 0) return;

  if (props.studio.state.activeTool === 'bucket') {
    props.studio.floodFill(x, y);
    renderCanvas();
    return;
  }

  if (props.studio.state.activeTool === 'picker') {
    props.studio.applyToolToPixel(x, y, 'picker');
    return;
  }

  // Push undo state before starting a brush stroke
  props.studio.pushHistory();
  isPainting = true;

  if (e.shiftKey && lastShiftPixel) {
    // Draw straight line from previous point
    const points = getLinePixels(lastShiftPixel.x, lastShiftPixel.y, x, y);
    for (const pt of points) {
      props.studio.applyToolToPixel(pt.x, pt.y);
    }
  } else {
    props.studio.applyToolToPixel(x, y);
  }

  lastPaintPixel = { x, y };
  lastShiftPixel = { x, y };
  renderCanvas();
  props.studio.state.version++;
}

function onPointerMove(e) {
  if (isPanning.value) {
    const dx = e.clientX - startPanMouseX;
    const dy = e.clientY - startPanMouseY;
    const step = Math.max(1, Math.round(props.studio.state.zoom));
    // Snap dragging delta strictly to whole Minecraft skin pixels
    const snappedDx = Math.round(dx / step) * step;
    const snappedDy = Math.round(dy / step) * step;
    panX.value = initialPanX + snappedDx;
    panY.value = initialPanY + snappedDy;
    return;
  }

  const { x, y } = getCanvasPixelFromEvent(e);
  hoverX.value = x;
  hoverY.value = y;

  if (!isPainting || x < 0 || y < 0) return;

  // Bresenham line smoothing between pointermove events
  if (lastPaintPixel && (lastPaintPixel.x !== x || lastPaintPixel.y !== y)) {
    const points = getLinePixels(lastPaintPixel.x, lastPaintPixel.y, x, y);
    for (const pt of points) {
      props.studio.applyToolToPixel(pt.x, pt.y);
    }
    lastPaintPixel = { x, y };
    lastShiftPixel = { x, y };
    renderCanvas();
    props.studio.state.version++;
  }
}

function onPointerUp() {
  isPanning.value = false;
  if (isPainting) {
    isPainting = false;
    lastPaintPixel = null;
    renderCanvas();
    props.studio.state.version++;
  }
}

function onPointerLeave() {
  hoverX.value = -1;
  hoverY.value = -1;
  if (isPainting) {
    isPainting = false;
    lastPaintPixel = null;
  }
  isPanning.value = false;
}

let resizeObserver = null;

onMounted(() => {
  nextTick(() => {
    containerWidth.value = wrapperRef.value?.clientWidth || viewportRef.value?.clientWidth || 400;
    resetView();
    renderCanvas();
  });

  if (window.ResizeObserver && (wrapperRef.value || viewportRef.value)) {
    const target = wrapperRef.value || viewportRef.value;
    resizeObserver = new ResizeObserver((entries) => {
      if (entries && entries[0] && entries[0].contentRect) {
        containerWidth.value = entries[0].contentRect.width;
      } else if (target) {
        containerWidth.value = target.clientWidth;
      }
      resetView(false);
    });
    resizeObserver.observe(target);
  }

  window.addEventListener('pointerup', onPointerUp);
});

onUnmounted(() => {
  if (resizeObserver) {
    resizeObserver.disconnect();
    resizeObserver = null;
  }
  window.removeEventListener('pointerup', onPointerUp);
});
</script>

<style scoped>
.checkerboard-bg {
  background-color: #080d14;
  background-image: conic-gradient(
    #0e1622 90deg,
    #080d14 90deg 180deg,
    #0e1622 180deg 270deg,
    #080d14 270deg
  );
  background-repeat: repeat;
}

.image-render-pixel {
  image-rendering: pixelated;
  image-rendering: -moz-crisp-edges;
  image-rendering: crisp-edges;
}
</style>
