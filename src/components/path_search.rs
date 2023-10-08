use crate::components::source_anchor::SourceAnchor;
use leptos::*;
use leptos_dom::helpers::IntervalHandle;
use rand::Rng;
use std::collections::HashMap;
use std::fmt;

#[derive(Clone, Debug)]
enum Algorithm {
    Corner,
    Wall,
}

impl std::fmt::Display for Algorithm {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Algorithm::Wall => write!(f, "Wall"),
            Algorithm::Corner => write!(f, "Corner"),
        }
    }
}

impl std::str::FromStr for Algorithm {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "Wall" => Ok(Algorithm::Wall),
            "Corner" => Ok(Algorithm::Corner),
            _ => Err(()),
        }
    }
}

#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash)]
struct CoordinatePair {
    x_pos: i64,
    y_pos: i64,
}

impl std::ops::Add for CoordinatePair {
    type Output = CoordinatePair;

    fn add(self, other: CoordinatePair) -> CoordinatePair {
        CoordinatePair {
            x_pos: self.x_pos + other.x_pos,
            y_pos: self.y_pos + other.y_pos,
        }
    }
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

fn distance(coord1: &CoordinatePair, coord2: &CoordinatePair) -> f64 {
    let dx = (coord1.x_pos - coord2.x_pos) as f64;
    let dy = (coord1.y_pos - coord2.y_pos) as f64;
    (dx * dx + dy * dy).sqrt()
}

fn distance_to_walls(point: &CoordinatePair, grid_size: ReadSignal<u64>) -> (i64, i64, i64, i64) {
    let distance_left = point.x_pos;
    let distance_right = grid_size() as i64 - point.x_pos - 1;
    let distance_top = point.y_pos;
    let distance_bottom = grid_size() as i64 - point.y_pos - 1;

    (distance_left, distance_right, distance_top, distance_bottom)
}

fn add_candidates(
    grid_size: ReadSignal<u64>,
    current_cell: ReadSignal<Option<CoordinatePair>>,
    grid: ReadSignal<Grid>,
    current_path_candidates: ReadSignal<VecCoordinate>,
    set_current_path_candidates: WriteSignal<VecCoordinate>,
) {
    let viable_neighbors = [(0, 1), (0, -1), (1, 0), (-1, 0)]
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
        let distance_a = distance(&current_cell().unwrap(), a);
        let distance_b = distance(&current_cell().unwrap(), b);
        distance_a.partial_cmp(&distance_b).unwrap()
    });

    set_current_path_candidates.update(|path| {
        logging::log!("path len {}", path.0.len());

        if viable_neighbors.iter().any(|c| {
            distance(&corners.first().unwrap(), c)
                < distance(&corners.first().unwrap(), &current_cell().unwrap())
        }) {
            path.0.extend(viable_neighbors);
            path.0.sort_by(|a, b| {
                let distance_a = distance(&corners[0], a);
                let distance_b = distance(&corners[0], b);
                distance_b.partial_cmp(&distance_a).unwrap()
            });
        } else {
            path.0.extend(viable_neighbors);
            path.0.sort_by(|a, b| {
                let distance_a = distance(&corners[1], a);
                let distance_b = distance(&corners[1], b);
                distance_b.partial_cmp(&distance_a).unwrap()
            });
        }
    });
}

fn add_candidates_walls(
    grid_size: ReadSignal<u64>,
    current_cell: ReadSignal<Option<CoordinatePair>>,
    grid: ReadSignal<Grid>,
    current_path_candidates: ReadSignal<VecCoordinate>,
    set_current_path_candidates: WriteSignal<VecCoordinate>,
) {
    let viable_neighbors = [(0, 1), (0, -1), (1, 0), (-1, 0)]
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
        path.0.extend(viable_neighbors);
        path.0.sort_by_key(|point| {
            let (left, right, top, bottom) = distance_to_walls(point, grid_size);
            *[left, right, top, bottom].iter().max().unwrap()
        });
    });
}

fn calculate_next(
    grid_size: ReadSignal<u64>,
    grid: ReadSignal<Grid>,
    set_grid: WriteSignal<Grid>,
    start_cell_coord: ReadSignal<Option<CoordinatePair>>,
    current_path_candidates: ReadSignal<VecCoordinate>,
    set_current_path_candidates: WriteSignal<VecCoordinate>,
    current_cell: ReadSignal<Option<CoordinatePair>>,
    set_current_cell: WriteSignal<Option<CoordinatePair>>,
    algorithm: ReadSignal<Algorithm>,
) {
    if current_cell().is_none() {
        set_current_cell(start_cell_coord());
        set_grid.update(|grid| {
            let cell = grid.0.get_mut(&start_cell_coord().unwrap()).unwrap();
            cell.visited = true;
        });
    } else {
        match algorithm() {
            Algorithm::Wall => add_candidates_walls(
                grid_size,
                current_cell,
                grid,
                current_path_candidates,
                set_current_path_candidates,
            ),
            Algorithm::Corner => add_candidates(
                grid_size,
                current_cell,
                grid,
                current_path_candidates,
                set_current_path_candidates,
            ),
        };
        if let Some(next_visit_coord) = current_path_candidates().0.last() {
            set_current_path_candidates.update(|path| {
                path.0.pop();
            });

            set_current_cell(Some(*next_visit_coord));
            set_grid.update(|grid| {
                let cell = grid.0.get_mut(&next_visit_coord).unwrap();
                cell.visited = true;
            });
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
    current_path_candidates: ReadSignal<VecCoordinate>,
    set_current_path_candidates: WriteSignal<VecCoordinate>,
    current_cell: ReadSignal<Option<CoordinatePair>>,
    set_current_cell: WriteSignal<Option<CoordinatePair>>,
    algorithm: ReadSignal<Algorithm>,
    set_algorithm: WriteSignal<Algorithm>,
) -> impl IntoView {
    let (interval_handle, set_interval_handle) = create_signal(None::<IntervalHandle>);
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
                    grid,
                    set_grid,
                    start_cell_coord,
                    current_path_candidates,
                    set_current_path_candidates,
                    current_cell,
                    set_current_cell,
                    algorithm,
                )
            },
            std::time::Duration::from_millis(interval_ms()),
        );
        set_interval_handle(interval_handle.ok());
    };

    view! {
        <div class="flex flex-row space-x-10 mb-10">
            <div class="flex flex-col text-charcoal dark:text-gray">
                <label for="grid_size">
                    Grid Size
                </label>
                <input
                    type="text"
                    id="grid_size"
                    on:input=move |ev| {
                        set_grid_size(event_target_value(&ev).parse::<u64>().unwrap());
                    }

                    prop:value=grid_size
                />
                <label for="obstacle_probability">
                    Obstacle probability
                </label>
                <input
                    type="text"
                    id="obstacle_probability"
                    on:input=move |ev| {
                        set_obstacle_probability(event_target_value(&ev).parse::<f64>().unwrap());
                    }

                    prop:value=obstacle_probability
                />
                <label for="interval_time">
                    Simulation speed in ms
                </label>
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
                <select name="algorithm" on:change= move |ev| { set_algorithm(event_target_value(&ev).parse::<Algorithm>().unwrap()); }>
                    <option value="">
                        --Please choose an algorithm--
                    </option>
                    <option value=Algorithm::Corner.to_string()>
                        {Algorithm::Corner.to_string()}
                    </option>
                    <option value=Algorithm::Wall.to_string()>
                        {Algorithm::Wall.to_string()}
                    </option>
                </select>
                <button on:click=move |_| {
                    if let Some(handle) = interval_handle() {
                        handle.clear();
                    }
                    randomize_cells(obstacle_probability(), grid_size(), set_grid)
                }>
                    Randomize
                </button>
                <button on:click=move |_| {
                    create_simulation_interval();
                }>
                    Simulate
                </button>

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
                                                    class=("bg-red-500", move || { is_current_cell() == true })

                                                    class=(
                                                        "bg-blue-500",
                                                        move || {
                                                            is_start_cell() == false && is_end_cell() == false
                                                                && is_current_cell() == false && is_visited() == true
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
    let (algorithm, set_algorithm) = create_signal(Algorithm::Wall);

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
                algorithm=algorithm
                set_algorithm=set_algorithm
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
