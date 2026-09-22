<template>
  <div
    v-if="isOpen"
    class="fixed inset-0 z-50 flex items-center justify-center bg-black/80 backdrop-blur-sm p-4 animate-in fade-in duration-200"
    @click.self="dismiss"
  >
    <div
      class="w-full max-w-xl bg-[#090d14] border border-red-500/40 rounded-2xl shadow-2xl shadow-red-950/40 flex flex-col overflow-hidden max-h-[85vh]"
    >
      <!-- Header -->
      <div class="p-5 border-b border-red-500/20 bg-gradient-to-r from-red-950/40 via-red-900/10 to-transparent flex items-center justify-between">
        <div class="flex items-center gap-3">
          <div class="w-10 h-10 rounded-xl bg-red-500/20 border border-red-500/40 flex items-center justify-center text-red-400 text-lg shrink-0">
            ⚠️
          </div>
          <div>
            <div class="flex items-center gap-2">
              <h2 class="text-base font-bold text-white tracking-wide">
                {{ crashData?.analysis?.title || 'Minecraft Crashed Unexpectedly' }}
              </h2>
              <span class="px-2 py-0.5 rounded text-[10px] font-mono font-bold bg-red-500/20 text-red-300 border border-red-500/30">
                Code {{ crashData?.code ?? 1 }}
              </span>
            </div>
            <div class="text-xs text-slate-400 mt-0.5">
              Instance: <span class="text-slate-300 font-medium">{{ crashData?.label || 'Game' }}</span>
            </div>
          </div>
        </div>
        <button
          class="text-slate-400 hover:text-white p-1 rounded-lg hover:bg-slate-800 transition"
          @click="dismiss"
        >
          ✕
        </button>
      </div>

      <!-- Body -->
      <div class="p-5 overflow-y-auto flex flex-col gap-4 text-xs">
        <!-- Diagnosis & Suggested Fix Box -->
        <div class="p-3.5 rounded-xl bg-slate-900/90 border border-slate-800 flex flex-col gap-2">
          <div class="text-slate-300 leading-relaxed font-sans">
            {{ crashData?.analysis?.explanation || 'Minecraft terminated with a non-zero exit status.' }}
          </div>
          <div
            v-if="crashData?.analysis?.suggestedFix"
            class="mt-1 p-2.5 rounded-lg bg-cyan-950/40 border border-cyan-500/30 flex items-start gap-2.5 text-cyan-200"
          >
            <span class="text-sm shrink-0">💡</span>
            <div>
              <span class="font-bold text-cyan-300 block mb-0.5">Suggested Solution:</span>
              <span class="text-slate-300 leading-relaxed">{{ crashData.analysis.suggestedFix }}</span>
            </div>
          </div>
        </div>

        <!-- Log Snippet -->
        <div v-if="crashData?.analysis?.relevantLines?.length">
          <div class="flex items-center justify-between mb-1.5 text-slate-400 font-medium">
            <span>Relevant Log Snippet</span>
            <span class="text-[10px] font-mono text-slate-500">Last events before crash</span>
          </div>
          <div class="bg-[#040608] border border-slate-800 rounded-xl p-3 font-mono text-[11px] text-slate-300 overflow-x-auto max-h-[160px] select-text">
            <div
              v-for="(line, idx) in crashData.analysis.relevantLines"
              :key="idx"
              class="leading-tight py-0.5"
              :class="line.toLowerCase().includes('error') || line.toLowerCase().includes('exception') || line.toLowerCase().includes('crash') ? 'text-red-400 font-semibold' : 'text-slate-400'"
            >
              {{ line }}
            </div>
          </div>
        </div>

        <div v-if="copied" class="text-center text-[11px] text-emerald-400 font-medium">
          ✓ Crash details copied to clipboard!
        </div>
      </div>

      <!-- Footer Actions -->
      <div class="p-4 border-t border-slate-800/80 bg-slate-950/60 flex items-center justify-between gap-3">
        <button
          type="button"
          class="z-btn-ghost text-xs px-3 py-1.5 rounded-lg border border-slate-700 hover:border-slate-600 flex items-center gap-1.5 transition text-slate-300"
          @click="copyDiagnostics"
        >
          <span>📋</span>
          <span>Copy Details</span>
        </button>

        <div class="flex items-center gap-2">
          <button
            type="button"
            class="z-btn-ghost text-xs px-3 py-1.5 rounded-lg border border-slate-700 hover:border-slate-600 transition text-slate-300"
            @click="dismiss"
          >
            Close
          </button>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup>
import { ref } from 'vue';

const props = defineProps({
  isOpen: {
    type: Boolean,
    default: false,
  },
  crashData: {
    type: Object,
    default: null,
  },
});

const emit = defineEmits(['close']);
const copied = ref(false);

function dismiss() {
  emit('close');
}

async function copyDiagnostics() {
  if (!props.crashData) return;
  const analysis = props.crashData.analysis || {};
  const text = [
    `=== Minecraft Crash Report ===`,
    `Instance: ${props.crashData.label || 'Unknown'}`,
    `Exit Code: ${props.crashData.code ?? 1}`,
    `Category: ${analysis.category || 'Generic'}`,
    `Title: ${analysis.title || 'Crash'}`,
    `Explanation: ${analysis.explanation || ''}`,
    `Suggested Fix: ${analysis.suggestedFix || ''}`,
    `\n=== Relevant Lines ===`,
    ...(analysis.relevantLines || []),
  ].join('\n');

  try {
    await navigator.clipboard.writeText(text);
    copied.value = true;
    setTimeout(() => {
      copied.value = false;
    }, 2500);
  } catch (err) {
    console.error('Failed to copy to clipboard:', err);
  }
}
</script>
