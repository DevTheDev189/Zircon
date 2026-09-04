<template>
  <div
    ref="wrapperRef"
    class="relative w-full h-full flex flex-col bg-[#070b10] border border-slate-800/90 rounded-2xl overflow-hidden select-none"
  >
    <!-- Top 3D Viewport Controls Bar -->
    <div class="h-9 px-3 bg-[#0a0f16] border-b border-slate-800/80 flex items-center justify-between z-10">
      <div class="flex items-center gap-2 min-w-0">
        <span
          class="text-[11px] font-bold uppercase tracking-wider text-slate-400 flex items-center gap-1.5 shrink-0"
          title="3D Voxel Painter"
        >
          <svg class="w-3.5 h-3.5 text-cyan-400 shrink-0" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
            <path d="M21 16V8a2 2 0 0 0-1-1.73l-7-4a2 2 0 0 0-2 0l-7 4A2 2 0 0 0 3 8v8a2 2 0 0 0 1 1.73l7 4a2 2 0 0 0 2 0l7-4A2 2 0 0 0 21 16z" />
            <polyline points="3.27 6.96 12 12.01 20.73 6.96" />
            <line x1="12" y1="22.08" x2="12" y2="12" />
          </svg>
          <span v-if="containerWidth >= 380">3D Voxel Painter</span>
          <span v-else-if="containerWidth >= 320">3D View</span>
        </span>
      </div>

      <!-- Mode Toggle & Angle Presets -->
      <div class="flex items-center gap-1.5">
        <!-- Paint vs Orbit Mode Toggle -->
        <div class="flex items-center bg-[#070b10] p-0.5 rounded-lg border border-slate-800">
          <button
            type="button"
            class="px-2 py-0.5 rounded text-[10px] font-semibold transition-all flex items-center gap-1"
            :class="
              studio.state.viewMode3D === 'paint'
                ? 'bg-cyan-500/25 text-cyan-300 font-bold shadow-sm'
                : 'text-slate-400 hover:text-white'
            "
            title="Paint Mode: Left-click to paint, Right-click to rotate"
            @click="studio.state.viewMode3D = 'paint'"
          >
            <svg class="w-3 h-3" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
              <path d="M17 3a2.828 2.828 0 1 1 4 4L7.5 20.5 2 22l1.5-5.5L17 3z" />
            </svg>
            Paint
          </button>
          <button
            type="button"
            class="px-2 py-0.5 rounded text-[10px] font-semibold transition-all flex items-center gap-1"
            :class="
              studio.state.viewMode3D === 'orbit'
                ? 'bg-cyan-500/25 text-cyan-300 font-bold shadow-sm'
                : 'text-slate-400 hover:text-white'
            "
            title="Orbit Mode: Left-click rotates camera"
            @click="studio.state.viewMode3D = 'orbit'"
          >
            <svg class="w-3 h-3" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
              <path d="M21.5 2v6h-6M21.34 15.57a10 10 0 1 1-.57-8.38l5.67-5.67" />
            </svg>
            Orbit
          </button>
        </div>

        <div class="h-4 w-px bg-slate-800 mx-1" />

        <!-- Camera Angle Dropdown -->
        <div class="flex items-center gap-1">
          <select
            v-model="selectedAngle"
            class="text-[10px] font-semibold py-0.5 px-1.5 bg-[#121c27] border border-slate-700/80 hover:border-slate-600 text-slate-300 rounded-md focus:outline-none focus:border-cyan-400 cursor-pointer"
            title="Camera View Angle"
            @change="onAngleChange"
          >
            <option value="iso">Isometric</option>
            <option value="front">Front</option>
            <option value="back">Back</option>
            <option value="left">Left</option>
            <option value="right">Right</option>
            <option value="top">Top</option>
          </select>

          <div class="h-3 w-px bg-slate-800 mx-0.5" />

          <!-- In-Viewport HUD Toggle -->
          <button
            type="button"
            class="px-2 h-6 rounded border text-[10px] font-bold flex items-center gap-1 transition-all"
            :class="
              showPartsHud
                ? 'bg-cyan-500/25 border-cyan-400 text-cyan-200 shadow-[0_0_8px_rgba(56,189,248,0.3)]'
                : 'bg-slate-900 border-slate-800 text-slate-400 hover:text-white hover:border-slate-700'
            "
            title="Toggle Body Parts & Layers HUD"
            @click="showPartsHud = !showPartsHud"
          >
            <svg class="w-3 h-3 text-cyan-400" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
              <path d="M20 21v-2a4 4 0 0 0-4-4H8a4 4 0 0 0-4 4v2" />
              <circle cx="12" cy="7" r="4" />
            </svg>
            Parts
          </button>
        </div>
      </div>
    </div>

    <!-- 3D WebGL Canvas Container -->
    <div
      ref="containerRef"
      class="relative flex-1 min-h-0 w-full h-full overflow-hidden"
      :class="
        studio.state.viewMode3D === 'orbit' || isRotating
          ? 'cursor-grab active:cursor-grabbing'
          : 'cursor-crosshair'
      "
      @wheel.prevent="onWheel"
      @pointerdown="onPointerDown"
      @pointermove="onPointerMove"
      @pointerup="onPointerUp"
      @contextmenu.prevent
    >
      <canvas ref="canvasRef" class="w-full h-full block" />

      <!-- Floating In-Viewport Mannequin & Layer HUD -->
      <transition name="fade">
        <div
          v-if="showPartsHud"
          class="absolute top-3 right-3 z-20 w-52 bg-[#09101a]/95 backdrop-blur-md border border-cyan-500/40 rounded-xl p-2.5 shadow-2xl"
        >
          <div class="flex items-center justify-between mb-1.5 pb-1 border-b border-slate-800">
            <span class="text-[10px] font-bold uppercase tracking-wider text-cyan-300">Parts HUD</span>
            <button
              class="text-slate-400 hover:text-white p-0.5 rounded transition-colors"
              title="Close HUD"
              @click="showPartsHud = false"
            >
              <svg class="w-3 h-3 text-slate-400 hover:text-white" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                <line x1="18" y1="6" x2="6" y2="18" />
                <line x1="6" y1="6" x2="18" y2="18" />
              </svg>
            </button>
          </div>
          <SkinMannequinWidget :studio="studio" />
        </div>
      </transition>
    </div>

    <!-- Bottom Status Info Bar -->
    <div class="h-6 px-3 bg-[#0a0f16] border-t border-slate-800/80 flex items-center justify-between text-[10px] font-mono text-slate-400 z-10">
      <div class="flex items-center gap-3">
        <span v-if="hoverPixel">
          Raycast UV: X: <strong class="text-cyan-300">{{ hoverPixel.x }}</strong> Y: <strong class="text-cyan-300">{{ hoverPixel.y }}</strong>
        </span>
        <span v-else class="text-slate-600">3D viewport ready</span>
      </div>
      <div class="flex items-center gap-3 text-slate-500">
        <span>Right-click or Alt+Drag to rotate</span>
      </div>
    </div>
  </div>
</template>

<script setup>
import { onBeforeUnmount, onMounted, ref, watch } from 'vue';
import * as THREE from 'three';
import SkinMannequinWidget from './SkinMannequinWidget.vue';
import { getLinePixels } from './skinStudioState';

const props = defineProps({
  studio: { type: Object, required: true },
});

const wrapperRef = ref(null);
const containerRef = ref(null);
const canvasRef = ref(null);

const hoverPixel = ref(null);
const showPartsHud = ref(false);
const selectedAngle = ref('iso');
const containerWidth = ref(400);

let renderer = null;
let scene = null;
let camera = null;
let characterGroup = null;
let material = null;
let offscreenTextureCanvas = null;
let offscreenTextureCtx = null;
let skinTexture = null;
let animationId = null;
let resizeObserver = null;

let yaw = -Math.PI / 6;
let pitch = -Math.PI / 16;
let cameraDistance = 46;
let isRotating = false;
let isPainting3D = false;
let startX = 0;
let startY = 0;
let lastHitPixel = null;

const raycaster = new THREE.Raycaster();
const mouseVec = new THREE.Vector2();

// --- Atlas Layout Boxes ---
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

const HEAD_BOX = { id: 'head', isOverlay: false, size: [8, 8, 8], center: [0, 28, 0], atlas: faces([0, 0], 8, 8, 8) };
const HAT_BOX = { id: 'head', isOverlay: true, size: [8.8, 8.8, 8.8], center: [0, 28, 0], atlas: faces([32, 0], 8, 8, 8) };
const BODY_BOX = { id: 'body', isOverlay: false, size: [8, 12, 4], center: [0, 18, 0], atlas: faces([16, 16], 8, 12, 4) };
const JACKET_BOX = { id: 'body', isOverlay: true, size: [8.5, 12.5, 4.5], center: [0, 18, 0], atlas: faces([16, 32], 8, 12, 4) };

// Classic (4px)
const R_ARM_BOX = { id: 'rightArm', isOverlay: false, size: [4, 12, 4], center: [-6, 18, 0], atlas: faces([40, 16], 4, 12, 4) };
const R_SLEEVE_BOX = { id: 'rightArm', isOverlay: true, size: [4.5, 12.5, 4.5], center: [-6, 18, 0], atlas: faces([40, 32], 4, 12, 4) };
const L_ARM_BOX = { id: 'leftArm', isOverlay: false, size: [4, 12, 4], center: [6, 18, 0], atlas: faces([32, 48], 4, 12, 4) };
const L_SLEEVE_BOX = { id: 'leftArm', isOverlay: true, size: [4.5, 12.5, 4.5], center: [6, 18, 0], atlas: faces([48, 48], 4, 12, 4) };

// Slim (3px - Alex)
const R_ARM_SLIM_BOX = { id: 'rightArm', isOverlay: false, size: [3, 12, 4], center: [-5.5, 18, 0], atlas: faces([40, 16], 3, 12, 4) };
const R_SLEEVE_SLIM_BOX = { id: 'rightArm', isOverlay: true, size: [3.5, 12.5, 4.5], center: [-5.5, 18, 0], atlas: faces([40, 32], 3, 12, 4) };
const L_ARM_SLIM_BOX = { id: 'leftArm', isOverlay: false, size: [3, 12, 4], center: [5.5, 18, 0], atlas: faces([32, 48], 3, 12, 4) };
const L_SLEEVE_SLIM_BOX = { id: 'leftArm', isOverlay: true, size: [3.5, 12.5, 4.5], center: [5.5, 18, 0], atlas: faces([48, 48], 3, 12, 4) };

const R_LEG_BOX = { id: 'rightLeg', isOverlay: false, size: [4, 12, 4], center: [-2, 6, 0], atlas: faces([0, 16], 4, 12, 4) };
const R_PANTS_BOX = { id: 'rightLeg', isOverlay: true, size: [4.5, 12.5, 4.5], center: [-2, 6, 0], atlas: faces([0, 32], 4, 12, 4) };
const L_LEG_BOX = { id: 'leftLeg', isOverlay: false, size: [4, 12, 4], center: [2, 6, 0], atlas: faces([16, 48], 4, 12, 4) };
const L_PANTS_BOX = { id: 'leftLeg', isOverlay: true, size: [4.5, 12.5, 4.5], center: [2, 6, 0], atlas: faces([0, 48], 4, 12, 4) };

function getBoxList(variant) {
  const isSlim = variant === 'slim';
  return [
    HEAD_BOX, HAT_BOX,
    BODY_BOX, JACKET_BOX,
    isSlim ? R_ARM_SLIM_BOX : R_ARM_BOX,
    isSlim ? R_SLEEVE_SLIM_BOX : R_SLEEVE_BOX,
    isSlim ? L_ARM_SLIM_BOX : L_ARM_BOX,
    isSlim ? L_SLEEVE_SLIM_BOX : L_SLEEVE_BOX,
    R_LEG_BOX, R_PANTS_BOX,
    L_LEG_BOX, L_PANTS_BOX,
  ];
}

function buildBoxGeometry(box) {
  const [Cx, Cy, Cz] = box.center;
  const [sx, sy, sz] = box.size;
  const hx = sx / 2, hy = sy / 2, hz = sz / 2;
  const a = box.atlas;

  const facesList = [
    { key: 'front', norm: [0, 0, 1], corners: [[Cx-hx, Cy+hy, Cz+hz], [Cx-hx, Cy-hy, Cz+hz], [Cx+hx, Cy+hy, Cz+hz], [Cx+hx, Cy-hy, Cz+hz]] },
    { key: 'back', norm: [0, 0, -1], corners: [[Cx+hx, Cy+hy, Cz-hz], [Cx+hx, Cy-hy, Cz-hz], [Cx-hx, Cy+hy, Cz-hz], [Cx-hx, Cy-hy, Cz-hz]] },
    { key: 'right', norm: [-1, 0, 0], corners: [[Cx-hx, Cy+hy, Cz-hz], [Cx-hx, Cy-hy, Cz-hz], [Cx-hx, Cy+hy, Cz+hz], [Cx-hx, Cy-hy, Cz+hz]] },
    { key: 'left', norm: [1, 0, 0], corners: [[Cx+hx, Cy+hy, Cz+hz], [Cx+hx, Cy-hy, Cz+hz], [Cx+hx, Cy+hy, Cz-hz], [Cx+hx, Cy-hy, Cz-hz]] },
    { key: 'top', norm: [0, 1, 0], corners: [[Cx-hx, Cy+hy, Cz-hz], [Cx-hx, Cy+hy, Cz+hz], [Cx+hx, Cy+hy, Cz-hz], [Cx+hx, Cy+hy, Cz+hz]] },
    { key: 'bottom', norm: [0, -1, 0], corners: [[Cx-hx, Cy-hy, Cz+hz], [Cx-hx, Cy-hy, Cz-hz], [Cx+hx, Cy-hy, Cz+hz], [Cx+hx, Cy-hy, Cz-hz]] },
  ];

  const positions = [];
  const normals = [];
  const uvs = [];
  const indices = [];

  for (const f of facesList) {
    const r = a[f.key];
    const u0 = r[0] / 64;
    const u1 = (r[0] + r[2]) / 64;
    const v0 = 1.0 - r[1] / 64;
    const v1 = 1.0 - (r[1] + r[3]) / 64;
    const baseIdx = positions.length / 3;
    for (const v of f.corners) positions.push(v[0], v[1], v[2]);
    for (let i = 0; i < 4; i++) normals.push(f.norm[0], f.norm[1], f.norm[2]);
    uvs.push(u0, v0, u0, v1, u1, v0, u1, v1);
    indices.push(
      baseIdx, baseIdx + 1, baseIdx + 2,
      baseIdx + 2, baseIdx + 1, baseIdx + 3
    );
  }

  const geo = new THREE.BufferGeometry();
  geo.setAttribute('position', new THREE.Float32BufferAttribute(positions, 3));
  geo.setAttribute('normal', new THREE.Float32BufferAttribute(normals, 3));
  geo.setAttribute('uv', new THREE.Float32BufferAttribute(uvs, 2));
  geo.setIndex(indices);
  return geo;
}

function buildCharacterModel(variant = 'classic') {
  const group = new THREE.Group();
  const boxList = getBoxList(variant);

  for (const box of boxList) {
    const geo = buildBoxGeometry(box);
    const mesh = new THREE.Mesh(geo, material);
    mesh.name = `${box.id}_${box.isOverlay ? 'overlay' : 'base'}`;
    mesh.userData = {
      part: box.id,
      isOverlay: box.isOverlay,
    };
    mesh.renderOrder = box.isOverlay ? 1 : 0;
    group.add(mesh);
  }

  return group;
}

// Update mesh visibility to match studio state
function updateMeshVisibility() {
  if (!characterGroup) return;
  const parts = props.studio.state.visibleParts;
  const layers = props.studio.state.layers;

  characterGroup.children.forEach((mesh) => {
    const { part, isOverlay } = mesh.userData;
    const partVisible = parts[part] !== false;
    const layerVisible = isOverlay ? layers.overlay : layers.base;
    mesh.visible = partVisible && layerVisible;
  });
}

// Update Three.js texture with pixels from studio buffer
function syncTexture() {
  if (!offscreenTextureCtx || !skinTexture) return;
  const imgData = offscreenTextureCtx.createImageData(64, 64);
  imgData.data.set(props.studio.pixelBuffer);
  offscreenTextureCtx.putImageData(imgData, 0, 0);
  skinTexture.needsUpdate = true;
}

watch(
  () => props.studio.state.version,
  () => {
    syncTexture();
  }
);

watch(
  () => props.studio.state.variant,
  (v) => {
    if (scene && characterGroup) {
      scene.remove(characterGroup);
      characterGroup = buildCharacterModel(v);
      characterGroup.scale.setScalar(0.78);
      scene.add(characterGroup);
      updateMeshVisibility();
    }
  }
);

watch(
  () => [
    props.studio.state.visibleParts.head,
    props.studio.state.visibleParts.body,
    props.studio.state.visibleParts.rightArm,
    props.studio.state.visibleParts.leftArm,
    props.studio.state.visibleParts.rightLeg,
    props.studio.state.visibleParts.leftLeg,
    props.studio.state.layers.base,
    props.studio.state.layers.overlay,
  ],
  () => {
    updateMeshVisibility();
  }
);

// Camera Angles & Presets
function setCameraAngle(targetYaw, targetPitch) {
  yaw = targetYaw;
  pitch = targetPitch;
}

function resetCamera() {
  yaw = -Math.PI / 6;
  pitch = -Math.PI / 16;
  cameraDistance = 46;
  if (camera) {
    camera.position.set(0, 16, cameraDistance);
    camera.lookAt(0, 13, 0);
  }
}

function onAngleChange() {
  switch (selectedAngle.value) {
    case 'front':
      setCameraAngle(0, 0);
      break;
    case 'back':
      setCameraAngle(Math.PI, 0);
      break;
    case 'left':
      setCameraAngle(Math.PI / 2, 0);
      break;
    case 'right':
      setCameraAngle(-Math.PI / 2, 0);
      break;
    case 'top':
      setCameraAngle(0, -Math.PI / 2 + 0.05);
      break;
    case 'iso':
    default:
      resetCamera();
      break;
  }
}

function onWheel(e) {
  cameraDistance = Math.max(22, Math.min(80, cameraDistance + e.deltaY * 0.05));
  if (camera) {
    camera.position.z = cameraDistance;
  }
}

// Raycasting to find 64x64 pixel on the 3D model
function raycastPixel(clientX, clientY) {
  if (!containerRef.value || !characterGroup || !camera) return null;
  const rect = containerRef.value.getBoundingClientRect();
  mouseVec.x = ((clientX - rect.left) / rect.width) * 2 - 1;
  mouseVec.y = -((clientY - rect.top) / rect.height) * 2 + 1;

  raycaster.setFromCamera(mouseVec, camera);

  // Only raycast visible meshes
  const visibleMeshes = characterGroup.children.filter((m) => m.visible);
  const intersects = raycaster.intersectObjects(visibleMeshes, false);

  if (intersects.length > 0 && intersects[0].uv) {
    const uv = intersects[0].uv;
    const x = Math.min(63, Math.max(0, Math.floor(uv.x * 64)));
    const y = Math.min(63, Math.max(0, Math.floor((1.0 - uv.y) * 64)));
    return { x, y, mesh: intersects[0].object };
  }
  return null;
}

// Pointer Handlers for Painting / Orbiting
function onPointerDown(e) {
  startX = e.clientX;
  startY = e.clientY;

  // Right click, Middle click, Alt key, or Orbit mode -> rotate camera
  if (e.button === 2 || e.button === 1 || e.altKey || props.studio.state.viewMode3D === 'orbit') {
    isRotating = true;
    return;
  }

  if (e.button !== 0) return;

  const hit = raycastPixel(e.clientX, e.clientY);
  if (!hit) {
    // If clicked empty background in paint mode, allow rotating
    isRotating = true;
    return;
  }

  if (props.studio.state.activeTool === 'bucket') {
    props.studio.floodFill(hit.x, hit.y);
    syncTexture();
    return;
  }

  if (props.studio.state.activeTool === 'picker') {
    props.studio.applyToolToPixel(hit.x, hit.y, 'picker');
    return;
  }

  props.studio.pushHistory();
  isPainting3D = true;
  lastHitPixel = { x: hit.x, y: hit.y };
  props.studio.applyToolToPixel(hit.x, hit.y);
  syncTexture();
  props.studio.state.version++;
}

function onPointerMove(e) {
  if (isRotating) {
    const dx = e.clientX - startX;
    const dy = e.clientY - startY;
    startX = e.clientX;
    startY = e.clientY;
    yaw += dx * 0.01;
    pitch = Math.max(-Math.PI / 3, Math.min(Math.PI / 3, pitch + dy * 0.01));
    return;
  }

  const hit = raycastPixel(e.clientX, e.clientY);
  hoverPixel.value = hit ? { x: hit.x, y: hit.y } : null;

  if (!isPainting3D || !hit) return;

  if (lastHitPixel && (lastHitPixel.x !== hit.x || lastHitPixel.y !== hit.y)) {
    const points = getLinePixels(lastHitPixel.x, lastHitPixel.y, hit.x, hit.y);
    for (const pt of points) {
      props.studio.applyToolToPixel(pt.x, pt.y);
    }
    lastHitPixel = { x: hit.x, y: hit.y };
    syncTexture();
    props.studio.state.version++;
  }
}

function onPointerUp() {
  isRotating = false;
  if (isPainting3D) {
    isPainting3D = false;
    lastHitPixel = null;
    syncTexture();
    props.studio.state.version++;
  }
}

function resize() {
  const target = wrapperRef.value || containerRef.value;
  if (target) {
    containerWidth.value = target.clientWidth || 300;
  }
  if (!renderer || !containerRef.value || !camera) return;
  const w = containerRef.value.clientWidth || 300;
  const h = containerRef.value.clientHeight || 400;
  if (w === 0 || h === 0) return;
  renderer.setSize(w, h, false);
  camera.aspect = w / h;
  camera.updateProjectionMatrix();
}

function animate() {
  animationId = requestAnimationFrame(animate);
  if (characterGroup) {
    characterGroup.rotation.set(pitch, yaw, 0);
  }
  if (renderer && scene && camera) {
    renderer.render(scene, camera);
  }
}

onMounted(() => {
  const mount = containerRef.value;
  const w = mount.clientWidth || 360;
  const h = mount.clientHeight || 440;

  // Offscreen canvas for texture updates
  offscreenTextureCanvas = document.createElement('canvas');
  offscreenTextureCanvas.width = 64;
  offscreenTextureCanvas.height = 64;
  offscreenTextureCtx = offscreenTextureCanvas.getContext('2d', { willReadFrequently: true });
  offscreenTextureCtx.imageSmoothingEnabled = false;

  skinTexture = new THREE.CanvasTexture(offscreenTextureCanvas);
  skinTexture.magFilter = THREE.NearestFilter;
  skinTexture.minFilter = THREE.NearestFilter;
  skinTexture.generateMipmaps = false;
  skinTexture.colorSpace = THREE.SRGBColorSpace;

  renderer = new THREE.WebGLRenderer({
    canvas: canvasRef.value,
    antialias: true,
    alpha: true,
  });
  renderer.setSize(w, h, false);
  renderer.setPixelRatio(Math.min(window.devicePixelRatio, 2));
  renderer.outputColorSpace = THREE.SRGBColorSpace;

  scene = new THREE.Scene();
  camera = new THREE.PerspectiveCamera(40, w / h, 0.1, 150);
  camera.position.set(0, 16, cameraDistance);
  camera.lookAt(0, 13, 0);

  // Soft directional lights
  const keyLight = new THREE.DirectionalLight(0xffffff, 1.6);
  keyLight.position.set(10, 30, 20);
  scene.add(keyLight);

  const rimLight = new THREE.DirectionalLight(0x47d2c9, 0.6);
  rimLight.position.set(-15, 10, -12);
  scene.add(rimLight);

  scene.add(new THREE.AmbientLight(0xffffff, 0.45));

  // Ground shadow disc
  const shadow = new THREE.Mesh(
    new THREE.CircleGeometry(13, 48),
    new THREE.MeshBasicMaterial({ color: 0x000000, transparent: true, opacity: 0.35 })
  );
  shadow.rotation.x = -Math.PI / 2;
  shadow.position.y = -0.02;
  scene.add(shadow);

  // Shared skin material
  material = new THREE.MeshLambertMaterial({
    color: 0xffffff,
    map: skinTexture,
    transparent: true,
    alphaTest: 0.2,
    side: THREE.FrontSide,
  });

  characterGroup = buildCharacterModel(props.studio.state.variant);
  characterGroup.scale.setScalar(0.78);
  scene.add(characterGroup);

  syncTexture();
  updateMeshVisibility();

  const mountTarget = wrapperRef.value || containerRef.value;
  if (mountTarget) {
    containerWidth.value = mountTarget.clientWidth || 360;
  }

  if (window.ResizeObserver && mountTarget) {
    resizeObserver = new ResizeObserver((entries) => {
      if (entries && entries[0] && entries[0].contentRect) {
        containerWidth.value = entries[0].contentRect.width;
      }
      resize();
    });
    resizeObserver.observe(mountTarget);
  } else {
    window.addEventListener('resize', resize);
  }

  window.addEventListener('pointerup', onPointerUp);
  animate();
});

onBeforeUnmount(() => {
  cancelAnimationFrame(animationId);
  window.removeEventListener('pointerup', onPointerUp);
  window.removeEventListener('resize', resize);
  if (resizeObserver) {
    resizeObserver.disconnect();
    resizeObserver = null;
  }
  if (skinTexture) skinTexture.dispose();
  if (renderer) renderer.dispose();
});
</script>
