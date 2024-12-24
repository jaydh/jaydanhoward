use leptos::prelude::*;
use leptos_router::hooks::use_location;

#[component]
pub fn Link(path: &'static str, display_text: &'static str) -> impl IntoView {
    let location = use_location();
    let pathname = move || location.pathname.get();

    let is_match = move || pathname().contains(&path);
    let is_not_match = move || !is_match();

    view! {
        <a
            href="&path"
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
}
