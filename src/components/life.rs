use leptos::*;
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

fn prepare_neighbors(read_cells: ReadSignal<CellVec>, set_cells: WriteSignal<CellVec>) {
    let current_cells = &read_cells().0;
    let mut next_cells = current_cells.clone();

    for cell in &mut next_cells {
        for dy in -1..=1 {
            for dx in -1..=1 {
                if dx == 0 && dy == 0 {
                    continue; // Skip the current cell
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

fn calculate_next(read_cells: ReadSignal<CellVec>, set_cells: WriteSignal<CellVec>) {
    let current_cells = &read_cells().0;
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
pub fn Life(cx: Scope) -> impl IntoView {
    let (cells, set_cells) = create_signal::<CellVec>(cx, CellVec(Vec::new()));
    let (interval_ms, set_interval_ms) = create_signal(cx, 200);
    let (grid_size, set_grid_size) = create_signal(cx, 25);
    let range = move || 0..grid_size();
    let (alive_probability, set_alive_probability) = create_signal(cx, 0.6);

    view! { cx,
        <div class="flex flex-col">
            <h1>"Conway's Game of Life"</h1>
            <label for="grid_size">
                Grid Size
            </label>
            <input
                type="text"
                id="grid_size"
                on:input=move |ev| {
                    set_grid_size(event_target_value(&ev).parse::<u32>().unwrap());
                }

                prop:value=grid_size
            />
            <label for="alive_probability">
                Alive probability
            </label>
            <input
                type="text"
                id="alive_probability"
                on:input=move |ev| {
                    set_alive_probability(event_target_value(&ev).parse::<f64>().unwrap());
                }

                prop:value=alive_probability
            />
            <label for="interval_time">
                Simulation speed in ms
            </label>
            <input
                type="text"
                id="interval_time"
                on:input=move |ev| {
                    set_interval_ms(event_target_value(&ev).parse::<u64>().unwrap());
                }

                prop:value=interval_ms
            />
            <button on:click=move |_| {
                set_cells(CellVec(Vec::new()))
            }>
                Reset
            </button>
            <button on:click=move |_| {
                randomize_cells(alive_probability(), grid_size(), set_cells)
            }>
                Randomize
            </button>
            <button on:click=move |_| {
                prepare_neighbors(cells, set_cells);
                let _ = set_interval_with_handle(
                    move || {
                        calculate_next(cells, set_cells);
                    },
                    std::time::Duration::from_millis(interval_ms()),
                );
            }>
                Simulate
            </button>
            <div class="flex flex-col items-center">
                {move || {
                    range()
                        .clone()
                        .map(|x| {
                            view! { cx,
                                <div class="flex flex-row">
                                    {move || {
                                        range()
                                            .clone()
                                            .map(|y| {
                                                let isAlive = move || {
                                                    cells
                                                        .get()
                                                        .0
                                                        .iter()
                                                        .any(|c| {
                                                            c.x_pos == x as i32 && c.y_pos == y as i32 && c.alive
                                                        })
                                                };

                                                view! { cx,
                                                    <div
                                                        class="w-10 h-10 border-2 border-green-600"
                                                        class=("bg-amber-500", move || isAlive() == true)
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
                                                                    next_cells[pos] = Cell {
                                                                        alive: !cells().0[pos].alive,
                                                                        x_pos: x as i32,
                                                                        y_pos: y as i32,
                                                                        neighbors: Vec::new(),
                                                                    };
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
                                                    >
                                                    </div>
                                                }
                                            })
                                            .collect_view(cx)
                                    }}

                                </div>
                            }
                        })
                        .collect_view(cx)
                }}

            </div>
        </div>
    }
}
