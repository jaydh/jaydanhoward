#![allow(clippy::all)]
use crate::components::icons::Icon;
use leptos::prelude::*;

// ── Shaders ───────────────────────────────────────────────────────────────────

#[cfg(not(feature = "ssr"))]
const VERT: &str = r#"#version 300 es
in vec2 a_pos;
out vec2 v_uv;
void main() {
    v_uv = a_pos * 0.5 + 0.5;
    gl_Position = vec4(a_pos, 0.0, 1.0);
}"#;

// Ping-pong step: reads current state texture, applies Conway's rules.
// Wraps at grid edges (REPEAT addressing handles this).
#[cfg(not(feature = "ssr"))]
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

// Display: maps alive/dead to colors. u_zoom > 1 zooms in centered on u_zoom_center.
#[cfg(not(feature = "ssr"))]
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

// ── WebGL renderer ────────────────────────────────────────────────────────────

#[cfg(not(feature = "ssr"))]
pub struct LifeGl {
    gl: web_sys::WebGl2RenderingContext,
    step_prog: web_sys::WebGlProgram,
    draw_prog: web_sys::WebGlProgram,
    textures: [web_sys::WebGlTexture; 2],
    fbs: [web_sys::WebGlFramebuffer; 2],
    quad_vao: web_sys::WebGlVertexArrayObject,
    current: usize,
    pub grid_w: u32,
    pub grid_h: u32,
}

#[cfg(not(feature = "ssr"))]
impl LifeGl {
    pub fn new(
        canvas: &web_sys::HtmlCanvasElement,
        grid_w: u32,
        grid_h: u32,
    ) -> Result<Self, String> {
        use wasm_bindgen::JsCast;
        use web_sys::WebGl2RenderingContext as GL;

        let gl = canvas
            .get_context("webgl2")
            .map_err(|_| "get_context failed")?
            .ok_or("no webgl2")?
            .dyn_into::<GL>()
            .map_err(|_| "cast failed")?;

        let step_prog = Self::compile_program(&gl, VERT, STEP_FRAG)?;
        let draw_prog = Self::compile_program(&gl, VERT, DRAW_FRAG)?;

        // Full-screen triangle pair as a VAO.
        // Use the step program's attribute location; both programs share VERT.
        let quad_vao = gl.create_vertex_array().ok_or("vao")?;
        gl.bind_vertex_array(Some(&quad_vao));
        let quad_buf = gl.create_buffer().ok_or("quad buf")?;
        gl.bind_buffer(GL::ARRAY_BUFFER, Some(&quad_buf));
        let verts: [f32; 12] = [-1., -1., 1., -1., -1., 1., -1., 1., 1., -1., 1., 1.];
        unsafe {
            gl.buffer_data_with_array_buffer_view(
                GL::ARRAY_BUFFER,
                &js_sys::Float32Array::view(&verts),
                GL::STATIC_DRAW,
            );
        }
        let loc = gl.get_attrib_location(&step_prog, "a_pos") as u32;
        gl.enable_vertex_attrib_array(loc);
        gl.vertex_attrib_pointer_with_i32(loc, 2, GL::FLOAT, false, 0, 0);
        gl.bind_vertex_array(None);

        let t0 = Self::make_texture(&gl, grid_w, grid_h)?;
        let t1 = Self::make_texture(&gl, grid_w, grid_h)?;
        let fb0 = Self::make_framebuffer(&gl, &t0)?;
        let fb1 = Self::make_framebuffer(&gl, &t1)?;

        Ok(Self {
            gl,
            step_prog,
            draw_prog,
            textures: [t0, t1],
            fbs: [fb0, fb1],
            quad_vao,
            current: 0,
            grid_w,
            grid_h,
        })
    }

    pub fn randomize(&self, probability: f32) {
        use rand::Rng;
        use web_sys::WebGl2RenderingContext as GL;
        let mut rng = rand::rng();
        let data: Vec<u8> = (0..(self.grid_w * self.grid_h))
            .map(|_| if rng.random::<f32>() < probability { 255 } else { 0 })
            .collect();
        self.upload_state(&data);
        // Clear the back buffer to all dead so the first step doesn't read garbage.
        self.gl.bind_texture(GL::TEXTURE_2D, Some(&self.textures[1 - self.current]));
        let dead = vec![0u8; (self.grid_w * self.grid_h) as usize];
        let _ = self.gl.tex_image_2d_with_i32_and_i32_and_i32_and_format_and_type_and_opt_u8_array(
            GL::TEXTURE_2D, 0, GL::R8 as i32,
            self.grid_w as i32, self.grid_h as i32, 0,
            GL::RED, GL::UNSIGNED_BYTE, Some(&dead),
        );
        self.gl.bind_texture(GL::TEXTURE_2D, None);
    }

    pub fn step(&mut self) {
        use web_sys::WebGl2RenderingContext as GL;
        let next = 1 - self.current;
        let gl = &self.gl;

        gl.bind_framebuffer(GL::FRAMEBUFFER, Some(&self.fbs[next]));
        gl.viewport(0, 0, self.grid_w as i32, self.grid_h as i32);
        gl.use_program(Some(&self.step_prog));
        gl.active_texture(GL::TEXTURE0);
        gl.bind_texture(GL::TEXTURE_2D, Some(&self.textures[self.current]));
        if let Some(loc) = gl.get_uniform_location(&self.step_prog, "u_state") {
            gl.uniform1i(Some(&loc), 0);
        }
        if let Some(loc) = gl.get_uniform_location(&self.step_prog, "u_res") {
            gl.uniform2f(Some(&loc), self.grid_w as f32, self.grid_h as f32);
        }
        gl.bind_vertex_array(Some(&self.quad_vao));
        gl.draw_arrays(GL::TRIANGLES, 0, 6);
        gl.bind_vertex_array(None);
        gl.bind_framebuffer(GL::FRAMEBUFFER, None);

        self.current = next;
    }

    pub fn draw(&self, canvas_w: i32, canvas_h: i32, dark_mode: bool, zoom: f32, zoom_cx: f32, zoom_cy: f32) {
        use web_sys::WebGl2RenderingContext as GL;
        let gl = &self.gl;
        gl.viewport(0, 0, canvas_w, canvas_h);
        gl.use_program(Some(&self.draw_prog));
        gl.active_texture(GL::TEXTURE0);
        gl.bind_texture(GL::TEXTURE_2D, Some(&self.textures[self.current]));
        if let Some(loc) = gl.get_uniform_location(&self.draw_prog, "u_state") {
            gl.uniform1i(Some(&loc), 0);
        }
        // blue-400 / gray-900 for dark; blue-500 / white for light
        let (alive, dead) = if dark_mode {
            ([0.376_f32, 0.647, 0.980], [0.067_f32, 0.094, 0.153])
        } else {
            ([0.231_f32, 0.510, 0.965], [1.0_f32, 1.0, 1.0])
        };
        if let Some(loc) = gl.get_uniform_location(&self.draw_prog, "u_alive") {
            gl.uniform3f(Some(&loc), alive[0], alive[1], alive[2]);
        }
        if let Some(loc) = gl.get_uniform_location(&self.draw_prog, "u_dead") {
            gl.uniform3f(Some(&loc), dead[0], dead[1], dead[2]);
        }
        if let Some(loc) = gl.get_uniform_location(&self.draw_prog, "u_zoom") {
            gl.uniform1f(Some(&loc), zoom.max(0.01));
        }
        if let Some(loc) = gl.get_uniform_location(&self.draw_prog, "u_zoom_center") {
            gl.uniform2f(Some(&loc), zoom_cx, zoom_cy);
        }
        gl.bind_vertex_array(Some(&self.quad_vao));
        gl.draw_arrays(GL::TRIANGLES, 0, 6);
        gl.bind_vertex_array(None);
    }

    /// Paint a circle of alive cells at the given canvas coordinates.
    pub fn paint(&self, canvas_x: f32, canvas_y: f32, canvas_w: f32, canvas_h: f32, radius: i32) {
        use web_sys::WebGl2RenderingContext as GL;
        let cx = (canvas_x / canvas_w * self.grid_w as f32) as i32;
        let cy = ((1.0 - canvas_y / canvas_h) * self.grid_h as f32) as i32; // flip Y
        let dia = (2 * radius + 1) as u32;
        let mut patch = vec![255u8; (dia * dia) as usize];
        // mask to circle
        for dy in -(radius)..=radius {
            for dx in -(radius)..=radius {
                if dx * dx + dy * dy > radius * radius {
                    patch[((dy + radius) as u32 * dia + (dx + radius) as u32) as usize] = 0;
                }
            }
        }
        let x0 = (cx - radius).clamp(0, self.grid_w as i32 - dia as i32);
        let y0 = (cy - radius).clamp(0, self.grid_h as i32 - dia as i32);
        self.gl.bind_texture(GL::TEXTURE_2D, Some(&self.textures[self.current]));
        let _ = self.gl.tex_sub_image_2d_with_i32_and_i32_and_u32_and_type_and_opt_u8_array(
            GL::TEXTURE_2D, 0,
            x0, y0, dia as i32, dia as i32,
            GL::RED, GL::UNSIGNED_BYTE, Some(&patch),
        );
        self.gl.bind_texture(GL::TEXTURE_2D, None);
    }

    fn upload_state(&self, data: &[u8]) {
        use web_sys::WebGl2RenderingContext as GL;
        self.gl.bind_texture(GL::TEXTURE_2D, Some(&self.textures[self.current]));
        let _ = self.gl.tex_image_2d_with_i32_and_i32_and_i32_and_format_and_type_and_opt_u8_array(
            GL::TEXTURE_2D, 0, GL::R8 as i32,
            self.grid_w as i32, self.grid_h as i32, 0,
            GL::RED, GL::UNSIGNED_BYTE, Some(data),
        );
        self.gl.bind_texture(GL::TEXTURE_2D, None);
    }

    fn compile_program(
        gl: &web_sys::WebGl2RenderingContext,
        vert: &str,
        frag: &str,
    ) -> Result<web_sys::WebGlProgram, String> {
        use web_sys::WebGl2RenderingContext as GL;
        let vs = Self::compile_shader(gl, GL::VERTEX_SHADER, vert)?;
        let fs = Self::compile_shader(gl, GL::FRAGMENT_SHADER, frag)?;
        let prog = gl.create_program().ok_or("program")?;
        gl.attach_shader(&prog, &vs);
        gl.attach_shader(&prog, &fs);
        gl.link_program(&prog);
        if !gl.get_program_parameter(&prog, GL::LINK_STATUS).as_bool().unwrap_or(false) {
            return Err(gl.get_program_info_log(&prog).unwrap_or_else(|| "link error".into()));
        }
        Ok(prog)
    }

    fn compile_shader(
        gl: &web_sys::WebGl2RenderingContext,
        shader_type: u32,
        src: &str,
    ) -> Result<web_sys::WebGlShader, String> {
        use web_sys::WebGl2RenderingContext as GL;
        let shader = gl.create_shader(shader_type).ok_or("shader")?;
        gl.shader_source(&shader, src);
        gl.compile_shader(&shader);
        if !gl.get_shader_parameter(&shader, GL::COMPILE_STATUS).as_bool().unwrap_or(false) {
            return Err(gl.get_shader_info_log(&shader).unwrap_or_else(|| "compile error".into()));
        }
        Ok(shader)
    }

    fn make_texture(
        gl: &web_sys::WebGl2RenderingContext,
        w: u32,
        h: u32,
    ) -> Result<web_sys::WebGlTexture, String> {
        use web_sys::WebGl2RenderingContext as GL;
        let tex = gl.create_texture().ok_or("texture")?;
        gl.bind_texture(GL::TEXTURE_2D, Some(&tex));
        gl.tex_parameteri(GL::TEXTURE_2D, GL::TEXTURE_MIN_FILTER, GL::NEAREST as i32);
        gl.tex_parameteri(GL::TEXTURE_2D, GL::TEXTURE_MAG_FILTER, GL::NEAREST as i32);
        // REPEAT so cells at edges wrap around to opposite side
        gl.tex_parameteri(GL::TEXTURE_2D, GL::TEXTURE_WRAP_S, GL::REPEAT as i32);
        gl.tex_parameteri(GL::TEXTURE_2D, GL::TEXTURE_WRAP_T, GL::REPEAT as i32);
        gl.tex_image_2d_with_i32_and_i32_and_i32_and_format_and_type_and_opt_u8_array(
            GL::TEXTURE_2D, 0, GL::R8 as i32,
            w as i32, h as i32, 0,
            GL::RED, GL::UNSIGNED_BYTE, None,
        ).map_err(|e| format!("{e:?}"))?;
        gl.bind_texture(GL::TEXTURE_2D, None);
        Ok(tex)
    }

    fn make_framebuffer(
        gl: &web_sys::WebGl2RenderingContext,
        tex: &web_sys::WebGlTexture,
    ) -> Result<web_sys::WebGlFramebuffer, String> {
        use web_sys::WebGl2RenderingContext as GL;
        let fb = gl.create_framebuffer().ok_or("framebuffer")?;
        gl.bind_framebuffer(GL::FRAMEBUFFER, Some(&fb));
        gl.framebuffer_texture_2d(
            GL::FRAMEBUFFER, GL::COLOR_ATTACHMENT0, GL::TEXTURE_2D, Some(tex), 0,
        );
        gl.bind_framebuffer(GL::FRAMEBUFFER, None);
        Ok(fb)
    }
}

// ── Component ─────────────────────────────────────────────────────────────────

#[component]
pub fn LifeGame(
    #[prop(optional)] initial_alive_probability: Option<f64>,
    #[prop(optional)] initial_interval_ms: Option<u64>,
    #[prop(default = false)]
    #[allow(unused_variables)]
    auto_start: bool,
) -> impl IntoView {
    #[cfg(not(feature = "ssr"))]
    use std::cell::{Cell, RefCell};
    #[cfg(not(feature = "ssr"))]
    use std::rc::Rc;

    #[cfg(not(feature = "ssr"))]
    const GRID_SIZE: u32 = 2048;

    let (alive_probability, set_alive_probability) =
        signal(initial_alive_probability.unwrap_or(0.35));
    let (interval_ms, set_interval_ms) =
        signal(initial_interval_ms.unwrap_or(16));
    let (show_settings, set_show_settings) = signal(false);
    let (zoom, set_zoom) = signal(1.0_f32);
    let (is_navigate, set_is_navigate) = signal(false);
    #[cfg(feature = "ssr")]
    let (_zoom_center, _set_zoom_center) = signal((0.5_f32, 0.5_f32));
    #[cfg(not(feature = "ssr"))]
    let (zoom_center, set_zoom_center) = signal((0.5_f32, 0.5_f32));

    let canvas_ref = NodeRef::<leptos::html::Canvas>::new();
    let container_ref = NodeRef::<leptos::html::Div>::new();

    let (is_running, set_is_running) = signal(false);

    // ── WebGL init + RAF loop ─────────────────────────────────────────────────
    #[cfg(not(feature = "ssr"))]
    {
        use wasm_bindgen::prelude::*;
        use wasm_bindgen::JsCast;

        let renderer: Rc<RefCell<Option<LifeGl>>> = Rc::new(RefCell::new(None));
        let running_flag: Rc<Cell<bool>> = Rc::new(Cell::new(false));
        let loop_active: Rc<Cell<bool>> = Rc::new(Cell::new(false));

        // Keep running_flag in sync with is_running signal
        {
            let running_flag = running_flag.clone();
            Effect::new(move |_| {
                running_flag.set(is_running());
            });
        }

        // Init renderer once canvas mounts
        {
            let renderer = renderer.clone();
            let running_flag = running_flag.clone();
            let loop_active = loop_active.clone();

            Effect::new(move |_| {
                let Some(canvas) = canvas_ref.get() else { return; };
                if renderer.borrow().is_some() { return; }

                let canvas_el: &web_sys::HtmlCanvasElement = canvas.as_ref();
                match LifeGl::new(canvas_el, GRID_SIZE, GRID_SIZE) {
                    Ok(gl) => {
                        let prob = alive_probability.get_untracked() as f32;
                        gl.randomize(prob);
                        *renderer.borrow_mut() = Some(gl);

                        // Start the RAF loop once
                        if !loop_active.get() {
                            loop_active.set(true);
                            start_raf_loop(
                                renderer.clone(),
                                running_flag.clone(),
                                interval_ms,
                                zoom,
                                zoom_center,
                            );
                        }
                    }
                    Err(e) => web_sys::console::error_1(&format!("WebGL init: {e}").into()),
                }
            });
        }

        // ── Mouse painting / drag navigation ──────────────────────────────────
        {
            let renderer = renderer.clone();
            let painting: Rc<Cell<bool>> = Rc::new(Cell::new(false));
            let dragging: Rc<Cell<bool>> = Rc::new(Cell::new(false));
            let drag_last: Rc<Cell<(f32, f32)>> = Rc::new(Cell::new((0.0, 0.0)));
            let drag_w: Rc<Cell<f32>> = Rc::new(Cell::new(1.0));
            let drag_h: Rc<Cell<f32>> = Rc::new(Cell::new(1.0));

            let painting_down = painting.clone();
            let dragging_down = dragging.clone();
            let drag_last_down = drag_last.clone();
            let drag_w_down = drag_w.clone();
            let drag_h_down = drag_h.clone();
            let renderer_down = renderer.clone();
            let on_pointerdown = move |e: web_sys::PointerEvent| {
                if is_navigate.get_untracked() {
                    let canvas = canvas_ref.get().unwrap();
                    let el: &web_sys::HtmlCanvasElement = canvas.as_ref();
                    let rect = el.get_bounding_client_rect();
                    drag_w_down.set(rect.width() as f32);
                    drag_h_down.set(rect.height() as f32);
                    dragging_down.set(true);
                    drag_last_down.set((e.client_x() as f32, e.client_y() as f32));
                } else {
                    painting_down.set(true);
                    if let Some(ref gl) = *renderer_down.borrow() {
                        let canvas = canvas_ref.get().unwrap();
                        let el: &web_sys::HtmlCanvasElement = canvas.as_ref();
                        let rect = el.get_bounding_client_rect();
                        let x = e.client_x() as f32 - rect.left() as f32;
                        let y = e.client_y() as f32 - rect.top() as f32;
                        gl.paint(x, y, rect.width() as f32, rect.height() as f32, 3);
                    }
                }
            };

            let painting_move = painting.clone();
            let dragging_move = dragging.clone();
            let drag_last_move = drag_last.clone();
            let drag_w_move = drag_w.clone();
            let drag_h_move = drag_h.clone();
            let renderer_move = renderer.clone();
            let on_pointermove = move |e: web_sys::PointerEvent| {
                if is_navigate.get_untracked() {
                    if !dragging_move.get() { return; }
                    let (lx, ly) = drag_last_move.get();
                    let cx = e.client_x() as f32;
                    let cy = e.client_y() as f32;
                    let dx = (cx - lx) / drag_w_move.get();
                    let dy = (cy - ly) / drag_h_move.get();
                    drag_last_move.set((cx, cy));
                    let z = zoom.get_untracked();
                    set_zoom_center.update(|(ocx, ocy)| {
                        *ocx -= dx / z;
                        *ocy -= dy / z;
                    });
                } else {
                    if !painting_move.get() { return; }
                    if let Some(ref gl) = *renderer_move.borrow() {
                        let canvas = canvas_ref.get().unwrap();
                        let el: &web_sys::HtmlCanvasElement = canvas.as_ref();
                        let rect = el.get_bounding_client_rect();
                        let x = e.client_x() as f32 - rect.left() as f32;
                        let y = e.client_y() as f32 - rect.top() as f32;
                        gl.paint(x, y, rect.width() as f32, rect.height() as f32, 3);
                    }
                }
            };

            let on_pointerup = move |_: web_sys::PointerEvent| {
                painting.set(false);
                dragging.set(false);
            };

            Effect::new(move |_| {
                let Some(canvas) = canvas_ref.get() else { return; };
                let el: &web_sys::HtmlCanvasElement = canvas.as_ref();

                // Wheel: zoom toward cursor (works in both modes)
                {
                    let el2 = el.clone();
                    let cb = Closure::wrap(Box::new(move |e: web_sys::WheelEvent| {
                        e.prevent_default();
                        let rect = el2.get_bounding_client_rect();
                        let sx = (e.client_x() as f32 - rect.left() as f32) / rect.width() as f32;
                        let sy = (e.client_y() as f32 - rect.top() as f32) / rect.height() as f32;
                        let old_z = zoom.get_untracked();
                        let factor = if e.delta_y() > 0.0 { 1.0 / 1.15 } else { 1.15 };
                        let new_z = (old_z * factor).clamp(1.0, 16.0);
                        let (cx, cy) = zoom_center.get_untracked();
                        let wx = (sx - cx) / old_z + cx;
                        let wy = (sy - cy) / old_z + cy;
                        let new_cx = if (new_z - 1.0).abs() > 1e-4 { (new_z * wx - sx) / (new_z - 1.0) } else { 0.5 };
                        let new_cy = if (new_z - 1.0).abs() > 1e-4 { (new_z * wy - sy) / (new_z - 1.0) } else { 0.5 };
                        set_zoom(new_z);
                        set_zoom_center((new_cx, new_cy));
                    }) as Box<dyn FnMut(_)>);
                    el.add_event_listener_with_callback("wheel", cb.as_ref().unchecked_ref()).ok();
                    cb.forget();
                }

                let cb_down = Closure::wrap(Box::new(on_pointerdown.clone()) as Box<dyn FnMut(_)>);
                let cb_move = Closure::wrap(Box::new(on_pointermove.clone()) as Box<dyn FnMut(_)>);
                let cb_up = Closure::wrap(Box::new(on_pointerup.clone()) as Box<dyn FnMut(_)>);
                el.add_event_listener_with_callback("pointerdown", cb_down.as_ref().unchecked_ref()).ok();
                el.add_event_listener_with_callback("pointermove", cb_move.as_ref().unchecked_ref()).ok();
                el.add_event_listener_with_callback("pointerup", cb_up.as_ref().unchecked_ref()).ok();
                cb_down.forget();
                cb_move.forget();
                cb_up.forget();
            });
        }

        // ── IntersectionObserver: pause when off-screen ────────────────────────
        {
            let renderer = renderer.clone();
            let has_started = Rc::new(Cell::new(false));

            // Build the callback outside the Effect so we can leak it without
            // making the Effect closure FnOnce (Closure::forget takes ownership).
            let callback = Rc::new(Closure::wrap(Box::new(
                move |entries: js_sys::Array, _: web_sys::IntersectionObserver| {
                    for entry in entries.iter() {
                        let Ok(entry) = entry.dyn_into::<web_sys::IntersectionObserverEntry>()
                        else { continue; };
                        if entry.is_intersecting() {
                            if !has_started.get() {
                                has_started.set(true);
                                if let Some(ref gl) = *renderer.borrow() {
                                    let prob = alive_probability.get_untracked() as f32;
                                    gl.randomize(prob);
                                }
                            }
                            set_is_running(true);
                        } else {
                            set_is_running(false);
                        }
                    }
                },
            ) as Box<dyn FnMut(js_sys::Array, web_sys::IntersectionObserver)>));

            let callback_ref = callback.clone();
            Effect::new(move |_| {
                let Some(container) = container_ref.get() else { return; };
                if let Ok(observer) =
                    web_sys::IntersectionObserver::new(callback_ref.as_ref().as_ref().unchecked_ref())
                {
                    observer.observe(&container);
                }
            });

            // Leak the Rc so the closure stays alive for the page lifetime.
            std::mem::forget(callback);
        }
    }

    // ── Controls ──────────────────────────────────────────────────────────────
    #[cfg(not(feature = "ssr"))]
    let reset = {
        let canvas_ref2 = canvas_ref;
        move || {
            let Some(_canvas) = canvas_ref2.get() else { return; };
            // Signal change will cause re-randomize on next visible frame via observer
            set_is_running(false);
            set_is_running(true);
        }
    };

    #[cfg(feature = "ssr")]
    let reset = || {};

    view! {
        <div node_ref=container_ref class="w-full flex flex-col gap-4 items-center relative">
            <div class="flex gap-3 items-center flex-wrap justify-center">
                <div class="flex gap-2">
                    <button
                        class="px-4 py-1.5 text-sm rounded border transition-all duration-200 hover:bg-accent hover:bg-opacity-10"
                        style:border-color="#3B82F6"
                        style:color="#3B82F6"
                        on:click=move |_| set_is_running(!is_running())
                        aria-label=move || if is_running() { "Pause" } else { "Play" }
                    >
                        {move || if is_running() { "▌▌" } else { "▶" }}
                    </button>
                    <button
                        class="px-4 py-1.5 text-sm rounded border transition-all duration-200 hover:bg-accent hover:bg-opacity-10"
                        style:border-color="#3B82F6"
                        style:color="#3B82F6"
                        on:click=move |_| reset()
                        aria-label="Reset"
                    >"↻"</button>
                    <button
                        class=move || format!(
                            "px-4 py-1.5 text-sm rounded border transition-all duration-200 {}",
                            if is_navigate() {
                                "bg-blue-500 text-white border-blue-500"
                            } else {
                                "border-border text-charcoal hover:bg-border hover:bg-opacity-20"
                            }
                        )
                        on:click=move |_| set_is_navigate.update(|v| *v = !*v)
                        aria-label=move || if is_navigate() { "Switch to Draw mode" } else { "Switch to Navigate mode" }
                        title=move || if is_navigate() { "Navigate (drag to pan, scroll to zoom) — click to draw" } else { "Draw — click to navigate" }
                    >
                        {move || if is_navigate() { "Navigate" } else { "Draw" }}
                    </button>
                    <button
                        class="px-4 py-1.5 text-sm rounded border border-border text-charcoal hover:bg-border hover:bg-opacity-20 transition-all duration-200 flex items-center justify-center"
                        on:click=move |_| set_show_settings.update(|v| *v = !*v)
                        aria-label="Settings"
                    >
                        <Icon name="cog" class="w-4 h-4" />
                    </button>
                </div>
                <span class="text-sm text-charcoal-light">"2048×2048"</span>
                <div class="flex items-center gap-2">
                    <label class="text-xs text-charcoal-light font-mono">"zoom"</label>
                    <input
                        type="range" min="1" max="16" step="0.5"
                        prop:value=move || zoom().to_string()
                        on:input=move |e| {
                            if let Ok(v) = event_target_value(&e).parse::<f32>() {
                                set_zoom(v);
                            }
                        }
                        class="w-24 accent-blue-500"
                    />
                    <span class="text-xs text-charcoal-light font-mono w-8">
                        {move || format!("{:.1}x", zoom())}
                    </span>
                </div>
            </div>

            <Show when=move || show_settings()>
                <div class="absolute top-12 right-0 z-20 bg-surface border border-border rounded-lg shadow-minimal-lg p-6 min-w-[300px]">
                    <div class="flex flex-col gap-4">
                        <div class="flex items-center justify-between mb-2">
                            <h3 class="text-lg font-medium text-charcoal">"Settings"</h3>
                            <button
                                class="text-charcoal-lighter hover:text-charcoal transition-colors"
                                on:click=move |_| set_show_settings.set(false)
                                aria-label="Close"
                            >"✕"</button>
                        </div>

                        <div class="flex flex-col gap-2">
                            <label class="text-sm font-medium text-charcoal">
                                "Alive Probability"
                            </label>
                            <input
                                type="range"
                                min="0.1" max="0.9" step="0.05"
                                class="w-full accent-blue-500"
                                on:input=move |ev| {
                                    if let Ok(v) = event_target_value(&ev).parse::<f64>() {
                                        set_alive_probability(v);
                                    }
                                }
                                prop:value=alive_probability
                            />
                            <span class="text-xs text-charcoal-light text-right">
                                {move || format!("{:.0}%", alive_probability() * 100.0)}
                            </span>
                        </div>

                        <div class="flex flex-col gap-2">
                            <label class="text-sm font-medium text-charcoal">"Speed"</label>
                            <select
                                class="px-3 py-2 rounded border border-border bg-surface text-charcoal focus:outline-none focus:ring-2 focus:ring-accent"
                                on:change=move |ev| {
                                    if let Ok(v) = event_target_value(&ev).parse::<u64>() {
                                        set_interval_ms(v);
                                    }
                                }
                            >
                                {[(16u64,"~60 fps"),(33,"~30 fps"),(66,"~15 fps"),(125,"~8 fps"),(250,"~4 fps")].into_iter().map(|(ms, label)| {
                                    view! {
                                        <option value=ms.to_string() selected=move || interval_ms() == ms>
                                            {label}
                                        </option>
                                    }
                                }).collect_view()}
                            </select>
                        </div>

                        <div class="flex items-center gap-4 pt-2 text-sm text-charcoal-light">
                            <a
                                class="text-accent hover:underline"
                                href="https://en.wikipedia.org/wiki/Conway%27s_Game_of_Life"
                                target="_blank"
                                rel="noreferrer"
                            >"Learn More"</a>
                        </div>
                    </div>
                </div>
            </Show>

            <canvas
                node_ref=canvas_ref
                class="w-full aspect-square border border-border touch-none"
                class:cursor-crosshair=move || !is_navigate()
                class:cursor-grab=move || is_navigate()
                style="max-height: 60vh; object-fit: contain;"
                on:click=move |e| {
                    #[cfg(feature = "ssr")]
                    let _ = &e;
                    #[cfg(not(feature = "ssr"))]
                    {
                        use wasm_bindgen::JsCast as _;
                        // Shift-click re-centers the zoom on the clicked point
                        if e.shift_key() {
                            if let Some(canvas) = e.target()
                                .and_then(|t| t.dyn_into::<web_sys::HtmlCanvasElement>().ok())
                            {
                                let rect = canvas.get_bounding_client_rect();
                                let cx = (e.client_x() as f64 - rect.left()) / rect.width();
                                let cy = (e.client_y() as f64 - rect.top()) / rect.height();
                                let (old_cx, old_cy) = zoom_center.get_untracked();
                                let z = zoom.get_untracked();
                                let wx = (cx as f32 - old_cx) / z + old_cx;
                                let wy = (cy as f32 - old_cy) / z + old_cy;
                                set_zoom_center((wx, wy));
                            }
                        }
                    }
                }
            ></canvas>
        </div>
    }
}

// ── RAF loop ──────────────────────────────────────────────────────────────────

#[cfg(not(feature = "ssr"))]
fn start_raf_loop(
    renderer: std::rc::Rc<std::cell::RefCell<Option<LifeGl>>>,
    running: std::rc::Rc<std::cell::Cell<bool>>,
    interval_ms: ReadSignal<u64>,
    zoom: ReadSignal<f32>,
    zoom_center: ReadSignal<(f32, f32)>,
) {
    use std::cell::{Cell, RefCell};
    use std::rc::Rc;
    use wasm_bindgen::prelude::*;
    use wasm_bindgen::JsCast;

    let last_step: Rc<Cell<f64>> = Rc::new(Cell::new(0.0));

    // Recursive closure: holds a reference to itself via Rc<RefCell<Option<…>>>.
    let f: Rc<RefCell<Option<Closure<dyn FnMut()>>>> = Rc::new(RefCell::new(None));
    let f_outer = f.clone();

    *f.borrow_mut() = Some(Closure::wrap(Box::new(move || {
        let window = web_sys::window().unwrap();

        // Skip frame entirely if renderer not initialized yet.
        if renderer.borrow().is_none() {
            window
                .request_animation_frame(
                    f_outer.borrow().as_ref().unwrap().as_ref().unchecked_ref(),
                )
                .unwrap();
            return;
        }

        // Sync canvas size and collect display params outside any renderer borrow.
        let canvas_params = window.document()
            .and_then(|d| d.query_selector("canvas.w-full.aspect-square").ok().flatten())
            .and_then(|el| el.dyn_into::<web_sys::HtmlCanvasElement>().ok())
            .map(|canvas| {
                let w = canvas.client_width() as u32;
                let h = canvas.client_height() as u32;
                if w > 0 && h > 0 && (canvas.width() != w || canvas.height() != h) {
                    canvas.set_width(w);
                    canvas.set_height(h);
                }
                let dark = window.document()
                    .and_then(|d| d.document_element())
                    .map(|el| el.class_list().contains("dark"))
                    .unwrap_or(false);
                (canvas.width() as i32, canvas.height() as i32, dark)
            });

        if let Some((cw, ch, dark)) = canvas_params {
            if running.get() {
                let now = js_sys::Date::now();
                let min_gap = interval_ms.get_untracked() as f64;
                if now - last_step.get() >= min_gap {
                    last_step.set(now);
                    renderer.borrow_mut().as_mut().unwrap().step();
                }
            }
            let z = zoom.get_untracked();
            let (cx, cy) = zoom_center.get_untracked();
            renderer.borrow().as_ref().unwrap().draw(cw, ch, dark, z, cx, cy);
        }

        window
            .request_animation_frame(
                f_outer.borrow().as_ref().unwrap().as_ref().unchecked_ref(),
            )
            .unwrap();
    }) as Box<dyn FnMut()>));

    web_sys::window()
        .unwrap()
        .request_animation_frame(f.borrow().as_ref().unwrap().as_ref().unchecked_ref())
        .unwrap();

    // Leak the Rc to keep the closure alive for the page lifetime.
    std::mem::forget(f);
}

// ── Entry point ───────────────────────────────────────────────────────────────

#[component]
pub fn Life() -> impl IntoView {
    view! {
        <LifeGame auto_start=true />
    }
}
