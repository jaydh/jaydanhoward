use crate::components::icons::Icon;
use leptos::prelude::*;

#[component]
pub fn SourceAnchor(href: &'static str) -> impl IntoView {
    view! {
        <div class="group fixed bottom-8 right-8 aria-hidden='true' z-50">
            <a
                class="flex items-center justify-center w-12 h-12 rounded-full bg-accent text-white hover:bg-accent-dark transition-all duration-200 shadow-minimal-lg hover:shadow-minimal-xl"
                href=href
                target="_blank"
                rel="noreferrer"
                aria-label="source_anchor"
            >
                <Icon name="code" class="w-5 h-5" />
            </a>
            <span class="absolute bottom-full mb-2 right-0 pointer-events-none opacity-0 transition-opacity group-hover:opacity-100 bg-surface border border-border text-charcoal px-3 py-2 rounded-lg shadow-minimal-lg whitespace-nowrap text-sm">
                View Source Code
            </span>
        </div>
    }
}
