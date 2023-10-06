use crate::components::source_anchor::SourceAnchor;
use leptos::*;
use leptos_dom::helpers::IntervalHandle;
use rand::Rng;
use std::collections::HashMap;
use std::fmt;

#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash)]
struct CoordinatePair {
    x_pos: i64,
    y_pos: i64,
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
struct VecCoordinate(Vec<CoordinatePair>);

impl fmt::Display for VecCoordinate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (i, item) in self.0.iter().enumerate() {
            write!(f, "Item {}: {}\n", i, item)?;
        }
        Ok(())
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
                CoordinatePair {
                    x_pos: x as i64,
                    y_pos: y as i64,
                },
                Cell {
                    coordiantes: CoordinatePair {
                        x_pos: x as i64,
                        y_pos: y as i64,
                    },
                    is_passable: rng.gen::<f64>() > passable_probability,
                    visited: false,
                },
            );
        }
    }
    set_grid(grid);
}

fn get_next_corner(
    start_cell_coord: ReadSignal<Option<CoordinatePair>>,
    grid_size: ReadSignal<u64>,
    corner: ReadSignal<Option<CoordinatePair>>,
) -> CoordinatePair {
    let mut corners = [
        CoordinatePair { x_pos: 0, y_pos: 0 },
        CoordinatePair {
            x_pos: 0,
            y_pos: grid_size() as i64,
        },
        CoordinatePair {
            x_pos: grid_size() as i64,
            y_pos: 0,
        },
        CoordinatePair {
            x_pos: grid_size() as i64,
            y_pos: grid_size() as i64,
        },
    ];
    corners.sort_by(|a, b| {
        let distance_a = distance(&start_cell_coord().unwrap(), a);
        let distance_b = distance(&start_cell_coord().unwrap(), b);
        distance_a.partial_cmp(&distance_b).unwrap()
    });

    if let Some(last) = corner() {
        let mut iter = corners.iter().cycle().skip_while(|&&dir| dir != last);
        iter.next();

        if let Some(&next_corner) = iter.next() {
            return next_corner;
        }
    }

    corners[0]
}

fn distance(coord1: &CoordinatePair, coord2: &CoordinatePair) -> f64 {
    let dx = (coord1.x_pos - coord2.x_pos) as f64;
    let dy = (coord1.y_pos - coord2.y_pos) as f64;
    (dx * dx + dy * dy).sqrt()
}

fn add_candidates(
    current_cell: ReadSignal<Option<CoordinatePair>>,
    corner: ReadSignal<Option<CoordinatePair>>,
    grid: ReadSignal<Grid>,
    current_path_candidates: ReadSignal<VecCoordinate>,
    set_current_path_candidates: WriteSignal<VecCoordinate>,
) {
    let neighbors = [(0, 1), (0, -1), (1, 0), (-1, 0)]
        .map(|(x, y)| {
            grid()
                .0
                .get(&CoordinatePair {
                    x_pos: current_cell().unwrap().x_pos + x,
                    y_pos: current_cell().unwrap().y_pos + y,
                })
                .cloned()
        })
        .into_iter()
        .filter(|n| {
            n.as_ref()
                .map(|c| {
                    c.is_passable
                        && !c.visited
                        && !current_path_candidates().0.contains(&c.coordiantes)
                })
                .unwrap_or(false)
        })
        .map(|cell| cell.unwrap().coordiantes)
        .collect::<Vec<CoordinatePair>>();

    set_current_path_candidates.update(|path| {
        path.0.extend(neighbors);
        path.0.sort_by(|a, b| {
            let distance_a = distance(&corner().unwrap(), a);
            let distance_b = distance(&corner().unwrap(), b);
            distance_b.partial_cmp(&distance_a).unwrap()
        });
    });
}

fn calculate_next(
    grid_size: ReadSignal<u64>,
    corner: ReadSignal<Option<CoordinatePair>>,
    set_corner: WriteSignal<Option<CoordinatePair>>,
    grid: ReadSignal<Grid>,
    set_grid: WriteSignal<Grid>,
    current_path_candidates: ReadSignal<VecCoordinate>,
    set_current_path_candidates: WriteSignal<VecCoordinate>,
    start_cell_coord: ReadSignal<Option<CoordinatePair>>,
    current_cell: ReadSignal<Option<CoordinatePair>>,
    set_current_cell: WriteSignal<Option<CoordinatePair>>,
) {
    if current_cell().is_none() {
        set_current_cell(start_cell_coord());
        set_grid.update(|grid| {
            let cell = grid.0.get_mut(&start_cell_coord().unwrap()).unwrap();
            cell.visited = true;
        });
        set_corner(Some(get_next_corner(start_cell_coord, grid_size, corner)));
    } else {
        if current_cell() == corner() {
            set_corner(Some(get_next_corner(start_cell_coord, grid_size, corner)));
        }
        add_candidates(
            current_cell,
            corner,
            grid,
            current_path_candidates,
            set_current_path_candidates,
        );
        if let Some(next_visit_coord) = current_path_candidates().0.last() {
            set_current_path_candidates.update(|path| {
                path.0.pop();
            });

            set_current_cell(Some(*next_visit_coord));
            set_grid.update(|grid| {
                let cell = grid.0.get_mut(&next_visit_coord).unwrap();
                cell.visited = true;
            });
        } else {
            set_corner(Some(get_next_corner(start_cell_coord, grid_size, corner)));
        };
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
    current_path_candidates: ReadSignal<VecCoordinate>,
    set_current_path_candidates: WriteSignal<VecCoordinate>,
    current_cell: ReadSignal<Option<CoordinatePair>>,
    set_current_cell: WriteSignal<Option<CoordinatePair>>,
) -> impl IntoView {
    let (interval_handle, set_interval_handle) = create_signal(None::<IntervalHandle>);
    let (corner, set_corner) = create_signal(None::<CoordinatePair>);
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
                if current_cell() == end_cell_coord() {
                    interval_handle().unwrap().clear();
                }
                calculate_next(
                    grid_size,
                    corner,
                    set_corner,
                    grid,
                    set_grid,
                    current_path_candidates,
                    set_current_path_candidates,
                    start_cell_coord,
                    current_cell,
                    set_current_cell,
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
    start_cell_coord: ReadSignal<Option<CoordinatePair>>,
    set_start_cell_coord: WriteSignal<Option<CoordinatePair>>,
    end_cell_coord: ReadSignal<Option<CoordinatePair>>,
    set_end_cell_coord: WriteSignal<Option<CoordinatePair>>,
    current_cell: ReadSignal<Option<CoordinatePair>>,
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
                                                            x_pos: x as i64,
                                                            y_pos: y as i64,
                                                        },
                                                    )
                                                    .map(|c| c.is_passable)
                                                    .unwrap_or(false)
                                            };
                                            let is_current_cell = move || {
                                                current_cell()
                                                    .map(|c| (x as i64, y as i64) == (c.x_pos, c.y_pos))
                                                    .unwrap_or(false)
                                            };

                                            let is_start_cell = move || {
                                                start_cell_coord()
                                                    .map(|c| (x as i64, y as i64) == (c.x_pos, c.y_pos))
                                                    .unwrap_or(false)
                                            };
                                            let is_end_cell = move || {
                                                end_cell_coord()
                                                    .map(|c| (x as i64, y as i64) == (c.x_pos, c.y_pos))
                                                    .unwrap_or(false)
                                            };
                                            let is_visited = move || {
                                                grid()
                                                    .0
                                                    .get(
                                                        &CoordinatePair {
                                                            x_pos: x as i64,
                                                            y_pos: y as i64,
                                                        },
                                                    )
                                                    .map(|c| c.visited)
                                                    .unwrap_or(false)
                                            };

                                            let on_click = move |_| {
                                                let clicked_on_start = move || {
                                                    start_cell_coord()
                                                        .map(|c| (c.x_pos, c.y_pos) == (x as i64, y as i64))
                                                        .unwrap_or(false)
                                                };
                                                let clicked_on_end = move || {
                                                    end_cell_coord()
                                                        .map(|c| (c.x_pos, c.y_pos) == (x as i64, y as i64))
                                                        .unwrap_or(false)
                                                };
                                                if clicked_on_start() {
                                                    set_start_cell_coord(None);
                                                } else if clicked_on_end() {
                                                    set_end_cell_coord(None);
                                                } else if start_cell_coord().is_none() {
                                                    set_start_cell_coord(
                                                        Some(CoordinatePair {
                                                            x_pos: x as i64,
                                                            y_pos: y as i64,
                                                        }),
                                                    );
                                                } else if end_cell_coord().is_none() {
                                                    set_end_cell_coord(
                                                        Some(CoordinatePair {
                                                            x_pos: x as i64,
                                                            y_pos: y as i64,
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
                                                        "bg-red-500",
                                                        move || { is_current_cell() == true },
                                                    )
                                                    class=(
                                                        "bg-blue-500",
                                                        move || {
                                                            is_start_cell() == false && is_end_cell() == false && is_current_cell() == false
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
    let (current_cell, set_current_cell) = create_signal(None::<CoordinatePair>);

    let (start_cell_coord, set_start_cell_coord) = create_signal(None::<CoordinatePair>);
    let (end_cell_coord, set_end_cell_coord) = create_signal(None::<CoordinatePair>);
    let (current_path_candidates, set_current_path_candidates) = create_signal(VecCoordinate {
        0: Vec::<CoordinatePair>::new(),
    });

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
                current_path_candidates=current_path_candidates
                set_current_path_candidates=set_current_path_candidates
                current_cell=current_cell
                set_current_cell=set_current_cell
            />
            <SearchGrid
                grid_size=grid_size
                grid=grid
                start_cell_coord=start_cell_coord
                set_start_cell_coord
                end_cell_coord=end_cell_coord
                set_end_cell_coord=set_end_cell_coord
                current_cell=current_cell
            />
        </div>
    }
}
