use leptos::*;
use std::fmt;

#[derive(Debug, Clone)]
struct Cell {
    alive: bool,
    x_pos: u32,
    y_pos: u32,
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

#[component]
pub fn Life(cx: Scope) -> impl IntoView {
    let (cells, set_cells) = create_signal::<CellVec>(cx, CellVec(Vec::new()));
    let grid_size: u32 = 5;
    let range = 0..grid_size;

    create_effect(cx, move |_| {
        cells.with(|c| log!("cells {}", c));
    });

    view! { cx,
        <div>
            "Conway's Game of Life"
            {range
                .clone()
                .map(|x| {
                    view! { cx,
                        <div class="flex flex-row">
                            {range
                                .clone()
                                .map(|y| {
                                    let isAlive = move || cells.get().0.iter().find(|&c| c.x_pos == x && c.y_pos == y).is_some();

                                    view! { cx,
                                        <div
                                            class="w-20 h-20 border-4 bg-amber-500"
                                            class=("bg-amber-500", move || isAlive() == true)
                                            on:click=move |_| {
                                                set_cells
                                                    .update(|v| {
                                                        v.0
                                                            .push(Cell {
                                                                alive: true,
                                                                x_pos: x,
                                                                y_pos: y,
                                                            });
                                                    });
                                            }
                                        >

                                            {x}
                                            {y}
                                        </div>
                                    }
                                })
                                .collect_view(cx)}
                        </div>
                    }
                })
                .collect_view(cx)}
        </div>
    }
}
