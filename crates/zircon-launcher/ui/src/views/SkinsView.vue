<template>
  <div class="h-full flex gap-5 p-5 overflow-hidden">
    <!-- Left: preview + actions -->
    <div class="w-[400px] min-w-[340px] z-card flex flex-col">
      <span class="z-label mb-2 text-center">3D Player Preview</span>
      <div class="flex-1 min-h-0 rounded-lg overflow-hidden bg-bg">
        <Player3DPreview ref="previewRef" :image-uri="previewUrl" />
      </div>

      <div class="flex items-center gap-3 mt-3">
        <span class="z-label shrink-0">Arms</span>
        <select v-model="variant" class="z-input flex-1 !w-auto" @change="onVariantChange">
          <option value="classic">Classic (Steve)</option>
          <option value="slim">Slim (Alex)</option>
        </select>
      </div>

      <button
        class="z-btn-accent w-full py-2.5 mt-3 text-base"
        :disabled="saving || !previewUrl"
        @click="saveAction"
      >
        {{ saving ? 'Saving…' : 'SAVE' }}
      </button>
      <button
        class="z-btn-danger w-full py-2 mt-2"
        :disabled="!canDelete"
        :title="canDelete ? '' : 'The active skin and presets cannot be deleted'"
        @click="deleteAction"
      >
        Delete
      </button>

      <p class="z-label mt-3 min-h-[34px] text-center whitespace-pre-line">
        {{ statusText }}
      </p>
    </div>

    <!-- Right: flat gallery -->
    <div class="flex-1 min-w-0 flex flex-col">
      <div class="flex items-center justify-between mb-3">
        <span class="z-section">Skins</span>
        <button
          class="z-btn-ghost text-xs"
          title="Reload presets from the skins folder (~/.mcmanager/skins/presets)"
          @click="refreshGallery"
        >
          ⟳ Refresh
        </button>
      </div>
      <div class="flex-1 min-h-0 overflow-y-auto pr-1">
        <div class="grid grid-cols-4 gap-3">
          <button
            v-for="skin in skins"
            :key="skin.id"
            class="relative bg-card border rounded-lg p-2 text-center transition-colors"
            :class="
              isActive(skin)
                ? 'border-accent shadow-[0_0_0_1px_rgba(71,210,201,0.2)]'
                : isPreviewed(skin)
                  ? 'border-accent/50 hover:border-accent'
                  : 'border-edge hover:border-[#3d444d] hover:bg-[#1a2129]'
            "
            @click="selectSkin(skin)"
          >
            <img
              :src="skin.faceUrl || skin.dataUrl"
              class="w-full aspect-square image-render-pixel"
              alt=""
            />
            <div class="text-[10px] text-muted mt-1 truncate">{{ skin.label }}</div>
            <span
              v-if="isActive(skin)"
              class="absolute top-1.5 right-1.5 text-[8px] font-bold text-[#032b28] bg-accent rounded px-1 py-px"
            >
              ACTIVE
            </span>
          </button>

          <!-- Add a skin from a local PNG file -->
          <button
            class="bg-card border border-dashed border-edge rounded-lg p-2 text-center transition-colors hover:border-accent hover:bg-[#1a2129] aspect-square"
            title="Add a skin from a PNG file"
            @click="addSkin"
          >
            <svg
              class="w-8 h-8 mx-auto text-muted"
              viewBox="0 0 24 24"
              fill="none"
              stroke="currentColor"
              stroke-width="2"
              stroke-linecap="round"
            >
              <path d="M12 5v14M5 12h14" />
            </svg>
            <div class="text-[10px] text-muted mt-1">Add Skin</div>
          </button>
        </div>
        <div v-if="skins.length === 0" class="text-muted text-sm py-6 text-center">
          No skins yet — drop PNGs into the presets folder and press Refresh.
        </div>
      </div>
    </div>
  </div>
</template>

<script setup>
import { computed, onMounted, ref, watch } from 'vue';
import Player3DPreview from '../components/Player3DPreview.vue';
import { api, pickFile, PNG_FILTER, skinFaceDataUrl } from '../lib/api';

const props = defineProps({
  session: { type: Object, default: null },
});

const previewRef = ref(null);
const previewUrl = ref(null);
const skins = ref([]);
const statusText = ref('Your Minecraft skin loads automatically when signed in.');
const variant = ref('classic');
const saving = ref(false);

// What the preview currently shows:
//   { kind: 'active' } | { kind: 'preset', key } | { kind: 'history', name, label }
const previewSource = ref(null);

const canDelete = computed(() => previewSource.value?.kind === 'history');

function isActive(skin) {
  return skin.kind === 'active';
}

function isPreviewed(skin) {
  return previewUrl.value != null && skin.dataUrl === previewUrl.value;
}

// History entries pushed from a previous active skin carry no meaningful name
// (`active_skin`/`previous`); label them with the time they were saved instead.
function historyLabel(name) {
  const base = name.replace('.png', '');
  const match = /^(\d+)-(.*)$/.exec(base);
  const namePart = match ? match[2] : base;
  if (namePart === 'active_skin' || namePart === 'previous') {
    const ts = match ? Number(match[1]) : NaN;
    if (Number.isFinite(ts) && ts > 0) return new Date(ts).toLocaleString();
  }
  return namePart || base;
}

async function refreshGallery() {
  let presets = [];
  let active = null;
  let history = [];
  try {
    presets = await api.getBundledSkins();
  } catch {
    presets = [];
  }
  try {
    active = await api.getActiveSkin();
  } catch {
    active = null;
  }
  try {
    history = await api.getSkinHistory();
  } catch {
    history = [];
  }

  const activeDataUrl = active?.dataUrl;
  const presetUrls = new Set(presets.map((p) => p.dataUrl));

  // One flat list: active first, then presets, then history. Entries that
  // duplicate the active skin, a preset, or an earlier history entry are
  // hidden — the canonical tile wins.
  const list = [];
  if (active) {
    list.push({
      id: 'active',
      name: 'active_skin.png',
      label: 'Active',
      dataUrl: active.dataUrl,
      variant: active.variant || 'classic',
      kind: 'active',
    });
  }
  for (const p of presets) {
    if (p.dataUrl === activeDataUrl) continue;
    list.push({
      id: `preset:${p.name}`,
      name: p.name,
      label: p.name.replace('.png', ''),
      dataUrl: p.dataUrl,
      variant: p.variant || 'classic',
      kind: 'preset',
    });
  }
  const seenHistory = new Set();
  for (const h of history) {
    if (h.dataUrl === activeDataUrl || presetUrls.has(h.dataUrl) || seenHistory.has(h.dataUrl))
      continue;
    seenHistory.add(h.dataUrl);
    list.push({
      id: `history:${h.name}`,
      name: h.name,
      label: historyLabel(h.name),
      dataUrl: h.dataUrl,
      variant: h.variant || 'classic',
      kind: 'history',
    });
  }

  // Face thumbnails (head + hat front layers) rendered locally. Compute them
  // BEFORE assigning to the reactive ref, otherwise the template never sees
  // the enriched objects and falls back to the full 64x64 texture.
  await Promise.all(
    list.map(async (skin) => {
      skin.faceUrl = (await skinFaceDataUrl(skin.dataUrl)) || skin.dataUrl;
    })
  );
  skins.value = list;
}

function selectSkin(skin) {
  previewUrl.value = skin.dataUrl;
  variant.value = skin.variant || 'classic';
  if (skin.kind === 'preset') {
    previewSource.value = { kind: 'preset', key: `bundled:${skin.name}` };
  } else if (skin.kind === 'history') {
    previewSource.value = { kind: 'history', name: skin.name, label: skin.label };
  } else {
    previewSource.value = { kind: 'active' };
  }
  statusText.value = `Previewing ${skin.label}. SAVE activates it and uploads it to Minecraft.`;
}

// Boot / sign-in: fetch the player's Minecraft skin and make it the active one.
async function syncFromMojang() {
  if (!props.session?.uuid) return;
  try {
    const skin = await api.fetchMojangSkinActive(props.session.uuid);
    previewUrl.value = skin.dataUrl;
    variant.value = skin.variant || 'classic';
    previewSource.value = { kind: 'active' };
    statusText.value = 'Fetched your Minecraft skin.';
    await refreshGallery();
  } catch (e) {
    const message = String(e);
    statusText.value = /no custom mojang skin/i.test(message)
      ? 'No custom Minecraft skin yet — pick a preset or create one.'
      : `Could not fetch your Minecraft skin: ${message}`;
  }
}

// Opens a PNG file picker and adds the skin (it becomes the active skin; the
// previous active moves to history). Press SAVE afterwards to upload it.
async function addSkin() {
  const path = await pickFile(PNG_FILTER);
  if (!path) return;
  try {
    await api.saveSkin(path, variant.value);
    statusText.value = 'Added skin — press SAVE to upload it to Minecraft.';
    await refreshGallery();
    const active = skins.value.find((s) => s.kind === 'active');
    if (active) selectSkin(active);
  } catch (e) {
    statusText.value = `Could not add skin: ${e}`;
  }
}

async function saveAction() {
  if (!previewSource.value || !previewUrl.value) {
    statusText.value = 'Select a skin to preview first.';
    return;
  }
  saving.value = true;
  try {
    if (previewSource.value.kind === 'preset') {
      await api.saveBundledSkin(previewSource.value.key, variant.value);
    } else if (previewSource.value.kind === 'history') {
      await api.activateHistorySkin(previewSource.value.name, variant.value);
    }
    // The previewed skin is now the active one — upload it to Minecraft.
    previewSource.value = { kind: 'active' };
    if (props.session?.uuid) {
      await api.uploadSkinToMojang(variant.value);
      statusText.value = `Saved and uploaded (${variant.value} arms).`;
    } else {
      statusText.value = 'Saved locally — sign in to upload to Minecraft.';
    }
    await refreshGallery();
  } catch (e) {
    statusText.value = `Save failed: ${e}`;
  } finally {
    saving.value = false;
  }
}

async function deleteAction() {
  const source = previewSource.value;
  if (source?.kind !== 'history') return;
  try {
    await api.deleteHistorySkin(source.name);
    statusText.value = `Deleted '${source.label}'.`;
    await refreshGallery();
    const active = skins.value.find((s) => s.kind === 'active');
    if (active) {
      selectSkin(active);
    } else if (skins.value.length) {
      selectSkin(skins.value[0]);
    } else {
      previewUrl.value = null;
      previewSource.value = null;
    }
  } catch (e) {
    statusText.value = `Delete failed: ${e}`;
  }
}

// Persists the selected arms model for the active skin.
async function onVariantChange() {
  try {
    await api.setActiveSkinVariant(variant.value);
  } catch (e) {
    statusText.value = `Could not save variant: ${e}`;
  }
}

onMounted(async () => {
  await refreshGallery();
  const active = skins.value.find((s) => s.kind === 'active');
  if (active) {
    selectSkin(active);
  } else if (skins.value.length) {
    selectSkin(skins.value[0]);
  }
  await syncFromMojang();
});

watch(
  () => props.session?.uuid,
  (uuid) => {
    if (uuid) syncFromMojang();
  }
);
</script>

<style scoped>
.image-render-pixel {
  image-rendering: pixelated;
}
</style>
