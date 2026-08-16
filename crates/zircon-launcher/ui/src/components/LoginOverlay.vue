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
      <img
        :src="zirconTitle"
        alt="Zircon"
        class="h-10 w-auto mx-auto mb-5 select-none drop-shadow-[0_0_18px_rgba(71,210,201,0.35)]"
        draggable="false"
      />
      <h2 class="text-white text-lg font-bold mb-2">Sign in with Microsoft</h2>
      <p class="text-muted text-sm mb-6">
        Sign in to launch and play on Zircon servers. Your Minecraft profile,
        skins and saved servers are stored locally.
      </p>

      <button
        class="z-btn w-full py-2.5 text-base disabled:opacity-60 flex items-center justify-center gap-2.5 bg-white text-[#1f2328] shadow-lg shadow-black/25 hover:bg-gray-50 hover:shadow-xl active:translate-y-px"
        :disabled="busy"
        @click="onLogin"
      >
        <svg v-if="!busy" class="w-5 h-5 shrink-0" viewBox="0 0 23 23" aria-hidden="true">
          <path fill="#f35325" d="M0 0h11v11H0z" />
          <path fill="#81bc06" d="M12 0h11v11H12z" />
          <path fill="#05a6f0" d="M0 12h11v11H0z" />
          <path fill="#ffba08" d="M12 12h11v11H12z" />
        </svg>
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
import zirconTitle from '../assets/zircon-title.svg';

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
