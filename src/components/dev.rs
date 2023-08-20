use leptos::*;

#[component]
pub fn Lighthouse(cx: Scope) -> impl IntoView {
    view! { cx,

        <h2 class="font-semibold">r#"Programmatically generated lighthosue report("#
            <a class="underline" href="https://github.com/jaydh/jaydanhoward/blob/main/lighthouse/entrypoint.sh" target="_blank" rel="noreferrer">
                for every deploy
            </a>
             r#")"#. Note that the performance score here suffers from running chrome on small instance size, my local machine regularly get 99+.
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
