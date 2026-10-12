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
      <h2 class="text-white text-lg font-bold mb-1.5 tracking-tight">{{ $t('auth.sign_in_title') }}</h2>
      <p class="text-slate-400 text-xs mb-6 leading-relaxed">
        {{ $t('auth.sign_in_description') }}
      </p>

      <button
        class="z-btn w-full py-2.5 text-sm font-bold disabled:opacity-60 flex items-center justify-center gap-2.5 bg-white text-[#1f2328] shadow-lg shadow-black/30 hover:bg-gray-100 hover:shadow-xl active:translate-y-px rounded-xl transition-all cursor-pointer"
        :disabled="busy"
        @click="onLogin"
      >
        <svg v-if="!busy" class="w-4 h-4 shrink-0" viewBox="0 0 23 23" aria-hidden="true">
          <path fill="#f35325" d="M0 0h11v11H0z" />
          <path fill="#81bc06" d="M12 0h11v11H12z" />
          <path fill="#05a6f0" d="M0 12h11v11H0z" />
          <path fill="#ffba08" d="M12 12h11v11H12z" />
        </svg>
        <span v-if="busy">{{ $t('auth.opening_browser') }}</span>
        <span v-else>{{ $t('auth.continue_with_microsoft') }}</span>
      </button>

      <p v-if="status && !manualAuthUrl" class="text-xs text-cyan-300/90 font-medium mt-4 whitespace-pre-line">
        {{ status }}
      </p>

      <!-- Manual Browser Fallback Card (for Linux, minimal WMs, or when browser fails to launch) -->
      <div
        v-if="manualAuthUrl"
        class="mt-4 p-4 rounded-2xl bg-slate-900/95 border border-cyan-500/40 text-left shadow-xl shadow-cyan-950/30 animate-fadeIn"
      >
        <div class="flex items-start gap-3 mb-3">
          <div
            class="w-8 h-8 rounded-xl flex items-center justify-center shrink-0 border"
            :class="!autoOpened ? 'bg-amber-500/15 border-amber-500/30 text-amber-400' : 'bg-cyan-500/15 border-cyan-500/30 text-cyan-400'"
          >
            <svg class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M10 6H6a2 2 0 00-2 2v10a2 2 0 002 2h10a2 2 0 002-2v-4M14 4h6m0 0v6m0-6L10 14" />
            </svg>
          </div>
          <div class="flex-1 min-w-0">
            <p class="text-xs font-bold text-white tracking-wide">
              {{ !autoOpened ? $t('auth.manual_browser_title') : $t('auth.browser_opened_title') }}
            </p>
            <p class="text-[11px] text-slate-400 mt-0.5 leading-relaxed">
              {{ !autoOpened ? $t('auth.manual_browser_desc') : $t('auth.browser_opened_desc') }}
            </p>
          </div>
        </div>

        <!-- Action Buttons -->
        <div class="flex items-center gap-2 mt-3.5">
          <button
            type="button"
            class="flex-1 py-2 px-3 rounded-xl text-xs font-bold bg-cyan-500 hover:bg-cyan-400 text-slate-950 shadow-md shadow-cyan-500/20 active:scale-[0.98] transition-all flex items-center justify-center gap-1.5 cursor-pointer"
            @click="openManualAuthUrl"
          >
            <svg class="w-3.5 h-3.5" fill="none" viewBox="0 0 24 24" stroke="currentColor">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M10 6H6a2 2 0 00-2 2v10a2 2 0 002 2h10a2 2 0 002-2v-4M14 4h6m0 0v6m0-6L10 14" />
            </svg>
            <span>{{ $t('auth.open_in_browser') }}</span>
          </button>

          <button
            type="button"
            class="py-2 px-3 rounded-xl text-xs font-bold bg-slate-800/90 hover:bg-slate-700 text-slate-200 border border-slate-700/80 active:scale-[0.98] transition-all flex items-center justify-center gap-1.5 cursor-pointer"
            @click="copyManualAuthUrl"
          >
            <svg v-if="!copied" class="w-3.5 h-3.5 text-slate-400" fill="none" viewBox="0 0 24 24" stroke="currentColor">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M8 16H6a2 2 0 01-2-2V6a2 2 0 012-2h8a2 2 0 012 2v2m-6 12h8a2 2 0 002-2v-8a2 2 0 00-2-2h-8a2 2 0 00-2 2v8a2 2 0 002 2z" />
            </svg>
            <svg v-else class="w-3.5 h-3.5 text-emerald-400" fill="none" viewBox="0 0 24 24" stroke="currentColor">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M5 13l4 4L19 7" />
            </svg>
            <span>{{ copied ? $t('auth.copied') : $t('auth.copy_link') }}</span>
          </button>
        </div>

        <!-- URL Preview Snippet -->
        <div class="mt-2.5 bg-[#05080e] p-2 rounded-lg border border-slate-800 text-[10px] font-mono text-slate-400 truncate select-all">
          {{ manualAuthUrl }}
        </div>

        <!-- Listening status pill -->
        <div class="mt-2.5 flex items-center gap-2 text-[10px] text-cyan-300 font-mono">
          <span class="w-1.5 h-1.5 rounded-full bg-cyan-400 animate-ping"></span>
          <span>{{ $t('auth.waiting_for_browser') }}</span>
        </div>
      </div>

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
            <p class="text-xs font-bold text-amber-300">{{ $t('auth.minecraft_required_title') }}</p>
            <p class="text-xs text-amber-200/80 mt-1 leading-relaxed">
              {{ $t('auth.minecraft_required_description') }}
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
          <span>{{ $t('auth.buy_minecraft_button') }}</span>
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
import { ref, computed, onMounted, onUnmounted } from 'vue';
import { api, onAuthManualUrl } from '../lib/api';
import zirconTitle from '../assets/zircon-title.svg';

const MINECRAFT_BUY_URL = 'https://www.minecraft.net/store/minecraft-java-bedrock-edition-pc';

const emit = defineEmits(['logged-in']);

const props = defineProps({
  visible: { type: Boolean, default: false },
});

const busy = ref(false);
const status = ref('');
const error = ref('');
const manualAuthUrl = ref('');
const autoOpened = ref(true);
const copied = ref(false);
let unlistenAuthUrl = null;

onMounted(async () => {
  try {
    unlistenAuthUrl = await onAuthManualUrl((payload) => {
      if (payload && payload.url) {
        manualAuthUrl.value = payload.url;
        autoOpened.value = payload.browser_opened;
      }
    });
  } catch (err) {
    console.warn('Failed to listen to auth-manual-url:', err);
  }
});

onUnmounted(() => {
  if (unlistenAuthUrl) {
    unlistenAuthUrl();
  }
});

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

function openManualAuthUrl() {
  if (!manualAuthUrl.value) return;
  api.openBrowserUrl(manualAuthUrl.value).catch(() => {
    window.open(manualAuthUrl.value, '_blank', 'noopener,noreferrer');
  });
}

function copyManualAuthUrl() {
  if (!manualAuthUrl.value) return;
  navigator.clipboard.writeText(manualAuthUrl.value).then(() => {
    copied.value = true;
    setTimeout(() => {
      copied.value = false;
    }, 2500);
  }).catch(() => {});
}

async function onLogin() {
  busy.value = true;
  error.value = '';
  manualAuthUrl.value = '';
  copied.value = false;
  autoOpened.value = true;
  status.value = 'Opening browser for Microsoft login…';
  try {
    const session = await api.loginMicrosoft();
    manualAuthUrl.value = '';
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
