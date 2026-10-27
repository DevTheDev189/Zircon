<template>
  <div
    v-if="open && instance"
    class="fixed inset-0 z-50 flex items-center justify-center p-4 bg-black/80 backdrop-blur-md transition-all duration-300"
    @click.self="$emit('close')"
  >
    <div
      class="w-full max-w-lg bg-[#0b0f17]/95 border border-slate-800/90 rounded-2xl shadow-2xl shadow-cyan-950/30 overflow-hidden flex flex-col animate-in fade-in zoom-in-95 duration-200"
    >
      <!-- Header -->
      <div class="px-6 py-5 border-b border-slate-800/80 flex items-center justify-between bg-gradient-to-r from-slate-900/90 via-slate-900/50 to-cyan-950/20">
        <div class="flex items-center gap-3">
          <div class="w-10 h-10 rounded-xl bg-cyan-500/10 border border-cyan-500/30 flex items-center justify-center text-cyan-400 shadow-inner">
            <svg class="w-5 h-5" fill="none" viewBox="0 0 24 24" stroke="currentColor">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M11 5H6a2 2 0 00-2 2v11a2 2 0 002 2h11a2 2 0 002-2v-5m-1.414-9.414a2 2 0 112.828 2.828L11.828 15H9v-2.828l8.586-8.586z" />
            </svg>
          </div>
          <div>
            <h2 class="text-base font-bold text-slate-100 flex items-center gap-2">
              Instance Configuration & Version
              <span class="text-[10px] font-mono px-2 py-0.5 rounded-full bg-cyan-500/10 text-cyan-400 border border-cyan-500/20">Edit</span>
            </h2>
            <p class="text-xs text-slate-400">Configure Minecraft & modloader version before launching or sharing</p>
          </div>
        </div>
        <button
          class="text-slate-500 hover:text-slate-300 p-1.5 rounded-lg hover:bg-slate-800/60 transition-colors cursor-pointer"
          @click="$emit('close')"
        >
          <svg class="w-5 h-5" fill="none" viewBox="0 0 24 24" stroke="currentColor">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 18L18 6M6 6l12 12" />
          </svg>
        </button>
      </div>

      <!-- Form Body -->
      <div class="p-6 overflow-y-auto space-y-4 max-h-[70vh]">
        <!-- Instance Name -->
        <div>
          <label class="block text-xs font-semibold text-slate-300 mb-1.5">Instance Name</label>
          <input
            v-model="form.name"
            class="z-input text-xs"
            placeholder="My Instance"
          />
        </div>

        <!-- Minecraft Version -->
        <div>
          <label class="block text-xs font-semibold text-slate-300 mb-1.5">Minecraft Version</label>
          <select
            v-model="form.mcVersion"
            class="z-input text-xs"
            @change="fetchLoaderVersions"
          >
            <option v-for="v in mcVersionOptions" :key="v" :value="v">{{ v }}</option>
          </select>
        </div>

        <!-- Mod Loader Type & Version Grid -->
        <div class="grid grid-cols-2 gap-3">
          <div>
            <label class="block text-xs font-semibold text-slate-300 mb-1.5">Mod Loader</label>
            <select
              v-model="form.loaderType"
              class="z-input text-xs capitalize"
              @change="fetchLoaderVersions"
            >
              <option v-for="l in loaderTypes" :key="l" :value="l" class="capitalize">{{ l }}</option>
            </select>
          </div>

          <div v-if="form.loaderType !== 'vanilla'">
            <div class="flex items-center justify-between mb-1.5">
              <label class="block text-xs font-semibold text-slate-300">Loader Version</label>
              <span v-if="loadingLoaderVersions" class="text-[10px] text-cyan-400 font-mono animate-pulse">Fetching builds...</span>
              <button
                v-else
                type="button"
                class="text-[10px] text-cyan-400 hover:text-cyan-300 underline font-mono cursor-pointer"
                @click="customVersionMode = !customVersionMode"
              >
                {{ customVersionMode ? 'Pick from list' : 'Custom version' }}
              </button>
            </div>

            <!-- Custom text input mode -->
            <input
              v-if="customVersionMode || !loaderVersions.length"
              v-model="form.loaderVersion"
              class="z-input text-xs font-mono"
              placeholder="e.g. 47.4.18"
            />

            <!-- Dropdown selector mode -->
            <select
              v-else
              v-model="form.loaderVersion"
              class="z-input text-xs font-mono"
            >
              <option v-for="lv in loaderVersions" :key="lv" :value="lv">
                {{ lv }} {{ lv === recommendedLoaderVersion ? '(Recommended)' : '' }}
              </option>
            </select>
          </div>
        </div>

        <!-- Loader Change Warning -->
        <div
          v-if="isLoaderChanged"
          class="p-3 bg-amber-500/10 border border-amber-500/30 rounded-xl flex items-start gap-2.5 text-amber-200 text-xs"
        >
          <svg class="w-4 h-4 text-amber-400 shrink-0 mt-0.5" fill="none" viewBox="0 0 24 24" stroke="currentColor">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 9v2m0 4h.01m-6.938 4h13.856c1.54 0 2.502-1.667 1.732-3L13.732 4c-.77-1.333-2.694-1.333-3.464 0L3.34 16c-.77 1.333.192 3 1.732 3z" />
          </svg>
          <div>
            <span class="font-bold block mb-0.5">Loader / Version Changed</span>
            <span class="text-amber-300/90 leading-relaxed">
              Changing the loader or game version will update launch parameters. Existing mod JARs in this instance may need to be updated or re-synced to run properly.
            </span>
          </div>
        </div>

        <div v-if="error" class="p-3 bg-rose-500/10 border border-rose-500/30 rounded-xl text-rose-300 text-xs">
          {{ error }}
        </div>
      </div>

      <!-- Footer Buttons -->
      <div class="px-6 py-4 border-t border-slate-800/80 bg-slate-950/40 flex items-center justify-between">
        <button
          type="button"
          class="z-btn-ghost text-xs px-4 py-2 rounded-xl font-semibold border border-slate-700/80 hover:border-slate-600 hover:text-white cursor-pointer"
          @click="$emit('close')"
        >
          Cancel
        </button>
        <button
          type="button"
          class="z-btn-accent text-xs font-bold px-5 py-2 rounded-xl shadow-md hover:shadow-cyan-500/25 flex items-center gap-1.5 cursor-pointer"
          :disabled="saving || !form.name.trim()"
          @click="saveInstance"
        >
          <span v-if="saving" class="w-3 h-3 border-2 border-slate-900 border-t-transparent rounded-full animate-spin"></span>
          <span>{{ saving ? 'Saving...' : 'Apply Changes' }}</span>
        </button>
      </div>
    </div>
  </div>
</template>

<script setup>
import { ref, computed, watch } from 'vue';
import { api } from '../../lib/api';

const props = defineProps({
  open: { type: Boolean, default: false },
  instance: { type: Object, default: null },
  mcVersions: { type: Array, default: () => [] }
});

const emit = defineEmits(['close', 'saved']);

const loaderTypes = ['fabric', 'forge', 'neoforge', 'quilt', 'vanilla'];

const form = ref({
  name: '',
  mcVersion: '1.20.1',
  loaderType: 'fabric',
  loaderVersion: ''
});

const loaderVersions = ref([]);
const recommendedLoaderVersion = ref('');
const loadingLoaderVersions = ref(false);
const customVersionMode = ref(false);
const saving = ref(false);
const error = ref('');

const mcVersionOptions = computed(() => {
  if (props.mcVersions && props.mcVersions.length) {
    return props.mcVersions;
  }
  return ['1.21.4', '1.21.1', '1.20.4', '1.20.1', '1.19.4', '1.18.2', '1.16.5', '1.12.2', '1.7.10'];
});

const isLoaderChanged = computed(() => {
  if (!props.instance) return false;
  const origLoader = (props.instance.modLoader?.type || 'vanilla').toLowerCase();
  const origVer = props.instance.modLoader?.version || '';
  const origMc = props.instance.minecraftVersion || '';
  return (
    form.value.loaderType.toLowerCase() !== origLoader ||
    form.value.loaderVersion !== origVer ||
    form.value.mcVersion !== origMc
  );
});

async function fetchLoaderVersions() {
  const loader = form.value.loaderType;
  const mc = form.value.mcVersion;
  if (!loader || loader === 'vanilla' || !mc) {
    loaderVersions.value = [];
    recommendedLoaderVersion.value = '';
    return;
  }

  loadingLoaderVersions.value = true;
  try {
    const res = await api.getLoaderVersions(loader, mc);
    loaderVersions.value = res?.versions || [];
    recommendedLoaderVersion.value = res?.recommended || '';

    // If current selected version is in list, keep it; otherwise if recommended exists, adopt it if version was empty
    if (!form.value.loaderVersion && recommendedLoaderVersion.value) {
      form.value.loaderVersion = recommendedLoaderVersion.value;
    } else if (form.value.loaderVersion && !loaderVersions.value.includes(form.value.loaderVersion)) {
      // Check if it's a custom version; if not in list, keep as custom
      customVersionMode.value = true;
    }
  } catch (e) {
    loaderVersions.value = [];
    recommendedLoaderVersion.value = '';
  } finally {
    loadingLoaderVersions.value = false;
  }
}

watch(
  () => props.open,
  (isOpen) => {
    if (isOpen && props.instance) {
      error.value = '';
      form.value.name = props.instance.name || '';
      form.value.mcVersion = props.instance.minecraftVersion || '1.20.1';
      form.value.loaderType = (props.instance.modLoader?.type || 'fabric').toLowerCase();
      form.value.loaderVersion = props.instance.modLoader?.version || '';
      customVersionMode.value = false;
      fetchLoaderVersions();
    }
  },
  { immediate: true }
);

async function saveInstance() {
  if (!props.instance || !form.value.name.trim()) return;
  saving.value = true;
  error.value = '';
  try {
    const updated = await api.updateOfflineInstance(props.instance.id, {
      name: form.value.name.trim(),
      mcVersion: form.value.mcVersion,
      loaderType: form.value.loaderType,
      loaderVersion: form.value.loaderType === 'vanilla' ? '' : form.value.loaderVersion
    });
    emit('saved', updated);
    emit('close');
  } catch (err) {
    error.value = typeof err === 'string' ? err : (err.message || 'Failed to update instance');
  } finally {
    saving.value = false;
  }
}
</script>
