import { ref, shallowReactive, computed } from 'vue';
import en from '../locales/en.json';

// Lazy loader map for all compiled language JSON files (ignoring English which is statically bundled and hidden cache files)
const localeLoaders = import.meta.glob(['../locales/[a-zA-Z]*.json', '!../locales/en.json']);

// Reactive dictionary of loaded locales, pre-seeded with English
export const locales = shallowReactive({
  en
});

// Helper to asynchronously load a locale
export async function loadLocale(code) {
  if (!code || locales[code]) return locales[code];
  const loader = localeLoaders[`../locales/${code}.json`];
  if (!loader) return null;
  try {
    const mod = await loader();
    const data = mod.default || mod;
    locales[code] = data;
    return data;
  } catch (err) {
    console.warn(`Failed to lazy load locale '${code}':`, err);
    return null;
  }
}

// Comprehensive metadata catalog for all supported languages
export const ALL_LOCALE_METADATA = {
  en: { name: 'English', native: 'English', flag: '🇺🇸' },
  es: { name: 'Spanish', native: 'Español', flag: '🇪🇸' },
  'pt-BR': { name: 'Portuguese (Brazil)', native: 'Português (Brasil)', flag: '🇧🇷' },
  'pt-PT': { name: 'Portuguese (Portugal)', native: 'Português (Portugal)', flag: '🇵🇹' },
  de: { name: 'German', native: 'Deutsch', flag: '🇩🇪' },
  fr: { name: 'French', native: 'Français', flag: '🇫🇷' },
  it: { name: 'Italian', native: 'Italiano', flag: '🇮🇹' },
  ru: { name: 'Russian', native: 'Русский', flag: '🇷🇺' },
  pl: { name: 'Polish', native: 'Polski', flag: '🇵🇱' },
  nl: { name: 'Dutch', native: 'Nederlands', flag: '🇳🇱' },
  tr: { name: 'Turkish', native: 'Türkçe', flag: '🇹🇷' },
  sv: { name: 'Swedish', native: 'Svenska', flag: '🇸🇪' },
  uk: { name: 'Ukrainian', native: 'Українська', flag: '🇺🇦' },
  cs: { name: 'Czech', native: 'Čeština', flag: '🇨🇿' },
  fi: { name: 'Finnish', native: 'Suomi', flag: '🇫🇮' },
  da: { name: 'Danish', native: 'Dansk', flag: '🇩🇰' },
  no: { name: 'Norwegian', native: 'Norsk Bokmål', flag: '🇳🇴' },
  hu: { name: 'Hungarian', native: 'Magyar', flag: '🇭🇺' },
  ro: { name: 'Romanian', native: 'Română', flag: '🇷🇴' },
  el: { name: 'Greek', native: 'Ελληνικά', flag: '🇬🇷' },
  bg: { name: 'Bulgarian', native: 'Български', flag: '🇧🇬' },
  sk: { name: 'Slovak', native: 'Slovenčina', flag: '🇸🇰' },
  hr: { name: 'Croatian', native: 'Hrvatski', flag: '🇭🇷' },
  sr: { name: 'Serbian', native: 'Српски', flag: '🇷🇸' },
  sl: { name: 'Slovenian', native: 'Slovenščina', flag: '🇸🇮' },
  lt: { name: 'Lithuanian', native: 'Lietuvių', flag: '🇱🇹' },
  lv: { name: 'Latvian', native: 'Latviešu', flag: '🇱🇻' },
  et: { name: 'Estonian', native: 'Eesti', flag: '🇪🇪' },
  'zh-CN': { name: 'Simplified Chinese', native: '简体中文', flag: '🇨🇳' },
  'zh-TW': { name: 'Traditional Chinese', native: '繁體中文', flag: '🇹🇼' },
  ja: { name: 'Japanese', native: '日本語', flag: '🇯🇵' },
  ko: { name: 'Korean', native: '한국어', flag: '🇰🇷' },
  id: { name: 'Indonesian', native: 'Bahasa Indonesia', flag: '🇮🇩' },
  vi: { name: 'Vietnamese', native: 'Tiếng Việt', flag: '🇻🇳' },
  th: { name: 'Thai', native: 'ไทย', flag: '🇹🇭' },
  ms: { name: 'Malay', native: 'Bahasa Melayu', flag: '🇲🇾' },
  fil: { name: 'Filipino', native: 'Filipino', flag: '🇵🇭' },
  hi: { name: 'Hindi', native: 'हिन्दी', flag: '🇮🇳' },
  ar: { name: 'Arabic', native: 'العربية', flag: '🇸🇦' },
  he: { name: 'Hebrew', native: 'עברית', flag: '🇮🇱' },
  fa: { name: 'Persian', native: 'فارسی', flag: '🇮🇷' }
};

// Available Locales Catalog computed from full metadata catalog
export const AVAILABLE_LOCALES = computed(() => {
  return Object.keys(ALL_LOCALE_METADATA).map(code => ({
    code,
    ...ALL_LOCALE_METADATA[code]
  }));
});

const RTL_LOCALES = new Set(['ar', 'he', 'fa']);

/**
 * Detects the best matching locale from the operating system / browser preferences.
 */
function detectSystemLocale(supportedLocales) {
  if (typeof navigator === 'undefined') return 'en';

  const userLanguages = navigator.languages && navigator.languages.length
    ? navigator.languages
    : [navigator.language || navigator.userLanguage];

  for (const lang of userLanguages) {
    if (!lang) continue;
    const cleanLang = lang.trim();

    // 1. Direct exact match (e.g. 'pt-BR', 'zh-CN', 'es')
    if (supportedLocales[cleanLang]) {
      return cleanLang;
    }

    // 2. Case-insensitive match (e.g. 'pt-br' -> 'pt-BR')
    const caseMatch = Object.keys(supportedLocales).find(
      code => code.toLowerCase() === cleanLang.toLowerCase()
    );
    if (caseMatch) {
      return caseMatch;
    }

    // 3. Script normalization (e.g. 'zh-Hans' -> 'zh-CN', 'zh-Hant' -> 'zh-TW')
    if (cleanLang.startsWith('zh-Hans') && supportedLocales['zh-CN']) return 'zh-CN';
    if (cleanLang.startsWith('zh-Hant') && supportedLocales['zh-TW']) return 'zh-TW';

    // 4. Primary language prefix match (e.g. 'es-MX' -> 'es', 'de-AT' -> 'de', 'fr-FR' -> 'fr')
    const primary = cleanLang.split(/[-_]/)[0];
    if (supportedLocales[primary]) {
      return primary;
    }

    // 5. Default dialect variants
    if (primary === 'pt' && supportedLocales['pt-BR']) return 'pt-BR';
    if (primary === 'zh' && supportedLocales['zh-CN']) return 'zh-CN';
  }

  return 'en';
}

function getInitialLocale() {
  if (typeof window !== 'undefined' && typeof URLSearchParams !== 'undefined') {
    const params = new URLSearchParams(window.location.search);
    const queryLang = params.get('lang');
    if (queryLang && (ALL_LOCALE_METADATA[queryLang] || queryLang === 'en')) {
      return queryLang;
    }
  }
  if (typeof localStorage !== 'undefined') {
    const saved = localStorage.getItem('zircon_locale');
    if (saved && (ALL_LOCALE_METADATA[saved] || saved === 'en')) {
      return saved;
    }
  }
  return detectSystemLocale(ALL_LOCALE_METADATA);
}

// Reactive State
const initialCode = getInitialLocale();
export const currentLocale = ref(initialCode);
if (initialCode !== 'en') {
  loadLocale(initialCode);
}

// Apply initial document direction & lang
if (typeof document !== 'undefined') {
  document.documentElement.lang = initialCode;
  document.documentElement.dir = RTL_LOCALES.has(initialCode) ? 'rtl' : 'ltr';
}

function resolveKey(obj, path) {
  if (!obj || !path) return undefined;
  const segments = path.split('.');
  let current = obj;
  for (const seg of segments) {
    if (current == null || typeof current !== 'object') return undefined;
    current = current[seg];
  }
  return current;
}

/**
 * Translate a key with optional variable interpolation
 * Usage in template: t('settings.ram.label') or t('settings.ram.system_ram', { total: 16, recommended: 6 })
 */
export function t(key, vars = {}) {
  const activeObj = locales[currentLocale.value];
  const englishObj = locales['en'];

  let str = resolveKey(activeObj, key);
  if (str === undefined) {
    str = resolveKey(englishObj, key);
  }
  if (str === undefined) {
    return key; // Graceful fallback: return raw path
  }

  if (typeof str !== 'string') return str;

  // Interpolate {param}
  if (vars && Object.keys(vars).length > 0) {
    return str.replace(/\{([\w.]+)\}/g, (match, param) => {
      return vars[param] !== undefined ? String(vars[param]) : match;
    });
  }

  return str;
}

/**
 * Switch active locale dynamically
 */
export async function setLocale(code) {
  if (!code) return;
  if (!locales[code] && code !== 'en') {
    await loadLocale(code);
  }
  currentLocale.value = code;
  if (typeof localStorage !== 'undefined') {
    localStorage.setItem('zircon_locale', code);
  }
  if (typeof document !== 'undefined') {
    document.documentElement.lang = code;
    document.documentElement.dir = RTL_LOCALES.has(code) ? 'rtl' : 'ltr';
  }
}

/**
 * Register dynamic bundle (used for streaming or AI compiled locales)
 */
export function registerLocale(code, bundle) {
  locales[code] = bundle;
}

// Vue Plugin for installation in main.js
export const i18nPlugin = {
  install(app) {
    app.config.globalProperties.$t = t;
    app.provide('i18n', {
      t,
      currentLocale,
      setLocale,
      availableLocales: AVAILABLE_LOCALES
    });
  }
};
