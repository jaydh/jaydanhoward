use crate::components::source_anchor::SourceAnchor;
use leptos::*;

#[component]
pub fn Lighthouse() -> impl IntoView {
    view! {
        <div>
            r#"Programmatically generated lighthouse report("#
            <a
                class="font-semibold underline"
                href="https://github.com/jaydh/jaydanhoward/blob/main/lighthouse/entrypoint.sh"
                target="_blank"
                rel="noreferrer"
            >
                for every deploy
            </a> r#")"#
            . Note that the performance score here suffers from running chrome on small instance size, my local machine regularly get 99+.
        </div>
        <iframe
            src="/assets/lighthouse.html"
            title="Lighthouse Report"
            class="grow w-full"
        ></iframe>
    }
}

#[component]
pub fn Dev() -> impl IntoView {
    view! {
        <SourceAnchor href="#[git]"/>
        <Lighthouse/>
    }
}
