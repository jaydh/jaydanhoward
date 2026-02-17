use crate::components::icons::Icon;
use crate::components::source_anchor::SourceAnchor;
use leptos::prelude::*;
use std::collections::HashSet;

#[cfg(not(feature = "ssr"))]
use rand::Rng;

#[cfg(not(feature = "ssr"))]
use serde::{Deserialize, Serialize};

#[cfg(not(feature = "ssr"))]
#[derive(Serialize, Deserialize, Clone, Debug)]
struct WorkerRequest {
    alive_cells: Vec<(i32, i32)>,
    grid_size: u32,
}

#[cfg(not(feature = "ssr"))]
#[derive(Serialize, Deserialize, Clone, Debug)]
struct WorkerResponse {
    alive_cells: Vec<(i32, i32)>,
}

#[derive(Clone, Default)]
struct AliveCells(HashSet<(i32, i32)>);

#[cfg(not(feature = "ssr"))]
fn calculate_next(
    cells: ReadSignal<AliveCells>,
    set_cells: WriteSignal<AliveCells>,
    grid_size: u32,
) {
    let current_alive = &cells.get_untracked().0;
    let next_alive = calculate_next_generation_pure(current_alive, grid_size);
    set_cells(AliveCells(next_alive));
}

// Pure function for calculation (can be called from worker or main thread)
#[cfg(not(feature = "ssr"))]
fn calculate_next_generation_pure(
    current_alive: &HashSet<(i32, i32)>,
    grid_size: u32,
) -> HashSet<(i32, i32)> {
    let mut neighbor_counts: std::collections::HashMap<(i32, i32), i32> =
        std::collections::HashMap::new();

    // Count neighbors for all cells adjacent to alive cells
    for &(x, y) in current_alive.iter() {
        for dy in -1..=1 {
            for dx in -1..=1 {
                if dx == 0 && dy == 0 {
                    continue;
                }
                let neighbor_x = x + dx;
                let neighbor_y = y + dy;

                if neighbor_x >= 0
                    && neighbor_y >= 0
                    && neighbor_x < grid_size as i32
                    && neighbor_y < grid_size as i32
                {
                    *neighbor_counts.entry((neighbor_x, neighbor_y)).or_insert(0) += 1;
                }
            }
        }
    }

    // Apply Game of Life rules
    let mut next_alive = HashSet::new();
    for (&pos, &count) in neighbor_counts.iter() {
        let is_alive = current_alive.contains(&pos);
        if count == 3 || (is_alive && count == 2) {
            next_alive.insert(pos);
        }
    }

    next_alive
}

// Export for use in Web Worker
#[cfg(not(feature = "ssr"))]
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn life_worker_calculate(request_json: &str) -> String {
    let request: WorkerRequest = serde_json::from_str(request_json).unwrap();
    let current_alive: HashSet<(i32, i32)> = request.alive_cells.into_iter().collect();
    let next_alive = calculate_next_generation_pure(&current_alive, request.grid_size);

    let response = WorkerResponse {
        alive_cells: next_alive.into_iter().collect(),
    };

    serde_json::to_string(&response).unwrap()
}

#[cfg(not(feature = "ssr"))]
fn randomize_cells(alive_probability: f64, grid_size: u32, set_cells: WriteSignal<AliveCells>) {
    let mut rng = rand::thread_rng();
    let mut alive_cells = HashSet::new();
    for x in 0..grid_size {
        for y in 0..grid_size {
            if rng.gen::<f64>() < alive_probability {
                alive_cells.insert((x as i32, y as i32));
            }
        }
    }
    set_cells(AliveCells(alive_cells));
}

#[component]
fn Grid(
    #[allow(unused_variables)] grid_size: ReadSignal<u32>,
    #[allow(unused_variables)] cells: ReadSignal<AliveCells>,
    #[allow(unused_variables)] set_cells: WriteSignal<AliveCells>,
) -> impl IntoView {
    #[cfg(not(feature = "ssr"))]
    use leptos::html::Canvas;

    #[cfg(not(feature = "ssr"))]
    use std::rc::Rc;
    #[cfg(not(feature = "ssr"))]
    use std::cell::RefCell;

    #[cfg(not(feature = "ssr"))]
    let canvas_ref = NodeRef::<Canvas>::new();

    #[cfg(not(feature = "ssr"))]
    let (resize_trigger, set_resize_trigger) = signal(0);

    #[cfg(not(feature = "ssr"))]
    let render_pending = Rc::new(RefCell::new(false));

    // Set up window resize listener
    #[cfg(not(feature = "ssr"))]
    {
        Effect::new(move |_| {
            let handle =
                leptos::leptos_dom::helpers::window_event_listener(leptos::ev::resize, move |_| {
                    set_resize_trigger.update(|n| *n += 1);
                });
            // Store handle to keep listener alive; it auto-cleans on drop
            std::mem::forget(handle);
        });
    }

    // Optimized rendering with requestAnimationFrame batching
    #[cfg(not(feature = "ssr"))]
    Effect::new(move |_| {
        use wasm_bindgen::prelude::*;
        use wasm_bindgen::JsCast;

        // Depend on resize_trigger to re-run on window resize
        let _ = resize_trigger();
        let _ = cells(); // Track cells changes

        let Some(canvas) = canvas_ref.get() else {
            return;
        };

        // Prevent multiple pending renders
        if *render_pending.borrow() {
            return;
        }
        *render_pending.borrow_mut() = true;

        let canvas_element: &web_sys::HtmlCanvasElement = canvas.as_ref();
        let parent = canvas_element.parent_element().unwrap();
        let container_width = parent.client_width() as f64;

        // Get window inner height for better sizing
        let window = web_sys::window().unwrap();
        let window_height = window.inner_height().unwrap().as_f64().unwrap();

        // Use 90% of container width, but cap at reasonable sizes
        let max_width = container_width * 0.95;
        let max_height = window_height * 0.6; // Use 60% of viewport height

        // Use the smaller dimension to keep it square
        let max_size = max_width.min(max_height).clamp(300.0, 800.0); // Between 300-800px

        let grid = grid_size();
        let cell_px = (max_size / grid as f64).floor().max(1.0);
        let canvas_size = (cell_px * grid as f64) as u32;

        canvas.set_width(canvas_size);
        canvas.set_height(canvas_size);

        // Use requestAnimationFrame to batch rendering
        let canvas_clone = canvas.clone();
        let cells_snapshot = cells.get_untracked().0.clone();
        let render_pending_clone = render_pending.clone();

        let closure = Closure::once(Box::new(move || {
            let context = canvas_clone
                .get_context("2d")
                .unwrap()
                .unwrap()
                .unchecked_into::<web_sys::CanvasRenderingContext2d>();

            // Clear canvas
            context.set_fill_style_str("#FFFFFF");
            context.fill_rect(0.0, 0.0, canvas_size as f64, canvas_size as f64);

            // Draw alive cells - batch with beginPath/fill for better performance
            context.set_fill_style_str("#3B82F6");
            context.begin_path();
            for &(x, y) in cells_snapshot.iter() {
                context.rect(x as f64 * cell_px, y as f64 * cell_px, cell_px, cell_px);
            }
            context.fill();

            *render_pending_clone.borrow_mut() = false;
        }) as Box<dyn FnOnce()>);

        window
            .request_animation_frame(closure.as_ref().unchecked_ref())
            .unwrap();
        closure.forget();
    });

    #[cfg(not(feature = "ssr"))]
    let handle_click = move |event: web_sys::MouseEvent| {
        let Some(canvas) = canvas_ref.get() else {
            return;
        };
        let canvas_element: &web_sys::HtmlCanvasElement = canvas.as_ref();
        let rect = canvas_element.get_bounding_client_rect();
        let canvas_width = canvas_element.width() as f64;
        let grid = grid_size();
        let cell_px = canvas_width / grid as f64;

        let x = ((event.client_x() as f64 - rect.left()) / cell_px) as i32;
        let y = ((event.client_y() as f64 - rect.top()) / cell_px) as i32;

        let pos = (x, y);
        set_cells.update(|alive_cells| {
            if alive_cells.0.contains(&pos) {
                alive_cells.0.remove(&pos);
            } else {
                alive_cells.0.insert(pos);
            }
        });
    };

    #[cfg(feature = "ssr")]
    let handle_click = move |_event: leptos::ev::MouseEvent| {};

    #[cfg(not(feature = "ssr"))]
    return view! {
        <canvas
            node_ref=canvas_ref
            class="border border-border cursor-pointer"
            on:click=handle_click
        ></canvas>
    };

    #[cfg(feature = "ssr")]
    view! {
        <canvas
            class="border border-border cursor-pointer"
            on:click=handle_click
        ></canvas>
    }
}

#[component]
pub fn LifeGame(
    #[prop(optional)] initial_grid_size: Option<u32>,
    #[prop(optional)] initial_alive_probability: Option<f64>,
    #[prop(optional)] initial_interval_ms: Option<u64>,
    #[prop(default = false)]
    #[allow(unused_variables)]
    auto_start: bool,
) -> impl IntoView {
    let (cells, set_cells) = signal::<AliveCells>(AliveCells::default());
    let alive_cells = move || cells().0.len();

    let (grid_size, set_grid_size) = signal(initial_grid_size.unwrap_or(25));
    let (alive_probability, set_alive_probability) =
        signal(initial_alive_probability.unwrap_or(0.6));
    let (interval_ms, set_interval_ms) = signal(initial_interval_ms.unwrap_or(100));

    let (show_settings, set_show_settings) = signal(false);

    #[cfg(not(feature = "ssr"))]
    let (is_running, set_is_running) = signal(false);

    let container_ref = NodeRef::<leptos::html::Div>::new();

    // Animation loop using Web Worker for off-thread calculations
    #[cfg(not(feature = "ssr"))]
    {
        use leptos::leptos_dom::helpers::IntervalHandle;
        use wasm_bindgen::{prelude::*, JsCast};
        use web_sys::{Worker, MessageEvent};
        use std::rc::Rc;
        use std::cell::RefCell;

        let interval_handle: StoredValue<Option<IntervalHandle>> = StoredValue::new(None);
        let worker: Rc<RefCell<Option<Worker>>> = Rc::new(RefCell::new(None));
        let worker_ready: Rc<RefCell<bool>> = Rc::new(RefCell::new(false));
        let calculation_pending: Rc<RefCell<bool>> = Rc::new(RefCell::new(false));

        // Initialize Web Worker
        {
            let worker = worker.clone();
            let worker_ready = worker_ready.clone();
            let calculation_pending = calculation_pending.clone();

            Effect::new(move |_| {
                if worker.borrow().is_some() {
                    return;
                }

                let Ok(w) = Worker::new("/life-worker.js") else {
                    web_sys::console::error_1(&"Failed to create worker".into());
                    return;
                };

                // Set up message handler for worker responses
                let worker_ready = worker_ready.clone();
                let calculation_pending = calculation_pending.clone();

                let onmessage = Closure::wrap(Box::new(move |event: MessageEvent| {
                    let data = event.data();
                    let Ok(response) = serde_wasm_bindgen::from_value::<serde_json::Value>(data) else {
                        return;
                    };

                    match response.get("type").and_then(|t| t.as_str()) {
                        Some("ready") => {
                            *worker_ready.borrow_mut() = true;
                        }
                        Some("result") => {
                            *calculation_pending.borrow_mut() = false;
                            if let Some(alive_cells) = response.get("aliveCells") {
                                if let Ok(cells_vec) = serde_json::from_value::<Vec<(i32, i32)>>(alive_cells.clone()) {
                                    let new_cells: HashSet<(i32, i32)> = cells_vec.into_iter().collect();
                                    set_cells(AliveCells(new_cells));
                                }
                            }
                        }
                        Some("error") => {
                            *calculation_pending.borrow_mut() = false;
                            if let Some(error) = response.get("error") {
                                web_sys::console::error_1(&format!("Worker error: {}", error).into());
                            }
                        }
                        _ => {}
                    }
                }) as Box<dyn FnMut(_)>);

                w.set_onmessage(Some(onmessage.as_ref().unchecked_ref()));
                onmessage.forget();

                // Initialize worker with WASM path
                let init_msg = serde_json::json!({
                    "type": "init",
                    "wasmPath": "/jaydanhoward_wasm/jaydanhoward_wasm.js"
                });
                if let Ok(msg) = serde_wasm_bindgen::to_value(&init_msg) {
                    let _ = w.post_message(&msg);
                }

                *worker.borrow_mut() = Some(w);
            });
        }

        // Animation loop
        {
            let worker = worker.clone();
            let worker_ready = worker_ready.clone();
            let calculation_pending = calculation_pending.clone();

            Effect::new(move |_| {
                // Clear previous interval if any
                if let Some(handle) = interval_handle.get_value() {
                    handle.clear();
                }

                // Track is_running to re-run effect when it changes
                if !is_running() {
                    interval_handle.set_value(None);
                    return;
                }

                let worker = worker.clone();
                let worker_ready = worker_ready.clone();
                let calculation_pending = calculation_pending.clone();

                // Create interval that sends calculation requests to worker or falls back to sync
                let handle = leptos::leptos_dom::helpers::set_interval_with_handle(
                    move || {
                        let use_worker = *worker_ready.borrow() && worker.borrow().is_some();

                        if use_worker {
                            // Use Web Worker for off-thread calculation
                            if *calculation_pending.borrow() {
                                return; // Skip if calculation already in progress
                            }

                            let worker_guard = worker.borrow();
                            let Some(ref w) = *worker_guard else {
                                return;
                            };

                            *calculation_pending.borrow_mut() = true;

                            let current_cells = cells.get_untracked();
                            let alive_vec: Vec<(i32, i32)> = current_cells.0.iter().copied().collect();

                            let request = serde_json::json!({
                                "type": "calculate",
                                "aliveCells": alive_vec,
                                "gridSize": grid_size.get_untracked()
                            });

                            if let Ok(msg) = serde_wasm_bindgen::to_value(&request) {
                                let _ = w.post_message(&msg);
                            } else {
                                *calculation_pending.borrow_mut() = false;
                            }
                        } else {
                            // Fallback to synchronous calculation if worker not ready
                            calculate_next(cells, set_cells, grid_size.get_untracked());
                        }
                    },
                    std::time::Duration::from_millis(interval_ms.get_untracked()),
                )
                .ok();

                interval_handle.set_value(handle);
            });
        }
    }

    #[cfg(not(feature = "ssr"))]
    let start_simulation = move || {
        set_is_running(true);
    };

    #[cfg(not(feature = "ssr"))]
    let stop_simulation = move || {
        set_is_running(false);
    };

    #[cfg(not(feature = "ssr"))]
    let toggle_simulation = move || {
        set_is_running(!is_running());
    };

    // Auto-start when element comes into view
    #[cfg(not(feature = "ssr"))]
    {
        use wasm_bindgen::prelude::*;
        use wasm_bindgen::JsCast;
        use std::rc::Rc;
        use std::cell::RefCell;

        let has_started = Rc::new(RefCell::new(false));
        let initial_probability = alive_probability.get_untracked();
        let initial_grid_size = grid_size.get_untracked();

        Effect::new(move |_| {
            let Some(container) = container_ref.get() else {
                return;
            };

            let has_started = has_started.clone();

            // Create IntersectionObserver to detect when element is visible
            let callback = Closure::wrap(Box::new(move |entries: js_sys::Array, _observer: web_sys::IntersectionObserver| {
                for entry in entries.iter() {
                    let entry: web_sys::IntersectionObserverEntry = entry.unchecked_into();

                    if entry.is_intersecting() && !*has_started.borrow() {
                        *has_started.borrow_mut() = true;
                        // Randomize cells
                        randomize_cells(initial_probability, initial_grid_size, set_cells);
                        // Start simulation
                        set_is_running(true);
                    }
                }
            }) as Box<dyn FnMut(js_sys::Array, web_sys::IntersectionObserver)>);

            let observer = web_sys::IntersectionObserver::new(callback.as_ref().unchecked_ref()).unwrap();
            observer.observe(&container);

            callback.forget();
        });
    }

    #[cfg(feature = "ssr")]
    let _start_simulation = || {};
    #[cfg(feature = "ssr")]
    let _stop_simulation = || {};

    #[cfg(not(feature = "ssr"))]
    let reset = move || {
        stop_simulation();
        randomize_cells(alive_probability(), grid_size(), set_cells);
        start_simulation();
    };

    #[cfg(feature = "ssr")]
    #[allow(unused_variables)]
    let reset = || {};

    view! {
        <div
            node_ref=container_ref
            class="w-full flex flex-col gap-4 items-center relative"
        >
            <div class="flex gap-3 items-center">
                <div class="flex gap-2">
                    {
                        #[cfg(not(feature = "ssr"))]
                        {
                            view! {
                                <button
                                    class="px-4 py-1.5 text-sm rounded border transition-all duration-200 hover:bg-accent hover:bg-opacity-10"
                                    style:border-color="#3B82F6"
                                    style:color="#3B82F6"
                                    on:click=move |_| toggle_simulation()
                                    aria-label=move || if is_running() { "Pause simulation" } else { "Play simulation" }
                                >
                                    {move || if is_running() { "▌▌" } else { "▶" }}
                                </button>
                                <button
                                    class="px-4 py-1.5 text-sm rounded border transition-all duration-200 hover:bg-accent hover:bg-opacity-10"
                                    style:border-color="#3B82F6"
                                    style:color="#3B82F6"
                                    on:click=move |_| reset()
                                    aria-label="Reset simulation"
                                >
                                    "↻"
                                </button>
                                <button
                                    class="px-4 py-1.5 text-sm rounded border border-border text-charcoal hover:bg-border hover:bg-opacity-20 transition-all duration-200 flex items-center justify-center"
                                    on:click=move |_| set_show_settings.update(|v| *v = !*v)
                                    aria-label="Open settings"
                                >
                                    <Icon name="cog" class="w-4 h-4" />
                                </button>
                            }
                        }
                        #[cfg(feature = "ssr")]
                        {
                            view! {
                                <button class="px-4 py-1.5 text-sm rounded border border-border text-charcoal" aria-label="Play simulation">
                                    "▶"
                                </button>
                                <button
                                    class="px-4 py-1.5 text-sm rounded border"
                                    style:border-color="#3B82F6"
                                    style:color="#3B82F6"
                                    aria-label="Reset simulation"
                                >
                                    "↻"
                                </button>
                                <button class="px-4 py-1.5 text-sm rounded border border-border text-charcoal flex items-center justify-center" aria-label="Open settings">
                                    <Icon name="cog" class="w-4 h-4" />
                                </button>
                            }
                        }
                    }
                </div>
                <span class="text-sm text-charcoal-light">
                    {alive_cells} " cells"
                </span>
            </div>

            <Show when=move || show_settings()>
                <div class="absolute top-12 right-0 z-20 bg-surface border border-border rounded-lg shadow-minimal-lg p-6 min-w-[320px]">
                    <div class="flex flex-col gap-4">
                        <div class="flex items-center justify-between mb-2">
                            <h3 class="text-lg font-medium text-charcoal">"Settings"</h3>
                            <button
                                class="text-charcoal-lighter hover:text-charcoal transition-colors"
                                on:click=move |_| set_show_settings.set(false)
                                aria-label="Close settings"
                            >
                                "✕"
                            </button>
                        </div>

                        <div class="flex flex-col gap-2">
                            <label for="grid_size" class="text-sm font-medium text-charcoal">
                                "Grid Size"
                            </label>
                            <input
                                type="number"
                                id="grid_size"
                                class="px-3 py-2 rounded border border-border bg-surface text-charcoal focus:outline-none focus:ring-2 focus:ring-accent transition-all"
                                on:input=move |ev| {
                                    if let Ok(val) = event_target_value(&ev).parse::<u32>() {
                                        set_grid_size(val);
                                    }
                                }

                                prop:value=grid_size
                            />
                        </div>

                        <div class="flex flex-col gap-2">
                            <label for="alive_probability" class="text-sm font-medium text-charcoal">
                                "Alive Probability"
                            </label>
                            <input
                                type="number"
                                id="alive_probability"
                                step="0.1"
                                min="0"
                                max="1"
                                class="px-3 py-2 rounded border border-border bg-surface text-charcoal focus:outline-none focus:ring-2 focus:ring-accent transition-all"
                                on:input=move |ev| {
                                    if let Ok(val) = event_target_value(&ev).parse::<f64>() {
                                        set_alive_probability(val);
                                    }
                                }

                                prop:value=alive_probability
                            />
                        </div>

                        <div class="flex flex-col gap-2">
                            <label for="interval_ms" class="text-sm font-medium text-charcoal">
                                "Speed (ms)"
                            </label>
                            <input
                                type="number"
                                id="interval_ms"
                                class="px-3 py-2 rounded border border-border bg-surface text-charcoal focus:outline-none focus:ring-2 focus:ring-accent transition-all"
                                on:input=move |ev| {
                                    if let Ok(val) = event_target_value(&ev).parse::<u64>() {
                                        set_interval_ms(val);
                                    }
                                }

                                prop:value=interval_ms
                            />
                        </div>

                        <div class="flex items-center gap-4 pt-2 text-sm text-charcoal-light">
                            <a
                                class="text-accent hover:underline transition-colors duration-200"
                                href="https://en.wikipedia.org/wiki/Conway%27s_Game_of_Life"
                                target="_blank"
                                rel="noreferrer"
                            >
                                "Learn More"
                            </a>
                        </div>
                    </div>
                </div>
            </Show>

            <div class="w-full flex justify-center">
                <Grid grid_size cells set_cells />
            </div>
        </div>
    }
}

#[component]
pub fn Life() -> impl IntoView {
    view! {
        <SourceAnchor href="#[git]" />
        <div class="max-w-7xl mx-auto px-8 w-full flex flex-col gap-8 items-center">
            <h1 class="text-3xl font-bold text-charcoal">
                "Conway's Game of Life"
            </h1>
            <LifeGame auto_start=true initial_grid_size=250 />
        </div>
    }
}
