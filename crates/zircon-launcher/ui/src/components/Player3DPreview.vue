<template>
  <div ref="container" class="relative w-full h-full overflow-hidden select-none">
    <canvas
      ref="canvas"
      class="w-full h-full block cursor-grab active:cursor-grabbing transition-opacity duration-300"
      :class="isReady ? 'opacity-100' : 'opacity-0'"
    />
  </div>
</template>

<script setup>
// WebGL 3D Minecraft player skin renderer built on Three.js
import { onActivated, onBeforeUnmount, onMounted, ref, watch } from 'vue';
import * as THREE from 'three';
import { createDefaultSteveDataUrl, getCachedActiveSkin } from '../lib/api';

const props = defineProps({
  imageUri: { type: String, default: null },
  defaultSkinUri: { type: String, default: null },
  variant: { type: String, default: 'classic' },
});

const container = ref(null);
const canvas = ref(null);
const skinLoaded = ref(false);
const isReady = ref(false);

let renderer = null;
let scene = null;
let camera = null;
let group = null;
let material = null;
let animationId = null;
let resizeObserver = null;
let dragging = false;
let lastX = 0;
let lastY = 0;
let yaw = -Math.PI / 8;
let pitch = -Math.PI / 16;
let currentVariant = 'classic';
let currentSkinUri = null;
let loadRequestId = 0;

// In-memory cache of decoded 64x64 skin canvases to allow instant synchronous texture application
const skinCanvasCache = new Map();

// ---- Atlas layouts ---------------------------------------------------------
function faces(base, w, h, d) {
  const [bu, bv] = base;
  return {
    front: [bu + d, bv + d, w, h],
    back: [bu + d + w + d, bv + d, w, h],
    right: [bu, bv + d, d, h],
    left: [bu + d + w, bv + d, d, h],
    top: [bu + d, bv, w, d],
    bottom: [bu + d + w, bv, w, d],
  };
}

const HEAD = { size: [8, 8, 8], center: [0, 28, 0], atlas: faces([0, 0], 8, 8, 8) };
const HAT = { size: [8.8, 8.8, 8.8], center: [0, 28, 0], atlas: faces([32, 0], 8, 8, 8) };
const BODY = { size: [8, 12, 4], center: [0, 18, 0], atlas: faces([16, 16], 8, 12, 4) };
const JACKET = { size: [8.5, 12.5, 4.5], center: [0, 18, 0], atlas: faces([16, 32], 8, 12, 4) };

// Classic (4px arms)
const R_ARM = { size: [4, 12, 4], center: [-6, 18, 0], atlas: faces([40, 16], 4, 12, 4) };
const R_SLEEVE = { size: [4.5, 12.5, 4.5], center: [-6, 18, 0], atlas: faces([40, 32], 4, 12, 4) };
const L_ARM = { size: [4, 12, 4], center: [6, 18, 0], atlas: faces([32, 48], 4, 12, 4) };
const L_SLEEVE = { size: [4.5, 12.5, 4.5], center: [6, 18, 0], atlas: faces([48, 48], 4, 12, 4) };

// Slim (3px arms - Alex)
const R_ARM_SLIM = { size: [3, 12, 4], center: [-5.5, 18, 0], atlas: faces([40, 16], 3, 12, 4) };
const R_SLEEVE_SLIM = { size: [3.5, 12.5, 4.5], center: [-5.5, 18, 0], atlas: faces([40, 32], 3, 12, 4) };
const L_ARM_SLIM = { size: [3, 12, 4], center: [5.5, 18, 0], atlas: faces([32, 48], 3, 12, 4) };
const L_SLEEVE_SLIM = { size: [3.5, 12.5, 4.5], center: [5.5, 18, 0], atlas: faces([48, 48], 3, 12, 4) };

const R_LEG = { size: [4, 12, 4], center: [-2, 6, 0], atlas: faces([0, 16], 4, 12, 4) };
const R_PANTS = { size: [4.5, 12.5, 4.5], center: [-2, 6, 0], atlas: faces([0, 32], 4, 12, 4) };
const L_LEG = { size: [4, 12, 4], center: [2, 6, 0], atlas: faces([16, 48], 4, 12, 4) };
const L_PANTS = { size: [4.5, 12.5, 4.5], center: [2, 6, 0], atlas: faces([0, 48], 4, 12, 4) };

function getBoxes(variant) {
  const isSlim = variant === 'slim';
  const rArm = isSlim ? R_ARM_SLIM : R_ARM;
  const lArm = isSlim ? L_ARM_SLIM : L_ARM;
  const rSleeve = isSlim ? R_SLEEVE_SLIM : R_SLEEVE;
  const lSleeve = isSlim ? L_SLEEVE_SLIM : L_SLEEVE;

  const baseBoxes = [HEAD, BODY, rArm, lArm, R_LEG, L_LEG];
  const overlayBoxes = [HAT, JACKET, rSleeve, lSleeve, R_PANTS, L_PANTS];
  return { baseBoxes, overlayBoxes };
}

// ---- Geometry builder ------------------------------------------------------
function buildBoxesInto(builder, boxes) {
  for (const box of boxes) {
    const [Cx, Cy, Cz] = box.center;
    const [sx, sy, sz] = box.size;
    const hx = sx / 2, hy = sy / 2, hz = sz / 2;
    const a = box.atlas;

    const faces = [
      { key: 'front', norm: [0, 0, 1], corners: [[Cx-hx, Cy+hy, Cz+hz], [Cx-hx, Cy-hy, Cz+hz], [Cx+hx, Cy+hy, Cz+hz], [Cx+hx, Cy-hy, Cz+hz]] },
      { key: 'back', norm: [0, 0, -1], corners: [[Cx+hx, Cy+hy, Cz-hz], [Cx+hx, Cy-hy, Cz-hz], [Cx-hx, Cy+hy, Cz-hz], [Cx-hx, Cy-hy, Cz-hz]] },
      { key: 'right', norm: [-1, 0, 0], corners: [[Cx-hx, Cy+hy, Cz-hz], [Cx-hx, Cy-hy, Cz-hz], [Cx-hx, Cy+hy, Cz+hz], [Cx-hx, Cy-hy, Cz+hz]] },
      { key: 'left', norm: [1, 0, 0], corners: [[Cx+hx, Cy+hy, Cz+hz], [Cx+hx, Cy-hy, Cz+hz], [Cx+hx, Cy+hy, Cz-hz], [Cx+hx, Cy-hy, Cz-hz]] },
      { key: 'top', norm: [0, 1, 0], corners: [[Cx-hx, Cy+hy, Cz-hz], [Cx-hx, Cy+hy, Cz+hz], [Cx+hx, Cy+hy, Cz-hz], [Cx+hx, Cy+hy, Cz+hz]] },
      { key: 'bottom', norm: [0, -1, 0], corners: [[Cx-hx, Cy-hy, Cz+hz], [Cx-hx, Cy-hy, Cz-hz], [Cx+hx, Cy-hy, Cz+hz], [Cx+hx, Cy-hy, Cz-hz]] },
    ];

    for (const face of faces) {
      const r = a[face.key];
      const w64 = 64, h64 = 64;
      const u0 = r[0] / w64;
      const u1 = (r[0] + r[2]) / w64;
      const v0 = 1.0 - r[1] / h64;
      const v1 = 1.0 - (r[1] + r[3]) / h64;
      const baseIdx = builder.positions.length / 3;
      const c = face.corners;
      for (const v of c) builder.positions.push(v[0], v[1], v[2]);
      for (let i = 0; i < 4; i++) builder.normals.push(face.norm[0], face.norm[1], face.norm[2]);
      builder.uvs.push(u0, v0, u0, v1, u1, v0, u1, v1);
      builder.indices.push(
        baseIdx, baseIdx + 1, baseIdx + 2,
        baseIdx + 2, baseIdx + 1, baseIdx + 3
      );
    }
  }
}

function buildModel(variant = 'classic') {
  const { baseBoxes, overlayBoxes } = getBoxes(variant);
  const baseBuilder = { positions: [], normals: [], uvs: [], indices: [] };
  const overlayBuilder = { positions: [], normals: [], uvs: [], indices: [] };
  buildBoxesInto(baseBuilder, baseBoxes);
  buildBoxesInto(overlayBuilder, overlayBoxes);

  const toGeo = (b) => {
    const geo = new THREE.BufferGeometry();
    geo.setAttribute('position', new THREE.Float32BufferAttribute(b.positions, 3));
    geo.setAttribute('normal', new THREE.Float32BufferAttribute(b.normals, 3));
    geo.setAttribute('uv', new THREE.Float32BufferAttribute(b.uvs, 2));
    geo.setIndex(b.indices);
    return geo;
  };

  const base = new THREE.Mesh(toGeo(baseBuilder), material);
  const overlay = new THREE.Mesh(toGeo(overlayBuilder), material);
  overlay.renderOrder = 1;
  base.renderOrder = 0;
  const model = new THREE.Group();
  model.add(base, overlay);
  return model;
}

// ---- Texture / skin --------------------------------------------------------
function copyFlipped(ctx, sx, sy, sw, sh, dx, dy) {
  ctx.save();
  ctx.translate(dx + sw, dy);
  ctx.scale(-1, 1);
  ctx.drawImage(ctx.canvas, sx, sy, sw, sh, 0, 0, sw, sh);
  ctx.restore();
}

function isAreaTransparent(ctx, x, y, w, h) {
  try {
    const imgData = ctx.getImageData(x, y, w, h).data;
    for (let i = 3; i < imgData.length; i += 4) {
      if (imgData[i] > 10) return false;
    }
  } catch {
    return false;
  }
  return true;
}

function mirrorLegacyLimb(ctx, isArm) {
  if (isArm) {
    // Right Arm (40, 16) -> Left Arm (32, 48)
    copyFlipped(ctx, 44, 16, 4, 4, 36, 48); // Top
    copyFlipped(ctx, 48, 16, 4, 4, 40, 48); // Bottom
    copyFlipped(ctx, 48, 20, 4, 12, 32, 52); // Inside
    copyFlipped(ctx, 44, 20, 4, 12, 36, 52); // Front
    copyFlipped(ctx, 40, 20, 4, 12, 40, 52); // Outside
    copyFlipped(ctx, 52, 20, 4, 12, 44, 52); // Back
  } else {
    // Right Leg (0, 16) -> Left Leg (16, 48)
    copyFlipped(ctx, 4, 16, 4, 4, 20, 48); // Top
    copyFlipped(ctx, 8, 16, 4, 4, 24, 48); // Bottom
    copyFlipped(ctx, 8, 20, 4, 12, 16, 52); // Inside
    copyFlipped(ctx, 4, 20, 4, 12, 20, 52); // Front
    copyFlipped(ctx, 0, 20, 4, 12, 24, 52); // Outside
    copyFlipped(ctx, 12, 20, 4, 12, 28, 52); // Back
  }
}

function createSkinCanvas(image) {
  if (!image || image.width < 32 || image.height < 32) {
    return null;
  }

  const cvs = document.createElement('canvas');
  cvs.width = 64;
  cvs.height = 64;
  const ctx = cvs.getContext('2d', { willReadFrequently: true });
  ctx.imageSmoothingEnabled = false;
  ctx.drawImage(image, 0, 0);

  const isLegacy64x32 = image.height === 32 || image.height < 64;
  const isLeftLegEmpty = isLegacy64x32 || isAreaTransparent(ctx, 16, 48, 16, 16);
  const isLeftArmEmpty = isLegacy64x32 || isAreaTransparent(ctx, 32, 48, 16, 16);

  if (isLeftLegEmpty) {
    mirrorLegacyLimb(ctx, false);
  }
  if (isLeftArmEmpty) {
    mirrorLegacyLimb(ctx, true);
  }

  return cvs;
}

function makeTextureFromCanvas(cvs) {
  const texture = new THREE.CanvasTexture(cvs);
  texture.magFilter = THREE.NearestFilter;
  texture.minFilter = THREE.NearestFilter;
  texture.generateMipmaps = false;
  texture.colorSpace = THREE.SRGBColorSpace;
  return texture;
}

function applySkinCanvas(cvs) {
  if (!material || !cvs) return;
  const tex = makeTextureFromCanvas(cvs);
  if (material.map) material.map.dispose();
  material.map = tex;
  material.needsUpdate = true;
  skinLoaded.value = true;
  isReady.value = true;
}

function applySkin(imageUri) {
  const uri =
    imageUri ||
    props.defaultSkinUri ||
    getCachedActiveSkin()?.dataUrl ||
    createDefaultSteveDataUrl();
  currentSkinUri = uri;

  // If already cached, apply synchronously without delay or flicker
  if (skinCanvasCache.has(uri)) {
    applySkinCanvas(skinCanvasCache.get(uri));
    return;
  }

  const requestId = ++loadRequestId;
  const img = new Image();
  img.crossOrigin = 'anonymous';
  img.onload = () => {
    if (requestId !== loadRequestId) return;
    const cvs = createSkinCanvas(img);
    if (cvs) {
      skinCanvasCache.set(uri, cvs);
      applySkinCanvas(cvs);
    } else {
      applyFallbackSkin();
    }
  };
  img.onerror = () => {
    if (requestId !== loadRequestId) return;
    applyFallbackSkin();
  };
  img.src = uri;
}

function applyFallbackSkin() {
  const fallback = createDefaultSteveDataUrl();
  if (skinCanvasCache.has(fallback)) {
    applySkinCanvas(skinCanvasCache.get(fallback));
    return;
  }
  const img = new Image();
  img.onload = () => {
    const cvs = createSkinCanvas(img);
    if (cvs) {
      skinCanvasCache.set(fallback, cvs);
      applySkinCanvas(cvs);
    }
  };
  img.src = fallback;
}

function setVariant(variant) {
  currentVariant = variant || 'classic';
  if (scene && group) {
    const rot = { x: group.rotation.x, y: group.rotation.y, z: group.rotation.z };
    scene.remove(group);
    group = buildModel(currentVariant);
    group.scale.setScalar(0.8);
    group.rotation.set(rot.x, rot.y, rot.z);
    scene.add(group);
  }
}

function updateSkin(imageUri) {
  applySkin(imageUri);
}

function resetSkin() {
  applyFallbackSkin();
}

// ---- Render loop / interaction ---------------------------------------------
function resize() {
  if (!renderer || !container.value) return;
  const w = container.value.clientWidth || 300;
  const h = container.value.clientHeight || 400;
  if (w === 0 || h === 0) return;
  renderer.setSize(w, h, false);
  camera.aspect = w / h;
  camera.updateProjectionMatrix();
}

function animate() {
  animationId = requestAnimationFrame(animate);
  if (group) group.rotation.set(pitch, yaw, 0);
  if (renderer && scene && camera) {
    renderer.render(scene, camera);
  }
}

function onPointerDown(e) {
  dragging = true;
  lastX = e.clientX;
  lastY = e.clientY;
}

function onPointerMove(e) {
  if (!dragging) return;
  const dx = e.clientX - lastX;
  const dy = e.clientY - lastY;
  lastX = e.clientX;
  lastY = e.clientY;
  yaw += dx * 0.01;
  pitch = Math.max(-Math.PI / 4, Math.min(Math.PI / 4, pitch + dy * 0.01));
}

function onPointerUp() {
  dragging = false;
}

watch(
  () => props.imageUri,
  (uri) => {
    if (uri) applySkin(uri);
  }
);

watch(
  () => props.defaultSkinUri,
  (uri) => uri && applySkin(uri)
);

watch(
  () => props.variant,
  (v) => {
    if (v && v !== currentVariant) setVariant(v);
  }
);

defineExpose({ updateSkin, resetSkin, setVariant });

onMounted(() => {
  const mount = container.value;
  const w = mount.clientWidth || 360;
  const h = mount.clientHeight || 440;

  currentVariant = props.variant || 'classic';

  renderer = new THREE.WebGLRenderer({
    canvas: canvas.value,
    antialias: true,
    alpha: true,
  });
  renderer.setSize(w, h, false);
  renderer.setPixelRatio(Math.min(window.devicePixelRatio, 2));
  renderer.outputColorSpace = THREE.SRGBColorSpace;

  scene = new THREE.Scene();
  camera = new THREE.PerspectiveCamera(40, w / h, 0.1, 100);
  camera.position.set(0, 16, 46);
  camera.lookAt(0, 13, 0);

  // Soft key light + luminous cyan rim light
  const key = new THREE.DirectionalLight(0xffffff, 1.6);
  key.position.set(10, 30, 20);
  scene.add(key);
  const fill = new THREE.DirectionalLight(0x47d2c9, 0.6);
  fill.position.set(-15, 10, -12);
  scene.add(fill);
  scene.add(new THREE.AmbientLight(0xffffff, 0.4));

  // Soft ground shadow
  const shadow = new THREE.Mesh(
    new THREE.CircleGeometry(13, 48),
    new THREE.MeshBasicMaterial({ color: 0x000000, transparent: true, opacity: 0.4 })
  );
  shadow.rotation.x = -Math.PI / 2;
  shadow.position.y = -0.02;
  scene.add(shadow);

  material = new THREE.MeshLambertMaterial({
    color: 0xffffff,
    map: null,
    transparent: true,
    alphaTest: 0.5,
    side: THREE.FrontSide,
  });
  group = buildModel(currentVariant);
  group.scale.setScalar(0.8);
  scene.add(group);
  group.rotation.set(pitch, yaw, 0);

  mount.addEventListener('pointerdown', onPointerDown);
  window.addEventListener('pointermove', onPointerMove);
  window.addEventListener('pointerup', onPointerUp);

  if (window.ResizeObserver && mount) {
    resizeObserver = new ResizeObserver(() => {
      resize();
    });
    resizeObserver.observe(mount);
  } else {
    window.addEventListener('resize', resize);
  }

  // Load requested skin, cached active skin, or default Steve
  applySkin(props.imageUri || props.defaultSkinUri);

  animate();
});

onActivated(() => {
  resize();
});

onBeforeUnmount(() => {
  cancelAnimationFrame(animationId);
  if (container.value) {
    container.value.removeEventListener('pointerdown', onPointerDown);
  }
  window.removeEventListener('pointermove', onPointerMove);
  window.removeEventListener('pointerup', onPointerUp);
  window.removeEventListener('resize', resize);
  if (resizeObserver) {
    resizeObserver.disconnect();
    resizeObserver = null;
  }
  if (renderer) {
    renderer.dispose();
    renderer = null;
  }
});
</script>
