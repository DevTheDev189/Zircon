<template>
  <div
    v-if="isOpen"
    class="fixed inset-0 z-50 flex items-center justify-center bg-black/85 backdrop-blur-md p-4 animate-in fade-in duration-200"
    @click.self="dismiss"
  >
    <!-- Ambient Radial Glow -->
    <div class="absolute w-[680px] h-[520px] rounded-full bg-rose-600/15 blur-3xl pointer-events-none -z-10"></div>

    <!-- Glassmorphic Card Container -->
    <div
      class="w-full max-w-4xl bg-[#0b0f19]/95 backdrop-blur-2xl border border-rose-500/30 rounded-2xl shadow-2xl shadow-black/90 ring-1 ring-white/10 flex flex-col overflow-hidden max-h-[90vh] relative text-xs"
    >
      <!-- Header -->
      <div class="px-6 py-4 border-b border-slate-800/80 bg-gradient-to-r from-rose-950/40 via-slate-900/60 to-transparent flex items-center justify-between">
        <div class="flex items-center gap-3.5 min-w-0">
          <div class="w-10 h-10 rounded-xl bg-gradient-to-br from-rose-500/20 to-rose-950/50 border border-rose-500/40 flex items-center justify-center text-rose-400 shrink-0 shadow-[0_0_15px_rgba(244,63,94,0.3)]">
            <svg class="w-5 h-5" fill="none" viewBox="0 0 24 24" stroke="currentColor">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 9v2m0 4h.01m-6.938 4h13.856c1.54 0 2.502-1.667 1.732-3L13.732 4c-.77-1.333-2.694-1.333-3.464 0L3.34 16c-.77 1.333.192 3 1.732 3z" />
            </svg>
          </div>
          <div class="min-w-0">
            <div class="flex items-center gap-2 flex-wrap">
              <h2 class="text-sm font-bold text-slate-100 tracking-wide truncate">
                {{ crashData?.analysis?.title || $t('crash_dialog.title_default') }}
              </h2>
              <span class="px-2 py-0.5 rounded text-[10px] font-mono font-bold bg-rose-500/20 text-rose-300 border border-rose-500/30">
                {{ $t('crash_dialog.exit_code', { code: crashData?.code ?? 1 }) }}
              </span>
            </div>
            <div class="text-[11px] text-slate-400 mt-0.5 flex items-center gap-2 font-sans">
              <span>{{ $t('crash_dialog.instance_label') }} <span class="text-slate-200 font-medium">{{ crashData?.label || $t('crash_dialog.game_default') }}</span></span>
              <span v-if="crashData?.analysis?.category" class="text-slate-600">•</span>
              <span v-if="crashData?.analysis?.category" class="text-cyan-400 font-mono">{{ crashData.analysis.category }}</span>
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

      <!-- Split-Pane Body -->
      <div class="flex-1 overflow-hidden grid grid-cols-1 md:grid-cols-2 divide-y md:divide-y-0 md:divide-x divide-slate-800/80">
        <!-- Left Pane: Plain English Diagnostics & 1-Click Action Buttons -->
        <div class="p-6 overflow-y-auto flex flex-col gap-4">
          <!-- Root Cause Summary Box -->
          <div class="p-4 rounded-xl bg-slate-900/80 border border-slate-800 flex flex-col gap-2.5 shadow-inner">
            <div class="flex items-center gap-2 text-rose-400 font-bold text-xs">
              <svg class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M13 16h-1v-4h-1m1-4h.01M21 12a9 9 0 11-18 0 9 9 0 0118 0z" />
              </svg>
              <span>{{ $t('crash_dialog.plain_english_diagnosis') }}</span>
            </div>
            <p class="text-slate-300 leading-relaxed font-sans text-xs">
              {{ crashData?.analysis?.plainEnglishSummary || crashData?.analysis?.explanation || $t('crash_dialog.plain_english_default') }}
            </p>
          </div>

          <!-- Suggested Solution Note -->
          <div
            v-if="crashData?.analysis?.suggestedFix"
            class="p-3.5 rounded-xl bg-cyan-950/30 border border-cyan-500/30 flex items-start gap-3 text-cyan-200 shadow-[0_0_12px_rgba(6,182,212,0.08)]"
          >
            <div class="w-6 h-6 rounded-md bg-cyan-500/20 border border-cyan-500/40 flex items-center justify-center text-cyan-400 shrink-0 mt-0.5">
              <svg class="w-3.5 h-3.5" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9.663 17h4.673M12 3v1m6.364 1.636l-.707.707M21 12h-1M4 12H3m3.343-5.657l-.707-.707m2.828 9.9a5 5 0 117.072 0l-.548.547A3.374 3.374 0 0014 18.469V19a2 2 0 11-4 0v-.531c0-.895-.356-1.754-.988-2.386l-.548-.547z" />
              </svg>
            </div>
            <div class="min-w-0">
              <span class="font-bold text-cyan-300 block text-xs mb-0.5">{{ $t('crash_dialog.recommended_solution') }}</span>
              <span class="text-slate-300 leading-relaxed">{{ crashData.analysis.suggestedFix }}</span>
            </div>
          </div>

          <!-- 1-Click Action Buttons -->
          <div class="flex flex-col gap-2 mt-1">
            <span class="text-[11px] font-bold uppercase tracking-wider text-slate-400">{{ $t('crash_dialog.automated_remedies_title') }}</span>

            <!-- Primary Action Button -->
            <button
              v-if="primaryAction"
              type="button"
              class="w-full py-2.5 px-4 rounded-xl font-bold flex items-center justify-between transition group shadow-lg"
              :class="primaryButtonClass"
              :disabled="remediating"
              @click="applyAction(primaryAction)"
            >
              <div class="flex items-center gap-2.5 min-w-0">
                <svg v-if="!remediating" class="w-4 h-4 shrink-0" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                  <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M13 10V3L4 14h7v7l9-11h-7z" />
                </svg>
                <svg v-else class="w-4 h-4 shrink-0 animate-spin text-white" fill="none" viewBox="0 0 24 24">
                  <circle class="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" stroke-width="4"></circle>
                  <path class="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8v8H4z"></path>
                </svg>
                <span class="truncate">{{ primaryActionLabel }}</span>
              </div>
              <span class="text-[10px] font-mono px-1.5 py-0.5 rounded bg-black/20 group-hover:bg-black/30 shrink-0">{{ $t('crash_dialog.one_click_fix_badge') }}</span>
            </button>

            <!-- Secondary Actions (e.g. Rollback Snapshot) -->
            <div v-for="(secAction, idx) in secondaryActions" :key="idx">
              <button
                type="button"
                class="w-full py-2 px-3.5 rounded-xl border border-slate-700/80 bg-slate-900/60 hover:bg-slate-800 text-slate-200 flex items-center justify-between transition text-xs"
                :disabled="remediating"
                @click="applyAction(secAction)"
              >
                <div class="flex items-center gap-2 truncate">
                  <svg class="w-3.5 h-3.5 text-amber-400 shrink-0" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                    <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 8v4l3 3m6-3a9 9 0 11-18 0 9 9 0 0118 0z" />
                  </svg>
                  <span class="truncate">{{ getActionLabel(secAction) }}</span>
                </div>
                <span class="text-[10px] text-slate-400 font-mono">{{ $t('crash_dialog.rollback_badge') }}</span>
              </button>
            </div>

            <!-- Fallback if no automated actions were detected -->
            <div v-if="!primaryAction && !secondaryActions.length" class="text-slate-400 text-xs italic p-3 rounded-xl bg-slate-900/50 border border-slate-800">
              {{ $t('crash_dialog.no_remedy_fallback') }}
            </div>

            <!-- Remediation Feedback Status Banner -->
            <div
              v-if="remediationResult"
              class="p-3 rounded-xl flex items-center gap-2.5 text-xs font-semibold animate-in fade-in"
              :class="remediationResult.success ? 'bg-emerald-950/40 text-emerald-300 border border-emerald-500/40' : 'bg-rose-950/40 text-rose-300 border border-rose-500/40'"
            >
              <svg v-if="remediationResult.success" class="w-4 h-4 text-emerald-400 shrink-0" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M5 13l4 4L19 7" />
              </svg>
              <svg v-else class="w-4 h-4 text-rose-400 shrink-0" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 18L18 6M6 6l12 12" />
              </svg>
              <span class="flex-1">{{ remediationResult.message }}</span>
            </div>
          </div>
        </div>

        <!-- Right Pane: Log Snippet / Trace View -->
        <div class="p-6 overflow-y-auto flex flex-col gap-3 bg-[#070a12]/50">
          <div class="flex items-center justify-between text-slate-400 font-medium">
            <div class="flex items-center gap-2">
              <div class="flex items-center gap-1.5">
                <span class="w-2 h-2 rounded-full bg-rose-500/80"></span>
                <span class="w-2 h-2 rounded-full bg-amber-500/80"></span>
                <span class="w-2 h-2 rounded-full bg-emerald-500/80"></span>
              </div>
              <span class="text-xs font-semibold text-slate-300 ml-1">{{ $t('crash_dialog.crash_log_trace') }}</span>
            </div>
            <button
              class="text-[10px] text-slate-400 hover:text-white flex items-center gap-1 transition"
              @click="toggleFullLog"
            >
              <span>{{ showFullLog ? $t('crash_dialog.show_snippet') : $t('crash_dialog.show_full_log') }}</span>
            </button>
          </div>

          <!-- Trace Viewer -->
          <div class="bg-[#05070d]/90 border border-slate-800 rounded-xl p-3.5 font-mono text-[11px] text-slate-300 overflow-x-auto flex-1 min-h-[220px] max-h-[380px] select-text shadow-inner">
            <div
              v-for="(line, idx) in displayedLogLines"
              :key="idx"
              class="leading-relaxed py-0.5 font-mono"
              :class="isErrorLine(line) ? 'text-rose-400 font-semibold' : 'text-slate-400'"
            >
              {{ line }}
            </div>
            <div v-if="!displayedLogLines.length" class="text-slate-500 italic">
              {{ $t('crash_dialog.no_logs_recorded') }}
            </div>
          </div>
        </div>
      </div>

      <!-- Footer Actions -->
      <div class="px-6 py-3.5 border-t border-slate-800/80 bg-slate-950/70 flex items-center justify-between gap-3">
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
          <span>{{ copied ? $t('crash_dialog.copied') : $t('crash_dialog.copy_crash_log') }}</span>
        </button>

        <div class="flex items-center gap-2">
          <button
            type="button"
            class="z-btn-primary text-xs px-5 py-1.5 rounded-xl font-bold transition"
            @click="dismiss"
          >
            {{ $t('crash_dialog.dismiss') }}
          </button>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup>
import { computed, ref } from 'vue';
import { api } from '../lib/api';

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

const emit = defineEmits(['close', 'relaunch']);
const copied = ref(false);
const remediating = ref(false);
const remediationResult = ref(null);
const showFullLog = ref(false);

const primaryAction = computed(() => props.crashData?.analysis?.primaryAction || null);
const secondaryActions = computed(() => props.crashData?.analysis?.secondaryActions || []);

const primaryActionLabel = computed(() => {
  if (!primaryAction.value) return '';
  return getActionLabel(primaryAction.value);
});

const primaryButtonClass = computed(() => {
  const act = primaryAction.value;
  if (!act) return 'bg-cyan-600 text-white';
  const type = act.actionType || Object.keys(act)[0];
  switch (type) {
    case 'installMissingMod':
      return 'bg-gradient-to-r from-emerald-600 to-teal-600 hover:from-emerald-500 hover:to-teal-500 text-white';
    case 'disableMod':
      return 'bg-gradient-to-r from-amber-600 to-rose-600 hover:from-amber-500 hover:to-rose-500 text-white';
    case 'adjustMemory':
      return 'bg-gradient-to-r from-indigo-600 to-blue-600 hover:from-indigo-500 hover:to-blue-500 text-white';
    default:
      return 'bg-gradient-to-r from-cyan-600 to-blue-600 hover:from-cyan-500 hover:to-blue-500 text-white';
  }
});

function getActionLabel(action) {
  if (!action) return '';
  const type = action.actionType || Object.keys(action)[0];
  const payload = action.payload || action[type] || action;

  switch (type) {
    case 'installMissingMod':
      return `Install ${payload.projectName || payload.fileName}`;
    case 'disableMod':
      return `Disable ${payload.modName || payload.fileName}`;
    case 'removeDuplicateMod':
      return `Remove Duplicate (${payload.fileName})`;
    case 'adjustMemory':
      return `Allocate ${payload.recommendedGb || 4}GB RAM`;
    case 'resetConfigFile':
      return `Reset ${payload.filePath || 'Config File'}`;
    case 'rollbackSnapshot':
      return `Restore Snapshot: ${payload.label || payload.snapshotFilename}`;
    case 'changeJavaVersion':
      return `Select Java ${payload.requiredMajor || 21}`;
    default:
      return 'Apply Automated Fix';
  }
}

const displayedLogLines = computed(() => {
  if (showFullLog.value && props.crashData?.analysis?.sanitizedLog) {
    return props.crashData.analysis.sanitizedLog.split('\n');
  }
  return props.crashData?.analysis?.relevantLines || [];
});

function isErrorLine(line) {
  if (!line) return false;
  const l = line.toLowerCase();
  return (
    l.includes('error') ||
    l.includes('exception') ||
    l.includes('caused by') ||
    l.includes('missing') ||
    l.includes('crash')
  );
}

function toggleFullLog() {
  showFullLog.value = !showFullLog.value;
}

function dismiss() {
  emit('close');
}

async function applyAction(action) {
  if (!action) return;
  const instanceId = props.crashData?.instanceId || props.crashData?.label || props.crashData?.gameDir;
  if (!instanceId) return;

  remediating.value = true;
  remediationResult.value = null;

  try {
    const res = await api.executeCrashRemediation(instanceId, action);
    remediationResult.value = {
      success: true,
      message: res || 'Action completed successfully.',
    };
  } catch (err) {
    remediationResult.value = {
      success: false,
      message: `Failed: ${err}`,
    };
  } finally {
    remediating.value = false;
  }
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
    `Explanation: ${analysis.plainEnglishSummary || analysis.explanation || ''}`,
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
