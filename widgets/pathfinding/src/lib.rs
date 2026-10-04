//! Pathfinding race (Foster widget): 7 algorithms over one shared random
//! obstacle grid, each drawn to its own WebGL2 canvas from an R8 state
//! texture. Run/pause, randomize, zoom/pan and follow are per-visitor UI
//! state, all handled here. Foster mounts this on `#pathfinding-widget`
//! (`fx-widget`); the grid and GL contexts are built the first time it's
//! actually on screen.

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use wasm_bindgen::prelude::*;
use web_sys::{
    Element, HtmlCanvasElement, HtmlElement, HtmlInputElement, WebGl2RenderingContext as Gl,
    WebGlProgram, WebGlTexture, WebGlVertexArrayObject,
};
use widget_common::*;

// Panels render at ~280px; 256 keeps per-cell detail above what they can
// show while keeping the per-step queue work small.
const GRID: usize = 256;
const OBSTACLE_PROB: f64 = 0.2;
const STEPS_PER_FRAME: usize = 5;

const OBSTACLE: u8 = 0;
const UNVISITED: u8 = 1;
const FRONTIER: u8 = 2;
const VISITED: u8 = 3;
const PATH: u8 = 4;
const NO_PARENT: u32 = u32::MAX;

#[derive(Clone, Copy, PartialEq)]
enum Algo { Bfs, Dfs, Corner, Wall, RandomWalk, AStar, Greedy }

impl Algo {
    fn from_attr(s: &str) -> Option<Self> {
        Some(match s {
            "bfs" => Self::Bfs, "dfs" => Self::Dfs, "corner" => Self::Corner, "wall" => Self::Wall,
            "randomwalk" => Self::RandomWalk, "astar" => Self::AStar, "greedy" => Self::Greedy,
            _ => return None,
        })
    }
    fn informed(self) -> bool {
        matches!(self, Self::AStar | Self::Greedy)
    }
}

const VERT: &str = r#"#version 300 es
in vec2 a_pos;
out vec2 v_uv;
void main() {
    v_uv = a_pos * 0.5 + 0.5;
    gl_Position = vec4(a_pos, 0.0, 1.0);
}"#;

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
}"#;

struct PathRenderer {
    gl: Gl,
    prog: WebGlProgram,
    vao: WebGlVertexArrayObject,
    tex: WebGlTexture,
}

impl PathRenderer {
    fn new(canvas: &HtmlCanvasElement) -> Result<Self, String> {
        let gl = webgl2(canvas)?;
        let prog = link_program(&gl, VERT, DRAW_FRAG)?;
        let vao = fullscreen_quad(&gl, &prog)?;
        let tex = gl.create_texture().ok_or("createTexture failed")?;
        gl.bind_texture(Gl::TEXTURE_2D, Some(&tex));
        for (k, v) in [
            (Gl::TEXTURE_MIN_FILTER, Gl::NEAREST), (Gl::TEXTURE_MAG_FILTER, Gl::NEAREST),
            (Gl::TEXTURE_WRAP_S, Gl::CLAMP_TO_EDGE), (Gl::TEXTURE_WRAP_T, Gl::CLAMP_TO_EDGE),
        ] {
            gl.tex_parameteri(Gl::TEXTURE_2D, k, v as i32);
        }
        gl.bind_texture(Gl::TEXTURE_2D, None);
        Ok(Self { gl, prog, vao, tex })
    }

    fn upload(&self, state: &[u8]) {
        self.gl.bind_texture(Gl::TEXTURE_2D, Some(&self.tex));
        let _ = self.gl.tex_image_2d_with_i32_and_i32_and_i32_and_format_and_type_and_opt_u8_array(
            Gl::TEXTURE_2D, 0, Gl::R8 as i32, GRID as i32, GRID as i32, 0, Gl::RED, Gl::UNSIGNED_BYTE, Some(state),
        );
        self.gl.bind_texture(Gl::TEXTURE_2D, None);
    }

    #[allow(clippy::too_many_arguments)]
    fn draw(&self, w: i32, h: i32, dark: bool, start: (usize, usize), end: (usize, usize), zoom: f64, (zx, zy): (f64, f64)) {
        let gl = &self.gl;
        gl.viewport(0, 0, w, h);
        gl.use_program(Some(&self.prog));
        gl.active_texture(Gl::TEXTURE0);
        gl.bind_texture(Gl::TEXTURE_2D, Some(&self.tex));
        let u = |n: &str| gl.get_uniform_location(&self.prog, n);
        gl.uniform1i(u("u_state").as_ref(), 0);
        let (vis, bg, wall) = if dark {
            ([0.376, 0.647, 0.980], [0.067, 0.094, 0.153], [0.310, 0.400, 0.502])
        } else {
            ([0.231, 0.510, 0.965], [0.973, 0.980, 0.988], [0.180, 0.224, 0.286])
        };
        gl.uniform3f(u("u_visited").as_ref(), vis[0], vis[1], vis[2]);
        gl.uniform3f(u("u_bg").as_ref(), bg[0], bg[1], bg[2]);
        gl.uniform3f(u("u_wall").as_ref(), wall[0], wall[1], wall[2]);
        let g = GRID as f32;
        gl.uniform2f(u("u_start").as_ref(), start.0 as f32 / g, start.1 as f32 / g);
        gl.uniform2f(u("u_end").as_ref(), end.0 as f32 / g, end.1 as f32 / g);
        gl.uniform2f(u("u_res").as_ref(), g, g);
        gl.uniform1f(u("u_zoom").as_ref(), zoom.max(0.01) as f32);
        gl.uniform2f(u("u_zoom_center").as_ref(), zx as f32, zy as f32);
        gl.bind_vertex_array(Some(&self.vao));
        gl.draw_arrays(Gl::TRIANGLES, 0, 6);
        gl.bind_vertex_array(None);
    }
}

fn rand_below(n: usize) -> usize {
    ((js_sys::Math::random() * n as f64) as usize).min(n - 1)
}

struct Grid {
    base: Vec<u8>,
    start: (usize, usize),
    end: (usize, usize),
}

fn make_grid() -> Grid {
    loop {
        let mut base = vec![UNVISITED; GRID * GRID];
        let mut passable = Vec::new();
        for (i, cell) in base.iter_mut().enumerate() {
            if js_sys::Math::random() < OBSTACLE_PROB {
                *cell = OBSTACLE;
            } else {
                passable.push(i);
            }
        }
        if passable.len() < 2 {
            continue;
        }
        let si = rand_below(passable.len());
        let mut ei = rand_below(passable.len());
        while ei == si {
            ei = rand_below(passable.len());
        }
        let xy = |i: usize| (i % GRID, i / GRID);
        return Grid { start: xy(passable[si]), end: xy(passable[ei]), base };
    }
}

/// One algorithm's search over its own copy of the grid. Same step logic as
/// the original: one cell expanded per step; BFS is a FIFO (head pointer),
/// everything else pops from the back of `queue`.
struct AlgoRun {
    state: Vec<u8>,
    parent: Vec<u32>,
    queue: Vec<usize>,
    head: usize,
    start: (usize, usize),
    end: (usize, usize),
    initialized: bool,
    done: bool,
    steps: u32,
    completion_steps: Option<u32>,
    current: (usize, usize),
}

impl AlgoRun {
    fn new(g: &Grid) -> Self {
        Self {
            state: g.base.clone(), parent: vec![NO_PARENT; GRID * GRID], queue: Vec::new(), head: 0,
            start: g.start, end: g.end, initialized: false, done: false, steps: 0,
            completion_steps: None, current: g.start,
        }
    }

    fn idx((x, y): (usize, usize)) -> usize {
        y * GRID + x
    }

    fn neighbors(i: usize) -> impl Iterator<Item = usize> {
        let (x, y) = (i % GRID, i / GRID);
        [
            (x > 0).then(|| i - 1),
            (x + 1 < GRID).then(|| i + 1),
            (y > 0).then(|| i - GRID),
            (y + 1 < GRID).then(|| i + GRID),
        ]
        .into_iter()
        .flatten()
    }

    fn manhattan(a: usize, b: usize) -> usize {
        (a % GRID).abs_diff(b % GRID) + (a / GRID).abs_diff(b / GRID)
    }

    fn wall_dist(i: usize) -> usize {
        let (x, y) = (i % GRID, i / GRID);
        x.min(GRID - 1 - x).min(y).min(GRID - 1 - y)
    }

    fn corner_dist(i: usize) -> usize {
        [0, GRID - 1, GRID * (GRID - 1), GRID * GRID - 1].into_iter().map(|c| Self::manhattan(i, c)).min().unwrap()
    }

    fn step(&mut self, algo: Algo) {
        if self.done {
            return;
        }
        self.steps += 1;
        let (si, ei) = (Self::idx(self.start), Self::idx(self.end));

        if !self.initialized {
            self.initialized = true;
            self.state[si] = FRONTIER;
            self.queue.push(si);
            return;
        }

        let current = loop {
            let c = if algo == Algo::Bfs {
                if self.head >= self.queue.len() {
                    self.done = true;
                    return;
                }
                self.head += 1;
                self.queue[self.head - 1]
            } else {
                match self.queue.pop() {
                    Some(c) => c,
                    None => {
                        self.done = true;
                        return;
                    }
                }
            };
            if !matches!(self.state[c], VISITED | PATH) {
                break c;
            }
        };
        self.current = (current % GRID, current / GRID);

        self.state[current] = VISITED;
        if current == ei {
            self.completion_steps = Some(self.steps);
            self.done = true;
            self.reconstruct_path(si, ei);
            return;
        }

        let mut viable: Vec<usize> = Self::neighbors(current).filter(|&n| self.state[n] == UNVISITED).collect();
        for &n in &viable {
            self.state[n] = FRONTIER;
            self.parent[n] = current as u32;
        }

        // Sorting descending puts the preferred cell last, i.e. popped next.
        match algo {
            Algo::Bfs | Algo::Dfs => {}
            Algo::AStar | Algo::Greedy => {
                self.queue.extend(&viable);
                // Re-sort the whole remaining queue so the globally
                // closest-to-end cell is always popped next.
                let h = self.head;
                self.queue[h..].sort_by_key(|&c| std::cmp::Reverse(Self::manhattan(c, ei)));
                return;
            }
            Algo::Corner => viable.sort_by_key(|&c| std::cmp::Reverse(Self::corner_dist(c))),
            Algo::Wall => viable.sort_by_key(|&c| std::cmp::Reverse(Self::wall_dist(c))),
            Algo::RandomWalk => {
                for i in (1..viable.len()).rev() {
                    viable.swap(i, rand_below(i + 1));
                }
            }
        }
        self.queue.extend(viable);
    }

    fn reconstruct_path(&mut self, si: usize, ei: usize) {
        let mut curr = ei;
        for _ in 0..GRID * 8 {
            self.state[curr] = PATH;
            if curr == si || self.parent[curr] == NO_PARENT {
                break;
            }
            curr = self.parent[curr] as usize;
        }
    }
}

struct Panel {
    algo: Algo,
    canvas: HtmlCanvasElement,
    meta: HtmlElement,
    renderer: Option<PathRenderer>,
    run: Option<AlgoRun>,
    center: Rc<Cell<(f64, f64)>>,
    frame_count: usize,
    fps: usize,
}

struct State {
    panels: Vec<Panel>,
    running: bool,
    visible: bool,
    started: bool,
    following: bool,
    dirty: bool,
    blind_order: Vec<Algo>,
    informed_order: Vec<Algo>,
    last_fps_tick: f64,
}

fn rank_label(pos: usize) -> String {
    match pos {
        0 => "🥇 1st".into(),
        1 => "🥈 2nd".into(),
        2 => "🥉 3rd".into(),
        n => format!("{}th", n + 1),
    }
}

fn set_zoom_ui(z: f64) {
    by_id::<HtmlInputElement>("pf-zoom-slider").set_value(&z.to_string());
    by_id::<HtmlElement>("pf-zoom-label").set_text_content(Some(&format!("{z:.1}x")));
}

impl State {
    /// New grid and fresh runs; creates each panel's renderer on first use.
    /// Returns which panels got a new renderer (they still need canvas nav).
    fn regenerate(&mut self, zoom: &Cell<f64>) -> Vec<bool> {
        let grid = make_grid();
        self.blind_order.clear();
        self.informed_order.clear();
        zoom.set(1.0);
        set_zoom_ui(1.0);
        let g = GRID as f64;
        let center = ((grid.start.0 as f64 + grid.end.0 as f64) / g * 0.5, (grid.start.1 as f64 + grid.end.1 as f64) / g * 0.5);
        let mut fresh = Vec::with_capacity(self.panels.len());
        for p in &mut self.panels {
            fresh.push(p.renderer.is_none());
            p.run = Some(AlgoRun::new(&grid));
            p.frame_count = 0;
            p.fps = 0;
            p.center.set(center);
            if p.renderer.is_none() {
                p.canvas.set_width(p.canvas.client_width().max(1) as u32);
                p.canvas.set_height(p.canvas.client_height().max(1) as u32);
                match PathRenderer::new(&p.canvas) {
                    Ok(r) => p.renderer = Some(r),
                    Err(e) => web_sys::console::error_1(&format!("pathfinding WebGL init: {e}").into()),
                }
            }
        }
        self.dirty = true;
        fresh
    }
}

/// Called by Foster with the `#pathfinding-widget` element.
#[wasm_bindgen]
pub fn mount(root: Element) {
    let panels: Vec<Panel> = {
        let list = root.query_selector_all(".pf-panel[data-algo]").unwrap();
        (0..list.length())
            .filter_map(|i| {
                let el: Element = list.item(i)?.unchecked_into();
                Some(Panel {
                    algo: Algo::from_attr(&el.get_attribute("data-algo")?)?,
                    canvas: el.query_selector("canvas").ok()??.unchecked_into(),
                    meta: el.query_selector(".pf-meta").ok()??.unchecked_into(),
                    renderer: None, run: None, center: Rc::new(Cell::new((0.5, 0.5))), frame_count: 0, fps: 0,
                })
            })
            .collect()
    };
    let st = Rc::new(RefCell::new(State {
        panels, running: false, visible: false, started: false, following: false, dirty: true,
        blind_order: Vec::new(), informed_order: Vec::new(), last_fps_tick: 0.0,
    }));
    // One zoom level shared by all panels (each keeps its own center).
    let zoom = Rc::new(Cell::new(1.0));
    let mark_dirty: Rc<dyn Fn()> = {
        let st = Rc::downgrade(&st);
        Rc::new(move || {
            if let Some(st) = st.upgrade() {
                if let Ok(mut s) = st.try_borrow_mut() {
                    s.dirty = true;
                }
            }
        })
    };

    let toggle: HtmlElement = by_id("pf-toggle-run");
    let play_label: HtmlElement = toggle.query_selector(".pf-play-label").unwrap().unwrap().unchecked_into();
    let pause_label: HtmlElement = toggle.query_selector(".pf-pause-label").unwrap().unwrap().unchecked_into();
    let sync_toggle = {
        let st = st.clone();
        Rc::new(move || {
            let running = st.borrow().running;
            let _ = play_label.style().set_property("display", if running { "none" } else { "" });
            let _ = pause_label.style().set_property("display", if running { "" } else { "none" });
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
    let regenerate = {
        let (st, zoom, dirty) = (st.clone(), zoom.clone(), mark_dirty.clone());
        Rc::new(move || {
            let fresh = st.borrow_mut().regenerate(&zoom);
            // Pan/zoom for newly created panels: one shared zoom, each panel
            // its own center.
            let s = st.borrow();
            for (p, new) in s.panels.iter().zip(fresh) {
                if new {
                    let dirty = dirty.clone();
                    attach_canvas_nav(
                        &p.canvas,
                        NavHandles {
                            zoom: zoom.clone(),
                            center: p.center.clone(),
                            on_zoom: Rc::new(set_zoom_ui),
                            on_change: Rc::new(move || dirty()),
                        },
                        false,
                    );
                }
            }
        })
    };
    {
        let regenerate = regenerate.clone();
        on(&by_id::<Element>("pf-reset"), "click", move |_: web_sys::Event| regenerate());
    }
    {
        let (zoom, dirty) = (zoom.clone(), mark_dirty.clone());
        on(&by_id::<Element>("pf-zoom-slider"), "input", move |e: web_sys::Event| {
            let z: f64 = e.target().unwrap().unchecked_into::<HtmlInputElement>().value().parse().unwrap_or(1.0);
            zoom.set(z);
            by_id::<HtmlElement>("pf-zoom-label").set_text_content(Some(&format!("{z:.1}x")));
            dirty();
        });
    }
    {
        let (st, zoom) = (st.clone(), zoom.clone());
        let follow: HtmlElement = by_id("pf-follow");
        let btn = follow.clone();
        on(&follow, "click", move |_: web_sys::Event| {
            let mut s = st.borrow_mut();
            s.following = !s.following;
            let _ = btn.class_list().toggle_with_force("active", s.following);
            btn.set_text_content(Some(if s.following { "Following" } else { "Follow" }));
            if s.following {
                zoom.set(4.0);
                set_zoom_ui(4.0);
            }
            s.dirty = true;
        });
    }

    {
        let (st, sync, regenerate) = (st.clone(), sync_toggle.clone(), regenerate.clone());
        on_visibility(&root, move |visible| {
            let first = {
                let mut s = st.borrow_mut();
                s.visible = visible;
                s.running = visible;
                if visible {
                    s.dirty = true;
                }
                let first = visible && !s.started;
                s.started |= first;
                first
            };
            if first {
                regenerate();
            }
            sync();
        });
    }

    animate(move |now| {
        let mut s = st.borrow_mut();
        if !s.visible || !s.started {
            return 250; // off screen: poll cheaply until visible again
        }
        let dark = dark_mode();
        let fps_window = now - s.last_fps_tick >= 1000.0;
        let running = s.running;
        let following = s.following;
        let stepping = running && s.panels.iter().any(|p| p.run.as_ref().is_some_and(|r| !r.done));
        let dirty = s.dirty;
        let z = zoom.get();

        let State { panels, blind_order, informed_order, .. } = &mut *s;
        for p in panels.iter_mut() {
            let (Some(run), Some(renderer)) = (p.run.as_mut(), p.renderer.as_ref()) else { continue };
            let mut stepped = false;
            if running && !run.done {
                for _ in 0..STEPS_PER_FRAME {
                    run.step(p.algo);
                }
                stepped = true;
                p.frame_count += STEPS_PER_FRAME;
                if run.done && run.completion_steps.is_some() {
                    let order = if p.algo.informed() { &mut *informed_order } else { &mut *blind_order };
                    if !order.contains(&p.algo) {
                        order.push(p.algo);
                    }
                }
                if following && !run.done {
                    let g = GRID as f64;
                    p.center.set((run.current.0 as f64 / g, run.current.1 as f64 / g));
                }
            }

            let (cw, ch) = (p.canvas.client_width().max(1), p.canvas.client_height().max(1));
            let resized = p.canvas.width() != cw as u32 || p.canvas.height() != ch as u32;
            if resized {
                p.canvas.set_width(cw as u32);
                p.canvas.set_height(ch as u32);
            }
            if stepped || resized || dirty {
                renderer.upload(&run.state);
                renderer.draw(cw, ch, dark, run.start, run.end, z, p.center.get());
            }
            if fps_window {
                p.fps = p.frame_count;
                p.frame_count = 0;
            }

            let order = if p.algo.informed() { &*informed_order } else { &*blind_order };
            let rank = order.iter().position(|a| *a == p.algo).map(|i| format!(r#"<span class="pf-rank">{}</span>"#, rank_label(i)));
            let steps = match run.completion_steps {
                Some(n) => format!("{n} steps"),
                None if running && p.fps > 0 => format!("{} steps/s", p.fps),
                None => String::new(),
            };
            let sep = if rank.is_some() && !steps.is_empty() { " &middot; " } else { "" };
            p.meta.set_inner_html(&format!("{}{sep}{steps}", rank.unwrap_or_default()));
        }

        if fps_window {
            s.last_fps_tick = now;
        }
        // Nothing animating: keep a cheap low-rate redraw so theme changes
        // and pans still show up.
        if stepping {
            s.dirty = false;
            0
        } else {
            s.dirty = true;
            250
        }
    });
}
