// Real 3D satellite globe — WebGL2, ported line-for-line from the real
// site's src/components/satellite_renderer.rs (shaders, sphere/equator/
// pole geometry, camera matrices, draw calls) and satellite_tracker.rs
// (mouse-drag camera, zoom, preset views, orbit-type filter, Astranis
// pin). The one thing NOT ported line-for-line: the real site re-runs
// sgp4 in the browser every animation frame; Foster has no custom WASM,
// so src/satellites.rs runs the same sgp4 crate server-side once per
// tick (shared by every visitor) and this file polls /api/satellites for
// the latest real snapshot, interpolating between the two most recent
// ones for smooth motion. Foster itself only owns the run/pause +
// playback-speed labels (fx-machine="satellites"); it never sees a
// single satellite position.

const VERTEX_SHADER_SOURCE = `#version 300 es
in vec3 position;
in vec3 color;

out vec3 vColor;

uniform mat4 uModelViewMatrix;
uniform mat4 uProjectionMatrix;
uniform float u_point_size;

void main() {
    vColor = color;
    gl_Position = uProjectionMatrix * uModelViewMatrix * vec4(position, 1.0);
    gl_PointSize = u_point_size;
}
`;

const FRAGMENT_SHADER_SOURCE = `#version 300 es
precision highp float;

in vec3 vColor;
out vec4 fragColor;

void main() {
    fragColor = vec4(vColor, 1.0);
}
`;

const ASTRANIS_IDS = new Set([56371, 62454, 62455, 62456, 62457]);
const POLL_MS = 1000;

function compileShader(gl, type, source) {
  const shader = gl.createShader(type);
  gl.shaderSource(shader, source);
  gl.compileShader(shader);
  if (!gl.getShaderParameter(shader, gl.COMPILE_STATUS)) {
    const log = gl.getShaderInfoLog(shader);
    gl.deleteShader(shader);
    throw new Error('Shader compile error: ' + log);
  }
  return shader;
}

function linkProgram(gl, vert, frag) {
  const program = gl.createProgram();
  gl.attachShader(program, vert);
  gl.attachShader(program, frag);
  gl.linkProgram(program);
  if (!gl.getProgramParameter(program, gl.LINK_STATUS)) {
    const log = gl.getProgramInfoLog(program);
    gl.deleteProgram(program);
    throw new Error('Program link error: ' + log);
  }
  return program;
}

function generateSphere(radius, latBands, lonBands) {
  const vertices = [];
  const indices = [];

  for (let lat = 0; lat <= latBands; lat++) {
    const theta = (lat * Math.PI) / latBands;
    const sinTheta = Math.sin(theta);
    const cosTheta = Math.cos(theta);

    for (let lon = 0; lon <= lonBands; lon++) {
      const phi = (lon * 2 * Math.PI) / lonBands;
      const sinPhi = Math.sin(phi);
      const cosPhi = Math.cos(phi);

      const x = cosPhi * sinTheta;
      const y = cosTheta;
      const z = sinPhi * sinTheta;

      vertices.push(radius * x, radius * y, radius * z);
      vertices.push(0.2 + (0.3 * (y + 1.0)) / 2.0, 0.4 + (0.3 * (y + 1.0)) / 2.0, 0.8);
    }
  }

  for (let lat = 0; lat < latBands; lat++) {
    for (let lon = 0; lon < lonBands; lon++) {
      const first = lat * (lonBands + 1) + lon;
      const second = first + lonBands + 1;
      indices.push(first, second, first + 1);
      indices.push(second, second + 1, first + 1);
    }
  }

  return { vertices: new Float32Array(vertices), indices: new Uint16Array(indices) };
}

function perspectiveMatrix(fovDegrees, aspect, near, far) {
  const f = 1.0 / Math.tan((fovDegrees * Math.PI) / 360.0);
  const nf = 1.0 / (near - far);
  return new Float32Array([
    f / aspect, 0, 0, 0,
    0, f, 0, 0,
    0, 0, (far + near) * nf, -1,
    0, 0, 2 * far * near * nf, 0,
  ]);
}

function lookAt(eye, center, up) {
  let z = [eye[0] - center[0], eye[1] - center[1], eye[2] - center[2]];
  let zLen = Math.hypot(z[0], z[1], z[2]);
  z = [z[0] / zLen, z[1] / zLen, z[2] / zLen];

  let x = [
    up[1] * z[2] - up[2] * z[1],
    up[2] * z[0] - up[0] * z[2],
    up[0] * z[1] - up[1] * z[0],
  ];
  let xLen = Math.hypot(x[0], x[1], x[2]);
  x = [x[0] / xLen, x[1] / xLen, x[2] / xLen];

  const y = [
    z[1] * x[2] - z[2] * x[1],
    z[2] * x[0] - z[0] * x[2],
    z[0] * x[1] - z[1] * x[0],
  ];

  return new Float32Array([
    x[0], y[0], z[0], 0,
    x[1], y[1], z[1], 0,
    x[2], y[2], z[2], 0,
    -(x[0] * eye[0] + x[1] * eye[1] + x[2] * eye[2]),
    -(y[0] * eye[0] + y[1] * eye[1] + y[2] * eye[2]),
    -(z[0] * eye[0] + z[1] * eye[1] + z[2] * eye[2]),
    1,
  ]);
}

function multiplyMatrices(a, b) {
  const result = new Float32Array(16);
  for (let i = 0; i < 4; i++) {
    for (let j = 0; j < 4; j++) {
      let sum = 0;
      for (let k = 0; k < 4; k++) sum += a[i * 4 + k] * b[k * 4 + j];
      result[i * 4 + j] = sum;
    }
  }
  return result;
}

function getAltitudeColor(altitudeKm, inclinationDeg, isAstranis) {
  if (isAstranis) return [0.0, 0.86, 0.71];
  if (altitudeKm > 35000.0 && altitudeKm < 37000.0 && Math.abs(inclinationDeg) < 5.0) {
    return [1.0, 0.3, 0.3];
  }
  if (altitudeKm < 600.0) return [0.3, 0.8, 1.0];
  if (altitudeKm < 2000.0) return [0.5, 1.0, 0.5];
  if (altitudeKm < 20000.0) return [1.0, 0.8, 0.2];
  if (altitudeKm < 35000.0) return [1.0, 0.5, 0.2];
  return [0.8, 0.6, 1.0];
}

function bandIndex(altitudeKm, inclinationDeg) {
  if (altitudeKm > 35000.0 && altitudeKm < 37000.0 && Math.abs(inclinationDeg) < 5.0) return 4;
  if (altitudeKm < 600.0) return 0;
  if (altitudeKm < 2000.0) return 1;
  if (altitudeKm < 20000.0) return 2;
  if (altitudeKm < 35000.0) return 3;
  return 5;
}

class SatelliteRenderer {
  constructor(gl) {
    this.gl = gl;
    this.program = null;
    this.cameraAngleHorizontal = 0.0;
    this.cameraAngleVertical = 0.5;
    this.cameraDistance = 18.0;
    this.autoRotate = true;
    this.satellitePositions = [];
    this.astranisCount = 0;
    this.satVertexBuffer = null;
    this.astranisVertexBuffer = null;
  }

  initialize() {
    const gl = this.gl;
    const vert = compileShader(gl, gl.VERTEX_SHADER, VERTEX_SHADER_SOURCE);
    const frag = compileShader(gl, gl.FRAGMENT_SHADER, FRAGMENT_SHADER_SOURCE);
    this.program = linkProgram(gl, vert, frag);

    gl.enable(gl.DEPTH_TEST);
    gl.clearColor(0.0, 0.0, 0.0, 1.0);

    this.createEarthSphere();
    this.createEquatorLine();
    this.createPoleAxis();
    this.createPoleTips();
  }

  createEarthSphere() {
    const gl = this.gl;
    const { vertices, indices } = generateSphere(1.0, 32, 32);
    this.earthIndexCount = indices.length;

    this.earthVertexBuffer = gl.createBuffer();
    gl.bindBuffer(gl.ARRAY_BUFFER, this.earthVertexBuffer);
    gl.bufferData(gl.ARRAY_BUFFER, vertices, gl.STATIC_DRAW);

    this.earthIndexBuffer = gl.createBuffer();
    gl.bindBuffer(gl.ELEMENT_ARRAY_BUFFER, this.earthIndexBuffer);
    gl.bufferData(gl.ELEMENT_ARRAY_BUFFER, indices, gl.STATIC_DRAW);
  }

  createEquatorLine() {
    const gl = this.gl;
    const radius = 1.01;
    const segments = 128;
    const vertices = [];
    for (let i = 0; i <= segments; i++) {
      const angle = (i / segments) * 2.0 * Math.PI;
      const x = radius * Math.cos(angle);
      const z = radius * Math.sin(angle);
      vertices.push(x, 0.0, z, 1.0, 0.9, 0.2);
    }
    this.equatorVertexCount = segments + 1;
    this.equatorVertexBuffer = gl.createBuffer();
    gl.bindBuffer(gl.ARRAY_BUFFER, this.equatorVertexBuffer);
    gl.bufferData(gl.ARRAY_BUFFER, new Float32Array(vertices), gl.STATIC_DRAW);
  }

  createPoleAxis() {
    const gl = this.gl;
    const vertices = new Float32Array([
      0.0, -1.2, 0.0, 1.0, 0.45, 0.1,
      0.0, 0.0, 0.0, 0.6, 0.6, 0.6,
      0.0, 1.2, 0.0, 1.0, 1.0, 1.0,
    ]);
    this.poleAxisVertexCount = 3;
    this.poleAxisVertexBuffer = gl.createBuffer();
    gl.bindBuffer(gl.ARRAY_BUFFER, this.poleAxisVertexBuffer);
    gl.bufferData(gl.ARRAY_BUFFER, vertices, gl.STATIC_DRAW);
  }

  createPoleTips() {
    const gl = this.gl;
    const vertices = new Float32Array([
      0.0, 1.25, 0.0, 1.0, 1.0, 1.0,
      0.0, -1.25, 0.0, 1.0, 0.45, 0.1,
    ]);
    this.poleTipsVertexBuffer = gl.createBuffer();
    gl.bindBuffer(gl.ARRAY_BUFFER, this.poleTipsVertexBuffer);
    gl.bufferData(gl.ARRAY_BUFFER, vertices, gl.STATIC_DRAW);
  }

  adjustZoom(delta) {
    const zoomFactor = this.cameraDistance * 0.1;
    this.cameraDistance = Math.min(50.0, Math.max(1.5, this.cameraDistance - delta * zoomFactor));
  }

  setCameraDistance(distance) {
    this.cameraDistance = Math.min(50.0, Math.max(1.5, distance));
  }

  rotateCamera(deltaX, deltaY) {
    this.autoRotate = false;
    this.cameraAngleHorizontal += deltaX * 0.01;
    this.cameraAngleVertical = Math.min(1.5, Math.max(-1.5, this.cameraAngleVertical - deltaY * 0.01));
  }

  setPresetView(preset) {
    this.autoRotate = false;
    if (preset === 'equator') {
      this.cameraAngleHorizontal = 0.0;
      this.cameraAngleVertical = 0.0;
    } else if (preset === 'north') {
      this.cameraAngleHorizontal = 0.0;
      this.cameraAngleVertical = Math.PI / 2.0 - 0.1;
    } else if (preset === 'south') {
      this.cameraAngleHorizontal = 0.0;
      this.cameraAngleVertical = -(Math.PI / 2.0 - 0.1);
    } else if (preset === 'oblique') {
      this.cameraAngleHorizontal = 0.0;
      this.cameraAngleVertical = 0.5;
    }
  }

  updateSatellites(positions) {
    this.satellitePositions = positions;
    const regular = [];
    const astranis = [];

    for (const pos of positions) {
      const isAstranis = ASTRANIS_IDS.has(pos.norad_id);
      const color = getAltitudeColor(pos.altitude_km, pos.inclination_deg, isAstranis);
      const entry = [pos.x, pos.y, pos.z, color[0], color[1], color[2]];
      if (isAstranis) astranis.push(...entry);
      else regular.push(...entry);
    }

    const gl = this.gl;
    if (regular.length) {
      if (!this.satVertexBuffer) this.satVertexBuffer = gl.createBuffer();
      gl.bindBuffer(gl.ARRAY_BUFFER, this.satVertexBuffer);
      gl.bufferData(gl.ARRAY_BUFFER, new Float32Array(regular), gl.DYNAMIC_DRAW);
    }

    this.astranisCount = astranis.length / 6;
    if (this.astranisCount > 0) {
      if (!this.astranisVertexBuffer) this.astranisVertexBuffer = gl.createBuffer();
      gl.bindBuffer(gl.ARRAY_BUFFER, this.astranisVertexBuffer);
      gl.bufferData(gl.ARRAY_BUFFER, new Float32Array(astranis), gl.DYNAMIC_DRAW);
    }
  }

  render() {
    const gl = this.gl;
    gl.clear(gl.COLOR_BUFFER_BIT | gl.DEPTH_BUFFER_BIT);
    if (!this.program) return;
    gl.useProgram(this.program);

    if (this.autoRotate) this.cameraAngleHorizontal += 0.002;

    const cameraX = this.cameraDistance * Math.cos(this.cameraAngleHorizontal) * Math.cos(this.cameraAngleVertical);
    const cameraZ = this.cameraDistance * Math.sin(this.cameraAngleHorizontal) * Math.cos(this.cameraAngleVertical);
    const cameraY = this.cameraDistance * Math.sin(this.cameraAngleVertical);

    const canvas = gl.canvas;
    const projection = perspectiveMatrix(45.0, canvas.width / canvas.height, 0.1, 100.0);
    const view = lookAt([cameraX, cameraY, cameraZ], [0, 0, 0], [0, 1, 0]);
    const model = new Float32Array([1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1]);
    const modelView = multiplyMatrices(view, model);

    const program = this.program;
    gl.uniformMatrix4fv(gl.getUniformLocation(program, 'uProjectionMatrix'), false, projection);
    gl.uniformMatrix4fv(gl.getUniformLocation(program, 'uModelViewMatrix'), false, modelView);
    const pointSizeLoc = gl.getUniformLocation(program, 'u_point_size');
    const positionLoc = gl.getAttribLocation(program, 'position');
    const colorLoc = gl.getAttribLocation(program, 'color');
    const stride = 6 * 4;

    const bindAttrs = () => {
      gl.vertexAttribPointer(positionLoc, 3, gl.FLOAT, false, stride, 0);
      gl.enableVertexAttribArray(positionLoc);
      gl.vertexAttribPointer(colorLoc, 3, gl.FLOAT, false, stride, 3 * 4);
      gl.enableVertexAttribArray(colorLoc);
    };

    if (this.earthVertexBuffer && this.earthIndexBuffer) {
      gl.bindBuffer(gl.ARRAY_BUFFER, this.earthVertexBuffer);
      gl.bindBuffer(gl.ELEMENT_ARRAY_BUFFER, this.earthIndexBuffer);
      bindAttrs();
      gl.drawElements(gl.TRIANGLES, this.earthIndexCount, gl.UNSIGNED_SHORT, 0);
    }

    if (this.equatorVertexBuffer) {
      gl.bindBuffer(gl.ARRAY_BUFFER, this.equatorVertexBuffer);
      bindAttrs();
      gl.lineWidth(2.0);
      gl.drawArrays(gl.LINE_STRIP, 0, this.equatorVertexCount);
    }

    if (this.poleAxisVertexBuffer) {
      gl.bindBuffer(gl.ARRAY_BUFFER, this.poleAxisVertexBuffer);
      bindAttrs();
      gl.lineWidth(2.0);
      gl.drawArrays(gl.LINE_STRIP, 0, this.poleAxisVertexCount);
    }

    if (this.poleTipsVertexBuffer) {
      gl.uniform1f(pointSizeLoc, 8.0);
      gl.bindBuffer(gl.ARRAY_BUFFER, this.poleTipsVertexBuffer);
      bindAttrs();
      gl.drawArrays(gl.POINTS, 0, 2);
    }

    const regularCount = this.satellitePositions.length - this.astranisCount;
    if (regularCount > 0 && this.satVertexBuffer) {
      gl.uniform1f(pointSizeLoc, 2.0);
      gl.bindBuffer(gl.ARRAY_BUFFER, this.satVertexBuffer);
      bindAttrs();
      gl.drawArrays(gl.POINTS, 0, regularCount);
    }

    if (this.astranisCount > 0 && this.astranisVertexBuffer) {
      gl.uniform1f(pointSizeLoc, 5.0);
      gl.bindBuffer(gl.ARRAY_BUFFER, this.astranisVertexBuffer);
      bindAttrs();
      gl.drawArrays(gl.POINTS, 0, this.astranisCount);
    }
  }
}

export function initSatellites() {
  const canvas = document.getElementById('sat-canvas');
  const root = document.querySelector('[fx-machine="satellites"]');
  if (!canvas || !root) return;

  const gl = canvas.getContext('webgl2');
  if (!gl) {
    console.error('WebGL2 not supported in this browser');
    return;
  }

  canvas.width = canvas.clientWidth || 1200;
  canvas.height = 600;

  const renderer = new SatelliteRenderer(gl);
  renderer.initialize();

  // Mouse-drag camera controls — same contract as satellite_tracker.rs.
  let dragging = false;
  let lastX = 0;
  let lastY = 0;
  canvas.addEventListener('mousedown', (e) => {
    dragging = true;
    lastX = e.clientX;
    lastY = e.clientY;
  });
  canvas.addEventListener('mousemove', (e) => {
    if (!dragging) return;
    const dx = e.clientX - lastX;
    const dy = e.clientY - lastY;
    renderer.rotateCamera(dx, dy);
    lastX = e.clientX;
    lastY = e.clientY;
  });
  window.addEventListener('mouseup', () => { dragging = false; });
  canvas.addEventListener('mouseleave', () => { dragging = false; });

  // Zoom buttons, press-and-hold repeats — same timing as the real UI.
  function holdRepeat(button, fn) {
    let interval = null;
    let timeout = null;
    const clear = () => {
      if (timeout) { clearTimeout(timeout); timeout = null; }
      if (interval) { clearInterval(interval); interval = null; }
    };
    button.addEventListener('click', fn);
    button.addEventListener('mousedown', () => {
      clear();
      timeout = setTimeout(() => { interval = setInterval(fn, 50); }, 200);
    });
    button.addEventListener('mouseup', clear);
    button.addEventListener('mouseleave', clear);
  }
  holdRepeat(document.getElementById('sat-zoom-in'), () => renderer.adjustZoom(1.0));
  holdRepeat(document.getElementById('sat-zoom-out'), () => renderer.adjustZoom(-1.0));

  document.querySelectorAll('.sat-controls-preset button[data-preset]').forEach((btn) => {
    btn.addEventListener('click', () => renderer.setPresetView(btn.dataset.preset));
  });

  // Orbit-type filter + Astranis toggle — purely client-side view state,
  // same as the real site's local `orbit_filter`/`show_astranis` signals.
  let orbitFilter = 0b00111111;
  let showAstranis = true;
  document.querySelectorAll('.sat-filter').forEach((btn) => {
    btn.addEventListener('click', () => {
      if (btn.dataset.band === 'astranis') {
        showAstranis = !showAstranis;
        btn.classList.toggle('off', !showAstranis);
        renderer.setCameraDistance(showAstranis ? 18.0 : 4.0);
      } else {
        const bit = 1 << Number(btn.dataset.band);
        orbitFilter ^= bit;
        btn.classList.toggle('off', (orbitFilter & bit) === 0);
      }
    });
  });

  // Real data: poll the shared server-computed snapshot (src/satellites.rs)
  // and interpolate between the two most recent real samples for smooth
  // motion between polls, instead of recomputing sgp4 in the browser.
  let prev = null;
  let curr = null;
  let prevAt = 0;
  let currAt = 0;

  async function poll() {
    try {
      const res = await fetch('/api/satellites');
      const data = await res.json();
      if (!data.positions || !data.positions.length) return;
      prev = curr;
      prevAt = currAt;
      curr = data;
      currAt = performance.now();
      document.getElementById('sat-count').textContent = data.count;
      const d = new Date(data.time_ms);
      document.getElementById('sat-time').textContent = d.toISOString().slice(11, 16);
    } catch (e) {
      console.error('Failed to poll satellite positions', e);
    }
  }

  function interpolated() {
    if (!curr) return [];
    if (!prev || prev.positions.length !== curr.positions.length) return curr.positions;
    const span = currAt - prevAt || POLL_MS;
    const t = Math.min(1.3, (performance.now() - currAt) / span);
    const out = new Array(curr.positions.length);
    for (let i = 0; i < curr.positions.length; i++) {
      const a = prev.positions[i];
      const b = curr.positions[i];
      out[i] = {
        x: a.x + (b.x - a.x) * t,
        y: a.y + (b.y - a.y) * t,
        z: a.z + (b.z - a.z) * t,
        altitude_km: b.altitude_km,
        inclination_deg: b.inclination_deg,
        norad_id: b.norad_id,
      };
    }
    return out;
  }

  function frame() {
    const all = interpolated();
    const filtered = all.filter((p) => {
      if (ASTRANIS_IDS.has(p.norad_id)) return showAstranis;
      const idx = bandIndex(p.altitude_km, p.inclination_deg);
      return (orbitFilter >> idx) & 1;
    });
    renderer.updateSatellites(filtered);
    renderer.render();
    requestAnimationFrame(frame);
  }

  // Speed label — steps_per_tick lands on the DOM via a hidden fx-text
  // span (Foster's context binding only does scalar top-level lookups);
  // reformat it into m/s or h/s whenever the machine's snapshot changes.
  function updateSpeedLabel() {
    const raw = document.getElementById('sat-steps-raw');
    const stepsPerTick = Number(raw?.textContent || 12);
    const simMinPerSec = stepsPerTick * 5.0;
    document.getElementById('sat-speed-label').textContent =
      simMinPerSec < 60 ? `${simMinPerSec.toFixed(0)}m/s` : `${(simMinPerSec / 60).toFixed(1)}h/s`;
  }
  new MutationObserver(updateSpeedLabel).observe(root, { attributes: true, attributeFilter: ['data-fx-version'] });
  updateSpeedLabel();

  poll();
  setInterval(poll, POLL_MS);
  requestAnimationFrame(frame);
}
