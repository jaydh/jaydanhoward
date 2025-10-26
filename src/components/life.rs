use crate::components::source_anchor::SourceAnchor;
use leptos::prelude::*;
use rand::Rng;
use std::collections::HashSet;

#[derive(Clone, Default)]
struct AliveCells(HashSet<(i32, i32)>);

#[cfg(not(feature = "ssr"))]
fn calculate_next(
    cells: ReadSignal<AliveCells>,
    set_cells: WriteSignal<AliveCells>,
    grid_size: u32,
) {
    let current_alive = &cells().0;
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
        if count == 3 || is_alive && count == 2 {
            next_alive.insert(pos);
        }
    }

    set_cells(AliveCells(next_alive));
}

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
fn Controls(
    grid_size: ReadSignal<u32>,
    set_grid_size: WriteSignal<u32>,
    alive_probability: ReadSignal<f64>,
    set_alive_probability: WriteSignal<f64>,
    #[allow(unused_variables)] cells: ReadSignal<AliveCells>,
    set_cells: WriteSignal<AliveCells>,
    interval_ms: ReadSignal<u64>,
    set_interval_ms: WriteSignal<u64>,
    start_simulation: impl Fn() + 'static + Copy,
    stop_simulation: impl Fn() + 'static + Copy,
) -> impl IntoView {

    view! {
        <div class="flex flex-col gap-6 w-full max-w-2xl">
            <div class="grid grid-cols-1 md:grid-cols-3 gap-4">
                <div class="flex flex-col gap-2">
                    <label for="grid_size" class="text-sm font-medium text-charcoal dark:text-gray">
                        Grid Size
                    </label>
                    <input
                        type="text"
                        id="grid_size"
                        class="px-4 py-2 rounded-lg border border-border dark:border-border-dark bg-surface dark:bg-surface-dark text-charcoal dark:text-gray focus:outline-none focus:ring-2 focus:ring-accent dark:focus:ring-accent-light transition-all"
                        on:input=move |ev| {
                            set_grid_size(event_target_value(&ev).parse::<u32>().unwrap());
                        }

                        prop:value=grid_size
                    />
                </div>
                <div class="flex flex-col gap-2">
                    <label for="alive_probability" class="text-sm font-medium text-charcoal dark:text-gray">
                        Alive Probability
                    </label>
                    <input
                        type="text"
                        id="alive_probability"
                        class="px-4 py-2 rounded-lg border border-border dark:border-border-dark bg-surface dark:bg-surface-dark text-charcoal dark:text-gray focus:outline-none focus:ring-2 focus:ring-accent dark:focus:ring-accent-light transition-all"
                        on:input=move |ev| {
                            set_alive_probability(event_target_value(&ev).parse::<f64>().unwrap());
                        }

                        prop:value=alive_probability
                    />
                </div>
                <div class="flex flex-col gap-2">
                    <label for="interval_time" class="text-sm font-medium text-charcoal dark:text-gray">
                        Speed (ms)
                    </label>
                    <input
                        type="text"
                        id="interval_time"
                        class="px-4 py-2 rounded-lg border border-border dark:border-border-dark bg-surface dark:bg-surface-dark text-charcoal dark:text-gray focus:outline-none focus:ring-2 focus:ring-accent dark:focus:ring-accent-light transition-all"
                        on:input=move |ev| {
                            set_interval_ms(event_target_value(&ev).parse::<u64>().unwrap());
                        }

                        prop:value=interval_ms
                    />
                </div>
            </div>
            <div class="flex flex-row gap-3">
                <button
                    class="px-6 py-2 rounded-lg border border-border dark:border-border-dark bg-surface dark:bg-surface-dark text-charcoal dark:text-gray hover:bg-border dark:hover:bg-border-dark hover:bg-opacity-30 dark:hover:bg-opacity-30 transition-all duration-200 font-medium"
                    on:click=move |_| {
                        stop_simulation();
                        set_cells(AliveCells::default())
                    }
                >
                    Reset
                </button>
                <button
                    class="px-6 py-2 rounded-lg border border-border dark:border-border-dark bg-surface dark:bg-surface-dark text-charcoal dark:text-gray hover:bg-border dark:hover:bg-border-dark hover:bg-opacity-30 dark:hover:bg-opacity-30 transition-all duration-200 font-medium"
                    on:click=move |_| {
                        stop_simulation();
                        randomize_cells(alive_probability(), grid_size(), set_cells)
                    }
                >
                    Randomize
                </button>
                <button
                    class="px-6 py-2 rounded-lg bg-accent dark:bg-accent-light text-white hover:bg-accent-dark dark:hover:bg-accent transition-all duration-200 font-medium shadow-minimal"
                    on:click=move |_| {
                        start_simulation();
                    }
                >
                    Simulate
                </button>
            </div>
        </div>
    }
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
    use wasm_bindgen::JsCast;

    #[cfg(not(feature = "ssr"))]
    let canvas_ref = NodeRef::<Canvas>::new();

    #[cfg(not(feature = "ssr"))]
    let (resize_trigger, set_resize_trigger) = signal(0);

    // Set up window resize listener
    #[cfg(not(feature = "ssr"))]
    {
        use wasm_bindgen::closure::Closure;
        use wasm_bindgen::JsCast;

        Effect::new(move |_| {
            let window = web_sys::window().unwrap();
            let closure = Closure::wrap(Box::new(move || {
                set_resize_trigger.update(|n| *n += 1);
            }) as Box<dyn Fn()>);

            window
                .add_event_listener_with_callback("resize", closure.as_ref().unchecked_ref())
                .unwrap();

            closure.forget(); // Keep the closure alive
        });
    }

    #[cfg(not(feature = "ssr"))]
    Effect::new(move |_| {
        // Depend on resize_trigger to re-run on window resize
        let _ = resize_trigger();

        let Some(canvas) = canvas_ref.get() else {
            return;
        };

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
        let max_size = max_width.min(max_height).min(800.0).max(300.0); // Between 300-800px

        let grid = grid_size();
        let cell_px = (max_size / grid as f64).floor().max(1.0);
        let canvas_size = (cell_px * grid as f64) as u32;

        canvas.set_width(canvas_size);
        canvas.set_height(canvas_size);

        let context = canvas
            .get_context("2d")
            .unwrap()
            .unwrap()
            .unchecked_into::<web_sys::CanvasRenderingContext2d>();

        // Clear canvas
        context.set_fill_style_str("#FFFFFF");
        context.fill_rect(0.0, 0.0, canvas_size as f64, canvas_size as f64);

        // Draw grid lines (only if cells are large enough)
        if cell_px >= 5.0 {
            context.set_stroke_style_str("#E5E7EB");
            context.set_line_width(1.0);
            for i in 0..=grid {
                let pos = i as f64 * cell_px;
                context.begin_path();
                context.move_to(pos, 0.0);
                context.line_to(pos, canvas_size as f64);
                context.stroke();
                context.begin_path();
                context.move_to(0.0, pos);
                context.line_to(canvas_size as f64, pos);
                context.stroke();
            }
        }

        // Draw alive cells
        context.set_fill_style_str("#3B82F6");
        for &(x, y) in cells().0.iter() {
            context.fill_rect(
                x as f64 * cell_px,
                y as f64 * cell_px,
                cell_px,
                cell_px,
            );
        }
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
            class="border border-border dark:border-border-dark cursor-pointer"
            on:click=handle_click
        ></canvas>
    };

    #[cfg(feature = "ssr")]
    view! {
        <canvas
            class="border border-border dark:border-border-dark cursor-pointer"
            on:click=handle_click
        ></canvas>
    }
}

#[component]
pub fn LifeGame(
    #[prop(optional)] initial_grid_size: Option<u32>,
    #[prop(optional)] initial_alive_probability: Option<f64>,
    #[prop(optional)] initial_interval_ms: Option<u64>,
    #[prop(default = true)] show_controls: bool,
    #[prop(default = false)] #[allow(unused_variables)] auto_start: bool,
) -> impl IntoView {
    let (cells, set_cells) = signal::<AliveCells>(AliveCells::default());
    let alive_cells = move || cells().0.len();

    let (grid_size, set_grid_size) = signal(initial_grid_size.unwrap_or(25));
    let (alive_probability, set_alive_probability) = signal(initial_alive_probability.unwrap_or(0.6));
    let (interval_ms, set_interval_ms) = signal(initial_interval_ms.unwrap_or(200));

    #[cfg(not(feature = "ssr"))]
    let (interval_handle, set_interval_handle) = signal(None::<leptos::prelude::IntervalHandle>);
    #[cfg(not(feature = "ssr"))]
    let (is_running, set_is_running) = signal(false);

    #[cfg(not(feature = "ssr"))]
    let start_simulation = {
        let interval_handle = interval_handle;
        let set_interval_handle = set_interval_handle;
        let set_is_running = set_is_running;
        let interval_ms = interval_ms;
        let cells = cells;
        let set_cells = set_cells;
        let grid_size = grid_size;
        move || {
            if let Some(handle) = interval_handle() {
                handle.clear();
            }
            let handle = set_interval_with_handle(
                move || {
                    calculate_next(cells, set_cells, grid_size());
                },
                std::time::Duration::from_millis(interval_ms()),
            );
            set_interval_handle(handle.ok());
            set_is_running(true);
        }
    };

    #[cfg(not(feature = "ssr"))]
    let stop_simulation = {
        let interval_handle = interval_handle;
        let set_interval_handle = set_interval_handle;
        let set_is_running = set_is_running;
        move || {
            if let Some(handle) = interval_handle() {
                handle.clear();
            }
            set_interval_handle(None);
            set_is_running(false);
        }
    };

    #[cfg(not(feature = "ssr"))]
    let toggle_simulation = {
        let is_running = is_running;
        move || {
            if is_running() {
                stop_simulation();
            } else {
                start_simulation();
            }
        }
    };

    // Auto-start if requested
    #[cfg(not(feature = "ssr"))]
    if auto_start {
        let initial_probability = alive_probability();
        let initial_grid_size = grid_size();
        Effect::new(move |prev: Option<()>| {
            // Only run once on mount
            if prev.is_some() {
                return;
            }
            // Randomize cells on mount
            randomize_cells(initial_probability, initial_grid_size, set_cells);
            // Start simulation
            start_simulation();
        });
    }

    #[cfg(feature = "ssr")]
    let start_simulation = || {};
    #[cfg(feature = "ssr")]
    let stop_simulation = || {};

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
        <div class="w-full flex flex-col gap-4 items-center">
            <div class="flex gap-3 items-center">
                <div class="flex gap-2">
                    {
                        #[cfg(not(feature = "ssr"))]
                        {
                            view! {
                                <button
                                    class="px-4 py-1.5 text-sm rounded border transition-all duration-200"
                                    style:border-color="#3B82F6"
                                    style:color="#3B82F6"
                                    class:hover:bg-opacity-20=true
                                    on:click=move |_| toggle_simulation()
                                >
                                    {move || if is_running() { "▌▌" } else { "▶" }}
                                </button>
                                <button
                                    class="px-4 py-1.5 text-sm rounded border transition-all duration-200"
                                    style:border-color="#3B82F6"
                                    style:color="#3B82F6"
                                    class:hover:bg-opacity-20=true
                                    on:click=move |_| reset()
                                >
                                    "↻"
                                </button>
                            }
                        }
                        #[cfg(feature = "ssr")]
                        {
                            view! {
                                <button
                                    class="px-4 py-1.5 text-sm rounded border border-border dark:border-border-dark text-charcoal dark:text-gray hover:bg-border dark:hover:bg-border-dark hover:bg-opacity-20 dark:hover:bg-opacity-20 transition-all duration-200"
                                >
                                    "▶"
                                </button>
                                <button
                                    class="px-4 py-1.5 text-sm rounded border transition-all duration-200"
                                    style:border-color="#3B82F6"
                                    style:color="#3B82F6"
                                >
                                    "↻"
                                </button>
                            }
                        }
                    }
                </div>
                <span class="text-sm text-charcoal dark:text-gray opacity-75 dark:opacity-70">
                    {alive_cells} " cells"
                </span>
            </div>
            {show_controls.then(|| view! {
                <Controls
                    grid_size
                    set_grid_size
                    alive_probability
                    set_alive_probability
                    cells
                    set_cells
                    interval_ms
                    set_interval_ms
                    start_simulation
                    stop_simulation
                />
                <div class="flex items-center gap-4 text-sm text-charcoal dark:text-gray opacity-75 dark:opacity-70">
                    <span>"Alive Cells: " {alive_cells}</span>
                    <span class="text-border dark:text-border-dark">"|"</span>
                    <a
                        class="text-accent dark:text-accent-light hover:underline transition-colors duration-200"
                        href="https://en.wikipedia.org/wiki/Conway%27s_Game_of_Life"
                        target="_blank"
                        rel="noreferrer"
                    >
                        "Learn More"
                    </a>
                </div>
            })}
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
        <div class="max-w-7xl mx-auto px-8 py-16 w-full flex flex-col gap-8 items-center">
            <h1 class="text-3xl font-bold text-charcoal dark:text-gray">
                "Conway's Game of Life"
            </h1>
            <LifeGame />
        </div>
    }
}
