use crate::components::source_anchor::SourceAnchor;
use leptos::*;
use leptos_dom::helpers::IntervalHandle;
use rand::Rng;
use std::collections::HashMap;
use std::fmt;

#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash)]
struct CoordinatePair {
    x_pos: u64,
    y_pos: u64,
}

#[derive(Debug, Clone)]
struct Cell {
    is_passable: bool,
    visited: bool,
    coordiantes: CoordinatePair,
}

impl fmt::Display for CoordinatePair {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "x_pos: {}, y_pos: {}", self.x_pos, self.y_pos)
    }
}

impl fmt::Display for Cell {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "is_passable: {}, x_pos: {}, y_pos: {}, visited: {}",
            self.is_passable, self.coordiantes.x_pos, self.coordiantes.y_pos, self.visited
        )
    }
}

#[derive(Debug, Clone)]
struct Grid(HashMap<CoordinatePair, Cell>);

fn randomize_cells(passable_probability: f64, grid_size: u64, set_grid: WriteSignal<Grid>) {
    let mut rng = rand::thread_rng();
    let mut grid = Grid(HashMap::new());
    for x in 0..grid_size {
        for y in 0..grid_size {
            grid.0.insert(
                CoordinatePair { x_pos: x, y_pos: y },
                Cell {
                    coordiantes: CoordinatePair { x_pos: x, y_pos: y },
                    is_passable: rng.gen::<f64>() > passable_probability,
                    visited: false,
                },
            );
        }
    }
    set_grid(grid);
}

fn get_next_direction(direction: Option<(i64, i64)>) -> (i64, i64) {
    let directions = [(0, 1), (0, -1), (1, 0), (-1, 0)];
    if let Some(last) = direction {
        for dir in directions.iter().cycle() {
            if *dir != last {
                return *dir;
            }
        }
    }

    directions[0]
}

fn calculate_next(
    direction: ReadSignal<Option<(i64, i64)>>,
    set_direction: WriteSignal<Option<(i64, i64)>>,
    grid: ReadSignal<Grid>,
    set_grid: WriteSignal<Grid>,
    current_path: ReadSignal<Vec<CoordinatePair>>,
    set_current_path: WriteSignal<Vec<CoordinatePair>>,
    start_cell_coord: ReadSignal<Option<CoordinatePair>>,
    end_cell_coord: ReadSignal<Option<CoordinatePair>>,
) {
    if direction().is_none() {
        set_direction(Some(get_next_direction(direction())));
    }

    if current_path().len() == 0 {
        set_current_path.update(|path| path.push(start_cell_coord().unwrap()));
        set_grid.update(|grid| {
            let cell = grid.0.get_mut(&start_cell_coord().unwrap()).unwrap();
            cell.visited = true;
        });
    } else {
        let next_in_bounds = move || {
            current_path().last().unwrap().x_pos as i64 + &direction().unwrap().0 > 0
                && current_path().last().unwrap().y_pos as i64 + &direction().unwrap().1 > 0
        };

        if !next_in_bounds() {
            set_direction(Some(get_next_direction(direction())));
        } else {
            let next_visit_coord = CoordinatePair {
                x_pos: (current_path().last().unwrap().x_pos as i64 + direction().unwrap().0)
                    as u64,
                y_pos: (current_path().last().unwrap().y_pos as i64 + direction().unwrap().1)
                    as u64,
            };
            if grid().0.get(&next_visit_coord).unwrap().is_passable {
                logging::log!("pushing {}", next_visit_coord);
                set_current_path.update(|path| path.push(next_visit_coord));
                set_grid.update(|grid| {
                    let cell = grid.0.get_mut(&next_visit_coord).unwrap();
                    cell.visited = true;
                    logging::log!("visited {}", cell);
                });
            } else {
                set_direction(Some(get_next_direction(direction())));
            };
        }
    }
}

#[component]
fn Controls(
    grid_size: ReadSignal<u64>,
    set_grid_size: WriteSignal<u64>,
    grid: ReadSignal<Grid>,
    set_grid: WriteSignal<Grid>,
    obstacle_probability: ReadSignal<f64>,
    set_obstacle_probability: WriteSignal<f64>,
    start_cell_coord: ReadSignal<Option<CoordinatePair>>,
    end_cell_coord: ReadSignal<Option<CoordinatePair>>,
    current_path: ReadSignal<Vec<CoordinatePair>>,
    set_current_path: WriteSignal<Vec<CoordinatePair>>,
) -> impl IntoView {
    let (interval_handle, set_interval_handle) = create_signal(None::<IntervalHandle>);
    let (direction, set_direction) = create_signal(None::<(i64, i64)>);
    let (interval_ms, set_interval_ms) = create_signal(200);

    let create_simulation_interval = move || {
        if let Some(handle) = interval_handle() {
            handle.clear();
        }
        if start_cell_coord().is_none() || end_cell_coord().is_none() {
            return;
        }

        let interval_handle = set_interval_with_handle(
            move || {
                calculate_next(
                    direction,
                    set_direction,
                    grid,
                    set_grid,
                    current_path,
                    set_current_path,
                    start_cell_coord,
                    end_cell_coord,
                )
            },
            std::time::Duration::from_millis(interval_ms()),
        );
        set_interval_handle(interval_handle.ok());
    };

    view! {
        <div class="flex flex-row space-x-10 mb-10">
            <div class="flex flex-col text-charcoal dark:text-gray">
                <label for="grid_size">Grid Size</label>
                <input
                    type="text"
                    id="grid_size"
                    on:input=move |ev| {
                        set_grid_size(event_target_value(&ev).parse::<u64>().unwrap());
                    }

                    prop:value=grid_size
                />
                <label for="obstacle_probability">Obstacle probability</label>
                <input
                    type="text"
                    id="obstacle_probability"
                    on:input=move |ev| {
                        set_obstacle_probability(event_target_value(&ev).parse::<f64>().unwrap());
                    }

                    prop:value=obstacle_probability
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
                    randomize_cells(obstacle_probability(), grid_size(), set_grid)
                }>Randomize</button>
                <button on:click=move |_| {
                    create_simulation_interval();
                }>Simulate</button>

            </div>
        </div>
    }
}

#[component]
fn SearchGrid(
    grid_size: ReadSignal<u64>,
    grid: ReadSignal<Grid>,
    set_grid: WriteSignal<Grid>,
    start_cell_coord: ReadSignal<Option<CoordinatePair>>,
    set_start_cell_coord: WriteSignal<Option<CoordinatePair>>,
    end_cell_coord: ReadSignal<Option<CoordinatePair>>,
    set_end_cell_coord: WriteSignal<Option<CoordinatePair>>,
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
                                            let is_passable = move || {
                                                grid.get()
                                                    .0
                                                    .get(
                                                        &CoordinatePair {
                                                            x_pos: x,
                                                            y_pos: y,
                                                        },
                                                    )
                                                    .map(|c| c.is_passable)
                                                    .unwrap_or(false)
                                            };
                                            let is_start_cell = move || {
                                                start_cell_coord()
                                                    .map(|c| (x, y) == (c.x_pos, c.y_pos))
                                                    .unwrap_or(false)
                                            };
                                            let is_end_cell = move || {
                                                end_cell_coord()
                                                    .map(|c| (x, y) == (c.x_pos, c.y_pos))
                                                    .unwrap_or(false)
                                            };
                                            let is_visited = move || {
                                                grid()
                                                    .0
                                                    .get(
                                                        &CoordinatePair {
                                                            x_pos: x,
                                                            y_pos: y,
                                                        },
                                                    )
                                                    .map(|c| c.visited)
                                                    .unwrap_or(false)
                                            };
                                            let on_click = move |_| {
                                                let clicked_on_start = move || {
                                                    start_cell_coord()
                                                        .map(|c| (c.x_pos, c.y_pos) == (x, y))
                                                        .unwrap_or(false)
                                                };
                                                let clicked_on_end = move || {
                                                    end_cell_coord()
                                                        .map(|c| (c.x_pos, c.y_pos) == (x, y))
                                                        .unwrap_or(false)
                                                };
                                                if clicked_on_start() {
                                                    set_start_cell_coord(None);
                                                } else if clicked_on_end() {
                                                    set_end_cell_coord(None);
                                                } else if start_cell_coord().is_none() {
                                                    set_start_cell_coord(
                                                        Some(CoordinatePair {
                                                            x_pos: x,
                                                            y_pos: y,
                                                        }),
                                                    );
                                                } else if end_cell_coord().is_none() {
                                                    set_end_cell_coord(
                                                        Some(CoordinatePair {
                                                            x_pos: x,
                                                            y_pos: y,
                                                        }),
                                                    );
                                                }
                                            };
                                            view! {
                                                <div
                                                    class="w-10 h-10 border-2 border-charcoal dark:border-gray"
                                                    class=("bg-green-500", move || is_start_cell() == true)
                                                    class=("bg-yellow-500", move || is_end_cell() == true)
                                                    class=(
                                                        "bg-blue-500",
                                                        move || {
                                                            is_start_cell() == false && is_end_cell() == false
                                                                && is_visited() == true
                                                        },
                                                    )

                                                    class=(
                                                        "bg-charcoal",
                                                        move || {
                                                            is_start_cell() == false && is_end_cell() == false
                                                                && is_visited() == false && is_passable() == true
                                                        },
                                                    )

                                                    class=(
                                                        "dark:bg-gray",
                                                        move || {
                                                            is_start_cell() == false && is_end_cell() == false
                                                                && is_visited() == false && is_passable() == true
                                                        },
                                                    )

                                                    on:click=on_click
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
pub fn PathSearch() -> impl IntoView {
    let (grid_size, set_grid_size) = create_signal(25);
    let (grid, set_grid) = create_signal(Grid(HashMap::new()));
    let (obstacle_probability, set_obstacle_probability) = create_signal(0.2);
    let (start_cell_coord, set_start_cell_coord) = create_signal(None::<CoordinatePair>);
    let (end_cell_coord, set_end_cell_coord) = create_signal(None::<CoordinatePair>);
    let (current_path, set_current_path) = create_signal(Vec::<CoordinatePair>::new());

    view! {
        <SourceAnchor href="#[git]"/>
        <div class="flex flex-col items-center">
            <Controls
                grid_size=grid_size
                set_grid_size=set_grid_size
                grid=grid
                set_grid=set_grid
                obstacle_probability=obstacle_probability
                set_obstacle_probability=set_obstacle_probability
                start_cell_coord=start_cell_coord
                end_cell_coord=end_cell_coord
                current_path=current_path
                set_current_path=set_current_path
            />
            <SearchGrid
                grid_size=grid_size
                grid=grid
                set_grid=set_grid
                start_cell_coord=start_cell_coord
                set_start_cell_coord
                end_cell_coord=end_cell_coord
                set_end_cell_coord=set_end_cell_coord
            />
        </div>
    }
}
