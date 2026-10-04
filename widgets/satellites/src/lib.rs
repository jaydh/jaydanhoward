//! 3D satellite globe (Foster widget): WebGL2 earth, equator, pole axis and
//! one point per tracked satellite, colored by orbit band, with drag/zoom
//! camera, preset views and band filters. Positions come from the server's
//! shared SGP4 snapshot (`GET /api/satellites`, polled once a second while
//! on screen) and are interpolated between the two latest samples for smooth
//! motion. Run/pause and speed live in the shared "satellites" Foster
//! machine this widget sits inside.

use serde::Deserialize;
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::{Element, HtmlCanvasElement, HtmlElement, WebGl2RenderingContext as Gl, WebGlBuffer, WebGlProgram};
use widget_common::*;

const VERT: &str = r#"#version 300 es
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
}"#;

const FRAG: &str = r#"#version 300 es
precision highp float;
in vec3 vColor;
out vec4 fragColor;
void main() {
    fragColor = vec4(vColor, 1.0);
}"#;

const ASTRANIS_IDS: [u32; 5] = [56371, 62454, 62455, 62456, 62457];
const POLL_MS: i32 = 1000;

#[derive(Deserialize, Clone, Copy)]
struct Pos {
    x: f32,
    y: f32,
    z: f32,
    altitude_km: f64,
    inclination_deg: f64,
    norad_id: u32,
}

#[derive(Deserialize)]
struct Snapshot {
    time_ms: f64,
    count: u64,
    positions: Vec<Pos>,
}

fn is_geo(alt: f64, incl: f64) -> bool {
    alt > 35000.0 && alt < 37000.0 && incl.abs() < 5.0
}

fn band_index(alt: f64, incl: f64) -> u32 {
    if is_geo(alt, incl) { 4 } else if alt < 600.0 { 0 } else if alt < 2000.0 { 1 }
    else if alt < 20000.0 { 2 } else if alt < 35000.0 { 3 } else { 5 }
}

fn color(alt: f64, incl: f64, astranis: bool) -> [f32; 3] {
    if astranis { return [0.0, 0.86, 0.71]; }
    if is_geo(alt, incl) { return [1.0, 0.3, 0.3]; }
    if alt < 600.0 { [0.3, 0.8, 1.0] } else if alt < 2000.0 { [0.5, 1.0, 0.5] }
    else if alt < 20000.0 { [1.0, 0.8, 0.2] } else if alt < 35000.0 { [1.0, 0.5, 0.2] } else { [0.8, 0.6, 1.0] }
}

fn perspective(fov_deg: f32, aspect: f32, near: f32, far: f32) -> [f32; 16] {
    let f = 1.0 / (fov_deg.to_radians() / 2.0).tan();
    let nf = 1.0 / (near - far);
    [f / aspect, 0., 0., 0., 0., f, 0., 0., 0., 0., (far + near) * nf, -1., 0., 0., 2. * far * near * nf, 0.]
}

fn look_at(eye: [f32; 3]) -> [f32; 16] {
    let norm = |v: [f32; 3]| { let l = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt(); [v[0] / l, v[1] / l, v[2] / l] };
    let cross = |a: [f32; 3], b: [f32; 3]| [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]];
    let dot = |a: [f32; 3], b: [f32; 3]| a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
    let z = norm(eye); // center is the origin
    let x = norm(cross([0., 1., 0.], z));
    let y = cross(z, x);
    [x[0], y[0], z[0], 0., x[1], y[1], z[1], 0., x[2], y[2], z[2], 0., -dot(x, eye), -dot(y, eye), -dot(z, eye), 1.]
}

struct Renderer {
    gl: Gl,
    prog: WebGlProgram,
    earth: (WebGlBuffer, WebGlBuffer, i32),
    equator: (WebGlBuffer, i32),
    pole_axis: WebGlBuffer,
    pole_tips: WebGlBuffer,
    sats: WebGlBuffer,
    astranis: WebGlBuffer,
    counts: (i32, i32),
    cam_h: f32,
    cam_v: f32,
    distance: f32,
    auto_rotate: bool,
}

fn static_buffer(gl: &Gl, target: u32, data: &js_sys::Object) -> Result<WebGlBuffer, String> {
    let b = gl.create_buffer().ok_or("createBuffer failed")?;
    gl.bind_buffer(target, Some(&b));
    gl.buffer_data_with_array_buffer_view(target, data, Gl::STATIC_DRAW);
    Ok(b)
}

impl Renderer {
    fn new(canvas: &HtmlCanvasElement) -> Result<Self, String> {
        let gl = webgl2(canvas)?;
        let prog = link_program(&gl, VERT, FRAG)?;
        gl.enable(Gl::DEPTH_TEST);
        gl.clear_color(0.0, 0.0, 0.0, 1.0);

        // Earth: 32x32 UV sphere, blue shaded by latitude.
        let (lat_b, lon_b) = (32u16, 32u16);
        let mut verts = Vec::new();
        for lat in 0..=lat_b {
            let theta = lat as f32 * std::f32::consts::PI / lat_b as f32;
            for lon in 0..=lon_b {
                let phi = lon as f32 * 2.0 * std::f32::consts::PI / lon_b as f32;
                let (x, y, z) = (phi.cos() * theta.sin(), theta.cos(), phi.sin() * theta.sin());
                verts.extend([x, y, z, 0.2 + 0.3 * (y + 1.0) / 2.0, 0.4 + 0.3 * (y + 1.0) / 2.0, 0.8]);
            }
        }
        let mut idx: Vec<u16> = Vec::new();
        for lat in 0..lat_b {
            for lon in 0..lon_b {
                let first = lat * (lon_b + 1) + lon;
                let second = first + lon_b + 1;
                idx.extend([first, second, first + 1, second, second + 1, first + 1]);
            }
        }
        let earth = (
            static_buffer(&gl, Gl::ARRAY_BUFFER, &js_sys::Float32Array::from(&verts[..]))?,
            static_buffer(&gl, Gl::ELEMENT_ARRAY_BUFFER, &js_sys::Uint16Array::from(&idx[..]))?,
            idx.len() as i32,
        );

        let segments = 128;
        let equator: Vec<f32> = (0..=segments)
            .flat_map(|i| {
                let a = i as f32 / segments as f32 * 2.0 * std::f32::consts::PI;
                [1.01 * a.cos(), 0.0, 1.01 * a.sin(), 1.0, 0.9, 0.2]
            })
            .collect();
        let equator = (static_buffer(&gl, Gl::ARRAY_BUFFER, &js_sys::Float32Array::from(&equator[..]))?, segments + 1);
        let pole_axis: [f32; 18] = [0., -1.2, 0., 1., 0.45, 0.1, 0., 0., 0., 0.6, 0.6, 0.6, 0., 1.2, 0., 1., 1., 1.];
        let pole_tips: [f32; 12] = [0., 1.25, 0., 1., 1., 1., 0., -1.25, 0., 1., 0.45, 0.1];
        Ok(Self {
            pole_axis: static_buffer(&gl, Gl::ARRAY_BUFFER, &js_sys::Float32Array::from(&pole_axis[..]))?,
            pole_tips: static_buffer(&gl, Gl::ARRAY_BUFFER, &js_sys::Float32Array::from(&pole_tips[..]))?,
            sats: gl.create_buffer().ok_or("createBuffer failed")?,
            astranis: gl.create_buffer().ok_or("createBuffer failed")?,
            counts: (0, 0),
            gl, prog, earth, equator,
            cam_h: 0.0, cam_v: 0.5, distance: 18.0, auto_rotate: true,
        })
    }

    fn adjust_zoom(&mut self, delta: f32) {
        self.distance = (self.distance - delta * self.distance * 0.1).clamp(1.5, 50.0);
    }

    fn rotate(&mut self, dx: f32, dy: f32) {
        self.auto_rotate = false;
        self.cam_h += dx * 0.01;
        self.cam_v = (self.cam_v - dy * 0.01).clamp(-1.5, 1.5);
    }

    fn preset(&mut self, name: &str) {
        self.auto_rotate = false;
        let half_pi = std::f32::consts::FRAC_PI_2;
        let v = match name { "equator" => 0.0, "north" => half_pi - 0.1, "south" => -(half_pi - 0.1), "oblique" => 0.5, _ => return };
        self.cam_h = 0.0;
        self.cam_v = v;
    }

    fn set_satellites(&mut self, sats: &[f32], astranis: &[f32]) {
        for (buf, data) in [(&self.sats, sats), (&self.astranis, astranis)] {
            if !data.is_empty() {
                self.gl.bind_buffer(Gl::ARRAY_BUFFER, Some(buf));
                self.gl.buffer_data_with_array_buffer_view(Gl::ARRAY_BUFFER, &js_sys::Float32Array::from(data), Gl::DYNAMIC_DRAW);
            }
        }
        self.counts = ((sats.len() / 6) as i32, (astranis.len() / 6) as i32);
    }

    fn render(&mut self, aspect: f32) {
        let gl = &self.gl;
        gl.clear(Gl::COLOR_BUFFER_BIT | Gl::DEPTH_BUFFER_BIT);
        gl.use_program(Some(&self.prog));
        if self.auto_rotate {
            self.cam_h += 0.002;
        }
        let d = self.distance;
        let eye = [d * self.cam_h.cos() * self.cam_v.cos(), d * self.cam_v.sin(), d * self.cam_h.sin() * self.cam_v.cos()];
        let u = |n: &str| gl.get_uniform_location(&self.prog, n);
        gl.uniform_matrix4fv_with_f32_array(u("uProjectionMatrix").as_ref(), false, &perspective(45.0, aspect, 0.1, 100.0));
        gl.uniform_matrix4fv_with_f32_array(u("uModelViewMatrix").as_ref(), false, &look_at(eye));
        let point_size = u("u_point_size");
        let pos_loc = gl.get_attrib_location(&self.prog, "position") as u32;
        let col_loc = gl.get_attrib_location(&self.prog, "color") as u32;
        let bind = |buf: &WebGlBuffer| {
            gl.bind_buffer(Gl::ARRAY_BUFFER, Some(buf));
            gl.vertex_attrib_pointer_with_i32(pos_loc, 3, Gl::FLOAT, false, 24, 0);
            gl.enable_vertex_attrib_array(pos_loc);
            gl.vertex_attrib_pointer_with_i32(col_loc, 3, Gl::FLOAT, false, 24, 12);
            gl.enable_vertex_attrib_array(col_loc);
        };

        bind(&self.earth.0);
        gl.bind_buffer(Gl::ELEMENT_ARRAY_BUFFER, Some(&self.earth.1));
        gl.draw_elements_with_i32(Gl::TRIANGLES, self.earth.2, Gl::UNSIGNED_SHORT, 0);
        gl.line_width(2.0);
        bind(&self.equator.0);
        gl.draw_arrays(Gl::LINE_STRIP, 0, self.equator.1);
        bind(&self.pole_axis);
        gl.draw_arrays(Gl::LINE_STRIP, 0, 3);
        gl.uniform1f(point_size.as_ref(), 8.0);
        bind(&self.pole_tips);
        gl.draw_arrays(Gl::POINTS, 0, 2);
        if self.counts.0 > 0 {
            gl.uniform1f(point_size.as_ref(), 2.0);
            bind(&self.sats);
            gl.draw_arrays(Gl::POINTS, 0, self.counts.0);
        }
        if self.counts.1 > 0 {
            gl.uniform1f(point_size.as_ref(), 5.0);
            bind(&self.astranis);
            gl.draw_arrays(Gl::POINTS, 0, self.counts.1);
        }
    }
}

struct Samples {
    prev: Option<(Snapshot, f64)>,
    curr: Option<(Snapshot, f64)>,
}

fn now() -> f64 {
    window().performance().map(|p| p.now()).unwrap_or(0.0)
}

async fn fetch_snapshot() -> Result<Snapshot, JsValue> {
    let resp: web_sys::Response = wasm_bindgen_futures::JsFuture::from(window().fetch_with_str("/api/satellites")).await?.dyn_into()?;
    let text = wasm_bindgen_futures::JsFuture::from(resp.text()?).await?.as_string().unwrap_or_default();
    serde_json::from_str(&text).map_err(|e| JsValue::from_str(&e.to_string()))
}

/// Repeat `f` while a button is held (after 200ms, every 50ms); a plain
/// click runs it once.
fn hold_repeat(button: &Element, f: Rc<dyn Fn()>) {
    let timers: Rc<Cell<(i32, i32)>> = Rc::new(Cell::new((0, 0)));
    let clear = {
        let timers = timers.clone();
        Rc::new(move || {
            let (t, i) = timers.replace((0, 0));
            window().clear_timeout_with_handle(t);
            window().clear_interval_with_handle(i);
        })
    };
    { let f = f.clone(); on(button, "click", move |_: web_sys::Event| f()); }
    {
        let (clear, timers) = (clear.clone(), timers.clone());
        on(button, "mousedown", move |_: web_sys::Event| {
            clear();
            let (f, timers2) = (f.clone(), timers.clone());
            let start = Closure::once_into_js(move || {
                let tick = Closure::<dyn FnMut()>::new(move || f());
                let id = window().set_interval_with_callback_and_timeout_and_arguments_0(tick.as_ref().unchecked_ref(), 50).unwrap();
                tick.forget();
                timers2.set((0, id));
            });
            let id = window().set_timeout_with_callback_and_timeout_and_arguments_0(start.unchecked_ref(), 200).unwrap();
            timers.set((id, 0));
        });
    }
    for ev in ["mouseup", "mouseleave"] {
        let clear = clear.clone();
        on(button, ev, move |_: web_sys::Event| clear());
    }
}

/// Called by Foster with the `.sat-canvas-wrap` element.
#[wasm_bindgen]
pub fn mount(_el: Element) {
    let canvas: HtmlCanvasElement = by_id("sat-canvas");
    let root: Element = canvas.closest("[fx-machine]").ok().flatten().unwrap_or_else(|| canvas.clone().into());
    canvas.set_width(canvas.client_width().max(1) as u32);
    canvas.set_height(600);
    let renderer = match Renderer::new(&canvas) {
        Ok(r) => Rc::new(RefCell::new(r)),
        Err(e) => {
            web_sys::console::error_1(&format!("satellites WebGL init: {e}").into());
            return;
        }
    };

    // Drag to rotate.
    let drag: Rc<Cell<Option<(i32, i32)>>> = Rc::new(Cell::new(None));
    { let drag = drag.clone(); on(&canvas, "mousedown", move |e: web_sys::MouseEvent| drag.set(Some((e.client_x(), e.client_y())))); }
    {
        let (drag, r) = (drag.clone(), renderer.clone());
        on(&canvas, "mousemove", move |e: web_sys::MouseEvent| {
            let Some((lx, ly)) = drag.get() else { return };
            r.borrow_mut().rotate((e.client_x() - lx) as f32, (e.client_y() - ly) as f32);
            drag.set(Some((e.client_x(), e.client_y())));
        });
    }
    { let drag = drag.clone(); on(&window(), "mouseup", move |_: web_sys::Event| drag.set(None)); }
    { let drag = drag.clone(); on(&canvas, "mouseleave", move |_: web_sys::Event| drag.set(None)); }

    { let r = renderer.clone(); hold_repeat(&by_id("sat-zoom-in"), Rc::new(move || r.borrow_mut().adjust_zoom(1.0))); }
    { let r = renderer.clone(); hold_repeat(&by_id("sat-zoom-out"), Rc::new(move || r.borrow_mut().adjust_zoom(-1.0))); }
    let presets = document().query_selector_all(".sat-controls-preset button[data-preset]").unwrap();
    for i in 0..presets.length() {
        let btn: HtmlElement = presets.item(i).unwrap().unchecked_into();
        let (r, name) = (renderer.clone(), btn.dataset().get("preset").unwrap_or_default());
        on(&btn, "click", move |_: web_sys::Event| r.borrow_mut().preset(&name));
    }

    // Orbit-band filter + Astranis toggle (per-visitor view state).
    let band_mask = Rc::new(Cell::new(0b0011_1111u32));
    let show_astranis = Rc::new(Cell::new(true));
    let filters = document().query_selector_all(".sat-filter").unwrap();
    for i in 0..filters.length() {
        let btn: HtmlElement = filters.item(i).unwrap().unchecked_into();
        let band = btn.dataset().get("band").unwrap_or_default();
        let (mask, astr, r, b) = (band_mask.clone(), show_astranis.clone(), renderer.clone(), btn.clone());
        on(&btn, "click", move |_: web_sys::Event| {
            if band == "astranis" {
                astr.set(!astr.get());
                let _ = b.class_list().toggle_with_force("off", !astr.get());
                r.borrow_mut().distance = if astr.get() { 18.0 } else { 4.0 };
            } else if let Ok(n) = band.parse::<u32>() {
                mask.set(mask.get() ^ (1 << n));
                let _ = b.class_list().toggle_with_force("off", mask.get() & (1 << n) == 0);
            }
        });
    }

    let visible = Rc::new(Cell::new(false));
    { let visible = visible.clone(); on_visibility(&root, move |v| visible.set(v)); }

    // Poll the shared server snapshot while on screen.
    let samples = Rc::new(RefCell::new(Samples { prev: None, curr: None }));
    {
        let (samples, visible) = (samples.clone(), visible.clone());
        let poll = Closure::<dyn FnMut()>::new(move || {
            if !visible.get() {
                return; // off screen: skip the ~2MB fetch + parse
            }
            let samples = samples.clone();
            wasm_bindgen_futures::spawn_local(async move {
                match fetch_snapshot().await {
                    Ok(snap) if !snap.positions.is_empty() => {
                        by_id::<HtmlElement>("sat-count").set_text_content(Some(&snap.count.to_string()));
                        let mins = (snap.time_ms / 60_000.0).floor() as i64 % 1440;
                        by_id::<HtmlElement>("sat-time").set_text_content(Some(&format!("{:02}:{:02}", mins / 60, mins % 60)));
                        let mut s = samples.borrow_mut();
                        s.prev = s.curr.take();
                        s.curr = Some((snap, now()));
                    }
                    Ok(_) => {}
                    Err(e) => web_sys::console::error_2(&"Failed to poll satellite positions".into(), &e),
                }
            });
        });
        let f: &js_sys::Function = poll.as_ref().unchecked_ref();
        let _ = f.call0(&JsValue::NULL);
        window().set_interval_with_callback_and_timeout_and_arguments_0(f, POLL_MS).unwrap();
        poll.forget();
    }

    let (mut sats, mut astranis): (Vec<f32>, Vec<f32>) = (Vec::new(), Vec::new());
    animate(move |_| {
        if !visible.get() {
            return 250;
        }
        let s = samples.borrow();
        if let Some((curr, curr_at)) = &s.curr {
            // Interpolate between the two latest samples (same satellite
            // order), extrapolating up to 30% past the latest.
            let lerp = s.prev.as_ref().filter(|(p, _)| p.positions.len() == curr.positions.len()).map(|(p, p_at)| {
                let span = if curr_at - p_at > 0.0 { curr_at - p_at } else { POLL_MS as f64 };
                (p, ((now() - curr_at) / span).min(1.3) as f32)
            });
            sats.clear();
            astranis.clear();
            let (mask, show_a) = (band_mask.get(), show_astranis.get());
            for (i, b) in curr.positions.iter().enumerate() {
                let is_a = ASTRANIS_IDS.contains(&b.norad_id);
                if (is_a && !show_a) || (!is_a && (mask >> band_index(b.altitude_km, b.inclination_deg)) & 1 == 0) {
                    continue;
                }
                let (x, y, z) = match lerp {
                    Some((p, t)) => {
                        let a = p.positions[i];
                        (a.x + (b.x - a.x) * t, a.y + (b.y - a.y) * t, a.z + (b.z - a.z) * t)
                    }
                    None => (b.x, b.y, b.z),
                };
                let c = color(b.altitude_km, b.inclination_deg, is_a);
                (if is_a { &mut astranis } else { &mut sats }).extend([x, y, z, c[0], c[1], c[2]]);
            }
            renderer.borrow_mut().set_satellites(&sats, &astranis);
        }
        let aspect = canvas.width() as f32 / canvas.height().max(1) as f32;
        renderer.borrow_mut().render(aspect);
        0
    });
}
