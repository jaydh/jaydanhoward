use leptos::*;

#[component]
pub fn SourceAnchor(cx: Scope, href: &'static str) -> impl IntoView {
    view! {
            cx,
            <Show when= move || href != "#[git]" fallback=|_| ()>
                <div class="group">
                    <a class="animate-bounce fixed bottom-12 right-12 fas fa-code" href=href target="_blank" rel="noreferrer" />
                    <span
                      class="fixed bottom-16 right-12 pointer-events-none opacity-0 transition-opacity group-hover:opacity-100"
                    >
                           Self-Referencing Source Code
                    </span>
                </div>
            </Show>
    }
}
