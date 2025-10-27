use crate::components::dev::Lighthouse;
use leptos::prelude::*;

#[component]
pub fn Work() -> impl IntoView {
    view! {
        <div class="max-w-7xl mx-auto px-8 py-16 w-full flex flex-col gap-16">
            <section class="flex flex-col gap-6">
                <h2 class="text-2xl font-bold text-charcoal">Projects</h2>
                <div class="flex flex-col gap-4 text-base leading-loose text-charcoal opacity-90">
                    <p>
                        <a href="/work/life" class="text-accent hover:underline transition-colors duration-200 font-medium">
                            "Conway's Game of Life"
                        </a>
                        " - Interactive cellular automaton simulation"
                    </p>
                    <p>
                        <a href="/work/path" class="text-accent hover:underline transition-colors duration-200 font-medium">
                            "Path Search Visualizations"
                        </a>
                        " - Visualize pathfinding algorithms"
                    </p>
                </div>
            </section>

            <section class="flex flex-col gap-6">
                <h2 class="text-2xl font-bold text-charcoal">Performance</h2>
                <Lighthouse />
            </section>
        </div>
    }
}
