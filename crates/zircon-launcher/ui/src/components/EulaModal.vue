<template>
  <div
    v-if="isOpen"
    class="fixed inset-0 z-[100] flex items-center justify-center bg-black/80 backdrop-blur-md p-4 animate-fade-in"
  >
    <div
      class="relative w-full max-w-2xl max-h-[90vh] flex flex-col bg-[#0b101b] border border-cyan-500/30 rounded-2xl shadow-2xl shadow-cyan-950/40 overflow-hidden"
    >
      <!-- Ambient corner flare -->
      <div class="absolute -top-16 -right-16 w-44 h-44 bg-cyan-500/15 rounded-full blur-3xl pointer-events-none"></div>
      <div class="absolute -bottom-16 -left-16 w-44 h-44 bg-emerald-500/10 rounded-full blur-3xl pointer-events-none"></div>

      <!-- Header -->
      <div class="px-6 pt-6 pb-4 border-b border-slate-800/80 flex items-center justify-between shrink-0">
        <div class="flex items-center gap-3">
          <div class="w-10 h-10 rounded-xl bg-cyan-500/15 border border-cyan-500/30 flex items-center justify-center text-cyan-400 shrink-0 shadow-[0_0_12px_rgba(6,182,212,0.2)]">
            <svg class="w-5 h-5" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
              <path stroke-linecap="round" stroke-linejoin="round" d="M9 12l2 2 4-4m5.618-4.016A11.955 11.955 0 0112 2.944a11.955 11.955 0 01-8.618 3.04A12.02 12.02 0 003 9c0 5.591 3.824 10.29 9 11.622 5.176-1.332 9-6.03 9-11.622 0-1.042-.133-2.052-.382-3.016z" />
            </svg>
          </div>
          <div>
            <div class="flex items-center gap-2">
              <h2 class="text-lg font-extrabold text-white tracking-tight">Zircon End User License Agreement</h2>
              <span class="px-2 py-0.5 rounded-full text-[10px] font-mono font-bold bg-cyan-500/10 text-cyan-300 border border-cyan-500/30">
                BSL 1.1
              </span>
            </div>
            <p class="text-xs text-slate-400 mt-0.5">Please review and agree to the license terms to continue.</p>
          </div>
        </div>

        <button
          v-if="canDismiss"
          type="button"
          class="text-slate-400 hover:text-white p-1 rounded-lg hover:bg-slate-800/60 transition cursor-pointer"
          @click="$emit('close')"
        >
          <svg class="w-5 h-5" fill="none" viewBox="0 0 24 24" stroke="currentColor">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 18L18 6M6 6l12 12" />
          </svg>
        </button>
      </div>

      <!-- Body / Key Provisions & Scrollable Legal Terms -->
      <div class="flex-1 overflow-y-auto px-6 py-4 space-y-4 text-xs leading-relaxed">
        
        <!-- Key Terms Summary Cards -->
        <div class="grid grid-cols-1 md:grid-cols-2 gap-3">
          <div class="p-3 rounded-xl bg-slate-900/60 border border-slate-800">
            <div class="flex items-center gap-2 text-cyan-300 font-semibold mb-1">
              <svg class="w-4 h-4 text-cyan-400 shrink-0" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
                <path stroke-linecap="round" stroke-linejoin="round" d="M12 6.253v13m0-13C10.832 5.477 9.246 5 7.5 5S4.168 5.477 3 6.253v13C4.168 18.477 5.754 18 7.5 18s3.332.477 4.5 1.253m0-13C13.168 5.477 14.754 5 16.5 5c1.747 0 3.332.477 4.5 1.253v13C19.832 18.477 18.247 18 16.5 18c-1.746 0-3.332.477-4.5 1.253" />
              </svg>
              <span>Personal &amp; Community Free Use</span>
            </div>
            <p class="text-[11px] text-slate-400">
              Free to use, modify, run, and self-host for gaming, server networks, and community operations.
            </p>
          </div>

          <div class="p-3 rounded-xl bg-slate-900/60 border border-slate-800">
            <div class="flex items-center gap-2 text-emerald-300 font-semibold mb-1">
              <svg class="w-4 h-4 text-emerald-400 shrink-0" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
                <path stroke-linecap="round" stroke-linejoin="round" d="M12 15v2m-6 4h12a2 2 0 002-2v-6a2 2 0 00-2-2H6a2 2 0 00-2 2v6a2 2 0 002 2zm10-10V7a4 4 0 00-8 0v4h8z" />
              </svg>
              <span>Privacy &amp; Security Respecting</span>
            </div>
            <p class="text-[11px] text-slate-400">
              Zero password logging, zero ad trackers, and authentic Microsoft identity OAuth authentication.
            </p>
          </div>

          <div class="p-3 rounded-xl bg-slate-900/60 border border-slate-800">
            <div class="flex items-center gap-2 text-amber-300 font-semibold mb-1">
              <svg class="w-4 h-4 text-amber-400 shrink-0" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
                <path stroke-linecap="round" stroke-linejoin="round" d="M12 9v2m0 4h.01m-6.938 4h13.856c1.54 0 2.502-1.667 1.732-3L13.732 4c-.77-1.333-2.694-1.333-3.464 0L3.34 16c-.77 1.333.192 3 1.732 3z" />
              </svg>
              <span>Commercial Hosting Restriction</span>
            </div>
            <p class="text-[11px] text-slate-400">
              Under BSL 1.1, third parties may not resell Zircon as a commercial multi-tenant hosting panel or SaaS.
            </p>
          </div>

          <div class="p-3 rounded-xl bg-slate-900/60 border border-slate-800">
            <div class="flex items-center gap-2 text-slate-300 font-semibold mb-1">
              <svg class="w-4 h-4 text-cyan-400 shrink-0" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
                <path stroke-linecap="round" stroke-linejoin="round" d="M13 16h-1v-4h-1m1-4h.01M21 12a9 9 0 11-18 0 9 9 0 0118 0z" />
              </svg>
              <span>Independent Software</span>
            </div>
            <p class="text-[11px] text-slate-400">
              Zircon is an independent project and is not approved by, associated with, or endorsed by Mojang or Microsoft.
            </p>
          </div>
        </div>

        <!-- Full Legal Agreement Box -->
        <div class="mt-3">
          <div class="flex items-center justify-between mb-1.5">
            <span class="text-xs font-semibold text-slate-300 uppercase tracking-wider text-[11px]">Full Agreement Text</span>
            <span class="text-[10px] text-slate-500 font-mono">v1.0.0 • Business Source License 1.1</span>
          </div>
          <div
            class="bg-black/50 border border-slate-800 rounded-xl p-3.5 max-h-56 overflow-y-auto space-y-3 font-mono text-[11px] leading-relaxed text-slate-300 select-text"
          >
            <p class="font-bold text-white">1. GRANT OF LICENSE</p>
            <p>
              Subject to the terms and conditions of this Agreement and the Business Source License 1.1 (BSL-1.1), Licensor (Deven Winslow) grants you a non-exclusive, worldwide license to install, execute, copy, and modify the Software for personal, educational, community, and internal Minecraft server operations free of charge.
            </p>

            <p class="font-bold text-white">2. COMMERCIAL RESTRICTIONS</p>
            <p>
              You may NOT make the functionality of the Software available to third parties as a commercial, multi-tenant managed hosting service, server hosting platform, or cloud game hosting product competing directly with official Zircon managed services. Gaming communities monetized through donations or cosmetics in accordance with Mojang guidelines are fully permitted.
            </p>

            <p class="font-bold text-white">3. PRIVACY &amp; AUTHENTICATION</p>
            <p>
              Authentication is conducted directly with Microsoft's official identity endpoints. Zircon never stores, logs, or transmits your account password. All instances, worlds, and settings remain on your local device or self-hosted server.
            </p>

            <p class="font-bold text-white">4. DISCLAIMER OF WARRANTY &amp; LIABILITY</p>
            <p>
              THE SOFTWARE IS PROVIDED "AS IS" WITHOUT WARRANTY OF ANY KIND. IN NO EVENT SHALL THE AUTHOR BE LIABLE FOR ANY CLAIM, DAMAGES, LOSS OF WORLDS, OR OTHER LIABILITY ARISING FROM THE USE OF THE SOFTWARE.
            </p>

            <p class="font-bold text-white">5. CONVERSION TO OPEN SOURCE</p>
            <p>
              On January 1, 2030, this Software will automatically transition to the Apache License, Version 2.0.
            </p>
          </div>
        </div>

        <!-- Checkbox confirmation -->
        <label
          v-if="!canDismiss || showConsentCheckbox"
          class="flex items-start gap-3 p-3 rounded-xl bg-cyan-950/20 border border-cyan-500/30 cursor-pointer select-none transition hover:bg-cyan-950/30"
        >
          <input
            v-model="hasAgreed"
            type="checkbox"
            class="w-4 h-4 mt-0.5 rounded border-slate-700 text-cyan-500 focus:ring-0 focus:ring-offset-0 bg-slate-900 cursor-pointer"
          />
          <span class="text-xs text-slate-200 font-medium">
            I have read, understood, and agree to the <strong class="text-cyan-300">Zircon End User License Agreement</strong> and licensing terms.
          </span>
        </label>
      </div>

      <!-- Footer -->
      <div class="px-6 py-4 bg-slate-950/80 border-t border-slate-800/80 flex items-center justify-between shrink-0">
        <a
          href="https://github.com/DevTheDev189/Zircon/blob/main/LICENSE"
          target="_blank"
          rel="noopener noreferrer"
          class="text-[11px] text-cyan-400 hover:text-cyan-300 underline underline-offset-2 flex items-center gap-1 transition"
        >
          <span>View BSL 1.1 License</span>
          <svg class="w-3 h-3" fill="none" viewBox="0 0 24 24" stroke="currentColor">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M10 6H6a2 2 0 00-2 2v10a2 2 0 002 2h10a2 2 0 002-2v-4M14 4h6m0 0v6m0-6L10 14" />
          </svg>
        </a>

        <div class="flex items-center gap-2.5">
          <button
            v-if="!canDismiss"
            type="button"
            class="px-4 py-2 rounded-xl text-xs font-semibold text-slate-400 hover:text-slate-200 transition cursor-pointer"
            @click="onDecline"
          >
            Exit Application
          </button>
          <button
            v-else
            type="button"
            class="px-4 py-2 rounded-xl text-xs font-semibold text-slate-400 hover:text-slate-200 transition cursor-pointer"
            @click="$emit('close')"
          >
            Close
          </button>

          <button
            type="button"
            class="px-5 py-2 rounded-xl text-xs font-bold transition flex items-center gap-1.5 cursor-pointer shadow-lg"
            :class="hasAgreed
              ? 'bg-gradient-to-r from-cyan-500 to-emerald-500 hover:from-cyan-400 hover:to-emerald-400 text-slate-950 shadow-cyan-500/20'
              : 'bg-slate-800 text-slate-500 cursor-not-allowed'"
            :disabled="!hasAgreed"
            @click="onAccept"
          >
            <span>Agree &amp; Continue</span>
            <svg class="w-3.5 h-3.5" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2.5">
              <path stroke-linecap="round" stroke-linejoin="round" d="M5 13l4 4L19 7" />
            </svg>
          </button>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup>
import { ref, watch } from 'vue';

const props = defineProps({
  isOpen: {
    type: Boolean,
    default: false,
  },
  canDismiss: {
    type: Boolean,
    default: false,
  },
  showConsentCheckbox: {
    type: Boolean,
    default: true,
  },
});

const emit = defineEmits(['accept', 'decline', 'close']);

const hasAgreed = ref(props.canDismiss);

watch(() => props.isOpen, (newVal) => {
  if (newVal) {
    hasAgreed.value = props.canDismiss;
  }
});

function onAccept() {
  if (!hasAgreed.value) return;
  emit('accept');
}

function onDecline() {
  emit('decline');
}
</script>
