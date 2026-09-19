/**
 * Zircon Built-in Alert System
 * Production-ready, stylized glassmorphic modal alerts matching the Zirky aesthetic.
 * Replaces native browser alert() dialogs with a high-fidelity obsidian/cyan card.
 */
(function () {
    if (window.__zirconAlertInitialized) return;
    window.__zirconAlertInitialized = true;

    const queue = [];
    let isShowing = false;

    // Inject fallback stylesheet if not already present
    function ensureStyles() {
        if (document.getElementById('z-alert-injected-styles')) return;
        const style = document.createElement('style');
        style.id = 'z-alert-injected-styles';
        style.textContent = `
            .z-alert-backdrop {
                position: fixed; inset: 0; z-index: 999999;
                background: rgba(4, 7, 13, 0.8);
                backdrop-filter: blur(8px); -webkit-backdrop-filter: blur(8px);
                display: flex; align-items: center; justify-content: center;
                padding: 1.25rem;
                animation: z-alert-fade-in 0.2s cubic-bezier(0.16, 1, 0.3, 1) forwards;
            }
            .z-alert-backdrop.closing { animation: z-alert-fade-out 0.18s cubic-bezier(0.16, 1, 0.3, 1) forwards; }
            .z-alert-card {
                position: relative; width: 100%; max-width: 440px;
                background: radial-gradient(135% 135% at 50% 0%, #111b2b 0%, #080d16 100%);
                border: 1px solid rgba(71, 210, 201, 0.35); border-radius: 1.25rem;
                box-shadow: 0 25px 60px -10px rgba(0, 0, 0, 0.9), 0 -4px 30px rgba(71, 210, 201, 0.28), 0 0 35px rgba(71, 210, 201, 0.2), inset 0 1px 1px rgba(255, 255, 255, 0.15);
                padding: 1.75rem 1.5rem 1.5rem; display: flex; flex-direction: column; align-items: center; text-align: center; gap: 1.125rem; overflow: hidden;
                animation: z-alert-pop-in 0.25s cubic-bezier(0.16, 1, 0.3, 1) forwards;
            }
            .z-alert-backdrop.closing .z-alert-card { animation: z-alert-pop-out 0.18s cubic-bezier(0.16, 1, 0.3, 1) forwards; }
            .z-alert-glow {
                position: absolute; top: 0; left: 0; right: 0; height: 140px;
                background: radial-gradient(ellipse 75% 65% at 50% 12%, rgba(71, 210, 201, 0.22) 0%, transparent 80%);
                pointer-events: none;
            }
            .z-alert-card.state-error {
                border-color: rgba(244, 63, 94, 0.45);
                box-shadow: 0 25px 60px -10px rgba(0, 0, 0, 0.9), 0 -4px 30px rgba(244, 63, 94, 0.35), 0 0 35px rgba(244, 63, 94, 0.22), inset 0 1px 1px rgba(244, 63, 94, 0.3);
            }
            .z-alert-card.state-error .z-alert-glow {
                background: radial-gradient(ellipse 75% 65% at 50% 12%, rgba(244, 63, 94, 0.28) 0%, transparent 80%);
            }
            .z-alert-card.state-success {
                border-color: rgba(16, 185, 129, 0.45);
                box-shadow: 0 25px 60px -10px rgba(0, 0, 0, 0.9), 0 -4px 30px rgba(16, 185, 129, 0.35), 0 0 35px rgba(16, 185, 129, 0.22), inset 0 1px 1px rgba(16, 185, 129, 0.3);
            }
            .z-alert-card.state-success .z-alert-glow {
                background: radial-gradient(ellipse 75% 65% at 50% 12%, rgba(16, 185, 129, 0.28) 0%, transparent 80%);
            }
            .z-alert-header {
                display: flex; flex-direction: column; align-items: center; text-align: center; gap: 0.75rem; width: 100%; position: relative; z-index: 1;
            }
            .z-alert-icon-wrap {
                width: 3.25rem; height: 3.25rem; border-radius: 1rem;
                display: flex; align-items: center; justify-content: center; flex-shrink: 0;
                background: rgba(71, 210, 201, 0.12); border: 1px solid rgba(71, 210, 201, 0.35); color: #47d2c9;
                box-shadow: 0 0 20px rgba(71, 210, 201, 0.15);
            }
            .z-alert-card.state-error .z-alert-icon-wrap {
                background: rgba(244, 63, 94, 0.14); border-color: rgba(244, 63, 94, 0.4); color: #f43f5e;
                box-shadow: 0 0 20px rgba(244, 63, 94, 0.2);
            }
            .z-alert-card.state-success .z-alert-icon-wrap {
                background: rgba(16, 185, 129, 0.14); border-color: rgba(16, 185, 129, 0.4); color: #34d399;
                box-shadow: 0 0 20px rgba(16, 185, 129, 0.2);
            }
            .z-alert-title-wrap { display: flex; flex-direction: column; align-items: center; gap: 0.25rem; }
            .z-alert-subtitle {
                font-size: 0.6875rem; font-weight: 700; letter-spacing: 0.08em;
                text-transform: uppercase; color: #94a3b8; font-family: ui-sans-serif, system-ui, sans-serif;
            }
            .z-alert-title {
                font-size: 1.125rem; font-weight: 800; color: #ffffff; letter-spacing: -0.01em; margin: 0;
                font-family: ui-sans-serif, system-ui, sans-serif;
            }
            .z-alert-body { width: 100%; padding: 0.125rem 0.5rem; position: relative; z-index: 1; }
            .z-alert-message {
                color: #cbd5e1; font-size: 0.9375rem; line-height: 1.6; margin: 0;
                white-space: pre-wrap; word-break: break-word; font-family: ui-sans-serif, system-ui, sans-serif; text-align: center;
            }
            .z-alert-footer { width: 100%; margin-top: 0.5rem; position: relative; z-index: 1; }
            .z-alert-btn {
                width: 100%; display: block; text-align: center;
                background: linear-gradient(135deg, var(--color-accent-bright, #5adfd5) 0%, var(--color-accent, #47d2c9) 50%, var(--color-accent-deep, #20b2aa) 100%);
                border: 1px solid rgba(255, 255, 255, 0.25); color: var(--color-accent-ink, #022623); font-weight: 800;
                font-size: 0.875rem; letter-spacing: 0.02em; padding: 0.75rem 1.25rem;
                border-radius: 0.75rem; cursor: pointer; box-shadow: 0 4px 18px var(--color-accent-glow, rgba(71, 210, 201, 0.35));
                transition: all 0.2s cubic-bezier(0.4, 0, 0.2, 1); outline: none; font-family: ui-sans-serif, system-ui, sans-serif;
            }
            .z-alert-btn:hover {
                background: linear-gradient(135deg, var(--color-accent-bright, #5adfd5) 0%, var(--color-accent, #47d2c9) 50%, var(--color-accent-deep, #20b2aa) 100%);
                filter: brightness(1.1);
                box-shadow: 0 6px 24px var(--color-accent-glow-strong, rgba(71, 210, 201, 0.55));
                transform: translateY(-1px);
            }
            .z-alert-btn:active {
                transform: translateY(1px) scale(0.98);
                box-shadow: 0 2px 10px var(--color-accent-glow, rgba(71, 210, 201, 0.35));
            }
            @keyframes z-alert-fade-in { from { opacity: 0; } to { opacity: 1; } }
            @keyframes z-alert-fade-out { from { opacity: 1; } to { opacity: 0; } }
            @keyframes z-alert-pop-in { from { opacity: 0; transform: scale(0.94) translateY(12px); } to { opacity: 1; transform: scale(1) translateY(0); } }
            @keyframes z-alert-pop-out { from { opacity: 1; transform: scale(1) translateY(0); } to { opacity: 0; transform: scale(0.96) translateY(6px); } }
        `;
        document.head.appendChild(style);
    }

    function sanitize(str) {
        if (str === null || str === undefined) return '';
        const temp = document.createElement('div');
        temp.textContent = String(str);
        return temp.innerHTML;
    }

    function detectCategory(msg) {
        const text = String(msg || '');
        // Actual hard errors & exceptions -> Red (error)
        if (/failed|error|crash|cannot|could not|rejected|denied|exception/i.test(text)) {
            return { type: 'error', title: 'Action Failed', subtitle: 'Zircon Error Alert' };
        }
        // Harmless validation / input required -> Cyan (info)
        if (/please enter|required|missing|must be/i.test(text)) {
            return { type: 'info', title: 'Action Required', subtitle: 'Validation Notice' };
        }
        // Harmless lifecycle / operational events -> Cyan (info)
        if (/backup|backed up|restart|restarting|reload|reboot|syncing|maintenance/i.test(text)) {
            return { type: 'info', title: 'System Notice', subtitle: 'Operational Update' };
        }
        // Positive completions -> Emerald (success)
        if (/success|saved|deployed|restored|updated|completed|enabled|disabled|applied/i.test(text)) {
            return { type: 'success', title: 'Action Completed', subtitle: 'Zircon Success' };
        }
        // General notices -> Cyan (info)
        return { type: 'info', title: 'System Notice', subtitle: 'Zircon System Alert' };
    }

    function getIconSvg(type) {
        if (type === 'error') {
            return `<svg class="w-6 h-6" style="width: 1.5rem; height: 1.5rem;" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round" stroke-linejoin="round">
                <circle cx="12" cy="12" r="10"></circle>
                <line x1="12" y1="8" x2="12" y2="12"></line>
                <line x1="12" y1="16" x2="12.01" y2="16"></line>
            </svg>`;
        }
        if (type === 'success') {
            return `<svg class="w-6 h-6" style="width: 1.5rem; height: 1.5rem;" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round" stroke-linejoin="round">
                <path d="M22 11.08V12a10 10 0 1 1-5.93-9.14"></path>
                <polyline points="22 4 12 14.01 9 11.01"></polyline>
            </svg>`;
        }
        return `<svg class="w-6 h-6" style="width: 1.5rem; height: 1.5rem;" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round" stroke-linejoin="round">
            <circle cx="12" cy="12" r="10"></circle>
            <line x1="12" y1="8" x2="12" y2="12"></line>
            <line x1="12" y1="8" x2="12.01" y2="8"></line>
        </svg>`;
    }

    function showZirconAlert(message, options) {
        ensureStyles();
        return new Promise((resolve) => {
            const opts = typeof options === 'string' ? { title: options } : (options || {});
            const detected = detectCategory(message);
            const type = opts.type || detected.type;
            const title = opts.title || detected.title;
            const subtitle = opts.subtitle || detected.subtitle;
            const buttonText = opts.buttonText || 'Understood';

            queue.push({
                message: String(message !== undefined && message !== null ? message : ''),
                type,
                title,
                subtitle,
                buttonText,
                resolve
            });

            processQueue();
        });
    }

    function processQueue() {
        if (isShowing || queue.length === 0) return;
        isShowing = true;

        const current = queue.shift();
        const backdrop = document.createElement('div');
        backdrop.className = 'z-alert-backdrop';
        backdrop.setAttribute('role', 'dialog');
        backdrop.setAttribute('aria-modal', 'true');

        const card = document.createElement('div');
        card.className = `z-alert-card state-${current.type}`;

        card.innerHTML = `
            <div class="z-alert-glow"></div>
            <div class="z-alert-header">
                <div class="z-alert-icon-wrap">
                    ${getIconSvg(current.type)}
                </div>
                <div class="z-alert-title-wrap">
                    <span class="z-alert-subtitle">${sanitize(current.subtitle)}</span>
                    <h4 class="z-alert-title">${sanitize(current.title)}</h4>
                </div>
            </div>
            <div class="z-alert-body">
                <p class="z-alert-message">${sanitize(current.message)}</p>
            </div>
            <div class="z-alert-footer">
                <button type="button" class="z-alert-btn">${sanitize(current.buttonText)}</button>
            </div>
        `;

        backdrop.appendChild(card);
        document.body.appendChild(backdrop);

        const btn = card.querySelector('.z-alert-btn');

        if (btn) {
            setTimeout(() => btn.focus(), 30);
        }

        let dismissed = false;
        function dismiss() {
            if (dismissed) return;
            dismissed = true;

            document.removeEventListener('keydown', handleKeydown);
            backdrop.classList.add('closing');

            setTimeout(() => {
                if (backdrop.parentNode) {
                    backdrop.parentNode.removeChild(backdrop);
                }
                isShowing = false;
                current.resolve(true);
                processQueue();
            }, 180);
        }

        function handleKeydown(e) {
            if (e.key === 'Enter' || e.key === 'Escape' || e.key === ' ') {
                e.preventDefault();
                dismiss();
            }
        }

        document.addEventListener('keydown', handleKeydown);
        btn.addEventListener('click', dismiss);
        backdrop.addEventListener('click', (e) => {
            if (e.target === backdrop) {
                dismiss();
            }
        });
    }

    // Expose global methods
    window.showZirconAlert = showZirconAlert;
    window.zirconAlert = showZirconAlert;

    // Override native browser alert
    if (typeof window !== 'undefined') {
        window.nativeAlert = window.alert;
        window.alert = function (message) {
            return showZirconAlert(message);
        };
    }
})();
