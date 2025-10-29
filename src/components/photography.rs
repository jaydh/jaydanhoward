use crate::components::source_anchor::SourceAnchor;
use leptos::prelude::*;

#[component]
pub fn Photography() -> impl IntoView {
    // Sample image data - you can replace these with actual image paths
    let images = vec![
        ("Sample Photo 1", "/assets/profile.webp"),
        ("Sample Photo 2", "/assets/cluster.webp"),
        ("Sample Photo 3", "/assets/profile.webp"),
        ("Sample Photo 4", "/assets/cluster.webp"),
        ("Sample Photo 5", "/assets/profile.webp"),
        ("Sample Photo 6", "/assets/cluster.webp"),
    ];

    view! {
        <SourceAnchor href="#[git]" />
        <div class="max-w-7xl mx-auto px-8 w-full">
            <div class="w-full grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-6">
                    {images
                        .into_iter()
                        .map(|(title, src)| {
                            view! {
                                <div class="group relative overflow-hidden rounded-xl shadow-minimal-lg hover:shadow-minimal-xl transition-all duration-300">
                                    <div class="aspect-square overflow-hidden bg-border">
                                        <img
                                            src=src
                                            alt=title
                                            class="w-full h-full object-cover transition-transform duration-500 group-hover:scale-110"
                                        />
                                    </div>
                                    <div class="absolute inset-0 bg-gradient-to-t from-charcoal/80 via-charcoal/0 to-charcoal/0 opacity-0 group-hover:opacity-100 transition-opacity duration-300">
                                        <div class="absolute bottom-0 left-0 right-0 p-6">
                                            <p class="text-white font-medium">{title}</p>
                                        </div>
                                    </div>
                                </div>
                            }
                        })
                        .collect_view()}
            </div>
        </div>
    }
}
