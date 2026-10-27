<template>
  <Transition name="fade">
    <div
      v-if="open"
      class="fixed inset-0 z-50 flex items-center justify-center p-4 bg-black/80 backdrop-blur-md"
      @click.self="$emit('close')"
    >
      <div
        class="w-full max-w-4xl bg-[#0c0d12] border border-neutral-800 rounded-3xl shadow-[0_24px_70px_rgba(0,0,0,0.9),0_0_30px_rgba(71,210,201,0.15)] overflow-hidden flex flex-col max-h-[85vh] relative text-slate-100"
      >
        <!-- Modal Header -->
        <div class="px-6 py-5 border-b border-neutral-800/80 flex items-center justify-between bg-neutral-950/60">
          <div class="flex items-center gap-3">
            <div class="w-10 h-10 rounded-2xl bg-cyan-500/15 border border-cyan-400/30 flex items-center justify-center text-cyan-300 shadow-[0_0_15px_rgba(71,210,201,0.2)]">
              <svg class="w-5 h-5" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 6V4m0 2a2 2 0 100 4m0-4a2 2 0 110 4m-6 8a2 2 0 100-4m0 4a2 2 0 110-4m0 4v2m0-6V4m6 6v10m6-2a2 2 0 100-4m0 4a2 2 0 110-4m0 4v2m0-6V4" />
              </svg>
            </div>
            <div>
              <h2 class="text-base font-black tracking-tight text-white flex items-center gap-2">
                Global Keybind Library & Sync
                <span class="text-[10px] font-mono px-2 py-0.5 rounded-full bg-cyan-500/15 border border-cyan-400/30 text-cyan-300 font-bold">
                  v0.9.0
                </span>
              </h2>
              <p class="text-xs text-slate-400 mt-0.5">
                Curate, customize, and synchronize your Minecraft controls across all your instances.
              </p>
            </div>
          </div>
          <button
            type="button"
            class="p-2 rounded-xl text-slate-400 hover:text-white hover:bg-neutral-800 transition"
            @click="$emit('close')"
          >
            <svg class="w-5 h-5" fill="none" viewBox="0 0 24 24" stroke="currentColor">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 18L18 6M6 6l12 12" />
            </svg>
          </button>
        </div>

        <!-- Notification Banner -->
        <div v-if="statusMessage" :class="['px-6 py-2.5 text-xs font-semibold flex items-center justify-between border-b', statusIsError ? 'bg-rose-950/60 text-rose-300 border-rose-800/60' : 'bg-cyan-950/50 text-cyan-200 border-cyan-800/50']">
          <span>{{ statusMessage }}</span>
          <button type="button" class="text-xs opacity-75 hover:opacity-100" @click="statusMessage = ''">✕</button>
        </div>

        <!-- Main Body: Two Column Split View -->
        <div class="flex-1 flex overflow-hidden min-h-[420px]">
          <!-- Left Column: Profile Selector & Import Actions -->
          <div class="w-72 border-r border-neutral-800/80 bg-neutral-950/40 p-4 flex flex-col gap-3 shrink-0">
            <div class="flex items-center justify-between">
              <span class="text-[11px] font-mono font-bold uppercase tracking-wider text-slate-400">Profiles</span>
              <button
                type="button"
                class="px-2.5 py-1 rounded-lg bg-cyan-500/20 hover:bg-cyan-500/30 text-cyan-300 border border-cyan-400/40 text-[11px] font-bold transition flex items-center gap-1"
                @click="createNewProfile"
              >
                + New
              </button>
            </div>

            <!-- Profile List -->
            <div class="flex-1 overflow-y-auto space-y-2 pr-1">
              <div v-if="loadingProfiles" class="text-center py-8 text-xs text-slate-500 animate-pulse">
                Loading keybind profiles...
              </div>
              <div v-else-if="profiles.length === 0" class="text-center py-8 text-xs text-slate-500">
                No profiles saved yet. Click "+ New" or extract from an instance below.
              </div>
              <div
                v-for="prof in profiles"
                :key="prof.id"
                :class="[
                  'p-3 rounded-2xl border transition cursor-pointer flex flex-col gap-1',
                  selectedProfileId === prof.id
                    ? 'bg-cyan-500/10 border-cyan-400/70 shadow-[0_0_15px_rgba(71,210,201,0.15)] ring-1 ring-cyan-400/40'
                    : 'bg-neutral-900/60 border-neutral-800/80 hover:border-neutral-700 hover:bg-neutral-900/90'
                ]"
                @click="selectProfile(prof.id)"
              >
                <div class="flex items-center justify-between">
                  <span class="text-xs font-bold text-white truncate">{{ prof.name }}</span>
                  <span class="text-[10px] font-mono text-cyan-300 font-bold bg-cyan-950/80 px-1.5 py-0.5 rounded border border-cyan-500/30">
                    {{ prof.count }} keys
                  </span>
                </div>
                <p v-if="prof.description" class="text-[11px] text-slate-400 line-clamp-1">
                  {{ prof.description }}
                </p>
              </div>
            </div>

            <!-- Import Source Controls -->
            <div class="pt-3 border-t border-neutral-800/80 flex flex-col gap-2">
              <span class="text-[10px] font-mono font-bold uppercase tracking-wider text-slate-400">Capture From Source</span>
              <button
                v-if="targetInstance"
                type="button"
                class="w-full px-3 py-2 rounded-xl bg-neutral-900 hover:bg-neutral-800 border border-neutral-700/60 text-slate-200 text-xs font-semibold flex items-center gap-2 transition"
                @click="extractFromCurrentInstance"
              >
                <span class="truncate">Import from {{ targetInstance.name || targetInstance.id }}</span>
              </button>
              <button
                type="button"
                class="w-full px-3 py-2 rounded-xl bg-neutral-900 hover:bg-neutral-800 border border-neutral-700/60 text-slate-300 text-xs font-semibold flex items-center justify-center gap-2 transition"
                @click="pickExternalOptionsFile"
              >
                <svg class="w-3.5 h-3.5 text-cyan-400" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                  <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M4 16v1a3 3 0 003 3h10a3 3 0 003-3v-1m-4-8l-4-4m0 0L8 8m4-4v12" />
                </svg>
                <span>Browse options.txt...</span>
              </button>
            </div>
          </div>

          <!-- Right Column: Profile Editor & Keybind Table -->
          <div class="flex-1 flex flex-col p-5 overflow-hidden bg-neutral-950/20">
            <template v-if="activeProfile">
              <!-- Profile Metadata Fields -->
              <div class="grid grid-cols-1 sm:grid-cols-2 gap-3 mb-4">
                <div>
                  <label class="block text-[11px] font-bold text-slate-400 mb-1">Profile Name</label>
                  <input
                    v-model="activeProfile.name"
                    type="text"
                    placeholder="e.g. FPS Competitive Layout"
                    class="w-full px-3 py-1.5 rounded-xl bg-neutral-900/90 border border-neutral-700 text-xs text-white focus:outline-none focus:border-cyan-400 focus:ring-1 focus:ring-cyan-400 transition"
                  />
                </div>
                <div>
                  <label class="block text-[11px] font-bold text-slate-400 mb-1">Description (Optional)</label>
                  <input
                    v-model="activeProfile.description"
                    type="text"
                    placeholder="e.g. Optimized for Bedwars and Movement"
                    class="w-full px-3 py-1.5 rounded-xl bg-neutral-900/90 border border-neutral-700 text-xs text-white focus:outline-none focus:border-cyan-400 focus:ring-1 focus:ring-cyan-400 transition"
                  />
                </div>
              </div>

              <!-- Search and Filter Bar -->
              <div class="flex items-center gap-3 mb-3">
                <div class="relative flex-1">
                  <input
                    v-model="searchQuery"
                    type="text"
                    placeholder="Filter keybinds (e.g. jump, sprint, attack, W)..."
                    class="w-full pl-9 pr-3 py-1.5 rounded-xl bg-neutral-900/90 border border-neutral-700 text-xs text-white placeholder-slate-500 focus:outline-none focus:border-cyan-400 transition"
                  />
                  <svg class="w-4 h-4 text-slate-500 absolute left-3 top-2.5" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                    <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M21 21l-6-6m2-5a7 7 0 11-14 0 7 7 0 0114 0z" />
                  </svg>
                </div>
                <div class="text-[11px] font-mono text-slate-400 shrink-0">
                  Showing {{ filteredKeybinds.length }} of {{ Object.keys(activeProfile.keybinds || {}).length }}
                </div>
              </div>

              <!-- Keybinds Table -->
              <div class="flex-1 overflow-y-auto border border-neutral-800 rounded-2xl bg-neutral-900/40">
                <table class="w-full text-left text-xs border-collapse">
                  <thead class="sticky top-0 bg-neutral-950/90 backdrop-blur-sm border-b border-neutral-800 text-[10px] font-mono uppercase text-slate-400">
                    <tr>
                      <th class="py-2.5 px-4 font-bold">Action / Trigger</th>
                      <th class="py-2.5 px-4 font-bold">Options Key Identifier</th>
                      <th class="py-2.5 px-4 font-bold text-right">Assigned Key</th>
                    </tr>
                  </thead>
                  <tbody class="divide-y divide-neutral-800/60">
                    <tr
                      v-for="item in filteredKeybinds"
                      :key="item.key"
                      class="hover:bg-neutral-800/30 transition-colors"
                    >
                      <td class="py-2 px-4 font-semibold text-slate-200">
                        {{ formatFriendlyAction(item.key) }}
                      </td>
                      <td class="py-2 px-4 font-mono text-[11px] text-slate-500">
                        {{ item.key }}
                      </td>
                      <td class="py-2 px-4 text-right">
                        <span class="inline-block px-2.5 py-1 rounded-lg bg-neutral-800 border border-neutral-700 font-mono text-[11px] font-bold text-cyan-300 shadow-sm">
                          {{ formatFriendlyKey(item.value) }}
                        </span>
                      </td>
                    </tr>
                    <tr v-if="filteredKeybinds.length === 0">
                      <td colspan="3" class="text-center py-8 text-xs text-slate-500">
                        No matching keybinds found.
                      </td>
                    </tr>
                  </tbody>
                </table>
              </div>
            </template>
            <div v-else class="flex-1 flex flex-col items-center justify-center text-center p-8 text-slate-500">
              <svg class="w-12 h-12 text-slate-700 mb-3" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="1.5" d="M12 6V4m0 2a2 2 0 100 4m0-4a2 2 0 110 4m-6 8a2 2 0 100-4m0 4a2 2 0 110-4m0 4v2m0-6V4m6 6v10m6-2a2 2 0 100-4m0 4a2 2 0 110-4m0 4v2m0-6V4" />
              </svg>
              <p class="text-xs font-semibold text-slate-400">Select or create a keybind profile to view and edit its mappings.</p>
            </div>
          </div>
        </div>

        <!-- Modal Footer: Sync Actions -->
        <div class="px-6 py-4 border-t border-neutral-800/80 bg-neutral-950/80 flex items-center justify-between gap-4">
          <div class="flex items-center gap-3">
            <button
              v-if="activeProfile"
              type="button"
              class="px-3 py-1.5 rounded-xl bg-rose-500/15 hover:bg-rose-500/25 text-rose-300 border border-rose-500/30 text-xs font-bold transition"
              @click="deleteCurrentProfile"
            >
              Delete
            </button>
            <button
              v-if="activeProfile"
              type="button"
              class="px-4 py-1.5 rounded-xl bg-neutral-800 hover:bg-neutral-700 text-white text-xs font-bold transition"
              @click="saveCurrentProfile"
            >
              Save Profile
            </button>
          </div>

          <div class="flex items-center gap-3">
            <!-- Apply to Instance dropdown -->
            <div v-if="activeProfile && availableInstances.length > 0" class="flex items-center gap-2">
              <select
                v-model="targetSyncInstanceId"
                class="px-3 py-1.5 rounded-xl bg-neutral-900 border border-neutral-700 text-xs text-slate-200 focus:outline-none focus:border-cyan-400"
              >
                <option value="" disabled>Select target instance...</option>
                <option
                  v-for="inst in availableInstances"
                  :key="inst.id"
                  :value="inst.id"
                >
                  {{ inst.name || inst.id }}
                </option>
              </select>
              <button
                type="button"
                :disabled="!targetSyncInstanceId || applyingSync"
                class="px-4 py-1.5 rounded-xl bg-cyan-500 hover:bg-cyan-400 disabled:opacity-50 text-slate-950 font-bold text-xs transition shadow-[0_0_15px_rgba(71,210,201,0.25)] flex items-center gap-1.5"
                @click="applyToSelectedInstance"
              >
                <span v-if="applyingSync" class="animate-spin text-slate-950">◌</span>
                <span>{{ applyingSync ? 'Syncing...' : 'Sync to Instance' }}</span>
              </button>
            </div>
            <button
              type="button"
              class="px-4 py-1.5 rounded-xl bg-neutral-800 hover:bg-neutral-700 text-slate-300 text-xs font-semibold transition"
              @click="$emit('close')"
            >
              Close
            </button>
          </div>
        </div>
      </div>
    </div>
  </Transition>
</template>

<script setup>
import { ref, computed, onMounted, watch } from 'vue';
import { api } from '../lib/api';
import { askConfirm } from '../lib/confirm';

const props = defineProps({
  open: { type: Boolean, default: false },
  targetInstance: { type: Object, default: null },
});

const emit = defineEmits(['close', 'applied']);

const profiles = ref([]);
const loadingProfiles = ref(false);
const selectedProfileId = ref('');
const activeProfile = ref(null);
const searchQuery = ref('');
const statusMessage = ref('');
const statusIsError = ref(false);
const targetSyncInstanceId = ref('');
const applyingSync = ref(false);
const availableInstances = ref([]);

const filteredKeybinds = computed(() => {
  if (!activeProfile.value?.keybinds) return [];
  const entries = Object.entries(activeProfile.value.keybinds).map(([k, v]) => ({ key: k, value: v }));
  if (!searchQuery.value.trim()) return entries;
  const q = searchQuery.value.toLowerCase();
  return entries.filter(
    (item) =>
      item.key.toLowerCase().includes(q) ||
      item.value.toLowerCase().includes(q) ||
      formatFriendlyAction(item.key).toLowerCase().includes(q) ||
      formatFriendlyKey(item.value).toLowerCase().includes(q)
  );
});

onMounted(() => {
  if (props.open) {
    initModal();
  }
});

watch(
  () => props.open,
  (isOpen) => {
    if (isOpen) {
      initModal();
    }
  }
);

async function initModal() {
  statusMessage.value = '';
  await loadProfiles();
  await loadInstances();
  if (props.targetInstance?.id) {
    targetSyncInstanceId.value = props.targetInstance.id;
  }
}

async function loadProfiles() {
  loadingProfiles.value = true;
  try {
    profiles.value = await api.listKeybindProfiles();
    if (profiles.value.length > 0 && !selectedProfileId.value) {
      await selectProfile(profiles.value[0].id);
    }
  } catch (err) {
    console.error('Failed to load keybind profiles:', err);
  } finally {
    loadingProfiles.value = false;
  }
}

async function loadInstances() {
  try {
    const list = await api.listOfflineInstances();
    availableInstances.value = list || [];
  } catch {
    availableInstances.value = [];
  }
}

async function selectProfile(id) {
  selectedProfileId.value = id;
  try {
    activeProfile.value = await api.getKeybindProfile(id);
  } catch (err) {
    showStatus(`Failed to load profile: ${err}`, true);
  }
}

function createNewProfile() {
  const newId = `profile_${Date.now()}`;
  activeProfile.value = {
    id: newId,
    name: 'New Custom Keybinds',
    description: '',
    created_at: Math.floor(Date.now() / 1000),
    updated_at: Math.floor(Date.now() / 1000),
    keybinds: {
      'key_key.forward': 'key.keyboard.w',
      'key_key.left': 'key.keyboard.a',
      'key_key.back': 'key.keyboard.s',
      'key_key.right': 'key.keyboard.d',
      'key_key.jump': 'key.keyboard.space',
      'key_key.sneak': 'key.keyboard.left.shift',
      'key_key.sprint': 'key.keyboard.left.control',
      'key_key.attack': 'key.mouse.left',
      'key_key.use': 'key.mouse.right',
      'key_key.inventory': 'key.keyboard.e',
      'key_key.drop': 'key.keyboard.q',
      'key_key.chat': 'key.keyboard.t',
    },
  };
  selectedProfileId.value = newId;
}

async function saveCurrentProfile() {
  if (!activeProfile.value) return;
  try {
    const saved = await api.saveKeybindProfile(activeProfile.value);
    activeProfile.value = saved;
    selectedProfileId.value = saved.id;
    await loadProfiles();
    showStatus(`Saved profile "${saved.name}".`);
  } catch (err) {
    showStatus(`Failed to save profile: ${err}`, true);
  }
}

async function deleteCurrentProfile() {
  if (!activeProfile.value) return;
  const ok = await askConfirm({
    title: 'Delete Keybind Profile',
    message: `Are you sure you want to delete profile "${activeProfile.value.name}"? This action cannot be undone.`,
    confirmText: 'Delete Profile',
    isDanger: true,
  });
  if (!ok) return;
  try {
    await api.deleteKeybindProfile(activeProfile.value.id);
    activeProfile.value = null;
    selectedProfileId.value = '';
    await loadProfiles();
    showStatus('Profile deleted.');
  } catch (err) {
    showStatus(`Failed to delete profile: ${err}`, true);
  }
}

async function extractFromCurrentInstance() {
  const targetId = props.targetInstance?.id || targetSyncInstanceId.value;
  if (!targetId) {
    showStatus('No instance selected to capture keys from.', true);
    return;
  }
  try {
    const binds = await api.extractInstanceKeybinds(targetId);
    if (Object.keys(binds).length === 0) {
      showStatus('No keybinds found in this instance (options.txt might not exist yet).', true);
      return;
    }
    const instName = props.targetInstance?.name || targetId;
    activeProfile.value = {
      id: `captured_${Date.now()}`,
      name: `${instName} Keybinds`,
      description: `Captured from instance ${instName}`,
      created_at: Math.floor(Date.now() / 1000),
      updated_at: Math.floor(Date.now() / 1000),
      keybinds: binds,
    };
    await saveCurrentProfile();
    showStatus(`Captured ${Object.keys(binds).length} keybinds from ${instName}!`);
  } catch (err) {
    showStatus(`Extraction failed: ${err}`, true);
  }
}

async function pickExternalOptionsFile() {
  try {
    const picked = await api.pickFile({
      title: 'Select Minecraft options.txt',
      filters: [{ name: 'Minecraft Options', extensions: ['txt'] }],
    });
    if (!picked) return;
    const binds = await api.extractOptionsFileKeybinds(picked);
    if (Object.keys(binds).length === 0) {
      showStatus('No keybind entries found in selected file.', true);
      return;
    }
    activeProfile.value = {
      id: `imported_${Date.now()}`,
      name: 'Imported options.txt Layout',
      description: `Imported from ${picked}`,
      created_at: Math.floor(Date.now() / 1000),
      updated_at: Math.floor(Date.now() / 1000),
      keybinds: binds,
    };
    await saveCurrentProfile();
    showStatus(`Successfully imported ${Object.keys(binds).length} keybinds from options.txt!`);
  } catch (err) {
    showStatus(`Import error: ${err}`, true);
  }
}

async function applyToSelectedInstance() {
  if (!activeProfile.value?.id || !targetSyncInstanceId.value) return;
  applyingSync.value = true;
  try {
    const res = await api.applyKeybindProfileToInstance(activeProfile.value.id, targetSyncInstanceId.value);
    showStatus(`Synced ${res.keys_applied} keys (${res.keys_updated} updated, ${res.keys_added} added). Backup created!`);
    emit('applied', res);
  } catch (err) {
    showStatus(`Failed to sync keybinds: ${err}`, true);
  } finally {
    applyingSync.value = false;
  }
}

function showStatus(msg, isError = false) {
  statusMessage.value = msg;
  statusIsError.value = isError;
}

function formatFriendlyAction(key) {
  const map = {
    'key_key.forward': 'Walk Forward',
    'key_key.back': 'Walk Backward',
    'key_key.left': 'Strafe Left',
    'key_key.right': 'Strafe Right',
    'key_key.jump': 'Jump',
    'key_key.sneak': 'Sneak',
    'key_key.sprint': 'Sprint',
    'key_key.attack': 'Attack / Destroy',
    'key_key.use': 'Use Item / Place Block',
    'key_key.pickItem': 'Pick Block',
    'key_key.drop': 'Drop Selected Item',
    'key_key.inventory': 'Open / Close Inventory',
    'key_key.chat': 'Open Chat',
    'key_key.playerlist': 'List Players',
    'key_key.command': 'Open Command',
    'key_key.screenshot': 'Take Screenshot',
    'key_key.togglePerspective': 'Toggle Perspective (F5)',
    'key_key.smoothCamera': 'Cinematic Camera',
    'key_key.fullscreen': 'Toggle Fullscreen',
    'key_key.spectatorOutlines': 'Highlight Players (Spectator)',
    'key_key.swapOffhand': 'Swap Item with Offhand',
    'key_key.saveToolbarActivator': 'Save Toolbar Activator',
    'key_key.loadToolbarActivator': 'Load Toolbar Activator',
    'key_key.advancements': 'Advancements',
  };
  if (map[key]) return map[key];
  if (key.startsWith('key_key.hotbar.')) {
    return `Hotbar Slot ${key.replace('key_key.hotbar.', '')}`;
  }
  return key.replace('key_', '').replace('key.', '').replace(/_/g, ' ');
}

function formatFriendlyKey(val) {
  if (!val) return 'None';
  return val
    .replace('key.keyboard.', '')
    .replace('key.mouse.', 'Mouse ')
    .replace('left.shift', 'Left Shift')
    .replace('right.shift', 'Right Shift')
    .replace('left.control', 'Left Ctrl')
    .replace('right.control', 'Right Ctrl')
    .replace('left.alt', 'Left Alt')
    .replace('right.alt', 'Right Alt')
    .replace('space', 'SPACE')
    .toUpperCase();
}
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
