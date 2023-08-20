use leptos::*;

#[component]
pub fn Lighthouse(cx: Scope) -> impl IntoView {
    view! { cx,

        <h2 class="text-semibold">r#"Lighthouse report ("#
            <a class="underline" href="https://github.com/jaydh/jaydanhoward/blob/main/lighthouse/entrypoint.sh" target="_blank" rel="noreferrer">
                generated for every deploy (performance score suffering from running chrome on small instance size)
            </a>
             r#")"#:
        </h2>
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
         <div class="flex flex-col">
             <a class="underline" href="https://github.com/jaydh/jaydanhoward" target="_blank" rel="noreferrer">Source code</a>
             <a class="underline" href="https://github.com/jaydh/jaydanhoward/actions" target="_blank" rel="noreferrer">Latest deploys</a>
         </div>
    }
}
#[component]
pub fn Dev(cx: Scope) -> impl IntoView {
    view! { cx,
        <Github />
        <Lighthouse />
    }
}
