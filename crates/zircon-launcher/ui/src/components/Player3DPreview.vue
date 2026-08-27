<template>
  <div ref="container" class="relative w-full h-full overflow-hidden select-none">
    <canvas ref="canvas" class="w-full h-full block cursor-grab active:cursor-grabbing" />
  </div>
</template>

<script setup>
// WebGL 3D Minecraft player skin renderer built on Three.js
import { onBeforeUnmount, onMounted, ref, watch } from 'vue';
import * as THREE from 'three';
import { createDefaultSteveDataUrl } from '../lib/api';

const container = ref(null);
const canvas = ref(null);
const skinLoaded = ref(false);

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
function processSkinTexture(image) {
  if (image.width < 32 || image.height < 32) {
    return null;
  }

  const canvas = document.createElement('canvas');
  canvas.width = 64;
  canvas.height = 64;
  const ctx = canvas.getContext('2d');
  ctx.imageSmoothingEnabled = false;
  ctx.drawImage(image, 0, 0);

  if (image.width === 64 && image.height === 32) {
    // Convert 64x32 legacy skin to 64x64
    ctx.save();
    ctx.translate(32, 48);
    ctx.scale(-1, 1);
    ctx.drawImage(canvas, 0, 16, 16, 16, -16, 0, 16, 16);
    ctx.restore();

    ctx.save();
    ctx.translate(48, 48);
    ctx.scale(-1, 1);
    ctx.drawImage(canvas, 40, 16, 16, 16, -16, 0, 16, 16);
    ctx.restore();
  }

  const texture = new THREE.CanvasTexture(canvas);
  texture.magFilter = THREE.NearestFilter;
  texture.minFilter = THREE.NearestFilter;
  texture.generateMipmaps = false;
  texture.colorSpace = THREE.SRGBColorSpace;
  return texture;
}

function applySkin(imageUri) {
  const uri = imageUri || createDefaultSteveDataUrl();
  currentSkinUri = uri;
  const img = new Image();
  img.crossOrigin = 'anonymous';
  img.onload = () => {
    if (!material) return;
    const tex = processSkinTexture(img);
    if (!tex) {
      applyFallbackSkin();
      return;
    }
    if (material.map) material.map.dispose();
    material.map = tex;
    material.needsUpdate = true;
    skinLoaded.value = true;
  };
  img.onerror = () => {
    applyFallbackSkin();
  };
  img.src = uri;
}

function applyFallbackSkin() {
  const fallback = createDefaultSteveDataUrl();
  const img = new Image();
  img.onload = () => {
    if (!material) return;
    const tex = processSkinTexture(img);
    if (tex) {
      if (material.map) material.map.dispose();
      material.map = tex;
      material.needsUpdate = true;
      skinLoaded.value = true;
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
  renderer.render(scene, camera);
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

const props = defineProps({
  imageUri: { type: String, default: null },
  defaultSkinUri: { type: String, default: null },
});

watch(
  () => props.imageUri,
  (uri) => applySkin(uri)
);

watch(
  () => props.defaultSkinUri,
  (uri) => uri && applySkin(uri)
);

defineExpose({ updateSkin, resetSkin, setVariant });

onMounted(() => {
  const mount = container.value;
  const w = mount.clientWidth || 360;
  const h = mount.clientHeight || 440;

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

  // Load requested skin or default Steve
  if (props.imageUri) applySkin(props.imageUri);
  else if (props.defaultSkinUri) applySkin(props.defaultSkinUri);
  else applyFallbackSkin();

  animate();
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
