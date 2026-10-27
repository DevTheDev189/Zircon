// Zircon Server Manager - Lightweight 40-Language Internationalization Runtime
(function (window) {
  'use strict';

  const STORAGE_KEY = 'zircon_server_locale';

  const SUPPORTED_LOCALES = [
    { code: 'en', name: 'English' },
    { code: 'es', name: 'Español' },
    { code: 'de', name: 'Deutsch' },
    { code: 'fr', name: 'Français' },
    { code: 'ja', name: '日本語' },
    { code: 'zh-CN', name: '简体中文' },
    { code: 'zh-TW', name: '繁體中文' },
    { code: 'ko', name: '한국어' },
    { code: 'pt-BR', name: 'Português (Brasil)' },
    { code: 'pt-PT', name: 'Português (Portugal)' },
    { code: 'it', name: 'Italiano' },
    { code: 'ru', name: 'Русский' },
    { code: 'uk', name: 'Українська' },
    { code: 'pl', name: 'Polski' },
    { code: 'nl', name: 'Nederlands' },
    { code: 'tr', name: 'Türkçe' },
    { code: 'sv', name: 'Svenska' },
    { code: 'no', name: 'Norsk' },
    { code: 'da', name: 'Dansk' },
    { code: 'fi', name: 'Suomi' },
    { code: 'cs', name: 'Čeština' },
    { code: 'sk', name: 'Slovenčina' },
    { code: 'hu', name: 'Magyar' },
    { code: 'ro', name: 'Română' },
    { code: 'bg', name: 'Български' },
    { code: 'el', name: 'Ελληνικά' },
    { code: 'sr', name: 'Српски' },
    { code: 'hr', name: 'Hrvatski' },
    { code: 'sl', name: 'Slovenščina' },
    { code: 'et', name: 'Eesti' },
    { code: 'lv', name: 'Latviešu' },
    { code: 'lt', name: 'Lietuvių' },
    { code: 'vi', name: 'Tiếng Việt' },
    { code: 'th', name: 'ไทย' },
    { code: 'id', name: 'Bahasa Indonesia' },
    { code: 'ms', name: 'Bahasa Melayu' },
    { code: 'fil', name: 'Filipino' },
    { code: 'hi', name: 'हिन्दी' },
    { code: 'ar', name: 'العربية' },
    { code: 'he', name: 'עברית' },
    { code: 'fa', name: 'فارسی' },
  ];

  const defaultMessages = {
    common: {
      save: "Save",
      saving: "Saving…",
      cancel: "Cancel",
      close: "Close",
      refresh: "Refresh",
      copy: "Copy",
      copied: "Copied to clipboard!",
      delete: "Delete",
      loading: "Loading…",
      apply: "Apply",
      edit: "Edit",
      search: "Search…",
      online: "Online",
      offline: "Offline",
      sleeping: "Sleeping",
      booting: "Booting…",
      falling_asleep: "Falling asleep…",
      shutting_down: "Shutting down…",
      start: "Start",
      stop: "Stop",
      restart: "Restart",
      wake: "Wake Server",
      detecting: "Detecting…"
    },
    nav: {
      global_stats: "Global Stats",
      servers: "Servers",
      create_server: "Create Server",
      import_server: "Import Server (.zip)",
      mods: "Mods",
      shaders: "Shaders",
      files: "Files",
      console: "Console",
      players: "Players",
      backups: "Backups",
      world: "World",
      settings: "Settings",
      disk_storage: "Disk Storage",
      server_manager: "Server Manager"
    },
    stats: {
      title: "System Telemetry & Metrics",
      subtitle: "Real-time CPU, Host RAM, Process RSS, and Disk activity",
      host_ram: "Host OS RAM",
      process_rss: "Java Process RSS",
      cpu_usage: "CPU Load",
      disk_allocation: "NVMe Storage Allocation",
      uptime: "System Uptime",
      active_instances: "Running Instances",
      sleeping_instances: "Sleeping Instances"
    },
    instances: {
      no_servers: "No servers yet — click Create Server.",
      create_title: "Create Minecraft Server",
      name_label: "Instance Name",
      version_label: "Minecraft Version",
      loader_label: "Mod Loader",
      port_label: "Port",
      memory_label: "Allocated Memory (GB)",
      start_server: "Start Server",
      wake_server: "Wake Server",
      stop_server: "Stop Server",
      restart_server: "Restart Server",
      players_online: "{count} online",
      sleeps_in: "Sleeps in {time}",
      idle_wakes: "Idle shutdown — wakes on player join"
    },
    mods: {
      title: "Installed Mods",
      count: "Installed Mods ({count})",
      search_placeholder: "Search installed mods…",
      loader_filter: "Loader Filter",
      all_loaders: "All Loaders",
      active_loader: "Active Loader ({loader})",
      export_mrpack: "Export .mrpack",
      copy_mrpack_url: "Copy .mrpack URL",
      copy_mrpack_tooltip: "Copy public .mrpack download link for non-Zircon players",
      add_mod: "Add Mod",
      install_update: "Update",
      no_mods: "No mods installed yet."
    },
    console: {
      title: "Server Console & Terminal",
      placeholder: "Send a command (e.g. /say Hello, /op Player)...",
      send: "Send",
      clear: "Clear"
    },
    players: {
      title: "Player Management",
      online: "Online Players",
      whitelist: "Whitelist",
      ops: "Operators",
      bans: "Banned Players",
      kick: "Kick",
      ban: "Ban",
      op: "Make Op",
      deop: "Remove Op"
    },
    backups: {
      title: "World & Instance Snapshots",
      create: "Create Backup",
      restore: "Restore",
      download: "Download",
      delete: "Delete"
    },
    security: {
      online_mode: "Online Mode (Mojang Authentication)",
      online_mode_warning: "Warning: Disabling online-mode allows offline/unverified players with spoofed UUIDs to connect.",
      whitelist_enforced: "Enforce Whitelist"
    }
  };

  let activeLocale = 'en';
  let activeMessages = defaultMessages;

  function resolvePath(obj, path) {
    if (!obj || !path) return undefined;
    const parts = path.split('.');
    let cur = obj;
    for (const p of parts) {
      if (cur == null || typeof cur !== 'object') return undefined;
      cur = cur[p];
    }
    return cur;
  }

  function t(key, params) {
    let text = resolvePath(activeMessages, key) ?? resolvePath(defaultMessages, key) ?? key;
    if (typeof text === 'string' && params && typeof params === 'object') {
      for (const [k, v] of Object.entries(params)) {
        text = text.replaceAll(`{${k}}`, v);
      }
    }
    return text;
  }

  async function loadLocale(localeCode) {
    if (localeCode === 'en') {
      activeMessages = defaultMessages;
      activeLocale = 'en';
      return;
    }
    try {
      const res = await fetch(`locales/${localeCode}.json`);
      if (res.ok) {
        activeMessages = await res.json();
        activeLocale = localeCode;
      } else {
        console.warn(`Locale ${localeCode} not found, falling back to English`);
        activeMessages = defaultMessages;
        activeLocale = 'en';
      }
    } catch (err) {
      console.warn(`Failed to fetch locale ${localeCode}:`, err);
      activeMessages = defaultMessages;
      activeLocale = 'en';
    }
  }

  function setLocale(localeCode) {
    localStorage.setItem(STORAGE_KEY, localeCode);
    loadLocale(localeCode).then(() => {
      // Trigger Vue re-render if available
      if (window.__zircon_app && typeof window.__zircon_app.$forceUpdate === 'function') {
        window.__zircon_app.$forceUpdate();
      } else if (window.location) {
        window.location.reload();
      }
    });
  }

  function detectBrowserLocale() {
    if (typeof navigator === 'undefined') return 'en';
    const navLangs = navigator.languages || [navigator.language || navigator.userLanguage];
    if (!navLangs || !navLangs.length) return 'en';

    for (const raw of navLangs) {
      if (!raw) continue;
      const clean = raw.trim();
      const exact = SUPPORTED_LOCALES.find(l => l.code.toLowerCase() === clean.toLowerCase());
      if (exact) return exact.code;

      const prefix = clean.split('-')[0].toLowerCase();
      const prefixMatch = SUPPORTED_LOCALES.find(l => l.code.toLowerCase() === prefix);
      if (prefixMatch) return prefixMatch.code;
    }
    return 'en';
  }

  function initI18n() {
    try {
      const saved = localStorage.getItem(STORAGE_KEY);
      if (saved && SUPPORTED_LOCALES.some(l => l.code === saved)) {
        loadLocale(saved);
      } else {
        const detected = detectBrowserLocale();
        if (detected && detected !== 'en') {
          loadLocale(detected);
        }
      }
    } catch {
      // localStorage disabled or security sandbox
    }
  }

  initI18n();

  // Export to window
  window.i18n = {
    t,
    setLocale,
    loadLocale,
    getLocale: () => activeLocale,
    getSupportedLocales: () => SUPPORTED_LOCALES,
  };
  window.t = t;
  window.SUPPORTED_LOCALES = SUPPORTED_LOCALES;
})(window);
