// Pathfinding grid — plain canvas + requestAnimationFrame, same pattern as
// life.js. Foster owns algorithm-select/run/reset state; this loop reads
// data-fx-state (run/paused) and two fx-text markers (algorithm,
// reset_nonce) off the DOM, same contract as every other canvas widget on
// this page. Unlike the other sections, there's no "what real data source"
// question here — the real site's pathfinding is already fully
// self-contained (client-side grid, no server dependency), so this is a
// straightforward port.

const COLS = 40;
const ROWS = 24;
const STEP_MS = 20;

function makeGrid() {
  const walls = new Set();
  for (let i = 0; i < COLS * ROWS * 0.28; i++) {
    const x = Math.floor(Math.random() * COLS);
    const y = Math.floor(Math.random() * ROWS);
    walls.add(`${x},${y}`);
  }
  const start = [1, 1];
  const end = [COLS - 2, ROWS - 2];
  walls.delete(`${start[0]},${start[1]}`);
  walls.delete(`${end[0]},${end[1]}`);
  return { walls, start, end };
}

function neighbors([x, y]) {
  return [[x + 1, y], [x - 1, y], [x, y + 1], [x, y - 1]]
    .filter(([nx, ny]) => nx >= 0 && nx < COLS && ny >= 0 && ny < ROWS);
}

function heuristic([x, y], [ex, ey]) {
  return Math.abs(x - ex) + Math.abs(y - ey);
}

// Generator-based BFS/A* so the render loop can step it one node at a time.
function* search(grid, algorithm) {
  const key = ([x, y]) => `${x},${y}`;
  const cameFrom = new Map();
  const visited = new Set([key(grid.start)]);
  const frontier = [{ pos: grid.start, cost: 0 }];

  while (frontier.length) {
    if (algorithm === 'astar') {
      frontier.sort((a, b) => (a.cost + heuristic(a.pos, grid.end)) - (b.cost + heuristic(b.pos, grid.end)));
    }
    const { pos, cost } = frontier.shift();
    yield { frontierPositions: frontier.map((f) => f.pos), visited, current: pos };

    if (pos[0] === grid.end[0] && pos[1] === grid.end[1]) {
      const path = [pos];
      let cur = key(pos);
      while (cameFrom.has(cur)) {
        cur = cameFrom.get(cur);
        path.push(cur.split(',').map(Number));
      }
      yield { done: true, path, visited };
      return;
    }

    for (const n of neighbors(pos)) {
      const k = key(n);
      if (visited.has(k) || grid.walls.has(k)) continue;
      visited.add(k);
      cameFrom.set(k, key(pos));
      frontier.push({ pos: n, cost: cost + 1 });
    }
  }
  yield { done: true, path: null, visited };
}

export function initPathfinding() {
  const canvas = document.getElementById('path-canvas');
  const root = document.querySelector('[fx-machine="pathfinding"]');
  const algoEl = document.getElementById('pathfinding-algorithm');
  const nonceEl = document.getElementById('pathfinding-reset-nonce');
  if (!canvas || !root || !algoEl || !nonceEl) return;

  const ctx2d = canvas.getContext('2d');
  const cw = canvas.width / COLS;
  const ch = canvas.height / ROWS;

  let grid = makeGrid();
  let algorithm = algoEl.textContent.trim() || 'bfs';
  let gen = search(grid, algorithm);
  let lastFrame = null;
  let lastStep = 0;
  let lastNonce = nonceEl.textContent;
  let lastAlgo = algorithm;

  function restart() {
    grid = makeGrid();
    algorithm = algoEl.textContent.trim() || 'bfs';
    gen = search(grid, algorithm);
    lastFrame = null;
  }

  function draw() {
    const surface = getComputedStyle(document.body).getPropertyValue('--surface') || '#0f1420';
    const accent = getComputedStyle(document.body).getPropertyValue('--accent') || '#60a5fa';
    const border = getComputedStyle(document.body).getPropertyValue('--border') || '#333';

    ctx2d.fillStyle = surface;
    ctx2d.fillRect(0, 0, canvas.width, canvas.height);

    for (const wallKey of grid.walls) {
      const [x, y] = wallKey.split(',').map(Number);
      ctx2d.fillStyle = border;
      ctx2d.fillRect(x * cw, y * ch, cw - 1, ch - 1);
    }

    if (lastFrame) {
      ctx2d.fillStyle = 'rgba(96,165,250,0.25)';
      for (const [x, y] of lastFrame.visited ? [...lastFrame.visited].map((k) => k.split(',').map(Number)) : []) {
        ctx2d.fillRect(x * cw, y * ch, cw - 1, ch - 1);
      }
      if (lastFrame.frontierPositions) {
        ctx2d.fillStyle = 'rgba(245,158,11,0.6)';
        for (const [x, y] of lastFrame.frontierPositions) {
          ctx2d.fillRect(x * cw, y * ch, cw - 1, ch - 1);
        }
      }
      if (lastFrame.path) {
        ctx2d.fillStyle = '#10b981';
        for (const [x, y] of lastFrame.path) {
          ctx2d.fillRect(x * cw, y * ch, cw - 1, ch - 1);
        }
      }
    }

    ctx2d.fillStyle = accent;
    ctx2d.fillRect(grid.start[0] * cw, grid.start[1] * ch, cw - 1, ch - 1);
    ctx2d.fillStyle = '#ef4444';
    ctx2d.fillRect(grid.end[0] * cw, grid.end[1] * ch, cw - 1, ch - 1);
  }

  function frame(ts) {
    if (nonceEl.textContent !== lastNonce || algoEl.textContent.trim() !== lastAlgo) {
      lastNonce = nonceEl.textContent;
      lastAlgo = algoEl.textContent.trim();
      restart();
    }

    if (root.getAttribute('data-fx-state') === 'running' && ts - lastStep >= STEP_MS) {
      const next = gen.next();
      if (!next.done) lastFrame = next.value;
      lastStep = ts;
    }

    draw();
    requestAnimationFrame(frame);
  }

  draw();
  requestAnimationFrame(frame);
}
