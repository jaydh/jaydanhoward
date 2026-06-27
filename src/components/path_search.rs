#![allow(clippy::all)]
use leptos::prelude::*;
use std::fmt;

// Cell state values stored in flat Vec<u8> and uploaded as R8 texture.
#[cfg(not(feature = "ssr"))]
const OBSTACLE: u8 = 0;
#[cfg(not(feature = "ssr"))]
const UNVISITED: u8 = 1;
#[cfg(not(feature = "ssr"))]
const FRONTIER: u8 = 2;
#[cfg(not(feature = "ssr"))]
const VISITED: u8 = 3;
#[cfg(not(feature = "ssr"))]
const PATH: u8 = 4;

#[derive(Clone, Debug, PartialEq)]
enum Algorithm {
    Bfs, Dfs, AStar, Greedy, Corner, Wall, RandomWalk,
}

impl fmt::Display for Algorithm {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Algorithm::Bfs        => write!(f, "BFS"),
            Algorithm::Dfs        => write!(f, "DFS"),
            Algorithm::AStar      => write!(f, "A*"),
            Algorithm::Greedy     => write!(f, "Greedy Best-First"),
            Algorithm::Corner     => write!(f, "Corner"),
            Algorithm::Wall       => write!(f, "Wall"),
            Algorithm::RandomWalk => write!(f, "Random Walk"),
        }
    }
}

// ── WebGL2 renderer ───────────────────────────────────────────────────────────

#[cfg(not(feature = "ssr"))]
const VERT: &str = r#"#version 300 es
in vec2 a_pos;
out vec2 v_uv;
void main() {
    v_uv = a_pos * 0.5 + 0.5;
    gl_Position = vec4(a_pos, 0.0, 1.0);
}"#;

// Maps state byte (0-4) to color; overlays start/end markers.
// u_zoom > 1.0 zooms in centered on u_zoom_center (normalised grid coords).
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
}
"#;

#[cfg(not(feature = "ssr"))]
pub struct PathRenderer {
    gl: web_sys::WebGl2RenderingContext,
    prog: web_sys::WebGlProgram,
    tex: web_sys::WebGlTexture,
    vao: web_sys::WebGlVertexArrayObject,
    grid_w: u32,
    grid_h: u32,
}

#[cfg(not(feature = "ssr"))]
impl PathRenderer {
    pub fn new(canvas: &web_sys::HtmlCanvasElement, grid_w: u32, grid_h: u32) -> Result<Self, String> {
        use wasm_bindgen::JsCast;
        use web_sys::WebGl2RenderingContext as GL;
        let gl = canvas.get_context("webgl2").map_err(|_| "ctx")?
            .ok_or("no webgl2")?.dyn_into::<GL>().map_err(|_| "cast")?;
        let prog = Self::compile_prog(&gl, VERT, DRAW_FRAG)?;
        let vao = gl.create_vertex_array().ok_or("vao")?;
        gl.bind_vertex_array(Some(&vao));
        let buf = gl.create_buffer().ok_or("buf")?;
        gl.bind_buffer(GL::ARRAY_BUFFER, Some(&buf));
        let verts: [f32; 12] = [-1.,-1., 1.,-1., -1.,1., -1.,1., 1.,-1., 1.,1.];
        unsafe {
            gl.buffer_data_with_array_buffer_view(
                GL::ARRAY_BUFFER, &js_sys::Float32Array::view(&verts), GL::STATIC_DRAW,
            );
        }
        let loc = gl.get_attrib_location(&prog, "a_pos") as u32;
        gl.enable_vertex_attrib_array(loc);
        gl.vertex_attrib_pointer_with_i32(loc, 2, GL::FLOAT, false, 0, 0);
        gl.bind_vertex_array(None);
        let tex = gl.create_texture().ok_or("tex")?;
        gl.bind_texture(GL::TEXTURE_2D, Some(&tex));
        gl.tex_parameteri(GL::TEXTURE_2D, GL::TEXTURE_MIN_FILTER, GL::NEAREST as i32);
        gl.tex_parameteri(GL::TEXTURE_2D, GL::TEXTURE_MAG_FILTER, GL::NEAREST as i32);
        gl.tex_parameteri(GL::TEXTURE_2D, GL::TEXTURE_WRAP_S, GL::CLAMP_TO_EDGE as i32);
        gl.tex_parameteri(GL::TEXTURE_2D, GL::TEXTURE_WRAP_T, GL::CLAMP_TO_EDGE as i32);
        gl.tex_image_2d_with_i32_and_i32_and_i32_and_format_and_type_and_opt_u8_array(
            GL::TEXTURE_2D, 0, GL::R8 as i32, grid_w as i32, grid_h as i32, 0,
            GL::RED, GL::UNSIGNED_BYTE, None,
        ).map_err(|e| format!("{e:?}"))?;
        gl.bind_texture(GL::TEXTURE_2D, None);
        Ok(Self { gl, prog, tex, vao, grid_w, grid_h })
    }

    pub fn upload(&self, state: &[u8]) {
        use web_sys::WebGl2RenderingContext as GL;
        self.gl.bind_texture(GL::TEXTURE_2D, Some(&self.tex));
        let _ = self.gl.tex_image_2d_with_i32_and_i32_and_i32_and_format_and_type_and_opt_u8_array(
            GL::TEXTURE_2D, 0, GL::R8 as i32, self.grid_w as i32, self.grid_h as i32, 0,
            GL::RED, GL::UNSIGNED_BYTE, Some(state),
        );
        self.gl.bind_texture(GL::TEXTURE_2D, None);
    }

    pub fn draw(&self, cw: i32, ch: i32, dark: bool, start: (u32, u32), end: (u32, u32), zoom: f32) {
        use web_sys::WebGl2RenderingContext as GL;
        let gl = &self.gl;
        gl.viewport(0, 0, cw, ch);
        gl.use_program(Some(&self.prog));
        gl.active_texture(GL::TEXTURE0);
        gl.bind_texture(GL::TEXTURE_2D, Some(&self.tex));
        if let Some(l) = gl.get_uniform_location(&self.prog, "u_state") { gl.uniform1i(Some(&l), 0); }
        let (vis, bg, wall) = if dark {
            ([0.376_f32,0.647,0.980], [0.067_f32,0.094,0.153], [0.310_f32,0.400,0.502])
        } else {
            ([0.231_f32,0.510,0.965], [0.973_f32,0.980,0.988], [0.180_f32,0.224,0.286])
        };
        if let Some(l) = gl.get_uniform_location(&self.prog, "u_visited") { gl.uniform3f(Some(&l), vis[0], vis[1], vis[2]); }
        if let Some(l) = gl.get_uniform_location(&self.prog, "u_bg")      { gl.uniform3f(Some(&l), bg[0], bg[1], bg[2]); }
        if let Some(l) = gl.get_uniform_location(&self.prog, "u_wall")    { gl.uniform3f(Some(&l), wall[0], wall[1], wall[2]); }
        let su = start.0 as f32 / self.grid_w as f32;
        let sv = start.1 as f32 / self.grid_h as f32;
        let eu = end.0 as f32 / self.grid_w as f32;
        let ev = end.1 as f32 / self.grid_h as f32;
        if let Some(l) = gl.get_uniform_location(&self.prog, "u_start") {
            gl.uniform2f(Some(&l), su, sv);
        }
        if let Some(l) = gl.get_uniform_location(&self.prog, "u_end") {
            gl.uniform2f(Some(&l), eu, ev);
        }
        if let Some(l) = gl.get_uniform_location(&self.prog, "u_res") {
            gl.uniform2f(Some(&l), self.grid_w as f32, self.grid_h as f32);
        }
        if let Some(l) = gl.get_uniform_location(&self.prog, "u_zoom") {
            gl.uniform1f(Some(&l), zoom.max(0.01));
        }
        if let Some(l) = gl.get_uniform_location(&self.prog, "u_zoom_center") {
            gl.uniform2f(Some(&l), (su + eu) * 0.5, (sv + ev) * 0.5);
        }
        gl.bind_vertex_array(Some(&self.vao));
        gl.draw_arrays(GL::TRIANGLES, 0, 6);
        gl.bind_vertex_array(None);
    }

    fn compile_prog(gl: &web_sys::WebGl2RenderingContext, vert: &str, frag: &str) -> Result<web_sys::WebGlProgram, String> {
        use web_sys::WebGl2RenderingContext as GL;
        let vs = Self::compile_shader(gl, GL::VERTEX_SHADER, vert)?;
        let fs = Self::compile_shader(gl, GL::FRAGMENT_SHADER, frag)?;
        let prog = gl.create_program().ok_or("prog")?;
        gl.attach_shader(&prog, &vs); gl.attach_shader(&prog, &fs);
        gl.link_program(&prog);
        if !gl.get_program_parameter(&prog, GL::LINK_STATUS).as_bool().unwrap_or(false) {
            return Err(gl.get_program_info_log(&prog).unwrap_or("link".into()));
        }
        Ok(prog)
    }

    fn compile_shader(gl: &web_sys::WebGl2RenderingContext, ty: u32, src: &str) -> Result<web_sys::WebGlShader, String> {
        use web_sys::WebGl2RenderingContext as GL;
        let s = gl.create_shader(ty).ok_or("shader")?;
        gl.shader_source(&s, src); gl.compile_shader(&s);
        if !gl.get_shader_parameter(&s, GL::COMPILE_STATUS).as_bool().unwrap_or(false) {
            return Err(gl.get_shader_info_log(&s).unwrap_or("compile".into()));
        }
        Ok(s)
    }
}

// ── Algorithm runner (CPU) ────────────────────────────────────────────────────

#[cfg(not(feature = "ssr"))]
pub struct AlgoRun {
    pub state: Vec<u8>,
    parent: Vec<u32>,
    queue: Vec<u32>, // BFS: used with head pointer (FIFO); others: stack (LIFO)
    head: usize,     // BFS dequeue head
    pub w: u32,
    pub h: u32,
    pub start: (u32, u32),
    pub end: (u32, u32),
    initialized: bool,
    pub done: bool,
    pub steps: u64,
    pub completion_steps: Option<u64>,
}

#[cfg(not(feature = "ssr"))]
impl AlgoRun {
    pub fn new(base: &[u8], w: u32, h: u32, start: (u32, u32), end: (u32, u32)) -> Self {
        // base[i] == OBSTACLE means blocked, everything else passable
        let state = base.to_vec();
        let parent = vec![u32::MAX; (w * h) as usize];
        Self { state, parent, queue: Vec::new(), head: 0, w, h, start, end,
               initialized: false, done: false, steps: 0, completion_steps: None }
    }

    fn idx(&self, x: u32, y: u32) -> u32 { y * self.w + x }
    fn start_idx(&self) -> u32 { self.idx(self.start.0, self.start.1) }
    fn end_idx(&self)   -> u32 { self.idx(self.end.0, self.end.1) }

    fn neighbors(&self, i: u32) -> [Option<u32>; 4] {
        let x = i % self.w; let y = i / self.w;
        [
            if x > 0 { Some(i - 1) } else { None },
            if x + 1 < self.w { Some(i + 1) } else { None },
            if y > 0 { Some(i - self.w) } else { None },
            if y + 1 < self.h { Some(i + self.w) } else { None },
        ]
    }

    fn manhattan(&self, a: u32, b: u32) -> u32 {
        let (ax, ay) = (a % self.w, a / self.w);
        let (bx, by) = (b % self.w, b / self.w);
        ax.abs_diff(bx) + ay.abs_diff(by)
    }

    fn wall_dist(&self, i: u32) -> u32 {
        let x = i % self.w; let y = i / self.w;
        x.min(self.w - 1 - x).min(y).min(self.h - 1 - y)
    }

    fn corner_dist(&self, i: u32) -> u32 {
        let corners = [0u32, self.w - 1, self.w * (self.h - 1), self.w * self.h - 1];
        corners.iter().map(|&c| self.manhattan(i, c)).min().unwrap_or(0)
    }

    pub fn step(&mut self, algo: &Algorithm) {
        if self.done { return; }
        self.steps += 1;

        let si = self.start_idx();
        let ei = self.end_idx();

        if !self.initialized {
            self.initialized = true;
            self.state[si as usize] = FRONTIER;
            self.queue.push(si);
            return;
        }

        // Pop next candidate (BFS = FIFO via head, others = LIFO)
        let current = loop {
            let c = match algo {
                Algorithm::Bfs => {
                    if self.head >= self.queue.len() { self.done = true; return; }
                    let c = self.queue[self.head]; self.head += 1; c
                }
                _ => {
                    if self.queue.is_empty() { self.done = true; return; }
                    self.queue.pop().unwrap()
                }
            };
            let s = self.state[c as usize];
            if s != VISITED && s != PATH { break c; }
        };

        if current == ei {
            self.state[current as usize] = VISITED;
            self.completion_steps = Some(self.steps);
            self.done = true;
            self.reconstruct_path(si, ei);
            return;
        }

        self.state[current as usize] = VISITED;

        let mut viable: Vec<u32> = self.neighbors(current)
            .iter().filter_map(|&n| n)
            .filter(|&n| self.state[n as usize] == UNVISITED)
            .collect();

        // Mark all as frontier before sorting so they don't get double-added
        for &n in &viable {
            self.state[n as usize] = FRONTIER;
            self.parent[n as usize] = current;
        }

        match algo {
            Algorithm::Bfs => {
                for &n in &viable { self.queue.push(n); }
            }
            Algorithm::Dfs => {
                for &n in &viable { self.queue.push(n); }
            }
            Algorithm::AStar | Algorithm::Greedy => {
                let ei2 = ei;
                // Sort new cells: best (lowest h) last so it's popped first
                viable.sort_by_key(|&n| self.manhattan(n, ei2));
                viable.reverse(); // worst at front, best at back of what we add
                for &n in &viable { self.queue.push(n); }
                // Re-sort whole remaining queue so globally best is always at back
                let h = self.head;
                let grid_w = self.w;
                self.queue[h..].sort_by_key(|&n| {
                    let (ax, ay) = (n % grid_w, n / grid_w);
                    let (bx, by) = (ei2 % grid_w, ei2 / grid_w);
                    u32::MAX - (ax.abs_diff(bx) + ay.abs_diff(by))
                });
            }
            Algorithm::Corner => {
                viable.sort_by_key(|&n| self.corner_dist(n)); // closest-to-corner last
                viable.reverse();
                for &n in &viable { self.queue.push(n); }
            }
            Algorithm::Wall => {
                viable.sort_by_key(|&n| self.wall_dist(n)); // closest-to-wall last
                viable.reverse();
                for &n in &viable { self.queue.push(n); }
            }
            Algorithm::RandomWalk => {
                let seed = self.steps.wrapping_mul(6364136223846793005).wrapping_add(current as u64);
                viable.sort_by_key(|&n| seed.wrapping_mul(n as u64 + 1) >> 32);
                for &n in &viable { self.queue.push(n); }
            }
        }
    }

    fn reconstruct_path(&mut self, si: u32, ei: u32) {
        let mut curr = ei;
        for _ in 0..(self.w + self.h) * 4 {
            self.state[curr as usize] = PATH;
            if curr == si { break; }
            let p = self.parent[curr as usize];
            if p == u32::MAX { break; }
            curr = p;
        }
    }
}

// ── AlgorithmSimulation component ─────────────────────────────────────────────

#[component]
#[allow(unused_variables)]
fn AlgorithmSimulation(
    algorithm: Algorithm,
    grid_version: ReadSignal<u32>,
    grid_data: std::rc::Rc<std::cell::RefCell<Option<(Vec<u8>, u32, u32, (u32,u32), (u32,u32))>>>,
    is_running: ReadSignal<bool>,
    completion_order: ReadSignal<Vec<Algorithm>>,
    set_completion_order: WriteSignal<Vec<Algorithm>>,
    zoom: ReadSignal<f32>,
) -> impl IntoView {
    let canvas_ref = NodeRef::<leptos::html::Canvas>::new();

    let (completion_steps, set_completion_steps) = signal(None::<u64>);
    let (fps, set_fps) = signal(0.0_f64);

    #[cfg(not(feature = "ssr"))]
    {
        use std::cell::{Cell, RefCell};
        use std::rc::Rc;
        use wasm_bindgen::prelude::*;
        use wasm_bindgen::JsCast;

        let renderer: Rc<RefCell<Option<PathRenderer>>> = Rc::new(RefCell::new(None));
        let run: Rc<RefCell<Option<AlgoRun>>> = Rc::new(RefCell::new(None));
        let running: Rc<Cell<bool>> = Rc::new(Cell::new(false));
        let loop_active: Rc<Cell<bool>> = Rc::new(Cell::new(false));
        let frame_count: Rc<Cell<u32>> = Rc::new(Cell::new(0));

        // Sync running flag
        { let r = running.clone(); Effect::new(move |_| { r.set(is_running()); }); }

        // Reset run when grid version changes
        {
            let run = run.clone();
            let gd = grid_data.clone();
            Effect::new(move |_| {
                let _ = grid_version();
                if let Some((ref base, w, h, start, end)) = *gd.borrow() {
                    *run.borrow_mut() = Some(AlgoRun::new(base, w, h, start, end));
                }
                set_completion_steps(None);
                set_fps(0.0);
            });
        }

        // Init renderer + start RAF loop once canvas mounts
        {
            let renderer = renderer.clone();
            let run = run.clone();
            let running = running.clone();
            let loop_active = loop_active.clone();
            let frame_count = frame_count.clone();
            let algo = algorithm.clone();
            let gd = grid_data.clone();

            Effect::new(move |_| {
                let Some(canvas) = canvas_ref.get() else { return; };
                if renderer.borrow().is_some() { return; }

                // Determine grid dimensions from current grid data
                let dims = gd.borrow().as_ref().map(|(_, w, h, s, e)| (*w, *h, *s, *e));
                let (gw, gh) = dims.map(|(w, h, _, _)| (w, h)).unwrap_or((2048, 2048));

                let el: &web_sys::HtmlCanvasElement = canvas.as_ref();
                match PathRenderer::new(el, gw, gh) {
                    Ok(r) => { *renderer.borrow_mut() = Some(r); }
                    Err(e) => { web_sys::console::error_1(&format!("PathRenderer: {e}").into()); return; }
                }

                if loop_active.get() { return; }
                loop_active.set(true);

                // RAF loop
                let f: Rc<RefCell<Option<Closure<dyn FnMut()>>>> = Rc::new(RefCell::new(None));
                let f_outer = f.clone();

                let run_raf = run.clone();
                let running_raf = running.clone();
                let frame_count_raf = frame_count.clone();
                let algo_raf = algo.clone();
                let renderer_raf = renderer.clone();

                *f.borrow_mut() = Some(Closure::wrap(Box::new(move || {
                    let run = &run_raf;
                    let running = &running_raf;
                    let frame_count = &frame_count_raf;
                    let algo = &algo_raf;
                    let renderer = &renderer_raf;
                    let window = web_sys::window().unwrap();

                    let canvas_params = canvas_ref.get_untracked().map(|c| {
                        let el: &web_sys::HtmlCanvasElement = c.as_ref();
                        let w = el.client_width() as u32;
                        let h = el.client_height() as u32;
                        if w > 0 && h > 0 && (el.width() != w || el.height() != h) {
                            el.set_width(w); el.set_height(h);
                        }
                        let dark = web_sys::window()
                            .and_then(|w| w.document())
                            .and_then(|d| d.document_element())
                            .map(|el| el.class_list().contains("dark"))
                            .unwrap_or(false);
                        (el.width() as i32, el.height() as i32, dark)
                    });

                    if let Some((cw, ch, dark)) = canvas_params {
                        let is_done = run.borrow().as_ref().map(|r| r.done).unwrap_or(true);

                        if running.get() && !is_done {
                            const STEPS: u32 = 5;
                            for _ in 0..STEPS {
                                if let Some(ref mut r) = *run.borrow_mut() {
                                    if !r.done { r.step(&algo); }
                                }
                            }
                            frame_count.set(frame_count.get() + STEPS);
                            if frame_count.get() >= 120 {
                                set_fps(frame_count.get() as f64);
                                frame_count.set(0);
                            }

                            // Check completion
                            let completed = run.borrow().as_ref()
                                .and_then(|r| if r.done { r.completion_steps } else { None });
                            if let Some(steps) = completed {
                                set_completion_steps(Some(steps));
                                let algo_clone = algo.clone();
                                set_completion_order.update(|order| {
                                    if !order.contains(&algo_clone) { order.push(algo_clone); }
                                });
                            }
                        }

                        // Upload + draw
                        if let (Some(ref rend), Some(ref run_ref)) = (&*renderer.borrow(), &*run.borrow()) {
                            rend.upload(&run_ref.state);
                            rend.draw(cw, ch, dark, run_ref.start, run_ref.end, zoom.get_untracked());
                        }
                    }

                    window.request_animation_frame(
                        f_outer.borrow().as_ref().unwrap().as_ref().unchecked_ref()
                    ).unwrap();
                }) as Box<dyn FnMut()>));

                web_sys::window().unwrap()
                    .request_animation_frame(f.borrow().as_ref().unwrap().as_ref().unchecked_ref())
                    .unwrap();
                std::mem::forget(f);
            });
        }
    }

    let algo_name = algorithm.to_string();
    let algo_clone = algorithm.clone();

    view! {
        <div class="flex flex-col gap-2 items-center" style="width: 55vh; max-width: 100%;">
            <div class="flex flex-col items-center gap-1">
                <div class="flex items-center gap-2">
                    <h3 class="text-sm font-medium text-charcoal">{algo_name}</h3>
                    {move || {
                        let order = completion_order();
                        order.iter().position(|a| a == &algo_clone).map(|pos| {
                            let rank = match pos {
                                0 => "🥇 1st", 1 => "🥈 2nd", 2 => "🥉 3rd",
                                3 => "4th", 4 => "5th", 5 => "6th", _ => "7th",
                            };
                            view! { <span class="text-xs font-bold text-accent">{rank}</span> }
                        })
                    }}
                </div>
                {move || completion_steps().map(|s| view! {
                    <span class="text-xs text-charcoal-light font-mono">{format!("{s} steps")}</span>
                })}
            </div>
            <canvas
                node_ref=canvas_ref
                class="border border-border aspect-square w-full"
                style="max-height: 55vh;"
            />
            <div class="text-xs text-charcoal-light font-mono min-h-[1.5rem]">
                {move || if is_running() && fps() > 0.0 && completion_steps().is_none() {
                    format!("{:.0} steps/s", fps())
                } else { String::new() }}
            </div>
        </div>
    }
}

// ── PathSearch component ───────────────────────────────────────────────────────

#[component]
pub fn PathSearch() -> impl IntoView {
    #[cfg(not(feature = "ssr"))]
    use std::cell::RefCell;
    #[cfg(not(feature = "ssr"))]
    use std::rc::Rc;

    #[cfg(not(feature = "ssr"))]
    const GRID_SIZE: u32 = 2048;
    #[cfg(not(feature = "ssr"))]
    const OBSTACLE_PROB: f64 = 0.2;
    let (is_running, set_is_running) = signal(false);
    let (zoom, set_zoom) = signal(1.0_f32);
    #[cfg(feature = "ssr")]
    let (grid_version, _set_grid_version) = signal(0_u32);
    #[cfg(not(feature = "ssr"))]
    let (grid_version, set_grid_version) = signal(0_u32);
    let (blind_order, set_blind_order) = signal(Vec::<Algorithm>::new());
    let (informed_order, set_informed_order) = signal(Vec::<Algorithm>::new());

    // Shared grid data: (state Vec, w, h, start, end)
    #[cfg(not(feature = "ssr"))]
    let grid_data: Rc<RefCell<Option<(Vec<u8>, u32, u32, (u32,u32), (u32,u32))>>> =
        Rc::new(RefCell::new(None));

    #[cfg(not(feature = "ssr"))]
    let do_randomize = {
        let gd = grid_data.clone();
        move |sz: u32, prob: f64| {
            use rand::Rng;
            let mut rng = rand::rng();
            let n = (sz * sz) as usize;
            let mut base = vec![UNVISITED; n];
            let mut passable: Vec<u32> = Vec::new();
            for i in 0..n {
                if rng.random::<f64>() < prob {
                    base[i] = OBSTACLE;
                } else {
                    passable.push(i as u32);
                }
            }
            if passable.len() < 2 {
                return;
            }
            let si = rng.random_range(0..passable.len());
            let mut ei = rng.random_range(0..passable.len());
            while ei == si { ei = rng.random_range(0..passable.len()); }
            let start_idx = passable[si];
            let end_idx = passable[ei];
            let start = (start_idx % sz, start_idx / sz);
            let end = (end_idx % sz, end_idx / sz);
            *gd.borrow_mut() = Some((base, sz, sz, start, end));
            set_grid_version.update(|v| *v += 1);
            set_blind_order(Vec::new());
            set_informed_order(Vec::new());
        }
    };

    let container_ref = NodeRef::<leptos::html::Div>::new();

    // Randomize on mount
    #[cfg(not(feature = "ssr"))]
    {
        let do_rand = do_randomize.clone();
        Effect::new(move |_| {
            do_rand(GRID_SIZE, OBSTACLE_PROB);
        });
    }

    // IntersectionObserver: auto start/stop
    #[cfg(not(feature = "ssr"))]
    {
        use std::cell::Cell;
        use std::rc::Rc;
        use wasm_bindgen::prelude::*;
        use wasm_bindgen::JsCast;
        let has_started = Rc::new(Cell::new(false));
        let callback = Rc::new(Closure::wrap(Box::new(
            move |entries: js_sys::Array, _: web_sys::IntersectionObserver| {
                for entry in entries.iter() {
                    let Ok(e) = entry.dyn_into::<web_sys::IntersectionObserverEntry>() else { continue; };
                    if e.is_intersecting() {
                        has_started.set(true);
                        set_is_running(true);
                    } else {
                        set_is_running(false);
                    }
                }
            },
        ) as Box<dyn FnMut(js_sys::Array, web_sys::IntersectionObserver)>));
        let cb_ref = callback.clone();
        Effect::new(move |_| {
            let Some(c) = container_ref.get() else { return; };
            if let Ok(obs) = web_sys::IntersectionObserver::new(cb_ref.as_ref().as_ref().unchecked_ref()) {
                obs.observe(&c);
            }
        });
        std::mem::forget(callback);
    }

    #[cfg(not(feature = "ssr"))]
    let randomize = {
        let do_rand = do_randomize.clone();
        move |_| {
            set_is_running(false);
            do_rand(GRID_SIZE, OBSTACLE_PROB);
            set_is_running(true);
        }
    };
    #[cfg(feature = "ssr")]
    let randomize = move |_| {};

    // Clone grid_data for each child
    #[cfg(not(feature = "ssr"))]
    let (gd1,gd2,gd3,gd4,gd5,gd6,gd7) = (
        grid_data.clone(), grid_data.clone(), grid_data.clone(), grid_data.clone(),
        grid_data.clone(), grid_data.clone(), grid_data.clone(),
    );
    #[cfg(feature = "ssr")]
    let (gd1,gd2,gd3,gd4,gd5,gd6,gd7) = (
        std::rc::Rc::new(std::cell::RefCell::new(None::<(Vec<u8>,u32,u32,(u32,u32),(u32,u32))>)),
        std::rc::Rc::new(std::cell::RefCell::new(None)),
        std::rc::Rc::new(std::cell::RefCell::new(None)),
        std::rc::Rc::new(std::cell::RefCell::new(None)),
        std::rc::Rc::new(std::cell::RefCell::new(None)),
        std::rc::Rc::new(std::cell::RefCell::new(None)),
        std::rc::Rc::new(std::cell::RefCell::new(None)),
    );

    view! {
        <div node_ref=container_ref class="w-full flex flex-col gap-8 items-center">
            <div class="flex gap-3 items-center flex-wrap justify-center">
                <button
                    class="px-4 py-1.5 text-sm rounded border transition-all duration-200 hover:bg-accent hover:bg-opacity-10"
                    style:border-color="#3B82F6" style:color="#3B82F6"
                    on:click=move |_| set_is_running.update(|r| *r = !*r)
                    aria-label=move || if is_running() { "Pause" } else { "Play" }
                >
                    {move || if is_running() {
                        view! {
                            <svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24" fill="currentColor">
                                <rect x="5" y="4" width="4" height="16" rx="1"/>
                                <rect x="15" y="4" width="4" height="16" rx="1"/>
                            </svg>
                        }.into_any()
                    } else {
                        view! {
                            <svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24" fill="currentColor">
                                <polygon points="5,3 19,12 5,21"/>
                            </svg>
                        }.into_any()
                    }}
                </button>
                <button
                    class="px-4 py-1.5 text-sm rounded border transition-all duration-200 hover:bg-accent hover:bg-opacity-10"
                    style:border-color="#3B82F6" style:color="#3B82F6"
                    on:click=randomize aria-label="Randomize"
                >"↻"</button>
                <div class="flex items-center gap-2">
                    <label class="text-xs text-charcoal-light font-mono">"zoom"</label>
                    <input
                        type="range" min="1" max="8" step="0.5"
                        prop:value=move || zoom().to_string()
                        on:input=move |e| {
                            if let Ok(v) = event_target_value(&e).parse::<f32>() {
                                set_zoom(v);
                            }
                        }
                        class="w-24 accent-accent"
                    />
                    <span class="text-xs text-charcoal-light font-mono w-8">
                        {move || format!("{:.1}x", zoom())}
                    </span>
                </div>
            </div>

            <div class="text-sm text-charcoal-light max-w-4xl mx-auto">
                "Pathfinding algorithms racing from start (green) to end (amber). Rendered via WebGL2."
            </div>

            // Blind Search
            <div class="w-full flex flex-col gap-6 mb-8">
                <div class="flex flex-col gap-2 items-center">
                    <h2 class="text-2xl font-bold text-charcoal">"Blind Search"</h2>
                    <p class="text-sm text-charcoal-light max-w-2xl text-center">
                        "Explores without knowing the destination location"
                    </p>
                </div>
                <div class="w-full flex flex-wrap gap-8 justify-center items-start">
                    <AlgorithmSimulation algorithm=Algorithm::Bfs grid_version=grid_version
                        grid_data=gd1 is_running=is_running zoom=zoom
                        completion_order=blind_order set_completion_order=set_blind_order />
                    <AlgorithmSimulation algorithm=Algorithm::Dfs grid_version=grid_version
                        grid_data=gd2 is_running=is_running zoom=zoom
                        completion_order=blind_order set_completion_order=set_blind_order />
                    <AlgorithmSimulation algorithm=Algorithm::Corner grid_version=grid_version
                        grid_data=gd3 is_running=is_running zoom=zoom
                        completion_order=blind_order set_completion_order=set_blind_order />
                    <AlgorithmSimulation algorithm=Algorithm::Wall grid_version=grid_version
                        grid_data=gd4 is_running=is_running zoom=zoom
                        completion_order=blind_order set_completion_order=set_blind_order />
                    <AlgorithmSimulation algorithm=Algorithm::RandomWalk grid_version=grid_version
                        grid_data=gd5 is_running=is_running zoom=zoom
                        completion_order=blind_order set_completion_order=set_blind_order />
                </div>
            </div>

            // Informed Search
            <div class="w-full flex flex-col gap-6">
                <div class="flex flex-col gap-2 items-center">
                    <h2 class="text-2xl font-bold text-charcoal">"Informed Search"</h2>
                    <p class="text-sm text-charcoal-light max-w-2xl text-center">
                        "Uses heuristics toward the destination to guide exploration"
                    </p>
                </div>
                <div class="w-full flex flex-wrap gap-8 justify-center items-start">
                    <AlgorithmSimulation algorithm=Algorithm::AStar grid_version=grid_version
                        grid_data=gd6 is_running=is_running zoom=zoom
                        completion_order=informed_order set_completion_order=set_informed_order />
                    <AlgorithmSimulation algorithm=Algorithm::Greedy grid_version=grid_version
                        grid_data=gd7 is_running=is_running zoom=zoom
                        completion_order=informed_order set_completion_order=set_informed_order />
                </div>
            </div>
        </div>
    }
}
