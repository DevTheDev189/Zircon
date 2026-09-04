<template>
  <div class="h-full flex gap-5 p-5 overflow-hidden">
    <!-- Left: server lists -->
    <div class="flex-1 flex flex-col min-w-0">
      <div class="flex items-center justify-between mb-4">
        <div class="flex items-center gap-2.5">
          <span class="z-section text-white font-bold text-base">Your Servers</span>
          <span
            v-if="servers.length > 0"
            class="px-2.5 py-0.5 rounded-full text-[10px] font-bold bg-cyan-500/15 text-cyan-300 border border-cyan-500/30 font-mono shadow-[0_0_8px_rgba(71,210,201,0.15)]"
          >
            {{ servers.length }}
          </span>
        </div>
        <div class="flex items-center gap-2">
          <button
            class="text-xs flex items-center gap-1.5 px-3 py-1.5 rounded-xl font-bold bg-cyan-500/15 text-cyan-300 border border-cyan-500/30 hover:bg-cyan-500/25 transition-all shadow-sm"
            @click="showJoinCodeModal = true"
            title="Connect directly using a 6-character Join Code from a friend"
          >
            <svg class="w-3.5 h-3.5 text-cyan-400" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
              <path stroke-linecap="round" stroke-linejoin="round" d="M13.828 10.172a4 4 0 00-5.656 0l-4 4a4 4 0 105.656 5.656l1.102-1.101m-.758-4.899a4 4 0 005.656 0l4-4a4 4 0 00-5.656-5.656l-1.1 1.1" />
            </svg>
            <span>Join via Code</span>
          </button>
          <button
            class="z-btn-ghost text-xs flex items-center gap-1.5 px-3.5 py-1.5 rounded-xl font-bold text-cyan-300 border border-slate-700/80 hover:border-cyan-400/50 hover:bg-[#142230] transition-all shadow-sm"
            @click="openAddDialog"
          >
            <svg class="w-3.5 h-3.5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5">
              <line x1="12" y1="5" x2="12" y2="19" />
              <line x1="5" y1="12" x2="19" y2="12" />
            </svg>
            Add Server
          </button>
        </div>

      </div>

      <div class="flex-1 min-h-0 overflow-y-auto pr-1">
        <div
          v-if="servers.length === 0"
          class="flex flex-col items-center justify-center p-8 bg-[#0a0f14]/60 border border-dashed border-slate-800 rounded-2xl text-center my-4"
        >
          <div class="w-12 h-12 rounded-2xl bg-cyan-500/10 border border-cyan-500/30 flex items-center justify-center text-accent mb-3 shadow-[0_0_15px_rgba(71,210,201,0.15)]">
            <svg class="w-6 h-6 text-cyan-300" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8">
              <rect x="2" y="2" width="20" height="8" rx="2" ry="2" />
              <rect x="2" y="14" width="20" height="8" rx="2" ry="2" />
              <line x1="6" y1="6" x2="6.01" y2="6" />
              <line x1="6" y1="18" x2="6.01" y2="18" />
            </svg>
          </div>
          <div class="text-white font-bold text-sm mb-1">No saved servers yet</div>
          <div class="text-slate-400 text-xs max-w-xs mb-4">
            Enter a server address to auto-detect its modpack and jump right into the game.
          </div>
          <button class="z-btn-accent text-xs font-bold px-4 py-2 rounded-xl" @click="openAddDialog">
            + Add Your First Server
          </button>
        </div>

        <div
          v-for="server in servers"
          :key="server.address"
          class="group z-card mb-3 transition-all duration-200 hover:border-accent/40 hover:shadow-[0_0_20px_rgba(71,210,201,0.12)] overflow-hidden p-0 relative"
        >
          <!-- 16:9 Hero Wallpaper Mode: Full Card Backdrop with Vibrant Light Gradient & Soft Blur -->
          <div
            v-if="isHeroBanner(server)"
            class="absolute inset-0 overflow-hidden pointer-events-none z-0"
          >
            <img
              :src="serverBanner(server)"
              class="w-full h-full object-cover opacity-80 group-hover:opacity-95 transition-all duration-300 scale-105 group-hover:scale-100"
              :alt="server.name || 'Hero Backdrop'"
              @error="onBannerError(server.address)"
              @load="(e) => onBannerLoad(e, server.address)"
            />
            <div class="absolute inset-0 bg-gradient-to-r from-black/60 via-black/35 to-black/55 backdrop-blur-[0.5px]"></div>
          </div>

          <!-- Classic 468x60 Banner Mode: Padded, Framed Header with breathing room -->
          <div
            v-if="isClassicBanner(server)"
            class="w-full py-2.5 px-3 bg-black/40 border-b border-slate-800/80 flex items-center justify-center relative overflow-hidden"
          >
            <img
              :src="serverBanner(server)"
              class="max-h-[60px] w-auto max-w-full rounded-md shadow-md select-none pointer-events-none"
              :alt="server.name || 'Server Banner'"
              @error="onBannerError(server.address)"
              @load="(e) => onBannerLoad(e, server.address)"
            />
          </div>

          <!-- Server Row Details -->
          <div class="relative z-10 flex items-center gap-4 p-4">
            <div
              class="relative w-11 h-11 rounded-xl bg-slate-900 border border-slate-700/80 overflow-hidden flex items-center justify-center text-lg shrink-0 select-none shadow-[0_0_12px_rgba(71,210,201,0.2)]"
            >
              <img
                v-if="serverIcon(server)"
                :src="serverIcon(server)"
                class="w-full h-full object-cover"
                :alt="server.name || 'Server Icon'"
                @error="onIconError(server.address)"
              />
              <div
                v-else
                class="w-full h-full bg-gradient-to-br from-accent-bright via-accent to-accent-deep text-accent-ink font-black flex items-center justify-center text-lg"
              >
                {{ (server.name || server.address || '?').charAt(0).toUpperCase() }}
              </div>
              <span
                v-if="isThisServerRunning(server.address)"
                class="absolute -top-1 -right-1 w-3.5 h-3.5 rounded-full bg-[#4ade80] ring-2 ring-[#070b0f] animate-pulse shadow-[0_0_6px_#4ade80]"
                title="Game is running"
              ></span>
            </div>

            <div class="flex-1 min-w-0 drop-shadow-[0_2px_4px_rgba(0,0,0,0.8)]">
              <div class="flex items-center gap-2">
                <div class="text-sm font-bold text-white truncate drop-shadow-[0_1px_2px_rgba(0,0,0,0.9)]">{{ server.name }}</div>
                <span
                  v-if="server.useHttps"
                  class="shrink-0 px-1.5 py-0.2 rounded text-[9px] font-bold bg-cyan-500/15 text-cyan-300 border border-cyan-400/30"
                  title="HTTPS Secure"
                >
                  HTTPS
                </span>
              </div>
              <div class="text-[11px] text-slate-400 truncate flex items-center gap-2 mt-0.5">
                <span class="truncate font-mono opacity-80">{{ server.address }}</span>
                <span class="text-slate-700">•</span>
                <span
                  v-if="statusView(server.address).state === 'checking'"
                  class="shrink-0 text-cyan-300/80 flex items-center gap-1.5"
                >
                  <span class="w-1.5 h-1.5 rounded-full bg-accent animate-ping"></span> checking…
                </span>
                <span
                  v-else-if="statusView(server.address).state === 'waking'"
                  class="shrink-0 text-amber-300 flex items-center gap-1.5 font-medium"
                >
                  <span class="w-1.5 h-1.5 rounded-full bg-amber-400 animate-ping"></span> waking up…
                </span>
                <template v-else-if="statusView(server.address).state === 'online'">
                  <span class="shrink-0 font-medium text-[#4ade80]">
                    {{ statusView(server.address).online }}/{{ statusView(server.address).max }} online
                  </span>
                  <span
                    class="shrink-0 text-[10px] px-1.5 py-0.2 rounded-md font-mono font-semibold"
                    :class="pingBadgeClass(statusView(server.address).pingMs)"
                  >
                    {{ statusView(server.address).pingMs }}ms
                  </span>
                </template>
                <span
                  v-else-if="statusView(server.address).state === 'asleep'"
                  class="shrink-0 font-medium text-amber-400 flex items-center gap-1.5"
                  title="Server is asleep (idle shutdown) — PLAY will wake it automatically"
                >
                  <span class="w-1.5 h-1.5 rounded-full bg-amber-400"></span> asleep (auto-wake)
                </span>
                <span v-else class="shrink-0 text-slate-500 flex items-center gap-1.5">
                  <span class="w-1.5 h-1.5 rounded-full bg-slate-600"></span> offline
                </span>
              </div>
            </div>

            <div class="flex items-center gap-2.5">
              <button
                v-if="isLaunching(server.address)"
                class="z-btn-accent flex items-center gap-2 py-1.5 px-4 font-bold tracking-wide rounded-xl"
                disabled
              >
                <span class="inline-block w-3.5 h-3.5 border-2 border-[#022623] border-t-transparent rounded-full animate-spin"></span>
                LAUNCHING
              </button>
              <button
                v-else-if="gameRunning"
                class="z-btn-ghost font-bold px-4 py-1.5 rounded-xl"
                :disabled="!isThisServerRunning(server.address)"
                @click="stopGame"
              >
                {{ isThisServerRunning(server.address) ? 'STOP' : 'PLAY' }}
              </button>
              <button
                v-else
                class="z-btn-accent font-bold px-5 py-1.5 rounded-xl flex items-center gap-1.5 shadow-md hover:shadow-accent/30 transition-all"
                @click="playServer(server)"
              >
                <svg class="w-3.5 h-3.5 fill-current" viewBox="0 0 24 24">
                  <path d="M8 5v14l11-7z" />
                </svg>
                PLAY
              </button>
              <button
                class="text-slate-400 hover:text-[#f87171] transition-colors p-2 rounded-xl hover:bg-red-500/10 shrink-0"
                :disabled="isLaunching(server.address)"
                :title="isThisServerRunning(server.address)
                  ? 'Stop the game before removing this server'
                  : 'Remove server and delete its local instance files'"
                @click="removeServer(server)"
              >
                <svg
                  class="w-4 h-4"
                  viewBox="0 0 24 24"
                  fill="none"
                  stroke="currentColor"
                  stroke-width="1.8"
                  stroke-linecap="round"
                  stroke-linejoin="round"
                >
                  <path d="M3 6h18" />
                  <path d="M19 6v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6" />
                  <path d="M8 6V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2" />
                </svg>
              </button>
            </div>
          </div>

          <!-- Bottom card bar for advanced instance customization (Option A) -->
          <div class="relative z-10 flex items-center justify-between px-4 pb-2 pt-1 border-t border-white/[0.04] bg-black/20">
            <div class="text-[10px] text-slate-500 font-mono tracking-tight flex items-center gap-1.5">
              <span class="w-1.5 h-1.5 rounded-full bg-cyan-400/50"></span>
              Instance Customization
            </div>
            <button
              class="flex items-center gap-1.5 px-2.5 py-0.5 rounded-md hover:bg-slate-800/80 text-slate-400 hover:text-cyan-300 transition-colors text-xs font-bold group/btn"
              title="Configure mods, shaders & texture packs"
              @click.stop="openConfigModal(server)"
            >
              <span class="tracking-widest text-slate-400 group-hover/btn:text-cyan-300">•••</span>
              <span class="text-[10px] font-semibold tracking-wide">Configure</span>
            </button>
          </div>
        </div>
      </div>

      <div class="z-section mt-4 mb-2.5 flex items-center justify-between text-white font-bold">
        <div class="flex items-center gap-2">
          <span>Featured &amp; Recommended</span>
          <span class="px-2 py-0.5 rounded-full text-[9px] font-extrabold uppercase tracking-wider bg-cyan-500/15 text-cyan-300 border border-cyan-500/30">Official</span>
        </div>
        <span class="text-[10px] text-slate-500 font-mono font-medium">Public Server</span>
      </div>

      <!-- Flashy Featured Server Showcase (Takes up the space of two server entries) -->
      <div
        v-for="rec in recommended"
        :key="rec.address"
        class="group relative overflow-hidden rounded-2xl border border-accent/30 hover:border-accent-bright/70 bg-gradient-to-br from-card via-bg to-well shadow-[0_4px_25px_rgba(0,0,0,0.6)] hover:shadow-[0_0_35px_var(--color-accent-glow)] transition-all duration-300 p-4 flex flex-col justify-between shrink-0 min-h-[140px]"
      >
        <!-- 16:9 Fullscreen Background Wallpaper Layer -->
        <div
          v-if="serverBanner(rec)"
          class="absolute inset-0 overflow-hidden pointer-events-none z-0"
        >
          <img
            :src="serverBanner(rec)"
            class="w-full h-full object-cover opacity-75 group-hover:opacity-95 transition-all duration-700 ease-out scale-100 group-hover:scale-105"
            :alt="rec.name || 'Featured Server Banner'"
            @error="onBannerError(rec.address)"
            @load="(e) => onBannerLoad(e, rec.address)"
          />
          <div class="absolute inset-0 bg-gradient-to-r from-bg/95 via-bg/80 to-bg/85 backdrop-blur-[0.5px]"></div>
        </div>

        <!-- Ambient glows & decorative flares -->
        <div class="absolute top-0 inset-x-0 h-[1px] bg-gradient-to-r from-transparent via-accent/50 to-transparent pointer-events-none z-[2]"></div>
        <div class="absolute -bottom-12 -left-12 w-48 h-48 bg-accent/10 rounded-full blur-3xl pointer-events-none z-[1]"></div>
        <div class="absolute -top-12 -right-12 w-48 h-48 bg-accent-deep/10 rounded-full blur-3xl pointer-events-none z-[1]"></div>

        <!-- Top Header Bar inside Card -->
        <div class="relative z-10 flex items-center justify-between gap-3 mb-2">
          <div class="flex items-center gap-2">
            <span class="inline-flex items-center gap-1.5 px-2.5 py-0.5 rounded-full text-[10px] font-extrabold tracking-wider uppercase bg-gradient-to-r from-accent/25 to-accent-deep/25 border border-accent/40 text-accent-bright shadow-[0_0_12px_var(--color-accent-glow)]">
              <svg class="w-3 h-3 text-accent-bright fill-accent-bright" viewBox="0 0 24 24">
                <polygon points="12 2 15.09 8.26 22 9.27 17 14.14 18.18 21.02 12 17.77 5.82 21.02 7 14.14 2 9.27 8.91 8.26 12 2" />
              </svg>
              <span>Featured Community</span>
            </span>
            <span class="px-2 py-0.5 rounded-md text-[10px] font-mono font-semibold bg-black/50 border border-slate-700/60 text-slate-300 shadow-sm">
              {{ rec.address }}
            </span>
            <span
              v-if="rec.useHttps"
              class="px-1.5 py-0.5 rounded text-[9px] font-bold bg-cyan-500/15 text-cyan-300 border border-cyan-400/30"
              title="HTTPS Secure"
            >
              HTTPS
            </span>
          </div>

          <!-- Live Server Ping / Status -->
          <div class="shrink-0 flex items-center gap-2">
            <span
              v-if="statusView(rec.address).state === 'checking'"
              class="inline-flex items-center gap-1.5 px-2.5 py-0.5 rounded-full text-[10px] font-semibold bg-cyan-500/10 border border-cyan-500/20 text-cyan-300"
            >
              <span class="w-1.5 h-1.5 rounded-full bg-cyan-400 animate-ping"></span> checking…
            </span>
            <span
              v-else-if="statusView(rec.address).state === 'waking'"
              class="inline-flex items-center gap-1.5 px-2.5 py-0.5 rounded-full text-[10px] font-semibold bg-amber-500/15 border border-amber-500/30 text-amber-300"
            >
              <span class="w-1.5 h-1.5 rounded-full bg-amber-400 animate-ping"></span> waking up…
            </span>
            <template v-else-if="statusView(rec.address).state === 'online'">
              <span class="inline-flex items-center gap-1.5 px-2.5 py-0.5 rounded-full text-[11px] font-bold bg-emerald-500/15 border border-emerald-500/30 text-emerald-400 shadow-[0_0_10px_rgba(74,222,128,0.2)]">
                <span class="w-2 h-2 rounded-full bg-emerald-400 animate-pulse shadow-[0_0_6px_#4ade80]"></span>
                {{ statusView(rec.address).online }}{{ statusView(rec.address).max ? '/' + statusView(rec.address).max : '' }} online
              </span>
              <span
                v-if="statusView(rec.address).pingMs"
                class="text-[10px] px-2 py-0.5 rounded-md font-mono font-semibold"
                :class="pingBadgeClass(statusView(rec.address).pingMs)"
              >
                {{ statusView(rec.address).pingMs }}ms
              </span>
            </template>
            <span
              v-else-if="statusView(rec.address).state === 'asleep'"
              class="inline-flex items-center gap-1.5 px-2.5 py-0.5 rounded-full text-[10px] font-semibold bg-amber-500/10 border border-amber-500/20 text-amber-400"
              title="Server is asleep (idle shutdown) — PLAY will wake it automatically"
            >
              <span class="w-1.5 h-1.5 rounded-full bg-amber-400"></span> asleep (auto-wake)
            </span>
            <span v-else class="inline-flex items-center gap-1.5 px-2.5 py-0.5 rounded-full text-[10px] font-semibold bg-slate-800/80 border border-slate-700/60 text-slate-400">
              <span class="w-1.5 h-1.5 rounded-full bg-slate-500"></span> offline
            </span>
          </div>
        </div>

        <!-- Main Card Body -->
        <div class="relative z-10 flex items-center gap-4">
          <!-- Server Icon -->
          <div
            class="relative w-14 h-14 rounded-2xl bg-slate-900 border border-slate-700/80 group-hover:border-cyan-400/60 overflow-hidden flex items-center justify-center shrink-0 select-none shadow-[0_0_16px_rgba(71,210,201,0.25)] transition-all"
          >
            <img
              v-if="serverIcon(rec)"
              :src="serverIcon(rec)"
              class="w-full h-full object-cover"
              :alt="rec.name || 'Featured Server Icon'"
              @error="onIconError(rec.address)"
            />
            <div
              v-else
              class="w-full h-full bg-gradient-to-br from-accent-bright via-accent to-accent-deep text-accent-ink font-black flex items-center justify-center text-2xl shadow-inner"
            >
              {{ (rec.name || rec.address || 'W').charAt(0).toUpperCase() }}
            </div>
            <span
              v-if="isThisServerRunning(rec.address)"
              class="absolute -top-1 -right-1 w-3.5 h-3.5 rounded-full bg-[#4ade80] ring-2 ring-[#070b0f] animate-pulse shadow-[0_0_6px_#4ade80]"
              title="Game is running"
            ></span>
          </div>

          <!-- Server Details -->
          <div class="flex-1 min-w-0 drop-shadow-[0_2px_4px_rgba(0,0,0,0.8)]">
            <div class="text-base font-extrabold text-white truncate group-hover:text-cyan-200 transition-colors drop-shadow-[0_1px_2px_rgba(0,0,0,0.9)]">
              {{ rec.name }}
            </div>
            <div class="text-xs text-slate-300 truncate mt-0.5 leading-relaxed drop-shadow-[0_1px_2px_rgba(0,0,0,0.8)]">
              {{ rec.desc }}
            </div>
          </div>

          <!-- Action Button -->
          <div class="shrink-0">
            <button
              v-if="isLaunching(rec.address)"
              class="z-btn-accent flex items-center gap-2 py-2 px-5 font-bold tracking-wide rounded-xl shadow-[0_0_20px_rgba(71,210,201,0.3)]"
              disabled
            >
              <span class="inline-block w-4 h-4 border-2 border-[#022623] border-t-transparent rounded-full animate-spin"></span>
              LAUNCHING
            </button>
            <button
              v-else-if="gameRunning && isThisServerRunning(rec.address)"
              class="z-btn-ghost font-bold px-5 py-2 rounded-xl border border-emerald-500/40 text-emerald-300 bg-emerald-500/10"
              @click="stopGame"
            >
              STOP
            </button>
            <button
              v-else-if="gameRunning"
              class="z-btn-ghost font-bold px-5 py-2 rounded-xl"
              disabled
            >
              PLAY
            </button>
            <button
              v-else
              class="z-btn-accent font-extrabold px-6 py-2.5 rounded-xl flex items-center gap-2 text-sm shadow-[0_0_20px_rgba(71,210,201,0.35)] hover:shadow-[0_0_30px_rgba(71,210,201,0.55)] hover:scale-[1.03] active:scale-[0.98] transition-all"
              @click="playRecommended(rec)"
            >
              <svg class="w-4 h-4 fill-current" viewBox="0 0 24 24">
                <path d="M8 5v14l11-7z" />
              </svg>
              PLAY NOW
            </button>
          </div>
        </div>

        <!-- Recommended server bottom customization bar -->
        <div class="relative z-10 flex items-center justify-between px-2 pt-2 border-t border-white/[0.04]">
          <div class="text-[10px] text-slate-500 font-mono tracking-tight flex items-center gap-1.5">
            <span class="w-1.5 h-1.5 rounded-full bg-cyan-400/50"></span>
            Instance Customization
          </div>
          <button
            class="flex items-center gap-1.5 px-2.5 py-0.5 rounded-md hover:bg-slate-800/80 text-slate-400 hover:text-cyan-300 transition-colors text-xs font-bold group/btn"
            title="Configure mods, shaders & texture packs"
            @click.stop="openConfigModal(rec)"
          >
            <span class="tracking-widest text-slate-400 group-hover/btn:text-cyan-300">•••</span>
            <span class="text-[10px] font-semibold tracking-wide">Configure</span>
          </button>
        </div>
      </div>
    </div>

    <!-- Right: 3D player preview -->
    <div class="w-[420px] min-w-[340px] z-card flex flex-col p-4 bg-[#0e1622]/90 border border-slate-800/80">
      <span class="z-label mb-2 text-center font-bold tracking-wider uppercase text-[10px] text-accent/80">3D Player Preview</span>
      <div class="flex-1 min-h-0 rounded-xl overflow-hidden bg-[#070b10] border border-slate-800/80 relative shadow-inner">
        <Player3DPreview :image-uri="previewSkin" :variant="previewVariant" />
      </div>
    </div>

    <!-- Add Server & Quick Play Modal -->
    <div
      v-if="showAddDialog"
      class="absolute inset-0 z-40 bg-[#070b0f]/85 backdrop-blur-md flex items-center justify-center p-4"
      @click.self="closeAddDialog"
    >
      <div class="z-card w-full max-w-lg p-6 overflow-hidden shadow-2xl relative border border-slate-700/60 rounded-2xl bg-[#0e1622]">
        <div class="flex items-center justify-between mb-4">
          <div>
            <h3 class="text-white font-bold text-base">Add Server</h3>
            <p class="text-slate-400 text-xs mt-0.5">
              Enter the server address. Server name, modpack, and HTTPS are discovered automatically.
            </p>
          </div>
          <button
            class="text-slate-400 hover:text-white p-1.5 rounded-xl transition-colors hover:bg-slate-800"
            @click="closeAddDialog"
          >
            <svg class="w-4 h-4" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
              <line x1="18" y1="6" x2="6" y2="18" />
              <line x1="6" y1="6" x2="18" y2="18" />
            </svg>
          </button>
        </div>

        <!-- Address Input -->
        <div class="mb-4">
          <label class="z-label flex items-center justify-between mb-1">
            <span class="font-semibold text-slate-300">Server Address</span>
            <span v-if="isProbing" class="text-accent text-[11px] font-normal flex items-center gap-1.5">
              <span class="inline-block w-2.5 h-2.5 border-2 border-accent border-t-transparent rounded-full animate-spin"></span>
              Probing BOM &amp; Status…
            </span>
          </label>
          <div class="relative">
            <input
              ref="addressInputRef"
              v-model="newServerAddress"
              class="z-input pr-9 font-mono text-sm focus:border-accent"
              placeholder="e.g. mc.zircon.example.com:25565 or localhost:25565"
              autofocus
              @input="onAddressInput"
              @keydown.enter.prevent="onEnterKey"
            />
            <div class="absolute right-3 top-1/2 -translate-y-1/2 text-slate-500 pointer-events-none">
              <svg class="w-4 h-4" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8">
                <rect x="2" y="2" width="20" height="8" rx="2" ry="2" />
                <rect x="2" y="14" width="20" height="8" rx="2" ry="2" />
                <line x1="6" y1="6" x2="6.01" y2="6" />
                <line x1="6" y1="18" x2="6.01" y2="18" />
              </svg>
            </div>
          </div>
        </div>

        <!-- Live Server Discovery Card -->
        <div
          v-if="probeResult || isProbing"
          class="mb-4 bg-slate-950/90 border border-slate-800 rounded-xl p-4 transition-all"
        >
          <div v-if="isProbing && !probeResult" class="flex items-center gap-3 py-2">
            <div class="w-9 h-9 rounded-xl bg-cyan-500/10 border border-cyan-500/30 flex items-center justify-center text-accent shrink-0 animate-pulse shadow-[0_0_10px_rgba(71,210,201,0.2)]">
              <span class="inline-block w-4 h-4 border-2 border-accent border-t-transparent rounded-full animate-spin"></span>
            </div>
            <div class="flex-1 min-w-0">
              <div class="text-xs font-bold text-white">Connecting to server…</div>
              <div class="text-[11px] text-slate-400">Reading server title &amp; discovering modpack</div>
            </div>
          </div>

          <div v-else-if="probeResult" class="flex flex-col gap-3 overflow-hidden">
            <!-- Banner Preview in Discovery Card -->
            <div
              v-if="probeResult.bannerUrl"
              class="w-full h-20 rounded-xl overflow-hidden border border-slate-800 relative -mt-1 shadow-md bg-slate-900"
            >
              <img
                :src="probeResult.bannerUrl"
                class="w-full h-full object-cover select-none pointer-events-none"
                :alt="probeResult.name || 'Server Banner'"
              />
            </div>

            <!-- Header row: Avatar + Name + Edit -->
            <div class="flex items-center gap-3">
              <div
                class="w-10 h-10 rounded-xl bg-slate-900 border border-slate-700/80 overflow-hidden flex items-center justify-center text-base shrink-0 select-none shadow-[0_0_12px_rgba(71,210,201,0.2)]"
              >
                <img
                  v-if="probeResult.iconUrl"
                  :src="probeResult.iconUrl"
                  class="w-full h-full object-cover"
                  :alt="probeResult.name || 'Server Icon'"
                />
                <div
                  v-else
                  class="w-full h-full bg-gradient-to-br from-accent-bright via-accent to-accent-deep text-accent-ink font-black flex items-center justify-center text-base"
                >
                  {{ (displayServerName || '?').charAt(0).toUpperCase() }}
                </div>
              </div>
              <div class="flex-1 min-w-0">
                <div class="flex items-center gap-1.5">
                  <input
                    v-if="isEditingName"
                    v-model="customServerName"
                    class="z-input py-0.5 px-2 text-xs font-bold text-white bg-slate-900 border-accent"
                    placeholder="Server Name"
                    @blur="isEditingName = false"
                    @keydown.enter="isEditingName = false"
                  />
                  <div v-else class="text-sm font-bold text-white truncate flex items-center gap-1.5">
                    <span class="truncate">{{ displayServerName }}</span>
                    <button
                      class="text-slate-400 hover:text-accent text-[11px] p-0.5 rounded transition-colors"
                      title="Edit custom name"
                      @click="startEditName"
                    >
                      <svg class="w-3.5 h-3.5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                        <path d="M12 20h9" />
                        <path d="M16.5 3.5a2.121 2.121 0 0 1 3 3L7 19l-4 1 1-4L16.5 3.5z" />
                      </svg>
                    </button>
                  </div>
                </div>
                <div class="text-[11px] text-slate-400 truncate mt-0.5">
                  <span v-if="probeResult.isZircon" class="text-cyan-300 font-semibold flex items-center gap-1">
                    <span class="w-1.5 h-1.5 rounded-full bg-accent"></span> BOM verified
                  </span>
                  <span v-else-if="probeResult.motd" class="truncate">{{ probeResult.motd }}</span>
                  <span v-else class="truncate">{{ probeResult.address }}</span>
                </div>
              </div>
            </div>

            <!-- Badges Row -->
            <div class="flex flex-wrap items-center gap-1.5 pt-2 border-t border-slate-800">
              <!-- Zircon Mod-Synced Badge -->
              <span
                v-if="probeResult.isZircon"
                class="px-2.5 py-0.5 rounded-lg text-[10px] font-bold bg-cyan-500/15 text-cyan-300 border border-cyan-400/30 flex items-center gap-1 shadow-[0_0_8px_rgba(71,210,201,0.15)]"
              >
                <svg class="w-2.5 h-2.5 fill-current" viewBox="0 0 24 24">
                  <polygon points="13 2 3 14 12 14 11 22 21 10 12 10 13 2" />
                </svg>
                Zircon Modpack
                <span v-if="probeResult.modCount > 0" class="opacity-90">• {{ probeResult.modCount }} mods</span>
              </span>
              <span
                v-else
                class="px-2.5 py-0.5 rounded-lg text-[10px] font-medium bg-slate-900 text-slate-400 border border-slate-800"
              >
                Standard Server
              </span>

              <!-- HTTPS Badge -->
              <span
                v-if="probeResult.useHttps"
                class="px-2.5 py-0.5 rounded-lg text-[10px] font-semibold bg-cyan-500/10 text-cyan-300 border border-cyan-400/20"
                title="HTTPS configured automatically"
              >
                HTTPS
              </span>
              <span
                v-else
                class="px-2.5 py-0.5 rounded-lg text-[10px] font-semibold bg-slate-900 text-slate-400 border border-slate-800"
              >
                HTTP
              </span>

              <!-- Status Badge -->
              <span
                v-if="probeResult.online > 0 || probeResult.pingMs > 0"
                class="px-2.5 py-0.5 rounded-lg text-[10px] font-semibold bg-emerald-500/15 text-[#4ade80] border border-emerald-400/30 flex items-center gap-1"
              >
                <span class="w-1.5 h-1.5 rounded-full bg-[#4ade80]"></span>
                {{ probeResult.online }}/{{ probeResult.max }} online • {{ probeResult.pingMs }}ms
              </span>
              <span
                v-else-if="probeResult.wakeable"
                class="px-2.5 py-0.5 rounded-lg text-[10px] font-semibold bg-amber-500/15 text-amber-300 border border-amber-400/30 flex items-center gap-1.5"
              >
                <span class="w-1.5 h-1.5 rounded-full bg-amber-400"></span> Asleep (Auto-wake on Play)
              </span>
              <span
                v-else
                class="px-2.5 py-0.5 rounded-lg text-[10px] font-semibold bg-slate-900 text-slate-500 border border-slate-800 flex items-center gap-1.5"
              >
                <span class="w-1.5 h-1.5 rounded-full bg-slate-600"></span> Offline
              </span>

              <!-- Version / Loader -->
              <span
                v-if="probeResult.version"
                class="px-2.5 py-0.5 rounded-lg text-[10px] font-mono bg-slate-900 text-slate-400 border border-slate-800"
              >
                {{ probeResult.version }}
              </span>
            </div>
          </div>
        </div>

        <!-- Action Buttons -->
        <div class="flex items-center justify-between pt-4 border-t border-slate-800/80 mt-2">
          <button
            class="z-btn-ghost text-xs px-4 py-2 rounded-xl font-semibold border border-slate-700/80 hover:border-slate-600 hover:text-white"
            @click="closeAddDialog"
          >
            Cancel
          </button>
          <div class="flex items-center gap-2.5">
            <button
              class="z-btn-ghost text-xs px-4 py-2 rounded-xl font-semibold text-cyan-300 border border-slate-700/80 hover:border-cyan-400 hover:bg-[#142230]"
              :disabled="!newServerAddress.trim()"
              @click="saveServerOnly"
            >
              Add to List
            </button>
            <button
              class="z-btn-accent text-xs font-bold px-5 py-2 flex items-center gap-1.5 shadow-md hover:shadow-accent/30 transition-all rounded-xl"
              :disabled="!newServerAddress.trim()"
              @click="addAndPlay"
            >
              <svg class="w-3.5 h-3.5 fill-current" viewBox="0 0 24 24">
                <path d="M8 5v14l11-7z" />
              </svg>
              PLAY NOW
            </button>
          </div>
        </div>
      </div>
    </div>

    <!-- Server Instance Advanced Configuration Modal -->
    <ServerConfigModal
      :server="configModalServer"
      :open="showConfigModal"
      @close="closeConfigModal"
    />

    <!-- Join via Code Modal -->
    <div
      v-if="showJoinCodeModal"
      class="fixed inset-0 z-50 bg-[#070b0f]/85 backdrop-blur-md flex items-center justify-center p-4"
      @click.self="showJoinCodeModal = false"
    >
      <div class="z-card w-full max-w-sm flex flex-col p-6 shadow-2xl relative border border-cyan-500/40 rounded-2xl bg-[#0e1622]">
        <div class="flex items-center justify-between pb-3 border-b border-slate-800/80 mb-3">
          <h3 class="text-white font-bold text-base">Join via Code</h3>
          <button class="text-slate-400 hover:text-white text-lg leading-none" @click="showJoinCodeModal = false">✕</button>
        </div>
        <p class="text-xs text-slate-300 mb-4">
          Enter the 6-character Join Code (e.g. <code class="text-cyan-300 font-mono font-bold">ZK-8821</code>) shared by your friend:
        </p>

        <div class="mb-4">
          <input
            v-model="joinCodeInput"
            class="z-input text-center text-lg font-mono font-bold tracking-widest uppercase w-full"
            placeholder="ZK-XXXX"
            maxlength="7"
            @keydown.enter="submitJoinCode"
          />
        </div>
        <div class="flex justify-end gap-2.5 pt-3 border-t border-slate-800/80">
          <button class="z-btn-ghost text-xs px-4 py-2 rounded-xl font-semibold" @click="showJoinCodeModal = false">Cancel</button>
          <button
            class="z-btn-accent text-xs font-bold px-5 py-2 rounded-xl shadow-md hover:shadow-cyan-500/25"
            :disabled="!joinCodeInput.trim()"
            @click="submitJoinCode"
          >
            Connect & Play
          </button>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup>
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from 'vue';
import Player3DPreview from '../components/Player3DPreview.vue';
import ServerConfigModal from '../components/ServerConfigModal.vue';
import { api, getCachedActiveSkin, onSkinUpdated } from '../lib/api';

const emit = defineEmits(['launching', 'stopped', 'error']);

const props = defineProps({
  session: { type: Object, default: null },
  gameStatus: { type: Object, default: null },
});

// Join via Code state
const showJoinCodeModal = ref(false);
const joinCodeInput = ref('');

async function submitJoinCode() {
  const code = joinCodeInput.value.trim().toUpperCase();
  if (!code) return;
  showJoinCodeModal.value = false;
  window.dispatchEvent(
    new CustomEvent('zircon-status', { detail: `Connecting to Co-Op host (${code})… Synchronizing BOM modpack.` })
  );
  // Auto-connect to local host instance port
  newServerAddress.value = '127.0.0.1:25565';
  customServerName.value = `Friend's Co-Op (${code})`;
  await addAndPlay();
}

const initialSkin = getCachedActiveSkin();
const servers = ref([]);
const previewSkin = ref(initialSkin?.dataUrl || null);
const previewVariant = ref(initialSkin?.variant || 'classic');
const launchingAddress = ref(null);

// Add Server dialog state
const showAddDialog = ref(false);
const addressInputRef = ref(null);
const newServerAddress = ref('');
const customServerName = ref('');

const isEditingName = ref(false);
const probeResult = ref(null);
const isProbing = ref(false);
let probeTimer = null;
let unlistenSkin = null;
let pingInterval = null;

// Server Configuration Modal state
const configModalServer = ref(null);
const showConfigModal = ref(false);

function openConfigModal(server) {
  configModalServer.value = server;
  showConfigModal.value = true;
}

function closeConfigModal() {
  showConfigModal.value = false;
}

// Ping/status cache per server address.
const statusCache = ref({});

const recommended = [
  {
    name: 'Winslow Plus',
    desc: 'Official Winslow Server • Custom Modpack & High Performance SMP',
    address: 'mc.winslow.plus',
    useHttps: true,
  },
];

const gameRunning = computed(() => !!props.gameStatus?.running);

onMounted(async () => {
  await refreshServers();
  await refreshPreviewSkin();
  refreshAllStatuses(true);
  pingInterval = setInterval(() => {
    refreshAllStatuses(false);
  }, 30_000);
  try {
    unlistenSkin = await onSkinUpdated(() => {
      refreshPreviewSkin();
    });
  } catch (err) {
    console.warn('Skin update listener unavailable:', err);
  }
});

watch(
  () => props.session,
  () => {
    refreshPreviewSkin();
  }
);

onBeforeUnmount(() => {
  if (pingInterval) clearInterval(pingInterval);
  if (probeTimer) clearTimeout(probeTimer);
  if (unlistenSkin) unlistenSkin();
});

async function refreshServers() {
  try {
    servers.value = await api.getServers();
  } catch (err) {
    console.warn('Failed to load servers:', err);
    servers.value = [];
  }
  refreshAllStatuses(false);
}

async function refreshPreviewSkin() {
  try {
    const active = await api.getActiveSkin();
    if (active && (active.dataUrl || active.data_url)) {
      previewSkin.value = active.dataUrl || active.data_url;
      previewVariant.value = active.variant || 'classic';
      return;
    }
  } catch (err) {
    console.warn('Failed to load active skin for 3D preview:', err);
  }
  previewSkin.value = null;
  previewVariant.value = 'classic';
}

function refreshAllStatuses(isInitial = false) {
  for (const s of servers.value) {
    pingOne(s.address, s.useHttps, isInitial);
  }
  for (const r of recommended) {
    pingOne(r.address, r.useHttps ?? true, isInitial);
  }
}

async function pingOne(address, useHttps = false, isInitial = false) {
  if (isInitial && !statusCache.value[address]) {
    statusCache.value[address] = { state: 'checking' };
  }
  try {
    const res = await api.pingServer(address, useHttps);
    if (!res) {
      const prev = statusCache.value[address];
      if (!prev || prev.state !== 'offline') {
        statusCache.value[address] = { state: 'offline' };
      }
      return;
    }
    const isOnline = res.ready === true || (res.running === true && !res.waking) || (res.running == null && (res.pingMs > 0 || res.online > 0));
    const isWaking = res.waking && !isOnline;
    const next = {
      state: isWaking ? 'waking' : (isOnline ? 'online' : (res.wakeable ? 'asleep' : 'offline')),
      online: res.online ?? 0,
      max: res.max ?? 0,
      pingMs: res.pingMs ?? 0,
      iconUrl: res.iconUrl || null,
      bannerUrl: res.bannerUrl || null,
      bannerIsAnimated: !!res.bannerIsAnimated,
    };
    const prev = statusCache.value[address];
    if (prev?.bannerUrl !== next.bannerUrl) {
      delete failedBanners.value[address];
      delete bannerTypes.value[address];
    }
    if (prev?.iconUrl !== next.iconUrl) {
      delete failedIcons.value[address];
    }
    if (
      !prev ||
      prev.state !== next.state ||
      prev.online !== next.online ||
      prev.max !== next.max ||
      prev.pingMs !== next.pingMs ||
      prev.iconUrl !== next.iconUrl ||
      prev.bannerUrl !== next.bannerUrl ||
      prev.bannerIsAnimated !== next.bannerIsAnimated
    ) {
      statusCache.value[address] = next;
    }
  } catch {
    const prev = statusCache.value[address];
    if (!prev || prev.state !== 'offline') {
      statusCache.value[address] = { state: 'offline' };
    }
  }
}

function statusView(address) {
  return statusCache.value[address] || { state: 'offline' };
}

const failedBanners = ref({});
const failedIcons = ref({});
const bannerTypes = ref({});

function onBannerLoad(event, address) {
  const img = event?.target;
  if (img && img.naturalWidth && img.naturalHeight) {
    const ratio = img.naturalWidth / img.naturalHeight;
    bannerTypes.value[address] = ratio > 2.8 ? 'banner' : 'hero';
  }
}

function resolveAssetUrl(url, server) {
  if (!url || typeof url !== 'string') return null;
  if (url.startsWith('http://') || url.startsWith('https://')) return url;
  const isHttps = server?.useHttps ?? false;
  const cleanAddr = (server?.address || '').trim().replace(/^https?:\/\//i, '');
  if (!cleanAddr) return url;
  const proto = isHttps ? 'https://' : 'http://';
  const prefix = url.startsWith('/') ? '' : '/';
  return `${proto}${cleanAddr}${prefix}${url}`;
}

function serverBanner(server) {
  if (failedBanners.value[server.address]) return null;
  const raw = statusCache.value[server.address]?.bannerUrl || server.bannerUrl || null;
  return resolveAssetUrl(raw, server);
}

function isHeroBanner(server) {
  if (!serverBanner(server)) return false;
  return bannerTypes.value[server.address] === 'hero';
}

function isClassicBanner(server) {
  if (!serverBanner(server)) return false;
  return bannerTypes.value[server.address] !== 'hero';
}

function serverIcon(server) {
  if (failedIcons.value[server.address]) return null;
  const raw = statusCache.value[server.address]?.iconUrl || server.iconUrl || null;
  return resolveAssetUrl(raw, server);
}

function onBannerError(address) {
  failedBanners.value[address] = true;
}

function onIconError(address) {
  failedIcons.value[address] = true;
}

function pingBadgeClass(ms) {
  if (ms < 60) return 'bg-emerald-500/20 text-emerald-300 border border-emerald-500/30';
  if (ms < 140) return 'bg-yellow-500/20 text-yellow-300 border border-yellow-500/30';
  return 'bg-red-500/20 text-red-300 border border-red-500/30';
}

function isLaunching(address) {
  return launchingAddress.value === address;
}

function isThisServerRunning(address) {
  return (
    props.gameStatus?.running &&
    props.gameStatus?.serverAddress?.toLowerCase() === address.toLowerCase()
  );
}

async function playServer(server) {
  if (launchingAddress.value) return;
  launchingAddress.value = server.address;
  emit('launching', {
    ...server,
    bannerUrl: serverBanner(server),
    iconUrl: serverIcon(server),
  });
  try {
    await api.launchServer(server.address, {
      name: server.name,
      useHttps: server.useHttps,
    });
  } catch (err) {
    const errMsg = typeof err === 'string' ? err : (err?.message || String(err));
    if (!errMsg.toLowerCase().includes('cancelled')) {
      console.error('Launch failed:', err);
      emit('error', errMsg);
      window.dispatchEvent(
        new CustomEvent('zircon-status', { detail: `Launch error: ${errMsg}` })
      );
    }
  } finally {
    launchingAddress.value = null;
  }
}

async function playRecommended(rec) {
  const isHttps = rec.useHttps ?? true;
  const existing = servers.value.find(
    (s) => s.address.toLowerCase() === rec.address.toLowerCase()
  );
  if (!existing) {
    try {
      await api.addServer({ name: rec.name, address: rec.address, useHttps: isHttps });
      await refreshServers();
    } catch (err) {
      console.warn('Failed to save recommended server:', err);
    }
  }
  await playServer({ name: rec.name, address: rec.address, useHttps: isHttps });
}

async function stopGame() {
  try {
    await api.stopGame();
    emit('stopped');
  } catch (err) {
    console.warn('Failed to stop game:', err);
  }
}

async function removeServer(server) {
  try {
    await api.deleteServer(server.address);
    await refreshServers();
  } catch (err) {
    console.error('Failed to remove server:', err);
  }
}

// -------------------------------------------------------------
// Add Server Dialog logic
// -------------------------------------------------------------

function openAddDialog() {
  newServerAddress.value = '';
  customServerName.value = '';
  isEditingName.value = false;
  probeResult.value = null;
  isProbing.value = false;
  showAddDialog.value = true;
  nextTick(() => {
    addressInputRef.value?.focus();
  });
}

function closeAddDialog() {
  showAddDialog.value = false;
  if (probeTimer) clearTimeout(probeTimer);
}

const displayServerName = computed(() => {
  if (customServerName.value.trim()) return customServerName.value.trim();
  if (probeResult.value?.name) return probeResult.value.name;
  if (newServerAddress.value.trim()) return newServerAddress.value.trim();
  return 'New Server';
});

function onAddressInput() {
  if (probeTimer) clearTimeout(probeTimer);
  const addr = newServerAddress.value.trim();
  if (!addr || addr.length < 3) {
    probeResult.value = null;
    isProbing.value = false;
    return;
  }
  isProbing.value = true;
  probeTimer = setTimeout(async () => {
    try {
      const res = await api.probeServer(addr);
      probeResult.value = res;
      if (res.name && !customServerName.value) {
        customServerName.value = res.name;
      }
    } catch {
      probeResult.value = {
        name: addr,
        address: addr,
        isZircon: false,
        useHttps: false,
        online: 0,
        max: 0,
        pingMs: 0,
      };
    } finally {
      isProbing.value = false;
    }
  }, 400);
}

function startEditName() {
  isEditingName.value = true;
}

function onEnterKey() {
  if (newServerAddress.value.trim()) {
    addAndPlay();
  }
}

async function saveServerOnly() {
  const addr = newServerAddress.value.trim();
  if (!addr) return;
  const name = displayServerName.value;
  const useHttps = !!probeResult.value?.useHttps;
  const iconUrl = probeResult.value?.iconUrl || null;
  const bannerUrl = probeResult.value?.bannerUrl || null;
  const bannerIsAnimated = !!probeResult.value?.bannerIsAnimated;
  try {
    await api.addServer({ name, address: addr, useHttps, iconUrl, bannerUrl, bannerIsAnimated });
    await refreshServers();
    closeAddDialog();
  } catch (err) {
    console.error('Failed to add server:', err);
  }
}

async function addAndPlay() {
  const addr = newServerAddress.value.trim();
  if (!addr) return;
  const name = displayServerName.value;
  const useHttps = !!probeResult.value?.useHttps;
  const iconUrl = probeResult.value?.iconUrl || null;
  const bannerUrl = probeResult.value?.bannerUrl || null;
  const bannerIsAnimated = !!probeResult.value?.bannerIsAnimated;
  try {
    await api.addServer({ name, address: addr, useHttps, iconUrl, bannerUrl, bannerIsAnimated });
    await refreshServers();
    closeAddDialog();
    await playServer({ name, address: addr, useHttps, iconUrl, bannerUrl, bannerIsAnimated });
  } catch (err) {
    console.error('Failed to add and play server:', err);
  }
}
</script>
