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
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9 12l2 2 4-4m5.618-4.016A11.955 11.955 0 0112 2.944a11.955 11.955 0 01-8.618 3.04A12.02 12.02 0 003 9c0 5.591 3.824 10.29 9 11.622 5.176-1.332 9-6.03 9-11.622 0-1.042-.133-2.052-.382-3.016z" />
            </svg>
          </div>
          <div>
            <h2 class="text-base font-bold text-slate-100 flex items-center gap-2">
              Mod Health & Disaster Recovery
              <span class="text-[10px] font-mono px-2 py-0.5 rounded-full bg-cyan-500/10 text-cyan-400 border border-cyan-500/20">Integrity Audit</span>
            </h2>
            <p class="text-xs text-slate-400">Verifies local JARs against cryptographic BOM hashes</p>
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
        <div v-if="auditing" class="text-center py-8 space-y-3">
          <div class="w-8 h-8 border-2 border-cyan-400 border-t-transparent rounded-full animate-spin mx-auto"></div>
          <p class="text-xs text-slate-400">Auditing physical files against BOM...</p>
        </div>

        <div v-else-if="auditReport" class="space-y-4">
          <!-- Summary Cards -->
          <div class="grid grid-cols-3 gap-2 text-center text-xs">
            <div class="bg-slate-900/60 border border-slate-800 rounded-xl p-3">
              <div class="text-emerald-400 font-bold text-base">{{ auditReport.verifiedCount }} / {{ auditReport.totalExpected }}</div>
              <div class="text-[10px] text-slate-400 mt-0.5">Verified Intact</div>
            </div>
            <div class="bg-slate-900/60 border border-slate-800 rounded-xl p-3">
              <div class="font-bold text-base" :class="auditReport.missingMods?.length ? 'text-rose-400' : 'text-slate-400'">
                {{ auditReport.missingMods?.length || 0 }}
              </div>
              <div class="text-[10px] text-slate-400 mt-0.5">Missing Files</div>
            </div>
            <div class="bg-slate-900/60 border border-slate-800 rounded-xl p-3">
              <div class="font-bold text-base" :class="auditReport.corruptedMods?.length ? 'text-amber-400' : 'text-slate-400'">
                {{ auditReport.corruptedMods?.length || 0 }}
              </div>
              <div class="text-[10px] text-slate-400 mt-0.5">Hash Mismatch</div>
            </div>
          </div>

          <!-- All Healthy Banner -->
          <div
            v-if="auditReport.missingMods?.length === 0 && auditReport.corruptedMods?.length === 0"
            class="bg-emerald-950/30 border border-emerald-800/50 rounded-xl p-3.5 text-xs text-emerald-300 flex items-center gap-2.5"
          >
            <span class="text-base">✓</span>
            <span>All mods are healthy and verified! No missing or corrupted files detected.</span>
          </div>

          <!-- Problem Items -->
          <div v-else class="space-y-2">
            <div class="text-xs font-semibold text-rose-300 flex items-center justify-between">
              <span>Issues Detected</span>
              <span class="text-[11px] text-slate-400">{{ (auditReport.missingMods?.length || 0) + (auditReport.corruptedMods?.length || 0) }} total</span>
            </div>

            <div class="max-h-48 overflow-y-auto space-y-1.5 bg-[#070b10] border border-slate-800 rounded-xl p-3">
              <div
                v-for="m in auditReport.missingMods"
                :key="'miss-' + m.filename"
                class="flex items-center justify-between text-xs py-1 text-rose-300"
              >
                <span class="truncate font-mono">{{ m.filename }}</span>
                <span class="text-[10px] px-2 py-0.5 rounded bg-rose-950/80 border border-rose-800 text-rose-400 shrink-0">Missing</span>
              </div>
              <div
                v-for="m in auditReport.corruptedMods"
                :key="'corr-' + m.filename"
                class="flex items-center justify-between text-xs py-1 text-amber-300"
              >
                <span class="truncate font-mono">{{ m.filename }}</span>
                <span class="text-[10px] px-2 py-0.5 rounded bg-amber-950/80 border border-amber-800 text-amber-400 shrink-0">Corrupted Hash</span>
              </div>
            </div>
          </div>

          <!-- Unmanaged Files -->
          <div v-if="auditReport.unmanagedFiles?.length > 0" class="p-3 bg-slate-950/40 border border-slate-800/80 rounded-xl text-xs space-y-1">
            <div class="text-slate-400 font-semibold">Unmanaged Files ({{ auditReport.unmanagedFiles.length }}):</div>
            <div class="text-[11px] text-slate-500 truncate" v-for="f in auditReport.unmanagedFiles.slice(0, 3)" :key="f">
              • {{ f }}
            </div>
          </div>
        </div>
      </div>

      <!-- Footer -->
      <div class="px-6 py-4 border-t border-slate-800/80 bg-slate-950/60 flex items-center justify-between">
        <button class="z-btn-ghost text-xs px-4 py-2 rounded-xl font-semibold" @click="handleClose">
          Close
        </button>

        <button
          v-if="hasIssues"
          class="z-btn-primary text-xs px-5 py-2.5 rounded-xl font-bold flex items-center gap-2"
          :disabled="repairing"
          @click="handleRepair"
        >
          <span v-if="repairing" class="w-3 h-3 border-2 border-white border-t-transparent rounded-full animate-spin"></span>
          <span>{{ repairing ? 'Repairing from CDNs...' : '1-Click Repair from CDNs' }}</span>
        </button>
      </div>
    </div>
  </div>
</template>

<script setup>
import { ref, computed, watch } from 'vue';
import { api } from '../lib/api.js';

const props = defineProps({
  open: { type: Boolean, default: false },
  instance: { type: Object, default: null },
});

const emit = defineEmits(['close', 'repaired']);

const auditReport = ref(null);
const auditing = ref(false);
const repairing = ref(false);

const hasIssues = computed(() => {
  if (!auditReport.value) return false;
  return (
    (auditReport.value.missingMods?.length || 0) > 0 ||
    (auditReport.value.corruptedMods?.length || 0) > 0
  );
});

watch(
  () => props.open,
  (isOpen) => {
    if (isOpen && props.instance) {
      runAudit();
    }
  }
);

async function runAudit() {
  if (!props.instance) return;
  auditing.value = true;
  try {
    auditReport.value = await api.auditInstanceMods(props.instance.id);
  } catch (e) {
    console.error('Audit failed:', e);
  } finally {
    auditing.value = false;
  }
}

async function handleRepair() {
  if (!props.instance) return;
  repairing.value = true;
  try {
    const repairedCount = await api.repairInstanceMods(props.instance.id);
    window.dispatchEvent(
      new CustomEvent('zircon-status', { detail: `Repaired ${repairedCount} mod(s) successfully!` })
    );
    emit('repaired');
    await runAudit();
  } catch (e) {
    window.dispatchEvent(new CustomEvent('zircon-status', { detail: `Repair failed: ${e}` }));
  } finally {
    repairing.value = false;
  }
}

function handleClose() {
  emit('close');
}
</script>
