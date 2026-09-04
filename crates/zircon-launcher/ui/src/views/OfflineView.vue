<template>
  <div class="h-full flex gap-5 p-5 overflow-hidden">
    <!-- Left: instance list -->
    <div class="w-[300px] min-w-[300px] z-card flex flex-col p-4 bg-[#0e1622]/90 border border-slate-800/80">
      <div class="flex items-center justify-between mb-4">
        <div class="flex items-center gap-2">
          <span class="z-section text-white font-bold">Instances</span>
          <span
            v-if="instances.length > 0"
            class="px-2 py-0.5 rounded-full text-[10px] font-bold bg-cyan-500/15 text-cyan-300 border border-cyan-500/30 font-mono shadow-[0_0_8px_rgba(71,210,201,0.15)]"
          >
            {{ instances.length }}
          </span>
        </div>
        <div class="flex items-center gap-1.5">
          <button
            class="z-btn-ghost text-[11px] font-bold px-2 py-1 rounded-lg border border-slate-700/80 text-cyan-400 hover:text-cyan-300 hover:border-cyan-400/50 flex items-center gap-1"
            title="Join a friend's world via 6-character code"
            @click="showJoinCodeModal = true"
          >
            Join Code
          </button>
          <button class="z-btn-accent text-xs font-bold px-3 py-1.5 rounded-xl shadow-md hover:shadow-cyan-500/25" @click="openNewInstance">+ New</button>
        </div>
      </div>
      <div class="flex-1 min-h-0 overflow-y-auto pr-1">
        <div
          v-for="instance in instances"
          :key="instance.id"
          class="flex items-center gap-3.5 border rounded-xl p-3.5 mb-2.5 cursor-pointer transition-all duration-200"
          :class="
            selected?.id === instance.id
              ? 'border-cyan-400 ring-1 ring-cyan-400/60 shadow-[0_0_16px_rgba(71,210,201,0.25)] bg-[#111c29]'
              : 'border-slate-800/80 bg-[#070b10]/60 hover:border-slate-700 hover:bg-[#121d2b]'
          "
          @click="selectInstance(instance)"
        >
          <div
            class="w-10 h-10 rounded-xl bg-gradient-to-br from-accent-bright via-accent to-accent-deep text-accent-ink font-black flex items-center justify-center text-base shrink-0 shadow-[0_0_10px_var(--color-accent-glow)]"
          >
            {{ instance.name.charAt(0).toUpperCase() }}
          </div>
          <div class="flex-1 min-w-0">
            <div class="text-[13px] font-bold text-white truncate">{{ instance.name }}</div>
            <div class="text-[11px] text-slate-400 font-mono truncate mt-0.5">
              MC {{ instance.minecraftVersion }} · <span class="capitalize">{{ instance.modLoader.type }}</span>
            </div>
          </div>
        </div>
        <div v-if="instances.length === 0" class="text-slate-500 text-xs py-8 text-center">
          No instances created yet. Click + New Instance to get started!
        </div>
      </div>
    </div>

    <!-- Right: instance detail & management -->
    <div class="flex-1 min-w-0 z-card flex flex-col p-5 bg-[#0e1622]/90 border border-slate-800/80">
      <template v-if="selected">
        <!-- Header summary & play action -->
        <div class="bg-[#070b10] border border-slate-800/90 rounded-xl p-4 shadow-inner mb-4">
          <!-- Tier 1: Identity & Hero Play Action -->
          <div class="flex items-center justify-between gap-4">
            <div class="min-w-0 flex-1">
              <div class="text-white font-bold text-lg tracking-wide truncate mb-1.5">{{ selected.name }}</div>
              <div class="flex items-center gap-2 text-xs flex-wrap font-mono">
                <span class="bg-slate-900 border border-slate-700/80 text-slate-200 px-2.5 py-0.5 rounded-lg text-[11px]">
                  MC <strong class="text-white font-bold">{{ selected.minecraftVersion }}</strong>
                </span>
                <span class="bg-cyan-500/10 border border-cyan-500/30 text-cyan-300 px-2.5 py-0.5 rounded-lg text-[11px] capitalize">
                  {{ selected.modLoader.type }} {{ selected.modLoader.version }}
                </span>
                <span class="text-slate-400 text-[11px] ml-1">
                  {{ mods.length }} mod{{ mods.length === 1 ? '' : 's' }}
                </span>
                <span v-if="selected.lastPlayed" class="text-slate-500 text-[11px]">
                  · {{ formatRelativeTime(selected.lastPlayed) }}
                </span>
              </div>
            </div>

            <!-- Primary Play Button -->
            <button
              class="z-btn-accent px-6 py-2.5 rounded-xl font-bold shadow-lg hover:shadow-cyan-500/25 flex items-center gap-2 text-sm tracking-wide shrink-0 transition-transform active:scale-95"
              :disabled="launching"
              @click="playOffline"
            >
              <span v-if="launching" class="inline-flex items-center gap-2">
                <span class="inline-block w-3.5 h-3.5 border-2 border-[#022623] border-t-transparent rounded-full animate-spin"></span>
                LAUNCHING…
              </span>
              <span v-else class="flex items-center gap-2">
                <svg class="w-4 h-4 fill-current" viewBox="0 0 24 24"><path d="M8 5v14l11-7z" /></svg>
                Play
              </span>
            </button>
          </div>


          <!-- Tier 2: Action Toolbar -->
          <div class="pt-3 mt-3 border-t border-slate-800/80 flex items-center justify-between gap-3">
            <div class="flex items-center gap-2 flex-wrap">
              <!-- Flush Open Folder split-button dropdown -->
              <div class="relative inline-flex items-stretch rounded-xl overflow-hidden border border-slate-700/80 shadow-sm h-8 bg-slate-900/80">
                <button
                  class="hover:bg-slate-800 hover:text-white text-xs px-3 font-semibold text-slate-200 flex items-center gap-1.5 transition-colors"
                  @click="openFolder(null)"
                  title="Open Instance Folder (.minecraft)"
                >
                  <svg class="w-3.5 h-3.5 text-cyan-400 shrink-0" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                    <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M3 7v10a2 2 0 002 2h14a2 2 0 002-2V9a2 2 0 00-2-2h-6l-2-2H5a2 2 0 00-2 2z" />
                  </svg>
                  <span>Folder</span>
                </button>
                <button
                  class="hover:bg-slate-800 hover:text-white text-xs px-2 border-l border-slate-700/80 text-slate-400 hover:text-white flex items-center justify-center transition-colors"
                  @click="showOpenFolderDropdown = !showOpenFolderDropdown"
                  title="Choose Subfolder"
                >
                  <svg class="w-3 h-3 transition-transform" :class="{ 'rotate-180': showOpenFolderDropdown }" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                    <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M19 9l-7 7-7-7" />
                  </svg>
                </button>

                <!-- Dropdown Menu -->
                <div
                  v-if="showOpenFolderDropdown"
                  class="absolute left-0 top-9 w-52 bg-[#0c141f] border border-slate-700 rounded-xl shadow-2xl py-1.5 z-50 text-xs text-slate-200"
                >
                  <button class="w-full text-left px-3.5 py-1.5 hover:bg-slate-800/80 flex items-center justify-between" @click="openFolder(null)">
                    <span>Instance Root</span>
                    <span class="text-[10px] text-slate-500 font-mono">.minecraft</span>
                  </button>
                  <button class="w-full text-left px-3.5 py-1.5 hover:bg-slate-800/80 flex items-center justify-between" @click="openFolder('mods')">
                    <span>Mods</span>
                    <span class="text-[10px] text-slate-500 font-mono">mods/</span>
                  </button>
                  <button class="w-full text-left px-3.5 py-1.5 hover:bg-slate-800/80 flex items-center justify-between" @click="openFolder('config')">
                    <span>Config</span>
                    <span class="text-[10px] text-slate-500 font-mono">config/</span>
                  </button>
                  <button class="w-full text-left px-3.5 py-1.5 hover:bg-slate-800/80 flex items-center justify-between" @click="openFolder('saves')">
                    <span>Saves (Worlds)</span>
                    <span class="text-[10px] text-slate-500 font-mono">saves/</span>
                  </button>
                  <button class="w-full text-left px-3.5 py-1.5 hover:bg-slate-800/80 flex items-center justify-between" @click="openFolder('screenshots')">
                    <span>Screenshots</span>
                    <span class="text-[10px] text-slate-500 font-mono">screenshots/</span>
                  </button>
                  <button class="w-full text-left px-3.5 py-1.5 hover:bg-slate-800/80 flex items-center justify-between" @click="openFolder('shaderpacks')">
                    <span>Shaders</span>
                    <span class="text-[10px] text-slate-500 font-mono">shaderpacks/</span>
                  </button>
                  <button class="w-full text-left px-3.5 py-1.5 hover:bg-slate-800/80 flex items-center justify-between" @click="openFolder('resourcepacks')">
                    <span>Resource Packs</span>
                    <span class="text-[10px] text-slate-500 font-mono">resourcepacks/</span>
                  </button>
                  <button class="w-full text-left px-3.5 py-1.5 hover:bg-slate-800/80 flex items-center justify-between" @click="openFolder('logs')">
                    <span>Logs</span>
                    <span class="text-[10px] text-slate-500 font-mono">logs/</span>
                  </button>
                </div>
              </div>

              <!-- Host for Friends Co-Op (On-Brand Cyan) -->
              <button
                class="text-xs px-3 h-8 rounded-xl font-semibold bg-cyan-500/15 hover:bg-cyan-500/25 text-cyan-300 border border-cyan-500/30 shadow-sm flex items-center gap-1.5 transition-colors"
                @click="openCoopModal"
                title="Host this instance for friends with a zero-config Join Code"
              >
                <svg class="w-3.5 h-3.5 text-cyan-400" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                  <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 4.354a4 4 0 110 5.292M15 21H3v-1a6 6 0 0112 0v1zm0 0h6v-1a6 6 0 00-9-5.197M13 7a4 4 0 11-8 0 4 4 0 018 0z" />
                </svg>
                <span>Host for Friends</span>
              </button>

              <!-- Actions Dropdown -->
              <div class="relative inline-flex">
                <button
                  class="z-btn-ghost text-xs px-3 h-8 rounded-xl font-semibold border border-slate-700/80 hover:border-slate-600 hover:text-white flex items-center gap-1.5 transition"
                  @click="showActionsDropdown = !showActionsDropdown"
                >
                  <svg class="w-3.5 h-3.5 text-slate-400" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                    <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 5v.01M12 12v.01M12 19v.01M12 6a1 1 0 110-2 1 1 0 010 2zm0 7a1 1 0 110-2 1 1 0 010 2zm0 7a1 1 0 110-2 1 1 0 010 2z" />
                  </svg>
                  <span>Actions</span>
                  <svg class="w-3 h-3 transition-transform" :class="{ 'rotate-180': showActionsDropdown }" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                    <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M19 9l-7 7-7-7" />
                  </svg>
                </button>
                <div
                  v-if="showActionsDropdown"
                  class="absolute left-0 top-9 w-52 bg-[#0c141f] border border-slate-700 rounded-xl shadow-2xl py-1.5 z-50 text-xs text-slate-200"
                >
                  <button class="w-full text-left px-3.5 py-1.5 hover:bg-slate-800/80 flex items-center gap-2" @click="promptCloneInstance">
                    <span>Clone Instance</span>
                  </button>
                  <button class="w-full text-left px-3.5 py-1.5 hover:bg-slate-800/80 flex items-center gap-2" @click="exportMrpack">
                    <span>Export .mrpack</span>
                  </button>
                  <button class="w-full text-left px-3.5 py-1.5 hover:bg-slate-800/80 flex items-center gap-2" @click="exportDedicatedServer">
                    <span>Export to Server ZIP</span>
                  </button>
                  <button class="w-full text-left px-3.5 py-1.5 hover:bg-slate-800/80 flex items-center gap-2 text-cyan-300 hover:text-cyan-200" @click="openCoopModal">
                    <svg class="w-3.5 h-3.5 text-cyan-400 shrink-0" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                      <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M17 20h5v-2a3 3 0 00-5.356-1.857M17 20H7m10 0v-2c0-.656-.126-1.283-.356-1.857M7 20H2v-2a3 3 0 015.356-1.857M7 20v-2c0-.656.126-1.283.356-1.857m0 0a5.002 5.002 0 019.288 0M15 7a3 3 0 11-6 0 3 3 0 016 0zm6 3a2 2 0 11-4 0 2 2 0 014 0zM7 10a2 2 0 11-4 0 2 2 0 014 0z" />
                    </svg>
                    <span>Host for Friends Co-Op</span>
                  </button>
                </div>
              </div>
            </div>

            <!-- Danger Delete Button -->
            <button
              class="text-xs px-3 h-8 rounded-xl font-semibold text-red-400 hover:text-red-300 hover:bg-red-500/10 border border-transparent hover:border-red-500/30 transition-all flex items-center gap-1.5 shrink-0"
              @click="deleteInstance"
              title="Delete this offline instance"
            >
              <svg class="w-3.5 h-3.5" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M19 7l-.867 12.142A2 2 0 0116.138 21H7.862a2 2 0 01-1.995-1.858L5 7m5 4v6m4-6v6m1-10V4a1 1 0 00-1-1h-4a1 1 0 00-1 1v3M4 7h16" />
              </svg>
              <span>Delete</span>
            </button>
          </div>

          <!-- Segmented tab selector matching web-app style -->
          <div class="mt-4 pt-3 border-t border-slate-800/80 flex items-center">
            <div class="z-segmented-track">
              <button
                type="button"
                class="z-segmented-pill"
                :class="{ 'active': activeTab === 'mods' }"
                @click="activeTab = 'mods'"
              >
                Mods ({{ mods.length }})
              </button>
              <button
                type="button"
                class="z-segmented-pill"
                :class="{ 'active': activeTab === 'shaders' }"
                @click="activeTab = 'shaders'"
              >
                Shaders ({{ detailedPacks.shaderpacks.length }})
              </button>
              <button
                type="button"
                class="z-segmented-pill"
                :class="{ 'active': activeTab === 'textures' }"
                @click="activeTab = 'textures'"
              >
                Texture Packs ({{ detailedPacks.resourcepacks.length }})
              </button>
              <button
                type="button"
                class="z-segmented-pill"
                :class="{ 'active': activeTab === 'worlds' }"
                @click="activeTab = 'worlds'; loadWorlds();"
              >
                Worlds & Saves ({{ worldsList.length }})
              </button>
              <button
                type="button"
                class="z-segmented-pill"
                :class="{ 'active': activeTab === 'screenshots' }"
                @click="activeTab = 'screenshots'; loadScreenshots();"
              >
                Screenshots ({{ screenshotsList.length }})
              </button>
            </div>
          </div>
        </div>


        <!-- Tab contents (scrollable body) -->
        <div class="flex-1 min-h-0 overflow-y-auto pr-1 flex flex-col gap-4">
          <!-- ================= TAB: MODS ================= -->
          <template v-if="activeTab === 'mods'">
            <div class="bg-[#070b10] border border-slate-800/90 rounded-xl p-4 shadow-inner">
              <div class="flex items-center justify-between mb-3">
                <div class="flex items-center gap-2.5">
                  <div class="z-section text-white font-bold text-sm">Installed Mods ({{ mods.length }})</div>
                  <button
                    class="z-btn-ghost text-xs px-2.5 py-1 rounded-lg border border-slate-700 hover:border-slate-600 flex items-center gap-1.5 transition"
                    :disabled="checkingUpdates || updatingMods"
                    @click="checkModUpdates"
                    title="Check Modrinth for updates to installed mods"
                  >
                    <span v-if="checkingUpdates" class="w-2.5 h-2.5 border-2 border-cyan-400 border-t-transparent rounded-full animate-spin"></span>
                    <span v-else class="text-cyan-400">⟳</span>
                    <span>{{ checkingUpdates ? 'Checking…' : 'Check Updates' }}</span>
                  </button>

                  <span
                    v-if="availableUpdates.length > 0"
                    class="px-2 py-0.5 rounded-full text-[11px] font-bold bg-amber-500/20 text-amber-300 border border-amber-500/30"
                  >
                    {{ availableUpdates.length }} update{{ availableUpdates.length > 1 ? 's' : '' }} available
                  </span>
                  <button
                    v-if="availableUpdates.length > 0"
                    class="z-btn text-xs px-3 py-1 rounded-lg font-bold bg-amber-500/20 hover:bg-amber-500/30 text-amber-200 border border-amber-500/40 shadow-sm flex items-center gap-1.5 transition"
                    :disabled="updatingMods"
                    @click="applyModUpdates(availableUpdates)"
                    title="Update all mods with automatic rollback backup"
                  >
                    <span v-if="updatingMods" class="w-2.5 h-2.5 border-2 border-amber-400 border-t-transparent rounded-full animate-spin"></span>
                    <span>{{ updatingMods ? 'Updating…' : 'Update All' }}</span>
                  </button>
                </div>

                <label
                  v-if="mods.length > 0"
                  class="flex items-center gap-2 text-xs text-slate-400 cursor-pointer select-none"
                >
                  <input
                    type="checkbox"
                    class="zircon-check select-all-check"
                    :checked="Boolean(allModsSelected)"
                    @change="toggleSelectAllMods"
                  />
                  <span>Select All</span>
                </label>
              </div>

              <!-- Bulk selection bar -->
              <div
                v-if="selectedModCount > 0"
                class="flex items-center gap-2 mb-3 bg-slate-900 border border-slate-700/80 rounded-xl px-3 py-2 text-xs"
              >
                <span class="text-slate-400 font-medium">{{ selectedModCount }} selected</span>
                <div class="flex-1"></div>
                <button type="button" class="z-btn-ghost text-[10px] px-2.5 py-1 rounded-lg font-semibold" @click="bulkEnableSelected">Enable</button>
                <button type="button" class="z-btn-ghost text-[10px] px-2.5 py-1 rounded-lg font-semibold" @click="bulkDisableSelected">Disable</button>
                <button type="button" class="text-[10px] px-2.5 py-1 text-red-400 hover:text-red-300 font-semibold" @click="bulkDeleteSelected">Delete</button>
              </div>

              <!-- Mods list -->
              <div
                v-if="mods.length"
                class="max-h-[220px] overflow-y-auto mb-3 flex flex-col gap-1.5 pr-1"
              >
                <div
                  v-for="mod in mods"
                  :key="mod.filename"
                  class="flex items-center gap-2.5 p-2.5 rounded-xl bg-slate-900/60 border border-slate-800/80 text-xs transition hover:border-slate-700"
                  :class="{ 'opacity-50': !mod.enabled }"
                >
                  <input
                    type="checkbox"
                    class="zircon-check shrink-0 item-select-check"
                    :checked="Boolean(selectedMods[mod.filename])"
                    @change="toggleModSelected(mod.filename)"
                  />
                  <div class="w-8 h-8 rounded-lg bg-slate-950 border border-slate-800 flex items-center justify-center shrink-0 overflow-hidden shadow-inner">
                    <img
                      v-if="mod.iconUrl"
                      :src="mod.iconUrl"
                      class="w-full h-full object-cover"
                      loading="lazy"
                    />
                    <svg v-else class="w-4 h-4 text-cyan-400/60" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8">
                      <polygon points="12 2 2 7 12 12 22 7 12 2" />
                      <polyline points="2 17 12 22 22 17" />
                      <polyline points="2 12 12 17 22 12" />
                    </svg>
                  </div>
                  <div class="flex-1 min-w-0">
                    <div class="flex items-center gap-1.5 truncate">
                      <span class="truncate text-white font-medium">{{ mod.filename }}</span>
                      <span
                        v-if="mod.version"
                        class="bg-slate-950 text-slate-400 border border-slate-800 text-[10px] px-1.5 py-0.2 rounded font-mono shrink-0"
                      >{{ mod.version }}</span>
                    </div>
                    <div v-if="mod.author" class="text-[10px] text-slate-500">by {{ mod.author }}</div>
                  </div>
                  <span class="text-slate-500 font-mono text-[11px]">{{ fmtBytes(mod.sizeBytes) }}</span>
                  <button
                    v-if="modUpdateMap[mod.filename]"
                    class="text-[11px] px-2 py-0.5 rounded-md font-semibold bg-amber-500/20 text-amber-300 border border-amber-500/40 hover:bg-amber-500/30 transition flex items-center gap-1 shrink-0"
                    title="Update to latest compatible version"
                    :disabled="updatingMods"
                    @click="applyModUpdates([modUpdateMap[mod.filename]])"
                  >
                    <span>Update → {{ modUpdateMap[mod.filename].latestVersionNumber }}</span>
                  </button>
                  <button
                    type="button"
                    class="z-toggle shrink-0"
                    :class="{ 'z-toggle-on': Boolean(mod.enabled) }"
                    :title="mod.enabled ? 'Deactivate mod' : 'Activate mod'"
                    @click="toggleModEnabled(mod)"
                  >
                    <span class="z-toggle-thumb"></span>
                  </button>
                  <button
                    class="text-red-400 hover:text-red-300 text-xs px-2 py-0.5 hover:bg-red-500/10 rounded-lg transition-colors font-medium flex items-center gap-1 shrink-0"
                    title="Delete"
                    @click="deleteMod(mod.filename)"
                  >
                    <svg class="w-3.5 h-3.5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8">
                      <path d="M3 6h18M19 6v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6M8 6V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2" />
                    </svg>
                    <span>Delete</span>
                  </button>
                </div>
              </div>

              <!-- Custom upload / drop zone -->
              <div
                class="zircon-drop-zone p-4 text-center text-xs text-slate-400 cursor-pointer rounded-xl border border-dashed border-slate-800 hover:border-cyan-500/50 transition"
                @dragover.prevent
                @drop.prevent="onModDrop"
              >
                Drop <code class="text-cyan-300 font-mono">.jar</code> mod files here (or <button class="text-cyan-400 underline font-semibold hover:text-cyan-300" @click="browseMods">browse files</button>)
              </div>
            </div>

            <!-- Mod Discovery: Modrinth & CurseForge -->
            <div class="bg-[#070b10] border border-slate-800/90 rounded-xl p-4 shadow-inner">
              <div class="flex items-center justify-between gap-2 mb-3">
                <div class="z-section text-white font-bold text-sm">Search &amp; Install Mods</div>
                <!-- Provider toggle pills -->
                <div class="flex items-center gap-1 bg-slate-900 border border-slate-800 rounded-xl p-0.5">
                  <button
                    class="flex items-center gap-1.5 px-3 py-1 rounded-lg text-xs font-bold transition-all"
                    :class="modProvider === 'modrinth' ? 'bg-[#1bd96a]/20 text-[#46d66d] border border-[#1bd96a]/40 shadow-sm' : 'text-slate-400 hover:text-white'"
                    @click="setModProvider('modrinth')"
                  >
                    <img src="../assets/modrinth.svg" class="w-3.5 h-3.5" />
                    Modrinth
                  </button>
                  <button
                    class="flex items-center gap-1.5 px-3 py-1 rounded-lg text-xs font-bold transition-all"
                    :class="modProvider === 'curseforge' ? 'bg-[#f16436]/20 text-[#f16436] border border-[#f16436]/40 shadow-sm' : 'text-slate-400 hover:text-white'"
                    @click="setModProvider('curseforge')"
                  >
                    <img src="../assets/curseforge.svg" class="w-3.5 h-3.5" />
                    CurseForge
                  </button>
                </div>
              </div>

              <!-- Search Bar & Controls -->
              <div class="flex gap-2">
                <input
                  v-model="modSearchQuery"
                  class="z-input flex-1 text-xs"
                  :placeholder="`Search ${modProvider === 'curseforge' ? 'CurseForge' : 'Modrinth'} (e.g. Sodium, Iris, FerriteCore)...`"
                  @keydown.enter="searchMods"
                />
                <button class="z-btn-ghost px-4 text-xs font-bold shrink-0" :disabled="modSearchBusy" @click="searchMods">Search</button>
              </div>

              <div class="flex items-center justify-between mt-2 text-xs text-slate-400">
                <label class="flex items-center gap-1.5 cursor-pointer select-none">
                  <input type="checkbox" v-model="modSearchAllVersions" class="zircon-check" @change="searchMods" />
                  <span>Show all Minecraft versions</span>
                </label>
                <span v-if="modSearchBusy" class="text-cyan-400 font-mono flex items-center gap-1.5">
                  <span class="inline-block w-2.5 h-2.5 border-2 border-accent border-t-transparent rounded-full animate-spin"></span>
                  Searching…
                </span>
              </div>

              <!-- Search results list -->
              <div class="mt-3 flex flex-col gap-2 max-h-[300px] overflow-y-auto pr-1">
                <div
                  v-for="hit in modResults"
                  :key="hit.projectId || hit.id"
                  class="bg-slate-900/60 border border-slate-800 rounded-xl p-3 flex flex-col gap-2 transition hover:border-slate-700"
                >
                  <div class="flex items-start gap-3">
                    <img
                      v-if="hit.iconUrl"
                      :src="hit.iconUrl"
                      class="w-10 h-10 rounded-lg shrink-0 mt-0.5 object-cover bg-slate-950"
                      loading="lazy"
                    />
                    <div class="flex-1 min-w-0">
                      <div class="flex items-center justify-between gap-2">
                        <div class="text-xs font-bold text-white truncate">{{ hit.title || hit.name }}</div>
                        <button
                          v-if="hit.origin === 'curseforge' || modProvider === 'curseforge'"
                          class="text-xs px-3 py-1.5 rounded-lg font-bold shrink-0 flex items-center gap-1.5 transition disabled:opacity-50 disabled:cursor-not-allowed bg-[#F16436]/20 text-orange-200 border border-[#F16436]/40 hover:bg-[#F16436]/30 shadow-sm"
                          @click="openCurseforgeModal(hit, 'mod')"
                        >
                          Get on CurseForge
                        </button>
                        <button
                          v-else
                          class="text-xs px-3 py-1.5 rounded-lg font-bold shrink-0 flex items-center gap-1.5 transition disabled:opacity-50 disabled:cursor-not-allowed bg-[#46d66d]/20 text-green-200 border border-[#46d66d]/40 hover:bg-[#46d66d]/30 shadow-sm"
                          :disabled="installingId === (hit.projectId || hit.id) || hit.versionsLoading"
                          @click="installMod(hit)"
                        >
                          <svg
                            v-if="installingId === (hit.projectId || hit.id)"
                            class="animate-spin h-3.5 w-3.5 text-[#46d66d]"
                            xmlns="http://www.w3.org/2000/svg"
                            fill="none"
                            viewBox="0 0 24 24"
                          >
                            <circle class="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" stroke-width="4"></circle>
                            <path class="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4zm2 5.291A7.962 7.962 0 014 12H0c0 3.042 1.135 5.824 3 7.938l3-2.647z"></path>
                          </svg>
                          <span>{{ installingId === (hit.projectId || hit.id) ? 'Installing…' : 'Install' }}</span>
                        </button>
                      </div>
                      <div v-if="hit.description || hit.summary" class="text-[11px] text-slate-300 line-clamp-2 my-1">
                        {{ hit.description || hit.summary }}
                      </div>
                      <div class="flex items-center gap-2 text-[10px] text-slate-400 flex-wrap">
                        <span v-if="hit.author">by <strong class="text-white font-medium">{{ hit.author }}</strong></span>
                        <span>·</span>
                        <span>{{ fmtCount(hit.downloads || hit.downloadCount) }} downloads</span>
                        <span>·</span>
                        <a
                          :href="hit.projectUrl || hit.websiteUrl"
                          target="_blank"
                          rel="noopener noreferrer"
                          class="font-medium hover:underline inline-flex items-center gap-0.5"
                          :class="(hit.origin === 'curseforge' || modProvider === 'curseforge') ? 'text-[#F16436]' : 'text-[#46d66d]'"
                          @click.prevent="openExternalLink(hit.projectUrl || hit.websiteUrl)"
                        >
                          View on {{ (hit.origin === 'curseforge' || modProvider === 'curseforge') ? 'CurseForge' : 'Modrinth' }} ↗
                        </a>
                      </div>
                    </div>
                  </div>

                  <!-- Version selector for Modrinth / CurseForge -->
                  <div class="flex items-center gap-2 pt-2 border-t border-slate-800/80">
                    <label class="text-[10px] text-slate-400 shrink-0 font-semibold">Version:</label>
                    <select
                      v-model="hit.selectedVersionId"
                      :disabled="installingId === (hit.projectId || hit.id) || hit.versionsLoading || hit.versionsFailed || !hit.versionOptions?.length"
                      class="flex-1 min-w-0 bg-slate-950 border border-slate-700 rounded-lg px-2.5 py-1 text-[11px] text-slate-200 disabled:opacity-50 focus:border-cyan-400 focus:outline-none"
                    >
                      <option v-if="hit.versionsLoading" value="" disabled>Loading versions…</option>
                      <option v-else-if="hit.versionsFailed || !hit.versionOptions?.length" value="" disabled>No compatible versions found</option>
                      <option
                        v-for="v in hit.versionOptions"
                        :key="v.id"
                        :value="v.id"
                      >
                        {{ v.versionNumber || v.name || v.fileName }}
                      </option>
                    </select>
                  </div>
                </div>
                <div v-if="!modSearchBusy && modSearchDone && modResults.length === 0" class="text-xs text-slate-500 py-2">
                  No mods found for query on {{ modProvider === 'curseforge' ? 'CurseForge' : 'Modrinth' }}.
                </div>
              </div>
            </div>
          </template>

          <!-- ================= TAB: SHADERS ================= -->
          <template v-else-if="activeTab === 'shaders'">
            <div class="bg-[#070b10] border border-slate-800/90 rounded-xl p-4 shadow-inner">
              <div class="z-section mb-2 text-white font-bold text-sm">Active Shaderpack</div>
              <select v-model="activeShaderpack" class="z-input mb-4 text-xs" @change="onShaderpackChange">
                <option value="">None (shaders disabled)</option>
                <option v-for="p in detailedPacks.shaderpacks" :key="p.filename" :value="p.filename">
                  {{ p.title || p.filename }} {{ p.version ? `(${p.version})` : '' }}
                </option>
              </select>

              <div class="z-section mb-2 text-white font-bold text-sm">Installed Shaders ({{ detailedPacks.shaderpacks.length }})</div>
              <div v-if="detailedPacks.shaderpacks.length" class="max-h-[180px] overflow-y-auto mb-3 flex flex-col gap-1.5 pr-1">
                <div
                  v-for="p in detailedPacks.shaderpacks"
                  :key="p.filename"
                  class="flex items-center gap-2.5 p-2.5 rounded-xl bg-slate-900/60 border border-slate-800/80 text-xs"
                >
                  <div class="flex-1 min-w-0">
                    <div class="text-white font-medium truncate">{{ p.title || p.filename }}</div>
                    <div v-if="p.description" class="text-[10px] text-slate-400 truncate">{{ p.description }}</div>
                  </div>
                  <span class="text-slate-500 font-mono text-[11px]">{{ fmtBytes(p.sizeBytes) }}</span>
                  <button
                    class="text-red-400 hover:text-red-300 text-xs px-2 py-0.5 hover:bg-red-500/10 rounded-lg transition-colors font-medium flex items-center gap-1 shrink-0"
                    title="Delete"
                    @click="deletePack('shader', p.filename)"
                  >
                    <svg class="w-3.5 h-3.5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8">
                      <path d="M3 6h18M19 6v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6M8 6V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2" />
                    </svg>
                    <span>Delete</span>
                  </button>
                </div>
              </div>

              <!-- Custom upload / drop zone for shaders -->
              <div
                class="zircon-drop-zone p-4 text-center text-xs text-slate-400 cursor-pointer rounded-xl border border-dashed border-slate-800 hover:border-cyan-500/50 transition"
                @dragover.prevent
                @drop.prevent="onShaderDrop"
              >
                Drop <code class="text-cyan-300 font-mono">.zip</code> shaderpack files here (or <button class="text-cyan-400 underline font-semibold hover:text-cyan-300" @click="browseShaders">browse files</button>)
              </div>
            </div>

            <!-- Shaders Discovery: Modrinth & CurseForge -->
            <div class="bg-[#070b10] border border-slate-800/90 rounded-xl p-4 shadow-inner">
              <div class="flex items-center justify-between gap-2 mb-3">
                <div class="z-section text-white font-bold text-sm">Search &amp; Install Shaders</div>
                <div class="flex items-center gap-1 bg-slate-900 border border-slate-800 rounded-xl p-0.5">
                  <button
                    class="flex items-center gap-1.5 px-3 py-1 rounded-lg text-xs font-bold transition-all"
                    :class="shaderProvider === 'modrinth' ? 'bg-[#1bd96a]/20 text-[#46d66d] border border-[#1bd96a]/40 shadow-sm' : 'text-slate-400 hover:text-white'"
                    @click="setShaderProvider('modrinth')"
                  >
                    <img src="../assets/modrinth.svg" class="w-3.5 h-3.5" />
                    Modrinth
                  </button>
                  <button
                    class="flex items-center gap-1.5 px-3 py-1 rounded-lg text-xs font-bold transition-all"
                    :class="shaderProvider === 'curseforge' ? 'bg-[#f16436]/20 text-[#f16436] border border-[#f16436]/40 shadow-sm' : 'text-slate-400 hover:text-white'"
                    @click="setShaderProvider('curseforge')"
                  >
                    <img src="../assets/curseforge.svg" class="w-3.5 h-3.5" />
                    CurseForge
                  </button>
                </div>
              </div>

              <div class="flex gap-2">
                <input
                  v-model="shaderSearchQuery"
                  class="z-input flex-1 text-xs"
                  placeholder="Search shaderpacks (e.g. Complementary, BSL, Bliss)..."
                  @keydown.enter="searchShaders"
                />
                <button class="z-btn-ghost px-4 text-xs font-bold shrink-0" :disabled="shaderSearchBusy" @click="searchShaders">Search</button>
              </div>

              <div class="flex items-center justify-between mt-2 text-xs text-slate-400">
                <label class="flex items-center gap-1.5 cursor-pointer select-none">
                  <input type="checkbox" v-model="shaderSearchAllVersions" class="zircon-check" @change="searchShaders" />
                  <span>Show all Minecraft versions</span>
                </label>
                <span v-if="shaderSearchBusy" class="text-cyan-400 font-mono flex items-center gap-1.5">
                  <span class="inline-block w-2.5 h-2.5 border-2 border-accent border-t-transparent rounded-full animate-spin"></span>
                  Searching…
                </span>
              </div>

              <!-- Shaders results -->
              <div class="mt-3 flex flex-col gap-2 max-h-[300px] overflow-y-auto pr-1">
                <div
                  v-for="hit in shaderResults"
                  :key="hit.projectId || hit.id"
                  class="bg-slate-900/60 border border-slate-800 rounded-xl p-3 flex flex-col gap-2 transition hover:border-slate-700"
                >
                  <div class="flex items-start gap-3">
                    <img
                      v-if="hit.iconUrl"
                      :src="hit.iconUrl"
                      class="w-10 h-10 rounded-lg shrink-0 mt-0.5 object-cover bg-slate-950"
                      loading="lazy"
                    />
                    <div class="flex-1 min-w-0">
                      <div class="flex items-center justify-between gap-2">
                        <div class="text-xs font-bold text-white truncate">{{ hit.title || hit.name }}</div>
                        <button
                          v-if="hit.origin === 'curseforge' || shaderProvider === 'curseforge'"
                          class="text-xs px-3 py-1.5 rounded-lg font-bold shrink-0 flex items-center gap-1.5 transition disabled:opacity-50 disabled:cursor-not-allowed bg-[#F16436]/20 text-orange-200 border border-[#F16436]/40 hover:bg-[#F16436]/30 shadow-sm"
                          @click="openCurseforgeModal(hit, 'shaderpack')"
                        >
                          Get on CurseForge
                        </button>
                        <button
                          v-else
                          class="text-xs px-3 py-1.5 rounded-lg font-bold shrink-0 flex items-center gap-1.5 transition disabled:opacity-50 disabled:cursor-not-allowed bg-[#46d66d]/20 text-green-200 border border-[#46d66d]/40 hover:bg-[#46d66d]/30 shadow-sm"
                          :disabled="installingId === (hit.projectId || hit.id) || hit.versionsLoading"
                          @click="installPackItem(hit, 'shader')"
                        >
                          <svg
                            v-if="installingId === (hit.projectId || hit.id)"
                            class="animate-spin h-3.5 w-3.5 text-[#46d66d]"
                            xmlns="http://www.w3.org/2000/svg"
                            fill="none"
                            viewBox="0 0 24 24"
                          >
                            <circle class="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" stroke-width="4"></circle>
                            <path class="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4zm2 5.291A7.962 7.962 0 014 12H0c0 3.042 1.135 5.824 3 7.938l3-2.647z"></path>
                          </svg>
                          <span>{{ installingId === (hit.projectId || hit.id) ? 'Installing…' : 'Install' }}</span>
                        </button>
                      </div>
                      <div v-if="hit.description || hit.summary" class="text-[11px] text-slate-300 line-clamp-2 my-1">
                        {{ hit.description || hit.summary }}
                      </div>
                      <div class="flex items-center gap-2 text-[10px] text-slate-400 flex-wrap">
                        <span v-if="hit.author">by <strong class="text-white font-medium">{{ hit.author }}</strong></span>
                        <span>·</span>
                        <span>{{ fmtCount(hit.downloads || hit.downloadCount) }} downloads</span>
                        <span>·</span>
                        <a
                          :href="hit.projectUrl || hit.websiteUrl"
                          target="_blank"
                          rel="noopener noreferrer"
                          class="font-medium hover:underline inline-flex items-center gap-0.5"
                          :class="(hit.origin === 'curseforge' || shaderProvider === 'curseforge') ? 'text-[#F16436]' : 'text-[#46d66d]'"
                          @click.prevent="openExternalLink(hit.projectUrl || hit.websiteUrl)"
                        >
                          {{ (hit.origin === 'curseforge' || shaderProvider === 'curseforge') ? 'View on CurseForge ↗' : 'View on Modrinth ↗' }}
                        </a>
                      </div>
                    </div>
                  </div>

                  <div class="flex items-center gap-2 pt-2 border-t border-slate-800/80">
                    <label class="text-[10px] text-slate-400 shrink-0 font-semibold">Version:</label>
                    <select
                      v-model="hit.selectedVersionId"
                      :disabled="installingId === (hit.projectId || hit.id) || hit.versionsLoading || hit.versionsFailed || !hit.versionOptions?.length"
                      class="flex-1 min-w-0 bg-slate-950 border border-slate-700 rounded-lg px-2.5 py-1 text-[11px] text-slate-200 disabled:opacity-50 focus:border-cyan-400 focus:outline-none"
                    >
                      <option v-if="hit.versionsLoading" value="" disabled>Loading versions…</option>
                      <option v-else-if="hit.versionsFailed || !hit.versionOptions?.length" value="" disabled>No compatible versions found</option>
                      <option v-for="v in hit.versionOptions" :key="v.id" :value="v.id">
                        {{ v.versionNumber || v.name || v.fileName }}
                      </option>
                    </select>
                  </div>
                </div>
                <div v-if="!shaderSearchBusy && shaderSearchDone && shaderResults.length === 0" class="text-xs text-slate-500 py-2">
                  No shaders found.
                </div>
              </div>
            </div>
          </template>

          <!-- ================= TAB: TEXTURE PACKS ================= -->
          <template v-else-if="activeTab === 'textures'">
            <div class="bg-[#070b10] border border-slate-800/90 rounded-xl p-4 shadow-inner">
              <div class="z-section mb-2 text-white font-bold text-sm">Installed Texture Packs ({{ detailedPacks.resourcepacks.length }})</div>
              <div v-if="detailedPacks.resourcepacks.length" class="max-h-[220px] overflow-y-auto mb-3 flex flex-col gap-1.5 pr-1">
                <label
                  v-for="p in detailedPacks.resourcepacks"
                  :key="p.filename"
                  class="flex items-center gap-2.5 text-xs cursor-pointer p-2.5 rounded-xl bg-slate-900/60 border border-slate-800/80 transition hover:border-slate-700"
                >
                  <input
                    type="checkbox"
                    class="zircon-check shrink-0"
                    :checked="packs.activeResourcepacks.includes(p.filename)"
                    @change="togglePack(p.filename)"
                  />
                  <div class="flex-1 min-w-0">
                    <div class="text-slate-200 font-medium truncate">{{ p.title || p.filename }}</div>
                    <div v-if="p.description" class="text-[10px] text-slate-400 truncate">{{ p.description }}</div>
                  </div>
                  <span class="bg-slate-950 text-slate-400 border border-slate-800 text-[10px] px-1.5 py-0.2 rounded font-mono shrink-0">
                    {{ p.version || (p.packFormat ? 'v' + p.packFormat : 'Pack') }}
                  </span>
                  <span class="text-slate-500 font-mono text-[11px] shrink-0">{{ fmtBytes(p.sizeBytes) }}</span>
                  <button
                    class="text-red-400 hover:text-red-300 text-xs px-2 py-0.5 hover:bg-red-500/10 rounded-lg transition-colors font-medium flex items-center gap-1 shrink-0"
                    title="Delete"
                    @click.stop.prevent="deletePack('resource', p.filename)"
                  >
                    <svg class="w-3.5 h-3.5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8">
                      <path d="M3 6h18M19 6v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6M8 6V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2" />
                    </svg>
                    <span>Delete</span>
                  </button>
                </label>
              </div>
              <div v-else class="text-xs text-slate-500 mb-3">No texture packs installed.</div>

              <!-- Custom upload / drop zone for texture packs -->
              <div
                class="zircon-drop-zone p-4 text-center text-xs text-slate-400 cursor-pointer rounded-xl border border-dashed border-slate-800 hover:border-cyan-500/50 transition"
                @dragover.prevent
                @drop.prevent="onTextureDrop"
              >
                Drop <code class="text-cyan-300 font-mono">.zip</code> texture pack files here (or <button class="text-cyan-400 underline font-semibold hover:text-cyan-300" @click="browseTextures">browse files</button>)
              </div>
            </div>

            <!-- Texture Packs Discovery: Modrinth & CurseForge -->
            <div class="bg-[#070b10] border border-slate-800/90 rounded-xl p-4 shadow-inner">
              <div class="flex items-center justify-between gap-2 mb-3">
                <div class="z-section text-white font-bold text-sm">Search &amp; Install Texture Packs</div>
                <div class="flex items-center gap-1 bg-slate-900 border border-slate-800 rounded-xl p-0.5">
                  <button
                    class="flex items-center gap-1.5 px-3 py-1 rounded-lg text-xs font-bold transition-all"
                    :class="textureProvider === 'modrinth' ? 'bg-[#1bd96a]/20 text-[#46d66d] border border-[#1bd96a]/40 shadow-sm' : 'text-slate-400 hover:text-white'"
                    @click="setTextureProvider('modrinth')"
                  >
                    <img src="../assets/modrinth.svg" class="w-3.5 h-3.5" />
                    Modrinth
                  </button>
                  <button
                    class="flex items-center gap-1.5 px-3 py-1 rounded-lg text-xs font-bold transition-all"
                    :class="textureProvider === 'curseforge' ? 'bg-[#f16436]/20 text-[#f16436] border border-[#f16436]/40 shadow-sm' : 'text-slate-400 hover:text-white'"
                    @click="setTextureProvider('curseforge')"
                  >
                    <img src="../assets/curseforge.svg" class="w-3.5 h-3.5" />
                    CurseForge
                  </button>
                </div>
              </div>

              <div class="flex gap-2">
                <input
                  v-model="textureSearchQuery"
                  class="z-input flex-1 text-xs"
                  placeholder="Search texture packs (e.g. Faithful, Bare Bones, Fresh Animations)..."
                  @keydown.enter="searchTextures"
                />
                <button class="z-btn-ghost px-4 text-xs font-bold shrink-0" :disabled="textureSearchBusy" @click="searchTextures">Search</button>
              </div>

              <div class="flex items-center justify-between mt-2 text-xs text-slate-400">
                <label class="flex items-center gap-1.5 cursor-pointer select-none">
                  <input type="checkbox" v-model="textureSearchAllVersions" class="zircon-check" @change="searchTextures" />
                  <span>Show all Minecraft versions</span>
                </label>
                <span v-if="textureSearchBusy" class="text-cyan-400 font-mono flex items-center gap-1.5">
                  <span class="inline-block w-2.5 h-2.5 border-2 border-accent border-t-transparent rounded-full animate-spin"></span>
                  Searching packs…
                </span> <!-- end busy spinner -->
              </div>

              <!-- Texture results -->
              <div class="mt-3 flex flex-col gap-2 max-h-[300px] overflow-y-auto pr-1">
                <div
                  v-for="hit in textureResults"
                  :key="hit.projectId || hit.id"
                  class="bg-slate-900/60 border border-slate-800 rounded-xl p-3 flex flex-col gap-2 transition hover:border-slate-700"
                >
                  <div class="flex items-start gap-3">
                    <img
                      v-if="hit.iconUrl"
                      :src="hit.iconUrl"
                      class="w-10 h-10 rounded-lg shrink-0 mt-0.5 object-cover bg-slate-950"
                      loading="lazy"
                    />
                    <div class="flex-1 min-w-0">
                      <div class="flex items-center justify-between gap-2">
                        <div class="text-xs font-bold text-white truncate">{{ hit.title || hit.name }}</div>
                        <button
                          v-if="hit.origin === 'curseforge' || textureProvider === 'curseforge'"
                          class="text-xs px-3 py-1.5 rounded-lg font-bold shrink-0 flex items-center gap-1.5 transition disabled:opacity-50 disabled:cursor-not-allowed bg-[#F16436]/20 text-orange-200 border border-[#F16436]/40 hover:bg-[#F16436]/30 shadow-sm"
                          @click="openCurseforgeModal(hit, 'resourcepack')"
                        >
                          Get on CurseForge
                        </button>
                        <button
                          v-else
                          class="text-xs px-3 py-1.5 rounded-lg font-bold shrink-0 flex items-center gap-1.5 transition disabled:opacity-50 disabled:cursor-not-allowed bg-[#46d66d]/20 text-green-200 border border-[#46d66d]/40 hover:bg-[#46d66d]/30 shadow-sm"
                          :disabled="installingId === (hit.projectId || hit.id) || hit.versionsLoading"
                          @click="installPackItem(hit, 'resourcepack')"
                        >
                          <svg
                            v-if="installingId === (hit.projectId || hit.id)"
                            class="animate-spin h-3.5 w-3.5 text-[#46d66d]"
                            xmlns="http://www.w3.org/2000/svg"
                            fill="none"
                            viewBox="0 0 24 24"
                          >
                            <circle class="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" stroke-width="4"></circle>
                            <path class="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4zm2 5.291A7.962 7.962 0 014 12H0c0 3.042 1.135 5.824 3 7.938l3-2.647z"></path>
                          </svg>
                          <span>{{ installingId === (hit.projectId || hit.id) ? 'Installing…' : 'Install' }}</span>
                        </button>
                      </div>
                      <div v-if="hit.description || hit.summary" class="text-[11px] text-slate-300 line-clamp-2 my-1">
                        {{ hit.description || hit.summary }}
                      </div>
                      <div class="flex items-center gap-2 text-[10px] text-slate-400 flex-wrap">
                        <span v-if="hit.author">by <strong class="text-white font-medium">{{ hit.author }}</strong></span>
                        <span>·</span>
                        <span>{{ fmtCount(hit.downloads || hit.downloadCount) }} downloads</span>
                        <span>·</span>
                        <a
                          :href="hit.projectUrl || hit.websiteUrl"
                          target="_blank"
                          rel="noopener noreferrer"
                          class="font-medium hover:underline inline-flex items-center gap-0.5"
                          :class="(hit.origin === 'curseforge' || textureProvider === 'curseforge') ? 'text-[#F16436]' : 'text-[#46d66d]'"
                          @click.prevent="openExternalLink(hit.projectUrl || hit.websiteUrl)"
                        >
                          {{ (hit.origin === 'curseforge' || textureProvider === 'curseforge') ? 'View on CurseForge ↗' : 'View on Modrinth ↗' }}
                        </a>
                      </div>
                    </div>
                  </div>

                  <div class="flex items-center gap-2 pt-2 border-t border-slate-800/80">
                    <label class="text-[10px] text-slate-400 shrink-0 font-semibold">Version:</label>
                    <select
                      v-model="hit.selectedVersionId"
                      :disabled="installingId === (hit.projectId || hit.id) || hit.versionsLoading || hit.versionsFailed || !hit.versionOptions?.length"
                      class="flex-1 min-w-0 bg-slate-950 border border-slate-700 rounded-lg px-2.5 py-1 text-[11px] text-slate-200 disabled:opacity-50 focus:border-cyan-400 focus:outline-none"
                    >
                      <option v-if="hit.versionsLoading" value="" disabled>Loading versions…</option>
                      <option v-else-if="hit.versionsFailed || !hit.versionOptions?.length" value="" disabled>No compatible versions found</option>
                      <option v-for="v in hit.versionOptions" :key="v.id" :value="v.id">
                        {{ v.versionNumber || v.name || v.fileName }}
                      </option>
                    </select>
                  </div>
                </div>
                <div v-if="!textureSearchBusy && textureSearchDone && textureResults.length === 0" class="text-xs text-slate-500 py-2">
                  No texture packs found.
                </div>
              </div>
            </div>
          </template>

          <!-- ================= TAB: WORLDS & SAVES ================= -->
          <template v-else-if="activeTab === 'worlds'">
            <div class="bg-[#070b10] border border-slate-800/90 rounded-xl p-4 shadow-inner flex flex-col gap-4">
              <div class="flex items-center justify-between">
                <div class="flex items-center gap-2.5">
                  <div class="z-section text-white font-bold text-sm">Singleplayer Worlds ({{ worldsList.length }})</div>
                  <button
                    class="z-btn-ghost text-xs px-2.5 py-1 rounded-lg border border-slate-700 hover:border-slate-600 flex items-center gap-1.5 transition"
                    :disabled="loadingWorldsState"
                    @click="loadWorlds"
                    title="Refresh worlds list"
                  >
                    <span v-if="loadingWorldsState" class="w-2.5 h-2.5 border-2 border-cyan-400 border-t-transparent rounded-full animate-spin"></span>
                    <span v-else class="text-cyan-400">⟳</span>
                    <span>Refresh</span>
                  </button>
                </div>
                <button
                  class="z-btn-ghost text-xs px-3 py-1.5 rounded-lg border border-slate-700 hover:border-slate-600 flex items-center gap-1.5 transition"
                  @click="openBackupsModal(null)"
                  title="View all world backup snapshots"
                >
                  <svg class="w-3.5 h-3.5 text-cyan-400" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                    <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 8v4l3 3m6-3a9 9 0 11-18 0 9 9 0 0118 0z" />
                  </svg>
                  <span>All Backups</span>
                </button>
              </div>

              <!-- Worlds List Grid -->
              <div v-if="worldsList.length > 0" class="grid grid-cols-1 md:grid-cols-2 gap-3">
                <div
                  v-for="w in worldsList"
                  :key="w.folderName"
                  class="bg-slate-900/60 border border-slate-800 rounded-xl p-3.5 flex flex-col justify-between gap-3 hover:border-slate-700 transition"
                >
                  <div class="flex items-start gap-3">
                    <div class="w-12 h-12 rounded-lg bg-slate-950 border border-slate-800 shrink-0 overflow-hidden flex items-center justify-center">
                      <img v-if="w.iconDataUrl" :src="w.iconDataUrl" class="w-full h-full object-cover" />
                      <svg v-else class="w-6 h-6 text-slate-600" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="1.5" d="M3.055 11H5a2 2 0 012 2v1a2 2 0 002 2 2 2 0 012 2v2.945M8 3.935V5.5A2.5 2.5 0 0010.5 8h.5a2 2 0 012 2 2 2 0 104 0 2 2 0 012-2h1.064M15 20.488V18a2 2 0 012-2h3.064M21 12a9 9 0 11-18 0 9 9 0 0118 0z" />
                      </svg>
                    </div>
                    <div class="flex-1 min-w-0">
                      <div class="flex items-center gap-2">
                        <span class="font-bold text-white text-xs truncate">{{ w.levelName }}</span>
                        <span
                          class="px-1.5 py-0.2 rounded text-[10px] font-bold"
                          :class="w.hardcore ? 'bg-red-500/20 text-red-300 border border-red-500/30' : (w.gameType === 'Creative' ? 'bg-sky-500/20 text-sky-300 border border-sky-500/30' : 'bg-cyan-500/20 text-cyan-300 border border-cyan-500/30')"
                        >
                          {{ w.hardcore ? 'Hardcore' : w.gameType }}
                        </span>
                      </div>
                      <div class="text-[11px] text-slate-400 font-mono mt-0.5 truncate">{{ w.folderName }}</div>
                      <div class="flex items-center gap-2 text-[10px] text-slate-500 mt-1">
                        <span>{{ fmtSize(w.sizeBytes) }}</span>
                        <span v-if="w.lastPlayed">·</span>
                        <span v-if="w.lastPlayed">{{ formatRelativeTime(w.lastPlayed) }}</span>
                        <span v-if="w.seed !== null">·</span>
                        <span v-if="w.seed !== null" class="font-mono">Seed: {{ w.seed }}</span>
                      </div>
                    </div>
                  </div>

                  <!-- World Card Actions -->
                  <div class="flex items-center justify-end gap-2 pt-2 border-t border-slate-800/80">
                    <button
                      class="z-btn-ghost text-xs px-2.5 py-1 rounded-lg border border-slate-700 hover:border-slate-600 flex items-center gap-1.5 transition"
                      @click="createWorldBackup(w.folderName)"
                      title="Create an instant timestamped backup of this world"
                    >
                      <svg class="w-3.5 h-3.5 text-emerald-400" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M8 7H5a2 2 0 00-2 2v9a2 2 0 002 2h14a2 2 0 002-2V9a2 2 0 00-2-2h-3m-1 4l-3 3m0 0l-3-3m3 3V4" />
                      </svg>
                      <span>Backup</span>
                    </button>
                    <button
                      class="z-btn-ghost text-xs px-2.5 py-1 rounded-lg border border-slate-700 hover:border-slate-600 flex items-center gap-1.5 transition"
                      @click="openBackupsModal(w.folderName)"
                      title="View and restore previous backups for this world"
                    >
                      <span>Snapshots</span>
                    </button>
                    <button
                      class="z-btn-ghost text-xs px-2.5 py-1 rounded-lg border border-slate-700 hover:border-slate-600 flex items-center gap-1.5 transition"
                      @click="exportWorldZip(w.folderName)"
                      title="Export world as a .zip anywhere on your disk"
                    >
                      <span>Export</span>
                    </button>
                  </div>
                </div>
              </div>
              <div v-else-if="!loadingWorldsState" class="py-8 text-center text-slate-500 text-xs">
                No singleplayer worlds found in this instance. Launch Minecraft and create a world to manage it here!
              </div>
            </div>
          </template>

          <!-- ================= TAB: SCREENSHOTS ================= -->
          <template v-else-if="activeTab === 'screenshots'">
            <div class="bg-[#070b10] border border-slate-800/90 rounded-xl p-4 shadow-inner flex flex-col gap-4">
              <div class="flex items-center justify-between">
                <div class="flex items-center gap-2.5">
                  <div class="z-section text-white font-bold text-sm">Screenshot Gallery ({{ screenshotsList.length }})</div>
                  <button
                    class="z-btn-ghost text-xs px-2.5 py-1 rounded-lg border border-slate-700 hover:border-slate-600 flex items-center gap-1.5 transition"
                    :disabled="loadingScreenshotsState"
                    @click="loadScreenshots"
                    title="Refresh screenshots gallery"
                  >
                    <span v-if="loadingScreenshotsState" class="w-2.5 h-2.5 border-2 border-cyan-400 border-t-transparent rounded-full animate-spin"></span>
                    <span v-else class="text-cyan-400">⟳</span>
                    <span>Refresh</span>
                  </button>
                </div>
                <button
                  class="z-btn-ghost text-xs px-3 py-1.5 rounded-lg border border-slate-700 hover:border-slate-600 flex items-center gap-1.5 transition"
                  @click="openFolder('screenshots')"
                  title="Open screenshots folder in operating system file explorer"
                >
                  <svg class="w-3.5 h-3.5 text-cyan-400" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                    <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M5 19a2 2 0 01-2-2V7a2 2 0 012-2h4l2 2h4a2 2 0 012 2v1M5 19h14a2 2 0 002-2v-5a2 2 0 00-2-2H9a2 2 0 00-2 2v5a2 2 0 01-2 2z" />
                  </svg>
                  <span>Open Folder</span>
                </button>
              </div>

              <!-- Screenshot Gallery Grid -->
              <div v-if="screenshotsList.length > 0" class="grid grid-cols-2 sm:grid-cols-3 md:grid-cols-4 gap-3">
                <div
                  v-for="s in screenshotsList"
                  :key="s.filename"
                  class="group relative bg-slate-950 border border-slate-800 rounded-xl overflow-hidden hover:border-cyan-500/50 transition cursor-pointer shadow-md"
                  @click="openLightbox(s)"
                >
                  <div class="aspect-video w-full overflow-hidden bg-slate-900 flex items-center justify-center">
                    <img :src="s.dataUrl" class="w-full h-full object-cover group-hover:scale-105 transition duration-300" loading="lazy" />
                  </div>
                  <div class="p-2 bg-slate-900/90 flex items-center justify-between text-[10px] text-slate-400">
                    <span class="truncate font-mono mr-1">{{ s.filename }}</span>
                    <span class="shrink-0">{{ fmtSize(s.sizeBytes) }}</span>
                  </div>

                  <!-- Hover Action Overlay -->
                  <div class="absolute inset-0 bg-slate-950/70 opacity-0 group-hover:opacity-100 transition-opacity flex items-center justify-center gap-2 p-2 pointer-events-none group-hover:pointer-events-auto">
                    <button
                      class="px-2.5 py-1 bg-cyan-500/20 text-cyan-300 hover:bg-cyan-500/30 border border-cyan-500/40 rounded-lg text-xs font-bold transition"
                      @click.stop="openLightbox(s)"
                    >
                      View
                    </button>
                    <button
                      class="px-2.5 py-1 bg-red-500/20 text-red-300 hover:bg-red-500/30 border border-red-500/40 rounded-lg text-xs font-bold transition"
                      @click.stop="deleteScreenshot(s.filename)"
                    >
                      Delete
                    </button>
                  </div>
                </div>
              </div>
              <div v-else-if="!loadingScreenshotsState" class="py-8 text-center text-slate-500 text-xs">
                No screenshots found. Press <code class="text-cyan-300 font-mono">F2</code> in-game to capture screenshots!
              </div>
            </div>
          </template>
        </div>
      </template>


      <div v-else class="flex-1 flex items-center justify-center text-slate-500 text-sm">
        Select an instance to manage mods, shaders &amp; texture packs.
      </div>
    </div>

    <!-- ================= MODAL: CURSEFORGE COUNTDOWN & DROP ================= -->
    <div
      v-if="curseforgeModal.open"
      class="absolute inset-0 z-50 bg-[#070b0f]/85 backdrop-blur-md flex items-center justify-center p-4"
      @click.self="closeCurseforgeModal"
    >
      <div class="z-card w-full max-w-[500px] p-6 overflow-hidden shadow-2xl relative border border-slate-700/70 rounded-2xl bg-[#0e1622]">
        <div class="flex items-start justify-between gap-3 mb-4">
          <div class="flex items-center gap-3">
            <img v-if="curseforgeModal.iconUrl" :src="curseforgeModal.iconUrl" class="w-11 h-11 rounded-xl object-cover bg-slate-950 shrink-0" />
            <div class="w-11 h-11 rounded-xl bg-[#f16436]/20 border border-[#f16436]/40 flex items-center justify-center shrink-0" v-else>
              <img src="../assets/curseforge.svg" class="w-6 h-6" />
            </div>
            <div>
              <div class="text-white font-bold text-base truncate">{{ curseforgeModal.title }}</div>
              <div class="text-[11px] text-slate-400">CurseForge {{ curseforgeModal.packType === 'shaderpack' ? 'Shaderpack' : (curseforgeModal.packType === 'resourcepack' ? 'Texture Pack' : 'Mod') }}</div>
            </div>
          </div>
          <button class="text-slate-400 hover:text-white text-lg leading-none" @click="closeCurseforgeModal">✕</button>
        </div>

        <!-- Countdown / Download link state -->
        <div v-if="!curseforgeModal.success" class="mb-4 bg-slate-950/80 border border-slate-800 rounded-xl p-3.5 text-xs text-slate-300 flex flex-col gap-2">
          <div v-if="curseforgeModal.countdown > 0" class="flex items-center justify-between">
            <span>Opening CurseForge download page in <strong class="text-cyan-400 font-mono">{{ curseforgeModal.countdown }}s</strong>…</span>
            <button class="text-cyan-400 underline font-semibold hover:text-cyan-300" @click="triggerCurseforgeDownload">Open Now</button>
          </div>
          <div v-else class="flex items-center justify-between">
            <span class="text-emerald-400 font-medium">✓ CurseForge download page opened</span>
            <button class="text-cyan-400 underline font-semibold hover:text-cyan-300" @click="triggerCurseforgeDownload">Re-open Link</button>
          </div>
          <div v-if="curseforgeModal.targetFileName" class="text-[11px] text-slate-400 font-mono">
            Expected file: <span class="text-slate-200">{{ curseforgeModal.targetFileName }}</span>
          </div>
        </div>

        <!-- Success feedback state -->
        <div v-if="curseforgeModal.success" class="mb-4 bg-emerald-950/50 border border-emerald-800/80 rounded-xl p-3.5 text-xs text-emerald-300 flex items-center gap-2.5">
          <span class="text-emerald-400 text-base font-bold">✓</span>
          <span><strong>{{ curseforgeModal.successTitle || curseforgeModal.title }}</strong> installed successfully!</span>
        </div>

        <!-- Drop zone inside modal -->
        <div
          class="zircon-drop-zone p-6 text-center text-xs text-slate-400 cursor-pointer rounded-xl border border-dashed transition mb-4"
          :class="curseforgeModal.success ? 'border-emerald-500/50 bg-emerald-950/20' : 'border-slate-700 hover:border-cyan-500/60 bg-slate-950/40'"
          @dragover.prevent
          @drop.prevent="onCurseforgeModalDrop"
        >
          <div class="text-slate-200 font-semibold mb-1">
            {{ curseforgeModal.success ? 'File received!' : 'Drop downloaded file here' }}
          </div>
          <div class="text-[11px] text-slate-400">
            Accepts <code class="text-cyan-300 font-mono">{{ curseforgeModal.packType === 'mod' ? '.jar' : '.zip' }}</code> (or <button class="text-cyan-400 underline font-semibold hover:text-cyan-300" @click="browseCurseforgeModalFile">browse file</button>)
          </div>
        </div>

        <div class="flex justify-end gap-2.5 pt-3 border-t border-slate-800/80">
          <button class="z-btn-ghost text-xs px-5 py-2 rounded-xl font-semibold" @click="closeCurseforgeModal">
            {{ curseforgeModal.success ? 'Done' : 'Close' }}
          </button>
        </div>
      </div>
    </div>

    <!-- ================= MODAL: NEW INSTANCE ================= -->
    <div
      v-if="showNewDialog"
      class="absolute inset-0 z-40 bg-[#070b0f]/85 backdrop-blur-md flex items-center justify-center p-4"
      @click.self="!modpackProgress.active && (showNewDialog = false)"
    >
      <div class="z-card w-full max-w-[640px] max-h-[85vh] flex flex-col p-6 overflow-hidden shadow-2xl relative border border-slate-700/60 rounded-2xl bg-[#0e1622]">
        <div class="flex items-center justify-between pb-3 border-b border-slate-800/80 mb-4">
          <h3 class="text-white font-bold text-base">New Instance</h3>
          <button
            v-if="!modpackProgress.active"
            class="text-slate-400 hover:text-white text-lg leading-none"
            @click="showNewDialog = false"
          >
            ✕
          </button>
        </div>

        <!-- 3 Tab Selector -->
        <div class="z-segmented-track mb-4 w-full flex">
          <button
            type="button"
            class="z-segmented-pill flex-1 text-center py-2 text-xs font-semibold"
            :class="{ 'active': newInstanceTab === 'modpacks' }"
            @click="newInstanceTab = 'modpacks'"
          >
            Modrinth Modpacks
          </button>
          <button
            type="button"
            class="z-segmented-pill flex-1 text-center py-2 text-xs font-semibold"
            :class="{ 'active': newInstanceTab === 'blank' }"
            @click="newInstanceTab = 'blank'"
          >
            Blank Instance
          </button>
          <button
            type="button"
            class="z-segmented-pill flex-1 text-center py-2 text-xs font-semibold"
            :class="{ 'active': newInstanceTab === 'import' }"
            @click="newInstanceTab = 'import'"
          >
            Import Modpack
          </button>
        </div>



        <!-- TAB 1: BROWSE MODPACKS -->
        <div v-if="newInstanceTab === 'modpacks'" class="flex-1 min-h-0 flex flex-col gap-3 overflow-hidden">
          <div class="flex gap-2">
            <input
              v-model="modpackSearchQuery"
              class="z-input flex-1 text-xs"
              placeholder="Search Modrinth modpacks (e.g. Cobblemon, Fabulously Optimized, Better MC)..."
              @keydown.enter="searchModpacks"
            />
            <button
              class="z-btn-ghost px-4 text-xs font-bold shrink-0"
              :disabled="modpackSearchBusy"
              @click="searchModpacks"
            >
              Search
            </button>
          </div>

          <div class="flex items-center justify-between text-xs text-slate-400 px-0.5">
            <span>{{ modpackResults.length ? `${modpackResults.length} modpacks found` : 'Popular modpacks' }}</span>
            <span v-if="modpackSearchBusy" class="text-cyan-400 font-mono flex items-center gap-1.5">
              <span class="inline-block w-2.5 h-2.5 border-2 border-accent border-t-transparent rounded-full animate-spin"></span>
              Searching…
            </span>
          </div>

          <!-- Modpack results scroll area -->
          <div class="flex-1 min-h-0 overflow-y-auto pr-1 flex flex-col gap-2">
            <div
              v-for="hit in modpackResults"
              :key="hit.projectId || hit.id"
              class="bg-slate-900/60 border border-slate-800 rounded-xl p-3 flex flex-col gap-2 transition hover:border-slate-700"
            >
              <div class="flex items-start gap-3">
                <img
                  v-if="hit.iconUrl"
                  :src="hit.iconUrl"
                  class="w-11 h-11 rounded-lg shrink-0 mt-0.5 object-cover bg-slate-950"
                  loading="lazy"
                />
                <div class="w-11 h-11 rounded-lg shrink-0 mt-0.5 bg-gradient-to-br from-[#1bd96a]/20 to-[#1bd96a]/5 border border-[#1bd96a]/30 flex items-center justify-center font-bold text-white text-sm" v-else>
                  {{ (hit.title || hit.name || 'M').charAt(0).toUpperCase() }}
                </div>
                <div class="flex-1 min-w-0">
                  <div class="flex items-center justify-between gap-2">
                    <div class="text-xs font-bold text-white truncate">{{ hit.title || hit.name }}</div>
                    <button
                      class="text-xs px-3 py-1.5 rounded-lg font-bold shrink-0 flex items-center gap-1.5 transition disabled:opacity-50 disabled:cursor-not-allowed bg-[#1bd96a]/20 text-emerald-300 border border-[#1bd96a]/40 hover:bg-[#1bd96a]/30 shadow-sm"
                      :disabled="installingModpackId === (hit.projectId || hit.id) || modpackProgress.active"
                      @click="installModpack(hit)"
                    >
                      <svg
                        v-if="installingModpackId === (hit.projectId || hit.id)"
                        class="animate-spin h-3.5 w-3.5 text-[#1bd96a]"
                        xmlns="http://www.w3.org/2000/svg"
                        fill="none"
                        viewBox="0 0 24 24"
                      >
                        <circle class="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" stroke-width="4"></circle>
                        <path class="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4zm2 5.291A7.962 7.962 0 014 12H0c0 3.042 1.135 5.824 3 7.938l3-2.647z"></path>
                      </svg>
                      <span>Install Modpack</span>
                    </button>
                  </div>
                  <div class="text-[11px] text-slate-400 line-clamp-2 mt-0.5 leading-relaxed">
                    {{ hit.summary || hit.description || 'No description provided.' }}
                  </div>
                  <div class="flex items-center gap-3 text-[10px] text-slate-400 font-mono mt-1.5">
                    <span v-if="hit.author">by <strong class="text-slate-300">{{ hit.author }}</strong></span>
                    <span v-if="hit.downloads">⬇ {{ fmtCount(hit.downloads) }}</span>
                  </div>

                  <!-- Version selector -->
                  <div class="mt-2 flex items-center gap-2">
                    <span class="text-[10px] text-slate-400 shrink-0 font-mono">Version:</span>
                    <select
                      v-model="selectedModpackVersions[hit.projectId || hit.id]"
                      :disabled="installingModpackId === (hit.projectId || hit.id) || hit.versionsLoading"
                      class="flex-1 min-w-0 bg-slate-950 border border-slate-700 rounded-lg px-2.5 py-1 text-[11px] text-slate-200 disabled:opacity-50 focus:border-cyan-400 focus:outline-none"
                    >
                      <option v-if="hit.versionsLoading" value="" disabled>Loading versions…</option>
                      <option v-else-if="!hit.versionOptions?.length" value="" disabled>No downloadable versions</option>
                      <option v-for="v in hit.versionOptions" :key="v.id" :value="v.id">
                        {{ v.versionNumber || v.name }}
                      </option>
                    </select>
                  </div>
                </div>
              </div>
            </div>
            <div v-if="!modpackSearchBusy && modpackSearchDone && modpackResults.length === 0" class="text-xs text-slate-500 py-6 text-center">
              No modpacks found. Try a different search keyword.
            </div>
          </div>
        </div>

        <!-- TAB 2: BLANK INSTANCE -->
        <div v-else-if="newInstanceTab === 'blank'" class="flex-1 overflow-y-auto pr-1">
          <label class="z-label font-semibold text-slate-300 block mb-1">Instance name</label>
          <input v-model="newForm.name" class="z-input mb-3" placeholder="My Modded World" />
          <label class="z-label font-semibold text-slate-300 block mb-1">Minecraft version</label>
          <select v-model="newForm.mcVersion" class="z-input mb-3" @change="updateLoaderVersions">
            <option v-for="v in mcVersions" :key="v" :value="v">{{ v }}</option>
          </select>
          <label class="z-label font-semibold text-slate-300 block mb-1">Mod loader</label>
          <select v-model="newForm.loaderType" class="z-input mb-3" @change="updateLoaderVersions">
            <option v-for="l in loaderTypes" :key="l" :value="l" class="capitalize">{{ l }}</option>
          </select>
          <template v-if="newForm.loaderType !== 'vanilla'">
            <label class="z-label font-semibold text-slate-300 block mb-1 flex items-center justify-between">
              <span>Loader version</span>
              <span v-if="loadingLoaderVersions" class="text-[11px] text-cyan-400 font-mono">Fetching versions…</span>
              <span v-else-if="recommendedLoaderVersion" class="text-[11px] text-slate-400 font-mono">
                Recommended: {{ recommendedLoaderVersion }}
              </span>
            </label>
            <div v-if="loaderVersions.length > 0" class="mb-5">
              <select v-model="newForm.loaderVersion" class="z-input">
                <option v-for="lv in loaderVersions" :key="lv" :value="lv">
                  {{ lv }} {{ lv === recommendedLoaderVersion ? '★ (Recommended)' : '' }}
                </option>
              </select>
            </div>
            <input
              v-else
              v-model="newForm.loaderVersion"
              class="z-input mb-5"
              placeholder="e.g. 0.16.10"
            />
          </template>
          <div class="flex justify-end gap-2.5 pt-4 border-t border-slate-800/80">
            <button class="z-btn-ghost text-xs px-4 py-2 rounded-xl font-semibold border border-slate-700/80 hover:border-slate-600 hover:text-white" @click="showNewDialog = false">Cancel</button>
            <button class="z-btn-accent text-xs font-bold px-5 py-2 rounded-xl shadow-md hover:shadow-cyan-500/25" :disabled="creating" @click="createInstance">Create</button>
          </div>
        </div>

        <!-- TAB 3: IMPORT MODPACK (.MRPACK OR CURSEFORGE .ZIP) -->
        <div v-else-if="newInstanceTab === 'import'" class="flex-1 flex flex-col gap-4 overflow-y-auto pr-1">
          <div
            class="zircon-drop-zone p-6 text-center text-xs text-slate-400 cursor-pointer rounded-2xl border-2 border-dashed border-slate-700/80 hover:border-cyan-500/60 bg-slate-900/40 transition"
            @click="browseLocalMrpack"
          >
            <svg class="w-8 h-8 mx-auto mb-2 text-slate-500 group-hover:text-cyan-400 transition-colors" fill="none" viewBox="0 0 24 24" stroke="currentColor">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="1.8" d="M7 16a4 4 0 01-.88-7.903A5 5 0 1115.9 6L16 6a5 5 0 011 9.9M15 13l-3-3m0 0l-3 3m3-3v12" />
            </svg>
            <div v-if="importFilePath" class="font-mono text-cyan-300 text-xs break-all mb-1 font-semibold">
              {{ importFilePath }}
            </div>
            <div v-else>
              Click to browse or drop a <code class="text-cyan-300 font-mono">.mrpack</code> or <code class="text-amber-300 font-mono">.zip</code> archive here
            </div>
            <div class="text-[10px] text-slate-500 mt-1">Supports Modrinth (.mrpack) and CurseForge (.zip) export archives</div>
          </div>

          <div>
            <label class="z-label font-semibold text-slate-300 block mb-1">Custom Instance Name (Optional)</label>
            <input
              v-model="importCustomName"
              class="z-input text-xs"
              placeholder="Defaults to pack name declared in archive manifest"
            />
          </div>

          <div class="flex justify-end gap-2.5 pt-4 border-t border-slate-800/80 mt-auto">
            <button class="z-btn-ghost text-xs px-4 py-2 rounded-xl font-semibold border border-slate-700/80 hover:border-slate-600 hover:text-white" @click="showNewDialog = false">Cancel</button>
            <button
              class="z-btn-accent text-xs font-bold px-5 py-2 rounded-xl shadow-md hover:shadow-cyan-500/25"
              :disabled="!importFilePath || importingModpack || modpackProgress.active"
              @click="installLocalMrpack"
            >
              {{ importingModpack ? 'Importing…' : 'Import & Install' }}
            </button>
          </div>
        </div>


        <!-- INSTALLATION PROGRESS OVERLAY -->
        <div
          v-if="modpackProgress.active"
          class="absolute inset-0 bg-[#0e1622]/95 backdrop-blur-sm z-50 flex flex-col items-center justify-center p-6 text-center"
        >
          <div class="w-12 h-12 rounded-full border-4 border-cyan-500/20 border-t-cyan-400 animate-spin mb-4"></div>
          <div class="text-white font-bold text-sm mb-1">{{ modpackProgress.phase || 'Installing Modpack…' }}</div>
          <div class="text-xs text-slate-400 font-mono max-w-sm truncate mb-4">{{ modpackProgress.detail }}</div>
          <div class="w-full max-w-md bg-slate-950 border border-slate-800 rounded-full h-2.5 overflow-hidden shadow-inner">
            <div
              class="bg-gradient-to-r from-cyan-500 to-emerald-400 h-full transition-all duration-300 ease-out"
              :style="{ width: `${Math.round((modpackProgress.fraction || 0) * 100)}%` }"
            ></div>
          </div>
          <div class="text-[11px] text-slate-500 font-mono mt-2">
            {{ Math.round((modpackProgress.fraction || 0) * 100) }}% complete
          </div>
        </div>
      </div>
    </div>

    <!-- ================= MODAL: CLONE INSTANCE ================= -->
    <div
      v-if="showCloneDialog"
      class="absolute inset-0 z-40 bg-[#070b0f]/85 backdrop-blur-md flex items-center justify-center p-4"
      @click.self="showCloneDialog = false"
    >
      <div class="z-card w-full max-w-md flex flex-col p-6 shadow-2xl relative border border-slate-700/60 rounded-2xl bg-[#0e1622]">
        <h3 class="text-white font-bold text-base mb-2">Clone Instance</h3>
        <p class="text-xs text-slate-400 mb-4">
          Create an isolated copy of <strong>{{ selected?.name }}</strong> to safely test new mods, configs, or loaders.
        </p>
        <div class="mb-4">
          <label class="z-label font-semibold text-slate-300 block mb-1">Cloned Instance Name</label>
          <input
            v-model="cloneName"
            class="z-input text-xs w-full"
            placeholder="e.g. My Instance (Copy)"
            @keydown.enter="executeCloneInstance"
          />
        </div>
        <div class="flex justify-end gap-2.5 pt-3 border-t border-slate-800/80">
          <button class="z-btn-ghost text-xs px-4 py-2 rounded-xl font-semibold" @click="showCloneDialog = false">Cancel</button>
          <button
            class="z-btn-accent text-xs font-bold px-5 py-2 rounded-xl shadow-md hover:shadow-cyan-500/25"
            :disabled="!cloneName.trim() || cloning"
            @click="executeCloneInstance"
          >
            {{ cloning ? 'Cloning…' : 'Clone Instance' }}
          </button>
        </div>
      </div>
    </div>

    <!-- ================= MODAL: WORLD BACKUPS ================= -->
    <div
      v-if="showBackupsDialog"
      class="absolute inset-0 z-40 bg-[#070b0f]/85 backdrop-blur-md flex items-center justify-center p-4"
      @click.self="showBackupsDialog = false"
    >
      <div class="z-card w-full max-w-lg max-h-[80vh] flex flex-col p-6 shadow-2xl relative border border-slate-700/60 rounded-2xl bg-[#0e1622]">
        <div class="flex items-center justify-between pb-3 border-b border-slate-800/80 mb-3">
          <h3 class="text-white font-bold text-base">
            {{ backupsFilterWorld ? `Backups for "${backupsFilterWorld}"` : 'All World Backups' }}
          </h3>
          <button class="text-slate-400 hover:text-white text-lg leading-none" @click="showBackupsDialog = false">✕</button>
        </div>

        <div class="flex-1 overflow-y-auto flex flex-col gap-2 min-h-0 pr-1">
          <div v-if="loadingBackups" class="py-6 text-center text-xs text-slate-400">Loading backups…</div>
          <div v-else-if="backupsList.length === 0" class="py-8 text-center text-xs text-slate-500">
            No backup snapshots found. Click "Backup" on any world to create one!
          </div>
          <div
            v-for="b in backupsList"
            :key="b.filename"
            class="bg-slate-900/60 border border-slate-800 rounded-xl p-3 flex items-center justify-between gap-3 hover:border-slate-700 transition"
          >
            <div class="flex-1 min-w-0">
              <div class="text-xs font-bold text-slate-200 truncate">{{ b.worldName }}</div>
              <div class="text-[11px] text-slate-400 font-mono mt-0.5 truncate">{{ b.filename }}</div>
              <div class="text-[10px] text-slate-500 mt-0.5 flex items-center gap-2">
                <span>{{ fmtSize(b.sizeBytes) }}</span>
                <span>·</span>
                <span>{{ formatRelativeTime(b.createdTimestamp) }}</span>
              </div>
            </div>
            <div class="flex items-center gap-2 shrink-0">
              <button
                class="text-xs px-2.5 py-1 rounded-lg font-semibold bg-emerald-500/20 text-emerald-300 border border-emerald-500/30 hover:bg-emerald-500/30 transition"
                @click="restoreBackup(b.filename)"
                title="Restore this snapshot"
              >
                Restore
              </button>
              <button
                class="text-xs px-2 py-1 rounded-lg font-semibold bg-red-500/20 text-red-300 border border-red-500/30 hover:bg-red-500/30 transition"
                @click="deleteBackup(b.filename)"
                title="Delete this snapshot"
              >
                Delete
              </button>
            </div>
          </div>
        </div>

        <div class="flex justify-end pt-3 border-t border-slate-800/80 mt-3">
          <button class="z-btn-ghost text-xs px-5 py-2 rounded-xl font-semibold" @click="showBackupsDialog = false">Close</button>
        </div>
      </div>
    </div>

    <!-- ================= MODAL: SCREENSHOT LIGHTBOX ================= -->
    <div
      v-if="lightboxScreenshot"
      class="absolute inset-0 z-50 bg-[#070b0f]/95 backdrop-blur-md flex flex-col items-center justify-center p-6"
      @click.self="closeLightbox"
    >
      <div class="w-full max-w-5xl flex items-center justify-between text-xs text-slate-300 mb-3 px-1">
        <span class="font-mono truncate mr-2">{{ lightboxScreenshot.filename }} ({{ fmtSize(lightboxScreenshot.sizeBytes) }})</span>
        <button class="text-slate-400 hover:text-white text-xl leading-none" @click="closeLightbox">✕</button>
      </div>
      <div class="max-w-5xl max-h-[80vh] flex items-center justify-center overflow-hidden rounded-2xl border border-slate-800 bg-black shadow-2xl">
        <img :src="lightboxScreenshot.dataUrl" class="max-w-full max-h-[80vh] object-contain" />
      </div>
    </div>

    <!-- ================= MODAL: HOST FOR FRIENDS CO-OP ================= -->
    <div
      v-if="showCoopModal"
      class="absolute inset-0 z-40 bg-[#070b0f]/85 backdrop-blur-md flex items-center justify-center p-4"
      @click.self="showCoopModal = false"
    >
      <div class="z-card w-full max-w-md flex flex-col p-6 shadow-2xl relative border border-cyan-500/40 rounded-2xl bg-[#0e1622]">
        <div class="flex items-center justify-between pb-3 border-b border-slate-800/80 mb-3">
          <div class="flex items-center gap-2">
            <span class="w-2.5 h-2.5 rounded-full bg-emerald-400 animate-pulse"></span>
            <h3 class="text-white font-bold text-base">Host for Friends Co-Op</h3>
          </div>
          <button class="text-slate-400 hover:text-white text-lg leading-none" @click="showCoopModal = false">✕</button>
        </div>

        <p class="text-xs text-slate-300 mb-4 leading-relaxed">
          Zero-config peer multiplayer session. Share this code with friends running Zircon Launcher — their clients will automatically synchronize your mods and connect seamlessly!
        </p>

        <div class="bg-slate-950 border border-cyan-500/40 rounded-2xl p-4 text-center mb-4 shadow-inner">
          <div class="text-[10px] font-bold text-cyan-300 uppercase tracking-widest mb-1">Your Join Code</div>
          <div class="text-3xl font-mono font-black tracking-wider text-white select-all">
            {{ coopSession?.joinCode || 'GENERATING…' }}
          </div>
          <div class="text-[11px] text-slate-400 mt-1">World: <strong>{{ coopSession?.worldName }}</strong></div>
          <div v-if="coopSession?.upnp?.externalIp" class="text-[11px] font-mono text-cyan-300 mt-0.5">
            Public IP: <span class="font-bold select-all">{{ coopSession.upnp.externalIp }}</span>
          </div>
          <div class="text-[10px] font-mono text-slate-500 mt-0.5">
            Game Port: {{ coopSession?.gamePort || 25565 }} · P2P Port: {{ coopSession?.p2pPort || 25566 }}
          </div>
        </div>

        <!-- UPnP Network Status Indicator Badge -->
        <div
          v-if="coopSession?.upnp"
          class="mb-4 p-3 rounded-xl border text-[11px] leading-relaxed flex items-start gap-2.5"
          :class="coopSession.upnp.available && coopSession.upnp.gamePortMapped && coopSession.upnp.p2pPortMapped
            ? 'bg-emerald-950/30 border-emerald-500/40 text-emerald-300'
            : 'bg-amber-950/30 border-amber-500/40 text-amber-300'"
        >
          <span
            class="w-2.5 h-2.5 rounded-full shrink-0 mt-0.5"
            :class="coopSession.upnp.available && coopSession.upnp.gamePortMapped && coopSession.upnp.p2pPortMapped
              ? 'bg-emerald-400 shadow-[0_0_8px_rgba(52,211,153,0.8)]'
              : 'bg-amber-400 shadow-[0_0_8px_rgba(251,191,36,0.8)]'"
          ></span>
          <div>
            <strong class="font-semibold block mb-0.5">
              {{ (coopSession.upnp.available && coopSession.upnp.gamePortMapped && coopSession.upnp.p2pPortMapped) ? 'UPnP Active · Zero-Config Ready' : 'UPnP Not Available' }}
            </strong>
            <span class="text-slate-300">
              {{ (coopSession.upnp.available && coopSession.upnp.gamePortMapped && coopSession.upnp.p2pPortMapped)
                ? 'Ports 25565 & 25566 opened automatically on your router. Friends can join seamlessly!'
                : 'UPnP is disabled on this router. Local friends can join via LAN, but internet friends may require manual router port forwarding.' }}
            </span>
          </div>
        </div>

        <!-- Security Policy Notice -->
        <div class="mb-4 p-3 rounded-xl bg-slate-900/90 border border-amber-500/30 text-[11px] text-slate-300 flex items-start gap-2.5">
          <svg class="w-4 h-4 text-amber-400 shrink-0 mt-0.5" fill="none" viewBox="0 0 24 24" stroke="currentColor">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 9v2m0 4h.01m-6.938 4h13.856c1.54 0 2.502-1.667 1.732-3L13.732 4c-.77-1.333-2.694-1.333-3.464 0L3.34 16c-.77 1.333.192 3 1.732 3z" />
          </svg>
          <div class="leading-relaxed">
            <strong class="text-amber-300 font-semibold block mb-0.5">Security Notice: P2P Mod Streaming</strong>
            Catalog-verified mods (Modrinth &amp; CurseForge) stream automatically to joining friends. Any custom loose <code class="text-amber-200 font-mono">.jar</code> files require your friend to have Developer Mode enabled in Settings to approve and stream.
          </div>
        </div>

        <div class="flex gap-2 mb-4">
          <button
            class="flex-1 z-btn-accent py-2 text-xs font-bold rounded-xl shadow-md flex items-center justify-center gap-1.5"
            @click="copyJoinCode"
          >
            <span>{{ codeCopied ? '✓ Copied Code!' : 'Copy Join Code' }}</span>
          </button>
        </div>

        <div class="flex items-center justify-between pt-3 border-t border-slate-800/80">
          <button class="text-xs text-red-400 hover:text-red-300 font-semibold" @click="stopCoop">
            Stop Hosting Session
          </button>
          <button class="z-btn-ghost text-xs px-4 py-2 rounded-xl font-semibold" @click="showCoopModal = false">
            Minimize
          </button>
        </div>
      </div>
    </div>

    <!-- JOIN BY CODE MODAL -->
    <JoinByCodeModal
      :open="showJoinCodeModal"
      :instances="instances"
      :default-instance-id="selected?.id"
      @close="showJoinCodeModal = false"
      @joined="onJoinedViaCode"
    />

    <!-- MOD DEPENDENCY CONFIRMATION PROMPT MODAL -->
    <DependencyPromptModal
      :open="dependencyModalOpen"
      :dependency-data="pendingDependencyData"
      @confirm="onConfirmDependencies"
      @skip="onSkipDependencies"
      @close="onCloseDependencies"
    />
  </div>
</template>

<script setup>
import { computed, onBeforeUnmount, onMounted, ref } from 'vue';
import { getCurrentWebview } from '@tauri-apps/api/webview';
import DependencyPromptModal from '../components/DependencyPromptModal.vue';
import JoinByCodeModal from '../components/JoinByCodeModal.vue';
import {
  api,
  fmtBytes,
  JAR_FILTER,
  PACK_FILTER,
  ZIP_FILTER,
  MRPACK_FILTER,
  MODPACK_FILTER,
  pickFiles,
  pickFile,
  saveFile,
  onModpackProgress,
} from '../lib/api';

const emit = defineEmits(['launching', 'stopped', 'error']);

const instances = ref([]);
const selected = ref(null);
const selectedDir = ref('');
const activeTab = ref('mods'); // 'mods' | 'shaders' | 'textures' | 'worlds' | 'screenshots'

// Actions & Open folder dropdowns
const showOpenFolderDropdown = ref(false);
const showActionsDropdown = ref(false);

// Clone Modal
const showCloneDialog = ref(false);
const cloneName = ref('');
const cloning = ref(false);

// Worlds state
const worldsList = ref([]);
const loadingWorldsState = ref(false);

// Backups state
const showBackupsDialog = ref(false);
const backupsFilterWorld = ref(null);
const backupsList = ref([]);
const loadingBackups = ref(false);

// Screenshots state
const screenshotsList = ref([]);
const loadingScreenshotsState = ref(false);
const lightboxScreenshot = ref(null);

// Co-Op Session state
const showCoopModal = ref(false);
const showJoinCodeModal = ref(false);
const coopSession = ref(null);
const coopLoading = ref(false);
const codeCopied = ref(false);

function onJoinedViaCode() {
  loadInstances();
}

// New Instance modal tabs & state
const newInstanceTab = ref('modpacks'); // 'modpacks' | 'blank' | 'import'
const modpackSearchQuery = ref('');
const modpackResults = ref([]);
const modpackSearchBusy = ref(false);

const modpackSearchDone = ref(false);
const selectedModpackVersions = ref({});
const installingModpackId = ref('');

const importFilePath = ref('');
const importCustomName = ref('');
const importingModpack = ref(false);

const modpackProgress = ref({
  active: false,
  phase: '',
  current: 0,
  total: 0,
  fraction: 0,
  detail: '',
});


// Mods state
const mods = ref([]);
const selectedMods = ref({});
const modProvider = ref('modrinth'); // 'modrinth' | 'curseforge'
const modSearchQuery = ref('');
const modSearchAllVersions = ref(false);
const modResults = ref([]);
const modSearchBusy = ref(false);
const modSearchDone = ref(false);

// Packs state
const packs = ref({ shaderpacks: [], resourcepacks: [], activeResourcepacks: [] });
const detailedPacks = ref({ shaderpacks: [], resourcepacks: [], shadersEnabled: false });
const activeShaderpack = ref('');

// Shaders search state
const shaderProvider = ref('modrinth');
const shaderSearchQuery = ref('');
const shaderSearchAllVersions = ref(false);
const shaderResults = ref([]);
const shaderSearchBusy = ref(false);
const shaderSearchDone = ref(false);

// Textures search state
const textureProvider = ref('modrinth');
const textureSearchQuery = ref('');
const textureSearchAllVersions = ref(false);
const textureResults = ref([]);
const textureSearchBusy = ref(false);
const textureSearchDone = ref(false);

// Generic installing tracker
const installingId = ref('');
const launching = ref(false);

// CurseForge Countdown & Drop Modal
const curseforgeModal = ref({
  open: false,
  title: '',
  iconUrl: '',
  projectUrl: '',
  targetFileName: '',
  packType: 'mod', // 'mod' | 'shaderpack' | 'resourcepack'
  countdown: 3,
  countdownInterval: null,
  success: false,
  successTitle: '',
});

// New instance modal
const showNewDialog = ref(false);
const creating = ref(false);
const mcVersions = ref([]);
const loaderTypes = ref([]);
const loaderVersions = ref([]);
const recommendedLoaderVersion = ref('');
const loadingLoaderVersions = ref(false);
const newForm = ref({ name: '', mcVersion: '1.20.4', loaderType: 'fabric', loaderVersion: '' });

const selectedModCount = computed(() => {
  const activeSelections = selectedMods.value;
  return activeSelections ? Object.values(activeSelections).filter(Boolean).length : 0;
});
const allModsSelected = computed(() => {
  const currentMods = mods.value;
  if (!Array.isArray(currentMods) || currentMods.length === 0) return false;
  return currentMods.every((entry) => Boolean(selectedMods.value[entry.filename]));
});

function fmtCount(n) {
  if (!n) return '0';
  if (n >= 1_000_000) return `${(n / 1_000_000).toFixed(1)}M`;
  if (n >= 1_000) return `${(n / 1_000).toFixed(1)}K`;
  return String(n);
}

function openExternalLink(url) {
  if (!url) return;
  api.openBrowserUrl(url).catch(() => {
    window.open(url, '_blank', 'noopener,noreferrer');
  });
}

// ---------------------------------------------------------------------------
// Instance Lifecycle
// ---------------------------------------------------------------------------

async function loadInstances() {
  instances.value = await api.listOfflineInstances();
  if (instances.value.length && !selected.value) {
    await selectInstance(instances.value[0]);
  }
}

async function selectInstance(instance) {
  selected.value = instance;
  selectedDir.value = await api.getOfflineInstanceDir(instance.id);
  await Promise.all([loadMods(), loadPacks()]);
}

async function loadMods() {
  if (!selected.value) return;
  mods.value = await api.listOfflineMods(selected.value.id);
  selectedMods.value = {};
}

async function loadPacks() {
  if (!selected.value || !selectedDir.value) return;
  const targetDir = selectedDir.value;
  const [allBasicPacks, allDetailedPacks] = await Promise.all([
    api.listInstancePacks(targetDir),
    api.listInstancePacksDetailed(targetDir),
  ]); // load pack catalogs
  packs.value = allBasicPacks;
  detailedPacks.value = allDetailedPacks;
  activeShaderpack.value = packs.value?.activeShaderpack || '';
}

async function playOffline() {
  if (!selected.value) return;
  launching.value = true;
  emit('launching');
  try {
    await api.launchOfflineInstance(selected.value.id);
  } catch (e) {
    const errMsg = typeof e === 'string' ? e : (e?.message || String(e));
    emit('error', errMsg);
    window.dispatchEvent(new CustomEvent('zircon-status', { detail: `Error: ${errMsg}` }));
    emit('stopped');
  } finally {
    launching.value = false;
    await loadInstances();
  }
}

async function deleteInstance() {
  if (!selected.value) return;
  if (!window.confirm(`Delete '${selected.value.name}' and all of its files?`)) return;
  await api.deleteOfflineInstance(selected.value.id);
  selected.value = null;
  await loadInstances();
}

// ---------------------------------------------------------------------------
// Mods Operations
// ---------------------------------------------------------------------------

async function deleteMod(filename) {
  await api.deleteOfflineMod(selected.value.id, filename);
  await loadMods();
}

function toggleModSelected(filename) {
  if (!filename) return;
  const current = { ...selectedMods.value };
  if (current[filename]) {
    delete current[filename];
  } else {
    current[filename] = true;
  }
  selectedMods.value = current;
}

function toggleSelectAllMods() {
  if (allModsSelected.value) {
    selectedMods.value = {};
    return;
  }
  const selectionMap = {};
  for (const item of mods.value || []) {
    if (item?.filename) {
      selectionMap[item.filename] = true;
    }
  }
  selectedMods.value = selectionMap;
}

async function toggleModEnabled(mod) {
  if (!mod?.filename || !selected.value) return;
  try {
    await api.setOfflineModEnabled(selected.value.id, mod.filename, !mod.enabled);
    await loadMods();
  } catch (err) {
    console.error('Failed to toggle offline mod state:', err);
  }
}

async function bulkEnableSelected() {
  if (!selected.value) return;
  const targetFiles = Object.keys(selectedMods.value || {});
  if (targetFiles.length === 0) return;
  try {
    await Promise.all(targetFiles.map((target) => api.setOfflineModEnabled(selected.value.id, target, true)));
    await loadMods();
  } catch (err) {
    console.error('Failed to bulk enable offline mods:', err);
  }
}

async function bulkDisableSelected() {
  if (!selected.value) return;
  const targetFiles = Object.keys(selectedMods.value || {});
  if (targetFiles.length === 0) return;
  try {
    await Promise.all(targetFiles.map((target) => api.setOfflineModEnabled(selected.value.id, target, false)));
    await loadMods();
  } catch (err) {
    console.error('Failed to bulk disable offline mods:', err);
  }
}

async function bulkDeleteSelected() {
  if (!selected.value) return;
  const targetFiles = Object.keys(selectedMods.value || {});
  if (targetFiles.length === 0) return;
  const count = targetFiles.length;
  const label = count === 1 ? 'mod' : 'mods';
  if (!window.confirm(`Permanently remove ${count} selected ${label}?`)) return;
  try {
    await Promise.all(targetFiles.map((target) => api.deleteOfflineMod(selected.value.id, target)));
    selectedMods.value = {};
    await loadMods();
  } catch (err) {
    console.error('Failed to bulk delete offline mods:', err);
  }
}

async function browseMods() {
  if (!selected.value) return;
  const picked = await pickFiles({ multiple: true, filters: [JAR_FILTER] });
  if (!picked || !picked.length) return;
  for (const path of picked) {
    await api.importOfflineModFile(selected.value.id, path);
  }
  await loadMods();
}

async function onModDrop(event) {
  if (!selected.value) return;
  const files = event.dataTransfer?.files;
  if (!files || !files.length) return;
  for (const file of files) {
    if (!file.name.endsWith('.jar')) continue;
    const arrayBuffer = await file.arrayBuffer();
    const bytes = Array.from(new Uint8Array(arrayBuffer));
    await api.importOfflineModBytes(selected.value.id, file.name, bytes);
  }
  await loadMods();
}

// ---------------------------------------------------------------------------
// Mod Discovery (Search & Install)
// ---------------------------------------------------------------------------

function setModProvider(provider) {
  modProvider.value = provider;
  if (modSearchQuery.value.trim()) {
    searchMods();
  }
}

async function searchMods() {
  const query = modSearchQuery.value.trim();
  if (!query || !selected.value) return;
  modSearchBusy.value = true;
  modSearchDone.value = false;
  try {
    const hits = await api.searchMods(
      selected.value.id,
      query,
      modProvider.value,
      'mod',
      modSearchAllVersions.value
    );
    modResults.value = hits.map((hit) => ({
      ...hit,
      versionOptions: [],
      selectedVersionId: '',
      versionsLoading: true,
      versionsFailed: false,
    }));
    modSearchDone.value = true;
    for (const hit of modResults.value) {
      loadModVersions(hit);
    }
  } catch (e) {
    window.dispatchEvent(new CustomEvent('zircon-status', { detail: `Mod search error: ${e}` }));
  } finally {
    modSearchBusy.value = false;
  }
}

async function loadModVersions(hit) {
  try {
    const versions = await api.listModVersions(
      selected.value.id,
      hit.projectId || hit.id,
      hit.origin || modProvider.value,
      modSearchAllVersions.value
    );
    hit.versionOptions = versions;
    hit.selectedVersionId = versions[0]?.id || '';
  } catch {
    hit.versionsFailed = true;
  } finally {
    hit.versionsLoading = false;
  }
}

const dependencyModalOpen = ref(false);
const pendingDependencyData = ref(null);
const pendingHit = ref(null);

const checkingUpdates = ref(false);
const updatingMods = ref(false);
const availableUpdates = ref([]);
const modUpdateMap = computed(() => {
  const map = {};
  for (const u of availableUpdates.value) {
    map[u.filename] = u;
  }
  return map;
});

async function checkModUpdates() {
  if (!selected.value) return;
  checkingUpdates.value = true;
  try {
    availableUpdates.value = (await api.checkInstanceModUpdates(selected.value.id)) || [];
    if (availableUpdates.value.length === 0) {
      window.dispatchEvent(new CustomEvent('zircon-status', { detail: 'All mods are up to date!' }));
    } else {
      window.dispatchEvent(
        new CustomEvent('zircon-status', { detail: `Found ${availableUpdates.value.length} mod update(s).` })
      );
    }
  } catch (e) {
    window.dispatchEvent(new CustomEvent('zircon-status', { detail: `Update check failed: ${e}` }));
  } finally {
    checkingUpdates.value = false;
  }
}

async function applyModUpdates(updateList) {
  if (!selected.value || !updateList?.length) return;
  updatingMods.value = true;
  try {
    const payloads = updateList.map((u) => ({
      currentFilename: u.filename,
      latestFilename: u.latestFilename,
      downloadUrl: u.downloadUrl,
      sha1: u.sha1,
    }));
    const res = await api.updateInstanceMods(selected.value.id, payloads);
    await loadMods();
    await checkModUpdates();
    window.dispatchEvent(
      new CustomEvent('zircon-status', {
        detail: `Successfully updated ${res.updatedCount} mod(s). Backups created in .mod_staging/backups/`,
      })
    );
  } catch (e) {
    window.dispatchEvent(new CustomEvent('zircon-status', { detail: `Update error: ${e}` }));
  } finally {
    updatingMods.value = false;
  }
}

async function installMod(hit) {
  if (!selected.value) return;
  const id = hit.projectId || hit.id;
  installingId.value = id;
  try {
    // Check dependencies via backend for Modrinth mods
    if (hit.origin !== 'curseforge' && modProvider.value !== 'curseforge') {
      const depCheck = await api.checkModDependencies(selected.value.id, id, hit.selectedVersionId || null);
      if (
        (depCheck.requiredMissing && depCheck.requiredMissing.length > 0) ||
        (depCheck.optionalMissing && depCheck.optionalMissing.length > 0) ||
        (depCheck.incompatibleInstalled && depCheck.incompatibleInstalled.length > 0)
      ) {
        pendingDependencyData.value = depCheck;
        pendingHit.value = hit;
        dependencyModalOpen.value = true;
        return;
      }
    }

    await api.installModrinthPack(selected.value.id, id, hit.selectedVersionId, 'mod');
    await loadMods();
  } catch (e) {
    window.dispatchEvent(new CustomEvent('zircon-status', { detail: `Install failed: ${e}` }));
  } finally {
    installingId.value = '';
  }
}

async function onConfirmDependencies(items) {
  dependencyModalOpen.value = false;
  if (!selected.value) return;
  installingId.value = pendingHit.value ? (pendingHit.value.projectId || pendingHit.value.id) : 'batch';
  try {
    await api.installModWithDependencies(selected.value.id, items);
    await loadMods();
    window.dispatchEvent(
      new CustomEvent('zircon-status', { detail: `Installed ${items.length} mod(s) successfully!` })
    );
  } catch (e) {
    window.dispatchEvent(new CustomEvent('zircon-status', { detail: `Install failed: ${e}` }));
  } finally {
    installingId.value = '';
    pendingDependencyData.value = null;
    pendingHit.value = null;
  }
}

async function onSkipDependencies() {
  dependencyModalOpen.value = false;
  if (!selected.value || !pendingHit.value) return;
  const hit = pendingHit.value;
  const id = hit.projectId || hit.id;
  installingId.value = id;
  try {
    await api.installModrinthPack(selected.value.id, id, hit.selectedVersionId, 'mod');
    await loadMods();
  } catch (e) {
    window.dispatchEvent(new CustomEvent('zircon-status', { detail: `Install failed: ${e}` }));
  } finally {
    installingId.value = '';
    pendingDependencyData.value = null;
    pendingHit.value = null;
  }
}

function onCloseDependencies() {
  dependencyModalOpen.value = false;
  installingId.value = '';
  pendingDependencyData.value = null;
  pendingHit.value = null;
}


// ---------------------------------------------------------------------------
// Shaders & Texture Packs Operations
// ---------------------------------------------------------------------------

async function onShaderpackChange() {
  if (!selectedDir.value) return;
  await api.setActiveShaderpack(selectedDir.value, activeShaderpack.value);
  await loadPacks();
}

async function togglePack(filename) {
  if (!selectedDir.value) return;
  const current = packs.value.activeResourcepacks || [];
  const next = current.includes(filename)
    ? current.filter((f) => f !== filename)
    : [...current, filename];
  await api.setActiveResourcepacks(selectedDir.value, next);
  await loadPacks();
}

async function deletePack(kind, filename) {
  if (!selectedDir.value) return;
  await api.removeLocalPack(selectedDir.value, kind, filename);
  await loadPacks();
}

async function browseShaders() {
  if (!selectedDir.value) return;
  const picked = await pickFiles({ multiple: true, filters: [PACK_FILTER] });
  if (!picked || !picked.length) return;
  for (const p of picked) {
    await api.importInstancePack(selectedDir.value, 'shader', p);
  }
  await loadPacks();
}

async function onShaderDrop(event) {
  if (!selectedDir.value) return;
  const files = event.dataTransfer?.files;
  if (!files || !files.length) return;
  for (const file of files) {
    if (!file.name.endsWith('.zip')) continue;
    const arrayBuffer = await file.arrayBuffer();
    const bytes = Array.from(new Uint8Array(arrayBuffer));
    await api.importInstancePackBytes(selectedDir.value, 'shader', file.name, bytes);
  }
  await loadPacks();
}

async function browseTextures() {
  if (!selectedDir.value) return;
  const picked = await pickFiles({ multiple: true, filters: [PACK_FILTER] });
  if (!picked || !picked.length) return;
  for (const p of picked) {
    await api.importInstancePack(selectedDir.value, 'resource', p);
  }
  await loadPacks();
}

async function onTextureDrop(event) {
  if (!selectedDir.value) return;
  const files = event.dataTransfer?.files;
  if (!files || !files.length) return;
  for (const file of files) {
    if (!file.name.endsWith('.zip')) continue;
    const arrayBuffer = await file.arrayBuffer();
    const bytes = Array.from(new Uint8Array(arrayBuffer));
    await api.importInstancePackBytes(selectedDir.value, 'resource', file.name, bytes);
  }
  await loadPacks();
}

// ---------------------------------------------------------------------------
// Shaders & Texture Packs Discovery
// ---------------------------------------------------------------------------

function setShaderProvider(provider) {
  shaderProvider.value = provider;
  if (shaderSearchQuery.value.trim()) searchShaders();
}

async function searchShaders() {
  const query = shaderSearchQuery.value.trim();
  if (!query || !selected.value) return;
  shaderSearchBusy.value = true;
  shaderSearchDone.value = false;
  try {
    const hits = await api.searchMods(
      selected.value.id,
      query,
      shaderProvider.value,
      'shader',
      shaderSearchAllVersions.value
    );
    shaderResults.value = hits.map((hit) => ({
      ...hit,
      versionOptions: [],
      selectedVersionId: '',
      versionsLoading: true,
      versionsFailed: false,
    }));
    shaderSearchDone.value = true;
    for (const hit of shaderResults.value) {
      loadShaderVersions(hit);
    }
  } catch (e) {
    window.dispatchEvent(new CustomEvent('zircon-status', { detail: `Shader search error: ${e}` }));
  } finally {
    shaderSearchBusy.value = false;
  }
}

async function loadShaderVersions(hit) {
  try {
    const versions = await api.listModVersions(
      selected.value.id,
      hit.projectId || hit.id,
      hit.origin || shaderProvider.value,
      shaderSearchAllVersions.value
    );
    hit.versionOptions = versions;
    hit.selectedVersionId = versions[0]?.id || '';
  } catch {
    hit.versionsFailed = true;
  } finally {
    hit.versionsLoading = false;
  }
}

function setTextureProvider(provider) {
  textureProvider.value = provider;
  if (textureSearchQuery.value.trim()) searchTextures();
}

async function searchTextures() {
  const query = textureSearchQuery.value.trim();
  if (!query || !selected.value) return;
  textureSearchBusy.value = true;
  textureSearchDone.value = false;
  try {
    const hits = await api.searchMods(
      selected.value.id,
      query,
      textureProvider.value,
      'resourcepack',
      textureSearchAllVersions.value
    );
    textureResults.value = hits.map((hit) => ({
      ...hit,
      versionOptions: [],
      selectedVersionId: '',
      versionsLoading: true,
      versionsFailed: false,
    }));
    textureSearchDone.value = true;
    for (const hit of textureResults.value) {
      loadTextureVersions(hit);
    }
  } catch (e) {
    window.dispatchEvent(new CustomEvent('zircon-status', { detail: `Texture search error: ${e}` }));
  } finally {
    textureSearchBusy.value = false;
  }
}

async function loadTextureVersions(hit) {
  try {
    const versions = await api.listModVersions(
      selected.value.id,
      hit.projectId || hit.id,
      hit.origin || textureProvider.value,
      textureSearchAllVersions.value
    );
    hit.versionOptions = versions;
    hit.selectedVersionId = versions[0]?.id || '';
  } catch {
    hit.versionsFailed = true;
  } finally {
    hit.versionsLoading = false;
  }
}

async function installPackItem(hit, packType) {
  if (!selected.value) return;
  const id = hit.projectId || hit.id;
  installingId.value = id;
  try {
    await api.installModrinthPack(selected.value.id, id, hit.selectedVersionId, packType);
    await loadPacks();
  } catch (e) {
    window.dispatchEvent(new CustomEvent('zircon-status', { detail: `Pack install failed: ${e}` }));
  } finally {
    installingId.value = '';
  }
}

// ---------------------------------------------------------------------------
// CurseForge Countdown & Drop Modal Logic
// ---------------------------------------------------------------------------

function openCurseforgeModal(hit, packType) {
  const fileOpt = (hit.versionOptions || []).find((v) => v.id === hit.selectedVersionId);
  const targetUrl = hit.projectUrl || hit.websiteUrl;

  if (curseforgeModal.value.countdownInterval) {
    clearInterval(curseforgeModal.value.countdownInterval);
  }

  curseforgeModal.value = {
    open: true,
    title: hit.title || hit.name,
    iconUrl: hit.iconUrl || '',
    projectUrl: targetUrl,
    targetFileName: fileOpt?.fileName || fileOpt?.file_name || '',
    packType: packType,
    countdown: 3,
    countdownInterval: null,
    success: false,
    successTitle: '',
  };

  curseforgeModal.value.countdownInterval = setInterval(() => {
    if (curseforgeModal.value.countdown > 1) {
      curseforgeModal.value.countdown--;
    } else {
      triggerCurseforgeDownload();
    }
  }, 1000);
}

function triggerCurseforgeDownload() {
  if (curseforgeModal.value.countdownInterval) {
    clearInterval(curseforgeModal.value.countdownInterval);
    curseforgeModal.value.countdownInterval = null;
  }
  curseforgeModal.value.countdown = 0;
  if (curseforgeModal.value.projectUrl) {
    openExternalLink(curseforgeModal.value.projectUrl);
  }
}

function closeCurseforgeModal() {
  if (curseforgeModal.value.countdownInterval) {
    clearInterval(curseforgeModal.value.countdownInterval);
    curseforgeModal.value.countdownInterval = null;
  }
  curseforgeModal.value.open = false;
  curseforgeModal.value.success = false;
}

async function onCurseforgeModalDrop(event) {
  const files = event.dataTransfer?.files;
  if (!files || !files.length || !selected.value) return;
  const file = files[0];
  const arrayBuffer = await file.arrayBuffer();
  const bytes = Array.from(new Uint8Array(arrayBuffer));

  if (curseforgeModal.value.packType === 'mod') {
    await api.importOfflineModBytes(selected.value.id, file.name, bytes);
    await loadMods();
  } else {
    const kind = curseforgeModal.value.packType === 'shaderpack' ? 'shader' : 'resource';
    await api.importInstancePackBytes(selectedDir.value, kind, file.name, bytes);
    await loadPacks();
  }

  curseforgeModal.value.success = true;
  curseforgeModal.value.successTitle = curseforgeModal.value.title;
  setTimeout(() => {
    if (curseforgeModal.value.open) closeCurseforgeModal();
  }, 2500);
}

async function browseCurseforgeModalFile() {
  if (!selected.value) return;
  const isMod = curseforgeModal.value.packType === 'mod';
  const filter = isMod ? JAR_FILTER : PACK_FILTER;
  const picked = await pickFiles({ multiple: false, filters: [filter] });
  if (!picked || !picked.length) return;
  const path = picked[0];

  if (isMod) {
    await api.importOfflineModFile(selected.value.id, path);
    await loadMods();
  } else {
    const kind = curseforgeModal.value.packType === 'shaderpack' ? 'shader' : 'resource';
    await api.importInstancePack(selectedDir.value, kind, path);
    await loadPacks();
  }

  curseforgeModal.value.success = true;
  curseforgeModal.value.successTitle = curseforgeModal.value.title;
  setTimeout(() => {
    if (curseforgeModal.value.open) closeCurseforgeModal();
  }, 2500);
}

// ---------------------------------------------------------------------------
// New Instance Creation & Form
// ---------------------------------------------------------------------------

async function updateLoaderVersions() {
  const mc = newForm.value.mcVersion;
  const loader = newForm.value.loaderType;
  if (!mc || !loader || loader === 'vanilla') {
    loaderVersions.value = [];
    recommendedLoaderVersion.value = '';
    newForm.value.loaderVersion = '';
    return;
  }
  loadingLoaderVersions.value = true;
  try {
    const res = await api.getLoaderVersions(loader, mc);
    loaderVersions.value = res?.versions || [];
    recommendedLoaderVersion.value = res?.recommended || '';
    if (res?.recommended) {
      newForm.value.loaderVersion = res.recommended;
    } else if (loaderVersions.value.length > 0) {
      newForm.value.loaderVersion = loaderVersions.value[0];
    } else {
    }
  } catch (err) {
    console.warn('Failed to fetch loader versions:', err);
    loaderVersions.value = [];
    recommendedLoaderVersion.value = '';
  } finally {
    loadingLoaderVersions.value = false;
  }
}

async function openFolder(subfolder = null) {
  showOpenFolderDropdown.value = false;
  if (!selected.value) return;
  try {
    await api.openInstanceFolder(selected.value.id, subfolder);
  } catch (err) {
    window.dispatchEvent(new CustomEvent('zircon-status', { detail: `Could not open folder: ${err}` }));
  }
}

async function searchModpacks() {
  modpackSearchBusy.value = true;
  modpackSearchDone.value = true;
  try {
    const hits = await api.searchMods('', modpackSearchQuery.value, 'modrinth', 'modpack', true);
    modpackResults.value = hits;
    for (const hit of hits.slice(0, 10)) {
      loadModpackVersions(hit);
    }
  } catch (err) {
    console.error('Modpack search error:', err);
    modpackResults.value = [];
  } finally {
    modpackSearchBusy.value = false;
  }
}

async function loadModpackVersions(hit) {
  const id = hit.projectId || hit.id;
  if (hit.versionOptions || hit.versionsLoading) return;
  hit.versionsLoading = true;
  try {
    const versions = await api.listModVersions('', id, 'modrinth', true);
    hit.versionOptions = versions;
    if (versions && versions.length > 0) {
      selectedModpackVersions.value[id] = versions[0].id;
    }
  } catch {
    hit.versionOptions = [];
  } finally {
    hit.versionsLoading = false;
  }
}

async function installModpack(hit) {
  const id = hit.projectId || hit.id;
  const versionId = selectedModpackVersions.value[id] || null;
  installingModpackId.value = id;
  modpackProgress.value = {
    active: true,
    phase: 'Initializing...',
    current: 0,
    total: 1,
    fraction: 0.05,
    detail: `Preparing ${hit.title || hit.name}`,
  };

  try {
    const inst = await api.installModrinthModpack(id, versionId, hit.title || hit.name);
    await loadInstances();
    if (inst && inst.id) {
      const created = instances.value.find((i) => i.id === inst.id);
      if (created) await selectInstance(created);
    }
    showNewDialog.value = false;
  } catch (err) {
    window.dispatchEvent(new CustomEvent('zircon-status', { detail: `Modpack install error: ${err}` }));
  } finally {
    installingModpackId.value = '';
    modpackProgress.value.active = false;
  }
}

async function browseLocalMrpack() {
  try {
    const picked = await pickFile(MODPACK_FILTER);
    if (picked) {
      importFilePath.value = picked;
      if (!importCustomName.value) {
        const filename = picked.split(/[/\\]/).pop();
        if (filename) {
          importCustomName.value = filename.replace(/\.(mrpack|zip)$/i, '');
        }
      }
    }
  } catch (err) {
    console.warn('Pick file error:', err);
  }
}

async function installLocalMrpack() {
  if (!importFilePath.value) return;
  importingModpack.value = true;
  modpackProgress.value = {
    active: true,
    phase: 'Importing modpack archive...',
    current: 0,
    total: 1,
    fraction: 0.05,
    detail: importFilePath.value,
  };


  try {
    const inst = await api.importLocalMrpack(importFilePath.value, importCustomName.value || null);
    await loadInstances();
    if (inst && inst.id) {
      const created = instances.value.find((i) => i.id === inst.id);
      if (created) await selectInstance(created);
    }
    showNewDialog.value = false;
    importFilePath.value = '';
    importCustomName.value = '';
  } catch (err) {
    window.dispatchEvent(new CustomEvent('zircon-status', { detail: `Import error: ${err}` }));
  } finally {
    importingModpack.value = false;
    modpackProgress.value.active = false;
  }
}

async function openNewInstance() {
  newInstanceTab.value = 'modpacks';
  const defaultMc = mcVersions.value[0] || '1.21.4';
  const defaultLoader = loaderTypes.value[0] || 'fabric';
  newForm.value = {
    name: '',
    mcVersion: defaultMc,
    loaderType: defaultLoader,
    loaderVersion: '',
  };
  showNewDialog.value = true;
  if (modpackResults.value.length === 0) {
    searchModpacks();
  }
  await updateLoaderVersions();
}

async function createInstance() {
  if (!newForm.value.name.trim()) return;
  creating.value = true;
  try {
    const created = await api.createOfflineInstance({
      name: newForm.value.name.trim(),
      minecraftVersion: newForm.value.mcVersion,
      modLoader: {
        type: newForm.value.loaderType,
        version: newForm.value.loaderType === 'vanilla' ? '' : (newForm.value.loaderVersion || ''),
      },
    });
    showNewDialog.value = false;
    await loadInstances();
    if (created && created.id) {
      await selectInstance(created);
    }
  } catch (e) {
    window.dispatchEvent(new CustomEvent('zircon-status', { detail: `Create error: ${e}` }));
  } finally {
    creating.value = false;
  }
}

function onWindowClick(e) {
  if (showOpenFolderDropdown.value && !e.target.closest('.relative')) {
    showOpenFolderDropdown.value = false;
  }
  if (showActionsDropdown.value && !e.target.closest('.relative')) {
    showActionsDropdown.value = false;
  }
}

// ---------------------------------------------------------------------------
// Phase 3: Cloning, Export, Worlds, Backups, Screenshots & Co-Op
// ---------------------------------------------------------------------------

function formatRelativeTime(timestamp) {
  if (!timestamp) return '';
  const now = Date.now();
  const diff = now - timestamp;
  if (diff < 60 * 1000) return 'just now';
  if (diff < 60 * 60 * 1000) return `${Math.floor(diff / (60 * 1000))}m ago`;
  if (diff < 24 * 60 * 60 * 1000) return `${Math.floor(diff / (24 * 60 * 60 * 1000))}h ago`;
  if (diff < 30 * 24 * 60 * 60 * 1000) return `${Math.floor(diff / (24 * 60 * 60 * 1000))}d ago`;
  return new Date(timestamp).toLocaleDateString();
}

function fmtSize(bytes) {
  return fmtBytes(bytes || 0);
}

function promptCloneInstance() {
  showActionsDropdown.value = false;
  if (!selected.value) return;
  cloneName.value = `${selected.value.name} (Copy)`;
  showCloneDialog.value = true;
}

async function executeCloneInstance() {
  if (!selected.value || !cloneName.value.trim()) return;
  cloning.value = true;
  try {
    const cloned = await api.cloneOfflineInstance(selected.value.id, cloneName.value.trim());
    await loadInstances();
    selected.value = cloned;
    showCloneDialog.value = false;
    window.dispatchEvent(
      new CustomEvent('zircon-status', { detail: `Cloned instance to "${cloned.name}" successfully!` })
    );
  } catch (e) {
    window.dispatchEvent(new CustomEvent('zircon-status', { detail: `Clone failed: ${e}` }));
  } finally {
    cloning.value = false;
  }
}

async function exportMrpack() {
  showActionsDropdown.value = false;
  if (!selected.value) return;
  try {
    const defaultName = `${selected.value.name.replace(/[^a-zA-Z0-9_-]/g, '_')}.mrpack`;
    const target = await saveFile({
      defaultPath: defaultName,
      filters: MRPACK_FILTER,
    });
    if (!target) return;
    await api.exportOfflineInstanceMrpack(selected.value.id, target);
    window.dispatchEvent(
      new CustomEvent('zircon-status', { detail: `Exported .mrpack modpack successfully to ${target}` })
    );
  } catch (e) {
    window.dispatchEvent(new CustomEvent('zircon-status', { detail: `Export failed: ${e}` }));
  }
}

async function exportDedicatedServer() {
  showActionsDropdown.value = false;
  if (!selected.value) return;
  try {
    const defaultName = `${selected.value.name.replace(/[^a-zA-Z0-9_-]/g, '_')}-server.zip`;
    const target = await saveFile({
      defaultPath: defaultName,
      filters: ZIP_FILTER,
    });
    if (!target) return;
    await api.exportToZirconServer(selected.value.id, null, target);
    window.dispatchEvent(
      new CustomEvent('zircon-status', { detail: `Exported dedicated server package successfully to ${target}` })
    );
  } catch (e) {
    window.dispatchEvent(new CustomEvent('zircon-status', { detail: `Server export failed: ${e}` }));
  }
}

async function loadWorlds() {
  if (!selected.value) return;
  loadingWorldsState.value = true;
  try {
    worldsList.value = (await api.listInstanceWorlds(selected.value.id)) || [];
  } catch (e) {
    console.error('Failed to load worlds:', e);
  } finally {
    loadingWorldsState.value = false;
  }
}

async function createWorldBackup(folderName) {
  if (!selected.value) return;
  try {
    const filename = await api.backupInstanceWorld(selected.value.id, folderName);
    window.dispatchEvent(
      new CustomEvent('zircon-status', { detail: `Created backup snapshot "${filename}" successfully!` })
    );
  } catch (e) {
    window.dispatchEvent(new CustomEvent('zircon-status', { detail: `Backup failed: ${e}` }));
  }
}

async function openBackupsModal(filterWorld = null) {
  backupsFilterWorld.value = filterWorld;
  showBackupsDialog.value = true;
  await loadBackups();
}

async function loadBackups() {
  if (!selected.value) return;
  loadingBackups.value = true;
  try {
    const all = (await api.listInstanceWorldBackups(selected.value.id)) || [];
    if (backupsFilterWorld.value) {
      backupsList.value = all.filter((b) => b.worldName === backupsFilterWorld.value);
    } else {
      backupsList.value = all;
    }
  } catch (e) {
    console.error('Failed to load backups:', e);
  } finally {
    loadingBackups.value = false;
  }
}

async function restoreBackup(backupFilename) {
  if (!selected.value) return;
  if (!confirm(`Are you sure you want to restore "${backupFilename}"? Any unsaved world progress will be overwritten.`)) {
    return;
  }
  try {
    await api.restoreInstanceWorldBackup(selected.value.id, backupFilename);
    await loadWorlds();
    window.dispatchEvent(
      new CustomEvent('zircon-status', { detail: `Restored snapshot "${backupFilename}" successfully!` })
    );
  } catch (e) {
    window.dispatchEvent(new CustomEvent('zircon-status', { detail: `Restore failed: ${e}` }));
  }
}

async function deleteBackup(backupFilename) {
  if (!selected.value) return;
  try {
    await api.deleteInstanceWorldBackup(selected.value.id, backupFilename);
    await loadBackups();
    window.dispatchEvent(
      new CustomEvent('zircon-status', { detail: `Deleted backup snapshot "${backupFilename}"` })
    );
  } catch (e) {
    window.dispatchEvent(new CustomEvent('zircon-status', { detail: `Delete backup failed: ${e}` }));
  }
}

async function exportWorldZip(folderName) {
  if (!selected.value) return;
  try {
    const defaultName = `${folderName}.zip`;
    const target = await saveFile({
      defaultPath: defaultName,
      filters: ZIP_FILTER,
    });
    if (!target) return;
    const backupName = await api.backupInstanceWorld(selected.value.id, folderName);
    window.dispatchEvent(
      new CustomEvent('zircon-status', { detail: `Exported world snapshot "${backupName}" to backups folder.` })
    );
  } catch (e) {
    window.dispatchEvent(new CustomEvent('zircon-status', { detail: `Export world failed: ${e}` }));
  }
}

async function loadScreenshots() {
  if (!selected.value) return;
  loadingScreenshotsState.value = true;
  try {
    screenshotsList.value = (await api.listInstanceScreenshots(selected.value.id)) || [];
  } catch (e) {
    console.error('Failed to load screenshots:', e);
  } finally {
    loadingScreenshotsState.value = false;
  }
}

function openLightbox(screenshot) {
  lightboxScreenshot.value = screenshot;
}

function closeLightbox() {
  lightboxScreenshot.value = null;
}

async function deleteScreenshot(filename) {
  if (!selected.value) return;
  try {
    await api.deleteInstanceScreenshot(selected.value.id, filename);
    await loadScreenshots();
    if (lightboxScreenshot.value?.filename === filename) {
      closeLightbox();
    }
    window.dispatchEvent(
      new CustomEvent('zircon-status', { detail: `Deleted screenshot "${filename}"` })
    );
  } catch (e) {
    window.dispatchEvent(new CustomEvent('zircon-status', { detail: `Delete screenshot failed: ${e}` }));
  }
}

async function openCoopModal() {
  if (!selected.value) return;
  showCoopModal.value = true;
  codeCopied.value = false;
  coopLoading.value = true;
  try {
    const status = await api.getCoopSessionStatus();
    if (status && status.instanceId === selected.value.id) {
      coopSession.value = status;
    } else {
      const worlds = (await api.listInstanceWorlds(selected.value.id)) || [];
      const worldName = worlds[0]?.folderName || 'New World';
      coopSession.value = await api.startCoopSession(selected.value.id, worldName);
    }
  } catch (e) {
    window.dispatchEvent(new CustomEvent('zircon-status', { detail: `Could not start co-op session: ${e}` }));
  } finally {
    coopLoading.value = false;
  }
}

async function stopCoop() {
  try {
    await api.stopCoopSession();
    coopSession.value = null;
    showCoopModal.value = false;
    window.dispatchEvent(new CustomEvent('zircon-status', { detail: 'Co-op session ended.' }));
  } catch (e) {
    console.error(e);
  }
}

function copyJoinCode() {
  if (!coopSession.value?.joinCode) return;
  navigator.clipboard.writeText(coopSession.value.joinCode);
  codeCopied.value = true;
  setTimeout(() => {
    codeCopied.value = false;
  }, 2000);
}

let unlistenFileDrop;
let unlistenModpackProgress;

onMounted(async () => {
  window.addEventListener('click', onWindowClick);
  try {
    const [mcRes, loadersRes] = await Promise.allSettled([
      api.listMinecraftVersions(),
      api.listLoaderTypes(),
    ]);
    if (mcRes.status === 'fulfilled' && Array.isArray(mcRes.value) && mcRes.value.length) {
      mcVersions.value = mcRes.value;
    } else {
      const meta = await api.getLauncherMetadata().catch(() => null);
      mcVersions.value = meta?.minecraftVersions || [
        '1.21.4', '1.21.3', '1.21.1', '1.21', '1.20.6', '1.20.4',
        '1.20.2', '1.20.1', '1.19.4', '1.19.2', '1.18.2', '1.16.5', '1.12.2'
      ];
    }
    if (loadersRes.status === 'fulfilled' && Array.isArray(loadersRes.value) && loadersRes.value.length) {
      loaderTypes.value = loadersRes.value;
    } else {
      loaderTypes.value = ['fabric', 'quilt', 'forge', 'neoforge', 'vanilla'];
    }
  } catch {
    mcVersions.value = [
      '1.21.4', '1.21.3', '1.21.1', '1.21', '1.20.6', '1.20.4',
      '1.20.2', '1.20.1', '1.19.4', '1.19.2', '1.18.2', '1.16.5', '1.12.2'
    ];
    loaderTypes.value = ['fabric', 'quilt', 'forge', 'neoforge', 'vanilla'];
  }
  await loadInstances();

  try {
    unlistenModpackProgress = await onModpackProgress((p) => {
      modpackProgress.value = {
        active: true,
        phase: p.phase,
        current: p.current,
        total: p.total,
        fraction: p.fraction,
        detail: p.detail,
      };
    });
  } catch (err) {
    console.warn('Modpack progress listener error:', err);
  }

  try {
    const webview = getCurrentWebview();
    unlistenFileDrop = await webview.onDragDropEvent(async (event) => {
      if (event.payload.type === 'drop') {
        const paths = event.payload.paths;
        if (selected.value && paths && paths.length) {
          for (const path of paths) {
            if (path.endsWith('.jar')) {
              await api.importOfflineModFile(selected.value.id, path);
            } else if (path.endsWith('.zip')) {
              await api.importInstancePack(selectedDir.value, 'resource', path);
            }
          }
          await Promise.all([loadMods(), loadPacks()]);
        }
      }
    });
  } catch (err) {
    console.warn('Native drag-drop listener unavailable:', err);
  }
});

onBeforeUnmount(() => {
  window.removeEventListener('click', onWindowClick);
  if (unlistenModpackProgress) unlistenModpackProgress();
  if (unlistenFileDrop) unlistenFileDrop();
  if (curseforgeModal.value.countdownInterval) {
    clearInterval(curseforgeModal.value.countdownInterval);
  }
});
</script>
