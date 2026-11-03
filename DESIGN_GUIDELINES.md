# Zircon UI Design System Guidelines (Cyber-Obsidian)

This document establishes the official design system rules for **Zircon Companion Launcher** (`crates/zircon-launcher/ui`) and **Zircon Server Web App** (`crates/zircon-server/assets/web`).

All developers and AI coding agents working in this repository **must strictly adhere to these guidelines** to preserve visual consistency and ensure 100% compatibility with the dynamic theming engine in Settings.

---

## 1. Golden Rule: Never Hardcode Colors or Radii

❌ **Forbidden in templates and styles:**
- Hardcoding surface backgrounds: `bg-[#0e1622]`, `bg-[#070b10]`, `bg-[#070b0f]`, `bg-[#0a0f14]`, `bg-slate-950`
- Hardcoding border colors: `border-slate-800`, `border-[#263545]`, `border-cyan-500/40`
- Hardcoding arbitrary border radii: `rounded-2xl`, `rounded-[14px]` (bypasses button geometry settings)
- Hardcoding text accent colors: `text-[#47d2c9]`, `text-cyan-400` (bypasses player accent choice)

✅ **Required:**
- Use the shared semantic component classes (`.z-card`, `.z-well`, `.z-btn-primary`, `.z-input`, `.z-badge`).
- When writing custom CSS, always reference CSS variables:
  - Surfaces: `var(--color-bg)`, `var(--color-sidebar)`, `var(--color-card)`, `var(--color-well)`, `var(--color-border)`
  - Accents: `var(--color-accent)`, `var(--color-accent-bright)`, `var(--color-accent-deep)`, `var(--color-accent-glow)`
  - Geometry: `var(--border-radius-card)`, `var(--border-radius-btn)`, `var(--border-radius-input)`
  - Glass: `var(--card-blur)`, `var(--card-opacity)`, `var(--border-opacity)`

---

## 2. Core Component Class Reference

### Surfaces
| Class | Description | Standard Usage |
| :--- | :--- | :--- |
| `.z-card` | Standard Obsidian glass card surface with 1px border and elevation | Main view panels, toolbars, sidebar cards, list items |
| `.z-card-interactive` | Card with subtle hover lift (`-2px`), active press bounce, and accent glow | Selectable instances, server cards, mod tiles |
| `.z-well` | Recessed, dark background container (`#05080e`) | Log viewers, console output, code blocks, search bars |
| `.z-modal-backdrop` | Fixed full-screen backdrop with `rgba(0,0,0,0.82)` and 12px blur | Dialog and confirmation overlays |
| `.z-modal-dialog` | Glassmorphic floating card with accent border and 30px glow | Modal containers |

### Buttons
| Class | Description | Standard Usage |
| :--- | :--- | :--- |
| `.z-btn` | Base button class (flex center, rounded, transition, disabled state) | Applied to all buttons |
| `.z-btn-primary` / `.z-btn-accent` | Radiant gradient fill using current accent + dark ink text | "Launch Game", "Save", "Install", primary actions |
| `.z-btn-secondary` | Dark surface (`#1c2530`) with border; turns accent on hover | "Cancel", "Back", "Options", toolbar actions |
| `.z-btn-ghost` | Frameless, transparent background; highlights on hover | Header close buttons, icon actions, inline links |
| `.z-btn-danger` | Soft red fill with crimson outline | "Delete Instance", "Stop Server", "Uninstall" |

### Form Controls
| Class | Description | Standard Usage |
| :--- | :--- | :--- |
| `.z-input` | Dark background, subtle border, placeholder color, focus glow | Text inputs, number inputs, search boxes |
| `.z-select` | Custom select container matching `.z-input` | Dropdown menus, version selectors |
| `.z-toggle` | Animated pill slider switch (with `.z-toggle-on` & `.z-toggle-thumb`) | Feature toggles, settings switches |
| `.z-textarea` | Multiline text input matching `.z-input` | JVM arguments, server MOTD, custom notes |

### Status Badges
| Class | Description | Standard Usage |
| :--- | :--- | :--- |
| `.z-badge` | Base pill container with uppercase font | Base tag |
| `.z-badge-live` | Emerald glowing pill (`#34d399`) | "LIVE", server running, online status |
| `.z-badge-accent` | Brand accent glow pill | Mod loader tags ("FABRIC", "NEOFORGE") |
| `.z-badge-warn` | Amber alert pill (`#fbbf24`) | "OUTDATED", missing dependency |
| `.z-badge-danger` | Red alert pill (`#f87171`) | "CRASHED", port conflict, error |

---

## 3. Automated Visual Inspection Loop

Antigravity and developers can take instant UI snapshots:
1. In the Launcher, press **F11** or **Ctrl+Shift+S** to snap the active view to `scratch/ui_snapshots/`.
2. In the console or via IPC, call `window.__takeUiScreenshot("view_name")` or `api.captureUi("view_name")`.
3. Read the resulting PNG using `read_file` to visually verify layout, alignment, and theming.

