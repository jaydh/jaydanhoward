use leptos::prelude::*;

#[component]
pub fn Lighthouse() -> impl IntoView {
    let (loaded, set_loaded) = signal(false);

    view! {
        <div class="flex flex-col gap-6 h-full min-h-[50vh]">
            <div class="text-base leading-loose text-charcoal">
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
            {move || {
                if loaded.get() {
                    view! {
                        <iframe
                            src="/assets/lighthouse.html"
                            title="Lighthouse Report"
                            class="grow w-full rounded-xl border border-border shadow-minimal-lg min-h-[50vh]"
                        ></iframe>
                    }.into_any()
                } else {
                    view! {
                        <div class="grow w-full rounded-xl border border-border shadow-minimal-lg min-h-[50vh] flex items-center justify-center bg-surface-raised">
                            <button
                                class="px-4 py-2 rounded-lg bg-accent text-white text-sm font-medium hover:opacity-90 transition-opacity"
                                on:click=move |_| set_loaded.set(true)
                            >
                                "Load Report"
                            </button>
                        </div>
                    }.into_any()
                }
            }}
        </div>
    }
}

#[component]
pub fn Dev() -> impl IntoView {
    view! {
        <Lighthouse />
    }
}
