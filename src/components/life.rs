use leptos::*;
use rand::Rng;
use std::fmt;

#[derive(Debug, Clone)]
struct Cell {
    alive: bool,
    x_pos: i32,
    y_pos: i32,
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

fn get_alive_neighbor_count(cell: &Cell, cells: &Vec<Cell>) -> i32 {
    let mut count = 0;
    for other_cell in cells {
        if (other_cell.x_pos != cell.x_pos) || (other_cell.y_pos != cell.y_pos) {
            let dx = (other_cell.x_pos - cell.x_pos).abs();
            let dy = (other_cell.y_pos - cell.y_pos).abs();
            if dx <= 1 && dy <= 1 && (dx + dy > 0) && other_cell.alive {
                count += 1;
            }
        }
    }
    count
}

fn calculate_next(read_cells: ReadSignal<CellVec>, set_cells: WriteSignal<CellVec>) {
    read_cells.with(|c| log!("cells {}", c));
    let current_cells = &read_cells().0;
    let mut next_cells = current_cells.clone();

    for cell in &mut next_cells {
        let alive_neighbor_count = get_alive_neighbor_count(&cell, current_cells);
        log!(
            "cell {},{}, has {} neighbors",
            cell.x_pos,
            cell.y_pos,
            alive_neighbor_count
        );
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

    // Update the set_cells with the new state of cells
    set_cells(CellVec(next_cells));
}

fn randomize_cells(grid_size: u32, set_cells: WriteSignal<CellVec>) {
    let mut rng = rand::thread_rng();
    let mut cells: Vec<Cell> = Vec::new();
    for x in 0..grid_size {
        for y in 0..grid_size {
            cells.push(Cell {
                x_pos: x as i32,
                y_pos: y as i32,
                alive: rng.gen::<f64>() < 0.6,
            })
        }
    }
    set_cells(CellVec(cells));
}

#[component]
pub fn Life(cx: Scope) -> impl IntoView {
    let (cells, set_cells) = create_signal::<CellVec>(cx, CellVec(Vec::new()));
    let grid_size: u32 = 50;
    let range = 0..grid_size;

    view! { cx,
        <div class="flex flex-col">
            <h1>"Conway's Game of Life"</h1>
            <button on:click=move |_| { randomize_cells(grid_size,  set_cells) }>
                Randomize
            </button>
            <button on:click=move |_| {
                let _ = set_interval_with_handle(
                    move || {
                        calculate_next(cells, set_cells);
                    },
                    std::time::Duration::from_millis(500 as u64));
            }>
               Simulate
            </button>
            <div>
                {range
                    .clone()
                    .map(|x| {
                        view! { cx,
                            <div class="flex flex-row">
                                {range
                                    .clone()
                                    .map(|y| {
                                        let isAlive = move || {
                                            cells
                                                .get()
                                                .0
                                                .iter()
                                                .any(|c| c.x_pos == x as i32 && c.y_pos == y as i32 && c.alive)
                                        };

                                        view! { cx,
                                            <div
                                                class="w-10 h-10 border-2 border-green-600 bg-amber-500"
                                                class=("bg-amber-500", move || isAlive() == true)
                                                on:click=move |_| {
                                                    set_cells
                                                        .update(|v| {
                                                            v.0
                                                                .push(Cell {
                                                                    alive: true,
                                                                    x_pos: x as i32,
                                                                    y_pos: y as i32,
                                                                });
                                                        });
                                                }
                                            >
                                            </div>
                                        }
                                    })
                                    .collect_view(cx)}
                            </div>
                        }
                    })
                    .collect_view(cx)}
            </div>
        </div>
    }
}
