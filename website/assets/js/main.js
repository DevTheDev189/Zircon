/* Zircon site — minimal progressive-enhancement JS (no dependencies) */

document.addEventListener('DOMContentLoaded', () => {
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
    tabButtons.forEach((btn) => {
      btn.addEventListener('click', () => {
        const target = btn.dataset.tab;
        tabButtons.forEach((b) => b.classList.toggle('active', b === btn));
        tabPanels.forEach((p) => p.classList.toggle('active', p.dataset.panel === target));
        // Let deep links + history work: update the hash without scrolling.
        history.replaceState(null, '', '#' + target);
      });
    });

    // Honour a hash like #server-owners on load, else keep the first tab.
    const wanted = window.location.hash.slice(1);
    const initial = wanted && document.querySelector(`.tab-btn[data-tab="${wanted}"]`)
      ? wanted
      : (tabButtons[0]?.dataset.tab || '');
    const btn = document.querySelector(`.tab-btn[data-tab="${initial}"]`);
    if (btn) btn.click();
  }
});
