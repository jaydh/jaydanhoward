use leptos::*;

#[component]
pub fn Lighthouse(cx: Scope) -> impl IntoView {
    view! { cx,
        <div>
            "Programmatically generated lighthouse report("
            <a
                class="font-semibold underline"
                href="https://github.com/jaydh/jaydanhoward/blob/main/lighthouse/entrypoint.sh"
                target="_blank"
                rel="noreferrer"
            >
                for every deploy
            </a> ")"
            . Note that the performance score here suffers from running chrome on small instance size, my local machine regularly get 99+.
        </div>
        <iframe
            src="/assets/lighthouse.html"
            title="Lighthouse Report"
            class="w-full h-4/6"
        ></iframe>
    }
}

#[component]
pub fn Github(cx: Scope) -> impl IntoView {
    view! { cx,
        <div class="flex flex-col font-semibold">
            <a
                class="underline"
                href="https://github.com/jaydh/jaydanhoward"
                target="_blank"
                rel="noreferrer"
            >
                Source code
            </a>
            <a
                class="underline"
                href="https://github.com/jaydh/jaydanhoward/actions"
                target="_blank"
                rel="noreferrer"
            >
                Latest deploys
            </a>
        </div>
    }
}
#[component]
pub fn Dev(cx: Scope) -> impl IntoView {
    view! { cx,
        <a class="fixed bottom-0 right-0 p-12 fas fa-code" href="#[git]"></a>
        <Github/>
        <Lighthouse/>
    }
}
