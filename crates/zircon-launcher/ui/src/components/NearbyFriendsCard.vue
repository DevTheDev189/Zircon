<template>
  <div
    v-if="servers.length > 0 || showEmptyState"
    class="mb-4 rounded-2xl bg-gradient-to-r from-emerald-950/30 via-slate-900/60 to-slate-950/80 border border-emerald-500/30 p-4 shadow-[0_0_25px_rgba(16,185,129,0.12)] backdrop-blur-md transition-all duration-300"
  >
    <!-- Header with radar pulse -->
    <div class="flex items-center justify-between mb-3">
      <div class="flex items-center gap-2.5">
        <span class="relative flex h-2.5 w-2.5">
          <span class="animate-ping absolute inline-flex h-full w-full rounded-full bg-emerald-400 opacity-75"></span>
          <span class="relative inline-flex rounded-full h-2.5 w-2.5 bg-emerald-500"></span>
        </span>
        <div class="flex items-center gap-2">
          <h3 class="text-sm font-bold text-white tracking-wide flex items-center gap-1.5">
            <span>{{ $t('nearby_friends.title') }}</span>
          </h3>
          <span
            v-if="servers.length > 0"
            class="px-2 py-0.5 rounded-full text-[10px] font-bold bg-emerald-500/20 text-emerald-300 border border-emerald-500/40 font-mono shadow-[0_0_8px_rgba(16,185,129,0.3)]"
          >
            {{ $t('nearby_friends.active_badge', { count: servers.length }) }}
          </span>
        </div>
      </div>

      <div class="flex items-center gap-2">
        <span class="text-[11px] text-slate-400 font-mono hidden sm:inline">
          {{ $t('nearby_friends.dual_beacon') }}
        </span>
        <button
          class="p-1.5 rounded-lg text-slate-400 hover:text-white hover:bg-slate-800/60 transition-colors"
          :title="$t('nearby_friends.refresh_tooltip')"
          @click="refreshScan"
        >
          <svg class="w-3.5 h-3.5" :class="{ 'animate-spin': isRefreshing }" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
            <path stroke-linecap="round" stroke-linejoin="round" d="M4 4v5h.582m15.356 2A8.001 8.001 0 004.582 9m0 0H9m11 11v-5h-.581m0 0a8.003 8.003 0 01-15.357-2m15.357 2H15" />
          </svg>
        </button>
      </div>
    </div>

    <!-- Empty state when manually shown or searching -->
    <div
      v-if="servers.length === 0"
      class="flex items-center gap-3 py-3 px-3.5 bg-slate-950/40 border border-dashed border-slate-800 rounded-xl"
    >
      <div class="w-8 h-8 rounded-lg bg-emerald-500/10 border border-emerald-500/20 flex items-center justify-center text-emerald-400 shrink-0">
        <svg class="w-4 h-4 animate-pulse" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
          <circle cx="12" cy="12" r="10" />
          <path d="M12 2a14.5 14.5 0 0 0 0 20 14.5 14.5 0 0 0 0-20" />
          <path d="M2 12h20" />
        </svg>
      </div>
      <div class="flex-1 min-w-0">
        <div class="text-xs font-semibold text-slate-300">{{ $t('nearby_friends.scanning_wifi') }}</div>
        <div class="text-[11px] text-slate-500 truncate">{{ $t('nearby_friends.scanning_desc') }}</div>
      </div>
    </div>

    <!-- Active discovered server cards -->
    <div v-else class="grid grid-cols-1 md:grid-cols-2 gap-2.5">
      <div
        v-for="srv in servers"
        :key="srv.id || srv.address"
        class="group/srv flex items-center justify-between p-3 rounded-xl bg-slate-900/80 border border-slate-700/80 hover:border-emerald-500/50 hover:bg-slate-900 hover:shadow-[0_0_18px_rgba(16,185,129,0.2)] transition-all duration-200"
      >
        <div class="flex items-center gap-3 min-w-0 flex-1 pr-2">
          <!-- Host Head Avatar or Network Beacon Icon -->
          <div class="relative w-10 h-10 rounded-xl bg-slate-950 border border-slate-800 overflow-hidden flex items-center justify-center shrink-0 shadow-sm">
            <img
              v-if="srv.hostUsername"
              :src="`https://mc-heads.net/avatar/${srv.hostUsername}/40`"
              class="w-full h-full object-cover image-render-pixel"
              :alt="srv.hostUsername"
              @error="(e) => e.target.style.display = 'none'"
            />
            <div
              v-else
              class="w-full h-full bg-gradient-to-br from-emerald-500/20 to-teal-700/30 flex items-center justify-center text-emerald-400"
            >
              <svg class="w-5 h-5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                <path stroke-linecap="round" stroke-linejoin="round" d="M13 10V3L4 14h7v7l9-11h-7z" />
              </svg>
            </div>
            <span
              v-if="srv.isZirconEnhanced"
              class="absolute -bottom-0.5 -right-0.5 w-3.5 h-3.5 rounded-full bg-cyan-500 border-2 border-slate-900 flex items-center justify-center text-[7px] font-bold text-black"
              title="Zircon Enhanced Beacon"
            >
              Z
            </span>
          </div>

          <!-- Metadata -->
          <div class="min-w-0 flex-1">
            <div class="flex items-center gap-1.5 mb-0.5">
              <span class="text-xs font-bold text-white truncate max-w-[150px]">
                {{ srv.motd || srv.name || $t('nearby_friends.local_world') }}
              </span>
              <span
                v-if="srv.isZirconEnhanced"
                class="px-1.5 py-0.2 rounded text-[9px] font-bold bg-cyan-500/15 text-cyan-300 border border-cyan-500/30"
              >
                {{ $t('nearby_friends.enhanced_badge') }}
              </span>
            </div>

            <div class="flex items-center gap-2 text-[10px] text-slate-400 font-mono truncate">
              <span class="text-emerald-400 font-medium truncate">
                {{ srv.hostUsername ? $t('nearby_friends.users_world', { user: srv.hostUsername }) : srv.hostAddress }}
              </span>
              <span class="text-slate-600">•</span>
              <span class="truncate">{{ srv.address }}</span>
            </div>

            <div class="flex items-center gap-1.5 mt-1">
              <span
                v-if="srv.mcVersion"
                class="px-1.5 py-0.2 rounded text-[9px] font-mono bg-slate-950 text-slate-300 border border-slate-800"
              >
                {{ srv.mcVersion }}
              </span>
              <span
                v-if="srv.loaderType"
                class="px-1.5 py-0.2 rounded text-[9px] font-semibold bg-emerald-950/40 text-emerald-300 border border-emerald-800/40"
              >
                {{ srv.loaderType }}
              </span>
              <span
                v-if="srv.joinCode"
                class="px-1.5 py-0.2 rounded text-[9px] font-mono font-bold bg-purple-500/20 text-purple-300 border border-purple-500/40"
              >
                ZK-{{ srv.joinCode }}
              </span>
            </div>
          </div>
        </div>

        <!-- Join Action -->
        <button
          class="shrink-0 px-3.5 py-2 rounded-xl text-xs font-bold bg-emerald-500 text-slate-950 hover:bg-emerald-400 transition-all shadow-[0_0_12px_rgba(16,185,129,0.3)] hover:shadow-[0_0_18px_rgba(16,185,129,0.5)] flex items-center gap-1.5 cursor-pointer active:scale-95"
          @click="$emit('join', srv)"
        >
          <svg class="w-3.5 h-3.5 fill-current" viewBox="0 0 24 24">
            <path d="M8 5v14l11-7z" />
          </svg>
          <span>{{ $t('nearby_friends.join_button') }}</span>
        </button>
      </div>
    </div>
  </div>
</template>

<script setup>
import { onBeforeUnmount, onMounted, ref } from 'vue';
import { api, onLanServersChanged } from '../lib/api';

const props = defineProps({
  showEmptyState: {
    type: Boolean,
    default: false,
  },
});

defineEmits(['join']);

const servers = ref([]);
const isRefreshing = ref(false);
let unlistenLan = null;

async function refreshScan() {
  isRefreshing.value = true;
  try {
    const list = await api.getDiscoveredLanServers();
    servers.value = Array.isArray(list) ? list : [];
  } catch (err) {
    console.error('Failed to load discovered LAN servers:', err);
  } finally {
    setTimeout(() => {
      isRefreshing.value = false;
    }, 500);
  }
}

onMounted(async () => {
  await refreshScan();
  try {
    unlistenLan = await onLanServersChanged((updatedList) => {
      if (Array.isArray(updatedList)) {
        servers.value = updatedList;
      }
    });
  } catch (err) {
    console.error('Failed to register onLanServersChanged listener:', err);
  }
});

onBeforeUnmount(() => {
  if (typeof unlistenLan === 'function') {
    unlistenLan();
  }
});
</script>
