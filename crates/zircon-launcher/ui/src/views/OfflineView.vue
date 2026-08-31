<template>
  <div class="h-full flex gap-5 p-5 overflow-hidden">
    <!-- Left: instance list -->
    <div class="w-[300px] min-w-[300px] z-card flex flex-col p-4 bg-[#0e1622]/90 border border-slate-800/80">
      <div class="flex items-center justify-between mb-4">
        <span class="z-section text-white font-bold">Offline Instances</span>
        <button class="z-btn-accent text-xs font-bold px-3 py-1.5 rounded-xl shadow-md hover:shadow-cyan-500/25" @click="openNewInstance">+ New Instance</button>
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
            class="w-10 h-10 rounded-xl bg-gradient-to-br from-[#5adfd5] via-[#47d2c9] to-[#20b2aa] text-[#022623] font-black flex items-center justify-center text-base shrink-0 shadow-[0_0_10px_rgba(71,210,201,0.25)]"
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
          No offline instances yet.
        </div>
      </div>
    </div>

    <!-- Right: instance detail & management -->
    <div class="flex-1 min-w-0 z-card flex flex-col p-5 bg-[#0e1622]/90 border border-slate-800/80">
      <template v-if="selected">
        <!-- Header summary & play action -->
        <div class="bg-[#070b10] border border-slate-800/90 rounded-xl p-4 shadow-inner mb-4">
          <div class="flex items-center justify-between gap-4">
            <div class="min-w-0">
              <div class="text-white font-bold text-base truncate">{{ selected.name }}</div>
              <div class="flex items-center gap-3 text-xs text-slate-400 mt-1 font-mono">
                <span>Minecraft <strong class="text-white">{{ selected.minecraftVersion }}</strong></span>
                <span>·</span>
                <span>Loader <strong class="text-cyan-300 capitalize">{{ selected.modLoader.type }} {{ selected.modLoader.version }}</strong></span>
              </div>
            </div>
            <div class="flex items-center gap-2 shrink-0">
              <button class="z-btn-accent px-5 py-2 rounded-xl font-bold shadow-md hover:shadow-cyan-500/25" :disabled="launching" @click="playOffline">
                <span v-if="launching" class="inline-flex items-center gap-2">
                  <span class="inline-block w-3.5 h-3.5 border-2 border-[#022623] border-t-transparent rounded-full animate-spin"></span>
                  LAUNCHING…
                </span>
                <span v-else>Play Offline</span>
              </button>
              <button class="z-btn-danger text-xs px-3.5 py-2 rounded-xl font-bold" @click="deleteInstance">Delete</button>
            </div>
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
            </div>
          </div>
        </div>

        <!-- Tab contents (scrollable body) -->
        <div class="flex-1 min-h-0 overflow-y-auto pr-1 flex flex-col gap-4">
          <!-- ================= TAB: MODS ================= -->
          <template v-if="activeTab === 'mods'">
            <div class="bg-[#070b10] border border-slate-800/90 rounded-xl p-4 shadow-inner">
              <div class="flex items-center justify-between mb-3">
                <div class="z-section text-white font-bold text-sm">Installed Mods ({{ mods.length }})</div>
                <label
                  v-if="mods.length"
                  class="flex items-center gap-2 text-xs text-slate-400 cursor-pointer select-none"
                >
                  <input
                    type="checkbox"
                    class="zircon-check"
                    :checked="allModsSelected"
                    @change="toggleSelectAllMods"
                  />
                  Select All
                </label>
              </div>

              <!-- Bulk selection bar -->
              <div
                v-if="selectedModCount > 0"
                class="flex items-center gap-2 mb-3 bg-slate-900 border border-slate-700/80 rounded-xl px-3 py-2 text-xs"
              >
                <span class="text-slate-400 font-medium">{{ selectedModCount }} selected</span>
                <div class="flex-1"></div>
                <button class="z-btn-ghost text-[10px] px-2.5 py-1 rounded-lg font-semibold" @click="bulkEnableSelected">Enable</button>
                <button class="z-btn-ghost text-[10px] px-2.5 py-1 rounded-lg font-semibold" @click="bulkDisableSelected">Disable</button>
                <button class="text-[10px] px-2.5 py-1 text-red-400 hover:text-red-300 font-semibold" @click="bulkDeleteSelected">Delete</button>
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
                    class="zircon-check shrink-0"
                    :checked="!!selectedMods[mod.filename]"
                    @change="toggleModSelected(mod.filename)"
                  />
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
                    class="z-toggle shrink-0"
                    :class="{ 'z-toggle-on': mod.enabled }"
                    :title="mod.enabled ? 'Disable' : 'Enable'"
                    @click="toggleModEnabled(mod)"
                  >
                    <span class="z-toggle-thumb"></span>
                  </button>
                  <button class="text-slate-400 hover:text-red-400 text-xs px-1.5 py-0.5 transition shrink-0" title="Delete" @click="deleteMod(mod.filename)">✕</button>
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
                          class="z-btn-accent text-[10px] px-3 py-1 font-bold shrink-0 rounded-lg bg-[#f16436] hover:bg-[#ff7547] text-white border-none shadow-none"
                          @click="openCurseforgeModal(hit, 'mod')"
                        >
                          Get on CurseForge
                        </button>
                        <button
                          v-else
                          class="z-btn-ghost text-[10px] px-3 py-1 font-bold shrink-0 rounded-lg"
                          :disabled="installingId === (hit.projectId || hit.id) || hit.versionsLoading"
                          @click="installMod(hit)"
                        >
                          {{ installingId === (hit.projectId || hit.id) ? 'Installing…' : 'Install' }}
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
                          class="text-cyan-400 hover:underline inline-flex items-center gap-0.5"
                          @click.prevent="openExternalLink(hit.projectUrl || hit.websiteUrl)"
                        >
                          {{ (hit.origin === 'curseforge' || modProvider === 'curseforge') ? 'View on CurseForge ↗' : 'View on Modrinth ↗' }}
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
                  <button class="text-slate-400 hover:text-red-400 text-xs px-1.5 py-0.5 transition" title="Delete" @click="deletePack('shader', p.filename)">✕</button>
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
                          class="z-btn-accent text-[10px] px-3 py-1 font-bold shrink-0 rounded-lg bg-[#f16436] hover:bg-[#ff7547] text-white border-none shadow-none"
                          @click="openCurseforgeModal(hit, 'shaderpack')"
                        >
                          Get on CurseForge
                        </button>
                        <button
                          v-else
                          class="z-btn-ghost text-[10px] px-3 py-1 font-bold shrink-0 rounded-lg"
                          :disabled="installingId === (hit.projectId || hit.id) || hit.versionsLoading"
                          @click="installPackItem(hit, 'shader')"
                        >
                          {{ installingId === (hit.projectId || hit.id) ? 'Installing…' : 'Install' }}
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
                          class="text-cyan-400 hover:underline inline-flex items-center gap-0.5"
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
                  <button class="text-slate-400 hover:text-red-400 text-xs px-1.5 py-0.5 transition shrink-0" title="Delete" @click.stop.prevent="deletePack('resource', p.filename)">✕</button>
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
                  Searching…
                </span>
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
                          class="z-btn-accent text-[10px] px-3 py-1 font-bold shrink-0 rounded-lg bg-[#f16436] hover:bg-[#ff7547] text-white border-none shadow-none"
                          @click="openCurseforgeModal(hit, 'resourcepack')"
                        >
                          Get on CurseForge
                        </button>
                        <button
                          v-else
                          class="z-btn-ghost text-[10px] px-3 py-1 font-bold shrink-0 rounded-lg"
                          :disabled="installingId === (hit.projectId || hit.id) || hit.versionsLoading"
                          @click="installPackItem(hit, 'resourcepack')"
                        >
                          {{ installingId === (hit.projectId || hit.id) ? 'Installing…' : 'Install' }}
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
                          class="text-cyan-400 hover:underline inline-flex items-center gap-0.5"
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
        </div>
      </template>

      <div v-else class="flex-1 flex items-center justify-center text-slate-500 text-sm">
        Select an offline instance to manage mods, shaders &amp; texture packs.
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
      @click.self="showNewDialog = false"
    >
      <div class="z-card w-full max-w-[440px] p-6 overflow-hidden shadow-2xl relative border border-slate-700/60 rounded-2xl bg-[#0e1622]">
        <h3 class="text-white font-bold text-base mb-4">New Offline Instance</h3>
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
    </div>
  </div>
</template>

<script setup>
import { computed, onBeforeUnmount, onMounted, ref } from 'vue';
import { getCurrentWebview } from '@tauri-apps/api/webview';
import {
  api,
  fmtBytes,
  JAR_FILTER,
  PACK_FILTER,
  pickFiles,
} from '../lib/api';

const emit = defineEmits(['launching', 'stopped', 'error']);

const instances = ref([]);
const selected = ref(null);
const selectedDir = ref('');
const activeTab = ref('mods'); // 'mods' | 'shaders' | 'textures'

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

const selectedModCount = computed(() => Object.keys(selectedMods.value).length);
const allModsSelected = computed(
  () => mods.value.length > 0 && mods.value.every((m) => selectedMods.value[m.filename])
);

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
  const [basicPacks, detailed] = await Promise.all([
    api.listInstancePacks(selectedDir.value),
    api.listInstancePacksDetailed(selectedDir.value),
  ]);
  packs.value = basicPacks;
  detailedPacks.value = detailed;
  activeShaderpack.value = packs.value.activeShaderpack || '';
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
  const next = { ...selectedMods.value };
  if (next[filename]) delete next[filename];
  else next[filename] = true;
  selectedMods.value = next;
}

function toggleSelectAllMods() {
  if (allModsSelected.value) {
    selectedMods.value = {};
    return;
  }
  const next = {};
  for (const mod of mods.value) next[mod.filename] = true;
  selectedMods.value = next;
}

async function toggleModEnabled(mod) {
  await api.setOfflineModEnabled(selected.value.id, mod.filename, !mod.enabled);
  await loadMods();
}

async function bulkEnableSelected() {
  const filenames = Object.keys(selectedMods.value);
  if (!filenames.length) return;
  await Promise.all(filenames.map((f) => api.setOfflineModEnabled(selected.value.id, f, true)));
  await loadMods();
}

async function bulkDisableSelected() {
  const filenames = Object.keys(selectedMods.value);
  if (!filenames.length) return;
  await Promise.all(filenames.map((f) => api.setOfflineModEnabled(selected.value.id, f, false)));
  await loadMods();
}

async function bulkDeleteSelected() {
  const filenames = Object.keys(selectedMods.value);
  if (!filenames.length) return;
  if (!window.confirm(`Delete ${filenames.length} selected mod(s)?`)) return;
  await Promise.all(filenames.map((fn) => api.deleteOfflineMod(selected.value.id, fn)));
  await loadMods();
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

async function installMod(hit) {
  if (!selected.value) return;
  const id = hit.projectId || hit.id;
  installingId.value = id;
  try {
    await api.installModrinthPack(selected.value.id, id, hit.selectedVersionId, 'mod');
    await loadMods();
  } catch (e) {
    window.dispatchEvent(new CustomEvent('zircon-status', { detail: `Install failed: ${e}` }));
  } finally {
    installingId.value = '';
  }
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
      newForm.value.loaderVersion = '';
    }
  } catch (err) {
    console.warn('Failed to fetch loader versions:', err);
    loaderVersions.value = [];
    recommendedLoaderVersion.value = '';
  } finally {
    loadingLoaderVersions.value = false;
  }
}

async function openNewInstance() {
  const defaultMc = mcVersions.value[0] || '1.21.4';
  const defaultLoader = loaderTypes.value[0] || 'fabric';
  newForm.value = {
    name: '',
    mcVersion: defaultMc,
    loaderType: defaultLoader,
    loaderVersion: '',
  };
  showNewDialog.value = true;
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

let unlistenFileDrop;

onMounted(async () => {
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
  if (unlistenFileDrop) unlistenFileDrop();
  if (curseforgeModal.value.countdownInterval) {
    clearInterval(curseforgeModal.value.countdownInterval);
  }
});
</script>
