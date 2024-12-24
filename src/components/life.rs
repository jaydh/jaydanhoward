use crate::components::source_anchor::SourceAnchor;
use leptos::prelude::*;
use leptos::*;
use leptos_dom::helpers::IntervalHandle;
use rand::Rng;
use std::fmt;

#[derive(Debug, Clone)]
struct Cell {
    alive: bool,
    x_pos: i32,
    y_pos: i32,
    neighbors: Vec<usize>,
}

impl fmt::Display for Cell {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "alive: {}, x_pos: {}, y_pos: {}",
            self.alive, self.x_pos, self.y_pos
        )
    }
}

#[derive(Clone)]
struct CellVec(Vec<Cell>);
impl fmt::Display for CellVec {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (i, item) in self.0.iter().enumerate() {
            write!(f, "Item {}: {}\n", i, item)?;
        }
        Ok(())
    }
}

fn prepare_neighbors(cells: ReadSignal<CellVec>, set_cells: WriteSignal<CellVec>) {
    let current_cells = &cells().0;
    let mut next_cells = current_cells.clone();

    for cell in &mut next_cells {
        for dy in -1..=1 {
            for dx in -1..=1 {
                if dx == 0 && dy == 0 {
                    continue;
                }

                let neighbor_x = cell.x_pos as isize + dx;
                let neighbor_y = cell.y_pos as isize + dy;

                if neighbor_x >= 0 && neighbor_y >= 0 {
                    if let Some(neighbor_index) = current_cells.iter().position(|c| {
                        c.x_pos as isize == neighbor_x && c.y_pos as isize == neighbor_y
                    }) {
                        cell.neighbors.push(neighbor_index);
                    }
                }
            }
        }
    }
    set_cells(CellVec(next_cells));
}

fn get_alive_neighbor_count(cell: &Cell, cells: &Vec<Cell>) -> i32 {
    let mut count = 0;
    for other_cell_index in &cell.neighbors {
        if cells[*other_cell_index].alive {
            count += 1;
        }
    }
    count
}

fn calculate_next(cells: ReadSignal<CellVec>, set_cells: WriteSignal<CellVec>) {
    let current_cells = &cells().0;
    let mut next_cells = current_cells.clone();

    for cell in &mut next_cells {
        let alive_neighbor_count = get_alive_neighbor_count(&cell, current_cells);
        if cell.alive {
            if alive_neighbor_count != 2 && alive_neighbor_count != 3 {
                cell.alive = false;
            }
        } else {
            if alive_neighbor_count == 3 {
                cell.alive = true;
            }
        }
    }

    set_cells(CellVec(next_cells));
}

fn randomize_cells(alive_probability: f64, grid_size: u32, set_cells: WriteSignal<CellVec>) {
    let mut rng = rand::thread_rng();
    let mut cells: Vec<Cell> = Vec::new();
    for x in 0..grid_size {
        for y in 0..grid_size {
            cells.push(Cell {
                x_pos: x as i32,
                y_pos: y as i32,
                alive: rng.gen::<f64>() < alive_probability,
                neighbors: Vec::new(),
            })
        }
    }
    set_cells(CellVec(cells));
}

#[component]
fn Controls(
    grid_size: ReadSignal<u32>,
    set_grid_size: WriteSignal<u32>,
    alive_probability: ReadSignal<f64>,
    set_alive_probability: WriteSignal<f64>,
    cells: ReadSignal<CellVec>,
    set_cells: WriteSignal<CellVec>,
) -> impl IntoView {
    let (interval_handle, set_interval_handle) = create_signal(None::<IntervalHandle>);
    let (interval_ms, set_interval_ms) = create_signal(200);

    let create_simulation_interval = move || {
        if let Some(handle) = interval_handle() {
            handle.clear();
        }
        let interval_handle = set_interval_with_handle(
            move || {
                calculate_next(cells, set_cells);
            },
            std::time::Duration::from_millis(interval_ms()),
        );
        set_interval_handle(interval_handle.ok());
    };

    view! {
        <div class="flex flex-row space-x-10">
            <div class="flex flex-col text-charcoal dark:text-gray">
                <label for="grid_size">Grid Size</label>
                <input
                    type="text"
                    id="grid_size"
                    on:input=move |ev| {
                        set_grid_size(event_target_value(&ev).parse::<u32>().unwrap());
                    }

                    prop:value=grid_size
                />
                <label for="alive_probability">Alive probability</label>
                <input
                    type="text"
                    id="alive_probability"
                    on:input=move |ev| {
                        set_alive_probability(event_target_value(&ev).parse::<f64>().unwrap());
                    }

                    prop:value=alive_probability
                />
                <label for="interval_time">Simulation speed in ms</label>
                <input
                    type="text"
                    id="interval_time"
                    on:input=move |ev| {
                        set_interval_ms(event_target_value(&ev).parse::<u64>().unwrap());
                        if interval_handle().is_some() {
                            create_simulation_interval();
                        }
                    }

                    prop:value=interval_ms
                />
            </div>
            <div class="flex flex-col">
                <button on:click=move |_| {
                    if let Some(handle) = interval_handle() {
                        handle.clear();
                    }
                    set_cells(CellVec(Vec::new()))
                }>Reset</button>
                <button on:click=move |_| {
                    if let Some(handle) = interval_handle() {
                        handle.clear();
                    }
                    randomize_cells(alive_probability(), grid_size(), set_cells)
                }>Randomize</button>
                <button on:click=move |_| {
                    prepare_neighbors(cells, set_cells);
                    create_simulation_interval();
                }>Simulate</button>
            </div>
        </div>
    }
}

#[component]
fn Grid(
    grid_size: ReadSignal<u32>,
    cells: ReadSignal<CellVec>,
    set_cells: WriteSignal<CellVec>,
) -> impl IntoView {
    let range = move || 0..grid_size();
    view! {
        <div class="flex flex-col">
            {move || {
                range()
                    .clone()
                    .map(|y| {
                        view! {
                            <div class="flex flex-row">
                                {move || {
                                    range()
                                        .clone()
                                        .map(|x| {
                                            let is_alive = move || {
                                                cells
                                                    .get()
                                                    .0
                                                    .iter()
                                                    .any(|c| {
                                                        c.x_pos == x as i32 && c.y_pos == y as i32 && c.alive
                                                    })
                                            };
                                            view! {
                                                <div
                                                    class="w-10 h-10 border-2 border-charcoal dark:border-gray"
                                                    class=("bg-charcoal", move || is_alive() == true)
                                                    class=("dark:bg-gray", move || is_alive() == true)
                                                    on:click=move |_| {
                                                        match cells()
                                                            .0
                                                            .iter()
                                                            .position(|item| {
                                                                item.x_pos == x as i32 && item.y_pos == y as i32
                                                            })
                                                        {
                                                            Some(pos) => {
                                                                let mut next_cells = cells().0.clone();
                                                                next_cells[pos].alive = !cells().0[pos].alive;
                                                                set_cells(CellVec(next_cells));
                                                            }
                                                            None => {
                                                                set_cells
                                                                    .update(|v| {
                                                                        v.0
                                                                            .push(Cell {
                                                                                alive: true,
                                                                                x_pos: x as i32,
                                                                                y_pos: y as i32,
                                                                                neighbors: Vec::new(),
                                                                            });
                                                                    });
                                                            }
                                                        }
                                                    }
                                                ></div>
                                            }
                                        })
                                        .collect_view()
                                }}

                            </div>
                        }
                    })
                    .collect_view()
            }}

        </div>
    }
}

#[component]
pub fn Life() -> impl IntoView {
    let (cells, set_cells) = create_signal::<CellVec>(CellVec(Vec::new()));
    let alive_cells = move || cells().0.into_iter().filter(|c| c.alive).count();

    let (grid_size, set_grid_size) = create_signal(25);
    let (alive_probability, set_alive_probability) = create_signal(0.6);

    view! {
        <SourceAnchor href="#[git]" />
        <div class="flex flex-col items-center">
            <a
                class="hover:underline relative block px-3 py-2 transition"
                href="https://en.wikipedia.org/wiki/Conway%27s_Game_of_Life"
                target="_blank"
                rel="noreferrer"
            >
                "Wiki"
            </a>
            <h2>Alive Cells: {alive_cells}</h2>
            <Controls
                grid_size
                set_grid_size
                alive_probability
                set_alive_probability
                cells
                set_cells
            />
            <div class="mt-20">
                <Grid grid_size cells set_cells />
            </div>
        </div>
    }
}
