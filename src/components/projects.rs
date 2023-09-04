use leptos::*;
use leptos_router::{use_location, Outlet};

#[component]
pub fn Projects(cx: Scope) -> impl IntoView {
    let location = use_location(cx);
    let pathname = move || location.pathname.get();

    let routes = vec![("life", "Conway's Game of Life")];

    view! { cx,
            <div>
                <div class="flex flex-row gap-10 mb-20">
                    <span>"Projects hosted on this site while learning Rust/Leptos"</span>
                    {routes.into_iter()
                        .map(|(route, display_text)| {
                            let is_match = move || pathname() == format!("/projects/{}", route);
                            let is_not_match = move || !is_match();

                            view! { cx,
                                <a
                                    href={route}
                                    class=("underline", is_match)
                                    class=("font-bold", is_match)
                                    class=("cursor-default", is_match)
                                    class=("cursor-pointer", is_not_match)
                                    class=("hover:underline", is_not_match)
                                    class=("no-underline", is_not_match)
                                    class=("font-medium", is_not_match)
                                >
                                    {display_text}
                                </a>
                            }
                        })
                        .collect_view(cx)}
                </div>
                <Outlet/>
        </div>
    }
}
