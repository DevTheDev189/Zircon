/* ============================================================
   Zircon Website JavaScript
   Interactive showcases, carousels, lightbox modal, and motion.
   ============================================================ */

const RELEASE_URLS = {
  // Windows Launcher
  launcherExe: 'https://downloads.zirconmc.net/updates/launcher/Zircon_0.4.8_x64-setup.exe',
  launcherMsi: 'https://downloads.zirconmc.net/updates/launcher/Zircon_0.4.8_x64_en-US.msi',

  // macOS Launcher
  launcherDmg: 'https://downloads.zirconmc.net/updates/launcher/Zircon_0.4.8_aarch64.dmg',
  launcherApp: 'https://downloads.zirconmc.net/updates/launcher/Zircon-macOS-app.tar.gz',

  // Linux Launcher
  launcherAppImage: 'https://downloads.zirconmc.net/updates/launcher/Zircon_0.4.8_amd64.AppImage',
  launcherDeb: 'https://downloads.zirconmc.net/updates/launcher/zircon_0.4.8_amd64.deb',
  launcherRpm: 'https://downloads.zirconmc.net/updates/launcher/zircon-0.4.8-1.x86_64.rpm',

  // Server packages
  serverZip: 'https://downloads.zirconmc.net/updates/server/v0.4.8/zircon-server-windows-x86_64.zip',
  serverTar: 'https://downloads.zirconmc.net/updates/server/v0.4.8/zircon-server-linux-x86_64.tar.gz',
};

document.addEventListener('DOMContentLoaded', () => {
  // ---------------------------------------------------------------------------
  // 1. Wire every [data-download] button to the release URLs
  // ---------------------------------------------------------------------------
  document.querySelectorAll('[data-download]').forEach((a) => {
    const url = RELEASE_URLS[a.dataset.download];
    if (url) a.setAttribute('href', url);
  });

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
      });
    });
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
      });

      tabPanels.forEach((p) => {
        const on = p.dataset.panel === name;
        p.classList.toggle('active', on);
      });

      if (shouldScroll && found) {
        const targetPanel = document.querySelector(`.tab-panel[data-panel="${name}"]`) || document.querySelector('.tabs');
        if (targetPanel) {
          targetPanel.scrollIntoView({ behavior: 'smooth', block: 'start' });
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
      });
    });

    // Intercept any link on the page that links to #players or #server-owners
    document.addEventListener('click', (e) => {
      const anchor = e.target.closest('a');
      if (!anchor) return;
      const href = anchor.getAttribute('href') || '';
      const match = href.match(/#(players|server-owners)$/);
      if (match) {
        const targetTab = match[1];
        if (document.querySelector(`.tab-btn[data-tab="${targetTab}"]`)) {
          e.preventDefault();
          activateTab(targetTab, true);
          history.pushState(null, '', '#' + targetTab);
        }
      }
    });

    // Handle browser back/forward and hash changes
    window.addEventListener('hashchange', () => {
      const wanted = window.location.hash.slice(1);
      if (wanted && document.querySelector(`.tab-btn[data-tab="${wanted}"]`)) {
        activateTab(wanted, false);
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
      });
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
      });
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
          });
        }
      });
    };

    if (prevBtn) {
      prevBtn.addEventListener('click', (e) => {
        e.preventDefault();
        showSlide(currentIndex - 1);
        resetAutoTimer();
      });
    }
    if (nextBtn) {
      nextBtn.addEventListener('click', (e) => {
        e.preventDefault();
        showSlide(currentIndex + 1);
        resetAutoTimer();
      });
    }
    dots.forEach((dot, i) => {
      dot.addEventListener('click', (e) => {
        e.preventDefault();
        showSlide(i);
        resetAutoTimer();
      });
    });
    thumbs.forEach((thumb, i) => {
      thumb.addEventListener('click', (e) => {
        e.preventDefault();
        showSlide(i);
        resetAutoTimer();
      });
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
  });

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
        });
      });
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
  });

  window.addEventListener('keydown', (e) => {
    if (e.key === 'Escape' && lightbox.classList.contains('active')) {
      closeLightbox();
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
      });
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
        });
      });
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
      });
    }, {
      rootMargin: '0px 0px -30px 0px',
      threshold: 0.05,
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
      });
    });
  });
});