// Conway's Game of Life — real GPU ping-pong shader simulation, ported
// line-for-line from the real src/components/life.rs: a 2048×2048 R8
// texture holds cell state, a fragment shader computes the next
// generation (read current state, apply Conway's rules, write to the
// other texture via a framebuffer), and a separate draw shader maps
// alive/dead to colors with zoom/pan. Entirely client-side — run/pause/
// reset/zoom/settings are all per-visitor UI state, not a Foster machine,
// same reasoning as pathfinding.js/theme.js.

const GRID_SIZE = 2048;

const VERT = `#version 300 es
in vec2 a_pos;
out vec2 v_uv;
void main() {
    v_uv = a_pos * 0.5 + 0.5;
    gl_Position = vec4(a_pos, 0.0, 1.0);
}`;

const STEP_FRAG = `#version 300 es
precision highp float;
in vec2 v_uv;
out vec4 o;
uniform sampler2D u_state;
uniform vec2 u_res;
void main() {
    vec2 d = 1.0 / u_res;
    float c = texture(u_state, v_uv).r;
    float n =
        texture(u_state, v_uv + vec2(-d.x,-d.y)).r +
        texture(u_state, v_uv + vec2( 0.0,-d.y)).r +
        texture(u_state, v_uv + vec2( d.x,-d.y)).r +
        texture(u_state, v_uv + vec2(-d.x, 0.0)).r +
        texture(u_state, v_uv + vec2( d.x, 0.0)).r +
        texture(u_state, v_uv + vec2(-d.x, d.y)).r +
        texture(u_state, v_uv + vec2( 0.0, d.y)).r +
        texture(u_state, v_uv + vec2( d.x, d.y)).r;
    float nb = floor(n + 0.5);
    float next = (c > 0.5)
        ? ((nb == 2.0 || nb == 3.0) ? 1.0 : 0.0)
        : ((nb == 3.0) ? 1.0 : 0.0);
    o = vec4(next, 0.0, 0.0, 1.0);
}`;

const DRAW_FRAG = `#version 300 es
precision mediump float;
in vec2 v_uv;
out vec4 o;
uniform sampler2D u_state;
uniform vec3 u_alive;
uniform vec3 u_dead;
uniform float u_zoom;
uniform vec2 u_zoom_center;
void main() {
    vec2 uv = (v_uv - u_zoom_center) / u_zoom + u_zoom_center;
    if (uv.x < 0.0 || uv.x > 1.0 || uv.y < 0.0 || uv.y > 1.0) {
        o = vec4(u_dead, 1.0);
        return;
    }
    float c = texture(u_state, uv).r;
    o = vec4(mix(u_dead, u_alive, c), 1.0);
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

class LifeGl {
  constructor(canvas, gridW, gridH) {
    const gl = canvas.getContext('webgl2');
    if (!gl) throw new Error('no webgl2');
    this.gl = gl;
    this.gridW = gridW;
    this.gridH = gridH;
    this.stepProg = linkProgram(gl, VERT, STEP_FRAG);
    this.drawProg = linkProgram(gl, VERT, DRAW_FRAG);

    this.quadVao = gl.createVertexArray();
    gl.bindVertexArray(this.quadVao);
    const buf = gl.createBuffer();
    gl.bindBuffer(gl.ARRAY_BUFFER, buf);
    const verts = new Float32Array([-1, -1, 1, -1, -1, 1, -1, 1, 1, -1, 1, 1]);
    gl.bufferData(gl.ARRAY_BUFFER, verts, gl.STATIC_DRAW);
    const loc = gl.getAttribLocation(this.stepProg, 'a_pos');
    gl.enableVertexAttribArray(loc);
    gl.vertexAttribPointer(loc, 2, gl.FLOAT, false, 0, 0);
    gl.bindVertexArray(null);

    this.textures = [this.makeTexture(gridW, gridH), this.makeTexture(gridW, gridH)];
    this.fbs = [this.makeFramebuffer(this.textures[0]), this.makeFramebuffer(this.textures[1])];
    this.current = 0;
  }

  makeTexture(w, h) {
    const gl = this.gl;
    const tex = gl.createTexture();
    gl.bindTexture(gl.TEXTURE_2D, tex);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MIN_FILTER, gl.NEAREST);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MAG_FILTER, gl.NEAREST);
    // REPEAT so cells at edges wrap around to the opposite side
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_S, gl.REPEAT);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_T, gl.REPEAT);
    gl.texImage2D(gl.TEXTURE_2D, 0, gl.R8, w, h, 0, gl.RED, gl.UNSIGNED_BYTE, null);
    gl.bindTexture(gl.TEXTURE_2D, null);
    return tex;
  }

  makeFramebuffer(tex) {
    const gl = this.gl;
    const fb = gl.createFramebuffer();
    gl.bindFramebuffer(gl.FRAMEBUFFER, fb);
    gl.framebufferTexture2D(gl.FRAMEBUFFER, gl.COLOR_ATTACHMENT0, gl.TEXTURE_2D, tex, 0);
    gl.bindFramebuffer(gl.FRAMEBUFFER, null);
    return fb;
  }

  randomize(probability) {
    const gl = this.gl;
    const n = this.gridW * this.gridH;
    const data = new Uint8Array(n);
    for (let i = 0; i < n; i++) data[i] = Math.random() < probability ? 255 : 0;
    gl.bindTexture(gl.TEXTURE_2D, this.textures[this.current]);
    gl.texImage2D(gl.TEXTURE_2D, 0, gl.R8, this.gridW, this.gridH, 0, gl.RED, gl.UNSIGNED_BYTE, data);
    // Clear the back buffer to all dead so the first step doesn't read garbage.
    gl.bindTexture(gl.TEXTURE_2D, this.textures[1 - this.current]);
    gl.texImage2D(gl.TEXTURE_2D, 0, gl.R8, this.gridW, this.gridH, 0, gl.RED, gl.UNSIGNED_BYTE, new Uint8Array(n));
    gl.bindTexture(gl.TEXTURE_2D, null);
  }

  step() {
    const gl = this.gl;
    const next = 1 - this.current;
    gl.bindFramebuffer(gl.FRAMEBUFFER, this.fbs[next]);
    gl.viewport(0, 0, this.gridW, this.gridH);
    gl.useProgram(this.stepProg);
    gl.activeTexture(gl.TEXTURE0);
    gl.bindTexture(gl.TEXTURE_2D, this.textures[this.current]);
    gl.uniform1i(gl.getUniformLocation(this.stepProg, 'u_state'), 0);
    gl.uniform2f(gl.getUniformLocation(this.stepProg, 'u_res'), this.gridW, this.gridH);
    gl.bindVertexArray(this.quadVao);
    gl.drawArrays(gl.TRIANGLES, 0, 6);
    gl.bindVertexArray(null);
    gl.bindFramebuffer(gl.FRAMEBUFFER, null);
    this.current = next;
  }

  draw(canvasW, canvasH, dark, zoom, zcx, zcy) {
    const gl = this.gl;
    gl.viewport(0, 0, canvasW, canvasH);
    gl.useProgram(this.drawProg);
    gl.activeTexture(gl.TEXTURE0);
    gl.bindTexture(gl.TEXTURE_2D, this.textures[this.current]);
    gl.uniform1i(gl.getUniformLocation(this.drawProg, 'u_state'), 0);
    const [alive, dead] = dark
      ? [[0.376, 0.647, 0.980], [0.067, 0.094, 0.153]]
      : [[0.231, 0.510, 0.965], [1.0, 1.0, 1.0]];
    gl.uniform3f(gl.getUniformLocation(this.drawProg, 'u_alive'), ...alive);
    gl.uniform3f(gl.getUniformLocation(this.drawProg, 'u_dead'), ...dead);
    gl.uniform1f(gl.getUniformLocation(this.drawProg, 'u_zoom'), Math.max(zoom, 0.01));
    gl.uniform2f(gl.getUniformLocation(this.drawProg, 'u_zoom_center'), zcx, zcy);
    gl.bindVertexArray(this.quadVao);
    gl.drawArrays(gl.TRIANGLES, 0, 6);
    gl.bindVertexArray(null);
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

  // Shift-click re-centers the zoom on the clicked point.
  canvas.addEventListener('click', (e) => {
    if (!e.shiftKey) return;
    const rect = canvas.getBoundingClientRect();
    const cx = (e.clientX - rect.left) / rect.width;
    const cy = (e.clientY - rect.top) / rect.height;
    const [ocx, ocy] = getZoomCenter();
    const z = getZoom();
    const wx = (cx - ocx) / z + ocx;
    const wy = (cy - ocy) / z + ocy;
    setZoomCenter([wx, wy]);
  });
}

export function initLife() {
  const widget = document.getElementById('life-widget');
  const canvas = document.getElementById('life-canvas');
  if (!widget || !canvas) return;

  let renderer = null;
  let running = false;
  let zoom = 1.0;
  let zoomCenter = [0.5, 0.5];
  let aliveProbability = 0.35;
  let intervalMs = 16;
  let lastStep = 0;
  let hasStarted = false;

  const toggleBtn = document.getElementById('life-toggle-run');
  const runLabel = toggleBtn.querySelector('.life-run-label');
  const pauseLabel = toggleBtn.querySelector('.life-pause-label');

  function syncToggleButton() {
    runLabel.style.display = running ? 'none' : '';
    pauseLabel.style.display = running ? '' : 'none';
  }

  toggleBtn.addEventListener('click', () => {
    running = !running;
    syncToggleButton();
  });

  document.getElementById('life-reset').addEventListener('click', () => {
    if (renderer) renderer.randomize(aliveProbability);
  });

  const zoomSlider = document.getElementById('life-zoom-slider');
  const zoomLabel = document.getElementById('life-zoom-label');
  zoomSlider.addEventListener('input', (e) => {
    zoom = parseFloat(e.target.value);
    zoomLabel.textContent = `${zoom.toFixed(1)}x`;
  });

  const settingsPanel = document.getElementById('life-settings-panel');
  document.getElementById('life-settings-toggle').addEventListener('click', () => {
    settingsPanel.style.display = settingsPanel.style.display === 'none' ? '' : 'none';
  });
  document.getElementById('life-settings-close').addEventListener('click', () => {
    settingsPanel.style.display = 'none';
  });

  const probSlider = document.getElementById('life-prob-slider');
  const probLabel = document.getElementById('life-prob-label');
  probSlider.addEventListener('input', (e) => {
    aliveProbability = parseFloat(e.target.value);
    probLabel.textContent = `${Math.round(aliveProbability * 100)}%`;
  });

  document.getElementById('life-speed-select').addEventListener('change', (e) => {
    intervalMs = parseInt(e.target.value, 10);
  });

  attachCanvasNav(
    canvas,
    () => zoom,
    (z) => { zoom = z; zoomSlider.value = String(z); zoomLabel.textContent = `${z.toFixed(1)}x`; },
    () => zoomCenter,
    (c) => { zoomCenter = c; },
  );

  // Auto-play when scrolled into view, pause when scrolled away — same as
  // the real site's IntersectionObserver. Only randomizes on the first
  // time it becomes visible (matches real site's has_started guard).
  new IntersectionObserver((entries) => {
    for (const entry of entries) {
      if (entry.isIntersecting) {
        if (!hasStarted) {
          hasStarted = true;
          if (renderer) renderer.randomize(aliveProbability);
        }
        running = true;
      } else {
        running = false;
      }
      syncToggleButton();
    }
  }, { threshold: 0.1 }).observe(widget);

  function frame(ts) {
    if (renderer) {
      const cw = canvas.clientWidth || 720;
      const ch = canvas.clientHeight || 720;
      if (canvas.width !== cw || canvas.height !== ch) {
        canvas.width = cw;
        canvas.height = ch;
      }
      if (running && ts - lastStep >= intervalMs) {
        renderer.step();
        lastStep = ts;
      }
      const dark = document.documentElement.classList.contains('dark');
      renderer.draw(cw, ch, dark, zoom, zoomCenter[0], zoomCenter[1]);
    }
    requestAnimationFrame(frame);
  }

  // Init renderer once canvas has real layout dimensions.
  const initRenderer = () => {
    if (renderer) return;
    canvas.width = canvas.clientWidth || 720;
    canvas.height = canvas.clientHeight || 720;
    try {
      renderer = new LifeGl(canvas, GRID_SIZE, GRID_SIZE);
      renderer.randomize(aliveProbability);
    } catch (e) {
      console.error('Life WebGL init:', e.message);
    }
  };
  initRenderer();

  requestAnimationFrame(frame);
}
