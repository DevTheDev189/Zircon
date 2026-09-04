<template>
  <div class="w-full h-full flex flex-col bg-[#070b10] overflow-hidden select-none relative">
    <!-- TIER 1: Document & File Action Bar (Height 40px, never wraps or overlaps) -->
    <div class="h-10 px-3.5 bg-[#0a0f16] border-b border-slate-800/80 flex items-center justify-between gap-2.5 shrink-0 z-20">
      <!-- Left: File / Skin Name & Model Variant -->
      <div class="flex items-center gap-2.5 min-w-0">
        <!-- Studio Icon Indicator -->
        <div class="flex items-center gap-1.5 shrink-0 text-cyan-400">
          <svg class="w-4 h-4" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
            <path d="m12 3-1.9 5.8a2 2 0 0 1-1.3 1.3L3 12l5.8 1.9a2 2 0 0 1 1.3 1.3L12 21l1.9-5.8a2 2 0 0 1 1.3-1.3L21 12l-5.8-1.9a2 2 0 0 1-1.3-1.3L12 3z" />
          </svg>
        </div>

        <!-- Skin Name Input -->
        <div class="flex items-center min-w-0">
          <input
            v-model="studio.state.skinName"
            type="text"
            class="bg-[#111c2a] border border-slate-700/80 hover:border-slate-600 focus:border-cyan-400 rounded-lg px-2.5 py-1 text-xs font-bold text-white w-32 sm:w-44 truncate focus:outline-none transition-colors"
            placeholder="skin_name.png"
            title="Skin Filename"
          />
        </div>

        <!-- Variant Selector Dropdown -->
        <select
          v-model="studio.state.variant"
          class="text-xs font-semibold py-1 px-2.5 bg-[#121c27] border border-slate-700/80 hover:border-slate-600 text-slate-200 rounded-lg shrink-0 focus:outline-none focus:border-cyan-400 cursor-pointer"
          title="Minecraft Skin Model Type"
        >
          <option value="classic">Classic (4px arms)</option>
          <option value="slim">Slim (3px arms)</option>
        </select>
      </div>

      <!-- Right: Main Document Actions (New, Import, Export, Save, Apply) -->
      <div class="flex items-center gap-1.5 shrink-0">
        <!-- New Skin Menu Button -->
        <div class="relative">
          <button
            type="button"
            class="z-btn-ghost text-xs px-2.5 py-1 rounded-lg font-semibold flex items-center gap-1.5 hover:text-white"
            @click="showNewModal = !showNewModal"
          >
            <svg class="w-3.5 h-3.5 text-slate-400" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
              <path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z" />
              <polyline points="14 2 14 8 20 8" />
              <line x1="12" y1="18" x2="12" y2="12" />
              <line x1="9" y1="15" x2="15" y2="15" />
            </svg>
            <span>New</span>
          </button>

          <!-- New Skin Dropdown Popover -->
          <div
            v-if="showNewModal"
            class="absolute right-0 mt-1.5 w-48 bg-[#0d1520] border border-slate-700 rounded-xl shadow-2xl p-1 z-50 flex flex-col gap-0.5"
          >
            <button
              type="button"
              class="w-full text-left px-2.5 py-1.5 text-xs text-slate-200 hover:bg-cyan-500/20 hover:text-cyan-300 rounded-lg font-semibold transition-colors"
              @click="loadTemplateAndClose('steve')"
            >
              Steve Template (Classic)
            </button>
            <button
              type="button"
              class="w-full text-left px-2.5 py-1.5 text-xs text-slate-200 hover:bg-cyan-500/20 hover:text-cyan-300 rounded-lg font-semibold transition-colors"
              @click="loadTemplateAndClose('alex')"
            >
              Alex Template (Slim)
            </button>
            <button
              type="button"
              class="w-full text-left px-2.5 py-1.5 text-xs text-slate-200 hover:bg-cyan-500/20 hover:text-cyan-300 rounded-lg font-semibold transition-colors"
              @click="loadTemplateAndClose('blank')"
            >
              Blank Transparent
            </button>
          </div>
        </div>

        <!-- Import PNG -->
        <button
          type="button"
          class="z-btn-ghost text-xs px-2.5 py-1 rounded-lg font-semibold flex items-center gap-1.5 hover:text-white"
          title="Import an existing 64x64 skin PNG"
          @click="importSkinFile"
        >
          <svg class="w-3.5 h-3.5 text-slate-400" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
            <path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4" />
            <polyline points="7 10 12 15 17 10" />
            <line x1="12" y1="15" x2="12" y2="3" />
          </svg>
          <span>Import</span>
        </button>

        <!-- Export PNG -->
        <button
          type="button"
          class="z-btn-ghost text-xs px-2.5 py-1 rounded-lg font-semibold flex items-center gap-1.5 hover:text-white"
          title="Export current skin as PNG"
          @click="exportSkinPng"
        >
          <svg class="w-3.5 h-3.5 text-slate-400" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
            <path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4" />
            <polyline points="17 8 12 3 7 8" />
            <line x1="12" y1="3" x2="12" y2="15" />
          </svg>
          <span>Export</span>
        </button>

        <!-- Save to My Skins Gallery -->
        <button
          type="button"
          class="z-btn-ghost text-xs px-3 py-1 rounded-lg font-bold flex items-center gap-1.5 border border-slate-700/80 hover:border-cyan-400 hover:text-cyan-300"
          :disabled="isSaving"
          @click="saveToGallery"
        >
          <svg class="w-3.5 h-3.5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
            <path d="M19 21H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h11l5 5v11a2 2 0 0 1-2 2z" />
            <polyline points="17 21 17 13 7 13 7 21" />
            <polyline points="7 3 7 8 15 8" />
          </svg>
          <span>Save</span>
        </button>

        <!-- Apply & Sync to Minecraft -->
        <button
          type="button"
          class="z-btn-accent text-xs px-3.5 py-1 rounded-lg font-bold flex items-center gap-1.5 shadow-md hover:shadow-cyan-500/25 shrink-0"
          :disabled="isApplying"
          @click="applyAndSync"
        >
          <span v-if="isApplying" class="inline-block w-3.5 h-3.5 border-2 border-[#022623] border-t-transparent rounded-full animate-spin"></span>
          <svg v-else class="w-3.5 h-3.5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5">
            <polyline points="20 6 9 17 4 12" />
          </svg>
          <span>{{ isApplying ? 'Applying…' : 'Apply & Sync' }}</span>
        </button>
      </div>
    </div>

    <!-- TIER 2: Studio Viewport, History & Dynamic Layout Toolbar (Height 36px) -->
    <div class="h-9 px-3.5 bg-[#070c14] border-b border-slate-800/60 flex items-center justify-between gap-3 shrink-0 z-10">
      <!-- Left: Sidebar Toggle & History (Undo / Redo) -->
      <div class="flex items-center gap-2 shrink-0">
        <!-- Sidebar Toggle Button -->
        <button
          type="button"
          class="px-2.5 py-0.5 text-xs font-semibold rounded-lg border transition-all flex items-center gap-1.5"
          :class="
            isSidebarOpen
              ? 'bg-cyan-500/15 border-cyan-500/40 text-cyan-300 shadow-sm'
              : 'bg-slate-900/80 border-slate-800 text-slate-400 hover:text-white hover:border-slate-700'
          "
          :title="isSidebarOpen ? 'Collapse Tools & Layers Sidebar' : 'Expand Tools & Layers Sidebar'"
          @click="isSidebarOpen = !isSidebarOpen"
        >
          <svg class="w-3.5 h-3.5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
            <rect x="3" y="3" width="18" height="18" rx="2" />
            <line x1="9" y1="3" x2="9" y2="21" />
          </svg>
          <span>Sidebar</span>
        </button>

        <div class="h-4 w-px bg-slate-800" />

        <!-- Undo / Redo Group -->
        <div class="flex items-center bg-[#070b10] p-0.5 rounded-lg border border-slate-800">
          <button
            type="button"
            class="p-1 rounded text-xs font-semibold flex items-center gap-1 transition-all"
            :class="
              studio.state.canUndo
                ? 'text-slate-300 hover:text-white hover:bg-slate-800'
                : 'text-slate-600 cursor-not-allowed opacity-40'
            "
            :disabled="!studio.state.canUndo"
            title="Undo (Ctrl+Z)"
            @click="studio.undo"
          >
            <svg class="w-3.5 h-3.5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
              <polyline points="1 4 1 10 7 10" />
              <path d="M3.51 15a9 9 0 1 0 2.13-9.36L1 10" />
            </svg>
          </button>

          <button
            type="button"
            class="p-1 rounded text-xs font-semibold flex items-center gap-1 transition-all"
            :class="
              studio.state.canRedo
                ? 'text-slate-300 hover:text-white hover:bg-slate-800'
                : 'text-slate-600 cursor-not-allowed opacity-40'
            "
            :disabled="!studio.state.canRedo"
            title="Redo (Ctrl+Y)"
            @click="studio.redo"
          >
            <svg class="w-3.5 h-3.5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
              <polyline points="23 4 23 10 17 10" />
              <path d="M20.49 15a9 9 0 1 1-2.12-9.36L23 10" />
            </svg>
          </button>
        </div>
      </div>

      <!-- Center: Workspace View Switcher (Split, 2D Canvas, 3D Viewport) -->
      <div class="flex items-center bg-[#070b10] p-0.5 rounded-lg border border-slate-800 shrink-0">
        <button
          type="button"
          class="px-2.5 py-0.5 text-[11px] font-bold rounded transition-all flex items-center gap-1.5"
          :class="
            workspaceMode === 'split'
              ? 'bg-cyan-500/25 text-cyan-300 border border-cyan-500/40 shadow-sm'
              : 'text-slate-400 hover:text-white'
          "
          title="Split View: 2D Canvas and 3D Viewport side-by-side"
          @click="workspaceMode = 'split'"
        >
          <svg class="w-3 h-3" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
            <rect x="3" y="3" width="18" height="18" rx="2" />
            <line x1="12" y1="3" x2="12" y2="21" />
          </svg>
          <span>Split</span>
        </button>

        <button
          type="button"
          class="px-2.5 py-0.5 text-[11px] font-bold rounded transition-all flex items-center gap-1.5"
          :class="
            workspaceMode === '2d'
              ? 'bg-cyan-500/25 text-cyan-300 border border-cyan-500/40 shadow-sm'
              : 'text-slate-400 hover:text-white'
          "
          title="2D Canvas: Maximize 2D pixel editor"
          @click="workspaceMode = '2d'"
        >
          <svg class="w-3 h-3" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
            <rect x="3" y="3" width="18" height="18" rx="2" />
            <line x1="3" y1="9" x2="21" y2="9" />
            <line x1="9" y1="21" x2="9" y2="9" />
          </svg>
          <span>2D Canvas</span>
        </button>

        <button
          type="button"
          class="px-2.5 py-0.5 text-[11px] font-bold rounded transition-all flex items-center gap-1.5"
          :class="
            workspaceMode === '3d'
              ? 'bg-cyan-500/25 text-cyan-300 border border-cyan-500/40 shadow-sm'
              : 'text-slate-400 hover:text-white'
          "
          title="3D Model: Maximize 3D character viewport"
          @click="workspaceMode = '3d'"
        >
          <svg class="w-3 h-3" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
            <path d="M21 16V8a2 2 0 0 0-1-1.73l-7-4a2 2 0 0 0-2 0l-7 4A2 2 0 0 0 3 8v8a2 2 0 0 0 1 1.73l7 4a2 2 0 0 0 2 0l7-4A2 2 0 0 0 21 16z" />
            <polyline points="3.27 6.96 12 12.01 20.73 6.96" />
          </svg>
          <span>3D Model</span>
        </button>
      </div>

      <!-- Right: Dynamic Split Presets & Status Indicator -->
      <div class="flex items-center gap-2 shrink-0">
        <!-- Split Ratio Presets (Only visible in Split mode) -->
        <div v-if="workspaceMode === 'split'" class="flex items-center gap-1 bg-[#070b10] px-1 py-0.5 rounded-lg border border-slate-800 text-[10px] font-semibold text-slate-400">
          <span class="text-[9px] uppercase tracking-wider text-slate-500 px-1">Ratio:</span>
          <button
            type="button"
            class="px-1.5 py-0.5 rounded hover:text-white transition-colors"
            :class="splitRatio === 65 ? 'bg-cyan-500/25 text-cyan-300 font-bold' : ''"
            title="Wider 2D Canvas (65% / 35%)"
            @click="splitRatio = 65"
          >
            2D 65%
          </button>
          <button
            type="button"
            class="px-1.5 py-0.5 rounded hover:text-white transition-colors"
            :class="splitRatio === 50 ? 'bg-cyan-500/25 text-cyan-300 font-bold' : ''"
            title="Balanced Split (50% / 50%)"
            @click="splitRatio = 50"
          >
            50:50
          </button>
          <button
            type="button"
            class="px-1.5 py-0.5 rounded hover:text-white transition-colors"
            :class="splitRatio === 35 ? 'bg-cyan-500/25 text-cyan-300 font-bold' : ''"
            title="Wider 3D Viewport (35% / 65%)"
            @click="splitRatio = 35"
          >
            3D 65%
          </button>
        </div>

        <div class="h-4 w-px bg-slate-800 hidden sm:block" />

        <!-- Format Badge -->
        <span class="text-[10px] font-mono text-slate-500 hidden sm:inline">64x64 RGBA</span>
      </div>
    </div>

    <!-- MAIN WORKSPACE AREA: Sidebar + Split/Dynamic Viewports -->
    <div class="flex-1 min-h-0 flex p-2.5 gap-2.5 overflow-hidden relative">
      <!-- Left Column: Collapsible Sidebar (Tools & Colors vs Parts & Layers) -->
      <div
        v-show="isSidebarOpen"
        class="w-64 min-w-[250px] max-w-[275px] flex flex-col overflow-hidden shrink-0 transition-all duration-200"
      >
        <!-- Sidebar Segmented Tab Switcher -->
        <div class="flex items-center bg-[#070b10] p-0.5 rounded-xl border border-slate-800/90 mb-2 shrink-0">
          <button
            type="button"
            class="flex-1 py-1 text-[11px] font-bold rounded-lg transition-all text-center flex items-center justify-center gap-1.5"
            :class="
              sidebarTab === 'tools'
                ? 'bg-cyan-500/20 text-cyan-300 shadow-sm border border-cyan-500/40'
                : 'text-slate-400 hover:text-slate-200'
            "
            @click="sidebarTab = 'tools'"
          >
            <svg class="w-3 h-3" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
              <path d="M17 3a2.828 2.828 0 1 1 4 4L7.5 20.5 2 22l1.5-5.5L17 3z" />
            </svg>
            Tools & Colors
          </button>
          <button
            type="button"
            class="flex-1 py-1 text-[11px] font-bold rounded-lg transition-all text-center flex items-center justify-center gap-1.5"
            :class="
              sidebarTab === 'parts'
                ? 'bg-cyan-500/20 text-cyan-300 shadow-sm border border-cyan-500/40'
                : 'text-slate-400 hover:text-slate-200'
            "
            @click="sidebarTab = 'parts'"
          >
            <svg class="w-3 h-3" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
              <path d="M20 21v-2a4 4 0 0 0-4-4H8a4 4 0 0 0-4 4v2" />
              <circle cx="12" cy="7" r="4" />
            </svg>
            Parts & Layers
          </button>
        </div>

        <!-- Sidebar Content: Tool suite OR Mannequin Part Isolation -->
        <div class="flex-1 min-h-0 overflow-y-auto pr-0.5 custom-scrollbar">
          <SkinToolbar v-if="sidebarTab === 'tools'" :studio="studio" />
          <SkinMannequinWidget v-else-if="sidebarTab === 'parts'" :studio="studio" />
        </div>
      </div>

      <!-- Center / Right: Dynamic Split Workspace Area -->
      <div ref="splitAreaRef" class="flex-1 min-h-0 flex gap-0 overflow-hidden relative">
        <!-- Center: 2D UV Pixel Grid Canvas (Dynamic Resizing) -->
        <div
          v-show="workspaceMode === 'split' || workspaceMode === '2d'"
          class="h-full min-w-0 transition-all overflow-hidden"
          :style="{
            width: workspaceMode === 'split' ? `${splitRatio}%` : '100%',
            flex: workspaceMode === 'split' ? 'none' : '1',
          }"
        >
          <SkinCanvas2D :studio="studio" />
        </div>

        <!-- Draggable Resizer Handle (Only visible in Split mode) -->
        <div
          v-if="workspaceMode === 'split'"
          class="w-2.5 mx-[-1px] group cursor-col-resize flex items-center justify-center shrink-0 z-20 select-none hover:bg-cyan-500/20 active:bg-cyan-500/40 rounded transition-colors"
          title="Drag left or right to resize viewports"
          @pointerdown="startSplitDrag"
        >
          <div class="w-1 h-10 rounded-full bg-slate-700 group-hover:bg-cyan-400 group-active:bg-cyan-300 transition-colors shadow-sm" />
        </div>

        <!-- Right: 3D Three.js Raycast Viewport (Dynamic Resizing) -->
        <div
          v-show="workspaceMode === 'split' || workspaceMode === '3d'"
          class="h-full min-w-0 transition-all overflow-hidden"
          :style="{
            width: workspaceMode === 'split' ? `${100 - splitRatio}%` : '100%',
            flex: workspaceMode === 'split' ? 'none' : '1',
          }"
        >
          <SkinViewport3D :studio="studio" />
        </div>
      </div>
    </div>

    <!-- Status Toast Notification -->
    <transition name="fade">
      <div
        v-if="toastMessage"
        class="absolute bottom-4 right-4 bg-[#0e1724] border border-cyan-500/60 text-cyan-200 text-xs px-3.5 py-2 rounded-xl shadow-2xl flex items-center gap-2 z-50 font-medium"
      >
        <svg class="w-4 h-4 text-cyan-400 shrink-0" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
          <circle cx="12" cy="12" r="10" />
          <polyline points="12 6 12 12 14 14" />
        </svg>
        <span>{{ toastMessage }}</span>
      </div>
    </transition>
  </div>
</template>

<script setup>
import { onMounted, onUnmounted, ref } from 'vue';
import SkinToolbar from './SkinToolbar.vue';
import SkinMannequinWidget from './SkinMannequinWidget.vue';
import SkinCanvas2D from './SkinCanvas2D.vue';
import SkinViewport3D from './SkinViewport3D.vue';
import { api, pickFile, PNG_FILTER } from '../../lib/api';

const props = defineProps({
  studio: { type: Object, required: true },
  session: { type: Object, default: null },
});

const emit = defineEmits(['skinSaved', 'skinApplied']);

const showNewModal = ref(false);
const isSaving = ref(false);
const isApplying = ref(false);
const workspaceMode = ref('split'); // 'split' | '2d' | '3d'
const sidebarTab = ref('tools'); // 'tools' | 'parts'
const isSidebarOpen = ref(true); // collapsible sidebar
const splitRatio = ref(50); // percentage for 2D canvas in split view
const splitAreaRef = ref(null);
const toastMessage = ref('');
let toastTimeout = null;
let isDraggingSplit = false;

// --- Draggable Splitter Logic ---
function startSplitDrag(e) {
  isDraggingSplit = true;
  window.addEventListener('pointermove', onSplitDrag);
  window.addEventListener('pointerup', stopSplitDrag);
  document.body.style.cursor = 'col-resize';
  document.body.style.userSelect = 'none';
}

function onSplitDrag(e) {
  if (!isDraggingSplit || !splitAreaRef.value) return;
  const rect = splitAreaRef.value.getBoundingClientRect();
  if (rect.width <= 0) return;
  const offsetX = e.clientX - rect.left;
  const rawRatio = (offsetX / rect.width) * 100;
  // Clamp between 20% and 80% to keep both viewports usable
  splitRatio.value = Math.min(80, Math.max(20, Math.round(rawRatio)));
}

function stopSplitDrag() {
  if (!isDraggingSplit) return;
  isDraggingSplit = false;
  window.removeEventListener('pointermove', onSplitDrag);
  window.removeEventListener('pointerup', stopSplitDrag);
  document.body.style.cursor = '';
  document.body.style.userSelect = '';
}

function showToast(msg, duration = 3000) {
  toastMessage.value = msg;
  if (toastTimeout) clearTimeout(toastTimeout);
  toastTimeout = setTimeout(() => {
    toastMessage.value = '';
  }, duration);
}

function loadTemplateAndClose(type) {
  showNewModal.value = false;
  props.studio.loadTemplate(type);
  showToast(`Loaded ${type.charAt(0).toUpperCase() + type.slice(1)} template.`);
}

async function importSkinFile() {
  const path = await pickFile(PNG_FILTER);
  if (!path) return;
  try {
    showToast('Importing skin…');
    const cleanFilename = path.split(/[/\\]/).pop() || 'imported_skin.png';
    const img = new Image();
    img.crossOrigin = 'anonymous';
    img.onload = () => {
      props.studio.loadFromDataUrl(img.src, cleanFilename);
      showToast(`Imported '${cleanFilename}'`);
    };
    img.src = path.startsWith('http') || path.startsWith('data:') ? path : `https://asset.localhost/${path}`;
  } catch (err) {
    console.error('Failed to import skin in studio:', err);
    showToast(`Error importing: ${err}`);
  }
}

function exportSkinPng() {
  const dataUrl = props.studio.toDataUrl();
  const safeName = (props.studio.state.skinName || 'custom_skin.png').replace(/[^a-zA-Z0-9._-]/g, '_');
  const cleanName = safeName.endsWith('.png') ? safeName : `${safeName}.png`;

  const link = document.createElement('a');
  link.download = cleanName;
  link.href = dataUrl;
  document.body.appendChild(link);
  link.click();
  document.body.removeChild(link);

  showToast(`Exported '${cleanName}' to downloads`);
}

async function saveToGallery() {
  isSaving.value = true;
  try {
    const bytes = props.studio.toBytes();
    const safeName = (props.studio.state.skinName || 'custom_skin.png').replace(/[^a-zA-Z0-9._-]/g, '_');
    const cleanName = safeName.endsWith('.png') ? safeName : `${safeName}.png`;

    await api.saveSkinBytes(cleanName, bytes, props.studio.state.variant);
    props.studio.state.isDirty = false;
    showToast(`Saved '${cleanName}' to your skins gallery!`);
    emit('skinSaved');
  } catch (err) {
    console.error('Failed to save skin from studio:', err);
    showToast(`Error saving: ${err}`);
  } finally {
    isSaving.value = false;
  }
}

async function applyAndSync() {
  isApplying.value = true;
  try {
    const bytes = props.studio.toBytes();
    const safeName = (props.studio.state.skinName || 'custom_skin.png').replace(/[^a-zA-Z0-9._-]/g, '_');
    const cleanName = safeName.endsWith('.png') ? safeName : `${safeName}.png`;

    // 1. Save to active launcher skin
    await api.saveSkinBytes(cleanName, bytes, props.studio.state.variant);

    // 2. Upload to Mojang if logged in
    if (props.session?.username) {
      showToast('Syncing skin to Minecraft (Mojang)…');
      try {
        await api.uploadSkinToMojang(props.studio.state.variant);
        showToast('Skin applied locally and synced to Minecraft!');
      } catch (mojangErr) {
        console.warn('Mojang sync notice:', mojangErr);
        showToast(`Saved locally (Mojang notice: ${mojangErr})`);
      }
    } else {
      showToast('Skin applied locally (sign in to sync to Minecraft)');
    }

    props.studio.state.isDirty = false;
    emit('skinApplied');
  } catch (err) {
    console.error('Failed to apply skin from studio:', err);
    showToast(`Error applying skin: ${err}`);
  } finally {
    isApplying.value = false;
  }
}

// Global Keyboard Shortcuts
function onKeyDown(e) {
  if (['INPUT', 'SELECT', 'TEXTAREA'].includes(e.target?.tagName)) return;

  if (e.ctrlKey || e.metaKey) {
    if (e.key === 'z' || e.key === 'Z') {
      if (e.shiftKey) {
        e.preventDefault();
        props.studio.redo();
      } else {
        e.preventDefault();
        props.studio.undo();
      }
    } else if (e.key === 'y' || e.key === 'Y') {
      e.preventDefault();
      props.studio.redo();
    } else if (e.key === 's' || e.key === 'S') {
      e.preventDefault();
      saveToGallery();
    }
    return;
  }

  const key = e.key.toLowerCase();
  if (key === 'b') props.studio.state.activeTool = 'pencil';
  else if (key === 'e') props.studio.state.activeTool = 'eraser';
  else if (key === 'i') props.studio.state.activeTool = 'picker';
  else if (key === 'g') props.studio.state.activeTool = 'bucket';
  else if (key === 'n') props.studio.state.activeTool = 'noise';
}

onMounted(() => {
  window.addEventListener('keydown', onKeyDown);
});

onUnmounted(() => {
  window.removeEventListener('keydown', onKeyDown);
  stopSplitDrag();
  if (toastTimeout) clearTimeout(toastTimeout);
});
</script>

<style scoped>
.fade-enter-active,
.fade-leave-active {
  transition: opacity 0.2s ease;
}
.fade-enter-from,
.fade-leave-to {
  opacity: 0;
}
</style>
