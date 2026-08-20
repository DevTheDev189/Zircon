<template>
  <div class="h-full p-5 overflow-y-auto">
    <div class="max-w-[560px] z-card">
      <h3 class="text-white font-bold mb-5">Settings</h3>

      <!-- RAM slider -->
      <div class="mb-6">
        <div class="flex items-center justify-between mb-2">
          <span class="z-label">Max Memory Allocation (RAM)</span>
          <span class="text-sm font-bold text-accent">{{ settings.memoryGb }} GB</span>
        </div>
        <input
          v-model.number="settings.memoryGb"
          type="range"
          min="2"
          max="16"
          step="1"
          class="w-full accent-[#47d2c9]"
        />
        <div class="flex justify-between text-[10px] text-muted">
          <span>2 GB</span><span>16 GB</span>
        </div>
        <p class="z-label mt-1">
          Applied to offline instance launches (replaces the instance's -Xmx). Server launches use the standard 4 GB default.
        </p>
      </div>

      <button class="z-btn-accent" :disabled="saving" @click="save">
        {{ saving ? 'Saving…' : 'Save Settings' }}
      </button>
      <p class="z-label mt-3">{{ savedAt }}</p>

      <!-- Debug logs -->
      <div class="mt-8 pt-6 border-t border-edge">
        <div class="flex items-center justify-between mb-2">
          <h3 class="text-white font-bold">Debug Logs</h3>
          <div class="flex gap-2">
            <button class="z-btn-ghost text-[11px]" @click="copyLogs">Copy to Clipboard</button>
            <button class="z-btn-ghost text-[11px]" @click="clearLogs">Clear Logs</button>
          </div>
        </div>
        <p class="z-label mb-2">
          Recent launcher events (in-memory only — cleared on exit). Useful when reporting issues.
        </p>
        <button class="z-btn-ghost text-[11px] mb-2" @click="refreshLogs">Refresh</button>
        <pre class="bg-black/40 border border-edge rounded-lg p-3 text-[10px] leading-relaxed text-muted h-64 overflow-y-auto whitespace-pre-wrap">{{ logText || 'No log lines captured yet.' }}</pre>
        <p v-if="copiedAt" class="z-label mt-2">{{ copiedAt }}</p>
      </div>
    </div>
  </div>
</template>

<script setup>
import { onMounted, ref } from 'vue';
import { api } from '../lib/api';

const settings = ref({ memoryGb: 4 });
const saving = ref(false);
const savedAt = ref('');
const logText = ref('');
const copiedAt = ref('');

onMounted(async () => {
  try {
    settings.value = await api.getSettings();
  } catch {
    // keep defaults
  }
  refreshLogs();
});

async function refreshLogs() {
  try {
    const lines = await api.getLauncherLogs();
    logText.value = lines.join('\n');
  } catch {
    logText.value = 'Failed to read launcher logs.';
  }
}

async function copyLogs() {
  try {
    await navigator.clipboard.writeText(logText.value || '');
    copiedAt.value = `Copied at ${new Date().toLocaleTimeString()}.`;
  } catch {
    copiedAt.value = 'Copy failed — select and copy manually.';
  }
}

async function clearLogs() {
  try {
    await api.clearLauncherLogs();
    logText.value = '';
    copiedAt.value = '';
  } catch {
    copiedAt.value = 'Failed to clear logs.';
  }
}

async function save() {
  saving.value = true;
  try {
    await api.saveSettings(settings.value);
    savedAt.value = `Saved at ${new Date().toLocaleTimeString()}.`;
  } catch (e) {
    savedAt.value = `Save failed: ${e}`;
  } finally {
    saving.value = false;
  }
}
</script>
