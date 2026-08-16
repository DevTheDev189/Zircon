// Thin typed wrappers over the Tauri IPC surface exposed by
// `crates/zircon-launcher/src/commands.rs`. Argument keys use the camelCase
// form Tauri maps onto the Rust snake_case parameters automatically.
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';

export const api = {
  // Auth
  loginMicrosoft: () => invoke('login_microsoft'),
  getCachedSession: () => invoke('get_cached_session'),
  getSession: () => invoke('get_session'),
  logout: () => invoke('logout'),

  // Servers
  loadSavedServers: () => invoke('load_saved_servers'),
  saveServerList: (servers) => invoke('save_server_list', { serversList: servers }),
  serverStatus: (address, useHttps = false) =>
    invoke('server_status', { address, useHttps }),
  deleteServer: (address) => invoke('delete_saved_server', { address }),
  launchServer: (address, name, installRecommendedPacks, useHttps = false) =>
    invoke('launch_server', { address, name, installRecommendedPacks, useHttps }),
  respondShaderChoice: (requestId, enabled, remember) =>
    invoke('respond_shader_choice', { requestId, enabled, remember }),
  stopGame: () => invoke('stop_game'),
  getGameStatus: () => invoke('get_game_status'),

  // Offline instances
  listOfflineInstances: () => invoke('list_offline_instances'),
  createOfflineInstance: (name, mcVersion, loaderType, loaderVersion) =>
    invoke('create_offline_instance', { name, mcVersion, loaderType, loaderVersion }),
  deleteOfflineInstance: (id) => invoke('delete_offline_instance', { id }),
  getOfflineInstanceDir: (id) => invoke('get_offline_instance_dir', { id }),
  launchOfflineInstance: (id) => invoke('launch_offline_instance', { id }),
  listOfflineMods: (id) => invoke('list_offline_mods', { id }),
  deleteOfflineMod: (id, filename) => invoke('delete_offline_mod', { id, filename }),
  addOfflineMod: (id, sourcePath) => invoke('add_offline_mod', { id, sourcePath }),

  // Skins
  getActiveSkin: () => invoke('get_active_skin'),
  getSkinHeadIcon: () => invoke('get_skin_head_icon'),
  saveSkin: (sourcePath, variant) => invoke('save_skin', { sourcePath, variant }),
  setActiveSkinVariant: (variant) => invoke('set_active_skin_variant', { variant }),
  removeSkin: () => invoke('remove_skin'),
  getSkinHistory: () => invoke('get_skin_history'),
  getBundledSkins: () => invoke('get_bundled_skins'),
  saveBundledSkin: (key, variant) => invoke('save_bundled_skin', { key, variant }),
  fetchMojangSkin: (uuid) => invoke('fetch_mojang_skin', { uuid }),
  fetchMojangSkinActive: (uuid) => invoke('fetch_mojang_skin_active', { uuid }),
  fetchMojangSkinPreview: (uuid) => invoke('fetch_mojang_skin_preview', { uuid }),
  activateHistorySkin: (filename, variant) =>
    invoke('activate_history_skin', { filename, variant }),
  deleteHistorySkin: (filename) => invoke('delete_history_skin', { filename }),
  uploadSkinToMojang: (variant) => invoke('upload_skin_to_mojang', { variant }),

  // Packs
  listInstancePacks: (gameDir) => invoke('list_instance_packs', { gameDir }),
  addLocalPack: (gameDir, sourcePath, kind) =>
    invoke('add_local_pack', { gameDir, sourcePath, kind }),
  removeLocalPack: (gameDir, kind, filename) =>
    invoke('remove_local_pack', { gameDir, kind, filename }),
  setActiveShaderpack: (gameDir, filename) =>
    invoke('set_active_shaderpack', { gameDir, filename }),
  toggleResourcepack: (gameDir, filename) =>
    invoke('toggle_resourcepack', { gameDir, filename }),

  // Modrinth
  searchModrinth: (instanceId, query) => invoke('search_modrinth', { instanceId, query }),
  installModrinthMod: (instanceId, projectId) =>
    invoke('install_modrinth_mod', { instanceId, projectId }),
  listMinecraftVersions: () => invoke('list_minecraft_versions'),
  listLoaderTypes: () => invoke('list_loader_types'),

  // Settings
  getSettings: () => invoke('get_settings'),
  saveSettings: (settings) => invoke('save_settings', { settings }),
};

// Launch-flow events emitted from Rust while a game is being prepared/run.
export const onLaunchStatus = (cb) => listen('launch-status', (e) => cb(e.payload));
export const onLaunchProgress = (cb) => listen('launch-progress', (e) => cb(e.payload));
export const onGameOutput = (cb) => listen('game-output', (e) => cb(e.payload));
export const onGameStatus = (cb) => listen('game-status', (e) => cb(e.payload));

// Emitted by Rust whenever the active skin changes (save, add, boot fetch,
// remove) so the sidebar avatar can refresh.
export const onSkinUpdated = (cb) => listen('skin-updated', () => cb());

// Emitted by Rust during a server launch when the server offers shaders and
// the player's choice has not been remembered yet. Respond via
// `respondShaderChoice` to continue the launch.
export const onShaderRequest = (cb) => listen('shader-request', (e) => cb(e.payload));

// Native file dialogs (WebView2 file inputs only expose a fake path).
export async function pickFile(filters) {
  const { open } = await import('@tauri-apps/plugin-dialog');
  const picked = await open({ multiple: false, filters });
  return typeof picked === 'string' ? picked : null;
}

export async function pickFiles(filters) {
  const { open } = await import('@tauri-apps/plugin-dialog');
  const picked = await open({ multiple: true, filters });
  return Array.isArray(picked) ? picked : [];
}

// Shared filter presets.
export const JAR_FILTER = [{ name: 'JAR Files', extensions: ['jar'] }];
export const PNG_FILTER = [{ name: 'PNG Images', extensions: ['png'] }];
export const PACK_FILTER = [{ name: 'ZIP Archives', extensions: ['zip'] }];

// Small helpers -----------------------------------------------------------

// Renders a skin's front face (base head layer + hat overlay layer, exactly
// like the in-game model) as a small pixel-art PNG data URL. Falls back to
// `null` when the skin cannot be decoded.
export function skinFaceDataUrl(skinDataUrl, scale = 8) {
  return new Promise((resolve) => {
    if (!skinDataUrl) return resolve(null);
    const img = new Image();
    img.onload = () => {
      try {
        // Draw the full skin at its native size first so the source regions
        // below can be sampled exactly.
        const src = document.createElement('canvas');
        src.width = 64;
        src.height = 64;
        const sctx = src.getContext('2d');
        sctx.imageSmoothingEnabled = false;
        sctx.drawImage(img, 0, 0);

        const size = 8 * scale;
        const out = document.createElement('canvas');
        out.width = size;
        out.height = size;
        const octx = out.getContext('2d');
        octx.imageSmoothingEnabled = false;
        // Head front (8,8)-(16,16), then hat front (40,8)-(48,16) on top.
        octx.drawImage(src, 8, 8, 8, 8, 0, 0, size, size);
        octx.drawImage(src, 40, 8, 8, 8, 0, 0, size, size);
        resolve(out.toDataURL('image/png'));
      } catch {
        resolve(null);
      }
    };
    img.onerror = () => resolve(null);
    img.src = skinDataUrl;
  });
}

export function fmtBytes(bytes) {
  if (!bytes) return '0 B';
  const units = ['B', 'KB', 'MB', 'GB'];
  let i = 0;
  let v = bytes;
  while (v >= 1024 && i < units.length - 1) {
    v /= 1024;
    i += 1;
  }
  return `${v.toFixed(i === 0 ? 0 : 1)} ${units[i]}`;
}
