<template>
  <div class="h-full flex gap-5 p-5 overflow-hidden">
    <!-- Left: instance list -->
    <div class="w-[310px] min-w-[310px] z-card flex flex-col p-4 bg-[#0e1622]/90 border border-slate-800/80">
      <div class="flex items-center justify-between mb-4">
        <span class="z-section text-white font-bold">Offline Instances</span>
        <button class="z-btn-accent text-xs font-bold px-3 py-1.5 rounded-xl shadow-md hover:shadow-cyan-500/25" @click="openNewInstance">+ New Instance</button>
      </div>
      <div class="flex-1 min-h-0 overflow-y-auto pr-1">
        <div
          v-for="instance in instances"
          :key="instance.id"
          class="flex items-center gap-3.5 border rounded-xl p-3.5 mb-2.5 cursor-pointer transition-all duration-200"
          :class="
            selected?.id === instance.id
              ? 'border-cyan-400 ring-1 ring-cyan-400/60 shadow-[0_0_16px_rgba(71,210,201,0.25)] bg-[#111c29]'
              : 'border-slate-800/80 bg-[#070b10]/60 hover:border-slate-700 hover:bg-[#121d2b]'
          "
          @click="selectInstance(instance)"
        >
          <div
            class="w-10 h-10 rounded-xl bg-gradient-to-br from-[#5adfd5] via-[#47d2c9] to-[#20b2aa] text-[#022623] font-black flex items-center justify-center text-base shrink-0 shadow-[0_0_10px_rgba(71,210,201,0.25)]"
          >
            {{ instance.name.charAt(0).toUpperCase() }}
          </div>
          <div class="flex-1 min-w-0">
            <div class="text-[13px] font-bold text-white truncate">{{ instance.name }}</div>
            <div class="text-[11px] text-slate-400 font-mono truncate mt-0.5">
              MC {{ instance.minecraftVersion }} · <span class="capitalize">{{ instance.modLoader.type }}</span>
            </div>
          </div>
        </div>
        <div v-if="instances.length === 0" class="text-slate-500 text-xs py-8 text-center">
          No offline instances yet.
        </div>
      </div>
    </div>

    <!-- Right: instance detail -->
    <div class="flex-1 min-w-0 z-card flex flex-col p-5 bg-[#0e1622]/90 border border-slate-800/80">
      <template v-if="selected">
        <div class="flex-1 min-h-0 overflow-y-auto pr-1 flex flex-col gap-4">
          <!-- Meta -->
          <div class="bg-[#070b10] border border-slate-800/90 rounded-xl p-4 shadow-inner">
            <div class="z-section mb-2 text-white font-bold text-sm">{{ selected.name }}</div>
            <div class="grid grid-cols-2 gap-2 text-xs">
              <div><span class="text-slate-400 font-semibold">Minecraft:</span> <span class="text-white font-mono ml-1">{{ selected.minecraftVersion }}</span></div>
              <div><span class="text-slate-400 font-semibold">Loader:</span> <span class="text-cyan-300 font-mono capitalize ml-1">{{ selected.modLoader.type }} {{ selected.modLoader.version }}</span></div>
            </div>
          </div>

          <!-- Mods -->
          <div class="bg-[#070b10] border border-slate-800/90 rounded-xl p-4 shadow-inner">
            <div class="flex items-center justify-between mb-3">
              <div class="z-section text-white font-bold text-sm">Mods ({{ mods.length }})</div>
              <label
                v-if="mods.length"
                class="flex items-center gap-2 text-xs text-slate-400 cursor-pointer select-none"
              >
                <input
                  type="checkbox"
                  class="zircon-check"
                  :checked="allModsSelected"
                  @change="toggleSelectAllMods"
                />
                Select All
              </label>
            </div>

            <div
              v-if="selectedModCount > 0"
              class="flex items-center gap-2 mb-3 bg-slate-900 border border-slate-700/80 rounded-xl px-3 py-2 text-xs"
            >
              <span class="text-slate-400 font-medium">{{ selectedModCount }} selected</span>
              <div class="flex-1"></div>
              <button class="z-btn-ghost text-[10px] px-2.5 py-1 rounded-lg" @click="bulkEnableSelected">Enable</button>
              <button class="z-btn-ghost text-[10px] px-2.5 py-1 rounded-lg" @click="bulkDisableSelected">Disable</button>
              <button class="text-[10px] px-2.5 py-1 text-red-400 hover:text-red-300 font-semibold" @click="bulkDeleteSelected">Delete</button>
            </div>

            <div
              v-if="mods.length"
              class="max-h-[160px] overflow-y-auto mb-3 flex flex-col gap-1.5 pr-1"
            >
              <div
                v-for="mod in mods"
                :key="mod.filename"
                class="flex items-center gap-2.5 p-2 rounded-xl bg-slate-900/60 border border-slate-800/80 text-xs transition hover:border-slate-700"
                :class="{ 'opacity-50': !mod.enabled }"
              >
                <input
                  type="checkbox"
                  class="zircon-check shrink-0"
                  :checked="!!selectedMods[mod.filename]"
                  @change="toggleModSelected(mod.filename)"
                />
                <div class="flex-1 min-w-0">
                  <div class="flex items-center gap-1.5 truncate">
                    <span class="truncate text-white font-medium">{{ mod.filename }}</span>
                    <span
                      v-if="mod.version"
                      class="bg-slate-950 text-slate-400 border border-slate-800 text-[10px] px-1.5 py-0.2 rounded font-mono shrink-0"
                    >{{ mod.version }}</span>
                  </div>
                  <div v-if="mod.author" class="text-[10px] text-slate-500">by {{ mod.author }}</div>
                </div>
                <span class="text-slate-500 font-mono text-[11px]">{{ fmtBytes(mod.sizeBytes) }}</span>
                <button
                  class="z-toggle"
                  :class="{ 'z-toggle-on': mod.enabled }"
                  :title="mod.enabled ? 'Disable' : 'Enable'"
                  @click="toggleModEnabled(mod)"
                >
                  <span class="z-toggle-thumb"></span>
                </button>
                <button class="text-slate-400 hover:text-red-400 text-xs px-1.5 py-0.5 transition" title="Delete" @click="deleteMod(mod.filename)">✕</button>
              </div>
            </div>
            <div
              class="zircon-drop-zone p-4 text-center text-xs text-slate-400 cursor-pointer"
              @dragover.prevent
              @drop.prevent="onDrop"
            >
              Drop .jar mod files here (or <button class="text-cyan-400 underline font-semibold" @click="browseMods">browse files</button>)
            </div>

            <!-- Modrinth search -->
            <div class="flex gap-2 mt-4">
              <input
                v-model="modrinthQuery"
                class="z-input"
                placeholder="Search Modrinth (e.g. Sodium, Iris, FerriteCore)..."
                @keydown.enter="searchModrinth"
              />
              <button class="z-btn-ghost px-4" :disabled="modSearchBusy" @click="searchModrinth">Search</button>
            </div>
            <div v-if="modSearchBusy" class="text-xs text-cyan-400 mt-2 font-mono flex items-center gap-1.5">
              <span class="inline-block w-2.5 h-2.5 border-2 border-accent border-t-transparent rounded-full animate-spin"></span>
              Searching Modrinth…
            </div>
            <div class="mt-2.5 flex flex-col gap-2 max-h-[260px] overflow-y-auto pr-1">
              <div
                v-for="hit in modResults"
                :key="hit.projectId"
                class="bg-slate-900/60 border border-slate-800 rounded-xl p-3 flex flex-col gap-2 transition hover:border-slate-700"
              >
                <div class="flex items-start gap-3">
                  <img
                    v-if="hit.iconUrl"
                    :src="hit.iconUrl"
                    class="w-9 h-9 rounded-lg shrink-0 mt-0.5 object-cover"
                    loading="lazy"
                  />
                  <div class="flex-1 min-w-0">
                    <div class="flex items-center justify-between gap-2">
                      <div class="text-xs font-bold text-white truncate">{{ hit.title }}</div>
                      <button
                        class="z-btn-ghost text-[10px] px-3 py-1 font-bold shrink-0 rounded-lg"
                        :disabled="installing === hit.projectId || hit.versionsLoading"
                        @click="installMod(hit)"
                      >
                        {{ installing === hit.projectId ? 'Installing…' : 'Install' }}
                      </button>
                    </div>
                    <div v-if="hit.description" class="text-[11px] text-slate-300 line-clamp-2 my-1">
                      {{ hit.description }}
                    </div>
                    <div class="flex items-center gap-2 text-[10px] text-slate-400 flex-wrap">
                      <span>by <strong class="text-white font-medium">{{ hit.author }}</strong></span>
                      <span>·</span>
                      <span>{{ fmtCount(hit.downloads) }} downloads</span>
                      <span>·</span>
                      <a
                        :href="hit.projectUrl || ('https://modrinth.com/project/' + (hit.slug || hit.projectId))"
                        target="_blank"
                        rel="noopener noreferrer"
                        class="text-cyan-400 hover:underline inline-flex items-center gap-0.5"
                        @click.prevent="openModrinthLink(hit)"
                      >
                        View on Modrinth ↗
                      </a>
                    </div>
                  </div>
                </div>

                <!-- Version selector -->
                <div class="flex items-center gap-2 pt-2 border-t border-slate-800">
                  <label class="text-[10px] text-slate-400 shrink-0 font-semibold">Version:</label>
                  <select
                    v-model="hit.selectedVersionId"
                    :disabled="installing === hit.projectId || hit.versionsLoading || hit.versionsFailed || !hit.versionOptions?.length"
                    class="flex-1 min-w-0 bg-slate-950 border border-slate-700 rounded-lg px-2.5 py-1 text-[11px] text-slate-200 disabled:opacity-50 focus:border-cyan-400 focus:outline-none"
                  >
                    <option v-if="hit.versionsLoading" value="" disabled>Loading versions…</option>
                    <option v-else-if="hit.versionsFailed || !hit.versionOptions?.length" value="" disabled>No compatible versions</option>
                    <option
                      v-for="v in hit.versionOptions"
                      :key="v.id"
                      :value="v.id"
                    >
                      {{ v.versionNumber || v.name }}
                    </option>
                  </select>
                </div>
              </div>
              <div v-if="!modSearchBusy && modSearchDone && modResults.length === 0" class="text-xs text-slate-500">
                No mods found for this Minecraft version + loader.
              </div>
            </div>
          </div>

          <!-- Packs -->
          <div class="bg-[#070b10] border border-slate-800/90 rounded-xl p-4 shadow-inner">
            <div class="z-section mb-2 text-white font-bold text-sm">Shaders &amp; Texture Packs</div>

            <div class="text-xs text-slate-400 mb-1 font-semibold">Shaders</div>
            <select v-model="activeShaderpack" class="z-input mb-2" @change="onShaderpackChange">
              <option value="">None (shaders disabled)</option>
              <option v-for="p in detailedPacks.shaderpacks" :key="p.filename" :value="p.filename">
                {{ p.title || p.filename }} ({{ p.version || 'Unknown' }})
              </option>
            </select>
            <button class="z-btn-ghost text-[11px] mb-4" @click="addLocalPack('shader')">+ Add Shaderpack (.zip)</button>

            <div class="text-xs text-slate-400 mb-1 font-semibold">Texture Packs</div>
            <div class="flex flex-col gap-1.5 mb-2.5">
              <label
                v-for="p in detailedPacks.resourcepacks"
                :key="p.filename"
                class="flex items-center gap-2.5 text-xs cursor-pointer p-2 rounded-lg bg-slate-900/60 border border-slate-800"
              >
                <input
                  type="checkbox"
                  class="zircon-check"
                  :checked="packs.activeResourcepacks.includes(p.filename)"
                  @change="togglePack(p.filename)"
                />
                <span class="truncate text-slate-200 flex-1 font-medium">{{ p.title || p.filename }}</span>
                <span class="bg-slate-950 text-slate-400 border border-slate-800 text-[10px] px-1.5 py-0.2 rounded font-mono shrink-0">
                  {{ p.version || (p.packFormat ? 'v' + p.packFormat : 'Unknown') }}
                </span>
              </label>
              <div v-if="detailedPacks.resourcepacks.length === 0" class="text-xs text-slate-500">
                No texture packs added.
              </div>
            </div>
            <button class="z-btn-ghost text-[11px]" @click="addLocalPack('resource')">+ Add Texture Pack (.zip)</button>
          </div>
        </div>

        <!-- Actions -->
        <div class="flex gap-2.5 mt-4 shrink-0 pt-3 border-t border-slate-800/80">
          <button class="z-btn-accent flex-1 py-2.5 rounded-xl font-bold" :disabled="launching" @click="playOffline">
            <span v-if="launching" class="inline-flex items-center gap-2">
              <span class="inline-block w-3.5 h-3.5 border-2 border-[#022623] border-t-transparent rounded-full animate-spin"></span>
              LAUNCHING…
            </span>
            <span v-else>Play Offline</span>
          </button>
          <button class="z-btn-danger px-5 rounded-xl font-bold" @click="deleteInstance">Delete</button>
        </div>
      </template>

      <div v-else class="flex-1 flex items-center justify-center text-slate-500 text-sm">
        Select an instance to manage mods &amp; packs.
      </div>
    </div>

    <!-- New instance modal -->
    <div
      v-if="showNewDialog"
      class="absolute inset-0 z-40 bg-[#070b0f]/85 backdrop-blur-md flex items-center justify-center p-4"
      @click.self="showNewDialog = false"
    >
      <div class="z-card w-full max-w-[440px] p-6 overflow-hidden shadow-2xl relative border border-slate-700/60 rounded-2xl bg-[#0e1622]">
        <h3 class="text-white font-bold text-base mb-4">New Offline Instance</h3>
        <label class="z-label font-semibold text-slate-300 block mb-1">Instance name</label>
        <input v-model="newForm.name" class="z-input mb-3" placeholder="My Modded World" />
        <label class="z-label font-semibold text-slate-300 block mb-1">Minecraft version</label>
        <select v-model="newForm.mcVersion" class="z-input mb-3">
          <option v-for="v in mcVersions" :key="v" :value="v">{{ v }}</option>
        </select>
        <label class="z-label font-semibold text-slate-300 block mb-1">Mod loader</label>
        <select v-model="newForm.loaderType" class="z-input mb-3">
          <option v-for="l in loaderTypes" :key="l" :value="l" class="capitalize">{{ l }}</option>
        </select>
        <label class="z-label font-semibold text-slate-300 block mb-1">Loader version (optional)</label>
        <input
          v-model="newForm.loaderVersion"
          class="z-input mb-5"
          placeholder="e.g. 0.15.11"
        />
        <div class="flex justify-end gap-2.5 pt-4 border-t border-slate-800/80">
          <button class="z-btn-ghost text-xs px-4 py-2 rounded-xl font-semibold border border-slate-700/80 hover:border-slate-600 hover:text-white" @click="showNewDialog = false">Cancel</button>
          <button class="z-btn-accent text-xs font-bold px-5 py-2 rounded-xl shadow-md hover:shadow-cyan-500/25" :disabled="creating" @click="createInstance">Create</button>
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

const emit = defineEmits(['launching', 'stopped', 'error']);

const instances = ref([]);
const selected = ref(null);
const selectedDir = ref('');
const mods = ref([]);
const selectedMods = ref({});
const packs = ref({
  shaderpacks: [],
  resourcepacks: [],
  activeResourcepacks: [],
});
const detailedPacks = ref({
  shaderpacks: [],
  resourcepacks: [],
  shadersEnabled: false,
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

const selectedModCount = computed(() => Object.keys(selectedMods.value).length);
const allModsSelected = computed(
  () => mods.value.length > 0 && mods.value.every((m) => selectedMods.value[m.filename])
);

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
  selectedMods.value = {};
}

async function loadPacks() {
  if (!selected.value) return;
  const [basicPacks, detailed] = await Promise.all([
    api.listInstancePacks(selectedDir.value),
    api.listInstancePacksDetailed(selectedDir.value),
  ]);
  packs.value = basicPacks;
  detailedPacks.value = detailed;
  activeShaderpack.value = packs.value.activeShaderpack || '';
}

async function playOffline() {
  if (!selected.value) return;
  launching.value = true;
  emit('launching');
  try {
    await api.launchOfflineInstance(selected.value.id);
  } catch (e) {
    const errMsg = typeof e === 'string' ? e : (e?.message || String(e));
    emit('error', errMsg);
    window.dispatchEvent(new CustomEvent('zircon-status', { detail: `Error: ${errMsg}` }));
    emit('stopped');
  } finally {
    launching.value = false;
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

function toggleModSelected(filename) {
  const next = { ...selectedMods.value };
  if (next[filename]) {
    delete next[filename];
  } else {
    next[filename] = true;
  }
  selectedMods.value = next;
}

function toggleSelectAllMods() {
  if (allModsSelected.value) {
    selectedMods.value = {};
    return;
  }
  const next = {};
  for (const mod of mods.value) next[mod.filename] = true;
  selectedMods.value = next;
}

async function toggleModEnabled(mod) {
  await api.setOfflineModEnabled(selected.value.id, mod.filename, !mod.enabled);
  await loadMods();
}

async function setSelectedModsEnabled(enabled) {
  const filenames = Object.keys(selectedMods.value);
  if (!filenames.length) return;
  await Promise.all(
    filenames.map((filename) => api.setOfflineModEnabled(selected.value.id, filename, enabled))
  );
  await loadMods();
}

function bulkEnableSelected() {
  return setSelectedModsEnabled(true);
}
function bulkDisableSelected() {
  return setSelectedModsEnabled(false);
}

async function bulkDeleteSelected() {
  const filenames = Object.keys(selectedMods.value);
  if (!filenames.length) return;
  if (!window.confirm(`Delete ${filenames.length} selected mod(s)?`)) return;
  await Promise.all(filenames.map((fn) => api.deleteOfflineMod(selected.value.id, fn)));
  await loadMods();
}

async function browseMods() {
  if (!selected.value) return;
  const picked = await pickFiles({ multiple: true, filters: [JAR_FILTER] });
  if (!picked || !picked.length) return;
  for (const path of picked) {
    await api.importOfflineModFile(selected.value.id, path);
  }
  await loadMods();
}

async function onDrop(event) {
  if (!selected.value) return;
  const files = event.dataTransfer?.files;
  if (!files || !files.length) return;
  for (const file of files) {
    if (!file.name.endsWith('.jar')) continue;
    const arrayBuffer = await file.arrayBuffer();
    const bytes = Array.from(new Uint8Array(arrayBuffer));
    await api.importOfflineModBytes(selected.value.id, file.name, bytes);
  }
  await loadMods();
}

async function searchModrinth() {
  const query = modrinthQuery.value.trim();
  if (!query || !selected.value) return;
  modSearchBusy.value = true;
  modSearchDone.value = false;
  try {
    const hits = await api.searchModrinthMods(
      query,
      selected.value.minecraftVersion,
      selected.value.modLoader.type
    );
    modResults.value = hits.map((hit) => ({
      ...hit,
      versionOptions: [],
      selectedVersionId: '',
      versionsLoading: true,
      versionsFailed: false,
    }));
    modSearchDone.value = true;
    for (const hit of modResults.value) {
      loadModVersions(hit);
    }
  } catch (e) {
    window.dispatchEvent(new CustomEvent('zircon-status', { detail: `Mod search error: ${e}` }));
  } finally {
    modSearchBusy.value = false;
  }
}

async function loadModVersions(hit) {
  try {
    const versions = await api.getModrinthVersions(
      hit.projectId,
      selected.value.minecraftVersion,
      selected.value.modLoader.type
    );
    hit.versionOptions = versions;
    hit.selectedVersionId = versions[0]?.id || '';
  } catch {
    hit.versionsFailed = true;
  } finally {
    hit.versionsLoading = false;
  }
}

async function installMod(hit) {
  if (!hit.selectedVersionId || !selected.value) return;
  installing.value = hit.projectId;
  try {
    await api.installModrinthVersion(selected.value.id, hit.selectedVersionId);
    await loadMods();
  } catch (e) {
    window.dispatchEvent(new CustomEvent('zircon-status', { detail: `Install failed: ${e}` }));
  } finally {
    installing.value = '';
  }
}

function openModrinthLink(hit) {
  const url = hit.projectUrl || `https://modrinth.com/project/${hit.slug || hit.projectId}`;
  api.openBrowserUrl(url).catch((err) => {
    console.warn('Failed to open external link:', err);
    window.open(url, '_blank', 'noopener,noreferrer');
  });
}

async function onShaderpackChange() {
  if (!selectedDir.value) return;
  await api.setActiveShaderpack(selectedDir.value, activeShaderpack.value);
  await loadPacks();
}

async function togglePack(filename) {
  if (!selectedDir.value) return;
  const current = packs.value.activeResourcepacks || [];
  const next = current.includes(filename)
    ? current.filter((f) => f !== filename)
    : [...current, filename];
  await api.setActiveResourcepacks(selectedDir.value, next);
  await loadPacks();
}

async function addLocalPack(kind) {
  if (!selectedDir.value) return;
  const picked = await pickFiles({ multiple: true, filters: [PACK_FILTER] });
  if (!picked || !picked.length) return;
  for (const p of picked) {
    await api.importInstancePack(selectedDir.value, kind, p);
  }
  await loadPacks();
}

async function openNewInstance() {
  newForm.value = {
    name: '',
    mcVersion: mcVersions.value[0] || '1.20.4',
    loaderType: loaderTypes.value[0] || 'fabric',
    loaderVersion: '',
  };
  showNewDialog.value = true;
}

async function createInstance() {
  if (!newForm.value.name.trim()) return;
  creating.value = true;
  try {
    const created = await api.createOfflineInstance({
      name: newForm.value.name.trim(),
      minecraftVersion: newForm.value.mcVersion,
      modLoader: {
        type: newForm.value.loaderType,
        version: newForm.value.loaderVersion || undefined,
      },
    });
    showNewDialog.value = false;
    await loadInstances();
    await selectInstance(created);
  } catch (e) {
    window.dispatchEvent(new CustomEvent('zircon-status', { detail: `Create error: ${e}` }));
  } finally {
    creating.value = false;
  }
}

let unlistenFileDrop;

onMounted(async () => {
  try {
    const meta = await api.getLauncherMetadata();
    mcVersions.value = meta.minecraftVersions || ['1.21.4', '1.20.4', '1.19.4'];
    loaderTypes.value = meta.loaderTypes || ['fabric', 'quilt', 'forge', 'neoforge', 'vanilla'];
  } catch {
    mcVersions.value = ['1.21.4', '1.20.4', '1.19.4'];
    loaderTypes.value = ['fabric', 'quilt', 'forge', 'neoforge', 'vanilla'];
  }
  await loadInstances();

  try {
    const webview = getCurrentWebview();
    unlistenFileDrop = await webview.onDragDropEvent(async (event) => {
      if (event.payload.type === 'drop') {
        const paths = event.payload.paths;
        if (selected.value && paths && paths.length) {
          for (const path of paths) {
            if (path.endsWith('.jar')) {
              await api.importOfflineModFile(selected.value.id, path);
            } else if (path.endsWith('.zip')) {
              await api.importInstancePack(selectedDir.value, 'resource', path);
            }
          }
          await Promise.all([loadMods(), loadPacks()]);
        }
      }
    });
  } catch (err) {
    console.warn('Native drag-drop listener unavailable:', err);
  }
});

onBeforeUnmount(() => {
  if (unlistenFileDrop) unlistenFileDrop();
});
</script>
