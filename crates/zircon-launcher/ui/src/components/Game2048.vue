<template>
  <div
    class="game-2048-container flex flex-col h-full select-none rounded-2xl p-4 transition-all duration-300"
    :class="[
      embedded
        ? 'bg-[#080e17]/80 border border-teal-500/25 shadow-xl shadow-black/40 backdrop-blur-md'
        : 'bg-[#09111c]/95 border border-cyan-500/30 shadow-2xl shadow-black/60 backdrop-blur-lg'
    ]"
  >
    <!-- Header: Title, Scores, Actions -->
    <div class="flex items-center justify-between gap-2 mb-3">
      <div class="flex items-center gap-2">
        <div class="flex h-7 w-7 items-center justify-center rounded-lg bg-teal-500/15 border border-teal-400/30 text-teal-300">
          <svg class="h-4 w-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <rect x="3" y="3" width="7" height="7" rx="1.5" stroke-width="2" />
            <rect x="14" y="3" width="7" height="7" rx="1.5" stroke-width="2" />
            <rect x="14" y="14" width="7" height="7" rx="1.5" stroke-width="2" />
            <rect x="3" y="14" width="7" height="7" rx="1.5" stroke-width="2" />
          </svg>
        </div>
        <div>
          <div class="flex items-center gap-1.5">
            <span class="text-xs font-black tracking-wider text-teal-300 uppercase font-mono">2048</span>
            <span class="text-[9px] px-1.5 py-0.5 rounded-full bg-teal-500/20 text-teal-200 font-bold tracking-tight border border-teal-500/30">
              MINIGAME
            </span>
          </div>
          <p class="text-[10px] text-slate-400 font-medium leading-none mt-0.5">Join tiles to reach 2048!</p>
        </div>
      </div>

      <div class="flex items-center gap-2">
        <!-- Score & Best Boxes -->
        <div class="flex gap-1.5">
          <div class="px-2.5 py-1 rounded-xl bg-[#0b1420] border border-teal-500/20 text-center min-w-[52px] shadow-sm">
            <span class="block text-[8px] uppercase tracking-wider text-slate-400 font-bold">Score</span>
            <span class="block text-xs font-black text-teal-200 font-mono leading-none mt-0.5">{{ score }}</span>
          </div>
          <div class="px-2.5 py-1 rounded-xl bg-[#0b1420] border border-emerald-500/20 text-center min-w-[52px] shadow-sm">
            <span class="block text-[8px] uppercase tracking-wider text-emerald-400/80 font-bold">Best</span>
            <span class="block text-xs font-black text-emerald-300 font-mono leading-none mt-0.5">{{ bestScore }}</span>
          </div>
        </div>

        <!-- Reset Button -->
        <button
          type="button"
          class="p-1.5 rounded-xl border border-white/10 bg-white/5 hover:bg-teal-500/20 hover:border-teal-400/50 text-slate-300 hover:text-teal-200 transition-all text-xs"
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
          class="p-1.5 rounded-xl border border-white/10 bg-white/5 hover:bg-rose-500/20 hover:border-rose-400/60 text-slate-400 hover:text-rose-300 transition-all text-xs"
          title="Minimize Minigame"
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
      class="relative w-full aspect-square max-w-[270px] mx-auto bg-[#04080e] rounded-xl p-2 border border-teal-500/20 shadow-inner touch-none overflow-hidden select-none"
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
          class="rounded-lg bg-[#0c1522]/80 border border-slate-800/80 w-full h-full min-h-0 min-w-0"
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
            class="tile-inner w-full h-full flex items-center justify-center rounded-lg font-bold text-center select-none shadow-md backdrop-blur-xs transition-colors"
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
        class="absolute inset-0 z-20 bg-[#05090f]/90 backdrop-blur-md rounded-xl flex flex-col items-center justify-center p-4 text-center animate-fade-in border border-rose-500/30"
      >
        <span class="text-[10px] font-black uppercase tracking-widest text-rose-400 mb-1">Game Over</span>
        <p class="text-sm font-extrabold text-white mb-3">No more moves!</p>
        <button
          type="button"
          class="z-btn-accent text-xs font-bold px-4 py-1.5 rounded-xl shadow-lg hover:shadow-teal-400/30 transition-all active:scale-95"
          @click="resetGame"
        >
          Try Again
        </button>
      </div>

      <!-- Win Overlay (2048 reached) -->
      <div
        v-else-if="gameWon && !dismissedWin"
        class="absolute inset-0 z-20 bg-[#06151f]/92 backdrop-blur-md rounded-xl flex flex-col items-center justify-center p-4 text-center animate-fade-in border border-teal-400/40 shadow-[0_0_25px_rgba(71,210,201,0.35)]"
      >
        <span class="text-[10px] font-black uppercase tracking-widest text-teal-300 mb-1 drop-shadow-[0_0_8px_#47d2c9]">Victory!</span>
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
        <kbd class="px-1.5 py-0.5 rounded bg-slate-800 border border-slate-700 text-teal-300 font-sans font-bold text-[9px]">↑</kbd>
        <kbd class="px-1.5 py-0.5 rounded bg-slate-800 border border-slate-700 text-teal-300 font-sans font-bold text-[9px]">↓</kbd>
        <kbd class="px-1.5 py-0.5 rounded bg-slate-800 border border-slate-700 text-teal-300 font-sans font-bold text-[9px]">←</kbd>
        <kbd class="px-1.5 py-0.5 rounded bg-slate-800 border border-slate-700 text-teal-300 font-sans font-bold text-[9px]">→</kbd>
        <span class="ml-1 text-slate-500">or WASD</span>
      </span>
      <span class="text-slate-500 hover:text-slate-300 cursor-pointer transition-colors" @click="$emit('close')">
        ✕ Hide
      </span>
    </div>
  </div>
</template>

<script setup>
import { ref, onMounted, onUnmounted } from 'vue';

const props = defineProps({
  embedded: {
    type: Boolean,
    default: false,
  },
});

const emit = defineEmits(['close']);

const boardRef = ref(null);

let nextTileId = 1;
const activeTiles = ref([]);

const score = ref(0);
const bestScore = ref(0);
const gameOver = ref(false);
const gameWon = ref(false);
const dismissedWin = ref(false);

const STORAGE_STATE_KEY = 'zircon_2048_state';
const STORAGE_BEST_KEY = 'zircon_2048_best';

// --- Responsive Tile Position Style ---
function getTilePositionStyle(row, col) {
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
        activeTiles.value = parsed.tiles.map((t) => ({
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
  const emptyCells = [];

  for (let r = 0; r < 4; r++) {
    for (let c = 0; c < 4; c++) {
      if (!occupied.has(`${r}-${c}`)) {
        emptyCells.push({ row: r, col: c });
      }
    }
  }

  if (emptyCells.length === 0) return;
  const rand = emptyCells[Math.floor(Math.random() * emptyCells.length)];
  const newTile = {
    id: nextTileId++,
    value: Math.random() < 0.9 ? 2 : 4,
    row: rand.row,
    col: rand.col,
    isNew: true,
  };
  activeTiles.value.push(newTile);
}

// --- Sliding Movement with CSS Transition ---
function move(direction) {
  if (gameOver.value) return;

  // Clear previous transient animation flags
  for (const t of activeTiles.value) {
    t.isNew = false;
    t.isMerged = false;
  }

  // Build current 4x4 matrix of tile references
  const grid = Array.from({ length: 4 }, () => [null, null, null, null]);
  for (const tile of activeTiles.value) {
    grid[tile.row][tile.col] = tile;
  }

  let anyMoved = false;
  let scoreGained = 0;
  const tilesToRemove = new Set();
  const tilesToAdd = [];

  const isHorizontal = direction === 'left' || direction === 'right';
  const isForward = direction === 'right' || direction === 'down';

  for (let line = 0; line < 4; line++) {
    // Collect non-empty tiles along line in directional order
    const lineTiles = [];
    for (let i = 0; i < 4; i++) {
      const pos = isForward ? 3 - i : i;
      const tile = isHorizontal ? grid[line][pos] : grid[pos][line];
      if (tile) lineTiles.push(tile);
    }

    let targetIdx = 0;
    let i = 0;
    while (i < lineTiles.length) {
      const current = lineTiles[i];
      const targetPos = isForward ? 3 - targetIdx : targetIdx;

      if (i + 1 < lineTiles.length && lineTiles[i].value === lineTiles[i + 1].value) {
        const nextTile = lineTiles[i + 1];
        const mergedValue = current.value * 2;
        scoreGained += mergedValue;

        const targetRow = isHorizontal ? line : targetPos;
        const targetCol = isHorizontal ? targetPos : line;

        if (current.row !== targetRow || current.col !== targetCol) {
          current.row = targetRow;
          current.col = targetCol;
          anyMoved = true;
        }
        if (nextTile.row !== targetRow || nextTile.col !== targetCol) {
          nextTile.row = targetRow;
          nextTile.col = targetCol;
          anyMoved = true;
        }

        tilesToRemove.add(current.id);
        tilesToRemove.add(nextTile.id);

        tilesToAdd.push({
          id: nextTileId++,
          value: mergedValue,
          row: targetRow,
          col: targetCol,
          isMerged: true,
        });

        if (mergedValue === 2048) gameWon.value = true;
        i += 2;
      } else {
        const targetRow = isHorizontal ? line : targetPos;
        const targetCol = isHorizontal ? targetPos : line;

        if (current.row !== targetRow || current.col !== targetCol) {
          current.row = targetRow;
          current.col = targetCol;
          anyMoved = true;
        }
        i += 1;
      }
      targetIdx++;
    }
  }

  if (anyMoved) {
    activeTiles.value = activeTiles.value.filter((t) => !tilesToRemove.has(t.id));
    activeTiles.value.push(...tilesToAdd);

    score.value += scoreGained;
    if (score.value > bestScore.value) {
      bestScore.value = score.value;
    }

    spawnTile();
    saveGameState();

    checkGameOver();
  }
}

function checkGameOver() {
  if (activeTiles.value.length < 16) return;

  const grid = Array.from({ length: 4 }, () => [0, 0, 0, 0]);
  for (const t of activeTiles.value) {
    grid[t.row][t.col] = t.value;
  }

  for (let r = 0; r < 4; r++) {
    for (let c = 0; c < 4; c++) {
      const val = grid[r][c];
      if (val === 0) return;
      if (r < 3 && grid[r + 1][c] === val) return;
      if (c < 3 && grid[r][c + 1] === val) return;
    }
  }

  gameOver.value = true;
}

// --- Key & Touch Handlers ---
function handleKeyDown(e) {
  const tag = e.target?.tagName?.toLowerCase();
  if (tag === 'input' || tag === 'textarea' || e.target?.isContentEditable) {
    return;
  }

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

function handleTouchStart(e) {
  if (e.touches.length === 1) {
    touchStartX = e.touches[0].clientX;
    touchStartY = e.touches[0].clientY;
  }
}

function handleTouchEnd(e) {
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

// --- Tile Styling (Vibrant Cyberpunk Minecraft Gem Palette) ---
function getTileClass(val) {
  switch (val) {
    case 2:
      return 'bg-[#0e1f2d] text-teal-200 border border-teal-500/30 shadow-sm';
    case 4:
      return 'bg-[#102a3f] text-teal-100 border border-teal-500/45 shadow-sm';
    case 8:
      return 'bg-[#123854] text-cyan-50 border border-cyan-400/50 shadow-md shadow-cyan-950/50';
    case 16:
      return 'bg-[#14476a] text-white border border-cyan-400/70 shadow-md shadow-cyan-900/50';
    case 32:
      return 'bg-[#0f5c80] text-white border border-teal-400/80 shadow-lg shadow-teal-900/50';
    case 64:
      return 'bg-[#0d738f] text-white border border-emerald-400/80 shadow-lg shadow-emerald-900/60 font-extrabold';
    case 128:
      return 'bg-[#0a8c9e] text-white border border-emerald-300 shadow-xl shadow-emerald-700/60 font-extrabold';
    case 256:
      return 'bg-gradient-to-br from-[#0ab4aa] to-[#06847d] text-slate-950 border border-teal-200 shadow-xl shadow-teal-500/50 font-black';
    case 512:
      return 'bg-gradient-to-br from-[#20d7be] to-[#0bb19d] text-slate-950 border border-white shadow-2xl shadow-teal-400/70 font-black';
    case 1024:
      return 'bg-gradient-to-br from-[#38ebd5] to-[#12c4b0] text-slate-950 border border-white shadow-2xl shadow-cyan-400/80 font-black';
    case 2048:
      return 'bg-gradient-to-br from-[#68fff0] via-[#47d2c9] to-[#14b8a6] text-slate-950 border-2 border-white shadow-[0_0_25px_rgba(71,210,201,0.95)] animate-pulse font-black';
    default:
      return 'bg-gradient-to-br from-white via-teal-200 to-emerald-400 text-slate-950 border-2 border-white shadow-[0_0_30px_rgba(255,255,255,0.9)] font-black';
  }
}

function getTileTextSize(val) {
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

defineExpose({
  resetGame,
  focusBoard,
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
