<template>
  <div ref="container" class="relative w-full h-full overflow-hidden">
    <canvas ref="canvas" class="w-full h-full block" />
    <div
      v-if="!skinLoaded"
      class="absolute inset-0 flex items-center justify-center text-muted text-sm pointer-events-none"
    >
      No skin yet — pick one to preview
    </div>
  </div>
</template>

<script setup>
// WebGL 3D Minecraft player skin renderer built on Three.js — the webview
// replacement for the JavaFX/LWJGL `Player3DRenderer` (Step 5.3).
//
// The model is the classic dual-layer 64x64 box figure: 6 base boxes (head,
// body, right/left arm, right/left leg) plus 6 outer overlay boxes (hat,
// jacket, sleeves, pants) inflated by 0.25 units. Every face samples its
// region from the standard 64x64 skin atlas with `NearestFilter` for crisp
// pixel art. Mouse drag rotates the figure: yaw freely, pitch clamped to ±45°.
import { onBeforeUnmount, onMounted, ref, watch } from 'vue';
import * as THREE from 'three';

const container = ref(null);
const canvas = ref(null);
const skinLoaded = ref(false);

let renderer = null;
let scene = null;
let camera = null;
let group = null;
let material = null;
let animationId = null;
let dragging = false;
let lastX = 0;
let lastY = 0;
let yaw = -Math.PI / 6;
let pitch = -Math.PI / 12;

// One box: dimensions (sx, sy, sz) centered at (x, y, z). `faces` maps each
// face to its `[u, v]` top-left corner in the 64x64 atlas. Region sizes are
// derived from the box dimensions (front/back: sx×sy, right/left: sz×sy,
// top/bottom: sx×sz), matching the vanilla Box unwrap.
function createBox(sx, sy, sz, x, y, z, faces) {
  const group = new THREE.Group();

  const addPlane = (axis, dir, region, flipU, flipV) => {
    const [u0, v0] = region;
    const rw = axis === 'x' ? sz : sx; // side faces are sz wide; front/back/top/bottom are sx
    const rh = axis === 'y' ? sz : sy; // top/bottom are sz tall; side faces are sy tall
    const geo = new THREE.PlaneGeometry(1, 1);
    const pos = geo.attributes.position;
    const uv = geo.attributes.uv;
    for (let i = 0; i < pos.count; i++) {
      const lx = pos.getX(i); // plane local x in [-0.5, 0.5]
      const ly = pos.getY(i); // plane local y in [-0.5, 0.5]
      let wx = 0;
      let wy = 0;
      let wz = 0;
      let tu = 0;
      let tv = 0;
      if (axis === 'x') {
        // plane local x = world z, local y = world y
        wx = dir * (sx / 2);
        wz = lx * sz;
        wy = ly * sy;
        tu = (wz + sz / 2) / sz;
        tv = (wy + sy / 2) / sy;
      } else if (axis === 'y') {
        // plane local x = world x, local y = world z
        wy = dir * (sy / 2);
        wx = lx * sx;
        wz = ly * sz;
        tu = (wx + sx / 2) / sx;
        tv = (wz + sz / 2) / sz;
      } else {
        // plane local x = world x, local y = world y
        wz = dir * (sz / 2);
        wx = lx * sx;
        wy = ly * sy;
        tu = (wx + sx / 2) / sx;
        tv = (wy + sy / 2) / sy;
      }
      if (flipU) tu = 1 - tu;
      if (flipV) tv = 1 - tv;
      pos.setXYZ(i, wx, wy, wz);
      uv.setXY(i, (u0 + tu * rw) / 64, (v0 + tv * rh) / 64);
    }
    geo.computeVertexNormals();
    const mesh = new THREE.Mesh(geo, material);
    mesh.position.set(x, y, z);
    group.add(mesh);
  };

  // +z front, -z back, +x right, -x left, +y top, -y bottom.
  // Orientation convention (viewed from outside the model):
  //   front: u → +x         back: u → -x
  //   right: u → -z         left: u → +z
  //   top:   u → +x, v → +z  bottom: u → -x, v → -z
  addPlane('z', 1, faces.front, false, false);
  addPlane('z', -1, faces.back, true, false);
  addPlane('x', 1, faces.right, true, false);
  addPlane('x', -1, faces.left, false, false);
  addPlane('y', 1, faces.top, false, false);
  addPlane('y', -1, faces.bottom, true, true);
  return group;
}

// Atlas face origins for every box, from the standard 64x64 Minecraft skin
// layout. The hat's top/bottom sit shifted left relative to the head's, so
// every box is written out explicitly.
const HEAD = { front: [8, 8], back: [24, 8], right: [0, 8], left: [16, 8], top: [8, 0], bottom: [16, 0] };
const HAT = { front: [40, 8], back: [56, 8], right: [32, 8], left: [48, 8], top: [32, 0], bottom: [40, 0] };
const BODY = { front: [20, 20], back: [32, 20], right: [16, 20], left: [28, 20], top: [20, 16], bottom: [28, 16] };
const JACKET = { front: [20, 36], back: [32, 36], right: [16, 36], left: [28, 36], top: [20, 32], bottom: [28, 32] };
const R_ARM = { front: [44, 20], back: [52, 20], right: [40, 20], left: [48, 20], top: [44, 16], bottom: [48, 16] };
const R_SLEEVE = { front: [44, 36], back: [52, 36], right: [40, 36], left: [48, 36], top: [44, 32], bottom: [48, 32] };
const L_ARM = { front: [36, 52], back: [44, 52], right: [32, 52], left: [40, 52], top: [36, 48], bottom: [40, 48] };
const L_SLEEVE = { front: [52, 52], back: [60, 52], right: [48, 52], left: [56, 52], top: [52, 48], bottom: [56, 48] };
const R_LEG = { front: [4, 20], back: [12, 20], right: [0, 20], left: [8, 20], top: [4, 16], bottom: [8, 16] };
const R_PANTS = { front: [4, 36], back: [12, 36], right: [0, 36], left: [8, 36], top: [4, 32], bottom: [8, 32] };
const L_LEG = { front: [20, 52], back: [28, 52], right: [16, 52], left: [24, 52], top: [20, 48], bottom: [24, 48] };
const L_PANTS = { front: [36, 52], back: [44, 52], right: [32, 52], left: [40, 52], top: [36, 48], bottom: [40, 48] };

// Builds the 12-box dual-layer figure: 6 base boxes + 6 overlays inflated by
// 0.25 units on every side.
function buildPlayerModel() {
  const model = new THREE.Group();
  const boxes = [
    // Head / Hat: 8x8x8 centered at y=24 (hat region starts at u=32).
    createBox(8, 8, 8, 0, 24, 0, HEAD),
    createBox(8.5, 8.5, 8.5, 0, 24, 0, HAT),
    // Body / Jacket: 8 wide x 12 tall x 4 deep, centered at y=12.
    createBox(8, 12, 4, 0, 12, 0, BODY),
    createBox(8.5, 12.5, 4.5, 0, 12, 0, JACKET),
    // Arms / Sleeves: 4x12x4 at x=±6 (player's left arm is +x).
    createBox(4, 12, 4, -6, 12, 0, R_ARM),
    createBox(4.5, 12.5, 4.5, -6, 12, 0, R_SLEEVE),
    createBox(4, 12, 4, 6, 12, 0, L_ARM),
    createBox(4.5, 12.5, 4.5, 6, 12, 0, L_SLEEVE),
    // Legs / Pants: 4x12x4 at x=±2, from y=0 up to y=12.
    createBox(4, 12, 4, -2, 0, 0, R_LEG),
    createBox(4.5, 12.5, 4.5, -2, 0, 0, R_PANTS),
    createBox(4, 12, 4, 2, 0, 0, L_LEG),
    createBox(4.5, 12.5, 4.5, 2, 0, 0, L_PANTS),
  ];
  for (const box of boxes) model.add(box);
  return model;
}

function applySkin(imageUri) {
  if (!imageUri) return;
  const img = new Image();
  img.onload = () => {
    if (!material) return;
    if (material.map) material.map.dispose();
    const texture = new THREE.Texture(img);
    texture.magFilter = THREE.NearestFilter;
    texture.minFilter = THREE.NearestFilter;
    texture.generateMipmaps = false;
    texture.colorSpace = THREE.SRGBColorSpace;
    texture.needsUpdate = true;
    material.map = texture;
    material.needsUpdate = true;
    skinLoaded.value = true;
  };
  img.onerror = () => {
    skinLoaded.value = false;
  };
  img.src = imageUri;
}

function updateSkin(imageUri) {
  applySkin(imageUri);
}

function resetSkin() {
  skinLoaded.value = false;
  if (material) {
    if (material.map) material.map.dispose();
    material.map = null;
    material.needsUpdate = true;
  }
}

function resize() {
  if (!renderer || !container.value) return;
  const w = container.value.clientWidth;
  const h = container.value.clientHeight;
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
  (uri) => uri && applySkin(uri)
);

watch(
  () => props.defaultSkinUri,
  (uri) => uri && applySkin(uri)
);

defineExpose({ updateSkin, resetSkin });

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
  camera.position.set(0, 16, 38);
  camera.lookAt(0, 13, 0);

  // Soft key light + teal rim so the figure reads against the dark background.
  const key = new THREE.DirectionalLight(0xffffff, 1.5);
  key.position.set(10, 30, 20);
  scene.add(key);
  const fill = new THREE.DirectionalLight(0x9ad7d4, 0.45);
  fill.position.set(-15, 10, -12);
  scene.add(fill);
  scene.add(new THREE.AmbientLight(0xffffff, 0.35));

  // Soft ground shadow so the figure doesn't float.
  const shadow = new THREE.Mesh(
    new THREE.CircleGeometry(13, 48),
    new THREE.MeshBasicMaterial({ color: 0x000000, transparent: true, opacity: 0.25 })
  );
  shadow.rotation.x = -Math.PI / 2;
  shadow.position.y = -0.02;
  scene.add(shadow);

  material = new THREE.MeshLambertMaterial({ color: 0xffffff, map: null });
  group = buildPlayerModel();
  scene.add(group);
  group.rotation.set(pitch, yaw, 0);

  mount.addEventListener('pointerdown', onPointerDown);
  window.addEventListener('pointermove', onPointerMove);
  window.addEventListener('pointerup', onPointerUp);
  window.addEventListener('resize', resize);

  if (props.imageUri) applySkin(props.imageUri);
  else if (props.defaultSkinUri) applySkin(props.defaultSkinUri);

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
  if (renderer) {
    renderer.dispose();
    renderer = null;
  }
});
</script>
