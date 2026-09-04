// UI Theme presets and dynamic CSS custom property management.

export const THEME_PRESETS = [
  {
    id: 'zircon-cyan',
    name: 'Zircon Cyan',
    description: 'Signature crystalline cyan glow',
    hex: '#47d2c9',
    rgb: '71 210 201',
    bright: '#5adfd5',
    brightRgb: '90 223 213',
    deep: '#20b2aa',
    deepRgb: '32 178 170',
    ink: '#022623',
    inkRgb: '2 38 35',
    glow: 'rgba(71, 210, 201, 0.35)',
    glowStrong: 'rgba(71, 210, 201, 0.5)',
  },
  {
    id: 'amethyst-purple',
    name: 'Amethyst Purple',
    description: 'Vibrant crystalline geode purple',
    hex: '#a855f7',
    rgb: '168 85 247',
    bright: '#c084fc',
    brightRgb: '192 132 252',
    deep: '#9333ea',
    deepRgb: '147 51 234',
    ink: '#230738',
    inkRgb: '35 7 56',
    glow: 'rgba(168, 85, 247, 0.35)',
    glowStrong: 'rgba(168, 85, 247, 0.5)',
  },
  {
    id: 'emerald-green',
    name: 'Emerald Green',
    description: 'Lush villager emerald shimmer',
    hex: '#22c55e',
    rgb: '34 197 94',
    bright: '#4ade80',
    brightRgb: '74 222 128',
    deep: '#16a34a',
    deepRgb: '22 163 74',
    ink: '#03240e',
    inkRgb: '3 36 14',
    glow: 'rgba(34, 197, 94, 0.35)',
    glowStrong: 'rgba(34, 197, 94, 0.5)',
  },
  {
    id: 'redstone-crimson',
    name: 'Redstone Crimson',
    description: 'High-voltage redstone pulse',
    hex: '#ef4444',
    rgb: '239 68 68',
    bright: '#f87171',
    brightRgb: '248 113 113',
    deep: '#dc2626',
    deepRgb: '220 38 38',
    ink: '#2c0707',
    inkRgb: '44 7 7',
    glow: 'rgba(239, 68, 68, 0.35)',
    glowStrong: 'rgba(239, 68, 68, 0.5)',
  },
  {
    id: 'blaze-amber',
    name: 'Blaze Amber',
    description: 'Fiery Nether fortress incandescent glow',
    hex: '#f59e0b',
    rgb: '245 158 11',
    bright: '#fbbf24',
    brightRgb: '251 191 36',
    deep: '#d97706',
    deepRgb: '217 119 6',
    ink: '#2d1702',
    inkRgb: '45 23 2',
    glow: 'rgba(245, 158, 11, 0.35)',
    glowStrong: 'rgba(245, 158, 11, 0.5)',
  },
  {
    id: 'diamond-blue',
    name: 'Diamond Blue',
    description: 'Deep cavern diamond ore glow',
    hex: '#38bdf8',
    rgb: '56 189 248',
    bright: '#7dd3fc',
    brightRgb: '125 211 252',
    deep: '#0284c7',
    deepRgb: '2 132 199',
    ink: '#031f30',
    inkRgb: '3 31 48',
    glow: 'rgba(56, 189, 248, 0.35)',
    glowStrong: 'rgba(56, 189, 248, 0.5)',
  },
  {
    id: 'obsidian-slate',
    name: 'Obsidian Slate',
    description: 'Monochrome stealth carbon',
    hex: '#94a3b8',
    rgb: '148 163 184',
    bright: '#cbd5e1',
    brightRgb: '203 213 225',
    deep: '#64748b',
    deepRgb: '100 116 139',
    ink: '#0f172a',
    inkRgb: '15 23 42',
    glow: 'rgba(148, 163, 184, 0.35)',
    glowStrong: 'rgba(148, 163, 184, 0.5)',
  },
];

export const BG_THEME_PRESETS = [
  {
    id: 'deep-void',
    name: 'Deep Void',
    description: 'Signature Zircon cosmic dark space',
    bg: '#070b0f',
    bgRgb: '7 11 15',
    sidebar: '#0a0f14',
    sidebarRgb: '10 15 20',
    card: '#0e1622',
    cardRgb: '14 22 34',
    well: '#070b10',
    wellRgb: '7 11 16',
    border: '#263545',
    borderRgb: '38 53 69',
    btnSecondary: '#1c2530',
    btnSecondaryRgb: '28 37 48',
  },
  {
    id: 'oled-black',
    name: 'OLED Pure Black',
    description: 'Pitch black with supreme pixel contrast',
    bg: '#000000',
    bgRgb: '0 0 0',
    sidebar: '#050505',
    sidebarRgb: '5 5 5',
    card: '#0d0d0d',
    cardRgb: '13 13 13',
    well: '#050505',
    wellRgb: '5 5 5',
    border: '#222222',
    borderRgb: '34 34 34',
    btnSecondary: '#141414',
    btnSecondaryRgb: '20 20 20',
  },
  {
    id: 'abyssal-navy',
    name: 'Abyssal Navy',
    description: 'Deep maritime twilight oceanic depths',
    bg: '#050a14',
    bgRgb: '5 10 20',
    sidebar: '#081020',
    sidebarRgb: '8 16 32',
    card: '#0c1830',
    cardRgb: '12 24 48',
    well: '#060d1a',
    wellRgb: '6 13 26',
    border: '#1d3050',
    borderRgb: '29 48 80',
    btnSecondary: '#13223d',
    btnSecondaryRgb: '19 34 61',
  },
  {
    id: 'carbon-slate',
    name: 'Carbon Slate',
    description: 'Modern stealth graphite slate',
    bg: '#0b0f19',
    bgRgb: '11 15 25',
    sidebar: '#111827',
    sidebarRgb: '17 24 39',
    card: '#1e293b',
    cardRgb: '30 41 59',
    well: '#0f172a',
    wellRgb: '15 23 42',
    border: '#334155',
    borderRgb: '51 65 85',
    btnSecondary: '#283548',
    btnSecondaryRgb: '40 53 72',
  },
  {
    id: 'royal-obsidian',
    name: 'Royal Obsidian',
    description: 'Deep midnight amethyst velvet',
    bg: '#0a0614',
    bgRgb: '10 6 20',
    sidebar: '#100a20',
    sidebarRgb: '16 10 32',
    card: '#1a1030',
    cardRgb: '26 16 48',
    well: '#0b0716',
    wellRgb: '11 7 22',
    border: '#352055',
    borderRgb: '53 32 85',
    btnSecondary: '#261642',
    btnSecondaryRgb: '38 22 66',
  },
  {
    id: 'emerald-shadow',
    name: 'Emerald Shadow',
    description: 'Dark enchanted night canopy',
    bg: '#040c08',
    bgRgb: '4 12 8',
    sidebar: '#07140d',
    sidebarRgb: '7 20 13',
    card: '#0d2217',
    cardRgb: '13 34 23',
    well: '#050e09',
    wellRgb: '5 14 9',
    border: '#1c422f',
    borderRgb: '28 66 47',
    btnSecondary: '#122e20',
    btnSecondaryRgb: '18 46 32',
  },
];

export const BUTTON_STYLES = [
  {
    id: 'rounded',
    name: 'Rounded',
    description: 'Balanced modern curved corners (8px)',
    btnRadius: '0.5rem',
    cardRadius: '0.875rem',
    inputRadius: '0.5rem',
  },
  {
    id: 'pill',
    name: 'Pill',
    description: 'Ultra-smooth rounded capsule geometry',
    btnRadius: '9999px',
    cardRadius: '1.25rem',
    inputRadius: '9999px',
  },
  {
    id: 'sharp',
    name: 'Sharp',
    description: 'Crisp, technical squared corners (4px)',
    btnRadius: '0.25rem',
    cardRadius: '0.375rem',
    inputRadius: '0.25rem',
  },
];

export const GLASS_EFFECTS = [
  {
    id: 'standard',
    name: 'Standard Frost',
    description: '12px blur with balanced translucency',
    blur: '12px',
    cardOpacity: '0.85',
    borderOpacity: '0.55',
  },
  {
    id: 'frosted',
    name: 'Deep Frost',
    description: '22px blur with high glass transparency',
    blur: '22px',
    cardOpacity: '0.68',
    borderOpacity: '0.7',
  },
  {
    id: 'solid',
    name: 'Solid Opaque',
    description: '100% solid surfaces for peak contrast and speed',
    blur: '0px',
    cardOpacity: '1.0',
    borderOpacity: '0.8',
  },
];

export const CURATED_THEMES = [
  {
    id: 'default',
    name: 'Zircon Cyan',
    subtitle: 'Classic Void & Crystal',
    accent: '#47d2c9',
    bg: '#070b0f',
    card: '#0e1622',
    theme: 'zircon-cyan',
    bgTheme: 'deep-void',
    buttonStyle: 'rounded',
    glassEffect: 'standard',
  },
  {
    id: 'purple-purple',
    name: 'Amethyst Geode',
    subtitle: 'Royal Violet & Obsidian',
    accent: '#a855f7',
    bg: '#0a0614',
    card: '#1a1030',
    theme: 'amethyst-purple',
    bgTheme: 'royal-obsidian',
    buttonStyle: 'rounded',
    glassEffect: 'frosted',
  },
  {
    id: 'green-green',
    name: 'Emerald Canopy',
    subtitle: 'Lush Green & Shadow Forest',
    accent: '#22c55e',
    bg: '#040c08',
    card: '#0d2217',
    theme: 'emerald-green',
    bgTheme: 'emerald-shadow',
    buttonStyle: 'rounded',
    glassEffect: 'standard',
  },
  {
    id: 'orange-blue',
    name: 'Blaze & Abyss',
    subtitle: 'Solar Amber & Deep Navy',
    accent: '#f59e0b',
    bg: '#050a14',
    card: '#0c1830',
    theme: 'blaze-amber',
    bgTheme: 'abyssal-navy',
    buttonStyle: 'pill',
    glassEffect: 'standard',
  },
  {
    id: 'oled-crimson',
    name: 'Redstone OLED',
    subtitle: 'Crimson Pulse & Pure Black',
    accent: '#ef4444',
    bg: '#000000',
    card: '#0d0d0d',
    theme: 'redstone-crimson',
    bgTheme: 'oled-black',
    buttonStyle: 'sharp',
    glassEffect: 'solid',
  },
  {
    id: 'diamond-slate',
    name: 'Diamond Carbon',
    subtitle: 'Glacial Blue & Stealth Slate',
    accent: '#38bdf8',
    bg: '#0b0f19',
    card: '#1e293b',
    theme: 'diamond-blue',
    bgTheme: 'carbon-slate',
    buttonStyle: 'rounded',
    glassEffect: 'standard',
  },
];

export function detectCuratedTheme(theme, bgTheme) {
  const match = CURATED_THEMES.find(c => c.theme === theme && c.bgTheme === bgTheme);
  return match ? match.id : 'custom';
}

const LOCAL_STORAGE_THEME_KEY = 'zircon-theme';
const LOCAL_STORAGE_CUSTOM_HEX_KEY = 'zircon-custom-hex';
const LOCAL_STORAGE_BG_THEME_KEY = 'zircon-bg-theme';
const LOCAL_STORAGE_CUSTOM_BG_KEY = 'zircon-custom-bg';
const LOCAL_STORAGE_CUSTOM_CARD_KEY = 'zircon-custom-card';
const LOCAL_STORAGE_BUTTON_STYLE_KEY = 'zircon-button-style';
const LOCAL_STORAGE_GLASS_EFFECT_KEY = 'zircon-glass-effect';

/**
 * Parses 3 or 6 digit hex string to {r, g, b}.
 */
export function hexToRgb(hex) {
  let c = (hex || '').replace('#', '').trim();
  if (c.length === 3) {
    c = c.split('').map(x => x + x).join('');
  }
  const num = parseInt(c, 16);
  if (isNaN(num) || c.length !== 6) {
    return { r: 71, g: 210, b: 201 }; // fallback
  }
  return {
    r: (num >> 16) & 255,
    g: (num >> 8) & 255,
    b: num & 255,
  };
}

function rgbToHex(r, g, b) {
  const clamp = (v) => Math.max(0, Math.min(255, Math.round(v)));
  return '#' + [r, g, b].map(v => clamp(v).toString(16).padStart(2, '0')).join('');
}

/**
 * Dynamically computes bright, deep, and ink variants for any custom hex color.
 */
export function generateCustomPalette(hex) {
  const { r, g, b } = hexToRgb(hex);

  // Bright (+28% brightness, clamped)
  const brightR = Math.min(255, Math.round(r + (255 - r) * 0.28));
  const brightG = Math.min(255, Math.round(g + (255 - g) * 0.28));
  const brightB = Math.min(255, Math.round(b + (255 - b) * 0.28));

  // Deep (-22% brightness)
  const deepR = Math.round(r * 0.78);
  const deepG = Math.round(g * 0.78);
  const deepB = Math.round(b * 0.78);

  // Ink (dark tint for text on bright accent buttons)
  const inkR = Math.max(2, Math.round(r * 0.12));
  const inkG = Math.max(4, Math.round(g * 0.12));
  const inkB = Math.max(6, Math.round(b * 0.12));

  return {
    id: 'custom',
    name: 'Custom',
    description: 'Player customized accent color',
    hex,
    rgb: `${r} ${g} ${b}`,
    bright: rgbToHex(brightR, brightG, brightB),
    brightRgb: `${brightR} ${brightG} ${brightB}`,
    deep: rgbToHex(deepR, deepG, deepB),
    deepRgb: `${deepR} ${deepG} ${deepB}`,
    ink: rgbToHex(inkR, inkG, inkB),
    inkRgb: `${inkR} ${inkG} ${inkB}`,
    glow: `rgba(${r}, ${g}, ${b}, 0.35)`,
    glowStrong: `rgba(${r}, ${g}, ${b}, 0.5)`,
  };
}

/**
 * Generates custom background surfaces from a player-selected base hex.
 */
export function generateCustomBgPalette(baseHex, cardHex = null) {
  const base = hexToRgb(baseHex);
  const card = cardHex ? hexToRgb(cardHex) : {
    r: Math.min(255, Math.round(base.r * 1.5 + 8)),
    g: Math.min(255, Math.round(base.g * 1.5 + 10)),
    b: Math.min(255, Math.round(base.b * 1.5 + 14)),
  };
  const sidebar = {
    r: Math.min(255, Math.round(base.r * 1.2 + 4)),
    g: Math.min(255, Math.round(base.g * 1.2 + 5)),
    b: Math.min(255, Math.round(base.b * 1.2 + 7)),
  };
  const well = {
    r: Math.max(0, Math.round(base.r * 0.8)),
    g: Math.max(0, Math.round(base.g * 0.8)),
    b: Math.max(0, Math.round(base.b * 0.8)),
  };
  const border = {
    r: Math.min(255, Math.round(card.r * 1.6 + 18)),
    g: Math.min(255, Math.round(card.g * 1.6 + 22)),
    b: Math.min(255, Math.round(card.b * 1.6 + 28)),
  };
  const btnSecondary = {
    r: Math.min(255, Math.round(card.r * 1.3 + 10)),
    g: Math.min(255, Math.round(card.g * 1.3 + 12)),
    b: Math.min(255, Math.round(card.b * 1.3 + 16)),
  };

  return {
    id: 'custom',
    name: 'Custom Canvas',
    description: 'Custom background and surface palette',
    bg: baseHex,
    bgRgb: `${base.r} ${base.g} ${base.b}`,
    sidebar: rgbToHex(sidebar.r, sidebar.g, sidebar.b),
    sidebarRgb: `${sidebar.r} ${sidebar.g} ${sidebar.b}`,
    card: rgbToHex(card.r, card.g, card.b),
    cardRgb: `${card.r} ${card.g} ${card.b}`,
    well: rgbToHex(well.r, well.g, well.b),
    wellRgb: `${well.r} ${well.g} ${well.b}`,
    border: rgbToHex(border.r, border.g, border.b),
    borderRgb: `${border.r} ${border.g} ${border.b}`,
    btnSecondary: rgbToHex(btnSecondary.r, btnSecondary.g, btnSecondary.b),
    btnSecondaryRgb: `${btnSecondary.r} ${btnSecondary.g} ${btnSecondary.b}`,
  };
}

/**
 * Applies the selected theme and writes CSS variables onto :root.
 */
export function applyTheme(options = {}) {
  // Support both legacy signature applyTheme(themeId, customHex) and new object signature
  let themeId = typeof options === 'string' ? options : (options.theme || 'zircon-cyan');
  let customAccent = typeof options === 'string' ? arguments[1] : options.customAccent;
  let bgThemeId = options.bgTheme || 'deep-void';
  let customBg = options.customBg;
  let customCardBg = options.customCardBg;
  let buttonStyleId = options.buttonStyle || 'rounded';
  let glassEffectId = options.glassEffect || 'standard';

  // 1. Accent color
  let accentObj = null;
  if (themeId === 'custom' && customAccent) {
    accentObj = generateCustomPalette(customAccent);
  } else {
    accentObj = THEME_PRESETS.find(p => p.id === themeId) || THEME_PRESETS[0];
  }

  // 2. Background palette
  let bgObj = null;
  if (bgThemeId === 'custom' && customBg) {
    bgObj = generateCustomBgPalette(customBg, customCardBg);
  } else {
    bgObj = BG_THEME_PRESETS.find(p => p.id === bgThemeId) || BG_THEME_PRESETS[0];
  }

  // 3. Button & geometry style
  const btnObj = BUTTON_STYLES.find(s => s.id === buttonStyleId) || BUTTON_STYLES[0];

  // 4. Glass effect
  const glassObj = GLASS_EFFECTS.find(g => g.id === glassEffectId) || GLASS_EFFECTS[0];

  const root = document.documentElement;
  root.dataset.theme = accentObj.id;
  root.dataset.bgTheme = bgObj.id;
  root.dataset.buttonStyle = btnObj.id;
  root.dataset.glassEffect = glassObj.id;

  // Set Accent CSS variables
  root.style.setProperty('--color-accent', accentObj.hex);
  root.style.setProperty('--color-accent-rgb', accentObj.rgb);
  root.style.setProperty('--color-accent-bright', accentObj.bright);
  root.style.setProperty('--color-accent-bright-rgb', accentObj.brightRgb);
  root.style.setProperty('--color-accent-deep', accentObj.deep);
  root.style.setProperty('--color-accent-deep-rgb', accentObj.deepRgb);
  root.style.setProperty('--color-accent-ink', accentObj.ink);
  root.style.setProperty('--color-accent-ink-rgb', accentObj.inkRgb);
  root.style.setProperty('--color-accent-glow', accentObj.glow);
  root.style.setProperty('--color-accent-glow-strong', accentObj.glowStrong);

  // Set Background & Surface CSS variables
  root.style.setProperty('--color-bg', bgObj.bg);
  root.style.setProperty('--color-bg-rgb', bgObj.bgRgb);
  root.style.setProperty('--color-sidebar', bgObj.sidebar);
  root.style.setProperty('--color-sidebar-rgb', bgObj.sidebarRgb);
  root.style.setProperty('--color-card', bgObj.card);
  root.style.setProperty('--color-card-rgb', bgObj.cardRgb);
  root.style.setProperty('--color-well', bgObj.well);
  root.style.setProperty('--color-well-rgb', bgObj.wellRgb);
  root.style.setProperty('--color-border', bgObj.border);
  root.style.setProperty('--color-border-rgb', bgObj.borderRgb);
  root.style.setProperty('--color-btn-secondary', bgObj.btnSecondary);
  root.style.setProperty('--color-btn-secondary-rgb', bgObj.btnSecondaryRgb);

  // Set Geometry CSS variables
  root.style.setProperty('--border-radius-btn', btnObj.btnRadius);
  root.style.setProperty('--border-radius-card', btnObj.cardRadius);
  root.style.setProperty('--border-radius-input', btnObj.inputRadius);

  // Set Glassmorphism CSS variables
  root.style.setProperty('--card-blur', glassObj.blur);
  root.style.setProperty('--card-opacity', glassObj.cardOpacity);
  root.style.setProperty('--border-opacity', glassObj.borderOpacity);

  // Cache to localStorage for instantaneous zero-flicker reload
  try {
    localStorage.setItem(LOCAL_STORAGE_THEME_KEY, themeId);
    if (customAccent) localStorage.setItem(LOCAL_STORAGE_CUSTOM_HEX_KEY, customAccent);
    localStorage.setItem(LOCAL_STORAGE_BG_THEME_KEY, bgThemeId);
    if (customBg) localStorage.setItem(LOCAL_STORAGE_CUSTOM_BG_KEY, customBg);
    if (customCardBg) localStorage.setItem(LOCAL_STORAGE_CUSTOM_CARD_KEY, customCardBg);
    localStorage.setItem(LOCAL_STORAGE_BUTTON_STYLE_KEY, buttonStyleId);
    localStorage.setItem(LOCAL_STORAGE_GLASS_EFFECT_KEY, glassEffectId);
  } catch {
    // ignore
  }

  return {
    accent: accentObj,
    bg: bgObj,
    button: btnObj,
    glass: glassObj,
  };
}

/**
 * Called on application startup to immediately restore theme from localStorage without flicker.
 */
export function initTheme() {
  try {
    const savedTheme = localStorage.getItem(LOCAL_STORAGE_THEME_KEY) || 'zircon-cyan';
    const savedCustomHex = localStorage.getItem(LOCAL_STORAGE_CUSTOM_HEX_KEY) || '#47d2c9';
    const savedBgTheme = localStorage.getItem(LOCAL_STORAGE_BG_THEME_KEY) || 'deep-void';
    const savedCustomBg = localStorage.getItem(LOCAL_STORAGE_CUSTOM_BG_KEY) || '#070b0f';
    const savedCustomCard = localStorage.getItem(LOCAL_STORAGE_CUSTOM_CARD_KEY) || '#0e1622';
    const savedButtonStyle = localStorage.getItem(LOCAL_STORAGE_BUTTON_STYLE_KEY) || 'rounded';
    const savedGlassEffect = localStorage.getItem(LOCAL_STORAGE_GLASS_EFFECT_KEY) || 'standard';

    return applyTheme({
      theme: savedTheme,
      customAccent: savedCustomHex,
      bgTheme: savedBgTheme,
      customBg: savedCustomBg,
      customCardBg: savedCustomCard,
      buttonStyle: savedButtonStyle,
      glassEffect: savedGlassEffect,
    });
  } catch {
    return applyTheme('zircon-cyan');
  }
}
