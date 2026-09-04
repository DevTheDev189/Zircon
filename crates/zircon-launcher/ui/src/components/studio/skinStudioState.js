// Shared state and utilities for Zircon Skin Paint Studio
import { reactive, ref } from 'vue';
import { ZIRCON_STEVE_DATA_URL } from '../../lib/api.js';

// --- Default Templates ---
// Minimal Alex base skin (64x64)
export const ALEX_BASE_DATA_URL =
  'data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAEAAAABACAYAAACqaXHeAAAATUlEQVR42u3BAQEAMAyAMLLQv+i59AYAAAAAAAC21P8AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAwH0B9/wAAeh+7eAAAAAASUVORK5CYII=';

// --- Region and UV definitions ---
export const BODY_PARTS = [
  { id: 'head', label: 'Head' },
  { id: 'body', label: 'Torso' },
  { id: 'rightArm', label: 'Right Arm' },
  { id: 'leftArm', label: 'Left Arm' },
  { id: 'rightLeg', label: 'Right Leg' },
  { id: 'leftLeg', label: 'Left Leg' },
];

export function getRegions(variant = 'classic') {
  const isSlim = variant === 'slim';
  const armW = isSlim ? 3 : 4;

  return [
    // Base Layer
    { id: 'head_base', part: 'head', layer: 'base', label: 'Head (Base)', bounds: [0, 0, 32, 16], color: 'rgba(56, 189, 248, 0.15)' },
    { id: 'hat', part: 'head', layer: 'overlay', label: 'Hat (Outer)', bounds: [32, 0, 32, 16], color: 'rgba(56, 189, 248, 0.25)' },

    { id: 'body_base', part: 'body', layer: 'base', label: 'Torso (Base)', bounds: [16, 16, 24, 16], color: 'rgba(74, 222, 128, 0.15)' },
    { id: 'jacket', part: 'body', layer: 'overlay', label: 'Jacket (Outer)', bounds: [16, 32, 24, 16], color: 'rgba(74, 222, 128, 0.25)' },

    { id: 'r_arm_base', part: 'rightArm', layer: 'base', label: 'R. Arm (Base)', bounds: [40, 16, armW * 2 + 8, 16], color: 'rgba(251, 146, 60, 0.15)' },
    { id: 'r_sleeve', part: 'rightArm', layer: 'overlay', label: 'R. Sleeve (Outer)', bounds: [40, 32, armW * 2 + 8, 16], color: 'rgba(251, 146, 60, 0.25)' },

    { id: 'r_leg_base', part: 'rightLeg', layer: 'base', label: 'R. Leg (Base)', bounds: [0, 16, 16, 16], color: 'rgba(168, 85, 247, 0.15)' },
    { id: 'r_pants', part: 'rightLeg', layer: 'overlay', label: 'R. Pants (Outer)', bounds: [0, 32, 16, 16], color: 'rgba(168, 85, 247, 0.25)' },

    { id: 'l_leg_base', part: 'leftLeg', layer: 'base', label: 'L. Leg (Base)', bounds: [16, 48, 16, 16], color: 'rgba(236, 72, 153, 0.15)' },
    { id: 'l_pants', part: 'leftLeg', layer: 'overlay', label: 'L. Pants (Outer)', bounds: [0, 48, 16, 16], color: 'rgba(236, 72, 153, 0.25)' },

    { id: 'l_arm_base', part: 'leftArm', layer: 'base', label: 'L. Arm (Base)', bounds: [32, 48, armW * 2 + 8, 16], color: 'rgba(250, 204, 21, 0.15)' },
    { id: 'l_sleeve', part: 'leftArm', layer: 'overlay', label: 'L. Sleeve (Outer)', bounds: [48, 48, armW * 2 + 8, 16], color: 'rgba(250, 204, 21, 0.25)' },
  ];
}

export function getFaceBoxes(variant = 'classic') {
  const isSlim = variant === 'slim';
  const armW = isSlim ? 3 : 4;
  return [
    // Head Base
    { part: 'head', layer: 'base', face: 'top', rect: [8, 0, 8, 8] },
    { part: 'head', layer: 'base', face: 'bottom', rect: [16, 0, 8, 8] },
    { part: 'head', layer: 'base', face: 'right', rect: [0, 8, 8, 8] },
    { part: 'head', layer: 'base', face: 'front', rect: [8, 8, 8, 8] },
    { part: 'head', layer: 'base', face: 'left', rect: [16, 8, 8, 8] },
    { part: 'head', layer: 'base', face: 'back', rect: [24, 8, 8, 8] },

    // Hat (Overlay)
    { part: 'head', layer: 'overlay', face: 'top', rect: [40, 0, 8, 8] },
    { part: 'head', layer: 'overlay', face: 'bottom', rect: [48, 0, 8, 8] },
    { part: 'head', layer: 'overlay', face: 'right', rect: [32, 8, 8, 8] },
    { part: 'head', layer: 'overlay', face: 'front', rect: [40, 8, 8, 8] },
    { part: 'head', layer: 'overlay', face: 'left', rect: [48, 8, 8, 8] },
    { part: 'head', layer: 'overlay', face: 'back', rect: [56, 8, 8, 8] },

    // Torso Base
    { part: 'body', layer: 'base', face: 'top', rect: [20, 16, 8, 4] },
    { part: 'body', layer: 'base', face: 'bottom', rect: [28, 16, 8, 4] },
    { part: 'body', layer: 'base', face: 'right', rect: [16, 20, 4, 12] },
    { part: 'body', layer: 'base', face: 'front', rect: [20, 20, 8, 12] },
    { part: 'body', layer: 'base', face: 'left', rect: [28, 20, 4, 12] },
    { part: 'body', layer: 'base', face: 'back', rect: [32, 20, 8, 12] },

    // Jacket (Overlay)
    { part: 'body', layer: 'overlay', face: 'top', rect: [20, 32, 8, 4] },
    { part: 'body', layer: 'overlay', face: 'bottom', rect: [28, 32, 8, 4] },
    { part: 'body', layer: 'overlay', face: 'right', rect: [16, 36, 4, 12] },
    { part: 'body', layer: 'overlay', face: 'front', rect: [20, 36, 8, 12] },
    { part: 'body', layer: 'overlay', face: 'left', rect: [28, 36, 4, 12] },
    { part: 'body', layer: 'overlay', face: 'back', rect: [32, 36, 8, 12] },

    // Right Arm Base
    { part: 'rightArm', layer: 'base', face: 'top', rect: [44, 16, armW, 4] },
    { part: 'rightArm', layer: 'base', face: 'bottom', rect: [44 + armW, 16, armW, 4] },
    { part: 'rightArm', layer: 'base', face: 'right', rect: [40, 20, 4, 12] },
    { part: 'rightArm', layer: 'base', face: 'front', rect: [44, 20, armW, 12] },
    { part: 'rightArm', layer: 'base', face: 'left', rect: [44 + armW, 20, 4, 12] },
    { part: 'rightArm', layer: 'base', face: 'back', rect: [48 + armW, 20, armW, 12] },

    // Right Sleeve (Overlay)
    { part: 'rightArm', layer: 'overlay', face: 'top', rect: [44, 32, armW, 4] },
    { part: 'rightArm', layer: 'overlay', face: 'bottom', rect: [44 + armW, 32, armW, 4] },
    { part: 'rightArm', layer: 'overlay', face: 'right', rect: [40, 36, 4, 12] },
    { part: 'rightArm', layer: 'overlay', face: 'front', rect: [44, 36, armW, 12] },
    { part: 'rightArm', layer: 'overlay', face: 'left', rect: [44 + armW, 36, 4, 12] },
    { part: 'rightArm', layer: 'overlay', face: 'back', rect: [48 + armW, 36, armW, 12] },

    // Left Arm Base
    { part: 'leftArm', layer: 'base', face: 'top', rect: [36, 48, armW, 4] },
    { part: 'leftArm', layer: 'base', face: 'bottom', rect: [36 + armW, 48, armW, 4] },
    { part: 'leftArm', layer: 'base', face: 'right', rect: [32, 52, 4, 12] },
    { part: 'leftArm', layer: 'base', face: 'front', rect: [36, 52, armW, 12] },
    { part: 'leftArm', layer: 'base', face: 'left', rect: [36 + armW, 52, 4, 12] },
    { part: 'leftArm', layer: 'base', face: 'back', rect: [40 + armW, 52, armW, 12] },

    // Left Sleeve (Overlay)
    { part: 'leftArm', layer: 'overlay', face: 'top', rect: [52, 48, armW, 4] },
    { part: 'leftArm', layer: 'overlay', face: 'bottom', rect: [52 + armW, 48, armW, 4] },
    { part: 'leftArm', layer: 'overlay', face: 'right', rect: [48, 52, 4, 12] },
    { part: 'leftArm', layer: 'overlay', face: 'front', rect: [52, 52, armW, 12] },
    { part: 'leftArm', layer: 'overlay', face: 'left', rect: [52 + armW, 52, 4, 12] },
    { part: 'leftArm', layer: 'overlay', face: 'back', rect: [56 + armW, 52, armW, 12] },

    // Right Leg Base
    { part: 'rightLeg', layer: 'base', face: 'top', rect: [4, 16, 4, 4] },
    { part: 'rightLeg', layer: 'base', face: 'bottom', rect: [8, 16, 4, 4] },
    { part: 'rightLeg', layer: 'base', face: 'right', rect: [0, 20, 4, 12] },
    { part: 'rightLeg', layer: 'base', face: 'front', rect: [4, 20, 4, 12] },
    { part: 'rightLeg', layer: 'base', face: 'left', rect: [8, 20, 4, 12] },
    { part: 'rightLeg', layer: 'base', face: 'back', rect: [12, 20, 4, 12] },

    // Right Pants (Overlay)
    { part: 'rightLeg', layer: 'overlay', face: 'top', rect: [4, 32, 4, 4] },
    { part: 'rightLeg', layer: 'overlay', face: 'bottom', rect: [8, 32, 4, 4] },
    { part: 'rightLeg', layer: 'overlay', face: 'right', rect: [0, 36, 4, 12] },
    { part: 'rightLeg', layer: 'overlay', face: 'front', rect: [4, 36, 4, 12] },
    { part: 'rightLeg', layer: 'overlay', face: 'left', rect: [8, 36, 4, 12] },
    { part: 'rightLeg', layer: 'overlay', face: 'back', rect: [12, 36, 4, 12] },

    // Left Leg Base
    { part: 'leftLeg', layer: 'base', face: 'top', rect: [20, 48, 4, 4] },
    { part: 'leftLeg', layer: 'base', face: 'bottom', rect: [24, 48, 4, 4] },
    { part: 'leftLeg', layer: 'base', face: 'right', rect: [16, 52, 4, 12] },
    { part: 'leftLeg', layer: 'base', face: 'front', rect: [20, 52, 4, 12] },
    { part: 'leftLeg', layer: 'base', face: 'left', rect: [24, 52, 4, 12] },
    { part: 'leftLeg', layer: 'base', face: 'back', rect: [28, 52, 4, 12] },

    // Left Pants (Overlay)
    { part: 'leftLeg', layer: 'overlay', face: 'top', rect: [4, 48, 4, 4] },
    { part: 'leftLeg', layer: 'overlay', face: 'bottom', rect: [8, 48, 4, 4] },
    { part: 'leftLeg', layer: 'overlay', face: 'right', rect: [0, 52, 4, 12] },
    { part: 'leftLeg', layer: 'overlay', face: 'front', rect: [4, 52, 4, 12] },
    { part: 'leftLeg', layer: 'overlay', face: 'left', rect: [8, 52, 4, 12] },
    { part: 'leftLeg', layer: 'overlay', face: 'back', rect: [12, 52, 4, 12] },
  ];
}

// Find which face bounding box contains pixel (x, y)
export function findFaceAt(x, y, variant = 'classic') {
  const boxes = getFaceBoxes(variant);
  for (const b of boxes) {
    const [bx, by, bw, bh] = b.rect;
    if (x >= bx && x < bx + bw && y >= by && y < by + bh) {
      return b;
    }
  }
  return null;
}

// Check if a pixel belongs to a currently visible part and layer
export function isPixelEditable(x, y, studio) {
  const face = findFaceAt(x, y, studio.variant);
  if (!face) return true; // generic area

  // Check layer visibility
  if (face.layer === 'base' && !studio.layers.base) return false;
  if (face.layer === 'overlay' && !studio.layers.overlay) return false;

  // Check active layer constraint
  if (studio.activeLayer === 'base' && face.layer !== 'base') return false;
  if (studio.activeLayer === 'overlay' && face.layer !== 'overlay') return false;

  // Check body part visibility
  if (!studio.visibleParts[face.part]) return false;

  return true;
}

// --- Curated Color Palettes ---
export const PRESET_PALETTES = {
  skin: [
    '#fcd3b6', '#fbc5a0', '#eab28c', '#d59b73', '#b87d55', '#9a613c',
    '#7d4b2b', '#5c3319', '#3f210d', '#2b1507', '#fff1e6', '#ecd8c9',
  ],
  hair: [
    '#1c1917', '#292524', '#44403c', '#57534e', '#78716c', '#a8a29e',
    '#78350f', '#92400e', '#b45309', '#d97706', '#f59e0b', '#fbbf24',
  ],
  clothes: [
    '#1e293b', '#334155', '#475569', '#64748b', '#0f766e', '#0d9488',
    '#14b8a6', '#2dd4bf', '#1e3a8a', '#1d4ed8', '#3b82f6', '#60a5fa',
  ],
  armor: [
    '#f1f5f9', '#cbd5e1', '#94a3b8', '#64748b', '#e2e8f0', '#b91c1c',
    '#ef4444', '#f87171', '#047857', '#10b981', '#34d399', '#6ee7b7',
  ],
  dyes: [
    '#000000', '#ffffff', '#b91c1c', '#ea580c', '#eab308', '#16a34a',
    '#0284c7', '#7c3aed', '#db2777', '#4b5563', '#9ca3af', '#059669',
  ],
};

// --- Color Space Utilities ---
export function hexToRgb(hex) {
  let c = hex.replace('#', '');
  if (c.length === 3) {
    c = c[0] + c[0] + c[1] + c[1] + c[2] + c[2];
  }
  const num = parseInt(c, 16);
  return {
    r: (num >> 16) & 255,
    g: (num >> 8) & 255,
    b: num & 255,
  };
}

export function rgbToHex(r, g, b) {
  const h = (1 << 24) + (r << 16) + (g << 8) + b;
  return '#' + h.toString(16).slice(1).toLowerCase();
}

export function rgbToHsl(r, g, b) {
  r /= 255;
  g /= 255;
  b /= 255;
  const max = Math.max(r, g, b);
  const min = Math.min(r, g, b);
  let h = 0, s = 0;
  const l = (max + min) / 2;

  if (max !== min) {
    const d = max - min;
    s = l > 0.5 ? d / (2 - max - min) : d / (max + min);
    switch (max) {
      case r: h = (g - b) / d + (g < b ? 6 : 0); break;
      case g: h = (b - r) / d + 2; break;
      case b: h = (r - g) / d + 4; break;
    }
    h /= 6;
  }
  return { h, s, l };
}

export function hslToRgb(h, s, l) {
  let r, g, b;
  if (s === 0) {
    r = g = b = l;
  } else {
    const hue2rgb = (p, q, t) => {
      if (t < 0) t += 1;
      if (t > 1) t -= 1;
      if (t < 1 / 6) return p + (q - p) * 6 * t;
      if (t < 1 / 2) return q;
      if (t < 2 / 3) return p + (q - p) * (2 / 3 - t) * 6;
      return p;
    };
    const q = l < 0.5 ? l * (1 + s) : l + s - l * s;
    const p = 2 * l - q;
    r = hue2rgb(p, q, h + 1 / 3);
    g = hue2rgb(p, q, h);
    b = hue2rgb(p, q, h - 1 / 3);
  }
  return {
    r: Math.round(r * 255),
    g: Math.round(g * 255),
    b: Math.round(b * 255),
  };
}

// Adjust pixel luminance by delta (-1 to 1)
export function adjustLuminance(r, g, b, delta) {
  const { h, s, l } = rgbToHsl(r, g, b);
  const newL = Math.max(0, Math.min(1, l + delta));
  return hslToRgb(h, s, newL);
}

// Random noise jitter for shading brush (+/- 6% to 10%)
export function getNoiseColor(r, g, b, amount = 0.08) {
  const delta = (Math.random() * 2 - 1) * amount;
  return adjustLuminance(r, g, b, delta);
}

// --- Bresenham's line algorithm ---
export function getLinePixels(x0, y0, x1, y1) {
  const points = [];
  const dx = Math.abs(x1 - x0);
  const dy = Math.abs(y1 - y0);
  const sx = x0 < x1 ? 1 : -1;
  const sy = y0 < y1 ? 1 : -1;
  let err = dx - dy;

  let cx = x0;
  let cy = y0;

  while (true) {
    points.push({ x: cx, y: cy });
    if (cx === x1 && cy === y1) break;
    const e2 = 2 * err;
    if (e2 > -dy) {
      err -= dy;
      cx += sx;
    }
    if (e2 < dx) {
      err += dx;
      cy += sy;
    }
  }
  return points;
}

// --- Central Reactive Studio State Factory ---
export function createStudioState() {
  const state = reactive({
    skinName: 'custom_skin.png',
    variant: 'classic', // 'classic' | 'slim'
    activeTool: 'pencil', // 'pencil' | 'eraser' | 'picker' | 'bucket' | 'noise' | 'lighten' | 'darken'
    activeColor: '#38bdf8',
    activeAlpha: 255, // 0-255
    selectedPalette: 'skin',
    recentColors: ['#38bdf8', '#fcd3b6', '#334155', '#16a34a', '#eab308', '#ef4444', '#ffffff', '#000000'],
    customPalette: [],

    // Layers & Visibility
    layers: {
      base: true,
      overlay: true,
    },
    activeLayer: 'all', // 'all' | 'base' | 'overlay'
    visibleParts: {
      head: true,
      body: true,
      rightArm: true,
      leftArm: true,
      rightLeg: true,
      leftLeg: true,
    },

    // View options
    showGrid: true,
    showGuides: true,
    zoom: 10, // 2D zoom level
    viewMode3D: 'paint', // 'paint' | 'orbit'

    // History and synchronization version
    version: 0,
    canUndo: false,
    canRedo: false,
    isDirty: false,
  });

  // 64x64 RGBA pixel buffer (16,384 bytes)
  const pixelBuffer = new Uint8ClampedArray(64 * 64 * 4);

  // Undo / Redo stacks of Uint8ClampedArray copies
  const MAX_HISTORY = 50;
  let undoStack = [];
  let redoStack = [];

  function updateHistoryState() {
    state.canUndo = undoStack.length > 0;
    state.canRedo = redoStack.length > 0;
  }

  function pushHistory() {
    undoStack.push(new Uint8ClampedArray(pixelBuffer));
    if (undoStack.length > MAX_HISTORY) {
      undoStack.shift();
    }
    redoStack = [];
    state.isDirty = true;
    updateHistoryState();
  }

  function undo() {
    if (undoStack.length === 0) return;
    redoStack.push(new Uint8ClampedArray(pixelBuffer));
    const prev = undoStack.pop();
    pixelBuffer.set(prev);
    state.version++;
    updateHistoryState();
  }

  function redo() {
    if (redoStack.length === 0) return;
    undoStack.push(new Uint8ClampedArray(pixelBuffer));
    const next = redoStack.pop();
    pixelBuffer.set(next);
    state.version++;
    updateHistoryState();
  }

  function getPixel(x, y) {
    if (x < 0 || x >= 64 || y < 0 || y >= 64) return null;
    const idx = (y * 64 + x) * 4;
    return {
      r: pixelBuffer[idx],
      g: pixelBuffer[idx + 1],
      b: pixelBuffer[idx + 2],
      a: pixelBuffer[idx + 3],
    };
  }

  function setPixelRaw(x, y, r, g, b, a) {
    if (x < 0 || x >= 64 || y < 0 || y >= 64) return;
    const idx = (y * 64 + x) * 4;
    pixelBuffer[idx] = r;
    pixelBuffer[idx + 1] = g;
    pixelBuffer[idx + 2] = b;
    pixelBuffer[idx + 3] = a;
  }

  // Paint single pixel taking into account tool & layer filtering
  function applyToolToPixel(x, y, tool = state.activeTool) {
    if (x < 0 || x >= 64 || y < 0 || y >= 64) return false;
    if (!isPixelEditable(x, y, state)) return false;

    const current = getPixel(x, y);
    if (!current) return false;

    if (tool === 'pencil') {
      const rgb = hexToRgb(state.activeColor);
      setPixelRaw(x, y, rgb.r, rgb.g, rgb.b, state.activeAlpha);
      return true;
    } else if (tool === 'eraser') {
      setPixelRaw(x, y, 0, 0, 0, 0);
      return true;
    } else if (tool === 'picker') {
      if (current.a > 0) {
        const hex = rgbToHex(current.r, current.g, current.b);
        state.activeColor = hex;
        state.activeAlpha = current.a;
        addColorToRecent(hex);
      }
      return false;
    } else if (tool === 'noise') {
      const rgb = hexToRgb(state.activeColor);
      const noisy = getNoiseColor(rgb.r, rgb.g, rgb.b, 0.08);
      setPixelRaw(x, y, noisy.r, noisy.g, noisy.b, state.activeAlpha);
      return true;
    } else if (tool === 'lighten') {
      if (current.a === 0) return false;
      const lightened = adjustLuminance(current.r, current.g, current.b, 0.08);
      setPixelRaw(x, y, lightened.r, lightened.g, lightened.b, current.a);
      return true;
    } else if (tool === 'darken') {
      if (current.a === 0) return false;
      const darkened = adjustLuminance(current.r, current.g, current.b, -0.08);
      setPixelRaw(x, y, darkened.r, darkened.g, darkened.b, current.a);
      return true;
    }

    return false;
  }

  // 4-way flood fill bounded by UV face boundary (prevents color bleed)
  function floodFill(startX, startY) {
    if (startX < 0 || startX >= 64 || startY < 0 || startY >= 64) return;
    if (!isPixelEditable(startX, startY, state)) return;

    const targetColor = getPixel(startX, startY);
    if (!targetColor) return;

    const fillRgb = hexToRgb(state.activeColor);
    const fillAlpha = state.activeAlpha;

    if (
      targetColor.r === fillRgb.r &&
      targetColor.g === fillRgb.g &&
      targetColor.b === fillRgb.b &&
      targetColor.a === fillAlpha
    ) {
      return;
    }

    // Determine bounding rectangle
    const face = findFaceAt(startX, startY, state.variant);
    const minX = face ? face.rect[0] : 0;
    const minY = face ? face.rect[1] : 0;
    const maxX = face ? face.rect[0] + face.rect[2] - 1 : 63;
    const maxY = face ? face.rect[1] + face.rect[3] - 1 : 63;

    pushHistory();

    const queue = [{ x: startX, y: startY }];
    const visited = new Uint8Array(64 * 64);

    while (queue.length > 0) {
      const { x, y } = queue.pop();
      const posIdx = y * 64 + x;
      if (visited[posIdx]) continue;
      visited[posIdx] = 1;

      const p = getPixel(x, y);
      if (
        p &&
        p.r === targetColor.r &&
        p.g === targetColor.g &&
        p.b === targetColor.b &&
        p.a === targetColor.a
      ) {
        setPixelRaw(x, y, fillRgb.r, fillRgb.g, fillRgb.b, fillAlpha);

        if (x > minX) queue.push({ x: x - 1, y });
        if (x < maxX) queue.push({ x: x + 1, y });
        if (y > minY) queue.push({ x, y: y - 1 });
        if (y < maxY) queue.push({ x, y: y + 1 });
      }
    }

    state.version++;
  }

  function addColorToRecent(hex) {
    if (!hex) return;
    const clean = hex.toLowerCase();
    const idx = state.recentColors.indexOf(clean);
    if (idx >= 0) {
      state.recentColors.splice(idx, 1);
    }
    state.recentColors.unshift(clean);
    if (state.recentColors.length > 16) {
      state.recentColors.pop();
    }
  }

  // Load a skin from HTML Image or Data URL
  async function loadFromDataUrl(dataUrl, name = 'custom_skin.png', variant = null) {
    return new Promise((resolve, reject) => {
      const img = new Image();
      img.crossOrigin = 'anonymous';
      img.onload = () => {
        const cvs = document.createElement('canvas');
        cvs.width = 64;
        cvs.height = 64;
        const ctx = cvs.getContext('2d');
        ctx.imageSmoothingEnabled = false;
        ctx.drawImage(img, 0, 0, 64, 64);
        const imgData = ctx.getImageData(0, 0, 64, 64);
        pixelBuffer.set(imgData.data);

        undoStack = [];
        redoStack = [];
        state.isDirty = false;
        state.skinName = name;
        if (variant) state.variant = variant;
        updateHistoryState();
        state.version++;
        resolve();
      };
      img.onerror = (e) => reject(e);
      img.src = dataUrl || ZIRCON_STEVE_DATA_URL;
    });
  }

  // Load default template
  async function loadTemplate(type = 'steve') {
    pushHistory();
    if (type === 'steve') {
      state.variant = 'classic';
      await loadFromDataUrl(ZIRCON_STEVE_DATA_URL, 'Steve.png', 'classic');
    } else if (type === 'alex') {
      state.variant = 'slim';
      await loadFromDataUrl(ALEX_BASE_DATA_URL, 'Alex.png', 'slim');
    } else if (type === 'blank') {
      // Clean transparent canvas
      pixelBuffer.fill(0);
      state.skinName = 'new_skin.png';
      state.version++;
    }
  }

  // Export current 64x64 buffer as PNG Data URL
  function toDataUrl() {
    const cvs = document.createElement('canvas');
    cvs.width = 64;
    cvs.height = 64;
    const ctx = cvs.getContext('2d');
    const imgData = ctx.createImageData(64, 64);
    imgData.data.set(pixelBuffer);
    ctx.putImageData(imgData, 0, 0);
    return cvs.toDataURL('image/png');
  }

  // Export as raw bytes array
  function toBytes() {
    const dataUrl = toDataUrl();
    const base64 = dataUrl.split(',')[1];
    const binary = atob(base64);
    const bytes = new Uint8Array(binary.length);
    for (let i = 0; i < binary.length; i++) {
      bytes[i] = binary.charCodeAt(i);
    }
    return Array.from(bytes);
  }

  return {
    state,
    pixelBuffer,
    pushHistory,
    undo,
    redo,
    getPixel,
    setPixelRaw,
    applyToolToPixel,
    floodFill,
    addColorToRecent,
    loadFromDataUrl,
    loadTemplate,
    toDataUrl,
    toBytes,
  };
}
