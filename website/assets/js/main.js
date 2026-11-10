/* ============================================================
   Zircon Website JavaScript
   Interactive showcases, carousels, lightbox modal, and motion.
   Defensive architecture: Every module is isolated so no single
   DOM absence ever prevents subsequent handlers from binding.
   ============================================================ */

document.addEventListener('DOMContentLoaded', () => {

  // ---------------------------------------------------------------------------
  // 1. Multi-Platform OS Auto-Detection & Selector
  // ---------------------------------------------------------------------------
  try {
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
        btn.addEventListener('click', (e) => {
          e.preventDefault();
          setOS(btn.dataset.os);
        });
      });
    }

    // Hero Direct Download 1-Click Router
    const heroPrimaryBtn = document.getElementById('hero-primary-download');
    if (heroPrimaryBtn) {
      if (detectedOS === 'macos') {
        heroPrimaryBtn.href = 'https://downloads.zirconmc.net/updates/launcher/Zircon_0.9.5_aarch64.dmg';
        const label = heroPrimaryBtn.querySelector('.download-label');
        if (label) label.textContent = 'Download for macOS (.dmg)';
      } else if (detectedOS === 'linux') {
        heroPrimaryBtn.href = 'https://downloads.zirconmc.net/updates/launcher/Zircon_0.9.5_amd64.AppImage';
        const label = heroPrimaryBtn.querySelector('.download-label');
        if (label) label.textContent = 'Download for Linux (.AppImage)';
      } else {
        heroPrimaryBtn.href = 'https://downloads.zirconmc.net/updates/launcher/Zircon_0.9.5_x64-setup.exe';
        const label = heroPrimaryBtn.querySelector('.download-label');
        if (label) label.textContent = 'Download for Windows (.exe)';
      }
    }
  } catch (err) {
    console.warn('OS detector notice:', err);
  }

  // ---------------------------------------------------------------------------
  // 2. Mobile Nav Toggle & Responsive Dropdown
  // ---------------------------------------------------------------------------
  try {
    const toggle = document.querySelector('.nav-toggle');
    const links = document.querySelector('.nav-links');
    if (toggle && links) {
      toggle.addEventListener('click', (e) => {
        e.stopPropagation();
        const open = links.classList.toggle('open');
        toggle.setAttribute('aria-expanded', String(open));
      });

      // Close when clicking outside
      document.addEventListener('click', (e) => {
        if (!toggle.contains(e.target) && !links.contains(e.target) && links.classList.contains('open')) {
          links.classList.remove('open');
          toggle.setAttribute('aria-expanded', 'false');
        }
      });

      // Close when link chosen
      links.querySelectorAll('a').forEach((a) => {
        a.addEventListener('click', () => {
          links.classList.remove('open');
          toggle.setAttribute('aria-expanded', 'false');
        });
      });
    }
  } catch (err) {
    console.warn('Nav toggle notice:', err);
  }

  // ---------------------------------------------------------------------------
  // 3. Download Tabs (Players / Server Owners / Cloud)
  // ---------------------------------------------------------------------------
  try {
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

      tabButtons.forEach((btn) => {
        btn.addEventListener('click', (e) => {
          e.preventDefault();
          const tabName = btn.dataset.tab;
          if (tabName) {
            activateTab(tabName, false);
            try {
              history.replaceState(null, '', '#' + tabName);
            } catch (hErr) {}
          }
        });
      });

      // Intercept any link on page linking to #players, #server-owners, or #cloud
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
            try {
              history.pushState(null, '', '#' + targetTab);
            } catch (hErr) {}
          }
        }
      });

      window.addEventListener('hashchange', () => {
        const wanted = window.location.hash.slice(1);
        if (wanted && document.querySelector(`.tab-btn[data-tab="${wanted}"]`)) {
          activateTab(wanted, false);
        }
      });

      // Honour hash on initial load
      const wanted = window.location.hash.slice(1);
      const initial = wanted && document.querySelector(`.tab-btn[data-tab="${wanted}"]`)
        ? wanted
        : (tabButtons[0]?.dataset.tab || 'players');
      activateTab(initial, false);
    }
  } catch (err) {
    console.warn('Download tabs notice:', err);
  }

  // ---------------------------------------------------------------------------
  // 4. Hero Showcase Tabs & Views (Index Page Split-Hero)
  // ---------------------------------------------------------------------------
  try {
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
        tab.addEventListener('click', (e) => {
          e.preventDefault();
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
  } catch (err) {
    console.warn('Hero showcase notice:', err);
  }

  // ---------------------------------------------------------------------------
  // 5. Delight Features Slideshow (Index Page Section 3)
  // ---------------------------------------------------------------------------
  try {
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
        if (!slides.length) return;
        currentIndex = (idx + slides.length) % slides.length;
        slides.forEach((s, i) => s.classList.toggle('active', i === currentIndex));
        navBtns.forEach((b, i) => b.classList.toggle('active', i === currentIndex));
        dots.forEach((d, i) => d.classList.toggle('active', i === currentIndex));
      };

      navBtns.forEach((btn, i) => {
        btn.addEventListener('click', (e) => {
          e.preventDefault();
          showSlide(i);
          resetTimer();
        });
      });

      dots.forEach((dot, i) => {
        dot.addEventListener('click', (e) => {
          e.preventDefault();
          showSlide(i);
          resetTimer();
        });
      });

      if (prevBtn) {
        prevBtn.addEventListener('click', (e) => {
          e.preventDefault();
          showSlide(currentIndex - 1);
          resetTimer();
        });
      }

      if (nextBtn) {
        nextBtn.addEventListener('click', (e) => {
          e.preventDefault();
          showSlide(currentIndex + 1);
          resetTimer();
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
  } catch (err) {
    console.warn('Delight slideshow notice:', err);
  }

  // ---------------------------------------------------------------------------
  // 6. Interactive Carousels (Launcher & Server Manager Pages)
  // ---------------------------------------------------------------------------
  try {
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
        if (!slides.length) return;
        currentIndex = (index + slides.length) % slides.length;
        slides.forEach((slide, i) => slide.classList.toggle('active', i === currentIndex));
        dots.forEach((dot, i) => dot.classList.toggle('active', i === currentIndex));

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
  } catch (err) {
    console.warn('Carousel notice:', err);
  }

  // ---------------------------------------------------------------------------
  // 7. Docs Category Switcher (Docs Page Tabs)
  // ---------------------------------------------------------------------------
  try {
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
  } catch (err) {
    console.warn('Docs switcher notice:', err);
  }

  // ---------------------------------------------------------------------------
  // 8. Global Lightbox Modal & Zoomable Images
  // ---------------------------------------------------------------------------
  try {
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
      if (!src || !lightboxImg) return;
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
  } catch (err) {
    console.warn('Lightbox notice:', err);
  }

  // ---------------------------------------------------------------------------
  // 9. FAQ Category Filtering (For Players, For Server Hosts, Security, Troubleshoot)
  // ---------------------------------------------------------------------------
  try {
    const filterPills = document.querySelectorAll('.filter-pill');
    const faqItems = document.querySelectorAll('.faq-item');
    if (filterPills.length && faqItems.length) {
      filterPills.forEach((pill) => {
        pill.addEventListener('click', (e) => {
          e.preventDefault();
          const cat = pill.dataset.category;
          filterPills.forEach((p) => p.classList.toggle('active', p === pill));
          faqItems.forEach((item) => {
            const match = cat === 'all' || item.dataset.category === cat;
            item.style.display = match ? 'block' : 'none';
          });
        });
      });
    }
  } catch (err) {
    console.warn('FAQ filtering notice:', err);
  }

  // ---------------------------------------------------------------------------
  // 10. Smooth Reveal Animations
  // ---------------------------------------------------------------------------
  try {
    const revealElements = document.querySelectorAll('.card, .step, .gallery-card, .feature-split');
    if ('IntersectionObserver' in window && revealElements.length) {
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
  } catch (err) {
    console.warn('Reveal animation notice:', err);
  }

  // ---------------------------------------------------------------------------
  // 11. Copy to Clipboard for Code Blocks
  // ---------------------------------------------------------------------------
  try {
    document.querySelectorAll('.codeblock').forEach((block) => {
      block.style.cursor = 'pointer';
      block.setAttribute('title', 'Click to copy');
      block.addEventListener('click', () => {
        const text = block.textContent.trim();
        if (navigator.clipboard?.writeText) {
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
        }
      });
    });
  } catch (err) {
    console.warn('Clipboard notice:', err);
  }

  // ---------------------------------------------------------------------------
  // 12. Manual Language Selection & Override Persistence
  // ---------------------------------------------------------------------------
  window.setZirconLocale = function(lang) {
    if (!lang) return;
    try {
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
    } catch (lErr) {
      console.warn('Locale switch notice:', lErr);
    }
  };
});
