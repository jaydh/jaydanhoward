use crate::components::source_anchor::SourceAnchor;
use leptos::prelude::*;

#[component]
pub fn Photography() -> impl IntoView {
    // Sample image data - you can replace these with actual image paths
    let images: Vec<(Option<&str>, &str)> = vec![
        (
            None,
            "https://caddy.jaydanhoward.com/data/DSC00196.ARW.webp",
        ),
        (
            None,
            "https://caddy.jaydanhoward.com/data/DSC00215.ARW.webp",
        ),
        (
            None,
            "https://caddy.jaydanhoward.com/data/DSC00278.ARW.webp",
        ),
        (
            None,
            "https://caddy.jaydanhoward.com/data/DSC00279.ARW.webp",
        ),
    ];

    // State for preview modal
    let (selected_image, set_selected_image) = signal::<Option<usize>>(None);

    view! {
        <SourceAnchor href="#[git]" />
        <div class="max-w-7xl mx-auto px-8 w-full flex flex-col gap-8 items-center">
            <h1 class="text-3xl font-bold text-charcoal">
                "Photography"
            </h1>
            <div class="w-full grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-6" style="contain: layout style paint;">
                    {images
                        .iter()
                        .enumerate()
                        .map(|(idx, (title, src))| {
                            let src = *src;
                            let title = *title;
                            view! {
                                <div
                                    class="group relative overflow-hidden rounded-xl shadow-minimal-lg cursor-pointer"
                                    style="contain: layout style paint;"
                                    on:click=move |_| set_selected_image(Some(idx))
                                >
                                    <div class="aspect-square overflow-hidden bg-border">
                                        <img
                                            src=src
                                            alt=title.unwrap_or(src)
                                            loading="lazy"
                                            decoding="async"
                                            class="w-full h-full object-cover"
                                        />
                                    </div>
                                    {
                                        title.map(|t| view! {
                                            <div class="absolute inset-0 bg-gradient-to-t from-charcoal/80 via-charcoal/0 to-charcoal/0 opacity-0 group-hover:opacity-100 transition-opacity duration-300">
                                                <div class="absolute bottom-0 left-0 right-0 p-6">
                                                    <p class="text-white font-medium">{t}</p>
                                                </div>
                                            </div>
                                        })
                                    }
                                </div>
                            }
                        })
                        .collect_view()}
            </div>

            // Preview Modal
            {move || {
                selected_image().map(|idx| {
                    let (title, src) = images[idx];
                    view! {
                        <div
                            class="fixed inset-0 z-50 flex items-center justify-center bg-charcoal/90 backdrop-blur-sm animate-in fade-in duration-200"
                            on:click=move |_| set_selected_image(None)
                        >
                            <div class="relative max-w-7xl max-h-screen p-4 md:p-8">
                                <button
                                    class="absolute top-2 right-2 md:top-4 md:right-4 text-white/80 hover:text-white text-4xl font-light leading-none z-10 w-12 h-12 flex items-center justify-center rounded-full hover:bg-white/10 transition-all"
                                    on:click=move |e| {
                                        e.stop_propagation();
                                        set_selected_image(None);
                                    }
                                >
                                    "×"
                                </button>
                                <img
                                    src=src
                                    alt=title.unwrap_or(src)
                                    class="max-w-full max-h-[90vh] object-contain rounded-lg shadow-2xl"
                                    on:click=move |e| e.stop_propagation()
                                />
                                {
                                    title.map(|t| view! {
                                        <div class="absolute bottom-0 left-0 right-0 bg-gradient-to-t from-charcoal via-charcoal/50 to-transparent p-6 rounded-b-lg">
                                            <p class="text-white font-medium text-center">{t}</p>
                                        </div>
                                    })
                                }
                            </div>
                        </div>
                    }
                })
            }}
        </div>
    }
}
