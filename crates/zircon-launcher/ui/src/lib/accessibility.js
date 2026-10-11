/**
 * Accessibility settings manager for Zircon Launcher.
 * Restores and persists accessibility options (OpenDyslexic / Lexend font,
 * UI scaling, reduced motion, high contrast) across launcher sessions.
 */

export function applyAccessibilitySettings(options = {}) {
  if (typeof document === 'undefined') return;
  const root = document.documentElement;

  const fontFamily =
    options.fontFamily ??
    (typeof localStorage !== 'undefined' ? localStorage.getItem('zircon_font') : null) ??
    'default';

  const uiScale =
    options.uiScale ??
    (typeof localStorage !== 'undefined' && localStorage.getItem('zircon_ui_scale')
      ? Number(localStorage.getItem('zircon_ui_scale'))
      : 1.0);

  const reduceMotion =
    options.reduceMotion ??
    (typeof localStorage !== 'undefined'
      ? localStorage.getItem('zircon_reduce_motion') === 'true'
      : false);

  const highContrast =
    options.highContrast ??
    (typeof localStorage !== 'undefined'
      ? localStorage.getItem('zircon_high_contrast') === 'true'
      : false);

  // Font
  root.classList.remove('font-dyslexic', 'font-lexend');
  if (fontFamily === 'open-dyslexic') {
    root.classList.add('font-dyslexic');
  } else if (fontFamily === 'lexend') {
    root.classList.add('font-lexend');
  }

  // UI Scale
  if (uiScale && uiScale !== 1.0) {
    root.setAttribute('data-ui-scale', String(uiScale));
  } else {
    root.removeAttribute('data-ui-scale');
  }

  // Reduce Motion
  if (reduceMotion) {
    root.classList.add('reduce-motion');
  } else {
    root.classList.remove('reduce-motion');
  }

  // High Contrast
  if (highContrast) {
    root.classList.add('high-contrast');
  } else {
    root.classList.remove('high-contrast');
  }

  if (typeof localStorage !== 'undefined') {
    if (options.fontFamily !== undefined) {
      localStorage.setItem('zircon_font', options.fontFamily);
    }
    if (options.uiScale !== undefined) {
      localStorage.setItem('zircon_ui_scale', String(options.uiScale));
    }
    if (options.reduceMotion !== undefined) {
      localStorage.setItem('zircon_reduce_motion', String(options.reduceMotion));
    }
    if (options.highContrast !== undefined) {
      localStorage.setItem('zircon_high_contrast', String(options.highContrast));
    }
    if (options.colorblindBadges !== undefined) {
      localStorage.setItem('zircon_colorblind', String(options.colorblindBadges));
    }
    if (options.soundCues !== undefined) {
      localStorage.setItem('zircon_sound_cues', String(options.soundCues));
    }
  }
}

/**
 * Called on application startup to restore accessibility classes immediately.
 */
export function initAccessibility() {
  applyAccessibilitySettings();
}
