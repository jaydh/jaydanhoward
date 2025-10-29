use crate::components::source_anchor::SourceAnchor;
use leptos::prelude::*;

#[component]
pub fn Lighthouse() -> impl IntoView {
    view! {
        <div class="flex flex-col gap-6 h-full min-h-[50vh]">
            <div class="text-base leading-loose text-charcoal opacity-90">
                "Here is a programmatically generated lighthouse report "
                <a
                    class="text-accent hover:underline transition-colors duration-200"
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
                class="grow w-full rounded-xl border border-border shadow-minimal-lg"
            ></iframe>
        </div>
    }
}

#[component]
pub fn Dev() -> impl IntoView {
    view! {
        <SourceAnchor href="#[git]" />
        <Lighthouse />
    }
}
