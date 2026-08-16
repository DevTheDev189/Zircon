<template>
  <div class="h-full flex gap-5 p-5 overflow-hidden">
    <!-- Left: preview + actions -->
    <div class="w-[400px] min-w-[340px] z-card flex flex-col">
      <span class="z-label mb-2 text-center">3D Player Preview</span>
      <div class="flex-1 min-h-0 rounded-lg overflow-hidden bg-bg">
        <Player3DPreview ref="previewRef" :image-uri="previewUrl" />
      </div>
      <button class="z-btn-accent w-full py-2.5 mt-3 text-base" @click="saveAction">SAVE</button>
      <div class="flex gap-2 mt-2">
        <button class="z-btn-ghost flex-1" @click="removeSkin">Remove Skin</button>
        <button
          class="z-btn-ghost flex-1"
          :disabled="!session"
          :title="session ? '' : 'Sign in first'"
          @click="fetchMojang"
        >
          Fetch from Mojang
        </button>
      </div>
      <div class="flex gap-2 mt-2 items-center">
        <select v-model="variant" class="z-input flex-1 !w-auto">
          <option value="classic">Classic arms</option>
          <option value="slim">Slim arms (Alex)</option>
        </select>
        <button
          class="z-btn-ghost flex-1"
          :disabled="!session || !hasActiveSkin"
          @click="uploadToMojang"
        >
          Upload to Mojang
        </button>
      </div>
      <p class="z-label mt-3 min-h-[34px] text-center">
        {{ statusText }}
      </p>
    </div>

    <!-- Right: gallery -->
    <div class="flex-1 min-w-0 flex flex-col">
      <div class="flex items-center justify-between mb-3">
        <span class="z-section">Skin Gallery</span>
        <button class="z-btn-ghost text-xs" @click="pickSkin">+ Add Skin</button>
      </div>
      <div class="flex-1 min-h-0 overflow-y-auto pr-1">
        <div class="z-label mb-2">Bundled</div>
        <div class="grid grid-cols-4 gap-3 mb-4">
          <button
            v-for="skin in bundled"
            :key="skin.name"
            class="bg-card border rounded-lg p-2 text-center transition-colors"
            :class="isPreview(skin) ? 'border-accent' : 'border-edge hover:border-[#3d444d]'"
            @click="selectBundled(skin)"
          >
            <img :src="skin.dataUrl" class="w-full aspect-square image-render-pixel" />
            <div class="text-[11px] text-muted mt-1 truncate">{{ skin.name.replace('.png', '') }}</div>
          </button>
        </div>

        <div class="z-label mb-2">Active Skin</div>
        <div class="grid grid-cols-4 gap-3 mb-4">
          <button
            v-if="activeSkin"
            class="bg-card border rounded-lg p-2 text-center transition-colors"
            :class="isPreview(activeSkin) ? 'border-accent' : 'border-edge hover:border-[#3d444d]'"
            @click="previewSaved(activeSkin, 'active_skin.png')"
          >
            <img :src="activeSkin.dataUrl" class="w-full aspect-square image-render-pixel" />
            <div class="text-[11px] text-muted mt-1">active</div>
          </button>
          <div v-else class="text-xs text-muted col-span-4">No custom skin yet.</div>
        </div>

        <div class="z-label mb-2">History</div>
        <div v-if="history.length" class="grid grid-cols-4 gap-3">
          <button
            v-for="skin in history"
            :key="skin.name"
            class="bg-card border rounded-lg p-2 text-center transition-colors"
            :class="isPreview(skin) ? 'border-accent' : 'border-edge hover:border-[#3d444d]'"
            @click="previewSaved(skin, skin.name)"
          >
            <img :src="skin.dataUrl" class="w-full aspect-square image-render-pixel" />
            <div class="text-[10px] text-muted mt-1 truncate">{{ skin.name }}</div>
          </button>
        </div>
        <div v-else class="text-xs text-muted">No saved skins yet.</div>
      </div>
    </div>
  </div>
</template>

<script setup>
import { computed, onMounted, ref } from 'vue';
import Player3DPreview from '../components/Player3DPreview.vue';
import { api, PNG_FILTER, pickFile } from '../lib/api';

const props = defineProps({
  session: { type: Object, default: null },
});

const previewRef = ref(null);
const previewUrl = ref(null);
const bundled = ref([]);
const activeSkin = ref(null);
const history = ref([]);
const statusText = ref('Preview a skin, then press SAVE to activate it.');
const variant = ref('classic');

// Where the preview came from, so SAVE knows what to persist.
const previewSource = ref(null); // { kind: 'bundled', key } | { kind: 'file', path } | { kind: 'saved' }

const hasActiveSkin = computed(() => !!activeSkin.value);

function isPreview(skin) {
  return previewUrl.value === skin.dataUrl;
}

async function refreshGallery() {
  try {
    bundled.value = await api.getBundledSkins();
  } catch {
    bundled.value = [];
  }
  try {
    activeSkin.value = await api.getActiveSkin();
  } catch {
    activeSkin.value = null;
  }
  try {
    history.value = await api.getSkinHistory();
  } catch {
    history.value = [];
  }
}

function selectBundled(skin) {
  previewUrl.value = skin.dataUrl;
  previewSource.value = { kind: 'bundled', key: `bundled:${skin.name}` };
  statusText.value = `Previewing bundled skin '${skin.name}'. Press SAVE to activate.`;
}

function previewSaved(skin, label) {
  previewUrl.value = skin.dataUrl;
  previewSource.value = { kind: 'saved' };
  statusText.value = `Previewing '${label}'. It is already saved.`;
}

async function pickSkin() {
  const path = await pickFile(PNG_FILTER);
  if (!path) return;
  // WebView2 can't read arbitrary local files, so the Rust side saves the
  // picked file directly (active + history) and returns the stored skin.
  try {
    await api.saveSkin(path);
    statusText.value = 'Saved skin from file.';
    await refreshGallery();
    const active = await api.getActiveSkin();
    if (active) previewUrl.value = active.dataUrl;
    previewSource.value = { kind: 'saved' };
  } catch (e) {
    statusText.value = `Could not save skin: ${e}`;
  }
}

async function saveAction() {
  if (!previewSource.value) {
    statusText.value = 'Select a skin to preview first.';
    return;
  }
  try {
    if (previewSource.value.kind === 'bundled') {
      await api.saveBundledSkin(previewSource.value.key);
      statusText.value = 'Bundled skin activated.';
    } else {
      statusText.value = 'This skin is already saved.';
    }
    await refreshGallery();
  } catch (e) {
    statusText.value = `Save failed: ${e}`;
  }
}

async function removeSkin() {
  await api.removeSkin();
  activeSkin.value = null;
  if (previewSource.value?.kind === 'saved') {
    previewUrl.value = bundled.value[0]?.dataUrl || null;
    previewSource.value = bundled.value[0]
      ? { kind: 'bundled', key: `bundled:${bundled.value[0].name}` }
      : null;
  }
  statusText.value = 'Active skin removed.';
}

async function fetchMojang() {
  if (!props.session?.uuid) {
    statusText.value = 'Sign in first to fetch your Mojang skin.';
    return;
  }
  statusText.value = 'Fetching Mojang skin…';
  try {
    const skin = await api.fetchMojangSkin(props.session.uuid);
    previewUrl.value = skin.dataUrl;
    variant.value = skin.variant || 'classic';
    previewSource.value = { kind: 'saved' };
    statusText.value = `Mojang skin fetched (${skin.variant || 'classic'}) and activated.`;
    await refreshGallery();
  } catch (e) {
    statusText.value = `Could not fetch Mojang skin: ${e}`;
  }
}

async function uploadToMojang() {
  statusText.value = 'Uploading skin to Mojang…';
  try {
    await api.uploadSkinToMojang(variant.value);
    statusText.value = `Skin uploaded (${variant.value} arms). It may take a moment to appear in-game.`;
  } catch (e) {
    statusText.value = `Upload failed: ${e}`;
  }
}

onMounted(async () => {
  await refreshGallery();
  if (activeSkin.value) {
    previewUrl.value = activeSkin.value.dataUrl;
    previewSource.value = { kind: 'saved' };
  } else if (bundled.value.length) {
    previewUrl.value = bundled.value[0].dataUrl;
    previewSource.value = { kind: 'bundled', key: `bundled:${bundled.value[0].name}` };
  }
});
</script>

<style scoped>
.image-render-pixel {
  image-rendering: pixelated;
}
</style>
