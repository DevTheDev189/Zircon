<template>
  <div
    v-if="visible"
    class="absolute inset-0 z-50 flex items-center justify-center bg-[#070b0f]/85 backdrop-blur-md p-4"
  >
    <!-- Soft cyan ambient glow behind the card -->
    <div class="absolute inset-0 overflow-hidden pointer-events-none">
      <div
        class="absolute top-1/2 left-1/2 -translate-x-1/2 -translate-y-1/2 w-[520px] h-[520px] rounded-full bg-accent/12 blur-3xl"
      ></div>
    </div>

    <div class="z-card w-full max-w-[420px] text-center p-8 shadow-2xl relative border border-slate-700/60 overflow-hidden">
      <img
        :src="zirconTitle"
        alt="Zircon"
        class="h-10 w-auto mx-auto mb-4 select-none drop-shadow-[0_0_18px_rgba(71,210,201,0.35)]"
        draggable="false"
      />
      <h2 class="text-white text-lg font-bold mb-1.5 tracking-tight">Sign in with Microsoft</h2>
      <p class="text-slate-400 text-xs mb-6 leading-relaxed">
        Sign in to launch and play on Zircon servers. Your Minecraft profile,
        skins and saved servers are stored locally.
      </p>

      <button
        class="z-btn w-full py-2.5 text-sm font-bold disabled:opacity-60 flex items-center justify-center gap-2.5 bg-white text-[#1f2328] shadow-lg shadow-black/30 hover:bg-gray-100 hover:shadow-xl active:translate-y-px rounded-xl transition-all"
        :disabled="busy"
        @click="onLogin"
      >
        <svg v-if="!busy" class="w-4 h-4 shrink-0" viewBox="0 0 23 23" aria-hidden="true">
          <path fill="#f35325" d="M0 0h11v11H0z" />
          <path fill="#81bc06" d="M12 0h11v11H12z" />
          <path fill="#05a6f0" d="M0 12h11v11H0z" />
          <path fill="#ffba08" d="M12 12h11v11H12z" />
        </svg>
        <span v-if="busy">Opening browser…</span>
        <span v-else>Continue with Microsoft</span>
      </button>

      <p v-if="status" class="text-xs text-cyan-300/90 font-medium mt-4 whitespace-pre-line">
        {{ status }}
      </p>

      <!-- Ownership required card -->
      <div
        v-if="isOwnershipError"
        class="mt-4 p-3.5 rounded-xl bg-amber-500/10 border border-amber-500/30 text-left"
      >
        <div class="flex items-start gap-2.5">
          <svg
            class="w-5 h-5 text-amber-400 shrink-0 mt-0.5"
            fill="none"
            viewBox="0 0 24 24"
            stroke="currentColor"
          >
            <path
              stroke-linecap="round"
              stroke-linejoin="round"
              stroke-width="2"
              d="M12 9v2m0 4h.01m-6.938 4h13.856c1.54 0 2.502-1.667 1.732-3L13.732 4c-.77-1.333-2.694-1.333-3.464 0L3.34 16c-.77 1.333.192 3 1.732 3z"
            />
          </svg>
          <div class="flex-1">
            <p class="text-xs font-bold text-amber-300">Minecraft Java Edition Required</p>
            <p class="text-xs text-amber-200/80 mt-1 leading-relaxed">
              This Microsoft account does not own Minecraft Java Edition. You must purchase Minecraft Java Edition to play on Zircon.
            </p>
          </div>
        </div>

        <button
          type="button"
          class="w-full mt-3 py-2.5 px-4 rounded-xl text-xs font-bold flex items-center justify-center gap-2 bg-gradient-to-r from-emerald-500 to-teal-500 hover:from-emerald-400 hover:to-teal-400 text-slate-950 shadow-lg shadow-emerald-500/20 active:scale-[0.98] transition-all cursor-pointer"
          @click="openBuyPage"
        >
          <svg class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor">
            <path
              stroke-linecap="round"
              stroke-linejoin="round"
              stroke-width="2"
              d="M3 3h2l.4 2M7 13h10l4-8H5.4M7 13L5.4 5M7 13l-2.293 2.293c-.63.63-.184 1.707.707 1.707H17m0 0a2 2 0 100 4 2 2 0 000-4zm-8 2a2 2 0 11-4 0 2 2 0 014 0z"
            />
          </svg>
          <span>Buy Minecraft Java Edition</span>
          <svg class="w-3.5 h-3.5 opacity-70" fill="none" viewBox="0 0 24 24" stroke="currentColor">
            <path
              stroke-linecap="round"
              stroke-linejoin="round"
              stroke-width="2"
              d="M10 6H6a2 2 0 00-2 2v10a2 2 0 002 2h10a2 2 0 002-2v-4M14 4h6m0 0v6m0-6L10 14"
            />
          </svg>
        </button>
      </div>

      <!-- Generic error message -->
      <p
        v-else-if="error"
        class="text-xs text-[#f87171] mt-3 font-medium whitespace-pre-line"
      >
        {{ error }}
      </p>
    </div>
  </div>
</template>

<script setup>
import { ref, computed } from 'vue';
import { api } from '../lib/api';
import zirconTitle from '../assets/zircon-title.svg';

const MINECRAFT_BUY_URL = 'https://www.minecraft.net/store/minecraft-java-bedrock-edition-pc';

const emit = defineEmits(['logged-in']);

defineProps({
  visible: { type: Boolean, default: false },
});

const busy = ref(false);
const status = ref('');
const error = ref('');

const isOwnershipError = computed(() => {
  if (!error.value) return false;
  const msg = error.value.toLowerCase();
  return (
    msg.includes('does not own minecraft') ||
    (msg.includes('404') && (msg.includes('minecraft') || msg.includes('profile') || msg.includes('http 404'))) ||
    msg.includes('minecraft.net/store')
  );
});

function openBuyPage() {
  api.openBrowserUrl(MINECRAFT_BUY_URL).catch(() => {
    window.open(MINECRAFT_BUY_URL, '_blank', 'noopener,noreferrer');
  });
}

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
