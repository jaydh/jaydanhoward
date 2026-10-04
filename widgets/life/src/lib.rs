//! Conway's Game of Life — GPU ping-pong simulation (Foster widget).
//!
//! A 512x512 R8 texture holds cell state; a fragment shader computes the next
//! generation into the other texture via a framebuffer, and a draw shader maps
//! alive/dead to colors with zoom/pan. Run/pause/reset/zoom/settings are
//! per-visitor UI state, all handled here. Foster mounts this on
//! `#life-widget` (`fx-widget`) when it nears the viewport; the GL setup
//! itself waits until it's actually on screen.

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use wasm_bindgen::prelude::*;
use web_sys::{
    Element, HtmlCanvasElement, HtmlElement, HtmlInputElement, HtmlSelectElement,
    WebGl2RenderingContext as Gl, WebGlFramebuffer, WebGlProgram, WebGlTexture, WebGlVertexArrayObject,
};
use widget_common::*;

// A 2048 grid ran ~4M fragment invocations (8 texture samples each) per tick
// for a canvas that maxes out at 720px; 512 is still finer than the display.
const GRID: i32 = 512;

const VERT: &str = r#"#version 300 es
in vec2 a_pos;
out vec2 v_uv;
void main() {
    v_uv = a_pos * 0.5 + 0.5;
    gl_Position = vec4(a_pos, 0.0, 1.0);
}"#;

const STEP_FRAG: &str = r#"#version 300 es
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
}"#;

const DRAW_FRAG: &str = r#"#version 300 es
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
}"#;

struct LifeGl {
    gl: Gl,
    step_prog: WebGlProgram,
    draw_prog: WebGlProgram,
    textures: [WebGlTexture; 2],
    fbs: [WebGlFramebuffer; 2],
    quad: WebGlVertexArrayObject,
    current: usize,
}

impl LifeGl {
    fn new(canvas: &HtmlCanvasElement) -> Result<Self, String> {
        let gl = webgl2(canvas)?;
        let step_prog = link_program(&gl, VERT, STEP_FRAG)?;
        let draw_prog = link_program(&gl, VERT, DRAW_FRAG)?;
        let quad = fullscreen_quad(&gl, &step_prog)?;
        let textures = [make_texture(&gl)?, make_texture(&gl)?];
        let fbs = [make_framebuffer(&gl, &textures[0])?, make_framebuffer(&gl, &textures[1])?];
        Ok(Self { gl, step_prog, draw_prog, textures, fbs, quad, current: 0 })
    }

    fn upload(&self, tex: &WebGlTexture, data: &[u8]) {
        self.gl.bind_texture(Gl::TEXTURE_2D, Some(tex));
        let _ = self.gl.tex_image_2d_with_i32_and_i32_and_i32_and_format_and_type_and_opt_u8_array(
            Gl::TEXTURE_2D, 0, Gl::R8 as i32, GRID, GRID, 0, Gl::RED, Gl::UNSIGNED_BYTE, Some(data),
        );
    }

    fn randomize(&self, probability: f64) {
        let n = (GRID * GRID) as usize;
        let data: Vec<u8> = (0..n).map(|_| if js_sys::Math::random() < probability { 255 } else { 0 }).collect();
        self.upload(&self.textures[self.current], &data);
        // Clear the back buffer so the first step doesn't read garbage.
        self.upload(&self.textures[1 - self.current], &vec![0; n]);
        self.gl.bind_texture(Gl::TEXTURE_2D, None);
    }

    fn step(&mut self) {
        let gl = &self.gl;
        let next = 1 - self.current;
        gl.bind_framebuffer(Gl::FRAMEBUFFER, Some(&self.fbs[next]));
        gl.viewport(0, 0, GRID, GRID);
        gl.use_program(Some(&self.step_prog));
        gl.active_texture(Gl::TEXTURE0);
        gl.bind_texture(Gl::TEXTURE_2D, Some(&self.textures[self.current]));
        gl.uniform1i(gl.get_uniform_location(&self.step_prog, "u_state").as_ref(), 0);
        gl.uniform2f(gl.get_uniform_location(&self.step_prog, "u_res").as_ref(), GRID as f32, GRID as f32);
        gl.bind_vertex_array(Some(&self.quad));
        gl.draw_arrays(Gl::TRIANGLES, 0, 6);
        gl.bind_vertex_array(None);
        gl.bind_framebuffer(Gl::FRAMEBUFFER, None);
        self.current = next;
    }

    fn draw(&self, w: i32, h: i32, dark: bool, zoom: f64, (zx, zy): (f64, f64)) {
        let gl = &self.gl;
        let p = &self.draw_prog;
        gl.viewport(0, 0, w, h);
        gl.use_program(Some(p));
        gl.active_texture(Gl::TEXTURE0);
        gl.bind_texture(Gl::TEXTURE_2D, Some(&self.textures[self.current]));
        gl.uniform1i(gl.get_uniform_location(p, "u_state").as_ref(), 0);
        let (alive, dead) = if dark {
            ([0.376, 0.647, 0.980], [0.067, 0.094, 0.153])
        } else {
            ([0.231, 0.510, 0.965], [1.0, 1.0, 1.0])
        };
        gl.uniform3f(gl.get_uniform_location(p, "u_alive").as_ref(), alive[0], alive[1], alive[2]);
        gl.uniform3f(gl.get_uniform_location(p, "u_dead").as_ref(), dead[0], dead[1], dead[2]);
        gl.uniform1f(gl.get_uniform_location(p, "u_zoom").as_ref(), zoom.max(0.01) as f32);
        gl.uniform2f(gl.get_uniform_location(p, "u_zoom_center").as_ref(), zx as f32, zy as f32);
        gl.bind_vertex_array(Some(&self.quad));
        gl.draw_arrays(Gl::TRIANGLES, 0, 6);
        gl.bind_vertex_array(None);
    }
}

fn make_texture(gl: &Gl) -> Result<WebGlTexture, String> {
    let tex = gl.create_texture().ok_or("createTexture failed")?;
    gl.bind_texture(Gl::TEXTURE_2D, Some(&tex));
    gl.tex_parameteri(Gl::TEXTURE_2D, Gl::TEXTURE_MIN_FILTER, Gl::NEAREST as i32);
    gl.tex_parameteri(Gl::TEXTURE_2D, Gl::TEXTURE_MAG_FILTER, Gl::NEAREST as i32);
    // REPEAT so cells at edges wrap around to the opposite side.
    gl.tex_parameteri(Gl::TEXTURE_2D, Gl::TEXTURE_WRAP_S, Gl::REPEAT as i32);
    gl.tex_parameteri(Gl::TEXTURE_2D, Gl::TEXTURE_WRAP_T, Gl::REPEAT as i32);
    gl.tex_image_2d_with_i32_and_i32_and_i32_and_format_and_type_and_opt_u8_array(
        Gl::TEXTURE_2D, 0, Gl::R8 as i32, GRID, GRID, 0, Gl::RED, Gl::UNSIGNED_BYTE, None,
    )
    .map_err(|_| "texImage2D failed")?;
    gl.bind_texture(Gl::TEXTURE_2D, None);
    Ok(tex)
}

fn make_framebuffer(gl: &Gl, tex: &WebGlTexture) -> Result<WebGlFramebuffer, String> {
    let fb = gl.create_framebuffer().ok_or("createFramebuffer failed")?;
    gl.bind_framebuffer(Gl::FRAMEBUFFER, Some(&fb));
    gl.framebuffer_texture_2d(Gl::FRAMEBUFFER, Gl::COLOR_ATTACHMENT0, Gl::TEXTURE_2D, Some(tex), 0);
    gl.bind_framebuffer(Gl::FRAMEBUFFER, None);
    Ok(fb)
}

struct State {
    renderer: Option<LifeGl>,
    /// Stepping (the play/pause button also sets this).
    running: bool,
    /// On screen at all — off-screen, the loop touches nothing.
    visible: bool,
    started: bool,
    probability: f64,
    interval_ms: f64,
    last_step: f64,
}

fn show(el: &HtmlElement, on: bool) {
    let _ = el.style().set_property("display", if on { "" } else { "none" });
}

/// Called by Foster with the `#life-widget` element.
#[wasm_bindgen]
pub fn mount(widget: Element) {
    let canvas: HtmlCanvasElement = by_id("life-canvas");
    let toggle: HtmlElement = by_id("life-toggle-run");
    let run_label: HtmlElement = toggle.query_selector(".life-run-label").unwrap().unwrap().unchecked_into();
    let pause_label: HtmlElement = toggle.query_selector(".life-pause-label").unwrap().unwrap().unchecked_into();
    let zoom_slider: HtmlInputElement = by_id("life-zoom-slider");
    let zoom_label: HtmlElement = by_id("life-zoom-label");
    let panel: HtmlElement = by_id("life-settings-panel");

    let st = Rc::new(RefCell::new(State {
        renderer: None, running: false, visible: false, started: false,
        probability: 0.35, interval_ms: 16.0, last_step: 0.0,
    }));
    let zoom = Rc::new(Cell::new(1.0));
    let center = Rc::new(Cell::new((0.5, 0.5)));

    let sync_toggle = {
        let (st, run_label, pause_label) = (st.clone(), run_label.clone(), pause_label.clone());
        Rc::new(move || {
            let running = st.borrow().running;
            show(&run_label, !running);
            show(&pause_label, running);
        })
    };

    {
        let (st, sync) = (st.clone(), sync_toggle.clone());
        on(&toggle, "click", move |_: web_sys::Event| {
            let r = !st.borrow().running;
            st.borrow_mut().running = r;
            sync();
        });
    }
    {
        let st = st.clone();
        on(&by_id::<Element>("life-reset"), "click", move |_: web_sys::Event| {
            let s = st.borrow();
            if let Some(r) = &s.renderer {
                r.randomize(s.probability);
            }
        });
    }
    {
        let (zoom, label) = (zoom.clone(), zoom_label.clone());
        on(&zoom_slider, "input", move |e: web_sys::Event| {
            let v: f64 = e.target().unwrap().unchecked_into::<HtmlInputElement>().value().parse().unwrap_or(1.0);
            zoom.set(v);
            label.set_text_content(Some(&format!("{v:.1}x")));
        });
    }
    {
        let panel2 = panel.clone();
        on(&by_id::<Element>("life-settings-toggle"), "click", move |_: web_sys::Event| {
            let hidden = panel2.style().get_property_value("display").unwrap_or_default() == "none";
            show(&panel2, hidden);
        });
        on(&by_id::<Element>("life-settings-close"), "click", move |_: web_sys::Event| show(&panel, false));
    }
    {
        let (st, label) = (st.clone(), by_id::<HtmlElement>("life-prob-label"));
        on(&by_id::<Element>("life-prob-slider"), "input", move |e: web_sys::Event| {
            let v: f64 = e.target().unwrap().unchecked_into::<HtmlInputElement>().value().parse().unwrap_or(0.35);
            st.borrow_mut().probability = v;
            label.set_text_content(Some(&format!("{}%", (v * 100.0).round())));
        });
    }
    {
        let st = st.clone();
        on(&by_id::<Element>("life-speed-select"), "change", move |e: web_sys::Event| {
            let v: f64 = e.target().unwrap().unchecked_into::<HtmlSelectElement>().value().parse().unwrap_or(16.0);
            st.borrow_mut().interval_ms = v;
        });
    }

    attach_canvas_nav(
        &canvas,
        NavHandles {
            zoom: zoom.clone(),
            center: center.clone(),
            on_zoom: Rc::new(move |z| {
                zoom_slider.set_value(&z.to_string());
                zoom_label.set_text_content(Some(&format!("{z:.1}x")));
            }),
            on_change: Rc::new(|| {}),
        },
        true,
    );

    // Auto-play while on screen; build the GL state the first time.
    {
        let (st, canvas, sync) = (st.clone(), canvas.clone(), sync_toggle.clone());
        on_visibility(&widget, move |visible| {
            {
                let mut s = st.borrow_mut();
                s.visible = visible;
                s.running = visible;
                if visible && !s.started {
                    s.started = true;
                    canvas.set_width(canvas.client_width().max(1) as u32);
                    canvas.set_height(canvas.client_height().max(1) as u32);
                    match LifeGl::new(&canvas) {
                        Ok(r) => {
                            r.randomize(s.probability);
                            s.renderer = Some(r);
                        }
                        Err(e) => web_sys::console::error_1(&format!("Life WebGL init: {e}").into()),
                    }
                }
            }
            sync();
        });
    }

    animate(move |ts| {
        let mut s = st.borrow_mut();
        if !s.visible || s.renderer.is_none() {
            return 250; // off screen: poll cheaply until visible again
        }
        let (cw, ch) = (canvas.client_width().max(1), canvas.client_height().max(1));
        if canvas.width() != cw as u32 || canvas.height() != ch as u32 {
            canvas.set_width(cw as u32);
            canvas.set_height(ch as u32);
        }
        if s.running && ts - s.last_step >= s.interval_ms {
            s.renderer.as_mut().unwrap().step();
            s.last_step = ts;
        }
        s.renderer.as_ref().unwrap().draw(cw, ch, dark_mode(), zoom.get(), center.get());
        0
    });
}
