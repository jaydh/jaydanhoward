// Conway's Game of Life — plain canvas + requestAnimationFrame, deliberately
// independent of Foster. It only touches the "life" machine's DOM through the
// public contract every fx-machine root exposes: `data-fx-state`, updated
// by foster-client on every snapshot (initial fetch, transition response, or
// SSE push). No Rust/WASM binding into foster-client, no server round trip
// per frame — the whole point of this file is to prove that's enough.

const GRID_W = 96;
const GRID_H = 54;
const STEP_MS = 90;

function makeGrid(density) {
  const cells = new Uint8Array(GRID_W * GRID_H);
  for (let i = 0; i < cells.length; i++) {
    cells[i] = Math.random() < density ? 1 : 0;
  }
  return cells;
}

function step(cells) {
  const next = new Uint8Array(cells.length);
  for (let y = 0; y < GRID_H; y++) {
    for (let x = 0; x < GRID_W; x++) {
      let n = 0;
      for (let dy = -1; dy <= 1; dy++) {
        for (let dx = -1; dx <= 1; dx++) {
          if (dx === 0 && dy === 0) continue;
          const nx = (x + dx + GRID_W) % GRID_W;
          const ny = (y + dy + GRID_H) % GRID_H;
          n += cells[ny * GRID_W + nx];
        }
      }
      const alive = cells[y * GRID_W + x] === 1;
      next[y * GRID_W + x] = alive ? (n === 2 || n === 3 ? 1 : 0) : (n === 3 ? 1 : 0);
    }
  }
  return next;
}

export function initLife() {
  const canvas = document.getElementById('life-canvas');
  const root = document.querySelector('[fx-machine="life"]');
  const nonceEl = document.getElementById('life-reset-nonce');
  if (!canvas || !root || !nonceEl) return;

  const ctx2d = canvas.getContext('2d');
  const cw = canvas.width / GRID_W;
  const ch = canvas.height / GRID_H;

  let cells = makeGrid(0.35);
  let lastNonce = nonceEl.textContent;
  let lastStep = 0;

  function draw() {
    ctx2d.fillStyle = getComputedStyle(document.body).getPropertyValue('--surface') || '#0f1420';
    ctx2d.fillRect(0, 0, canvas.width, canvas.height);
    ctx2d.fillStyle = getComputedStyle(document.body).getPropertyValue('--accent') || '#60a5fa';
    for (let y = 0; y < GRID_H; y++) {
      for (let x = 0; x < GRID_W; x++) {
        if (cells[y * GRID_W + x]) {
          ctx2d.fillRect(x * cw, y * ch, cw - 1, ch - 1);
        }
      }
    }
  }

  function frame(ts) {
    // The only thing read from Foster: whether we're "running", straight off
    // the DOM attribute foster-client keeps in sync with the server.
    const state = root.getAttribute('data-fx-state');

    if (nonceEl.textContent !== lastNonce) {
      lastNonce = nonceEl.textContent;
      cells = makeGrid(0.35);
    }

    if (state === 'running' && ts - lastStep >= STEP_MS) {
      cells = step(cells);
      lastStep = ts;
    }

    draw();
    requestAnimationFrame(frame);
  }

  draw();
  requestAnimationFrame(frame);
}
