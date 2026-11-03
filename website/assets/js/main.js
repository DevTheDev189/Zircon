/* ============================================================
   Zircon Website JavaScript
   Interactive showcases, carousels, lightbox modal, and motion.
   ============================================================ */

document.addEventListener('DOMContentLoaded', () => {

  // ---------------------------------------------------------------------------
  // 1b. Multi-Platform OS Auto-Detection & Selector
  // ---------------------------------------------------------------------------
  const detectClientOS = () => {
    const ua = (navigator.userAgent || '').toLowerCase();
    const plat = (navigator.userAgentData?.platform || navigator.platform || '').toLowerCase();
    if (plat.includes('mac') || ua.includes('macintosh') || ua.includes('mac os')) return 'macos';
    if (plat.includes('linux') || ua.includes('linux') || ua.includes('x11')) return 'linux';
    return 'windows';
  };

  const detectedOS = detectClientOS();
  const osLabelEl = document.getElementById('detected-os-label');
  const osPills = document.querySelectorAll('.os-pill');
  const osPanels = document.querySelectorAll('.os-content');

  const setOS = (osKey) => {
    const names = { windows: 'Windows', macos: 'macOS', linux: 'Linux' };
    if (osLabelEl) osLabelEl.textContent = names[osKey] || osKey;
    osPills.forEach((p) => p.classList.toggle('active', p.dataset.os === osKey));
    osPanels.forEach((panel) => panel.classList.toggle('active', panel.dataset.osPanel === osKey));
  };

  if (osPills.length) {
    setOS(detectedOS);
    osPills.forEach((btn) => {
      btn.addEventListener('click', () => {
        setOS(btn.dataset.os);
        // ---------------------------------------------------------------------------
  // 13. Interactive Hero Benchmark Slider
  // ---------------------------------------------------------------------------
  const benchmarkSlider = document.getElementById('benchmarkSlider');
  const benchmarkPlayerCount = document.getElementById('benchmarkPlayerCount');
  const benchmarkManualTime = document.getElementById('benchmarkManualTime');
  const benchmarkManualBar = document.getElementById('benchmarkManualBar');
  const benchmarkManualSubText = document.getElementById('benchmarkManualSubText');
  const benchmarkSliderNote = document.getElementById('benchmarkSliderNote');

  if (benchmarkSlider && benchmarkPlayerCount) {
    const scenarios = [
      { players: 1, time: '12 min', bar: '35%', note: '"Missing dependency cloth-config. Why won\'t this jar load?"', sub: 'Manual folder hunt & missing dependencies' },
      { players: 2, time: '25 min', bar: '55%', note: '"Wait, are you on Fabric 0.15 or 0.16? My client is crashing."', sub: 'Version mismatch & config conflict' },
      { players: 3, time: '35 min', bar: '70%', note: '"Someone downloaded the Forge build instead of Fabric. Redownloading..."', sub: 'Wrong mod loader build downloaded' },
      { players: 4, time: '45 min', bar: '82%', note: '"One friend has Java 17, server needs Java 21. Discord screenshare begins."', sub: 'Java runtime version mismatch' },
      { players: 6, time: '1.2 hrs', bar: '92%', note: '"Zip file from Google Drive exceeded download quota. Uploading to MediaFire..."', sub: 'Broken links & cloud download quotas' },
      { players: 8, time: '2.0 hrs', bar: '97%', note: '"Two friends gave up and logged off. Game night effectively canceled."', sub: 'Friend fatigue & canceled game night' },
      { players: 12, time: '3.5 hrs', bar: '100%', note: '"Nobody is playing. You spent the entire Friday night troubleshooting in Discord."', sub: 'Complete group failure & wasted evening' },
    ];

    const updateBenchmark = (val) => {
      const num = parseInt(val, 10);
      benchmarkPlayerCount.textContent = num === 1 ? '1 Player' : `${num} Players`;

      let closest = scenarios[0];
      for (const s of scenarios) {
        if (num >= s.players) closest = s;
      }

      if (benchmarkManualTime) benchmarkManualTime.textContent = closest.time;
      if (benchmarkManualBar) benchmarkManualBar.style.width = closest.bar;
      if (benchmarkManualSubText) benchmarkManualSubText.textContent = closest.sub;
      if (benchmarkSliderNote) benchmarkSliderNote.textContent = closest.note;
    };

    benchmarkSlider.addEventListener('input', (e) => {
      updateBenchmark(e.target.value);
    });
  }
});
      // ---------------------------------------------------------------------------
  // 13. Interactive Hero Benchmark Slider
  // ---------------------------------------------------------------------------
  const benchmarkSlider = document.getElementById('benchmarkSlider');
  const benchmarkPlayerCount = document.getElementById('benchmarkPlayerCount');
  const benchmarkManualTime = document.getElementById('benchmarkManualTime');
  const benchmarkManualBar = document.getElementById('benchmarkManualBar');
  const benchmarkManualSubText = document.getElementById('benchmarkManualSubText');
  const benchmarkSliderNote = document.getElementById('benchmarkSliderNote');

  if (benchmarkSlider && benchmarkPlayerCount) {
    const scenarios = [
      { players: 1, time: '12 min', bar: '35%', note: '"Missing dependency cloth-config. Why won\'t this jar load?"', sub: 'Manual folder hunt & missing dependencies' },
      { players: 2, time: '25 min', bar: '55%', note: '"Wait, are you on Fabric 0.15 or 0.16? My client is crashing."', sub: 'Version mismatch & config conflict' },
      { players: 3, time: '35 min', bar: '70%', note: '"Someone downloaded the Forge build instead of Fabric. Redownloading..."', sub: 'Wrong mod loader build downloaded' },
      { players: 4, time: '45 min', bar: '82%', note: '"One friend has Java 17, server needs Java 21. Discord screenshare begins."', sub: 'Java runtime version mismatch' },
      { players: 6, time: '1.2 hrs', bar: '92%', note: '"Zip file from Google Drive exceeded download quota. Uploading to MediaFire..."', sub: 'Broken links & cloud download quotas' },
      { players: 8, time: '2.0 hrs', bar: '97%', note: '"Two friends gave up and logged off. Game night effectively canceled."', sub: 'Friend fatigue & canceled game night' },
      { players: 12, time: '3.5 hrs', bar: '100%', note: '"Nobody is playing. You spent the entire Friday night troubleshooting in Discord."', sub: 'Complete group failure & wasted evening' },
    ];

    const updateBenchmark = (val) => {
      const num = parseInt(val, 10);
      benchmarkPlayerCount.textContent = num === 1 ? '1 Player' : `${num} Players`;

      let closest = scenarios[0];
      for (const s of scenarios) {
        if (num >= s.players) closest = s;
      }

      if (benchmarkManualTime) benchmarkManualTime.textContent = closest.time;
      if (benchmarkManualBar) benchmarkManualBar.style.width = closest.bar;
      if (benchmarkManualSubText) benchmarkManualSubText.textContent = closest.sub;
      if (benchmarkSliderNote) benchmarkSliderNote.textContent = closest.note;
    };

    benchmarkSlider.addEventListener('input', (e) => {
      updateBenchmark(e.target.value);
    });
  }
});
  }

  // ---------------------------------------------------------------------------
  // 1c. Hero Direct Download 1-Click Router
  // ---------------------------------------------------------------------------
  const heroPrimaryBtn = document.getElementById('hero-primary-download');
  if (heroPrimaryBtn) {
    if (detectedOS === 'macos') {
      heroPrimaryBtn.href = 'https://downloads.zirconmc.net/updates/launcher/Zircon_0.9.4_aarch64.dmg';
      const label = heroPrimaryBtn.querySelector('.download-label');
      if (label) label.textContent = 'Download for macOS (.dmg)';
    } else if (detectedOS === 'linux') {
      heroPrimaryBtn.href = 'https://downloads.zirconmc.net/updates/launcher/Zircon_0.9.4_amd64.AppImage';
      const label = heroPrimaryBtn.querySelector('.download-label');
      if (label) label.textContent = 'Download for Linux (.AppImage)';
    } else {
      heroPrimaryBtn.href = 'https://downloads.zirconmc.net/updates/launcher/Zircon_0.9.4_x64-setup.exe';
      const label = heroPrimaryBtn.querySelector('.download-label');
      if (label) label.textContent = 'Download for Windows (.exe)';
    }
  }

  // ---------------------------------------------------------------------------
  // 2. Mobile nav toggle
  // ---------------------------------------------------------------------------
  const toggle = document.querySelector('.nav-toggle');
  const links = document.querySelector('.nav-links');
  if (toggle && links) {
    toggle.addEventListener('click', () => {
      const open = links.classList.toggle('open');
      toggle.setAttribute('aria-expanded', String(open));
      // ---------------------------------------------------------------------------
  // 13. Interactive Hero Benchmark Slider
  // ---------------------------------------------------------------------------
  const benchmarkSlider = document.getElementById('benchmarkSlider');
  const benchmarkPlayerCount = document.getElementById('benchmarkPlayerCount');
  const benchmarkManualTime = document.getElementById('benchmarkManualTime');
  const benchmarkManualBar = document.getElementById('benchmarkManualBar');
  const benchmarkManualSubText = document.getElementById('benchmarkManualSubText');
  const benchmarkSliderNote = document.getElementById('benchmarkSliderNote');

  if (benchmarkSlider && benchmarkPlayerCount) {
    const scenarios = [
      { players: 1, time: '12 min', bar: '35%', note: '"Missing dependency cloth-config. Why won\'t this jar load?"', sub: 'Manual folder hunt & missing dependencies' },
      { players: 2, time: '25 min', bar: '55%', note: '"Wait, are you on Fabric 0.15 or 0.16? My client is crashing."', sub: 'Version mismatch & config conflict' },
      { players: 3, time: '35 min', bar: '70%', note: '"Someone downloaded the Forge build instead of Fabric. Redownloading..."', sub: 'Wrong mod loader build downloaded' },
      { players: 4, time: '45 min', bar: '82%', note: '"One friend has Java 17, server needs Java 21. Discord screenshare begins."', sub: 'Java runtime version mismatch' },
      { players: 6, time: '1.2 hrs', bar: '92%', note: '"Zip file from Google Drive exceeded download quota. Uploading to MediaFire..."', sub: 'Broken links & cloud download quotas' },
      { players: 8, time: '2.0 hrs', bar: '97%', note: '"Two friends gave up and logged off. Game night effectively canceled."', sub: 'Friend fatigue & canceled game night' },
      { players: 12, time: '3.5 hrs', bar: '100%', note: '"Nobody is playing. You spent the entire Friday night troubleshooting in Discord."', sub: 'Complete group failure & wasted evening' },
    ];

    const updateBenchmark = (val) => {
      const num = parseInt(val, 10);
      benchmarkPlayerCount.textContent = num === 1 ? '1 Player' : `${num} Players`;

      let closest = scenarios[0];
      for (const s of scenarios) {
        if (num >= s.players) closest = s;
      }

      if (benchmarkManualTime) benchmarkManualTime.textContent = closest.time;
      if (benchmarkManualBar) benchmarkManualBar.style.width = closest.bar;
      if (benchmarkManualSubText) benchmarkManualSubText.textContent = closest.sub;
      if (benchmarkSliderNote) benchmarkSliderNote.textContent = closest.note;
    };

    benchmarkSlider.addEventListener('input', (e) => {
      updateBenchmark(e.target.value);
    });
  }
});
    // Close the menu when a link is chosen.
    links.querySelectorAll('a').forEach((a) =>
      a.addEventListener('click', () => {
        links.classList.remove('open');
        toggle.setAttribute('aria-expanded', 'false');
      })
    );
  }

  // ---------------------------------------------------------------------------
  // 3. Download tabs (players / server owners)
  // ---------------------------------------------------------------------------
  const tabButtons = document.querySelectorAll('.tab-btn');
  const tabPanels = document.querySelectorAll('.tab-panel');

  if (tabButtons.length && tabPanels.length) {
    const activateTab = (name, shouldScroll = false) => {
      let found = false;
      tabButtons.forEach((btn) => {
        const on = btn.dataset.tab === name;
        if (on) found = true;
        btn.classList.toggle('active', on);
        btn.setAttribute('aria-selected', String(on));
        // ---------------------------------------------------------------------------
  // 13. Interactive Hero Benchmark Slider
  // ---------------------------------------------------------------------------
  const benchmarkSlider = document.getElementById('benchmarkSlider');
  const benchmarkPlayerCount = document.getElementById('benchmarkPlayerCount');
  const benchmarkManualTime = document.getElementById('benchmarkManualTime');
  const benchmarkManualBar = document.getElementById('benchmarkManualBar');
  const benchmarkManualSubText = document.getElementById('benchmarkManualSubText');
  const benchmarkSliderNote = document.getElementById('benchmarkSliderNote');

  if (benchmarkSlider && benchmarkPlayerCount) {
    const scenarios = [
      { players: 1, time: '12 min', bar: '35%', note: '"Missing dependency cloth-config. Why won\'t this jar load?"', sub: 'Manual folder hunt & missing dependencies' },
      { players: 2, time: '25 min', bar: '55%', note: '"Wait, are you on Fabric 0.15 or 0.16? My client is crashing."', sub: 'Version mismatch & config conflict' },
      { players: 3, time: '35 min', bar: '70%', note: '"Someone downloaded the Forge build instead of Fabric. Redownloading..."', sub: 'Wrong mod loader build downloaded' },
      { players: 4, time: '45 min', bar: '82%', note: '"One friend has Java 17, server needs Java 21. Discord screenshare begins."', sub: 'Java runtime version mismatch' },
      { players: 6, time: '1.2 hrs', bar: '92%', note: '"Zip file from Google Drive exceeded download quota. Uploading to MediaFire..."', sub: 'Broken links & cloud download quotas' },
      { players: 8, time: '2.0 hrs', bar: '97%', note: '"Two friends gave up and logged off. Game night effectively canceled."', sub: 'Friend fatigue & canceled game night' },
      { players: 12, time: '3.5 hrs', bar: '100%', note: '"Nobody is playing. You spent the entire Friday night troubleshooting in Discord."', sub: 'Complete group failure & wasted evening' },
    ];

    const updateBenchmark = (val) => {
      const num = parseInt(val, 10);
      benchmarkPlayerCount.textContent = num === 1 ? '1 Player' : `${num} Players`;

      let closest = scenarios[0];
      for (const s of scenarios) {
        if (num >= s.players) closest = s;
      }

      if (benchmarkManualTime) benchmarkManualTime.textContent = closest.time;
      if (benchmarkManualBar) benchmarkManualBar.style.width = closest.bar;
      if (benchmarkManualSubText) benchmarkManualSubText.textContent = closest.sub;
      if (benchmarkSliderNote) benchmarkSliderNote.textContent = closest.note;
    };

    benchmarkSlider.addEventListener('input', (e) => {
      updateBenchmark(e.target.value);
    });
  }
});

      tabPanels.forEach((p) => {
        const on = p.dataset.panel === name;
        p.classList.toggle('active', on);
        // ---------------------------------------------------------------------------
  // 13. Interactive Hero Benchmark Slider
  // ---------------------------------------------------------------------------
  const benchmarkSlider = document.getElementById('benchmarkSlider');
  const benchmarkPlayerCount = document.getElementById('benchmarkPlayerCount');
  const benchmarkManualTime = document.getElementById('benchmarkManualTime');
  const benchmarkManualBar = document.getElementById('benchmarkManualBar');
  const benchmarkManualSubText = document.getElementById('benchmarkManualSubText');
  const benchmarkSliderNote = document.getElementById('benchmarkSliderNote');

  if (benchmarkSlider && benchmarkPlayerCount) {
    const scenarios = [
      { players: 1, time: '12 min', bar: '35%', note: '"Missing dependency cloth-config. Why won\'t this jar load?"', sub: 'Manual folder hunt & missing dependencies' },
      { players: 2, time: '25 min', bar: '55%', note: '"Wait, are you on Fabric 0.15 or 0.16? My client is crashing."', sub: 'Version mismatch & config conflict' },
      { players: 3, time: '35 min', bar: '70%', note: '"Someone downloaded the Forge build instead of Fabric. Redownloading..."', sub: 'Wrong mod loader build downloaded' },
      { players: 4, time: '45 min', bar: '82%', note: '"One friend has Java 17, server needs Java 21. Discord screenshare begins."', sub: 'Java runtime version mismatch' },
      { players: 6, time: '1.2 hrs', bar: '92%', note: '"Zip file from Google Drive exceeded download quota. Uploading to MediaFire..."', sub: 'Broken links & cloud download quotas' },
      { players: 8, time: '2.0 hrs', bar: '97%', note: '"Two friends gave up and logged off. Game night effectively canceled."', sub: 'Friend fatigue & canceled game night' },
      { players: 12, time: '3.5 hrs', bar: '100%', note: '"Nobody is playing. You spent the entire Friday night troubleshooting in Discord."', sub: 'Complete group failure & wasted evening' },
    ];

    const updateBenchmark = (val) => {
      const num = parseInt(val, 10);
      benchmarkPlayerCount.textContent = num === 1 ? '1 Player' : `${num} Players`;

      let closest = scenarios[0];
      for (const s of scenarios) {
        if (num >= s.players) closest = s;
      }

      if (benchmarkManualTime) benchmarkManualTime.textContent = closest.time;
      if (benchmarkManualBar) benchmarkManualBar.style.width = closest.bar;
      if (benchmarkManualSubText) benchmarkManualSubText.textContent = closest.sub;
      if (benchmarkSliderNote) benchmarkSliderNote.textContent = closest.note;
    };

    benchmarkSlider.addEventListener('input', (e) => {
      updateBenchmark(e.target.value);
    });
  }
});

      if (shouldScroll && found) {
        const targetPanel = document.querySelector(`.tab-panel[data-panel="${name}"]`) || document.querySelector('.tabs');
        if (targetPanel) {
          targetPanel.scrollIntoView({ behavior: 'smooth', block: 'start'   // ---------------------------------------------------------------------------
  // 13. Interactive Hero Benchmark Slider
  // ---------------------------------------------------------------------------
  const benchmarkSlider = document.getElementById('benchmarkSlider');
  const benchmarkPlayerCount = document.getElementById('benchmarkPlayerCount');
  const benchmarkManualTime = document.getElementById('benchmarkManualTime');
  const benchmarkManualBar = document.getElementById('benchmarkManualBar');
  const benchmarkManualSubText = document.getElementById('benchmarkManualSubText');
  const benchmarkSliderNote = document.getElementById('benchmarkSliderNote');

  if (benchmarkSlider && benchmarkPlayerCount) {
    const scenarios = [
      { players: 1, time: '12 min', bar: '35%', note: '"Missing dependency cloth-config. Why won\'t this jar load?"', sub: 'Manual folder hunt & missing dependencies' },
      { players: 2, time: '25 min', bar: '55%', note: '"Wait, are you on Fabric 0.15 or 0.16? My client is crashing."', sub: 'Version mismatch & config conflict' },
      { players: 3, time: '35 min', bar: '70%', note: '"Someone downloaded the Forge build instead of Fabric. Redownloading..."', sub: 'Wrong mod loader build downloaded' },
      { players: 4, time: '45 min', bar: '82%', note: '"One friend has Java 17, server needs Java 21. Discord screenshare begins."', sub: 'Java runtime version mismatch' },
      { players: 6, time: '1.2 hrs', bar: '92%', note: '"Zip file from Google Drive exceeded download quota. Uploading to MediaFire..."', sub: 'Broken links & cloud download quotas' },
      { players: 8, time: '2.0 hrs', bar: '97%', note: '"Two friends gave up and logged off. Game night effectively canceled."', sub: 'Friend fatigue & canceled game night' },
      { players: 12, time: '3.5 hrs', bar: '100%', note: '"Nobody is playing. You spent the entire Friday night troubleshooting in Discord."', sub: 'Complete group failure & wasted evening' },
    ];

    const updateBenchmark = (val) => {
      const num = parseInt(val, 10);
      benchmarkPlayerCount.textContent = num === 1 ? '1 Player' : `${num} Players`;

      let closest = scenarios[0];
      for (const s of scenarios) {
        if (num >= s.players) closest = s;
      }

      if (benchmarkManualTime) benchmarkManualTime.textContent = closest.time;
      if (benchmarkManualBar) benchmarkManualBar.style.width = closest.bar;
      if (benchmarkManualSubText) benchmarkManualSubText.textContent = closest.sub;
      if (benchmarkSliderNote) benchmarkSliderNote.textContent = closest.note;
    };

    benchmarkSlider.addEventListener('input', (e) => {
      updateBenchmark(e.target.value);
    });
  }
});
        }
      }
    };

    // Tab button click listener
    tabButtons.forEach((btn) => {
      btn.addEventListener('click', (e) => {
        e.preventDefault();
        const tabName = btn.dataset.tab;
        activateTab(tabName, false);
        history.replaceState(null, '', '#' + tabName);
        // ---------------------------------------------------------------------------
  // 13. Interactive Hero Benchmark Slider
  // ---------------------------------------------------------------------------
  const benchmarkSlider = document.getElementById('benchmarkSlider');
  const benchmarkPlayerCount = document.getElementById('benchmarkPlayerCount');
  const benchmarkManualTime = document.getElementById('benchmarkManualTime');
  const benchmarkManualBar = document.getElementById('benchmarkManualBar');
  const benchmarkManualSubText = document.getElementById('benchmarkManualSubText');
  const benchmarkSliderNote = document.getElementById('benchmarkSliderNote');

  if (benchmarkSlider && benchmarkPlayerCount) {
    const scenarios = [
      { players: 1, time: '12 min', bar: '35%', note: '"Missing dependency cloth-config. Why won\'t this jar load?"', sub: 'Manual folder hunt & missing dependencies' },
      { players: 2, time: '25 min', bar: '55%', note: '"Wait, are you on Fabric 0.15 or 0.16? My client is crashing."', sub: 'Version mismatch & config conflict' },
      { players: 3, time: '35 min', bar: '70%', note: '"Someone downloaded the Forge build instead of Fabric. Redownloading..."', sub: 'Wrong mod loader build downloaded' },
      { players: 4, time: '45 min', bar: '82%', note: '"One friend has Java 17, server needs Java 21. Discord screenshare begins."', sub: 'Java runtime version mismatch' },
      { players: 6, time: '1.2 hrs', bar: '92%', note: '"Zip file from Google Drive exceeded download quota. Uploading to MediaFire..."', sub: 'Broken links & cloud download quotas' },
      { players: 8, time: '2.0 hrs', bar: '97%', note: '"Two friends gave up and logged off. Game night effectively canceled."', sub: 'Friend fatigue & canceled game night' },
      { players: 12, time: '3.5 hrs', bar: '100%', note: '"Nobody is playing. You spent the entire Friday night troubleshooting in Discord."', sub: 'Complete group failure & wasted evening' },
    ];

    const updateBenchmark = (val) => {
      const num = parseInt(val, 10);
      benchmarkPlayerCount.textContent = num === 1 ? '1 Player' : `${num} Players`;

      let closest = scenarios[0];
      for (const s of scenarios) {
        if (num >= s.players) closest = s;
      }

      if (benchmarkManualTime) benchmarkManualTime.textContent = closest.time;
      if (benchmarkManualBar) benchmarkManualBar.style.width = closest.bar;
      if (benchmarkManualSubText) benchmarkManualSubText.textContent = closest.sub;
      if (benchmarkSliderNote) benchmarkSliderNote.textContent = closest.note;
    };

    benchmarkSlider.addEventListener('input', (e) => {
      updateBenchmark(e.target.value);
    });
  }
});
      // ---------------------------------------------------------------------------
  // 13. Interactive Hero Benchmark Slider
  // ---------------------------------------------------------------------------
  const benchmarkSlider = document.getElementById('benchmarkSlider');
  const benchmarkPlayerCount = document.getElementById('benchmarkPlayerCount');
  const benchmarkManualTime = document.getElementById('benchmarkManualTime');
  const benchmarkManualBar = document.getElementById('benchmarkManualBar');
  const benchmarkManualSubText = document.getElementById('benchmarkManualSubText');
  const benchmarkSliderNote = document.getElementById('benchmarkSliderNote');

  if (benchmarkSlider && benchmarkPlayerCount) {
    const scenarios = [
      { players: 1, time: '12 min', bar: '35%', note: '"Missing dependency cloth-config. Why won\'t this jar load?"', sub: 'Manual folder hunt & missing dependencies' },
      { players: 2, time: '25 min', bar: '55%', note: '"Wait, are you on Fabric 0.15 or 0.16? My client is crashing."', sub: 'Version mismatch & config conflict' },
      { players: 3, time: '35 min', bar: '70%', note: '"Someone downloaded the Forge build instead of Fabric. Redownloading..."', sub: 'Wrong mod loader build downloaded' },
      { players: 4, time: '45 min', bar: '82%', note: '"One friend has Java 17, server needs Java 21. Discord screenshare begins."', sub: 'Java runtime version mismatch' },
      { players: 6, time: '1.2 hrs', bar: '92%', note: '"Zip file from Google Drive exceeded download quota. Uploading to MediaFire..."', sub: 'Broken links & cloud download quotas' },
      { players: 8, time: '2.0 hrs', bar: '97%', note: '"Two friends gave up and logged off. Game night effectively canceled."', sub: 'Friend fatigue & canceled game night' },
      { players: 12, time: '3.5 hrs', bar: '100%', note: '"Nobody is playing. You spent the entire Friday night troubleshooting in Discord."', sub: 'Complete group failure & wasted evening' },
    ];

    const updateBenchmark = (val) => {
      const num = parseInt(val, 10);
      benchmarkPlayerCount.textContent = num === 1 ? '1 Player' : `${num} Players`;

      let closest = scenarios[0];
      for (const s of scenarios) {
        if (num >= s.players) closest = s;
      }

      if (benchmarkManualTime) benchmarkManualTime.textContent = closest.time;
      if (benchmarkManualBar) benchmarkManualBar.style.width = closest.bar;
      if (benchmarkManualSubText) benchmarkManualSubText.textContent = closest.sub;
      if (benchmarkSliderNote) benchmarkSliderNote.textContent = closest.note;
    };

    benchmarkSlider.addEventListener('input', (e) => {
      updateBenchmark(e.target.value);
    });
  }
});

    // Intercept any link on the page that links to #players or #server-owners
    document.addEventListener('click', (e) => {
      const anchor = e.target.closest('a');
      if (!anchor) return;
      const href = anchor.getAttribute('href') || '';
      const match = href.match(/#(players|server-owners|cloud)$/);
      if (match) {
        const targetTab = match[1];
        if (document.querySelector(`.tab-btn[data-tab="${targetTab}"]`)) {
          e.preventDefault();
          activateTab(targetTab, true);
          history.pushState(null, '', '#' + targetTab);
        }
      }
      // ---------------------------------------------------------------------------
  // 13. Interactive Hero Benchmark Slider
  // ---------------------------------------------------------------------------
  const benchmarkSlider = document.getElementById('benchmarkSlider');
  const benchmarkPlayerCount = document.getElementById('benchmarkPlayerCount');
  const benchmarkManualTime = document.getElementById('benchmarkManualTime');
  const benchmarkManualBar = document.getElementById('benchmarkManualBar');
  const benchmarkManualSubText = document.getElementById('benchmarkManualSubText');
  const benchmarkSliderNote = document.getElementById('benchmarkSliderNote');

  if (benchmarkSlider && benchmarkPlayerCount) {
    const scenarios = [
      { players: 1, time: '12 min', bar: '35%', note: '"Missing dependency cloth-config. Why won\'t this jar load?"', sub: 'Manual folder hunt & missing dependencies' },
      { players: 2, time: '25 min', bar: '55%', note: '"Wait, are you on Fabric 0.15 or 0.16? My client is crashing."', sub: 'Version mismatch & config conflict' },
      { players: 3, time: '35 min', bar: '70%', note: '"Someone downloaded the Forge build instead of Fabric. Redownloading..."', sub: 'Wrong mod loader build downloaded' },
      { players: 4, time: '45 min', bar: '82%', note: '"One friend has Java 17, server needs Java 21. Discord screenshare begins."', sub: 'Java runtime version mismatch' },
      { players: 6, time: '1.2 hrs', bar: '92%', note: '"Zip file from Google Drive exceeded download quota. Uploading to MediaFire..."', sub: 'Broken links & cloud download quotas' },
      { players: 8, time: '2.0 hrs', bar: '97%', note: '"Two friends gave up and logged off. Game night effectively canceled."', sub: 'Friend fatigue & canceled game night' },
      { players: 12, time: '3.5 hrs', bar: '100%', note: '"Nobody is playing. You spent the entire Friday night troubleshooting in Discord."', sub: 'Complete group failure & wasted evening' },
    ];

    const updateBenchmark = (val) => {
      const num = parseInt(val, 10);
      benchmarkPlayerCount.textContent = num === 1 ? '1 Player' : `${num} Players`;

      let closest = scenarios[0];
      for (const s of scenarios) {
        if (num >= s.players) closest = s;
      }

      if (benchmarkManualTime) benchmarkManualTime.textContent = closest.time;
      if (benchmarkManualBar) benchmarkManualBar.style.width = closest.bar;
      if (benchmarkManualSubText) benchmarkManualSubText.textContent = closest.sub;
      if (benchmarkSliderNote) benchmarkSliderNote.textContent = closest.note;
    };

    benchmarkSlider.addEventListener('input', (e) => {
      updateBenchmark(e.target.value);
    });
  }
});

    // Handle browser back/forward and hash changes
    window.addEventListener('hashchange', () => {
      const wanted = window.location.hash.slice(1);
      if (wanted && document.querySelector(`.tab-btn[data-tab="${wanted}"]`)) {
        activateTab(wanted, false);
      }
      // ---------------------------------------------------------------------------
  // 13. Interactive Hero Benchmark Slider
  // ---------------------------------------------------------------------------
  const benchmarkSlider = document.getElementById('benchmarkSlider');
  const benchmarkPlayerCount = document.getElementById('benchmarkPlayerCount');
  const benchmarkManualTime = document.getElementById('benchmarkManualTime');
  const benchmarkManualBar = document.getElementById('benchmarkManualBar');
  const benchmarkManualSubText = document.getElementById('benchmarkManualSubText');
  const benchmarkSliderNote = document.getElementById('benchmarkSliderNote');

  if (benchmarkSlider && benchmarkPlayerCount) {
    const scenarios = [
      { players: 1, time: '12 min', bar: '35%', note: '"Missing dependency cloth-config. Why won\'t this jar load?"', sub: 'Manual folder hunt & missing dependencies' },
      { players: 2, time: '25 min', bar: '55%', note: '"Wait, are you on Fabric 0.15 or 0.16? My client is crashing."', sub: 'Version mismatch & config conflict' },
      { players: 3, time: '35 min', bar: '70%', note: '"Someone downloaded the Forge build instead of Fabric. Redownloading..."', sub: 'Wrong mod loader build downloaded' },
      { players: 4, time: '45 min', bar: '82%', note: '"One friend has Java 17, server needs Java 21. Discord screenshare begins."', sub: 'Java runtime version mismatch' },
      { players: 6, time: '1.2 hrs', bar: '92%', note: '"Zip file from Google Drive exceeded download quota. Uploading to MediaFire..."', sub: 'Broken links & cloud download quotas' },
      { players: 8, time: '2.0 hrs', bar: '97%', note: '"Two friends gave up and logged off. Game night effectively canceled."', sub: 'Friend fatigue & canceled game night' },
      { players: 12, time: '3.5 hrs', bar: '100%', note: '"Nobody is playing. You spent the entire Friday night troubleshooting in Discord."', sub: 'Complete group failure & wasted evening' },
    ];

    const updateBenchmark = (val) => {
      const num = parseInt(val, 10);
      benchmarkPlayerCount.textContent = num === 1 ? '1 Player' : `${num} Players`;

      let closest = scenarios[0];
      for (const s of scenarios) {
        if (num >= s.players) closest = s;
      }

      if (benchmarkManualTime) benchmarkManualTime.textContent = closest.time;
      if (benchmarkManualBar) benchmarkManualBar.style.width = closest.bar;
      if (benchmarkManualSubText) benchmarkManualSubText.textContent = closest.sub;
      if (benchmarkSliderNote) benchmarkSliderNote.textContent = closest.note;
    };

    benchmarkSlider.addEventListener('input', (e) => {
      updateBenchmark(e.target.value);
    });
  }
});

    // Honour hash on load (#server-owners or #players), else default to first tab
    const wanted = window.location.hash.slice(1);
    const initial = wanted && document.querySelector(`.tab-btn[data-tab="${wanted}"]`)
      ? wanted
      : (tabButtons[0]?.dataset.tab || 'players');
    activateTab(initial, false);
  }

  // ---------------------------------------------------------------------------
  // 4. Hero Showcase Tabs & Auto-advance (NO window scrolling)
  // ---------------------------------------------------------------------------
  const showcaseTabs = document.querySelectorAll('.showcase-tab');
  const showcaseViews = document.querySelectorAll('.showcase-view');
  const frameTitleText = document.getElementById('frameTitleText');
  const showcaseContainer = document.querySelector('.showcase-container');

  if (showcaseTabs.length && showcaseViews.length) {
    let currentShowcaseIndex = 0;
    let showcaseTimer = null;

    const activateShowcase = (index) => {
      currentShowcaseIndex = (index + showcaseTabs.length) % showcaseTabs.length;
      const tab = showcaseTabs[currentShowcaseIndex];
      const viewId = tab.dataset.view;
      const title = tab.dataset.title;

      showcaseTabs.forEach((t, i) => t.classList.toggle('active', i === currentShowcaseIndex));
      showcaseViews.forEach((v) => v.classList.toggle('active', v.dataset.view === viewId));
      if (frameTitleText && title) {
        frameTitleText.textContent = title;
      }
    };

    showcaseTabs.forEach((tab, index) => {
      tab.addEventListener('click', () => {
        activateShowcase(index);
        resetShowcaseTimer();
        // ---------------------------------------------------------------------------
  // 13. Interactive Hero Benchmark Slider
  // ---------------------------------------------------------------------------
  const benchmarkSlider = document.getElementById('benchmarkSlider');
  const benchmarkPlayerCount = document.getElementById('benchmarkPlayerCount');
  const benchmarkManualTime = document.getElementById('benchmarkManualTime');
  const benchmarkManualBar = document.getElementById('benchmarkManualBar');
  const benchmarkManualSubText = document.getElementById('benchmarkManualSubText');
  const benchmarkSliderNote = document.getElementById('benchmarkSliderNote');

  if (benchmarkSlider && benchmarkPlayerCount) {
    const scenarios = [
      { players: 1, time: '12 min', bar: '35%', note: '"Missing dependency cloth-config. Why won\'t this jar load?"', sub: 'Manual folder hunt & missing dependencies' },
      { players: 2, time: '25 min', bar: '55%', note: '"Wait, are you on Fabric 0.15 or 0.16? My client is crashing."', sub: 'Version mismatch & config conflict' },
      { players: 3, time: '35 min', bar: '70%', note: '"Someone downloaded the Forge build instead of Fabric. Redownloading..."', sub: 'Wrong mod loader build downloaded' },
      { players: 4, time: '45 min', bar: '82%', note: '"One friend has Java 17, server needs Java 21. Discord screenshare begins."', sub: 'Java runtime version mismatch' },
      { players: 6, time: '1.2 hrs', bar: '92%', note: '"Zip file from Google Drive exceeded download quota. Uploading to MediaFire..."', sub: 'Broken links & cloud download quotas' },
      { players: 8, time: '2.0 hrs', bar: '97%', note: '"Two friends gave up and logged off. Game night effectively canceled."', sub: 'Friend fatigue & canceled game night' },
      { players: 12, time: '3.5 hrs', bar: '100%', note: '"Nobody is playing. You spent the entire Friday night troubleshooting in Discord."', sub: 'Complete group failure & wasted evening' },
    ];

    const updateBenchmark = (val) => {
      const num = parseInt(val, 10);
      benchmarkPlayerCount.textContent = num === 1 ? '1 Player' : `${num} Players`;

      let closest = scenarios[0];
      for (const s of scenarios) {
        if (num >= s.players) closest = s;
      }

      if (benchmarkManualTime) benchmarkManualTime.textContent = closest.time;
      if (benchmarkManualBar) benchmarkManualBar.style.width = closest.bar;
      if (benchmarkManualSubText) benchmarkManualSubText.textContent = closest.sub;
      if (benchmarkSliderNote) benchmarkSliderNote.textContent = closest.note;
    };

    benchmarkSlider.addEventListener('input', (e) => {
      updateBenchmark(e.target.value);
    });
  }
});
      // ---------------------------------------------------------------------------
  // 13. Interactive Hero Benchmark Slider
  // ---------------------------------------------------------------------------
  const benchmarkSlider = document.getElementById('benchmarkSlider');
  const benchmarkPlayerCount = document.getElementById('benchmarkPlayerCount');
  const benchmarkManualTime = document.getElementById('benchmarkManualTime');
  const benchmarkManualBar = document.getElementById('benchmarkManualBar');
  const benchmarkManualSubText = document.getElementById('benchmarkManualSubText');
  const benchmarkSliderNote = document.getElementById('benchmarkSliderNote');

  if (benchmarkSlider && benchmarkPlayerCount) {
    const scenarios = [
      { players: 1, time: '12 min', bar: '35%', note: '"Missing dependency cloth-config. Why won\'t this jar load?"', sub: 'Manual folder hunt & missing dependencies' },
      { players: 2, time: '25 min', bar: '55%', note: '"Wait, are you on Fabric 0.15 or 0.16? My client is crashing."', sub: 'Version mismatch & config conflict' },
      { players: 3, time: '35 min', bar: '70%', note: '"Someone downloaded the Forge build instead of Fabric. Redownloading..."', sub: 'Wrong mod loader build downloaded' },
      { players: 4, time: '45 min', bar: '82%', note: '"One friend has Java 17, server needs Java 21. Discord screenshare begins."', sub: 'Java runtime version mismatch' },
      { players: 6, time: '1.2 hrs', bar: '92%', note: '"Zip file from Google Drive exceeded download quota. Uploading to MediaFire..."', sub: 'Broken links & cloud download quotas' },
      { players: 8, time: '2.0 hrs', bar: '97%', note: '"Two friends gave up and logged off. Game night effectively canceled."', sub: 'Friend fatigue & canceled game night' },
      { players: 12, time: '3.5 hrs', bar: '100%', note: '"Nobody is playing. You spent the entire Friday night troubleshooting in Discord."', sub: 'Complete group failure & wasted evening' },
    ];

    const updateBenchmark = (val) => {
      const num = parseInt(val, 10);
      benchmarkPlayerCount.textContent = num === 1 ? '1 Player' : `${num} Players`;

      let closest = scenarios[0];
      for (const s of scenarios) {
        if (num >= s.players) closest = s;
      }

      if (benchmarkManualTime) benchmarkManualTime.textContent = closest.time;
      if (benchmarkManualBar) benchmarkManualBar.style.width = closest.bar;
      if (benchmarkManualSubText) benchmarkManualSubText.textContent = closest.sub;
      if (benchmarkSliderNote) benchmarkSliderNote.textContent = closest.note;
    };

    benchmarkSlider.addEventListener('input', (e) => {
      updateBenchmark(e.target.value);
    });
  }
});

    const startShowcaseTimer = () => {
      stopShowcaseTimer();
      showcaseTimer = setInterval(() => {
        activateShowcase(currentShowcaseIndex + 1);
      }, 5500);
    };

    const stopShowcaseTimer = () => {
      if (showcaseTimer) {
        clearInterval(showcaseTimer);
        showcaseTimer = null;
      }
    };

    const resetShowcaseTimer = () => {
      stopShowcaseTimer();
      startShowcaseTimer();
    };

    if (showcaseContainer) {
      showcaseContainer.addEventListener('mouseenter', stopShowcaseTimer);
      showcaseContainer.addEventListener('mouseleave', startShowcaseTimer);
      startShowcaseTimer();
    }
  }

  // ---------------------------------------------------------------------------
  // 5. Interactive Feature Tour / Deck (Home Page)
  // ---------------------------------------------------------------------------
  const deckTabs = document.querySelectorAll('.deck-tab');
  const deckPanels = document.querySelectorAll('.deck-panel');
  if (deckTabs.length && deckPanels.length) {
    deckTabs.forEach((tab, index) => {
      tab.addEventListener('click', () => {
        const targetDeck = tab.dataset.deck;
        deckTabs.forEach((t) => t.classList.toggle('active', t === tab));
        deckPanels.forEach((p) => p.classList.toggle('active', p.dataset.deck === targetDeck));
        // ---------------------------------------------------------------------------
  // 13. Interactive Hero Benchmark Slider
  // ---------------------------------------------------------------------------
  const benchmarkSlider = document.getElementById('benchmarkSlider');
  const benchmarkPlayerCount = document.getElementById('benchmarkPlayerCount');
  const benchmarkManualTime = document.getElementById('benchmarkManualTime');
  const benchmarkManualBar = document.getElementById('benchmarkManualBar');
  const benchmarkManualSubText = document.getElementById('benchmarkManualSubText');
  const benchmarkSliderNote = document.getElementById('benchmarkSliderNote');

  if (benchmarkSlider && benchmarkPlayerCount) {
    const scenarios = [
      { players: 1, time: '12 min', bar: '35%', note: '"Missing dependency cloth-config. Why won\'t this jar load?"', sub: 'Manual folder hunt & missing dependencies' },
      { players: 2, time: '25 min', bar: '55%', note: '"Wait, are you on Fabric 0.15 or 0.16? My client is crashing."', sub: 'Version mismatch & config conflict' },
      { players: 3, time: '35 min', bar: '70%', note: '"Someone downloaded the Forge build instead of Fabric. Redownloading..."', sub: 'Wrong mod loader build downloaded' },
      { players: 4, time: '45 min', bar: '82%', note: '"One friend has Java 17, server needs Java 21. Discord screenshare begins."', sub: 'Java runtime version mismatch' },
      { players: 6, time: '1.2 hrs', bar: '92%', note: '"Zip file from Google Drive exceeded download quota. Uploading to MediaFire..."', sub: 'Broken links & cloud download quotas' },
      { players: 8, time: '2.0 hrs', bar: '97%', note: '"Two friends gave up and logged off. Game night effectively canceled."', sub: 'Friend fatigue & canceled game night' },
      { players: 12, time: '3.5 hrs', bar: '100%', note: '"Nobody is playing. You spent the entire Friday night troubleshooting in Discord."', sub: 'Complete group failure & wasted evening' },
    ];

    const updateBenchmark = (val) => {
      const num = parseInt(val, 10);
      benchmarkPlayerCount.textContent = num === 1 ? '1 Player' : `${num} Players`;

      let closest = scenarios[0];
      for (const s of scenarios) {
        if (num >= s.players) closest = s;
      }

      if (benchmarkManualTime) benchmarkManualTime.textContent = closest.time;
      if (benchmarkManualBar) benchmarkManualBar.style.width = closest.bar;
      if (benchmarkManualSubText) benchmarkManualSubText.textContent = closest.sub;
      if (benchmarkSliderNote) benchmarkSliderNote.textContent = closest.note;
    };

    benchmarkSlider.addEventListener('input', (e) => {
      updateBenchmark(e.target.value);
    });
  }
});
      // ---------------------------------------------------------------------------
  // 13. Interactive Hero Benchmark Slider
  // ---------------------------------------------------------------------------
  const benchmarkSlider = document.getElementById('benchmarkSlider');
  const benchmarkPlayerCount = document.getElementById('benchmarkPlayerCount');
  const benchmarkManualTime = document.getElementById('benchmarkManualTime');
  const benchmarkManualBar = document.getElementById('benchmarkManualBar');
  const benchmarkManualSubText = document.getElementById('benchmarkManualSubText');
  const benchmarkSliderNote = document.getElementById('benchmarkSliderNote');

  if (benchmarkSlider && benchmarkPlayerCount) {
    const scenarios = [
      { players: 1, time: '12 min', bar: '35%', note: '"Missing dependency cloth-config. Why won\'t this jar load?"', sub: 'Manual folder hunt & missing dependencies' },
      { players: 2, time: '25 min', bar: '55%', note: '"Wait, are you on Fabric 0.15 or 0.16? My client is crashing."', sub: 'Version mismatch & config conflict' },
      { players: 3, time: '35 min', bar: '70%', note: '"Someone downloaded the Forge build instead of Fabric. Redownloading..."', sub: 'Wrong mod loader build downloaded' },
      { players: 4, time: '45 min', bar: '82%', note: '"One friend has Java 17, server needs Java 21. Discord screenshare begins."', sub: 'Java runtime version mismatch' },
      { players: 6, time: '1.2 hrs', bar: '92%', note: '"Zip file from Google Drive exceeded download quota. Uploading to MediaFire..."', sub: 'Broken links & cloud download quotas' },
      { players: 8, time: '2.0 hrs', bar: '97%', note: '"Two friends gave up and logged off. Game night effectively canceled."', sub: 'Friend fatigue & canceled game night' },
      { players: 12, time: '3.5 hrs', bar: '100%', note: '"Nobody is playing. You spent the entire Friday night troubleshooting in Discord."', sub: 'Complete group failure & wasted evening' },
    ];

    const updateBenchmark = (val) => {
      const num = parseInt(val, 10);
      benchmarkPlayerCount.textContent = num === 1 ? '1 Player' : `${num} Players`;

      let closest = scenarios[0];
      for (const s of scenarios) {
        if (num >= s.players) closest = s;
      }

      if (benchmarkManualTime) benchmarkManualTime.textContent = closest.time;
      if (benchmarkManualBar) benchmarkManualBar.style.width = closest.bar;
      if (benchmarkManualSubText) benchmarkManualSubText.textContent = closest.sub;
      if (benchmarkSliderNote) benchmarkSliderNote.textContent = closest.note;
    };

    benchmarkSlider.addEventListener('input', (e) => {
      updateBenchmark(e.target.value);
    });
  }
});
  }

  // ---------------------------------------------------------------------------
  // 6. Interactive Carousels (Fixed: NEVER scroll the window when image changes)
  // ---------------------------------------------------------------------------
  document.querySelectorAll('.carousel').forEach((carousel) => {
    const slides = carousel.querySelectorAll('.carousel-slide');
    const dots = carousel.querySelectorAll('.carousel-dot');
    const thumbs = carousel.querySelectorAll('.carousel-thumb');
    const thumbsContainer = carousel.querySelector('.carousel-thumbs');
    const prevBtn = carousel.querySelector('.carousel-prev');
    const nextBtn = carousel.querySelector('.carousel-next');
    let currentIndex = 0;
    let autoTimer = null;

    const showSlide = (index) => {
      currentIndex = (index + slides.length) % slides.length;
      slides.forEach((slide, i) => slide.classList.toggle('active', i === currentIndex));
      dots.forEach((dot, i) => dot.classList.toggle('active', i === currentIndex));

      // Scroll ONLY the internal horizontal thumbnail container, NEVER the page window
      thumbs.forEach((thumb, i) => {
        const isActive = i === currentIndex;
        thumb.classList.toggle('active', isActive);
        if (isActive && thumbsContainer) {
          const thumbLeft = thumb.offsetLeft;
          const containerWidth = thumbsContainer.clientWidth;
          const thumbWidth = thumb.clientWidth;
          thumbsContainer.scrollTo({
            left: thumbLeft - (containerWidth / 2) + (thumbWidth / 2),
            behavior: 'smooth'
            // ---------------------------------------------------------------------------
  // 13. Interactive Hero Benchmark Slider
  // ---------------------------------------------------------------------------
  const benchmarkSlider = document.getElementById('benchmarkSlider');
  const benchmarkPlayerCount = document.getElementById('benchmarkPlayerCount');
  const benchmarkManualTime = document.getElementById('benchmarkManualTime');
  const benchmarkManualBar = document.getElementById('benchmarkManualBar');
  const benchmarkManualSubText = document.getElementById('benchmarkManualSubText');
  const benchmarkSliderNote = document.getElementById('benchmarkSliderNote');

  if (benchmarkSlider && benchmarkPlayerCount) {
    const scenarios = [
      { players: 1, time: '12 min', bar: '35%', note: '"Missing dependency cloth-config. Why won\'t this jar load?"', sub: 'Manual folder hunt & missing dependencies' },
      { players: 2, time: '25 min', bar: '55%', note: '"Wait, are you on Fabric 0.15 or 0.16? My client is crashing."', sub: 'Version mismatch & config conflict' },
      { players: 3, time: '35 min', bar: '70%', note: '"Someone downloaded the Forge build instead of Fabric. Redownloading..."', sub: 'Wrong mod loader build downloaded' },
      { players: 4, time: '45 min', bar: '82%', note: '"One friend has Java 17, server needs Java 21. Discord screenshare begins."', sub: 'Java runtime version mismatch' },
      { players: 6, time: '1.2 hrs', bar: '92%', note: '"Zip file from Google Drive exceeded download quota. Uploading to MediaFire..."', sub: 'Broken links & cloud download quotas' },
      { players: 8, time: '2.0 hrs', bar: '97%', note: '"Two friends gave up and logged off. Game night effectively canceled."', sub: 'Friend fatigue & canceled game night' },
      { players: 12, time: '3.5 hrs', bar: '100%', note: '"Nobody is playing. You spent the entire Friday night troubleshooting in Discord."', sub: 'Complete group failure & wasted evening' },
    ];

    const updateBenchmark = (val) => {
      const num = parseInt(val, 10);
      benchmarkPlayerCount.textContent = num === 1 ? '1 Player' : `${num} Players`;

      let closest = scenarios[0];
      for (const s of scenarios) {
        if (num >= s.players) closest = s;
      }

      if (benchmarkManualTime) benchmarkManualTime.textContent = closest.time;
      if (benchmarkManualBar) benchmarkManualBar.style.width = closest.bar;
      if (benchmarkManualSubText) benchmarkManualSubText.textContent = closest.sub;
      if (benchmarkSliderNote) benchmarkSliderNote.textContent = closest.note;
    };

    benchmarkSlider.addEventListener('input', (e) => {
      updateBenchmark(e.target.value);
    });
  }
});
        }
        // ---------------------------------------------------------------------------
  // 13. Interactive Hero Benchmark Slider
  // ---------------------------------------------------------------------------
  const benchmarkSlider = document.getElementById('benchmarkSlider');
  const benchmarkPlayerCount = document.getElementById('benchmarkPlayerCount');
  const benchmarkManualTime = document.getElementById('benchmarkManualTime');
  const benchmarkManualBar = document.getElementById('benchmarkManualBar');
  const benchmarkManualSubText = document.getElementById('benchmarkManualSubText');
  const benchmarkSliderNote = document.getElementById('benchmarkSliderNote');

  if (benchmarkSlider && benchmarkPlayerCount) {
    const scenarios = [
      { players: 1, time: '12 min', bar: '35%', note: '"Missing dependency cloth-config. Why won\'t this jar load?"', sub: 'Manual folder hunt & missing dependencies' },
      { players: 2, time: '25 min', bar: '55%', note: '"Wait, are you on Fabric 0.15 or 0.16? My client is crashing."', sub: 'Version mismatch & config conflict' },
      { players: 3, time: '35 min', bar: '70%', note: '"Someone downloaded the Forge build instead of Fabric. Redownloading..."', sub: 'Wrong mod loader build downloaded' },
      { players: 4, time: '45 min', bar: '82%', note: '"One friend has Java 17, server needs Java 21. Discord screenshare begins."', sub: 'Java runtime version mismatch' },
      { players: 6, time: '1.2 hrs', bar: '92%', note: '"Zip file from Google Drive exceeded download quota. Uploading to MediaFire..."', sub: 'Broken links & cloud download quotas' },
      { players: 8, time: '2.0 hrs', bar: '97%', note: '"Two friends gave up and logged off. Game night effectively canceled."', sub: 'Friend fatigue & canceled game night' },
      { players: 12, time: '3.5 hrs', bar: '100%', note: '"Nobody is playing. You spent the entire Friday night troubleshooting in Discord."', sub: 'Complete group failure & wasted evening' },
    ];

    const updateBenchmark = (val) => {
      const num = parseInt(val, 10);
      benchmarkPlayerCount.textContent = num === 1 ? '1 Player' : `${num} Players`;

      let closest = scenarios[0];
      for (const s of scenarios) {
        if (num >= s.players) closest = s;
      }

      if (benchmarkManualTime) benchmarkManualTime.textContent = closest.time;
      if (benchmarkManualBar) benchmarkManualBar.style.width = closest.bar;
      if (benchmarkManualSubText) benchmarkManualSubText.textContent = closest.sub;
      if (benchmarkSliderNote) benchmarkSliderNote.textContent = closest.note;
    };

    benchmarkSlider.addEventListener('input', (e) => {
      updateBenchmark(e.target.value);
    });
  }
});
    };

    if (prevBtn) {
      prevBtn.addEventListener('click', (e) => {
        e.preventDefault();
        showSlide(currentIndex - 1);
        resetAutoTimer();
        // ---------------------------------------------------------------------------
  // 13. Interactive Hero Benchmark Slider
  // ---------------------------------------------------------------------------
  const benchmarkSlider = document.getElementById('benchmarkSlider');
  const benchmarkPlayerCount = document.getElementById('benchmarkPlayerCount');
  const benchmarkManualTime = document.getElementById('benchmarkManualTime');
  const benchmarkManualBar = document.getElementById('benchmarkManualBar');
  const benchmarkManualSubText = document.getElementById('benchmarkManualSubText');
  const benchmarkSliderNote = document.getElementById('benchmarkSliderNote');

  if (benchmarkSlider && benchmarkPlayerCount) {
    const scenarios = [
      { players: 1, time: '12 min', bar: '35%', note: '"Missing dependency cloth-config. Why won\'t this jar load?"', sub: 'Manual folder hunt & missing dependencies' },
      { players: 2, time: '25 min', bar: '55%', note: '"Wait, are you on Fabric 0.15 or 0.16? My client is crashing."', sub: 'Version mismatch & config conflict' },
      { players: 3, time: '35 min', bar: '70%', note: '"Someone downloaded the Forge build instead of Fabric. Redownloading..."', sub: 'Wrong mod loader build downloaded' },
      { players: 4, time: '45 min', bar: '82%', note: '"One friend has Java 17, server needs Java 21. Discord screenshare begins."', sub: 'Java runtime version mismatch' },
      { players: 6, time: '1.2 hrs', bar: '92%', note: '"Zip file from Google Drive exceeded download quota. Uploading to MediaFire..."', sub: 'Broken links & cloud download quotas' },
      { players: 8, time: '2.0 hrs', bar: '97%', note: '"Two friends gave up and logged off. Game night effectively canceled."', sub: 'Friend fatigue & canceled game night' },
      { players: 12, time: '3.5 hrs', bar: '100%', note: '"Nobody is playing. You spent the entire Friday night troubleshooting in Discord."', sub: 'Complete group failure & wasted evening' },
    ];

    const updateBenchmark = (val) => {
      const num = parseInt(val, 10);
      benchmarkPlayerCount.textContent = num === 1 ? '1 Player' : `${num} Players`;

      let closest = scenarios[0];
      for (const s of scenarios) {
        if (num >= s.players) closest = s;
      }

      if (benchmarkManualTime) benchmarkManualTime.textContent = closest.time;
      if (benchmarkManualBar) benchmarkManualBar.style.width = closest.bar;
      if (benchmarkManualSubText) benchmarkManualSubText.textContent = closest.sub;
      if (benchmarkSliderNote) benchmarkSliderNote.textContent = closest.note;
    };

    benchmarkSlider.addEventListener('input', (e) => {
      updateBenchmark(e.target.value);
    });
  }
});
    }
    if (nextBtn) {
      nextBtn.addEventListener('click', (e) => {
        e.preventDefault();
        showSlide(currentIndex + 1);
        resetAutoTimer();
        // ---------------------------------------------------------------------------
  // 13. Interactive Hero Benchmark Slider
  // ---------------------------------------------------------------------------
  const benchmarkSlider = document.getElementById('benchmarkSlider');
  const benchmarkPlayerCount = document.getElementById('benchmarkPlayerCount');
  const benchmarkManualTime = document.getElementById('benchmarkManualTime');
  const benchmarkManualBar = document.getElementById('benchmarkManualBar');
  const benchmarkManualSubText = document.getElementById('benchmarkManualSubText');
  const benchmarkSliderNote = document.getElementById('benchmarkSliderNote');

  if (benchmarkSlider && benchmarkPlayerCount) {
    const scenarios = [
      { players: 1, time: '12 min', bar: '35%', note: '"Missing dependency cloth-config. Why won\'t this jar load?"', sub: 'Manual folder hunt & missing dependencies' },
      { players: 2, time: '25 min', bar: '55%', note: '"Wait, are you on Fabric 0.15 or 0.16? My client is crashing."', sub: 'Version mismatch & config conflict' },
      { players: 3, time: '35 min', bar: '70%', note: '"Someone downloaded the Forge build instead of Fabric. Redownloading..."', sub: 'Wrong mod loader build downloaded' },
      { players: 4, time: '45 min', bar: '82%', note: '"One friend has Java 17, server needs Java 21. Discord screenshare begins."', sub: 'Java runtime version mismatch' },
      { players: 6, time: '1.2 hrs', bar: '92%', note: '"Zip file from Google Drive exceeded download quota. Uploading to MediaFire..."', sub: 'Broken links & cloud download quotas' },
      { players: 8, time: '2.0 hrs', bar: '97%', note: '"Two friends gave up and logged off. Game night effectively canceled."', sub: 'Friend fatigue & canceled game night' },
      { players: 12, time: '3.5 hrs', bar: '100%', note: '"Nobody is playing. You spent the entire Friday night troubleshooting in Discord."', sub: 'Complete group failure & wasted evening' },
    ];

    const updateBenchmark = (val) => {
      const num = parseInt(val, 10);
      benchmarkPlayerCount.textContent = num === 1 ? '1 Player' : `${num} Players`;

      let closest = scenarios[0];
      for (const s of scenarios) {
        if (num >= s.players) closest = s;
      }

      if (benchmarkManualTime) benchmarkManualTime.textContent = closest.time;
      if (benchmarkManualBar) benchmarkManualBar.style.width = closest.bar;
      if (benchmarkManualSubText) benchmarkManualSubText.textContent = closest.sub;
      if (benchmarkSliderNote) benchmarkSliderNote.textContent = closest.note;
    };

    benchmarkSlider.addEventListener('input', (e) => {
      updateBenchmark(e.target.value);
    });
  }
});
    }
    dots.forEach((dot, i) => {
      dot.addEventListener('click', (e) => {
        e.preventDefault();
        showSlide(i);
        resetAutoTimer();
        // ---------------------------------------------------------------------------
  // 13. Interactive Hero Benchmark Slider
  // ---------------------------------------------------------------------------
  const benchmarkSlider = document.getElementById('benchmarkSlider');
  const benchmarkPlayerCount = document.getElementById('benchmarkPlayerCount');
  const benchmarkManualTime = document.getElementById('benchmarkManualTime');
  const benchmarkManualBar = document.getElementById('benchmarkManualBar');
  const benchmarkManualSubText = document.getElementById('benchmarkManualSubText');
  const benchmarkSliderNote = document.getElementById('benchmarkSliderNote');

  if (benchmarkSlider && benchmarkPlayerCount) {
    const scenarios = [
      { players: 1, time: '12 min', bar: '35%', note: '"Missing dependency cloth-config. Why won\'t this jar load?"', sub: 'Manual folder hunt & missing dependencies' },
      { players: 2, time: '25 min', bar: '55%', note: '"Wait, are you on Fabric 0.15 or 0.16? My client is crashing."', sub: 'Version mismatch & config conflict' },
      { players: 3, time: '35 min', bar: '70%', note: '"Someone downloaded the Forge build instead of Fabric. Redownloading..."', sub: 'Wrong mod loader build downloaded' },
      { players: 4, time: '45 min', bar: '82%', note: '"One friend has Java 17, server needs Java 21. Discord screenshare begins."', sub: 'Java runtime version mismatch' },
      { players: 6, time: '1.2 hrs', bar: '92%', note: '"Zip file from Google Drive exceeded download quota. Uploading to MediaFire..."', sub: 'Broken links & cloud download quotas' },
      { players: 8, time: '2.0 hrs', bar: '97%', note: '"Two friends gave up and logged off. Game night effectively canceled."', sub: 'Friend fatigue & canceled game night' },
      { players: 12, time: '3.5 hrs', bar: '100%', note: '"Nobody is playing. You spent the entire Friday night troubleshooting in Discord."', sub: 'Complete group failure & wasted evening' },
    ];

    const updateBenchmark = (val) => {
      const num = parseInt(val, 10);
      benchmarkPlayerCount.textContent = num === 1 ? '1 Player' : `${num} Players`;

      let closest = scenarios[0];
      for (const s of scenarios) {
        if (num >= s.players) closest = s;
      }

      if (benchmarkManualTime) benchmarkManualTime.textContent = closest.time;
      if (benchmarkManualBar) benchmarkManualBar.style.width = closest.bar;
      if (benchmarkManualSubText) benchmarkManualSubText.textContent = closest.sub;
      if (benchmarkSliderNote) benchmarkSliderNote.textContent = closest.note;
    };

    benchmarkSlider.addEventListener('input', (e) => {
      updateBenchmark(e.target.value);
    });
  }
});
      // ---------------------------------------------------------------------------
  // 13. Interactive Hero Benchmark Slider
  // ---------------------------------------------------------------------------
  const benchmarkSlider = document.getElementById('benchmarkSlider');
  const benchmarkPlayerCount = document.getElementById('benchmarkPlayerCount');
  const benchmarkManualTime = document.getElementById('benchmarkManualTime');
  const benchmarkManualBar = document.getElementById('benchmarkManualBar');
  const benchmarkManualSubText = document.getElementById('benchmarkManualSubText');
  const benchmarkSliderNote = document.getElementById('benchmarkSliderNote');

  if (benchmarkSlider && benchmarkPlayerCount) {
    const scenarios = [
      { players: 1, time: '12 min', bar: '35%', note: '"Missing dependency cloth-config. Why won\'t this jar load?"', sub: 'Manual folder hunt & missing dependencies' },
      { players: 2, time: '25 min', bar: '55%', note: '"Wait, are you on Fabric 0.15 or 0.16? My client is crashing."', sub: 'Version mismatch & config conflict' },
      { players: 3, time: '35 min', bar: '70%', note: '"Someone downloaded the Forge build instead of Fabric. Redownloading..."', sub: 'Wrong mod loader build downloaded' },
      { players: 4, time: '45 min', bar: '82%', note: '"One friend has Java 17, server needs Java 21. Discord screenshare begins."', sub: 'Java runtime version mismatch' },
      { players: 6, time: '1.2 hrs', bar: '92%', note: '"Zip file from Google Drive exceeded download quota. Uploading to MediaFire..."', sub: 'Broken links & cloud download quotas' },
      { players: 8, time: '2.0 hrs', bar: '97%', note: '"Two friends gave up and logged off. Game night effectively canceled."', sub: 'Friend fatigue & canceled game night' },
      { players: 12, time: '3.5 hrs', bar: '100%', note: '"Nobody is playing. You spent the entire Friday night troubleshooting in Discord."', sub: 'Complete group failure & wasted evening' },
    ];

    const updateBenchmark = (val) => {
      const num = parseInt(val, 10);
      benchmarkPlayerCount.textContent = num === 1 ? '1 Player' : `${num} Players`;

      let closest = scenarios[0];
      for (const s of scenarios) {
        if (num >= s.players) closest = s;
      }

      if (benchmarkManualTime) benchmarkManualTime.textContent = closest.time;
      if (benchmarkManualBar) benchmarkManualBar.style.width = closest.bar;
      if (benchmarkManualSubText) benchmarkManualSubText.textContent = closest.sub;
      if (benchmarkSliderNote) benchmarkSliderNote.textContent = closest.note;
    };

    benchmarkSlider.addEventListener('input', (e) => {
      updateBenchmark(e.target.value);
    });
  }
});
    thumbs.forEach((thumb, i) => {
      thumb.addEventListener('click', (e) => {
        e.preventDefault();
        showSlide(i);
        resetAutoTimer();
        // ---------------------------------------------------------------------------
  // 13. Interactive Hero Benchmark Slider
  // ---------------------------------------------------------------------------
  const benchmarkSlider = document.getElementById('benchmarkSlider');
  const benchmarkPlayerCount = document.getElementById('benchmarkPlayerCount');
  const benchmarkManualTime = document.getElementById('benchmarkManualTime');
  const benchmarkManualBar = document.getElementById('benchmarkManualBar');
  const benchmarkManualSubText = document.getElementById('benchmarkManualSubText');
  const benchmarkSliderNote = document.getElementById('benchmarkSliderNote');

  if (benchmarkSlider && benchmarkPlayerCount) {
    const scenarios = [
      { players: 1, time: '12 min', bar: '35%', note: '"Missing dependency cloth-config. Why won\'t this jar load?"', sub: 'Manual folder hunt & missing dependencies' },
      { players: 2, time: '25 min', bar: '55%', note: '"Wait, are you on Fabric 0.15 or 0.16? My client is crashing."', sub: 'Version mismatch & config conflict' },
      { players: 3, time: '35 min', bar: '70%', note: '"Someone downloaded the Forge build instead of Fabric. Redownloading..."', sub: 'Wrong mod loader build downloaded' },
      { players: 4, time: '45 min', bar: '82%', note: '"One friend has Java 17, server needs Java 21. Discord screenshare begins."', sub: 'Java runtime version mismatch' },
      { players: 6, time: '1.2 hrs', bar: '92%', note: '"Zip file from Google Drive exceeded download quota. Uploading to MediaFire..."', sub: 'Broken links & cloud download quotas' },
      { players: 8, time: '2.0 hrs', bar: '97%', note: '"Two friends gave up and logged off. Game night effectively canceled."', sub: 'Friend fatigue & canceled game night' },
      { players: 12, time: '3.5 hrs', bar: '100%', note: '"Nobody is playing. You spent the entire Friday night troubleshooting in Discord."', sub: 'Complete group failure & wasted evening' },
    ];

    const updateBenchmark = (val) => {
      const num = parseInt(val, 10);
      benchmarkPlayerCount.textContent = num === 1 ? '1 Player' : `${num} Players`;

      let closest = scenarios[0];
      for (const s of scenarios) {
        if (num >= s.players) closest = s;
      }

      if (benchmarkManualTime) benchmarkManualTime.textContent = closest.time;
      if (benchmarkManualBar) benchmarkManualBar.style.width = closest.bar;
      if (benchmarkManualSubText) benchmarkManualSubText.textContent = closest.sub;
      if (benchmarkSliderNote) benchmarkSliderNote.textContent = closest.note;
    };

    benchmarkSlider.addEventListener('input', (e) => {
      updateBenchmark(e.target.value);
    });
  }
});
      // ---------------------------------------------------------------------------
  // 13. Interactive Hero Benchmark Slider
  // ---------------------------------------------------------------------------
  const benchmarkSlider = document.getElementById('benchmarkSlider');
  const benchmarkPlayerCount = document.getElementById('benchmarkPlayerCount');
  const benchmarkManualTime = document.getElementById('benchmarkManualTime');
  const benchmarkManualBar = document.getElementById('benchmarkManualBar');
  const benchmarkManualSubText = document.getElementById('benchmarkManualSubText');
  const benchmarkSliderNote = document.getElementById('benchmarkSliderNote');

  if (benchmarkSlider && benchmarkPlayerCount) {
    const scenarios = [
      { players: 1, time: '12 min', bar: '35%', note: '"Missing dependency cloth-config. Why won\'t this jar load?"', sub: 'Manual folder hunt & missing dependencies' },
      { players: 2, time: '25 min', bar: '55%', note: '"Wait, are you on Fabric 0.15 or 0.16? My client is crashing."', sub: 'Version mismatch & config conflict' },
      { players: 3, time: '35 min', bar: '70%', note: '"Someone downloaded the Forge build instead of Fabric. Redownloading..."', sub: 'Wrong mod loader build downloaded' },
      { players: 4, time: '45 min', bar: '82%', note: '"One friend has Java 17, server needs Java 21. Discord screenshare begins."', sub: 'Java runtime version mismatch' },
      { players: 6, time: '1.2 hrs', bar: '92%', note: '"Zip file from Google Drive exceeded download quota. Uploading to MediaFire..."', sub: 'Broken links & cloud download quotas' },
      { players: 8, time: '2.0 hrs', bar: '97%', note: '"Two friends gave up and logged off. Game night effectively canceled."', sub: 'Friend fatigue & canceled game night' },
      { players: 12, time: '3.5 hrs', bar: '100%', note: '"Nobody is playing. You spent the entire Friday night troubleshooting in Discord."', sub: 'Complete group failure & wasted evening' },
    ];

    const updateBenchmark = (val) => {
      const num = parseInt(val, 10);
      benchmarkPlayerCount.textContent = num === 1 ? '1 Player' : `${num} Players`;

      let closest = scenarios[0];
      for (const s of scenarios) {
        if (num >= s.players) closest = s;
      }

      if (benchmarkManualTime) benchmarkManualTime.textContent = closest.time;
      if (benchmarkManualBar) benchmarkManualBar.style.width = closest.bar;
      if (benchmarkManualSubText) benchmarkManualSubText.textContent = closest.sub;
      if (benchmarkSliderNote) benchmarkSliderNote.textContent = closest.note;
    };

    benchmarkSlider.addEventListener('input', (e) => {
      updateBenchmark(e.target.value);
    });
  }
});

    const startAutoTimer = () => {
      if (carousel.dataset.auto !== 'false') {
        stopAutoTimer();
        autoTimer = setInterval(() => {
          showSlide(currentIndex + 1);
        }, 6500);
      }
    };

    const stopAutoTimer = () => {
      if (autoTimer) {
        clearInterval(autoTimer);
        autoTimer = null;
      }
    };

    const resetAutoTimer = () => {
      stopAutoTimer();
      startAutoTimer();
    };

    carousel.addEventListener('mouseenter', stopAutoTimer);
    carousel.addEventListener('mouseleave', startAutoTimer);
    startAutoTimer();
    // ---------------------------------------------------------------------------
  // 13. Interactive Hero Benchmark Slider
  // ---------------------------------------------------------------------------
  const benchmarkSlider = document.getElementById('benchmarkSlider');
  const benchmarkPlayerCount = document.getElementById('benchmarkPlayerCount');
  const benchmarkManualTime = document.getElementById('benchmarkManualTime');
  const benchmarkManualBar = document.getElementById('benchmarkManualBar');
  const benchmarkManualSubText = document.getElementById('benchmarkManualSubText');
  const benchmarkSliderNote = document.getElementById('benchmarkSliderNote');

  if (benchmarkSlider && benchmarkPlayerCount) {
    const scenarios = [
      { players: 1, time: '12 min', bar: '35%', note: '"Missing dependency cloth-config. Why won\'t this jar load?"', sub: 'Manual folder hunt & missing dependencies' },
      { players: 2, time: '25 min', bar: '55%', note: '"Wait, are you on Fabric 0.15 or 0.16? My client is crashing."', sub: 'Version mismatch & config conflict' },
      { players: 3, time: '35 min', bar: '70%', note: '"Someone downloaded the Forge build instead of Fabric. Redownloading..."', sub: 'Wrong mod loader build downloaded' },
      { players: 4, time: '45 min', bar: '82%', note: '"One friend has Java 17, server needs Java 21. Discord screenshare begins."', sub: 'Java runtime version mismatch' },
      { players: 6, time: '1.2 hrs', bar: '92%', note: '"Zip file from Google Drive exceeded download quota. Uploading to MediaFire..."', sub: 'Broken links & cloud download quotas' },
      { players: 8, time: '2.0 hrs', bar: '97%', note: '"Two friends gave up and logged off. Game night effectively canceled."', sub: 'Friend fatigue & canceled game night' },
      { players: 12, time: '3.5 hrs', bar: '100%', note: '"Nobody is playing. You spent the entire Friday night troubleshooting in Discord."', sub: 'Complete group failure & wasted evening' },
    ];

    const updateBenchmark = (val) => {
      const num = parseInt(val, 10);
      benchmarkPlayerCount.textContent = num === 1 ? '1 Player' : `${num} Players`;

      let closest = scenarios[0];
      for (const s of scenarios) {
        if (num >= s.players) closest = s;
      }

      if (benchmarkManualTime) benchmarkManualTime.textContent = closest.time;
      if (benchmarkManualBar) benchmarkManualBar.style.width = closest.bar;
      if (benchmarkManualSubText) benchmarkManualSubText.textContent = closest.sub;
      if (benchmarkSliderNote) benchmarkSliderNote.textContent = closest.note;
    };

    benchmarkSlider.addEventListener('input', (e) => {
      updateBenchmark(e.target.value);
    });
  }
});

  // ---------------------------------------------------------------------------
  // 6b. Delight Features Slideshow Handler
  // ---------------------------------------------------------------------------
  const delightSlideshow = document.querySelector('.delight-slideshow');
  if (delightSlideshow) {
    const navBtns = delightSlideshow.querySelectorAll('.delight-nav-btn');
    const slides = delightSlideshow.querySelectorAll('.delight-slide');
    const dots = delightSlideshow.querySelectorAll('.delight-dot');
    const prevBtn = delightSlideshow.querySelector('.delight-prev');
    const nextBtn = delightSlideshow.querySelector('.delight-next');
    let currentIndex = 0;
    let timer = null;

    const showSlide = (idx) => {
      currentIndex = (idx + slides.length) % slides.length;
      slides.forEach((s, i) => s.classList.toggle('active', i === currentIndex));
      navBtns.forEach((b, i) => b.classList.toggle('active', i === currentIndex));
      dots.forEach((d, i) => d.classList.toggle('active', i === currentIndex));
    };

    navBtns.forEach((btn, i) => {
      btn.addEventListener('click', () => {
        showSlide(i);
        resetTimer();
        // ---------------------------------------------------------------------------
  // 13. Interactive Hero Benchmark Slider
  // ---------------------------------------------------------------------------
  const benchmarkSlider = document.getElementById('benchmarkSlider');
  const benchmarkPlayerCount = document.getElementById('benchmarkPlayerCount');
  const benchmarkManualTime = document.getElementById('benchmarkManualTime');
  const benchmarkManualBar = document.getElementById('benchmarkManualBar');
  const benchmarkManualSubText = document.getElementById('benchmarkManualSubText');
  const benchmarkSliderNote = document.getElementById('benchmarkSliderNote');

  if (benchmarkSlider && benchmarkPlayerCount) {
    const scenarios = [
      { players: 1, time: '12 min', bar: '35%', note: '"Missing dependency cloth-config. Why won\'t this jar load?"', sub: 'Manual folder hunt & missing dependencies' },
      { players: 2, time: '25 min', bar: '55%', note: '"Wait, are you on Fabric 0.15 or 0.16? My client is crashing."', sub: 'Version mismatch & config conflict' },
      { players: 3, time: '35 min', bar: '70%', note: '"Someone downloaded the Forge build instead of Fabric. Redownloading..."', sub: 'Wrong mod loader build downloaded' },
      { players: 4, time: '45 min', bar: '82%', note: '"One friend has Java 17, server needs Java 21. Discord screenshare begins."', sub: 'Java runtime version mismatch' },
      { players: 6, time: '1.2 hrs', bar: '92%', note: '"Zip file from Google Drive exceeded download quota. Uploading to MediaFire..."', sub: 'Broken links & cloud download quotas' },
      { players: 8, time: '2.0 hrs', bar: '97%', note: '"Two friends gave up and logged off. Game night effectively canceled."', sub: 'Friend fatigue & canceled game night' },
      { players: 12, time: '3.5 hrs', bar: '100%', note: '"Nobody is playing. You spent the entire Friday night troubleshooting in Discord."', sub: 'Complete group failure & wasted evening' },
    ];

    const updateBenchmark = (val) => {
      const num = parseInt(val, 10);
      benchmarkPlayerCount.textContent = num === 1 ? '1 Player' : `${num} Players`;

      let closest = scenarios[0];
      for (const s of scenarios) {
        if (num >= s.players) closest = s;
      }

      if (benchmarkManualTime) benchmarkManualTime.textContent = closest.time;
      if (benchmarkManualBar) benchmarkManualBar.style.width = closest.bar;
      if (benchmarkManualSubText) benchmarkManualSubText.textContent = closest.sub;
      if (benchmarkSliderNote) benchmarkSliderNote.textContent = closest.note;
    };

    benchmarkSlider.addEventListener('input', (e) => {
      updateBenchmark(e.target.value);
    });
  }
});
      // ---------------------------------------------------------------------------
  // 13. Interactive Hero Benchmark Slider
  // ---------------------------------------------------------------------------
  const benchmarkSlider = document.getElementById('benchmarkSlider');
  const benchmarkPlayerCount = document.getElementById('benchmarkPlayerCount');
  const benchmarkManualTime = document.getElementById('benchmarkManualTime');
  const benchmarkManualBar = document.getElementById('benchmarkManualBar');
  const benchmarkManualSubText = document.getElementById('benchmarkManualSubText');
  const benchmarkSliderNote = document.getElementById('benchmarkSliderNote');

  if (benchmarkSlider && benchmarkPlayerCount) {
    const scenarios = [
      { players: 1, time: '12 min', bar: '35%', note: '"Missing dependency cloth-config. Why won\'t this jar load?"', sub: 'Manual folder hunt & missing dependencies' },
      { players: 2, time: '25 min', bar: '55%', note: '"Wait, are you on Fabric 0.15 or 0.16? My client is crashing."', sub: 'Version mismatch & config conflict' },
      { players: 3, time: '35 min', bar: '70%', note: '"Someone downloaded the Forge build instead of Fabric. Redownloading..."', sub: 'Wrong mod loader build downloaded' },
      { players: 4, time: '45 min', bar: '82%', note: '"One friend has Java 17, server needs Java 21. Discord screenshare begins."', sub: 'Java runtime version mismatch' },
      { players: 6, time: '1.2 hrs', bar: '92%', note: '"Zip file from Google Drive exceeded download quota. Uploading to MediaFire..."', sub: 'Broken links & cloud download quotas' },
      { players: 8, time: '2.0 hrs', bar: '97%', note: '"Two friends gave up and logged off. Game night effectively canceled."', sub: 'Friend fatigue & canceled game night' },
      { players: 12, time: '3.5 hrs', bar: '100%', note: '"Nobody is playing. You spent the entire Friday night troubleshooting in Discord."', sub: 'Complete group failure & wasted evening' },
    ];

    const updateBenchmark = (val) => {
      const num = parseInt(val, 10);
      benchmarkPlayerCount.textContent = num === 1 ? '1 Player' : `${num} Players`;

      let closest = scenarios[0];
      for (const s of scenarios) {
        if (num >= s.players) closest = s;
      }

      if (benchmarkManualTime) benchmarkManualTime.textContent = closest.time;
      if (benchmarkManualBar) benchmarkManualBar.style.width = closest.bar;
      if (benchmarkManualSubText) benchmarkManualSubText.textContent = closest.sub;
      if (benchmarkSliderNote) benchmarkSliderNote.textContent = closest.note;
    };

    benchmarkSlider.addEventListener('input', (e) => {
      updateBenchmark(e.target.value);
    });
  }
});

    dots.forEach((dot, i) => {
      dot.addEventListener('click', () => {
        showSlide(i);
        resetTimer();
        // ---------------------------------------------------------------------------
  // 13. Interactive Hero Benchmark Slider
  // ---------------------------------------------------------------------------
  const benchmarkSlider = document.getElementById('benchmarkSlider');
  const benchmarkPlayerCount = document.getElementById('benchmarkPlayerCount');
  const benchmarkManualTime = document.getElementById('benchmarkManualTime');
  const benchmarkManualBar = document.getElementById('benchmarkManualBar');
  const benchmarkManualSubText = document.getElementById('benchmarkManualSubText');
  const benchmarkSliderNote = document.getElementById('benchmarkSliderNote');

  if (benchmarkSlider && benchmarkPlayerCount) {
    const scenarios = [
      { players: 1, time: '12 min', bar: '35%', note: '"Missing dependency cloth-config. Why won\'t this jar load?"', sub: 'Manual folder hunt & missing dependencies' },
      { players: 2, time: '25 min', bar: '55%', note: '"Wait, are you on Fabric 0.15 or 0.16? My client is crashing."', sub: 'Version mismatch & config conflict' },
      { players: 3, time: '35 min', bar: '70%', note: '"Someone downloaded the Forge build instead of Fabric. Redownloading..."', sub: 'Wrong mod loader build downloaded' },
      { players: 4, time: '45 min', bar: '82%', note: '"One friend has Java 17, server needs Java 21. Discord screenshare begins."', sub: 'Java runtime version mismatch' },
      { players: 6, time: '1.2 hrs', bar: '92%', note: '"Zip file from Google Drive exceeded download quota. Uploading to MediaFire..."', sub: 'Broken links & cloud download quotas' },
      { players: 8, time: '2.0 hrs', bar: '97%', note: '"Two friends gave up and logged off. Game night effectively canceled."', sub: 'Friend fatigue & canceled game night' },
      { players: 12, time: '3.5 hrs', bar: '100%', note: '"Nobody is playing. You spent the entire Friday night troubleshooting in Discord."', sub: 'Complete group failure & wasted evening' },
    ];

    const updateBenchmark = (val) => {
      const num = parseInt(val, 10);
      benchmarkPlayerCount.textContent = num === 1 ? '1 Player' : `${num} Players`;

      let closest = scenarios[0];
      for (const s of scenarios) {
        if (num >= s.players) closest = s;
      }

      if (benchmarkManualTime) benchmarkManualTime.textContent = closest.time;
      if (benchmarkManualBar) benchmarkManualBar.style.width = closest.bar;
      if (benchmarkManualSubText) benchmarkManualSubText.textContent = closest.sub;
      if (benchmarkSliderNote) benchmarkSliderNote.textContent = closest.note;
    };

    benchmarkSlider.addEventListener('input', (e) => {
      updateBenchmark(e.target.value);
    });
  }
});
      // ---------------------------------------------------------------------------
  // 13. Interactive Hero Benchmark Slider
  // ---------------------------------------------------------------------------
  const benchmarkSlider = document.getElementById('benchmarkSlider');
  const benchmarkPlayerCount = document.getElementById('benchmarkPlayerCount');
  const benchmarkManualTime = document.getElementById('benchmarkManualTime');
  const benchmarkManualBar = document.getElementById('benchmarkManualBar');
  const benchmarkManualSubText = document.getElementById('benchmarkManualSubText');
  const benchmarkSliderNote = document.getElementById('benchmarkSliderNote');

  if (benchmarkSlider && benchmarkPlayerCount) {
    const scenarios = [
      { players: 1, time: '12 min', bar: '35%', note: '"Missing dependency cloth-config. Why won\'t this jar load?"', sub: 'Manual folder hunt & missing dependencies' },
      { players: 2, time: '25 min', bar: '55%', note: '"Wait, are you on Fabric 0.15 or 0.16? My client is crashing."', sub: 'Version mismatch & config conflict' },
      { players: 3, time: '35 min', bar: '70%', note: '"Someone downloaded the Forge build instead of Fabric. Redownloading..."', sub: 'Wrong mod loader build downloaded' },
      { players: 4, time: '45 min', bar: '82%', note: '"One friend has Java 17, server needs Java 21. Discord screenshare begins."', sub: 'Java runtime version mismatch' },
      { players: 6, time: '1.2 hrs', bar: '92%', note: '"Zip file from Google Drive exceeded download quota. Uploading to MediaFire..."', sub: 'Broken links & cloud download quotas' },
      { players: 8, time: '2.0 hrs', bar: '97%', note: '"Two friends gave up and logged off. Game night effectively canceled."', sub: 'Friend fatigue & canceled game night' },
      { players: 12, time: '3.5 hrs', bar: '100%', note: '"Nobody is playing. You spent the entire Friday night troubleshooting in Discord."', sub: 'Complete group failure & wasted evening' },
    ];

    const updateBenchmark = (val) => {
      const num = parseInt(val, 10);
      benchmarkPlayerCount.textContent = num === 1 ? '1 Player' : `${num} Players`;

      let closest = scenarios[0];
      for (const s of scenarios) {
        if (num >= s.players) closest = s;
      }

      if (benchmarkManualTime) benchmarkManualTime.textContent = closest.time;
      if (benchmarkManualBar) benchmarkManualBar.style.width = closest.bar;
      if (benchmarkManualSubText) benchmarkManualSubText.textContent = closest.sub;
      if (benchmarkSliderNote) benchmarkSliderNote.textContent = closest.note;
    };

    benchmarkSlider.addEventListener('input', (e) => {
      updateBenchmark(e.target.value);
    });
  }
});

    if (prevBtn) {
      prevBtn.addEventListener('click', (e) => {
        e.preventDefault();
        showSlide(currentIndex - 1);
        resetTimer();
        // ---------------------------------------------------------------------------
  // 13. Interactive Hero Benchmark Slider
  // ---------------------------------------------------------------------------
  const benchmarkSlider = document.getElementById('benchmarkSlider');
  const benchmarkPlayerCount = document.getElementById('benchmarkPlayerCount');
  const benchmarkManualTime = document.getElementById('benchmarkManualTime');
  const benchmarkManualBar = document.getElementById('benchmarkManualBar');
  const benchmarkManualSubText = document.getElementById('benchmarkManualSubText');
  const benchmarkSliderNote = document.getElementById('benchmarkSliderNote');

  if (benchmarkSlider && benchmarkPlayerCount) {
    const scenarios = [
      { players: 1, time: '12 min', bar: '35%', note: '"Missing dependency cloth-config. Why won\'t this jar load?"', sub: 'Manual folder hunt & missing dependencies' },
      { players: 2, time: '25 min', bar: '55%', note: '"Wait, are you on Fabric 0.15 or 0.16? My client is crashing."', sub: 'Version mismatch & config conflict' },
      { players: 3, time: '35 min', bar: '70%', note: '"Someone downloaded the Forge build instead of Fabric. Redownloading..."', sub: 'Wrong mod loader build downloaded' },
      { players: 4, time: '45 min', bar: '82%', note: '"One friend has Java 17, server needs Java 21. Discord screenshare begins."', sub: 'Java runtime version mismatch' },
      { players: 6, time: '1.2 hrs', bar: '92%', note: '"Zip file from Google Drive exceeded download quota. Uploading to MediaFire..."', sub: 'Broken links & cloud download quotas' },
      { players: 8, time: '2.0 hrs', bar: '97%', note: '"Two friends gave up and logged off. Game night effectively canceled."', sub: 'Friend fatigue & canceled game night' },
      { players: 12, time: '3.5 hrs', bar: '100%', note: '"Nobody is playing. You spent the entire Friday night troubleshooting in Discord."', sub: 'Complete group failure & wasted evening' },
    ];

    const updateBenchmark = (val) => {
      const num = parseInt(val, 10);
      benchmarkPlayerCount.textContent = num === 1 ? '1 Player' : `${num} Players`;

      let closest = scenarios[0];
      for (const s of scenarios) {
        if (num >= s.players) closest = s;
      }

      if (benchmarkManualTime) benchmarkManualTime.textContent = closest.time;
      if (benchmarkManualBar) benchmarkManualBar.style.width = closest.bar;
      if (benchmarkManualSubText) benchmarkManualSubText.textContent = closest.sub;
      if (benchmarkSliderNote) benchmarkSliderNote.textContent = closest.note;
    };

    benchmarkSlider.addEventListener('input', (e) => {
      updateBenchmark(e.target.value);
    });
  }
});
    }

    if (nextBtn) {
      nextBtn.addEventListener('click', (e) => {
        e.preventDefault();
        showSlide(currentIndex + 1);
        resetTimer();
        // ---------------------------------------------------------------------------
  // 13. Interactive Hero Benchmark Slider
  // ---------------------------------------------------------------------------
  const benchmarkSlider = document.getElementById('benchmarkSlider');
  const benchmarkPlayerCount = document.getElementById('benchmarkPlayerCount');
  const benchmarkManualTime = document.getElementById('benchmarkManualTime');
  const benchmarkManualBar = document.getElementById('benchmarkManualBar');
  const benchmarkManualSubText = document.getElementById('benchmarkManualSubText');
  const benchmarkSliderNote = document.getElementById('benchmarkSliderNote');

  if (benchmarkSlider && benchmarkPlayerCount) {
    const scenarios = [
      { players: 1, time: '12 min', bar: '35%', note: '"Missing dependency cloth-config. Why won\'t this jar load?"', sub: 'Manual folder hunt & missing dependencies' },
      { players: 2, time: '25 min', bar: '55%', note: '"Wait, are you on Fabric 0.15 or 0.16? My client is crashing."', sub: 'Version mismatch & config conflict' },
      { players: 3, time: '35 min', bar: '70%', note: '"Someone downloaded the Forge build instead of Fabric. Redownloading..."', sub: 'Wrong mod loader build downloaded' },
      { players: 4, time: '45 min', bar: '82%', note: '"One friend has Java 17, server needs Java 21. Discord screenshare begins."', sub: 'Java runtime version mismatch' },
      { players: 6, time: '1.2 hrs', bar: '92%', note: '"Zip file from Google Drive exceeded download quota. Uploading to MediaFire..."', sub: 'Broken links & cloud download quotas' },
      { players: 8, time: '2.0 hrs', bar: '97%', note: '"Two friends gave up and logged off. Game night effectively canceled."', sub: 'Friend fatigue & canceled game night' },
      { players: 12, time: '3.5 hrs', bar: '100%', note: '"Nobody is playing. You spent the entire Friday night troubleshooting in Discord."', sub: 'Complete group failure & wasted evening' },
    ];

    const updateBenchmark = (val) => {
      const num = parseInt(val, 10);
      benchmarkPlayerCount.textContent = num === 1 ? '1 Player' : `${num} Players`;

      let closest = scenarios[0];
      for (const s of scenarios) {
        if (num >= s.players) closest = s;
      }

      if (benchmarkManualTime) benchmarkManualTime.textContent = closest.time;
      if (benchmarkManualBar) benchmarkManualBar.style.width = closest.bar;
      if (benchmarkManualSubText) benchmarkManualSubText.textContent = closest.sub;
      if (benchmarkSliderNote) benchmarkSliderNote.textContent = closest.note;
    };

    benchmarkSlider.addEventListener('input', (e) => {
      updateBenchmark(e.target.value);
    });
  }
});
    }

    const startTimer = () => {
      stopTimer();
      timer = setInterval(() => {
        showSlide(currentIndex + 1);
      }, 6000);
    };

    const stopTimer = () => {
      if (timer) {
        clearInterval(timer);
        timer = null;
      }
    };

    const resetTimer = () => {
      stopTimer();
      startTimer();
    };

    delightSlideshow.addEventListener('mouseenter', stopTimer);
    delightSlideshow.addEventListener('mouseleave', startTimer);
    startTimer();
  }

  // ---------------------------------------------------------------------------
  // 7. Docs Category Switcher (Docs Page Tabs)
  // ---------------------------------------------------------------------------
  const docPills = document.querySelectorAll('.docs-pill');
  const docSections = document.querySelectorAll('.docs-guide-section');
  if (docPills.length && docSections.length) {
    docPills.forEach((pill) => {
      pill.addEventListener('click', (e) => {
        e.preventDefault();
        const targetGuide = pill.dataset.guide;
        docPills.forEach((p) => p.classList.toggle('active', p === pill));
        docSections.forEach((s) => {
          s.style.display = (targetGuide === 'all' || s.dataset.guide === targetGuide) ? 'block' : 'none';
          // ---------------------------------------------------------------------------
  // 13. Interactive Hero Benchmark Slider
  // ---------------------------------------------------------------------------
  const benchmarkSlider = document.getElementById('benchmarkSlider');
  const benchmarkPlayerCount = document.getElementById('benchmarkPlayerCount');
  const benchmarkManualTime = document.getElementById('benchmarkManualTime');
  const benchmarkManualBar = document.getElementById('benchmarkManualBar');
  const benchmarkManualSubText = document.getElementById('benchmarkManualSubText');
  const benchmarkSliderNote = document.getElementById('benchmarkSliderNote');

  if (benchmarkSlider && benchmarkPlayerCount) {
    const scenarios = [
      { players: 1, time: '12 min', bar: '35%', note: '"Missing dependency cloth-config. Why won\'t this jar load?"', sub: 'Manual folder hunt & missing dependencies' },
      { players: 2, time: '25 min', bar: '55%', note: '"Wait, are you on Fabric 0.15 or 0.16? My client is crashing."', sub: 'Version mismatch & config conflict' },
      { players: 3, time: '35 min', bar: '70%', note: '"Someone downloaded the Forge build instead of Fabric. Redownloading..."', sub: 'Wrong mod loader build downloaded' },
      { players: 4, time: '45 min', bar: '82%', note: '"One friend has Java 17, server needs Java 21. Discord screenshare begins."', sub: 'Java runtime version mismatch' },
      { players: 6, time: '1.2 hrs', bar: '92%', note: '"Zip file from Google Drive exceeded download quota. Uploading to MediaFire..."', sub: 'Broken links & cloud download quotas' },
      { players: 8, time: '2.0 hrs', bar: '97%', note: '"Two friends gave up and logged off. Game night effectively canceled."', sub: 'Friend fatigue & canceled game night' },
      { players: 12, time: '3.5 hrs', bar: '100%', note: '"Nobody is playing. You spent the entire Friday night troubleshooting in Discord."', sub: 'Complete group failure & wasted evening' },
    ];

    const updateBenchmark = (val) => {
      const num = parseInt(val, 10);
      benchmarkPlayerCount.textContent = num === 1 ? '1 Player' : `${num} Players`;

      let closest = scenarios[0];
      for (const s of scenarios) {
        if (num >= s.players) closest = s;
      }

      if (benchmarkManualTime) benchmarkManualTime.textContent = closest.time;
      if (benchmarkManualBar) benchmarkManualBar.style.width = closest.bar;
      if (benchmarkManualSubText) benchmarkManualSubText.textContent = closest.sub;
      if (benchmarkSliderNote) benchmarkSliderNote.textContent = closest.note;
    };

    benchmarkSlider.addEventListener('input', (e) => {
      updateBenchmark(e.target.value);
    });
  }
});
        // ---------------------------------------------------------------------------
  // 13. Interactive Hero Benchmark Slider
  // ---------------------------------------------------------------------------
  const benchmarkSlider = document.getElementById('benchmarkSlider');
  const benchmarkPlayerCount = document.getElementById('benchmarkPlayerCount');
  const benchmarkManualTime = document.getElementById('benchmarkManualTime');
  const benchmarkManualBar = document.getElementById('benchmarkManualBar');
  const benchmarkManualSubText = document.getElementById('benchmarkManualSubText');
  const benchmarkSliderNote = document.getElementById('benchmarkSliderNote');

  if (benchmarkSlider && benchmarkPlayerCount) {
    const scenarios = [
      { players: 1, time: '12 min', bar: '35%', note: '"Missing dependency cloth-config. Why won\'t this jar load?"', sub: 'Manual folder hunt & missing dependencies' },
      { players: 2, time: '25 min', bar: '55%', note: '"Wait, are you on Fabric 0.15 or 0.16? My client is crashing."', sub: 'Version mismatch & config conflict' },
      { players: 3, time: '35 min', bar: '70%', note: '"Someone downloaded the Forge build instead of Fabric. Redownloading..."', sub: 'Wrong mod loader build downloaded' },
      { players: 4, time: '45 min', bar: '82%', note: '"One friend has Java 17, server needs Java 21. Discord screenshare begins."', sub: 'Java runtime version mismatch' },
      { players: 6, time: '1.2 hrs', bar: '92%', note: '"Zip file from Google Drive exceeded download quota. Uploading to MediaFire..."', sub: 'Broken links & cloud download quotas' },
      { players: 8, time: '2.0 hrs', bar: '97%', note: '"Two friends gave up and logged off. Game night effectively canceled."', sub: 'Friend fatigue & canceled game night' },
      { players: 12, time: '3.5 hrs', bar: '100%', note: '"Nobody is playing. You spent the entire Friday night troubleshooting in Discord."', sub: 'Complete group failure & wasted evening' },
    ];

    const updateBenchmark = (val) => {
      const num = parseInt(val, 10);
      benchmarkPlayerCount.textContent = num === 1 ? '1 Player' : `${num} Players`;

      let closest = scenarios[0];
      for (const s of scenarios) {
        if (num >= s.players) closest = s;
      }

      if (benchmarkManualTime) benchmarkManualTime.textContent = closest.time;
      if (benchmarkManualBar) benchmarkManualBar.style.width = closest.bar;
      if (benchmarkManualSubText) benchmarkManualSubText.textContent = closest.sub;
      if (benchmarkSliderNote) benchmarkSliderNote.textContent = closest.note;
    };

    benchmarkSlider.addEventListener('input', (e) => {
      updateBenchmark(e.target.value);
    });
  }
});
      // ---------------------------------------------------------------------------
  // 13. Interactive Hero Benchmark Slider
  // ---------------------------------------------------------------------------
  const benchmarkSlider = document.getElementById('benchmarkSlider');
  const benchmarkPlayerCount = document.getElementById('benchmarkPlayerCount');
  const benchmarkManualTime = document.getElementById('benchmarkManualTime');
  const benchmarkManualBar = document.getElementById('benchmarkManualBar');
  const benchmarkManualSubText = document.getElementById('benchmarkManualSubText');
  const benchmarkSliderNote = document.getElementById('benchmarkSliderNote');

  if (benchmarkSlider && benchmarkPlayerCount) {
    const scenarios = [
      { players: 1, time: '12 min', bar: '35%', note: '"Missing dependency cloth-config. Why won\'t this jar load?"', sub: 'Manual folder hunt & missing dependencies' },
      { players: 2, time: '25 min', bar: '55%', note: '"Wait, are you on Fabric 0.15 or 0.16? My client is crashing."', sub: 'Version mismatch & config conflict' },
      { players: 3, time: '35 min', bar: '70%', note: '"Someone downloaded the Forge build instead of Fabric. Redownloading..."', sub: 'Wrong mod loader build downloaded' },
      { players: 4, time: '45 min', bar: '82%', note: '"One friend has Java 17, server needs Java 21. Discord screenshare begins."', sub: 'Java runtime version mismatch' },
      { players: 6, time: '1.2 hrs', bar: '92%', note: '"Zip file from Google Drive exceeded download quota. Uploading to MediaFire..."', sub: 'Broken links & cloud download quotas' },
      { players: 8, time: '2.0 hrs', bar: '97%', note: '"Two friends gave up and logged off. Game night effectively canceled."', sub: 'Friend fatigue & canceled game night' },
      { players: 12, time: '3.5 hrs', bar: '100%', note: '"Nobody is playing. You spent the entire Friday night troubleshooting in Discord."', sub: 'Complete group failure & wasted evening' },
    ];

    const updateBenchmark = (val) => {
      const num = parseInt(val, 10);
      benchmarkPlayerCount.textContent = num === 1 ? '1 Player' : `${num} Players`;

      let closest = scenarios[0];
      for (const s of scenarios) {
        if (num >= s.players) closest = s;
      }

      if (benchmarkManualTime) benchmarkManualTime.textContent = closest.time;
      if (benchmarkManualBar) benchmarkManualBar.style.width = closest.bar;
      if (benchmarkManualSubText) benchmarkManualSubText.textContent = closest.sub;
      if (benchmarkSliderNote) benchmarkSliderNote.textContent = closest.note;
    };

    benchmarkSlider.addEventListener('input', (e) => {
      updateBenchmark(e.target.value);
    });
  }
});
  }

  // ---------------------------------------------------------------------------
  // 8. Global Lightbox Modal
  // ---------------------------------------------------------------------------
  let lightbox = document.querySelector('.lightbox-modal');
  if (!lightbox) {
    lightbox = document.createElement('div');
    lightbox.className = 'lightbox-modal';
    lightbox.setAttribute('role', 'dialog');
    lightbox.setAttribute('aria-modal', 'true');
    lightbox.innerHTML = `
      <div class="lightbox-dialog">
        <div class="lightbox-header">
          <div class="lightbox-title">
            <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><rect x="3" y="3" width="18" height="18" rx="2"/><circle cx="8.5" cy="8.5" r="1.5"/><polyline points="21 15 16 10 5 21"/></svg>
            <span id="lightboxCaptionText">Screenshot Preview</span>
          </div>
          <button class="lightbox-close" aria-label="Close lightbox">
            <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><line x1="18" y1="6" x2="6" y2="18"/><line x1="6" y1="6" x2="18" y2="18"/></svg>
          </button>
        </div>
        <div class="lightbox-body">
          <img id="lightboxImg" src="" alt="Screenshot">
        </div>
        <div class="lightbox-caption">
          <span id="lightboxSubtext">High Resolution Interface Preview</span>
          <span style="font-size: 12px; color: var(--muted);">Press ESC to close</span>
        </div>
      </div>
    `;
    document.body.appendChild(lightbox);
  }

  const lightboxImg = document.getElementById('lightboxImg');
  const lightboxCaptionText = document.getElementById('lightboxCaptionText');
  const lightboxCloseBtn = lightbox.querySelector('.lightbox-close');

  const openLightbox = (src, alt) => {
    if (!src) return;
    lightboxImg.src = src;
    lightboxImg.alt = alt || 'Zircon Screenshot';
    if (lightboxCaptionText) {
      lightboxCaptionText.textContent = alt || 'Zircon Screenshot Preview';
    }
    lightbox.classList.add('active');
    document.body.style.overflow = 'hidden';
  };

  const closeLightbox = () => {
    lightbox.classList.remove('active');
    document.body.style.overflow = '';
  };

  if (lightboxCloseBtn) {
    lightboxCloseBtn.addEventListener('click', closeLightbox);
  }
  lightbox.addEventListener('click', (e) => {
    if (e.target === lightbox || e.target.classList.contains('lightbox-body')) {
      closeLightbox();
    }
    // ---------------------------------------------------------------------------
  // 13. Interactive Hero Benchmark Slider
  // ---------------------------------------------------------------------------
  const benchmarkSlider = document.getElementById('benchmarkSlider');
  const benchmarkPlayerCount = document.getElementById('benchmarkPlayerCount');
  const benchmarkManualTime = document.getElementById('benchmarkManualTime');
  const benchmarkManualBar = document.getElementById('benchmarkManualBar');
  const benchmarkManualSubText = document.getElementById('benchmarkManualSubText');
  const benchmarkSliderNote = document.getElementById('benchmarkSliderNote');

  if (benchmarkSlider && benchmarkPlayerCount) {
    const scenarios = [
      { players: 1, time: '12 min', bar: '35%', note: '"Missing dependency cloth-config. Why won\'t this jar load?"', sub: 'Manual folder hunt & missing dependencies' },
      { players: 2, time: '25 min', bar: '55%', note: '"Wait, are you on Fabric 0.15 or 0.16? My client is crashing."', sub: 'Version mismatch & config conflict' },
      { players: 3, time: '35 min', bar: '70%', note: '"Someone downloaded the Forge build instead of Fabric. Redownloading..."', sub: 'Wrong mod loader build downloaded' },
      { players: 4, time: '45 min', bar: '82%', note: '"One friend has Java 17, server needs Java 21. Discord screenshare begins."', sub: 'Java runtime version mismatch' },
      { players: 6, time: '1.2 hrs', bar: '92%', note: '"Zip file from Google Drive exceeded download quota. Uploading to MediaFire..."', sub: 'Broken links & cloud download quotas' },
      { players: 8, time: '2.0 hrs', bar: '97%', note: '"Two friends gave up and logged off. Game night effectively canceled."', sub: 'Friend fatigue & canceled game night' },
      { players: 12, time: '3.5 hrs', bar: '100%', note: '"Nobody is playing. You spent the entire Friday night troubleshooting in Discord."', sub: 'Complete group failure & wasted evening' },
    ];

    const updateBenchmark = (val) => {
      const num = parseInt(val, 10);
      benchmarkPlayerCount.textContent = num === 1 ? '1 Player' : `${num} Players`;

      let closest = scenarios[0];
      for (const s of scenarios) {
        if (num >= s.players) closest = s;
      }

      if (benchmarkManualTime) benchmarkManualTime.textContent = closest.time;
      if (benchmarkManualBar) benchmarkManualBar.style.width = closest.bar;
      if (benchmarkManualSubText) benchmarkManualSubText.textContent = closest.sub;
      if (benchmarkSliderNote) benchmarkSliderNote.textContent = closest.note;
    };

    benchmarkSlider.addEventListener('input', (e) => {
      updateBenchmark(e.target.value);
    });
  }
});

  window.addEventListener('keydown', (e) => {
    if (e.key === 'Escape' && lightbox.classList.contains('active')) {
      closeLightbox();
    }
    // ---------------------------------------------------------------------------
  // 13. Interactive Hero Benchmark Slider
  // ---------------------------------------------------------------------------
  const benchmarkSlider = document.getElementById('benchmarkSlider');
  const benchmarkPlayerCount = document.getElementById('benchmarkPlayerCount');
  const benchmarkManualTime = document.getElementById('benchmarkManualTime');
  const benchmarkManualBar = document.getElementById('benchmarkManualBar');
  const benchmarkManualSubText = document.getElementById('benchmarkManualSubText');
  const benchmarkSliderNote = document.getElementById('benchmarkSliderNote');

  if (benchmarkSlider && benchmarkPlayerCount) {
    const scenarios = [
      { players: 1, time: '12 min', bar: '35%', note: '"Missing dependency cloth-config. Why won\'t this jar load?"', sub: 'Manual folder hunt & missing dependencies' },
      { players: 2, time: '25 min', bar: '55%', note: '"Wait, are you on Fabric 0.15 or 0.16? My client is crashing."', sub: 'Version mismatch & config conflict' },
      { players: 3, time: '35 min', bar: '70%', note: '"Someone downloaded the Forge build instead of Fabric. Redownloading..."', sub: 'Wrong mod loader build downloaded' },
      { players: 4, time: '45 min', bar: '82%', note: '"One friend has Java 17, server needs Java 21. Discord screenshare begins."', sub: 'Java runtime version mismatch' },
      { players: 6, time: '1.2 hrs', bar: '92%', note: '"Zip file from Google Drive exceeded download quota. Uploading to MediaFire..."', sub: 'Broken links & cloud download quotas' },
      { players: 8, time: '2.0 hrs', bar: '97%', note: '"Two friends gave up and logged off. Game night effectively canceled."', sub: 'Friend fatigue & canceled game night' },
      { players: 12, time: '3.5 hrs', bar: '100%', note: '"Nobody is playing. You spent the entire Friday night troubleshooting in Discord."', sub: 'Complete group failure & wasted evening' },
    ];

    const updateBenchmark = (val) => {
      const num = parseInt(val, 10);
      benchmarkPlayerCount.textContent = num === 1 ? '1 Player' : `${num} Players`;

      let closest = scenarios[0];
      for (const s of scenarios) {
        if (num >= s.players) closest = s;
      }

      if (benchmarkManualTime) benchmarkManualTime.textContent = closest.time;
      if (benchmarkManualBar) benchmarkManualBar.style.width = closest.bar;
      if (benchmarkManualSubText) benchmarkManualSubText.textContent = closest.sub;
      if (benchmarkSliderNote) benchmarkSliderNote.textContent = closest.note;
    };

    benchmarkSlider.addEventListener('input', (e) => {
      updateBenchmark(e.target.value);
    });
  }
});

  // Attach zoom trigger to screenshots across the site
  const setupZoomableImages = () => {
    document.querySelectorAll('.app-frame img, .gallery-card-img-wrap, .gallery-card-img, .frame-zoom-btn, [data-zoom]').forEach((el) => {
      el.style.cursor = 'zoom-in';
      el.addEventListener('click', (e) => {
        e.stopPropagation();
        let img = el.tagName.toLowerCase() === 'img' ? el : el.closest('.app-frame')?.querySelector('img') || el.querySelector('img');
        if (img) {
          openLightbox(img.src, img.alt);
        }
        // ---------------------------------------------------------------------------
  // 13. Interactive Hero Benchmark Slider
  // ---------------------------------------------------------------------------
  const benchmarkSlider = document.getElementById('benchmarkSlider');
  const benchmarkPlayerCount = document.getElementById('benchmarkPlayerCount');
  const benchmarkManualTime = document.getElementById('benchmarkManualTime');
  const benchmarkManualBar = document.getElementById('benchmarkManualBar');
  const benchmarkManualSubText = document.getElementById('benchmarkManualSubText');
  const benchmarkSliderNote = document.getElementById('benchmarkSliderNote');

  if (benchmarkSlider && benchmarkPlayerCount) {
    const scenarios = [
      { players: 1, time: '12 min', bar: '35%', note: '"Missing dependency cloth-config. Why won\'t this jar load?"', sub: 'Manual folder hunt & missing dependencies' },
      { players: 2, time: '25 min', bar: '55%', note: '"Wait, are you on Fabric 0.15 or 0.16? My client is crashing."', sub: 'Version mismatch & config conflict' },
      { players: 3, time: '35 min', bar: '70%', note: '"Someone downloaded the Forge build instead of Fabric. Redownloading..."', sub: 'Wrong mod loader build downloaded' },
      { players: 4, time: '45 min', bar: '82%', note: '"One friend has Java 17, server needs Java 21. Discord screenshare begins."', sub: 'Java runtime version mismatch' },
      { players: 6, time: '1.2 hrs', bar: '92%', note: '"Zip file from Google Drive exceeded download quota. Uploading to MediaFire..."', sub: 'Broken links & cloud download quotas' },
      { players: 8, time: '2.0 hrs', bar: '97%', note: '"Two friends gave up and logged off. Game night effectively canceled."', sub: 'Friend fatigue & canceled game night' },
      { players: 12, time: '3.5 hrs', bar: '100%', note: '"Nobody is playing. You spent the entire Friday night troubleshooting in Discord."', sub: 'Complete group failure & wasted evening' },
    ];

    const updateBenchmark = (val) => {
      const num = parseInt(val, 10);
      benchmarkPlayerCount.textContent = num === 1 ? '1 Player' : `${num} Players`;

      let closest = scenarios[0];
      for (const s of scenarios) {
        if (num >= s.players) closest = s;
      }

      if (benchmarkManualTime) benchmarkManualTime.textContent = closest.time;
      if (benchmarkManualBar) benchmarkManualBar.style.width = closest.bar;
      if (benchmarkManualSubText) benchmarkManualSubText.textContent = closest.sub;
      if (benchmarkSliderNote) benchmarkSliderNote.textContent = closest.note;
    };

    benchmarkSlider.addEventListener('input', (e) => {
      updateBenchmark(e.target.value);
    });
  }
});
      // ---------------------------------------------------------------------------
  // 13. Interactive Hero Benchmark Slider
  // ---------------------------------------------------------------------------
  const benchmarkSlider = document.getElementById('benchmarkSlider');
  const benchmarkPlayerCount = document.getElementById('benchmarkPlayerCount');
  const benchmarkManualTime = document.getElementById('benchmarkManualTime');
  const benchmarkManualBar = document.getElementById('benchmarkManualBar');
  const benchmarkManualSubText = document.getElementById('benchmarkManualSubText');
  const benchmarkSliderNote = document.getElementById('benchmarkSliderNote');

  if (benchmarkSlider && benchmarkPlayerCount) {
    const scenarios = [
      { players: 1, time: '12 min', bar: '35%', note: '"Missing dependency cloth-config. Why won\'t this jar load?"', sub: 'Manual folder hunt & missing dependencies' },
      { players: 2, time: '25 min', bar: '55%', note: '"Wait, are you on Fabric 0.15 or 0.16? My client is crashing."', sub: 'Version mismatch & config conflict' },
      { players: 3, time: '35 min', bar: '70%', note: '"Someone downloaded the Forge build instead of Fabric. Redownloading..."', sub: 'Wrong mod loader build downloaded' },
      { players: 4, time: '45 min', bar: '82%', note: '"One friend has Java 17, server needs Java 21. Discord screenshare begins."', sub: 'Java runtime version mismatch' },
      { players: 6, time: '1.2 hrs', bar: '92%', note: '"Zip file from Google Drive exceeded download quota. Uploading to MediaFire..."', sub: 'Broken links & cloud download quotas' },
      { players: 8, time: '2.0 hrs', bar: '97%', note: '"Two friends gave up and logged off. Game night effectively canceled."', sub: 'Friend fatigue & canceled game night' },
      { players: 12, time: '3.5 hrs', bar: '100%', note: '"Nobody is playing. You spent the entire Friday night troubleshooting in Discord."', sub: 'Complete group failure & wasted evening' },
    ];

    const updateBenchmark = (val) => {
      const num = parseInt(val, 10);
      benchmarkPlayerCount.textContent = num === 1 ? '1 Player' : `${num} Players`;

      let closest = scenarios[0];
      for (const s of scenarios) {
        if (num >= s.players) closest = s;
      }

      if (benchmarkManualTime) benchmarkManualTime.textContent = closest.time;
      if (benchmarkManualBar) benchmarkManualBar.style.width = closest.bar;
      if (benchmarkManualSubText) benchmarkManualSubText.textContent = closest.sub;
      if (benchmarkSliderNote) benchmarkSliderNote.textContent = closest.note;
    };

    benchmarkSlider.addEventListener('input', (e) => {
      updateBenchmark(e.target.value);
    });
  }
});
  };
  setupZoomableImages();

  // ---------------------------------------------------------------------------
  // 9. FAQ Category Filtering
  // ---------------------------------------------------------------------------
  const filterPills = document.querySelectorAll('.filter-pill');
  const faqItems = document.querySelectorAll('.faq-item');
  if (filterPills.length && faqItems.length) {
    filterPills.forEach((pill) => {
      pill.addEventListener('click', () => {
        const cat = pill.dataset.category;
        filterPills.forEach((p) => p.classList.toggle('active', p === pill));
        faqItems.forEach((item) => {
          const match = cat === 'all' || item.dataset.category === cat;
          item.style.display = match ? 'block' : 'none';
          // ---------------------------------------------------------------------------
  // 13. Interactive Hero Benchmark Slider
  // ---------------------------------------------------------------------------
  const benchmarkSlider = document.getElementById('benchmarkSlider');
  const benchmarkPlayerCount = document.getElementById('benchmarkPlayerCount');
  const benchmarkManualTime = document.getElementById('benchmarkManualTime');
  const benchmarkManualBar = document.getElementById('benchmarkManualBar');
  const benchmarkManualSubText = document.getElementById('benchmarkManualSubText');
  const benchmarkSliderNote = document.getElementById('benchmarkSliderNote');

  if (benchmarkSlider && benchmarkPlayerCount) {
    const scenarios = [
      { players: 1, time: '12 min', bar: '35%', note: '"Missing dependency cloth-config. Why won\'t this jar load?"', sub: 'Manual folder hunt & missing dependencies' },
      { players: 2, time: '25 min', bar: '55%', note: '"Wait, are you on Fabric 0.15 or 0.16? My client is crashing."', sub: 'Version mismatch & config conflict' },
      { players: 3, time: '35 min', bar: '70%', note: '"Someone downloaded the Forge build instead of Fabric. Redownloading..."', sub: 'Wrong mod loader build downloaded' },
      { players: 4, time: '45 min', bar: '82%', note: '"One friend has Java 17, server needs Java 21. Discord screenshare begins."', sub: 'Java runtime version mismatch' },
      { players: 6, time: '1.2 hrs', bar: '92%', note: '"Zip file from Google Drive exceeded download quota. Uploading to MediaFire..."', sub: 'Broken links & cloud download quotas' },
      { players: 8, time: '2.0 hrs', bar: '97%', note: '"Two friends gave up and logged off. Game night effectively canceled."', sub: 'Friend fatigue & canceled game night' },
      { players: 12, time: '3.5 hrs', bar: '100%', note: '"Nobody is playing. You spent the entire Friday night troubleshooting in Discord."', sub: 'Complete group failure & wasted evening' },
    ];

    const updateBenchmark = (val) => {
      const num = parseInt(val, 10);
      benchmarkPlayerCount.textContent = num === 1 ? '1 Player' : `${num} Players`;

      let closest = scenarios[0];
      for (const s of scenarios) {
        if (num >= s.players) closest = s;
      }

      if (benchmarkManualTime) benchmarkManualTime.textContent = closest.time;
      if (benchmarkManualBar) benchmarkManualBar.style.width = closest.bar;
      if (benchmarkManualSubText) benchmarkManualSubText.textContent = closest.sub;
      if (benchmarkSliderNote) benchmarkSliderNote.textContent = closest.note;
    };

    benchmarkSlider.addEventListener('input', (e) => {
      updateBenchmark(e.target.value);
    });
  }
});
        // ---------------------------------------------------------------------------
  // 13. Interactive Hero Benchmark Slider
  // ---------------------------------------------------------------------------
  const benchmarkSlider = document.getElementById('benchmarkSlider');
  const benchmarkPlayerCount = document.getElementById('benchmarkPlayerCount');
  const benchmarkManualTime = document.getElementById('benchmarkManualTime');
  const benchmarkManualBar = document.getElementById('benchmarkManualBar');
  const benchmarkManualSubText = document.getElementById('benchmarkManualSubText');
  const benchmarkSliderNote = document.getElementById('benchmarkSliderNote');

  if (benchmarkSlider && benchmarkPlayerCount) {
    const scenarios = [
      { players: 1, time: '12 min', bar: '35%', note: '"Missing dependency cloth-config. Why won\'t this jar load?"', sub: 'Manual folder hunt & missing dependencies' },
      { players: 2, time: '25 min', bar: '55%', note: '"Wait, are you on Fabric 0.15 or 0.16? My client is crashing."', sub: 'Version mismatch & config conflict' },
      { players: 3, time: '35 min', bar: '70%', note: '"Someone downloaded the Forge build instead of Fabric. Redownloading..."', sub: 'Wrong mod loader build downloaded' },
      { players: 4, time: '45 min', bar: '82%', note: '"One friend has Java 17, server needs Java 21. Discord screenshare begins."', sub: 'Java runtime version mismatch' },
      { players: 6, time: '1.2 hrs', bar: '92%', note: '"Zip file from Google Drive exceeded download quota. Uploading to MediaFire..."', sub: 'Broken links & cloud download quotas' },
      { players: 8, time: '2.0 hrs', bar: '97%', note: '"Two friends gave up and logged off. Game night effectively canceled."', sub: 'Friend fatigue & canceled game night' },
      { players: 12, time: '3.5 hrs', bar: '100%', note: '"Nobody is playing. You spent the entire Friday night troubleshooting in Discord."', sub: 'Complete group failure & wasted evening' },
    ];

    const updateBenchmark = (val) => {
      const num = parseInt(val, 10);
      benchmarkPlayerCount.textContent = num === 1 ? '1 Player' : `${num} Players`;

      let closest = scenarios[0];
      for (const s of scenarios) {
        if (num >= s.players) closest = s;
      }

      if (benchmarkManualTime) benchmarkManualTime.textContent = closest.time;
      if (benchmarkManualBar) benchmarkManualBar.style.width = closest.bar;
      if (benchmarkManualSubText) benchmarkManualSubText.textContent = closest.sub;
      if (benchmarkSliderNote) benchmarkSliderNote.textContent = closest.note;
    };

    benchmarkSlider.addEventListener('input', (e) => {
      updateBenchmark(e.target.value);
    });
  }
});
      // ---------------------------------------------------------------------------
  // 13. Interactive Hero Benchmark Slider
  // ---------------------------------------------------------------------------
  const benchmarkSlider = document.getElementById('benchmarkSlider');
  const benchmarkPlayerCount = document.getElementById('benchmarkPlayerCount');
  const benchmarkManualTime = document.getElementById('benchmarkManualTime');
  const benchmarkManualBar = document.getElementById('benchmarkManualBar');
  const benchmarkManualSubText = document.getElementById('benchmarkManualSubText');
  const benchmarkSliderNote = document.getElementById('benchmarkSliderNote');

  if (benchmarkSlider && benchmarkPlayerCount) {
    const scenarios = [
      { players: 1, time: '12 min', bar: '35%', note: '"Missing dependency cloth-config. Why won\'t this jar load?"', sub: 'Manual folder hunt & missing dependencies' },
      { players: 2, time: '25 min', bar: '55%', note: '"Wait, are you on Fabric 0.15 or 0.16? My client is crashing."', sub: 'Version mismatch & config conflict' },
      { players: 3, time: '35 min', bar: '70%', note: '"Someone downloaded the Forge build instead of Fabric. Redownloading..."', sub: 'Wrong mod loader build downloaded' },
      { players: 4, time: '45 min', bar: '82%', note: '"One friend has Java 17, server needs Java 21. Discord screenshare begins."', sub: 'Java runtime version mismatch' },
      { players: 6, time: '1.2 hrs', bar: '92%', note: '"Zip file from Google Drive exceeded download quota. Uploading to MediaFire..."', sub: 'Broken links & cloud download quotas' },
      { players: 8, time: '2.0 hrs', bar: '97%', note: '"Two friends gave up and logged off. Game night effectively canceled."', sub: 'Friend fatigue & canceled game night' },
      { players: 12, time: '3.5 hrs', bar: '100%', note: '"Nobody is playing. You spent the entire Friday night troubleshooting in Discord."', sub: 'Complete group failure & wasted evening' },
    ];

    const updateBenchmark = (val) => {
      const num = parseInt(val, 10);
      benchmarkPlayerCount.textContent = num === 1 ? '1 Player' : `${num} Players`;

      let closest = scenarios[0];
      for (const s of scenarios) {
        if (num >= s.players) closest = s;
      }

      if (benchmarkManualTime) benchmarkManualTime.textContent = closest.time;
      if (benchmarkManualBar) benchmarkManualBar.style.width = closest.bar;
      if (benchmarkManualSubText) benchmarkManualSubText.textContent = closest.sub;
      if (benchmarkSliderNote) benchmarkSliderNote.textContent = closest.note;
    };

    benchmarkSlider.addEventListener('input', (e) => {
      updateBenchmark(e.target.value);
    });
  }
});
  }

  // ---------------------------------------------------------------------------
  // 10. Smooth Reveal Animations (Never hides above-the-fold content)
  // ---------------------------------------------------------------------------
  const revealElements = document.querySelectorAll('.card, .step, .gallery-card, .feature-split');
  if ('IntersectionObserver' in window) {
    revealElements.forEach((el) => el.classList.add('reveal'));
    const revealObserver = new IntersectionObserver((entries, observer) => {
      entries.forEach((entry) => {
        if (entry.isIntersecting) {
          entry.target.classList.add('revealed');
          observer.unobserve(entry.target);
        }
        // ---------------------------------------------------------------------------
  // 13. Interactive Hero Benchmark Slider
  // ---------------------------------------------------------------------------
  const benchmarkSlider = document.getElementById('benchmarkSlider');
  const benchmarkPlayerCount = document.getElementById('benchmarkPlayerCount');
  const benchmarkManualTime = document.getElementById('benchmarkManualTime');
  const benchmarkManualBar = document.getElementById('benchmarkManualBar');
  const benchmarkManualSubText = document.getElementById('benchmarkManualSubText');
  const benchmarkSliderNote = document.getElementById('benchmarkSliderNote');

  if (benchmarkSlider && benchmarkPlayerCount) {
    const scenarios = [
      { players: 1, time: '12 min', bar: '35%', note: '"Missing dependency cloth-config. Why won\'t this jar load?"', sub: 'Manual folder hunt & missing dependencies' },
      { players: 2, time: '25 min', bar: '55%', note: '"Wait, are you on Fabric 0.15 or 0.16? My client is crashing."', sub: 'Version mismatch & config conflict' },
      { players: 3, time: '35 min', bar: '70%', note: '"Someone downloaded the Forge build instead of Fabric. Redownloading..."', sub: 'Wrong mod loader build downloaded' },
      { players: 4, time: '45 min', bar: '82%', note: '"One friend has Java 17, server needs Java 21. Discord screenshare begins."', sub: 'Java runtime version mismatch' },
      { players: 6, time: '1.2 hrs', bar: '92%', note: '"Zip file from Google Drive exceeded download quota. Uploading to MediaFire..."', sub: 'Broken links & cloud download quotas' },
      { players: 8, time: '2.0 hrs', bar: '97%', note: '"Two friends gave up and logged off. Game night effectively canceled."', sub: 'Friend fatigue & canceled game night' },
      { players: 12, time: '3.5 hrs', bar: '100%', note: '"Nobody is playing. You spent the entire Friday night troubleshooting in Discord."', sub: 'Complete group failure & wasted evening' },
    ];

    const updateBenchmark = (val) => {
      const num = parseInt(val, 10);
      benchmarkPlayerCount.textContent = num === 1 ? '1 Player' : `${num} Players`;

      let closest = scenarios[0];
      for (const s of scenarios) {
        if (num >= s.players) closest = s;
      }

      if (benchmarkManualTime) benchmarkManualTime.textContent = closest.time;
      if (benchmarkManualBar) benchmarkManualBar.style.width = closest.bar;
      if (benchmarkManualSubText) benchmarkManualSubText.textContent = closest.sub;
      if (benchmarkSliderNote) benchmarkSliderNote.textContent = closest.note;
    };

    benchmarkSlider.addEventListener('input', (e) => {
      updateBenchmark(e.target.value);
    });
  }
});
    }, {
      rootMargin: '0px 0px -30px 0px',
      threshold: 0.05,
      // ---------------------------------------------------------------------------
  // 13. Interactive Hero Benchmark Slider
  // ---------------------------------------------------------------------------
  const benchmarkSlider = document.getElementById('benchmarkSlider');
  const benchmarkPlayerCount = document.getElementById('benchmarkPlayerCount');
  const benchmarkManualTime = document.getElementById('benchmarkManualTime');
  const benchmarkManualBar = document.getElementById('benchmarkManualBar');
  const benchmarkManualSubText = document.getElementById('benchmarkManualSubText');
  const benchmarkSliderNote = document.getElementById('benchmarkSliderNote');

  if (benchmarkSlider && benchmarkPlayerCount) {
    const scenarios = [
      { players: 1, time: '12 min', bar: '35%', note: '"Missing dependency cloth-config. Why won\'t this jar load?"', sub: 'Manual folder hunt & missing dependencies' },
      { players: 2, time: '25 min', bar: '55%', note: '"Wait, are you on Fabric 0.15 or 0.16? My client is crashing."', sub: 'Version mismatch & config conflict' },
      { players: 3, time: '35 min', bar: '70%', note: '"Someone downloaded the Forge build instead of Fabric. Redownloading..."', sub: 'Wrong mod loader build downloaded' },
      { players: 4, time: '45 min', bar: '82%', note: '"One friend has Java 17, server needs Java 21. Discord screenshare begins."', sub: 'Java runtime version mismatch' },
      { players: 6, time: '1.2 hrs', bar: '92%', note: '"Zip file from Google Drive exceeded download quota. Uploading to MediaFire..."', sub: 'Broken links & cloud download quotas' },
      { players: 8, time: '2.0 hrs', bar: '97%', note: '"Two friends gave up and logged off. Game night effectively canceled."', sub: 'Friend fatigue & canceled game night' },
      { players: 12, time: '3.5 hrs', bar: '100%', note: '"Nobody is playing. You spent the entire Friday night troubleshooting in Discord."', sub: 'Complete group failure & wasted evening' },
    ];

    const updateBenchmark = (val) => {
      const num = parseInt(val, 10);
      benchmarkPlayerCount.textContent = num === 1 ? '1 Player' : `${num} Players`;

      let closest = scenarios[0];
      for (const s of scenarios) {
        if (num >= s.players) closest = s;
      }

      if (benchmarkManualTime) benchmarkManualTime.textContent = closest.time;
      if (benchmarkManualBar) benchmarkManualBar.style.width = closest.bar;
      if (benchmarkManualSubText) benchmarkManualSubText.textContent = closest.sub;
      if (benchmarkSliderNote) benchmarkSliderNote.textContent = closest.note;
    };

    benchmarkSlider.addEventListener('input', (e) => {
      updateBenchmark(e.target.value);
    });
  }
});

    revealElements.forEach((el) => revealObserver.observe(el));
  }

  // ---------------------------------------------------------------------------
  // 11. Copy to Clipboard for Code Blocks
  // ---------------------------------------------------------------------------
  document.querySelectorAll('.codeblock').forEach((block) => {
    block.style.cursor = 'pointer';
    block.setAttribute('title', 'Click to copy');
    block.addEventListener('click', () => {
      const text = block.textContent.trim();
      navigator.clipboard.writeText(text).then(() => {
        const originalBorder = block.style.borderColor;
        block.style.borderColor = 'var(--accent)';
        const originalText = block.getAttribute('title');
        block.setAttribute('title', 'Copied to clipboard!');
        setTimeout(() => {
          block.style.borderColor = originalBorder;
          block.setAttribute('title', originalText);
        }, 1200);
        // ---------------------------------------------------------------------------
  // 13. Interactive Hero Benchmark Slider
  // ---------------------------------------------------------------------------
  const benchmarkSlider = document.getElementById('benchmarkSlider');
  const benchmarkPlayerCount = document.getElementById('benchmarkPlayerCount');
  const benchmarkManualTime = document.getElementById('benchmarkManualTime');
  const benchmarkManualBar = document.getElementById('benchmarkManualBar');
  const benchmarkManualSubText = document.getElementById('benchmarkManualSubText');
  const benchmarkSliderNote = document.getElementById('benchmarkSliderNote');

  if (benchmarkSlider && benchmarkPlayerCount) {
    const scenarios = [
      { players: 1, time: '12 min', bar: '35%', note: '"Missing dependency cloth-config. Why won\'t this jar load?"', sub: 'Manual folder hunt & missing dependencies' },
      { players: 2, time: '25 min', bar: '55%', note: '"Wait, are you on Fabric 0.15 or 0.16? My client is crashing."', sub: 'Version mismatch & config conflict' },
      { players: 3, time: '35 min', bar: '70%', note: '"Someone downloaded the Forge build instead of Fabric. Redownloading..."', sub: 'Wrong mod loader build downloaded' },
      { players: 4, time: '45 min', bar: '82%', note: '"One friend has Java 17, server needs Java 21. Discord screenshare begins."', sub: 'Java runtime version mismatch' },
      { players: 6, time: '1.2 hrs', bar: '92%', note: '"Zip file from Google Drive exceeded download quota. Uploading to MediaFire..."', sub: 'Broken links & cloud download quotas' },
      { players: 8, time: '2.0 hrs', bar: '97%', note: '"Two friends gave up and logged off. Game night effectively canceled."', sub: 'Friend fatigue & canceled game night' },
      { players: 12, time: '3.5 hrs', bar: '100%', note: '"Nobody is playing. You spent the entire Friday night troubleshooting in Discord."', sub: 'Complete group failure & wasted evening' },
    ];

    const updateBenchmark = (val) => {
      const num = parseInt(val, 10);
      benchmarkPlayerCount.textContent = num === 1 ? '1 Player' : `${num} Players`;

      let closest = scenarios[0];
      for (const s of scenarios) {
        if (num >= s.players) closest = s;
      }

      if (benchmarkManualTime) benchmarkManualTime.textContent = closest.time;
      if (benchmarkManualBar) benchmarkManualBar.style.width = closest.bar;
      if (benchmarkManualSubText) benchmarkManualSubText.textContent = closest.sub;
      if (benchmarkSliderNote) benchmarkSliderNote.textContent = closest.note;
    };

    benchmarkSlider.addEventListener('input', (e) => {
      updateBenchmark(e.target.value);
    });
  }
});
      // ---------------------------------------------------------------------------
  // 13. Interactive Hero Benchmark Slider
  // ---------------------------------------------------------------------------
  const benchmarkSlider = document.getElementById('benchmarkSlider');
  const benchmarkPlayerCount = document.getElementById('benchmarkPlayerCount');
  const benchmarkManualTime = document.getElementById('benchmarkManualTime');
  const benchmarkManualBar = document.getElementById('benchmarkManualBar');
  const benchmarkManualSubText = document.getElementById('benchmarkManualSubText');
  const benchmarkSliderNote = document.getElementById('benchmarkSliderNote');

  if (benchmarkSlider && benchmarkPlayerCount) {
    const scenarios = [
      { players: 1, time: '12 min', bar: '35%', note: '"Missing dependency cloth-config. Why won\'t this jar load?"', sub: 'Manual folder hunt & missing dependencies' },
      { players: 2, time: '25 min', bar: '55%', note: '"Wait, are you on Fabric 0.15 or 0.16? My client is crashing."', sub: 'Version mismatch & config conflict' },
      { players: 3, time: '35 min', bar: '70%', note: '"Someone downloaded the Forge build instead of Fabric. Redownloading..."', sub: 'Wrong mod loader build downloaded' },
      { players: 4, time: '45 min', bar: '82%', note: '"One friend has Java 17, server needs Java 21. Discord screenshare begins."', sub: 'Java runtime version mismatch' },
      { players: 6, time: '1.2 hrs', bar: '92%', note: '"Zip file from Google Drive exceeded download quota. Uploading to MediaFire..."', sub: 'Broken links & cloud download quotas' },
      { players: 8, time: '2.0 hrs', bar: '97%', note: '"Two friends gave up and logged off. Game night effectively canceled."', sub: 'Friend fatigue & canceled game night' },
      { players: 12, time: '3.5 hrs', bar: '100%', note: '"Nobody is playing. You spent the entire Friday night troubleshooting in Discord."', sub: 'Complete group failure & wasted evening' },
    ];

    const updateBenchmark = (val) => {
      const num = parseInt(val, 10);
      benchmarkPlayerCount.textContent = num === 1 ? '1 Player' : `${num} Players`;

      let closest = scenarios[0];
      for (const s of scenarios) {
        if (num >= s.players) closest = s;
      }

      if (benchmarkManualTime) benchmarkManualTime.textContent = closest.time;
      if (benchmarkManualBar) benchmarkManualBar.style.width = closest.bar;
      if (benchmarkManualSubText) benchmarkManualSubText.textContent = closest.sub;
      if (benchmarkSliderNote) benchmarkSliderNote.textContent = closest.note;
    };

    benchmarkSlider.addEventListener('input', (e) => {
      updateBenchmark(e.target.value);
    });
  }
});
    // ---------------------------------------------------------------------------
  // 13. Interactive Hero Benchmark Slider
  // ---------------------------------------------------------------------------
  const benchmarkSlider = document.getElementById('benchmarkSlider');
  const benchmarkPlayerCount = document.getElementById('benchmarkPlayerCount');
  const benchmarkManualTime = document.getElementById('benchmarkManualTime');
  const benchmarkManualBar = document.getElementById('benchmarkManualBar');
  const benchmarkManualSubText = document.getElementById('benchmarkManualSubText');
  const benchmarkSliderNote = document.getElementById('benchmarkSliderNote');

  if (benchmarkSlider && benchmarkPlayerCount) {
    const scenarios = [
      { players: 1, time: '12 min', bar: '35%', note: '"Missing dependency cloth-config. Why won\'t this jar load?"', sub: 'Manual folder hunt & missing dependencies' },
      { players: 2, time: '25 min', bar: '55%', note: '"Wait, are you on Fabric 0.15 or 0.16? My client is crashing."', sub: 'Version mismatch & config conflict' },
      { players: 3, time: '35 min', bar: '70%', note: '"Someone downloaded the Forge build instead of Fabric. Redownloading..."', sub: 'Wrong mod loader build downloaded' },
      { players: 4, time: '45 min', bar: '82%', note: '"One friend has Java 17, server needs Java 21. Discord screenshare begins."', sub: 'Java runtime version mismatch' },
      { players: 6, time: '1.2 hrs', bar: '92%', note: '"Zip file from Google Drive exceeded download quota. Uploading to MediaFire..."', sub: 'Broken links & cloud download quotas' },
      { players: 8, time: '2.0 hrs', bar: '97%', note: '"Two friends gave up and logged off. Game night effectively canceled."', sub: 'Friend fatigue & canceled game night' },
      { players: 12, time: '3.5 hrs', bar: '100%', note: '"Nobody is playing. You spent the entire Friday night troubleshooting in Discord."', sub: 'Complete group failure & wasted evening' },
    ];

    const updateBenchmark = (val) => {
      const num = parseInt(val, 10);
      benchmarkPlayerCount.textContent = num === 1 ? '1 Player' : `${num} Players`;

      let closest = scenarios[0];
      for (const s of scenarios) {
        if (num >= s.players) closest = s;
      }

      if (benchmarkManualTime) benchmarkManualTime.textContent = closest.time;
      if (benchmarkManualBar) benchmarkManualBar.style.width = closest.bar;
      if (benchmarkManualSubText) benchmarkManualSubText.textContent = closest.sub;
      if (benchmarkSliderNote) benchmarkSliderNote.textContent = closest.note;
    };

    benchmarkSlider.addEventListener('input', (e) => {
      updateBenchmark(e.target.value);
    });
  }
});

  // ---------------------------------------------------------------------------
  // 12. Manual Language Selection & Override Persistence
  // ---------------------------------------------------------------------------
  window.setZirconLocale = function(lang) {
    if (!lang) return;
    localStorage.setItem('zircon_locale_override', lang);
    var p = window.location.pathname;
    var segments = p.split('/').filter(Boolean);
    var supported = [
      'es', 'pt-br', 'pt-pt', 'de', 'fr', 'it', 'ru', 'pl', 'nl', 'tr',
      'sv', 'uk', 'cs', 'fi', 'da', 'no', 'hu', 'ro', 'el', 'bg', 'sk',
      'hr', 'sr', 'sl', 'lt', 'lv', 'et', 'zh-cn', 'zh-tw', 'ja', 'ko',
      'id', 'vi', 'th', 'ms', 'fil', 'hi', 'ar', 'he', 'fa'
    ];
    if (segments.length > 0 && supported.indexOf(segments[0]) !== -1) {
      if (lang === 'en') {
        segments.shift();
        p = '/' + segments.join('/') + (window.location.pathname.endsWith('/') && segments.length > 0 ? '/' : '');
      } else {
        segments[0] = lang;
        p = '/' + segments.join('/') + (window.location.pathname.endsWith('/') ? '/' : '');
      }
    } else if (lang !== 'en') {
      p = '/' + lang + (p === '/' ? '/' : (p.startsWith('/') ? p : '/' + p));
    }
    window.location.href = p + window.location.search + window.location.hash;
  };
  // ---------------------------------------------------------------------------
  // 13. Interactive Hero Benchmark Slider
  // ---------------------------------------------------------------------------
  const benchmarkSlider = document.getElementById('benchmarkSlider');
  const benchmarkPlayerCount = document.getElementById('benchmarkPlayerCount');
  const benchmarkManualTime = document.getElementById('benchmarkManualTime');
  const benchmarkManualBar = document.getElementById('benchmarkManualBar');
  const benchmarkManualSubText = document.getElementById('benchmarkManualSubText');
  const benchmarkSliderNote = document.getElementById('benchmarkSliderNote');

  if (benchmarkSlider && benchmarkPlayerCount) {
    const scenarios = [
      { players: 1, time: '12 min', bar: '35%', note: '"Missing dependency cloth-config. Why won\'t this jar load?"', sub: 'Manual folder hunt & missing dependencies' },
      { players: 2, time: '25 min', bar: '55%', note: '"Wait, are you on Fabric 0.15 or 0.16? My client is crashing."', sub: 'Version mismatch & config conflict' },
      { players: 3, time: '35 min', bar: '70%', note: '"Someone downloaded the Forge build instead of Fabric. Redownloading..."', sub: 'Wrong mod loader build downloaded' },
      { players: 4, time: '45 min', bar: '82%', note: '"One friend has Java 17, server needs Java 21. Discord screenshare begins."', sub: 'Java runtime version mismatch' },
      { players: 6, time: '1.2 hrs', bar: '92%', note: '"Zip file from Google Drive exceeded download quota. Uploading to MediaFire..."', sub: 'Broken links & cloud download quotas' },
      { players: 8, time: '2.0 hrs', bar: '97%', note: '"Two friends gave up and logged off. Game night effectively canceled."', sub: 'Friend fatigue & canceled game night' },
      { players: 12, time: '3.5 hrs', bar: '100%', note: '"Nobody is playing. You spent the entire Friday night troubleshooting in Discord."', sub: 'Complete group failure & wasted evening' },
    ];

    const updateBenchmark = (val) => {
      const num = parseInt(val, 10);
      benchmarkPlayerCount.textContent = num === 1 ? '1 Player' : `${num} Players`;

      let closest = scenarios[0];
      for (const s of scenarios) {
        if (num >= s.players) closest = s;
      }

      if (benchmarkManualTime) benchmarkManualTime.textContent = closest.time;
      if (benchmarkManualBar) benchmarkManualBar.style.width = closest.bar;
      if (benchmarkManualSubText) benchmarkManualSubText.textContent = closest.sub;
      if (benchmarkSliderNote) benchmarkSliderNote.textContent = closest.note;
    };

    benchmarkSlider.addEventListener('input', (e) => {
      updateBenchmark(e.target.value);
    });
  }
});