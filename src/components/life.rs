use crate::components::source_anchor::SourceAnchor;
use leptos::prelude::*;
use leptos::*;
use leptos_dom::helpers::IntervalHandle;
use rand::Rng;
use std::collections::HashSet;

#[derive(Clone, Default)]
struct AliveCells(HashSet<(i32, i32)>);

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
    cells: ReadSignal<AliveCells>,
    set_cells: WriteSignal<AliveCells>,
) -> impl IntoView {
    let (interval_handle, set_interval_handle) = signal(None::<IntervalHandle>);
    let (interval_ms, set_interval_ms) = signal(200);

    let create_simulation_interval = move || {
        if let Some(handle) = interval_handle() {
            handle.clear();
        }
        let interval_handle = set_interval_with_handle(
            move || {
                calculate_next(cells, set_cells, grid_size());
            },
            std::time::Duration::from_millis(interval_ms()),
        );
        set_interval_handle(interval_handle.ok());
    };

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
                            if interval_handle().is_some() {
                                create_simulation_interval();
                            }
                        }

                        prop:value=interval_ms
                    />
                </div>
            </div>
            <div class="flex flex-row gap-3">
                <button
                    class="px-6 py-2 rounded-lg border border-border dark:border-border-dark bg-surface dark:bg-surface-dark text-charcoal dark:text-gray hover:bg-border dark:hover:bg-border-dark hover:bg-opacity-30 dark:hover:bg-opacity-30 transition-all duration-200 font-medium"
                    on:click=move |_| {
                        if let Some(handle) = interval_handle() {
                            handle.clear();
                        }
                        set_cells(AliveCells::default())
                    }
                >
                    Reset
                </button>
                <button
                    class="px-6 py-2 rounded-lg border border-border dark:border-border-dark bg-surface dark:bg-surface-dark text-charcoal dark:text-gray hover:bg-border dark:hover:bg-border-dark hover:bg-opacity-30 dark:hover:bg-opacity-30 transition-all duration-200 font-medium"
                    on:click=move |_| {
                        if let Some(handle) = interval_handle() {
                            handle.clear();
                        }
                        randomize_cells(alive_probability(), grid_size(), set_cells)
                    }
                >
                    Randomize
                </button>
                <button
                    class="px-6 py-2 rounded-lg bg-accent dark:bg-accent-light text-white hover:bg-accent-dark dark:hover:bg-accent transition-all duration-200 font-medium shadow-minimal"
                    on:click=move |_| {
                        create_simulation_interval();
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
    grid_size: ReadSignal<u32>,
    #[allow(unused_variables)] cells: ReadSignal<AliveCells>,
    #[allow(unused_variables)] set_cells: WriteSignal<AliveCells>,
) -> impl IntoView {
    #[cfg(not(feature = "ssr"))]
    use leptos::html::Canvas;
    #[cfg(not(feature = "ssr"))]
    use wasm_bindgen::JsCast;

    #[cfg(not(feature = "ssr"))]
    let canvas_ref = NodeRef::<Canvas>::new();

    // Determine cell size based on grid size
    #[allow(unused_variables)]
    let cell_size = move || {
        let size = grid_size();
        if size > 100 {
            3
        } else if size > 50 {
            5
        } else {
            10
        }
    };

    #[cfg(not(feature = "ssr"))]
    Effect::new(move |_| {
        let Some(canvas) = canvas_ref.get() else {
            return;
        };
        let context = canvas
            .get_context("2d")
            .unwrap()
            .unwrap()
            .unchecked_into::<web_sys::CanvasRenderingContext2d>();

        let grid = grid_size();
        let cell_px = cell_size();
        let canvas_size = grid * cell_px;

        canvas.set_width(canvas_size);
        canvas.set_height(canvas_size);

        // Clear canvas
        context.set_fill_style_str("#FFFFFF");
        context.fill_rect(0.0, 0.0, canvas_size as f64, canvas_size as f64);

        // Draw grid lines
        context.set_stroke_style_str("#E5E7EB");
        context.set_line_width(1.0);
        for i in 0..=grid {
            let pos = (i * cell_px) as f64;
            context.begin_path();
            context.move_to(pos, 0.0);
            context.line_to(pos, canvas_size as f64);
            context.stroke();
            context.begin_path();
            context.move_to(0.0, pos);
            context.line_to(canvas_size as f64, pos);
            context.stroke();
        }

        // Draw alive cells
        context.set_fill_style_str("#3B82F6");
        for &(x, y) in cells().0.iter() {
            context.fill_rect(
                (x * cell_px as i32) as f64,
                (y * cell_px as i32) as f64,
                cell_px as f64,
                cell_px as f64,
            );
        }
    });

    #[cfg(not(feature = "ssr"))]
    let handle_click = move |event: web_sys::MouseEvent| {
        let Some(canvas) = canvas_ref.get() else {
            return;
        };
        let canvas_element: &web_sys::Element = canvas.as_ref();
        let rect = canvas_element.get_bounding_client_rect();
        let cell_px = cell_size();
        let x = ((event.client_x() as f64 - rect.left()) / cell_px as f64) as i32;
        let y = ((event.client_y() as f64 - rect.top()) / cell_px as f64) as i32;

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
pub fn Life() -> impl IntoView {
    let (cells, set_cells) = signal::<AliveCells>(AliveCells::default());
    let alive_cells = move || cells().0.len();

    let (grid_size, set_grid_size) = signal(25);
    let (alive_probability, set_alive_probability) = signal(0.6);

    view! {
        <SourceAnchor href="#[git]" />
        <div class="max-w-7xl mx-auto px-8 py-16 w-full flex flex-col gap-8 items-center">
            <h1 class="text-3xl font-bold text-charcoal dark:text-gray">
                "Conway's Game of Life"
            </h1>
            <Controls
                grid_size
                set_grid_size
                alive_probability
                set_alive_probability
                cells
                set_cells
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
            <div class="mt-4">
                <Grid grid_size cells set_cells />
            </div>
        </div>
    }
}
