use crate::components::icons::Icon;
use leptos::prelude::*;
#[cfg(not(feature = "ssr"))]
use rand::Rng;
use std::collections::{HashMap, HashSet};
use std::fmt;

#[derive(Clone, Debug, PartialEq)]
enum Algorithm {
    Corner,
    Wall,
    Bfs,
    Dfs,
    AStar,
    Greedy,
    RandomWalk,
}

impl std::fmt::Display for Algorithm {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Algorithm::Wall => write!(f, "Wall"),
            Algorithm::Corner => write!(f, "Corner"),
            Algorithm::Bfs => write!(f, "BFS"),
            Algorithm::Dfs => write!(f, "DFS"),
            Algorithm::AStar => write!(f, "A*"),
            Algorithm::Greedy => write!(f, "Greedy Best-First"),
            Algorithm::RandomWalk => write!(f, "Random Walk"),
        }
    }
}

impl std::str::FromStr for Algorithm {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "Wall" => Ok(Algorithm::Wall),
            "Corner" => Ok(Algorithm::Corner),
            "BFS" => Ok(Algorithm::Bfs),
            "DFS" => Ok(Algorithm::Dfs),
            "A*" => Ok(Algorithm::AStar),
            "Greedy Best-First" => Ok(Algorithm::Greedy),
            "Random Walk" => Ok(Algorithm::RandomWalk),
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
    parent: Option<CoordinatePair>,
    // Track when cell was visited for visual fading
    #[allow(dead_code)]
    visit_step: Option<u64>,
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
            "is_passable: {}, x_pos: {}, y_pos: {}, visited: {}, parent: {:?}",
            self.is_passable,
            self.coordiantes.x_pos,
            self.coordiantes.y_pos,
            self.visited,
            self.parent
        )
    }
}

#[derive(Debug, Clone)]
struct VecCoordinate(Vec<CoordinatePair>);

impl fmt::Display for VecCoordinate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (i, item) in self.0.iter().enumerate() {
            writeln!(f, "Item {i}: {item}")?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
struct Grid(HashMap<CoordinatePair, Cell>);

#[cfg(not(feature = "ssr"))]
fn randomize_cells(
    obstacle_probability: f64,
    grid_size: u64,
    set_grid: WriteSignal<Grid>,
    set_start: WriteSignal<Option<CoordinatePair>>,
    set_end: WriteSignal<Option<CoordinatePair>>,
) {
    let mut rng = rand::thread_rng();
    let mut grid = Grid(HashMap::new());
    let mut passable_cells = Vec::new();

    for x in 0..grid_size {
        for y in 0..grid_size {
            let coord = CoordinatePair {
                x_pos: x as i64,
                y_pos: y as i64,
            };
            let is_passable = rng.gen::<f64>() > obstacle_probability;

            if is_passable {
                passable_cells.push(coord);
            }

            grid.0.insert(
                coord,
                Cell {
                    coordiantes: coord,
                    is_passable,
                    visited: false,
                    parent: None,
                    visit_step: None,
                },
            );
        }
    }

    // Pick random start and end from passable cells
    if passable_cells.len() >= 2 {
        let start_idx = rng.gen_range(0..passable_cells.len());
        let start = passable_cells[start_idx];
        set_start(Some(start));

        // Pick a different cell for end
        let mut end_idx = rng.gen_range(0..passable_cells.len());
        while end_idx == start_idx && passable_cells.len() > 1 {
            end_idx = rng.gen_range(0..passable_cells.len());
        }
        let end = passable_cells[end_idx];
        set_end(Some(end));
    }

    set_grid(grid);
}

#[cfg(not(feature = "ssr"))]
fn find_shortest_path_in_visited(
    start: CoordinatePair,
    end: CoordinatePair,
    grid: &HashMap<CoordinatePair, Cell>,
    visited_cells: &HashSet<CoordinatePair>,
) -> HashSet<CoordinatePair> {
    use std::collections::VecDeque;

    let mut queue = VecDeque::new();
    let mut visited = HashSet::new();
    let mut parent_map: HashMap<CoordinatePair, CoordinatePair> = HashMap::new();

    queue.push_back(start);
    visited.insert(start);

    while let Some(current) = queue.pop_front() {
        if current == end {
            // Found the end, backtrack to build path
            let mut path = HashSet::new();
            let mut curr = end;
            path.insert(curr);

            while let Some(&p) = parent_map.get(&curr) {
                path.insert(p);
                curr = p;
                if curr == start {
                    break;
                }
            }

            return path;
        }

        // Explore neighbors (only within visited cells)
        for (dx, dy) in [(0, 1), (0, -1), (1, 0), (-1, 0)] {
            let neighbor = CoordinatePair {
                x_pos: current.x_pos + dx,
                y_pos: current.y_pos + dy,
            };

            if let Some(cell) = grid.get(&neighbor) {
                // Only explore if the cell was visited by the original algorithm
                if cell.is_passable
                    && visited_cells.contains(&neighbor)
                    && !visited.contains(&neighbor)
                {
                    visited.insert(neighbor);
                    parent_map.insert(neighbor, current);
                    queue.push_back(neighbor);
                }
            }
        }
    }

    // No path found, return empty set
    HashSet::new()
}

#[cfg(not(feature = "ssr"))]
fn distance(coord1: &CoordinatePair, coord2: &CoordinatePair) -> f64 {
    let dx = (coord1.x_pos - coord2.x_pos) as f64;
    let dy = (coord1.y_pos - coord2.y_pos) as f64;
    (dx * dx + dy * dy).sqrt()
}

#[cfg(not(feature = "ssr"))]
fn manhattan_distance(coord1: &CoordinatePair, coord2: &CoordinatePair) -> i64 {
    (coord1.x_pos - coord2.x_pos).abs() + (coord1.y_pos - coord2.y_pos).abs()
}

#[cfg(not(feature = "ssr"))]
fn distance_to_closest_walls(point: &CoordinatePair, grid_size: ReadSignal<u64>) -> i64 {
    let distance_left = point.x_pos;
    let distance_right = grid_size.get_untracked() as i64 - point.x_pos - 1;
    let distance_top = point.y_pos;
    let distance_bottom = grid_size.get_untracked() as i64 - point.y_pos - 1;

    *[distance_left, distance_right, distance_top, distance_bottom]
        .iter()
        .min()
        .unwrap()
}

#[cfg(not(feature = "ssr"))]
fn add_candidates(
    grid_size: ReadSignal<u64>,
    current_cell: ReadSignal<Option<CoordinatePair>>,
    grid: ReadSignal<Grid>,
    current_path_candidates: ReadSignal<VecCoordinate>,
    set_current_path_candidates: WriteSignal<VecCoordinate>,
) {
    let viable_neighbors = [(0, 1), (0, -1), (1, 0), (-1, 0)]
        .map(|(x, y)| {
            grid.get_untracked()
                .0
                .get(&CoordinatePair {
                    x_pos: current_cell.get_untracked().unwrap().x_pos + x,
                    y_pos: current_cell.get_untracked().unwrap().y_pos + y,
                })
                .cloned()
        })
        .into_iter()
        .filter(|n| {
            n.as_ref()
                .map(|c| {
                    c.is_passable
                        && !c.visited
                        && !current_path_candidates
                            .get_untracked()
                            .0
                            .contains(&c.coordiantes)
                })
                .unwrap_or(false)
        })
        .map(|cell| cell.unwrap().coordiantes)
        .collect::<Vec<CoordinatePair>>();

    let mut corners = [
        CoordinatePair { x_pos: 0, y_pos: 0 },
        CoordinatePair {
            x_pos: 0,
            y_pos: grid_size.get_untracked() as i64,
        },
        CoordinatePair {
            x_pos: grid_size.get_untracked() as i64,
            y_pos: 0,
        },
        CoordinatePair {
            x_pos: grid_size.get_untracked() as i64,
            y_pos: grid_size.get_untracked() as i64,
        },
    ];
    corners.sort_by(|a, b| {
        let distance_a = distance(&current_cell.get_untracked().unwrap(), a);
        let distance_b = distance(&current_cell.get_untracked().unwrap(), b);
        distance_a.partial_cmp(&distance_b).unwrap()
    });

    set_current_path_candidates.update(|path| {
        if viable_neighbors.iter().any(|c| {
            distance(corners.first().unwrap(), c)
                < distance(
                    corners.first().unwrap(),
                    &current_cell.get_untracked().unwrap(),
                )
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

#[cfg(not(feature = "ssr"))]
fn add_candidates_walls(
    grid_size: ReadSignal<u64>,
    current_cell: ReadSignal<Option<CoordinatePair>>,
    grid: ReadSignal<Grid>,
    current_path_candidates: ReadSignal<VecCoordinate>,
    set_current_path_candidates: WriteSignal<VecCoordinate>,
) {
    let viable_neighbors = [(0, 1), (0, -1), (1, 0), (-1, 0)]
        .map(|(x, y)| {
            grid.get_untracked()
                .0
                .get(&CoordinatePair {
                    x_pos: current_cell.get_untracked().unwrap().x_pos + x,
                    y_pos: current_cell.get_untracked().unwrap().y_pos + y,
                })
                .cloned()
        })
        .into_iter()
        .filter(|n| {
            n.as_ref()
                .map(|c| {
                    c.is_passable
                        && !c.visited
                        && !current_path_candidates
                            .get_untracked()
                            .0
                            .contains(&c.coordiantes)
                })
                .unwrap_or(false)
        })
        .map(|cell| cell.unwrap().coordiantes)
        .collect::<Vec<CoordinatePair>>();

    let mut corners = [
        CoordinatePair { x_pos: 0, y_pos: 0 },
        CoordinatePair {
            x_pos: 0,
            y_pos: grid_size.get_untracked() as i64,
        },
        CoordinatePair {
            x_pos: grid_size.get_untracked() as i64,
            y_pos: 0,
        },
        CoordinatePair {
            x_pos: grid_size.get_untracked() as i64,
            y_pos: grid_size.get_untracked() as i64,
        },
    ];
    corners.sort_by(|a, b| {
        let distance_a = distance(&current_cell.get_untracked().unwrap(), a);
        let distance_b = distance(&current_cell.get_untracked().unwrap(), b);
        distance_a.partial_cmp(&distance_b).unwrap()
    });

    set_current_path_candidates.update(|path| {
        path.0.extend(viable_neighbors);
        path.0.sort_by(|a, b| {
            let a_wall_distance = distance_to_closest_walls(a, grid_size);
            let b_wall_distance = distance_to_closest_walls(b, grid_size);

            if a_wall_distance == b_wall_distance {
                let distance_a = distance(&corners[0], a);
                let distance_b = distance(&corners[0], b);
                distance_b.partial_cmp(&distance_a).unwrap()
            } else {
                b_wall_distance.partial_cmp(&a_wall_distance).unwrap()
            }
        });
    });
}

#[cfg(not(feature = "ssr"))]
#[allow(clippy::too_many_arguments)]
fn calculate_next(
    grid_size: ReadSignal<u64>,
    grid: ReadSignal<Grid>,
    set_grid: WriteSignal<Grid>,
    start_cell_coord: ReadSignal<Option<CoordinatePair>>,
    end_cell_coord: ReadSignal<Option<CoordinatePair>>,
    current_path_candidates: ReadSignal<VecCoordinate>,
    set_current_path_candidates: WriteSignal<VecCoordinate>,
    current_cell: ReadSignal<Option<CoordinatePair>>,
    set_current_cell: WriteSignal<Option<CoordinatePair>>,
    algorithm: ReadSignal<Algorithm>,
    step_count: ReadSignal<u64>,
) {
    if current_cell.get_untracked().is_none() {
        // For other algorithms, set current_cell and mark as visited
        set_current_cell(start_cell_coord.get_untracked());
        set_grid.update(|grid| {
            let cell = grid
                .0
                .get_mut(&start_cell_coord.get_untracked().unwrap())
                .unwrap();
            cell.visited = true;
            cell.parent = None;
            cell.visit_step = Some(0);
        });
    } else {
        match algorithm.get_untracked() {
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
            Algorithm::Bfs => {
                // BFS: mark visited when adding to queue (FIFO needs this)
                let viable_neighbors = [(0, 1), (0, -1), (1, 0), (-1, 0)]
                    .iter()
                    .filter_map(|(x, y)| {
                        let coord = CoordinatePair {
                            x_pos: current_cell.get_untracked().unwrap().x_pos + x,
                            y_pos: current_cell.get_untracked().unwrap().y_pos + y,
                        };
                        grid.get_untracked().0.get(&coord).and_then(|c| {
                            if c.is_passable && !c.visited {
                                Some(coord)
                            } else {
                                None
                            }
                        })
                    })
                    .collect::<Vec<CoordinatePair>>();

                // Mark all neighbors as visited NOW and set their parent
                let current_cell_value = current_cell.get_untracked();
                let current_step = step_count.get_untracked();
                for neighbor in &viable_neighbors {
                    set_grid.update(|grid| {
                        if let Some(cell) = grid.0.get_mut(neighbor) {
                            if !cell.visited {
                                cell.visited = true;
                                cell.parent = current_cell_value;
                                cell.visit_step = Some(current_step);
                            }
                        }
                    });
                }

                set_current_path_candidates.update(|path| {
                    path.0.extend(viable_neighbors);
                });
            }
            Algorithm::Dfs => {
                // DFS: don't mark when adding, let pop logic handle it (LIFO behavior)
                // Explore clockwise: up, right, down, left
                // Since we pop from back (LIFO), add in reverse: left, down, right, up
                let viable_neighbors = [(-1, 0), (0, 1), (1, 0), (0, -1)]
                    .iter()
                    .filter_map(|(x, y)| {
                        let coord = CoordinatePair {
                            x_pos: current_cell.get_untracked().unwrap().x_pos + x,
                            y_pos: current_cell.get_untracked().unwrap().y_pos + y,
                        };
                        grid.get_untracked().0.get(&coord).and_then(|c| {
                            if c.is_passable && !c.visited {
                                Some(coord)
                            } else {
                                None
                            }
                        })
                    })
                    .collect::<Vec<CoordinatePair>>();

                set_current_path_candidates.update(|path| {
                    path.0.extend(viable_neighbors);
                });
            }
            Algorithm::RandomWalk => {
                // Random Walk: explore neighbors in random order (allows backtracking)
                let mut viable_neighbors = [(0, 1), (0, -1), (1, 0), (-1, 0)]
                    .iter()
                    .filter_map(|(x, y)| {
                        let coord = CoordinatePair {
                            x_pos: current_cell.get_untracked().unwrap().x_pos + x,
                            y_pos: current_cell.get_untracked().unwrap().y_pos + y,
                        };
                        grid.get_untracked().0.get(&coord).and_then(|c| {
                            if c.is_passable && !c.visited {
                                Some(coord)
                            } else {
                                None
                            }
                        })
                    })
                    .collect::<Vec<CoordinatePair>>();

                // Shuffle neighbors using coordinate-based pseudo-random ordering
                // This gives random exploration with backtracking capability
                let curr = current_cell.get_untracked().unwrap();
                viable_neighbors.sort_by_key(|coord| {
                    ((coord.x_pos * 73856093)
                        ^ (coord.y_pos * 19349663)
                        ^ (curr.x_pos * 83492791)) as usize
                });

                set_current_path_candidates.update(|path| {
                    path.0.extend(viable_neighbors);
                });
            }
            Algorithm::AStar | Algorithm::Greedy => {
                // A* / Greedy: don't mark when adding, let pop logic handle it
                let mut viable_neighbors = [(0, 1), (0, -1), (1, 0), (-1, 0)]
                    .iter()
                    .filter_map(|(x, y)| {
                        let coord = CoordinatePair {
                            x_pos: current_cell.get_untracked().unwrap().x_pos + x,
                            y_pos: current_cell.get_untracked().unwrap().y_pos + y,
                        };
                        grid.get_untracked().0.get(&coord).and_then(|c| {
                            if c.is_passable && !c.visited {
                                Some(coord)
                            } else {
                                None
                            }
                        })
                    })
                    .collect::<Vec<CoordinatePair>>();

                // Sort by heuristic (Manhattan distance to goal) - higher distance last so we pop best first
                if let Some(end) = end_cell_coord.get_untracked() {
                    viable_neighbors.sort_by_key(|coord| -(manhattan_distance(coord, &end)));
                }

                set_current_path_candidates.update(|path| {
                    path.0.extend(viable_neighbors);
                    // Keep sorted by heuristic
                    if let Some(end) = end_cell_coord.get_untracked() {
                        path.0
                            .sort_by_key(|coord| -(manhattan_distance(coord, &end)));
                    }
                });
            }
        };
        // Pop next cell from queue
        if matches!(algorithm.get_untracked(), Algorithm::Bfs) {
            // BFS: pop from front (FIFO)
            if let Some(next_visit_coord) =
                current_path_candidates.get_untracked().0.first().copied()
            {
                set_current_path_candidates.update(|path| {
                    if !path.0.is_empty() {
                        path.0.remove(0);
                    }
                });
                set_current_cell(Some(next_visit_coord));
            }
        } else {
            // DFS, A*, Greedy, Corner, Wall, RandomWalk: pop from back (LIFO)
            loop {
                let next_visit_coord = current_path_candidates.get_untracked().0.last().copied();

                if let Some(next_visit_coord) = next_visit_coord {
                    let is_visited = grid
                        .get_untracked()
                        .0
                        .get(&next_visit_coord)
                        .map(|c| c.visited)
                        .unwrap_or(true);

                    set_current_path_candidates.update(|path| {
                        path.0.pop();
                    });

                    if is_visited {
                        continue;
                    }

                    let previous_cell = current_cell.get_untracked();
                    let current_step = step_count.get_untracked();
                    set_current_cell(Some(next_visit_coord));
                    set_grid.update(|grid| {
                        let cell = grid.0.get_mut(&next_visit_coord).unwrap();
                        cell.visited = true;
                        cell.parent = previous_cell;
                        cell.visit_step = Some(current_step);
                    });
                    break;
                } else {
                    break;
                }
            }
        }
    }
}

#[component]
fn SearchGrid(
    #[allow(unused_variables)] grid_size: ReadSignal<u64>,
    #[allow(unused_variables)] grid: ReadSignal<Grid>,
    #[allow(unused_variables)] start_cell_coord: ReadSignal<Option<CoordinatePair>>,
    #[allow(unused_variables)] end_cell_coord: ReadSignal<Option<CoordinatePair>>,
    #[allow(unused_variables)] current_cell: ReadSignal<Option<CoordinatePair>>,
    #[allow(unused_variables)] final_path: ReadSignal<HashSet<CoordinatePair>>,
    #[allow(unused_variables)] step_count: ReadSignal<u64>,
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

            closure.forget();
        });
    }

    // Render canvas
    #[cfg(not(feature = "ssr"))]
    Effect::new(move |_| {
        let _ = resize_trigger();

        let Some(canvas) = canvas_ref.get() else {
            return;
        };

        let canvas_element: &web_sys::HtmlCanvasElement = canvas.as_ref();
        let parent = canvas_element.parent_element().unwrap();
        let container_width = parent.client_width() as f64;

        let window = web_sys::window().unwrap();
        let window_height = window.inner_height().unwrap().as_f64().unwrap();

        let grid_sz = grid_size();

        // Set minimum cell size based on grid size for visibility
        let min_cell_px = if grid_sz <= 50 {
            8.0
        } else if grid_sz <= 100 {
            5.0
        } else {
            3.0 // Smaller minimum for very large grids
        };

        // For layout, aim for canvas that's roughly 1/3 of container width (to fit 3 side by side)
        // Or full container width on mobile
        let target_width = if container_width < 768.0 {
            container_width * 0.9 // Mobile: nearly full width
        } else {
            (container_width / 3.2).min(600.0) // Desktop: ~1/3 width, max 600px
        };

        let max_height = window_height * 0.5;
        let available_size = target_width.min(max_height).max(250.0);

        let cell_px = (available_size / grid_sz as f64).floor().max(min_cell_px);
        let canvas_size = (cell_px * grid_sz as f64) as u32;

        canvas.set_width(canvas_size);
        canvas.set_height(canvas_size);

        let context = canvas
            .get_context("2d")
            .unwrap()
            .unwrap()
            .unchecked_into::<web_sys::CanvasRenderingContext2d>();

        // Check dark mode
        let is_dark = web_sys::window()
            .and_then(|w| w.document())
            .and_then(|d| d.document_element())
            .map(|el| el.class_list().contains("dark"))
            .unwrap_or(false);

        // Clear canvas
        let bg_color = if is_dark { "#111827" } else { "#f9fafb" };
        context.set_fill_style_str(bg_color);
        context.fill_rect(0.0, 0.0, canvas_size as f64, canvas_size as f64);

        let current_grid = grid();
        let start = start_cell_coord();
        let end = end_cell_coord();
        let current = current_cell();
        let path = final_path();
        let current_step = step_count();

        // Draw all cells
        for x in 0..grid_sz {
            for y in 0..grid_sz {
                let coord = CoordinatePair {
                    x_pos: x as i64,
                    y_pos: y as i64,
                };

                let cell = current_grid.0.get(&coord);
                let is_passable = cell.map(|c| c.is_passable).unwrap_or(false);
                let is_visited = cell.map(|c| c.visited).unwrap_or(false);
                let visit_step = cell.and_then(|c| c.visit_step);
                let is_start = start.map(|c| coord == c).unwrap_or(false);
                let is_end = end.map(|c| coord == c).unwrap_or(false);
                let is_current = current.map(|c| coord == c).unwrap_or(false);
                let in_final_path = path.contains(&coord);

                // Calculate fade factor based on visit recency (0.0 = old, 1.0 = recent)
                let fade_window = 50.0; // Number of steps to fade over
                let fade_factor = if let Some(v_step) = visit_step {
                    let steps_ago = current_step.saturating_sub(v_step) as f64;
                    (1.0 - (steps_ago / fade_window)).max(0.3) // Minimum 30% brightness
                } else {
                    1.0
                };

                let color = if !is_passable {
                    if is_dark { "#9CA3AF".to_string() } else { "#1f2937".to_string() }
                } else if is_start {
                    "#22c55e".to_string() // green-500 - start
                } else if is_end {
                    "#f59e0b".to_string() // amber-500 - end
                } else if is_current {
                    "#ef4444".to_string() // red-500 - current
                } else if in_final_path {
                    "#c084fc".to_string() // purple-400 - path
                } else if is_visited {
                    if is_dark {
                        format!("rgba(96, 165, 250, {fade_factor})")
                    } else {
                        format!("rgba(147, 197, 253, {fade_factor})")
                    }
                } else {
                    bg_color.to_string()
                };

                context.set_fill_style_str(&color);
                context.fill_rect(x as f64 * cell_px, y as f64 * cell_px, cell_px, cell_px);
            }
        }

        // Draw grid lines (only for larger cells)
        if cell_px >= 5.0 {
            context.set_stroke_style_str(if is_dark { "#374151" } else { "#d1d5db" });
            context.set_line_width(1.0);
            for i in 0..=grid_sz {
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

        // Draw special markers for start and end (always visible)
        if let Some(s) = start {
            let center_x = s.x_pos as f64 * cell_px + cell_px / 2.0;
            let center_y = s.y_pos as f64 * cell_px + cell_px / 2.0;
            let marker_size = (cell_px * 0.7).clamp(6.0, 20.0);

            context.set_fill_style_str("#22c55e"); // green
            context.begin_path();
            context
                .arc(
                    center_x,
                    center_y,
                    marker_size / 2.0,
                    0.0,
                    2.0 * std::f64::consts::PI,
                )
                .unwrap();
            context.fill();

            // Border for visibility
            context.set_stroke_style_str(if is_dark { "#111827" } else { "#ffffff" });
            context.set_line_width(2.0);
            context.stroke();
        }

        if let Some(e) = end {
            let center_x = e.x_pos as f64 * cell_px + cell_px / 2.0;
            let center_y = e.y_pos as f64 * cell_px + cell_px / 2.0;
            let marker_size = (cell_px * 0.7).clamp(6.0, 20.0);

            context.set_fill_style_str("#f59e0b"); // amber
            context.begin_path();
            context
                .arc(
                    center_x,
                    center_y,
                    marker_size / 2.0,
                    0.0,
                    2.0 * std::f64::consts::PI,
                )
                .unwrap();
            context.fill();

            // Border for visibility
            context.set_stroke_style_str(if is_dark { "#111827" } else { "#ffffff" });
            context.set_line_width(2.0);
            context.stroke();
        }
    });

    #[cfg(not(feature = "ssr"))]
    return view! {
        <canvas
            node_ref=canvas_ref
            class="border border-border"
        ></canvas>
    };

    #[cfg(feature = "ssr")]
    view! {
        <canvas
            class="border border-border"
        ></canvas>
    }
}

#[component]
#[allow(unused_variables)]
fn AlgorithmSimulation(
    algorithm: Algorithm,
    grid_size: ReadSignal<u64>,
    shared_grid: ReadSignal<Grid>,
    #[allow(unused_variables)] set_shared_grid: WriteSignal<Grid>,
    start_cell_coord: ReadSignal<Option<CoordinatePair>>,
    end_cell_coord: ReadSignal<Option<CoordinatePair>>,
    is_running: ReadSignal<bool>,
    completion_order: ReadSignal<Vec<Algorithm>>,
    set_completion_order: WriteSignal<Vec<Algorithm>>,
) -> impl IntoView {
    // Each algorithm has its own copy of the grid and simulation state
    let (grid, set_grid) = signal(Grid(HashMap::new()));
    let (current_cell, set_current_cell) = signal(None::<CoordinatePair>);
    let (current_path_candidates, set_current_path_candidates) =
        signal(VecCoordinate(Vec::<CoordinatePair>::new()));
    let (final_path, set_final_path) = signal(HashSet::<CoordinatePair>::new());
    let (fps, set_fps) = signal(0.0);
    let (step_count, set_step_count) = signal(0_u64);
    let (completion_steps, set_completion_steps) = signal(None::<u64>);

    // Clone the shared grid when it changes
    #[cfg(not(feature = "ssr"))]
    {
        Effect::new(move |_| {
            set_grid(shared_grid().clone());
        });
    }

    // Fast simulation loop using requestAnimationFrame for smooth visual updates
    #[cfg(not(feature = "ssr"))]
    let (frame_count, set_frame_count) = signal(0);

    #[cfg(not(feature = "ssr"))]
    let (algo_signal, _) = signal(algorithm.clone());

    #[cfg(not(feature = "ssr"))]
    {
        use std::cell::RefCell;
        use std::rc::Rc;
        use wasm_bindgen::prelude::*;
        use wasm_bindgen::JsCast;

        Effect::new(move |_| {
            if !is_running() {
                return;
            }

            let window = web_sys::window().unwrap();

            // Create the animation loop
            let animate = Rc::new(RefCell::new(None::<Closure<dyn FnMut()>>));
            let animate_clone = animate.clone();

            let closure = Closure::wrap(Box::new(move || {
                if !is_running.get_untracked() || completion_steps.get_untracked().is_some() {
                    return;
                }

                // Run multiple steps per frame for better performance
                const STEPS_PER_FRAME: u32 = 3;

                for _ in 0..STEPS_PER_FRAME {
                    // Check if already complete
                    if completion_steps.get_untracked().is_some() {
                        break;
                    }

                    // Increment step counter
                    set_step_count.update(|c| *c += 1);

                    // Update FPS (steps per second)
                    set_frame_count.update(|c| *c += 1);
                    if frame_count.get_untracked() >= 100 {
                        set_fps(frame_count.get_untracked() as f64);
                        set_frame_count(0);
                    }

                    // Check if simulation is complete
                    // For other algorithms, we complete when we reach the end cell
                    let is_complete = current_cell.get_untracked()
                        == end_cell_coord.get_untracked()
                        && current_cell.get_untracked().is_some();

                    if is_complete {
                        // Record completion steps
                        set_completion_steps(Some(step_count.get_untracked()));

                        let path = {
                            // DFS/A*/Greedy/Corner/Wall: find shortest path within visited cells only
                            let current_grid = grid.get_untracked();
                            let visited_cells: HashSet<CoordinatePair> = current_grid
                                .0
                                .iter()
                                .filter(|(_, cell)| cell.visited)
                                .map(|(coord, _)| *coord)
                                .collect();

                            find_shortest_path_in_visited(
                                start_cell_coord.get_untracked().unwrap(),
                                end_cell_coord.get_untracked().unwrap(),
                                &current_grid.0,
                                &visited_cells,
                            )
                        };

                        set_final_path(path);

                        // Record completion order
                        let algo = algo_signal.get_untracked();
                        set_completion_order.update(|order| {
                            if !order.contains(&algo) {
                                order.push(algo);
                            }
                        });

                        break;
                    }

                    let should_continue = {
                        // For other algorithms: stop when we reach the end
                        current_cell.get_untracked() != end_cell_coord.get_untracked()
                            && completion_steps.get_untracked().is_none()
                    };

                    if should_continue {
                        calculate_next(
                            grid_size,
                            grid,
                            set_grid,
                            start_cell_coord,
                            end_cell_coord,
                            current_path_candidates,
                            set_current_path_candidates,
                            current_cell,
                            set_current_cell,
                            algo_signal,
                            step_count,
                        );
                    }
                }

                // Schedule next frame
                if is_running.get_untracked() && completion_steps.get_untracked().is_none() {
                    let window = web_sys::window().unwrap();
                    if let Some(ref closure) = *animate_clone.borrow() {
                        window
                            .request_animation_frame(closure.as_ref().unchecked_ref())
                            .unwrap();
                    }
                }
            }) as Box<dyn FnMut()>);

            // Store the closure and start the loop
            *animate.borrow_mut() = Some(closure);

            if let Some(ref closure) = *animate.borrow() {
                window
                    .request_animation_frame(closure.as_ref().unchecked_ref())
                    .unwrap();
            };
        });
    }

    // Reset state when shared_grid changes
    #[cfg(not(feature = "ssr"))]
    {
        Effect::new(move |_| {
            let _ = shared_grid();
            // Always reset when grid changes
            set_current_cell(None);
            set_current_path_candidates(VecCoordinate(Vec::new()));
            set_final_path(HashSet::new());
            set_frame_count(0);
            set_fps(0.0);
            set_step_count(0);
            set_completion_steps(None);
        });
    }

    let algo = algorithm.clone();

    view! {
        <div class="flex flex-col gap-2 items-center">
            <div class="flex flex-col items-center gap-1">
                <div class="flex items-center gap-2">
                    <h3 class="text-sm font-medium text-charcoal">{algorithm.to_string()}</h3>
                    {move || {
                        let order = completion_order();
                        order.iter().position(|a| a == &algo).map(|pos| {
                            let rank_text = match pos {
                                0 => "🥇 1st".to_string(),
                                1 => "🥈 2nd".to_string(),
                                2 => "🥉 3rd".to_string(),
                                3 => "4th".to_string(),
                                4 => "5th".to_string(),
                                5 => "6th".to_string(),
                                6 => "7th".to_string(),
                                _ => format!("{}th", pos + 1),
                            };
                            view! {
                                <span class="text-xs font-bold text-accent">{rank_text}</span>
                            }
                        })
                    }}
                </div>
                {move || {
                    completion_steps().map(|steps| {
                        view! {
                            <span class="text-xs text-charcoal-light font-mono">
                                {format!("{steps} steps")}
                            </span>
                        }
                    })
                }}
            </div>
            <SearchGrid
                grid_size=grid_size
                grid=grid
                start_cell_coord=start_cell_coord
                end_cell_coord=end_cell_coord
                current_cell=current_cell
                final_path=final_path
                step_count=step_count
            />
            <div class="text-xs text-charcoal-light font-mono min-h-[1.5rem]">
                {move || if is_running() && fps() > 0.0 {
                    format!("{:.0} steps/s", fps())
                } else {
                    String::from(" ")
                }}
            </div>
        </div>
    }
}

#[component]
#[allow(unused_variables)]
pub fn PathSearch() -> impl IntoView {
    let (grid_size, set_grid_size) = signal(75_u64);
    let (obstacle_probability, set_obstacle_probability) = signal(0.2);
    let (show_settings, set_show_settings) = signal(false);

    // Initialize empty grid - will be randomized on mount
    let initial_grid = Grid(HashMap::new());
    let (grid, set_grid) = signal(initial_grid);

    let (start_cell_coord, set_start_cell_coord) = signal(None::<CoordinatePair>);
    let (end_cell_coord, set_end_cell_coord) = signal(None::<CoordinatePair>);
    let (is_running, set_is_running) = signal(false); // Will auto-start when visible
    let (blind_completion_order, set_blind_completion_order) = signal(Vec::<Algorithm>::new());
    let (informed_completion_order, set_informed_completion_order) =
        signal(Vec::<Algorithm>::new());

    let container_ref = NodeRef::<leptos::html::Div>::new();

    // Randomize grid on initial mount and when grid size changes
    #[cfg(not(feature = "ssr"))]
    {
        use leptos::prelude::Effect;
        Effect::new(move |prev: Option<()>| {
            let _ = grid_size(); // Track grid_size changes
            randomize_cells(
                obstacle_probability(),
                grid_size(),
                set_grid,
                set_start_cell_coord,
                set_end_cell_coord,
            );
            // Only reset state on subsequent changes (not on initial mount)
            if prev.is_some() {
                set_is_running(false);
            }
            set_blind_completion_order(Vec::new());
            set_informed_completion_order(Vec::new());
        });
    }

    // Auto-start when element comes into view
    #[cfg(not(feature = "ssr"))]
    {
        use std::cell::RefCell;
        use std::rc::Rc;
        use wasm_bindgen::prelude::*;
        use wasm_bindgen::JsCast;

        let has_started = Rc::new(RefCell::new(false));

        Effect::new(move |_| {
            let Some(container) = container_ref.get() else {
                return;
            };

            let has_started = has_started.clone();

            // Create IntersectionObserver to detect when element is visible
            let callback = Closure::wrap(Box::new(
                move |entries: js_sys::Array, _observer: web_sys::IntersectionObserver| {
                    for entry in entries.iter() {
                        let entry: web_sys::IntersectionObserverEntry = entry.unchecked_into();

                        if entry.is_intersecting() {
                            if !*has_started.borrow() {
                                *has_started.borrow_mut() = true;
                            }
                            // Start simulation when visible
                            set_is_running(true);
                        } else {
                            // Pause simulation when not visible
                            set_is_running(false);
                        }
                    }
                },
            )
                as Box<dyn FnMut(js_sys::Array, web_sys::IntersectionObserver)>);

            let observer =
                web_sys::IntersectionObserver::new(callback.as_ref().unchecked_ref()).unwrap();
            observer.observe(&container);

            callback.forget();
        });
    }

    let toggle_simulation = move |_| {
        set_is_running.update(|r| *r = !*r);
    };

    #[cfg(not(feature = "ssr"))]
    let randomize = move |_| {
        set_is_running(false);
        randomize_cells(
            obstacle_probability(),
            grid_size(),
            set_grid,
            set_start_cell_coord,
            set_end_cell_coord,
        );
        set_blind_completion_order(Vec::new());
        set_informed_completion_order(Vec::new());
    };

    #[cfg(feature = "ssr")]
    let randomize = move |_| {};

    view! {
        <div
            node_ref=container_ref
            class="max-w-7xl mx-auto px-8 w-full flex flex-col gap-8 items-center relative"
        >
            <h1 class="text-3xl font-bold text-charcoal">
                "Pathfinding Algorithms"
            </h1>

            <div class="flex gap-3 items-center">
                <button
                    class="px-4 py-1.5 text-sm rounded border transition-all duration-200 hover:bg-accent hover:bg-opacity-10"
                    style:border-color="#3B82F6"
                    style:color="#3B82F6"
                    on:click=toggle_simulation
                    aria-label=move || if is_running() { "Pause simulation" } else { "Play simulation" }
                >
                    {move || if is_running() { "▌▌" } else { "▶" }}
                </button>
                <button
                    class="px-4 py-1.5 text-sm rounded border transition-all duration-200 hover:bg-accent hover:bg-opacity-10"
                    style:border-color="#3B82F6"
                    style:color="#3B82F6"
                    on:click=randomize
                    aria-label="Randomize grid"
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
            </div>

            <Show when=move || show_settings()>
                <div class="absolute top-32 right-8 z-20 bg-surface border border-border rounded-lg shadow-minimal-lg p-6 min-w-[320px]">
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
                                    if let Ok(val) = event_target_value(&ev).parse::<u64>() {
                                        set_grid_size(val);
                                    }
                                }
                                prop:value=grid_size
                            />
                        </div>

                        <div class="flex flex-col gap-2">
                            <label for="obstacle_probability" class="text-sm font-medium text-charcoal">
                                "Obstacle Probability"
                            </label>
                            <input
                                type="number"
                                id="obstacle_probability"
                                step="0.1"
                                min="0"
                                max="1"
                                class="px-3 py-2 rounded border border-border bg-surface text-charcoal focus:outline-none focus:ring-2 focus:ring-accent transition-all"
                                on:input=move |ev| {
                                    if let Ok(val) = event_target_value(&ev).parse::<f64>() {
                                        set_obstacle_probability(val);
                                    }
                                }
                                prop:value=obstacle_probability
                            />
                        </div>
                    </div>
                </div>
            </Show>

            <div class="text-sm text-charcoal-light max-w-4xl mx-auto mb-8">
                "Pathfinding algorithms racing to find the shortest route from start (green) to end (yellow)."
            </div>

            // Blind Search Algorithms
            <div class="w-full flex flex-col gap-6 mb-12">
                <div class="flex flex-col gap-2 items-center">
                    <h2 class="text-2xl font-bold text-charcoal">"Blind Search"</h2>
                    <p class="text-sm text-charcoal-light max-w-2xl text-center">
                        "Explores without knowing the destination location"
                    </p>
                </div>
                <div class="w-full flex flex-wrap gap-8 justify-center items-start">
                    <AlgorithmSimulation
                        algorithm=Algorithm::Bfs
                        grid_size=grid_size
                        shared_grid=grid
                        set_shared_grid=set_grid
                        start_cell_coord=start_cell_coord
                        end_cell_coord=end_cell_coord
                        is_running=is_running
                        completion_order=blind_completion_order
                        set_completion_order=set_blind_completion_order
                    />
                    <AlgorithmSimulation
                        algorithm=Algorithm::Dfs
                        grid_size=grid_size
                        shared_grid=grid
                        set_shared_grid=set_grid
                        start_cell_coord=start_cell_coord
                        end_cell_coord=end_cell_coord
                        is_running=is_running
                        completion_order=blind_completion_order
                        set_completion_order=set_blind_completion_order
                    />
                    <AlgorithmSimulation
                        algorithm=Algorithm::Corner
                        grid_size=grid_size
                        shared_grid=grid
                        set_shared_grid=set_grid
                        start_cell_coord=start_cell_coord
                        end_cell_coord=end_cell_coord
                        is_running=is_running
                        completion_order=blind_completion_order
                        set_completion_order=set_blind_completion_order
                    />
                    <AlgorithmSimulation
                        algorithm=Algorithm::Wall
                        grid_size=grid_size
                        shared_grid=grid
                        set_shared_grid=set_grid
                        start_cell_coord=start_cell_coord
                        end_cell_coord=end_cell_coord
                        is_running=is_running
                        completion_order=blind_completion_order
                        set_completion_order=set_blind_completion_order
                    />
                    <AlgorithmSimulation
                        algorithm=Algorithm::RandomWalk
                        grid_size=grid_size
                        shared_grid=grid
                        set_shared_grid=set_grid
                        start_cell_coord=start_cell_coord
                        end_cell_coord=end_cell_coord
                        is_running=is_running
                        completion_order=blind_completion_order
                        set_completion_order=set_blind_completion_order
                    />
                </div>
            </div>

            // Informed Search Algorithms
            <div class="w-full flex flex-col gap-6">
                <div class="flex flex-col gap-2 items-center">
                    <h2 class="text-2xl font-bold text-charcoal">"Informed Search"</h2>
                    <p class="text-sm text-charcoal-light max-w-2xl text-center">
                        "Uses heuristics or knowledge of the destination to guide exploration more efficiently"
                    </p>
                </div>
                <div class="w-full flex flex-wrap gap-8 justify-center items-start">
                    <AlgorithmSimulation
                        algorithm=Algorithm::AStar
                        grid_size=grid_size
                        shared_grid=grid
                        set_shared_grid=set_grid
                        start_cell_coord=start_cell_coord
                        end_cell_coord=end_cell_coord
                        is_running=is_running
                        completion_order=informed_completion_order
                        set_completion_order=set_informed_completion_order
                    />
                    <AlgorithmSimulation
                        algorithm=Algorithm::Greedy
                        grid_size=grid_size
                        shared_grid=grid
                        set_shared_grid=set_grid
                        start_cell_coord=start_cell_coord
                        end_cell_coord=end_cell_coord
                        is_running=is_running
                        completion_order=informed_completion_order
                        set_completion_order=set_informed_completion_order
                    />
                </div>
            </div>
        </div>
    }
}
