use crate::components::source_anchor::SourceAnchor;
use leptos::prelude::*;

#[component]
pub fn Lighthouse() -> impl IntoView {
    view! {
        <div>
            "Here is a programmatically generated lighthouse report "
            <a
                class="font-semibold underline"
                href="https://github.com/jaydh/jaydanhoward/blob/main/lighthouse/entrypoint.sh"
                target="_blank"
                rel="noreferrer"
            >
                for every deploy
            </a> "that gets kicked off as part of a k8s job for every new deploy of this site."
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
        <SourceAnchor href="#[git]" />
        <Lighthouse />
    }
}
