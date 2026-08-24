<template>
  <div class="h-full flex gap-5 p-5 overflow-hidden">
    <!-- Left: instance list -->
    <div class="w-[300px] min-w-[300px] z-card flex flex-col">
      <div class="flex items-center justify-between mb-3">
        <span class="z-section">Offline Instances</span>
        <button class="z-btn-accent text-xs" @click="openNewInstance">+ New Instance</button>
      </div>
      <div class="flex-1 min-h-0 overflow-y-auto pr-1">
        <div
          v-for="instance in instances"
          :key="instance.id"
          class="flex items-center gap-3 bg-bg border rounded-lg p-3 mb-2.5 cursor-pointer transition-colors"
          :class="
            selected?.id === instance.id
              ? 'border-accent bg-[#142129] shadow-[0_0_0_1px_rgba(71,210,201,0.15)]'
              : 'border-edge hover:border-[#3d444d] hover:bg-[#1a2129]'
          "
          @click="selectInstance(instance)"
        >
          <div
            class="w-8 h-8 rounded-md bg-gradient-to-br from-[#24313d] to-[#1a222b] text-accent flex items-center justify-center text-sm shrink-0 border border-edge"
          >
            {{ instance.name.charAt(0).toUpperCase() }}
          </div>
          <div class="flex-1 min-w-0">
            <div class="text-[13px] font-bold text-white truncate">{{ instance.name }}</div>
            <div class="text-[11px] text-muted truncate">
              MC {{ instance.minecraftVersion }} · {{ instance.modLoader.type }}
            </div>
          </div>
        </div>
        <div v-if="instances.length === 0" class="text-muted text-sm py-6 text-center">
          No offline instances yet.
        </div>
      </div>
    </div>

    <!-- Right: instance detail -->
    <div class="flex-1 min-w-0 z-card flex flex-col">
      <template v-if="selected">
        <div class="flex-1 min-h-0 overflow-y-auto pr-1 flex flex-col gap-4">
          <!-- Meta -->
          <div class="bg-bg border border-edge rounded-lg p-3">
            <div class="z-section mb-2">{{ selected.name }}</div>
            <div class="grid grid-cols-2 gap-1 text-xs">
              <span class="z-label">Minecraft:</span><span class="text-text">{{ selected.minecraftVersion }}</span>
              <span class="z-label">Loader:</span
              ><span class="text-text capitalize">{{ selected.modLoader.type }} {{ selected.modLoader.version }}</span>
            </div>
          </div>

          <!-- Mods -->
          <div class="bg-bg border border-edge rounded-lg p-3">
            <div class="z-section mb-2">Mods</div>
            <div
              v-if="mods.length"
              class="max-h-[140px] overflow-y-auto mb-2 flex flex-col gap-1"
            >
              <div
                v-for="mod in mods"
                :key="mod.filename"
                class="flex items-center gap-2 text-xs"
              >
                <div class="flex-1 min-w-0">
                  <div class="truncate text-text">{{ mod.filename }}</div>
                  <div v-if="mod.author" class="text-[10px] text-muted">by {{ mod.author }}</div>
                </div>
                <span class="text-muted">{{ fmtBytes(mod.sizeBytes) }}</span>
                <button class="text-muted hover:text-[#f85149]" title="Delete" @click="deleteMod(mod.filename)">✕</button>
              </div>
            </div>
            <div
              class="border border-dashed border-edge rounded-lg p-3 text-center text-xs text-muted"
              @dragover.prevent
              @drop.prevent="onDrop"
            >
              Drop .jar mod files here (or <button class="text-accent underline" @click="browseMods">browse</button>)
            </div>

            <!-- Modrinth search -->
            <div class="flex gap-2 mt-3">
              <input
                v-model="modrinthQuery"
                class="z-input"
                placeholder="Search Modrinth (e.g. Sodium)"
                @keydown.enter="searchModrinth"
              />
              <button class="z-btn-ghost" :disabled="modSearchBusy" @click="searchModrinth">Search</button>
            </div>
            <div v-if="modSearchBusy" class="text-xs text-muted mt-2">Searching Modrinth…</div>
            <div class="mt-2 flex flex-col gap-1 max-h-[180px] overflow-y-auto">
              <div
                v-for="hit in modResults"
                :key="hit.projectId"
                class="flex items-center gap-2 bg-card border border-edge rounded-md p-2"
              >
                <img
                  v-if="hit.iconUrl"
                  :src="hit.iconUrl"
                  class="w-7 h-7 rounded"
                  loading="lazy"
                />
                <div class="flex-1 min-w-0">
                  <div class="text-xs font-bold text-white truncate">{{ hit.title }}</div>
                  <div class="text-[10px] text-muted truncate">
                    {{ hit.author }} · {{ fmtCount(hit.downloads) }} downloads
                  </div>
                </div>
                <button
                  class="z-btn-ghost text-[10px]"
                  :disabled="installing === hit.projectId"
                  @click="installMod(hit)"
                >
                  {{ installing === hit.projectId ? '…' : 'Install' }}
                </button>
              </div>
              <div v-if="!modSearchBusy && modSearchDone && modResults.length === 0" class="text-xs text-muted">
                No mods found for this Minecraft version + loader.
              </div>
            </div>
          </div>

          <!-- Packs -->
          <div class="bg-bg border border-edge rounded-lg p-3">
            <div class="z-section mb-2">Shaders &amp; Texture Packs</div>

            <div class="text-xs text-muted mb-1">Shaders</div>
            <select v-model="activeShaderpack" class="z-input mb-2" @change="onShaderpackChange">
              <option value="">None (shaders disabled)</option>
              <option v-for="name in packs.shaderpacks" :key="name" :value="name">{{ name }}</option>
            </select>
            <button class="z-btn-ghost text-[11px] mb-3" @click="addLocalPack('shader')">+ Add Shaderpack (.zip)</button>

            <div class="text-xs text-muted mb-1">Texture Packs</div>
            <div class="flex flex-col gap-1 mb-2">
              <label
                v-for="name in packs.resourcepacks"
                :key="name"
                class="flex items-center gap-2 text-xs cursor-pointer"
              >
                <input
                  type="checkbox"
                  class="accent-[#47d2c9]"
                  :checked="packs.activeResourcepacks.includes(name)"
                  @change="togglePack(name)"
                />
                <span class="truncate text-text">{{ name }}</span>
              </label>
              <div v-if="packs.resourcepacks.length === 0" class="text-xs text-muted">
                No texture packs added.
              </div>
            </div>
            <button class="z-btn-ghost text-[11px]" @click="addLocalPack('resource')">+ Add Texture Pack (.zip)</button>
          </div>
        </div>

        <!-- Actions -->
        <div class="flex gap-2 mt-4 shrink-0">
          <button class="z-btn-accent flex-1 py-2.5" :disabled="launching" @click="playOffline">
            <span v-if="launching" class="inline-flex items-center gap-2">
              <span class="inline-block w-3.5 h-3.5 border-2 border-[#022c29] border-t-transparent rounded-full animate-spin"></span>
              LAUNCHING…
            </span>
            <span v-else>Play Offline</span>
          </button>
          <button class="z-btn-danger px-5" @click="deleteInstance">Delete</button>
        </div>
      </template>

      <div v-else class="flex-1 flex items-center justify-center text-muted">
        Select an instance to manage mods &amp; packs.
      </div>
    </div>

    <!-- New instance modal -->
    <div
      v-if="showNewDialog"
      class="absolute inset-0 z-40 bg-black/60 backdrop-blur-sm flex items-center justify-center"
      @click.self="showNewDialog = false"
    >
      <div class="z-card w-[440px] pt-0 overflow-hidden">
        <div
          class="h-[3px] bg-gradient-to-r from-accent to-[#1f8f87] -mx-4 -mt-4 mb-4"
        ></div>
        <h3 class="text-white font-bold mb-4">New Offline Instance</h3>
        <label class="z-label">Instance name</label>
        <input v-model="newForm.name" class="z-input mb-3" placeholder="My Modded World" />
        <label class="z-label">Minecraft version</label>
        <select v-model="newForm.mcVersion" class="z-input mb-3">
          <option v-for="v in mcVersions" :key="v" :value="v">{{ v }}</option>
        </select>
        <label class="z-label">Mod loader</label>
        <select v-model="newForm.loaderType" class="z-input mb-3">
          <option v-for="l in loaderTypes" :key="l" :value="l" class="capitalize">{{ l }}</option>
        </select>
        <label class="z-label">Loader version (optional)</label>
        <input
          v-model="newForm.loaderVersion"
          class="z-input mb-4"
          placeholder="e.g. 0.15.11"
        />
        <div class="flex justify-end gap-2">
          <button class="z-btn-ghost" @click="showNewDialog = false">Cancel</button>
          <button class="z-btn-accent" :disabled="creating" @click="createInstance">Create</button>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup>
import { computed, onBeforeUnmount, onMounted, ref } from 'vue';
import { getCurrentWebview } from '@tauri-apps/api/webview';
import {
  api,
  fmtBytes,
  JAR_FILTER,
  PACK_FILTER,
  pickFiles,
} from '../lib/api';

const emit = defineEmits(['launching', 'stopped']);

const instances = ref([]);
const selected = ref(null);
const selectedDir = ref('');
const mods = ref([]);
const packs = ref({
  shaderpacks: [],
  resourcepacks: [],
  activeResourcepacks: [],
});
const activeShaderpack = ref('');
const launching = ref(false);

// Modrinth
const modrinthQuery = ref('');
const modResults = ref([]);
const modSearchBusy = ref(false);
const modSearchDone = ref(false);
const installing = ref('');

// New instance modal
const showNewDialog = ref(false);
const creating = ref(false);
const mcVersions = ref([]);
const loaderTypes = ref([]);
const newForm = ref({ name: '', mcVersion: '1.20.4', loaderType: 'fabric', loaderVersion: '' });

const allPacks = computed(() => packs.value);

// Formats a raw download count like the Modrinth API returns it.
function fmtCount(n) {
  if (!n) return '0';
  if (n >= 1_000_000) return `${(n / 1_000_000).toFixed(1)}M`;
  if (n >= 1_000) return `${(n / 1_000).toFixed(1)}K`;
  return String(n);
}

async function loadInstances() {
  instances.value = await api.listOfflineInstances();
  if (instances.value.length && !selected.value) {
    await selectInstance(instances.value[0]);
  }
}

async function selectInstance(instance) {
  selected.value = instance;
  selectedDir.value = await api.getOfflineInstanceDir(instance.id);
  await Promise.all([loadMods(), loadPacks()]);
}

async function loadMods() {
  if (!selected.value) return;
  mods.value = await api.listOfflineMods(selected.value.id);
}

async function loadPacks() {
  if (!selected.value) return;
  packs.value = await api.listInstancePacks(selectedDir.value);
  activeShaderpack.value = packs.value.activeShaderpack || '';
}

async function playOffline() {
  if (!selected.value) return;
  launching.value = true;
  emit('launching');
  try {
    await api.launchOfflineInstance(selected.value.id);
  } catch (e) {
    window.dispatchEvent(new CustomEvent('zircon-status', { detail: `Error: ${e}` }));
  } finally {
    launching.value = false;
    emit('stopped');
    await loadInstances();
  }
}

async function deleteInstance() {
  if (!selected.value) return;
  if (!window.confirm(`Delete '${selected.value.name}' and all of its files?`)) return;
  await api.deleteOfflineInstance(selected.value.id);
  selected.value = null;
  await loadInstances();
}

async function deleteMod(filename) {
  await api.deleteOfflineMod(selected.value.id, filename);
  await loadMods();
}

async function browseMods() {
  const files = await pickFiles(JAR_FILTER);
  for (const file of files) {
    await api.addOfflineMod(selected.value.id, file);
  }
  await loadMods();
}

async function onDrop(event) {
  const files = event.dataTransfer?.files;
  if (!files || files.length === 0) return;
  for (const file of files) {
    const path = file.path || file.webkitRelativePath;
    if (path && path.toLowerCase().endsWith('.jar')) {
      await api.addOfflineMod(selected.value.id, path);
    }
  }
  await loadMods();
}

async function searchModrinth() {
  const query = modrinthQuery.value.trim();
  if (!query || !selected.value) return;
  modSearchBusy.value = true;
  modSearchDone.value = false;
  try {
    modResults.value = await api.searchModrinth(selected.value.id, query);
  } catch (e) {
    window.dispatchEvent(new CustomEvent('zircon-status', { detail: `Search failed: ${e}` }));
    modResults.value = [];
  } finally {
    modSearchBusy.value = false;
    modSearchDone.value = true;
  }
}

async function installMod(hit) {
  installing.value = hit.projectId;
  try {
    const filename = await api.installModrinthMod(selected.value.id, hit.projectId);
    window.dispatchEvent(new CustomEvent('zircon-status', { detail: `Installed ${filename}` }));
    await loadMods();
  } catch (e) {
    window.dispatchEvent(new CustomEvent('zircon-status', { detail: `Install failed: ${e}` }));
  } finally {
    installing.value = '';
  }
}

async function addLocalPack(kind) {
  const [file] = await pickFiles(PACK_FILTER);
  if (!file) return;
  await api.addLocalPack(selectedDir.value, file, kind);
  await loadPacks();
}

async function onShaderpackChange() {
  await api.setActiveShaderpack(selectedDir.value, activeShaderpack.value);
  await loadPacks();
}

async function togglePack(name) {
  await api.toggleResourcepack(selectedDir.value, name);
  await loadPacks();
}

async function openNewInstance() {
  showNewDialog.value = true;
  if (mcVersions.value.length === 0) {
    try {
      mcVersions.value = await api.listMinecraftVersions();
      if (!mcVersions.value.includes(newForm.value.mcVersion)) {
        newForm.value.mcVersion = mcVersions.value[0] || '1.20.4';
      }
    } catch {
      mcVersions.value = ['1.20.4'];
    }
  }
  if (loaderTypes.value.length === 0) {
    try {
      loaderTypes.value = await api.listLoaderTypes();
    } catch {
      loaderTypes.value = ['vanilla', 'fabric', 'forge', 'neoforge', 'quilt'];
    }
  }
}

async function createInstance() {
  creating.value = true;
  try {
    const instance = await api.createOfflineInstance(
      newForm.value.name,
      newForm.value.mcVersion,
      newForm.value.loaderType,
      newForm.value.loaderVersion
    );
    showNewDialog.value = false;
    newForm.value = { name: '', mcVersion: newForm.value.mcVersion, loaderType: newForm.value.loaderType, loaderVersion: '' };
    await loadInstances();
    await selectInstance(instance);
  } catch (e) {
    window.dispatchEvent(new CustomEvent('zircon-status', { detail: `Create failed: ${e}` }));
  } finally {
    creating.value = false;
  }
}

let unlistenDrop = null;

onMounted(async () => {
  await loadInstances();
  try {
    const webview = getCurrentWebview();
    unlistenDrop = await webview.onDragDropEvent((event) => {
      if (event.payload.type === 'drop') {
        for (const path of event.payload.paths) {
          if (path.toLowerCase().endsWith('.jar')) {
            api.addOfflineMod(selected.value?.id || '', path).catch(() => {});
          }
        }
        setTimeout(loadMods, 300);
      }
    });
  } catch {
    // Drag-drop events unavailable (browser preview) — fall back to browsing.
  }
});

onBeforeUnmount(() => {
  if (unlistenDrop) unlistenDrop();
});
</script>
