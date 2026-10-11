<template>
  <div
    v-if="open"
    class="absolute inset-0 z-40 bg-[#070b0f]/85 backdrop-blur-md flex items-center justify-center p-4"
    @click.self="!modpackProgress?.active && $emit('close')"
  >
    <div class="z-card w-full max-w-[640px] max-h-[85vh] flex flex-col p-6 overflow-hidden shadow-2xl relative border border-slate-700/60 rounded-2xl bg-[#0e1622]">
      <div class="flex items-center justify-between pb-3 border-b border-slate-800/80 mb-4">
        <h3 class="text-white font-bold text-base">{{ $t('instance_creation.title') }}</h3>
        <button
          v-if="!modpackProgress?.active"
          class="text-slate-400 hover:text-white text-lg leading-none"
          @click="$emit('close')"
        >
          ✕
        </button>
      </div>

      <!-- 3 Tab Selector: Scratch, Import, Clone -->
      <div class="z-segmented-track mb-4 w-full flex">
        <button
          type="button"
          class="z-segmented-pill flex-1 text-center py-2 text-xs font-semibold"
          :class="{ 'active': activeTab === 'scratch' }"
          @click="$emit('update:activeTab', 'scratch')"
        >
          {{ $t('instance_creation.tab_scratch') }}
        </button>
        <button
          type="button"
          class="z-segmented-pill flex-1 text-center py-2 text-xs font-semibold"
          :class="{ 'active': activeTab === 'import' }"
          @click="$emit('update:activeTab', 'import')"
        >
          {{ $t('instance_creation.tab_import') }}
        </button>
        <button
          type="button"
          class="z-segmented-pill flex-1 text-center py-2 text-xs font-semibold"
          :class="{ 'active': activeTab === 'clone' }"
          @click="$emit('update:activeTab', 'clone')"
        >
          {{ $t('instance_creation.tab_clone') }}
        </button>
        <button
          type="button"
          class="z-segmented-pill flex-1 text-center py-2 text-xs font-semibold text-cyan-400 hover:text-cyan-300"
          @click="$emit('migrate')"
        >
          {{ $t('instance_creation.tab_migrate') }}
        </button>
      </div>

      <!-- TAB 1: CREATE FROM SCRATCH -->
      <div v-if="activeTab === 'scratch'" class="flex-1 overflow-y-auto pr-1">
        <label class="z-label font-semibold text-slate-300 block mb-1">{{ $t('instance_creation.name_label') }}</label>
        <input v-model="newForm.name" class="z-input mb-3 text-xs" :placeholder="$t('instance_creation.name_placeholder')" />
        <label class="z-label font-semibold text-slate-300 block mb-1">{{ $t('instance_creation.mc_version_label') }}</label>
        <select v-model="newForm.mcVersion" class="z-input mb-3 text-xs" @change="$emit('updateLoaderVersions')">
          <option v-for="v in mcVersions" :key="v" :value="v">{{ v }}</option>
        </select>
        <label class="z-label font-semibold text-slate-300 block mb-1">{{ $t('instance_creation.loader_label') }}</label>
        <select v-model="newForm.loaderType" class="z-input mb-3 text-xs" @change="$emit('updateLoaderVersions')">
          <option v-for="l in loaderTypes" :key="l" :value="l" class="capitalize">{{ l }}</option>
        </select>
        <template v-if="newForm.loaderType !== 'vanilla'">
          <label class="z-label font-semibold text-slate-300 block mb-1 flex items-center justify-between">
            <span>{{ $t('instance_creation.loader_version_label') }}</span>
            <span v-if="loadingLoaderVersions" class="text-[11px] text-cyan-400 font-mono">{{ $t('instance_creation.fetching_versions') }}</span>
            <span v-else-if="recommendedLoaderVersion" class="text-[11px] text-slate-400 font-mono">
              {{ $t('instance_creation.recommended_prefix', { version: recommendedLoaderVersion }) }}
            </span>
          </label>
          <div v-if="loaderVersions.length > 0" class="mb-5">
            <select v-model="newForm.loaderVersion" class="z-input text-xs">
              <option v-for="lv in loaderVersions" :key="lv" :value="lv">
                {{ lv }} {{ lv === recommendedLoaderVersion ? $t('instance_creation.recommended') : '' }}
              </option>
            </select>
          </div>
          <input
            v-else
            v-model="newForm.loaderVersion"
            class="z-input mb-5 text-xs"
            placeholder="e.g. 0.16.10"
          />
        </template>

        <div class="mb-5">
          <label class="z-label mb-1.5 text-xs text-slate-300">{{ $t('instance_creation.world_seed_label') }}</label>
          <input
            v-model="newForm.seed"
            class="z-input text-xs font-mono"
            :placeholder="$t('instance_creation.world_seed_placeholder')"
          />
        </div>
        <div class="flex justify-end gap-2.5 pt-4 border-t border-slate-800/80 mt-auto">
          <button class="z-btn-ghost text-xs px-4 py-2 rounded-xl font-semibold border border-slate-700/80 hover:border-slate-600 hover:text-white" @click="$emit('close')">{{ $t('instance_creation.cancel') }}</button>
          <button class="z-btn-accent text-xs font-bold px-5 py-2 rounded-xl shadow-md hover:shadow-cyan-500/25" :disabled="!newForm.name?.trim() || creating" @click="$emit('create')">
            {{ creating ? $t('instance_creation.creating_btn') : $t('instance_creation.create_btn') }}
          </button>
        </div>
      </div>

      <!-- TAB 2: UPLOAD ARCHIVE / IMPORT -->
      <div v-else-if="activeTab === 'import'" class="flex-1 flex flex-col gap-3 overflow-hidden">
        <div class="flex items-center justify-between px-0.5">
          <span class="text-xs text-slate-400 font-medium">
            {{ importMode === 'discovery' ? $t('instance_creation.import_discovery_hint') : $t('instance_creation.import_archive_hint') }}
          </span>
          <button
            class="text-xs text-cyan-400 hover:text-cyan-300 font-semibold underline flex items-center gap-1"
            @click="$emit('toggleImportMode')"
          >
            <span>{{ importMode === 'discovery' ? $t('instance_creation.btn_upload_file') : $t('instance_creation.btn_browse_modrinth') }}</span>
          </button>
        </div>

        <!-- Sub-view A: Archive File Drop -->
        <div v-if="importMode === 'archive'" class="flex-1 flex flex-col gap-4 overflow-y-auto pr-1">
          <div
            class="zircon-drop-zone p-6 text-center text-xs text-slate-400 cursor-pointer rounded-2xl border-2 border-dashed border-slate-700/80 hover:border-cyan-500/60 bg-slate-900/40 transition"
            @click="$emit('browseArchive')"
            @dragover.prevent
            @drop.prevent="$emit('dropArchive', $event)"
          >
            <svg class="w-8 h-8 mx-auto mb-2 text-slate-500 hover:text-cyan-400 transition-colors" fill="none" viewBox="0 0 24 24" stroke="currentColor">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="1.8" d="M7 16a4 4 0 01-.88-7.903A5 5 0 1115.9 6L16 6a5 5 0 011 9.9M15 13l-3-3m0 0l-3 3m3-3v12" />
            </svg>
            <div v-if="importFilePath" class="font-mono text-cyan-300 text-xs break-all mb-1 font-semibold">
              {{ importFilePath }}
            </div>
            <div v-else>
              {{ $t('instance_creation.drop_archive_prompt') }}
            </div>
            <div class="text-[10px] text-slate-500 mt-1">{{ $t('instance_creation.drop_archive_desc') }}</div>
          </div>

          <div>
            <label class="z-label font-semibold text-slate-300 block mb-1">{{ $t('instance_creation.import_name_label') }}</label>
            <input
              :value="importCustomName"
              @input="$emit('update:importCustomName', $event.target.value)"
              class="z-input text-xs"
              :placeholder="$t('instance_creation.import_name_placeholder')"
            />
          </div>

          <div class="flex justify-end gap-2.5 pt-4 border-t border-slate-800/80 mt-auto">
            <button class="z-btn-ghost text-xs px-4 py-2 rounded-xl font-semibold border border-slate-700/80 hover:border-slate-600 hover:text-white" @click="$emit('close')">{{ $t('instance_creation.cancel') }}</button>
            <button
              class="z-btn-accent text-xs font-bold px-5 py-2 rounded-xl shadow-md hover:shadow-cyan-500/25"
              :disabled="!importFilePath || importingModpack || modpackProgress?.active"
              @click="$emit('installArchive')"
            >
              {{ importingModpack ? $t('instance_creation.importing_btn') : $t('instance_creation.import_btn') }}
            </button>
          </div>
        </div>

        <!-- Sub-view B: Modrinth Online Discovery -->
        <div v-else class="flex-1 min-h-0 flex flex-col gap-3 overflow-hidden">
          <div class="flex gap-2">
            <input
              :value="modpackSearchQuery"
              @input="$emit('update:modpackSearchQuery', $event.target.value)"
              class="z-input flex-1 text-xs"
              :placeholder="$t('instance_creation.search_modpacks_placeholder')"
              @keydown.enter="$emit('searchModpacks')"
            />
            <button
              class="z-btn-ghost px-4 text-xs font-bold shrink-0"
              :disabled="modpackSearchBusy"
              @click="$emit('searchModpacks')"
            >
              {{ $t('instance_creation.search_btn') }}
            </button>
          </div>

          <div class="flex items-center justify-between text-xs text-slate-400 px-0.5">
            <span>{{ modpackResults?.length ? $t('instance_creation.modpacks_found', { count: modpackResults.length }) : $t('instance_creation.popular_modpacks') }}</span>
            <span v-if="modpackSearchBusy" class="text-cyan-400 font-mono flex items-center gap-1.5">
              <span class="inline-block w-2.5 h-2.5 border-2 border-accent border-t-transparent rounded-full animate-spin"></span>
              {{ $t('instance_creation.searching') }}
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
                <div v-else class="w-11 h-11 rounded-lg shrink-0 mt-0.5 bg-gradient-to-br from-[#1bd96a]/20 to-[#1bd96a]/5 border border-[#1bd96a]/30 flex items-center justify-center font-bold text-white text-sm">
                  {{ (hit.title || hit.name || 'M').charAt(0).toUpperCase() }}
                </div>
                <div class="flex-1 min-w-0">
                  <div class="flex items-center justify-between gap-2">
                    <div class="text-xs font-bold text-white truncate">{{ hit.title || hit.name }}</div>
                    <button
                      class="text-xs px-3 py-1.5 rounded-lg font-bold shrink-0 flex items-center gap-1.5 transition disabled:opacity-50 disabled:cursor-not-allowed bg-[#1bd96a]/20 text-emerald-300 border border-[#1bd96a]/40 hover:bg-[#1bd96a]/30 shadow-sm"
                      :disabled="installingModpackId === (hit.projectId || hit.id) || modpackProgress?.active"
                      @click="$emit('installModpack', hit)"
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
                      <span>{{ $t('instance_creation.install_modpack') }}</span>
                    </button>
                  </div>
                  <div class="text-[11px] text-slate-400 line-clamp-2 mt-0.5 leading-relaxed">
                    {{ hit.summary || hit.description || $t('instance_creation.no_desc') }}
                  </div>
                  <div class="flex items-center gap-3 text-[10px] text-slate-400 font-mono mt-1.5">
                    <span v-if="hit.author">{{ $t('instance_creation.by_author', { author: hit.author }) }}</span>
                    <span v-if="hit.downloads">⬇ {{ fmtCount(hit.downloads) }}</span>
                  </div>

                  <!-- Version selector -->
                  <div class="mt-2 flex items-center gap-2">
                    <span class="text-[10px] text-slate-400 shrink-0 font-mono">{{ $t('instance_creation.version_label') }}</span>
                    <select
                      v-model="selectedModpackVersions[hit.projectId || hit.id]"
                      :disabled="installingModpackId === (hit.projectId || hit.id) || hit.versionsLoading"
                      class="flex-1 min-w-0 bg-slate-950 border border-slate-700 rounded-lg px-2.5 py-1 text-[11px] text-slate-200 disabled:opacity-50 focus:border-cyan-400 focus:outline-none"
                    >
                      <option v-if="hit.versionsLoading" value="" disabled>{{ $t('instance_creation.loading_versions') }}</option>
                      <option v-else-if="!hit.versionOptions?.length" value="" disabled>{{ $t('instance_creation.no_versions') }}</option>
                      <option v-for="v in hit.versionOptions" :key="v.id" :value="v.id">
                        {{ v.versionNumber || v.name }}
                      </option>
                    </select>
                  </div>
                </div>
              </div>
            </div>
            <div v-if="!modpackSearchBusy && modpackSearchDone && modpackResults?.length === 0" class="text-xs text-slate-500 py-6 text-center">
              {{ $t('instance_creation.no_modpacks_found') }}
            </div>
          </div>
        </div>
      </div>

      <!-- TAB 3: CLONE EXISTING INSTANCE -->
      <div v-else-if="activeTab === 'clone'" class="flex-1 overflow-y-auto pr-1 flex flex-col gap-3.5">
        <div v-if="instances?.length === 0" class="py-10 text-center text-slate-500 text-xs">
          {{ $t('instance_creation.clone_no_instances') }}
        </div>

        <template v-else>
          <div>
            <label class="z-label font-semibold text-slate-300 block mb-1">{{ $t('instance_creation.clone_source_label') }}</label>
            <select :value="cloneSourceId" @change="$emit('update:cloneSourceId', $event.target.value); $emit('cloneSourceChange', $event.target.value)" class="z-input text-xs">
              <option v-for="inst in instances" :key="inst.id" :value="inst.id">
                {{ inst.name }} (MC {{ inst.minecraftVersion }} · {{ inst.modLoader?.type || 'vanilla' }})
              </option>
            </select>
          </div>

          <div>
            <label class="z-label font-semibold text-slate-300 block mb-1">{{ $t('instance_creation.clone_name_label') }}</label>
            <input :value="cloneName" @input="$emit('update:cloneName', $event.target.value)" class="z-input text-xs" :placeholder="$t('instance_creation.clone_name_placeholder')" />
          </div>

          <div class="grid grid-cols-2 gap-3">
            <div>
              <label class="z-label font-semibold text-slate-300 block mb-1">{{ $t('instance_creation.loader_label') }}</label>
              <select :value="cloneLoaderType" @change="$emit('update:cloneLoaderType', $event.target.value); $emit('updateCloneLoaderVersions', $event.target.value)" class="z-input text-xs">
                <option v-for="l in loaderTypes" :key="l" :value="l" class="capitalize">{{ l }}</option>
              </select>
            </div>
            <div v-if="cloneLoaderType !== 'vanilla'">
              <label class="z-label font-semibold text-slate-300 block mb-1 flex items-center justify-between">
                <span>{{ $t('instance_creation.loader_version_label') }}</span>
                <span v-if="loadingCloneLoaderVersions" class="text-[10px] text-cyan-400 font-mono">{{ $t('instance_creation.fetching_versions') }}</span>
              </label>
              <select v-if="cloneLoaderVersions?.length > 0" :value="cloneLoaderVersion" @change="$emit('update:cloneLoaderVersion', $event.target.value)" class="z-input text-xs">
                <option v-for="lv in cloneLoaderVersions" :key="lv" :value="lv">
                  {{ lv }} {{ lv === cloneRecommendedLoaderVersion ? $t('instance_creation.recommended') : '' }}
                </option>
              </select>
              <input v-else :value="cloneLoaderVersion" @input="$emit('update:cloneLoaderVersion', $event.target.value)" class="z-input text-xs" placeholder="e.g. 0.16.10" />
            </div>
          </div>

          <!-- Loader Change Warning Card -->
          <div
            v-if="isCloneLoaderChanged"
            class="p-3 bg-amber-500/10 border border-amber-500/30 rounded-xl flex items-start gap-2.5 text-amber-200 text-xs"
          >
            <svg class="w-4 h-4 text-amber-400 shrink-0 mt-0.5" fill="none" viewBox="0 0 24 24" stroke="currentColor">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 9v2m0 4h.01m-6.938 4h13.856c1.54 0 2.502-1.667 1.732-3L13.732 4c-.77-1.333-2.694-1.333-3.464 0L3.34 16c-.77 1.333.192 3 1.732 3z" />
            </svg>
            <div>
              <span class="font-bold block mb-0.5">{{ $t('instance_creation.clone_warning_title') }}</span>
              <span class="text-amber-300/90 leading-relaxed">
                {{ $t('instance_creation.clone_warning_desc', { from: selectedCloneSource?.modLoader?.type || 'vanilla', to: cloneLoaderType }) }}
              </span>
            </div>
          </div>

          <!-- Selective Copy Options -->
          <div class="p-3 bg-slate-900/50 border border-slate-800/80 rounded-xl space-y-2">
            <span class="text-xs font-semibold text-slate-300 block mb-1">{{ $t('instance_creation.selective_copy_title') }}</span>
            <label class="flex items-center gap-2.5 text-xs text-slate-300 cursor-pointer select-none">
              <input type="checkbox" :checked="cloneCopyWorld" @change="$emit('update:cloneCopyWorld', $event.target.checked)" class="zircon-check" />
              <span>{{ $t('instance_creation.copy_worlds') }}</span>
            </label>
            <label class="flex items-center gap-2.5 text-xs text-slate-300 cursor-pointer select-none">
              <input type="checkbox" :checked="cloneCopyMods" @change="$emit('update:cloneCopyMods', $event.target.checked)" class="zircon-check" />
              <span>{{ $t('instance_creation.copy_mods') }}</span>
            </label>
          </div>

          <div class="flex justify-end gap-2.5 pt-4 border-t border-slate-800/80 mt-auto">
            <button class="z-btn-ghost text-xs px-4 py-2 rounded-xl font-semibold border border-slate-700/80 hover:border-slate-600 hover:text-white" @click="$emit('close')">{{ $t('instance_creation.cancel') }}</button>
            <button
              class="z-btn-accent text-xs font-bold px-5 py-2 rounded-xl shadow-md hover:shadow-cyan-500/25 flex items-center gap-1.5"
              :disabled="!cloneSourceId || !cloneName?.trim() || cloning"
              @click="$emit('clone')"
            >
              <span v-if="cloning" class="w-3 h-3 border-2 border-slate-900 border-t-transparent rounded-full animate-spin"></span>
              <span>{{ cloning ? $t('instance_creation.cloning_btn') : $t('instance_creation.clone_btn') }}</span>
            </button>
          </div>
        </template>
      </div>

      <!-- INSTALLATION PROGRESS OVERLAY -->
      <div
        v-if="modpackProgress?.active"
        class="absolute inset-0 bg-[#0e1622]/95 backdrop-blur-sm z-50 flex flex-col items-center justify-center p-6 text-center"
      >
        <div class="w-12 h-12 rounded-full border-4 border-cyan-500/20 border-t-cyan-400 animate-spin mb-4"></div>
        <div class="text-white font-bold text-sm mb-1">{{ modpackProgress.phase || $t('instance_creation.progress_installing') }}</div>
        <div class="text-xs text-slate-400 font-mono max-w-sm truncate mb-4">{{ modpackProgress.detail }}</div>
        <div class="w-full max-w-md bg-slate-950 border border-slate-800 rounded-full h-2.5 overflow-hidden shadow-inner">
          <div
            class="bg-gradient-to-r from-cyan-500 to-emerald-400 h-full transition-all duration-300 ease-out"
            :style="{ width: `${Math.round((modpackProgress.fraction || 0) * 100)}%` }"
          ></div>
        </div>
        <div class="text-[11px] text-slate-500 font-mono mt-2">
          {{ $t('instance_creation.progress_complete', { percent: Math.round((modpackProgress.fraction || 0) * 100) }) }}
        </div>
      </div>
    </div>
  </div>
</template>

<script setup>
defineProps({
  open: { type: Boolean, default: false },
  activeTab: { type: String, default: 'scratch' },
  modpackProgress: { type: Object, default: () => ({ active: false }) },
  newForm: { type: Object, default: () => ({}) },
  mcVersions: { type: Array, default: () => [] },
  loaderTypes: { type: Array, default: () => [] },
  loaderVersions: { type: Array, default: () => [] },
  loadingLoaderVersions: { type: Boolean, default: false },
  recommendedLoaderVersion: { type: String, default: '' },
  creating: { type: Boolean, default: false },
  importMode: { type: String, default: 'archive' },
  importFilePath: { type: String, default: '' },
  importCustomName: { type: String, default: '' },
  importingModpack: { type: Boolean, default: false },
  modpackSearchQuery: { type: String, default: '' },
  modpackSearchBusy: { type: Boolean, default: false },
  modpackSearchDone: { type: Boolean, default: false },
  modpackResults: { type: Array, default: () => [] },
  installingModpackId: { type: [String, Number], default: null },
  selectedModpackVersions: { type: Object, default: () => ({}) },
  instances: { type: Array, default: () => [] },
  cloneSourceId: { type: String, default: '' },
  cloneName: { type: String, default: '' },
  cloneLoaderType: { type: String, default: 'vanilla' },
  cloneLoaderVersion: { type: String, default: '' },
  cloneLoaderVersions: { type: Array, default: () => [] },
  loadingCloneLoaderVersions: { type: Boolean, default: false },
  cloneRecommendedLoaderVersion: { type: String, default: '' },
  isCloneLoaderChanged: { type: Boolean, default: false },
  selectedCloneSource: { type: Object, default: null },
  cloneCopyWorld: { type: Boolean, default: true },
  cloneCopyMods: { type: Boolean, default: true },
  cloning: { type: Boolean, default: false },
  fmtCount: { type: Function, default: (c) => c },
});

defineEmits([
  'close',
  'migrate',
  'update:activeTab',
  'update:importCustomName',
  'update:modpackSearchQuery',
  'update:cloneSourceId',
  'update:cloneName',
  'update:cloneLoaderType',
  'update:cloneLoaderVersion',
  'update:cloneCopyWorld',
  'update:cloneCopyMods',
  'updateLoaderVersions',
  'create',
  'toggleImportMode',
  'browseArchive',
  'dropArchive',
  'installArchive',
  'searchModpacks',
  'installModpack',
  'cloneSourceChange',
  'updateCloneLoaderVersions',
  'clone',
]);
</script>
