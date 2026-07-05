#![allow(clippy::all)]
//! Shared drag-to-pan + scroll-to-zoom wiring for the WebGL grid canvases
//! (Game of Life, Pathfinding). Both canvases zoom toward the cursor and
//! pan by the same normalised-grid-coordinate math; this is that logic
//! factored into one place instead of being duplicated per component.

use leptos::prelude::*;

#[cfg(not(feature = "ssr"))]
pub fn attach_canvas_nav(
    canvas_ref: NodeRef<leptos::html::Canvas>,
    zoom: ReadSignal<f32>,
    set_zoom: WriteSignal<f32>,
    zoom_center: ReadSignal<(f32, f32)>,
    set_zoom_center: WriteSignal<(f32, f32)>,
    is_navigate: impl Fn() -> bool + Clone + 'static,
    on_paint: Option<Box<dyn Fn(f32, f32, f32, f32)>>,
) {
    use std::cell::Cell;
    use std::rc::Rc;
    use wasm_bindgen::prelude::*;
    use wasm_bindgen::JsCast;

    let attached: Rc<Cell<bool>> = Rc::new(Cell::new(false));
    let on_paint: Rc<Option<Box<dyn Fn(f32, f32, f32, f32)>>> = Rc::new(on_paint);

    Effect::new(move |_| {
        let Some(canvas) = canvas_ref.get() else { return; };
        if attached.get() { return; }
        attached.set(true);

        let el: web_sys::HtmlCanvasElement = {
            let r: &web_sys::HtmlCanvasElement = canvas.as_ref();
            r.clone()
        };

        let painting: Rc<Cell<bool>> = Rc::new(Cell::new(false));
        let dragging: Rc<Cell<bool>> = Rc::new(Cell::new(false));
        let drag_last: Rc<Cell<(f32, f32)>> = Rc::new(Cell::new((0.0, 0.0)));
        let drag_w: Rc<Cell<f32>> = Rc::new(Cell::new(1.0));
        let drag_h: Rc<Cell<f32>> = Rc::new(Cell::new(1.0));

        // pointerdown
        {
            let el2 = el.clone();
            let painting = painting.clone();
            let dragging = dragging.clone();
            let drag_last = drag_last.clone();
            let drag_w = drag_w.clone();
            let drag_h = drag_h.clone();
            let is_navigate = is_navigate.clone();
            let on_paint = on_paint.clone();
            let cb = Closure::<dyn FnMut(_)>::new(move |e: web_sys::PointerEvent| {
                let rect = el2.get_bounding_client_rect();
                if is_navigate() {
                    drag_w.set(rect.width() as f32);
                    drag_h.set(rect.height() as f32);
                    dragging.set(true);
                    drag_last.set((e.client_x() as f32, e.client_y() as f32));
                    let _ = el2.set_pointer_capture(e.pointer_id());
                    e.prevent_default();
                } else {
                    painting.set(true);
                    if let Some(ref paint) = *on_paint {
                        let x = e.client_x() as f32 - rect.left() as f32;
                        let y = e.client_y() as f32 - rect.top() as f32;
                        paint(x, y, rect.width() as f32, rect.height() as f32);
                    }
                }
            });
            el.add_event_listener_with_callback("pointerdown", cb.as_ref().unchecked_ref()).ok();
            cb.forget();
        }

        // pointermove
        {
            let el2 = el.clone();
            let painting = painting.clone();
            let dragging = dragging.clone();
            let drag_last = drag_last.clone();
            let drag_w = drag_w.clone();
            let drag_h = drag_h.clone();
            let is_navigate = is_navigate.clone();
            let on_paint = on_paint.clone();
            let cb = Closure::<dyn FnMut(_)>::new(move |e: web_sys::PointerEvent| {
                if is_navigate() {
                    if !dragging.get() { return; }
                    let (lx, ly) = drag_last.get();
                    let cx = e.client_x() as f32;
                    let cy = e.client_y() as f32;
                    let dx = (cx - lx) / drag_w.get();
                    let dy = (cy - ly) / drag_h.get();
                    drag_last.set((cx, cy));
                    let z = zoom.get_untracked();
                    set_zoom_center.update(|(ocx, ocy)| {
                        *ocx -= dx / z;
                        *ocy += dy / z;
                    });
                } else {
                    if !painting.get() { return; }
                    if let Some(ref paint) = *on_paint {
                        let rect = el2.get_bounding_client_rect();
                        let x = e.client_x() as f32 - rect.left() as f32;
                        let y = e.client_y() as f32 - rect.top() as f32;
                        paint(x, y, rect.width() as f32, rect.height() as f32);
                    }
                }
            });
            el.add_event_listener_with_callback("pointermove", cb.as_ref().unchecked_ref()).ok();
            cb.forget();
        }

        // pointerup
        {
            let painting = painting.clone();
            let dragging = dragging.clone();
            let cb = Closure::<dyn FnMut(_)>::new(move |_: web_sys::PointerEvent| {
                painting.set(false);
                dragging.set(false);
            });
            el.add_event_listener_with_callback("pointerup", cb.as_ref().unchecked_ref()).ok();
            cb.forget();
        }

        // wheel – zoom toward cursor (works regardless of navigate/paint mode)
        {
            let el2 = el.clone();
            let cb = Closure::<dyn FnMut(_)>::new(move |e: web_sys::WheelEvent| {
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
            });
            el.add_event_listener_with_callback("wheel", cb.as_ref().unchecked_ref()).ok();
            cb.forget();
        }
    });
}
