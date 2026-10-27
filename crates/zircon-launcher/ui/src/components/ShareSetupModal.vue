<template>
  <div
    v-if="open"
    class="fixed inset-0 z-50 flex items-center justify-center p-4 bg-black/80 backdrop-blur-md transition-all duration-300"
    @click.self="handleClose"
  >
    <div
      class="w-full max-w-2xl bg-[#0b0f17]/95 border border-slate-800/90 rounded-2xl shadow-2xl shadow-cyan-950/30 overflow-hidden flex flex-col animate-in fade-in zoom-in-95 duration-200 max-h-[90vh]"
    >
      <!-- Header -->
      <div class="px-6 py-5 border-b border-slate-800/80 flex items-center justify-between bg-gradient-to-r from-slate-900/90 via-slate-900/50 to-cyan-950/20">
        <div class="flex items-center gap-3">
          <div class="w-10 h-10 rounded-xl bg-cyan-500/10 border border-cyan-500/30 flex items-center justify-center text-cyan-400 shadow-inner">
            <svg class="w-5 h-5" fill="none" viewBox="0 0 24 24" stroke="currentColor">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M8.684 13.342C8.886 12.938 9 12.482 9 12c0-.482-.114-.938-.316-1.342m0 2.684a3 3 0 110-2.684m0 2.684l6.632 3.316m-6.632-6l6.632-3.316m0 0a3 3 0 105.367-2.684 3 3 0 00-5.367 2.684zm0 9.316a3 3 0 105.368 2.684 3 3 0 00-5.368-2.684z" />
            </svg>
          </div>
          <div>
            <h2 class="text-base font-bold text-slate-100 flex items-center gap-2">
              {{ $t('share_setup.title') }}
              <span class="text-[10px] font-mono px-2 py-0.5 rounded-full bg-cyan-500/10 text-cyan-400 border border-cyan-500/20">{{ $t('share_setup.badge') }}</span>
            </h2>
            <p class="text-xs text-slate-400">{{ $t('share_setup.subtitle', { name: instance?.name, version: instance?.minecraft_version, count: modCount }) }}</p>
          </div>
        </div>
        <button
          class="text-slate-500 hover:text-slate-300 p-1.5 rounded-lg hover:bg-slate-800/60 transition-colors"
          @click="handleClose"
        >
          <svg class="w-5 h-5" fill="none" viewBox="0 0 24 24" stroke="currentColor">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 18L18 6M6 6l12 12" />
          </svg>
        </button>
      </div>

      <!-- Target Version Banner & Change Version Quick Action -->
      <div class="px-6 py-2.5 bg-slate-900/60 border-b border-slate-800/80 flex items-center justify-between text-xs">
        <div class="flex items-center gap-2 font-mono">
          <span class="text-slate-400">Target:</span>
          <span class="px-2 py-0.5 rounded bg-slate-800 text-slate-200 font-bold">MC {{ instance?.minecraftVersion || instance?.minecraft_version }}</span>
          <span class="px-2 py-0.5 rounded bg-cyan-950/80 text-cyan-300 border border-cyan-800/60 capitalize">
            {{ instance?.modLoader?.type || instance?.mod_loader?.type || 'vanilla' }} {{ instance?.modLoader?.version || instance?.mod_loader?.version || '' }}
          </span>
        </div>
        <button
          type="button"
          class="text-[11px] text-cyan-400 hover:text-cyan-300 font-semibold underline flex items-center gap-1 cursor-pointer"
          @click="$emit('editVersion')"
        >
          <svg class="w-3 h-3" fill="none" viewBox="0 0 24 24" stroke="currentColor">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M15.232 5.232l3.536 3.536m-2.036-5.036a2.5 2.5 0 113.536 3.536L6.5 21.036H3v-3.572L16.732 3.732z" />
          </svg>
          <span>Change Version</span>
        </button>
      </div>

      <!-- Navigation Tabs -->
      <div class="px-6 pt-3 border-b border-slate-800/60 flex gap-2 bg-slate-950/40">
        <button
          class="px-3.5 py-2 text-xs font-semibold rounded-t-xl transition-all border-b-2 flex items-center gap-1.5"
          :class="activeTab === 'share_code' ? 'text-cyan-400 border-cyan-400 bg-cyan-950/20' : 'text-slate-400 border-transparent hover:text-slate-200'"
          @click="selectTab('share_code')"
        >
          <span>{{ $t('share_setup.tab_share_code') }}</span>
        </button>
        <button
          class="px-3.5 py-2 text-xs font-semibold rounded-t-xl transition-all border-b-2 flex items-center gap-1.5"
          :class="activeTab === 'mrpack' ? 'text-cyan-400 border-cyan-400 bg-cyan-950/20' : 'text-slate-400 border-transparent hover:text-slate-200'"
          @click="selectTab('mrpack')"
        >
          <span>{{ $t('share_setup.tab_mrpack') }}</span>
        </button>
        <button
          class="px-3.5 py-2 text-xs font-semibold rounded-t-xl transition-all border-b-2 flex items-center gap-1.5"
          :class="activeTab === 'json' ? 'text-cyan-400 border-cyan-400 bg-cyan-950/20' : 'text-slate-400 border-transparent hover:text-slate-200'"
          @click="selectTab('json')"
        >
          <span>{{ $t('share_setup.tab_json') }}</span>
        </button>
        <button
          class="px-3.5 py-2 text-xs font-semibold rounded-t-xl transition-all border-b-2 flex items-center gap-1.5"
          :class="activeTab === 'markdown' ? 'text-cyan-400 border-cyan-400 bg-cyan-950/20' : 'text-slate-400 border-transparent hover:text-slate-200'"
          @click="selectTab('markdown')"
        >
          <span>{{ $t('share_setup.tab_markdown') }}</span>
        </button>
      </div>

      <!-- Body Content -->
      <div class="p-6 overflow-y-auto space-y-4 flex-1">
        <!-- TAB 1: Share Code -->
        <div v-if="activeTab === 'share_code'" class="space-y-4">
          <div class="bg-cyan-950/20 border border-cyan-500/20 rounded-xl p-3.5 text-xs text-cyan-200/90 leading-relaxed">
            <strong>{{ $t('share_setup.share_banner_title') }}</strong> {{ $t('share_setup.share_banner_desc') }}
          </div>

          <div>
            <label class="block text-xs font-semibold text-slate-300 mb-1.5">{{ $t('share_setup.share_string_label') }}</label>
            <div class="relative">
              <textarea
                readonly
                rows="3"
                class="w-full bg-[#070b10] border border-slate-700/80 rounded-xl p-3 text-xs font-mono text-cyan-300 resize-none focus:outline-none focus:border-cyan-500"
                :value="shareCode"
              ></textarea>
            </div>
          </div>

          <div class="flex items-center justify-between pt-2">
            <span class="text-xs text-slate-500">{{ $t('share_setup.decodes_offline') }}</span>
            <button
              class="z-btn-primary text-xs px-5 py-2.5 rounded-xl font-bold flex items-center gap-2 shadow-lg shadow-cyan-900/30"
              @click="copyShareCode"
            >
              <svg v-if="!codeCopied" class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M8 5H6a2 2 0 00-2 2v12a2 2 0 002 2h10a2 2 0 002-2v-1M8 5a2 2 0 002 2h2a2 2 0 002-2M8 5a2 2 0 012-2h2a2 2 0 012 2m0 0h2a2 2 0 012 2v3m2 4H10m0 0l3-3m-3 3l3 3" />
              </svg>
              <span v-else class="text-emerald-400">✓</span>
              <span>{{ codeCopied ? $t('share_setup.copied_clipboard') : $t('share_setup.copy_share_code') }}</span>
            </button>
          </div>
        </div>

        <!-- TAB 2: Modpack (.mrpack) -->
        <div v-if="activeTab === 'mrpack'" class="space-y-4">
          <div class="bg-slate-900/70 border border-slate-800 rounded-xl p-3.5 text-xs text-slate-300 leading-relaxed">
            <strong>{{ $t('share_setup.mrpack_banner_title') }}</strong> {{ $t('share_setup.mrpack_banner_desc') }}
          </div>

          <div class="p-4 bg-slate-950/50 border border-slate-800 rounded-xl flex items-center justify-between">
            <div>
              <div class="text-xs font-bold text-white">{{ instance?.name }}.mrpack</div>
              <div class="text-[11px] text-slate-500 mt-0.5">{{ $t('share_setup.mrpack_manifest_desc') }}</div>
            </div>
            <button
              class="z-btn-primary text-xs px-4 py-2 rounded-xl font-bold flex items-center gap-1.5"
              :disabled="exportingMrpack"
              @click="handleExportMrpack"
            >
              <span v-if="exportingMrpack" class="w-3 h-3 border-2 border-white border-t-transparent rounded-full animate-spin"></span>
              <span>{{ exportingMrpack ? $t('share_setup.exporting_mrpack') : $t('share_setup.export_mrpack') }}</span>
            </button>
          </div>
        </div>

        <!-- TAB 3: Recipe Manifest (.json) -->
        <div v-if="activeTab === 'json'" class="space-y-4">
          <div class="bg-slate-900/70 border border-slate-800 rounded-xl p-3.5 text-xs text-slate-300 leading-relaxed">
            <strong>{{ $t('share_setup.json_banner_title') }}</strong> {{ $t('share_setup.json_banner_desc') }}
          </div>

          <div class="relative">
            <pre class="w-full max-h-52 overflow-y-auto bg-[#070b10] border border-slate-800 rounded-xl p-3 text-[11px] font-mono text-slate-300">{{ jsonPayload }}</pre>
          </div>

          <div class="flex items-center justify-end gap-2">
            <button class="z-btn-ghost text-xs px-4 py-2 rounded-xl font-semibold" @click="copyJson">
              {{ jsonCopied ? $t('share_setup.copied_json') : $t('share_setup.copy_json') }}
            </button>
            <button class="z-btn-primary text-xs px-4 py-2 rounded-xl font-bold" @click="downloadJsonFile">
              {{ $t('share_setup.save_json') }}
            </button>
          </div>
        </div>

        <!-- TAB 4: Discord / Markdown -->
        <div v-if="activeTab === 'markdown'" class="space-y-4">
          <div class="bg-slate-900/70 border border-slate-800 rounded-xl p-3.5 text-xs text-slate-300 leading-relaxed">
            <strong>{{ $t('share_setup.markdown_banner_title') }}</strong> {{ $t('share_setup.markdown_banner_desc') }}
          </div>

          <div class="relative">
            <textarea
              readonly
              rows="8"
              class="w-full bg-[#070b10] border border-slate-800 rounded-xl p-3 text-xs font-mono text-slate-300 resize-none focus:outline-none"
              :value="markdownPayload"
            ></textarea>
          </div>

          <div class="flex items-center justify-end">
            <button class="z-btn-primary text-xs px-4 py-2 rounded-xl font-bold flex items-center gap-1.5" @click="copyMarkdown">
              <span>{{ markdownCopied ? $t('share_setup.copied_markdown') : $t('share_setup.copy_markdown') }}</span>
            </button>
          </div>
        </div>
      </div>

      <!-- Footer -->
      <div class="px-6 py-4 border-t border-slate-800/80 bg-slate-950/60 flex items-center justify-end">
        <button class="z-btn-ghost text-xs px-4 py-2 rounded-xl font-semibold" @click="handleClose">
          {{ $t('common.close') }}
        </button>
      </div>
    </div>
  </div>
</template>

<script setup>
import { ref, watch } from 'vue';
import { api, saveFile, MRPACK_FILTER, JSON_FILTER } from '../lib/api.js';

const props = defineProps({
  open: { type: Boolean, default: false },
  instance: { type: Object, default: null },
  modCount: { type: Number, default: 0 },
});

const emit = defineEmits(['close', 'editVersion']);

const activeTab = ref('share_code');
const shareCode = ref('');
const jsonPayload = ref('');
const markdownPayload = ref('');

const codeCopied = ref(false);
const jsonCopied = ref(false);
const markdownCopied = ref(false);
const exportingMrpack = ref(false);

watch(
  () => props.open,
  async (isOpen) => {
    if (isOpen && props.instance) {
      loadPayloads();
    }
  }
);

async function loadPayloads() {
  if (!props.instance) return;
  try {
    shareCode.value = await api.exportInstanceSetup(props.instance.id, 'share_code');
    jsonPayload.value = await api.exportInstanceSetup(props.instance.id, 'json');
    markdownPayload.value = await api.exportInstanceSetup(props.instance.id, 'markdown');
  } catch (e) {
    console.error('Failed to generate export payloads:', e);
  }
}

function selectTab(tab) {
  activeTab.value = tab;
}

async function copyShareCode() {
  if (!shareCode.value) return;
  await navigator.clipboard.writeText(shareCode.value);
  codeCopied.value = true;
  setTimeout(() => (codeCopied.value = false), 2500);
}

async function copyJson() {
  if (!jsonPayload.value) return;
  await navigator.clipboard.writeText(jsonPayload.value);
  jsonCopied.value = true;
  setTimeout(() => (jsonCopied.value = false), 2500);
}

async function copyMarkdown() {
  if (!markdownPayload.value) return;
  await navigator.clipboard.writeText(markdownPayload.value);
  markdownCopied.value = true;
  setTimeout(() => (markdownCopied.value = false), 2500);
}

async function handleExportMrpack() {
  if (!props.instance) return;
  exportingMrpack.value = true;
  try {
    const defaultName = `${props.instance.name.replace(/[^a-zA-Z0-9_-]/g, '_')}.mrpack`;
    const target = await saveFile({
      defaultPath: defaultName,
      filters: MRPACK_FILTER,
    });
    if (!target) return;
    await api.exportOfflineInstanceMrpack(props.instance.id, target);
    window.dispatchEvent(
      new CustomEvent('zircon-status', { detail: `Exported .mrpack successfully to ${target}` })
    );
    handleClose();
  } catch (e) {
    window.dispatchEvent(new CustomEvent('zircon-status', { detail: `Export failed: ${e}` }));
  } finally {
    exportingMrpack.value = false;
  }
}

async function downloadJsonFile() {
  if (!props.instance) return;
  try {
    const defaultName = `${props.instance.name.replace(/[^a-zA-Z0-9_-]/g, '_')}-bom.json`;
    const target = await saveFile({
      defaultPath: defaultName,
      filters: [{ name: 'JSON Manifest', extensions: ['json'] }],
    });
    if (!target) return;
    // We can write it via invoke or clipboard notification
    const blob = new Blob([jsonPayload.value], { type: 'application/json' });
    const url = URL.createObjectURL(blob);
    const a = document.createElement('a');
    a.href = url;
    a.download = defaultName;
    a.click();
    URL.revokeObjectURL(url);
  } catch (e) {
    console.error('Download json failed:', e);
  }
}

function handleClose() {
  emit('close');
}
</script>
