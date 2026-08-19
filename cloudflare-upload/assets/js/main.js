/* Zircon site — minimal progressive-enhancement JS (no dependencies) */

// ---------------------------------------------------------------------------
// Release download paths — update these when publishing a new version so the
// site always points at the latest artifacts on the R2 bucket. The files live
// under /updates/ (the same binaries the auto-updaters consume) — there is no
// separate /downloads/ folder. Note the launcher filenames embed the version
// and the server zip lives under a versioned path, so both need the version
// bumped here (and in tauri.conf.json / Cargo.toml / build.bat) each release.
// ---------------------------------------------------------------------------
const RELEASE_URLS = {
  // Launcher MSI installer (primary "Download for Windows" button).
  launcherMsi: 'https://zirconmc.net/updates/launcher/Zircon_0.1.0_x64_en-US.msi',
  // Launcher NSIS setup exe (secondary button).
  launcherExe: 'https://zirconmc.net/updates/launcher/Zircon_0.1.0_x64-setup.exe',
  // Server manager Windows package (versioned path per the server updater).
  serverZip: 'https://zirconmc.net/updates/server/v0.1.0/zircon-server-windows-x86_64.zip',
};

document.addEventListener('DOMContentLoaded', () => {
  // ---- Wire every [data-download] button to the release URLs above ----
  document.querySelectorAll('[data-download]').forEach((a) => {
    const url = RELEASE_URLS[a.dataset.download];
    if (url) a.setAttribute('href', url);
  });

  // ---- Mobile nav toggle ----
  const toggle = document.querySelector('.nav-toggle');
  const links = document.querySelector('.nav-links');
  if (toggle && links) {
    toggle.addEventListener('click', () => {
      const open = links.classList.toggle('open');
      toggle.setAttribute('aria-expanded', String(open));
    });
    // Close the menu when a link is chosen.
    links.querySelectorAll('a').forEach((a) =>
      a.addEventListener('click', () => {
        links.classList.remove('open');
        toggle.setAttribute('aria-expanded', 'false');
      })
    );
  }

  // ---- Download tabs (players / server owners) ----
  const tabButtons = document.querySelectorAll('.tab-btn');
  const tabPanels = document.querySelectorAll('.tab-panel');
  if (tabButtons.length && tabPanels.length) {
    const activateTab = (name) => {
      tabButtons.forEach((btn) => {
        const on = btn.dataset.tab === name;
        btn.classList.toggle('active', on);
        btn.setAttribute('aria-selected', String(on));
      });
      tabPanels.forEach((p) => p.classList.toggle('active', p.dataset.panel === name));
    };

    tabButtons.forEach((btn) => {
      btn.addEventListener('click', () => {
        activateTab(btn.dataset.tab);
        // Let deep links + history work: update the hash without scrolling.
        history.replaceState(null, '', '#' + btn.dataset.tab);
      });
    });

    // Same-page hash links (the "I'm a Player" / "I Run a Server" CTAs and footer
    // links) change the hash without reloading the page — switch tabs to match.
    window.addEventListener('hashchange', () => {
      const wanted = window.location.hash.slice(1);
      if (wanted && document.querySelector(`.tab-btn[data-tab="${wanted}"]`)) {
        activateTab(wanted);
      }
    });

    // Honour a hash like #server-owners on load, else keep the first tab.
    const wanted = window.location.hash.slice(1);
    const initial = wanted && document.querySelector(`.tab-btn[data-tab="${wanted}"]`)
      ? wanted
      : (tabButtons[0]?.dataset.tab || '');
    activateTab(initial);
  }
});
