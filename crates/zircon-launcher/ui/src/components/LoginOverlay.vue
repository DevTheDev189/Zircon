<template>
  <div
    v-if="visible"
    class="absolute inset-0 z-50 flex items-center justify-center bg-bg"
  >
    <div class="z-card w-[420px] text-center p-8 shadow-2xl">
      <div
        class="inline-block bg-accent text-[#022c29] font-bold rounded-lg px-3 py-1.5 text-xl mb-4"
      >
        ⚡ Zircon
      </div>
      <h2 class="text-white text-lg font-bold mb-2">Sign in with Microsoft</h2>
      <p class="text-muted text-sm mb-6">
        Sign in to launch and play on Zircon servers. Your Minecraft profile,
        skins and saved servers are stored locally.
      </p>

      <button
        class="z-btn-accent w-full py-2.5 text-base disabled:opacity-60"
        :disabled="busy"
        @click="onLogin"
      >
        <span v-if="busy">Opening browser…</span>
        <span v-else>Continue with Microsoft</span>
      </button>

      <p v-if="status" class="text-xs text-muted mt-4 whitespace-pre-line">
        {{ status }}
      </p>
      <p v-if="error" class="text-xs text-[#f85149] mt-2 whitespace-pre-line">
        {{ error }}
      </p>
    </div>
  </div>
</template>

<script setup>
import { ref } from 'vue';
import { api } from '../lib/api';

const emit = defineEmits(['logged-in']);

defineProps({
  visible: { type: Boolean, default: false },
});

const busy = ref(false);
const status = ref('');
const error = ref('');

async function onLogin() {
  busy.value = true;
  error.value = '';
  status.value = 'Opening browser for Microsoft login…';
  try {
    const session = await api.loginMicrosoft();
    status.value = `Signed in as ${session.username}`;
    emit('logged-in', session);
  } catch (e) {
    error.value = `Login failed: ${e}`;
    status.value = '';
  } finally {
    busy.value = false;
  }
}
</script>
