// Pathfinding — 7 algorithms racing in parallel over one shared random
// 2048×2048 obstacle grid, each with its own WebGL2 canvas (R8 state
// texture). Ported line-for-line from the real src/components/
// path_search.rs (shaders, AlgoRun step logic, PathRenderer, drag-to-pan +
// scroll-to-zoom from canvas_nav.rs). Entirely client-side — the real
// site's pathfinding never touched the server either (its whole component
// is #[cfg(not(feature = "ssr"))]) — so Foster only owns run/pause and a
// reset nonce; everything else here (7 runners, zoom, pan, follow,
// completion order) is local JS state, same as the real site's local
// Leptos signals.

const GRID_SIZE = 2048;
const OBSTACLE_PROB = 0.2;
const STEPS_PER_FRAME = 5;

const OBSTACLE = 0, UNVISITED = 1, FRONTIER = 2, VISITED = 3, PATH = 4;

const BLIND_ALGOS = ['bfs', 'dfs', 'corner', 'wall', 'randomwalk'];
const INFORMED_ALGOS = ['astar', 'greedy'];
const ALL_ALGOS = [...BLIND_ALGOS, ...INFORMED_ALGOS];

const VERT = `#version 300 es
in vec2 a_pos;
out vec2 v_uv;
void main() {
    v_uv = a_pos * 0.5 + 0.5;
    gl_Position = vec4(a_pos, 0.0, 1.0);
}`;

const DRAW_FRAG = `#version 300 es
precision mediump float;
in vec2 v_uv;
out vec4 o;
uniform sampler2D u_state;
uniform vec3 u_visited;
uniform vec3 u_bg;
uniform vec3 u_wall;
uniform vec2 u_start;
uniform vec2 u_end;
uniform vec2 u_res;
uniform float u_zoom;
uniform vec2 u_zoom_center;
void main() {
    vec2 uv = (v_uv - u_zoom_center) / u_zoom + u_zoom_center;
    if (uv.x < 0.0 || uv.x > 1.0 || uv.y < 0.0 || uv.y > 1.0) {
        o = vec4(u_bg, 1.0);
        return;
    }
    float s = floor(texture(u_state, uv).r * 255.0 + 0.5);
    vec3 col;
    if      (s < 0.5) { col = u_wall; }
    else if (s < 1.5) { col = u_bg; }
    else if (s < 2.5) { col = vec3(0.937, 0.267, 0.267); }
    else if (s < 3.5) { col = u_visited; }
    else              { col = vec3(0.753, 0.518, 0.988); }
    vec2 ps = (uv - u_start) * u_res;
    if (dot(ps, ps) < 9.0) { col = vec3(0.133, 0.773, 0.369); }
    vec2 pe = (uv - u_end) * u_res;
    if (dot(pe, pe) < 9.0) { col = vec3(0.961, 0.620, 0.043); }
    o = vec4(col, 1.0);
}`;

function compileShader(gl, type, source) {
  const s = gl.createShader(type);
  gl.shaderSource(s, source);
  gl.compileShader(s);
  if (!gl.getShaderParameter(s, gl.COMPILE_STATUS)) {
    throw new Error(gl.getShaderInfoLog(s));
  }
  return s;
}

function linkProgram(gl, vert, frag) {
  const vs = compileShader(gl, gl.VERTEX_SHADER, vert);
  const fs = compileShader(gl, gl.FRAGMENT_SHADER, frag);
  const p = gl.createProgram();
  gl.attachShader(p, vs);
  gl.attachShader(p, fs);
  gl.linkProgram(p);
  if (!gl.getProgramParameter(p, gl.LINK_STATUS)) {
    throw new Error(gl.getProgramInfoLog(p));
  }
  return p;
}

class PathRenderer {
  constructor(canvas, gridW, gridH) {
    const gl = canvas.getContext('webgl2');
    if (!gl) throw new Error('no webgl2');
    this.gl = gl;
    this.gridW = gridW;
    this.gridH = gridH;
    this.prog = linkProgram(gl, VERT, DRAW_FRAG);

    this.vao = gl.createVertexArray();
    gl.bindVertexArray(this.vao);
    const buf = gl.createBuffer();
    gl.bindBuffer(gl.ARRAY_BUFFER, buf);
    const verts = new Float32Array([-1, -1, 1, -1, -1, 1, -1, 1, 1, -1, 1, 1]);
    gl.bufferData(gl.ARRAY_BUFFER, verts, gl.STATIC_DRAW);
    const loc = gl.getAttribLocation(this.prog, 'a_pos');
    gl.enableVertexAttribArray(loc);
    gl.vertexAttribPointer(loc, 2, gl.FLOAT, false, 0, 0);
    gl.bindVertexArray(null);

    this.tex = gl.createTexture();
    gl.bindTexture(gl.TEXTURE_2D, this.tex);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MIN_FILTER, gl.NEAREST);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MAG_FILTER, gl.NEAREST);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_S, gl.CLAMP_TO_EDGE);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_T, gl.CLAMP_TO_EDGE);
    gl.texImage2D(gl.TEXTURE_2D, 0, gl.R8, gridW, gridH, 0, gl.RED, gl.UNSIGNED_BYTE, null);
    gl.bindTexture(gl.TEXTURE_2D, null);
  }

  upload(state) {
    const gl = this.gl;
    gl.bindTexture(gl.TEXTURE_2D, this.tex);
    gl.texImage2D(gl.TEXTURE_2D, 0, gl.R8, this.gridW, this.gridH, 0, gl.RED, gl.UNSIGNED_BYTE, state);
    gl.bindTexture(gl.TEXTURE_2D, null);
  }

  draw(cw, ch, dark, start, end, zoom, zcx, zcy) {
    const gl = this.gl;
    gl.viewport(0, 0, cw, ch);
    gl.useProgram(this.prog);
    gl.activeTexture(gl.TEXTURE0);
    gl.bindTexture(gl.TEXTURE_2D, this.tex);
    const u = (name) => gl.getUniformLocation(this.prog, name);
    gl.uniform1i(u('u_state'), 0);
    const [vis, bg, wall] = dark
      ? [[0.376, 0.647, 0.980], [0.067, 0.094, 0.153], [0.310, 0.400, 0.502]]
      : [[0.231, 0.510, 0.965], [0.973, 0.980, 0.988], [0.180, 0.224, 0.286]];
    gl.uniform3f(u('u_visited'), ...vis);
    gl.uniform3f(u('u_bg'), ...bg);
    gl.uniform3f(u('u_wall'), ...wall);
    gl.uniform2f(u('u_start'), start[0] / this.gridW, start[1] / this.gridH);
    gl.uniform2f(u('u_end'), end[0] / this.gridW, end[1] / this.gridH);
    gl.uniform2f(u('u_res'), this.gridW, this.gridH);
    gl.uniform1f(u('u_zoom'), Math.max(zoom, 0.01));
    gl.uniform2f(u('u_zoom_center'), zcx, zcy);
    gl.bindVertexArray(this.vao);
    gl.drawArrays(gl.TRIANGLES, 0, 6);
    gl.bindVertexArray(null);
  }
}

function makeGrid(size, prob) {
  const n = size * size;
  const base = new Uint8Array(n).fill(UNVISITED);
  const passable = [];
  for (let i = 0; i < n; i++) {
    if (Math.random() < prob) {
      base[i] = OBSTACLE;
    } else {
      passable.push(i);
    }
  }
  if (passable.length < 2) return makeGrid(size, prob);
  const si = Math.floor(Math.random() * passable.length);
  let ei = Math.floor(Math.random() * passable.length);
  while (ei === si) ei = Math.floor(Math.random() * passable.length);
  const startIdx = passable[si];
  const endIdx = passable[ei];
  const start = [startIdx % size, Math.floor(startIdx / size)];
  const end = [endIdx % size, Math.floor(endIdx / size)];
  return { base, w: size, h: size, start, end };
}

class AlgoRun {
  constructor(base, w, h, start, end) {
    this.state = base.slice();
    this.parent = new Uint32Array(w * h).fill(0xffffffff);
    this.queue = [];
    this.head = 0;
    this.w = w;
    this.h = h;
    this.start = start;
    this.end = end;
    this.initialized = false;
    this.done = false;
    this.steps = 0;
    this.completionSteps = null;
    this.currentPos = start;
  }

  idx(x, y) { return y * this.w + x; }
  startIdx() { return this.idx(this.start[0], this.start[1]); }
  endIdx() { return this.idx(this.end[0], this.end[1]); }

  neighbors(i) {
    const x = i % this.w, y = Math.floor(i / this.w);
    const out = [];
    if (x > 0) out.push(i - 1);
    if (x + 1 < this.w) out.push(i + 1);
    if (y > 0) out.push(i - this.w);
    if (y + 1 < this.h) out.push(i + this.w);
    return out;
  }

  manhattan(a, b) {
    const ax = a % this.w, ay = Math.floor(a / this.w);
    const bx = b % this.w, by = Math.floor(b / this.w);
    return Math.abs(ax - bx) + Math.abs(ay - by);
  }

  wallDist(i) {
    const x = i % this.w, y = Math.floor(i / this.w);
    return Math.min(x, this.w - 1 - x, y, this.h - 1 - y);
  }

  cornerDist(i) {
    const corners = [0, this.w - 1, this.w * (this.h - 1), this.w * this.h - 1];
    return Math.min(...corners.map((c) => this.manhattan(i, c)));
  }

  step(algo) {
    if (this.done) return;
    this.steps++;

    const si = this.startIdx();
    const ei = this.endIdx();

    if (!this.initialized) {
      this.initialized = true;
      this.state[si] = FRONTIER;
      this.queue.push(si);
      return;
    }

    let current;
    for (;;) {
      let c;
      if (algo === 'bfs') {
        if (this.head >= this.queue.length) { this.done = true; return; }
        c = this.queue[this.head++];
      } else {
        if (this.queue.length === 0) { this.done = true; return; }
        c = this.queue.pop();
      }
      const s = this.state[c];
      if (s !== VISITED && s !== PATH) { current = c; break; }
    }

    this.currentPos = [current % this.w, Math.floor(current / this.w)];

    if (current === ei) {
      this.state[current] = VISITED;
      this.completionSteps = this.steps;
      this.done = true;
      this.reconstructPath(si, ei);
      return;
    }

    this.state[current] = VISITED;

    const viable = this.neighbors(current).filter((n) => this.state[n] === UNVISITED);
    for (const n of viable) {
      this.state[n] = FRONTIER;
      this.parent[n] = current;
    }

    switch (algo) {
      case 'bfs':
      case 'dfs':
        for (const n of viable) this.queue.push(n);
        break;
      case 'astar':
      case 'greedy': {
        for (const n of viable) this.queue.push(n);
        // Re-sort the whole remaining queue so the globally closest-to-end
        // cell is always at the back (popped next) — matches the real
        // site's re-sort-on-every-step approach exactly.
        const h = this.head;
        const tail = this.queue.slice(h);
        tail.sort((a, b) => this.manhattan(b, ei) - this.manhattan(a, ei));
        for (let i = 0; i < tail.length; i++) this.queue[h + i] = tail[i];
        break;
      }
      case 'corner': {
        viable.sort((a, b) => this.cornerDist(b) - this.cornerDist(a));
        for (const n of viable) this.queue.push(n);
        break;
      }
      case 'wall': {
        viable.sort((a, b) => this.wallDist(b) - this.wallDist(a));
        for (const n of viable) this.queue.push(n);
        break;
      }
      case 'randomwalk': {
        viable.sort(() => Math.random() - 0.5);
        for (const n of viable) this.queue.push(n);
        break;
      }
    }
  }

  reconstructPath(si, ei) {
    let curr = ei;
    const maxSteps = (this.w + this.h) * 4;
    for (let i = 0; i < maxSteps; i++) {
      this.state[curr] = PATH;
      if (curr === si) break;
      const p = this.parent[curr];
      if (p === 0xffffffff) break;
      curr = p;
    }
  }
}

function attachCanvasNav(canvas, getZoom, setZoom, getZoomCenter, setZoomCenter) {
  let dragging = false;
  let lastX = 0, lastY = 0, dragW = 1, dragH = 1;

  canvas.addEventListener('pointerdown', (e) => {
    const rect = canvas.getBoundingClientRect();
    dragW = rect.width; dragH = rect.height;
    dragging = true;
    lastX = e.clientX; lastY = e.clientY;
    canvas.setPointerCapture(e.pointerId);
    e.preventDefault();
  });

  canvas.addEventListener('pointermove', (e) => {
    if (!dragging) return;
    const dx = (e.clientX - lastX) / dragW;
    const dy = (e.clientY - lastY) / dragH;
    lastX = e.clientX; lastY = e.clientY;
    const z = getZoom();
    const [ocx, ocy] = getZoomCenter();
    setZoomCenter([ocx - dx / z, ocy + dy / z]);
  });

  canvas.addEventListener('pointerup', () => { dragging = false; });

  canvas.addEventListener('wheel', (e) => {
    e.preventDefault();
    const rect = canvas.getBoundingClientRect();
    const sx = (e.clientX - rect.left) / rect.width;
    const sy = (e.clientY - rect.top) / rect.height;
    const oldZ = getZoom();
    const factor = e.deltaY > 0 ? 1 / 1.15 : 1.15;
    const newZ = Math.min(16, Math.max(1, oldZ * factor));
    const [cx, cy] = getZoomCenter();
    const wx = (sx - cx) / oldZ + cx;
    const wy = (sy - cy) / oldZ + cy;
    const newCx = Math.abs(newZ - 1.0) > 1e-4 ? (newZ * wx - sx) / (newZ - 1.0) : 0.5;
    const newCy = Math.abs(newZ - 1.0) > 1e-4 ? (newZ * wy - sy) / (newZ - 1.0) : 0.5;
    setZoom(newZ);
    setZoomCenter([newCx, newCy]);
  }, { passive: false });
}

function rankLabel(pos) {
  return ['🥇 1st', '🥈 2nd', '🥉 3rd', '4th', '5th', '6th', '7th'][pos] || `${pos + 1}th`;
}

export function initPathfinding() {
  const root = document.getElementById('pathfinding-widget');
  if (!root) return;

  let zoom = 1.0;
  let following = false;
  let blindOrder = [];
  let informedOrder = [];
  let running = false;

  const toggleBtn = document.getElementById('pf-toggle-run');
  const playLabel = toggleBtn.querySelector('.pf-play-label');
  const pauseLabel = toggleBtn.querySelector('.pf-pause-label');

  function syncToggleButton() {
    playLabel.style.display = running ? 'none' : '';
    pauseLabel.style.display = running ? '' : 'none';
  }

  toggleBtn.addEventListener('click', () => {
    running = !running;
    syncToggleButton();
  });

  // Auto-play when scrolled into view, pause when scrolled away — same
  // per-visitor behavior as the real site's IntersectionObserver, and the
  // same reasoning as life.js for why this isn't a Foster machine.
  new IntersectionObserver((entries) => {
    for (const entry of entries) {
      running = entry.isIntersecting;
      syncToggleButton();
    }
  }, { threshold: 0.1 }).observe(root);

  const panels = ALL_ALGOS.map((algo) => {
    const el = root.querySelector(`.pf-panel[data-algo="${algo}"]`);
    const canvas = el.querySelector('canvas');
    const meta = el.querySelector('.pf-meta');
    return { algo, el, canvas, meta, renderer: null, run: null, zoomCenter: [0.5, 0.5], frameCount: 0, fps: 0 };
  });

  function groupOrder(algo) {
    return BLIND_ALGOS.includes(algo) ? blindOrder : informedOrder;
  }

  function regenerate() {
    const grid = makeGrid(GRID_SIZE, OBSTACLE_PROB);
    blindOrder = [];
    informedOrder = [];
    zoom = 1.0;
    document.getElementById('pf-zoom-slider').value = '1';
    document.getElementById('pf-zoom-label').textContent = '1.0x';
    for (const p of panels) {
      p.run = new AlgoRun(grid.base, grid.w, grid.h, grid.start, grid.end);
      p.frameCount = 0;
      p.fps = 0;
      const cx = (grid.start[0] / grid.w + grid.end[0] / grid.w) * 0.5;
      const cy = (grid.start[1] / grid.h + grid.end[1] / grid.h) * 0.5;
      p.zoomCenter = [cx, cy];
      if (!p.renderer) {
        p.canvas.width = p.canvas.clientWidth || 280;
        p.canvas.height = p.canvas.clientHeight || 280;
        p.renderer = new PathRenderer(p.canvas, grid.w, grid.h);
        attachCanvasNav(
          p.canvas,
          () => zoom,
          (z) => {
            zoom = z;
            document.getElementById('pf-zoom-slider').value = String(z);
            document.getElementById('pf-zoom-label').textContent = `${z.toFixed(1)}x`;
          },
          () => p.zoomCenter,
          (c) => { p.zoomCenter = c; },
        );
      }
    }
  }

  regenerate();

  document.getElementById('pf-reset').addEventListener('click', regenerate);

  document.getElementById('pf-zoom-slider').addEventListener('input', (e) => {
    zoom = parseFloat(e.target.value);
    document.getElementById('pf-zoom-label').textContent = `${zoom.toFixed(1)}x`;
  });

  const followBtn = document.getElementById('pf-follow');
  followBtn.addEventListener('click', () => {
    following = !following;
    followBtn.classList.toggle('active', following);
    followBtn.textContent = following ? 'Following' : 'Follow';
    if (following) {
      zoom = 4.0;
      document.getElementById('pf-zoom-slider').value = '4';
      document.getElementById('pf-zoom-label').textContent = '4.0x';
    }
  });

  let lastFpsTick = performance.now();

  function frame() {
    const dark = document.documentElement.classList.contains('dark');
    const now = performance.now();
    const fpsWindow = now - lastFpsTick >= 1000;

    for (const p of panels) {
      if (!p.run) continue;

      if (running && !p.run.done) {
        for (let i = 0; i < STEPS_PER_FRAME; i++) {
          if (!p.run.done) p.run.step(p.algo);
        }
        p.frameCount += STEPS_PER_FRAME;

        if (p.run.done && p.run.completionSteps !== null) {
          const order = groupOrder(p.algo);
          if (!order.includes(p.algo)) order.push(p.algo);
        }

        if (following && !p.run.done) {
          p.zoomCenter = [p.run.currentPos[0] / p.run.w, p.run.currentPos[1] / p.run.h];
        }
      }

      const cw = p.canvas.clientWidth || 280;
      const ch = p.canvas.clientHeight || 280;
      if (p.canvas.width !== cw || p.canvas.height !== ch) {
        p.canvas.width = cw;
        p.canvas.height = ch;
      }
      p.renderer.upload(p.run.state);
      p.renderer.draw(cw, ch, dark, p.run.start, p.run.end, zoom, p.zoomCenter[0], p.zoomCenter[1]);

      if (fpsWindow) {
        p.fps = p.frameCount;
        p.frameCount = 0;
      }

      const order = groupOrder(p.algo);
      const pos = order.indexOf(p.algo);
      const rank = pos >= 0 ? `<span class="pf-rank">${rankLabel(pos)}</span>` : '';
      const steps = p.run.completionSteps !== null
        ? `${p.run.completionSteps} steps`
        : (running && p.fps > 0 ? `${p.fps} steps/s` : '');
      p.meta.innerHTML = `${rank}${rank && steps ? ' &middot; ' : ''}${steps}`;
    }

    if (fpsWindow) lastFpsTick = now;
    requestAnimationFrame(frame);
  }

  requestAnimationFrame(frame);
}
