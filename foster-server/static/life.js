// Conway's Game of Life — plain canvas + requestAnimationFrame, entirely
// client-side. Run/pause/reset used to be a Foster machine, but that state
// is inherently per-visitor (auto-play-on-scroll-into-view, matching the
// real src/components/life.rs's own IntersectionObserver-driven local
// signal) — Foster machines are one shared instance across every
// connected client, so that had made one visitor's scroll position
// control play/pause for everyone. No server round trip per frame either
// way; this just also drops the unnecessary shared-state layer.

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
  const widget = document.getElementById('life-widget');
  const toggleBtn = document.getElementById('life-toggle-run');
  const resetBtn = document.getElementById('life-reset');
  if (!canvas || !widget || !toggleBtn || !resetBtn) return;

  const runLabel = toggleBtn.querySelector('.life-run-label');
  const pauseLabel = toggleBtn.querySelector('.life-pause-label');
  const ctx2d = canvas.getContext('2d');
  const cw = canvas.width / GRID_W;
  const ch = canvas.height / GRID_H;

  let cells = makeGrid(0.35);
  let running = false;
  let lastStep = 0;

  function syncButton() {
    runLabel.style.display = running ? 'none' : '';
    pauseLabel.style.display = running ? '' : 'none';
  }

  toggleBtn.addEventListener('click', () => {
    running = !running;
    syncButton();
  });

  resetBtn.addEventListener('click', () => {
    cells = makeGrid(0.35);
  });

  // Auto-play when scrolled into view, pause when scrolled away — same
  // IntersectionObserver behavior as the real site.
  const observer = new IntersectionObserver((entries) => {
    for (const entry of entries) {
      if (entry.isIntersecting) {
        running = true;
      } else {
        running = false;
      }
      syncButton();
    }
  }, { threshold: 0.1 });
  observer.observe(widget);

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
    if (running && ts - lastStep >= STEP_MS) {
      cells = step(cells);
      lastStep = ts;
    }
    draw();
    requestAnimationFrame(frame);
  }

  syncButton();
  draw();
  requestAnimationFrame(frame);
}
