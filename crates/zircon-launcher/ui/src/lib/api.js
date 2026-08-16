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
  launchServer: (address, name, installRecommendedPacks) =>
    invoke('launch_server', { address, name, installRecommendedPacks }),
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
  saveSkin: (sourcePath) => invoke('save_skin', { sourcePath }),
  removeSkin: () => invoke('remove_skin'),
  getSkinHistory: () => invoke('get_skin_history'),
  getBundledSkins: () => invoke('get_bundled_skins'),
  saveBundledSkin: (key) => invoke('save_bundled_skin', { key }),
  fetchMojangSkin: (uuid) => invoke('fetch_mojang_skin', { uuid }),
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
