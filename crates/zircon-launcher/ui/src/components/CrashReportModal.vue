<template>
  <div
    v-if="isOpen"
    class="fixed inset-0 z-50 flex items-center justify-center bg-black/80 backdrop-blur-md p-4 animate-in fade-in duration-200"
    @click.self="dismiss"
  >
    <!-- Ambient Radial Glow -->
    <div class="absolute w-[560px] h-[400px] rounded-full bg-rose-600/10 blur-3xl pointer-events-none -z-10"></div>

    <!-- Glassmorphic Card Container -->
    <div
      class="w-full max-w-xl bg-[#0b0f19]/90 backdrop-blur-2xl border border-rose-500/30 rounded-2xl shadow-2xl shadow-black/80 ring-1 ring-white/10 flex flex-col overflow-hidden max-h-[85vh] relative"
    >
      <!-- Header -->
      <div class="px-6 py-5 border-b border-slate-800/80 bg-gradient-to-r from-rose-950/30 via-slate-900/40 to-transparent flex items-center justify-between">
        <div class="flex items-center gap-3.5 min-w-0">
          <div class="w-10 h-10 rounded-xl bg-gradient-to-br from-rose-500/20 to-rose-950/40 border border-rose-500/40 flex items-center justify-center text-rose-400 shrink-0 shadow-[0_0_15px_rgba(244,63,94,0.25)]">
            <svg class="w-5 h-5" fill="none" viewBox="0 0 24 24" stroke="currentColor">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 9v2m0 4h.01m-6.938 4h13.856c1.54 0 2.502-1.667 1.732-3L13.732 4c-.77-1.333-2.694-1.333-3.464 0L3.34 16c-.77 1.333.192 3 1.732 3z" />
            </svg>
          </div>
          <div class="min-w-0">
            <div class="flex items-center gap-2 flex-wrap">
              <h2 class="text-sm font-bold text-slate-100 tracking-wide truncate">
                {{ crashData?.analysis?.title || 'Minecraft Crashed Unexpectedly' }}
              </h2>
              <span class="px-2 py-0.5 rounded text-[10px] font-mono font-bold bg-rose-500/20 text-rose-300 border border-rose-500/30">
                Code {{ crashData?.code ?? 1 }}
              </span>
            </div>
            <div class="text-[11px] text-slate-400 mt-0.5 flex items-center gap-2 font-sans">
              <span>Instance: <span class="text-slate-200 font-medium">{{ crashData?.label || 'Game' }}</span></span>
              <span v-if="crashData?.analysis?.category" class="text-slate-600">•</span>
              <span v-if="crashData?.analysis?.category" class="text-cyan-400/90 font-mono">{{ crashData.analysis.category }}</span>
            </div>
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

      <!-- Body -->
      <div class="p-6 overflow-y-auto flex flex-col gap-4 text-xs">
        <!-- Diagnosis & Suggested Fix Box -->
        <div class="p-4 rounded-xl bg-slate-900/70 border border-slate-800/90 flex flex-col gap-3 shadow-inner">
          <div class="text-slate-300 leading-relaxed font-sans text-xs">
            {{ crashData?.analysis?.explanation || 'Minecraft terminated with a non-zero exit status.' }}
          </div>
          <div
            v-if="crashData?.analysis?.suggestedFix"
            class="p-3 rounded-lg bg-cyan-950/30 border border-cyan-500/30 flex items-start gap-3 text-cyan-200 shadow-[0_0_12px_rgba(6,182,212,0.08)]"
          >
            <div class="w-6 h-6 rounded-md bg-cyan-500/20 border border-cyan-500/40 flex items-center justify-center text-cyan-400 shrink-0 mt-0.5">
              <svg class="w-3.5 h-3.5" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9.663 17h4.673M12 3v1m6.364 1.636l-.707.707M21 12h-1M4 12H3m3.343-5.657l-.707-.707m2.828 9.9a5 5 0 117.072 0l-.548.547A3.374 3.374 0 0014 18.469V19a2 2 0 11-4 0v-.531c0-.895-.356-1.754-.988-2.386l-.548-.547z" />
              </svg>
            </div>
            <div class="min-w-0">
              <span class="font-bold text-cyan-300 block text-xs mb-0.5">Suggested Solution</span>
              <span class="text-slate-300 leading-relaxed">{{ crashData.analysis.suggestedFix }}</span>
            </div>
          </div>
        </div>

        <!-- Log Snippet -->
        <div v-if="crashData?.analysis?.relevantLines?.length">
          <div class="flex items-center justify-between mb-2 text-slate-400 font-medium">
            <div class="flex items-center gap-2">
              <div class="flex items-center gap-1.5">
                <span class="w-2 h-2 rounded-full bg-rose-500/80"></span>
                <span class="w-2 h-2 rounded-full bg-amber-500/80"></span>
                <span class="w-2 h-2 rounded-full bg-emerald-500/80"></span>
              </div>
              <span class="text-xs font-semibold text-slate-300 ml-1">Crash Log Trace</span>
            </div>
            <span class="text-[10px] font-mono text-slate-500">Last events before termination</span>
          </div>
          <div class="bg-[#05070d]/90 border border-slate-800 rounded-xl p-3.5 font-mono text-[11px] text-slate-300 overflow-x-auto max-h-[170px] select-text shadow-inner">
            <div
              v-for="(line, idx) in crashData.analysis.relevantLines"
              :key="idx"
              class="leading-snug py-0.5 font-mono"
              :class="line.toLowerCase().includes('error') || line.toLowerCase().includes('exception') || line.toLowerCase().includes('crash') ? 'text-rose-400 font-semibold' : 'text-slate-400'"
            >
              {{ line }}
            </div>
          </div>
        </div>

        <div v-if="copied" class="text-center text-[11px] text-emerald-400 font-medium flex items-center justify-center gap-1.5">
          <svg class="w-3.5 h-3.5 text-emerald-400" fill="none" viewBox="0 0 24 24" stroke="currentColor">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M5 13l4 4L19 7" />
          </svg>
          <span>Crash details copied to clipboard</span>
        </div>
      </div>

      <!-- Footer Actions -->
      <div class="px-6 py-4 border-t border-slate-800/80 bg-slate-950/70 flex items-center justify-between gap-3">
        <button
          type="button"
          class="z-btn-ghost text-xs px-3.5 py-1.5 rounded-xl border border-slate-700 hover:border-slate-600 flex items-center gap-2 transition text-slate-300 hover:text-white"
          @click="copyDiagnostics"
        >
          <svg v-if="!copied" class="w-3.5 h-3.5 text-slate-400" fill="none" viewBox="0 0 24 24" stroke="currentColor">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M8 5H6a2 2 0 00-2 2v12a2 2 0 002 2h10a2 2 0 002-2v-1M8 5a2 2 0 002 2h2a2 2 0 002-2M8 5a2 2 0 012-2h2a2 2 0 012 2v3m2 4H10m0 0l3-3m-3 3l3 3" />
          </svg>
          <svg v-else class="w-3.5 h-3.5 text-emerald-400" fill="none" viewBox="0 0 24 24" stroke="currentColor">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M5 13l4 4L19 7" />
          </svg>
          <span>{{ copied ? 'Copied' : 'Copy Details' }}</span>
        </button>

        <div class="flex items-center gap-2">
          <button
            type="button"
            class="z-btn-primary text-xs px-5 py-1.5 rounded-xl font-bold transition"
            @click="dismiss"
          >
            Dismiss
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
