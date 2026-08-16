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

      <!-- Toggles -->
      <label class="flex items-start gap-3 mb-4 cursor-pointer">
        <input
          v-model="settings.strictVerification"
          type="checkbox"
          class="mt-0.5 accent-[#47d2c9]"
        />
        <span>
          <span class="block text-sm font-bold text-white">Strict hash verification</span>
          <span class="block text-xs text-muted">
            Abort a mod sync when a mod's hash cannot be verified against Modrinth/CurseForge.
          </span>
        </span>
      </label>

      <label class="flex items-start gap-3 mb-6 cursor-pointer">
        <input
          v-model="settings.trustDirectMods"
          type="checkbox"
          class="mt-0.5 accent-[#47d2c9]"
        />
        <span>
          <span class="block text-sm font-bold text-white">Trust direct custom mods</span>
          <span class="block text-xs text-muted">
            Accept server mods with no provider to verify against (needed when strict mode is on).
          </span>
        </span>
      </label>

      <button class="z-btn-accent" :disabled="saving" @click="save">
        {{ saving ? 'Saving…' : 'Save Settings' }}
      </button>
      <p class="z-label mt-3">{{ savedAt }}</p>
    </div>
  </div>
</template>

<script setup>
import { onMounted, ref } from 'vue';
import { api } from '../lib/api';

const settings = ref({ memoryGb: 4, strictVerification: true, trustDirectMods: false });
const saving = ref(false);
const savedAt = ref('');

onMounted(async () => {
  try {
    settings.value = await api.getSettings();
  } catch {
    // keep defaults
  }
});

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
