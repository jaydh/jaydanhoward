use leptos::*;

#[component]
pub fn Lighthouse(cx: Scope) -> impl IntoView {
    view! { cx,
        <iframe
            src="assets/lighthouse.html"
            title="Lighthouse Report"
            class="w-full h-4/6"
        />
    }
}
