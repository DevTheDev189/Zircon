import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import * as THREE from 'three';

const ACTIVE_SKIN_CACHE_KEY = 'zircon_active_skin_cache';

let cachedActiveSkin = (() => {
  try {
    const raw = localStorage.getItem(ACTIVE_SKIN_CACHE_KEY);
    if (raw) return JSON.parse(raw);
  } catch {}
  return null;
})();

export function getCachedActiveSkin() {
  return cachedActiveSkin;
}

export function setCachedActiveSkin(skin) {
  cachedActiveSkin = skin;
  try {
    if (skin) {
      localStorage.setItem(ACTIVE_SKIN_CACHE_KEY, JSON.stringify(skin));
    } else {
      localStorage.removeItem(ACTIVE_SKIN_CACHE_KEY);
    }
  } catch {}
}

export const api = {
  // Auth
  loginMicrosoft: () => invoke('login_microsoft'),
  getCachedSession: () => invoke('get_cached_session'),
  getSession: () => invoke('get_session'),
  logout: () => invoke('logout'),
  listAccounts: () => invoke('list_accounts'),
  switchAccount: (uuid) => invoke('switch_account', { uuid }),
  removeAccount: (uuid) => invoke('remove_account', { uuid }),

  // Servers
  loadSavedServers: () => invoke('load_saved_servers'),

  getServers: () => invoke('load_saved_servers'),
  saveServerList: (servers) => invoke('save_server_list', { serversList: servers }),
  addServer: async (server) => {
    const list = (await invoke('load_saved_servers')) || [];
    const idx = list.findIndex(
      (s) => s.address.toLowerCase() === server.address.toLowerCase()
    );
    if (idx >= 0) {
      list[idx] = { ...list[idx], ...server };
    } else {
      list.push(server);
    }
    await invoke('save_server_list', { serversList: list });
    return list;
  },
  serverStatus: (address, useHttps = false) =>
    invoke('server_status', { address, useHttps }),
  pingServer: (address, useHttps = false) =>
    invoke('server_status', { address, useHttps }),
  probeServer: (address) => invoke('probe_server', { address }),
  deleteServer: (address) => invoke('delete_saved_server', { address }),
  deleteSavedServer: (address) => invoke('delete_saved_server', { address }),
  launchServer: (address, nameOrOpts, installRecommendedPacks = false, useHttps = false) => {
    let name = null;
    let installPacks = false;
    let https = useHttps;
    if (typeof nameOrOpts === 'object' && nameOrOpts !== null) {
      name = nameOrOpts.name || null;
      if (typeof nameOrOpts.installRecommendedPacks === 'boolean') {
        installPacks = nameOrOpts.installRecommendedPacks;
      }
      if (typeof nameOrOpts.useHttps === 'boolean') {
        https = nameOrOpts.useHttps;
      }
    } else if (typeof nameOrOpts === 'string') {
      name = nameOrOpts;
      installPacks = installRecommendedPacks;
    }
    return invoke('launch_server', {
      address,
      name,
      installRecommendedPacks: installPacks,
      useHttps: https,
    });
  },
  respondShaderChoice: (requestId, enabled, remember) =>
    invoke('respond_shader_choice', { requestId, enabled, remember }),
  respondKeyPrompt: (requestId, accepted) =>
    invoke('respond_key_prompt', { requestId, accepted }),
  stopGame: () => invoke('stop_game'),
  getGameStatus: () => invoke('get_game_status'),

  // Offline instances
  listOfflineInstances: () => invoke('list_offline_instances'),
  createOfflineInstance: (optionsOrName, mcVersion, loaderType, loaderVersion) => {
    if (typeof optionsOrName === 'object' && optionsOrName !== null) {
      const {
        name,
        minecraftVersion,
        mcVersion: mcVer,
        modLoader,
        loaderType: lType,
        loaderVersion: lVer,
      } = optionsOrName;
      return invoke('create_offline_instance', {
        name: name || '',
        mcVersion: minecraftVersion || mcVer || '',
        loaderType: lType || modLoader?.type || 'fabric',
        loaderVersion: lVer !== undefined ? lVer : (modLoader?.version || ''),
      });
    }
    return invoke('create_offline_instance', {
      name: optionsOrName || '',
      mcVersion: mcVersion || '',
      loaderType: loaderType || 'fabric',
      loaderVersion: loaderVersion || '',
    });
  },
  deleteOfflineInstance: (id) => invoke('delete_offline_instance', { id }),
  cloneOfflineInstance: (id, newName) => invoke('clone_offline_instance', { id, newName }),
  getOfflineInstanceDir: (id) => invoke('get_offline_instance_dir', { id }),
  openInstanceFolder: (instanceId, subfolder = null) =>
    invoke('open_instance_folder', { instanceId, subfolder }),
  installModrinthModpack: (projectId, versionId = null, customName = null) =>
    invoke('install_modrinth_modpack', { projectId, versionId, customName }),
  importLocalMrpack: (filePath, customName = null) =>
    invoke('import_local_mrpack', { filePath, customName }),
  exportOfflineInstanceMrpack: (instanceId, exportPath) =>
    invoke('export_offline_instance_mrpack', { instanceId, exportPath }),
  exportToZirconServer: (instanceId, worldFolder, exportPath) =>
    invoke('export_to_zircon_server', { instanceId, worldFolder, exportPath }),

  // Worlds, Backups & Screenshots
  listInstanceWorlds: (instanceId) => invoke('list_instance_worlds', { instanceId }),
  backupInstanceWorld: (instanceId, worldFolder) =>
    invoke('backup_instance_world', { instanceId, worldFolder }),
  listInstanceWorldBackups: (instanceId) => invoke('list_instance_world_backups', { instanceId }),
  restoreInstanceWorldBackup: (instanceId, backupFilename) =>
    invoke('restore_instance_world_backup', { instanceId, backupFilename }),
  deleteInstanceWorldBackup: (instanceId, backupFilename) =>
    invoke('delete_instance_world_backup', { instanceId, backupFilename }),
  listInstanceScreenshots: (instanceId) => invoke('list_instance_screenshots', { instanceId }),
  deleteInstanceScreenshot: (instanceId, filename) =>
    invoke('delete_instance_screenshot', { instanceId, filename }),

  // Tier 2: Co-Op Session (Host for Friends) & P2P Mod Sync
  startCoopSession: (instanceId, worldName, preferredP2pPort = null) =>
    invoke('start_coop_session', { instanceId, worldName, preferredP2pPort }),
  stopCoopSession: () => invoke('stop_coop_session'),
  getCoopSessionStatus: () => invoke('get_coop_session_status'),
  resolveCoopCode: (codeOrAddress) => invoke('resolve_coop_code', { codeOrAddress }),
  coopPreflight: (hostAddress, p2pPort, gamePort, targetInstanceId = null) =>
    invoke('coop_preflight', { hostAddress, p2pPort, gamePort, targetInstanceId }),
  coopSyncMods: (hostAddress, p2pPort, missingMods, approvedCustomSha1s = [], targetInstanceId = null) =>
    invoke('coop_sync_mods', { hostAddress, p2pPort, missingMods, approvedCustomSha1s, targetInstanceId }),

  launchOfflineInstance: (id) => invoke('launch_offline_instance', { id }),

  listOfflineMods: (id) => invoke('list_offline_mods', { id }),
  deleteOfflineMod: (id, filename) => invoke('delete_offline_mod', { id, filename }),
  setOfflineModEnabled: (id, filename, enabled) =>
    invoke('set_offline_mod_enabled', { id, filename, enabled }),
  addOfflineMod: (id, sourcePath) => invoke('import_offline_mod_file', { id, sourcePath }),
  importOfflineModFile: (id, sourcePath) => invoke('import_offline_mod_file', { id, sourcePath }),
  importOfflineModBytes: (id, filename, bytes) =>
    invoke('import_offline_mod_bytes', { id, filename, bytes }),


  // Server instance mods
  getServerInstanceMods: (address) => invoke('get_server_instance_mods', { address }),
  addServerModFile: (address, sourcePath) => invoke('add_server_mod_file', { address, sourcePath }),
  addServerModBytes: (address, filename, bytes) => invoke('add_server_mod_bytes', { address, filename, bytes }),
  setServerModEnabled: (address, filename, enabled) => invoke('set_server_mod_enabled', { address, filename, enabled }),
  deleteServerMod: (address, filename) => invoke('delete_server_mod', { address, filename }),
  installServerModrinthMod: (address, projectId, versionId = null) =>
    invoke('install_server_modrinth_mod', { address, projectId, versionId }),

  // Skins
  getActiveSkin: async () => {
    const res = await invoke('get_active_skin');
    if (res && (res.dataUrl || res.data_url)) {
      setCachedActiveSkin({
        dataUrl: res.dataUrl || res.data_url,
        variant: res.variant || 'classic',
        name: res.name || 'active_skin.png',
      });
    } else if (res === null) {
      setCachedActiveSkin(null);
    }
    return res;
  },
  getSkinHeadIcon: () => invoke('get_skin_head_icon'),
  saveSkin: async (sourcePath, variant = 'classic') => {
    const res = await invoke('save_skin', { sourcePath, variant });
    api.getActiveSkin().catch(() => {});
    return res;
  },
  importCustomSkin: (sourcePath, variant = 'classic') =>
    api.saveSkin(sourcePath, variant),
  setActiveSkinVariant: async (variant) => {
    const res = await invoke('set_active_skin_variant', { variant });
    if (cachedActiveSkin) {
      setCachedActiveSkin({ ...cachedActiveSkin, variant });
    }
    return res;
  },
  removeSkin: async () => {
    const res = await invoke('remove_skin');
    setCachedActiveSkin(null);
    return res;
  },
  getSkinHistory: () => invoke('get_skin_history'),
  getBundledSkins: () => invoke('get_bundled_skins'),
  saveBundledSkin: async (key, variant) => {
    const res = await invoke('save_bundled_skin', { key, variant });
    api.getActiveSkin().catch(() => {});
    return res;
  },
  fetchMojangSkin: (uuid) => invoke('fetch_mojang_skin', { uuid }),
  fetchMojangSkinActive: async (uuid) => {
    const res = await invoke('fetch_mojang_skin_active', { uuid });
    api.getActiveSkin().catch(() => {});
    return res;
  },
  fetchMojangSkinPreview: (uuid) => invoke('fetch_mojang_skin_preview', { uuid }),
  fetchSkinByUsername: (username) => invoke('fetch_skin_by_username', { username }),
  fetchCommunitySkins: (after) => invoke('fetch_community_skins', { after, page: after }),
  fetchSkinByUrl: (url, name) => invoke('fetch_skin_by_url', { url, name }),
  saveSkinBytes: async (name, bytes, variant = 'classic') => {
    const res = await invoke('save_skin_bytes', { name, bytes, variant });
    api.getActiveSkin().catch(() => {});
    return res;
  },
  activateHistorySkin: async (filename, variant) => {
    const res = await invoke('activate_history_skin', { filename, variant });
    api.getActiveSkin().catch(() => {});
    return res;
  },
  deleteHistorySkin: (filename) => invoke('delete_history_skin', { filename }),
  deletePresetSkin: (filename) => invoke('delete_history_skin', { filename }),
  renameSkin: (filename, newName) => invoke('rename_skin', { filename, newName }),
  uploadSkinToMojang: (variant = 'classic') => invoke('upload_skin_to_mojang', { variant }),

  // Packs
  listInstancePacks: (gameDir) => invoke('list_instance_packs', { gameDir }),
  listInstancePacksDetailed: (gameDir) => invoke('list_instance_packs_detailed', { gameDir }), // fetch pack metadata
  addLocalPack: (gameDir, sourcePath, kind) =>
    invoke('add_local_pack', { gameDir, sourcePath, kind }),
  importInstancePack: (gameDir, kind, sourcePath) =>
    invoke('import_instance_pack', { gameDir, kind, sourcePath }),
  importInstancePackBytes: (gameDir, kind, filename, bytes) =>
    invoke('import_instance_pack_bytes', { gameDir, kind, filename, bytes }),
  removeLocalPack: (gameDir, kind, filename) =>
    invoke('remove_local_pack', { gameDir, kind, filename }),
  setActiveShaderpack: (gameDir, filename) =>
    invoke('set_active_shaderpack', { gameDir, filename }),
  setActiveResourcepacks: (gameDir, filenames) =>
    invoke('set_active_resourcepacks', { gameDir, filenames }),
  toggleResourcepack: (gameDir, filename) =>
    invoke('toggle_resourcepack', { gameDir, filename }),

  // Unified Mod & Pack Discovery (Modrinth & CurseForge)
  searchMods: (instanceId, query, origin = 'modrinth', projectType = 'mod', allVersions = false) =>
    invoke('search_mods', { instanceId, query, origin, projectType, allVersions }),
  searchModrinthMods: (query, mcVersion, loader) =>
    invoke('search_mods', { instanceId: '', query, origin: 'modrinth', projectType: 'mod', allVersions: true }),
  searchModrinth: (instanceId, query, projectType = 'mod', allVersions = false) =>
    invoke('search_mods', { instanceId, query, origin: 'modrinth', projectType, allVersions }),
  listModVersions: (instanceId, projectId, origin = 'modrinth', allVersions = false) =>
    invoke('list_mod_versions', { instanceId, projectId, origin, allVersions }),
  getModrinthVersions: (projectId, mcVersion, loader) =>
    invoke('list_mod_versions', { instanceId: '', projectId, origin: 'modrinth', allVersions: true }),
  listModrinthVersions: (instanceId, projectId, allVersions = false) =>
    invoke('list_mod_versions', { instanceId, projectId, origin: 'modrinth', allVersions }),
  installModrinthPack: (instanceId, projectId, versionId = null, projectType = 'mod') =>
    invoke('install_modrinth_pack', { instanceId, projectId, versionId, projectType }),
  installModrinthMod: (instanceId, projectId, versionId = null) =>
    invoke('install_modrinth_pack', { instanceId, projectId, versionId, projectType: 'mod' }),
  installModrinthVersion: (instanceId, versionId, projectType = 'mod') =>
    invoke('install_modrinth_pack', { instanceId, projectId: '', versionId, projectType }),
  checkModDependencies: (instanceId, projectId, versionId = null) =>
    invoke('check_mod_dependencies', { instanceId, projectId, versionId }),
  installModWithDependencies: (instanceId, items) =>
    invoke('install_mod_with_dependencies', { instanceId, items }),
  checkInstanceModUpdates: (instanceId) =>
    invoke('check_instance_mod_updates', { instanceId }),
  updateInstanceMods: (instanceId, updates) =>
    invoke('update_instance_mods', { instanceId, updates }),
  listMinecraftVersions: () => invoke('list_minecraft_versions'),

  getMinecraftVersions: (snapshots = false) => invoke('get_minecraft_versions', { snapshots }),
  listLoaderTypes: () => invoke('list_loader_types'),
  getLoaderVersions: (loader, mcVersion) => invoke('get_loader_versions', { loader, mcVersion }),
  getLauncherMetadata: () => invoke('get_launcher_metadata'),
  openExternalUrl: (url) => invoke('open_browser_url', { url }),
  openBrowserUrl: (url) => invoke('open_browser_url', { url }),

  // Settings & App Info
  getSettings: () => invoke('get_settings'),
  saveSettings: (settings) => invoke('save_settings', { settings }),
  showMainWindow: () => invoke('show_main_window'),
  getLauncherVersion: () => invoke('get_launcher_version'),
  logDebug: (message) => invoke('log_debug_message', { message }),

  // Debug logs & crash diagnostics
  getLauncherLogs: () => invoke('get_launcher_logs'),
  getDebugLogs: () => invoke('get_launcher_logs'),
  clearLauncherLogs: () => invoke('clear_launcher_logs'),
  clearDebugLogs: () => invoke('clear_launcher_logs'),
  getLastInstanceLog: () => invoke('get_last_instance_log'),
  getLastMcLog: () => invoke('get_last_instance_log'),
  clearLastInstanceLog: () => invoke('clear_last_instance_log'),
  clearLastMcLog: () => invoke('clear_last_instance_log'),
  checkGameCrash: (gameDir) => invoke('check_game_crash', { gameDir }),
};

// Launch-flow events emitted from Rust while a game is being prepared/run.
export const onLaunchStatus = (cb) => listen('launch-status', (e) => cb(e.payload));
export const onLaunchProgress = (cb) => listen('launch-progress', (e) => cb(e.payload));
export const onGameOutput = (cb) => listen('game-output', (e) => cb(e.payload));
export const onGameStatus = (cb) => listen('game-status', (e) => cb(e.payload));
export const onGameWindowReady = (cb) => listen('game-window-ready', (e) => cb(e.payload));
export const onModpackProgress = (cb) => listen('modpack-progress', (e) => cb(e.payload));

// Emitted by Rust whenever the active skin changes (save, add, boot fetch,
// remove) so the sidebar avatar can refresh.
export const onSkinUpdated = (cb) => listen('skin-updated', () => cb());

// Emitted by Rust during a server launch when the server offers shaders and
// the player's choice has not been remembered yet. Respond via
// `respondShaderChoice` to continue the launch.
export const onShaderRequest = (cb) => listen('shader-request', (e) => cb(e.payload));

// Emitted by Rust during a server launch when the server presents a different
// Ed25519 key than the one pinned on first contact (reinstall or takeover).
// Respond via `respondKeyPrompt` to accept or reject the rotation.
export const onServerKeyMismatch = (cb) => listen('server-key-mismatch', (e) => cb(e.payload));

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

export async function saveFile(options = {}) {
  const { save } = await import('@tauri-apps/plugin-dialog');
  return await save(options);
}

// Shared filter presets.
export const JAR_FILTER = [{ name: 'JAR Files', extensions: ['jar'] }];
export const PNG_FILTER = [{ name: 'PNG Images', extensions: ['png'] }];
export const PACK_FILTER = [{ name: 'ZIP Archives', extensions: ['zip'] }];
export const ZIP_FILTER = [{ name: 'ZIP Archives', extensions: ['zip'] }];
export const MRPACK_FILTER = [{ name: 'Modrinth Modpack', extensions: ['mrpack'] }];
export const MODPACK_FILTER = [{ name: 'Modpack Archives (.mrpack, .zip)', extensions: ['mrpack', 'zip'] }];
export const EXE_FILTER = [{ name: 'Executables', extensions: ['exe', 'bat', 'cmd', '*'] }];




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

// Canonical 64x64 Zircon-Steve Skin (Official Zircon character)
export const ZIRCON_STEVE_DATA_URL =
  'data:image/png;base64,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';

export function createDefaultSteveDataUrl() {
  return ZIRCON_STEVE_DATA_URL;
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

// ---- 3D Isometric Skin Render Engine ---------------------------------------
function copyFlipped(ctx, sx, sy, sw, sh, dx, dy) {
  ctx.save();
  ctx.translate(dx + sw, dy);
  ctx.scale(-1, 1);
  ctx.drawImage(ctx.canvas, sx, sy, sw, sh, 0, 0, sw, sh);
  ctx.restore();
}

function isAreaTransparent(ctx, x, y, w, h) {
  try {
    const imgData = ctx.getImageData(x, y, w, h).data;
    for (let i = 3; i < imgData.length; i += 4) {
      if (imgData[i] > 10) return false;
    }
  } catch {
    return false;
  }
  return true;
}

function mirrorLegacyLimb(ctx, isArm) {
  if (isArm) {
    copyFlipped(ctx, 44, 16, 4, 4, 36, 48); // Top
    copyFlipped(ctx, 48, 16, 4, 4, 40, 48); // Bottom
    copyFlipped(ctx, 48, 20, 4, 12, 32, 52); // Inside
    copyFlipped(ctx, 44, 20, 4, 12, 36, 52); // Front
    copyFlipped(ctx, 40, 20, 4, 12, 40, 52); // Outside
    copyFlipped(ctx, 52, 20, 4, 12, 44, 52); // Back
  } else {
    copyFlipped(ctx, 4, 16, 4, 4, 20, 48); // Top
    copyFlipped(ctx, 8, 16, 4, 4, 24, 48); // Bottom
    copyFlipped(ctx, 8, 20, 4, 12, 16, 52); // Inside
    copyFlipped(ctx, 4, 20, 4, 12, 20, 52); // Front
    copyFlipped(ctx, 0, 20, 4, 12, 24, 52); // Outside
    copyFlipped(ctx, 12, 20, 4, 12, 28, 52); // Back
  }
}

export function processSkinCanvas(image) {
  if (!image || image.width < 32 || image.height < 32) return null;
  const cvs = document.createElement('canvas');
  cvs.width = 64;
  cvs.height = 64;
  const ctx = cvs.getContext('2d', { willReadFrequently: true });
  ctx.imageSmoothingEnabled = false;
  ctx.drawImage(image, 0, 0);

  const isLegacy64x32 = image.height === 32 || image.height < 64;
  const isLeftLegEmpty = isLegacy64x32 || isAreaTransparent(ctx, 16, 48, 16, 16);
  const isLeftArmEmpty = isLegacy64x32 || isAreaTransparent(ctx, 32, 48, 16, 16);

  if (isLeftLegEmpty) mirrorLegacyLimb(ctx, false);
  if (isLeftArmEmpty) mirrorLegacyLimb(ctx, true);

  return cvs;
}

function faces(base, w, h, d) {
  const [bu, bv] = base;
  return {
    front: [bu + d, bv + d, w, h],
    back: [bu + d + w + d, bv + d, w, h],
    right: [bu, bv + d, d, h],
    left: [bu + d + w, bv + d, d, h],
    top: [bu + d, bv, w, d],
    bottom: [bu + d + w, bv, w, d],
  };
}

const HEAD = { size: [8, 8, 8], center: [0, 28, 0], atlas: faces([0, 0], 8, 8, 8) };
const HAT = { size: [8.8, 8.8, 8.8], center: [0, 28, 0], atlas: faces([32, 0], 8, 8, 8) };
const BODY = { size: [8, 12, 4], center: [0, 18, 0], atlas: faces([16, 16], 8, 12, 4) };
const JACKET = { size: [8.5, 12.5, 4.5], center: [0, 18, 0], atlas: faces([16, 32], 8, 12, 4) };
const R_ARM = { size: [4, 12, 4], center: [-6, 18, 0], atlas: faces([40, 16], 4, 12, 4) };
const R_SLEEVE = { size: [4.5, 12.5, 4.5], center: [-6, 18, 0], atlas: faces([40, 32], 4, 12, 4) };
const L_ARM = { size: [4, 12, 4], center: [6, 18, 0], atlas: faces([32, 48], 4, 12, 4) };
const L_SLEEVE = { size: [4.5, 12.5, 4.5], center: [6, 18, 0], atlas: faces([48, 48], 4, 12, 4) };
const R_ARM_SLIM = { size: [3, 12, 4], center: [-5.5, 18, 0], atlas: faces([40, 16], 3, 12, 4) };
const R_SLEEVE_SLIM = { size: [3.5, 12.5, 4.5], center: [-5.5, 18, 0], atlas: faces([40, 32], 3, 12, 4) };
const L_ARM_SLIM = { size: [3, 12, 4], center: [5.5, 18, 0], atlas: faces([32, 48], 3, 12, 4) };
const L_SLEEVE_SLIM = { size: [3.5, 12.5, 4.5], center: [5.5, 18, 0], atlas: faces([48, 48], 3, 12, 4) };
const R_LEG = { size: [4, 12, 4], center: [-2, 6, 0], atlas: faces([0, 16], 4, 12, 4) };
const R_PANTS = { size: [4.5, 12.5, 4.5], center: [-2, 6, 0], atlas: faces([0, 32], 4, 12, 4) };
const L_LEG = { size: [4, 12, 4], center: [2, 6, 0], atlas: faces([16, 48], 4, 12, 4) };
const L_PANTS = { size: [4.5, 12.5, 4.5], center: [2, 6, 0], atlas: faces([0, 48], 4, 12, 4) };

function getModelBoxes(variant) {
  const isSlim = variant === 'slim';
  const rArm = isSlim ? R_ARM_SLIM : R_ARM;
  const lArm = isSlim ? L_ARM_SLIM : L_ARM;
  const rSleeve = isSlim ? R_SLEEVE_SLIM : R_SLEEVE;
  const lSleeve = isSlim ? L_SLEEVE_SLIM : L_SLEEVE;
  const baseBoxes = [HEAD, BODY, rArm, lArm, R_LEG, L_LEG];
  const overlayBoxes = [HAT, JACKET, rSleeve, lSleeve, R_PANTS, L_PANTS];
  return { baseBoxes, overlayBoxes };
}

function buildBoxesIntoGeo(builder, boxes) {
  for (const box of boxes) {
    const [Cx, Cy, Cz] = box.center;
    const [sx, sy, sz] = box.size;
    const hx = sx / 2, hy = sy / 2, hz = sz / 2;
    const a = box.atlas;
    const facesList = [
      { key: 'front', norm: [0, 0, 1], corners: [[Cx-hx, Cy+hy, Cz+hz], [Cx-hx, Cy-hy, Cz+hz], [Cx+hx, Cy+hy, Cz+hz], [Cx+hx, Cy-hy, Cz+hz]] },
      { key: 'back', norm: [0, 0, -1], corners: [[Cx+hx, Cy+hy, Cz-hz], [Cx+hx, Cy-hy, Cz-hz], [Cx-hx, Cy+hy, Cz-hz], [Cx-hx, Cy-hy, Cz-hz]] },
      { key: 'right', norm: [-1, 0, 0], corners: [[Cx-hx, Cy+hy, Cz-hz], [Cx-hx, Cy-hy, Cz-hz], [Cx-hx, Cy+hy, Cz+hz], [Cx-hx, Cy-hy, Cz+hz]] },
      { key: 'left', norm: [1, 0, 0], corners: [[Cx+hx, Cy+hy, Cz+hz], [Cx+hx, Cy-hy, Cz+hz], [Cx+hx, Cy+hy, Cz-hz], [Cx+hx, Cy-hy, Cz-hz]] },
      { key: 'top', norm: [0, 1, 0], corners: [[Cx-hx, Cy+hy, Cz-hz], [Cx-hx, Cy+hy, Cz+hz], [Cx+hx, Cy+hy, Cz-hz], [Cx+hx, Cy+hy, Cz+hz]] },
      { key: 'bottom', norm: [0, -1, 0], corners: [[Cx-hx, Cy-hy, Cz+hz], [Cx-hx, Cy-hy, Cz-hz], [Cx+hx, Cy-hy, Cz+hz], [Cx+hx, Cy-hy, Cz-hz]] },
    ];
    for (const f of facesList) {
      const r = a[f.key];
      const u0 = r[0] / 64;
      const u1 = (r[0] + r[2]) / 64;
      const v0 = 1.0 - r[1] / 64;
      const v1 = 1.0 - (r[1] + r[3]) / 64;
      const baseIdx = builder.positions.length / 3;
      for (const v of f.corners) builder.positions.push(v[0], v[1], v[2]);
      for (let i = 0; i < 4; i++) builder.normals.push(f.norm[0], f.norm[1], f.norm[2]);
      builder.uvs.push(u0, v0, u0, v1, u1, v0, u1, v1);
      builder.indices.push(
        baseIdx, baseIdx + 1, baseIdx + 2,
        baseIdx + 2, baseIdx + 1, baseIdx + 3
      );
    }
  }
}

let offscreenCtx = null;

export async function renderSkinIsometric3D(skinDataUrl, variant = 'classic') {
  return new Promise((resolve) => {
    if (!skinDataUrl) return resolve(null);
    const img = new Image();
    img.crossOrigin = 'anonymous';
    img.onload = () => {
      try {
        if (!offscreenCtx) {
          const canvas = document.createElement('canvas');
          canvas.width = 160;
          canvas.height = 220;
          const renderer = new THREE.WebGLRenderer({
            canvas,
            alpha: true,
            antialias: true,
            preserveDrawingBuffer: true,
          });
          renderer.setSize(160, 220, false);
          renderer.outputColorSpace = THREE.SRGBColorSpace;

          const scene = new THREE.Scene();
          const camera = new THREE.PerspectiveCamera(38, 160 / 220, 0.1, 100);
          camera.position.set(0, 16, 44);
          camera.lookAt(0, 13, 0);

          const key = new THREE.DirectionalLight(0xffffff, 1.8);
          key.position.set(10, 30, 20);
          scene.add(key);

          const fill = new THREE.DirectionalLight(0x47d2c9, 0.6);
          fill.position.set(-15, 10, -12);
          scene.add(fill);

          scene.add(new THREE.AmbientLight(0xffffff, 0.5));

          const material = new THREE.MeshLambertMaterial({
            color: 0xffffff,
            transparent: true,
            alphaTest: 0.5,
            side: THREE.FrontSide,
          });

          offscreenCtx = { canvas, renderer, scene, camera, material };
        }

        const { canvas, renderer, scene, camera, material } = offscreenCtx;
        const cvs = processSkinCanvas(img);
        if (!cvs) return resolve(null);

        const tex = new THREE.CanvasTexture(cvs);
        tex.magFilter = THREE.NearestFilter;
        tex.minFilter = THREE.NearestFilter;
        tex.generateMipmaps = false;
        tex.colorSpace = THREE.SRGBColorSpace;

        material.map = tex;
        material.needsUpdate = true;

        // Clear existing model
        const toRemove = scene.children.filter((c) => c.isGroup);
        toRemove.forEach((c) => scene.remove(c));

        const { baseBoxes, overlayBoxes } = getModelBoxes(variant);
        const baseBuilder = { positions: [], normals: [], uvs: [], indices: [] };
        const overlayBuilder = { positions: [], normals: [], uvs: [], indices: [] };
        buildBoxesIntoGeo(baseBuilder, baseBoxes);
        buildBoxesIntoGeo(overlayBuilder, overlayBoxes);

        const toGeo = (b) => {
          const geo = new THREE.BufferGeometry();
          geo.setAttribute('position', new THREE.Float32BufferAttribute(b.positions, 3));
          geo.setAttribute('normal', new THREE.Float32BufferAttribute(b.normals, 3));
          geo.setAttribute('uv', new THREE.Float32BufferAttribute(b.uvs, 2));
          geo.setIndex(b.indices);
          return geo;
        };

        const base = new THREE.Mesh(toGeo(baseBuilder), material);
        const overlay = new THREE.Mesh(toGeo(overlayBuilder), material);
        overlay.renderOrder = 1;
        base.renderOrder = 0;
        const group = new THREE.Group();
        group.add(base, overlay);
        group.scale.setScalar(0.76);
        group.rotation.set(-0.1, -0.35, 0); // Gentle 3D front-isometric angle
        scene.add(group);

        renderer.render(scene, camera);
        const renderedUri = canvas.toDataURL('image/png');
        tex.dispose();
        resolve(renderedUri);
      } catch (err) {
        console.error('Failed to render 3D isometric skin:', err);
        resolve(null);
      }
    };
    img.onerror = () => resolve(null);
    img.src = skinDataUrl;
  });
}
