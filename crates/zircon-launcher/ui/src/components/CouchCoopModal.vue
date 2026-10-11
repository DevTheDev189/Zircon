<template>
  <div
    v-if="isOpen"
    class="fixed inset-0 z-50 flex items-center justify-center p-4 bg-black/80 backdrop-blur-xl animate-fade-in"
    @click.self="close"
  >
    <div
      class="relative w-full max-w-4xl bg-gradient-to-b from-[#0e1622]/95 via-[#0e1622]/90 to-[#070b10]/95 border border-white/10 rounded-2xl shadow-[0_24px_64px_rgba(0,0,0,0.85),0_0_40px_var(--color-accent-glow)] flex flex-col max-h-[92vh] overflow-hidden text-slate-100 backdrop-blur-2xl"
    >
      <!-- Top Specular Rim Highlight -->
      <div class="absolute inset-x-0 top-0 h-px bg-gradient-to-r from-transparent via-white/20 to-transparent pointer-events-none"></div>

      <!-- Header -->
      <div class="px-7 py-[22px] border-b border-white/10 bg-gradient-to-r from-[#0e1622]/90 via-[#0e1622]/75 to-[#070b10]/90 flex items-center justify-between relative z-10">
        <div class="flex items-center gap-4">
          <div class="w-11 h-11 rounded-2xl bg-gradient-to-br from-accent/25 via-accent/15 to-transparent border border-accent/40 flex items-center justify-center text-accent-bright shadow-[0_0_20px_var(--color-accent-glow)] shrink-0">
            <svg class="w-5 h-5" fill="none" viewBox="0 0 24 24" stroke="currentColor">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M15 5v2m0 4v2m0 4v2M5 5a2 2 0 00-2 2v3a2 2 0 110 4v3a2 2 0 002 2h14a2 2 0 002-2v-3a2 2 0 110-4V7a2 2 0 00-2-2H5z" />
            </svg>
          </div>
          <div>
            <div class="flex items-center gap-2.5 flex-wrap">
              <h2 class="text-base font-extrabold text-white tracking-wide">{{ $t('coop.title') }}</h2>
              <span class="px-2.5 py-0.5 rounded-full text-[10px] font-bold bg-accent/15 text-accent-bright border border-accent/30 font-mono shadow-[0_0_10px_var(--color-accent-glow)]">
                {{ serverAddress ? $t('coop.online_server') : $t('coop.local_world') }}
              </span>
              <span class="px-2.5 py-0.5 rounded-full text-[10px] font-mono font-bold bg-white/5 text-slate-300 border border-white/10">
                {{ $t('coop.viewports', { count: playerCount }) }}
              </span>
            </div>
            <p class="text-xs text-slate-400 mt-1">
              {{ serverAddress ? $t('coop.subtitle_server', { address: serverAddress }) : $t('coop.subtitle_local') }}
            </p>
          </div>
        </div>
        <button
          class="p-2.5 text-slate-400 hover:text-white rounded-xl hover:bg-white/10 transition-colors cursor-pointer shrink-0"
          @click="close"
        >
          <svg class="w-5 h-5" fill="none" viewBox="0 0 24 24" stroke="currentColor">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 18L18 6M6 6l12 12" />
          </svg>
        </button>
      </div>

      <!-- Body -->
      <div class="flex-1 overflow-y-auto p-6 space-y-6">
        <!-- Active Session Banner -->
        <div
          v-if="sessionStatus.isActive"
          class="p-4 rounded-xl border border-accent/40 bg-gradient-to-r from-accent/15 via-accent/10 to-transparent flex items-center justify-between gap-4 shadow-[0_0_24px_var(--color-accent-glow)] backdrop-blur-md"
        >
          <div class="flex items-center gap-3">
            <span class="flex h-3 w-3 relative">
              <span class="animate-ping absolute inline-flex h-full w-full rounded-full bg-accent-bright opacity-75"></span>
              <span class="relative inline-flex rounded-full h-3 w-3 bg-accent"></span>
            </span>
            <div>
              <div class="text-sm font-bold text-accent-bright">{{ $t('coop.session_active') }}</div>
              <div class="text-xs text-slate-300">
                {{ $t('coop.instances_running', { count: sessionStatus.playerCount, pids: sessionStatus.pids?.join(', ') }) }}
              </div>
            </div>
          </div>
          <button
            class="z-btn-danger px-3.5 py-1.5 rounded-xl text-xs font-bold transition flex items-center gap-2 cursor-pointer shadow-[0_0_12px_rgba(239,68,68,0.3)]"
            :disabled="stopping"
            @click="stopSession"
          >
            <span v-if="stopping" class="w-3.5 h-3.5 border-2 border-red-300 border-t-transparent rounded-full animate-spin"></span>
            <span>{{ stopping ? $t('coop.stopping') : $t('coop.stop_session') }}</span>
          </button>
        </div>

        <!-- Section 1: Player Count & Layout Picker -->
        <div class="grid grid-cols-1 md:grid-cols-2 gap-5">
          <!-- Player Count Stepper -->
          <div class="bg-[#070b10]/75 border border-white/10 rounded-2xl p-5 flex flex-col justify-between backdrop-blur-md shadow-lg shadow-black/20">
            <div>
              <div class="text-xs font-bold uppercase tracking-wider text-slate-400 mb-1">{{ $t('coop.number_of_viewports') }}</div>
              <p class="text-xs text-slate-500 mb-3.5">{{ $t('coop.select_players_desc') }}</p>
            </div>
            <div class="flex items-center gap-2.5">
              <button
                v-for="count in [2, 3, 4]"
                :key="count"
                type="button"
                class="flex-1 py-3 px-3 rounded-xl font-bold text-xs transition border flex items-center justify-center gap-2 cursor-pointer"
                :class="
                  playerCount === count
                    ? 'bg-gradient-to-r from-accent to-accent-deep text-accent-ink font-black shadow-[0_0_16px_var(--color-accent-glow)] border-accent-bright'
                    : 'bg-[#0e1622]/70 text-slate-400 border-white/5 hover:border-accent/40 hover:text-slate-200'
                "
                @click="setPlayerCount(count)"
              >
                <span>{{ $t('coop.players_count', { count }) }}</span>
              </button>
            </div>
          </div>

          <!-- Layout Partition Selector -->
          <div class="bg-[#070b10]/75 border border-white/10 rounded-2xl p-5 backdrop-blur-md shadow-lg shadow-black/20">
            <div class="text-xs font-bold uppercase tracking-wider text-slate-400 mb-1">{{ $t('coop.layout_partition') }}</div>
            <p class="text-xs text-slate-500 mb-3.5">{{ $t('coop.choose_layout_desc') }}</p>
            <div class="grid grid-cols-2 gap-3" v-if="playerCount === 2">
              <button
                type="button"
                class="p-3 rounded-xl border text-xs font-semibold flex items-center gap-3.5 transition cursor-pointer"
                :class="
                  selectedLayout === 'twoPlayerVertical'
                    ? 'bg-accent/15 text-accent-bright border-accent/60 shadow-[0_0_16px_var(--color-accent-glow)]'
                    : 'bg-[#0e1622]/70 text-slate-400 border-white/5 hover:border-white/15 hover:text-white'
                "
                @click="selectedLayout = 'twoPlayerVertical'"
              >
                <div class="w-8 h-6 border border-slate-600 rounded flex overflow-hidden shrink-0">
                  <div class="w-1/2 h-full border-r border-accent/40 bg-accent/45"></div>
                  <div class="w-1/2 h-full bg-accent/20"></div>
                </div>
                <span>{{ $t('coop.vertical_split') }}</span>
              </button>
              <button
                type="button"
                class="p-3 rounded-xl border text-xs font-semibold flex items-center gap-3.5 transition cursor-pointer"
                :class="
                  selectedLayout === 'twoPlayerHorizontal'
                    ? 'bg-accent/15 text-accent-bright border-accent/60 shadow-[0_0_16px_var(--color-accent-glow)]'
                    : 'bg-[#0e1622]/70 text-slate-400 border-white/5 hover:border-white/15 hover:text-white'
                "
                @click="selectedLayout = 'twoPlayerHorizontal'"
              >
                <div class="w-8 h-6 border border-slate-600 rounded flex flex-col overflow-hidden shrink-0">
                  <div class="w-full h-1/2 border-b border-accent/40 bg-accent/45"></div>
                  <div class="w-full h-1/2 bg-accent/20"></div>
                </div>
                <span>{{ $t('coop.horizontal_split') }}</span>
              </button>
            </div>
            <div v-else-if="playerCount === 3" class="p-3 rounded-xl border border-accent/40 bg-accent/10 text-accent-bright text-xs flex items-center gap-3.5 shadow-[0_0_14px_var(--color-accent-glow)]">
              <div class="w-8 h-6 border border-accent/40 rounded flex flex-col overflow-hidden shrink-0">
                <div class="w-full h-1/2 border-b border-accent/40 bg-accent/45"></div>
                <div class="w-full h-1/2 flex">
                  <div class="w-1/2 h-full border-r border-accent/40 bg-accent/28"></div>
                  <div class="w-1/2 h-full bg-accent/16"></div>
                </div>
              </div>
              <span class="text-xs text-slate-200">{{ $t('coop.layout_3p_desc') }}</span>
            </div>
            <div v-else class="p-3 rounded-xl border border-accent/40 bg-accent/10 text-accent-bright text-xs flex items-center gap-3.5 shadow-[0_0_14px_var(--color-accent-glow)]">
              <div class="w-8 h-6 border border-accent/40 rounded grid grid-cols-2 grid-rows-2 overflow-hidden shrink-0">
                <div class="border-r border-b border-accent/40 bg-accent/45"></div>
                <div class="border-b border-accent/40 bg-accent/30"></div>
                <div class="border-r border-accent/40 bg-accent/22"></div>
                <div class="bg-accent/14"></div>
              </div>
              <span class="text-xs text-slate-200">{{ $t('coop.layout_4p_desc') }}</span>
            </div>
          </div>
        </div>

        <!-- Account Notice (if more accounts needed) -->
        <div
          v-if="availableAccounts.length < playerCount"
          class="px-5 py-4 rounded-2xl border border-accent/30 bg-accent/10 flex items-center justify-between gap-4 backdrop-blur-md shadow-[0_0_16px_var(--color-accent-glow)]/30 my-1"
        >
          <div class="flex items-center gap-3.5 pl-1.5">
            <div class="w-8 h-8 rounded-xl bg-accent/20 border border-accent/30 flex items-center justify-center shrink-0 shadow-[0_0_10px_var(--color-accent-glow)]">
              <svg class="w-4.5 h-4.5 text-accent-bright" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M13 16h-1v-4h-1m1-4h.01M21 12a9 9 0 11-18 0 9 9 0 0118 0z" />
              </svg>
            </div>
            <div class="text-xs text-slate-200">
              <span class="font-bold text-accent-bright">{{ $t('coop.assign_profiles_title') }} </span>
              <span>{{ $t('coop.assign_profiles_desc', { count: availableAccounts.length }) }}</span>
            </div>
          </div>
          <button
            type="button"
            class="z-btn-secondary px-4 py-2 rounded-xl text-xs font-bold transition flex items-center gap-2 shrink-0 hover:border-accent/50 shadow-sm cursor-pointer"
            :disabled="loggingIn"
            @click="addAccount"
          >
            <span v-if="loggingIn" class="w-3.5 h-3.5 border-2 border-accent-bright border-t-transparent rounded-full animate-spin"></span>
            <svg v-else class="w-3.5 h-3.5 text-accent-bright" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
              <path stroke-linecap="round" stroke-linejoin="round" d="M12 4v16m8-8H4"/>
            </svg>
            <span class="text-slate-100">{{ loggingIn ? $t('coop.signing_in') : $t('coop.add_account') }}</span>
          </button>
        </div>

        <!-- Section 2: Player Configurations -->
        <div>
          <div class="flex items-center justify-between mb-4">
            <h3 class="text-xs font-bold uppercase tracking-wider text-slate-400">{{ $t('coop.viewport_config_title') }}</h3>
            <div class="flex items-center gap-4">
              <button
                type="button"
                class="text-xs text-accent-bright hover:text-white font-semibold flex items-center gap-1.5 transition cursor-pointer"
                :disabled="loggingIn"
                @click="addAccount"
              >
                <svg class="w-3.5 h-3.5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                  <path stroke-linecap="round" stroke-linejoin="round" d="M12 4v16m8-8H4"/>
                </svg>
                <span>{{ $t('coop.add_account') }}</span>
              </button>
              <div class="flex items-center gap-2 text-[11px] text-slate-400">
                <span
                  class="w-2 h-2 rounded-full"
                  :class="detectedGamepads.length > 0 ? 'bg-accent-bright shadow-[0_0_8px_var(--color-accent-glow)]' : 'bg-slate-600'"
                ></span>
                <span>{{ $t('coop.controllers_connected', { count: detectedGamepads.length }) }}</span>
                <button
                  type="button"
                  class="text-accent-bright hover:underline text-[10px] cursor-pointer ml-1"
                  @click="scanGamepads"
                >
                  {{ $t('coop.rescan') }}
                </button>
              </div>
            </div>
          </div>

          <!-- Player Cards Grid (Harmonized shades of user theme color) -->
          <div class="grid grid-cols-1 md:grid-cols-2 gap-5">
            <div
              v-for="(player, idx) in activePlayersList"
              :key="idx"
              class="bg-[#070b10]/75 border rounded-2xl p-5 flex flex-col gap-4 relative transition-all backdrop-blur-md shadow-lg shadow-black/20"
              :class="getPlayerCardBorderClass(idx, isDuplicateAccount(player.accountUuid, idx))"
            >
              <!-- Player Header Badge -->
              <div class="flex items-center justify-between">
                <div class="flex items-center gap-2.5">
                  <span
                    class="w-7 h-7 rounded-xl text-xs font-black flex items-center justify-center font-mono transition-transform hover:scale-105"
                    :class="getPlayerBadgeClass(idx)"
                  >
                    P{{ idx + 1 }}
                  </span>
                  <div>
                    <span class="font-bold text-xs text-white">
                      {{ idx === 0 ? $t('coop.host_viewport') : $t('coop.player_viewport', { num: idx + 1 }) }}
                    </span>
                  </div>
                </div>
                <span
                  v-if="player.name"
                  class="text-[10px] font-mono px-2 py-0.5 rounded-lg bg-accent/10 text-accent-bright border border-accent/25 truncate max-w-[140px]"
                >
                  {{ player.name }}
                </span>
              </div>

              <!-- Account Selection -->
              <div>
                <div class="flex items-center justify-between mb-1.5">
                  <label class="block text-[11px] font-medium text-slate-400">
                    {{ $t('coop.player_profile') }}
                  </label>
                  <button
                    type="button"
                    class="text-[11px] text-accent-bright hover:underline font-semibold cursor-pointer flex items-center gap-1 transition"
                    :disabled="loggingIn"
                    @click="addAccount"
                    :title="$t('coop.add_account')"
                  >
                    <span>+ {{ $t('coop.add_account') }}</span>
                  </button>
                </div>
                <select
                  v-model="player.accountUuid"
                  class="z-input w-full text-xs"
                  :class="isDuplicateAccount(player.accountUuid, idx) ? 'border-red-500 bg-red-500/10' : ''"
                  @change="onAccountSelected(player, idx)"
                >
                  <option value="" disabled>{{ $t('coop.select_account_placeholder') }}</option>
                  <option
                    v-for="acc in availableAccounts"
                    :key="acc.uuid"
                    :value="acc.uuid"
                    :disabled="isAccountUsedByOther(acc.uuid, idx)"
                  >
                    {{ acc.username }} {{ isAccountUsedByOther(acc.uuid, idx) ? $t('coop.assigned_to_other') : '' }}
                  </option>
                </select>
                <div v-if="isDuplicateAccount(player.accountUuid, idx)" class="text-[10px] text-red-400 mt-1 flex items-center gap-1">
                  <svg class="w-3 h-3 shrink-0" fill="currentColor" viewBox="0 0 20 20">
                    <path fill-rule="evenodd" d="M18 10a8 8 0 11-16 0 8 8 0 0116 0zm-7 4a1 1 0 11-2 0 1 1 0 012 0zm-1-9a1 1 0 00-1 1v4a1 1 0 102 0V6a1 1 0 00-1-1z" clip-rule="evenodd"/>
                  </svg>
                  <span>{{ $t('coop.account_already_assigned') }}</span>
                </div>
              </div>

              <!-- Controller Dropdown -->
              <div>
                <label class="block text-[11px] font-medium text-slate-400 mb-1.5">{{ $t('coop.input_device_label') }}</label>
                <select
                  v-model="player.controllerId"
                  class="z-input w-full text-xs"
                >
                  <option :value="null">
                    {{ idx === 0 ? $t('coop.keyboard_mouse_primary') : $t('coop.keyboard_mouse_fallback') }}
                  </option>
                  <option
                    v-for="gp in detectedGamepads"
                    :key="gp.id + gp.index"
                    :value="gp.id"
                  >
                    {{ $t('coop.controller_device', { index: gp.index + 1, id: gp.id.slice(0, 30) }) }}
                  </option>
                </select>
              </div>

              <!-- Sliders: RAM & Render Distance -->
              <div class="grid grid-cols-2 gap-4 pt-3 mt-1 border-t border-white/5">
                <div>
                  <div class="flex justify-between items-center text-[11px] text-slate-400 mb-1.5">
                    <span>{{ $t('coop.ram_allocation') }}</span>
                    <span class="font-mono text-xs font-bold text-accent-bright bg-accent/10 px-2 py-0.5 rounded-lg border border-accent/20">
                      {{ player.memoryGb }} GB
                    </span>
                  </div>
                  <input
                    v-model.number="player.memoryGb"
                    type="range"
                    min="2"
                    max="8"
                    step="1"
                    class="w-full accent-[var(--color-accent)] cursor-pointer h-2 bg-slate-800 rounded-lg hover:brightness-110 my-1"
                  />
                </div>
                <div>
                  <div class="flex justify-between items-center text-[11px] text-slate-400 mb-1.5">
                    <span>{{ $t('coop.render_distance') }}</span>
                    <span class="font-mono text-xs font-bold text-accent-bright bg-accent/10 px-2 py-0.5 rounded-lg border border-accent/20">
                      {{ player.renderDistance }} ch
                    </span>
                  </div>
                  <input
                    v-model.number="player.renderDistance"
                    type="range"
                    min="4"
                    max="16"
                    step="2"
                    class="w-full accent-[var(--color-accent)] cursor-pointer h-2 bg-slate-800 rounded-lg hover:brightness-110 my-1"
                  />
                </div>
              </div>
            </div>
          </div>
        </div>

        <!-- Section 3: Summary Bar -->
        <div class="p-4.5 rounded-2xl border border-white/10 bg-[#070b10]/75 flex items-center justify-between text-xs backdrop-blur-md shadow-inner mt-4 mb-1">
          <div class="flex items-center gap-3">
            <svg class="w-5 h-5 text-accent-bright shrink-0" fill="none" viewBox="0 0 24 24" stroke="currentColor">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9.75 17L9 20l-1 1h8l-1-1-.75-3M3 13h18M5 17h14a2 2 0 002-2V5a2 2 0 00-2-2H5a2 2 0 00-2 2v10a2 2 0 002 2z" />
            </svg>
            <div>
              <span class="text-slate-300 font-medium">{{ $t('coop.session_summary') }} </span>
              <span class="font-bold text-white font-mono">{{ $t('coop.session_summary_count', { count: playerCount }) }}</span>
              <span class="text-slate-400 ml-2">{{ $t('coop.ram_allocated_format', { ram: totalRamRequested }) }}</span>
            </div>
          </div>
          <div class="inline-flex items-center gap-1.5 px-3.5 py-1.5 rounded-full bg-accent/10 border border-accent/30 text-accent-bright font-mono text-[11px] shadow-[0_0_8px_var(--color-accent-glow)]">
            <span class="w-1.5 h-1.5 rounded-full bg-accent-bright animate-pulse"></span>
            <span>{{ $t('coop.unfocused_active_badge') }}</span>
          </div>
        </div>
      </div>

      <!-- Footer -->
      <div class="px-6 py-5 border-t border-white/10 bg-gradient-to-r from-[#0e1622]/95 via-[#0e1622]/85 to-[#070b10]/95 flex items-center justify-between relative z-10">
        <button
          type="button"
          class="z-btn-ghost px-5 py-2.5 rounded-xl text-xs font-semibold text-slate-300 hover:text-white transition cursor-pointer"
          @click="close"
        >
          {{ $t('common.cancel') }}
        </button>

        <div class="flex items-center gap-3">
          <button
            type="button"
            class="z-btn-accent text-xs font-extrabold px-7 py-3 rounded-xl shadow-[0_0_24px_var(--color-accent-glow)] hover:shadow-[0_0_32px_var(--color-accent-glow-strong)] flex items-center gap-2 transition active:scale-95 disabled:opacity-40 cursor-pointer"
            :disabled="launching || sessionStatus.isActive || !canLaunch"
            @click="launchSession"
          >
            <span v-if="launching" class="w-3.5 h-3.5 border-2 border-current border-t-transparent rounded-full animate-spin"></span>
            <svg v-else class="w-4 h-4 fill-current" viewBox="0 0 24 24">
              <path d="M8 5v14l11-7z" />
            </svg>
            <span>{{ launching ? $t('offline.launching') : $t('coop.launch_btn') }}</span>
          </button>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup>
import { ref, computed, onMounted, onUnmounted, watch } from 'vue';
import { api } from '../lib/api.js';

const props = defineProps({
  isOpen: {
    type: Boolean,
    default: false,
  },
  instance: {
    type: Object,
    default: null,
  },
  serverAddress: {
    type: String,
    default: '',
  },
});

const emit = defineEmits(['close']);

const playerCount = ref(2);
const selectedLayout = ref('twoPlayerVertical');
const detectedGamepads = ref([]);
const availableAccounts = ref([]);
const launching = ref(false);
const stopping = ref(false);
const loggingIn = ref(false);
const sessionStatus = ref({
  isActive: false,
  layout: null,
  playerCount: 0,
  pids: [],
});

const players = ref([
  {
    playerIndex: 0,
    accountUuid: '',
    name: '',
    controllerId: null,
    controllerName: null,
    renderDistance: 8,
    memoryGb: 4,
  },
  {
    playerIndex: 1,
    accountUuid: '',
    name: '',
    controllerId: null,
    controllerName: null,
    renderDistance: 6,
    memoryGb: 2,
  },
  {
    playerIndex: 2,
    accountUuid: '',
    name: '',
    controllerId: null,
    controllerName: null,
    renderDistance: 6,
    memoryGb: 2,
  },
  {
    playerIndex: 3,
    accountUuid: '',
    name: '',
    controllerId: null,
    controllerName: null,
    renderDistance: 6,
    memoryGb: 2,
  },
]);

const activePlayersList = computed(() => {
  return players.value.slice(0, playerCount.value);
});

const totalRamRequested = computed(() => {
  return activePlayersList.value.reduce((sum, p) => sum + (p.memoryGb || 0), 0);
});

const canLaunch = computed(() => {
  if (availableAccounts.value.length < playerCount.value) return false;
  const usedUuids = new Set();
  for (const p of activePlayersList.value) {
    if (!p.accountUuid) return false;
    if (usedUuids.has(p.accountUuid)) return false;
    usedUuids.add(p.accountUuid);
  }
  return true;
});

// Dynamic theme color shade helpers for P1, P2, P3, P4
function getPlayerBadgeClass(idx) {
  switch (idx) {
    case 0:
      return 'bg-gradient-to-br from-accent-bright to-accent text-accent-ink border border-accent-bright shadow-[0_0_12px_var(--color-accent-glow)]';
    case 1:
      return 'bg-accent/30 text-accent-bright border border-accent/50 shadow-[0_0_8px_var(--color-accent-glow)]';
    case 2:
      return 'bg-accent/20 text-accent border border-accent/35';
    default:
      return 'bg-accent/12 text-accent-deep border border-accent/25';
  }
}

function getPlayerCardBorderClass(idx, isDup) {
  if (isDup) return 'border-red-500/60 bg-red-500/10 shadow-[0_0_14px_rgba(239,68,68,0.2)]';
  switch (idx) {
    case 0:
      return 'border-accent/40 hover:border-accent/60 shadow-[0_0_16px_var(--color-accent-glow)]/40';
    case 1:
      return 'border-accent/30 hover:border-accent/50 shadow-[0_0_12px_var(--color-accent-glow)]/25';
    case 2:
      return 'border-accent/20 hover:border-accent/40 shadow-sm';
    default:
      return 'border-accent/15 hover:border-accent/30 shadow-sm';
  }
}

function isDuplicateAccount(uuid, currentIdx) {
  if (!uuid) return false;
  return activePlayersList.value.some((p, idx) => idx !== currentIdx && p.accountUuid === uuid);
}

function isAccountUsedByOther(uuid, currentIdx) {
  return activePlayersList.value.some((p, idx) => idx !== currentIdx && p.accountUuid === uuid);
}

function onAccountSelected(player, idx) {
  const acc = availableAccounts.value.find((a) => a.uuid === player.accountUuid);
  if (acc) {
    player.name = acc.username;
  }
}

async function loadAccounts() {
  try {
    const list = (await api.listAccounts()) || [];
    availableAccounts.value = list;
    activePlayersList.value.forEach((p, idx) => {
      if (!p.accountUuid && idx < list.length) {
        p.accountUuid = list[idx].uuid;
        p.name = list[idx].username;
      }
    });
  } catch (e) {
    console.error('Failed to load accounts:', e);
  }
}

async function addAccount() {
  loggingIn.value = true;
  window.dispatchEvent(
    new CustomEvent('zircon-status', {
      detail: 'Opening Microsoft login in your browser…',
    })
  );
  try {
    const session = await api.loginMicrosoft();
    if (session) {
      await loadAccounts();
      // Auto-assign newly logged-in account to the first player needing an account
      const targetPlayer = activePlayersList.value.find(
        (p, idx) => !p.accountUuid || isDuplicateAccount(p.accountUuid, idx)
      );
      if (targetPlayer) {
        targetPlayer.accountUuid = session.uuid;
        targetPlayer.name = session.username;
      }
      window.dispatchEvent(
        new CustomEvent('zircon-status', {
          detail: `Signed in as ${session.username}!`,
        })
      );
      window.dispatchEvent(new CustomEvent('accounts-updated'));
    }
  } catch (e) {
    console.error('Login error:', e);
    const msg = String(e).toLowerCase();
    if (msg.includes('does not own minecraft') || (msg.includes('404') && (msg.includes('profile') || msg.includes('minecraft')))) {
      window.dispatchEvent(
        new CustomEvent('zircon-status', {
          detail: 'Account does not own Minecraft Java Edition.',
        })
      );
      api.openBrowserUrl('https://www.minecraft.net/store/minecraft-java-bedrock-edition-pc').catch(() => {});
    } else {
      window.dispatchEvent(
        new CustomEvent('zircon-status', {
          detail: `Login failed: ${e}`,
        })
      );
    }
  } finally {
    loggingIn.value = false;
  }
}

function setPlayerCount(count) {
  playerCount.value = count;
  if (count === 2) {
    if (selectedLayout.value !== 'twoPlayerHorizontal') {
      selectedLayout.value = 'twoPlayerVertical';
    }
  } else if (count === 3) {
    selectedLayout.value = 'threePlayerGrid';
  } else if (count === 4) {
    selectedLayout.value = 'fourPlayerGrid';
  }
  activePlayersList.value.forEach((p, idx) => {
    if (!p.accountUuid) {
      const unused = availableAccounts.value.find(
        (a) => !activePlayersList.value.some((other) => other.accountUuid === a.uuid)
      );
      if (unused) {
        p.accountUuid = unused.uuid;
        p.name = unused.username;
      }
    }
  });
}

function scanGamepads() {
  if (typeof navigator !== 'undefined' && navigator.getGamepads) {
    const raw = navigator.getGamepads();
    const list = [];
    for (let i = 0; i < raw.length; i++) {
      if (raw[i]) {
        list.push({
          index: raw[i].index,
          id: raw[i].id,
        });
      }
    }
    detectedGamepads.value = list;
    list.forEach((gp, idx) => {
      if (idx < players.value.length && !players.value[idx].controllerId) {
        players.value[idx].controllerId = gp.id;
        players.value[idx].controllerName = gp.id;
      }
    });
  }
}

async function checkStatus() {
  try {
    const status = await api.getCouchCoopStatus();
    if (status) {
      sessionStatus.value = status;
    }
  } catch (e) {
    console.debug('Failed to check split-screen status:', e);
  }
}

async function launchSession() {
  if (!props.instance || launching.value || !canLaunch.value) return;
  launching.value = true;
  try {
    const activePlayers = activePlayersList.value.map((p, idx) => ({
      playerIndex: idx,
      accountUuid: p.accountUuid,
      name: p.name || `Player_${idx + 1}`,
      controllerId: p.controllerId || null,
      controllerName: p.controllerName || null,
      renderDistance: p.renderDistance || 8,
      memoryGb: p.memoryGb || (idx === 0 ? 4 : 2),
    }));

    const req = {
      instanceId: props.instance.id || props.instance.name || 'default',
      serverAddress: props.serverAddress || null,
      layout: selectedLayout.value,
      players: activePlayers,
    };

    const status = await api.launchCouchCoopSession(req);
    sessionStatus.value = status;
    window.dispatchEvent(
      new CustomEvent('zircon-status', {
        detail: `Split-screen session launched with ${activePlayers.length} players!`,
      })
    );
    close();
  } catch (e) {
    window.dispatchEvent(
      new CustomEvent('zircon-status', {
        detail: `Launch error: ${e}`,
      })
    );
  } finally {
    launching.value = false;
  }
}

async function stopSession() {
  if (stopping.value) return;
  stopping.value = true;
  try {
    await api.stopCouchCoopSession();
    sessionStatus.value = {
      isActive: false,
      layout: null,
      playerCount: 0,
      pids: [],
    };
    window.dispatchEvent(
      new CustomEvent('zircon-status', {
        detail: 'Split-screen session stopped.',
      })
    );
  } catch (e) {
    window.dispatchEvent(
      new CustomEvent('zircon-status', {
        detail: `Stop error: ${e}`,
      })
    );
  } finally {
    stopping.value = false;
  }
}

function close() {
  emit('close');
}

let gamepadPollTimer = null;

onMounted(() => {
  scanGamepads();
  loadAccounts();
  checkStatus();
  if (typeof window !== 'undefined') {
    window.addEventListener('gamepadconnected', scanGamepads);
    window.addEventListener('gamepaddisconnected', scanGamepads);
    gamepadPollTimer = setInterval(scanGamepads, 3000);
  }
});

onUnmounted(() => {
  if (typeof window !== 'undefined') {
    window.removeEventListener('gamepadconnected', scanGamepads);
    window.removeEventListener('gamepaddisconnected', scanGamepads);
    if (gamepadPollTimer) clearInterval(gamepadPollTimer);
  }
});

watch(
  () => props.isOpen,
  (open) => {
    if (open) {
      scanGamepads();
      loadAccounts();
      checkStatus();
    }
  }
);
</script>
