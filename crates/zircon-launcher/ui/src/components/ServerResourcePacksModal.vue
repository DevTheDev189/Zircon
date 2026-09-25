<template>
  <div
    v-if="isOpen"
    class="fixed inset-0 z-50 flex items-center justify-center bg-black/80 backdrop-blur-md p-4 animate-in fade-in duration-200"
    @click.self="dismiss"
  >
    <!-- Ambient Glow Behind Modal -->
    <div class="absolute w-[520px] h-[360px] rounded-full bg-cyan-500/10 blur-3xl pointer-events-none -z-10"></div>

    <!-- Modal Container -->
    <div
      class="w-full max-w-lg bg-[#0b0f19]/95 backdrop-blur-2xl border border-cyan-500/30 rounded-2xl shadow-2xl shadow-black/80 ring-1 ring-white/10 flex flex-col overflow-hidden max-h-[85vh] relative"
    >
      <!-- Header -->
      <div class="px-6 py-5 border-b border-slate-800/80 bg-gradient-to-r from-cyan-950/30 via-slate-900/40 to-transparent flex items-center justify-between">
        <div class="flex items-center gap-3.5 min-w-0">
          <div class="w-10 h-10 rounded-xl bg-gradient-to-br from-cyan-500/20 to-cyan-950/40 border border-cyan-500/40 flex items-center justify-center text-cyan-400 shrink-0 shadow-[0_0_15px_rgba(6,182,212,0.25)]">
            <svg class="w-5 h-5" fill="none" viewBox="0 0 24 24" stroke="currentColor">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M4 16v1a3 3 0 003 3h10a3 3 0 003-3v-1m-4-8l-4-4m0 0L8 8m4-4v12" />
            </svg>
          </div>
          <div class="min-w-0">
            <h2 class="text-sm font-bold text-slate-100 tracking-wide truncate">
              Server Resource Packs Offered
            </h2>
            <p class="text-[11px] text-slate-400 mt-0.5 truncate">
              Choose which server resource packs to enable on your client.
            </p>
          </div>
        </div>
        <button
          class="text-slate-400 hover:text-white p-1.5 rounded-lg hover:bg-slate-800/80 transition ml-2 shrink-0"
          title="Close dialog"
          @click="dismiss"
        >
          <svg class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 18L18 6M6 6l12 12" />
          </svg>
        </button>
      </div>

      <!-- Body / Packs List -->
      <div class="p-6 overflow-y-auto flex flex-col gap-3 text-xs">
        <div class="text-[11px] text-slate-400 flex items-center justify-between px-1">
          <span>Available Server Packs ({{ serverPacks.length }})</span>
          <span class="text-cyan-400/90 font-mono text-[10px]">Toggled to Accept by default</span>
        </div>

        <div v-if="serverPacks.length === 0" class="text-xs text-slate-500 py-6 text-center italic">
          No server resource packs currently advertised.
        </div>

        <div
          v-for="pack in serverPacks"
          :key="pack.filename"
          class="p-3.5 rounded-xl bg-slate-900/70 border border-slate-800/90 flex items-center justify-between gap-3 shadow-inner hover:border-slate-750 transition"
        >
          <div class="flex items-center gap-3 min-w-0">
            <div class="w-10 h-10 rounded-lg bg-slate-950 border border-slate-800 flex items-center justify-center shrink-0 overflow-hidden shadow-inner">
              <img v-if="pack.iconUrl || pack.iconDataUrl" :src="pack.iconUrl || pack.iconDataUrl" class="w-full h-full object-cover" />
              <svg v-else class="w-5 h-5 text-cyan-400/60" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8">
                <rect x="3" y="3" width="18" height="18" rx="2" ry="2" />
                <line x1="3" y1="9" x2="21" y2="9" />
                <line x1="9" y1="21" x2="9" y2="9" />
              </svg>
            </div>
            <div class="min-w-0">
              <div class="flex items-center gap-2 flex-wrap">
                <span class="font-bold text-xs text-slate-100 truncate">{{ pack.title || pack.filename }}</span>
                <span v-if="pack.fileSize" class="text-[10px] font-mono text-slate-400">
                  {{ (pack.fileSize / (1024 * 1024)).toFixed(2) }} MB
                </span>
              </div>
              <p v-if="pack.description" class="text-[11px] text-slate-400 line-clamp-1 mt-0.5">
                {{ pack.description }}
              </p>
              <p v-else class="text-[10px] font-mono text-slate-500 truncate mt-0.5">
                {{ pack.filename }}
              </p>
            </div>
          </div>

          <!-- Toggle switch (YES / Accepted by default) -->
          <div class="flex items-center gap-2 shrink-0">
            <span
              class="text-[10px] font-mono font-bold uppercase transition"
              :class="decisions[pack.filename] ? 'text-cyan-400' : 'text-slate-500'"
            >
              {{ decisions[pack.filename] ? 'Accept' : 'Decline' }}
            </span>
            <button
              type="button"
              class="w-11 h-6 rounded-full transition-colors relative focus:outline-none"
              :class="decisions[pack.filename] ? 'bg-cyan-500 shadow-[0_0_10px_rgba(6,182,212,0.3)]' : 'bg-slate-800 border border-slate-700'"
              @click="toggleDecision(pack.filename)"
            >
              <span
                class="block w-4 h-4 rounded-full bg-white transition-transform transform shadow-sm"
                :class="decisions[pack.filename] ? 'translate-x-6' : 'translate-x-1'"
              ></span>
            </button>
          </div>
        </div>
      </div>

      <!-- Footer Actions -->
      <div class="px-6 py-4 border-t border-slate-800/80 bg-slate-950/70 flex items-center justify-between gap-3">
        <button
          type="button"
          class="text-xs text-slate-400 hover:text-slate-200 transition font-medium"
          @click="declineAll"
        >
          Decline All
        </button>

        <div class="flex items-center gap-2.5">
          <button
            type="button"
            class="z-btn-ghost text-xs px-4 py-2 rounded-xl border border-slate-700 hover:border-slate-600 text-slate-300 transition"
            @click="dismiss"
          >
            Cancel
          </button>
          <button
            type="button"
            class="z-btn-primary text-xs px-5 py-2 rounded-xl font-bold flex items-center gap-1.5 transition"
            @click="confirmDecisions"
          >
            <svg class="w-3.5 h-3.5" fill="none" viewBox="0 0 24 24" stroke="currentColor">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M5 13l4 4L19 7" />
            </svg>
            <span>Accept Selection</span>
          </button>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup>
import { ref, watch } from 'vue';

const props = defineProps({
  isOpen: {
    type: Boolean,
    default: false,
  },
  serverPacks: {
    type: Array,
    default: () => [],
  },
  initialDecisions: {
    type: Object,
    default: () => ({}),
  },
});

const emit = defineEmits(['close', 'accepted']);

// Decisions map: { [filename]: boolean }, default to true (YES)
const decisions = ref({});

watch(
  [() => props.serverPacks, () => props.isOpen],
  ([packs, open]) => {
    if (!open) return;
    const map = {};
    (packs || []).forEach((p) => {
      // If player already made an explicit decision previously, respect it; otherwise default to true (YES)
      if (props.initialDecisions && props.initialDecisions[p.filename] !== undefined) {
        map[p.filename] = Boolean(props.initialDecisions[p.filename]);
      } else {
        map[p.filename] = true; // Default to YES / Accept
      }
    });
    decisions.value = map;
  },
  { immediate: true }
);

function toggleDecision(filename) {
  decisions.value[filename] = !decisions.value[filename];
}

function declineAll() {
  const map = {};
  (props.serverPacks || []).forEach((p) => {
    map[p.filename] = false;
  });
  decisions.value = map;
}

function dismiss() {
  emit('close');
}

function confirmDecisions() {
  const accepted = [];
  const rejected = [];
  Object.keys(decisions.value).forEach((filename) => {
    if (decisions.value[filename]) {
      accepted.push(filename);
    } else {
      rejected.push(filename);
    }
  });
  emit('accepted', { accepted, rejected });
  emit('close');
}
</script>
