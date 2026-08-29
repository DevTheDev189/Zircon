<template>
  <div class="h-full flex gap-5 p-5 overflow-hidden">
    <!-- Left: 3D Preview + Actions Card -->
    <div class="w-[380px] min-w-[340px] z-card flex flex-col p-4 bg-[#0e1622]/90 border border-slate-800/80">
      <div class="flex items-center justify-between mb-2.5">
        <div class="flex items-center gap-2 min-w-0">
          <span class="text-xs font-bold uppercase tracking-wider text-slate-400">3D Player Preview</span>
        </div>
        <span
          v-if="selectedSkin?.isActive"
          class="text-[9px] font-extrabold uppercase px-2 py-0.5 rounded-full bg-cyan-500/20 text-cyan-300 border border-cyan-400/40 shadow-[0_0_8px_rgba(71,210,201,0.25)] shrink-0"
        >
          Active Skin
        </span>
      </div>

      <!-- 3D Canvas Container -->
      <div class="flex-1 min-h-0 rounded-xl overflow-hidden bg-[#070b10] border border-slate-800/80 relative shadow-inner">
        <Player3DPreview ref="previewRef" :image-uri="previewUrl" :variant="variant" />
      </div>

      <!-- Controls below Preview -->
      <div class="mt-3.5 flex flex-col gap-2.5">
        <div class="flex items-center justify-between gap-3 bg-[#0a0f16] p-2.5 rounded-xl border border-slate-800/70">
          <span class="text-xs font-semibold text-slate-300">Model Type:</span>
          <select
            v-model="variant"
            class="z-input !w-auto text-xs font-semibold py-1 px-2.5 bg-[#121c27] border-slate-700"
            @change="onVariantChange"
          >
            <option value="classic">Classic (Steve / 4px arms)</option>
            <option value="slim">Slim (Alex / 3px arms)</option>
          </select>
        </div>

        <!-- Apply & Sync Button -->
        <button
          class="z-btn-accent w-full py-2.5 text-xs font-bold rounded-xl flex items-center justify-center gap-2 shadow-lg hover:shadow-cyan-500/25 transition-all"
          :disabled="saving || !previewUrl"
          @click="saveAction"
        >
          <svg v-if="!saving" class="w-4 h-4" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5">
            <polyline points="20 6 9 17 4 12" />
          </svg>
          <span v-if="saving" class="inline-block w-3.5 h-3.5 border-2 border-[#022623] border-t-transparent rounded-full animate-spin"></span>
          {{
            saving
              ? (session?.username ? 'Applying & Syncing…' : 'Applying…')
              : (selectedSkin?.isActive
                  ? (session?.username ? 'Update & Sync to Minecraft' : 'Update Model Variant')
                  : (session?.username ? 'Apply & Sync to Minecraft' : 'Apply & Set as Active'))
          }}
        </button>

        <!-- Delete History Skin Button -->
        <button
          v-if="canDelete"
          class="z-btn-danger w-full py-1.5 text-xs font-semibold rounded-xl"
          @click="deleteAction"
        >
          Delete from Saved Skins
        </button>
      </div>

      <!-- Status Notice -->
      <p v-if="statusText" class="mt-2 text-center text-[11px] font-medium text-slate-400 min-h-[18px]">
        {{ statusText }}
      </p>
    </div>

    <!-- Right: Saved Skins & Gallery Grid -->
    <div class="flex-1 min-w-0 flex flex-col">
      <!-- Section Header -->
      <div class="flex items-center justify-between mb-3.5">
        <div class="flex items-center gap-2.5">
          <span class="text-white font-bold text-base">Saved Skins</span>
          <span class="px-2.5 py-0.5 rounded-full text-[10px] font-bold bg-cyan-500/15 text-cyan-300 border border-cyan-500/30 font-mono">
            {{ skins.length }}
          </span>
        </div>

        <div class="flex items-center gap-2">
          <button
            v-if="session?.uuid"
            class="z-btn-ghost text-xs px-3 py-1.5 rounded-xl font-semibold flex items-center gap-1.5 hover:text-cyan-300"
            title="Download current skin from your Minecraft account"
            :disabled="syncingMojang"
            @click="syncMojangSkin"
          >
            <svg class="w-3.5 h-3.5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
              <path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4" />
              <polyline points="7 10 12 15 17 10" />
              <line x1="12" y1="15" x2="12" y2="3" />
            </svg>
            {{ syncingMojang ? 'Syncing…' : 'Sync Mojang Skin' }}
          </button>

          <button
            class="z-btn-accent text-xs px-3.5 py-1.5 rounded-xl font-bold flex items-center gap-1.5"
            @click="addSkin"
          >
            <svg class="w-3.5 h-3.5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5">
              <line x1="12" y1="5" x2="12" y2="19" />
              <line x1="5" y1="12" x2="19" y2="12" />
            </svg>
            Import PNG
          </button>

          <button
            class="z-btn-ghost text-xs px-2.5 py-1.5 rounded-xl font-semibold"
            title="Refresh skin list"
            @click="refreshGallery"
          >
            ⟳
          </button>
        </div>
      </div>

      <!-- Gallery Grid -->
      <div class="flex-1 min-h-0 overflow-y-auto pr-1">
        <div class="grid grid-cols-2 md:grid-cols-3 lg:grid-cols-4 gap-3.5">
          <!-- Add Skin Tile -->
          <button
            class="zircon-drop-zone p-4 text-center transition-all flex flex-col items-center justify-center min-h-[160px] gap-2.5 cursor-pointer group"
            title="Import a custom Minecraft skin (.png)"
            @click="addSkin"
          >
            <div class="w-10 h-10 rounded-xl bg-cyan-500/10 border border-cyan-500/30 flex items-center justify-center text-cyan-300 group-hover:scale-110 transition-transform shadow-[0_0_12px_rgba(71,210,201,0.15)]">
              <svg class="w-5 h-5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5">
                <line x1="12" y1="5" x2="12" y2="19" />
                <line x1="5" y1="12" x2="19" y2="12" />
              </svg>
            </div>
            <div class="text-xs font-bold text-cyan-300">Import Skin</div>
            <span class="text-[10px] text-slate-500 font-mono">64x64 PNG</span>
          </button>

          <!-- Skin Tiles -->
          <div
            v-for="skin in skins"
            :key="skin.id"
            class="relative z-card p-3 flex flex-col justify-between cursor-pointer transition-all duration-200 group"
            :class="
              isSelected(skin)
                ? 'border-cyan-400 ring-1 ring-cyan-400/60 shadow-[0_0_16px_rgba(71,210,201,0.25)] bg-[#111c29]'
                : 'border-slate-800/80 bg-[#0e1722]/80 hover:border-slate-700 hover:bg-[#121d2b]'
            "
            @click="selectSkin(skin)"
          >
            <!-- Top Status Badge -->
            <div class="flex items-center justify-between mb-2">
              <span
                v-if="skin.isActive"
                class="text-[9px] font-black text-[#022623] bg-gradient-to-r from-[#5adfd5] to-[#47d2c9] rounded px-1.5 py-0.5 shadow-[0_0_8px_rgba(71,210,201,0.4)]"
              >
                ACTIVE
              </span>
              <span v-else class="text-[9px] font-mono text-slate-500 capitalize">
                {{ skin.variant || 'classic' }}
              </span>

              <button
                v-if="!skin.isActive"
                class="opacity-0 group-hover:opacity-100 text-slate-400 hover:text-red-400 p-1 rounded transition-opacity"
                title="Delete this skin"
                @click.stop="deleteSingleSkin(skin)"
              >
                <svg class="w-3.5 h-3.5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                  <path d="M3 6h18" />
                  <path d="M19 6v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6" />
                </svg>
              </button>
            </div>

            <!-- Skin Face Preview Box -->
            <div class="bg-[#070b10] rounded-xl p-3 mb-2.5 border border-slate-800/80 flex items-center justify-center">
              <img
                :src="skin.faceUrl || skin.dataUrl"
                class="w-16 h-16 image-render-pixel object-contain drop-shadow-[0_2px_8px_rgba(0,0,0,0.5)]"
                alt=""
              />
            </div>

            <!-- Skin Name & Inline Rename -->
            <div class="flex items-center justify-between min-h-[26px]">
              <!-- Editing Input -->
              <div
                v-if="editingSkinId === skin.id"
                class="flex items-center gap-1 w-full"
                @click.stop
              >
                <input
                  :id="`skin-rename-${skin.id}`"
                  v-model="editingName"
                  class="z-input !py-0.5 !px-1.5 text-xs font-bold w-full bg-[#162232] border-cyan-500/70 focus:ring-1 focus:ring-cyan-400"
                  placeholder="skin_name.png"
                  @keyup.enter="saveRename(skin)"
                  @keyup.esc="cancelRename"
                  @blur="saveRename(skin)"
                />
              </div>

              <!-- Static Display with Edit Trigger -->
              <div
                v-else
                class="flex items-center justify-between w-full group/name gap-1"
                :title="'Click to rename ' + skin.label"
                @click.stop="startRename(skin)"
              >
                <div class="text-xs font-bold text-white truncate flex-1 min-w-0 group-hover/name:text-cyan-300 transition-colors">
                  {{ skin.label }}
                </div>
                <button
                  class="opacity-0 group-hover/name:opacity-100 text-slate-400 hover:text-cyan-300 p-0.5 rounded transition-opacity shrink-0"
                  title="Rename filename"
                >
                  <svg class="w-3 h-3" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                    <path d="M11 4H4a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h14a2 2 0 0 0 2-2v-7" />
                    <path d="M18.5 2.5a2.121 2.121 0 0 1 3 3L12 15l-4 1 1-4 9.5-9.5z" />
                  </svg>
                </button>
              </div>
            </div>
          </div>
        </div>

        <div v-if="skins.length === 0" class="text-slate-500 text-sm py-12 text-center flex flex-col items-center gap-3">
          <div class="w-12 h-12 rounded-2xl bg-cyan-500/10 border border-cyan-500/30 flex items-center justify-center text-cyan-300">
            <svg class="w-6 h-6" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8">
              <path d="M20.38 3.46 16 2a4 4 0 0 1-8 0L3.62 3.46a2 2 0 0 0-1.34 2.23l.58 3.47a1 1 0 0 0 .99.84H6v10a2 2 0 0 0 2 2h8a2 2 0 0 0 2-2V10h2.15a1 1 0 0 0 .99-.84l.58-3.47a2 2 0 0 0-1.34-2.23z" />
            </svg>
          </div>
          <div class="text-white font-bold text-sm">No custom skins saved yet</div>
          <div class="text-slate-400 text-xs max-w-xs">
            Import a PNG skin file or sync your official Minecraft skin using the buttons above.
          </div>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup>
import { computed, nextTick, onMounted, ref, watch } from 'vue';
import Player3DPreview from '../components/Player3DPreview.vue';
import { api, createDefaultSteveDataUrl, getCachedActiveSkin, onSkinUpdated, pickFile, PNG_FILTER, skinFaceDataUrl } from '../lib/api';

const props = defineProps({
  session: { type: Object, default: null },
});

const initialSkin = getCachedActiveSkin();
const previewRef = ref(null);
const previewUrl = ref(initialSkin?.dataUrl || null);
const skins = ref([]);
const statusText = ref('');
const variant = ref(initialSkin?.variant || 'classic');
const selectedSkinId = ref(initialSkin ? 'active_skin' : null);
const saving = ref(false);
const syncingMojang = ref(false);

const editingSkinId = ref(null);
const editingName = ref('');

let unlistenSkin = null;

const selectedSkin = computed(() => {
  return skins.value.find((s) => s.id === selectedSkinId.value) || null;
});

const canDelete = computed(() => {
  if (!selectedSkin.value) return false;
  return !selectedSkin.value.isActive && selectedSkin.value.filename;
});

function isSelected(skin) {
  return skin.id === selectedSkinId.value;
}

onMounted(async () => {
  await refreshGallery();
  try {
    unlistenSkin = await onSkinUpdated(() => {
      refreshGallery();
    });
  } catch (err) {
    console.warn('Skin update listener unavailable:', err);
  }
});

watch(
  () => props.session,
  () => {
    refreshGallery();
  }
);

async function refreshGallery() {
  try {
    const rawList = [];
    const seenDataUrls = new Set();

    // 1. Fetch current active skin
    const active = await api.getActiveSkin();
    let hasActive = false;
    if (active && active.dataUrl) {
      hasActive = true;
      seenDataUrls.add(active.dataUrl);
      const activeLabel = (active.name && active.name !== 'active_skin.png') ? active.name : 'Active Skin.png';
      rawList.push({
        id: 'active_skin',
        label: activeLabel,
        filename: null,
        dataUrl: active.dataUrl,
        variant: active.variant || 'classic',
        isActive: true,
      });
    }

    // 2. Fetch history skins
    const history = (await api.getSkinHistory()) || [];
    let savedIndex = 1;
    for (let i = 0; i < history.length; i++) {
      const h = history[i];
      if (!h.dataUrl || seenDataUrls.has(h.dataUrl)) continue;
      seenDataUrls.add(h.dataUrl);

      let cleanName = h.name ? h.name.replace(/^\d+-/, '') : `skin_${savedIndex}.png`;
      if (cleanName === 'active_skin.png' || cleanName === 'active_skin') {
        cleanName = `Saved Skin ${savedIndex}.png`;
      }
      savedIndex++;

      rawList.push({
        id: `history_${h.name || i}`,
        label: cleanName,
        filename: h.name,
        dataUrl: h.dataUrl,
        variant: h.variant || 'classic',
        isActive: false,
      });
    }

    // If no custom skin exists at all, provide the default Zircon Steve preset
    if (rawList.length === 0) {
      const defaultDataUrl = createDefaultSteveDataUrl();
      rawList.push({
        id: 'default_steve',
        label: 'Zircon Steve.png',
        filename: null,
        dataUrl: defaultDataUrl,
        variant: 'classic',
        isActive: true,
      });
    }

    // Pre-render 2D front face thumbnails
    const enriched = await Promise.all(
      rawList.map(async (skin) => {
        let face = null;
        try {
          face = await skinFaceDataUrl(skin.dataUrl);
        } catch {
          face = null;
        }
        return {
          ...skin,
          faceUrl: face || skin.dataUrl,
        };
      })
    );

    skins.value = enriched;

    // Pick selected skin
    const currentActive = enriched.find((s) => s.isActive);
    if (currentActive && (!selectedSkinId.value || selectedSkinId.value === 'active_skin')) {
      selectSkin(currentActive);
    } else if (enriched.length && !selectedSkin.value) {
      selectSkin(enriched[0]);
    }
  } catch (err) {
    console.warn('Failed to load skins gallery:', err);
  }
}

function selectSkin(skin) {
  selectedSkinId.value = skin.id;
  previewUrl.value = skin.dataUrl;
  variant.value = skin.variant || 'classic';
  if (previewRef.value) {
    previewRef.value.setVariant(variant.value);
  }
  statusText.value = '';
}

async function onVariantChange() {
  if (previewRef.value) {
    previewRef.value.setVariant(variant.value);
  }
  if (selectedSkin.value?.isActive) {
    try {
      await api.setActiveSkinVariant(variant.value);
      statusText.value = `Model set to ${variant.value === 'slim' ? 'Slim (Alex)' : 'Classic (Steve)'}.`;
    } catch (err) {
      console.warn('Failed to update skin variant:', err);
    }
  }
}

async function saveAction() {
  if (!previewUrl.value || !selectedSkin.value) return;
  saving.value = true;
  const isOnline = !!props.session?.username;
  statusText.value = isOnline ? 'Applying skin & syncing to Minecraft…' : 'Applying skin…';
  try {
    if (selectedSkin.value.filename) {
      await api.activateHistorySkin(selectedSkin.value.filename, variant.value);
    } else {
      await api.setActiveSkinVariant(variant.value);
    }

    if (isOnline) {
      try {
        await api.uploadSkinToMojang(variant.value);
        statusText.value = 'Skin applied & synced to your Minecraft account!';
      } catch (uploadErr) {
        console.warn('Skin applied locally but Mojang upload failed:', uploadErr);
        statusText.value = `Skin applied locally, but Minecraft sync failed: ${uploadErr}`;
      }
    } else {
      statusText.value = 'Skin applied locally (sign in to sync to Minecraft)';
    }
    await refreshGallery();
  } catch (err) {
    console.error('Failed to save skin:', err);
    statusText.value = `Error applying skin: ${err}`;
  } finally {
    saving.value = false;
  }
}

async function syncMojangSkin() {
  if (!props.session?.uuid) return;
  syncingMojang.value = true;
  statusText.value = 'Downloading skin from Mojang…';
  try {
    await api.fetchMojangSkin(props.session.uuid);
    statusText.value = 'Downloaded skin from Mojang!';
    await refreshGallery();
  } catch (err) {
    console.error('Failed to sync skin from Mojang:', err);
    statusText.value = `Sync failed: ${err}`;
  } finally {
    syncingMojang.value = false;
  }
}

async function addSkin() {
  const path = await pickFile(PNG_FILTER);
  if (!path) return;
  try {
    statusText.value = 'Importing skin…';
    await api.saveSkin(path, variant.value);
    statusText.value = 'Skin imported successfully!';
    await refreshGallery();
  } catch (err) {
    console.error('Failed to import skin:', err);
    statusText.value = `Error importing skin: ${err}`;
  }
}

function startRename(skin) {
  editingSkinId.value = skin.id;
  editingName.value = skin.label;
  nextTick(() => {
    const input = document.getElementById(`skin-rename-${skin.id}`);
    if (input) {
      input.focus();
      input.select();
    }
  });
}

function cancelRename() {
  editingSkinId.value = null;
  editingName.value = '';
}

async function saveRename(skin) {
  if (editingSkinId.value !== skin.id) return;
  const rawNewName = editingName.value.trim();
  editingSkinId.value = null;
  if (!rawNewName || rawNewName === skin.label) return;

  try {
    statusText.value = 'Renaming skin…';
    const targetFilename = skin.isActive ? null : skin.filename;
    const updatedName = await api.renameSkin(targetFilename, rawNewName);
    statusText.value = `Renamed to ${updatedName}!`;
    await refreshGallery();
  } catch (err) {
    console.error('Failed to rename skin:', err);
    statusText.value = `Error renaming skin: ${err}`;
  }
}

async function deleteAction() {
  if (!selectedSkin.value?.filename) return;
  await deleteSingleSkin(selectedSkin.value);
}

async function deleteSingleSkin(skin) {
  if (!skin.filename || skin.isActive) return;
  try {
    await api.deleteHistorySkin(skin.filename);
    selectedSkinId.value = null;
    await refreshGallery();
    statusText.value = 'Skin deleted.';
  } catch (err) {
    console.error('Failed to delete skin:', err);
    statusText.value = `Error deleting skin: ${err}`;
  }
}
</script>

<style scoped>
.image-render-pixel {
  image-rendering: pixelated;
}
</style>

