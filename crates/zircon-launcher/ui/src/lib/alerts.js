/**
 * Zircon Launcher Built-in Alert System
 * Production-ready, stylized glassmorphic modal alerts.
 * Replaces native browser alert() dialogs with a high-fidelity obsidian/cyan card.
 */

const queue = [];
let isShowing = false;

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
        return `<svg style="width: 1.25rem; height: 1.25rem;" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
            <circle cx="12" cy="12" r="10"></circle>
            <line x1="12" y1="8" x2="12" y2="12"></line>
            <line x1="12" y1="16" x2="12.01" y2="16"></line>
        </svg>`;
    }
    if (type === 'success') {
        return `<svg style="width: 1.25rem; height: 1.25rem;" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
            <path d="M22 11.08V12a10 10 0 1 1-5.93-9.14"></path>
            <polyline points="22 4 12 14.01 9 11.01"></polyline>
        </svg>`;
    }
    return `<svg style="width: 1.25rem; height: 1.25rem;" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
        <circle cx="12" cy="12" r="10"></circle>
        <line x1="12" y1="8" x2="12"></line>
        <line x1="12" y1="8" x2="12.01" y2="8"></line>
    </svg>`;
}

export function showZirconAlert(message, options) {
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

export function initAlerts() {
    if (typeof window !== 'undefined') {
        window.showZirconAlert = showZirconAlert;
        window.zirconAlert = showZirconAlert;
        window.nativeAlert = window.alert;
        window.alert = function (message) {
            return showZirconAlert(message);
        };
    }
}
