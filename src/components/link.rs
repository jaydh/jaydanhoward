use leptos::prelude::*;
use leptos_router::components::A;
use leptos_router::hooks::use_location;

#[component]
pub fn Link(path: &'static str, display_text: &'static str) -> impl IntoView {
    let location = use_location();
    let pathname = move || location.pathname.get();

    let is_match = move || pathname().contains(path);
    let is_not_match = move || !is_match();

    view! {
        <A href=path>
            <span
                class="transition-all duration-200 px-4 py-2 rounded-md relative"
                class=("text-accent", is_match)
                class=("font-semibold", is_match)
                class=("after:absolute after:bottom-0 after:left-0 after:right-0 after:h-0.5 after:bg-accent", is_match)
                class=("cursor-default", is_match)
                class=("cursor-pointer", is_not_match)
                class=("hover:text-accent", is_not_match)
                class=("font-medium", is_not_match)
            >
                {display_text}
            </span>
        </A>
    }
}
