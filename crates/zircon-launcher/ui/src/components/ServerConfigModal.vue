<template>
  <div
    v-if="open"
    class="fixed inset-0 z-50 bg-[#070b0f]/85 backdrop-blur-md flex items-center justify-center p-4 select-none"
    @click.self="close"
  >
    <div
      class="z-card w-full max-w-4xl max-h-[90vh] flex flex-col p-6 bg-[#0e1622] border border-slate-700/60 rounded-2xl shadow-2xl overflow-hidden animate-in fade-in zoom-in-95 duration-150"
    >
      <!-- Modal Header -->
      <div class="flex items-center justify-between pb-4 border-b border-slate-800/80 mb-4 shrink-0">
        <div class="min-w-0">
          <div class="flex items-center gap-2.5">
            <h3 class="text-white font-extrabold text-lg truncate">
              {{ server?.name || server?.address || 'Server Configuration' }}
            </h3>
            <span
              v-if="serverData?.hasBom"
              class="px-2 py-0.5 rounded-full text-[10px] font-extrabold uppercase tracking-wider bg-cyan-500/15 text-cyan-300 border border-cyan-500/30"
            >
              Managed Sync
            </span>
          </div>
          <div class="flex items-center gap-2 mt-1 text-xs text-slate-400 font-mono">
            <span class="truncate text-slate-300">{{ server?.address }}</span>
            <template v-if="serverData?.minecraftVersion">
              <span class="text-slate-600">•</span>
              <span class="text-slate-300">MC {{ serverData.minecraftVersion }}</span>
            </template>
            <template v-if="serverData?.loaderType">
              <span class="text-slate-600">•</span>
              <span class="text-cyan-300 capitalize">{{ serverData.loaderType }} {{ serverData.loaderVersion || '' }}</span>
            </template>
          </div>
        </div>

        <div class="flex items-center gap-2 shrink-0">
          <!-- Open Folder split-button dropdown -->
          <div class="relative inline-flex items-stretch rounded-xl overflow-hidden border border-slate-700/80 shadow-sm h-8 bg-slate-900/80">
            <button
              class="hover:bg-slate-800 hover:text-white text-xs px-3 font-semibold text-slate-200 flex items-center gap-1.5 transition-colors"
              @click="openFolder(null)"
              title="Open Server Instance Folder (.minecraft)"
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
              class="absolute right-0 top-9 w-52 bg-[#0c141f] border border-slate-700 rounded-xl shadow-2xl py-1.5 z-50 text-xs text-slate-200"
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

          <button
            class="text-slate-400 hover:text-white p-2 rounded-xl transition-colors hover:bg-slate-800/80 shrink-0"
            title="Close (Esc)"
            @click="close"
          >
            <svg class="w-5 h-5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
              <line x1="18" y1="6" x2="6" y2="18" />
              <line x1="6" y1="6" x2="18" y2="18" />
            </svg>
          </button>
        </div>
      </div>


      <!-- Segmented Tab Navigation -->
      <div class="flex items-center mb-4 shrink-0">
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
            Shaders ({{ detailedPacks.shaderpacks?.length || 0 }})
          </button>
          <button
            type="button"
            class="z-segmented-pill"
            :class="{ 'active': activeTab === 'textures' }"
            @click="activeTab = 'textures'"
          >
            Texture Packs ({{ detailedPacks.resourcepacks?.length || 0 }})
          </button>
        </div>
      </div>

      <!-- Scrollable Tab Content Body -->
      <div class="flex-1 min-h-0 overflow-y-auto pr-1 flex flex-col gap-4">
        <!-- ================= TAB: MODS ================= -->
        <template v-if="activeTab === 'mods'">
          <!-- Installed & BOM Mods -->
          <div class="bg-[#070b10] border border-slate-800/90 rounded-xl p-4 shadow-inner">
            <div class="flex items-center justify-between mb-3">
              <div class="flex items-center gap-2.5">
                <span class="z-section text-white font-bold text-sm">Installed &amp; Server Mods</span>
                <span class="text-xs text-slate-500 font-mono">({{ filteredMods.length }})</span>
                <button
                  class="z-btn-ghost text-xs px-2.5 py-1 rounded-lg border border-slate-700 hover:border-slate-600 flex items-center gap-1.5 transition ml-1"
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
                  {{ availableUpdates.length }} update{{ availableUpdates.length > 1 ? 's' : '' }}
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

              <!-- Filter pills -->

              <div class="flex items-center gap-1 bg-slate-900 border border-slate-800/80 rounded-lg p-0.5 text-xs">
                <button
                  class="px-2.5 py-0.5 rounded-md font-semibold transition"
                  :class="modFilter === 'all' ? 'bg-cyan-500/20 text-cyan-300 font-bold' : 'text-slate-400 hover:text-white'"
                  @click="modFilter = 'all'"
                >
                  All ({{ mods.length }})
                </button>
                <button
                  class="px-2.5 py-0.5 rounded-md font-semibold transition"
                  :class="modFilter === 'server' ? 'bg-cyan-500/20 text-cyan-300 font-bold' : 'text-slate-400 hover:text-white'"
                  @click="modFilter = 'server'"
                >
                  Server ({{ bomModsCount }})
                </button>
                <button
                  class="px-2.5 py-0.5 rounded-md font-semibold transition"
                  :class="modFilter === 'custom' ? 'bg-cyan-500/20 text-cyan-300 font-bold' : 'text-slate-400 hover:text-white'"
                  @click="modFilter = 'custom'"
                >
                  Custom ({{ customModsCount }})
                </button>
              </div>
            </div>

            <!-- Loading State -->
            <div v-if="loadingMods" class="py-12 flex flex-col items-center justify-center text-slate-400 gap-2">
              <span class="inline-block w-6 h-6 border-2 border-accent border-t-transparent rounded-full animate-spin"></span>
              <span class="text-xs">Inspecting instance mods &amp; server BOM...</span>
            </div>

            <!-- Empty List -->
            <div v-else-if="filteredMods.length === 0" class="py-8 text-center text-slate-500 text-xs">
              No mods found in this category. Use search or file drop below to add client mods.
            </div>

            <!-- Mods List -->
            <div v-else class="max-h-[260px] overflow-y-auto flex flex-col gap-1.5 pr-1 mb-3">
              <div
                v-for="mod in filteredMods"
                :key="mod.filename"
                class="flex items-center gap-3 p-2.5 rounded-xl border text-xs transition"
                :class="[
                  mod.isCustom
                    ? 'bg-cyan-950/20 border-cyan-500/30 hover:border-cyan-400/60'
                    : 'bg-slate-900/70 border-slate-800/80 hover:border-slate-700',
                  { 'opacity-55': !mod.enabled }
                ]"
              >
                <!-- Mod Icon -->
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

                <!-- Mod Details -->
                <div class="flex-1 min-w-0">
                  <div class="flex items-center gap-1.5 truncate">
                    <span class="truncate text-white font-medium">{{ mod.name || mod.filename }}</span>
                    <span
                      v-if="mod.version"
                      class="bg-slate-950 text-slate-400 border border-slate-800 text-[10px] px-1.5 py-0.2 rounded font-mono shrink-0"
                    >
                      {{ mod.version }}
                    </span>
                  </div>
                  <div class="flex items-center gap-2 mt-0.5 text-[10px] text-slate-500">
                    <span v-if="mod.author" class="truncate">by {{ mod.author }}</span>
                    <span v-if="mod.author">•</span>
                    <span class="font-mono">{{ fmtBytes(mod.sizeBytes) }}</span>
                  </div>
                </div>

                <!-- Origin & Sync Status Badges -->
                <div class="flex items-center gap-1.5 shrink-0">
                  <span
                    v-if="mod.isBom"
                    class="px-2 py-0.5 rounded text-[9px] font-bold uppercase tracking-wider bg-slate-800/90 text-slate-300 border border-slate-700"
                    title="Required or recommended by server"
                  >
                    Server
                  </span>
                  <span
                    v-if="mod.isCustom"
                    class="px-2 py-0.5 rounded text-[9px] font-bold uppercase tracking-wider bg-cyan-500/20 text-cyan-200 border border-cyan-400/40 shadow-sm"
                    title="Player-added client mod"
                  >
                    Custom
                  </span>
                  <span
                    v-if="!mod.isDownloaded"
                    class="px-2 py-0.5 rounded text-[9px] font-bold uppercase tracking-wider bg-amber-500/15 text-amber-300 border border-amber-500/30"
                    title="Will download automatically on next launch"
                  >
                    Pending Sync
                  </span>
                </div>

                <!-- Controls -->
                <div class="flex items-center gap-2 shrink-0">
                  <!-- Custom Mod Toggle -->
                  <template v-if="mod.isCustom">
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
                      class="z-toggle"

                      :class="{ 'z-toggle-on': mod.enabled }"
                      :title="mod.enabled ? 'Disable Mod' : 'Enable Mod'"
                      @click="toggleModEnabled(mod)"
                    >
                      <span class="z-toggle-thumb"></span>
                    </button>
                    <button
                      class="text-red-400 hover:text-red-300 text-xs px-2 py-0.5 hover:bg-red-500/10 rounded-lg transition-colors font-medium flex items-center gap-1 shrink-0"
                      title="Delete Custom Mod"
                      @click="deleteMod(mod.filename)"
                    >
                      <svg class="w-3.5 h-3.5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8">
                        <path d="M3 6h18M19 6v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6M8 6V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2" />
                      </svg>
                      <span>Delete</span>
                    </button>
                  </template>

                  <!-- Server BOM Mod (Lock Indicator) -->
                  <template v-else>
                    <div class="text-slate-600 px-2 py-1 text-xs" title="Server mods are managed automatically">
                      <svg class="w-3.5 h-3.5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                        <rect x="3" y="11" width="18" height="11" rx="2" ry="2" />
                        <path d="M7 11V7a5 5 0 0 1 10 0v4" />
                      </svg>
                    </div>
                  </template>
                </div>
              </div>
            </div>

            <!-- Custom Upload / Drop Zone -->
            <div
              class="zircon-drop-zone p-4 text-center text-xs text-slate-400 cursor-pointer rounded-xl border border-dashed border-slate-800 hover:border-cyan-500/50 transition"
              @dragover.prevent
              @drop.prevent="onModDrop"
            >
              Drop <code class="text-cyan-300 font-mono">.jar</code> mod files here (or
              <button class="text-cyan-400 underline font-semibold hover:text-cyan-300" @click="browseMods">browse files</button>)
            </div>
          </div>

          <!-- Mod Discovery (Modrinth & CurseForge) -->
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

            <!-- Search input bar -->
            <div class="flex gap-2">
              <input
                v-model="modSearchQuery"
                class="z-input flex-1 text-xs"
                :placeholder="`Search ${modProvider === 'curseforge' ? 'CurseForge' : 'Modrinth'} (e.g. Sodium, Iris, JourneyMap)...`"
                @keydown.enter="searchMods"
              />
              <button
                class="z-btn-ghost px-4 text-xs font-bold shrink-0"
                :disabled="modSearchBusy"
                @click="searchMods"
              >
                Search
              </button>
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
            <div v-if="modResults.length" class="mt-3 flex flex-col gap-2 max-h-[280px] overflow-y-auto pr-1">
              <div
                v-for="hit in modResults"
                :key="hit.projectId || hit.id"
                class="bg-slate-900/70 border border-slate-800 rounded-xl p-3 flex flex-col gap-2 transition hover:border-slate-700"
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
                        @click="openExternalLink(hit.projectUrl || hit.websiteUrl)"
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
                    <div class="flex items-center gap-3 text-[10px] text-slate-400 font-mono">
                      <span v-if="hit.author">by {{ hit.author }}</span>
                      <span v-if="hit.downloads">⬇ {{ fmtCount(hit.downloads) }}</span>
                      <a
                        v-if="hit.projectUrl || hit.websiteUrl"
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

                <!-- Version selector -->
                <div v-if="hit.versionOptions && hit.versionOptions.length > 0" class="flex items-center gap-2 pt-2 border-t border-slate-800/60">
                  <span class="text-[10px] text-slate-400 shrink-0 font-medium">Version:</span>
                  <select
                    v-model="hit.selectedVersionId"
                    class="bg-slate-950 border border-slate-800 text-slate-200 text-[11px] rounded-lg px-2 py-1 flex-1 min-w-0 font-mono focus:border-cyan-400 outline-none"
                  >
                    <option v-for="ver in hit.versionOptions" :key="ver.id" :value="ver.id">
                      {{ ver.name || ver.versionNumber }} ({{ ver.fileName || ver.id }})
                    </option>
                  </select>
                </div>
              </div>
            </div>

            <div v-else-if="modSearchDone" class="mt-4 text-center text-slate-500 text-xs py-4">
              No mods found for query on {{ modProvider === 'curseforge' ? 'CurseForge' : 'Modrinth' }}.
            </div>
          </div>
        </template>

        <!-- ================= TAB: SHADERS ================= -->
        <template v-if="activeTab === 'shaders'">
          <div class="bg-[#070b10] border border-slate-800/90 rounded-xl p-4 shadow-inner flex flex-col gap-4">
            <div class="flex items-center justify-between">
              <div>
                <div class="z-section text-white font-bold text-sm">Shaders</div>
                <div class="text-xs text-slate-400 mt-0.5">Select active shaderpack or drop custom ones below.</div>
              </div>
              <div v-if="packs.activeShaderpack" class="text-xs text-cyan-300 font-mono font-bold flex items-center gap-1.5">
                <span class="w-1.5 h-1.5 rounded-full bg-cyan-400 animate-pulse"></span>
                Active: {{ packs.activeShaderpack }}
              </div>
            </div>

            <!-- Shaderpack Selector Cards -->
            <div class="grid grid-cols-2 gap-2">
              <div
                class="p-3 rounded-xl border cursor-pointer transition flex items-center justify-between"
                :class="!activeShaderpack ? 'border-cyan-400 bg-cyan-500/10 text-cyan-300' : 'border-slate-800 bg-slate-900/60 text-slate-400 hover:border-slate-700'"
                @click="setShaderpack('')"
              >
                <div class="text-xs font-bold">None (Shaders Disabled)</div>
                <span v-if="!activeShaderpack" class="text-cyan-400 font-bold text-xs">✓</span>
              </div>

              <div
                v-for="sp in (detailedPacks.shaderpacks || [])"
                :key="sp.filename"
                class="p-3 rounded-xl border cursor-pointer transition flex items-center justify-between gap-2"
                :class="activeShaderpack === sp.filename ? 'border-cyan-400 bg-cyan-500/10 text-white' : 'border-slate-800 bg-slate-900/60 text-slate-300 hover:border-slate-700'"
                @click="setShaderpack(sp.filename)"
              >
                <div class="min-w-0">
                  <div class="text-xs font-bold truncate">{{ sp.title || sp.filename }}</div>
                  <div v-if="sp.author" class="text-[10px] text-slate-500 truncate">by {{ sp.author }}</div>
                </div>
                <div class="flex items-center gap-2 shrink-0">
                  <span v-if="activeShaderpack === sp.filename" class="text-cyan-400 font-bold text-xs">✓</span>
                  <button
                    class="text-red-400 hover:text-red-300 text-xs px-2 py-0.5 hover:bg-red-500/10 rounded-lg transition-colors font-medium flex items-center gap-1 shrink-0"
                    title="Delete Shaderpack"
                    @click.stop="deletePack('shader', sp.filename)"
                  >
                    <svg class="w-3.5 h-3.5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8">
                      <path d="M3 6h18M19 6v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6M8 6V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2" />
                    </svg>
                    <span>Delete</span>
                  </button>
                </div>
              </div>
            </div>

            <!-- Custom Drop Zone -->
            <div
              class="zircon-drop-zone p-4 text-center text-xs text-slate-400 cursor-pointer rounded-xl border border-dashed border-slate-800 hover:border-cyan-500/50 transition"
              @dragover.prevent
              @drop.prevent="onShaderDrop"
            >
              Drop <code class="text-cyan-300 font-mono">.zip</code> shaderpacks here (or
              <button class="text-cyan-400 underline font-semibold hover:text-cyan-300" @click="browseShaders">browse files</button>)
            </div>
          </div>
        </template>

        <!-- ================= TAB: TEXTURE PACKS ================= -->
        <template v-if="activeTab === 'textures'">
          <div class="bg-[#070b10] border border-slate-800/90 rounded-xl p-4 shadow-inner flex flex-col gap-4">
            <div class="flex items-center justify-between">
              <div>
                <div class="z-section text-white font-bold text-sm">Resource &amp; Texture Packs</div>
                <div class="text-xs text-slate-400 mt-0.5">Toggle packs to apply them to your server instance.</div>
              </div>
            </div>

            <!-- Texture Packs List -->
            <div v-if="detailedPacks.resourcepacks && detailedPacks.resourcepacks.length" class="flex flex-col gap-2">
              <div
                v-for="rp in detailedPacks.resourcepacks"
                :key="rp.filename"
                class="flex items-center gap-3 p-3 rounded-xl bg-slate-900/60 border border-slate-800 transition hover:border-slate-700"
              >
                <div class="w-10 h-10 rounded-lg bg-slate-950 border border-slate-800 flex items-center justify-center shrink-0 overflow-hidden shadow-inner">
                  <img v-if="rp.iconDataUrl" :src="rp.iconDataUrl" class="w-full h-full object-cover" />
                  <svg v-else class="w-5 h-5 text-cyan-400/60" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8">
                    <rect x="3" y="3" width="18" height="18" rx="2" ry="2" />
                    <line x1="3" y1="9" x2="21" y2="9" />
                    <line x1="9" y1="21" x2="9" y2="9" />
                  </svg>
                </div>

                <div class="flex-1 min-w-0">
                  <div class="text-xs font-bold text-white truncate">{{ rp.title || rp.filename }}</div>
                  <div v-if="rp.description" class="text-[11px] text-slate-400 line-clamp-1 mt-0.5">{{ rp.description }}</div>
                </div>

                <div class="flex items-center gap-3 shrink-0">
                  <button
                    class="z-toggle"
                    :class="{ 'z-toggle-on': isPackActive(rp.filename) }"
                    :title="isPackActive(rp.filename) ? 'Deactivate' : 'Activate'"
                    @click="toggleTexturePack(rp.filename)"
                  >
                    <span class="z-toggle-thumb"></span>
                  </button>
                  <button
                    class="text-red-400 hover:text-red-300 text-xs px-2 py-0.5 hover:bg-red-500/10 rounded-lg transition-colors font-medium flex items-center gap-1 shrink-0"
                    title="Delete Resource Pack"
                    @click="deletePack('resource', rp.filename)"
                  >
                    <svg class="w-3.5 h-3.5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8">
                      <path d="M3 6h18M19 6v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6M8 6V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2" />
                    </svg>
                    <span>Delete</span>
                  </button>
                </div>
              </div>
            </div>

            <div v-else class="py-6 text-center text-slate-500 text-xs">
              No texture packs installed. Drop a .zip archive below to add one.
            </div>

            <!-- Custom Drop Zone -->
            <div
              class="zircon-drop-zone p-4 text-center text-xs text-slate-400 cursor-pointer rounded-xl border border-dashed border-slate-800 hover:border-cyan-500/50 transition"
              @dragover.prevent
              @drop.prevent="onTextureDrop"
            >
              Drop <code class="text-cyan-300 font-mono">.zip</code> texture packs here (or
              <button class="text-cyan-400 underline font-semibold hover:text-cyan-300" @click="browseTextures">browse files</button>)
            </div>
          </div>
        </template>
      </div>
    </div>

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
import { computed, ref, watch } from 'vue';
import DependencyPromptModal from './DependencyPromptModal.vue';
import { api, pickFiles } from '../lib/api';


const props = defineProps({
  server: { type: Object, default: null },
  open: { type: Boolean, default: false },
});

const emit = defineEmits(['close']);

const JAR_FILTER = { name: 'Minecraft Mod (.jar)', extensions: ['jar'] };
const PACK_FILTER = { name: 'Zip Archive (.zip)', extensions: ['zip'] };

const activeTab = ref('mods');
const serverData = ref(null);
const mods = ref([]);
const loadingMods = ref(false);
const modFilter = ref('all'); // 'all' | 'server' | 'custom'
const showOpenFolderDropdown = ref(false);

async function openFolder(subfolder = null) {
  showOpenFolderDropdown.value = false;
  if (!props.server?.address) return;
  try {
    await api.openInstanceFolder(`server:${props.server.address}`, subfolder);
  } catch (err) {
    console.warn('Could not open folder:', err);
  }
}


// Shaders & Texture Packs state
const packs = ref({ activeShaderpack: '', activeResourcepacks: [] });
const detailedPacks = ref({ shaderpacks: [], resourcepacks: [] });
const activeShaderpack = ref('');

// Discovery state
const modProvider = ref('modrinth'); // 'modrinth' | 'curseforge'
const modSearchQuery = ref('');
const modSearchAllVersions = ref(false);
const modSearchBusy = ref(false);
const modSearchDone = ref(false);
const modResults = ref([]);
const installingId = ref('');

const bomModsCount = computed(() => mods.value.filter((m) => m.isBom).length);
const customModsCount = computed(() => mods.value.filter((m) => m.isCustom).length);

const filteredMods = computed(() => {
  if (modFilter.value === 'server') return mods.value.filter((m) => m.isBom);
  if (modFilter.value === 'custom') return mods.value.filter((m) => m.isCustom);
  return mods.value;
});

watch(
  () => props.open,
  async (isOpen) => {
    if (isOpen && props.server?.address) {
      await loadServerMods();
    }
  },
  { immediate: true }
);

function close() {
  emit('close');
}

async function loadServerMods() {
  if (!props.server?.address) return;
  loadingMods.value = true;
  try {
    const res = await api.getServerInstanceMods(props.server.address);
    serverData.value = res;
    mods.value = res.mods || [];
    if (res.gameDir) {
      await loadPacks(res.gameDir);
    }
  } catch (err) {
    console.error('Failed to load server mods:', err);
    window.dispatchEvent(
      new CustomEvent('zircon-status', { detail: `Error loading server configuration: ${err}` })
    );
  } finally {
    loadingMods.value = false;
  }
}

async function loadPacks(gameDir) {
  const dir = gameDir || serverData.value?.gameDir;
  if (!dir) return;
  try {
    const [basicPacks, detailed] = await Promise.all([
      api.listInstancePacks(dir),
      api.listInstancePacksDetailed(dir),
    ]);
    packs.value = basicPacks;
    detailedPacks.value = detailed;
    activeShaderpack.value = basicPacks.activeShaderpack || '';
  } catch (err) {
    console.warn('Failed to load packs:', err);
  }
}

async function toggleModEnabled(mod) {
  if (!props.server?.address) return;
  try {
    await api.setServerModEnabled(props.server.address, mod.filename, !mod.enabled);
    await loadServerMods();
  } catch (err) {
    window.dispatchEvent(new CustomEvent('zircon-status', { detail: `Toggle failed: ${err}` }));
  }
}

async function deleteMod(filename) {
  if (!props.server?.address) return;
  if (!window.confirm(`Delete custom mod '${filename}'?`)) return;
  try {
    await api.deleteServerMod(props.server.address, filename);
    await loadServerMods();
  } catch (err) {
    window.dispatchEvent(new CustomEvent('zircon-status', { detail: `Delete failed: ${err}` }));
  }
}

async function browseMods() {
  if (!props.server?.address) return;
  const picked = await pickFiles({ multiple: true, filters: [JAR_FILTER] });
  if (!picked || !picked.length) return;
  for (const path of picked) {
    await api.addServerModFile(props.server.address, path);
  }
  await loadServerMods();
}

async function onModDrop(event) {
  if (!props.server?.address) return;
  const files = event.dataTransfer?.files;
  if (!files || !files.length) return;
  for (const file of files) {
    if (!file.name.endsWith('.jar')) continue;
    const arrayBuffer = await file.arrayBuffer();
    const bytes = Array.from(new Uint8Array(arrayBuffer));
    await api.addServerModBytes(props.server.address, file.name, bytes);
  }
  await loadServerMods();
}

// -------------------------------------------------------------
// Mod Discovery
// -------------------------------------------------------------

function setModProvider(provider) {
  modProvider.value = provider;
  if (modSearchQuery.value.trim()) {
    searchMods();
  }
}

async function searchMods() {
  const query = modSearchQuery.value.trim();
  if (!query || !props.server?.address) return;
  modSearchBusy.value = true;
  modSearchDone.value = false;
  try {
    const hits = await api.searchMods(
      `server:${props.server.address}`,
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
  } catch (err) {
    window.dispatchEvent(new CustomEvent('zircon-status', { detail: `Mod search error: ${err}` }));
  } finally {
    modSearchBusy.value = false;
  }
}

async function loadModVersions(hit) {
  if (!props.server?.address) return;
  try {
    const versions = await api.listModVersions(
      `server:${props.server.address}`,
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
  if (!props.server?.address) return;
  checkingUpdates.value = true;
  try {
    const instanceId = `server:${props.server.address}`;
    availableUpdates.value = (await api.checkInstanceModUpdates(instanceId)) || [];
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
  if (!props.server?.address || !updateList?.length) return;
  updatingMods.value = true;
  try {
    const instanceId = `server:${props.server.address}`;
    const payloads = updateList.map((u) => ({
      currentFilename: u.filename,
      latestFilename: u.latestFilename,
      downloadUrl: u.downloadUrl,
      sha1: u.sha1,
    }));
    const res = await api.updateInstanceMods(instanceId, payloads);
    await loadServerMods();
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
  if (!props.server?.address) return;
  const id = hit.projectId || hit.id;
  installingId.value = id;
  const instanceId = `server:${props.server.address}`;
  try {
    if (hit.origin !== 'curseforge' && modProvider.value !== 'curseforge') {
      const depCheck = await api.checkModDependencies(instanceId, id, hit.selectedVersionId || null);
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

    await api.installServerModrinthMod(props.server.address, id, hit.selectedVersionId);
    await loadServerMods();
    window.dispatchEvent(
      new CustomEvent('zircon-status', { detail: `Installed ${hit.title || hit.name} successfully!` })
    );
  } catch (err) {
    window.dispatchEvent(new CustomEvent('zircon-status', { detail: `Install failed: ${err}` }));
  } finally {
    installingId.value = '';
  }
}

async function onConfirmDependencies(items) {
  dependencyModalOpen.value = false;
  if (!props.server?.address) return;
  installingId.value = pendingHit.value ? (pendingHit.value.projectId || pendingHit.value.id) : 'batch';
  const instanceId = `server:${props.server.address}`;
  try {
    await api.installModWithDependencies(instanceId, items);
    await loadServerMods();
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
  if (!props.server?.address || !pendingHit.value) return;
  const hit = pendingHit.value;
  const id = hit.projectId || hit.id;
  installingId.value = id;
  try {
    await api.installServerModrinthMod(props.server.address, id, hit.selectedVersionId);
    await loadServerMods();
    window.dispatchEvent(
      new CustomEvent('zircon-status', { detail: `Installed ${hit.title || hit.name} successfully!` })
    );
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


// -------------------------------------------------------------
// Shaders & Texture Packs Actions
// -------------------------------------------------------------

async function setShaderpack(filename) {
  const dir = serverData.value?.gameDir;
  if (!dir) return;
  activeShaderpack.value = filename;
  await api.setActiveShaderpack(dir, filename);
  await loadPacks(dir);
}

function isPackActive(filename) {
  return (packs.value.activeResourcepacks || []).includes(filename);
}

async function toggleTexturePack(filename) {
  const dir = serverData.value?.gameDir;
  if (!dir) return;
  const current = packs.value.activeResourcepacks || [];
  const next = current.includes(filename)
    ? current.filter((f) => f !== filename)
    : [...current, filename];
  await api.setActiveResourcepacks(dir, next);
  await loadPacks(dir);
}

async function deletePack(kind, filename) {
  const dir = serverData.value?.gameDir;
  if (!dir) return;
  await api.removeLocalPack(dir, kind, filename);
  await loadPacks(dir);
}

async function browseShaders() {
  const dir = serverData.value?.gameDir;
  if (!dir) return;
  const picked = await pickFiles({ multiple: true, filters: [PACK_FILTER] });
  if (!picked || !picked.length) return;
  for (const p of picked) {
    await api.importInstancePack(dir, 'shader', p);
  }
  await loadPacks(dir);
}

async function onShaderDrop(event) {
  const dir = serverData.value?.gameDir;
  if (!dir) return;
  const files = event.dataTransfer?.files;
  if (!files || !files.length) return;
  for (const file of files) {
    if (!file.name.endsWith('.zip')) continue;
    const arrayBuffer = await file.arrayBuffer();
    const bytes = Array.from(new Uint8Array(arrayBuffer));
    await api.importInstancePackBytes(dir, 'shader', file.name, bytes);
  }
  await loadPacks(dir);
}

async function browseTextures() {
  const dir = serverData.value?.gameDir;
  if (!dir) return;
  const picked = await pickFiles({ multiple: true, filters: [PACK_FILTER] });
  if (!picked || !picked.length) return;
  for (const p of picked) {
    await api.importInstancePack(dir, 'resource', p);
  }
  await loadPacks(dir);
}

async function onTextureDrop(event) {
  const dir = serverData.value?.gameDir;
  if (!dir) return;
  const files = event.dataTransfer?.files;
  if (!files || !files.length) return;
  for (const file of files) {
    if (!file.name.endsWith('.zip')) continue;
    const arrayBuffer = await file.arrayBuffer();
    const bytes = Array.from(new Uint8Array(arrayBuffer));
    await api.importInstancePackBytes(dir, 'resource', file.name, bytes);
  }
  await loadPacks(dir);
}

function openExternalLink(url) {
  if (!url) return;
  api.openBrowserUrl(url).catch(() => {
    window.open(url, '_blank', 'noopener,noreferrer');
  });
}

function fmtBytes(bytes) {
  if (!bytes) return '0 B';
  const units = ['B', 'KB', 'MB', 'GB'];
  let size = bytes;
  let unitIndex = 0;
  while (size >= 1024 && unitIndex < units.length - 1) {
    size /= 1024;
    unitIndex++;
  }
  return `${size.toFixed(1)} ${units[unitIndex]}`;
}

function fmtCount(n) {
  if (!n) return '0';
  if (n >= 1_000_000) return `${(n / 1_000_000).toFixed(1)}M`;
  if (n >= 1_000) return `${(n / 1_000).toFixed(1)}K`;
  return String(n);
}
</script>
