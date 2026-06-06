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

// BFS wavefront expansion with MRT.
// State (R8):  0=obstacle  64=unvisited  128=frontier  192=visited
// Parent (R8): 0=none  64=from_left  128=from_right  192=from_below  255=from_above
#[cfg(not(feature = "ssr"))]
const STEP_FRAG: &str = r#"#version 300 es
precision highp float;
in vec2 v_uv;
layout(location = 0) out vec4 out_state;
layout(location = 1) out vec4 out_parent;
uniform sampler2D u_state;
uniform sampler2D u_parent;
uniform vec2 u_res;
void main() {
    vec2 d = 1.0 / u_res;
    float s = floor(texture(u_state, v_uv).r * 255.0 + 0.5);
    float p = texture(u_parent, v_uv).r;

    // Obstacle (0) or already visited+ (>=192): pass through.
    if (s < 32.0 || s > 160.0) {
        out_state  = vec4(s / 255.0, 0.0, 0.0, 1.0);
        out_parent = vec4(p, 0.0, 0.0, 1.0);
        return;
    }
    // Frontier (128) -> visited (192).
    if (s > 96.0) {
        out_state  = vec4(192.0 / 255.0, 0.0, 0.0, 1.0);
        out_parent = vec4(p, 0.0, 0.0, 1.0);
        return;
    }
    // Unvisited (64): become frontier if a 4-neighbor is currently frontier.
    float sl = floor(texture(u_state, v_uv + vec2(-d.x,  0.0)).r * 255.0 + 0.5);
    float sr = floor(texture(u_state, v_uv + vec2( d.x,  0.0)).r * 255.0 + 0.5);
    float sb = floor(texture(u_state, v_uv + vec2( 0.0, -d.y)).r * 255.0 + 0.5);
    float st = floor(texture(u_state, v_uv + vec2( 0.0,  d.y)).r * 255.0 + 0.5);
    bool fl = sl > 96.0 && sl < 160.0;
    bool fr = sr > 96.0 && sr < 160.0;
    bool fb = sb > 96.0 && sb < 160.0;
    bool ft = st > 96.0 && st < 160.0;
    if (fl || fr || fb || ft) {
        // Encode which neighbor was frontier so we can trace back later.
        float np = fl ? (64.0/255.0) : (fr ? (128.0/255.0) : (fb ? (192.0/255.0) : 1.0));
        out_state  = vec4(128.0 / 255.0, 0.0, 0.0, 1.0);
        out_parent = vec4(np, 0.0, 0.0, 1.0);
    } else {
        out_state  = vec4(s / 255.0, 0.0, 0.0, 1.0);
        out_parent = vec4(p, 0.0, 0.0, 1.0);
    }
}
"#;

// Display shader: maps state values to colors, overlays start/end markers.
#[cfg(not(feature = "ssr"))]
const DRAW_FRAG: &str = r#"#version 300 es
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
void main() {
    float s = floor(texture(u_state, v_uv).r * 255.0 + 0.5);
    vec3 col;
    if      (s < 32.0)  { col = u_wall; }
    else if (s > 224.0) { col = vec3(0.753, 0.518, 0.988); } // purple-400: path
    else if (s > 160.0) { col = u_visited; }
    else if (s > 96.0)  { col = vec3(0.937, 0.267, 0.267); } // red-500: frontier
    else                { col = u_bg; }

    // Start (green) and end (amber) circles, radius 5px in grid space.
    vec2 ps = (v_uv - u_start) * u_res;
    if (dot(ps, ps) < 25.0) { col = vec3(0.133, 0.773, 0.369); }
    vec2 pe = (v_uv - u_end) * u_res;
    if (dot(pe, pe) < 25.0) { col = vec3(0.961, 0.620, 0.043); }
    o = vec4(col, 1.0);
}
"#;

// ── PathGl ────────────────────────────────────────────────────────────────────

#[cfg(not(feature = "ssr"))]
pub struct PathGl {
    gl: web_sys::WebGl2RenderingContext,
    step_prog: web_sys::WebGlProgram,
    draw_prog: web_sys::WebGlProgram,
    // fbs[i] writes to state_texs[i] (attach 0) + parent_texs[i] (attach 1).
    state_texs: [web_sys::WebGlTexture; 2],
    parent_texs: [web_sys::WebGlTexture; 2],
    fbs: [web_sys::WebGlFramebuffer; 2],
    quad_vao: web_sys::WebGlVertexArrayObject,
    current: usize,
    pub grid_w: u32,
    pub grid_h: u32,
    pub start: (u32, u32),
    pub end: (u32, u32),
}

#[cfg(not(feature = "ssr"))]
impl PathGl {
    pub fn new(canvas: &web_sys::HtmlCanvasElement, grid_w: u32, grid_h: u32) -> Result<Self, String> {
        use wasm_bindgen::JsCast;
        use web_sys::WebGl2RenderingContext as GL;

        let gl = canvas
            .get_context("webgl2").map_err(|_| "get_context")?
            .ok_or("no webgl2")?
            .dyn_into::<GL>().map_err(|_| "cast")?;

        let step_prog = Self::compile_prog(&gl, VERT, STEP_FRAG)?;
        let draw_prog = Self::compile_prog(&gl, VERT, DRAW_FRAG)?;

        let quad_vao = gl.create_vertex_array().ok_or("vao")?;
        gl.bind_vertex_array(Some(&quad_vao));
        let buf = gl.create_buffer().ok_or("buf")?;
        gl.bind_buffer(GL::ARRAY_BUFFER, Some(&buf));
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

        let st0 = Self::make_r8(&gl, grid_w, grid_h)?;
        let st1 = Self::make_r8(&gl, grid_w, grid_h)?;
        let pt0 = Self::make_r8(&gl, grid_w, grid_h)?;
        let pt1 = Self::make_r8(&gl, grid_w, grid_h)?;
        let fb0 = Self::make_mrt_fb(&gl, &st0, &pt0)?;
        let fb1 = Self::make_mrt_fb(&gl, &st1, &pt1)?;

        Ok(Self {
            gl, step_prog, draw_prog,
            state_texs: [st0, st1],
            parent_texs: [pt0, pt1],
            fbs: [fb0, fb1],
            quad_vao, current: 0,
            grid_w, grid_h,
            start: (0, 0),
            end: (grid_w - 1, grid_h - 1),
        })
    }

    pub fn reset(&mut self, obstacle_prob: f32) {
        use rand::Rng;
        use web_sys::WebGl2RenderingContext as GL;

        let mut rng = rand::rng();
        let n = (self.grid_w * self.grid_h) as usize;

        let mut state = vec![0u8; n];
        for v in state.iter_mut() {
            if rng.random::<f32>() >= obstacle_prob { *v = 64; }
        }

        let start = Self::find_passable_near(&state, self.grid_w, 0, 0);
        let end = Self::find_passable_near(&state, self.grid_w, self.grid_w - 1, self.grid_h - 1);
        self.start = start;
        self.end = end;

        state[(start.1 * self.grid_w + start.0) as usize] = 128; // mark start as frontier

        self.current = 0;
        let gl = &self.gl;

        gl.bind_texture(GL::TEXTURE_2D, Some(&self.state_texs[0]));
        let _ = gl.tex_image_2d_with_i32_and_i32_and_i32_and_format_and_type_and_opt_u8_array(
            GL::TEXTURE_2D, 0, GL::R8 as i32,
            self.grid_w as i32, self.grid_h as i32, 0,
            GL::RED, GL::UNSIGNED_BYTE, Some(&state),
        );
        gl.bind_texture(GL::TEXTURE_2D, None);

        let dead = vec![0u8; n];
        for tex in [&self.state_texs[1], &self.parent_texs[0], &self.parent_texs[1]] {
            gl.bind_texture(GL::TEXTURE_2D, Some(tex));
            let _ = gl.tex_image_2d_with_i32_and_i32_and_i32_and_format_and_type_and_opt_u8_array(
                GL::TEXTURE_2D, 0, GL::R8 as i32,
                self.grid_w as i32, self.grid_h as i32, 0,
                GL::RED, GL::UNSIGNED_BYTE, Some(&dead),
            );
        }
        gl.bind_texture(GL::TEXTURE_2D, None);
    }

    fn find_passable_near(state: &[u8], grid_w: u32, tx: u32, ty: u32) -> (u32, u32) {
        let w = grid_w as i64;
        let h = state.len() as i64 / w;
        for r in 0i64.. {
            for dy in -r..=r {
                for dx in -r..=r {
                    if dx.abs() != r && dy.abs() != r { continue; }
                    let nx = tx as i64 + dx;
                    let ny = ty as i64 + dy;
                    if nx < 0 || ny < 0 || nx >= w || ny >= h { continue; }
                    if state[(ny * w + nx) as usize] == 64 {
                        return (nx as u32, ny as u32);
                    }
                }
            }
            if r > 200 { return (tx.min(grid_w - 1), ty); }
        }
        (tx, ty)
    }

    pub fn step(&mut self) {
        use web_sys::WebGl2RenderingContext as GL;
        let next = 1 - self.current;
        let gl = &self.gl;

        gl.bind_framebuffer(GL::FRAMEBUFFER, Some(&self.fbs[next]));
        gl.viewport(0, 0, self.grid_w as i32, self.grid_h as i32);
        gl.use_program(Some(&self.step_prog));

        gl.active_texture(GL::TEXTURE0);
        gl.bind_texture(GL::TEXTURE_2D, Some(&self.state_texs[self.current]));
        if let Some(l) = gl.get_uniform_location(&self.step_prog, "u_state") { gl.uniform1i(Some(&l), 0); }

        gl.active_texture(GL::TEXTURE1);
        gl.bind_texture(GL::TEXTURE_2D, Some(&self.parent_texs[self.current]));
        if let Some(l) = gl.get_uniform_location(&self.step_prog, "u_parent") { gl.uniform1i(Some(&l), 1); }

        if let Some(l) = gl.get_uniform_location(&self.step_prog, "u_res") {
            gl.uniform2f(Some(&l), self.grid_w as f32, self.grid_h as f32);
        }

        gl.bind_vertex_array(Some(&self.quad_vao));
        gl.draw_arrays(GL::TRIANGLES, 0, 6);
        gl.bind_vertex_array(None);
        gl.bind_framebuffer(GL::FRAMEBUFFER, None);

        self.current = next;
    }

    pub fn draw(&self, canvas_w: i32, canvas_h: i32, dark_mode: bool) {
        use web_sys::WebGl2RenderingContext as GL;
        let gl = &self.gl;

        gl.viewport(0, 0, canvas_w, canvas_h);
        gl.use_program(Some(&self.draw_prog));

        gl.active_texture(GL::TEXTURE0);
        gl.bind_texture(GL::TEXTURE_2D, Some(&self.state_texs[self.current]));
        if let Some(l) = gl.get_uniform_location(&self.draw_prog, "u_state") { gl.uniform1i(Some(&l), 0); }

        let (visited, bg, wall) = if dark_mode {
            ([0.376_f32, 0.647, 0.980], [0.067_f32, 0.094, 0.153], [0.310_f32, 0.400, 0.502])
        } else {
            ([0.231_f32, 0.510, 0.965], [0.973_f32, 0.980, 0.988], [0.180_f32, 0.224, 0.286])
        };

        if let Some(l) = gl.get_uniform_location(&self.draw_prog, "u_visited") {
            gl.uniform3f(Some(&l), visited[0], visited[1], visited[2]);
        }
        if let Some(l) = gl.get_uniform_location(&self.draw_prog, "u_bg") {
            gl.uniform3f(Some(&l), bg[0], bg[1], bg[2]);
        }
        if let Some(l) = gl.get_uniform_location(&self.draw_prog, "u_wall") {
            gl.uniform3f(Some(&l), wall[0], wall[1], wall[2]);
        }
        if let Some(l) = gl.get_uniform_location(&self.draw_prog, "u_start") {
            gl.uniform2f(Some(&l),
                self.start.0 as f32 / self.grid_w as f32,
                self.start.1 as f32 / self.grid_h as f32,
            );
        }
        if let Some(l) = gl.get_uniform_location(&self.draw_prog, "u_end") {
            gl.uniform2f(Some(&l),
                self.end.0 as f32 / self.grid_w as f32,
                self.end.1 as f32 / self.grid_h as f32,
            );
        }
        if let Some(l) = gl.get_uniform_location(&self.draw_prog, "u_res") {
            gl.uniform2f(Some(&l), self.grid_w as f32, self.grid_h as f32);
        }

        gl.bind_vertex_array(Some(&self.quad_vao));
        gl.draw_arrays(GL::TRIANGLES, 0, 6);
        gl.bind_vertex_array(None);
    }

    /// Read one byte from the state texture at the end cell; true if visited (>=192).
    pub fn check_found(&self) -> bool {
        use web_sys::WebGl2RenderingContext as GL;
        let gl = &self.gl;
        let (ex, ey) = self.end;
        gl.bind_framebuffer(GL::READ_FRAMEBUFFER, Some(&self.fbs[self.current]));
        gl.read_buffer(GL::COLOR_ATTACHMENT0);
        let buf = js_sys::Uint8Array::new_with_length(1);
        gl.read_pixels_with_opt_array_buffer_view(
            ex as i32, ey as i32, 1, 1,
            GL::RED, GL::UNSIGNED_BYTE, Some(&buf),
        ).ok();
        gl.bind_framebuffer(GL::READ_FRAMEBUFFER, None);
        buf.get_index(0) >= 192
    }

    /// Read parent texture, trace path from end→start, stamp path cells as 255.
    pub fn reconstruct_path(&self) {
        use web_sys::WebGl2RenderingContext as GL;
        let gl = &self.gl;
        let n = (self.grid_w * self.grid_h) as usize;

        gl.bind_framebuffer(GL::READ_FRAMEBUFFER, Some(&self.fbs[self.current]));
        gl.read_buffer(GL::COLOR_ATTACHMENT1); // parent is attachment 1
        let raw = js_sys::Uint8Array::new_with_length(n as u32);
        gl.read_pixels_with_opt_array_buffer_view(
            0, 0, self.grid_w as i32, self.grid_h as i32,
            GL::RED, GL::UNSIGNED_BYTE, Some(&raw),
        ).ok();
        gl.bind_framebuffer(GL::READ_FRAMEBUFFER, None);

        let mut parent = vec![0u8; n];
        raw.copy_to(&mut parent);

        let path_px = [255u8];
        let (mut cx, mut cy) = self.end;
        let (sx, sy) = self.start;
        let max = (self.grid_w + self.grid_h) * 4;

        gl.bind_texture(GL::TEXTURE_2D, Some(&self.state_texs[self.current]));
        for _ in 0..max {
            gl.tex_sub_image_2d_with_i32_and_i32_and_u32_and_type_and_opt_u8_array(
                GL::TEXTURE_2D, 0, cx as i32, cy as i32, 1, 1,
                GL::RED, GL::UNSIGNED_BYTE, Some(&path_px),
            ).ok();
            if cx == sx && cy == sy { break; }
            let p = parent[(cy * self.grid_w + cx) as usize];
            let (nx, ny) = match p {
                33..=96   => (cx.wrapping_sub(1), cy), // from_left: parent is left
                97..=160  => (cx + 1, cy),              // from_right
                161..=224 => (cx, cy.wrapping_sub(1)), // from_below
                225..=255 => (cx, cy + 1),              // from_above
                _         => break,
            };
            if nx >= self.grid_w || ny >= self.grid_h { break; }
            cx = nx; cy = ny;
        }
        gl.bind_texture(GL::TEXTURE_2D, None);
    }

    fn make_r8(gl: &web_sys::WebGl2RenderingContext, w: u32, h: u32) -> Result<web_sys::WebGlTexture, String> {
        use web_sys::WebGl2RenderingContext as GL;
        let tex = gl.create_texture().ok_or("tex")?;
        gl.bind_texture(GL::TEXTURE_2D, Some(&tex));
        gl.tex_parameteri(GL::TEXTURE_2D, GL::TEXTURE_MIN_FILTER, GL::NEAREST as i32);
        gl.tex_parameteri(GL::TEXTURE_2D, GL::TEXTURE_MAG_FILTER, GL::NEAREST as i32);
        gl.tex_parameteri(GL::TEXTURE_2D, GL::TEXTURE_WRAP_S, GL::CLAMP_TO_EDGE as i32);
        gl.tex_parameteri(GL::TEXTURE_2D, GL::TEXTURE_WRAP_T, GL::CLAMP_TO_EDGE as i32);
        gl.tex_image_2d_with_i32_and_i32_and_i32_and_format_and_type_and_opt_u8_array(
            GL::TEXTURE_2D, 0, GL::R8 as i32,
            w as i32, h as i32, 0,
            GL::RED, GL::UNSIGNED_BYTE, None,
        ).map_err(|e| format!("{e:?}"))?;
        gl.bind_texture(GL::TEXTURE_2D, None);
        Ok(tex)
    }

    fn make_mrt_fb(
        gl: &web_sys::WebGl2RenderingContext,
        state_tex: &web_sys::WebGlTexture,
        parent_tex: &web_sys::WebGlTexture,
    ) -> Result<web_sys::WebGlFramebuffer, String> {
        use web_sys::WebGl2RenderingContext as GL;
        let fb = gl.create_framebuffer().ok_or("fb")?;
        gl.bind_framebuffer(GL::FRAMEBUFFER, Some(&fb));
        gl.framebuffer_texture_2d(GL::FRAMEBUFFER, GL::COLOR_ATTACHMENT0, GL::TEXTURE_2D, Some(state_tex), 0);
        gl.framebuffer_texture_2d(GL::FRAMEBUFFER, GL::COLOR_ATTACHMENT1, GL::TEXTURE_2D, Some(parent_tex), 0);
        let bufs = js_sys::Array::of2(
            &wasm_bindgen::JsValue::from_f64(GL::COLOR_ATTACHMENT0 as f64),
            &wasm_bindgen::JsValue::from_f64(GL::COLOR_ATTACHMENT1 as f64),
        );
        gl.draw_buffers(&bufs);
        gl.bind_framebuffer(GL::FRAMEBUFFER, None);
        Ok(fb)
    }

    fn compile_prog(gl: &web_sys::WebGl2RenderingContext, vert: &str, frag: &str) -> Result<web_sys::WebGlProgram, String> {
        use web_sys::WebGl2RenderingContext as GL;
        let vs = Self::compile_shader(gl, GL::VERTEX_SHADER, vert)?;
        let fs = Self::compile_shader(gl, GL::FRAGMENT_SHADER, frag)?;
        let prog = gl.create_program().ok_or("prog")?;
        gl.attach_shader(&prog, &vs);
        gl.attach_shader(&prog, &fs);
        gl.link_program(&prog);
        if !gl.get_program_parameter(&prog, GL::LINK_STATUS).as_bool().unwrap_or(false) {
            return Err(gl.get_program_info_log(&prog).unwrap_or("link".into()));
        }
        Ok(prog)
    }

    fn compile_shader(gl: &web_sys::WebGl2RenderingContext, ty: u32, src: &str) -> Result<web_sys::WebGlShader, String> {
        use web_sys::WebGl2RenderingContext as GL;
        let s = gl.create_shader(ty).ok_or("shader")?;
        gl.shader_source(&s, src);
        gl.compile_shader(&s);
        if !gl.get_shader_parameter(&s, GL::COMPILE_STATUS).as_bool().unwrap_or(false) {
            return Err(gl.get_shader_info_log(&s).unwrap_or("compile".into()));
        }
        Ok(s)
    }
}

// ── RAF loop ──────────────────────────────────────────────────────────────────

#[cfg(not(feature = "ssr"))]
fn start_path_raf_loop(
    renderer: std::rc::Rc<std::cell::RefCell<Option<PathGl>>>,
    running: std::rc::Rc<std::cell::Cell<bool>>,
    found: std::rc::Rc<std::cell::Cell<bool>>,
    canvas_ref: NodeRef<leptos::html::Canvas>,
    steps_per_frame: ReadSignal<u32>,
    set_status: WriteSignal<String>,
) {
    use std::cell::RefCell;
    use std::rc::Rc;
    use wasm_bindgen::prelude::*;
    use wasm_bindgen::JsCast;

    let f: Rc<RefCell<Option<Closure<dyn FnMut()>>>> = Rc::new(RefCell::new(None));
    let f_outer = f.clone();

    *f.borrow_mut() = Some(Closure::wrap(Box::new(move || {
        let window = web_sys::window().unwrap();

        if renderer.borrow().is_none() {
            window.request_animation_frame(f_outer.borrow().as_ref().unwrap().as_ref().unchecked_ref()).unwrap();
            return;
        }

        let canvas_params = canvas_ref.get_untracked().map(|canvas| {
            let el: &web_sys::HtmlCanvasElement = canvas.as_ref();
            let w = el.client_width() as u32;
            let h = el.client_height() as u32;
            if w > 0 && h > 0 && (el.width() != w || el.height() != h) {
                el.set_width(w);
                el.set_height(h);
            }
            let dark = web_sys::window()
                .and_then(|w| w.document())
                .and_then(|d| d.document_element())
                .map(|el| el.class_list().contains("dark"))
                .unwrap_or(false);
            (el.width() as i32, el.height() as i32, dark)
        });

        if let Some((cw, ch, dark)) = canvas_params {
            if running.get() && !found.get() {
                let steps = steps_per_frame.get_untracked();
                for _ in 0..steps {
                    renderer.borrow_mut().as_mut().unwrap().step();
                }
                if renderer.borrow().as_ref().unwrap().check_found() {
                    found.set(true);
                    renderer.borrow().as_ref().unwrap().reconstruct_path();
                    set_status("Path found!".to_string());
                }
            }
            renderer.borrow().as_ref().unwrap().draw(cw, ch, dark);
        }

        window.request_animation_frame(f_outer.borrow().as_ref().unwrap().as_ref().unchecked_ref()).unwrap();
    }) as Box<dyn FnMut()>));

    web_sys::window().unwrap()
        .request_animation_frame(f.borrow().as_ref().unwrap().as_ref().unchecked_ref())
        .unwrap();

    std::mem::forget(f);
}

// ── Component ─────────────────────────────────────────────────────────────────

#[component]
pub fn PathSearch() -> impl IntoView {
    #[cfg(not(feature = "ssr"))]
    use std::cell::{Cell, RefCell};
    #[cfg(not(feature = "ssr"))]
    use std::rc::Rc;

    #[cfg(not(feature = "ssr"))]
    const GRID_SIZE: u32 = 2048;

    let (obstacle_prob, set_obstacle_prob) = signal(0.25_f64);
    let (steps_per_frame, set_steps_per_frame) = signal(8_u32);
    let (show_settings, set_show_settings) = signal(false);
    let (is_running, set_is_running) = signal(false);
    #[cfg(feature = "ssr")]
    let (status, _set_status) = signal(String::new());
    #[cfg(not(feature = "ssr"))]
    let (status, set_status) = signal(String::new());

    let canvas_ref = NodeRef::<leptos::html::Canvas>::new();
    let container_ref = NodeRef::<leptos::html::Div>::new();

    #[cfg(not(feature = "ssr"))]
    let renderer: Rc<RefCell<Option<PathGl>>> = Rc::new(RefCell::new(None));
    #[cfg(not(feature = "ssr"))]
    let running_flag: Rc<Cell<bool>> = Rc::new(Cell::new(false));
    #[cfg(not(feature = "ssr"))]
    let found_flag: Rc<Cell<bool>> = Rc::new(Cell::new(false));
    #[cfg(not(feature = "ssr"))]
    let loop_active: Rc<Cell<bool>> = Rc::new(Cell::new(false));

    // Keep running_flag in sync.
    #[cfg(not(feature = "ssr"))]
    {
        let running_flag = running_flag.clone();
        Effect::new(move |_| { running_flag.set(is_running()); });
    }

    // Init renderer once canvas mounts.
    #[cfg(not(feature = "ssr"))]
    {
        let renderer = renderer.clone();
        let running_flag = running_flag.clone();
        let found_flag = found_flag.clone();
        let loop_active = loop_active.clone();

        Effect::new(move |_| {
            let Some(canvas) = canvas_ref.get() else { return; };
            if renderer.borrow().is_some() { return; }
            let el: &web_sys::HtmlCanvasElement = canvas.as_ref();
            match PathGl::new(el, GRID_SIZE, GRID_SIZE) {
                Ok(mut gl) => {
                    gl.reset(obstacle_prob.get_untracked() as f32);
                    *renderer.borrow_mut() = Some(gl);
                    if !loop_active.get() {
                        loop_active.set(true);
                        start_path_raf_loop(
                            renderer.clone(),
                            running_flag.clone(),
                            found_flag.clone(),
                            canvas_ref,
                            steps_per_frame,
                            set_status,
                        );
                    }
                }
                Err(e) => web_sys::console::error_1(&format!("PathGl: {e}").into()),
            }
        });
    }

    // IntersectionObserver: auto start/stop, reset on first view.
    #[cfg(not(feature = "ssr"))]
    {
        use wasm_bindgen::prelude::*;
        use wasm_bindgen::JsCast;

        let renderer = renderer.clone();
        let found_flag = found_flag.clone();
        let has_started = Rc::new(Cell::new(false));

        let callback = Rc::new(Closure::wrap(Box::new(
            move |entries: js_sys::Array, _: web_sys::IntersectionObserver| {
                for entry in entries.iter() {
                    let Ok(entry) = entry.dyn_into::<web_sys::IntersectionObserverEntry>() else { continue; };
                    if entry.is_intersecting() {
                        if !has_started.get() {
                            has_started.set(true);
                            if let Some(ref mut gl) = *renderer.borrow_mut() {
                                found_flag.set(false);
                                set_status(String::new());
                                gl.reset(obstacle_prob.get_untracked() as f32);
                            }
                        }
                        set_is_running(true);
                    } else {
                        set_is_running(false);
                    }
                }
            },
        ) as Box<dyn FnMut(js_sys::Array, web_sys::IntersectionObserver)>));

        let cb_ref = callback.clone();
        Effect::new(move |_| {
            let Some(container) = container_ref.get() else { return; };
            if let Ok(obs) = web_sys::IntersectionObserver::new(cb_ref.as_ref().as_ref().unchecked_ref()) {
                obs.observe(&container);
            }
        });
        std::mem::forget(callback);
    }

    #[cfg(not(feature = "ssr"))]
    let randomize = {
        let renderer = renderer.clone();
        let found_flag = found_flag.clone();
        move |_| {
            set_is_running(false);
            found_flag.set(false);
            set_status(String::new());
            if let Some(ref mut gl) = *renderer.borrow_mut() {
                gl.reset(obstacle_prob.get_untracked() as f32);
            }
            set_is_running(true);
        }
    };
    #[cfg(feature = "ssr")]
    let randomize = move |_| {};

    view! {
        <div node_ref=container_ref class="w-full flex flex-col gap-4 items-center relative">
            <div class="flex gap-3 items-center flex-wrap justify-center">
                <button
                    class="px-4 py-1.5 text-sm rounded border transition-all duration-200 hover:bg-accent hover:bg-opacity-10"
                    style:border-color="#3B82F6" style:color="#3B82F6"
                    on:click=move |_| set_is_running.update(|r| *r = !*r)
                    aria-label=move || if is_running() { "Pause" } else { "Play" }
                >
                    {move || if is_running() { "▌▌" } else { "▶" }}
                </button>
                <button
                    class="px-4 py-1.5 text-sm rounded border transition-all duration-200 hover:bg-accent hover:bg-opacity-10"
                    style:border-color="#3B82F6" style:color="#3B82F6"
                    on:click=randomize aria-label="Randomize"
                >"↻"</button>
                <button
                    class="px-4 py-1.5 text-sm rounded border border-border text-charcoal hover:bg-border hover:bg-opacity-20 transition-all duration-200 flex items-center justify-center"
                    on:click=move |_| set_show_settings.update(|v| *v = !*v)
                    aria-label="Settings"
                >
                    <Icon name="cog" class="w-4 h-4" />
                </button>
                <span class="text-sm text-charcoal-light">"2048×2048 · GPU BFS"</span>
                {move || {
                    let s = status();
                    (!s.is_empty()).then(|| view! {
                        <span class="text-sm text-accent font-mono">{s}</span>
                    })
                }}
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
                                "Obstacle Density"
                            </label>
                            <input
                                type="range" min="0.05" max="0.6" step="0.05"
                                class="w-full accent-blue-500"
                                on:input=move |ev| {
                                    if let Ok(v) = event_target_value(&ev).parse::<f64>() {
                                        set_obstacle_prob(v);
                                    }
                                }
                                prop:value=obstacle_prob
                            />
                            <span class="text-xs text-charcoal-light text-right">
                                {move || format!("{:.0}%", obstacle_prob() * 100.0)}
                            </span>
                        </div>

                        <div class="flex flex-col gap-2">
                            <label class="text-sm font-medium text-charcoal">"Speed"</label>
                            <select
                                class="px-3 py-2 rounded border border-border bg-surface text-charcoal focus:outline-none focus:ring-2 focus:ring-accent"
                                on:change=move |ev| {
                                    if let Ok(v) = event_target_value(&ev).parse::<u32>() {
                                        set_steps_per_frame(v);
                                    }
                                }
                            >
                                {[(2u32,"Slow (2/frame)"),(4,"Normal (4/frame)"),(8,"Fast (8/frame)"),(16,"Very Fast (16/frame)")].into_iter().map(|(n, label)| {
                                    view! {
                                        <option value=n.to_string() selected=move || steps_per_frame() == n>
                                            {label}
                                        </option>
                                    }
                                }).collect_view()}
                            </select>
                        </div>
                    </div>
                </div>
            </Show>

            <div class="text-sm text-charcoal-light max-w-2xl text-center">
                "GPU BFS wavefront expanding from start (green) to end (amber) across a 2048×2048 grid."
            </div>

            <canvas
                node_ref=canvas_ref
                class="w-full aspect-square border border-border"
                style="max-height: 60vh; object-fit: contain;"
            />
        </div>
    }
}
