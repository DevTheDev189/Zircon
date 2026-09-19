<template>
  <div
    v-if="open"
    class="fixed inset-0 z-50 flex items-center justify-center p-4 bg-black/80 backdrop-blur-md transition-all duration-300"
    @click.self="handleClose"
  >
    <div
      class="w-full max-w-2xl bg-[#0b0f17]/95 border border-slate-800/90 rounded-2xl shadow-2xl shadow-cyan-950/30 overflow-hidden flex flex-col animate-in fade-in zoom-in-95 duration-200 max-h-[90vh]"
    >
      <!-- Header -->
      <div class="px-6 py-5 border-b border-slate-800/80 flex items-center justify-between bg-gradient-to-r from-slate-900/90 via-slate-900/50 to-cyan-950/20">
        <div class="flex items-center gap-3">
          <div class="w-10 h-10 rounded-xl bg-cyan-500/10 border border-cyan-500/30 flex items-center justify-center text-cyan-400 shadow-inner">
            <svg class="w-5 h-5" fill="none" viewBox="0 0 24 24" stroke="currentColor">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M4 16v1a3 3 0 003 3h10a3 3 0 003-3v-1m-4-8l-4-4m0 0L8 8m4-4v12" />
            </svg>
          </div>
          <div>
            <h2 class="text-base font-bold text-slate-100 flex items-center gap-2">
              Import Mod Setup
              <span class="text-[10px] font-mono px-2 py-0.5 rounded-full bg-cyan-500/10 text-cyan-400 border border-cyan-500/20">Universal Recipe</span>
            </h2>
            <p class="text-xs text-slate-400">Paste a Zircon Share Code, BOM JSON, or drop a manifest</p>
          </div>
        </div>
        <button
          class="text-slate-500 hover:text-slate-300 p-1.5 rounded-lg hover:bg-slate-800/60 transition-colors"
          @click="handleClose"
        >
          <svg class="w-5 h-5" fill="none" viewBox="0 0 24 24" stroke="currentColor">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 18L18 6M6 6l12 12" />
          </svg>
        </button>
      </div>

      <!-- Body Content -->
      <div class="p-6 overflow-y-auto space-y-5 flex-1">
        <!-- Input Area -->
        <div>
          <label class="block text-xs font-semibold text-slate-300 mb-1.5">Paste Share Code or JSON Manifest</label>
          <textarea
            v-model="rawInput"
            rows="3"
            placeholder="e.g. zircon://setup/... or paste raw JSON"
            class="w-full bg-[#070b10] border border-slate-700/80 rounded-xl p-3 text-xs font-mono text-cyan-300 placeholder:text-slate-600 resize-none focus:outline-none focus:border-cyan-500"
            @input="handleInput"
          ></textarea>
        </div>

        <!-- Preview Card -->
        <div v-if="preview" class="bg-slate-900/60 border border-slate-800 rounded-xl p-4 space-y-3">
          <div class="flex items-center justify-between border-b border-slate-800/80 pb-3">
            <div>
              <h3 class="text-sm font-bold text-white flex items-center gap-2">
                {{ preview.incomingBom?.serverTitle || 'Mod Setup' }}
                <span class="text-[11px] font-mono px-2 py-0.5 rounded-full bg-slate-800 text-slate-300">
                  MC {{ preview.incomingBom?.minecraftVersion }}
                </span>
                <span v-if="preview.incomingBom?.modLoader" class="text-[11px] font-mono px-2 py-0.5 rounded-full bg-cyan-950 text-cyan-300 border border-cyan-800">
                  {{ preview.incomingBom?.modLoader?.type }} {{ preview.incomingBom?.modLoader?.version }}
                </span>
              </h3>
              <p class="text-xs text-slate-400 mt-1">Contains {{ preview.incomingBom?.mods?.length || 0 }} mods</p>
            </div>
          </div>

          <!-- Diff Breakdown if target instance exists -->
          <div v-if="preview.diff" class="grid grid-cols-4 gap-2 pt-1 text-center text-xs">
            <div class="bg-emerald-950/30 border border-emerald-800/40 rounded-lg p-2">
              <div class="text-emerald-400 font-bold text-sm">+{{ preview.diff.toAdd?.length || 0 }}</div>
              <div class="text-[10px] text-slate-400">To Add</div>
            </div>
            <div class="bg-amber-950/30 border border-amber-800/40 rounded-lg p-2">
              <div class="text-amber-400 font-bold text-sm">~{{ preview.diff.toUpdate?.length || 0 }}</div>
              <div class="text-[10px] text-slate-400">To Update</div>
            </div>
            <div class="bg-slate-800/30 border border-slate-700/40 rounded-lg p-2">
              <div class="text-slate-300 font-bold text-sm">{{ preview.diff.unchanged?.length || 0 }}</div>
              <div class="text-[10px] text-slate-400">Unchanged</div>
            </div>
            <div class="bg-rose-950/30 border border-rose-800/40 rounded-lg p-2">
              <div class="text-rose-400 font-bold text-sm">-{{ preview.diff.toRemove?.length || 0 }}</div>
              <div class="text-[10px] text-slate-400">Removed on Replace</div>
            </div>
          </div>

          <!-- Strategy Selection -->
          <div class="space-y-2 pt-2">
            <label class="block text-xs font-semibold text-slate-300">Import Strategy</label>
            <div class="space-y-2">
              <label class="flex items-start gap-3 p-3 rounded-xl border cursor-pointer transition-all"
                :class="strategy === 'new_instance' ? 'bg-cyan-950/30 border-cyan-500/50' : 'bg-slate-950/40 border-slate-800 hover:border-slate-700'">
                <input type="radio" v-model="strategy" value="new_instance" class="mt-0.5 zircon-radio" />
                <div>
                  <div class="text-xs font-bold text-slate-200">Create New Offline Instance (Recommended)</div>
                  <div class="text-[11px] text-slate-400">Creates an isolated profile with these exact mods. Leaves existing instances untouched.</div>
                </div>
              </label>

              <label v-if="targetInstance" class="flex items-start gap-3 p-3 rounded-xl border cursor-pointer transition-all"
                :class="strategy === 'merge' ? 'bg-cyan-950/30 border-cyan-500/50' : 'bg-slate-950/40 border-slate-800 hover:border-slate-700'">
                <input type="radio" v-model="strategy" value="merge" class="mt-0.5 zircon-radio" />
                <div>
                  <div class="text-xs font-bold text-slate-200">Merge into "{{ targetInstance.name }}"</div>
                  <div class="text-[11px] text-slate-400">Adds missing mods to this instance without deleting your current client mods.</div>
                </div>
              </label>

              <label v-if="targetInstance" class="flex items-start gap-3 p-3 rounded-xl border cursor-pointer transition-all"
                :class="strategy === 'replace' ? 'bg-cyan-950/30 border-cyan-500/50' : 'bg-slate-950/40 border-slate-800 hover:border-slate-700'">
                <input type="radio" v-model="strategy" value="replace" class="mt-0.5 zircon-radio" />
                <div>
                  <div class="text-xs font-bold text-slate-200">Replace & Mirror "{{ targetInstance.name }}"</div>
                  <div class="text-[11px] text-slate-400">Guarantees an exact match with the sender. Automatically saves a safety backup snapshot.</div>
                </div>
              </label>
            </div>
          </div>
        </div>

        <div v-if="parseError" class="p-3 rounded-xl bg-rose-950/40 border border-rose-800/60 text-xs text-rose-300">
          {{ parseError }}
        </div>
      </div>

      <!-- Footer -->
      <div class="px-6 py-4 border-t border-slate-800/80 bg-slate-950/60 flex items-center justify-between">
        <button class="z-btn-ghost text-xs px-4 py-2 rounded-xl font-semibold" @click="handleClose">
          Cancel
        </button>
        <button
          class="z-btn-primary text-xs px-5 py-2.5 rounded-xl font-bold flex items-center gap-2"
          :disabled="!preview || applying"
          @click="handleApply"
        >
          <span v-if="applying" class="w-3 h-3 border-2 border-white border-t-transparent rounded-full animate-spin"></span>
          <span>{{ applying ? 'Downloading & Installing...' : 'Apply & Download Mods' }}</span>
        </button>
      </div>
    </div>
  </div>
</template>

<script setup>
import { ref, watch } from 'vue';
import { api } from '../lib/api.js';

const props = defineProps({
  open: { type: Boolean, default: false },
  targetInstance: { type: Object, default: null },
});

const emit = defineEmits(['close', 'imported']);

const rawInput = ref('');
const preview = ref(null);
const parseError = ref('');
const strategy = ref('new_instance');
const applying = ref(false);

watch(
  () => props.open,
  (isOpen) => {
    if (isOpen) {
      rawInput.value = '';
      preview.value = null;
      parseError.value = '';
      strategy.value = props.targetInstance ? 'merge' : 'new_instance';
    }
  }
);

let debounceTimer = null;
function handleInput() {
  clearTimeout(debounceTimer);
  debounceTimer = setTimeout(() => {
    runPreview();
  }, 300);
}

async function runPreview() {
  const code = rawInput.value.trim();
  if (!code) {
    preview.value = null;
    parseError.value = '';
    return;
  }

  try {
    parseError.value = '';
    const res = await api.previewImportSetup(code, props.targetInstance?.id);
    preview.value = res;
  } catch (e) {
    parseError.value = `Failed to parse setup: ${e}`;
    preview.value = null;
  }
}

async function handleApply() {
  if (!preview.value?.incomingBom) return;
  applying.value = true;
  try {
    const res = await api.applyImportSetup(
      preview.value.incomingBom,
      strategy.value,
      props.targetInstance?.id,
      null
    );
    window.dispatchEvent(
      new CustomEvent('zircon-status', { detail: `Setup applied successfully!` })
    );
    emit('imported', res);
    handleClose();
  } catch (e) {
    window.dispatchEvent(new CustomEvent('zircon-status', { detail: `Import failed: ${e}` }));
  } finally {
    applying.value = false;
  }
}

function handleClose() {
  emit('close');
}
</script>
