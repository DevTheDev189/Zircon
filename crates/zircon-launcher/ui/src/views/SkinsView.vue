<template>
  <div class="h-full flex flex-col overflow-hidden transition-all duration-150" :class="activeTab === 'studio' ? 'p-2.5' : 'p-5'">
    <!-- Top Navigation & Controls Bar -->
    <div class="flex items-center justify-between flex-wrap gap-2.5 shrink-0" :class="activeTab === 'studio' ? 'mb-2' : 'mb-4'">
      <!-- Segmented Tab Switcher -->
      <div class="z-segmented-track">
        <button
          type="button"
          class="z-segmented-pill"
          :class="{ 'active': activeTab === 'saved' }"
          @click="activeTab = 'saved'"
        >
          Saved Skins ({{ skins.length }})
        </button>
        <button
          type="button"
          class="z-segmented-pill"
          :class="{ 'active': activeTab === 'library' }"
          @click="onSelectLibraryTab"
        >
          Browse Library
        </button>
        <button
          type="button"
          class="z-segmented-pill"
          :class="{ 'active': activeTab === 'studio' }"
          @click="openStudio(selectedSkin)"
        >
          Paint Studio
        </button>
      </div>

      <div class="flex items-center gap-2">
        <button
          v-if="activeTab === 'saved'"
          class="z-btn-ghost text-xs px-3.5 py-1.5 rounded-xl font-bold flex items-center gap-1.5 border border-slate-700 hover:border-cyan-400 hover:text-cyan-300"
          @click="openStudioNew"
        >
          <svg class="w-3.5 h-3.5 text-cyan-400" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
            <path d="M12 5v14M5 12h14" />
          </svg>
          Create in Studio
        </button>

        <button
          v-if="activeTab === 'saved'"
          class="z-btn-accent text-xs px-3.5 py-1.5 rounded-xl font-bold flex items-center gap-1.5 shadow-sm hover:shadow-cyan-500/25"
          @click="addSkin"
        >
          <svg class="w-3.5 h-3.5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5">
            <line x1="12" y1="5" x2="12" y2="19" />
            <line x1="5" y1="12" x2="19" y2="12" />
          </svg>
          Import PNG
        </button>

        <button
          v-if="activeTab !== 'studio'"
          class="z-btn-ghost text-xs px-2.5 py-1.5 rounded-xl font-semibold"
          :title="activeTab === 'saved' ? 'Refresh saved skins' : 'Refresh library'"
          @click="activeTab === 'saved' ? refreshGallery() : reloadCurrentCommunityPage()"
        >
          <svg class="w-3.5 h-3.5 text-slate-400 hover:text-white" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
            <path d="M21.5 2v6h-6M21.34 15.57a10 10 0 1 1-.57-8.38l5.67-5.67" />
          </svg>
        </button>
      </div>
    </div>

    <!-- TAB 3: PAINT STUDIO (Full Width & Height Workspace) -->
    <div v-if="activeTab === 'studio'" class="flex-1 min-h-0">
      <SkinStudio
        :studio="studioInstance"
        :session="session"
        @skin-saved="onStudioSkinSaved"
        @skin-applied="onStudioSkinApplied"
      />
    </div>

    <!-- GALLERY VIEWS: SAVED SKINS & BROWSE LIBRARY (Split with 3D Preview Card on Left) -->
    <div v-else class="flex-1 min-h-0 flex gap-5 overflow-hidden">
      <!-- Left: 3D Preview + Actions Card -->
      <div class="w-[340px] min-w-[310px] z-card flex flex-col p-3.5 bg-[#0e1622]/90 border border-slate-800/80 rounded-2xl shrink-0">
        <div class="flex items-center justify-between mb-2">
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
        <div class="mt-3 flex flex-col gap-2.5">
          <div class="flex items-center justify-between gap-3 bg-[#0a0f16] p-2.5 rounded-xl border border-slate-800/70">
            <span class="text-xs font-semibold text-slate-300">Model Type:</span>
            <select
              v-model="variant"
              class="z-input !w-auto text-xs font-semibold py-1 px-2.5 bg-[#121c27] border-slate-700 rounded-lg"
              @change="onVariantChange"
            >
              <option value="classic">Classic (Steve / 4px arms)</option>
              <option value="slim">Slim (Alex / 3px arms)</option>
            </select>
          </div>

          <!-- Apply & Sync Button -->
          <button
            class="z-btn-accent w-full py-2.5 px-3 text-xs font-bold rounded-xl flex items-center justify-center gap-2 shadow-lg hover:shadow-cyan-500/25 transition-all"
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

          <!-- Edit in Paint Studio Button -->
          <button
            class="z-btn-ghost w-full py-2 px-3 text-xs font-bold rounded-xl flex items-center justify-center gap-1.5 border border-slate-700/70 hover:border-cyan-400 hover:text-cyan-300 transition-colors"
            @click="openStudio(selectedSkin)"
          >
            <svg class="w-3.5 h-3.5 text-cyan-400" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
              <path d="M17 3a2.828 2.828 0 1 1 4 4L7.5 20.5 2 22l1.5-5.5L17 3z" />
            </svg>
            Edit in Paint Studio
          </button>

          <!-- Save Library/Cloned Skin to Gallery Button -->
          <button
            v-if="selectedSkin?.isLibrary"
            class="z-btn-ghost w-full py-2 px-3 text-xs font-bold rounded-xl flex items-center justify-center gap-1.5"
            @click="saveLibrarySkinToGallery"
          >
            <svg class="w-3.5 h-3.5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
              <path d="M19 21H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h11l5 5v11a2 2 0 0 1-2 2z" />
              <polyline points="17 21 17 13 7 13 7 21" />
              <polyline points="7 3 7 8 15 8" />
            </svg>
            Save to My Skins
          </button>

          <!-- Delete History Skin Button -->
          <button
            v-if="canDelete"
            class="z-btn-danger w-full py-1.5 px-3 text-xs font-semibold rounded-xl"
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

      <!-- Right: Tabs + Skins Grid -->
      <div class="flex-1 min-w-0 flex flex-col overflow-hidden">

      <!-- TAB 1: SAVED SKINS -->
      <div v-if="activeTab === 'saved'" class="flex-1 min-h-0 overflow-y-auto pr-2 pb-2">
        <div class="grid grid-cols-2 lg:grid-cols-3 gap-5 p-1">
          <!-- Add Skin Tile -->
          <button
            class="zircon-drop-zone p-5 text-center transition-all flex flex-col items-center justify-center min-h-[220px] gap-3 cursor-pointer group rounded-2xl"
            title="Import a custom Minecraft skin (.png)"
            @click="addSkin"
          >
            <div class="w-12 h-12 rounded-2xl bg-cyan-500/10 border border-cyan-500/30 flex items-center justify-center text-cyan-300 group-hover:scale-110 transition-transform shadow-[0_0_16px_rgba(71,210,201,0.18)]">
              <svg class="w-6 h-6" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5">
                <line x1="12" y1="5" x2="12" y2="19" />
                <line x1="5" y1="12" x2="19" y2="12" />
              </svg>
            </div>
            <div class="text-xs font-bold text-cyan-300">Import Custom Skin</div>
            <span class="text-[10px] text-slate-500 font-mono">64x64 PNG file</span>
          </button>

          <!-- Skin Tiles with 3D Isometric Previews -->
          <div
            v-for="skin in skins"
            :key="skin.id"
            class="relative z-card p-3.5 flex flex-col justify-between cursor-pointer transition-all duration-200 group rounded-2xl min-h-[220px]"
            :class="
              isSelected(skin)
                ? 'border-cyan-400 ring-1 ring-cyan-400/60 shadow-[0_0_20px_rgba(71,210,201,0.25)] bg-[#111c29]'
                : 'border-slate-800/80 bg-[#0e1722]/80 hover:border-slate-700 hover:bg-[#121d2b]'
            "
            @click="selectSkin(skin)"
          >
            <!-- Top Status Badge -->
            <div class="flex items-center justify-between mb-2">
              <span
                v-if="skin.isActive"
                class="text-[9px] font-black text-accent-ink bg-gradient-to-r from-accent-bright to-accent rounded px-2 py-0.5 shadow-[0_0_8px_var(--color-accent-glow)]"
              >
                ACTIVE
              </span>
              <span
                v-else-if="skin.isPreset"
                class="text-[9px] font-bold text-cyan-300 bg-cyan-950/60 border border-cyan-500/40 rounded px-1.5 py-0.5"
              >
                DEFAULT
              </span>
              <span
                v-else
                class="text-[9px] font-mono text-slate-500 uppercase px-1.5 py-0.5 bg-slate-900 rounded border border-slate-800"
              >
                {{ skin.variant || 'classic' }}
              </span>

              <div class="flex items-center gap-1">
                <button
                  class="opacity-0 group-hover:opacity-100 text-slate-400 hover:text-cyan-300 p-0.5 rounded transition-opacity"
                  title="Edit in Paint Studio"
                  @click.stop="openStudio(skin)"
                >
                  <svg class="w-3.5 h-3.5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                    <path d="M17 3a2.828 2.828 0 1 1 4 4L7.5 20.5 2 22l1.5-5.5L17 3z" />
                  </svg>
                </button>

                <button
                  v-if="!skin.isActive && skin.filename"
                  class="opacity-0 group-hover:opacity-100 text-slate-500 hover:text-red-400 p-0.5 rounded transition-opacity"
                  title="Delete this skin"
                  @click.stop="deleteSingleSkin(skin)"
                >
                  <svg class="w-3.5 h-3.5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                    <path d="M3 6h18" />
                    <path d="M19 6v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6" />
                  </svg>
                </button>
              </div>
            </div>

            <!-- 3D Isometric Character Preview Box -->
            <div class="bg-[#070b10] rounded-xl p-2 mb-2.5 border border-slate-800/80 flex items-center justify-center min-h-[135px]">
              <img
                :src="skin.renderUrl || skin.dataUrl"
                class="h-32 object-contain drop-shadow-[0_4px_14px_rgba(0,0,0,0.65)]"
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
                  v-if="!skin.isPreset"
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
            Import a PNG skin file or select from the Browse Library tab above.
          </div>
        </div>
      </div>

      <!-- TAB 2: BROWSE LIBRARY (Clone by Username & Community Skins) -->
      <div v-else-if="activeTab === 'library'" class="flex-1 min-h-0 overflow-y-auto pr-2 pb-2 flex flex-col gap-5">
        <!-- 1. Clone by Minecraft Username -->
        <div class="bg-[#070b10] border border-slate-800/90 rounded-2xl p-5 shadow-inner">
          <div class="z-section mb-1 text-white font-bold text-sm">Clone Player Skin by Username</div>
          <p class="text-xs text-slate-400 mb-3.5 leading-relaxed">
            Enter any Minecraft player or creator's username to look up and clone their active skin directly.
          </p>
          <div class="flex gap-3">
            <input
              v-model="cloneUsername"
              class="z-input flex-1 text-xs py-2 px-3"
              placeholder="e.g. Notch, Jeb_, Technoblade, MumboJumbo..."
              @keydown.enter="cloneSkinByUsername"
            />
            <button
              class="z-btn-accent text-xs px-5 py-2 rounded-xl font-bold shrink-0 shadow-md hover:shadow-cyan-500/25"
              :disabled="cloning || !cloneUsername.trim()"
              @click="cloneSkinByUsername"
            >
              <span v-if="cloning" class="inline-flex items-center gap-1.5">
                <span class="inline-block w-3 h-3 border-2 border-[#022623] border-t-transparent rounded-full animate-spin"></span>
                Fetching…
              </span>
              <span v-else>Grab Skin</span>
            </button>
          </div>
        </div>

        <!-- 2. Community Skin Library from MineSkin V2 -->
        <div class="bg-[#070b10] border border-slate-800/90 rounded-2xl p-5 shadow-inner">
          <div class="flex items-center justify-between mb-4 flex-wrap gap-2.5">
            <div>
              <div class="z-section text-white font-bold text-sm">Community Skin Library</div>
              <p class="text-xs text-slate-400 mt-0.5">Explore live public Minecraft community skins.</p>
            </div>
            <div class="flex items-center gap-2">
              <button
                class="z-btn-ghost text-xs px-3 py-1.5 rounded-xl font-bold flex items-center gap-1"
                :disabled="communityLoading || currentPageIndex <= 0"
                @click="onPrevPage"
              >
                ← Prev
              </button>
              <span class="text-xs font-mono text-cyan-300 font-bold bg-slate-900 px-2.5 py-1 rounded-xl border border-slate-800">
                Page {{ currentPageIndex + 1 }}
              </span>
              <button
                class="z-btn-ghost text-xs px-3 py-1.5 rounded-xl font-bold flex items-center gap-1"
                :disabled="communityLoading || !nextCursor"
                @click="onNextPage"
              >
                Next →
              </button>
            </div>
          </div>

          <!-- Loading State -->
          <div v-if="communityLoading" class="py-16 text-center text-slate-400 text-xs flex flex-col items-center gap-3">
            <span class="inline-block w-6 h-6 border-2 border-cyan-400 border-t-transparent rounded-full animate-spin"></span>
            <span>Loading community skins…</span>
          </div>

          <!-- Error / Empty State -->
          <div v-else-if="communitySkins.length === 0" class="py-12 text-center text-slate-400 text-xs leading-relaxed">
            No community skins available right now. You can clone any skin above by typing a player username!
          </div>

          <!-- Community Grid with 3D Isometric Previews (2 at default window, 3-4 when fullscreen/wide) -->
          <div v-else class="grid grid-cols-2 2xl:grid-cols-3 min-[1900px]:grid-cols-4 gap-5">
            <div
              v-for="skin in communitySkins"
              :key="skin.id"
              class="relative z-card p-4 flex flex-col justify-between cursor-pointer transition-all duration-200 group border-slate-800/80 bg-[#0e1722]/80 hover:border-cyan-500/60 hover:bg-[#121d2b] rounded-2xl min-h-[235px]"
              :class="{
                'border-cyan-400 ring-1 ring-cyan-400/60 shadow-[0_0_20px_rgba(71,210,201,0.25)] bg-[#111c29]': isSelected(skin)
              }"
              @click="selectCommunitySkin(skin)"
            >
              <div class="flex items-center justify-between mb-2.5">
                <span class="text-[9px] font-bold text-cyan-300 bg-cyan-950/60 border border-cyan-500/40 rounded px-2 py-0.5">
                  Community
                </span>
                <span class="text-[9px] font-mono text-slate-400 uppercase px-2 py-0.5 bg-slate-900 rounded border border-slate-800">
                  {{ skin.variant || 'classic' }}
                </span>
              </div>

              <!-- 3D Isometric Character Render Box -->
              <div class="bg-[#070b10] rounded-xl p-3 mb-3 border border-slate-800/80 flex items-center justify-center min-h-[140px]">
                <img
                  :src="skin.renderUrl || skin.textureUrl"
                  class="h-32 object-contain drop-shadow-[0_4px_14px_rgba(0,0,0,0.65)]"
                  alt=""
                />
              </div>

              <!-- Skin Details & Select -->
              <div class="flex items-center justify-between gap-2 pt-0.5">
                <div class="min-w-0 flex-1">
                  <div class="text-xs font-bold text-white truncate" :title="skin.name">{{ skin.name }}</div>
                </div>
                <div class="flex items-center gap-1.5 shrink-0">
                  <button
                    class="z-btn-ghost text-xs px-2.5 py-1.5 rounded-xl font-bold hover:text-cyan-300"
                    @click.stop="selectCommunitySkin(skin)"
                  >
                    Preview
                  </button>
                  <button
                    class="z-btn-ghost text-xs px-2.5 py-1.5 rounded-xl font-bold text-cyan-300 hover:text-white border border-cyan-500/40 hover:border-cyan-400 flex items-center gap-1"
                    title="Open and edit in Paint Studio"
                    @click.stop="openCommunityInStudio(skin)"
                  >
                    <svg class="w-3 h-3 text-cyan-400" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                      <path d="M17 3a2.828 2.828 0 1 1 4 4L7.5 20.5 2 22l1.5-5.5L17 3z" />
                    </svg>
                    Edit
                  </button>
                </div>
              </div>
            </div>
          </div>
        </div>
      </div>
    </div>
  </div>
</div>
</template>

<script setup>
import { computed, nextTick, onMounted, ref, watch } from 'vue';
import Player3DPreview from '../components/Player3DPreview.vue';
import SkinStudio from '../components/studio/SkinStudio.vue';
import { createStudioState } from '../components/studio/skinStudioState';
import {
  api,
  createDefaultSteveDataUrl,
  getCachedActiveSkin,
  onSkinUpdated,
  pickFile,
  PNG_FILTER,
  renderSkinIsometric3D,
} from '../lib/api';

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
const selectedSkin = ref(null);
const saving = ref(false);
const activeTab = ref('saved'); // 'saved' | 'library' | 'studio'

// Paint Studio instance
const studioInstance = createStudioState();

function openStudio(skin = null) {
  const target = skin || selectedSkin.value || skins.value[0];
  if (target && target.dataUrl) {
    studioInstance.loadFromDataUrl(target.dataUrl, target.label || 'custom_skin.png', target.variant || 'classic');
  } else {
    studioInstance.loadTemplate('steve');
  }
  activeTab.value = 'studio';
}

function openStudioNew() {
  studioInstance.loadTemplate('steve');
  activeTab.value = 'studio';
}

async function openCommunityInStudio(skin) {
  statusText.value = `Loading '${skin.name}' for Studio…`;
  try {
    const fetched = await api.fetchSkinByUrl(skin.textureUrl, skin.name);
    if (fetched && fetched.dataUrl) {
      await studioInstance.loadFromDataUrl(fetched.dataUrl, `${skin.name}.png`, skin.variant || 'classic');
      activeTab.value = 'studio';
      statusText.value = '';
    }
  } catch (err) {
    console.error('Failed to load community skin into studio:', err);
    statusText.value = `Failed to load skin for studio: ${err}`;
  }
}

async function onStudioSkinSaved() {
  await refreshGallery();
}

async function onStudioSkinApplied() {
  await refreshGallery();
}

// Username cloning
const cloneUsername = ref('');
const cloning = ref(false);

// Community Library API with cursor-based pagination
const communitySkins = ref([]);
const cursorStack = ref([null]); // stack of cursor tokens for previous/next navigation
const currentPageIndex = ref(0);
const nextCursor = ref(null);
const communityLoading = ref(false);

// Inline rename
const editingSkinId = ref(null);
const editingName = ref('');

let unlistenSkin = null;

const canDelete = computed(() => {
  if (!selectedSkin.value) return false;
  return !selectedSkin.value.isActive && !selectedSkin.value.isLibrary && selectedSkin.value.filename;
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

function onSelectLibraryTab() {
  activeTab.value = 'library';
  if (communitySkins.value.length === 0 && !communityLoading.value) {
    cursorStack.value = [null];
    currentPageIndex.value = 0;
    loadCommunitySkins(null);
  }
}

function reloadCurrentCommunityPage() {
  const currentCursor = cursorStack.value[currentPageIndex.value] || null;
  loadCommunitySkins(currentCursor);
}

function onNextPage() {
  if (!nextCursor.value || communityLoading.value) return;
  currentPageIndex.value++;
  cursorStack.value[currentPageIndex.value] = nextCursor.value;
  loadCommunitySkins(nextCursor.value);
}

function onPrevPage() {
  if (currentPageIndex.value <= 0 || communityLoading.value) return;
  currentPageIndex.value--;
  const prevCursor = cursorStack.value[currentPageIndex.value] || null;
  loadCommunitySkins(prevCursor);
}

async function loadCommunitySkins(afterCursor = null) {
  communityLoading.value = true;
  try {
    const res = await api.fetchCommunitySkins(afterCursor);
    if (res && Array.isArray(res.skins)) {
      nextCursor.value = res.nextAfter || null;
      // Pre-compute 3D isometric character body renders for all cards
      const enriched = await Promise.all(
        res.skins.map(async (s) => {
          let render = null;
          try {
            render = await renderSkinIsometric3D(s.textureUrl, s.variant || 'classic');
          } catch {
            render = s.textureUrl;
          }
          return {
            ...s,
            renderUrl: render || s.textureUrl,
            isLibrary: true,
          };
        })
      );
      communitySkins.value = enriched;
    }
  } catch (err) {
    console.error('Failed to load community skins:', err);
    statusText.value = `Failed to load community skins: ${err}`;
  } finally {
    communityLoading.value = false;
  }
}

async function refreshGallery() {
  try {
    const rawList = [];
    const seenDataUrls = new Set();

    // 1. Fetch current active skin (auto-pull from Mojang if logged in and not yet cached)
    let active = await api.getActiveSkin();
    if ((!active || !active.dataUrl) && props.session?.uuid) {
      try {
        active = await api.fetchMojangSkinActive(props.session.uuid);
      } catch (e) {
        console.warn('Failed to auto-pull Mojang skin:', e);
      }
    }
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

    // Always include Zircon-Steve as a built-in default character option
    const defaultDataUrl = createDefaultSteveDataUrl();
    if (!seenDataUrls.has(defaultDataUrl)) {
      rawList.push({
        id: 'default_steve',
        label: 'Zircon-Steve.png',
        filename: null,
        dataUrl: defaultDataUrl,
        variant: 'classic',
        isActive: rawList.length === 0,
        isPreset: true,
      });
    }

    // Pre-render 3D isometric character body renders for each skin card
    const enriched = await Promise.all(
      rawList.map(async (skin) => {
        let render = null;
        try {
          render = await renderSkinIsometric3D(skin.dataUrl, skin.variant || 'classic');
        } catch {
          render = skin.dataUrl;
        }
        return { ...skin, renderUrl: render };
      })
    );

    skins.value = enriched;

    // Maintain selection or default to active
    if (!selectedSkinId.value || !skins.value.some((s) => s.id === selectedSkinId.value)) {
      const activeItem = skins.value.find((s) => s.isActive) || skins.value[0];
      if (activeItem) {
        selectSkin(activeItem);
      }
    }
  } catch (err) {
    console.error('Failed to refresh skins gallery:', err);
    statusText.value = `Error loading skins: ${err}`;
  }
}

function selectSkin(skin) {
  selectedSkinId.value = skin.id;
  selectedSkin.value = skin;
  previewUrl.value = skin.dataUrl;
  variant.value = skin.variant || 'classic';
  statusText.value = '';
}

async function selectCommunitySkin(skin) {
  selectedSkinId.value = skin.id;
  statusText.value = `Loading '${skin.name}' for preview…`;
  try {
    const fetched = await api.fetchSkinByUrl(skin.textureUrl, skin.name);
    if (fetched && fetched.dataUrl) {
      const item = {
        id: skin.id,
        label: `${skin.name}.png`,
        dataUrl: fetched.dataUrl,
        variant: skin.variant || 'classic',
        isLibrary: true,
      };
      selectedSkin.value = item;
      previewUrl.value = fetched.dataUrl;
      variant.value = skin.variant || 'classic';
      statusText.value = `Selected '${skin.name}'. Click 'Apply & Sync' or 'Save to My Skins'.`;
    }
  } catch (err) {
    console.error('Failed to fetch community skin for preview:', err);
    statusText.value = `Failed to preview skin: ${err}`;
  }
}

async function onVariantChange() {
  if (selectedSkin.value) {
    selectedSkin.value.variant = variant.value;
  }
  if (selectedSkin.value?.isActive) {
    try {
      await api.setActiveSkinVariant(variant.value);
      statusText.value = `Model variant updated to ${variant.value}`;
    } catch (err) {
      console.error('Failed to update variant:', err);
    }
  }
}

async function saveAction() {
  if (!previewUrl.value) return;
  saving.value = true;
  statusText.value = 'Applying skin…';
  try {
    const isMojangUser = !!props.session?.username;

    if (selectedSkin.value?.isLibrary || selectedSkin.value?.isPreset) {
      // Save preset/library/cloned skin to history & active
      const bytes = dataUrlToBytes(previewUrl.value);
      await api.saveSkinBytes(selectedSkin.value.label || 'zircon_steve.png', bytes, variant.value);
    } else if (selectedSkin.value?.filename && !selectedSkin.value.isActive) {
      // Activate history skin
      await api.activateHistorySkin(selectedSkin.value.filename, variant.value);
    } else if (selectedSkin.value?.isActive) {
      await api.setActiveSkinVariant(variant.value);
    }

    // Sync to Mojang if logged in
    if (isMojangUser) {
      statusText.value = 'Syncing skin to Mojang account…';
      try {
        await api.uploadSkinToMojang(variant.value);
        statusText.value = 'Skin successfully applied & synced to Minecraft!';
      } catch (uploadErr) {
        console.warn('Mojang skin upload failed:', uploadErr);
        statusText.value = `Applied locally (Mojang sync notice: ${uploadErr})`;
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

async function saveLibrarySkinToGallery() {
  if (!previewUrl.value || !selectedSkin.value) return;
  try {
    statusText.value = 'Saving to your skin gallery…';
    const bytes = dataUrlToBytes(previewUrl.value);
    const skinName = selectedSkin.value.label || 'custom_skin.png';
    await api.saveSkinBytes(skinName, bytes, variant.value);
    statusText.value = `Saved '${skinName}' to your skin gallery!`;
    await refreshGallery();
    activeTab.value = 'saved';
  } catch (err) {
    console.error('Failed to save library skin:', err);
    statusText.value = `Error saving skin: ${err}`;
  }
}

async function cloneSkinByUsername() {
  const username = cloneUsername.value.trim();
  if (!username) return;
  cloning.value = true;
  statusText.value = `Fetching skin for '${username}'…`;
  try {
    const skin = await api.fetchSkinByUsername(username);
    if (skin && skin.dataUrl) {
      const render = await renderSkinIsometric3D(skin.dataUrl, skin.variant || 'classic');
      const clonedItem = {
        id: `cloned_${username}_${Date.now()}`,
        label: `${username}.png`,
        dataUrl: skin.dataUrl,
        renderUrl: render || skin.dataUrl,
        variant: skin.variant || 'classic',
        isLibrary: true,
      };
      selectedSkinId.value = clonedItem.id;
      selectedSkin.value = clonedItem;
      previewUrl.value = skin.dataUrl;
      variant.value = skin.variant || 'classic';
      statusText.value = `Cloned skin for '${username}'! Click 'Apply & Sync' or 'Save to My Skins'.`;
    }
  } catch (err) {
    console.error('Failed to clone skin:', err);
    statusText.value = `Could not find skin for '${username}' (${err})`;
  } finally {
    cloning.value = false;
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
  if (!skin.filename || skin.isActive || skin.isLibrary) return;
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

function dataUrlToBytes(dataUrl) {
  const base64 = dataUrl.split(',')[1];
  const binary = atob(base64);
  const bytes = new Uint8Array(binary.length);
  for (let i = 0; i < binary.length; i++) {
    bytes[i] = binary.charCodeAt(i);
  }
  return Array.from(bytes);
}
</script>

<style scoped>
.image-render-pixel {
  image-rendering: pixelated;
}
</style>
