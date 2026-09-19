<template>
  <div
    v-if="open"
    class="fixed inset-0 z-50 flex items-center justify-center p-4 bg-black/80 backdrop-blur-md transition-all duration-300"
    @click.self="handleClose"
  >
    <div
      class="w-full max-w-xl bg-[#0b0f17]/95 border border-slate-800/90 rounded-2xl shadow-2xl shadow-cyan-950/30 overflow-hidden flex flex-col animate-in fade-in zoom-in-95 duration-200 max-h-[90vh]"
    >
      <!-- Header -->
      <div class="px-6 py-5 border-b border-slate-800/80 flex items-center justify-between bg-gradient-to-r from-slate-900/90 via-slate-900/50 to-cyan-950/20">
        <div class="flex items-center gap-3">
          <div class="w-10 h-10 rounded-xl bg-cyan-500/10 border border-cyan-500/30 flex items-center justify-center text-cyan-400 shadow-inner">
            <svg class="w-5 h-5" fill="none" viewBox="0 0 24 24" stroke="currentColor">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 8v4l3 3m6-3a9 9 0 11-18 0 9 9 0 0118 0z" />
            </svg>
          </div>
          <div>
            <h2 class="text-base font-bold text-slate-100 flex items-center gap-2">
              Mod List Time Machine
              <span class="text-[10px] font-mono px-2 py-0.5 rounded-full bg-cyan-500/10 text-cyan-400 border border-cyan-500/20">Snapshots</span>
            </h2>
            <p class="text-xs text-slate-400">Save named recovery points and rollback instantly</p>
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
      <div class="p-6 overflow-y-auto space-y-4 flex-1">
        <!-- Create Snapshot Box -->
        <div class="bg-slate-900/70 border border-slate-800 rounded-xl p-4 space-y-3">
          <label class="block text-xs font-semibold text-slate-300">Save New Snapshot</label>
          <div class="flex gap-2">
            <input
              v-model="newLabel"
              type="text"
              placeholder="e.g. Before 1.21 update, Stable shaders"
              class="z-input flex-1 text-xs px-3 py-2 placeholder:text-slate-600 focus:border-cyan-500"
              @keyup.enter="handleCreateSnapshot"
            />
            <button
              class="z-btn-primary text-xs px-4 py-2 rounded-xl font-bold flex items-center gap-1.5 shrink-0"
              :disabled="creating || !newLabel.trim()"
              @click="handleCreateSnapshot"
            >
              <span v-if="creating" class="w-3 h-3 border-2 border-white border-t-transparent rounded-full animate-spin"></span>
              <span>{{ creating ? 'Saving...' : '📸 Save Snapshot' }}</span>
            </button>
          </div>
        </div>

        <!-- Snapshots List -->
        <div class="space-y-2">
          <div class="text-xs font-semibold text-slate-400">Saved Snapshots ({{ snapshots.length }})</div>

          <div v-if="loading" class="text-xs text-slate-500 text-center py-6">
            Loading snapshots...
          </div>

          <div v-else-if="snapshots.length === 0" class="text-xs text-slate-500 text-center py-6 border border-dashed border-slate-800 rounded-xl">
            No snapshots saved yet. Create one above to preserve your current mod setup!
          </div>

          <div v-else class="space-y-2 max-h-72 overflow-y-auto pr-1">
            <div
              v-for="snap in snapshots"
              :key="snap.filename"
              class="bg-slate-950/60 border border-slate-850 hover:border-slate-700 rounded-xl p-3 flex items-center justify-between transition-colors"
            >
              <div class="min-w-0 pr-3">
                <div class="text-xs font-bold text-slate-200 truncate flex items-center gap-2">
                  <span>{{ snap.label }}</span>
                  <span class="text-[10px] font-mono px-2 py-0.5 rounded-full bg-slate-800 text-slate-400">
                    {{ snap.modCount }} mods
                  </span>
                </div>
                <div class="text-[10px] text-slate-500 mt-0.5 flex items-center gap-2">
                  <span>{{ formatDate(snap.createdAt) }}</span>
                  <span>•</span>
                  <span>{{ snap.modLoader }}</span>
                </div>
              </div>

              <div class="flex items-center gap-2 shrink-0">
                <button
                  class="z-btn-ghost text-xs px-3 py-1.5 rounded-lg font-semibold hover:text-cyan-300 hover:border-cyan-500/50"
                  :disabled="restoring === snap.filename"
                  @click="handleRestore(snap)"
                >
                  <span v-if="restoring === snap.filename" class="w-3 h-3 border-2 border-cyan-400 border-t-transparent rounded-full animate-spin"></span>
                  <span>{{ restoring === snap.filename ? 'Restoring...' : 'Rollback' }}</span>
                </button>
                <button
                  class="text-slate-500 hover:text-rose-400 p-1.5 rounded-lg hover:bg-rose-500/10 transition-colors"
                  title="Delete snapshot"
                  @click="handleDelete(snap)"
                >
                  <svg class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                    <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M19 7l-.867 12.142A2 2 0 0116.138 21H7.862a2 2 0 01-1.995-1.858L5 7m5 4v6m4-6v6m1-10V4a1 1 0 00-1-1h-4a1 1 0 00-1 1v3M4 7h16" />
                  </svg>
                </button>
              </div>
            </div>
          </div>
        </div>
      </div>

      <!-- Footer -->
      <div class="px-6 py-4 border-t border-slate-800/80 bg-slate-950/60 flex items-center justify-end">
        <button class="z-btn-ghost text-xs px-4 py-2 rounded-xl font-semibold" @click="handleClose">
          Done
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
  instance: { type: Object, default: null },
});

const emit = defineEmits(['close', 'restored']);

const newLabel = ref('');
const snapshots = ref([]);
const loading = ref(false);
const creating = ref(false);
const restoring = ref(null);

watch(
  () => props.open,
  (isOpen) => {
    if (isOpen && props.instance) {
      loadSnapshots();
    }
  }
);

async function loadSnapshots() {
  if (!props.instance) return;
  loading.value = true;
  try {
    snapshots.value = await api.listInstanceModSnapshots(props.instance.id);
  } catch (e) {
    console.error('Failed to load snapshots:', e);
  } finally {
    loading.value = false;
  }
}

async function handleCreateSnapshot() {
  if (!props.instance || !newLabel.value.trim()) return;
  creating.value = true;
  try {
    const snap = await api.createInstanceModSnapshot(props.instance.id, newLabel.value.trim());
    snapshots.value.unshift(snap);
    newLabel.value = '';
    window.dispatchEvent(
      new CustomEvent('zircon-status', { detail: `Saved snapshot "${snap.label}"` })
    );
  } catch (e) {
    window.dispatchEvent(new CustomEvent('zircon-status', { detail: `Snapshot failed: ${e}` }));
  } finally {
    creating.value = false;
  }
}

async function handleRestore(snap) {
  if (!props.instance) return;
  if (!confirm(`Are you sure you want to rollback to "${snap.label}"? Current mods will be backed up automatically.`)) {
    return;
  }
  restoring.value = snap.filename;
  try {
    const result = await api.restoreInstanceModSnapshot(props.instance.id, snap.filename);
    window.dispatchEvent(
      new CustomEvent('zircon-status', { detail: `Restored snapshot "${snap.label}"` })
    );
    emit('restored', result);
    handleClose();
  } catch (e) {
    window.dispatchEvent(new CustomEvent('zircon-status', { detail: `Restore failed: ${e}` }));
  } finally {
    restoring.value = null;
  }
}

async function handleDelete(snap) {
  if (!props.instance) return;
  if (!confirm(`Delete snapshot "${snap.label}"?`)) return;
  try {
    await api.deleteInstanceModSnapshot(props.instance.id, snap.filename);
    snapshots.value = snapshots.value.filter((s) => s.filename !== snap.filename);
  } catch (e) {
    console.error('Delete snapshot failed:', e);
  }
}

function formatDate(ts) {
  if (!ts) return 'Unknown';
  return new Date(ts).toLocaleString();
}

function handleClose() {
  emit('close');
}
</script>
