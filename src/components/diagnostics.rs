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

#[component]
pub fn Github(cx: Scope) -> impl IntoView {
    view! { cx,
         <div>
             <a class="underline" href="https://github.com/jaydh/jaydanhoward/actions">Latest deploys</a>
         </div>
    }
}
#[component]
pub fn Diagnostics(cx: Scope) -> impl IntoView {
    view! { cx,
        <Github />
        <Lighthouse />
    }
}
