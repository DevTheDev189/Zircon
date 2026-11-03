<template>
  <div
    class="zircon-background-container pointer-events-none absolute inset-0 overflow-hidden z-0 select-none"
    :class="{ 'reduce-motion': isPaused }"
  >
    <!-- MODE 1: CUSTOM WALLPAPER (IMAGE OR LOOPING VIDEO) -->
    <template v-if="motionPreset === 'custom-wallpaper' && wallpaperSrc">
      <!-- Looping Video Wallpaper -->
      <video
        v-if="wallpaperType === 'video'"
        ref="videoRef"
        :src="wallpaperSrc"
        autoplay
        loop
        muted
        playsinline
        class="absolute inset-0 w-full h-full object-cover z-0"
      ></video>

      <!-- Static or Slow-Drift Image Wallpaper -->
      <div
        v-else
        class="absolute inset-0 w-full h-full bg-cover bg-center z-0 z-wallpaper-kenburns"
        :style="{ backgroundImage: `url('${wallpaperSrc}')` }"
      ></div>

      <!-- Dimmer Overlay to guarantee readability across all views -->
      <div
        class="absolute inset-0 bg-black z-[1] transition-opacity duration-300"
        :style="{ opacity: wallpaperDim }"
      ></div>
    </template>

    <!-- MODE 2: VOXEL DOT WAVE (REAL 3D UNDULATING MATRIX WAVE) -->
    <div
      v-else-if="motionPreset === 'voxel-wave'"
      class="absolute inset-0 z-0 overflow-hidden"
    >
      <div class="z-ambient-glow-top opacity-15"></div>
      <canvas
        ref="waveCanvasRef"
        class="absolute inset-0 w-full h-full pointer-events-none z-0"
      ></canvas>
      <!-- Radial Vignette to keep center cards high contrast -->
      <div class="absolute inset-0 bg-radial-vignette pointer-events-none z-[1]"></div>
    </div>

    <!-- MODE 3: STATIC MINIMAL (NO MOTION / ECO) -->
    <div
      v-else-if="motionPreset === 'static-minimal'"
      class="absolute inset-0 z-0 bg-gradient-to-br from-card/30 via-bg to-well"
    ></div>

    <!-- MODE 4 (DEFAULT): CYBER AURORA GLOW ORBS -->
    <div
      v-else
      class="absolute inset-0 z-0 overflow-hidden"
    >
      <!-- Top Primary Beam -->
      <div class="z-ambient-glow-top"></div>
      <!-- Top-Left Corner Aura -->
      <div class="z-ambient-glow-corner"></div>
      <!-- Bottom-Right Floating Accent Orb -->
      <div class="z-ambient-glow-satellite"></div>
    </div>
  </div>
</template>

<script setup>
import { ref, onMounted, onUnmounted, watch, nextTick } from 'vue';
import { getSavedWallpaperConfig } from '../lib/theme';

const props = defineProps({
  gameRunning: {
    type: Boolean,
    default: false,
  },
});

const videoRef = ref(null);
const waveCanvasRef = ref(null);
const motionPreset = ref('cyber-orbs');
const wallpaperDim = ref(0.65);
const wallpaperSrc = ref(null);
const wallpaperType = ref('image');
const isPaused = ref(false);

let animFrameId = null;
let resizeObserver = null;

function syncConfig() {
  const cfg = getSavedWallpaperConfig();
  motionPreset.value = cfg.motionPreset;
  wallpaperDim.value = cfg.wallpaperDim;
  wallpaperSrc.value = cfg.wallpaperSrc;
  wallpaperType.value = cfg.wallpaperType;
  if (motionPreset.value === 'voxel-wave') {
    nextTick(() => startWaveAnimation());
  } else {
    stopWaveAnimation();
  }
}

function handleThemeChanged(e) {
  if (e?.detail) {
    motionPreset.value = e.detail.motionPreset || 'cyber-orbs';
    wallpaperDim.value = e.detail.wallpaperDim !== undefined ? e.detail.wallpaperDim : 0.65;
    wallpaperSrc.value = e.detail.wallpaperSrc || null;
    wallpaperType.value = e.detail.wallpaperType || 'image';
  } else {
    syncConfig();
  }
  if (motionPreset.value === 'voxel-wave') {
    nextTick(() => startWaveAnimation());
  } else {
    stopWaveAnimation();
  }
}

function handleVisibilityChange() {
  const hidden = document.hidden;
  isPaused.value = hidden || props.gameRunning;
  if (videoRef.value) {
    if (isPaused.value) {
      videoRef.value.pause();
    } else {
      videoRef.value.play().catch(() => {});
    }
  }
  if (isPaused.value) {
    stopWaveAnimation();
  } else if (motionPreset.value === 'voxel-wave') {
    startWaveAnimation();
  }
}

watch(
  () => props.gameRunning,
  (running) => {
    isPaused.value = running || document.hidden;
    if (videoRef.value) {
      if (isPaused.value) {
        videoRef.value.pause();
      } else {
        videoRef.value.play().catch(() => {});
      }
    }
    if (isPaused.value) {
      stopWaveAnimation();
    } else if (motionPreset.value === 'voxel-wave') {
      startWaveAnimation();
    }
  }
);

// =========================================================================
// Real 3D Undulating Voxel Dot Wave Engine (Ultra High Performance Canvas)
// =========================================================================
function stopWaveAnimation() {
  if (animFrameId) {
    cancelAnimationFrame(animFrameId);
    animFrameId = null;
  }
}

function startWaveAnimation() {
  stopWaveAnimation();
  if (isPaused.value || motionPreset.value !== 'voxel-wave') return;

  const canvas = waveCanvasRef.value;
  if (!canvas) return;
  const ctx = canvas.getContext('2d', { alpha: true });
  if (!ctx) return;

  function updateSize() {
    if (!canvas) return;
    const rect = canvas.getBoundingClientRect();
    const dpr = Math.min(window.devicePixelRatio || 1, 2);
    canvas.width = Math.floor(rect.width * dpr);
    canvas.height = Math.floor(rect.height * dpr);
  }
  updateSize();

  const cols = 42;
  const rows = 28;
  const spacingX = 46;
  const spacingZ = 36;
  const fov = 380;
  const cameraY = 135;
  const cameraZ = -120;

  function render(timestamp) {
    if (isPaused.value || motionPreset.value !== 'voxel-wave' || !canvas) {
      animFrameId = null;
      return;
    }

    const width = canvas.width;
    const height = canvas.height;
    ctx.clearRect(0, 0, width, height);

    // Get live accent color from CSS variables
    const accentRgb = getComputedStyle(document.documentElement)
      .getPropertyValue('--color-accent-rgb')
      .trim() || '71 210 201';
    const rgbParts = accentRgb.split(' ').join(',');

    const time = timestamp * 0.0016;

    for (let iz = 0; iz < rows; iz++) {
      const z3d = iz * spacingZ;
      const zDist = z3d - cameraZ;
      if (zDist <= 1) continue;

      const scale = fov / zDist;
      const depthRatio = 1 - iz / rows; // 1 at front, 0 at back

      for (let ix = 0; ix < cols; ix++) {
        const x3d = (ix - cols / 2) * spacingX;

        // Dual sinusoidal wave equation
        const y3d =
          Math.sin(ix * 0.22 + time * 1.1) * 26 +
          Math.cos(iz * 0.28 + time * 0.9) * 22;

        const screenX = width / 2 + x3d * scale;
        const screenY = height * 0.65 + (y3d - cameraY) * scale;

        if (screenX < -20 || screenX > width + 20 || screenY < -20 || screenY > height + 20) {
          continue;
        }

        // Voxel dot size and alpha with depth gradient
        const size = Math.max(1, scale * 3.4);
        const alpha = Math.min(0.85, Math.max(0.06, depthRatio * 0.7));

        ctx.fillStyle = `rgba(${rgbParts},${alpha})`;
        // Draw square voxel dot (sharp cyber look + 5x faster than arc)
        ctx.fillRect(screenX - size * 0.5, screenY - size * 0.5, size, size);
      }
    }

    animFrameId = requestAnimationFrame(render);
  }

  animFrameId = requestAnimationFrame(render);
}

onMounted(() => {
  syncConfig();
  window.addEventListener('zircon-theme-changed', handleThemeChanged);
  document.addEventListener('visibilitychange', handleVisibilityChange);
  window.addEventListener('resize', () => {
    if (motionPreset.value === 'voxel-wave') {
      startWaveAnimation();
    }
  });
});

onUnmounted(() => {
  stopWaveAnimation();
  window.removeEventListener('zircon-theme-changed', handleThemeChanged);
  document.removeEventListener('visibilitychange', handleVisibilityChange);
  if (resizeObserver) resizeObserver.disconnect();
});
</script>

<style scoped>
/* GPU Ken Burns Sub-Pixel Ambient Wallpaper Drift */
@keyframes zircon-ken-burns {
  0% {
    transform: translate3d(-1%, -1%, 0) scale(1.02);
  }
  50% {
    transform: translate3d(1.5%, 1%, 0) scale(1.06);
  }
  100% {
    transform: translate3d(-1%, -1%, 0) scale(1.02);
  }
}

.z-wallpaper-kenburns {
  animation: zircon-ken-burns 36s ease-in-out infinite alternate;
  will-change: transform;
}

.bg-radial-vignette {
  background: radial-gradient(circle at 50% 40%, transparent 25%, rgba(var(--color-bg-rgb), 0.94) 85%);
}

/* Satellite Ambient Glow Orb */
@keyframes zircon-ambient-satellite {
  0% {
    transform: translate3d(0, 0, 0) scale(1);
    opacity: 0.12;
  }
  50% {
    transform: translate3d(-30px, -20px, 0) scale(1.15);
    opacity: 0.22;
  }
  100% {
    transform: translate3d(0, 0, 0) scale(1);
    opacity: 0.12;
  }
}

.z-ambient-glow-satellite {
  position: absolute;
  bottom: -14rem;
  right: -10rem;
  width: 38rem;
  height: 38rem;
  border-radius: 9999px;
  background-color: var(--color-accent);
  filter: blur(85px);
  pointer-events: none;
  z-index: 0;
  animation: zircon-ambient-satellite 18s ease-in-out infinite;
  will-change: transform, opacity;
}

/* Pause animations when window is unfocused or game is running */
.reduce-motion * {
  animation-play-state: paused !important;
}
</style>