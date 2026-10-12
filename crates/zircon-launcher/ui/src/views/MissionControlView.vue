<template>
  <div class="h-full flex flex-col p-5 overflow-hidden text-slate-100">
    <!-- Top Cockpit Header -->
    <header class="flex items-center justify-between pb-4 mb-3.5 border-b border-slate-800/80 shrink-0 flex-wrap gap-3">
      <div class="flex items-center gap-3">
        <div
          class="w-10 h-10 rounded-xl border flex items-center justify-center shadow-[0_0_15px_var(--color-accent-glow)] shrink-0 transition-colors"
          :class="isGameRunning ? 'bg-cyan-500/10 border-cyan-500/30 text-cyan-400' : 'bg-slate-800/40 border-slate-700/60 text-slate-500'"
        >
          <svg class="w-5 h-5" :class="isGameRunning ? 'animate-pulse' : ''" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
            <circle cx="12" cy="12" r="3" />
            <path stroke-linecap="round" stroke-linejoin="round" d="M19.4 15a1.65 1.65 0 00.33 1.82l.06.06a2 2 0 010 2.83 2 2 0 01-2.83 0l-.06-.06a1.65 1.65 0 00-1.82-.33 1.65 1.65 0 00-1 1.51V21a2 2 0 01-2 2 2 2 0 01-2-2v-.09A1.65 1.65 0 009 19.4a1.65 1.65 0 00-1.82.33l-.06.06a2 2 0 01-2.83 0 2 2 0 010-2.83l.06-.06a1.65 1.65 0 00.33-1.82 1.65 1.65 0 00-1.51-1H3a2 2 0 01-2-2 2 2 0 012-2h.09A1.65 1.65 0 004.6 9a1.65 1.65 0 00-.33-1.82l-.06-.06a2 2 0 010-2.83 2 2 0 012.83 0l.06.06a1.65 1.65 0 001.82.33H9a1.65 1.65 0 001-1.51V3a2 2 0 012-2 2 2 0 012 2v.09a1.65 1.65 0 001 1.51 1.65 1.65 0 001.82-.33l.06-.06a2 2 0 012.83 0 2 2 0 010 2.83l-.06.06a1.65 1.65 0 00-.33 1.82V9a1.65 1.65 0 001.51 1H21a2 2 0 012 2 2 2 0 01-2 2h-.09a1.65 1.65 0 00-1.51 1z" />
          </svg>
        </div>
        <div>
          <div class="flex items-center gap-2 flex-wrap">
            <h1 class="text-base sm:text-lg font-black text-white tracking-wide">
              {{ telemetry?.instanceName || gameStatus?.label || (isGameRunning ? $t('mission_control.active_session') : $t('mission_control.hud_title')) }}
            </h1>
            <!-- Live Status or Idle Standby Badge -->
            <span
              v-if="isGameRunning"
              class="inline-flex items-center gap-1 px-2 py-0.5 rounded-full text-[10px] font-bold font-mono bg-emerald-500/10 text-emerald-400 border border-emerald-500/30 shadow-[0_0_8px_rgba(52,211,153,0.2)]"
            >
              <span class="w-1.5 h-1.5 rounded-full bg-emerald-400 animate-ping"></span>
              {{ $t('mission_control.live_pid', { pid: telemetry?.pid || gameStatus?.pid || '---' }) }}
            </span>
            <span
              v-else
              class="inline-flex items-center gap-1.5 px-2.5 py-0.5 rounded-full text-[10px] font-bold font-mono bg-slate-800/80 text-slate-400 border border-slate-700/60"
            >
              <span class="w-1.5 h-1.5 rounded-full bg-slate-500"></span>
              {{ $t('mission_control.standby_badge') }}
            </span>
          </div>

          <p class="text-xs text-slate-400 flex items-center gap-2 mt-0.5 flex-wrap font-sans">
            <span>{{ $t('mission_control.telemetry_subtitle') }}</span>
            <span class="text-slate-600">•</span>
            <span class="font-mono text-cyan-300">{{ $t('mission_control.uptime', { time: isGameRunning ? formattedUptime : '00:00:00' }) }}</span>
            <span class="text-slate-600">•</span>
            <span class="font-mono text-slate-400 text-[11px] flex items-center gap-1">
              <span class="text-emerald-400">↓ {{ formatNetSpeed(telemetry?.netRxKbps) }}</span>
              <span class="text-sky-400">↑ {{ formatNetSpeed(telemetry?.netTxKbps) }}</span>
            </span>
          </p>
        </div>
      </div>

      <!-- Quick Action Deck (Greyed out when no game booted) -->
      <div class="flex items-center gap-2 shrink-0">
        <!-- Snapshot button -->
        <button
          class="text-xs px-3 py-1.5 rounded-xl font-bold transition-all flex items-center gap-1.5 shadow-sm"
          :class="isGameRunning ? 'bg-cyan-500/15 text-cyan-300 border border-cyan-500/30 hover:bg-cyan-500/25 cursor-pointer' : 'bg-slate-900/40 text-slate-500 border border-slate-800/80 opacity-40 cursor-not-allowed'"
          :disabled="!isGameRunning || snapshotting"
          @click="takeQuickSnapshot"
          :title="$t('mission_control.snapshot_tooltip')"
        >
          <span v-if="snapshotting" class="w-3.5 h-3.5 border-2 border-cyan-300 border-t-transparent rounded-full animate-spin"></span>
          <svg v-else class="w-3.5 h-3.5" :class="isGameRunning ? 'text-cyan-400' : 'text-slate-500'" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
            <path stroke-linecap="round" stroke-linejoin="round" d="M12 8v4l3 3m6-3a9 9 0 11-18 0 9 9 0 0118 0z" />
          </svg>
          <span>{{ snapshotting ? $t('mission_control.snapshotting') : $t('mission_control.snapshot_btn') }}</span>
        </button>

        <!-- Screenshots button -->
        <button
          class="text-xs px-3 py-1.5 rounded-xl font-bold border transition-all flex items-center gap-1.5"
          :class="isGameRunning ? 'text-slate-300 border-slate-700/80 hover:border-slate-500 hover:text-white bg-slate-900/40 cursor-pointer' : 'text-slate-600 border-slate-800/80 bg-slate-900/20 opacity-40 cursor-not-allowed'"
          :disabled="!isGameRunning"
          @click="openScreenshots"
          :title="$t('mission_control.screenshots_tooltip')"
        >
          <svg class="w-3.5 h-3.5 text-slate-500" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
            <rect x="3" y="3" width="18" height="18" rx="2" ry="2" />
            <circle cx="8.5" cy="8.5" r="1.5" />
            <polyline points="21 15 16 10 5 21" />
          </svg>
          <span>{{ $t('mission_control.screenshots_btn') }}</span>
        </button>

        <!-- Stop Game button -->
        <button
          class="text-xs px-3 py-1.5 rounded-xl font-bold border transition-all flex items-center gap-1.5 shadow-sm"
          :class="isGameRunning ? 'bg-red-500/15 text-red-400 border-red-500/30 hover:bg-red-500/25 cursor-pointer shadow-red-500/10' : 'bg-slate-900/40 text-slate-600 border-slate-800/80 opacity-30 cursor-not-allowed'"
          :disabled="!isGameRunning"
          @click="confirmStopGame"
          :title="$t('mission_control.stop_game_tooltip')"
        >
          <svg class="w-3.5 h-3.5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5">
            <rect x="6" y="6" width="12" height="12" rx="1" />
          </svg>
          <span>{{ $t('mission_control.stop_game_btn') }}</span>
        </button>
      </div>
    </header>

    <!-- 4 High-Tech Telemetry Cards with Y-Axis Values & Spline Curves (Always visible) -->
    <div class="relative z-10 grid grid-cols-1 sm:grid-cols-2 xl:grid-cols-4 gap-3.5 mb-3.5 shrink-0">
      <!-- 1. CPU Dynamics (Host vs Game) -->
      <div class="z-card bg-[#0b1019]/90 border border-slate-800 p-3.5 rounded-2xl flex flex-col justify-between relative overflow-hidden group">
        <div class="flex items-center justify-between mb-1">
          <span class="text-xs font-bold uppercase tracking-wider text-slate-400 flex items-center gap-1.5 font-sans">
            <span class="w-2 h-2 rounded-full bg-cyan-400 shadow-[0_0_8px_var(--color-accent)]"></span>
            {{ $t('mission_control.cpu_dynamics') }}
          </span>
          <div class="flex items-center gap-1.5 font-mono text-xs font-bold">
            <template v-if="isGameRunning">
              <span class="text-cyan-300">Proc: {{ (telemetry?.cpuPercent || 0).toFixed(1) }}%</span>
              <span class="text-slate-600">|</span>
              <span class="text-slate-400 text-[11px]">Host: {{ (telemetry?.systemCpuPercent || 0).toFixed(1) }}%</span>
            </template>
            <template v-else>
              <span class="text-cyan-300">Host: {{ (telemetry?.systemCpuPercent || 0).toFixed(1) }}%</span>
              <span class="px-1.5 py-0.2 rounded text-[10px] bg-slate-800/80 text-slate-400 font-sans">{{ $t('mission_control.idle') }}</span>
            </template>
          </div>
        </div>

        <!-- SVG Spline Curve with Crisp HTML Y-Axis numbers -->
        <div class="relative w-full h-16 my-1 flex items-stretch">
          <div class="flex flex-col justify-between text-[9px] font-mono text-slate-500 pr-1.5 py-0.5 select-none w-8 text-right shrink-0">
            <span>100%</span>
            <span>50%</span>
            <span>0%</span>
          </div>
          <div class="relative flex-1 h-full overflow-hidden">
            <!-- Horizontal Grid Lines -->
            <div class="absolute inset-0 flex flex-col justify-between pointer-events-none opacity-20 py-1">
              <div class="border-b border-slate-600 border-dashed w-full"></div>
              <div class="border-b border-slate-600 border-dashed w-full"></div>
              <div class="border-b border-slate-600 w-full"></div>
            </div>
            <svg class="w-full h-full overflow-visible" viewBox="0 0 300 64" preserveAspectRatio="none">
              <defs>
                <linearGradient id="sysCpuGrad" x1="0" y1="0" x2="0" y2="1">
                  <stop offset="0%" stop-color="#47d2c9" stop-opacity="0.32" />
                  <stop offset="100%" stop-color="#20b2aa" stop-opacity="0.0" />
                </linearGradient>
                <linearGradient id="procCpuGrad" x1="0" y1="0" x2="0" y2="1">
                  <stop offset="0%" stop-color="#38bdf8" stop-opacity="0.38" />
                  <stop offset="100%" stop-color="#0284c7" stop-opacity="0.0" />
                </linearGradient>
              </defs>
              <path :d="buildSvgSpline(cpuSysHistory, 300, 64, 100, 0, true, 4, 4, 0)" fill="url(#sysCpuGrad)" />
              <path v-if="isGameRunning" :d="buildSvgSpline(cpuProcHistory, 300, 64, 100, 0, true, 4, 4, 0)" fill="url(#procCpuGrad)" />
              <path :d="buildSvgSpline(cpuSysHistory, 300, 64, 100, 0, false, 4, 4, 0)" fill="none" stroke="#20b2aa" stroke-width="1.8" stroke-opacity="0.8" />
              <path v-if="isGameRunning" :d="buildSvgSpline(cpuProcHistory, 300, 64, 100, 0, false, 4, 4, 0)" fill="none" stroke="#38bdf8" stroke-width="2.2" class="drop-shadow-[0_0_6px_rgba(56,189,248,0.5)]" />
            </svg>
          </div>
        </div>

        <div class="flex items-center justify-between text-[11px] text-slate-400 pt-1 border-t border-slate-800/60 font-mono">
          <span>{{ isGameRunning ? $t('mission_control.priority_high') : $t('mission_control.host_monitoring') }}</span>
          <span class="text-slate-500">{{ isGameRunning ? `PID: ${telemetry?.pid || '---'}` : $t('mission_control.system_active') }}</span>
        </div>
      </div>

      <!-- 2. Memory & Heap Dynamics (JVM Heap & System RAM) -->
      <div class="z-card bg-[#0b1019]/90 border border-slate-800 p-3.5 rounded-2xl flex flex-col justify-between relative overflow-hidden group">
        <div class="flex items-center justify-between mb-1">
          <span class="text-xs font-bold uppercase tracking-wider text-slate-400 flex items-center gap-1.5 font-sans">
            <span class="w-2 h-2 rounded-full bg-teal-400 shadow-[0_0_8px_rgba(45,212,191,0.6)]"></span>
            {{ $t('mission_control.memory_dynamics') }}
          </span>
          <div class="flex items-center gap-1.5 font-mono text-xs font-bold">
            <template v-if="isGameRunning">
              <span class="text-teal-300">{{ telemetry?.memoryUsedMb || 0 }} MB</span>
              <span class="text-slate-600">/</span>
              <span class="text-slate-400 text-[11px]">{{ telemetry?.memoryAllocatedMb || 4096 }} MB</span>
            </template>
            <template v-else>
              <span class="text-teal-300">{{ ((telemetry?.systemMemoryUsedMb || telemetry?.memoryUsedMb || 0) / 1024).toFixed(1) }} GB</span>
              <span class="text-slate-600">/</span>
              <span class="text-slate-400 text-[11px]">{{ ((telemetry?.systemMemoryTotalMb || telemetry?.memoryAllocatedMb || 16384) / 1024).toFixed(0) }} GB</span>
            </template>
          </div>
        </div>

        <!-- SVG Spline Curve with Crisp HTML Y-Axis numbers -->
        <div class="relative w-full h-16 my-1 flex items-stretch">
          <div class="flex flex-col justify-between text-[9px] font-mono text-slate-500 pr-1.5 py-0.5 select-none w-8 text-right shrink-0">
            <span>{{ isGameRunning ? Math.round(maxHeapVal / 1024) + 'G' : Math.round((telemetry?.systemMemoryTotalMb || 16384) / 1024) + 'G' }}</span>
            <span>{{ isGameRunning ? (maxHeapVal / 2048).toFixed(1) + 'G' : Math.round((telemetry?.systemMemoryTotalMb || 16384) / 2048) + 'G' }}</span>
            <span>0</span>
          </div>
          <div class="relative flex-1 h-full overflow-hidden">
            <!-- Horizontal Grid Lines -->
            <div class="absolute inset-0 flex flex-col justify-between pointer-events-none opacity-20 py-1">
              <div class="border-b border-slate-600 border-dashed w-full"></div>
              <div class="border-b border-slate-600 border-dashed w-full"></div>
              <div class="border-b border-slate-600 w-full"></div>
            </div>
            <svg class="w-full h-full overflow-visible" viewBox="0 0 300 64" preserveAspectRatio="none">
              <defs>
                <linearGradient id="memHeapGrad" x1="0" y1="0" x2="0" y2="1">
                  <stop offset="0%" stop-color="#2dd4bf" stop-opacity="0.32" />
                  <stop offset="100%" stop-color="#0f766e" stop-opacity="0.0" />
                </linearGradient>
              </defs>
              <path :d="buildSvgSpline(memHeapHistory, 300, 64, maxHeapVal, 0, true, 4, 4, 0)" fill="url(#memHeapGrad)" />
              <path :d="buildSvgSpline(memHeapHistory, 300, 64, maxHeapVal, 0, false, 4, 4, 0)" fill="none" stroke="#2dd4bf" stroke-width="2" class="drop-shadow-[0_0_6px_rgba(45,212,191,0.5)]" />
            </svg>
          </div>
        </div>

        <div class="flex items-center justify-between text-[11px] text-slate-400 pt-1 border-t border-slate-800/60 font-mono">
          <span>{{ isGameRunning ? `Alloc: ${memoryPercent.toFixed(1)}%` : `System: ${(((telemetry?.systemMemoryUsedMb || telemetry?.memoryUsedMb || 0) / (telemetry?.systemMemoryTotalMb || 1)) * 100).toFixed(1)}%` }}</span>
          <span :class="memoryPercent > 88 && isGameRunning ? 'text-amber-400 font-bold' : 'text-slate-500'">
            {{ isGameRunning ? (memoryPercent > 88 ? $t('mission_control.high_heap') : $t('mission_control.healthy_alloc')) : $t('mission_control.host_mem_active') }}
          </span>
        </div>
      </div>

      <!-- 3. Network Throughput & Bandwidth (RX / TX) -->
      <div class="z-card bg-[#0b1019]/90 border border-slate-800 p-3.5 rounded-2xl flex flex-col justify-between relative overflow-hidden group">
        <div class="flex items-center justify-between mb-1">
          <span class="text-xs font-bold uppercase tracking-wider text-slate-400 flex items-center gap-1.5 font-sans">
            <span class="w-2 h-2 rounded-full bg-emerald-400 shadow-[0_0_8px_rgba(52,211,153,0.6)]"></span>
            {{ $t('mission_control.network_traffic') }}
          </span>
          <div class="flex items-center gap-1.5 font-mono text-xs font-bold">
            <span class="text-emerald-400">↓ {{ formatNetSpeed(telemetry?.netRxKbps) }}</span>
            <span class="text-slate-600">|</span>
            <span class="text-sky-400 text-[11px]">↑ {{ formatNetSpeed(telemetry?.netTxKbps) }}</span>
          </div>
        </div>

        <!-- SVG Spline Curve with Crisp HTML Y-Axis numbers -->
        <div class="relative w-full h-16 my-1 flex items-stretch">
          <div class="flex flex-col justify-between text-[9px] font-mono text-slate-500 pr-1.5 py-0.5 select-none w-8 text-right shrink-0">
            <span>{{ formatYAxisNet(maxNetVal) }}</span>
            <span>{{ formatYAxisNet(maxNetVal / 2) }}</span>
            <span>0</span>
          </div>
          <div class="relative flex-1 h-full overflow-hidden">
            <!-- Horizontal Grid Lines -->
            <div class="absolute inset-0 flex flex-col justify-between pointer-events-none opacity-20 py-1">
              <div class="border-b border-slate-600 border-dashed w-full"></div>
              <div class="border-b border-slate-600 border-dashed w-full"></div>
              <div class="border-b border-slate-600 w-full"></div>
            </div>
            <svg class="w-full h-full overflow-visible" viewBox="0 0 300 64" preserveAspectRatio="none">
              <defs>
                <linearGradient id="netRxGrad" x1="0" y1="0" x2="0" y2="1">
                  <stop offset="0%" stop-color="#10b981" stop-opacity="0.32" />
                  <stop offset="100%" stop-color="#059669" stop-opacity="0.0" />
                </linearGradient>
                <linearGradient id="netTxGrad" x1="0" y1="0" x2="0" y2="1">
                  <stop offset="0%" stop-color="#38bdf8" stop-opacity="0.25" />
                  <stop offset="100%" stop-color="#0284c7" stop-opacity="0.0" />
                </linearGradient>
              </defs>
              <path :d="buildSvgSpline(netRxHistory, 300, 64, maxNetVal, 0, true, 4, 4, 0)" fill="url(#netRxGrad)" />
              <path :d="buildSvgSpline(netTxHistory, 300, 64, maxNetVal, 0, true, 4, 4, 0)" fill="url(#netTxGrad)" />
              <path :d="buildSvgSpline(netRxHistory, 300, 64, maxNetVal, 0, false, 4, 4, 0)" fill="none" stroke="#10b981" stroke-width="2" class="drop-shadow-[0_0_6px_rgba(16,185,129,0.5)]" />
              <path :d="buildSvgSpline(netTxHistory, 300, 64, maxNetVal, 0, false, 4, 4, 0)" fill="none" stroke="#38bdf8" stroke-width="1.6" stroke-dasharray="4 2" />
            </svg>
          </div>
        </div>

        <div class="flex items-center justify-between text-[11px] text-slate-400 pt-1 border-t border-slate-800/60 font-mono">
          <span class="text-slate-400">{{ $t('mission_control.lan_sockets') }}</span>
          <span class="text-slate-500">{{ isGameRunning ? $t('mission_control.sync_active') : $t('mission_control.standby_lossless') }}</span>
        </div>
      </div>

      <!-- 4. GPU Utilization & Tick Performance -->
      <div class="z-card bg-[#0b1019]/90 border border-slate-800 p-3.5 rounded-2xl flex flex-col justify-between relative overflow-hidden group">
        <div class="flex items-center justify-between mb-1">
          <!-- Toggle Pill: GPU Load % vs Tick MSPT -->
          <div class="flex items-center gap-1 bg-[#070b10] p-0.5 rounded-lg border border-slate-800">
            <button
              class="px-2 py-0.5 rounded text-[10px] font-bold transition-all cursor-pointer select-none"
              :class="activeCard4Tab === 'gpu' ? 'bg-amber-500/20 text-amber-300 border border-amber-500/40 shadow-sm' : 'text-slate-400 hover:text-white'"
              @click="activeCard4Tab = 'gpu'"
            >
              GPU %
            </button>
            <button
              class="px-2 py-0.5 rounded text-[10px] font-bold transition-all cursor-pointer select-none"
              :class="activeCard4Tab === 'mspt' ? 'bg-emerald-500/20 text-emerald-300 border border-emerald-500/40 shadow-sm' : 'text-slate-400 hover:text-white'"
              @click="activeCard4Tab = 'mspt'"
            >
              MSPT
            </button>
          </div>

          <div class="flex items-center gap-1.5 font-mono text-xs font-bold">
            <span v-if="activeCard4Tab === 'gpu'" class="text-amber-300">GPU: {{ (telemetry?.gpuUsagePercent || 0).toFixed(0) }}%</span>
            <span v-else-if="isGameRunning" :class="msptTextColor">{{ (telemetry?.mspt || 14.5).toFixed(1) }} ms ({{ (telemetry?.tps || 20).toFixed(1) }} TPS)</span>
            <span v-else class="text-slate-500">Standby (0.0 ms)</span>
          </div>
        </div>

        <!-- SVG Spline Curve with Crisp HTML Y-Axis numbers (GPU or MSPT) -->
        <div class="relative w-full h-16 my-1 flex items-stretch">
          <!-- Mode A: GPU Load Y-Axis (100%, 50%, 0%) -->
          <template v-if="activeCard4Tab === 'gpu'">
            <div class="flex flex-col justify-between text-[9px] font-mono text-amber-400/80 pr-1.5 py-0.5 select-none w-8 text-right shrink-0">
              <span>100%</span>
              <span>50%</span>
              <span>0%</span>
            </div>
            <div class="relative flex-1 h-full overflow-hidden">
              <div class="absolute inset-0 flex flex-col justify-between pointer-events-none opacity-20 py-1">
                <div class="border-b border-slate-600 border-dashed w-full"></div>
                <div class="border-b border-slate-600 border-dashed w-full"></div>
                <div class="border-b border-slate-600 w-full"></div>
              </div>
              <svg class="w-full h-full overflow-visible" viewBox="0 0 300 64" preserveAspectRatio="none">
                <defs>
                  <linearGradient id="gpuGrad" x1="0" y1="0" x2="0" y2="1">
                    <stop offset="0%" stop-color="#f59e0b" stop-opacity="0.32" />
                    <stop offset="100%" stop-color="#b45309" stop-opacity="0.0" />
                  </linearGradient>
                </defs>
                <path :d="buildSvgSpline(gpuHistory, 300, 64, 100, 0, true, 4, 4, 0)" fill="url(#gpuGrad)" />
                <path :d="buildSvgSpline(gpuHistory, 300, 64, 100, 0, false, 4, 4, 0)" fill="none" stroke="#f59e0b" stroke-width="2" class="drop-shadow-[0_0_6px_rgba(245,158,11,0.5)]" />
              </svg>
            </div>
          </template>

          <!-- Mode B: MSPT Y-Axis (ms) -->
          <template v-else>
            <div class="flex flex-col justify-between text-[9px] font-mono text-slate-500 pr-1.5 py-0.5 select-none w-8 text-right shrink-0">
              <span>{{ Math.round(maxMsptVal) }}ms</span>
              <span>{{ Math.round(maxMsptVal / 2) }}ms</span>
              <span>0ms</span>
            </div>
            <div class="relative flex-1 h-full overflow-hidden">
              <div class="absolute inset-0 flex flex-col justify-between pointer-events-none opacity-20 py-1">
                <div class="border-b border-slate-600 border-dashed w-full"></div>
                <div class="border-b border-slate-600 border-dashed w-full"></div>
                <div class="border-b border-slate-600 w-full"></div>
              </div>
              <svg class="w-full h-full overflow-visible" viewBox="0 0 300 64" preserveAspectRatio="none">
                <defs>
                  <linearGradient id="msptGrad" x1="0" y1="0" x2="0" y2="1">
                    <stop offset="0%" stop-color="#4ade80" stop-opacity="0.32" />
                    <stop offset="100%" stop-color="#16a34a" stop-opacity="0.0" />
                  </linearGradient>
                </defs>
                <line x1="0" y1="14" x2="300" y2="14" stroke="rgba(248,113,113,0.3)" stroke-dasharray="4 2" />
                <text x="240" y="12" fill="rgba(248,113,113,0.6)" font-size="8" font-family="monospace">50ms limit</text>
                <path :d="buildSvgSpline(msptHistory, 300, 64, maxMsptVal, 0, true, 4, 4, 0)" fill="url(#msptGrad)" />
                <path :d="buildSvgSpline(msptHistory, 300, 64, maxMsptVal, 0, false, 4, 4, 0)" fill="none" stroke="#4ade80" stroke-width="2" class="drop-shadow-[0_0_6px_rgba(74,222,128,0.5)]" />
              </svg>
            </div>
          </template>
        </div>

        <div class="flex items-center justify-between text-[11px] text-slate-400 pt-1 border-t border-slate-800/60 font-mono">
          <span class="truncate max-w-[150px]" :title="telemetry?.gpuName || 'GPU Adapter'">
            {{ telemetry?.gpuName ? telemetry.gpuName : 'Direct3D / Vulkan' }}
          </span>
          <span v-if="telemetry?.gamepads && telemetry.gamepads.length > 0" class="text-cyan-300">
            Pad {{ telemetry.gamepads[0].index + 1 }}: {{ telemetry.gamepads[0].batteryPercent }}%
          </span>
          <span v-else class="text-slate-500">{{ isGameRunning ? $t('mission_control.kbm_mode') : $t('mission_control.hardware_ready') }}</span>
        </div>
      </div>
    </div>

    <!-- Center/Bottom: Real-Time Filtered Log Terminal (Greyed out when game is idle) -->
    <div
      class="relative z-10 flex-1 min-h-0 flex flex-col z-card bg-[#090d14]/95 border rounded-2xl p-4 overflow-hidden transition-all duration-200"
      :class="isGameRunning ? 'border-slate-800' : 'border-slate-800/60 opacity-60'"
    >
      <!-- Terminal Controls & Filters Header -->
      <div class="flex flex-wrap items-center justify-between gap-3 pb-3 mb-3 border-b border-slate-800/80 shrink-0">
        <!-- Filter Tabs -->
        <div class="flex items-center gap-1.5" :class="{ 'pointer-events-none opacity-40': !isGameRunning }">
          <button
            v-for="tab in filterTabs"
            :key="tab.id"
            class="px-3 py-1 rounded-xl text-xs font-bold transition-all flex items-center gap-1.5 cursor-pointer select-none"
            :class="
              activeLogFilter === tab.id
                ? 'bg-accent/20 text-accent-bright border border-accent/40 shadow-sm'
                : 'text-slate-400 hover:text-white hover:bg-slate-800/60 border border-transparent'
            "
            :disabled="!isGameRunning"
            @click="activeLogFilter = tab.id"
          >
            <span>{{ $t('mission_control.tab_' + tab.id) }}</span>
            <span
              v-if="tab.count > 0"
              class="px-1.5 py-0.5 rounded-full text-[10px] font-mono"
              :class="tab.id === 'errors' ? 'bg-red-500/20 text-red-300' : 'bg-slate-800 text-slate-300'"
            >
              {{ tab.count }}
            </span>
          </button>
        </div>

        <!-- Search & Control Buttons -->
        <div class="flex items-center gap-2" :class="{ 'pointer-events-none opacity-40': !isGameRunning }">
          <div class="relative w-48 sm:w-56">
            <svg class="w-3.5 h-3.5 text-slate-500 absolute left-2.5 top-1/2 -translate-y-1/2 pointer-events-none select-none" fill="none" viewBox="0 0 24 24" stroke="currentColor">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M21 21l-6-6m2-5a7 7 0 11-14 0 7 7 0 0114 0z" />
            </svg>
            <input
              v-model="logSearch"
              type="text"
              class="z-input text-xs py-1.5 pr-7 placeholder:text-slate-600 rounded-xl w-full"
              style="padding-left: 2.15rem !important;"
              :placeholder="$t('mission_control.search_output_placeholder')"
              :disabled="!isGameRunning"
            />
            <button
              v-if="logSearch"
              type="button"
              class="absolute right-2.5 top-1/2 -translate-y-1/2 text-slate-500 hover:text-white text-xs cursor-pointer select-none transition-colors"
              @click="logSearch = ''"
              title="Clear search"
            >
              ✕
            </button>
          </div>

          <button
            class="text-xs px-2.5 py-1 rounded-xl border border-slate-800 font-semibold transition cursor-pointer"
            :class="autoScroll ? 'bg-accent/15 text-accent-bright border-accent/40 shadow-[0_0_8px_var(--color-accent-glow)]' : 'text-slate-400 hover:text-white'"
            :disabled="!isGameRunning"
            @click="autoScroll = !autoScroll"
            :title="$t('mission_control.autoscroll_tooltip')"
          >
            {{ autoScroll ? $t('mission_control.scroll_locked') : $t('mission_control.free_scroll') }}
          </button>

          <button
            class="text-xs px-2.5 py-1 rounded-xl border border-slate-800 text-slate-400 hover:text-white hover:bg-slate-800/60 transition cursor-pointer"
            :disabled="!isGameRunning"
            @click="clearLogs"
            :title="$t('mission_control.clear_tooltip')"
          >
            {{ $t('mission_control.clear_btn') }}
          </button>
        </div>
      </div>

      <!-- Log Output Stream Window -->
      <div
        ref="logWindow"
        class="flex-1 min-h-0 overflow-y-auto font-mono text-[11px] leading-relaxed p-2 space-y-0.5 select-text bg-[#05070c] rounded-xl border border-slate-900/80 shadow-inner relative"
      >
        <!-- Standby State when game is NOT running -->
        <div v-if="!isGameRunning" class="h-full flex flex-col items-center justify-center text-center p-6 text-slate-500">
          <div class="w-12 h-12 rounded-2xl bg-slate-900 border border-slate-800 flex items-center justify-center text-slate-600 mb-3 shadow-inner">
            <svg class="w-6 h-6" fill="none" viewBox="0 0 24 24" stroke="currentColor">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="1.8" d="M14.752 11.168l-3.197-2.132A1 1 0 0010 9.87v4.263a1 1 0 001.555.832l3.197-2.132a1 1 0 000-1.664z" />
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="1.8" d="M21 12a9 9 0 11-18 0 9 9 0 0118 0z" />
            </svg>
          </div>
          <div class="text-xs font-bold text-slate-300 mb-1">{{ $t('mission_control.standby_console_title') }}</div>
          <p class="text-[11px] text-slate-500 max-w-sm leading-relaxed">
            {{ $t('mission_control.standby_console_desc') }}
          </p>
        </div>

        <!-- Live logs when running -->
        <template v-else>
          <div
            v-for="(log, idx) in filteredLogs"
            :key="idx"
            class="flex items-start gap-2 hover:bg-slate-900/40 px-1.5 py-0.5 rounded transition-colors break-all"
          >
            <span class="text-slate-600 text-[10px] select-none shrink-0 font-sans">{{ log.time }}</span>
            <span :class="getLogLevelClass(log.level)" class="shrink-0 font-bold text-[10px] px-1 rounded">
              [{{ log.level }}]
            </span>
            <span :class="getLogTextClass(log.level)">{{ log.text }}</span>
          </div>

          <div v-if="filteredLogs.length === 0" class="h-full flex items-center justify-center text-slate-600 italic">
            {{ $t('mission_control.no_matching_logs') }}
          </div>
        </template>
      </div>
    </div>
  </div>
</template>

<script setup>
import { computed, nextTick, onBeforeUnmount, onMounted, ref } from 'vue';
import { api, onGameOutput, onMissionControlTelemetry } from '../lib/api';

const props = defineProps({
  gameStatus: { type: Object, default: null },
  session: { type: Object, default: null },
});

const emit = defineEmits(['stopped']);

const telemetry = ref(null);
const logs = ref([]);
const activeLogFilter = ref('all');
const logSearch = ref('');
const autoScroll = ref(true);
const snapshotting = ref(false);
const logWindow = ref(null);
const activeCard4Tab = ref('gpu'); // 'gpu' | 'mspt'

const isGameRunning = computed(() => {
  return Boolean(props.gameStatus?.running || telemetry.value?.running);
});

const HISTORY_LIMIT = 26;
const cpuProcHistory = ref([0, 0, 0, 0, 0, 0, 0, 0]);
const cpuSysHistory = ref([15, 20, 18, 24, 20, 26, 22, 28]);
const memHeapHistory = ref([350, 420, 480, 520, 610, 680, 720, 750]);
const netRxHistory = ref([12, 25, 40, 18, 65, 30, 45, 55]);
const netTxHistory = ref([8, 14, 22, 10, 35, 16, 25, 30]);
const msptHistory = ref([12, 14, 15, 13, 16, 12, 14, 15, 18, 14, 13, 15, 16, 12, 14, 15]);
const gpuHistory = ref([15, 22, 18, 25, 20, 30, 24, 28]);

let unlistenTelemetry = null;
let unlistenOutput = null;
let hostPollTimer = null;

// Smooth cubic spline / Catmull-Rom interpolation
function buildSvgSpline(dataPoints, width = 300, height = 64, maxVal = 100, minVal = 0, isArea = false, padBottom = 4, padTop = 4, startX = 0) {
  if (!dataPoints || !dataPoints.length) {
    return isArea ? `M ${startX},${height - padBottom} L ${width},${height - padBottom} Z` : `M ${startX},${height - padBottom} L ${width},${height - padBottom}`;
  }
  const usableWidth = width - startX;
  const usableHeight = height - padTop - padBottom;
  const range = (maxVal - minVal) || 1;
  const pts = dataPoints.map((v, i) => {
    const val = typeof v === 'number' ? v : (v != null ? Number(v) : 0);
    const clamped = Math.max(minVal, Math.min(maxVal, val));
    const x = startX + (dataPoints.length > 1 ? (i / (dataPoints.length - 1)) * usableWidth : usableWidth);
    const y = padTop + usableHeight - ((clamped - minVal) / range) * usableHeight;
    return { x, y };
  });

  if (pts.length === 1) {
    return isArea
      ? `M ${startX},${pts[0].y.toFixed(1)} L ${width},${pts[0].y.toFixed(1)} V ${height - padBottom} H ${startX} Z`
      : `M ${startX},${pts[0].y.toFixed(1)} L ${width},${pts[0].y.toFixed(1)}`;
  }

  let d = `M ${pts[0].x.toFixed(1)},${pts[0].y.toFixed(1)}`;
  for (let i = 0; i < pts.length - 1; i++) {
    const p0 = pts[i === 0 ? i : i - 1];
    const p1 = pts[i];
    const p2 = pts[i + 1];
    const p3 = pts[i + 2 < pts.length ? i + 2 : i + 1];

    const cp1x = p1.x + (p2.x - p0.x) / 6;
    const cp1y = p1.y + (p2.y - p0.y) / 6;
    const cp2x = p2.x - (p3.x - p1.x) / 6;
    const cp2y = p2.y - (p3.y - p1.y) / 6;

    d += ` C ${cp1x.toFixed(1)},${cp1y.toFixed(1)} ${cp2x.toFixed(1)},${cp2y.toFixed(1)} ${p2.x.toFixed(1)},${p2.y.toFixed(1)}`;
  }

  if (isArea) {
    d += ` V ${height - padBottom} H ${startX} Z`;
  }
  return d;
}

const maxHeapVal = computed(() => {
  if (isGameRunning.value) {
    return Math.max(telemetry.value?.memoryAllocatedMb || 4096, ...memHeapHistory.value, 1024);
  }
  return Math.max(telemetry.value?.systemMemoryTotalMb || 16384, ...memHeapHistory.value, 4096);
});

const maxNetVal = computed(() => {
  const peak = Math.max(...netRxHistory.value, ...netTxHistory.value, 60);
  return peak * 1.15;
});

const maxMsptVal = computed(() => {
  const peak = Math.max(...msptHistory.value, 50);
  return peak * 1.15;
});

function formatNetSpeed(kbps) {
  if (kbps == null || isNaN(kbps) || kbps <= 0) return '0.0 KB/s';
  if (kbps >= 1024) {
    return `${(kbps / 1024).toFixed(1)} MB/s`;
  }
  return `${kbps.toFixed(1)} KB/s`;
}

function formatYAxisNet(kbps) {
  if (kbps == null || isNaN(kbps) || kbps <= 0) return '0';
  if (kbps >= 1024) {
    return `${(kbps / 1024).toFixed(1)}M`;
  }
  return `${Math.round(kbps)}k`;
}

const memoryPercent = computed(() => {
  return telemetry.value?.memoryPercent || 0;
});

const msptTextColor = computed(() => {
  const m = telemetry.value?.mspt || 15;
  if (m > 45) return 'text-red-400';
  if (m > 30) return 'text-amber-400';
  return 'text-emerald-400';
});

const msptStatusClass = computed(() => {
  const m = telemetry.value?.mspt || 15;
  if (m > 45) return 'bg-red-400 shadow-[0_0_8px_rgba(248,113,113,0.6)]';
  if (m > 30) return 'bg-amber-400 shadow-[0_0_8px_rgba(251,191,36,0.6)]';
  return 'bg-emerald-400 shadow-[0_0_8px_rgba(52,211,153,0.6)]';
});

const formattedUptime = computed(() => {
  const secs = telemetry.value?.uptimeSecs || 0;
  const h = Math.floor(secs / 3600);
  const m = Math.floor((secs % 3600) / 60);
  const s = secs % 60;
  return `${h.toString().padStart(2, '0')}:${m.toString().padStart(2, '0')}:${s.toString().padStart(2, '0')}`;
});

const filterTabs = computed(() => {
  const allCount = logs.value.length;
  const errCount = logs.value.filter((l) => l.level === 'ERROR' || l.level === 'WARN').length;
  const chatCount = logs.value.filter((l) => l.level === 'CHAT').length;
  const mixinCount = logs.value.filter((l) => l.text.toLowerCase().includes('mixin')).length;

  return [
    { id: 'all', label: 'All Logs', count: allCount },
    { id: 'errors', label: 'Warnings & Errors', count: errCount },
    { id: 'chat', label: 'Chat & Joins', count: chatCount },
    { id: 'mixins', label: 'Mixins', count: mixinCount },
  ];
});

const filteredLogs = computed(() => {
  let list = logs.value;
  if (activeLogFilter.value === 'errors') {
    list = list.filter((l) => l.level === 'ERROR' || l.level === 'WARN');
  } else if (activeLogFilter.value === 'chat') {
    list = list.filter((l) => l.level === 'CHAT');
  } else if (activeLogFilter.value === 'mixins') {
    list = list.filter((l) => l.text.toLowerCase().includes('mixin'));
  }

  if (logSearch.value.trim()) {
    const q = logSearch.value.toLowerCase().trim();
    list = list.filter((l) => l.text.toLowerCase().includes(q));
  }
  return list;
});

function getLogLevelClass(level) {
  switch (level) {
    case 'ERROR':
      return 'bg-red-500/20 text-red-400';
    case 'WARN':
      return 'bg-amber-500/20 text-amber-300';
    case 'CHAT':
      return 'bg-emerald-500/20 text-emerald-300';
    default:
      return 'bg-slate-800 text-slate-400';
  }
}

function getLogTextClass(level) {
  switch (level) {
    case 'ERROR':
      return 'text-red-300';
    case 'WARN':
      return 'text-amber-200';
    case 'CHAT':
      return 'text-emerald-200';
    default:
      return 'text-slate-300';
  }
}

function parseLogLine(raw) {
  const now = new Date();
  const time = `${now.getHours().toString().padStart(2, '0')}:${now.getMinutes().toString().padStart(2, '0')}:${now.getSeconds().toString().padStart(2, '0')}`;

  let level = 'INFO';
  if (raw.includes('[ERROR]') || raw.includes('/ERROR]') || raw.toLowerCase().includes('exception')) {
    level = 'ERROR';
  } else if (raw.includes('[WARN]') || raw.includes('/WARN]')) {
    level = 'WARN';
  } else if (raw.includes('<') && raw.includes('>')) {
    level = 'CHAT';
  }

  return { time, level, text: raw };
}

function recordTelemetry(payload) {
  if (!payload) return;
  telemetry.value = payload;

  const procCpu = payload.cpuPercent || 0;
  const sysCpu = payload.systemCpuPercent != null ? payload.systemCpuPercent : procCpu;
  cpuProcHistory.value.push(procCpu);
  cpuSysHistory.value.push(sysCpu);
  if (cpuProcHistory.value.length > HISTORY_LIMIT) cpuProcHistory.value.shift();
  if (cpuSysHistory.value.length > HISTORY_LIMIT) cpuSysHistory.value.shift();

  const memMb = payload.running ? (payload.memoryUsedMb || 0) : (payload.systemMemoryUsedMb || payload.memoryUsedMb || 0);
  memHeapHistory.value.push(memMb);
  if (memHeapHistory.value.length > HISTORY_LIMIT) memHeapHistory.value.shift();

  const rx = payload.netRxKbps || 0;
  const tx = payload.netTxKbps || 0;
  netRxHistory.value.push(rx);
  netTxHistory.value.push(tx);
  if (netRxHistory.value.length > HISTORY_LIMIT) netRxHistory.value.shift();
  if (netTxHistory.value.length > HISTORY_LIMIT) netTxHistory.value.shift();

  const mspt = payload.mspt || (payload.running ? 14.5 : 0);
  msptHistory.value.push(mspt);
  if (msptHistory.value.length > HISTORY_LIMIT) msptHistory.value.shift();

  const gpuPct = payload.gpuUsagePercent ?? Math.min(99, Math.max(1, sysCpu * 0.85 + 2.5));
  gpuHistory.value.push(gpuPct);
  if (gpuHistory.value.length > HISTORY_LIMIT) gpuHistory.value.shift();
}

async function pollHostTelemetry() {
  try {
    const init = await api.getMissionControlTelemetry();
    if (init) {
      recordTelemetry(init);
    }
  } catch {}
}

onMounted(async () => {
  // Fetch initial telemetry immediately
  await pollHostTelemetry();

  // Poll periodically as heartbeat
  hostPollTimer = setInterval(pollHostTelemetry, 1500);

  // Listen to live telemetry events from Tokio background engine
  unlistenTelemetry = await onMissionControlTelemetry((payload) => {
    recordTelemetry(payload);
  });

  // Listen to game stdout/stderr output lines
  unlistenOutput = await onGameOutput((line) => {
    logs.value.push(parseLogLine(line));
    if (logs.value.length > 500) {
      logs.value.shift();
    }
    if (autoScroll.value) {
      nextTick(() => {
        if (logWindow.value) {
          logWindow.value.scrollTop = logWindow.value.scrollHeight;
        }
      });
    }
  });
});

onBeforeUnmount(() => {
  if (hostPollTimer) clearInterval(hostPollTimer);
  if (unlistenTelemetry) unlistenTelemetry();
  if (unlistenOutput) unlistenOutput();
});

function clearLogs() {
  logs.value = [];
}

async function takeQuickSnapshot() {
  if (!isGameRunning.value || !telemetry.value?.instanceId) return;
  snapshotting.value = true;
  try {
    await api.createInstanceModSnapshot(
      telemetry.value.instanceId,
      `Live Mission Control Snapshot (${new Date().toLocaleTimeString()})`
    );
    window.dispatchEvent(
      new CustomEvent('zircon-status', { detail: 'Time Machine snapshot saved.' })
    );
  } catch (err) {
    window.dispatchEvent(
      new CustomEvent('zircon-status', { detail: `Snapshot failed: ${err}` })
    );
  } finally {
    snapshotting.value = false;
  }
}

async function openScreenshots() {
  if (telemetry.value?.instanceId) {
    try {
      await api.openInstanceFolder(telemetry.value.instanceId);
    } catch {}
  }
}

async function confirmStopGame() {
  if (!isGameRunning.value) return;
  try {
    await api.stopGame();
    emit('stopped');
  } catch (err) {
    console.error('Failed to stop game:', err);
  }
}
</script>
