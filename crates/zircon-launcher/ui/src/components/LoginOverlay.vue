<template>
  <div
    v-if="visible"
    class="absolute inset-0 z-50 flex items-center justify-center bg-gradient-to-br from-[#0b1218] via-[#0d1117] to-[#10212a]"
  >
    <!-- Soft teal glow behind the card -->
    <div class="absolute inset-0 overflow-hidden pointer-events-none">
      <div
        class="absolute -top-24 left-1/2 -translate-x-1/2 w-[460px] h-[460px] rounded-full bg-accent/10 blur-3xl"
      ></div>
    </div>

    <div class="z-card w-[420px] text-center p-8 shadow-2xl relative">
      <div
        class="inline-flex items-center gap-2 bg-gradient-to-br from-accent to-[#1f8f87] text-[#032b28] font-extrabold rounded-lg px-4 py-2 text-xl mb-5 shadow-lg shadow-accent/25"
      >
        <svg class="w-5 h-5" viewBox="0 0 24 24" fill="currentColor">
          <path d="M13 2 3 14h9l-1 8 10-12h-9l1-8z" />
        </svg>
        Zircon
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
