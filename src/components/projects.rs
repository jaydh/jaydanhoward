use crate::components::link::Link;
use leptos::*;
use leptos_router::Outlet;

#[component]
pub fn Projects() -> impl IntoView {
    let routes = vec![
        ("life", "Conway's Game of Life"),
        ("path", "Path Search Visualizations"),
    ];

    view! {
        <div>
            <div class="flex flex-row gap-10 mb-20">
                {routes
                    .into_iter()
                    .map(|(path, display_text)| {
                        view! { <Link path=path display_text=display_text/> }
                    })
                    .collect_view()}
            </div>
            <Outlet/>
        </div>
    }
}
