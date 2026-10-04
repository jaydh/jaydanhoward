//! Shared plumbing for the WebGL widgets: DOM lookup, event listeners, the
//! animation loop, visibility, shader compilation, and canvas pan/zoom.

use std::cell::Cell;
use std::rc::Rc;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::{
    Document, Element, HtmlCanvasElement, WebGl2RenderingContext as Gl, WebGlProgram,
    WebGlShader, WebGlVertexArrayObject,
};

pub fn window() -> web_sys::Window {
    web_sys::window().expect("no window")
}

pub fn document() -> Document {
    window().document().expect("no document")
}

/// Element by id, cast to `T`. Panics if missing — widget markup is static.
pub fn by_id<T: JsCast>(id: &str) -> T {
    document()
        .get_element_by_id(id)
        .unwrap_or_else(|| panic!("#{id} missing"))
        .dyn_into::<T>()
        .unwrap_or_else(|_| panic!("#{id} has the wrong element type"))
}

/// Add a listener for the widget's lifetime (widgets are never unmounted).
pub fn on<E: JsCast + 'static>(target: &web_sys::EventTarget, event: &str, mut f: impl FnMut(E) + 'static) {
    on_with(target, event, false, move |e: web_sys::Event| f(e.unchecked_into()));
}

/// Like [`on`], but `passive: false`, for handlers that `preventDefault()` (wheel).
pub fn on_active<E: JsCast + 'static>(target: &web_sys::EventTarget, event: &str, mut f: impl FnMut(E) + 'static) {
    on_with(target, event, true, move |e: web_sys::Event| f(e.unchecked_into()));
}

fn on_with(target: &web_sys::EventTarget, event: &str, non_passive: bool, f: impl FnMut(web_sys::Event) + 'static) {
    let cb = Closure::<dyn FnMut(web_sys::Event)>::new(f);
    if non_passive {
        let opts = web_sys::AddEventListenerOptions::new();
        opts.set_passive(false);
        target
            .add_event_listener_with_callback_and_add_event_listener_options(event, cb.as_ref().unchecked_ref(), &opts)
            .unwrap();
    } else {
        target.add_event_listener_with_callback(event, cb.as_ref().unchecked_ref()).unwrap();
    }
    cb.forget();
}

/// Call `frame(timestamp_ms)` on every animation frame, forever. `frame`
/// returns the delay in ms before the next frame is requested (0 = next
/// frame), so idle widgets can poll slowly.
pub fn animate(mut frame: impl FnMut(f64) -> i32 + 'static) {
    type Slot<T> = Rc<std::cell::RefCell<Option<Closure<T>>>>;
    let on_frame: Slot<dyn FnMut(f64)> = Default::default();
    let on_timeout: Slot<dyn FnMut()> = Default::default();

    let request = |slot: &Slot<dyn FnMut(f64)>| {
        let cb = slot.borrow();
        window().request_animation_frame(cb.as_ref().unwrap().as_ref().unchecked_ref()).unwrap();
    };

    {
        let on_frame = on_frame.clone();
        *on_timeout.borrow_mut() = Some(Closure::new(move || request(&on_frame)));
    }
    {
        let (again, on_timeout) = (on_frame.clone(), on_timeout.clone());
        *on_frame.borrow_mut() = Some(Closure::new(move |ts: f64| {
            let delay = frame(ts);
            if delay <= 0 {
                request(&again);
            } else {
                let cb = on_timeout.borrow();
                window()
                    .set_timeout_with_callback_and_timeout_and_arguments_0(cb.as_ref().unwrap().as_ref().unchecked_ref(), delay)
                    .unwrap();
            }
        }));
    }
    request(&on_frame);
}

/// Call `f(is_visible)` whenever `el` crosses 10% visibility.
pub fn on_visibility(el: &Element, mut f: impl FnMut(bool) + 'static) {
    let cb = Closure::<dyn FnMut(js_sys::Array)>::new(move |entries: js_sys::Array| {
        for e in entries.iter() {
            f(e.unchecked_into::<web_sys::IntersectionObserverEntry>().is_intersecting());
        }
    });
    let opts = web_sys::IntersectionObserverInit::new();
    opts.set_threshold(&JsValue::from_f64(0.1));
    web_sys::IntersectionObserver::new_with_options(cb.as_ref().unchecked_ref(), &opts)
        .unwrap()
        .observe(el);
    cb.forget();
}

/// Whether the site's dark theme is on (the `dark` class on `<html>`).
pub fn dark_mode() -> bool {
    document().document_element().is_some_and(|h| h.class_list().contains("dark"))
}

pub fn webgl2(canvas: &HtmlCanvasElement) -> Result<Gl, String> {
    canvas
        .get_context("webgl2")
        .map_err(|_| "getContext failed".to_string())?
        .ok_or("no webgl2")?
        .dyn_into::<Gl>()
        .map_err(|_| "not a WebGL2 context".to_string())
}

fn compile(gl: &Gl, kind: u32, src: &str) -> Result<WebGlShader, String> {
    let s = gl.create_shader(kind).ok_or("createShader failed")?;
    gl.shader_source(&s, src);
    gl.compile_shader(&s);
    if gl.get_shader_parameter(&s, Gl::COMPILE_STATUS).as_bool() == Some(true) {
        Ok(s)
    } else {
        Err(gl.get_shader_info_log(&s).unwrap_or_default())
    }
}

pub fn link_program(gl: &Gl, vert: &str, frag: &str) -> Result<WebGlProgram, String> {
    let vs = compile(gl, Gl::VERTEX_SHADER, vert)?;
    let fs = compile(gl, Gl::FRAGMENT_SHADER, frag)?;
    let p = gl.create_program().ok_or("createProgram failed")?;
    gl.attach_shader(&p, &vs);
    gl.attach_shader(&p, &fs);
    gl.link_program(&p);
    if gl.get_program_parameter(&p, Gl::LINK_STATUS).as_bool() == Some(true) {
        Ok(p)
    } else {
        Err(gl.get_program_info_log(&p).unwrap_or_default())
    }
}

/// VAO for a full-screen quad (two triangles) bound to `prog`'s `a_pos`.
pub fn fullscreen_quad(gl: &Gl, prog: &WebGlProgram) -> Result<WebGlVertexArrayObject, String> {
    let vao = gl.create_vertex_array().ok_or("createVertexArray failed")?;
    gl.bind_vertex_array(Some(&vao));
    let buf = gl.create_buffer().ok_or("createBuffer failed")?;
    gl.bind_buffer(Gl::ARRAY_BUFFER, Some(&buf));
    let verts: [f32; 12] = [-1., -1., 1., -1., -1., 1., -1., 1., 1., -1., 1., 1.];
    gl.buffer_data_with_array_buffer_view(Gl::ARRAY_BUFFER, &js_sys::Float32Array::from(&verts[..]), Gl::STATIC_DRAW);
    let loc = gl.get_attrib_location(prog, "a_pos") as u32;
    gl.enable_vertex_attrib_array(loc);
    gl.vertex_attrib_pointer_with_i32(loc, 2, Gl::FLOAT, false, 0, 0);
    gl.bind_vertex_array(None);
    Ok(vao)
}

/// Shared zoom level + per-canvas zoom center, as used by [`attach_canvas_nav`].
pub struct NavHandles {
    pub zoom: Rc<Cell<f64>>,
    pub center: Rc<Cell<(f64, f64)>>,
    /// Called after a wheel zoom (to sync a slider/label).
    pub on_zoom: Rc<dyn Fn(f64)>,
    /// Called after any pan/zoom (to request a redraw).
    pub on_change: Rc<dyn Fn()>,
}

/// Drag to pan, wheel to zoom about the cursor (1x–16x), and optionally
/// shift-click to re-center. Coordinates are in [0,1] canvas space with the
/// zoom center as the fixed point, matching the draw shaders.
pub fn attach_canvas_nav(canvas: &HtmlCanvasElement, nav: NavHandles, shift_click_recenter: bool) {
    /// Active drag: last pointer x/y and the canvas rect's width/height.
    type Drag = Option<(f64, f64, f64, f64)>;
    let drag: Rc<Cell<Drag>> = Rc::new(Cell::new(None));
    let target: &web_sys::EventTarget = canvas.as_ref();

    {
        let (drag, canvas) = (drag.clone(), canvas.clone());
        on(target, "pointerdown", move |e: web_sys::PointerEvent| {
            let r = canvas.get_bounding_client_rect();
            drag.set(Some((e.client_x() as f64, e.client_y() as f64, r.width(), r.height())));
            let _ = canvas.set_pointer_capture(e.pointer_id());
            e.prevent_default();
        });
    }
    {
        let (drag, zoom, center, changed) = (drag.clone(), nav.zoom.clone(), nav.center.clone(), nav.on_change.clone());
        on(target, "pointermove", move |e: web_sys::PointerEvent| {
            let Some((lx, ly, w, h)) = drag.get() else { return };
            let (x, y) = (e.client_x() as f64, e.client_y() as f64);
            let (dx, dy) = ((x - lx) / w, (y - ly) / h);
            drag.set(Some((x, y, w, h)));
            let z = zoom.get();
            let (cx, cy) = center.get();
            center.set((cx - dx / z, cy + dy / z));
            changed();
        });
    }
    {
        let drag = drag.clone();
        on(target, "pointerup", move |_: web_sys::PointerEvent| drag.set(None));
    }
    {
        let (canvas, zoom, center, on_zoom, changed) =
            (canvas.clone(), nav.zoom.clone(), nav.center.clone(), nav.on_zoom.clone(), nav.on_change.clone());
        on_active(target, "wheel", move |e: web_sys::WheelEvent| {
            e.prevent_default();
            let r = canvas.get_bounding_client_rect();
            let sx = (e.client_x() as f64 - r.left()) / r.width();
            let sy = (e.client_y() as f64 - r.top()) / r.height();
            let old = zoom.get();
            let new = (old * if e.delta_y() > 0.0 { 1.0 / 1.15 } else { 1.15 }).clamp(1.0, 16.0);
            let (cx, cy) = center.get();
            let (wx, wy) = ((sx - cx) / old + cx, (sy - cy) / old + cy);
            let fixed = |w: f64, s: f64| if (new - 1.0).abs() > 1e-4 { (new * w - s) / (new - 1.0) } else { 0.5 };
            zoom.set(new);
            center.set((fixed(wx, sx), fixed(wy, sy)));
            on_zoom(new);
            changed();
        });
    }
    if shift_click_recenter {
        let (canvas, zoom, center, changed) = (canvas.clone(), nav.zoom.clone(), nav.center.clone(), nav.on_change.clone());
        on(target, "click", move |e: web_sys::MouseEvent| {
            if !e.shift_key() {
                return;
            }
            let r = canvas.get_bounding_client_rect();
            let (px, py) = ((e.client_x() as f64 - r.left()) / r.width(), (e.client_y() as f64 - r.top()) / r.height());
            let (ocx, ocy) = center.get();
            let z = zoom.get();
            center.set(((px - ocx) / z + ocx, (py - ocy) / z + ocy));
            changed();
        });
    }
}
