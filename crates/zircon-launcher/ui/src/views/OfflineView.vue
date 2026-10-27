<template>
  <div
    class="h-full flex flex-col p-5 gap-4"
    :class="viewMode === 'split' ? 'overflow-hidden' : 'overflow-y-auto'"
  >
    <!-- Master Header & Control Bar -->
    <div v-if="!isStudioFocused" class="relative z-50 z-card bg-[#0e1622]/90 border border-slate-800/80 px-4 py-3 flex items-center justify-between gap-3 shrink-0 flex-wrap sm:flex-nowrap transition-all duration-300">
      <!-- Left: Title, Count, Search, Loader Filter Pills -->
      <div class="flex items-center gap-3 flex-1 min-w-0 flex-wrap">
        <div class="flex items-center gap-2 mr-1">
          <span class="z-section text-white font-bold text-sm tracking-wide">{{ $t('offline.instances_title') }}</span>
          <span
            v-if="instances.length > 0"
            class="px-2 py-0.5 rounded-full text-[10px] font-bold bg-cyan-500/15 text-cyan-300 border border-cyan-500/30 font-mono shadow-[0_0_8px_rgba(71,210,201,0.15)]"
          >
            {{ instances.length }}
          </span>
        </div>

        <!-- Quick Search Bar -->
        <div class="relative w-48 sm:w-60 min-w-[160px]">
          <svg class="w-3.5 h-3.5 absolute left-3 top-1/2 -translate-y-1/2 text-slate-400 pointer-events-none" fill="none" viewBox="0 0 24 24" stroke="currentColor">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M21 21l-6-6m2-5a7 7 0 11-14 0 7 7 0 0114 0z" />
          </svg>
          <input
            v-model="searchQuery"
            type="text"
            :placeholder="$t('offline.search_placeholder')"
            class="w-full bg-[#070b10]/80 border border-slate-700/80 rounded-xl pr-7 py-1.5 text-xs text-white placeholder-slate-500 focus:outline-none focus:border-cyan-500/60 focus:ring-1 focus:ring-cyan-500/40 transition-all font-sans"
            style="padding-left: 2.35rem !important;"
            @focus="onSearchFocus"
          />
          <button
            v-if="searchQuery"
            class="absolute right-2.5 top-1/2 -translate-y-1/2 text-slate-400 hover:text-white text-xs cursor-pointer"
            @click="searchQuery = ''"
          >
            ✕
          </button>
        </div>

        <!-- Loader Filter Pills -->
        <div class="hidden md:flex items-center gap-1 bg-[#070b10]/80 p-1 rounded-xl border border-slate-800 text-[11px] shadow-inner">
          <button
            v-for="loader in ['all', 'fabric', 'neoforge', 'forge', 'vanilla']"
            :key="loader"
            class="px-3 py-1 rounded-lg font-semibold transition-all capitalize cursor-pointer select-none"
            :class="
              filterLoader === loader
                ? 'bg-accent/20 text-accent-bright border border-accent/40 shadow-[0_0_10px_var(--color-accent-glow)] font-bold'
                : 'text-slate-400 hover:text-white border border-transparent hover:bg-slate-800/40'
            "
            @click="filterLoader = loader"
          >
            {{ loader }}
          </button>
        </div>
      </div>

      <!-- Right: Sort, View Switcher, Tools Dropdown, New Instance Button -->
      <div class="flex items-center gap-2 shrink-0">
        <!-- Sort Selector -->
        <div class="relative hidden sm:inline-flex items-center">
          <select
            v-model="sortBy"
            class="bg-[#070b10]/80 border border-slate-700/80 rounded-xl px-3 py-1.5 text-xs text-slate-300 font-medium focus:outline-none focus:border-cyan-500/60 transition-all cursor-pointer appearance-none pr-7"
          >
            <option value="recent">{{ $t('offline.sort_recent') }}</option>
            <option value="name">{{ $t('offline.sort_name') }}</option>
            <option value="version">{{ $t('offline.sort_version') }}</option>
          </select>
          <svg class="w-3 h-3 absolute right-2.5 top-1/2 -translate-y-1/2 text-slate-400 pointer-events-none" fill="none" viewBox="0 0 24 24" stroke="currentColor">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M19 9l-7 7-7-7" />
          </svg>
        </div>

        <!-- View Switcher (Grid vs Split) -->
        <div class="flex items-center bg-[#070b10]/80 p-1 rounded-xl border border-slate-700/80 shadow-inner">
          <button
            class="p-1.5 rounded-lg transition-all cursor-pointer"
            :class="viewMode === 'grid' ? 'bg-accent/20 text-accent-bright border border-accent/40 shadow-[0_0_8px_var(--color-accent-glow)]' : 'text-slate-400 hover:text-white border border-transparent hover:bg-slate-800/40'"
            :title="$t('offline.grid_view')"
            @click="viewMode = 'grid'; isStudioFocused = false;"
          >
            <svg class="w-3.5 h-3.5" fill="none" viewBox="0 0 24 24" stroke="currentColor">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M4 6a2 2 0 012-2h2a2 2 0 012 2v2a2 2 0 01-2 2H6a2 2 0 01-2-2V6zM14 6a2 2 0 012-2h2a2 2 0 012 2v2a2 2 0 01-2 2h-2a2 2 0 01-2-2V6zM4 16a2 2 0 012-2h2a2 2 0 012 2v2a2 2 0 01-2 2H6a2 2 0 01-2-2v-2zM14 16a2 2 0 012-2h2a2 2 0 012 2v2a2 2 0 01-2 2h-2a2 2 0 01-2-2v-2z" />
            </svg>
          </button>
          <button
            class="p-1.5 rounded-lg transition-all cursor-pointer"
            :class="viewMode === 'split' ? 'bg-accent/20 text-accent-bright border border-accent/40 shadow-[0_0_8px_var(--color-accent-glow)]' : 'text-slate-400 hover:text-white border border-transparent hover:bg-slate-800/40'"
            :title="$t('offline.split_view')"
            @click="viewMode = 'split'"
          >
            <svg class="w-3.5 h-3.5" fill="none" viewBox="0 0 24 24" stroke="currentColor">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M4 5a1 1 0 011-1h14a1 1 0 011 1v14a1 1 0 01-1 1H5a1 1 0 01-1-1V5zm7 1v12h7a1 1 0 001-1V6a1 1 0 00-1-1h-7z" />
            </svg>
          </button>
        </div>

        <!-- Tools Dropdown (Migrate, Import, Join) -->
        <div class="relative z-50" data-dropdown="tools">
          <button
            class="px-2.5 py-1.5 rounded-xl border border-slate-700/80 bg-[#070b10]/80 hover:bg-[#111c2a] hover:border-cyan-500/40 text-slate-300 hover:text-white text-xs font-semibold flex items-center gap-1.5 transition-all shadow-sm cursor-pointer"
            @click="showToolsDropdown = !showToolsDropdown"
            :title="$t('offline.tools_tooltip')"
          >
            <svg class="w-3.5 h-3.5 text-cyan-400" fill="none" viewBox="0 0 24 24" stroke="currentColor">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 6V4m0 2a2 2 0 100 4m0-4a2 2 0 110 4m-6 8a2 2 0 100-4m0 4a2 2 0 110-4m0 4v2m0-6V4m6 6v10m6-2a2 2 0 100-4m0 4a2 2 0 110-4m0 4v2m0-6V4" />
            </svg>
            <span class="hidden sm:inline">{{ $t('offline.tools_btn') }}</span>
            <svg class="w-3 h-3 transition-transform" :class="{ 'rotate-180': showToolsDropdown }" fill="none" viewBox="0 0 24 24" stroke="currentColor">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M19 9l-7 7-7-7" />
            </svg>
          </button>

          <transition name="dropdown-pop">
            <div
              v-if="showToolsDropdown"
              class="absolute right-0 top-full mt-2 w-64 bg-[#0c1322] border border-white/15 ring-1 ring-cyan-500/30 rounded-2xl shadow-[0_25px_60px_rgba(0,0,0,0.95),0_0_25px_rgba(71,210,201,0.2)] p-1.5 z-50 text-xs backdrop-blur-2xl"
            >
              <!-- Ambient Top Accent Glow -->
              <div class="absolute top-0 inset-x-4 h-px bg-gradient-to-r from-transparent via-cyan-400/50 to-transparent pointer-events-none"></div>

              <!-- Header label -->
              <div class="px-3 pt-2 pb-1 text-[10px] font-bold uppercase tracking-wider text-cyan-400/80 font-mono">
                {{ $t('offline.tools_header') }}
              </div>

              <!-- Item 1: Migrate Launchers -->
              <button
                class="w-full text-left px-3 py-2 rounded-xl hover:bg-cyan-500/15 border border-transparent hover:border-cyan-500/30 text-slate-300 hover:text-white flex items-center gap-2.5 transition-all duration-150 group cursor-pointer"
                @click="showMigrationModal = true; showToolsDropdown = false;"
              >
                <div class="w-7 h-7 rounded-lg bg-cyan-500/10 border border-cyan-500/25 flex items-center justify-center text-cyan-400 group-hover:text-cyan-300 group-hover:bg-cyan-500/20 group-hover:scale-105 transition-all shadow-sm shrink-0">
                  <svg class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                    <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M8 7h12m0 0l-4-4m4 4l-4 4m0 6H4m0 0l4 4m-4-4l4-4" />
                  </svg>
                </div>
                <div class="min-w-0 flex-1">
                  <div class="font-semibold text-xs text-white group-hover:text-cyan-200 transition-colors">{{ $t('offline.migrate_launchers') }}</div>
                  <div class="text-[10px] text-slate-400 group-hover:text-slate-300 font-sans truncate">{{ $t('offline.migrate_launchers_sub') }}</div>
                </div>
              </button>

              <!-- Item 2: Import Setup -->
              <button
                class="w-full text-left px-3 py-2 rounded-xl hover:bg-cyan-500/15 border border-transparent hover:border-cyan-500/30 text-slate-300 hover:text-white flex items-center gap-2.5 transition-all duration-150 group cursor-pointer"
                @click="showImportModal = true; showToolsDropdown = false;"
              >
                <div class="w-7 h-7 rounded-lg bg-cyan-500/10 border border-cyan-500/25 flex items-center justify-center text-cyan-400 group-hover:text-cyan-300 group-hover:bg-cyan-500/20 group-hover:scale-105 transition-all shadow-sm shrink-0">
                  <svg class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                    <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M4 16v1a3 3 0 003 3h10a3 3 0 003-3v-1m-4-8l-4-4m0 0L8 8m4-4v12" />
                  </svg>
                </div>
                <div class="min-w-0 flex-1">
                  <div class="font-semibold text-xs text-white group-hover:text-cyan-200 transition-colors">{{ $t('offline.import_setup') }}</div>
                  <div class="text-[10px] text-slate-400 group-hover:text-slate-300 font-sans truncate">{{ $t('offline.import_setup_sub') }}</div>
                </div>
              </button>

              <!-- Item 3: Join Friend (P2P) -->
              <button
                class="w-full text-left px-3 py-2 rounded-xl hover:bg-cyan-500/15 border border-transparent hover:border-cyan-500/30 text-slate-300 hover:text-white flex items-center gap-2.5 transition-all duration-150 group cursor-pointer"
                @click="showJoinCodeModal = true; showToolsDropdown = false;"
              >
                <div class="w-7 h-7 rounded-lg bg-cyan-500/10 border border-cyan-500/25 flex items-center justify-center text-cyan-400 group-hover:text-cyan-300 group-hover:bg-cyan-500/20 group-hover:scale-105 transition-all shadow-sm shrink-0">
                  <svg class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                    <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M13.828 10.172a4 4 0 00-5.656 0l-4 4a4 4 0 105.656 5.656l1.102-1.101m-.758-4.899a4 4 0 005.656 0l4-4a4 4 0 00-5.656-5.656l-1.1 1.1" />
                  </svg>
                </div>
                <div class="min-w-0 flex-1">
                  <div class="font-semibold text-xs text-white group-hover:text-cyan-200 transition-colors">{{ $t('offline.join_friend') }}</div>
                  <div class="text-[10px] text-slate-400 group-hover:text-slate-300 font-sans truncate">{{ $t('offline.join_friend_sub') }}</div>
                </div>
              </button>
            </div>
          </transition>
        </div>

        <!-- Hero New Instance Button -->
        <button
          class="z-btn-accent text-xs font-bold px-3.5 py-1.5 rounded-xl shadow-md hover:shadow-cyan-500/25 flex items-center gap-1.5 transition-transform active:scale-95"
          @click="openNewInstance('scratch')"
        >
          <svg class="w-3.5 h-3.5 fill-current" viewBox="0 0 24 24"><path d="M19 13h-6v6h-2v-6H5v-2h6V5h2v6h6v2z"/></svg>
          <span>{{ $t('offline.new_instance') }}</span>
        </button>
      </div>
    </div>

    <!-- Main Content Area -->
    <div
      class="flex gap-5 relative z-0"
      :class="viewMode === 'split' ? 'flex-1 min-h-0 overflow-hidden' : 'w-full min-h-fit'"
    >
      <!-- MODE A: GRID VIEW (when viewMode === 'grid' and not in studio focus) -->
      <div
        v-if="viewMode === 'grid' && !isStudioFocused"
        class="flex-1 flex flex-col p-2 -m-2"
      >
        <!-- Cards Grid -->
        <div
          v-if="filteredInstances.length > 0"
          class="grid grid-cols-1 sm:grid-cols-2 md:grid-cols-3 xl:grid-cols-4 2xl:grid-cols-5 gap-4 pb-6"
        >
          <!-- Add Instance Tile (matching Skins style) -->
          <button
            v-if="!searchQuery"
            type="button"
            class="group relative z-card bg-[#070b10]/60 hover:bg-[#0e1622]/90 border-2 border-dashed border-slate-700/60 hover:border-accent/60 rounded-2xl p-5 flex flex-col items-center justify-center min-h-[185px] h-full text-center transition-all duration-300 hover:shadow-[0_8px_30px_var(--color-accent-glow)] hover:-translate-y-0.5 cursor-pointer"
            :title="$t('offline.create_tile_tooltip')"
            @click="openNewInstance('scratch')"
          >
            <div class="w-12 h-12 rounded-2xl bg-accent/10 border border-accent/30 flex items-center justify-center text-accent-bright group-hover:scale-110 group-hover:bg-accent/20 transition-all shadow-[0_0_16px_var(--color-accent-glow)] mb-3">
              <svg class="w-6 h-6" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5">
                <path stroke-linecap="round" stroke-linejoin="round" d="M12 5v14M5 12h14" />
              </svg>
            </div>
            <div class="text-xs font-bold text-white group-hover:text-accent-bright transition-colors">
              {{ $t('offline.create_tile_title') }}
            </div>
            <span class="text-[11px] text-slate-400 mt-1 font-sans">
              {{ $t('offline.create_tile_desc') }}
            </span>
          </button>

          <InstanceCard
            v-for="instance in filteredInstances"
            :key="instance.id"
            :instance="instance"
            :selected="viewMode === 'split' && selected?.id === instance.id"
            :banner-url="instanceBanner(instance)"
            :icon-url="instanceArtworks[instance.id]?.iconDataUrl"
            :active-dropdown="activeCardDropdownId === instance.id"
            :launching="launching"
            :format-relative-time="formatRelativeTime"
            @select="selectInstance"
            @play="playInstanceDirectly"
            @manage="openManageStudio"
            @toggle-dropdown="toggleCardDropdown"
            @clone="cloneInstanceCard"
            @open-folder="openInstanceFolderCard"
            @export-mrpack="exportMrpackCard"
            @delete="deleteInstanceById"
          />
        </div>

        <!-- Empty State for Grid View -->
        <div v-else class="flex-1 min-h-[360px] flex flex-col items-center justify-center text-center p-8">
          <div class="w-14 h-14 rounded-2xl bg-cyan-500/10 border border-cyan-500/30 flex items-center justify-center text-cyan-400 mb-4 shadow-[0_0_20px_rgba(71,210,201,0.2)]">
            <svg class="w-7 h-7" fill="none" viewBox="0 0 24 24" stroke="currentColor">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M20 7l-8-4-8 4m16 0l-8 4m8-4v10l-8 4m0-10L4 7m8 4v10M4 7v10l8 4" />
            </svg>
          </div>
          <div v-if="instances.length === 0">
            <h2 class="text-base font-bold text-white mb-1">No instances installed yet</h2>
            <p class="text-xs text-slate-400 max-w-sm mb-5 leading-relaxed">
              Create a fresh vanilla or modded profile, or effortlessly migrate your instances from Prism, CurseForge, or Modrinth.
            </p>
            <div class="flex items-center gap-3 justify-center">
              <button
                class="z-btn-accent text-xs font-bold py-2.5 px-4 rounded-xl shadow-lg hover:shadow-cyan-500/25 flex items-center gap-2 cursor-pointer hover:brightness-110 active:scale-95 transition-all"
                @click="openNewInstance('scratch')"
              >
                <svg class="w-4 h-4 fill-current" viewBox="0 0 24 24"><path d="M19 13h-6v6h-2v-6H5v-2h6V5h2v6h6v2z"/></svg>
                <span>Create Clean Instance</span>
              </button>
              <button
                class="rounded-xl border border-cyan-500/30 bg-cyan-950/30 hover:bg-cyan-500/15 hover:border-cyan-400/60 text-cyan-300 text-xs font-bold py-2.5 px-4 flex items-center gap-2 transition-all shadow-[0_0_15px_rgba(71,210,201,0.12)] cursor-pointer active:scale-95"
                @click="showMigrationModal = true"
              >
                <svg class="w-4 h-4 stroke-current fill-none" viewBox="0 0 24 24">
                  <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M8 7h12m0 0l-4-4m4 4l-4 4m0 6H4m0 0l4 4m-4-4l4-4" />
                </svg>
                <span>Migrate From Other Launchers</span>
              </button>
            </div>
          </div>
          <div v-else>
            <h2 class="text-base font-bold text-white mb-1">No matching instances</h2>
            <p class="text-xs text-slate-400 max-w-sm mb-4">
              No instances found matching your search or loader filter.
            </p>
            <button
              class="px-3 py-1.5 rounded-xl border border-slate-700/80 bg-slate-900/60 hover:bg-slate-800 text-slate-300 hover:text-white text-xs font-semibold"
              @click="searchQuery = ''; filterLoader = 'all';"
            >
              Clear Filters
            </button>
          </div>
        </div>
      </div>

      <!-- MODE B: SPLIT VIEW SIDEBAR (only when viewMode === 'split') -->
      <div
        v-if="viewMode === 'split'"
        class="w-[300px] min-w-[300px] z-card flex flex-col p-3.5 bg-[#0e1622]/90 border border-slate-800/80 shrink-0"
      >
        <div class="flex-1 min-h-0 overflow-y-auto pr-1 space-y-2">
          <div
            v-for="instance in filteredInstances"
            :key="instance.id"
            class="relative flex items-center gap-3 border rounded-xl p-3 cursor-pointer transition-all duration-200"
            :class="
              selected?.id === instance.id
                ? 'border-accent bg-[#111c29] shadow-[0_0_16px_var(--color-accent-glow)]'
                : 'border-slate-800/80 bg-[#070b10]/60 hover:border-accent/40 hover:shadow-[0_0_10px_var(--color-accent-glow)] hover:bg-[#121d2b]'
            "
            @click="selectInstance(instance)"
          >
            <span
              v-if="selected?.id === instance.id"
              class="absolute left-0 top-1/2 -translate-y-1/2 w-1 h-7 rounded-r-full bg-accent shadow-[0_0_10px_var(--color-accent)]"
            ></span>
            <div
              v-if="instanceArtworks[instance.id]?.iconDataUrl"
              class="w-9 h-9 rounded-xl overflow-hidden shrink-0 border border-slate-700/80 bg-slate-900 shadow-sm"
            >
              <img :src="instanceArtworks[instance.id].iconDataUrl" class="w-full h-full object-cover" alt="" />
            </div>
            <div
              v-else
              class="w-9 h-9 rounded-xl bg-gradient-to-br from-accent-bright via-accent to-accent-deep text-accent-ink font-black flex items-center justify-center text-sm shrink-0 shadow-[0_0_10px_var(--color-accent-glow)]"
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
          <div v-if="filteredInstances.length === 0" class="p-4 my-auto text-center space-y-3">
            <div class="text-xs font-bold text-white">No instances match</div>
          </div>
        </div>
      </div>

      <!-- Right / Studio Detail & Management Pane -->
      <div
        v-if="viewMode === 'split' || isStudioFocused"
        class="z-card flex flex-col p-5 bg-[#0e1622]/90 border border-slate-800/80"
        :class="viewMode === 'split' ? 'flex-1 min-w-0 min-h-0 overflow-y-auto' : 'w-full shrink-0 min-h-fit'"
      >
        <!-- Studio Mode Breadcrumb Header -->
        <div
          v-if="viewMode === 'grid' && isStudioFocused"
          class="mb-3.5 pb-2.5 border-b border-slate-800/80 flex items-center justify-between"
        >
          <button
            class="flex items-center gap-2 px-3 py-1.5 rounded-xl bg-slate-900/80 hover:bg-slate-800 border border-slate-700/80 text-slate-300 hover:text-white text-xs font-semibold transition-all group"
            @click="closeStudio"
          >
            <svg class="w-4 h-4 text-cyan-400 group-hover:-translate-x-0.5 transition-transform" fill="none" viewBox="0 0 24 24" stroke="currentColor">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M10 19l-7-7m0 0l7-7m-7 7h18" />
            </svg>
            <span>{{ $t('offline.all_instances') }}</span>
          </button>
          <div class="flex items-center gap-2 text-xs font-mono text-slate-400">
            <span class="w-2 h-2 rounded-full bg-cyan-400 animate-pulse"></span>
            <span>{{ $t('offline.instance_studio', { name: selected?.name }) }}</span>
          </div>
        </div>
      <template v-if="selected">
        <!-- Header summary & play action -->
        <div
          class="relative bg-[#070b10] border border-slate-800/90 rounded-xl p-4 shadow-inner mb-4 shrink-0 min-h-fit"
          :class="(showActionsDropdown || showOpenFolderDropdown) ? 'z-40 overflow-visible' : 'z-20 overflow-hidden'"
        >
          <!-- Static Ambient Wallpaper Backdrop (Matches ServersView) -->
          <div
            v-if="instanceBanner(selected)"
            class="absolute inset-0 pointer-events-none opacity-25 overflow-hidden rounded-xl z-0"
          >
            <img
              :src="instanceBanner(selected)"
              class="w-full h-full object-cover filter blur-[1px]"
              alt=""
            />
            <div class="absolute inset-0 bg-gradient-to-r from-[#070b10] via-transparent to-[#070b10]"></div>
          </div>

          <!-- Tier 1: Identity & Hero Play Action -->
          <div class="relative z-10 flex items-center justify-between gap-4">
            <div class="min-w-0 flex-1">
              <div class="text-white font-bold text-lg tracking-wide truncate mb-1.5">{{ selected.name }}</div>
              <div class="flex items-center gap-2 text-xs flex-wrap font-mono">
                <span class="bg-slate-900 border border-slate-700/80 text-slate-200 px-2.5 py-0.5 rounded-lg text-[11px]">
                  MC <strong class="text-white font-bold">{{ selected.minecraftVersion }}</strong>
                </span>
                <span class="bg-cyan-500/10 border border-cyan-500/30 text-cyan-300 px-2.5 py-0.5 rounded-lg text-[11px] capitalize">
                  {{ selected.modLoader.type }} {{ selected.modLoader.version }}
                </span>
                <button
                  type="button"
                  class="text-[11px] font-sans px-2 py-0.5 rounded-lg border border-slate-700/80 hover:border-cyan-500/50 bg-[#070b10]/60 hover:bg-cyan-500/10 text-slate-300 hover:text-cyan-300 transition-colors flex items-center gap-1 cursor-pointer"
                  title="Change Minecraft or mod loader version"
                  @click="openEditInstanceModal"
                >
                  <svg class="w-3 h-3 text-cyan-400" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                    <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M11 5H6a2 2 0 00-2 2v11a2 2 0 002 2h11a2 2 0 002-2v-5m-1.414-9.414a2 2 0 112.828 2.828L11.828 15H9v-2.828l8.586-8.586z" />
                  </svg>
                  <span>Edit Version</span>
                </button>
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
                {{ $t('offline.launching') }}
              </span>
              <span v-else class="flex items-center gap-2">
                <svg class="w-4 h-4 fill-current" viewBox="0 0 24 24"><path d="M8 5v14l11-7z" /></svg>
                {{ $t('offline.play_btn') }}
              </span>
            </button>
          </div>


          <!-- Tier 2: Action Toolbar -->
          <div class="pt-3 mt-3 border-t border-slate-800/80 flex items-center justify-between gap-3">
            <div class="flex items-center gap-2 flex-wrap">
              <!-- Flush Open Folder split-button dropdown -->
              <div
                class="relative inline-flex"
                :class="showOpenFolderDropdown ? 'z-50' : ''"
                data-dropdown="open-folder"
              >
                <div class="inline-flex items-stretch rounded-xl overflow-hidden border border-slate-700/80 shadow-sm h-8 bg-slate-900/80 hover:border-cyan-500/40 transition-colors">
                  <button
                    class="hover:bg-cyan-500/15 hover:text-white text-xs px-3 font-semibold text-slate-200 flex items-center gap-1.5 transition-colors cursor-pointer"
                    @click="openFolder(null)"
                    title="Open Instance Folder (.minecraft)"
                  >
                    <svg class="w-3.5 h-3.5 text-cyan-400 shrink-0" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                      <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M3 7v10a2 2 0 002 2h14a2 2 0 002-2V9a2 2 0 00-2-2h-6l-2-2H5a2 2 0 00-2 2z" />
                    </svg>
                    <span>{{ $t('server_config.folder_btn') }}</span>
                  </button>
                  <button
                    class="hover:bg-cyan-500/15 hover:text-white text-xs px-2 border-l border-slate-700/80 text-slate-400 hover:text-white flex items-center justify-center transition-colors cursor-pointer"
                    @click="showOpenFolderDropdown = !showOpenFolderDropdown"
                    title="Choose Subfolder"
                  >
                    <svg class="w-3 h-3 transition-transform" :class="{ 'rotate-180': showOpenFolderDropdown }" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                      <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M19 9l-7 7-7-7" />
                    </svg>
                  </button>
                </div>

                <!-- Dropdown Menu -->
                <transition name="dropdown-pop">
                  <div
                    v-if="showOpenFolderDropdown"
                    class="absolute left-0 top-full mt-1.5 w-60 bg-[#0c1322] border border-white/15 ring-1 ring-cyan-500/30 rounded-2xl shadow-[0_25px_60px_rgba(0,0,0,0.95),0_0_25px_rgba(71,210,201,0.2)] p-1.5 z-50 text-xs text-slate-200"
                  >
                    <div class="absolute top-0 inset-x-3 h-px bg-gradient-to-r from-transparent via-cyan-400/50 to-transparent pointer-events-none"></div>

                    <div class="px-3 pt-2 pb-1 text-[10px] font-bold uppercase tracking-wider text-cyan-400/80 font-mono">
                      Directories
                    </div>

                    <button
                      v-for="item in [
                        { name: 'Instance Root', path: '.minecraft', key: null },
                        { name: 'Mods', path: 'mods/', key: 'mods' },
                        { name: 'Config', path: 'config/', key: 'config' },
                        { name: 'Saves (Worlds)', path: 'saves/', key: 'saves' },
                        { name: 'Screenshots', path: 'screenshots/', key: 'screenshots' },
                        { name: 'Shaders', path: 'shaderpacks/', key: 'shaderpacks' },
                        { name: 'Resource Packs', path: 'resourcepacks/', key: 'resourcepacks' },
                        { name: 'Logs', path: 'logs/', key: 'logs' }
                      ]"
                      :key="item.path"
                      class="w-full text-left px-2.5 py-1.5 rounded-xl hover:bg-cyan-500/15 border border-transparent hover:border-cyan-500/30 flex items-center justify-between text-slate-300 hover:text-white transition-all duration-150 group cursor-pointer"
                      @click="openFolder(item.key)"
                    >
                      <div class="flex items-center gap-2">
                        <svg class="w-3.5 h-3.5 text-cyan-400/70 group-hover:text-cyan-300 shrink-0 transition-colors" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M3 7v10a2 2 0 002 2h14a2 2 0 002-2V9a2 2 0 00-2-2h-6l-2-2H5a2 2 0 00-2 2z" />
                        </svg>
                        <span class="font-medium text-xs text-white group-hover:text-cyan-200 transition-colors">{{ item.name }}</span>
                      </div>
                      <span class="text-[10px] px-1.5 py-0.5 rounded-md bg-white/5 border border-white/5 text-slate-400 font-mono group-hover:text-cyan-300 group-hover:border-cyan-500/30 transition-colors">{{ item.path }}</span>
                    </button>
                  </div>
                </transition>
              </div>

              <!-- Host for Friends Co-Op (On-Brand Cyan) -->
              <button
                class="text-xs px-3 h-8 rounded-xl font-semibold bg-cyan-500/15 hover:bg-cyan-500/25 text-cyan-300 border border-cyan-500/30 shadow-sm flex items-center gap-1.5 transition-colors cursor-pointer"
                @click="openCoopModal"
                :title="$t('offline.host_for_friends_tooltip')"
              >
                <svg class="w-3.5 h-3.5 text-cyan-400" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                  <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 4.354a4 4 0 110 5.292M15 21H3v-1a6 6 0 0112 0v1zm0 0h6v-1a6 6 0 00-9-5.197M13 7a4 4 0 11-8 0 4 4 0 018 0z" />
                </svg>
                <span>{{ $t('offline.host_for_friends') }}</span>
              </button>

              <!-- Couch Co-Op Split-Screen (Dynamic Theme Accent) -->
              <button
                class="text-xs px-3 h-8 rounded-xl font-semibold bg-accent/15 hover:bg-accent/25 text-accent-bright border border-accent/30 shadow-[0_0_10px_var(--color-accent-glow)] flex items-center gap-1.5 transition-all hover:border-accent/50 cursor-pointer"
                @click="showCouchCoopModal = true"
                :title="$t('offline.split_screen_tooltip')"
              >
                <svg class="w-3.5 h-3.5 text-accent-bright" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                  <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M15 5v2m0 4v2m0 4v2M5 5a2 2 0 00-2 2v3a2 2 0 110 4v3a2 2 0 002 2h14a2 2 0 002-2v-3a2 2 0 110-4V7a2 2 0 00-2-2H5z" />
                </svg>
                <span>{{ $t('offline.split_screen_btn') }}</span>
              </button>

              <!-- Actions Dropdown -->
              <div
                class="relative inline-flex"
                :class="showActionsDropdown ? 'z-50' : ''"
                data-dropdown="actions"
              >
                <button
                  class="z-btn-ghost text-xs px-3 h-8 rounded-xl font-semibold border border-slate-700/80 hover:border-cyan-500/40 hover:text-white flex items-center gap-1.5 transition-colors cursor-pointer"
                  @click="showActionsDropdown = !showActionsDropdown"
                >
                  <svg class="w-3.5 h-3.5 text-slate-400" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                    <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 5v.01M12 12v.01M12 19v.01M12 6a1 1 0 110-2 1 1 0 010 2zm0 7a1 1 0 110-2 1 1 0 010 2zm0 7a1 1 0 110-2 1 1 0 010 2z" />
                  </svg>
                  <span>{{ $t('offline.actions_btn') }}</span>
                  <svg class="w-3 h-3 transition-transform" :class="{ 'rotate-180': showActionsDropdown }" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                    <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M19 9l-7 7-7-7" />
                  </svg>
                </button>

                <transition name="dropdown-pop">
                  <div
                    v-if="showActionsDropdown"
                    class="absolute left-0 top-full mt-1.5 w-64 bg-[#0c1322] border border-white/15 ring-1 ring-cyan-500/30 rounded-2xl shadow-[0_25px_60px_rgba(0,0,0,0.95),0_0_25px_rgba(71,210,201,0.2)] p-1.5 z-50 text-xs max-h-[calc(100vh-220px)] overflow-y-auto custom-scrollbar"
                  >
                    <div class="absolute top-0 inset-x-3 h-px bg-gradient-to-r from-transparent via-cyan-400/50 to-transparent pointer-events-none"></div>

                    <!-- Group 1: Manage & Versioning -->
                    <div class="px-3 pt-2 pb-1 text-[10px] font-bold uppercase tracking-wider text-cyan-400/80 font-mono">
                      {{ $t('offline.actions_manage_versioning') }}
                    </div>

                    <button class="w-full text-left px-2.5 py-1.5 rounded-xl hover:bg-cyan-500/15 border border-transparent hover:border-cyan-500/30 flex items-center gap-2.5 transition-all duration-150 group cursor-pointer" @click="openEditInstanceModal">
                      <div class="w-6 h-6 rounded-lg bg-cyan-500/10 border border-cyan-500/25 flex items-center justify-center text-cyan-400 group-hover:text-cyan-300 group-hover:bg-cyan-500/20 group-hover:scale-105 transition-all shrink-0">
                        <svg class="w-3.5 h-3.5" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M11 5H6a2 2 0 00-2 2v11a2 2 0 002 2h11a2 2 0 002-2v-5m-1.414-9.414a2 2 0 112.828 2.828L11.828 15H9v-2.828l8.586-8.586z" />
                        </svg>
                      </div>
                      <span class="text-xs text-slate-200 group-hover:text-white font-medium">Edit Version &amp; Loader</span>
                    </button>

                    <button class="w-full text-left px-2.5 py-1.5 rounded-xl hover:bg-cyan-500/15 border border-transparent hover:border-cyan-500/30 flex items-center gap-2.5 transition-all duration-150 group cursor-pointer" @click="promptCloneInstance">
                      <div class="w-6 h-6 rounded-lg bg-cyan-500/10 border border-cyan-500/25 flex items-center justify-center text-cyan-400 group-hover:text-cyan-300 group-hover:bg-cyan-500/20 group-hover:scale-105 transition-all shrink-0">
                        <svg class="w-3.5 h-3.5" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M8 16H6a2 2 0 01-2-2V6a2 2 0 012-2h8a2 2 0 012 2v2m-6 12h8a2 2 0 002-2v-8a2 2 0 00-2-2h-8a2 2 0 00-2 2v8a2 2 0 002 2z" />
                        </svg>
                      </div>
                      <span class="text-xs text-slate-200 group-hover:text-white font-medium">{{ $t('offline.clone_instance') }}</span>
                    </button>

                    <button class="w-full text-left px-2.5 py-1.5 rounded-xl hover:bg-cyan-500/15 border border-transparent hover:border-cyan-500/30 flex items-center gap-2.5 transition-all duration-150 group cursor-pointer" @click="showSnapshotsModal = true; showActionsDropdown = false;">
                      <div class="w-6 h-6 rounded-lg bg-cyan-500/10 border border-cyan-500/25 flex items-center justify-center text-cyan-400 group-hover:text-cyan-300 group-hover:bg-cyan-500/20 group-hover:scale-105 transition-all shrink-0">
                        <svg class="w-3.5 h-3.5" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 8v4l3 3m6-3a9 9 0 11-18 0 9 9 0 0118 0z" />
                        </svg>
                      </div>
                      <span class="text-xs text-slate-200 group-hover:text-white font-medium">{{ $t('offline.time_machine') }}</span>
                    </button>

                                        <button class="w-full text-left px-2.5 py-1.5 rounded-xl hover:bg-cyan-500/15 border border-transparent hover:border-cyan-500/30 flex items-center gap-2.5 transition-all duration-150 group cursor-pointer" @click="showKeybindModal = true; showActionsDropdown = false;">
                      <div class="w-6 h-6 rounded-lg bg-cyan-500/10 border border-cyan-500/25 flex items-center justify-center text-cyan-400 group-hover:text-cyan-300 group-hover:bg-cyan-500/20 group-hover:scale-105 transition-all shrink-0">
                        <svg class="w-3.5 h-3.5" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 6V4m0 2a2 2 0 100 4m0-4a2 2 0 110 4m-6 8a2 2 0 100-4m0 4a2 2 0 110-4m0 4v2m0-6V4m6 6v10m6-2a2 2 0 100-4m0 4a2 2 0 110-4m0 4v2m0-6V4" />
                        </svg>
                      </div>
                      <span class="text-xs text-slate-200 group-hover:text-white font-medium">Keybinds &amp; Sync</span>
                    </button>

                    <button class="w-full text-left px-2.5 py-1.5 rounded-xl hover:bg-cyan-500/15 border border-transparent hover:border-cyan-500/30 flex items-center gap-2.5 transition-all duration-150 group cursor-pointer" @click="showAuditModal = true; showActionsDropdown = false;">
                      <div class="w-6 h-6 rounded-lg bg-cyan-500/10 border border-cyan-500/25 flex items-center justify-center text-cyan-400 group-hover:text-cyan-300 group-hover:bg-cyan-500/20 group-hover:scale-105 transition-all shrink-0">
                        <svg class="w-3.5 h-3.5" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9 12l2 2 4-4m5.618-4.016A11.955 11.955 0 0112 2.944a11.955 11.955 0 01-8.618 3.04A12.02 12.02 0 003 9c0 5.591 3.824 10.29 9 11.622 5.176-1.332 9-6.03 9-11.622 0-1.042-.133-2.052-.382-3.016z" />
                        </svg>
                      </div>
                      <span class="text-xs text-slate-200 group-hover:text-white font-medium">{{ $t('offline.audit_repair') }}</span>
                    </button>

                    <!-- Temporarily disabled for v0.5.0: Seed & Biome Radar Map
                    <button class="w-full text-left px-2.5 py-1.5 rounded-xl hover:bg-cyan-500/15 border border-transparent hover:border-cyan-500/30 flex items-center gap-2.5 transition-all duration-150 group cursor-pointer" @click="showRadarModal = true; showActionsDropdown = false;">
                      <div class="w-6 h-6 rounded-lg bg-cyan-500/10 border border-cyan-500/25 flex items-center justify-center text-cyan-400 group-hover:text-cyan-300 group-hover:bg-cyan-500/20 group-hover:scale-105 transition-all shrink-0">
                        <svg class="w-3.5 h-3.5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                          <circle cx="12" cy="12" r="9" stroke-dasharray="3 2" />
                          <circle cx="12" cy="12" r="3" fill="currentColor" fill-opacity="0.3" />
                        </svg>
                      </div>
                      <span class="text-xs text-slate-200 group-hover:text-white font-medium">{{ $t('offline.seed_radar') }}</span>
                    </button>
                    -->

                    <div class="h-px bg-gradient-to-r from-transparent via-white/10 to-transparent my-1.5 mx-2"></div>

                    <!-- Group 2: Multiplayer & Sharing -->
                    <div class="px-3 pt-1 pb-1 text-[10px] font-bold uppercase tracking-wider text-cyan-400/80 font-mono">
                      {{ $t('offline.actions_multiplayer_sharing') }}
                    </div>

                    <button class="w-full text-left px-2.5 py-1.5 rounded-xl hover:bg-cyan-500/15 border border-transparent hover:border-cyan-500/30 flex items-center gap-2.5 transition-all duration-150 group cursor-pointer" @click="showShareModal = true; showActionsDropdown = false;">
                      <div class="w-6 h-6 rounded-lg bg-cyan-500/10 border border-cyan-500/25 flex items-center justify-center text-cyan-400 group-hover:text-cyan-300 group-hover:bg-cyan-500/20 group-hover:scale-105 transition-all shrink-0">
                        <svg class="w-3.5 h-3.5" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M8.684 13.342C8.886 12.938 9 12.482 9 12c0-.482-.114-.938-.316-1.342m0 2.684a3 3 0 110-2.684m0 2.684l6.632 3.316m-6.632-6l6.632-3.316m0 0a3 3 0 105.367-2.684 3 3 0 00-5.367 2.684zm0 9.316a3 3 0 105.368 2.684 3 3 0 00-5.368-2.684z" />
                        </svg>
                      </div>
                      <span class="text-xs text-slate-200 group-hover:text-white font-medium">{{ $t('offline.share_setup_item') }}</span>
                    </button>

                    <button class="w-full text-left px-2.5 py-1.5 rounded-xl hover:bg-cyan-500/15 border border-transparent hover:border-cyan-500/30 flex items-center gap-2.5 transition-all duration-150 group cursor-pointer" @click="showCouchCoopModal = true; showActionsDropdown = false;">
                      <div class="w-6 h-6 rounded-lg bg-cyan-500/10 border border-cyan-500/25 flex items-center justify-center text-cyan-400 group-hover:text-cyan-300 group-hover:bg-cyan-500/20 group-hover:scale-105 transition-all shrink-0">
                        <svg class="w-3.5 h-3.5" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M15 5v2m0 4v2m0 4v2M5 5a2 2 0 00-2 2v3a2 2 0 110 4v3a2 2 0 002 2h14a2 2 0 002-2v-3a2 2 0 110-4V7a2 2 0 00-2-2H5z" />
                        </svg>
                      </div>
                      <span class="text-xs text-slate-200 group-hover:text-white font-medium">{{ $t('offline.split_screen_btn') }}</span>
                    </button>

                    <button class="w-full text-left px-2.5 py-1.5 rounded-xl hover:bg-cyan-500/15 border border-transparent hover:border-cyan-500/30 flex items-center gap-2.5 transition-all duration-150 group cursor-pointer" @click="openCoopModal">
                      <div class="w-6 h-6 rounded-lg bg-cyan-500/10 border border-cyan-500/25 flex items-center justify-center text-cyan-400 group-hover:text-cyan-300 group-hover:bg-cyan-500/20 group-hover:scale-105 transition-all shrink-0">
                        <svg class="w-3.5 h-3.5" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M17 20h5v-2a3 3 0 00-5.356-1.857M17 20H7m10 0v-2c0-.656-.126-1.283-.356-1.857M7 20H2v-2a3 3 0 015.356-1.857M7 20v-2c0-.656.126-1.283.356-1.857m0 0a5.002 5.002 0 019.288 0M15 7a3 3 0 11-6 0 3 3 0 016 0zm6 3a2 2 0 11-4 0 2 2 0 014 0zM7 10a2 2 0 11-4 0 2 2 0 014 0z" />
                        </svg>
                      </div>
                      <span class="text-xs text-slate-200 group-hover:text-white font-medium">{{ $t('offline.host_for_friends') }}</span>
                    </button>

                    <div class="h-px bg-gradient-to-r from-transparent via-white/10 to-transparent my-1.5 mx-2"></div>

                    <!-- Group 3: Export Packages -->
                    <div class="px-3 pt-1 pb-1 text-[10px] font-bold uppercase tracking-wider text-cyan-400/80 font-mono">
                      {{ $t('offline.actions_export_packages') }}
                    </div>

                    <button class="w-full text-left px-2.5 py-1.5 rounded-xl hover:bg-cyan-500/15 border border-transparent hover:border-cyan-500/30 flex items-center gap-2.5 transition-all duration-150 group cursor-pointer" @click="exportMrpack">
                      <div class="w-6 h-6 rounded-lg bg-cyan-500/10 border border-cyan-500/25 flex items-center justify-center text-cyan-400 group-hover:text-cyan-300 group-hover:bg-cyan-500/20 group-hover:scale-105 transition-all shrink-0">
                        <svg class="w-3.5 h-3.5" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M4 16v1a3 3 0 003 3h10a3 3 0 003-3v-1m-4-4l-4 4m0 0l-4-4m4 4V4" />
                        </svg>
                      </div>
                      <span class="text-xs text-slate-200 group-hover:text-white font-medium">{{ $t('offline.export_mrpack') }}</span>
                    </button>

                  </div>
                </transition>
              </div>
            </div>

            <!-- Danger Delete Button -->
            <button
              class="text-xs px-3 h-8 rounded-xl font-semibold text-red-400 hover:text-red-300 hover:bg-red-500/10 border border-transparent hover:border-red-500/30 transition-all flex items-center gap-1.5 shrink-0 cursor-pointer"
              @click="deleteInstance"
              :title="$t('offline.delete_instance_tooltip')"
            >
              <svg class="w-3.5 h-3.5" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M19 7l-.867 12.142A2 2 0 0116.138 21H7.862a2 2 0 01-1.995-1.858L5 7m5 4v6m4-6v6m1-10V4a1 1 0 00-1-1h-4a1 1 0 00-1 1v3M4 7h16" />
              </svg>
              <span>{{ $t('common.delete') }}</span>
            </button>
          </div>

          <!-- Segmented tab selector matching web-app style -->
          <div class="mt-4 pt-3 border-t border-slate-800/80 flex items-center overflow-x-auto pb-1 max-w-full">
            <div class="z-segmented-track flex-nowrap shrink-0">
              <button
                type="button"
                class="z-segmented-pill shrink-0 whitespace-nowrap"
                :class="{ 'active': activeTab === 'mods' }"
                @click="activeTab = 'mods'"
              >
                {{ $t('server_config.tab_mods', { count: mods.length }) }}
              </button>
              <button
                type="button"
                class="z-segmented-pill shrink-0 whitespace-nowrap"
                :class="{ 'active': activeTab === 'shaders' }"
                @click="activeTab = 'shaders'"
              >
                {{ $t('server_config.tab_shaders', { count: detailedPacks?.shaderpacks?.length || 0 }) }}
              </button>
              <button
                type="button"
                class="z-segmented-pill shrink-0 whitespace-nowrap"
                :class="{ 'active': activeTab === 'resources' || activeTab === 'textures' }"
                @click="activeTab = 'resources'"
              >
                {{ $t('server_config.tab_resourcepacks', { count: detailedPacks?.resourcepacks?.length || 0 }) }}
              </button>
              <button
                type="button"
                class="z-segmented-pill shrink-0 whitespace-nowrap"
                :class="{ 'active': activeTab === 'worlds' }"
                @click="activeTab = 'worlds'; loadWorlds();"
              >
                {{ $t('offline.tab_worlds', { count: worldsList.length }) }}
              </button>
              <button
                type="button"
                class="z-segmented-pill shrink-0 whitespace-nowrap"
                :class="{ 'active': activeTab === 'screenshots' }"
                @click="activeTab = 'screenshots'; loadScreenshots();"
              >
                {{ $t('offline.tab_screenshots', { count: screenshotsList.length }) }}
              </button>
            </div>
          </div>
        </div>


        <!-- Tab contents -->
        <div class="flex flex-col gap-4 relative z-10 pb-6 shrink-0 min-h-fit">
          <!-- ================= TAB: MODS ================= -->
          <template v-if="activeTab === 'mods'">
            <div class="bg-[#070b10] border border-slate-800/90 rounded-xl p-4 shadow-inner shrink-0 min-h-fit">
              <div class="flex items-center justify-between mb-3">
                <div class="flex items-center gap-2.5">
                  <div class="z-section text-white font-bold text-sm">{{ $t('offline.installed_mods', { count: mods.length }) }}</div>
                  <button
                    class="z-btn-ghost text-xs px-2.5 py-1 rounded-lg border border-slate-700 hover:border-slate-600 flex items-center gap-1.5 transition"
                    :disabled="checkingUpdates || updatingMods"
                    @click="checkModUpdates"
                    :title="$t('offline.check_updates_tooltip')"
                  >
                    <span v-if="checkingUpdates" class="w-2.5 h-2.5 border-2 border-cyan-400 border-t-transparent rounded-full animate-spin"></span>
                    <span v-else class="text-cyan-400">⟳</span>
                    <span>{{ checkingUpdates ? $t('offline.checking_updates') : $t('offline.check_updates') }}</span>
                  </button>

                  <span
                    v-if="availableUpdates.length > 0"
                    class="px-2 py-0.5 rounded-full text-[11px] font-bold bg-amber-500/20 text-amber-300 border border-amber-500/30"
                  >
                    {{ $t('offline.updates_available', { count: availableUpdates.length }) }}
                  </span>
                  <button
                    v-if="availableUpdates.length > 0"
                    class="z-btn text-xs px-3 py-1 rounded-lg font-bold bg-amber-500/20 hover:bg-amber-500/30 text-amber-200 border border-amber-500/40 shadow-sm flex items-center gap-1.5 transition"
                    :disabled="updatingMods"
                    @click="applyModUpdates(availableUpdates)"
                    :title="$t('offline.update_all_tooltip')"
                  >
                    <span v-if="updatingMods" class="w-2.5 h-2.5 border-2 border-amber-400 border-t-transparent rounded-full animate-spin"></span>
                    <span>{{ updatingMods ? $t('offline.updating_mods') : $t('offline.update_all') }}</span>
                  </button>

                  <button
                    class="z-btn-ghost text-xs px-2.5 py-1 rounded-lg border border-slate-700 hover:border-slate-600 flex items-center gap-1.5 transition text-cyan-400 hover:text-cyan-300"
                    @click="showSnapshotsModal = true"
                    :title="$t('offline.snapshots_tooltip')"
                  >
                    <svg class="w-3.5 h-3.5 text-cyan-400 shrink-0" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                      <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M3 9a2 2 0 012-2h.93a2 2 0 001.664-.89l.812-1.22A2 2 0 0110.07 4h3.86a2 2 0 011.664.89l.812 1.22A2 2 0 0018.07 7H19a2 2 0 012 2v9a2 2 0 01-2 2H5a2 2 0 01-2-2V9z" />
                      <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M15 13a3 3 0 11-6 0 3 3 0 016 0z" />
                    </svg>
                    <span>{{ $t('offline.snapshots') }}</span>
                  </button>

                  <button
                    class="z-btn-ghost text-xs px-2.5 py-1 rounded-lg border border-slate-700 hover:border-slate-600 flex items-center gap-1.5 transition text-cyan-400 hover:text-cyan-300"
                    @click="showShareModal = true"
                    :title="$t('offline.share_setup_tooltip')"
                  >
                    <svg class="w-3.5 h-3.5 text-cyan-400 shrink-0" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                      <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M8.684 13.342C8.886 12.938 9 12.482 9 12c0-.482-.114-.938-.316-1.342m0 2.684a3 3 0 110-2.684m0 2.684l6.632 3.316m-6.632-6l6.632-3.316m0 0a3 3 0 105.367-2.684 3 3 0 00-5.367 2.684zm0 9.316a3 3 0 105.368 2.684 3 3 0 00-5.368-2.684z" />
                    </svg>
                    <span>{{ $t('offline.share_setup') }}</span>
                  </button>

                  <button
                    class="z-btn-ghost text-xs px-2.5 py-1 rounded-lg border border-slate-700 hover:border-slate-600 flex items-center gap-1.5 transition text-emerald-400 hover:text-emerald-300"
                    @click="showAuditModal = true"
                    :title="$t('offline.audit_repair_tooltip')"
                  >
                    <svg class="w-3.5 h-3.5 text-emerald-400 shrink-0" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                      <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9 12l2 2 4-4m5.618-4.016A11.955 11.955 0 0112 2.944a11.955 11.955 0 01-8.618 3.04A12.02 12.02 0 003 9c0 5.591 3.824 10.29 9 11.622 5.176-1.332 9-6.03 9-11.622 0-1.042-.133-2.052-.382-3.016z" />
                    </svg>
                    <span>{{ $t('offline.audit_repair') }}</span>
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
                  <span>{{ $t('offline.select_all') }}</span>
                </label>
              </div>

              <!-- Rollback Banner -->
              <div
                v-if="updateRollbackBanner"
                class="flex items-center justify-between gap-3 mb-3 px-3.5 py-2.5 rounded-xl border border-amber-500/40 bg-amber-500/10 text-amber-200 text-xs shadow-lg backdrop-blur-md transition-all"
              >
                <div class="flex items-center gap-2.5 min-w-0">
                  <span class="flex h-2 w-2 relative shrink-0">
                    <span class="animate-ping absolute inline-flex h-full w-full rounded-full bg-amber-400 opacity-75"></span>
                    <span class="relative inline-flex rounded-full h-2 w-2 bg-amber-500"></span>
                  </span>
                  <div class="truncate">
                    <span class="font-bold text-amber-100">{{ updateRollbackBanner.count }} mod(s).</span>
                    <span class="text-amber-300/80 ml-1.5 text-[11px]">Time Machine snapshot created.</span>
                  </div>
                </div>
                <div class="flex items-center gap-2 shrink-0">
                  <button
                    type="button"
                    class="px-2.5 py-1 rounded-lg bg-amber-500/20 hover:bg-amber-500/30 text-amber-200 border border-amber-500/40 font-semibold text-xs flex items-center gap-1.5 transition active:scale-95 disabled:opacity-50"
                    :disabled="rollingBack"
                    @click="triggerBatchRollback"
                  >
                    <svg v-if="!rollingBack" class="w-3.5 h-3.5 text-amber-300" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                      <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M3 10h10a8 8 0 018 8v2M3 10l6 6m-6-6l6-6" />
                    </svg>
                    <span v-if="rollingBack" class="w-3 h-3 border-2 border-amber-300 border-t-transparent rounded-full animate-spin"></span>
                    <span>{{ rollingBack ? $t('offline.rolling_back') : $t('offline.one_click_rollback') }}</span>
                  </button>
                  <button
                    type="button"
                    class="p-1 text-amber-400/70 hover:text-amber-200 transition"
                    title="Dismiss"
                    @click="dismissRollbackBanner"
                  >
                    <svg class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                      <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 18L18 6M6 6l12 12" />
                    </svg>
                  </button>
                </div>
              </div>

              <!-- Bulk selection bar -->
              <div
                v-if="selectedModCount > 0"
                class="flex items-center gap-3 mb-3 bg-[#0a121e]/90 border border-accent/40 rounded-xl px-3.5 py-2 text-xs shadow-[0_0_12px_var(--color-accent-glow)]"
              >
                <div class="flex items-center gap-2">
                  <span class="w-2 h-2 rounded-full bg-accent animate-pulse shadow-[0_0_6px_var(--color-accent)]"></span>
                  <span class="text-white font-bold">{{ $t('offline.selected_count', { count: selectedModCount }) }}</span>
                </div>
                <div class="flex-1"></div>
                <button type="button" class="z-btn-ghost text-xs px-3 py-1 rounded-lg font-semibold hover:text-white" @click="bulkEnableSelected">{{ $t('offline.enable') }}</button>
                <button type="button" class="z-btn-ghost text-xs px-3 py-1 rounded-lg font-semibold hover:text-white" @click="bulkDisableSelected">{{ $t('offline.disable') }}</button>
                <button type="button" class="text-xs px-3 py-1 text-red-400 hover:text-red-300 font-semibold hover:bg-red-500/10 rounded-lg transition" @click="bulkDeleteSelected">{{ $t('common.delete') }}</button>
              </div>

              <!-- Category Filter Pills -->
              <div v-if="mods.length > 0" class="flex items-center gap-1.5 mb-3 overflow-x-auto pb-1 text-xs">
                <button
                  v-for="cat in modCategoriesWithCounts"
                  :key="cat.id"
                  type="button"
                  class="px-2.5 py-1 rounded-lg font-semibold transition shrink-0 flex items-center gap-1.5 cursor-pointer select-none"
                  :class="activeModCategory === cat.id ? 'bg-accent/20 text-accent-bright border border-accent/40 shadow-sm font-bold' : 'bg-slate-900/80 text-slate-400 hover:text-white border border-slate-800'"
                  @click="activeModCategory = cat.id"
                >
                  <span>{{ cat.label }}</span>
                  <span class="text-[10px] font-mono px-1.5 py-0.5 rounded-full bg-black/40" :class="activeModCategory === cat.id ? 'text-accent-bright' : 'text-slate-500'">
                    {{ cat.count }}
                  </span>
                </button>
              </div>

              <!-- Mods list with smooth animation reordering and drag handle -->
              <TransitionGroup
                v-if="displayedMods.length"
                name="stack-flip"
                tag="div"
                class="mb-3 flex flex-col gap-1.5"
              >
                <div
                  v-for="(mod, index) in displayedMods"
                  :key="mod.filename"
                  draggable="true"
                  @dragstart="onModDragStart(index, $event)"
                  @dragover.prevent="onModDragOver(index, $event)"
                  @drop.prevent="onModDropReorder(index, $event)"
                  @dragend="onModDragEnd"
                  class="flex items-center gap-2.5 p-2.5 rounded-xl bg-slate-900/60 border border-slate-800/80 text-xs transition hover:border-slate-700 cursor-grab active:cursor-grabbing"
                  :class="[
                    !mod.enabled ? 'opacity-50' : '',
                    modDropTargetIndex === index ? 'border-t-2 border-t-cyan-400 bg-cyan-950/30 shadow-[0_0_12px_rgba(6,182,212,0.15)]' : ''
                  ]"
                >
                  <!-- Drag Handle -->
                  <div class="text-slate-500 hover:text-cyan-400 cursor-grab px-0.5 select-none font-mono text-xs" title="Drag to reorder mod priority">
                    ⠿
                  </div>

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
                      <span
                        v-if="getModCategory(mod)?.id !== 'general'"
                        class="px-1.5 py-0.2 rounded text-[9px] font-bold bg-slate-800/80 text-cyan-300 border border-slate-700/60 shrink-0"
                      >
                        {{ getModCategory(mod)?.label }}
                      </span>
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
              </TransitionGroup>
              <div v-else-if="mods.length > 0" class="py-6 text-center text-slate-500 text-xs">
                {{ $t('offline.no_mods_category') }}
              </div>

              <!-- Custom upload / drop zone -->
              <div
                class="zircon-drop-zone p-4 text-center text-xs text-slate-400 cursor-pointer rounded-xl border border-dashed border-slate-800 hover:border-cyan-500/50 transition"
                @dragover.prevent
                @drop.prevent="onModDrop"
              >
                {{ $t('offline.drop_mods_prefix') }} <code class="text-cyan-300 font-mono">.jar</code> {{ $t('offline.drop_mods_middle') }} <button class="text-cyan-400 underline font-semibold hover:text-cyan-300" @click="browseMods">{{ $t('offline.browse_files') }}</button>{{ $t('offline.drop_mods_suffix') }}
              </div>
            </div>

            <!-- Mod Discovery: Modrinth & CurseForge -->
            <div class="bg-[#070b10] border border-slate-800/90 rounded-xl p-4 shadow-inner shrink-0 min-h-fit">
              <div class="flex items-center justify-between gap-2 mb-3">
                <div class="z-section text-white font-bold text-sm">{{ $t('offline.search_mods_title') }}</div>
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
                  :placeholder="$t('offline.search_mods_placeholder', { provider: modProvider === 'curseforge' ? 'CurseForge' : 'Modrinth' })"
                  @keydown.enter="searchMods"
                />
                <button class="z-btn-ghost px-4 text-xs font-bold shrink-0" :disabled="modSearchBusy" @click="searchMods">{{ $t('common.search') }}</button>
              </div>

              <div class="flex items-center justify-between mt-2.5 text-xs flex-wrap gap-2">
                <div class="flex items-center gap-2 flex-wrap">
                  <div
                    v-if="selected?.gameVersion && !modSearchAllVersions"
                    class="flex items-center gap-1.5 px-2.5 py-1 rounded-lg bg-cyan-500/15 border border-cyan-500/30 text-cyan-300 text-[11px] font-mono font-semibold shadow-[0_0_8px_rgba(71,210,201,0.15)]"
                    :title="`Filtered to Minecraft ${selected.gameVersion} (${selected.modLoader?.type || 'vanilla'})`"
                  >
                    <svg class="w-3 h-3 text-cyan-400 shrink-0" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                      <rect x="3" y="11" width="18" height="11" rx="2" ry="2" />
                      <path d="M7 11V7a5 5 0 0 1 10 0v4" />
                    </svg>
                    <span>Locked to Instance: {{ selected.gameVersion }} ({{ selected.modLoader?.type || 'vanilla' }})</span>
                  </div>

                  <div
                    v-else-if="modSearchAllVersions"
                    class="flex items-center gap-1.5 px-2.5 py-1 rounded-lg bg-amber-500/15 border border-amber-500/30 text-amber-300 text-[11px] font-mono font-semibold"
                    title="Cross-version search active. Results may not match current instance version."
                  >
                    <svg class="w-3 h-3 text-amber-400 shrink-0" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                      <rect x="3" y="11" width="18" height="11" rx="2" ry="2" />
                      <path d="M7 11V7a5 5 0 0 1 9.9-1" />
                    </svg>
                    <span>Unlocked: All Minecraft Versions</span>
                  </div>

                  <label class="flex items-center gap-1.5 cursor-pointer select-none text-slate-400 hover:text-white transition-colors">
                    <input type="checkbox" v-model="modSearchAllVersions" class="zircon-check" @change="searchMods" />
                    <span :class="modSearchAllVersions ? 'text-amber-300 font-semibold' : ''">{{ $t('offline.show_all_versions') }}</span>
                  </label>
                </div>

                <span v-if="modSearchBusy" class="text-cyan-400 font-mono flex items-center gap-1.5 text-xs">
                  <span class="inline-block w-2.5 h-2.5 border-2 border-accent border-t-transparent rounded-full animate-spin"></span>
                  {{ $t('offline.searching') }}
                </span>
              </div>

              <!-- Search results list -->
              <div class="mt-3 flex flex-col gap-2">
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
                          {{ $t('offline.get_curseforge') }}
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
                          <span>{{ installingId === (hit.projectId || hit.id) ? ($t('offline.installing') || 'Installing…') : $t('offline.install_mod') }}</span>
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
            <div class="bg-[#070b10] border border-slate-800/90 rounded-xl p-4 shadow-inner shrink-0 min-h-fit">
              <div class="z-section mb-2 text-white font-bold text-sm">{{ $t('offline.active_shaderpack') }}</div>
              <select v-model="activeShaderpack" class="z-input mb-4 text-xs" @change="onShaderpackChange">
                <option value="">{{ $t('offline.no_shaders') }}</option>
                <option v-for="p in detailedPacks.shaderpacks" :key="p.filename" :value="p.filename">
                  {{ p.title || p.filename }} {{ p.version ? `(${p.version})` : '' }}
                </option>
              </select>

              <div class="z-section mb-2 text-white font-bold text-sm">{{ $t('offline.installed_shaders', { count: detailedPacks.shaderpacks.length }) }}</div>
              <div v-if="detailedPacks.shaderpacks.length" class="mb-3 flex flex-col gap-1.5">
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
                    :title="$t('common.delete')"
                    @click="deletePack('shader', p.filename)"
                  >
                    <svg class="w-3.5 h-3.5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8">
                      <path d="M3 6h18M19 6v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6M8 6V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2" />
                    </svg>
                    <span>{{ $t('common.delete') }}</span>
                  </button>
                </div>
              </div>

              <!-- Custom upload / drop zone for shaders -->
              <div
                class="zircon-drop-zone p-4 text-center text-xs text-slate-400 cursor-pointer rounded-xl border border-dashed border-slate-800 hover:border-cyan-500/50 transition"
                @dragover.prevent
                @drop.prevent="onShaderDrop"
              >
                {{ $t('offline.drop_shaders_prefix') }} <code class="text-cyan-300 font-mono">.zip</code> {{ $t('offline.drop_shaders_middle') }} <button class="text-cyan-400 underline font-semibold hover:text-cyan-300" @click="browseShaders">{{ $t('offline.browse_files') }}</button>{{ $t('offline.drop_shaders_suffix') }}
              </div>
            </div>

            <!-- Shaders Discovery: Modrinth & CurseForge -->
            <div class="bg-[#070b10] border border-slate-800/90 rounded-xl p-4 shadow-inner shrink-0 min-h-fit">
              <div class="flex items-center justify-between gap-2 mb-3">
                <div class="z-section text-white font-bold text-sm">{{ $t('offline.search_shaders_title') }}</div>
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
                  :placeholder="$t('offline.search_shaders_placeholder', { provider: shaderProvider === 'curseforge' ? 'CurseForge' : 'Modrinth' })"
                  @keydown.enter="searchShaders"
                />
                <button class="z-btn-ghost px-4 text-xs font-bold shrink-0" :disabled="shaderSearchBusy" @click="searchShaders">{{ $t('common.search') }}</button>
              </div>

              <div class="flex items-center justify-between mt-2 text-xs text-slate-400">
                <label class="flex items-center gap-1.5 cursor-pointer select-none">
                  <input type="checkbox" v-model="shaderSearchAllVersions" class="zircon-check" @change="searchShaders" />
                  <span>{{ $t('offline.show_all_versions') }}</span>
                </label>
                <span v-if="shaderSearchBusy" class="text-cyan-400 font-mono flex items-center gap-1.5">
                  <span class="inline-block w-2.5 h-2.5 border-2 border-accent border-t-transparent rounded-full animate-spin"></span>
                  {{ $t('offline.searching') }}
                </span>
              </div>

              <!-- Shaders results -->
              <div class="mt-3 flex flex-col gap-2">
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
                          {{ $t('offline.get_curseforge') }}
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
                          <span>{{ installingId === (hit.projectId || hit.id) ? ($t('offline.installing') || 'Installing…') : $t('offline.install_mod') }}</span>
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

          <!-- ================= TAB: RESOURCE PACKS ================= -->
          <template v-else-if="activeTab === 'resources' || activeTab === 'textures'">
            <div class="bg-[#070b10] border border-slate-800/90 rounded-xl p-4 shadow-inner shrink-0 min-h-fit">
              <div class="flex items-center justify-between mb-2">
                <div>
                  <div class="z-section text-white font-bold text-sm">{{ $t('offline.active_resourcepacks', { count: activeResourcepacksList.length }) }}</div>
                  <div class="text-[11px] text-slate-400">{{ $t('offline.rp_priority_desc') }}</div>
                </div>
                <span class="text-[10px] text-cyan-400 font-mono bg-cyan-950/40 border border-cyan-800/60 px-2 py-0.5 rounded-full">
                  {{ $t('offline.priority_stack') }}
                </span>
              </div>

              <!-- Server Resource Packs Offered Banner (if any) -->
              <div
                v-if="serverAdvertisedPacks.length > 0"
                class="mb-3 p-3.5 rounded-xl bg-cyan-950/30 border border-cyan-500/30 flex items-center justify-between gap-3 shadow-[0_0_15px_rgba(6,182,212,0.06)]"
              >
                <div class="flex items-center gap-3">
                  <div class="w-8 h-8 rounded-lg bg-cyan-500/20 text-cyan-400 flex items-center justify-center text-sm font-bold shrink-0">
                    <svg class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                      <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M4 16v1a3 3 0 003 3h10a3 3 0 003-3v-1m-4-8l-4-4m0 0L8 8m4-4v12" />
                    </svg>
                  </div>
                  <div>
                    <div class="text-xs font-bold text-cyan-200">{{ $t('offline.server_rp_title', { count: serverAdvertisedPacks.length }) }}</div>
                    <div class="text-[10px] text-slate-400">{{ $t('offline.server_rp_desc') }}</div>
                  </div>
                </div>
                <button
                  type="button"
                  class="z-btn-primary text-xs px-3.5 py-1.5 rounded-lg font-bold flex items-center gap-1.5 shrink-0"
                  @click="openServerPacksModal"
                >
                  <span>{{ $t('offline.review_server_rp') }}</span>
                </button>
              </div>

              <!-- Active packs stack -->
              <div v-if="activeResourcepacksList.length">
                <TransitionGroup
                  name="stack-flip"
                  tag="div"
                  class="mb-4 flex flex-col gap-1.5"
                >
                  <div
                    v-for="(p, index) in activeResourcepacksList"
                    :key="p.filename"
                    draggable="true"
                    @dragstart="onRpDragStart(index, $event)"
                    @dragover.prevent="onRpDragOver(index, $event)"
                    @drop.prevent="onRpDrop(index, $event)"
                    @dragend="onRpDragEnd"
                    class="flex items-center gap-2.5 text-xs p-2.5 rounded-xl bg-slate-900/80 border border-cyan-500/30 shadow-sm transition hover:border-cyan-500/60 cursor-grab active:cursor-grabbing"
                    :class="{ 'border-t-2 border-t-cyan-400 bg-cyan-950/30 shadow-[0_0_12px_rgba(6,182,212,0.15)]': rpDropTargetIndex === index }"
                  >
                    <!-- Drag handle -->
                    <div class="text-slate-500 hover:text-cyan-400 cursor-grab px-0.5 select-none font-mono text-xs" title="Drag to reorder priority">
                      ⠿
                    </div>

                    <!-- Priority badge -->
                    <div class="px-2 py-0.5 rounded-md font-mono font-bold text-[10px] bg-cyan-500/20 text-cyan-300 border border-cyan-500/40 shrink-0" :title="index === 0 ? 'Highest Visual Priority' : `Priority Level ${index + 1}`">
                      #{{ index + 1 }} {{ index === 0 ? 'Top' : '' }}
                    </div>

                    <div class="w-8 h-8 rounded-lg bg-slate-950 border border-slate-800 flex items-center justify-center shrink-0 overflow-hidden shadow-inner">
                      <img v-if="p.iconDataUrl" :src="p.iconDataUrl" class="w-full h-full object-cover" />
                      <svg v-else class="w-4 h-4 text-cyan-400/60" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8">
                        <rect x="3" y="3" width="18" height="18" rx="2" ry="2" />
                        <line x1="3" y1="9" x2="21" y2="9" />
                        <line x1="9" y1="21" x2="9" y2="9" />
                      </svg>
                    </div>

                    <div class="flex-1 min-w-0">
                      <div class="flex items-center gap-1.5">
                        <span class="text-white font-semibold truncate">{{ p.title || p.filename }}</span>
                        <span v-if="p.serverEnforced" class="px-1.5 py-0.5 rounded text-[9px] font-bold bg-amber-500/20 text-amber-300 border border-amber-500/40 shrink-0 flex items-center gap-1">
                          <svg class="w-2.5 h-2.5 text-amber-400 shrink-0" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                            <rect width="18" height="11" x="3" y="11" rx="2" ry="2" />
                            <path d="M7 11V7a5 5 0 0 1 10 0v4" />
                          </svg>
                          Server Enforced
                        </span>
                      </div>
                      <div v-if="p.description" class="text-[10px] text-slate-400 truncate">{{ p.description }}</div>
                    </div>

                    <span class="bg-slate-950 text-slate-400 border border-slate-800 text-[10px] px-1.5 py-0.2 rounded font-mono shrink-0">
                      {{ p.version || (p.packFormat ? 'v' + p.packFormat : 'Pack') }}
                    </span>
                    <span class="text-slate-500 font-mono text-[11px] shrink-0">{{ fmtBytes(p.sizeBytes) }}</span>

                    <!-- Reorder buttons -->
                    <div class="flex items-center gap-1 shrink-0 ml-1">
                      <button
                        class="z-btn-ghost text-xs w-6 h-6 p-0 flex items-center justify-center rounded border border-slate-700 hover:border-cyan-400 hover:text-cyan-300 disabled:opacity-25 disabled:pointer-events-none"
                        :disabled="index === 0"
                        title="Move Up (Higher Priority)"
                        @click.stop="moveResourcepack(index, -1)"
                      >
                        ▲
                      </button>
                      <button
                        class="z-btn-ghost text-xs w-6 h-6 p-0 flex items-center justify-center rounded border border-slate-700 hover:border-cyan-400 hover:text-cyan-300 disabled:opacity-25 disabled:pointer-events-none"
                        :disabled="index === activeResourcepacksList.length - 1"
                        title="Move Down (Lower Priority)"
                        @click.stop="moveResourcepack(index, 1)"
                      >
                        ▼
                      </button>
                      <button
                        class="text-slate-400 hover:text-amber-300 text-xs px-2 py-0.5 hover:bg-amber-500/10 rounded transition font-medium ml-1"
                        title="Deactivate Pack"
                        @click.stop="disableResourcepack(p.filename)"
                      >
                        ✕
                      </button>
                    </div>
                  </div>
                </TransitionGroup>
              </div>
              <div v-else class="text-xs text-slate-500 mb-4 italic py-2">No resource packs currently active. Add one from the available list below.</div>

              <!-- Available / Inactive Packs -->
              <div class="z-section mb-2 text-white font-bold text-sm">Available Packs ({{ inactiveResourcepacksList.length }})</div>
              <div v-if="inactiveResourcepacksList.length" class="mb-3 flex flex-col gap-1.5">
                <div
                  v-for="p in inactiveResourcepacksList"
                  :key="p.filename"
                  class="flex items-center gap-2.5 text-xs p-2.5 rounded-xl bg-slate-900/40 border border-slate-800/80 transition hover:border-slate-700 opacity-80 hover:opacity-100"
                >
                  <div class="flex-1 min-w-0">
                    <div class="text-slate-300 font-medium truncate">{{ p.title || p.filename }}</div>
                    <div v-if="p.description" class="text-[10px] text-slate-500 truncate">{{ p.description }}</div>
                  </div>

                  <span class="bg-slate-950 text-slate-500 border border-slate-800 text-[10px] px-1.5 py-0.2 rounded font-mono shrink-0">
                    {{ p.version || (p.packFormat ? 'v' + p.packFormat : 'Pack') }}
                  </span>
                  <span class="text-slate-500 font-mono text-[11px] shrink-0">{{ fmtBytes(p.sizeBytes) }}</span>

                  <div class="flex items-center gap-2 shrink-0 ml-1">
                    <button
                      class="z-btn text-xs px-2.5 py-1 rounded-lg font-bold bg-cyan-500/20 hover:bg-cyan-500/30 text-cyan-300 border border-cyan-500/40 transition flex items-center gap-1"
                      title="Add to active resource packs stack"
                      @click="enableResourcepack(p.filename)"
                    >
                      <span>+ Enable</span>
                    </button>
                    <button
                      class="text-red-400 hover:text-red-300 text-xs px-2 py-0.5 hover:bg-red-500/10 rounded-lg transition-colors font-medium flex items-center gap-1"
                      title="Delete"
                      @click="deletePack('resource', p.filename)"
                    >
                      <svg class="w-3.5 h-3.5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8">
                        <path d="M3 6h18M19 6v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6M8 6V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2" />
                      </svg>
                      <span>Delete</span>
                    </button>
                  </div>
                </div>
              </div>
              <div v-else class="text-xs text-slate-500 mb-3 italic py-1">All installed resource packs are currently active.</div>

              <!-- Custom upload / drop zone for resource packs -->
              <div
                class="zircon-drop-zone p-4 text-center text-xs text-slate-400 cursor-pointer rounded-xl border border-dashed border-slate-800 hover:border-cyan-500/50 transition"
                @dragover.prevent
                @drop.prevent="onResourceDrop"
              >
                {{ $t('offline.drop_rp_prefix') }} <code class="text-cyan-300 font-mono">.zip</code> {{ $t('offline.drop_rp_middle') }} <button class="text-cyan-400 underline font-semibold hover:text-cyan-300" @click="browseResources">{{ $t('offline.browse_files') }}</button>{{ $t('offline.drop_rp_suffix') }}
              </div>
            </div>

            <!-- Resource Packs Discovery: Modrinth & CurseForge -->
            <div class="bg-[#070b10] border border-slate-800/90 rounded-xl p-4 shadow-inner shrink-0 min-h-fit">
              <div class="flex items-center justify-between gap-2 mb-3">
                <div class="z-section text-white font-bold text-sm">{{ $t('offline.search_rp_title') }}</div>
                <div class="flex items-center gap-1 bg-slate-900 border border-slate-800 rounded-xl p-0.5">
                  <button
                    class="flex items-center gap-1.5 px-3 py-1 rounded-lg text-xs font-bold transition-all"
                    :class="resourceProvider === 'modrinth' ? 'bg-[#1bd96a]/20 text-[#46d66d] border border-[#1bd96a]/40 shadow-sm' : 'text-slate-400 hover:text-white'"
                    @click="setResourceProvider('modrinth')"
                  >
                    <img src="../assets/modrinth.svg" class="w-3.5 h-3.5" />
                    Modrinth
                  </button>
                  <button
                    class="flex items-center gap-1.5 px-3 py-1 rounded-lg text-xs font-bold transition-all"
                    :class="resourceProvider === 'curseforge' ? 'bg-[#f16436]/20 text-[#f16436] border border-[#f16436]/40 shadow-sm' : 'text-slate-400 hover:text-white'"
                    @click="setResourceProvider('curseforge')"
                  >
                    <img src="../assets/curseforge.svg" class="w-3.5 h-3.5" />
                    CurseForge
                  </button>
                </div>
              </div>

              <div class="flex gap-2">
                <input
                  v-model="resourceSearchQuery"
                  class="z-input flex-1 text-xs"
                  :placeholder="$t('offline.search_rp_placeholder', { provider: resourceProvider === 'curseforge' ? 'CurseForge' : 'Modrinth' })"
                  @keydown.enter="searchResources"
                />
                <button class="z-btn-ghost px-4 text-xs font-bold shrink-0" :disabled="resourceSearchBusy" @click="searchResources">{{ $t('common.search') }}</button>
              </div>

              <div class="flex items-center justify-between mt-2 text-xs text-slate-400">
                <label class="flex items-center gap-1.5 cursor-pointer select-none">
                  <input type="checkbox" v-model="resourceSearchAllVersions" class="zircon-check" @change="searchResources" />
                  <span>{{ $t('offline.show_all_versions') }}</span>
                </label>
                <span v-if="resourceSearchBusy" class="text-cyan-400 font-mono flex items-center gap-1.5">
                  <span class="inline-block w-2.5 h-2.5 border-2 border-accent border-t-transparent rounded-full animate-spin"></span>
                  {{ $t('offline.searching') }}
                </span> <!-- end busy spinner -->
              </div>

              <!-- Resource results -->
              <div class="mt-3 flex flex-col gap-2">
                <div
                  v-for="hit in resourceResults"
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
                          v-if="hit.origin === 'curseforge' || resourceProvider === 'curseforge'"
                          class="text-xs px-3 py-1.5 rounded-lg font-bold shrink-0 flex items-center gap-1.5 transition disabled:opacity-50 disabled:cursor-not-allowed bg-[#F16436]/20 text-orange-200 border border-[#F16436]/40 hover:bg-[#F16436]/30 shadow-sm"
                          @click="openCurseforgeModal(hit, 'resourcepack')"
                        >
                          {{ $t('offline.get_curseforge') }}
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
                          <span>{{ installingId === (hit.projectId || hit.id) ? ($t('offline.installing') || 'Installing…') : $t('offline.install_mod') }}</span>
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
                          :class="(hit.origin === 'curseforge' || resourceProvider === 'curseforge') ? 'text-[#F16436]' : 'text-[#46d66d]'"
                          @click.prevent="openExternalLink(hit.projectUrl || hit.websiteUrl)"
                        >
                          {{ (hit.origin === 'curseforge' || resourceProvider === 'curseforge') ? 'View on CurseForge ↗' : 'View on Modrinth ↗' }}
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
                <div v-if="!resourceSearchBusy && resourceSearchDone && resourceResults.length === 0" class="text-xs text-slate-500 py-2">
                  No resource packs found.
                </div>
              </div>
            </div>
          </template>

          <!-- ================= TAB: WORLDS & SAVES ================= -->
          <template v-else-if="activeTab === 'worlds'">
            <div class="bg-[#070b10] border border-slate-800/90 rounded-xl p-4 shadow-inner flex flex-col gap-4 shrink-0 min-h-fit">
              <div class="flex items-center justify-between">
                <div class="flex items-center gap-2.5">
                  <div class="z-section text-white font-bold text-sm">{{ $t('offline.singleplayer_worlds', { count: worldsList.length }) }}</div>
                  <button
                    class="z-btn-ghost text-xs px-2.5 py-1 rounded-lg border border-slate-700 hover:border-slate-600 flex items-center gap-1.5 transition"
                    :disabled="loadingWorldsState"
                    @click="loadWorlds"
                    :title="$t('common.refresh')"
                  >
                    <span v-if="loadingWorldsState" class="w-2.5 h-2.5 border-2 border-cyan-400 border-t-transparent rounded-full animate-spin"></span>
                    <span v-else class="text-cyan-400">⟳</span>
                    <span>{{ $t('common.refresh') }}</span>
                  </button>
                </div>
                <div class="flex items-center gap-2">
                  <!-- Temporarily disabled for v0.5.0: Seed Radar HUD
                  <button
                    class="z-btn-accent text-xs px-3 py-1.5 rounded-lg flex items-center gap-1.5 transition font-bold"
                    @click="showRadarModal = true"
                    title="Open Seed & Biome Radar Visualizer"
                  >
                    <svg class="w-3.5 h-3.5 text-slate-900" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.2">
                      <circle cx="12" cy="12" r="9" stroke-dasharray="3 2" />
                      <circle cx="12" cy="12" r="3" fill="currentColor" fill-opacity="0.3" />
                    </svg>
                    <span>{{ $t('offline.seed_radar_hud') }}</span>
                  </button>
                  -->
                  <button
                    class="z-btn-ghost text-xs px-3 py-1.5 rounded-lg border border-slate-700 hover:border-slate-600 flex items-center gap-1.5 transition"
                    @click="openBackupsModal(null)"
                    title="View all world backup snapshots"
                  >
                    <svg class="w-3.5 h-3.5 text-cyan-400" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                      <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 8v4l3 3m6-3a9 9 0 11-18 0 9 9 0 0118 0z" />
                    </svg>
                    <span>{{ $t('offline.all_backups') }}</span>
                  </button>
                </div>
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
                    <!-- Temporarily disabled for v0.5.0: World Radar
                    <button
                      class="z-btn-ghost text-xs px-2.5 py-1 rounded-lg border border-cyan-500/30 hover:border-cyan-500/60 text-cyan-300 flex items-center gap-1.5 transition"
                      @click="showRadarModal = true"
                      title="Inspect this world in the Seed & Biome Radar Visualizer"
                    >
                      <svg class="w-3.5 h-3.5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                        <circle cx="12" cy="12" r="9" stroke-dasharray="3 2" />
                        <circle cx="12" cy="12" r="3" fill="currentColor" fill-opacity="0.3" />
                      </svg>
                      <span>{{ $t('offline.radar') }}</span>
                    </button>
                    -->
                    <button
                      class="z-btn-ghost text-xs px-2.5 py-1 rounded-lg border border-slate-700 hover:border-slate-600 flex items-center gap-1.5 transition"
                      @click="createWorldBackup(w.folderName)"
                      title="Create an instant timestamped backup of this world"
                    >
                      <svg class="w-3.5 h-3.5 text-emerald-400" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M8 7H5a2 2 0 00-2 2v9a2 2 0 002 2h14a2 2 0 002-2V9a2 2 0 00-2-2h-3m-1 4l-3 3m0 0l-3-3m3 3V4" />
                      </svg>
                      <span>{{ $t('offline.backup') }}</span>
                    </button>
                    <button
                      class="z-btn-ghost text-xs px-2.5 py-1 rounded-lg border border-slate-700 hover:border-slate-600 flex items-center gap-1.5 transition"
                      @click="openBackupsModal(w.folderName)"
                      title="View and restore previous backups for this world"
                    >
                      <span>{{ $t('offline.snapshots') }}</span>
                    </button>
                    <button
                      class="z-btn-ghost text-xs px-2.5 py-1 rounded-lg border border-slate-700 hover:border-slate-600 flex items-center gap-1.5 transition"
                      @click="exportWorldZip(w.folderName)"
                      title="Export world as a .zip anywhere on your disk"
                    >
                      <span>{{ $t('offline.export') }}</span>
                    </button>
                  </div>
                </div>
              </div>
              <div v-else-if="!loadingWorldsState" class="py-8 text-center text-slate-500 text-xs">
                {{ $t('offline.no_worlds') }}
              </div>
            </div>
          </template>

          <!-- ================= TAB: SCREENSHOTS ================= -->
          <template v-else-if="activeTab === 'screenshots'">
            <div class="bg-[#070b10] border border-slate-800/90 rounded-xl p-4 shadow-inner flex flex-col gap-4 shrink-0 min-h-fit">
              <div class="flex items-center justify-between">
                <div class="flex items-center gap-2.5">
                  <div class="z-section text-white font-bold text-sm">{{ $t('offline.screenshots_gallery', { count: screenshotsList.length }) }}</div>
                  <button
                    class="z-btn-ghost text-xs px-2.5 py-1 rounded-lg border border-slate-700 hover:border-slate-600 flex items-center gap-1.5 transition"
                    :disabled="loadingScreenshotsState"
                    @click="loadScreenshots"
                    :title="$t('common.refresh')"
                  >
                    <span v-if="loadingScreenshotsState" class="w-2.5 h-2.5 border-2 border-cyan-400 border-t-transparent rounded-full animate-spin"></span>
                    <span v-else class="text-cyan-400">⟳</span>
                    <span>{{ $t('common.refresh') }}</span>
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
                  <span>{{ $t('offline.open_folder') }}</span>
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
  </div>

    <!-- ================= MODAL: CURSEFORGE COUNTDOWN & DROP ================= -->
    <PackSyncModal
      :modal="curseforgeModal"
      @close="closeCurseforgeModal"
      @trigger-download="triggerCurseforgeDownload"
      @drop="onCurseforgeModalDrop"
      @browse="browseCurseforgeModalFile"
    />

    <!-- ================= MODAL: NEW INSTANCE ================= -->
    <InstanceCreationModal
      :open="showNewDialog"
      :active-tab="newDialogTab"
      :modpack-progress="modpackProgress"
      :new-form="newForm"
      :mc-versions="mcVersions"
      :loader-types="loaderTypes"
      :loader-versions="loaderVersions"
      :loading-loader-versions="loadingLoaderVersions"
      :recommended-loader-version="recommendedLoaderVersion"
      :creating="creating"
      :import-mode="importMode"
      :import-file-path="importFilePath"
      :import-custom-name="importCustomName"
      :importing-modpack="importingModpack"
      :modpack-search-query="modpackSearchQuery"
      :modpack-search-busy="modpackSearchBusy"
      :modpack-search-done="modpackSearchDone"
      :modpack-results="modpackResults"
      :installing-modpack-id="installingModpackId"
      :selected-modpack-versions="selectedModpackVersions"
      :instances="instances"
      :clone-source-id="cloneSourceId"
      :clone-name="cloneName"
      :clone-loader-type="cloneLoaderType"
      :clone-loader-version="cloneLoaderVersion"
      :clone-loader-versions="cloneLoaderVersions"
      :loading-clone-loader-versions="loadingCloneLoaderVersions"
      :clone-recommended-loader-version="cloneRecommendedLoaderVersion"
      :is-clone-loader-changed="isCloneLoaderChanged"
      :selected-clone-source="selectedCloneSource"
      :clone-copy-world="cloneCopyWorld"
      :clone-copy-mods="cloneCopyMods"
      :cloning="cloning"
      :fmt-count="fmtCount"
      @close="showNewDialog = false"
      @migrate="showMigrationModal = true"
      @update:active-tab="newDialogTab = $event"
      @update:import-custom-name="importCustomName = $event"
      @update:modpack-search-query="modpackSearchQuery = $event"
      @update:clone-source-id="cloneSourceId = $event"
      @update:clone-name="cloneName = $event"
      @update:clone-loader-type="cloneLoaderType = $event"
      @update:clone-loader-version="cloneLoaderVersion = $event"
      @update:clone-copy-world="cloneCopyWorld = $event"
      @update:clone-copy-mods="cloneCopyMods = $event"
      @update-loader-versions="updateLoaderVersions"
      @create="createInstance"
      @toggle-import-mode="toggleImportMode"
      @browse-archive="browseLocalMrpack"
      @drop-archive="onImportDrop"
      @install-archive="installLocalMrpack"
      @search-modpacks="searchModpacks"
      @install-modpack="installModpack"
      @clone-source-change="onCloneSourceChange"
      @update-clone-loader-versions="updateCloneLoaderVersions"
      @clone="executeCloneInstance"
    />

    <!-- ================= MODAL: WORLD BACKUPS ================= -->
    <WorldManagerModal
      :open="showBackupsDialog"
      :filter-world="filterBackupWorld"
      :loading="loadingBackups"
      :backups="filteredBackups"
      :fmt-size="fmtSize"
      :format-relative-time="formatRelativeTime"
      @close="showBackupsDialog = false"
      @restore="restoreBackup"
      @delete="deleteBackup"
    />

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
            <h3 class="text-white font-bold text-base">{{ $t('coop_host.title') }}</h3>
          </div>
          <button class="text-slate-400 hover:text-white text-lg leading-none" @click="showCoopModal = false">✕</button>
        </div>

        <p class="text-xs text-slate-300 mb-4 leading-relaxed">
          {{ $t('coop_host.subtitle') }}
        </p>

        <div class="bg-slate-950 border border-cyan-500/40 rounded-2xl p-4 text-center mb-4 shadow-inner">
          <div class="text-[10px] font-bold text-cyan-300 uppercase tracking-widest mb-1">{{ $t('coop_host.join_code_label') }}</div>
          <div class="text-3xl font-mono font-black tracking-wider text-white select-all">
            {{ coopSession?.joinCode || 'GENERATING…' }}
          </div>
          <div class="text-[11px] text-slate-400 mt-1">{{ $t('coop_host.hosted_world') }} <strong>{{ coopSession?.worldName }}</strong></div>
          <div v-if="coopSession?.upnp?.externalIp" class="text-[11px] font-mono text-cyan-300 mt-0.5">
            {{ $t('coop_host.detected_ip') }} <span class="font-bold select-all">{{ coopSession.upnp.externalIp }}</span>
          </div>
          <div class="text-[10px] font-mono text-slate-500 mt-0.5">
            {{ $t('coop_host.game_port') }}: {{ coopSession?.gamePort || 25565 }} · {{ $t('coop_host.sync_port') }}: {{ coopSession?.p2pPort || 25566 }}
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
              {{ (coopSession.upnp.available && coopSession.upnp.gamePortMapped && coopSession.upnp.p2pPortMapped) ? $t('coop_host.upnp_active') : $t('coop_host.upnp_unavailable') }}
            </strong>
            <span class="text-slate-300">
              {{ (coopSession.upnp.available && coopSession.upnp.gamePortMapped && coopSession.upnp.p2pPortMapped)
                ? $t('coop_host.upnp_active_desc', { game: '25565', p2p: '25566' })
                : $t('coop_host.upnp_unavailable_desc') }}
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
            <span>{{ codeCopied ? $t('coop_host.copied_feedback') : $t('coop_host.copy_code_tooltip') }}</span>
          </button>
        </div>

        <div class="flex items-center justify-between pt-3 border-t border-slate-800/80">
          <button class="text-xs text-red-400 hover:text-red-300 font-semibold" @click="stopCoop">
            {{ $t('coop_host.stop_hosting') }}
          </button>
          <button class="z-btn-ghost text-xs px-4 py-2 rounded-xl font-semibold" @click="showCoopModal = false">
            {{ $t('common.close') }}
          </button>
        </div>
      </div>
    </div>

    <!-- JOIN BY CODE MODAL -->
    <JoinByCodeModal
      :open="showJoinCodeModal"
      :instances="instances"
      :default-instance-id="selected?.id"
      :initial-code="deepLinkCode"
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

    <!-- SHARE SETUP MODAL -->
    <ShareSetupModal
      :open="showShareModal"
      :instance="selected"
      :instance-id="selected?.id"
      :instance-name="selected?.name"
      :mod-count="mods.length"
      @close="showShareModal = false"
      @edit-version="openEditInstanceModal"
    />

    <!-- EDIT INSTANCE MODAL -->
    <EditInstanceModal
      :open="showEditInstanceModal"
      :instance="selected"
      :mc-versions="mcVersions"
      @close="showEditInstanceModal = false"
      @saved="onInstanceUpdated"
    />

    <!-- IMPORT SETUP MODAL -->
    <ImportSetupModal
      :open="showImportModal"
      :target-instance="selected"
      :current-instance="selected"
      :initial-file-path="droppedMrpackPath"
      @close="closeImportModal"
      @applied="onSetupImported"
      @imported="onSetupImported"
    />

    <!-- TIME MACHINE SNAPSHOTS MODAL -->
    <SnapshotsModal
      :open="showSnapshotsModal"
      :instance-id="selected?.id"
      :instance-name="selected?.name"
      @close="showSnapshotsModal = false"
      @restored="onSnapshotRestored"
    />

    <!-- AUDIT & REPAIR MODAL -->
    <AuditRepairModal
      :open="showAuditModal"
      :instance-id="selected?.id"
      :instance-name="selected?.name"
      @close="showAuditModal = false"
      @repaired="onModsRepaired"
    />

    <!-- SERVER RESOURCE PACKS OFFERED MODAL -->
    <ServerResourcePacksModal
      :is-open="showServerPacksModal"
      :server-packs="serverAdvertisedPacks"
      :initial-decisions="serverPackDecisions"
      @accepted="onServerPacksAccepted"
      @close="showServerPacksModal = false"
    />

    <!-- UNIVERSAL LAUNCHER MIGRATION MODAL -->
    <LauncherMigrationModal
      :open="showMigrationModal"
      @close="showMigrationModal = false"
      @imported="onInstancesMigrated"
    />

        <!-- GLOBAL KEYBIND LIBRARY & CROSS-INSTANCE SYNC MODAL -->
    <KeybindManagerModal
      :open="showKeybindModal"
      :target-instance="selected"
      @close="showKeybindModal = false"
    />

    <!-- COUCH CO-OP SPLIT-SCREEN MODAL -->
    <CouchCoopModal
      :is-open="showCouchCoopModal"
      :instance="selected"
      @close="showCouchCoopModal = false"
    />

    <!-- Temporarily disabled for v0.5.0: WORLD SEED & BIOME RADAR MODAL
    <WorldSeedRadarModal
      :open="showRadarModal"
      :instance="selected"
      @close="showRadarModal = false"
    />
    -->
  </div>
</template>

<script setup>
import { computed, defineAsyncComponent, onBeforeUnmount, onMounted, ref, watch } from 'vue';
import { getCurrentWebview } from '@tauri-apps/api/webview';
import InstanceCard from '../components/offline/InstanceCard.vue';
import InstanceCreationModal from '../components/offline/InstanceCreationModal.vue';
import EditInstanceModal from '../components/offline/EditInstanceModal.vue';

// Lazy-loaded secondary modals
const DependencyPromptModal = defineAsyncComponent(() => import('../components/DependencyPromptModal.vue'));
const ServerResourcePacksModal = defineAsyncComponent(() => import('../components/ServerResourcePacksModal.vue'));
const JoinByCodeModal = defineAsyncComponent(() => import('../components/JoinByCodeModal.vue'));
const ShareSetupModal = defineAsyncComponent(() => import('../components/ShareSetupModal.vue'));
const ImportSetupModal = defineAsyncComponent(() => import('../components/ImportSetupModal.vue'));
const SnapshotsModal = defineAsyncComponent(() => import('../components/SnapshotsModal.vue'));
const AuditRepairModal = defineAsyncComponent(() => import('../components/AuditRepairModal.vue'));
const LauncherMigrationModal = defineAsyncComponent(() => import('../components/LauncherMigrationModal.vue'));
const CouchCoopModal = defineAsyncComponent(() => import('../components/CouchCoopModal.vue'));
const KeybindManagerModal = defineAsyncComponent(() => import('../components/KeybindManagerModal.vue'));
const WorldSeedRadarModal = defineAsyncComponent(() => import('../components/WorldSeedRadarModal.vue'));
const PackSyncModal = defineAsyncComponent(() => import('../components/offline/PackSyncModal.vue'));
const WorldManagerModal = defineAsyncComponent(() => import('../components/offline/WorldManagerModal.vue'));
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
import { askConfirm } from '../lib/confirm';

const emit = defineEmits(['launching', 'stopped', 'error']);

const props = defineProps({
  session: { type: Object, default: null },
  deepLinkCode: { type: String, default: '' },
});

const instances = ref([]);
const instanceArtworks = ref({});
const selected = ref(null);
const selectedDir = ref('');
const activeTab = ref('mods'); // 'mods' | 'shaders' | 'textures' | 'worlds' | 'screenshots'


watch(
  () => props.deepLinkCode,
  (code) => {
    if (code) {
      showJoinCodeModal.value = true;
    }
  },
  { immediate: true }
);

function instanceBanner(instance) {
  if (!instance) return null;
  const art = instanceArtworks.value[instance.id];
  if (!art) return null;
  if (art.bannerDataUrl) return art.bannerDataUrl;
  if (art.panoramaDataUrls && art.panoramaDataUrls.length > 0) {
    return art.panoramaDataUrls[0];
  }
  return null;
}

const instanceBannerUrls = computed(() => {
  const map = {};
  for (const inst of instances.value || []) {
    map[inst.id] = instanceBanner(inst);
  }
  return map;
});

const instanceIconUrls = computed(() => {
  const map = {};
  for (const inst of instances.value || []) {
    map[inst.id] = instanceArtworks.value[inst.id]?.iconDataUrl || null;
  }
  return map;
});

// View Mode, Search, Filter & Sort
const viewMode = ref('grid'); // 'grid' | 'split'
const isStudioFocused = ref(false);
const searchQuery = ref('');
const filterLoader = ref('all'); // 'all' | 'fabric' | 'neoforge' | 'forge' | 'vanilla'
const sortBy = ref('recent'); // 'recent' | 'name' | 'version'
const showToolsDropdown = ref(false);
const activeCardDropdownId = ref(null);

const filteredInstances = computed(() => {
  let list = [...(instances.value || [])];
  if (searchQuery.value && searchQuery.value.trim()) {
    const q = searchQuery.value.toLowerCase().trim();
    list = list.filter((i) =>
      (i.name && i.name.toLowerCase().includes(q)) ||
      (i.minecraftVersion && i.minecraftVersion.toLowerCase().includes(q)) ||
      (i.modLoader?.type && i.modLoader.type.toLowerCase().includes(q))
    );
  }
  if (filterLoader.value !== 'all') {
    list = list.filter((i) =>
      (i.modLoader?.type || 'vanilla').toLowerCase() === filterLoader.value.toLowerCase()
    );
  }
  if (sortBy.value === 'recent') {
    list.sort((a, b) => {
      const tA = a.lastPlayed ? new Date(a.lastPlayed).getTime() : 0;
      const tB = b.lastPlayed ? new Date(b.lastPlayed).getTime() : 0;
      return tB - tA;
    });
  } else if (sortBy.value === 'name') {
    list.sort((a, b) => (a.name || '').localeCompare(b.name || ''));
  } else if (sortBy.value === 'version') {
    list.sort((a, b) => (b.minecraftVersion || '').localeCompare(a.minecraftVersion || ''));
  }
  return list;
});

function openManageStudio(instance) {
  selectInstance(instance);
  isStudioFocused.value = true;
}

function closeStudio() {
  isStudioFocused.value = false;
}

function onSearchFocus() {
  if (isStudioFocused.value) {
    isStudioFocused.value = false;
  }
}

watch(searchQuery, (newVal) => {
  if (newVal && newVal.trim() && isStudioFocused.value) {
    isStudioFocused.value = false;
  }
});

async function playInstanceDirectly(instance) {
  await selectInstance(instance);
  await playOffline();
}

function toggleCardDropdown(instanceId, e) {
  e?.stopPropagation();
  activeCardDropdownId.value = activeCardDropdownId.value === instanceId ? null : instanceId;
}

function cloneInstanceCard(instance) {
  activeCardDropdownId.value = null;
  selected.value = instance;
  promptCloneInstance();
}

async function openInstanceFolderCard(instance) {
  activeCardDropdownId.value = null;
  try {
    await api.openInstanceFolder(instance.id, null);
  } catch (err) {
    window.dispatchEvent(new CustomEvent('zircon-status', { detail: `Could not open folder: ${err}` }));
  }
}

async function exportMrpackCard(instance) {
  activeCardDropdownId.value = null;
  selected.value = instance;
  await exportMrpack();
}

async function deleteInstanceById(instance) {
  if (!instance) return;
  activeCardDropdownId.value = null;
  const ok = await askConfirm({
    title: 'Delete Instance',
    message: `Are you sure you want to delete '${instance.name}' and all of its files? This action cannot be undone.`,
    confirmText: 'Delete Instance',
    isDanger: true,
  });
  if (!ok) return;
  await api.deleteOfflineInstance(instance.id);
  if (selected.value?.id === instance.id) {
    selected.value = null;
    isStudioFocused.value = false;
  }
  await loadInstances();
}

function openInstanceStudio(instance) {
  openManageStudio(instance);
}

function openCloneModal(instance) {
  cloneInstanceCard(instance);
}

function exportInstanceAsMrpack(instance) {
  exportMrpackCard(instance);
}

function installCinematicPresets(instance) {
  installCinematicPresetCard(instance);
}

// Actions & Open folder dropdowns
const showOpenFolderDropdown = ref(false);
const showActionsDropdown = ref(false);

// Clone Modal & Parameters
const cloneSourceId = ref('');
const cloneName = ref('');
const cloneLoaderType = ref('vanilla');
const cloneLoaderVersion = ref('');
const cloneLoaderVersions = ref([]);
const cloneRecommendedLoaderVersion = ref('');
const loadingCloneLoaderVersions = ref(false);
const cloneCopyWorld = ref(true);
const cloneCopyMods = ref(true);
const cloning = ref(false);

const selectedCloneSource = computed(() => {
  return instances.value.find((i) => i.id === cloneSourceId.value) || null;
});

const isCloneLoaderChanged = computed(() => {
  if (!selectedCloneSource.value || !cloneLoaderType.value) return false;
  const original = selectedCloneSource.value.modLoader?.type || 'vanilla';
  return cloneLoaderType.value.toLowerCase() !== original.toLowerCase();
});

// Worlds state
const worldsList = ref([]);
const loadingWorldsState = ref(false);
const showRadarModal = ref(false);

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

// Setup Sharing, Snapshots & Audit Modals
const showShareModal = ref(false);
const showImportModal = ref(false);
const droppedMrpackPath = ref('');
const showSnapshotsModal = ref(false);
const showAuditModal = ref(false);
const showMigrationModal = ref(false);
const showCouchCoopModal = ref(false);
const showKeybindModal = ref(false);
const showEditInstanceModal = ref(false);

function openEditInstanceModal() {
  showActionsDropdown.value = false;
  showEditInstanceModal.value = true;
}

async function onInstanceUpdated(updated) {
  await loadInstances();
  if (updated && updated.id) {
    const match = instances.value.find((i) => i.id === updated.id);
    if (match) {
      await selectInstance(match);
    }
  }
}

function closeImportModal() {
  showImportModal.value = false;
  droppedMrpackPath.value = '';
}

async function onInstancesMigrated(newInstances) {
  await loadInstances();
  if (newInstances && newInstances.length > 0) {
    const firstId = newInstances[0].id;
    const target = instances.value.find((i) => i.id === firstId);
    if (target) {
      selectInstance(target);
    }
  }
}

async function onSetupImported(res) {
  showImportModal.value = false;
  droppedMrpackPath.value = '';
  await loadInstances();
  const targetId = res?.id || res?.instanceId;
  if (targetId) {
    const target = instances.value.find((i) => i.id === targetId);
    if (target) {
      selectInstance(target);
      return;
    }
  }
  if (selected.value) {
    await loadMods();
  }
}

async function onSnapshotRestored() {
  if (selected.value) {
    await loadMods();
  }
}

async function onModsRepaired() {
  if (selected.value) {
    await loadMods();
  }
}

// New Instance modal tabs & state
const newInstanceTab = ref('scratch'); // 'scratch' | 'import' | 'clone'
const importMode = ref('archive'); // 'archive' | 'discovery'
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

// Mod Categorization
const MOD_CATEGORIES = [
  { id: 'all', label: 'All' },
  { id: 'performance', label: 'Performance', keywords: ['sodium', 'iris', 'lithium', 'ferritecore', 'indium', 'entityculling', 'immediatelyfast', 'krypton', 'modernfix', 'nbt-crafting', 'lazydfu', 'smoothboot', 'dynamic-fps', 'c2me', 'starlight', 'phosphor', 'embeddium', 'rubidium', 'radium', 'canary'] },
  { id: 'visuals', label: 'Visuals & UI', keywords: ['fabric-api', 'cloth-config', 'architectury', 'appleskin', 'modmenu', 'citresewn', 'continuity', 'lambdynamiclights', 'chat', 'hud', 'zoom', 'skin', 'armor', 'tooltip', 'rei', 'jei', 'emi', 'wthit', 'jade', 'blur', 'betterf3', 'presencefootsteps', 'soundphysics', 'waveycapes'] },
  { id: 'gameplay', label: 'Content & World', keywords: ['create', 'applied', 'botania', 'biomes', 'twilight', 'aether', 'adorn', 'farmersdelight', 'supplementaries', 'comforts', 'artifacts', 'dungeon', 'boss', 'mob', 'weapon', 'magic', 'tetra', 'tconstruct', 'mekanism', 'immersive', 'thermal'] },
  { id: 'utility', label: 'Utility & Tools', keywords: ['worldedit', 'xaero', 'journeymap', 'voxel', 'litematica', 'minihud', 'tweakeroo', 'carpet', 'voice', 'simple-voice', 'replaymod', 'axiom', 'spark'] },
  { id: 'libraries', label: 'Libraries', keywords: ['api', 'lib', 'library', 'core', 'kotlin', 'cloth', 'cardinal', 'geckolib', 'owo', 'architectury', 'puzzleslib', 'yungsapi', 'balm'] },
];

const activeModCategory = ref('all');

function getModCategory(mod) {
  if (!mod) return { id: 'general', label: 'General' };
  const name = (mod.filename || '').toLowerCase();
  for (const cat of MOD_CATEGORIES) {
    if (cat.id === 'all') continue;
    if (cat.keywords?.some((k) => name.includes(k))) {
      return cat;
    }
  }
  return { id: 'general', label: 'General' };
}

const modCategoriesWithCounts = computed(() => {
  const allMods = mods.value || [];
  return MOD_CATEGORIES.map((cat) => {
    if (cat.id === 'all') {
      return { ...cat, count: allMods.length };
    }
    const count = allMods.filter((m) => getModCategory(m).id === cat.id).length;
    return { ...cat, count };
  });
});

const displayedMods = computed(() => {
  const allMods = mods.value || [];
  if (activeModCategory.value === 'all') return allMods;
  return allMods.filter((m) => getModCategory(m).id === activeModCategory.value);
});

// Drag & Drop for Mods
const modDraggedIndex = ref(null);
const modDropTargetIndex = ref(null);

function onModDragStart(index, event) {
  modDraggedIndex.value = index;
  if (event?.dataTransfer) {
    event.dataTransfer.effectAllowed = 'move';
    event.dataTransfer.setData('text/plain', String(index));
  }
}

function onModDragOver(index, event) {
  if (modDraggedIndex.value === null || modDraggedIndex.value === index) return;
  modDropTargetIndex.value = index;
  if (event?.dataTransfer) {
    event.dataTransfer.dropEffect = 'move';
  }
}

function onModDropReorder(index) {
  const from = modDraggedIndex.value;
  const to = index;
  modDraggedIndex.value = null;
  modDropTargetIndex.value = null;
  if (from === null || from === undefined || from === to) return;

  const list = [...mods.value];
  const [moved] = list.splice(from, 1);
  list.splice(to, 0, moved);
  mods.value = list;
}

function onModDragEnd() {
  modDraggedIndex.value = null;
  modDropTargetIndex.value = null;
}

// Packs state
const packs = ref({ shaderpacks: [], resourcepacks: [], activeResourcepacks: [] });
const detailedPacks = ref({ shaderpacks: [], resourcepacks: [], shadersEnabled: false });
const activeShaderpack = ref('');

// Drag & Drop for Resource Packs
const rpDraggedIndex = ref(null);
const rpDropTargetIndex = ref(null);

function onRpDragStart(index, event) {
  rpDraggedIndex.value = index;
  if (event?.dataTransfer) {
    event.dataTransfer.effectAllowed = 'move';
    event.dataTransfer.setData('text/plain', String(index));
  }
}

function onRpDragOver(index, event) {
  if (rpDraggedIndex.value === null || rpDraggedIndex.value === index) return;
  rpDropTargetIndex.value = index;
  if (event?.dataTransfer) {
    event.dataTransfer.dropEffect = 'move';
  }
}

async function onRpDrop(index) {
  const from = rpDraggedIndex.value;
  const to = index;
  rpDraggedIndex.value = null;
  rpDropTargetIndex.value = null;
  if (from === null || from === undefined || from === to) return;

  if (!selectedDir.value) return;
  const current = [...(packs.value.activeResourcepacks || [])];
  const [moved] = current.splice(from, 1);
  current.splice(to, 0, moved);
  packs.value.activeResourcepacks = current;
  await api.setActiveResourcepacks(selectedDir.value, current);
}

function onRpDragEnd() {
  rpDraggedIndex.value = null;
  rpDropTargetIndex.value = null;
}

// Server Resource Packs modal & state
const showServerPacksModal = ref(false);
const serverPackDecisions = ref({});

const serverAdvertisedPacks = computed(() => {
  return (detailedPacks.value.resourcepacks || []).filter((p) => !p.isLocal);
});

async function openServerPacksModal() {
  if (selectedDir.value) {
    try {
      const decisions = await api.getServerPackDecisions(selectedDir.value);
      serverPackDecisions.value = decisions || {};
    } catch (e) {
      console.warn('Could not load server pack decisions:', e);
    }
  }
  showServerPacksModal.value = true;
}

async function onServerPacksAccepted({ accepted, rejected }) {
  if (!selectedDir.value) return;
  try {
    await api.setServerPackDecisions(selectedDir.value, accepted, rejected);
    await loadPacks();
    window.dispatchEvent(
      new CustomEvent('zircon-status', {
        detail: `Updated resource pack preferences: ${accepted.length} accepted, ${rejected.length} declined.`,
      })
    );
  } catch (err) {
    window.dispatchEvent(
      new CustomEvent('zircon-status', { detail: `Error saving resource pack decisions: ${err}` })
    );
  }
}

const activeResourcepacksList = computed(() => {
  const activeNames = packs.value.activeResourcepacks || [];
  const allPacks = detailedPacks.value.resourcepacks || [];
  const map = new Map(allPacks.map((p) => [p.filename, p]));
  return activeNames.map((name) => map.get(name) || { filename: name, title: name });
});

const inactiveResourcepacksList = computed(() => {
  const activeSet = new Set(packs.value.activeResourcepacks || []);
  const allPacks = detailedPacks.value.resourcepacks || [];
  return allPacks.filter((p) => !activeSet.has(p.filename));
});

// Shaders search state
const shaderProvider = ref('modrinth');
const shaderSearchQuery = ref('');
const shaderSearchAllVersions = ref(false);
const shaderResults = ref([]);
const shaderSearchBusy = ref(false);
const shaderSearchDone = ref(false);

// Resource packs search state
const resourceProvider = ref('modrinth');
const resourceSearchQuery = ref('');
const resourceSearchAllVersions = ref(false);
const resourceResults = ref([]);
const resourceSearchBusy = ref(false);
const resourceSearchDone = ref(false);

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
const newForm = ref({ name: '', mcVersion: '1.20.4', loaderType: 'fabric', loaderVersion: '', seed: '' });

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
  // Asynchronously fetch artwork for each instance
  for (const inst of instances.value) {
    api.getInstanceArtwork(inst.id).then((art) => {
      if (art) {
        instanceArtworks.value[inst.id] = art;
      }
    }).catch(() => {});
  }
}

async function selectInstance(instance) {
  selected.value = instance;
  updateRollbackBanner.value = null;
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
  emit('launching', selected.value ? {
    name: selected.value.name,
    address: `${selected.value.loaderType ? selected.value.loaderType.toUpperCase() : 'FABRIC'} ${selected.value.mcVersion || ''}`.trim(),
    iconUrl: selected.value.icon || null,
    bannerUrl: selected.value.banner || selected.value.bannerUrl || null,
  } : null);
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
  const ok = await askConfirm({
    title: 'Delete Instance',
    message: `Are you sure you want to delete '${selected.value.name}' and all of its files? This action cannot be undone.`,
    confirmText: 'Delete Instance',
    isDanger: true,
  });
  if (!ok) return;
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
  const ok = await askConfirm({
    title: 'Bulk Delete Mods',
    message: `Are you sure you want to permanently remove ${count} selected ${label}?`,
    confirmText: `Delete ${count} ${label}`,
    isDanger: true,
  });
  if (!ok) return;
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
const rollingBack = ref(false);
const updateRollbackBanner = ref(null);
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
    if (res && res.snapshotFilename) {
      updateRollbackBanner.value = {
        snapshotFilename: res.snapshotFilename,
        snapshotLabel: res.snapshotLabel || 'Pre-Update Snapshot',
        count: res.updatedCount,
        timestamp: Date.now(),
      };
    }
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

async function triggerBatchRollback() {
  if (!selected.value || !updateRollbackBanner.value?.snapshotFilename || rollingBack.value) return;
  rollingBack.value = true;
  try {
    const filename = updateRollbackBanner.value.snapshotFilename;
    const res = await api.rollbackModUpdate(selected.value.id, filename);
    await loadMods();
    await checkModUpdates();
    window.dispatchEvent(
      new CustomEvent('zircon-status', {
        detail: `Successfully rolled back ${res.restoredCount} mod(s) in < 1 second!`,
      })
    );
    updateRollbackBanner.value = null;
  } catch (e) {
    window.dispatchEvent(new CustomEvent('zircon-status', { detail: `Rollback error: ${e}` }));
  } finally {
    rollingBack.value = false;
  }
}

function dismissRollbackBanner() {
  updateRollbackBanner.value = null;
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

async function moveResourcepack(index, delta) {
  if (!selectedDir.value) return;
  const current = [...(packs.value.activeResourcepacks || [])];
  const targetIndex = index + delta;
  if (targetIndex < 0 || targetIndex >= current.length) return;
  const [item] = current.splice(index, 1);
  current.splice(targetIndex, 0, item);
  packs.value.activeResourcepacks = current;
  await api.setActiveResourcepacks(selectedDir.value, current);
}

async function enableResourcepack(filename) {
  if (!selectedDir.value) return;
  const current = packs.value.activeResourcepacks || [];
  if (current.includes(filename)) return;
  const next = [filename, ...current];
  packs.value.activeResourcepacks = next;
  await api.setActiveResourcepacks(selectedDir.value, next);
  await loadPacks();
}

async function disableResourcepack(filename) {
  if (!selectedDir.value) return;
  const current = packs.value.activeResourcepacks || [];
  const next = current.filter((f) => f !== filename);
  packs.value.activeResourcepacks = next;
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

async function browseResources() {
  if (!selectedDir.value) return;
  const picked = await pickFiles({ multiple: true, filters: [PACK_FILTER] });
  if (!picked || !picked.length) return;
  for (const p of picked) {
    await api.importInstancePack(selectedDir.value, 'resource', p);
  }
  await loadPacks();
}
const browseTextures = browseResources;

async function onResourceDrop(event) {
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
const onTextureDrop = onResourceDrop;

// ---------------------------------------------------------------------------
// Shaders & Resource Packs Discovery
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

function setResourceProvider(provider) {
  resourceProvider.value = provider;
  if (resourceSearchQuery.value.trim()) searchResources();
}
const setTextureProvider = setResourceProvider;

async function searchResources() {
  const query = resourceSearchQuery.value.trim();
  if (!query || !selected.value) return;
  resourceSearchBusy.value = true;
  resourceSearchDone.value = false;
  try {
    const hits = await api.searchMods(
      selected.value.id,
      query,
      resourceProvider.value,
      'resourcepack',
      resourceSearchAllVersions.value
    );
    resourceResults.value = hits.map((hit) => ({
      ...hit,
      versionOptions: [],
      selectedVersionId: '',
      versionsLoading: true,
      versionsFailed: false,
    }));
    resourceSearchDone.value = true;
    for (const hit of resourceResults.value) {
      loadResourceVersions(hit);
    }
  } catch (e) {
    window.dispatchEvent(new CustomEvent('zircon-status', { detail: `Resource pack search error: ${e}` }));
  } finally {
    resourceSearchBusy.value = false;
  }
}
const searchTextures = searchResources;

async function loadResourceVersions(hit) {
  try {
    const versions = await api.listModVersions(
      selected.value.id,
      hit.projectId || hit.id,
      hit.origin || resourceProvider.value,
      resourceSearchAllVersions.value
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

function toggleImportMode() {
  importMode.value = importMode.value === 'archive' ? 'discovery' : 'archive';
  if (importMode.value === 'discovery' && modpackResults.value.length === 0) {
    searchModpacks();
  }
}

const onCurseforgeDrop = onCurseforgeModalDrop;
const browseCurseforgePack = browseCurseforgeModalFile;
const browseImportArchive = browseLocalMrpack;
const installImportArchive = installLocalMrpack;

async function onImportDrop(e) {
  const files = e.dataTransfer?.files;
  if (!files || !files.length) return;
  const file = files[0];
  if (file.path) {
    importFilePath.value = file.path;
    if (!importCustomName.value) {
      importCustomName.value = file.name.replace(/\.(mrpack|zip)$/i, '');
    }
  }
}

async function updateCloneLoaderVersions() {
  const source = selectedCloneSource.value;
  const mc = source?.minecraftVersion;
  const loader = cloneLoaderType.value;
  if (!mc || !loader || loader === 'vanilla') {
    cloneLoaderVersions.value = [];
    cloneRecommendedLoaderVersion.value = '';
    cloneLoaderVersion.value = '';
    return;
  }
  loadingCloneLoaderVersions.value = true;
  try {
    const res = await api.getLoaderVersions(loader, mc);
    cloneLoaderVersions.value = res?.versions || [];
    cloneRecommendedLoaderVersion.value = res?.recommended || '';
    if (res?.recommended) {
      cloneLoaderVersion.value = res.recommended;
    } else if (cloneLoaderVersions.value.length > 0) {
      cloneLoaderVersion.value = cloneLoaderVersions.value[0];
    } else {
      cloneLoaderVersion.value = '';
    }
  } catch (err) {
    console.warn('Failed to fetch clone loader versions:', err);
    cloneLoaderVersions.value = [];
    cloneRecommendedLoaderVersion.value = '';
    cloneLoaderVersion.value = '';
  } finally {
    loadingCloneLoaderVersions.value = false;
  }
}

function onCloneSourceChange() {
  const source = selectedCloneSource.value;
  if (source) {
    cloneName.value = `${source.name} (Copy)`;
    cloneLoaderType.value = source.modLoader?.type || 'vanilla';
    cloneLoaderVersion.value = source.modLoader?.version || '';
    updateCloneLoaderVersions();
  }
}

async function openNewInstance(tab = 'scratch') {
  newInstanceTab.value = tab;
  importMode.value = 'archive';
  const defaultMc = mcVersions.value[0] || '1.21.4';
  const defaultLoader = loaderTypes.value[0] || 'fabric';
  newForm.value = {
    name: '',
    mcVersion: defaultMc,
    loaderType: defaultLoader,
    loaderVersion: '',
  };

  const initialSource = selected.value || (instances.value.length > 0 ? instances.value[0] : null);
  if (initialSource) {
    cloneSourceId.value = initialSource.id;
    cloneName.value = `${initialSource.name} (Copy)`;
    cloneLoaderType.value = initialSource.modLoader?.type || 'vanilla';
    cloneLoaderVersion.value = initialSource.modLoader?.version || '';
    updateCloneLoaderVersions();
  }
  cloneCopyWorld.value = true;
  cloneCopyMods.value = true;

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

function onWindowClick(e) {
  if (showOpenFolderDropdown.value && !e.target.closest('[data-dropdown="open-folder"]')) {
    showOpenFolderDropdown.value = false;
  }
  if (showActionsDropdown.value && !e.target.closest('[data-dropdown="actions"]')) {
    showActionsDropdown.value = false;
  }
  if (showToolsDropdown.value && !e.target.closest('[data-dropdown="tools"]')) {
    showToolsDropdown.value = false;
  }
  if (activeCardDropdownId.value && !e.target.closest('[data-dropdown="card-menu"]')) {
    activeCardDropdownId.value = null;
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
  cloneSourceId.value = selected.value.id;
  cloneName.value = `${selected.value.name} (Copy)`;
  cloneLoaderType.value = selected.value.modLoader?.type || 'vanilla';
  cloneLoaderVersion.value = selected.value.modLoader?.version || '';
  cloneCopyWorld.value = true;
  cloneCopyMods.value = true;
  newInstanceTab.value = 'clone';
  showNewDialog.value = true;
  updateCloneLoaderVersions();
}

async function executeCloneInstance() {
  if (!cloneSourceId.value || !cloneName.value.trim()) return;
  cloning.value = true;
  try {
    const cloned = await api.cloneOfflineInstance(
      cloneSourceId.value,
      cloneName.value.trim(),
      cloneLoaderType.value || null,
      cloneLoaderVersion.value || null,
      cloneCopyWorld.value,
      cloneCopyMods.value
    );
    await loadInstances();
    selected.value = cloned;
    showNewDialog.value = false;
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
  const ok = await askConfirm({
    title: 'Restore World Backup',
    message: `Are you sure you want to restore "${backupFilename}"? Any unsaved world progress will be overwritten.`,
    confirmText: 'Restore Backup',
    isDanger: true,
  });
  if (!ok) return;
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

  // Support automated navigation to modals via ?modal= query param for screenshots
  if (typeof window !== 'undefined' && typeof URLSearchParams !== 'undefined') {
    const params = new URLSearchParams(window.location.search);
    const modalParam = params.get('modal');
    if (modalParam === 'migration') {
      showMigrationModal.value = true;
    } else if (modalParam === 'couch_coop') {
      if (!selected.value && instances.value.length > 0) {
        selected.value = instances.value[0];
      }
      showCouchCoopModal.value = true;
    } else if (modalParam === 'audit') {
      showAuditModal.value = true;
    } else if (modalParam === 'share') {
      showShareModal.value = true;
    }
  }

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
    if (typeof window !== 'undefined' && window.__TAURI_INTERNALS__) {
      const webview = getCurrentWebview();
      unlistenFileDrop = await webview.onDragDropEvent(async (event) => {
      if (event.payload.type === 'drop') {
        const paths = event.payload.paths;
        if (!paths || !paths.length) return;

        // 1. Detect .mrpack or CurseForge modpacks dropped anywhere onto the window
        const mrpackPath = paths.find(
          (p) => p.toLowerCase().endsWith('.mrpack')
        );
        if (mrpackPath) {
          droppedMrpackPath.value = mrpackPath;
          showImportModal.value = true;
          window.dispatchEvent(
            new CustomEvent('zircon-mrpack-dropped', { detail: { filePath: mrpackPath } })
          );
          return;
        }

        // 2. Mod / Resource Pack drops when an instance is selected
        if (selected.value) {
          for (const path of paths) {
            if (path.endsWith('.jar')) {
              await api.importOfflineModFile(selected.value.id, path);
            } else if (path.toLowerCase().endsWith('.zip')) {
              // Check if the zip is actually a CurseForge or Modrinth modpack archive
              try {
                const inspect = await api.inspectLocalModpack(path);
                if (inspect && (inspect.archiveType === 'curseforge' || inspect.archiveType === 'modrinth')) {
                  droppedMrpackPath.value = path;
                  showImportModal.value = true;
                  return;
                }
              } catch (_) {
                // Not a modpack archive, treat as regular resource pack
              }
              await api.importInstancePack(selectedDir.value, 'resource', path);
            }
          }
          await Promise.all([loadMods(), loadPacks()]);
        }
      }
    });
    }
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

<style scoped>
.dropdown-pop-enter-active,
.dropdown-pop-leave-active {
  transition: opacity 0.16s cubic-bezier(0.16, 1, 0.3, 1), transform 0.16s cubic-bezier(0.16, 1, 0.3, 1);
}
.dropdown-pop-enter-from,
.dropdown-pop-leave-to {
  opacity: 0;
  transform: translateY(-5px) scale(0.97);
}

.custom-scrollbar::-webkit-scrollbar {
  width: 6px;
}
.custom-scrollbar::-webkit-scrollbar-track {
  background: rgba(7, 11, 16, 0.4);
  border-radius: 9999px;
}
.custom-scrollbar::-webkit-scrollbar-thumb {
  background: rgba(71, 210, 201, 0.35);
  border-radius: 9999px;
}
.custom-scrollbar::-webkit-scrollbar-thumb:hover {
  background: rgba(71, 210, 201, 0.7);
}
</style>
