/**
 * Mock API provider for Zircon Launcher UI
 * Enables standalone browser rendering, automated testing, and Playwright screenshot generation
 * without requiring the compiled Rust Tauri backend.
 */

export const isMockMode = (() => {
  if (typeof window === 'undefined') return false;
  const params = new URLSearchParams(window.location.search);
  return params.has('mock') || !window.__TAURI_INTERNALS__;
})();

const MOCK_SESSION = {
  uuid: '069a79f4-44e9-4726-a5be-fca90e38aaf5',
  username: 'Steve',
  avatar: 'https://minotar.net/avatar/Steve',
};

const MOCK_INSTANCES = [
  {
    id: 'flower-charm',
    name: 'Flower-Charm',
    minecraftVersion: '1.20.1',
    modLoader: { type: 'fabric', version: '0.15.11' },
    javaArgs: '-Xms4G -Xmx6G',
    lastPlayed: Date.now() - 1800000,
  },
  {
    id: 'atm9',
    name: 'All the Mods 9',
    minecraftVersion: '1.20.1',
    modLoader: { type: 'neoforge', version: '47.1.3' },
    javaArgs: '-Xms4G -Xmx8G',
    lastPlayed: Date.now() - 86400000,
  },
  {
    id: 'prominence',
    name: 'Prominence II RPG',
    minecraftVersion: '1.20.1',
    modLoader: { type: 'fabric', version: '0.15.11' },
    javaArgs: '-Xms4G -Xmx6G',
    lastPlayed: Date.now() - 172800000,
  },
];

const MOCK_MODS = [
  { filename: 'create-1.20.1-0.5.1.f.jar', size_bytes: 14820100, author: 'simibubi', version: '0.5.1.f', enabled: true },
  { filename: 'sodium-fabric-0.5.8+mc1.20.1.jar', size_bytes: 1240500, author: 'jellysquid3', version: '0.5.8', enabled: true },
  { filename: 'iris-1.6.17+1.20.1.jar', size_bytes: 2890000, author: 'coderbot', version: '1.6.17', enabled: true },
  { filename: 'jei-1.20.1-fabric-15.3.0.4.jar', size_bytes: 1650000, author: 'mezz', version: '15.3.0', enabled: true },
  { filename: 'waystones-fabric-1.20.1-14.1.3.jar', size_bytes: 520000, author: 'BlayTheNinth', version: '14.1.3', enabled: true },
  { filename: 'appleskin-fabric-mc1.20.1-2.5.1.jar', size_bytes: 310000, author: 'squeek502', version: '2.5.1', enabled: true },
  { filename: 'ferritecore-6.0.3-fabric.jar', size_bytes: 290000, author: 'malte0811', version: '6.0.3', enabled: true },
  { filename: 'clumps-fabric-1.20.1-12.0.0.3.jar', size_bytes: 180000, author: 'jaredlll08', version: '12.0.0', enabled: true },
];

const MOCK_SERVERS = [
  {
    name: 'Emerald Survival SMP',
    address: 'emerald.zirconmc.net',
    online: true,
    players: { online: 52, max: 100 },
    motd: '§a✦ Emerald SMP §7· §fAuto-Synced Modpack §e[1.20.1]',
    iconUrl: '/mock_assets/emerald_icon.png',
    bannerUrl: '/mock_assets/emerald_bg.png',
  },
  {
    name: 'Beehive SMP',
    address: 'play.beehivesmp.net',
    online: true,
    players: { online: 34, max: 75 },
    motd: '§6✦ Beehive SMP §7· §fCommunity Adventure & Honey §e[1.20.1]',
    iconUrl: '/mock_assets/beehive_icon.png',
    bannerUrl: '/mock_assets/beehive_bg.png',
  },
  {
    name: 'Flower-Charm Realm',
    address: 'flowercharm.zirconmc.net',
    online: true,
    players: { online: 28, max: 60 },
    motd: '§d✿ Flower-Charm §7· §fWholesome Modded SMP §e[1.20.1]',
    iconUrl: '/mock_assets/flower_charm_icon.png',
    bannerUrl: '/mock_assets/flower_charm_bg.jpg',
  },
];

const MOCK_MIGRATION = [
  {
    launcherType: 'curse_forge',
    launcherPath: 'C:\\Users\\Player\\curseforge\\minecraft\\Instances',
    instances: [
      { id: 'cf-1', name: 'Better MC [FORGE] - BMC4', minecraftVersion: '1.20.1', loaderType: 'Forge', loaderVersion: '47.1.3', modCount: 184, worldCount: 2, lastPlayedMs: Date.now() - 3600000, sourceLauncher: 'curse_forge' },
      { id: 'cf-2', name: 'DawnCraft - Echoes of Legends', minecraftVersion: '1.18.2', loaderType: 'Forge', loaderVersion: '40.2.1', modCount: 215, worldCount: 1, lastPlayedMs: Date.now() - 86400000, sourceLauncher: 'curse_forge' },
      { id: 'cf-3', name: 'RLCraft v2.9.3', minecraftVersion: '1.12.2', loaderType: 'Forge', loaderVersion: '14.23.5', modCount: 165, worldCount: 0, lastPlayedMs: Date.now() - 172800000, sourceLauncher: 'curse_forge' },
    ],
  },
  {
    launcherType: 'prism',
    launcherPath: 'C:\\Users\\Player\\AppData\\Roaming\\PrismLauncher\\instances',
    instances: [
      { id: 'pr-1', name: 'Fabulously Optimized', minecraftVersion: '1.20.4', loaderType: 'Fabric', loaderVersion: '0.15.11', modCount: 52, worldCount: 1, lastPlayedMs: Date.now() - 7200000, sourceLauncher: 'prism' },
      { id: 'pr-2', name: 'Vanilla Plus 1.20.1', minecraftVersion: '1.20.1', loaderType: 'Fabric', loaderVersion: '0.15.11', modCount: 38, worldCount: 3, lastPlayedMs: Date.now() - 259200000, sourceLauncher: 'prism' },
    ],
  },
  {
    launcherType: 'modrinth',
    launcherPath: 'C:\\Users\\Player\\AppData\\Roaming\\ModrinthApp\\profiles',
    instances: [
      { id: 'mr-1', name: 'Simply Optimized', minecraftVersion: '1.20.4', loaderType: 'Fabric', loaderVersion: '0.15.11', modCount: 41, worldCount: 1, lastPlayedMs: Date.now() - 400000000, sourceLauncher: 'modrinth' },
    ],
  },
];

const MOCK_GAMEPADS = [
  { id: 0, name: 'Xbox Wireless Controller', connected: true, battery: 'High' },
  { id: 1, name: 'DualSense Wireless Controller', connected: true, battery: '80%' },
  { id: 2, name: 'Nintendo Switch Pro Controller', connected: true, battery: 'Full' },
  { id: 3, name: 'Generic X-Input Gamepad', connected: true, battery: 'Medium' },
];

export async function handleMockInvoke(cmd, args = {}) {
  // Console trace for debugging mock coverage
  // console.debug(`[MockApi] invoke: ${cmd}`, args);

  switch (cmd) {
    case 'get_cached_session':
    case 'get_session':
    case 'login_microsoft':
      return MOCK_SESSION;

    case 'list_accounts':
      return [{ ...MOCK_SESSION, active: true }];

    case 'list_offline_instances':
      return MOCK_INSTANCES;

    case 'list_offline_mods':
      return MOCK_MODS;

    case 'load_saved_servers':
      return MOCK_SERVERS;

    case 'server_status': {
      const serverStatusMap = {
        'play.beehivesmp.net': {
          ready: true,
          running: true,
          online: 34,
          max: 75,
          pingMs: 24,
          version: '1.20.1',
          motd: '§6✦ Beehive SMP §7· §fCommunity Adventure & Honey',
          bannerUrl: '/mock_assets/beehive_bg.png',
          iconUrl: '/mock_assets/beehive_icon.png',
        },
        'play.zirconmc.net': {
          ready: true,
          running: true,
          online: 34,
          max: 75,
          pingMs: 24,
          version: '1.21.1',
          motd: '§6✦ ZirconSMP §7· §fCogs & Cottage Adventure',
          bannerUrl: '/mock_assets/beehive_bg.png',
          iconUrl: '/mock_assets/beehive_icon.png',
        },
        'emerald.zirconmc.net': {
          ready: true,
          running: true,
          online: 52,
          max: 100,
          pingMs: 18,
          version: '1.20.1',
          motd: '§a✦ Emerald SMP §7· §fAuto-Synced Modpack §e[1.20.1]',
          bannerUrl: '/mock_assets/emerald_bg.png',
          iconUrl: '/mock_assets/emerald_icon.png',
        },
        'flowercharm.zirconmc.net': {
          ready: true,
          running: true,
          online: 28,
          max: 60,
          pingMs: 31,
          version: '1.20.1',
          motd: '§d✿ Flower-Charm §7· §fWholesome Modded SMP §e[1.20.1]',
          bannerUrl: '/mock_assets/flower_charm_bg.jpg',
          iconUrl: '/mock_assets/flower_charm_icon.png',
        },
      };
      return serverStatusMap[args.address] || {
        ready: true,
        running: true,
        online: 24,
        max: 64,
        pingMs: 28,
        version: '1.20.1',
      };
    }

    case 'get_discovered_lan_servers':
      return [
        {
          id: 'lan-1',
          name: "Alex's Cobblemon Adventure",
          hostUsername: 'Alex',
          hostAddress: '192.168.1.142',
          address: '192.168.1.142:25565',
          isZirconEnhanced: true,
          mcVersion: '1.20.1',
          loaderType: 'Fabric',
          joinCode: '8942',
        },
        {
          id: 'lan-2',
          name: "Sarah's Survival World",
          hostUsername: 'Sarah',
          hostAddress: '192.168.1.108',
          address: '192.168.1.108:51820',
          isZirconEnhanced: true,
          mcVersion: '1.21',
          loaderType: 'NeoForge',
          joinCode: '4190',
        },
      ];

    case 'scan_installed_launchers':
      return MOCK_MIGRATION;

    case 'list_keybind_profiles':
      return [
        {
          id: 'fps_competitive',
          name: 'FPS Competitive',
          description: 'Optimized sprint on Shift, crouch on Ctrl, zoom on C',
          count: 32,
          updated_at: Math.floor(Date.now() / 1000),
        },
        {
          id: 'standard_modded',
          name: 'Standard Modded Layout',
          description: 'Default WASD with REI recipe lookup on R',
          count: 45,
          updated_at: Math.floor(Date.now() / 1000) - 86400,
        },
      ];

    case 'get_keybind_profile':
      return {
        id: args.id || 'fps_competitive',
        name: 'FPS Competitive',
        description: 'Optimized sprint on Shift, crouch on Ctrl, zoom on C',
        created_at: Math.floor(Date.now() / 1000) - 100000,
        updated_at: Math.floor(Date.now() / 1000),
        keybinds: {
          'key_key.forward': 'key.keyboard.w',
          'key_key.left': 'key.keyboard.a',
          'key_key.back': 'key.keyboard.s',
          'key_key.right': 'key.keyboard.d',
          'key_key.jump': 'key.keyboard.space',
          'key_key.sprint': 'key.keyboard.left.shift',
          'key_key.sneak': 'key.keyboard.left.control',
          'key_key.attack': 'key.mouse.left',
          'key_key.use': 'key.mouse.right',
          'key_key.inventory': 'key.keyboard.e',
          'key_key.drop': 'key.keyboard.q',
          'key_key.chat': 'key.keyboard.t',
        },
      };

    case 'save_keybind_profile':
      return { ...args.profile, updated_at: Math.floor(Date.now() / 1000) };

    case 'delete_keybind_profile':
      return null;

    case 'extract_instance_keybinds':
    case 'extract_options_file_keybinds':
      return {
        'key_key.forward': 'key.keyboard.w',
        'key_key.left': 'key.keyboard.a',
        'key_key.back': 'key.keyboard.s',
        'key_key.right': 'key.keyboard.d',
        'key_key.jump': 'key.keyboard.space',
        'key_key.sprint': 'key.keyboard.left.control',
        'key_key.sneak': 'key.keyboard.left.shift',
        'key_key.attack': 'key.mouse.left',
        'key_key.use': 'key.mouse.right',
        'key_key.inventory': 'key.keyboard.e',
      };

    case 'apply_keybind_profile_to_instance':
      return {
        profile_id: args.profileId,
        keys_applied: 10,
        keys_updated: 8,
        keys_added: 2,
        backup_path: 'options.txt.bak',
      };

    case 'get_game_status':
      return { running: false, label: '' };

    case 'list_instance_worlds':
      return [
        { name: 'Survival Base - Day 240', folder: 'world_survival', lastPlayed: Date.now() - 7200000, sizeBytes: 154000000 },
        { name: 'Redstone Testing Lab', folder: 'world_redstone', lastPlayed: Date.now() - 86400000, sizeBytes: 42000000 },
      ];

    case 'list_instance_screenshots':
      return [];

    case 'get_instance_artwork': {
      if (args.instanceId === 'flower-charm') {
        return {
          bannerDataUrl: null,
          iconDataUrl: '/mock_assets/flower_charm_icon.png',
          panoramaDataUrls: [],
        };
      }
      return { bannerDataUrl: null, iconDataUrl: null, panoramaDataUrls: [] };
    }

    case 'get_cloud_sync_status':
      return { connected: false, lastSync: null };

    case 'get_gamepad_assignments':
    case 'list_connected_gamepads':
      return MOCK_GAMEPADS;

    case 'get_server_instance_mods':
      return {
        serverAddress: args.address || 'play.zirconmc.net',
        gameDir: 'emerald-survival',
        minecraftVersion: '1.20.1',
        loaderType: 'Fabric',
        loaderVersion: '0.15.11',
        mods: [
          {
            filename: 'sodium-fabric-0.5.8+mc1.20.1.jar',
            name: 'Sodium',
            version: '0.5.8',
            author: 'jellysquid',
            sizeBytes: 1240000,
            enabled: true,
            iconUrl: 'https://cdn.modrinth.com/data/AANobbMI/icon.png',
            isBom: true,
            isCustom: false,
            isDownloaded: true,
            description: 'Modern rendering engine for Minecraft that greatly improves frame rates and reduces stutter.',
          },
          {
            filename: 'iris-1.7.0+mc1.20.1.jar',
            name: 'Iris Shaders',
            version: '1.7.0',
            author: 'coderbot',
            sizeBytes: 2450000,
            enabled: true,
            iconUrl: 'https://cdn.modrinth.com/data/YL57xq9U/icon.png',
            isBom: true,
            isCustom: false,
            isDownloaded: true,
            description: 'A modern shaders mod for Minecraft compatible with existing OptiFine shader packs.',
          },
          {
            filename: 'appleskin-fabric-mc1.20.1-2.5.1.jar',
            name: 'AppleSkin',
            version: '2.5.1',
            author: 'squeek502',
            sizeBytes: 154000,
            enabled: true,
            iconUrl: 'https://cdn.modrinth.com/data/EsAfCjCV/icon.png',
            isBom: false,
            isCustom: true,
            isDownloaded: true,
            description: 'Adds various food and hunger related tooltips and HUD enhancements.',
          },
        ],
        hasBom: true,
      };

    case 'list_instance_packs':
      return {
        shaderpacks: ['ComplementaryReimagined_r5.1.1.zip', 'BSL_v8.2.09.zip'],
        resourcepacks: ['Faithful32x-1.20.zip', 'StayTrue-1.20.zip'],
        activeShaderpack: 'ComplementaryReimagined_r5.1.1.zip',
        activeResourcepacks: ['Faithful32x-1.20.zip'],
        shadersEnabled: true,
        locallyAddedShaderpacks: ['ComplementaryReimagined_r5.1.1.zip', 'BSL_v8.2.09.zip'],
        locallyAddedResourcepacks: ['Faithful32x-1.20.zip', 'StayTrue-1.20.zip'],
      };

    case 'list_instance_packs_detailed':
      return {
        shaderpacks: [
          {
            filename: 'ComplementaryReimagined_r5.1.1.zip',
            title: 'Complementary Reimagined',
            author: 'EminGT',
            description: 'Exceptional visual quality and performance shaderpack.',
            iconUrl: 'https://cdn.modrinth.com/data/R2Fr3oaQ/icon.png',
            isActive: true,
            isLocal: true,
          },
          {
            filename: 'BSL_v8.2.09.zip',
            title: 'BSL Shaders',
            author: 'Capt Tatsu',
            description: 'Bright and colorful shaderpack with warm lighting.',
            iconUrl: 'https://cdn.modrinth.com/data/m72qlz7e/icon.png',
            isActive: false,
            isLocal: true,
          },
        ],
        resourcepacks: [
          {
            filename: 'Faithful32x-1.20.zip',
            title: 'Faithful 32x',
            author: 'Faithful Team',
            description: 'High-resolution textures that stay true to original Minecraft.',
            iconUrl: 'https://cdn.modrinth.com/data/1Kdd7AKh/icon.png',
            isActive: true,
            isLocal: true,
          },
          {
            filename: 'StayTrue-1.20.zip',
            title: 'Stay True',
            author: 'Trxvz',
            description: 'Subtle design tweaks to default textures without straying too far.',
            iconUrl: 'https://cdn.modrinth.com/data/u5bxb4Pq/icon.png',
            isActive: false,
            isLocal: true,
          },
        ],
        shadersEnabled: true,
      };

    case 'search_mods': {
      const type = args.projectType || 'mod';
      if (type === 'shader' || type === 'shaderpack') {
        return [
          {
            id: 'complementary-reimagined',
            projectId: 'R2Fr3oaQ',
            title: 'Complementary Reimagined',
            name: 'Complementary Reimagined',
            slug: 'complementary-reimagined',
            description: 'Top-tier shaders designed for realism and aesthetic perfection.',
            summary: 'Top-tier shaders designed for realism and aesthetic perfection.',
            author: 'EminGT',
            iconUrl: 'https://cdn.modrinth.com/data/R2Fr3oaQ/icon.png',
            downloads: 15420000,
            downloadCount: 15420000,
            projectUrl: 'https://modrinth.com/shader/complementary-reimagined',
            websiteUrl: 'https://modrinth.com/shader/complementary-reimagined',
            origin: args.origin || args.provider || 'modrinth',
            loaders: ['iris', 'optifine'],
          },
        ];
      }
      if (type === 'resourcepack' || type === 'resource') {
        return [
          {
            id: 'faithful-32x',
            projectId: '1Kdd7AKh',
            title: 'Faithful 32x',
            name: 'Faithful 32x',
            slug: 'faithful-32x',
            description: 'The definitive high-definition vanilla texture pack.',
            summary: 'The definitive high-definition vanilla texture pack.',
            author: 'Faithful Team',
            iconUrl: 'https://cdn.modrinth.com/data/1Kdd7AKh/icon.png',
            downloads: 8750000,
            downloadCount: 8750000,
            projectUrl: 'https://modrinth.com/resourcepack/faithful-32x',
            websiteUrl: 'https://modrinth.com/resourcepack/faithful-32x',
            origin: args.origin || args.provider || 'modrinth',
            loaders: ['minecraft'],
          },
        ];
      }
      return [
        {
          id: 'fabric-api',
          projectId: 'P7dR8mSH',
          title: 'Fabric API',
          name: 'Fabric API',
          slug: 'fabric-api',
          description: 'Essential hooks and inter-mod communication library for Fabric.',
          summary: 'Essential hooks and inter-mod communication library for Fabric.',
          author: 'modmuss50',
          iconUrl: 'https://cdn.modrinth.com/data/P7dR8mSH/icon.png',
          downloads: 45000000,
          downloadCount: 45000000,
          projectUrl: 'https://modrinth.com/mod/fabric-api',
          websiteUrl: 'https://modrinth.com/mod/fabric-api',
          origin: args.origin || args.provider || 'modrinth',
          loaders: ['fabric', 'quilt'],
        },
      ];
    }

    case 'list_mod_versions':
      return [
        {
          id: 'ver-1',
          name: '1.20.1-v1.0.0',
          versionNumber: '1.0.0',
          fileName: 'pack-1.0.0.zip',
          downloadUrl: 'https://example.com/pack.zip',
          fileSize: 4500000,
        },
      ];

    case 'install_modrinth_pack':
    case 'install_server_modrinth_mod':
      return 'installed_pack.zip';

    case 'set_server_mod_enabled':
    case 'delete_server_mod':
    case 'set_active_shaderpack':
    case 'set_active_resourcepacks':
    case 'set_server_pack_decisions':
      return true;

    case 'get_server_pack_decisions':
      return { accepted: ['Faithful32x-1.20.zip'], rejected: [] };

    case 'import_instance_pack':
    case 'import_instance_pack_bytes':
      return args.filename || 'imported_pack.zip';

    default:
      return null;
  }
}
