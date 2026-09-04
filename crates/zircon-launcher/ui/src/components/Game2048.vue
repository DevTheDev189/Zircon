<template>
  <div class="game-2048-container flex flex-col h-full select-none bg-[#09111c]/90 rounded-2xl border border-cyan-500/25 p-4 shadow-xl shadow-black/50 backdrop-blur-md">
    <!-- Header: Title, Scores, Actions -->
    <div class="flex items-center justify-between gap-2 mb-3">
      <div>
        <div class="flex items-center gap-1.5">
          <span class="text-xs font-black tracking-wider text-accent uppercase font-mono">2048</span>
          <span class="text-[10px] px-1.5 py-0.5 rounded bg-cyan-500/20 text-cyan-300 font-bold tracking-tight">Mini</span>
        </div>
        <p class="text-[10px] text-slate-400 font-medium">Join tiles to reach 2048!</p>
      </div>

      <div class="flex items-center gap-2">
        <!-- Score & Best Boxes -->
        <div class="flex gap-1.5">
          <div class="px-2.5 py-1 rounded-xl bg-slate-900/90 border border-slate-800 text-center min-w-[50px]">
            <span class="block text-[9px] uppercase tracking-wider text-slate-400 font-bold">Score</span>
            <span class="block text-xs font-extrabold text-cyan-200 font-mono leading-none mt-0.5">{{ score }}</span>
          </div>
          <div class="px-2.5 py-1 rounded-xl bg-slate-900/90 border border-slate-800 text-center min-w-[50px]">
            <span class="block text-[9px] uppercase tracking-wider text-slate-400 font-bold">Best</span>
            <span class="block text-xs font-extrabold text-accent font-mono leading-none mt-0.5">{{ bestScore }}</span>
          </div>
        </div>

        <!-- Reset Button -->
        <button
          type="button"
          class="p-1.5 rounded-xl border border-slate-700/80 bg-slate-800/80 hover:bg-cyan-500/20 hover:border-cyan-400 text-slate-300 hover:text-cyan-200 transition-all text-xs"
          title="Restart Game"
          @click="resetGame"
        >
          <svg class="w-3.5 h-3.5" viewBox="0 0 20 20" fill="currentColor">
            <path fill-rule="evenodd" d="M4 2a1 1 0 011 1v2.101a7.002 7.002 0 0111.601 2.566 1 1 0 11-1.885.666A5.002 5.002 0 005.999 7H9a1 1 0 010 2H4a1 1 0 01-1-1V3a1 1 0 011-1zm.008 9.057a1 1 0 011.276.61A5.002 5.002 0 0014.001 13H11a1 1 0 110-2h5a1 1 0 011 1v5a1 1 0 11-2 0v-2.101a7.002 7.002 0 01-11.601-2.566 1 1 0 01.61-1.276z" clip-rule="evenodd" />
          </svg>
        </button>

        <!-- Hide/Close Button -->
        <button
          type="button"
          class="p-1.5 rounded-xl border border-slate-700/80 bg-slate-800/80 hover:bg-rose-500/20 hover:border-rose-400/60 text-slate-300 hover:text-rose-300 transition-all text-xs"
          title="Hide 2048 Minigame"
          @click="$emit('close')"
        >
          <svg class="w-3.5 h-3.5" viewBox="0 0 20 20" fill="currentColor">
            <path fill-rule="evenodd" d="M4.293 4.293a1 1 0 011.414 0L10 8.586l4.293-4.293a1 1 0 111.414 1.414L11.414 10l4.293 4.293a1 1 0 01-1.414 1.414L10 11.414l-4.293 4.293a1 1 0 01-1.414-1.414L8.586 10 4.293 5.707a1 1 0 010-1.414z" clip-rule="evenodd" />
          </svg>
        </button>
      </div>
    </div>

    <!-- 4x4 Square Game Board -->
    <div
      ref="boardRef"
      class="relative w-full aspect-square max-w-[280px] mx-auto bg-[#050a10] rounded-xl p-2 border border-slate-800/90 shadow-inner touch-none overflow-hidden select-none"
      tabindex="0"
      @keydown="handleKeyDown"
      @touchstart="handleTouchStart"
      @touchend="handleTouchEnd"
    >
      <!-- Background Grid (16 empty cells) -->
      <div class="grid grid-cols-4 grid-rows-4 gap-2 w-full h-full">
        <div
          v-for="i in 16"
          :key="i"
          class="rounded-lg bg-[#0d1826]/70 border border-slate-800/60 w-full h-full min-h-0 min-w-0"
        />
      </div>

      <!-- Foreground Sliding Tiles Layer -->
      <div class="absolute inset-2 pointer-events-none">
        <div
          v-for="tile in activeTiles"
          :key="tile.id"
          class="tile-wrapper absolute"
          :style="getTilePositionStyle(tile.row, tile.col)"
        >
          <div
            class="tile-inner w-full h-full flex items-center justify-center rounded-lg font-bold text-center select-none shadow-md"
            :class="[getTileClass(tile.value), tile.isNew ? 'tile-appear' : '', tile.isMerged ? 'tile-pop' : '']"
          >
            <span
              class="leading-none select-none font-bold"
              :class="getTileTextSize(tile.value)"
            >
              {{ tile.value }}
            </span>
          </div>
        </div>
      </div>

      <!-- Game Over Overlay -->
      <div
        v-if="gameOver"
        class="absolute inset-0 z-20 bg-slate-950/85 backdrop-blur-sm rounded-xl flex flex-col items-center justify-center p-4 text-center animate-fade-in"
      >
        <span class="text-xs font-bold uppercase tracking-widest text-rose-400 mb-1">Game Over</span>
        <p class="text-sm font-extrabold text-white mb-3">No more moves!</p>
        <button
          type="button"
          class="z-btn-accent text-xs font-bold px-4 py-1.5 rounded-xl shadow-lg hover:shadow-accent/30 transition-all"
          @click="resetGame"
        >
          Try Again
        </button>
      </div>

      <!-- Win Overlay (2048 reached) -->
      <div
        v-else-if="gameWon && !dismissedWin"
        class="absolute inset-0 z-20 bg-cyan-950/90 backdrop-blur-sm rounded-xl flex flex-col items-center justify-center p-4 text-center animate-fade-in border border-accent/40 shadow-[0_0_20px_rgba(71,210,201,0.3)]"
      >
        <span class="text-xs font-black uppercase tracking-widest text-accent mb-1 drop-shadow-[0_0_8px_#47d2c9]">Victory!</span>
        <p class="text-sm font-extrabold text-white mb-3">You reached 2048!</p>
        <div class="flex gap-2">
          <button
            type="button"
            class="z-btn-ghost text-[11px] font-bold px-3 py-1.5 rounded-xl border border-slate-700 text-slate-300 hover:text-white"
            @click="dismissedWin = true"
          >
            Keep Going
          </button>
          <button
            type="button"
            class="z-btn-accent text-[11px] font-bold px-3 py-1.5 rounded-xl shadow-md"
            @click="resetGame"
          >
            New Game
          </button>
        </div>
      </div>
    </div>

    <!-- Footer Controls Hint -->
    <div class="mt-2.5 flex items-center justify-between text-[10px] text-slate-400 font-mono px-1">
      <span class="flex items-center gap-1">
        <kbd class="px-1.5 py-0.5 rounded bg-slate-800 border border-slate-700 text-cyan-300 font-sans font-bold">↑</kbd>
        <kbd class="px-1.5 py-0.5 rounded bg-slate-800 border border-slate-700 text-cyan-300 font-sans font-bold">↓</kbd>
        <kbd class="px-1.5 py-0.5 rounded bg-slate-800 border border-slate-700 text-cyan-300 font-sans font-bold">←</kbd>
        <kbd class="px-1.5 py-0.5 rounded bg-slate-800 border border-slate-700 text-cyan-300 font-sans font-bold">→</kbd>
        <span class="ml-1 text-slate-400">or WASD</span>
      </span>
      <span class="text-slate-500 hover:text-slate-400 cursor-pointer" @click="$emit('close')">
        ✕ Hide
      </span>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, onMounted, onUnmounted } from 'vue';

const emit = defineEmits<{
  (e: 'close'): void;
}>();

interface Tile {
  id: number;
  value: number;
  row: number;
  col: number;
  isNew?: boolean;
  isMerged?: boolean;
}

const boardRef = ref<HTMLElement | null>(null);

let nextTileId = 1;
const activeTiles = ref<Tile[]>([]);

const score = ref(0);
const bestScore = ref(0);
const gameOver = ref(false);
const gameWon = ref(false);
const dismissedWin = ref(false);

const STORAGE_STATE_KEY = 'zircon_2048_state';
const STORAGE_BEST_KEY = 'zircon_2048_best';

// --- Responsive Tile Position Style ---
function getTilePositionStyle(row: number, col: number) {
  return {
    width: 'calc((100% - 24px) / 4)',
    height: 'calc((100% - 24px) / 4)',
    transform: `translate(calc(${col} * (100% + 8px)), calc(${row} * (100% + 8px)))`,
  };
}

// --- Initialization & State Persistence ---
function loadSavedGame() {
  try {
    const savedBest = localStorage.getItem(STORAGE_BEST_KEY);
    if (savedBest) {
      bestScore.value = parseInt(savedBest, 10) || 0;
    }

    const savedState = localStorage.getItem(STORAGE_STATE_KEY);
    if (savedState) {
      const parsed = JSON.parse(savedState);
      if (Array.isArray(parsed.tiles) && parsed.tiles.length > 0) {
        activeTiles.value = parsed.tiles.map((t: any) => ({
          id: nextTileId++,
          value: t.value,
          row: t.row,
          col: t.col,
        }));
        score.value = parsed.score || 0;
        gameOver.value = parsed.gameOver || false;
        gameWon.value = parsed.gameWon || false;
        dismissedWin.value = parsed.dismissedWin || false;
        return;
      }
    }
  } catch {
    // Ignore storage errors and start fresh
  }
  startNewGame();
}

function saveGameState() {
  try {
    if (score.value > bestScore.value) {
      bestScore.value = score.value;
      localStorage.setItem(STORAGE_BEST_KEY, bestScore.value.toString());
    }
    localStorage.setItem(
      STORAGE_STATE_KEY,
      JSON.stringify({
        tiles: activeTiles.value.map((t) => ({
          value: t.value,
          row: t.row,
          col: t.col,
        })),
        score: score.value,
        gameOver: gameOver.value,
        gameWon: gameWon.value,
        dismissedWin: dismissedWin.value,
      })
    );
  } catch {
    // Ignore storage errors
  }
}

function startNewGame() {
  activeTiles.value = [];
  score.value = 0;
  gameOver.value = false;
  gameWon.value = false;
  dismissedWin.value = false;
  spawnTile();
  spawnTile();
  saveGameState();
}

function resetGame() {
  startNewGame();
  focusBoard();
}

function focusBoard() {
  if (boardRef.value) {
    boardRef.value.focus();
  }
}

// --- Spawning Tiles ---
function spawnTile() {
  const occupied = new Set(activeTiles.value.map((t) => `${t.row}-${t.col}`));
  const emptyCells: { row: number; col: number }[] = [];

  for (let r = 0; r < 4; r++) {
    for (let c = 0; c < 4; c++) {
      if (!occupied.has(`${r}-${c}`)) {
        emptyCells.push({ row: r, col: c });
      }
    }
  }

  if (emptyCells.length === 0) return;
  const rand = emptyCells[Math.floor(Math.random() * emptyCells.length)];
  const newTile: Tile = {
    id: nextTileId++,
    value: Math.random() < 0.9 ? 2 : 4,
    row: rand.row,
    col: rand.col,
    isNew: true,
  };
  activeTiles.value.push(newTile);
}

// --- Sliding Movement with CSS Transition ---
function move(direction: 'left' | 'right' | 'up' | 'down') {
  if (gameOver.value) return;

  // Clear previous transient animation flags
  for (const t of activeTiles.value) {
    t.isNew = false;
    t.isMerged = false;
  }

  // Build current 4x4 matrix of tile references
  const grid: (Tile | null)[][] = Array.from({ length: 4 }, () => [null, null, null, null]);
  for (const tile of activeTiles.value) {
    grid[tile.row][tile.col] = tile;
  }

  let anyMoved = false;
  let scoreGained = 0;
  const tilesToRemove = new Set<number>();
  const tilesToAdd: Tile[] = [];

  const isHorizontal = direction === 'left' || direction === 'right';
  const isForward = direction === 'right' || direction === 'down';

  for (let line = 0; line < 4; line++) {
    // Collect non-empty tiles along line in directional order
    const lineTiles: Tile[] = [];
    for (let i = 0; i < 4; i++) {
      const pos = isForward ? 3 - i : i;
      const r = isHorizontal ? line : pos;
      const c = isHorizontal ? pos : line;
      const t = grid[r][c];
      if (t) lineTiles.push(t);
    }

    let targetIdx = 0;
    for (let i = 0; i < lineTiles.length; i++) {
      const current = lineTiles[i];
      const next = i + 1 < lineTiles.length ? lineTiles[i + 1] : null;

      const targetPos = isForward ? 3 - targetIdx : targetIdx;
      const targetRow = isHorizontal ? line : targetPos;
      const targetCol = isHorizontal ? targetPos : line;

      if (next && next.value === current.value) {
        // Merge current and next!
        anyMoved = true;
        const mergedValue = current.value * 2;
        scoreGained += mergedValue;
        if (mergedValue === 2048) gameWon.value = true;

        current.row = targetRow;
        current.col = targetCol;
        next.row = targetRow;
        next.col = targetCol;

        tilesToRemove.add(current.id);
        tilesToRemove.add(next.id);

        tilesToAdd.push({
          id: nextTileId++,
          value: mergedValue,
          row: targetRow,
          col: targetCol,
          isMerged: true,
        });

        i++; // Skip next tile since it merged
        targetIdx++;
      } else {
        // Slide current to target position
        if (current.row !== targetRow || current.col !== targetCol) {
          anyMoved = true;
          current.row = targetRow;
          current.col = targetCol;
        }
        targetIdx++;
      }
    }
  }

  if (anyMoved) {
    score.value += scoreGained;
    activeTiles.value = activeTiles.value.filter((t) => !tilesToRemove.has(t.id)).concat(tilesToAdd);

    setTimeout(() => {
      spawnTile();
      checkGameOver();
      saveGameState();
    }, 130);
  }
}

function checkGameOver() {
  if (activeTiles.value.length < 16) return;

  const grid: number[][] = Array.from({ length: 4 }, () => [0, 0, 0, 0]);
  for (const t of activeTiles.value) {
    grid[t.row][t.col] = t.value;
  }

  for (let r = 0; r < 4; r++) {
    for (let c = 0; c < 4; c++) {
      const val = grid[r][c];
      if (r + 1 < 4 && grid[r + 1][c] === val) return;
      if (c + 1 < 4 && grid[r][c + 1] === val) return;
    }
  }

  gameOver.value = true;
}

// --- Key & Touch Handlers ---
function handleKeyDown(e: KeyboardEvent) {
  const key = e.key.toLowerCase();
  if (['arrowleft', 'arrowright', 'arrowup', 'arrowdown', 'a', 'd', 'w', 's'].includes(key)) {
    e.preventDefault();
  }

  if (key === 'arrowleft' || key === 'a') {
    move('left');
  } else if (key === 'arrowright' || key === 'd') {
    move('right');
  } else if (key === 'arrowup' || key === 'w') {
    move('up');
  } else if (key === 'arrowdown' || key === 's') {
    move('down');
  }
}

let touchStartX = 0;
let touchStartY = 0;

function handleTouchStart(e: TouchEvent) {
  if (e.touches.length === 1) {
    touchStartX = e.touches[0].clientX;
    touchStartY = e.touches[0].clientY;
  }
}

function handleTouchEnd(e: TouchEvent) {
  if (e.changedTouches.length === 1) {
    const deltaX = e.changedTouches[0].clientX - touchStartX;
    const deltaY = e.changedTouches[0].clientY - touchStartY;
    const minSwipeDist = 25;

    if (Math.abs(deltaX) > Math.abs(deltaY)) {
      if (Math.abs(deltaX) > minSwipeDist) {
        move(deltaX > 0 ? 'right' : 'left');
      }
    } else {
      if (Math.abs(deltaY) > minSwipeDist) {
        move(deltaY > 0 ? 'down' : 'up');
      }
    }
  }
}

// --- Tile Styling (Cyan & Blue Brand Palette) ---
function getTileClass(val: number): string {
  switch (val) {
    case 2:
      return 'bg-[#102235] text-cyan-200 border border-cyan-900/60 shadow-sm';
    case 4:
      return 'bg-[#132c48] text-cyan-100 border border-cyan-800/70 shadow-sm';
    case 8:
      return 'bg-[#163a62] text-cyan-50 border border-cyan-700/80 shadow-md shadow-cyan-950/40';
    case 16:
      return 'bg-[#174d82] text-white border border-cyan-600/80 shadow-md shadow-cyan-900/50';
    case 32:
      return 'bg-[#0e618e] text-white border border-cyan-500/80 shadow-lg shadow-cyan-900/50';
    case 64:
      return 'bg-[#0a7a9e] text-white border border-cyan-400/90 shadow-lg shadow-cyan-800/60 font-extrabold';
    case 128:
      return 'bg-[#0895b6] text-white border border-cyan-300 shadow-xl shadow-cyan-700/60 font-extrabold';
    case 256:
      return 'bg-[#07b3d3] text-slate-950 border border-cyan-200 shadow-xl shadow-cyan-600/70 font-black';
    case 512:
      return 'bg-[#10cbe6] text-slate-950 border border-white shadow-2xl shadow-cyan-500/80 font-black';
    case 1024:
      return 'bg-[#2be2f5] text-slate-950 border border-white shadow-2xl shadow-cyan-400/90 font-black';
    case 2048:
      return 'bg-gradient-to-br from-[#5adfd5] via-[#47d2c9] to-[#20b2aa] text-slate-950 border-2 border-white shadow-[0_0_25px_rgba(71,210,201,0.9)] animate-pulse font-black';
    default:
      return 'bg-gradient-to-br from-white via-cyan-200 to-cyan-400 text-slate-950 border-2 border-cyan-100 shadow-[0_0_30px_rgba(255,255,255,0.9)] font-black';
  }
}

function getTileTextSize(val: number): string {
  if (val < 100) return 'text-base sm:text-lg';
  if (val < 1000) return 'text-sm sm:text-base';
  if (val < 10000) return 'text-xs';
  return 'text-[10px]';
}

onMounted(() => {
  loadSavedGame();
  window.addEventListener('keydown', handleKeyDown);
});

onUnmounted(() => {
  window.removeEventListener('keydown', handleKeyDown);
});
</script>

<style scoped>
.game-2048-container:focus,
.game-2048-container *:focus {
  outline: none;
}

/* Sliding wrapper: smoothly glides across X & Y coordinates */
.tile-wrapper {
  transition: transform 120ms cubic-bezier(0.25, 1, 0.5, 1);
  will-change: transform;
}

/* Pop-in spawn animation on inner tile */
@keyframes tileAppear {
  0% {
    opacity: 0;
    transform: scale(0);
  }
  100% {
    opacity: 1;
    transform: scale(1);
  }
}

.tile-appear {
  animation: tileAppear 140ms ease-out forwards;
}

/* Merge pop animation on inner tile */
@keyframes tilePop {
  0% {
    transform: scale(0.85);
  }
  50% {
    transform: scale(1.15);
  }
  100% {
    transform: scale(1);
  }
}

.tile-pop {
  animation: tilePop 160ms ease-out forwards;
}

@keyframes fadeIn {
  from {
    opacity: 0;
    transform: scale(0.95);
  }
  to {
    opacity: 1;
    transform: scale(1);
  }
}

.animate-fade-in {
  animation: fadeIn 0.2s ease-out forwards;
}
</style>
