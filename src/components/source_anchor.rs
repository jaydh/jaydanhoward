use leptos::*;

#[component]
pub fn SourceAnchor(href: &'static str) -> impl IntoView {
    view! {
        <div class="group fixed bottom-12 right-12 ">
            <a
                class="fas fa-code"
                href=href
                target="_blank"
                rel="noreferrer"
            ></a>
            <span class="absolute -top-20 -left-20 pointer-events-none opacity-0 transition-opacity group-hover:opacity-100">
                Self-Referencing Source Code
            </span>
        </div>
    }
}
