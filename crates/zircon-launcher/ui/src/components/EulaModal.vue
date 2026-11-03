<template>
  <div
    v-if="isOpen"
    class="fixed inset-0 z-[100] flex items-center justify-center bg-black/80 backdrop-blur-md p-4 animate-fade-in"
    @click.self="$emit('close')"
  >
    <div
      class="relative w-full max-w-2xl max-h-[88vh] flex flex-col bg-[#0b101b] border border-cyan-500/30 rounded-2xl shadow-2xl shadow-cyan-950/40 overflow-hidden"
    >
      <!-- Ambient corner flare -->
      <div class="absolute -top-16 -right-16 w-44 h-44 bg-cyan-500/15 rounded-full blur-3xl pointer-events-none"></div>
      <div class="absolute -bottom-16 -left-16 w-44 h-44 bg-emerald-500/10 rounded-full blur-3xl pointer-events-none"></div>

      <!-- Header -->
      <div class="px-6 pt-5 pb-4 border-b border-slate-800/80 flex items-center justify-between shrink-0">
        <div class="flex items-center gap-3">
          <div class="w-10 h-10 rounded-xl bg-cyan-500/15 border border-cyan-500/30 flex items-center justify-center text-cyan-400 shrink-0 shadow-[0_0_12px_rgba(6,182,212,0.2)]">
            <svg class="w-5 h-5" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
              <path stroke-linecap="round" stroke-linejoin="round" d="M9 12l2 2 4-4m5.618-4.016A11.955 11.955 0 0112 2.944a11.955 11.955 0 01-8.618 3.04A12.02 12.02 0 003 9c0 5.591 3.824 10.29 9 11.622 5.176-1.332 9-6.03 9-11.622 0-1.042-.133-2.052-.382-3.016z" />
            </svg>
          </div>
          <div>
            <div class="flex items-center gap-2">
              <h2 class="text-base font-extrabold text-white tracking-tight">Zircon End User License Agreement</h2>
              <span class="px-2 py-0.5 rounded-full text-[10px] font-mono font-bold bg-cyan-500/10 text-cyan-300 border border-cyan-500/30">
                BSL 1.1
              </span>
            </div>
            <p class="text-xs text-slate-400 mt-0.5">Last updated: October 15, 2026 • Business Source License 1.1</p>
          </div>
        </div>

        <button
          type="button"
          class="text-slate-400 hover:text-white p-1 rounded-lg hover:bg-slate-800/60 transition cursor-pointer"
          @click="$emit('close')"
        >
          <svg class="w-5 h-5" fill="none" viewBox="0 0 24 24" stroke="currentColor">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 18L18 6M6 6l12 12" />
          </svg>
        </button>
      </div>

      <!-- Plain EULA Text Container -->
      <div class="flex-1 overflow-y-auto px-6 py-5 space-y-4 text-xs leading-relaxed text-slate-300 select-text">
        <div class="p-3.5 rounded-xl bg-slate-900/60 border border-slate-800/80 text-[11px] text-slate-400 leading-normal">
          <strong class="text-slate-200">Software:</strong> Zircon Companion Launcher, Zircon Server Manager, and related tools, engines, and libraries.<br />
          <strong class="text-slate-200">Licensor:</strong> Deven Winslow<br />
          <strong class="text-slate-200">License:</strong> Business Source License 1.1 (BSL-1.1) — Transitioning to Apache License 2.0 on 2030-01-01
        </div>

        <div class="space-y-4 text-[12px] font-sans">
          <div>
            <h3 class="text-xs font-bold text-cyan-300 uppercase tracking-wider mb-1">Important Notice</h3>
            <p class="text-slate-300">
              By installing, copying, accessing, or using the Software, you agree to be bound by the terms and conditions of this Agreement. If you do not agree to these terms, do not install, access, or use the Software.
            </p>
          </div>

          <div>
            <h3 class="text-xs font-bold text-cyan-300 uppercase tracking-wider mb-1">1. Grant of License</h3>
            <p class="text-slate-300 mb-2">
              Subject to the terms and restrictions of this Agreement and the accompanying BSL 1.1 License, the Licensor grants you a non-exclusive, worldwide, royalty-free license to:
            </p>
            <ul class="list-disc list-inside space-y-1 pl-2 text-slate-400 text-[11px]">
              <li><strong class="text-slate-300">Personal &amp; Educational Use:</strong> Install, run, and execute the Software on any computer, server, or device for personal, family, or educational purposes.</li>
              <li><strong class="text-slate-300">Community &amp; Self-Hosted Servers:</strong> Install, run, modify, and deploy the Software on bare-metal servers, home lab appliances, virtual machines, containers, or personal cloud instances to supervise and manage Minecraft servers.</li>
              <li><strong class="text-slate-300">Gaming Communities &amp; Server Networks:</strong> Minecraft networks, gaming communities, and server administrators (including networks that receive player donations, rank purchases, or cosmetic transactions in accordance with Mojang's Commercial Usage Guidelines) are explicitly permitted to utilize the Software internally.</li>
            </ul>
          </div>

          <div>
            <h3 class="text-xs font-bold text-cyan-300 uppercase tracking-wider mb-1">2. Prohibited Uses &amp; Commercial Restrictions</h3>
            <p class="text-slate-300 mb-2">Under the Business Source License 1.1:</p>
            <ul class="list-disc list-inside space-y-1 pl-2 text-slate-400 text-[11px]">
              <li><strong class="text-slate-300">Commercial Managed Hosting Restriction:</strong> You may not make the functionality of the Software available to third parties as a commercial, multi-tenant managed hosting service, server hosting platform, or cloud product competing directly with official Zircon managed services.</li>
              <li><strong class="text-slate-300">No Circumvention:</strong> You may not reverse engineer, decompile, or tamper with security controls, telemetry opt-outs, or cryptographic signature verifications within the Software.</li>
              <li><strong class="text-slate-300">Trademark &amp; Branding:</strong> The names "Zircon", "ZirconMC", the Zircon logo, and associated branding are proprietary. Modified forks may not use official Zircon branding in a manner that implies official endorsement.</li>
            </ul>
          </div>

          <div>
            <h3 class="text-xs font-bold text-cyan-300 uppercase tracking-wider mb-1">3. Privacy, Telemetry &amp; Authentication</h3>
            <ul class="list-disc list-inside space-y-1 pl-2 text-slate-400 text-[11px]">
              <li><strong class="text-slate-300">Zero Password Logging:</strong> Zircon never stores, logs, or transmits your account password. Authentication occurs directly with Microsoft's official OAuth endpoints.</li>
              <li><strong class="text-slate-300">Local Data Storage:</strong> Minecraft instance files, worlds, tokens, and configurations remain stored locally on your device or self-hosted server unless you explicitly enable cloud backups.</li>
              <li><strong class="text-slate-300">No Data Brokerage:</strong> Zircon does not sell, license, or broker personal information or browsing history to third parties or advertising networks.</li>
            </ul>
          </div>

          <div>
            <h3 class="text-xs font-bold text-cyan-300 uppercase tracking-wider mb-1">4. Third-Party Services &amp; Minecraft Disclaimer</h3>
            <p class="text-slate-300">
              Zircon is an independent project developed by Deven Winslow and community contributors. <strong>Zircon is not an official Minecraft product and is not approved by, endorsed by, or affiliated with Mojang Studios or Microsoft Corporation.</strong> Minecraft is a registered trademark of Mojang Synergies AB. To play Minecraft using Zircon, you must hold a legitimate Minecraft license purchased from Microsoft/Mojang.
            </p>
          </div>

          <div>
            <h3 class="text-xs font-bold text-cyan-300 uppercase tracking-wider mb-1">5. Disclaimer of Warranty &amp; Limitation of Liability</h3>
            <p class="text-slate-400 text-[11px] uppercase tracking-wide leading-relaxed font-mono bg-black/40 p-3 rounded-lg border border-slate-800">
              THE SOFTWARE IS PROVIDED "AS IS" WITHOUT WARRANTY OF ANY KIND. IN NO EVENT SHALL THE LICENSOR BE LIABLE FOR ANY CLAIM, DAMAGES, LOSS OF WORLDS, HARDWARE MALFUNCTION, OR OTHER LIABILITY ARISING FROM THE USE OF THE SOFTWARE.
            </p>
          </div>

          <div>
            <h3 class="text-xs font-bold text-cyan-300 uppercase tracking-wider mb-1">6. Conversion to Open Source</h3>
            <p class="text-slate-300">
              In accordance with the Business Source License 1.1 parameters, on <strong>January 1, 2030</strong>, the Software will automatically convert to the <strong>Apache License, Version 2.0</strong>.
            </p>
          </div>
        </div>
      </div>

      <!-- Footer -->
      <div class="px-6 py-4 bg-slate-950/80 border-t border-slate-800/80 flex items-center justify-between shrink-0">
        <a
          href="https://github.com/DevTheDev189/Zircon/blob/main/LICENSE"
          target="_blank"
          rel="noopener noreferrer"
          class="text-[11px] text-cyan-400 hover:text-cyan-300 underline underline-offset-2 flex items-center gap-1 transition"
        >
          <span>View BSL 1.1 License on GitHub</span>
          <svg class="w-3 h-3" fill="none" viewBox="0 0 24 24" stroke="currentColor">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M10 6H6a2 2 0 00-2 2v10a2 2 0 002 2h10a2 2 0 002-2v-4M14 4h6m0 0v6m0-6L10 14" />
          </svg>
        </a>

        <button
          type="button"
          class="px-5 py-2 rounded-xl text-xs font-bold bg-slate-800 hover:bg-slate-700 text-white transition cursor-pointer"
          @click="$emit('close')"
        >
          Close
        </button>
      </div>
    </div>
  </div>
</template>

<script setup>
defineProps({
  isOpen: {
    type: Boolean,
    default: false,
  },
});

defineEmits(['close']);
</script>
